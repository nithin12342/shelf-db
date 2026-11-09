// crates/sheaf-engine/src/bin/sheaf_db_app.rs — standalone executable application
// Full pipeline: ingest TQL script → parse → validate → BCSF build → kernel
// solve → .section emit. This is the production entry point for sheaf-db.
use std::collections::HashMap;
use std::time::Instant;

use sheaf_engine::api::result::{CohomologyState, SectionResult};
use sheaf_kernel::operators::coboundary::CoboundaryOp;
use sheaf_kernel::operators::laplacian::SheafLaplacian;
use sheaf_kernel::solvers::harmonic::harmonic_relax_target;
use sheaf_kernel::solvers::tikhonov::RelaxConfig;
use sheaf_query::ir::tir::lower_query;
use sheaf_query::ir::optimizer::{explain, prune_plan};
use sheaf_query::ir::validator::validate_program;
use sheaf_query::syntax::ast::{EmitItem, Expr, Lit, Program, Stmt};
use sheaf_query::syntax::parser::parse_program;
use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::reader::BcsfReader;

type MatchRow = (usize, HashMap<String, HashMap<String, Lit>>, Vec<usize>);

/// Fraud net: accounts, merchants, countries + payment hypercells.
/// Query: find high-risk transactions (amount > 1000) with root access.
const FRAUD_SCHEMA: &str = r#"
DEFINE CELL TYPE Account (dim: 0, discrete: { acct_id: u64, name: string, balance: u64 }, continuous: Float32[4]);
DEFINE CELL TYPE Merchant (dim: 0, discrete: { merchant_id: u64, category: string, is_rooted: bool }, continuous: Float32[4]);
DEFINE CELL TYPE Country (dim: 0, discrete: { code: string, region: string }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Payment (dim: 2, discrete: { pay_id: u64, amount: u64, risk: u8 }, continuous: Float32[4]);
DEFINE MORPHISM MAccount: Account -> Payment USING PROJECTION { acct_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MMerchant: Merchant -> Payment USING PROJECTION { merchant_id -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MCountry: Country -> Payment USING PROJECTION { code -> pay_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
"#;

const FRAUD_DATA: &str = r#"
INSERT CELL Account (1001) DISCRETE { acct_id: 1001, name: "Alice", balance: 5000 } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Account (1002) DISCRETE { acct_id: 1002, name: "Bob", balance: 300 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Account (1003) DISCRETE { acct_id: 1003, name: "Carol", balance: 9999 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Account (1004) DISCRETE { acct_id: 1004, name: "Dave", balance: 12000 } CONTINUOUS [0.20, -0.38, 0.60, 0.11];
INSERT CELL Account (1005) DISCRETE { acct_id: 1005, name: "Eve", balance: 750 } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT CELL Merchant (2001) DISCRETE { merchant_id: 2001, category: "grocery", is_rooted: false } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT CELL Merchant (2002) DISCRETE { merchant_id: 2002, category: "travel", is_rooted: true } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Merchant (2003) DISCRETE { merchant_id: 2003, category: "electronics", is_rooted: false } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Country (3001) DISCRETE { code: "US", region: "na" } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Country (3002) DISCRETE { code: "UK", region: "eu" } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT HYPERCELL Payment (90001) OVER BOUNDARY [Account(1001), Merchant(2001), Country(3001)] DISCRETE { pay_id: 90001, amount: 5000, risk: 1 } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT HYPERCELL Payment (90002) OVER BOUNDARY [Account(1002), Merchant(2002), Country(3002)] DISCRETE { pay_id: 90002, amount: 300, risk: 0 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT HYPERCELL Payment (90003) OVER BOUNDARY [Account(1003), Merchant(2001), Country(3001)] DISCRETE { pay_id: 90003, amount: 9999, risk: 1 } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT HYPERCELL Payment (90004) OVER BOUNDARY [Account(1004), Merchant(2002), Country(3002)] DISCRETE { pay_id: 90004, amount: 12000, risk: 0 } CONTINUOUS [0.20, -0.38, 0.60, 0.11];
INSERT HYPERCELL Payment (90005) OVER BOUNDARY [Account(1005), Merchant(2003), Country(3001)] DISCRETE { pay_id: 90005, amount: 750, risk: 1 } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
"#;

fn lit_to_string(l: &Lit) -> String {
    match l {
        Lit::Int(v) => v.to_string(),
        Lit::Float(v) => format!("{:.6}", v),
        Lit::Str(v) => v.clone(),
        Lit::Bool(v) => v.to_string(),
    }
}

fn cmp_numeric(op: &sheaf_query::syntax::ast::CmpOp, a: f64, b: f64) -> bool {
    use sheaf_query::syntax::ast::CmpOp::*;
    match op {
        Gt => a > b,
        Lt => a < b,
        Ge => a >= b,
        Le => a <= b,
        Eq => a == b,
        Ne => a != b,
    }
}

fn eval_expr(env: &HashMap<String, HashMap<String, Lit>>, e: &Expr) -> bool {
    match e {
        Expr::And(a, b) => eval_expr(env, a) && eval_expr(env, b),
        Expr::Or(a, b) => eval_expr(env, a) || eval_expr(env, b),
        Expr::Not(x) => !eval_expr(env, x),
        Expr::Cmp { path, op, lit } => {
            if path.len() < 2 {
                return false;
            }
            match env.get(&path[0]).and_then(|m| m.get(&path[1])) {
                Some(v) => {
                    use sheaf_query::syntax::ast::CmpOp::*;
                    match op {
                        Eq => lit_to_string(v) == lit_to_string(lit),
                        Ne => lit_to_string(v) != lit_to_string(lit),
                        Gt | Lt | Ge | Le => {
                            let pair = match (v, lit) {
                                (Lit::Int(a), Lit::Int(b)) => Some((*a as f64, *b as f64)),
                                (Lit::Float(a), Lit::Float(b)) => Some((*a, *b)),
                                (Lit::Int(a), Lit::Float(b)) => Some((*a as f64, *b)),
                                (Lit::Float(a), Lit::Int(b)) => Some((*a, *b as f64)),
                                _ => None,
                            };
                            match pair {
                                Some((a, b)) => cmp_numeric(op, a, b),
                                None => false,
                            }
                        }
                    }
                }
                None => false,
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // Parse optional --data-file flag.
    let mut data_file: Option<String> = None;
    let mut positional: Vec<String> = Vec::new();
    let mut i = 1usize;
    while i < args.len() {
        if args[i] == "--data-file" && i + 1 < args.len() {
            data_file = Some(args[i + 1].clone());
            i += 2;
            continue;
        }
        positional.push(args[i].clone());
        i += 1;
    }
    let query = if !positional.is_empty() {
        positional[0].clone()
    } else {
        "MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant, c: Country) WHERE t.amount > 1000 AND t.risk == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.name, m.category, c.code, t.amount, t.risk, SECTION_ENERGY, COHOMOLOGY_STATE };".to_string()
    };
    let src = if let Some(df) = &data_file {
        let data = std::fs::read_to_string(df).unwrap_or_else(|e| {
            eprintln!("DATA FILE ERROR: {}", e);
            std::process::exit(1);
        });
        format!("{}{}", data, query)
    } else {
        format!("{}{}{}", FRAUD_SCHEMA, FRAUD_DATA, query)
    };
    // Phase 1: Parse + validate
    let prog: Program = match parse_program(&src) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("PARSE ERROR: {}", e);
            std::process::exit(1);
        }
    };
    if let Some(e) = validate_program(&prog).first() {
        eprintln!("VALIDATION ERROR: {}", e.msg);
        std::process::exit(1);
    }
    // Phase 2: Build BCSF and Pre-folded Topological Fiber Bundle
    #[derive(Clone, Debug)]
    struct FiberSection {
        cell: usize,
        face_ids: Vec<usize>,
        boundary_refs: Vec<(String, (String, u64))>,
        discrete: Vec<(String, Lit)>,
    }
    let mut fiber_bundle: HashMap<String, Vec<FiberSection>> = HashMap::new();

    let mut builder = BcsfBuilder::new();
    let mut id_of: HashMap<(String, u64), usize> = HashMap::new();
    let mut gid_to_pair: HashMap<usize, (String, u64)> = HashMap::new();
    let mut dis_map: HashMap<(String, u64), Vec<(String, Lit)>> = HashMap::new();
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
                builder.push(0, Vec::new(), &db, &c.stalk, &[]);
                id_of.insert((c.type_name.clone(), c.id), next_gid);
                gid_to_pair.insert(next_gid, (c.type_name.clone(), c.id));
                next_gid += 1;
                dis_map.insert((c.type_name.clone(), c.id), c.discrete.clone());
            }
            Stmt::InsertHyper(h) => {
                let mut bnd = Vec::new();
                let mut face_ids = Vec::new();
                let mut bnd_refs = Vec::new();
                for r in &h.boundary {
                    let gid = *id_of.get(&(r.type_name.clone(), r.id)).unwrap();
                    bnd.push(gid as u64);
                    face_ids.push(gid);
                    bnd_refs.push((r.type_name.clone(), (r.type_name.clone(), r.id)));
                }
                let mut db = [0u8; 16];
                let class = *class_of.entry(h.type_name.clone()).or_insert_with(|| {
                    next_class += 1;
                    next_class - 1
                });
                db[..8].copy_from_slice(&class.to_le_bytes());
                let morph: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
                builder.push(2, bnd, &db, &h.stalk, &morph);
                id_of.insert((h.type_name.clone(), h.id), next_gid);
                gid_to_pair.insert(next_gid, (h.type_name.clone(), h.id));
                fiber_bundle.entry(h.type_name.clone()).or_default().push(FiberSection {
                    cell: next_gid,
                    face_ids,
                    boundary_refs: bnd_refs,
                    discrete: h.discrete.clone(),
                });
                next_gid += 1;
                dis_map.insert((h.type_name.clone(), h.id), h.discrete.clone());
            }
            _ => {}
        }
    }
    let reader: BcsfReader = builder.finish();
    if let Err(e) = reader.validate() {
        eprintln!("BCSF VALIDATION ERROR: {}", e);
        std::process::exit(1);
    }
    // Phase 3: Execute query
    let query_ast = match prog.stmts.last().unwrap() {
        Stmt::Query(q) => q.clone(),
        _ => {
            eprintln!("No query found");
            std::process::exit(1);
        }
    };
    let ops = lower_query(&query_ast);
    let plan = prune_plan(&ops, query_ast.overs.len());
    let t_start = Instant::now();
    // Phase 4: Match + solve using Pre-folded Topological Fiber Bundle
    let mut matched: Vec<MatchRow> = Vec::new();
    if let Some(sections) = fiber_bundle.get(&query_ast.hyper_type) {
        for sec in sections {
            if sec.boundary_refs.len() != query_ast.overs.len() {
                continue;
            }
            let mut env: HashMap<String, HashMap<String, Lit>> = HashMap::new();
            let mut ok = true;
            for ((var, typ), (b_typ, pair)) in query_ast.overs.iter().zip(sec.boundary_refs.iter()) {
                if b_typ != typ {
                    ok = false;
                    break;
                }
                env.insert(var.clone(), dis_map[pair].iter().cloned().collect());
            }
            if !ok {
                continue;
            }
            env.insert(query_ast.hyper_var.clone(), sec.discrete.iter().cloned().collect());
            if let Some(e) = &query_ast.where_clause {
                if !eval_expr(&env, e) {
                    continue;
                }
            }
            matched.push((sec.cell, env, sec.face_ids.clone()));
        }
    }
    // Phase 5: Kernel solve per match
    let d = 4usize;
    let faces: Vec<u32> = (0..3).collect();
    let signs: Vec<f32> = vec![1.0, -1.0, 1.0];
    let morph: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let op = CoboundaryOp::new(faces, signs, morph, d);
    let lap = SheafLaplacian::build(3, d, vec![op]);
    let mut all_results: Vec<SectionResult> = Vec::new();
    for (hcell, env, face_ids) in &matched {
        let mut x0 = vec![0.0f32; face_ids.len() * d];
        for (p, g) in face_ids.iter().enumerate() {
            x0[p * d..(p + 1) * d].copy_from_slice(reader.stalk(*g));
        }
        let t0 = Instant::now();
        let (xsol, res, _) = harmonic_relax_target(
            &lap,
            &x0,
            RelaxConfig {
                lambda: query_ast.solve.regularization as f32,
                tol: 1e-6,
                max_iter: query_ast.solve.max_iterations,
                energy_target: 1e-4,
                check_every: 1,
                n_threads: 1,
            },
        );
        let elapsed_ns = t0.elapsed().as_nanos();
        let exec_us = ((elapsed_ns + 999) / 1000).max(1) as u64;
        let hpair = gid_to_pair[hcell].clone();
        let mut payload: Vec<(String, String)> = Vec::new();
        let mut projections: Vec<(String, Vec<f32>)> = Vec::new();
        let mut cells: Vec<(String, u8)> = vec![(format!("{}(#{})", hpair.0, hpair.1), 2u8)];
        for item in &query_ast.emit {
            match item {
                EmitItem::Field(path) => {
                    let val = if path[0] == query_ast.hyper_var {
                        lit_to_string(&dis_map[&hpair].iter().find(|(k, _)| k == &path[1]).map(|(_, v)| v.clone()).unwrap())
                    } else {
                        lit_to_string(&env[&path[0]][&path[1]])
                    };
                    payload.push((format!("{}.{}", path[0], path[1]), val));
                }
                EmitItem::SectionEnergy => {
                    payload.push(("SECTION_ENERGY".to_string(), format!("{:.6}", res.energy)));
                }
                EmitItem::CohomologyState => {
                    payload.push(("COHOMOLOGY_STATE".to_string(), CohomologyState::certify(res.energy, query_ast.solve.energy_threshold).as_str().to_string()));
                }
            }
        }
        for (idx, (var, _)) in query_ast.overs.iter().enumerate() {
            let pair = gid_to_pair[&face_ids[idx]].clone();
            cells.push((format!("{}(#{})", pair.0, pair.1), 0u8));
            projections.push((var.clone(), xsol[idx * d..(idx + 1) * d].to_vec()));
        }
        all_results.push(SectionResult {
            status: "OK".to_string(),
            execution_time_us: exec_us,
            dimension: 2,
            cells,
            discrete_payload: payload,
            dirichlet_energy: res.energy,
            iterations_to_converge: res.iters,
            cohomology_state: CohomologyState::certify(res.energy, query_ast.solve.energy_threshold),
            stalk_projections: projections,
        });
    }
    let total_us = ((t_start.elapsed().as_nanos() + 999) / 1000).max(1) as u64;
    let median_us = if !all_results.is_empty() {
        (all_results.iter().map(|r| r.execution_time_us).sum::<u64>() / all_results.len() as u64).max(1)
    } else {
        total_us
    };
    // Output
    println!("=== sheaf-db query result ===");
    println!("Matched {} hypercells in {} us (median {} us)", matched.len(), total_us, median_us);
    for (i, r) in all_results.iter().enumerate() {
        println!("--- result {} ---", i);
        print!("{}", r.render());
    }
    // EXPLAIN
    println!("--- EXPLAIN ---");
    print!("{}", explain(&plan));

}
