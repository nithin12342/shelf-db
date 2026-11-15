use crate::eval;
use crate::model::{Aggregate, BinaryOp, DataType, Expr, Result, Value};

pub fn bind(expr: &Expr, columns: &[String], types: &[DataType], aggregates: bool) -> Result<(Expr, Option<DataType>)> {
    bind_at(expr, columns, types, aggregates, 0)
}

fn bind_at(
    expr: &Expr,
    columns: &[String],
    types: &[DataType],
    aggregates: bool,
    depth: usize,
) -> Result<(Expr, Option<DataType>)> {
    if depth > 128 {
        return Err("Expression nesting exceeds 128".into());
    }
    let recur = |expr: &Expr, allow| bind_at(expr, columns, types, allow, depth + 1);
    match expr {
        Expr::Column { relation, name } => {
            let index = eval::resolve(relation.as_deref(), name, columns)?;
            let (relation, name) = columns[index].rsplit_once('.').ok_or("Unqualified source column")?;
            Ok((Expr::Column { relation: Some(relation.into()), name: name.into() }, Some(types[index])))
        }
        Expr::Literal(value) => {
            eval::value::finite(value)?;
            let data_type = match value {
                Value::Null => None,
                Value::Bool(_) => Some(DataType::Bool),
                Value::Int(_) => Some(DataType::Int),
                Value::Float(_) => Some(DataType::Float),
                Value::Text(_) => Some(DataType::Text),
            };
            Ok((expr.clone(), data_type))
        }
        Expr::Not(inner) => {
            let (inner, data_type) = recur(inner, aggregates)?;
            boolean_type(data_type)?;
            Ok((Expr::Not(Box::new(inner)), Some(DataType::Bool)))
        }
        Expr::IsNull { expr: inner, negated } => {
            let (inner, _) = recur(inner, aggregates)?;
            Ok((Expr::IsNull { expr: Box::new(inner), negated: *negated }, Some(DataType::Bool)))
        }
        Expr::Binary { left, op, right } => {
            let (left, a) = recur(left, aggregates)?;
            let (right, b) = recur(right, aggregates)?;
            use BinaryOp::*;
            let data_type = match op {
                And | Or => {
                    boolean_type(a)?;
                    boolean_type(b)?;
                    Some(DataType::Bool)
                }
                Add | Sub | Mul | Div => {
                    numeric_type(a)?;
                    numeric_type(b)?;
                    if a == Some(DataType::Float) || b == Some(DataType::Float) {
                        Some(DataType::Float)
                    } else {
                        a.or(b)
                    }
                }
                _ => {
                    if let (Some(a), Some(b)) = (a, b) {
                        if a != b && !(is_numeric(a) && is_numeric(b)) {
                            return Err("Cannot compare incompatible expression types".into());
                        }
                    }
                    Some(DataType::Bool)
                }
            };
            Ok((Expr::Binary { left: Box::new(left), op: *op, right: Box::new(right) }, data_type))
        }
        Expr::Aggregate { function, expr: argument, distinct } => {
            if !aggregates {
                return Err("Aggregate is not allowed here or is nested".into());
            }
            let star = matches!(argument.as_deref(), Some(Expr::Column { relation: None, name }) if name == "*");
            if argument.is_none() || star {
                if *function != Aggregate::Count || *distinct {
                    return Err("Only COUNT(*) accepts a missing or wildcard argument, without DISTINCT".into());
                }
                return Ok((Expr::Aggregate { function: *function, expr: None, distinct: false }, Some(DataType::Int)));
            }
            let (argument, data_type) = recur(argument.as_deref().ok_or("Missing aggregate argument")?, false)?;
            let output = match function {
                Aggregate::Count => Some(DataType::Int),
                Aggregate::Sum => {
                    numeric_type(data_type)?;
                    data_type
                }
                Aggregate::Avg => {
                    numeric_type(data_type)?;
                    Some(DataType::Float)
                }
                Aggregate::Min | Aggregate::Max => data_type,
            };
            Ok((Expr::Aggregate { function: *function, expr: Some(Box::new(argument)), distinct: *distinct }, output))
        }
    }
}

fn is_numeric(data_type: DataType) -> bool {
    matches!(data_type, DataType::Int | DataType::Float)
}
fn numeric_type(data_type: Option<DataType>) -> Result<()> {
    if data_type.is_some_and(|t| !is_numeric(t)) {
        return Err("Arithmetic and SUM/AVG require numeric arguments".into());
    }
    Ok(())
}
pub fn boolean_type(data_type: Option<DataType>) -> Result<()> {
    if data_type.is_some_and(|t| t != DataType::Bool) {
        return Err("Predicate requires a boolean expression".into());
    }
    Ok(())
}

pub fn has_aggregate(expr: &Expr) -> bool {
    match expr {
        Expr::Aggregate { .. } => true,
        Expr::Binary { left, right, .. } => has_aggregate(left) || has_aggregate(right),
        Expr::Not(expr) | Expr::IsNull { expr, .. } => has_aggregate(expr),
        _ => false,
    }
}

pub fn grouped(expr: &Expr, groups: &[Expr]) -> Result<()> {
    if groups.contains(expr) {
        return Ok(());
    }
    match expr {
        Expr::Aggregate { .. } | Expr::Literal(_) => Ok(()),
        Expr::Binary { left, right, .. } => {
            grouped(left, groups)?;
            grouped(right, groups)
        }
        Expr::Not(expr) | Expr::IsNull { expr, .. } => grouped(expr, groups),
        Expr::Column { .. } => Err("Projection or ordering contains an ungrouped column".into()),
    }
}

// Only total predicates move ahead of joins. Arithmetic can fail on discarded rows.
pub fn safe_predicate(expr: &Expr) -> bool {
    match expr {
        Expr::Column { .. } | Expr::Literal(_) => true,
        Expr::Binary { left, op, right } => {
            !matches!(op, BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div)
                && safe_predicate(left)
                && safe_predicate(right)
        }
        Expr::Not(expr) | Expr::IsNull { expr, .. } => safe_predicate(expr),
        Expr::Aggregate { .. } => false,
    }
}

pub fn conjuncts<'a>(expr: &'a Expr, output: &mut Vec<&'a Expr>) {
    if let Expr::Binary { left, op: BinaryOp::And, right } = expr {
        conjuncts(left, output);
        conjuncts(right, output);
    } else {
        output.push(expr);
    }
}

pub fn relations<'a>(expr: &'a Expr, output: &mut Vec<&'a str>) {
    match expr {
        Expr::Column { relation: Some(relation), .. } => {
            if !output.contains(&relation.as_str()) {
                output.push(relation);
            }
        }
        Expr::Binary { left, right, .. } => {
            relations(left, output);
            relations(right, output);
        }
        Expr::Not(expr) | Expr::IsNull { expr, .. } => relations(expr, output),
        _ => (),
    }
}

pub fn label(expr: &Expr) -> String {
    match expr {
        Expr::Column { relation, name } => {
            relation.as_ref().map(|r| format!("{r}.{name}")).unwrap_or_else(|| name.clone())
        }
        Expr::Literal(Value::Text(text)) => format!("'{}'", text.replace('\'', "''")),
        Expr::Literal(value) => value.text(),
        Expr::Aggregate { function, expr, distinct } => format!(
            "{}({}{})",
            match function {
                Aggregate::Count => "COUNT",
                Aggregate::Sum => "SUM",
                Aggregate::Avg => "AVG",
                Aggregate::Min => "MIN",
                Aggregate::Max => "MAX",
            },
            if *distinct { "DISTINCT " } else { "" },
            expr.as_deref().map(label).unwrap_or_else(|| "*".into())
        ),
        Expr::Not(expr) => format!("NOT ({})", label(expr)),
        Expr::IsNull { expr, negated } => format!("{} IS {}NULL", label(expr), if *negated { "NOT " } else { "" }),
        Expr::Binary { left, op, right } => format!(
            "({} {} {})",
            label(left),
            match op {
                BinaryOp::Eq => "=",
                BinaryOp::Ne => "<>",
                BinaryOp::Lt => "<",
                BinaryOp::Le => "<=",
                BinaryOp::Gt => ">",
                BinaryOp::Ge => ">=",
                BinaryOp::And => "AND",
                BinaryOp::Or => "OR",
                BinaryOp::Add => "+",
                BinaryOp::Sub => "-",
                BinaryOp::Mul => "*",
                BinaryOp::Div => "/",
            },
            label(right)
        ),
    }
}
