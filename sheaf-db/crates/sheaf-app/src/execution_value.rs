use crate::model::{BinaryOp, Result, Value};
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub fn finite(value: &Value) -> Result<()> {
    if matches!(value, Value::Float(v) if !v.is_finite()) {
        return Err("Non-finite floating point value".into());
    }
    Ok(())
}

// Compare without rounding an integer through f64. The upper bound is exclusive.
fn int_float(integer: i64, float: f64) -> Ordering {
    if float >= 9_223_372_036_854_775_808.0 {
        return Ordering::Less;
    }
    if float < -9_223_372_036_854_775_808.0 {
        return Ordering::Greater;
    }
    match integer.cmp(&(float as i64)) {
        Ordering::Equal if float.fract() > 0.0 => Ordering::Less,
        Ordering::Equal if float.fract() < 0.0 => Ordering::Greater,
        order => order,
    }
}

pub fn compare(left: &Value, right: &Value) -> Result<Ordering> {
    finite(left)?;
    finite(right)?;
    Ok(match (left, right) {
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Float(a), Value::Float(b)) => a.partial_cmp(b).ok_or("Invalid float")?,
        (Value::Int(a), Value::Float(b)) => int_float(*a, *b),
        (Value::Float(a), Value::Int(b)) => int_float(*b, *a).reverse(),
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (Value::Null, Value::Null) => Ordering::Equal,
        _ => return Err("Cannot compare incompatible value types".into()),
    })
}

pub fn same(left: &[Value], right: &[Value]) -> Result<bool> {
    if left.len() != right.len() {
        return Ok(false);
    }
    for (a, b) in left.iter().zip(right) {
        match (a, b) {
            (Value::Null, Value::Null) => (),
            (Value::Null, _) | (_, Value::Null) => return Ok(false),
            _ if compare(a, b)? != Ordering::Equal => return Ok(false),
            _ => (),
        }
    }
    Ok(true)
}

// Hashes only select candidate buckets. Equality always resolves collisions.
// Integral floats share integer hashes, including negative zero.
pub fn hash(values: &[Value]) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    values.len().hash(&mut hasher);
    for value in values {
        finite(value)?;
        match value {
            Value::Null => 0u8.hash(&mut hasher),
            Value::Bool(v) => {
                1u8.hash(&mut hasher);
                v.hash(&mut hasher);
            }
            Value::Int(v) => {
                2u8.hash(&mut hasher);
                v.hash(&mut hasher);
            }
            Value::Float(v)
                if v.fract() == 0.0 && *v >= -9_223_372_036_854_775_808.0 && *v < 9_223_372_036_854_775_808.0 =>
            {
                2u8.hash(&mut hasher);
                (*v as i64).hash(&mut hasher);
            }
            Value::Float(v) => {
                3u8.hash(&mut hasher);
                v.to_bits().hash(&mut hasher);
            }
            Value::Text(v) => {
                4u8.hash(&mut hasher);
                v.hash(&mut hasher);
            }
        }
    }
    Ok(hasher.finish())
}

pub fn as_float(value: &Value) -> Result<f64> {
    match value {
        Value::Float(v) if v.is_finite() => Ok(*v),
        Value::Int(v) if int_float(*v, *v as f64) == Ordering::Equal => Ok(*v as f64),
        Value::Int(_) => Err("Integer cannot convert to float without precision loss".into()),
        _ => Err("Expected a finite number".into()),
    }
}

pub fn arithmetic(left: &Value, op: BinaryOp, right: &Value) -> Result<Value> {
    use BinaryOp::*;
    if let (Value::Int(a), Value::Int(b)) = (left, right) {
        let result = match op {
            Add => a.checked_add(*b),
            Sub => a.checked_sub(*b),
            Mul => a.checked_mul(*b),
            Div if *b == 0 => return Err("Division by zero".into()),
            Div => a.checked_div(*b),
            _ => return Err("Expected arithmetic operator".into()),
        };
        return result.map(Value::Int).ok_or_else(|| "Integer arithmetic overflow".into());
    }
    let a = as_float(left)?;
    let b = as_float(right)?;
    let result = match op {
        Add => a + b,
        Sub => a - b,
        Mul => a * b,
        Div if b == 0.0 => return Err("Division by zero".into()),
        Div => a / b,
        _ => return Err("Expected arithmetic operator".into()),
    };
    finite(&Value::Float(result))?;
    Ok(Value::Float(result))
}
