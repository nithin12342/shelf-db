use crate::model::{BinaryOp, Expr, Result, Row, Value};
use std::cmp::Ordering;

#[path = "execution_value.rs"]
pub(crate) mod value;

pub fn resolve(relation: Option<&str>, name: &str, columns: &[String]) -> Result<usize> {
    if name == "*" {
        return Err("Wildcard is only valid in a projection or COUNT(*)".into());
    }
    let mut found = None;
    for (index, column) in columns.iter().enumerate() {
        let matches = match relation {
            Some(relation) => column == &format!("{relation}.{name}"),
            None => column.rsplit('.').next() == Some(name),
        };
        if matches {
            if found.is_some() {
                return Err(format!("Ambiguous column: {name}"));
            }
            found = Some(index);
        }
    }
    found.ok_or_else(|| format!("Unknown column: {}{name}", relation.map(|r| format!("{r}.")).unwrap_or_default()))
}

/// Validate scalar expressions before mutation, including empty-table mutations.
pub fn validate(expr: &Expr, columns: &[String]) -> Result<()> {
    fn visit(expr: &Expr, columns: &[String], depth: usize) -> Result<()> {
        if depth > 128 {
            return Err("Expression nesting exceeds 128".into());
        }
        match expr {
            Expr::Column { relation, name } => {
                resolve(relation.as_deref(), name, columns)?;
            }
            Expr::Literal(value) => value::finite(value)?,
            Expr::Binary { left, right, .. } => {
                visit(left, columns, depth + 1)?;
                visit(right, columns, depth + 1)?;
            }
            Expr::Not(inner) | Expr::IsNull { expr: inner, .. } => visit(inner, columns, depth + 1)?,
            Expr::Aggregate { .. } => return Err("Aggregate is not a scalar expression".into()),
        }
        Ok(())
    }
    visit(expr, columns, 0)
}

/// NULL does not pass a WHERE or ON predicate. Other non-booleans are errors.
pub fn truth(value: Value) -> Result<bool> {
    Ok(boolean(&value)?.unwrap_or(false))
}

fn boolean(value: &Value) -> Result<Option<bool>> {
    match value {
        Value::Null => Ok(None),
        Value::Bool(v) => Ok(Some(*v)),
        _ => Err("A predicate requires a boolean or NULL".into()),
    }
}

pub fn eval(expr: &Expr, columns: &[String], row: &Row) -> Result<Value> {
    validate(expr, columns)?;
    eval_with(expr, columns, row, &mut |_| Err("Aggregate requires grouping".into()))
}

pub(crate) fn eval_with(
    expr: &Expr,
    columns: &[String],
    row: &Row,
    aggregate: &mut impl FnMut(&Expr) -> Result<Value>,
) -> Result<Value> {
    match expr {
        Expr::Column { relation, name } => {
            let index = resolve(relation.as_deref(), name, columns)?;
            let value = row.get(index).ok_or("Row width does not match its columns")?;
            value::finite(value)?;
            Ok(value.clone())
        }
        Expr::Literal(value) => {
            value::finite(value)?;
            Ok(value.clone())
        }
        Expr::Aggregate { .. } => aggregate(expr),
        Expr::Not(inner) => Ok(match boolean(&eval_with(inner, columns, row, aggregate)?)? {
            Some(v) => Value::Bool(!v),
            None => Value::Null,
        }),
        Expr::IsNull { expr: inner, negated } => {
            let is_null = matches!(eval_with(inner, columns, row, aggregate)?, Value::Null);
            Ok(Value::Bool(is_null != *negated))
        }
        Expr::Binary { left, op, right } => {
            let left = eval_with(left, columns, row, aggregate)?;
            let right = eval_with(right, columns, row, aggregate)?;
            binary(&left, *op, &right)
        }
    }
}

pub(crate) fn binary(left: &Value, op: BinaryOp, right: &Value) -> Result<Value> {
    use BinaryOp::*;
    if matches!(op, And | Or) {
        let a = boolean(left)?;
        let b = boolean(right)?;
        return Ok(match (op, a, b) {
            (And, Some(false), _) | (And, _, Some(false)) => Value::Bool(false),
            (And, Some(true), Some(true)) => Value::Bool(true),
            (Or, Some(true), _) | (Or, _, Some(true)) => Value::Bool(true),
            (Or, Some(false), Some(false)) => Value::Bool(false),
            _ => Value::Null,
        });
    }
    value::finite(left)?;
    value::finite(right)?;
    if matches!(left, Value::Null) || matches!(right, Value::Null) {
        return Ok(Value::Null);
    }
    if matches!(op, Add | Sub | Mul | Div) {
        return value::arithmetic(left, op, right);
    }
    let ordering = value::compare(left, right)?;
    Ok(Value::Bool(match op {
        Eq => ordering == Ordering::Equal,
        Ne => ordering != Ordering::Equal,
        Lt => ordering == Ordering::Less,
        Le => ordering != Ordering::Greater,
        Gt => ordering == Ordering::Greater,
        Ge => ordering != Ordering::Less,
        _ => unreachable!(),
    }))
}
