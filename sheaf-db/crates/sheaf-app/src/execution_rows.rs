use crate::eval;
use crate::eval::value;
use crate::executor::limits::{self, Budget, HASH_ENTRY, INDEX_ENTRY};
use crate::model::*;
use crate::relational_plan::{ExecutablePlan, JoinAlgorithm, ScanPlan};
use std::collections::HashMap;

type Index = HashMap<u64, Vec<usize>>;
struct Loaded {
    rows: Vec<Row>,
    index: Option<Index>,
}

fn load(source: &impl DataSource, scan: &ScanPlan, budget: &mut Budget, stats: &mut ExecutionStats) -> Result<Loaded> {
    // The source prunes pages before decode. The row filter below still runs,
    // so a kept page is only a cost and a skipped page never loses a match.
    let (mut rows, page_stats) = source.scan(&scan.relation.table, &scan.columns, &scan.predicates)?;
    stats.pages_total = stats.pages_total.saturating_add(page_stats.pages_total);
    stats.pages_skipped = stats.pages_skipped.saturating_add(page_stats.pages_skipped);
    stats.page_clauses = stats.page_clauses.saturating_add(page_stats.page_clauses);
    budget.reserve(limits::source_bytes(&rows)?)?;
    stats.scanned_rows = stats.scanned_rows.checked_add(rows.len()).ok_or("Scan count overflow")?;
    for row in &rows {
        if row.len() != scan.schema.len() {
            return Err("Source row width does not match schema".into());
        }
        for (value, column) in row.iter().zip(&scan.schema) {
            value::finite(value)?;
            let valid = match (value, column.data_type) {
                (Value::Null, _) => column.nullable,
                (Value::Int(_), DataType::Int | DataType::Float)
                | (Value::Float(_), DataType::Float)
                | (Value::Text(_), DataType::Text)
                | (Value::Bool(_), DataType::Bool) => true,
                _ => false,
            };
            if !valid {
                return Err(format!("Source value violates type or nullability of {}", column.name));
            }
        }
    }
    let mut failure = None;
    rows.retain(|row| {
        if failure.is_some() {
            return false;
        }
        for predicate in &scan.predicates {
            match eval::eval(predicate, &scan.columns, row).and_then(eval::truth) {
                Ok(true) => (),
                Ok(false) => return false,
                Err(error) => {
                    failure = Some(error);
                    return false;
                }
            }
        }
        true
    });
    if let Some(error) = failure {
        return Err(error);
    }
    Ok(Loaded { rows, index: None })
}

fn make_index(rows: &[Row], column: usize, budget: &mut Budget) -> Result<Index> {
    let mut index = Index::new();
    for (position, row) in rows.iter().enumerate() {
        let key = &row[column];
        if matches!(key, Value::Null) {
            continue;
        }
        let hash = value::hash(std::slice::from_ref(key))?;
        if !index.contains_key(&hash) {
            budget.reserve(HASH_ENTRY)?;
        }
        budget.reserve(INDEX_ENTRY)?;
        index.entry(hash).or_default().push(position);
    }
    Ok(index)
}

fn count_candidate(stats: &mut ExecutionStats) -> Result<()> {
    limits::row_limit(stats.candidate_rows)?;
    stats.candidate_rows += 1;
    Ok(())
}

fn append(prefix: &mut Row, row: &Row, budget: &mut Budget) -> Result<usize> {
    let text_bytes = row.iter().try_fold(0usize, |sum, v| match v {
        Value::Text(text) => sum.checked_add(text.len()).ok_or("Joined row size overflow"),
        _ => Ok(sum),
    })?;
    budget.reserve(text_bytes)?;
    prefix.extend(row.iter().cloned());
    Ok(text_bytes)
}

struct Walker<'a> {
    plan: &'a ExecutablePlan,
    loaded: &'a [Loaded],
}
impl Walker<'_> {
    fn walk(
        &self,
        level: usize,
        prefix: &mut Row,
        budget: &mut Budget,
        stats: &mut ExecutionStats,
        sink: &mut impl FnMut(&Row, &mut Budget) -> Result<()>,
    ) -> Result<()> {
        if level == self.plan.joins.len() {
            if let Some(filter) = &self.plan.filter {
                if !eval::truth(eval::eval(filter, &self.plan.columns, prefix)?)? {
                    return Ok(());
                }
            }
            return sink(prefix, budget);
        }
        let join = &self.plan.joins[level];
        if matches!(join.algorithm, JoinAlgorithm::HashIndex { .. }) {
            return self.hash_walk(level, prefix, budget, stats, sink);
        }
        let right = &self.loaded[level + 1];
        for position in 0..right.rows.len() {
            count_candidate(stats)?;
            let length = prefix.len();
            let charge = append(prefix, &right.rows[position], budget)?;
            // ON always sees the original left-to-right join layout.
            let passes = eval::truth(eval::eval(&join.on, &self.plan.columns[..prefix.len()], prefix)?)?;
            if passes {
                self.walk(level + 1, prefix, budget, stats, sink)?;
            }
            prefix.truncate(length);
            budget.release(charge);
        }
        Ok(())
    }

    fn hash_walk(
        &self,
        level: usize,
        prefix: &mut Row,
        budget: &mut Budget,
        stats: &mut ExecutionStats,
        sink: &mut impl FnMut(&Row, &mut Budget) -> Result<()>,
    ) -> Result<()> {
        let join = &self.plan.joins[level];
        let JoinAlgorithm::HashIndex { left_column, .. } = join.algorithm else { unreachable!() };
        if matches!(prefix[left_column], Value::Null) {
            return Ok(());
        }
        let hash = value::hash(std::slice::from_ref(&prefix[left_column]))?;
        let right = &self.loaded[level + 1];
        if let Some(positions) = right.index.as_ref().and_then(|index| index.get(&hash)) {
            for &position in positions {
                count_candidate(stats)?;
                let length = prefix.len();
                let charge = append(prefix, &right.rows[position], budget)?;
                if eval::truth(eval::eval(&join.on, &self.plan.columns[..prefix.len()], prefix)?)? {
                    self.walk(level + 1, prefix, budget, stats, sink)?;
                }
                prefix.truncate(length);
                budget.release(charge);
            }
        }
        Ok(())
    }
}

pub fn stream(
    source: &impl DataSource,
    plan: &ExecutablePlan,
    budget: &mut Budget,
    stats: &mut ExecutionStats,
    mut sink: impl FnMut(&Row, &mut Budget) -> Result<()>,
) -> Result<()> {
    let mut loaded = Vec::with_capacity(plan.joins.len() + 1);
    loaded.push(load(source, &plan.from, budget, stats)?);
    for join in &plan.joins {
        let mut right = load(source, &join.scan, budget, stats)?;
        if let JoinAlgorithm::HashIndex { right_column, .. } = join.algorithm {
            right.index = Some(make_index(&right.rows, right_column, budget)?);
        }
        loaded.push(right);
    }
    let prefix_bytes =
        plan.columns.len().checked_mul(std::mem::size_of::<Value>()).ok_or("Joined row width overflow")?;
    budget.reserve(prefix_bytes)?;
    let mut prefix = Vec::with_capacity(plan.columns.len());
    let walker = Walker { plan, loaded: &loaded };
    for row in &loaded[0].rows {
        if plan.joins.is_empty() {
            count_candidate(stats)?;
        }
        let charge = append(&mut prefix, row, budget)?;
        walker.walk(0, &mut prefix, budget, stats, &mut sink)?;
        prefix.clear();
        budget.release(charge);
    }
    // Charges remain conservative through finalization. All source storage drops here.
    Ok(())
}
