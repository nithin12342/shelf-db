use crate::eval;
use crate::model::*;
use std::collections::HashSet;

#[path = "execution_validate.rs"]
mod validation;

#[derive(Clone, Debug)]
pub struct ScanPlan {
    pub relation: Relation,
    pub columns: Vec<String>,
    pub schema: Vec<Column>,
    pub start: usize,
    pub predicates: Vec<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JoinAlgorithm {
    NestedLoop,
    HashIndex { left_column: usize, right_column: usize },
}

#[derive(Clone, Debug)]
pub struct JoinPlan {
    pub scan: ScanPlan,
    pub on: Expr,
    pub algorithm: JoinAlgorithm,
}

#[derive(Clone, Debug)]
pub struct ExecutablePlan {
    pub from: ScanPlan,
    pub joins: Vec<JoinPlan>,
    pub columns: Vec<String>,
    pub projection: Vec<Projection>,
    pub output_columns: Vec<String>,
    pub filter: Option<Expr>,
    pub group_by: Vec<Expr>,
    pub aggregate: bool,
    pub order_by: Vec<Order>,
    pub distinct: bool,
    pub limit: Option<usize>,
    pub offset: usize,
}

fn scan(source: &impl DataSource, relation: &Relation, start: usize) -> Result<ScanPlan> {
    let schema = source.columns(&relation.table)?;
    if schema.len() > MAX_QUERY_ROWS {
        return Err("Too many source columns".into());
    }
    let mut names = HashSet::new();
    for column in &schema {
        if !names.insert(&column.name) {
            return Err(format!("Duplicate source column: {}", column.name));
        }
    }
    let columns = schema.iter().map(|c| format!("{}.{}", relation.alias, c.name)).collect();
    Ok(ScanPlan { relation: relation.clone(), columns, schema, start, predicates: Vec::new() })
}

/// Build and validate with metadata only. Execution consumes these decisions.
pub fn build(source: &impl DataSource, query: &Query, optimize: bool) -> Result<ExecutablePlan> {
    if query.projection.is_empty() {
        return Err("SELECT requires a projection".into());
    }
    if query.joins.len() > 64 {
        return Err("Query exceeds 64 joins".into());
    }
    let mut aliases = HashSet::new();
    for relation in std::iter::once(&query.from).chain(query.joins.iter().map(|j| &j.relation)) {
        if !aliases.insert(&relation.alias) {
            return Err(format!("Duplicate relation alias: {}", relation.alias));
        }
    }
    let from = scan(source, &query.from, 0)?;
    let mut columns = from.columns.clone();
    let mut types: Vec<DataType> = from.schema.iter().map(|c| c.data_type).collect();
    let mut joins = Vec::new();
    for join in &query.joins {
        let right = scan(source, &join.relation, columns.len())?;
        columns.extend(right.columns.iter().cloned());
        types.extend(right.schema.iter().map(|c| c.data_type));
        let (on, data_type) = validation::bind(&join.on, &columns, &types, false)?;
        validation::boolean_type(data_type)?;
        let algorithm = if optimize { hash_join(&on, &columns, right.start)? } else { JoinAlgorithm::NestedLoop };
        joins.push(JoinPlan { scan: right, on, algorithm });
    }
    let filter = query
        .filter
        .as_ref()
        .map(|expr| {
            let (expr, data_type) = validation::bind(expr, &columns, &types, false)?;
            validation::boolean_type(data_type)?;
            Ok::<_, String>(expr)
        })
        .transpose()?;
    let group_by = query
        .group_by
        .iter()
        .map(|expr| validation::bind(expr, &columns, &types, false).map(|p| p.0))
        .collect::<Result<Vec<_>>>()?;
    let mut projection = Vec::new();
    let mut output_columns = Vec::new();
    let mut projection_aliases = HashSet::new();
    for item in &query.projection {
        if let Some(alias) = &item.alias {
            if !projection_aliases.insert(alias.clone()) {
                return Err(format!("Duplicate projection alias: {alias}"));
            }
        }
        if let Expr::Column { relation, name } = &item.expr {
            if name == "*" {
                if item.alias.is_some() {
                    return Err("A wildcard cannot have an alias".into());
                }
                let mut count = 0;
                for column in &columns {
                    let (owner, name) = column.rsplit_once('.').ok_or("Unqualified source column")?;
                    if relation.as_ref().is_none_or(|r| r == owner) {
                        projection.push(Projection {
                            expr: Expr::Column { relation: Some(owner.into()), name: name.into() },
                            alias: None,
                        });
                        output_columns.push(name.into());
                        count += 1;
                    }
                }
                if count == 0 {
                    return Err("Wildcard does not name any source columns".into());
                }
                continue;
            }
        }
        let (expr, _) = validation::bind(&item.expr, &columns, &types, true)?;
        let name = item.alias.clone().unwrap_or_else(|| match &expr {
            Expr::Column { name, .. } => name.clone(),
            _ => validation::label(&expr),
        });
        projection.push(Projection { expr, alias: item.alias.clone() });
        output_columns.push(name);
    }
    let mut order_by = Vec::new();
    for order in &query.order_by {
        let expanded = expand_aliases(&order.expr, &projection, 0)?;
        let (expr, _) = validation::bind(&expanded, &columns, &types, true)?;
        if query.distinct && !projection.iter().any(|p| p.expr == expr) {
            return Err("ORDER BY in SELECT DISTINCT must name a projected expression".into());
        }
        order_by.push(Order { expr, ascending: order.ascending, nulls_first: order.nulls_first });
    }
    let aggregate = !group_by.is_empty()
        || projection.iter().any(|p| validation::has_aggregate(&p.expr))
        || order_by.iter().any(|o| validation::has_aggregate(&o.expr));
    if aggregate {
        for expr in projection.iter().map(|p| &p.expr).chain(order_by.iter().map(|o| &o.expr)) {
            validation::grouped(expr, &group_by)?;
        }
    }
    let mut plan = ExecutablePlan {
        from,
        joins,
        columns,
        projection,
        output_columns,
        filter,
        group_by,
        aggregate,
        order_by,
        distinct: query.distinct,
        limit: query.limit,
        offset: query.offset,
    };
    if optimize {
        push_predicates(&mut plan);
    }
    Ok(plan)
}

fn expand_aliases(expr: &Expr, projection: &[Projection], depth: usize) -> Result<Expr> {
    if depth > 128 {
        return Err("Expression nesting exceeds 128".into());
    }
    let expand = |expr: &Expr| expand_aliases(expr, projection, depth + 1);
    Ok(match expr {
        Expr::Column { relation: None, name } => projection
            .iter()
            .find(|p| p.alias.as_ref() == Some(name))
            .map(|p| p.expr.clone())
            .unwrap_or_else(|| expr.clone()),
        Expr::Binary { left, op, right } => {
            Expr::Binary { left: Box::new(expand(left)?), op: *op, right: Box::new(expand(right)?) }
        }
        Expr::Not(inner) => Expr::Not(Box::new(expand(inner)?)),
        Expr::IsNull { expr, negated } => Expr::IsNull { expr: Box::new(expand(expr)?), negated: *negated },
        // Aggregate arguments resolve against source columns, not output aliases.
        Expr::Aggregate { .. } => expr.clone(),
        _ => expr.clone(),
    })
}

fn hash_join(on: &Expr, columns: &[String], right_start: usize) -> Result<JoinAlgorithm> {
    let mut conjuncts = Vec::new();
    validation::conjuncts(on, &mut conjuncts);
    for expr in conjuncts {
        if let Expr::Binary { left, op: BinaryOp::Eq, right } = expr {
            if let (Expr::Column { relation: a, name: x }, Expr::Column { relation: b, name: y }) =
                (left.as_ref(), right.as_ref())
            {
                let a = eval::resolve(a.as_deref(), x, columns)?;
                let b = eval::resolve(b.as_deref(), y, columns)?;
                let (left, right) = if a < right_start { (a, b) } else { (b, a) };
                if left < right_start && right >= right_start {
                    return Ok(JoinAlgorithm::HashIndex { left_column: left, right_column: right - right_start });
                }
            }
        }
    }
    Ok(JoinAlgorithm::NestedLoop)
}

fn push_predicates(plan: &mut ExecutablePlan) {
    let mut expressions = Vec::new();
    if let Some(filter) = &plan.filter {
        validation::conjuncts(filter, &mut expressions);
    }
    for join in &plan.joins {
        validation::conjuncts(&join.on, &mut expressions);
    }
    let mut pushed = Vec::new();
    for expr in expressions {
        let mut relations = Vec::new();
        validation::relations(expr, &mut relations);
        if relations.len() == 1 && validation::safe_predicate(expr) {
            pushed.push((relations[0].to_owned(), expr.clone()));
        }
    }
    for (alias, expr) in pushed {
        for scan in std::iter::once(&mut plan.from).chain(plan.joins.iter_mut().map(|j| &mut j.scan)) {
            if scan.relation.alias == alias && !scan.predicates.contains(&expr) {
                scan.predicates.push(expr.clone());
            }
        }
    }
}

impl ExecutablePlan {
    pub fn describe(&self) -> Vec<String> {
        fn describe_scan(scan: &ScanPlan, lines: &mut Vec<String>) {
            lines.push(format!("Scan {} AS {}", scan.relation.table, scan.relation.alias));
            for predicate in &scan.predicates {
                lines.push(format!("  Pushdown {}", validation::label(predicate)));
            }
        }
        let mut lines = Vec::new();
        describe_scan(&self.from, &mut lines);
        for join in &self.joins {
            describe_scan(&join.scan, &mut lines);
            lines.push(match join.algorithm {
                JoinAlgorithm::NestedLoop => "Inner nested-loop join (streamed candidates)".into(),
                JoinAlgorithm::HashIndex { left_column, right_column } => format!(
                    "Inner hash-index join {} = {} (duplicate-preserving buckets)",
                    self.columns[left_column], join.scan.columns[right_column]
                ),
            });
            lines.push(format!("  Full ON {}", validation::label(&join.on)));
        }
        if let Some(filter) = &self.filter {
            lines.push(format!("Filter {}", validation::label(filter)));
        }
        if self.aggregate {
            lines.push(format!("Aggregate with {} group expressions", self.group_by.len()));
        }
        lines.push(format!("Project {}", self.output_columns.join(", ")));
        if self.distinct {
            lines.push("Distinct projected rows (NULLs equal)".into());
        }
        for order in &self.order_by {
            lines.push(format!(
                "Order {} {} NULLS {}",
                validation::label(&order.expr),
                if order.ascending { "ASC" } else { "DESC" },
                if order.nulls_first.unwrap_or(!order.ascending) { "FIRST" } else { "LAST" }
            ));
        }
        lines.push(format!(
            "Offset {}, limit {}",
            self.offset,
            self.limit.map(|v| v.to_string()).unwrap_or_else(|| "none".into())
        ));
        lines.push(format!("Bounds: {MAX_QUERY_ROWS} candidates/rows, {MEMORY_LIMIT} bytes"));
        lines
    }
}
