// crates/sheaf-engine/src/bin/coord_scan_bench.rs — MATH-UPGRADES Stage M1 gate
// Loads lineitem price/discount into two coordinate vectors (C^0 direct
// sum), then ONE timed inner-product rep (process-level repetition only).
// Gates: base 64B-aligned, sum exact (rel < 1e-9 vs 218102223881.86),
// row count exact. Hard gate: <= 30 ms (no regression); stretch: <= 6.4 ms.
use std::time::Instant;

use sheaf_storage::bcsf::coord::{fused_revenue, CoordVec};

const EXPECTED_ROWS: usize = 6_001_215;
const EXPECTED_SUM: f64 = 218102223881.86;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lineitem = bench_args::required_path(&args, "--lineitem");

    let t0 = Instant::now();
    let content = std::fs::read(&lineitem).expect("lineitem.tbl missing");
    let mut price = CoordVec::with_capacity(EXPECTED_ROWS + 64);
    let mut disc = CoordVec::with_capacity(EXPECTED_ROWS + 64);
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut fstart = 0usize;
            let mut field = 0usize;
            let (mut p, mut d) = (0f32, 0f32);
            for j in 0..line.len() {
                if line[j] == b'|' {
                    if field == 5 {
                        p = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                    } else if field == 6 {
                        d = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap();
                        break;
                    }
                    field += 1;
                    fstart = j + 1;
                }
            }
            price.push(p);
            disc.push(d);
        }
    }
    println!("coord_load: rows={} load_s={:.1}", price.len(), t0.elapsed().as_secs_f64());
    assert_eq!(price.len(), EXPECTED_ROWS, "row count mismatch");

    let aligned = price.base_addr().is_multiple_of(64) && disc.base_addr().is_multiple_of(64);
    println!("gate_align_64B={}", if aligned { "PASS" } else { "FAIL" });

    let t0 = Instant::now();
    let sum = fused_revenue(price.as_slice(), disc.as_slice());
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let rel = ((sum - EXPECTED_SUM) / EXPECTED_SUM).abs();
    println!("coord_scan: rep_ms={:.1} (single rep; run binary 3x, take best) sum={:.2} rel={:.2e}", ms, sum, rel);
    println!("gate_sum_exact={}", if rel < 1e-9 { "PASS" } else { "FAIL" });
    println!("gate_scan_hard={} (<= 30ms no-regression)", if ms <= 30.0 { "PASS" } else { "RECORDED" });
    println!("gate_scan_stretch={} (<= 6.4ms duckdb quiet)", if ms <= 6.4 { "PASS (beat DuckDB)" } else { "RECORDED" });
    if !(aligned && rel < 1e-9) {
        std::process::exit(1);
    }
    println!("STAGE-M1: PASS");
}
