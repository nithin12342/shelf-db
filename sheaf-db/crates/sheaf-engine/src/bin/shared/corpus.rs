//! Shared corpus for phase4_bench — bin-local module (NOT part of the library).
//! Lives under src/bin/shared/ so cargo never builds it as a target or links
// Extracted from phase4_bench.rs to satisfy SPEC-022 (≤500 lines per file).
use std::collections::HashMap;
use sheaf_query::syntax::ast::{CmpOp, Expr, Lit};

const SCHEMA: &str = r#"
DEFINE CELL TYPE Person (dim: 0, discrete: { id: u64, name: string, status: u8 }, continuous: Float32[4]);
DEFINE CELL TYPE Device (dim: 0, discrete: { device_id: string, is_rooted: bool }, continuous: Float32[4]);
DEFINE CELL TYPE Location (dim: 0, discrete: { zone: string }, continuous: Float32[4]);
DEFINE HYPERCELL TYPE Transaction (dim: 2, discrete: { tx_id: u64, amount: f64 }, continuous: Float32[4]);
DEFINE MORPHISM MPerson: Person -> Transaction USING PROJECTION { id -> tx_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MDevice: Device -> Transaction USING PROJECTION { device_id -> tx_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
DEFINE MORPHISM MLocation: Location -> Transaction USING PROJECTION { zone -> tx_id } AND MATRIX [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]];
"#;

const DATA: &str = r#"
INSERT CELL Person (101) DISCRETE { id: 101, name: "Alice", status: 1 } CONTINUOUS [0.25, -0.40, 0.88, 0.12];
INSERT CELL Person (102) DISCRETE { id: 102, name: "Bob", status: 0 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
INSERT CELL Device (501) DISCRETE { device_id: "dev_xyz", is_rooted: false } CONTINUOUS [0.10, -0.35, 0.75, 0.05];
INSERT CELL Device (502) DISCRETE { device_id: "dev_q", is_rooted: true } CONTINUOUS [-0.50, 0.10, 0.20, -0.30];
INSERT CELL Location (301) DISCRETE { zone: "nyc" } CONTINUOUS [0.20, -0.38, 0.60, 0.11];
INSERT HYPERCELL Transaction (9001) OVER BOUNDARY [Person(101), Device(501), Location(301)] DISCRETE { tx_id: 9001, amount: 1500.0 } CONTINUOUS [0.20, -0.38, 0.60, 0.10];
INSERT HYPERCELL Transaction (9002) OVER BOUNDARY [Person(102), Device(502), Location(301)] DISCRETE { tx_id: 9002, amount: 50.0 } CONTINUOUS [0.10, 0.30, -0.20, 0.44];
"#;

pub fn valid_corpus() -> Vec<(String, String)> {
    let schema = SCHEMA;
    let data = DATA;
    let wheres = [
        "WHERE p.status == 1",
        "WHERE p.status != 0 AND d.is_rooted == false",
        "WHERE t.amount > 1000.0",
        "WHERE t.amount >= 50.0 AND l.zone == \"nyc\"",
        "WHERE NOT d.is_rooted == true",
        "WHERE p.status == 1 OR t.amount < 100.0",
    ];
    let thresholds = ["0.001", "0.005", "0.01", "0.05"];
    let emits = [
        "p.name, d.device_id, t.amount, SECTION_ENERGY, COHOMOLOGY_STATE",
        "p.name, SECTION_ENERGY",
        "t.amount, COHOMOLOGY_STATE, l.zone",
    ];
    let mut out = Vec::new();
    for i in 0..50 {
        let w = wheres[i % wheres.len()];
        let restrict = if i % 2 == 0 { "RESTRICT TO BOUNDARY (p, d) " } else { "" };
        let q = format!(
            "MATCH HYPERCELL t: Transaction OVER (p: Person, d: Device, l: Location) {} {}SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: {}) EMIT SECTION {{ {} }};",
            w, restrict, thresholds[i % thresholds.len()], emits[i % emits.len()]
        );
        out.push((format!("{}{}{}", schema, data, q), q));
    }
    out
}

pub struct BadCase {
    pub src: &'static str,
    pub kind: &'static str,
    pub line: usize,
    pub col: usize,
    pub names: &'static [&'static str],
    pub needs_schema: bool,
}

pub fn malformed_corpus() -> Vec<BadCase> {
    vec![
        BadCase { src: "FOO;", kind: "pos", line: 1, col: 1, names: &[], needs_schema: false },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name }",
            kind: "eol", line: 1, col: 0, names: &[], needs_schema: false,
        },
        BadCase { src: "MATCH @ HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };", kind: "pos", line: 1, col: 7, names: &[], needs_schema: false },
        BadCase { src: "\"abc", kind: "pos", line: 1, col: 5, names: &[], needs_schema: false },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name ",
            kind: "eol", line: 1, col: 0, names: &[], needs_schema: false,
        },
        BadCase { src: "SELECT p.name;", kind: "pos", line: 1, col: 1, names: &[], needs_schema: false },
        BadCase {
            src: "MATCH HYPERCELL t = Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "present", line: 1, col: 1, names: &["':'"], needs_schema: false,
        },
        BadCase {
            src: "INSERT CELL Person (101) DISCRETE { id: 101 } CONTINUOUS [1.0, 2.0;",
            kind: "present", line: 1, col: 1, names: &[], needs_schema: false,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Nope OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["Nope"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Nope) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["Nope"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) WHERE p.bogus == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["bogus"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) WHERE q.status == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["q"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name } RESTRICT TO BOUNDARY (zzz);",
            kind: "present", line: 1, col: 1, names: &[], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 0, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["MAX_ITERATIONS"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { };",
            kind: "named", line: 1, col: 1, names: &["EMIT"], needs_schema: true,
        },
        BadCase {
            src: "DEFINE CELL TYPE Person (dim: 0, discrete: { id: u64 }, continuous: Float32[4]); DEFINE CELL TYPE Person (dim: 0, discrete: { id: u64 }, continuous: Float32[4]);",
            kind: "named", line: 1, col: 1, names: &["Person"], needs_schema: true,
        },
        BadCase {
            src: "INSERT CELL Person (101) DISCRETE { id: 101 } CONTINUOUS [1.0];",
            kind: "named", line: 1, col: 1, names: &["Person"], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Person OVER (p: Person) SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "present", line: 1, col: 1, names: &[], needs_schema: true,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) SOLVE HARMONIC (BOGUS: 1.0, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name };",
            kind: "named", line: 1, col: 1, names: &["BOGUS"], needs_schema: false,
        },
        BadCase {
            src: "MATCH HYPERCELL t: Transaction OVER (p: Person) WHERE p.status == ;",
            kind: "present", line: 1, col: 1, names: &[], needs_schema: false,
        },
    ]
}

pub fn lit_to_string(l: &Lit) -> String {
    match l {
        Lit::Int(v) => v.to_string(),
        Lit::Float(v) => {
            let mut s = format!("{:.6}", v);
            while s.contains('.') && s.ends_with('0') {
                s.pop();
            }
            if s.ends_with('.') {
                s.push('0');
            }
            s
        }
        Lit::Str(v) => v.clone(),
        Lit::Bool(v) => v.to_string(),
    }
}

fn lit_eq(a: &Lit, b: &Lit) -> bool {
    match (a, b) {
        (Lit::Int(x), Lit::Int(y)) => x == y,
        (Lit::Float(x), Lit::Float(y)) => x == y,
        (Lit::Int(x), Lit::Float(y)) => (*x as f64) == *y,
        (Lit::Float(x), Lit::Int(y)) => *x == (*y as f64),
        (Lit::Str(x), Lit::Str(y)) => x == y,
        (Lit::Bool(x), Lit::Bool(y)) => x == y,
        _ => false,
    }
}

fn lit_cmp(path_val: &Lit, op: &CmpOp, lit: &Lit) -> bool {
    match op {
        CmpOp::Eq => lit_eq(path_val, lit),
        CmpOp::Ne => !lit_eq(path_val, lit),
        CmpOp::Gt | CmpOp::Lt | CmpOp::Ge | CmpOp::Le => {
            let pair = match (path_val, lit) {
                (Lit::Int(a), Lit::Int(b)) => Some((*a as f64, *b as f64)),
                (Lit::Float(a), Lit::Float(b)) => Some((*a, *b)),
                (Lit::Int(a), Lit::Float(b)) => Some((*a as f64, *b)),
                (Lit::Float(a), Lit::Int(b)) => Some((*a, *b as f64)),
                _ => None,
            };
            match pair {
                Some((a, b)) => match op {
                    CmpOp::Gt => a > b,
                    CmpOp::Lt => a < b,
                    CmpOp::Ge => a >= b,
                    CmpOp::Le => a <= b,
                    _ => false,
                },
                None => false,
            }
        }
    }
}

pub fn eval_expr(env: &HashMap<String, HashMap<String, Lit>>, e: &Expr) -> bool {
    match e {
        Expr::And(a, b) => eval_expr(env, a) && eval_expr(env, b),
        Expr::Or(a, b) => eval_expr(env, a) || eval_expr(env, b),
        Expr::Not(x) => !eval_expr(env, x),
        Expr::Cmp { path, op, lit } => {
            if path.len() < 2 {
                return false;
            }
            match env.get(&path[0]).and_then(|m| m.get(&path[1])) {
                Some(v) => lit_cmp(v, op, lit),
                None => false,
            }
        }
    }
}
