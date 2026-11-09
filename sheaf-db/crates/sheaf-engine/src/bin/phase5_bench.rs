// crates/sheaf-engine/src/bin/phase5_bench.rs — Phase-5 optimizer + validator E2E
// METHOD-009: Eulerian validator rejects delta2!=0 and dim mismatches.
// METHOD-010: N=6 EXPLAIN shows single slice, not O(64) fanout.
// METHOD-011: name lookup returns contiguous slice; parity with scan.
use std::collections::HashMap;

use sheaf_query::ir::optimizer::{
    explain, explain_costed, optimize_with_cost, prune_plan, HypercellStats,
};
use sheaf_query::ir::tir::lower_query;
use sheaf_query::ir::validator::{check_coboundary_ortho, check_eulerian_delta2, validate_program};
use sheaf_query::syntax::ast::Stmt;
use sheaf_query::syntax::parser::parse_program;
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::name_lookup::{extract_u64, NameIndex};
use sheaf_storage::bcsf::reader::BcsfReader;

#[path = "shared/bench_args.rs"]
mod bench_args;
#[path = "shared/bench_config.rs"]
mod bench_config;

const SCHEMA5: &str = r#"
DEFINE CELL TYPE Account (dim: 0, discrete: { acct_id: u64, name: string, balance: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE Merchant (dim: 0, discrete: { merchant_id: u64, category: string }, continuous: Float32[4]);
DEFINE CELL TYPE Country (dim: 0, discrete: { code: string, region: string }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Payment (dim: 2, discrete: { pay_id: u64, amount: u64, risk: u8 }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Refund (dim: 2, discrete: { ref_id: u64, reason: string }, continuous: Float32[4]);
DEFINE MORPHISM MAccount: Account -> Payment USING PROJECTION { acct_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MMerchant: Merchant -> Payment USING PROJECTION { merchant_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MCountry: Country -> Payment USING PROJECTION { code -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
"#;

const DATA5: &str = r#"
INSERT CELL Account (1001) DISCRETE { acct_id: 1001, name: "Alice", balance: 5000 } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Account (1002) DISCRETE { acct_id: 1002, name: "Bob", balance: 300 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Account (1003) DISCRETE { acct_id: 1003, name: "Carol", balance: 9999 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Account (1004) DISCRETE { acct_id: 1004, name: "Dave", balance: 12000 } CONTINUOUS [0.20, -0.38, 0.60, 0.11];
INSERT CELL Account (1005) DISCRETE { acct_id: 1005, name: "Eve", balance: 750 } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT CELL Account (1006) DISCRETE { acct_id: 1006, name: "Frank", balance: 4200 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Merchant (2001) DISCRETE { merchant_id: 2001, category: "grocery" } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT CELL Merchant (2002) DISCRETE { merchant_id: 2002, category: "travel" } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Country (3001) DISCRETE { code: "US", region: "na" } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Country (3002) DISCRETE { code: "UK", region: "eu" } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT HYPERCELL Payment (90001) OVER BOUNDARY [Account(1001), Merchant(2001), Country(3001)] DISCRETE { pay_id: 90001, amount: 5000, risk: 1 } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT HYPERCELL Payment (90002) OVER BOUNDARY [Account(1002), Merchant(2002), Country(3002)] DISCRETE { pay_id: 90002, amount: 300, risk: 0 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT HYPERCELL Payment (90003) OVER BOUNDARY [Account(1003), Merchant(2001), Country(3001)] DISCRETE { pay_id: 90003, amount: 9999, risk: 1 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT HYPERCELL Payment (90004) OVER BOUNDARY [Account(1004), Merchant(2002), Country(3002)] DISCRETE { pay_id: 90004, amount: 12000, risk: 0 } CONTINUOUS [0.20, -0.38, 0.60, 0.11];
INSERT HYPERCELL Payment (90005) OVER BOUNDARY [Account(1005), Merchant(2001), Country(3001)] DISCRETE { pay_id: 90005, amount: 750, risk: 1 } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT HYPERCELL Payment (90006) OVER BOUNDARY [Account(1006), Merchant(2002), Country(3002)] DISCRETE { pay_id: 90006, amount: 4200, risk: 0 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
"#;

fn full_program5(query: &str) -> String {
    format!("{}{}{}", SCHEMA5, DATA5, query)
}

fn main() {
    // ---- METHOD-009: Eulerian validator ----------------------------------
    // Valid: closed boundary (signed sum = 0).
    let ok_complex = vec![("Account".to_string(), 0u8), ("Merchant".to_string(), 0u8), ("Country".to_string(), 0u8)];
    let ok_signs = vec![1.0f32, -1.0f32, 1.0f32, -1.0f32]; // two pairs cancel
    let euler_ok = check_eulerian_delta2(2, &ok_complex, &ok_signs);
    // Invalid: open boundary (signed sum != 0).
    let bad_signs = vec![1.0f32, 1.0f32, 1.0f32, 1.0f32]; // no cancellation
    let euler_bad = check_eulerian_delta2(2, &ok_complex, &bad_signs);
    // Invalid: dim mismatch (dim-1 cell in dim-2 boundary).
    let dim_mismatch = vec![("Account".to_string(), 0u8), ("Bad".to_string(), 1u8)];
    let euler_dim = check_eulerian_delta2(2, &dim_mismatch, &ok_signs);
    let mut euler_pass = euler_ok.is_ok()
        && euler_bad.is_err()
        && euler_bad.unwrap_err().contains("delta2")
        && euler_dim.is_err()
        && euler_dim.unwrap_err().contains("dim mismatch");
    // Orthonormal coboundary: identity matrix.
    let ortho_ok = check_coboundary_ortho(&[
        vec![1.0, 0.0, 0.0, 0.0],
        vec![0.0, 1.0, 0.0, 0.0],
        vec![0.0, 0.0, 1.0, 0.0],
        vec![0.0, 0.0, 0.0, 1.0],
    ]);
    // Non-orthonormal: column of all ones.
    let ortho_bad = check_coboundary_ortho(&[
        vec![1.0, 1.0],
        vec![1.0, 1.0],
    ]);
    euler_pass &= ortho_ok.is_ok() && ortho_bad.is_err();
    println!("gate_eulerian_validator={}", if euler_pass { "PASS" } else { "FAIL" });

    // ---- METHOD-010: Optimizer EXPLAIN ------------------------------------
    // N=6 query: 6 OVER variables. Naive = O(64) fanout; pruned = 1 slice.
    let n6_query = "MATCH HYPERCELL t: Payment OVER (a: Account, b: Account, c: Account, d: Account, e: Account, f: Account) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name, SECTION_ENERGY };";
    let prog6 = parse_program(&full_program5(n6_query)).unwrap();
    assert!(validate_program(&prog6).is_empty());
    let q6 = match prog6.stmts.last().unwrap() { Stmt::Query(q) => q, _ => panic!() };
    let ops6 = lower_query(q6);
    let plan6 = prune_plan(&ops6, q6.overs.len());
    let explain6 = explain(&plan6);
    let optimizer_pass = plan6.total_slices == 1
        && q6.overs.len() == 6
        && explain6.contains("1 slices")
        && explain6.contains("PosetJoin");
    println!("gate_optimizer_explain={}", if optimizer_pass { "PASS" } else { "FAIL" });
    println!("{}", explain6);

    let stats = HypercellStats {
        total_rows: 500,
        ..Default::default()
    };
    let args: Vec<String> = std::env::args().collect();
    let config_path = bench_args::value(&args, "--config")
        .unwrap_or_else(bench_config::default_path);
    let config = bench_config::load(&config_path);
    let model = bench_config::cost_model(&config);
    let costed_plan = optimize_with_cost(&ops6, q6.overs.len(), &stats, &model);
    let costed_explain = explain_costed(&costed_plan);
    let cost_pass = costed_plan.total_cost > 0.0
        && costed_plan.scan_cost > 0.0
        && costed_explain.contains("COST: total=");
    println!("gate_statistical_optimizer={}", if cost_pass { "PASS" } else { "FAIL" });

    // ---- METHOD-011: Name lookup parity ------------------------------------
    let prog = parse_program(&full_program5(
        "MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant, c: Country) WHERE a.balance > 1000 AND t.risk == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name, m.category, t.amount, SECTION_ENERGY };",
    ))
    .unwrap();
    assert!(validate_program(&prog).is_empty());
    // Build BCSF from fixture data.
    let mut builder = BcsfBuilder::new();
    let mut id_of: HashMap<(String, u64), usize> = HashMap::new();
    let mut next_gid = 0usize;
    let mut class_of: HashMap<String, u64> = HashMap::new();
    let mut next_class = 100u64;
    for s in &prog.stmts {
        match s {
            Stmt::InsertCell(c) => {
                let mut db = [0u8; 16];
                let class = *class_of.entry(c.type_name.clone()).or_insert_with(|| {
                    next_class += 1;
                    next_class - 1
                });
                db[..8].copy_from_slice(&class.to_le_bytes());
                // balance at offset 8 (after 8-byte class), u64 LE
                if let Some((_, sheaf_query::syntax::ast::Lit::Int(v))) = c.discrete.iter().find(|(k, _)| k == "balance") {
                    db[8..16].copy_from_slice(&v.to_le_bytes());
                }
                builder.push(0, Vec::new(), &db, &c.stalk, &[]);
                id_of.insert((c.type_name.clone(), c.id), next_gid);
                next_gid += 1;
            }
            Stmt::InsertHyper(h) => {
                let mut bnd = Vec::new();
                for r in &h.boundary {
                    bnd.push(*id_of.get(&(r.type_name.clone(), r.id)).unwrap() as u64);
                }
                let mut db = [0u8; 16];
                let class = *class_of.entry(h.type_name.clone()).or_insert_with(|| {
                    next_class += 1;
                    next_class - 1
                });
                db[..8].copy_from_slice(&class.to_le_bytes());
                builder.push(2, bnd, &db, &h.stalk, &[]);
                id_of.insert((h.type_name.clone(), h.id), next_gid);
                next_gid += 1;
            }
            _ => {}
        }
    }
    let reader: BcsfReader = builder.finish();
    assert!(reader.validate().is_ok());
    // Build name index on Account.balance (offset 8).
    let fields = vec![("balance".to_string(), 8usize)];
    let cell_classes: Vec<(u64, String)> = class_of.iter().map(|(k, v)| (*v, k.clone())).collect();
    let nidx = NameIndex::build(&reader, &fields, &cell_classes);
    // Lookup balance = 5000 (Alice, cell 0).
    let alice_balance = 5000u64.to_le_bytes();
    let lookup_result = nidx.lookup("balance", &alice_balance);
    // Scan parity: find all cells with balance = 5000.
    let mut scan_result: Vec<usize> = Vec::new();
    for cell in 0..reader.len() {
        if extract_u64(&reader, cell, 8) == 5000u64 {
            scan_result.push(cell);
        }
    }
    let lookup_cells: Vec<usize> = lookup_result
        .map(|r| nidx.field_cells("balance").unwrap()[r.clone()].iter().map(|&c| c as usize).collect())
        .unwrap_or_default();
    // Parity: same cell set.
    let parity_ok = lookup_cells == scan_result
        && lookup_cells == vec![0]; // Alice is the only balance=5000 account.
    println!("gate_name_lookup_parity={}", if parity_ok { "PASS" } else { "FAIL" });

    if !(euler_pass && optimizer_pass && parity_ok) {
        std::process::exit(1);
    }
    println!("PHASE5 E2E: PASS");
}
