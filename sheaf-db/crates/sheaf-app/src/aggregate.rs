use crate::eval;
use crate::eval::value;
use crate::executor::limits::{self, Budget, HASH_ENTRY, INDEX_ENTRY};
use crate::model::{Aggregate, BinaryOp, Expr, Result, Row, Value};
use crate::relational_plan::ExecutablePlan;
use std::cmp::Ordering;
use std::collections::HashMap;

struct State {
    count: i64,
    value: Option<Value>,
    seen: Option<HashMap<u64, Vec<Value>>>,
}

impl State {
    fn new(expr: &Expr) -> Self {
        let distinct = matches!(expr, Expr::Aggregate { distinct: true, .. });
        Self { count: 0, value: None, seen: distinct.then(HashMap::new) }
    }

    fn accept(&mut self, value: &Value, budget: &mut Budget) -> Result<bool> {
        if let Some(seen) = &mut self.seen {
            let hash = value::hash(std::slice::from_ref(value))?;
            if let Some(bucket) = seen.get(&hash) {
                for old in bucket {
                    if value::compare(old, value)? == Ordering::Equal {
                        return Ok(false);
                    }
                }
            }
            if !seen.contains_key(&hash) {
                budget.reserve(HASH_ENTRY)?;
            }
            budget.reserve(limits::value_bytes(value).checked_mul(2).ok_or("Aggregate size overflow")?)?;
            seen.entry(hash).or_default().push(value.clone());
        }
        Ok(true)
    }

    fn update(&mut self, spec: &Expr, columns: &[String], row: &Row, budget: &mut Budget) -> Result<()> {
        let Expr::Aggregate { function, expr, .. } = spec else {
            return Err("Invalid aggregate plan".into());
        };
        let value = match expr {
            Some(expr) => eval::eval(expr, columns, row)?,
            None => Value::Int(1),
        };
        if matches!(value, Value::Null) || !self.accept(&value, budget)? {
            return Ok(());
        }
        self.count = self.count.checked_add(1).ok_or("COUNT overflow")?;
        let next = match function {
            Aggregate::Count => return Ok(()),
            Aggregate::Sum | Aggregate::Avg => match &self.value {
                Some(old) => value::arithmetic(old, BinaryOp::Add, &value)?,
                None => {
                    if !matches!(value, Value::Int(_) | Value::Float(_)) {
                        return Err("SUM and AVG require numeric arguments".into());
                    }
                    value
                }
            },
            Aggregate::Min | Aggregate::Max => {
                if let Some(old) = &self.value {
                    let order = value::compare(&value, old)?;
                    let replace = (*function == Aggregate::Min && order == Ordering::Less)
                        || (*function == Aggregate::Max && order == Ordering::Greater);
                    if !replace {
                        return Ok(());
                    }
                }
                value
            }
        };
        let old = self.value.as_ref().map(limits::value_bytes).unwrap_or(0) * 2;
        let new = limits::value_bytes(&next).checked_mul(2).ok_or("Aggregate size overflow")?;
        if new > old {
            budget.reserve(new - old)?;
        } else {
            budget.release(old - new);
        }
        self.value = Some(next);
        Ok(())
    }

    fn finish(&self, spec: &Expr) -> Result<Value> {
        let Expr::Aggregate { function, .. } = spec else {
            return Err("Invalid aggregate plan".into());
        };
        match function {
            Aggregate::Count => Ok(Value::Int(self.count)),
            Aggregate::Avg => match &self.value {
                None => Ok(Value::Null),
                Some(value) => {
                    // AVG has a floating result. Comparisons still use exact mixed-number ordering.
                    let sum = match value {
                        Value::Int(number) => *number as f64,
                        Value::Float(number) => *number,
                        _ => return Err("AVG requires a numeric sum".into()),
                    };
                    let result = sum / self.count as f64;
                    value::finite(&Value::Float(result))?;
                    Ok(Value::Float(result))
                }
            },
            _ => Ok(self.value.clone().unwrap_or(Value::Null)),
        }
    }
}

struct Group {
    key: Row,
    representative: Row,
    states: Vec<State>,
}

pub(crate) struct Aggregator<'a> {
    plan: &'a ExecutablePlan,
    specs: Vec<Expr>,
    groups: Vec<Group>,
    buckets: HashMap<u64, Vec<usize>>,
}

fn collect(expr: &Expr, specs: &mut Vec<Expr>) {
    match expr {
        Expr::Aggregate { .. } => {
            if !specs.contains(expr) {
                specs.push(expr.clone());
            }
        }
        Expr::Binary { left, right, .. } => {
            collect(left, specs);
            collect(right, specs);
        }
        Expr::Not(expr) | Expr::IsNull { expr, .. } => collect(expr, specs),
        _ => (),
    }
}

impl<'a> Aggregator<'a> {
    pub fn new(plan: &'a ExecutablePlan, budget: &mut Budget) -> Result<Self> {
        let mut specs = Vec::new();
        for expr in plan.projection.iter().map(|p| &p.expr).chain(plan.order_by.iter().map(|o| &o.expr)) {
            collect(expr, &mut specs);
        }
        let mut result = Self { plan, specs, groups: Vec::new(), buckets: HashMap::new() };
        if plan.group_by.is_empty() {
            // A global aggregate emits one row even when its input is empty.
            let bytes = plan.columns.len().checked_mul(std::mem::size_of::<Value>()).ok_or("Group width overflow")?;
            budget.reserve(bytes)?;
            let row = vec![Value::Null; plan.columns.len()];
            result.add_group(Vec::new(), &row, budget)?;
            budget.release(bytes);
        }
        Ok(result)
    }

    fn add_group(&mut self, key: Row, row: &Row, budget: &mut Budget) -> Result<usize> {
        limits::row_limit(self.groups.len())?;
        budget.reserve(std::mem::size_of::<Group>() * 2)?;
        budget.reserve(limits::row_bytes(&key)?.checked_mul(2).ok_or("Group size overflow")?)?;
        let representative = limits::copy_row(row, budget)?;
        budget.reserve(
            self.specs.len().checked_mul(std::mem::size_of::<State>() * 2).ok_or("Aggregate state size overflow")?,
        )?;
        let states = self.specs.iter().map(State::new).collect();
        let index = self.groups.len();
        self.groups.push(Group { key, representative, states });
        Ok(index)
    }

    pub fn consume(&mut self, row: &Row, budget: &mut Budget) -> Result<()> {
        let index = if self.plan.group_by.is_empty() {
            0
        } else {
            let mut key = Vec::new();
            let mut temporary = std::mem::size_of::<Row>();
            budget.reserve(temporary)?;
            for expr in &self.plan.group_by {
                let value = eval::eval(expr, &self.plan.columns, row)?;
                let bytes = limits::value_bytes(&value).checked_mul(2).ok_or("Group key size overflow")?;
                budget.reserve(bytes)?;
                temporary += bytes;
                key.push(value);
            }
            let hash = value::hash(&key)?;
            let mut found = None;
            if let Some(bucket) = self.buckets.get(&hash) {
                for &index in bucket {
                    if value::same(&self.groups[index].key, &key)? {
                        found = Some(index);
                        break;
                    }
                }
            }
            let index = match found {
                Some(index) => index,
                None => {
                    if !self.buckets.contains_key(&hash) {
                        budget.reserve(HASH_ENTRY)?;
                    }
                    budget.reserve(INDEX_ENTRY)?;
                    let index = self.add_group(key, row, budget)?;
                    self.buckets.entry(hash).or_default().push(index);
                    index
                }
            };
            budget.release(temporary);
            index
        };
        for (state, spec) in self.groups[index].states.iter_mut().zip(&self.specs) {
            state.update(spec, &self.plan.columns, row, budget)?;
        }
        Ok(())
    }

    pub fn finish(&self, mut sink: impl FnMut(&mut dyn FnMut(&Expr) -> Result<Value>) -> Result<()>) -> Result<()> {
        for group in &self.groups {
            let mut evaluate = |expr: &Expr| {
                eval::eval_with(expr, &self.plan.columns, &group.representative, &mut |aggregate| {
                    let index =
                        self.specs.iter().position(|spec| spec == aggregate).ok_or("Missing aggregate state")?;
                    group.states[index].finish(aggregate)
                })
            };
            sink(&mut evaluate)?;
        }
        Ok(())
    }
}
