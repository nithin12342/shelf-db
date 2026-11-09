// crates/sheaf-engine/src/bin/mt_scan_bench.rs — MAX-PERF P1 gate (METHOD-024)
// Loads lineitem price/discount into two plain Vecs (load untimed/reported),
// then ONE timed partitioned_sum per thread count (process-level repetition
// only). Gates: sum exact (rel < 1e-9 vs 218102223881.86), hard <= 8 ms
// parity, stretch <= 6.4 ms (DuckDB quiet).
use std::time::Instant;

use sheaf_kernel::simd::mt_scan::partitioned_sum;

const EXPECTED_ROWS: usize = 6_001_215;
const EXPECTED_SUM: f64 = 218102223881.86;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let lineitem = bench_args::required_path(&args, "--lineitem");

    let t0 = Instant::now();
    let content = std::fs::read(&lineitem).expect("lineitem.tbl missing");
    let mut price: Vec<f32> = Vec::with_capacity(EXPECTED_ROWS);
    let mut disc: Vec<f32> = Vec::with_capacity(EXPECTED_ROWS);
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
    println!("mt_load: rows={} load_s={:.1}", price.len(), t0.elapsed().as_secs_f64());
    assert_eq!(price.len(), EXPECTED_ROWS, "row count mismatch");

    for &nt in &[1usize, 2, 4] {
        let t0 = Instant::now();
        let sum = partitioned_sum(&price, &disc, nt);
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let rel = ((sum - EXPECTED_SUM) / EXPECTED_SUM).abs();
        println!("mt_scan_t{}: rep_ms={:.1} (single rep; run binary 3x, take best) rel={:.2e}", nt, ms, rel);
        assert!(rel < 1e-9, "sum mismatch at {} threads", nt);
    }
    println!("MT-SCAN-BENCH: COMPLETE (compare across thread counts + vs 6.4ms)");
}
