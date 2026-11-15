use crate::eval::value;
use crate::executor::limits::{self, Budget, HASH_ENTRY, INDEX_ENTRY};
use crate::model::{Expr, Result, Row, Value};
use crate::relational_plan::ExecutablePlan;
use std::cmp::Ordering;
use std::collections::HashMap;

struct Record {
    row: Row,
    order: Row,
}
pub struct Output<'a> {
    plan: &'a ExecutablePlan,
    records: Vec<Record>,
    distinct: HashMap<u64, Vec<usize>>,
}

impl<'a> Output<'a> {
    pub fn new(plan: &'a ExecutablePlan) -> Self {
        Self { plan, records: Vec::new(), distinct: HashMap::new() }
    }

    pub fn collect(&mut self, evaluate: &mut dyn FnMut(&Expr) -> Result<Value>, budget: &mut Budget) -> Result<()> {
        // Charge each value before adding it to the projected row.
        let mut charge = std::mem::size_of::<Record>() * 3;
        budget.reserve(charge)?;
        let mut row = Vec::new();
        for item in &self.plan.projection {
            let value = evaluate(&item.expr)?;
            let bytes = limits::value_bytes(&value).checked_mul(2).ok_or("Output size overflow")?;
            budget.reserve(bytes)?;
            charge += bytes;
            row.push(value);
        }
        let hash = if self.plan.distinct {
            let hash = value::hash(&row)?;
            if let Some(bucket) = self.distinct.get(&hash) {
                for &index in bucket {
                    if value::same(&self.records[index].row, &row)? {
                        budget.release(charge);
                        return Ok(());
                    }
                }
            }
            Some(hash)
        } else {
            None
        };
        limits::row_limit(self.records.len())?;
        let mut order = Vec::new();
        for item in &self.plan.order_by {
            let value = if let Some(index) = self.plan.projection.iter().position(|p| p.expr == item.expr) {
                // Account for the copied sort value before the copy.
                let bytes = limits::value_bytes(&row[index]).checked_mul(2).ok_or("Sort key size overflow")?;
                budget.reserve(bytes)?;
                row[index].clone()
            } else {
                let value = evaluate(&item.expr)?;
                budget.reserve(limits::value_bytes(&value).checked_mul(2).ok_or("Sort key size overflow")?)?;
                value
            };
            order.push(value);
        }
        if let Some(hash) = hash {
            if !self.distinct.contains_key(&hash) {
                budget.reserve(HASH_ENTRY)?;
            }
            budget.reserve(INDEX_ENTRY)?;
            self.distinct.entry(hash).or_default().push(self.records.len());
        }
        self.records.push(Record { row, order });
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<Row>> {
        let mut failure = None;
        if !self.plan.order_by.is_empty() {
            // Stable sorting preserves scan order when all sort keys are equal.
            self.records.sort_by(|a, b| {
                if failure.is_some() {
                    return Ordering::Equal;
                }
                for ((a, b), spec) in a.order.iter().zip(&b.order).zip(&self.plan.order_by) {
                    let nulls_first = spec.nulls_first.unwrap_or(!spec.ascending);
                    let order = match (a, b) {
                        (Value::Null, Value::Null) => Ordering::Equal,
                        (Value::Null, _) => {
                            if nulls_first {
                                Ordering::Less
                            } else {
                                Ordering::Greater
                            }
                        }
                        (_, Value::Null) => {
                            if nulls_first {
                                Ordering::Greater
                            } else {
                                Ordering::Less
                            }
                        }
                        _ => match value::compare(a, b) {
                            Ok(order) => {
                                if spec.ascending {
                                    order
                                } else {
                                    order.reverse()
                                }
                            }
                            Err(error) => {
                                failure = Some(error);
                                return Ordering::Equal;
                            }
                        },
                    };
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                Ordering::Equal
            });
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(self
            .records
            .into_iter()
            .skip(self.plan.offset)
            .take(self.plan.limit.unwrap_or(usize::MAX))
            .map(|record| record.row)
            .collect())
    }
}
