// page_stats.rs - page summaries and scan pruning (NF-12).
//
// Every stored page carries one summary per column: the smallest value, the
// largest value, the null count, and the value count. A predicate of the form
// `column OP literal` is tested against the summary before the page is
// decoded. A page that cannot hold a matching row is skipped, so the decoder
// never runs for it.
//
// The summary is conservative. When a comparison is impossible (a type
// mismatch, an unknown column, or a compound expression) the page is kept and
// the row filter decides. The row filter always runs after the page scan, so a
// wrong skip is a correctness bug and a kept page is only a cost.

use std::cmp::Ordering;

use crate::eval;
use crate::eval::value;
use crate::model::{BinaryOp, Column, Expr, Result, Row, Value};

/// The summary of one column inside one page.
#[derive(Clone, Debug, Default)]
pub struct ColumnStats {
    /// The smallest non-null value, or None when every value is null.
    pub min: Option<Value>,
    /// The largest non-null value, or None when every value is null.
    pub max: Option<Value>,
    /// How many values are null.
    pub nulls: usize,
    /// How many values the page holds.
    pub values: usize,
}

/// The summary of one page, one entry per table column.
#[derive(Clone, Debug, Default)]
pub struct PageStats {
    pub columns: Vec<ColumnStats>,
}

impl PageStats {
    /// Build the summary from the rows of one page.
    pub fn build(columns: &[Column], rows: &[Row]) -> Result<PageStats> {
        let mut summaries = vec![ColumnStats { values: rows.len(), ..ColumnStats::default() }; columns.len()];
        for row in rows {
            if row.len() != columns.len() {
                return Err("page row width does not match schema".into());
            }
            for (summary, value) in summaries.iter_mut().zip(row) {
                if matches!(value, Value::Null) {
                    summary.nulls += 1;
                    continue;
                }
                summary.min = Some(match summary.min.take() {
                    Some(current) if value::compare(value, &current)? != Ordering::Less => current,
                    _ => value.clone(),
                });
                summary.max = Some(match summary.max.take() {
                    Some(current) if value::compare(value, &current)? != Ordering::Greater => current,
                    _ => value.clone(),
                });
            }
        }
        Ok(PageStats { columns: summaries })
    }

    /// True when the page can hold a row that passes the predicate.
    pub fn might_match(&self, predicate: &PagePredicate) -> bool {
        let Some(column) = self.columns.get(predicate.column()) else {
            return true;
        };
        match predicate {
            PagePredicate::Compare { op, literal, .. } => {
                let (Some(min), Some(max)) = (&column.min, &column.max) else {
                    // Every value is null, and a comparison with null is null.
                    return false;
                };
                let lower = value::compare(min, literal).ok();
                let upper = value::compare(max, literal).ok();
                match op {
                    BinaryOp::Eq => {
                        matches!((lower, upper), (Some(l), Some(u)) if l != Ordering::Greater && u != Ordering::Less)
                    }
                    BinaryOp::Ne => {
                        // Only a page of one non-null value rules out `<>`.
                        if column.nulls > 0 || column.values == 0 {
                            return true;
                        }
                        !(lower == Some(Ordering::Equal) && upper == Some(Ordering::Equal))
                    }
                    BinaryOp::Lt => lower == Some(Ordering::Less),
                    BinaryOp::Le => matches!(lower, Some(order) if order != Ordering::Greater),
                    BinaryOp::Gt => upper == Some(Ordering::Greater),
                    BinaryOp::Ge => matches!(upper, Some(order) if order != Ordering::Less),
                    _ => true,
                }
            }
            PagePredicate::IsNull { negated, .. } => {
                if *negated {
                    column.values > column.nulls
                } else {
                    column.nulls > 0
                }
            }
        }
    }
}

/// One page-level clause compiled from a query predicate.
#[derive(Clone, Debug)]
pub enum PagePredicate {
    /// `column OP literal`.
    Compare { column: usize, op: BinaryOp, literal: Value },
    /// `column IS NULL` or `column IS NOT NULL`.
    IsNull { column: usize, negated: bool },
}

impl PagePredicate {
    /// The table column index the clause reads.
    pub fn column(&self) -> usize {
        match self {
            PagePredicate::Compare { column, .. } | PagePredicate::IsNull { column, .. } => *column,
        }
    }
}

/// The page-level form of a conjunction of query predicates.
#[derive(Clone, Debug, Default)]
pub struct PagePlan {
    predicates: Vec<PagePredicate>,
}

impl PagePlan {
    /// Compile the clauses that the summary can test. A clause that the summary
    /// cannot test is dropped, and the page is kept for it.
    pub fn compile(expressions: &[Expr], columns: &[String]) -> PagePlan {
        PagePlan {
            predicates: expressions
                .iter()
                .filter_map(|expr| classify(expr, columns))
                .collect(),
        }
    }

    /// How many clauses the summary tests.
    pub fn clauses(&self) -> usize {
        self.predicates.len()
    }

    /// True when the page can hold a row that passes every clause.
    pub fn might_match(&self, stats: &PageStats) -> bool {
        self.predicates.iter().all(|predicate| stats.might_match(predicate))
    }
}

fn classify(expr: &Expr, columns: &[String]) -> Option<PagePredicate> {
    match expr {
        Expr::Binary { left, op, right } => {
            let compare = matches!(
                op,
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge
            );
            if !compare {
                return None;
            }
            match (left.as_ref(), right.as_ref()) {
                (Expr::Column { .. }, Expr::Literal(literal)) => Some(PagePredicate::Compare {
                    column: resolve(left, columns)?,
                    op: *op,
                    literal: literal.clone(),
                }),
                (Expr::Literal(literal), Expr::Column { .. }) => Some(PagePredicate::Compare {
                    column: resolve(right, columns)?,
                    op: flip(*op),
                    literal: literal.clone(),
                }),
                _ => None,
            }
        }
        Expr::IsNull { expr: inner, negated } => match inner.as_ref() {
            Expr::Column { .. } => Some(PagePredicate::IsNull {
                column: resolve(inner, columns)?,
                negated: *negated,
            }),
            _ => None,
        },
        _ => None,
    }
}

fn resolve(expr: &Expr, columns: &[String]) -> Option<usize> {
    let Expr::Column { relation, name } = expr else {
        return None;
    };
    eval::resolve(relation.as_deref(), name, columns).ok()
}

/// Flip an operator when the literal is on the left: `5 < x` is `x > 5`.
fn flip(op: BinaryOp) -> BinaryOp {
    match op {
        BinaryOp::Lt => BinaryOp::Gt,
        BinaryOp::Le => BinaryOp::Ge,
        BinaryOp::Gt => BinaryOp::Lt,
        BinaryOp::Ge => BinaryOp::Le,
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn columns() -> Vec<Column> {
        vec![
            Column { name: "id".into(), data_type: crate::model::DataType::Int, nullable: false },
            Column { name: "score".into(), data_type: crate::model::DataType::Float, nullable: true },
        ]
    }

    fn rows() -> Vec<Row> {
        vec![
            vec![Value::Int(10), Value::Float(1.5)],
            vec![Value::Int(20), Value::Null],
            vec![Value::Int(30), Value::Float(2.5)],
        ]
    }

    fn plan(text: &str) -> PagePlan {
        let query = crate::sql::parse(&format!("SELECT id FROM t WHERE {text}")).expect("parse");
        let crate::model::Statement::Query(query) = &query[0] else {
            panic!("query");
        };
        let Some(filter) = &query.filter else {
            panic!("filter");
        };
        let columns = vec!["t.id".to_string(), "t.score".to_string()];
        PagePlan::compile(std::slice::from_ref(filter), &columns)
    }

    #[test]
    fn the_summary_reports_range_and_nulls() {
        let stats = PageStats::build(&columns(), &rows()).expect("stats");
        assert_eq!(stats.columns[0].min, Some(Value::Int(10)));
        assert_eq!(stats.columns[0].max, Some(Value::Int(30)));
        assert_eq!(stats.columns[1].min, Some(Value::Float(1.5)));
        assert_eq!(stats.columns[1].max, Some(Value::Float(2.5)));
        assert_eq!(stats.columns[1].nulls, 1);
        assert_eq!(stats.columns[1].values, 3);
    }

    #[test]
    fn a_range_predicate_prunes_a_page_outside_the_range() {
        assert!(plan("id > 5").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(!plan("id > 30").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(!plan("id < 10").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(plan("id >= 30").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(!plan("id <= 9").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(plan("id = 20").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(!plan("id = 31").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
    }

    #[test]
    fn a_null_predicate_uses_the_null_count() {
        assert!(plan("score IS NULL").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(plan("score IS NOT NULL").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(!plan("id IS NULL").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
    }

    #[test]
    fn a_literal_on_the_left_flips_the_operator() {
        assert!(!plan("30 < id").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
        assert!(plan("10 < id").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
    }

    #[test]
    fn a_compound_predicate_is_not_compiled() {
        assert_eq!(plan("id + 1 > 100").clauses(), 0);
        assert_eq!(plan("id > 5").clauses(), 1);
    }

    #[test]
    fn a_ne_predicate_keeps_every_page_that_holds_two_values() {
        assert!(plan("id <> 10").might_match(&PageStats::build(&columns(), &rows()).unwrap()));
    }
}
