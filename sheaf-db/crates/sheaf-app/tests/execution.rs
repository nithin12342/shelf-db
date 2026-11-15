// Compile the execution modules directly so these tests do not require a binary entry point.
#[path = "../src/aggregate.rs"]
mod aggregate;
#[path = "../src/eval.rs"]
mod eval;
#[path = "../src/executor.rs"]
mod executor;
#[path = "../src/model.rs"]
mod model;
#[path = "../src/relational_plan.rs"]
mod relational_plan;

use model::*;
use std::cell::Cell;
use std::collections::HashMap;

#[derive(Default)]
struct Source {
    tables: HashMap<String, (Vec<Column>, Vec<Row>)>,
    reads: Cell<usize>,
}
impl Source {
    fn table(mut self, name: &str, columns: &[(&str, DataType)], rows: Vec<Row>) -> Self {
        self.tables.insert(
            name.into(),
            (
                columns.iter().map(|(name, data_type)| Column { name: (*name).into(), data_type: *data_type, nullable: true }).collect(),
                rows,
            ),
        );
        self
    }
}
impl DataSource for Source {
    fn columns(&self, table: &str) -> Result<Vec<Column>> {
        Ok(self.tables.get(table).ok_or("Missing table")?.0.clone())
    }
    fn rows(&self, table: &str) -> Result<Vec<Row>> {
        self.reads.set(self.reads.get() + 1);
        Ok(self.tables.get(table).ok_or("Missing table")?.1.clone())
    }
}
fn col(name: &str) -> Expr {
    let (relation, name) = name.rsplit_once('.').map(|(r, n)| (Some(r.into()), n)).unwrap_or((None, name));
    Expr::Column { relation, name: name.into() }
}
fn lit(value: Value) -> Expr {
    Expr::Literal(value)
}
fn int(value: i64) -> Expr {
    lit(Value::Int(value))
}
fn bin(left: Expr, op: BinaryOp, right: Expr) -> Expr {
    Expr::Binary { left: Box::new(left), op, right: Box::new(right) }
}
fn agg(function: Aggregate, expr: Option<Expr>, distinct: bool) -> Expr {
    Expr::Aggregate { function, expr: expr.map(Box::new), distinct }
}
fn project(expr: Expr, alias: Option<&str>) -> Projection {
    Projection { expr, alias: alias.map(str::to_owned) }
}
fn relation(table: &str, alias: &str) -> Relation {
    Relation { table: table.into(), alias: alias.into() }
}
fn query(projection: Vec<Expr>) -> Query {
    Query {
        from: relation("items", "i"),
        joins: Vec::new(),
        filter: None,
        projection: projection.into_iter().map(|expr| project(expr, None)).collect(),
        group_by: Vec::new(),
        order_by: Vec::new(),
        limit: None,
        offset: 0,
        distinct: false,
    }
}
fn order(expr: Expr, ascending: bool, nulls_first: Option<bool>) -> Order {
    Order { expr, ascending, nulls_first }
}
fn both(source: &Source, query: &Query, expected: Vec<Row>) {
    let baseline = executor::execute(source, query, false).unwrap();
    let optimized = executor::execute(source, query, true).unwrap();
    assert_eq!(baseline.rows, expected);
    assert_eq!(optimized.rows, expected);
    assert_eq!(baseline.columns, optimized.columns);
    assert_eq!(baseline.stats.output_rows, expected.len());
}
fn items(rows: Vec<Row>) -> Source {
    Source::default().table("items", &[("x", DataType::Int)], rows)
}

#[test]
fn duplicate_join_nulls_pushdown_and_full_on() {
    let source = Source::default()
        .table(
            "items",
            &[("id", DataType::Int), ("value", DataType::Int)],
            vec![
                vec![Value::Int(1), Value::Int(10)],
                vec![Value::Int(1), Value::Int(10)],
                vec![Value::Null, Value::Int(8)],
                vec![Value::Int(2), Value::Int(20)],
            ],
        )
        .table(
            "right",
            &[("pad", DataType::Int), ("id", DataType::Int), ("value", DataType::Int)],
            vec![
                vec![Value::Int(90), Value::Int(1), Value::Int(11)],
                vec![Value::Int(91), Value::Int(1), Value::Int(11)],
                vec![Value::Int(92), Value::Int(1), Value::Int(9)],
                vec![Value::Int(93), Value::Null, Value::Int(12)],
                vec![Value::Int(94), Value::Int(2), Value::Int(30)],
            ],
        );
    let mut q = query(vec![col("i.id"), col("r.value")]);
    q.joins.push(Join {
        relation: relation("right", "r"),
        on: bin(bin(col("r.id"), BinaryOp::Eq, col("i.id")), BinaryOp::And, bin(col("r.value"), BinaryOp::Gt, col("i.value"))),
    });
    q.filter = Some(bin(col("r.value"), BinaryOp::Lt, int(20)));
    both(&source, &q, vec![vec![Value::Int(1), Value::Int(11)]; 4]);
    let plain = executor::execute(&source, &q, false).unwrap();
    let fast = executor::execute(&source, &q, true).unwrap();
    assert!(fast.stats.candidate_rows < plain.stats.candidate_rows);
    assert!(fast.plan.iter().any(|s| s.contains("hash-index")));
    assert!(fast.plan.iter().any(|s| s.contains("Pushdown (r.value")));
}

#[test]
fn multi_join_keeps_original_column_order() {
    let source = Source::default()
        .table("items", &[("x", DataType::Int)], vec![vec![Value::Int(2)], vec![Value::Int(1)]])
        .table(
            "b",
            &[("dummy", DataType::Int), ("x", DataType::Int)],
            vec![vec![Value::Int(8), Value::Int(1)], vec![Value::Int(9), Value::Int(2)]],
        )
        .table("c", &[("x", DataType::Int)], vec![vec![Value::Int(1)], vec![Value::Int(2)]]);
    let mut q = query(vec![col("i.x"), col("b.dummy"), col("c.x")]);
    q.joins.push(Join { relation: relation("b", "b"), on: bin(col("i.x"), BinaryOp::Eq, col("b.x")) });
    q.joins.push(Join { relation: relation("c", "c"), on: bin(col("c.x"), BinaryOp::Eq, col("b.x")) });
    q.filter = Some(bin(col("b.dummy"), BinaryOp::Gt, int(8)));
    both(&source, &q, vec![vec![Value::Int(2), Value::Int(9), Value::Int(2)]]);
}

#[test]
fn non_equality_join_uses_nested_loop() {
    let source = items(vec![vec![Value::Int(1)], vec![Value::Int(2)]]);
    let mut q = query(vec![col("i.x"), col("r.x")]);
    q.joins.push(Join { relation: relation("items", "r"), on: bin(col("i.x"), BinaryOp::Lt, col("r.x")) });
    both(&source, &q, vec![vec![Value::Int(1), Value::Int(2)]]);
    assert!(executor::explain(&source, &q, true).unwrap().iter().any(|line| line.contains("nested-loop")));
}

#[test]
fn grouped_aggregates_preserve_counts_and_null_group() {
    let source = Source::default().table(
        "items",
        &[("g", DataType::Text), ("x", DataType::Int)],
        vec![
            vec![Value::Text("a".into()), Value::Int(2)],
            vec![Value::Text("a".into()), Value::Int(2)],
            vec![Value::Text("a".into()), Value::Null],
            vec![Value::Null, Value::Int(4)],
            vec![Value::Null, Value::Int(6)],
        ],
    );
    let mut q = query(vec![
        col("g"),
        agg(Aggregate::Count, None, false),
        agg(Aggregate::Count, Some(col("x")), false),
        agg(Aggregate::Count, Some(col("x")), true),
        agg(Aggregate::Sum, Some(col("x")), false),
        agg(Aggregate::Avg, Some(col("x")), false),
        agg(Aggregate::Min, Some(col("x")), false),
        agg(Aggregate::Max, Some(col("x")), false),
    ]);
    q.group_by = vec![col("g")];
    q.order_by = vec![order(col("g"), true, None)];
    both(
        &source,
        &q,
        vec![
            vec![
                Value::Text("a".into()),
                Value::Int(3),
                Value::Int(2),
                Value::Int(1),
                Value::Int(4),
                Value::Float(2.0),
                Value::Int(2),
                Value::Int(2),
            ],
            vec![Value::Null, Value::Int(2), Value::Int(2), Value::Int(2), Value::Int(10), Value::Float(5.0), Value::Int(4), Value::Int(6)],
        ],
    );
}

#[test]
fn empty_and_all_null_aggregates() {
    let q = query(vec![
        agg(Aggregate::Count, Some(col("*")), false),
        agg(Aggregate::Sum, Some(col("x")), false),
        agg(Aggregate::Avg, Some(col("x")), false),
        agg(Aggregate::Min, Some(col("x")), false),
        agg(Aggregate::Max, Some(col("x")), false),
    ]);
    both(&items(vec![]), &q, vec![vec![Value::Int(0), Value::Null, Value::Null, Value::Null, Value::Null]]);
    both(&items(vec![vec![Value::Null]]), &q, vec![vec![Value::Int(1), Value::Null, Value::Null, Value::Null, Value::Null]]);
    let mut grouped = query(vec![col("x"), agg(Aggregate::Count, None, false)]);
    grouped.group_by = vec![col("x")];
    both(&items(vec![]), &grouped, vec![]);
}

#[test]
fn aggregate_expressions_and_distinct_sum() {
    let source = items(vec![vec![Value::Int(1)], vec![Value::Int(1)], vec![Value::Int(3)]]);
    let q = query(vec![bin(agg(Aggregate::Sum, Some(col("x")), true), BinaryOp::Add, int(2)), agg(Aggregate::Avg, Some(col("x")), true)]);
    both(&source, &q, vec![vec![Value::Int(6), Value::Float(2.0)]]);
    let mut q = query(vec![bin(col("x"), BinaryOp::Add, int(1)), agg(Aggregate::Count, None, false)]);
    q.group_by = vec![bin(col("i.x"), BinaryOp::Add, int(1))];
    both(&source, &q, vec![vec![Value::Int(2), Value::Int(2)], vec![Value::Int(4), Value::Int(1)]]);
}

#[test]
fn order_alias_expression_distinct_offset_limit() {
    let source = items(vec![vec![Value::Int(2)], vec![Value::Null], vec![Value::Int(1)], vec![Value::Int(2)], vec![Value::Int(3)]]);
    let mut q = query(vec![bin(col("x"), BinaryOp::Add, int(1))]);
    q.projection[0].alias = Some("next".into());
    q.order_by = vec![order(col("next"), false, None)];
    q.distinct = true;
    q.offset = 1;
    q.limit = Some(2);
    both(&source, &q, vec![vec![Value::Int(4)], vec![Value::Int(3)]]);
    q.order_by = vec![order(bin(col("x"), BinaryOp::Add, int(1)), true, Some(true))];
    both(&source, &q, vec![vec![Value::Int(2)], vec![Value::Int(3)]]);
    q.offset = usize::MAX;
    both(&source, &q, vec![]);
    q.offset = 0;
    q.limit = Some(0);
    both(&source, &q, vec![]);
}

#[test]
fn default_null_order_and_alias_priority() {
    let source = items(vec![vec![Value::Null], vec![Value::Int(1)], vec![Value::Int(2)]]);
    let mut q = query(vec![col("x")]);
    q.order_by = vec![order(col("x"), true, None)];
    both(&source, &q, vec![vec![Value::Int(1)], vec![Value::Int(2)], vec![Value::Null]]);
    q.order_by = vec![order(col("x"), false, None)];
    both(&source, &q, vec![vec![Value::Null], vec![Value::Int(2)], vec![Value::Int(1)]]);
    q.order_by[0].nulls_first = Some(false);
    both(&source, &q, vec![vec![Value::Int(2)], vec![Value::Int(1)], vec![Value::Null]]);
    q.projection = vec![project(bin(int(0), BinaryOp::Sub, col("i.x")), Some("x"))];
    q.order_by = vec![order(col("x"), true, None)];
    both(&source, &q, vec![vec![Value::Int(-2)], vec![Value::Int(-1)], vec![Value::Null]]);
}

#[test]
fn wildcard_expansion_and_duplicate_alias_errors() {
    let source = items(vec![vec![Value::Int(1)]]);
    let mut q = query(vec![col("*"), col("r.*")]);
    q.joins.push(Join { relation: relation("items", "r"), on: lit(Value::Bool(true)) });
    both(&source, &q, vec![vec![Value::Int(1), Value::Int(1), Value::Int(1)]]);
    q.joins[0].relation.alias = "i".into();
    assert!(executor::explain(&source, &q, true).is_err());
    let mut q = query(vec![col("x"), int(1)]);
    for p in &mut q.projection {
        p.alias = Some("dup".into());
    }
    assert!(executor::explain(&source, &q, true).is_err());
    assert!(executor::explain(&source, &query(vec![col("missing.*")]), true).is_err());
}

#[test]
fn metadata_validation_happens_before_any_data_read() {
    let source = items(vec![]);
    for optimize in [false, true] {
        assert!(executor::execute(&source, &query(vec![col("missing")]), optimize).is_err());
        let mut q = query(vec![col("x")]);
        q.joins.push(Join { relation: relation("items", "j"), on: bin(col("i.x"), BinaryOp::Eq, col("j.x")) });
        assert!(executor::execute(&source, &q, optimize).is_err());
        let mut q = query(vec![col("x")]);
        q.filter = Some(col("missing"));
        assert!(executor::execute(&source, &q, optimize).is_err());
        q.filter = None;
        q.order_by = vec![order(col("missing"), true, None)];
        assert!(executor::execute(&source, &q, optimize).is_err());
        executor::explain(&source, &query(vec![col("x")]), optimize).unwrap();
    }
    assert_eq!(source.reads.get(), 0);
}

#[test]
fn invalid_aggregates_and_ungrouped_columns_fail_on_empty_sources() {
    let source = items(vec![]);
    let invalid = vec![
        vec![col("x"), agg(Aggregate::Count, None, false)],
        vec![agg(Aggregate::Sum, None, false)],
        vec![agg(Aggregate::Count, None, true)],
        vec![agg(Aggregate::Sum, Some(col("*")), false)],
        vec![agg(Aggregate::Sum, Some(lit(Value::Text("1".into()))), false)],
        vec![agg(Aggregate::Avg, Some(lit(Value::Bool(true))), false)],
        vec![agg(Aggregate::Count, Some(agg(Aggregate::Count, None, false)), false)],
        vec![agg(Aggregate::Count, Some(col("missing")), false)],
    ];
    for projection in invalid {
        assert!(executor::execute(&source, &query(projection), true).is_err());
    }
    let mut q = query(vec![col("x")]);
    q.group_by = vec![bin(col("x"), BinaryOp::Add, int(1))];
    assert!(executor::explain(&source, &q, true).is_err());
    q.group_by = vec![agg(Aggregate::Count, None, false)];
    assert!(executor::explain(&source, &q, true).is_err());
    q.group_by.clear();
    q.filter = Some(int(1));
    assert!(executor::explain(&source, &q, true).is_err());
    assert_eq!(source.reads.get(), 0);
}

#[test]
fn mutation_eval_resolution_and_three_valued_logic() {
    let columns = vec!["a.x".into(), "b.x".into(), "a.flag".into()];
    let row = vec![Value::Int(3), Value::Int(4), Value::Null];
    assert!(eval::validate(&col("x"), &columns).is_err());
    assert!(eval::validate(&col("missing"), &columns).is_err());
    assert!(eval::validate(&agg(Aggregate::Count, None, false), &columns).is_err());
    assert_eq!(eval::eval(&bin(col("a.x"), BinaryOp::Add, int(2)), &columns, &row).unwrap(), Value::Int(5));
    assert_eq!(eval::eval(&Expr::IsNull { expr: Box::new(col("a.flag")), negated: false }, &columns, &row).unwrap(), Value::Bool(true));
    assert_eq!(eval::truth(Value::Null).unwrap(), false);
    assert!(eval::truth(Value::Int(1)).is_err());
    let vals = [Value::Bool(false), Value::Bool(true), Value::Null];
    let and = [
        [Value::Bool(false), Value::Bool(false), Value::Bool(false)],
        [Value::Bool(false), Value::Bool(true), Value::Null],
        [Value::Bool(false), Value::Null, Value::Null],
    ];
    let or = [
        [Value::Bool(false), Value::Bool(true), Value::Null],
        [Value::Bool(true), Value::Bool(true), Value::Bool(true)],
        [Value::Null, Value::Bool(true), Value::Null],
    ];
    for (a, left) in vals.iter().enumerate() {
        for (b, right) in vals.iter().enumerate() {
            assert_eq!(eval::eval(&bin(lit(left.clone()), BinaryOp::And, lit(right.clone())), &[], &vec![]).unwrap(), and[a][b]);
            assert_eq!(eval::eval(&bin(lit(left.clone()), BinaryOp::Or, lit(right.clone())), &[], &vec![]).unwrap(), or[a][b]);
        }
    }
    assert_eq!(eval::eval(&Expr::Not(Box::new(lit(Value::Null))), &[], &vec![]).unwrap(), Value::Null);
    assert!(eval::eval(&bin(lit(Value::Bool(false)), BinaryOp::And, int(1)), &[], &vec![]).is_err());
}

#[test]
fn mixed_numeric_precision_hash_equality_and_overflow() {
    let large = 9_007_199_254_740_993;
    let rounded = Value::Float(9_007_199_254_740_992.0);
    assert_eq!(eval::eval(&bin(int(large), BinaryOp::Gt, lit(rounded.clone())), &[], &vec![]).unwrap(), Value::Bool(true));
    assert_eq!(
        eval::eval(&bin(int(i64::MAX), BinaryOp::Lt, lit(Value::Float(9_223_372_036_854_775_808.0))), &[], &vec![]).unwrap(),
        Value::Bool(true)
    );
    assert!(eval::eval(&bin(int(1), BinaryOp::Eq, lit(Value::Text("1".into()))), &[], &vec![]).is_err());
    assert!(eval::eval(&bin(int(i64::MAX), BinaryOp::Add, int(1)), &[], &vec![]).is_err());
    assert!(eval::eval(&bin(int(i64::MIN), BinaryOp::Div, int(-1)), &[], &vec![]).is_err());
    assert!(eval::eval(&bin(int(1), BinaryOp::Div, int(0)), &[], &vec![]).is_err());
    let source = items(vec![vec![Value::Int(large)], vec![Value::Int(large - 1)], vec![Value::Int(0)]]).table(
        "floats",
        &[("x", DataType::Float)],
        vec![vec![rounded], vec![Value::Float(-0.0)], vec![Value::Null]],
    );
    let mut q = query(vec![col("i.x")]);
    q.joins.push(Join { relation: relation("floats", "f"), on: bin(col("i.x"), BinaryOp::Eq, col("f.x")) });
    both(&source, &q, vec![vec![Value::Int(large - 1)], vec![Value::Int(0)]]);
    let q = query(vec![agg(Aggregate::Sum, Some(col("x")), false)]);
    assert!(executor::execute(&items(vec![vec![Value::Int(i64::MAX)], vec![Value::Int(1)]]), &q, true).is_err());
}

#[test]
fn finite_floats_and_numeric_order_not_text_order() {
    let source = items(vec![vec![Value::Int(10)], vec![Value::Int(2)], vec![Value::Int(-3)]]);
    let mut q = query(vec![col("x")]);
    q.order_by.push(order(col("x"), true, None));
    both(&source, &q, vec![vec![Value::Int(-3)], vec![Value::Int(2)], vec![Value::Int(10)]]);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(eval::eval(&lit(Value::Float(value)), &[], &vec![]).is_err());
        let source = Source::default().table("items", &[("x", DataType::Float)], vec![vec![Value::Float(value)]]);
        assert!(executor::execute(&source, &q, true).is_err());
    }
    assert!(eval::eval(&bin(lit(Value::Float(f64::MAX)), BinaryOp::Mul, int(2)), &[], &vec![]).is_err());
}

#[test]
fn candidate_and_source_limits_fail_without_cartesian_collection() {
    let source = items((0..317).map(|i| vec![Value::Int(i)]).collect());
    let mut q = query(vec![col("i.x")]);
    q.joins.push(Join { relation: relation("items", "r"), on: lit(Value::Bool(true)) });
    for optimized in [false, true] {
        let error = executor::execute(&source, &q, optimized).unwrap_err();
        assert!(error.contains("row/candidate limit"), "{error}");
    }
    let source = items(vec![vec![Value::Int(1)]; MAX_QUERY_ROWS + 1]);
    assert!(executor::execute(&source, &query(vec![col("x")]), true).unwrap_err().contains("row limit"));
}

#[test]
fn disjunction_is_not_an_equijoin_or_cross_source_pushdown() {
    let source = items(vec![vec![Value::Int(1)], vec![Value::Null], vec![Value::Int(2)]]);
    let mut q = query(vec![col("i.x"), col("r.x")]);
    q.joins.push(Join {
        relation: relation("items", "r"),
        on: bin(bin(col("i.x"), BinaryOp::Eq, col("r.x")), BinaryOp::Or, bin(col("r.x"), BinaryOp::Eq, int(2))),
    });
    q.filter = Some(bin(bin(col("i.x"), BinaryOp::Eq, int(1)), BinaryOp::Or, Expr::IsNull { expr: Box::new(col("i.x")), negated: false }));
    both(&source, &q, vec![vec![Value::Int(1), Value::Int(1)], vec![Value::Int(1), Value::Int(2)], vec![Value::Null, Value::Int(2)]]);
    assert!(!executor::explain(&source, &q, true).unwrap().iter().any(|line| line.contains("hash-index")));
}

#[test]
fn group_order_alias_and_join_aggregate_duplicates() {
    let source = items(vec![vec![Value::Int(1)], vec![Value::Int(1)], vec![Value::Int(2)], vec![Value::Null]]);
    let mut q = query(vec![col("i.x"), agg(Aggregate::Count, None, false)]);
    q.projection[1].alias = Some("n".into());
    q.joins.push(Join { relation: relation("items", "r"), on: bin(col("i.x"), BinaryOp::Eq, col("r.x")) });
    q.group_by = vec![col("i.x")];
    q.order_by = vec![order(col("n"), false, None), order(col("i.x"), true, None)];
    both(&source, &q, vec![vec![Value::Int(1), Value::Int(4)], vec![Value::Int(2), Value::Int(1)]]);
    q.limit = Some(1);
    q.offset = 1;
    both(&source, &q, vec![vec![Value::Int(2), Value::Int(1)]]);
}

#[test]
fn unsafe_arithmetic_stays_after_the_join() {
    let source = items(vec![vec![Value::Int(0)], vec![Value::Int(1)]]).table("r", &[("x", DataType::Int)], vec![vec![Value::Int(1)]]);
    let mut q = query(vec![col("i.x")]);
    q.joins.push(Join { relation: relation("r", "r"), on: bin(col("i.x"), BinaryOp::Eq, col("r.x")) });
    q.filter = Some(bin(bin(int(1), BinaryOp::Div, col("i.x")), BinaryOp::Gt, int(0)));
    both(&source, &q, vec![vec![Value::Int(1)]]);
    assert!(!executor::explain(&source, &q, true).unwrap().iter().any(|line| line.contains("Pushdown")));
}

#[test]
fn exact_row_bound_and_large_integer_average() {
    let source = items(vec![vec![Value::Int(1)]; MAX_QUERY_ROWS]);
    both(&source, &query(vec![agg(Aggregate::Count, None, false)]), vec![vec![Value::Int(MAX_QUERY_ROWS as i64)]]);
    both(
        &items(vec![vec![Value::Int(i64::MAX)]]),
        &query(vec![agg(Aggregate::Avg, Some(col("x")), false)]),
        vec![vec![Value::Float(i64::MAX as f64)]],
    );
}

#[test]
fn memory_limit_rejects_wide_projection_before_large_collection() {
    let source = Source::default().table("items", &[("x", DataType::Text)], vec![vec![Value::Text("x".repeat(1024 * 1024))]]);
    let q = query(vec![col("x"); 70]);
    for optimize in [false, true] {
        assert!(executor::execute(&source, &q, optimize).unwrap_err().contains("memory limit"));
    }
}
