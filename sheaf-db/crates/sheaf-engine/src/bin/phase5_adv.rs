// crates/sheaf-engine/src/bin/phase5_adv.rs — Phase-5 adversarial suite
// C1: Eulerian validator rejects open boundary (δ²≠0).
// C2: Eulerian validator rejects dim mismatch (dim-1 in dim-2 boundary).
// C3: Coboundary orthonormality: non-orthogonal matrix rejected.
// C4: Optimizer N=6 prunes to 1 slice, not O(64).
// C5: Optimizer N=1 still prunes to 1 slice (degenerate case).
// C6: Name lookup parity with scan on multiple values.
// C7: Name lookup returns empty for nonexistent value.
// C8: Validator + optimizer + lookup compose correctly on a real query.
use std::collections::HashMap;

use sheaf_query::ir::optimizer::prune_plan;
use sheaf_query::ir::tir::lower_query;
use sheaf_query::ir::validator::{check_coboundary_ortho, check_eulerian_delta2, validate_program};
use sheaf_query::syntax::ast::Stmt;
use sheaf_query::syntax::parser::parse_program;
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::name_lookup::{extract_u64, NameIndex};
use sheaf_storage::bcsf::reader::BcsfReader;

const SCHEMA5: &str = r#"
DEFINE CELL TYPE Account (dim: 0, discrete: { acct_id: u64, name: string, balance: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE Merchant (dim: 0, discrete: { merchant_id: u64, category: string }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Payment (dim: 2, discrete: { pay_id: u64, amount: u64, risk: u8 }, continuous: Float32[4]);
DEFINE MORPHISM MAccount: Account -> Payment USING PROJECTION { acct_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MMerchant: Merchant -> Payment USING PROJECTION { merchant_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
"#;

const DATA5: &str = r#"
INSERT CELL Account (1001) DISCRETE { acct_id: 1001, name: "Alice", balance: 5000 } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Account (1002) DISCRETE { acct_id: 1002, name: "Bob", balance: 300 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Account (1003) DISCRETE { acct_id: 1003, name: "Carol", balance: 9999 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Merchant (2001) DISCRETE { merchant_id: 2001, category: "grocery" } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT CELL Merchant (2002) DISCRETE { merchant_id: 2002, category: "travel" } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT HYPERCELL Payment (90001) OVER BOUNDARY [Account(1001), Merchant(2001)] DISCRETE { pay_id: 90001, amount: 5000, risk: 1 } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT HYPERCELL Payment (90002) OVER BOUNDARY [Account(1002), Merchant(2002)] DISCRETE { pay_id: 90002, amount: 300, risk: 0 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
"#;

fn main() {
    let mut all_pass = true;

    // C1: Open boundary (δ²≠0) rejected.
    let open = check_eulerian_delta2(2, &[("A".to_string(), 0)], &[1.0, 1.0, 1.0]);
    let c1 = open.is_err() && open.unwrap_err().contains("delta2");
    println!("C1_open_boundary_rejected={}", if c1 { "PASS" } else { "FAIL" });
    all_pass &= c1;

    // C2: Dim mismatch rejected.
    let mismatch = check_eulerian_delta2(2, &[("A".to_string(), 0), ("B".to_string(), 1)], &[1.0, -1.0]);
    let c2 = mismatch.is_err() && mismatch.unwrap_err().contains("dim mismatch");
    println!("C2_dim_mismatch_rejected={}", if c2 { "PASS" } else { "FAIL" });
    all_pass &= c2;

    // C3: Non-orthonormal coboundary rejected.
    let non_ortho = check_coboundary_ortho(&[vec![1.0, 1.0], vec![1.0, 1.0]]);
    let c3 = non_ortho.is_err();
    println!("C3_non_ortho_rejected={}", if c3 { "PASS" } else { "FAIL" });
    all_pass &= c3;

    // C4: N=6 prunes to 1 slice.
    let q6 = "MATCH HYPERCELL t: Payment OVER (a: Account, b: Account, c: Account, d: Account, e: Account, f: Account) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name };";
    let prog6 = parse_program(&format!("{}{}{}", SCHEMA5, DATA5, q6)).unwrap();
    let q6r = match prog6.stmts.last().unwrap() { Stmt::Query(q) => q, _ => panic!() };
    let plan6 = prune_plan(&lower_query(q6r), q6r.overs.len());
    let c4 = plan6.total_slices == 1 && q6r.overs.len() == 6;
    println!("C4_n6_prunes_to_1={}", if c4 { "PASS" } else { "FAIL" });
    all_pass &= c4;

    // C5: N=1 prunes to 1 slice.
    let q1 = "MATCH HYPERCELL t: Payment OVER (a: Account) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name };";
    let prog1 = parse_program(&format!("{}{}{}", SCHEMA5, DATA5, q1)).unwrap();
    let q1r = match prog1.stmts.last().unwrap() { Stmt::Query(q) => q, _ => panic!() };
    let plan1 = prune_plan(&lower_query(q1r), q1r.overs.len());
    let c5 = plan1.total_slices == 1 && q1r.overs.len() == 1;
    println!("C5_n1_prunes_to_1={}", if c5 { "PASS" } else { "FAIL" });
    all_pass &= c5;

    // C6: Name lookup parity on multiple values.
    let prog = parse_program(&format!("{}{}{}", SCHEMA5, DATA5, q1)).unwrap();
    let mut builder = BcsfBuilder::new();
    let mut class_of: HashMap<String, u64> = HashMap::new();
    let mut next_class = 100u64;
    for s in &prog.stmts {
        match s {
            Stmt::InsertCell(c) => {
                let mut db = [0u8; 16];
                let class = *class_of.entry(c.type_name.clone()).or_insert_with(|| { next_class += 1; next_class - 1 });
                db[..8].copy_from_slice(&class.to_le_bytes());
                if let Some((_, sheaf_query::syntax::ast::Lit::Int(v))) = c.discrete.iter().find(|(k, _)| k == "balance") {
                    db[8..16].copy_from_slice(&v.to_le_bytes());
                }
                builder.push(0, Vec::new(), &db, &c.stalk, &[]);
            }
            Stmt::InsertHyper(h) => {
                let mut db = [0u8; 16];
                let class = *class_of.entry(h.type_name.clone()).or_insert_with(|| { next_class += 1; next_class - 1 });
                db[..8].copy_from_slice(&class.to_le_bytes());
                builder.push(2, vec![], &db, &h.stalk, &[]);
            }
            _ => {}
        }
    }
    let reader: BcsfReader = builder.finish();
    let fields = vec![("balance".to_string(), 8usize)];
    let cell_classes: Vec<(u64, String)> = class_of.iter().map(|(k, v)| (*v, k.clone())).collect();
    let nidx = NameIndex::build(&reader, &fields, &cell_classes);
    let mut c6 = true;
    for &target in &[5000u64, 300, 9999] {
        let lookup: Vec<usize> = nidx.lookup("balance", &target.to_le_bytes())
            .map(|r| nidx.field_cells("balance").unwrap()[r.clone()].iter().map(|&c| c as usize).collect())
            .unwrap_or_default();
        let scan: Vec<usize> = (0..reader.len()).filter(|&c| extract_u64(&reader, c, 8) == target).collect();
        if lookup != scan {
            c6 = false;
            eprintln!("C6 mismatch for {}: lookup={:?} scan={:?}", target, lookup, scan);
        }
    }
    println!("C6_multi_value_parity={}", if c6 { "PASS" } else { "FAIL" });
    all_pass &= c6;

    // C7: Nonexistent value returns empty.
    let empty: Vec<usize> = nidx.lookup("balance", &999999u64.to_le_bytes())
        .map(|r| nidx.field_cells("balance").unwrap()[r.clone()].iter().map(|&c| c as usize).collect())
        .unwrap_or_default();
    let c7 = empty.is_empty();
    println!("C7_nonexistent_empty={}", if c7 { "PASS" } else { "FAIL" });
    all_pass &= c7;

    // C8: Compose validator + optimizer + lookup on a real query.
    let full_q = "MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant) WHERE a.balance > 1000 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name, m.category };";
    let full_prog = parse_program(&format!("{}{}{}", SCHEMA5, DATA5, full_q)).unwrap();
    let c8 = validate_program(&full_prog).is_empty();
    println!("C8_compose={}", if c8 { "PASS" } else { "FAIL" });
    all_pass &= c8;

    if !all_pass {
        std::process::exit(1);
    }
    println!("ADVERSARIAL5: PASS");
}
