// crates/sheaf-engine/src/bin/preagg_bench.rs — MAX-PERF P3 gate (METHOD-025)
// Streams lineitem revenues through PreAgg keyed by precomputed order->seg
// map (built once at load, untimed). Timed: per-row probe + O(1) accumulate
// over 6M rows, then O(#groups) readout. Totals must match 93,574,920,897.
// Hard gate: totals exact.
use std::collections::HashMap;
use std::time::Instant;

use sheaf_storage::lsp::preagg::PreAgg;

const EXPECTED_TOTAL: f64 = 93574920897.0;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = bench_args::required_path(&args, "--dir");

    // ---- Load (untimed, reported): seg map + order->seg map ----
    let t0 = Instant::now();
    let content = std::fs::read(format!("{dir}\\customer.tbl")).unwrap();
    let mut seg_of: HashMap<u64, u8> = HashMap::with_capacity(160_000);
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut e = 0usize;
            while line[e] != b'|' {
                e += 1;
            }
            let id: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
            let mut f = 0usize;
            let mut fs2 = e + 1;
            let mut seg = 4u8;
            for j in e + 1..line.len() {
                if line[j] == b'|' {
                    if f == 5 {
                        seg = match &line[fs2..j] {
                            b"AUTOMOBILE" => 0,
                            b"BUILDING" => 1,
                            b"FURNITURE" => 2,
                            b"HOUSEHOLD" => 3,
                            _ => 4,
                        };
                        break;
                    }
                    f += 1;
                    fs2 = j + 1;
                }
            }
            seg_of.insert(id, seg);
        }
    }
    let content = std::fs::read(format!("{dir}\\orders.tbl")).unwrap();
    let mut order_seg: HashMap<u64, u8> = HashMap::with_capacity(1_600_000);
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut f0 = 0usize;
            while line[f0] != b'|' {
                f0 += 1;
            }
            let mut f1 = f0 + 1;
            while line[f1] != b'|' {
                f1 += 1;
            }
            let o: u64 = std::str::from_utf8(&line[..f0]).unwrap().parse().unwrap();
            let c: u64 = std::str::from_utf8(&line[f0 + 1..f1]).unwrap().parse().unwrap();
            if let Some(&s) = seg_of.get(&c) {
                order_seg.insert(o, s);
            }
        }
    }
    println!("preagg_load: {:.1}s (order->seg entries={})", t0.elapsed().as_secs_f64(), order_seg.len());

    // ---- Gate: timed ingest (probe + O(1) accumulate per row) ----
    let content = std::fs::read(format!("{dir}\\lineitem.tbl")).unwrap();
    let mut agg = PreAgg::new();
    let t0 = Instant::now();
    let mut start = 0usize;
    let mut n = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut fs = 0usize;
            let mut fstart = 0usize;
            let (mut o, mut price, mut disc) = (0u64, 0f64, 0f64);
            let mut ok = false;
            for j in 0..line.len() {
                if line[j] == b'|' {
                    match fs {
                        0 => o = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        5 => price = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        6 => disc = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        10 => {
                            ok = &line[fstart..j] < b"1995-01-01";
                            break;
                        }
                        _ => {}
                    }
                    fs += 1;
                    fstart = j + 1;
                }
            }
            if ok {
                if let Some(&s) = order_seg.get(&o) {
                    agg.ingest(s as u32, price * (1.0 - disc));
                    n += 1;
                }
            }
        }
    }
    let ingest_s = t0.elapsed().as_secs_f64();
    // Gate: O(#groups) readout.
    let t0 = Instant::now();
    let rows = agg.readout();
    let readout_us = t0.elapsed().as_secs_f64() * 1e6;
    let total: f64 = rows.iter().map(|&(_, s, _)| s).sum();
    let rel = ((total - EXPECTED_TOTAL) / EXPECTED_TOTAL).abs();
    println!("preagg_ingest: {:.1}s over {} events ({:.0} events/s)", ingest_s, n, n as f64 / ingest_s);
    println!("preagg_readout: {:.1}us groups={} total={:.0} rel={:.2e}", readout_us, rows.len(), total, rel);
    println!("gate_preagg_exact={}", if rel < 1e-9 { "PASS" } else { "FAIL" });
    if rel >= 1e-9 {
        std::process::exit(1);
    }
    println!("STAGE-P3: PASS");
}
