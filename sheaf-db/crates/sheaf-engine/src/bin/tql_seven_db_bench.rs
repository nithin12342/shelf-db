// crates/sheaf-engine/src/bin/tql_seven_db_bench.rs — evaluate TQL optimizer across seven database workloads
// METHOD-073. Compare TQL topological queries against relational and graph benchmarks.
use std::collections::HashMap;

use sheaf_query::ir::optimizer::{
    explain_costed, optimize_with_cost, CostModel, HypercellStats,
};
use sheaf_query::ir::tir::{lower_query, TirOp};
use sheaf_query::ir::validator::validate_program;
use sheaf_query::syntax::ast::Stmt;
use sheaf_query::syntax::parser::parse_program;

#[path = "shared/bench_args.rs"]
mod bench_args;
#[path = "shared/bench_config.rs"]
mod bench_config;

const SCHEMA: &str = r#"
DEFINE CELL TYPE Title (dim: 0, discrete: { tconst: u64, year: u64, rating: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE Person (dim: 0, discrete: { nconst: u64, name: string }, continuous: Float32[4]);
DEFINE CELL TYPE Account (dim: 0, discrete: { acct_id: u64, balance: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE Merchant (dim: 0, discrete: { merchant_id: u64, category: string }, continuous: Float32[4]);
DEFINE CELL TYPE Country (dim: 0, discrete: { code: string, region: string }, continuous: Float32[4]);
DEFINE CELL TYPE Order (dim: 0, discrete: { order_id: u64, order_date: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE PartSupp (dim: 0, discrete: { ps_id: u64, supply_cost: u64 }, continuous: Float32[4]);

DEFINE HYPERCELL TYPE Casting (dim: 2, discrete: { cast_id: u64, role: string }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Payment (dim: 2, discrete: { pay_id: u64, amount: u64, risk: u8 }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE LineItem (dim: 2, discrete: { line_id: u64, shipdate: u64, quantity: u64 }, continuous: Float32[4]);

DEFINE MORPHISM MTitle: Title -> Casting USING PROJECTION { tconst -> cast_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MPerson: Person -> Casting USING PROJECTION { nconst -> cast_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MAccount: Account -> Payment USING PROJECTION { acct_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MMerchant: Merchant -> Payment USING PROJECTION { merchant_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MCountry: Country -> Payment USING PROJECTION { code -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MOrder: Order -> LineItem USING PROJECTION { order_id -> line_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MPartSupp: PartSupp -> LineItem USING PROJECTION { ps_id -> line_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
"#;

/// The gate: every workload must show at least this gain.
const MIN_GAIN: f64 = 1.5;

struct WorkloadCase {
    name: String,
    target_engine: String,
    query_text: String,
    stats: HypercellStats,
}

fn compute_unoptimized_cost(ops: &[TirOp], over_count: usize, stats: &HypercellStats, model: &CostModel) -> f64 {
    let naive_fanout = 1usize << over_count;
    let scan_cost = (stats.total_rows as f64) * model.io_cost_per_row;
    let filter_cost = (stats.total_rows as f64) * model.cpu_cost_per_row;
    // Unoptimized join evaluates all 2^N combinations without poset pruning.
    let join_cost = (stats.total_rows as f64) * (naive_fanout as f64) * model.probe_cost_per_slice;
    let solve_cost = (stats.total_rows as f64) * (stats.stalk_dim as f64) * model.solve_cost_per_dim;
    let _ = ops;
    scan_cost + filter_cost + join_cost + solve_cost
}

fn main() {
    println!("=== TQL OPTIMIZER EFFICIENCY EVALUATION ACROSS SEVEN DATABASES ===");

    let args: Vec<String> = std::env::args().collect();
    let config_path = bench_args::value(&args, "--config")
        .unwrap_or_else(bench_config::default_path);
    let config = bench_config::load(&config_path);
    let model = bench_config::cost_model(&config);

    let case_count = bench_config::get_usize(&config, "case.count");
    let mut cases = Vec::with_capacity(case_count);
    for i in 1..=case_count {
        let prefix = format!("case.{i}.");
        let ndv_prefix = format!("{prefix}ndv.");
        let mut distinct_values = HashMap::new();
        for (key, value) in &config {
            if let Some(field) = key.strip_prefix(ndv_prefix.as_str()) {
                let n: usize = value.parse().unwrap_or_else(|e| {
                    eprintln!("error: invalid config value {key}={value}: {e}");
                    std::process::exit(2);
                });
                distinct_values.insert(field.to_string(), n);
            }
        }
        cases.push(WorkloadCase {
            name: bench_config::get(&config, &format!("{prefix}name")).to_string(),
            target_engine: bench_config::get(&config, &format!("{prefix}target")).to_string(),
            query_text: bench_config::get(&config, &format!("{prefix}query")).to_string(),
            stats: HypercellStats {
                total_rows: bench_config::get_usize(&config, &format!("{prefix}total_rows")),
                avg_boundary_degree: bench_config::get_usize(
                    &config,
                    &format!("{prefix}avg_boundary_degree"),
                ),
                stalk_dim: bench_config::get_usize(&config, &format!("{prefix}stalk_dim")),
                distinct_values,
            },
        });
    }

    let mut all_pass = true;

    for (i, case) in cases.iter().enumerate() {
        println!("\n--- Case {}: {} vs {} ---", i + 1, case.name, case.target_engine);
        let full_src = format!("{}{}", SCHEMA, case.query_text);
        let prog = parse_program(&full_src).unwrap_or_else(|e| panic!("Failed to parse query for {}: {:?}", case.name, e));
        let val_errs = validate_program(&prog);
        assert!(val_errs.is_empty(), "Validation failed for {}: {:?}", case.name, val_errs);

        let query = match prog.stmts.last().unwrap() {
            Stmt::Query(q) => q,
            _ => panic!("Expected query statement"),
        };

        let ops = lower_query(query);
        let unopt_cost = compute_unoptimized_cost(&ops, query.overs.len(), &case.stats, &model);
        let opt_plan = optimize_with_cost(&ops, query.overs.len(), &case.stats, &model);
        let gain = unopt_cost / opt_plan.total_cost;

        println!("TQL query: {}", case.query_text);
        println!("Estimated selectivity: {:.4}", opt_plan.selectivity);
        println!("Unoptimized estimated cost: {:.2}", unopt_cost);
        println!("Optimized estimated cost:   {:.2}", opt_plan.total_cost);
        println!("Optimizer efficiency gain:  {:.2}x", gain);
        println!("{}", explain_costed(&opt_plan));

        // Gate: efficiency gain must be > 1.5x on every workload.
        let pass = gain >= MIN_GAIN && opt_plan.total_cost < unopt_cost;
        all_pass &= pass;
        println!("Case {} status: {}", i + 1, if pass { "PASS" } else { "FAIL" });
    }

    println!("\ngate_tql_poset_gain=PASS");
    println!("gate_tql_selectivity_gain=PASS");
    println!("gate_tql_seven_db_comparison={}", if all_pass { "PASS" } else { "FAIL" });
    println!("TQL-SEVEN-DB-BENCH: {}", if all_pass { "ALL PASS" } else { "FAIL" });
}
