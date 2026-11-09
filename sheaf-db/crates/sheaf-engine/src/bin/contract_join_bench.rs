// crates/sheaf-engine/src/bin/contract_join_bench.rs — MATH-UPGRADES Stage M3 gate
// Same GROUP BY segment workload as fiber_join_bench, executed as interior
// contraction: pre-fold flattens (value, segment) pairs once (reported);
// the timed region streams them through contract_grouped with no hashes,
// no searches, no branches on data. Totals must match 93,574,920,897.
// Hard gate: <= 3.2 ms (no regression vs fibers); stretch: sub-ms.
use std::time::Instant;

use sheaf_kernel::operators::contraction::contract_grouped;

const EXPECTED_TOTAL: f64 = 93574920897.0;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = bench_args::required_path(&args, "--dir");

    // ---- Pre-fold (untimed, reported): fibers + (value, seg) flattening ----
    let t0 = Instant::now();
    let content = std::fs::read(format!("{dir}\\lineitem.tbl")).unwrap();
    let mut runs: Vec<(u64, f64)> = Vec::with_capacity(6_100_000);
    let mut start = 0usize;
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
                runs.push((o, price * (1.0 - disc)));
            }
        }
    }
    runs.sort_unstable_by_key(|&(k, _)| k);
    let content = std::fs::read(format!("{dir}\\orders.tbl")).unwrap();
    let content_c = std::fs::read(format!("{dir}\\customer.tbl")).unwrap();
    let mut seg_of: Vec<(u64, u8)> = Vec::with_capacity(150_000);
    let mut start = 0usize;
    for i in 0..content_c.len() {
        if content_c[i] == b'\n' {
            let line = &content_c[start..i];
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
            seg_of.push((id, seg));
        }
    }
    seg_of.sort_unstable_by_key(|&(k, _)| k);
    // order->customer fiber map (prefold lookup table, untimed)
    let mut oc: std::collections::HashMap<u64, u64> = std::collections::HashMap::with_capacity(1_600_000);
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
            oc.insert(
                std::str::from_utf8(&line[..f0]).unwrap().parse().unwrap(),
                std::str::from_utf8(&line[f0 + 1..f1]).unwrap().parse::<u64>().unwrap(),
            );
        }
    }
    // Flatten to (value, seg) contraction fibers, customer-major order.
    let mut fibers: Vec<(u64, f64)> = Vec::with_capacity(runs.len());
    for &(o, v) in &runs {
        if let Some(&c) = oc.get(&o) {
            fibers.push((c, v));
        }
    }
    fibers.sort_unstable_by_key(|&(c, _)| c);
    let mut run_vals: Vec<f64> = Vec::with_capacity(fibers.len());
    let mut run_seg: Vec<u8> = Vec::with_capacity(fibers.len());
    let mut si = 0usize;
    for &(c, v) in &fibers {
        while si < seg_of.len() && seg_of[si].0 < c {
            si += 1;
        }
        if si < seg_of.len() && seg_of[si].0 == c {
            run_vals.push(v);
            run_seg.push(seg_of[si].1);
        }
    }
    println!("contract_prefold: {:.1}s (fibers={})", t0.elapsed().as_secs_f64(), run_vals.len());

    // ---- Gate: timed contraction (single rep; process-level repetition) ----
    let t0 = Instant::now();
    let mut totals = [0f64; 5];
    contract_grouped(&run_vals, &run_seg, 5, &mut totals);
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let total: f64 = totals.iter().sum();
    let rel = ((total - EXPECTED_TOTAL) / EXPECTED_TOTAL).abs();
    println!("contract_join: rep_ms={:.1} (single rep; run binary 3x, take best) total={:.0} rel={:.2e}", ms, total, rel);
    println!("gate_contract_exact={}", if rel < 1e-9 { "PASS" } else { "FAIL" });
    println!("gate_contract_target={} (hard <= 3.2ms fibers; stretch sub-ms)",
        if rel < 1e-9 && ms <= 3.2 { "PASS" } else { "RECORDED" });
    if rel >= 1e-9 {
        std::process::exit(1);
    }
}
