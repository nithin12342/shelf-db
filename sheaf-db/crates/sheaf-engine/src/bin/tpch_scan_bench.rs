// crates/sheaf-engine/src/bin/tpch_scan_bench.rs — Test 1: DuckDB scan challenge
// Loads TPC-H SF1 lineitem.tbl (6,001,215 rows) into BCSF 0-cells with
// stalk=[price, discount], then times a full-slab SUM(price*(1-discount)).
// Load is untimed (mirrors DuckDB COPY-then-query); scan timed x3.
// Criterion: beat DuckDB 17.9 ms?
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

const EXPECTED_ROWS: usize = 6_001_215;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lineitem = bench_args::required_path(&args, "--lineitem");

    // ---- Load (untimed, reported separately) ----
    let t0 = Instant::now();
    let mut builder = BcsfBuilder::new();
    let mut n = 0usize;
    let mut db = [0u8; 16];
    // Reuse one discrete block; per-row orderkey skipped for scan speed
    // (scan touches stalks only — discrete layout irrelevant to the gate).
    let content = std::fs::read(&lineitem).expect("lineitem.tbl missing");
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            // fields: orderkey|partkey|suppkey|lineno|qty|price|discount|...
            // find 6th and 7th fields without full split (scan-faithful parse)
            let mut fstart = 0usize;
            let mut field = 0usize;
            let mut price = 0f32;
            let mut disc = 0f32;
            for j in 0..line.len() {
                if line[j] == b'|' {
                    if field == 5 {
                        price = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                    } else if field == 6 {
                        disc = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                        break;
                    }
                    field += 1;
                    fstart = j + 1;
                }
            }
            builder.push(0, Vec::new(), &db, &[price, disc], &[]);
            n += 1;
            db[0] = db[0].wrapping_add(1);
        }
    }
    let reader = builder.finish();
    let load_s = t0.elapsed().as_secs_f64();
    assert_eq!(n, EXPECTED_ROWS, "row count mismatch");
    assert!(reader.validate().is_ok());
    println!("scan_load: rows={} load_s={:.1} resident_MB={:.1}", n, load_s,
        reader.resident_bytes() as f64 / 1e6);

    // ---- Gate: single timed rep (NOT in-process best-of-3: scalar evolution
    // hoists invariant pure-loop sums out of timed regions — see SYNTHESIS_PLAN
    // methodology finding. Repeat at PROCESS level, take best across runs.)
    let slab = &reader.arena.stalks;
    assert_eq!(slab.len(), n * 2);
    let t0 = Instant::now();
    let mut acc = 0f64;
    let mut i = 0usize;
    while i < slab.len() {
        acc += slab[i] as f64 * (1.0 - slab[i + 1] as f64);
        i += 2;
    }
    let sum = acc;
    let rep_ms = t0.elapsed().as_secs_f64() * 1e3;
    println!("scan_sum: rep_ms={:.1} (single rep; run binary 3x, take best) sum={:.2} target_duckdb_ms=6.4", rep_ms, sum);
    println!("gate_scan={}", if rep_ms < 6.4 { "PASS (beat DuckDB)" } else { "RECORDED (DuckDB wins)" });
}
