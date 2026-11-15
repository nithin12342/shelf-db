use crate::aggregate::Aggregator;
use crate::eval;
use crate::model::{DataSource, ExecutionStats, Query, QueryOutput, Result};
use crate::relational_plan;
use std::time::Instant;

#[path = "execution_limits.rs"]
pub(crate) mod limits;
#[path = "execution_output.rs"]
mod output;
#[path = "execution_rows.rs"]
mod rows;

/// Explain validates names, types, grouping, and aliases without calling rows().
pub fn explain(source: &impl DataSource, query: &Query, optimize: bool) -> Result<Vec<String>> {
    Ok(relational_plan::build(source, query, optimize)?.describe())
}

pub fn execute(source: &impl DataSource, query: &Query, optimize: bool) -> Result<QueryOutput> {
    let started = Instant::now();
    let plan = relational_plan::build(source, query, optimize)?;
    let description = plan.describe();
    let mut budget = limits::Budget::default();
    let mut stats = ExecutionStats::default();
    let mut output = output::Output::new(&plan);
    if plan.limit != Some(0) {
        if plan.aggregate {
            let mut aggregate = Aggregator::new(&plan, &mut budget)?;
            rows::stream(source, &plan, &mut budget, &mut stats, |row, budget| aggregate.consume(row, budget))?;
            aggregate.finish(|evaluate| output.collect(evaluate, &mut budget))?;
        } else {
            rows::stream(source, &plan, &mut budget, &mut stats, |row, budget| {
                output.collect(&mut |expr| eval::eval(expr, &plan.columns, row), budget)
            })?;
        }
    }
    let rows = output.finish()?;
    stats.output_rows = rows.len();
    stats.elapsed_us = started.elapsed().as_micros();
    Ok(QueryOutput { columns: plan.output_columns, rows, affected: 0, tql: None, plan: description, stats })
}
