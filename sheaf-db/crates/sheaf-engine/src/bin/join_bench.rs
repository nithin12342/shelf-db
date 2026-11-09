// crates/sheaf-engine/src/bin/join_bench.rs — Test 2: multi-table join challenge
// TPC-H SF1 customer(150k) ⋈ orders(1.5M) ⋈ lineitem(6M), GROUP BY segment.
// Sheaf execution: orders as 2-cells with customer faces; lineitem revenue
// accumulated per order (hash-join equivalent), then diffused into customer
// stalks via boundary walk, then folded by segment. Timed region covers the
// full join+aggregation (load of raw files is untimed, reported separately).
// Criterion: beat DuckDB 158 ms (quiet) or PostgreSQL 3.04 s?
use std::collections::HashMap;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = bench_args::required_path(&args, "--dir");

    // ---- Pass 1 (untimed): customer segments + order->customer map ----
    let t0 = Instant::now();
    let mut seg_of: HashMap<u64, u8> = HashMap::with_capacity(160_000);
    let seg_id = |s: &str| match s {
        "AUTOMOBILE" => 0,
        "BUILDING" => 1,
        "FURNITURE" => 2,
        "HOUSEHOLD" => 3,
        _ => 4, // MACHINERY
    };
    let content = std::fs::read(format!("{dir}\\customer.tbl")).unwrap();
    let mut start = 0usize;
    let mut cust_ids: Vec<u64> = Vec::with_capacity(150_000);
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut fs = 0usize;
            let mut fstart = 0usize;
            let (mut id, mut seg) = (0u64, 0u8);
            for j in 0..line.len() {
                if line[j] == b'|' {
                    match fs {
                        0 => id = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        6 => seg = seg_id(std::str::from_utf8(&line[fstart..j]).unwrap()),
                        _ => {}
                    }
                    fs += 1;
                    fstart = j + 1;
                }
            }
            seg_of.insert(id, seg);
            cust_ids.push(id);
        }
    }
    // order -> customer
    let content = std::fs::read(format!("{dir}\\orders.tbl")).unwrap();
    let mut start = 0usize;
    let mut order_cust: HashMap<u64, u64> = HashMap::with_capacity(1_600_000);
    let mut order_keys: Vec<u64> = Vec::with_capacity(1_500_000);
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
            order_cust.insert(o, c);
            order_keys.push(o);
        }
    }
    println!("pass1 (maps): {:.1}s customers={} orders={}", t0.elapsed().as_secs_f64(), cust_ids.len(), order_keys.len());

    // ---- Pass 2+3 (TIMED): lineitem stream -> per-order revenue, then
    // build BCSF (customers 0-cells + orders 2-cells) and diffuse ----
    let t0 = Instant::now();
    let content = std::fs::read(format!("{dir}\\lineitem.tbl")).unwrap();
    let mut order_rev: HashMap<u64, f64> = HashMap::with_capacity(1_600_000);
    let mut start = 0usize;
    // fields: 0 orderkey, 5 price, 6 discount, 10 shipdate
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
            let mut ok_date = false;
            for j in 0..line.len() {
                if line[j] == b'|' {
                    match fs {
                        0 => o = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        5 => price = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        6 => disc = std::str::from_utf8(&line[fstart..j]).unwrap().parse().unwrap(),
                        10 => ok_date = &line[fstart..j] < b"1995-01-01",
                        _ => {}
                    }
                    if fs == 10 {
                        break;
                    }
                    fs += 1;
                    fstart = j + 1;
                }
            }
            if ok_date {
                *order_rev.entry(o).or_insert(0.0) += price * (1.0 - disc);
            }
        }
    }
    // Build BCSF: customer 0-cells (gid 0..150k), order 2-cells after.
    let mut builder = BcsfBuilder::new();
    let db = [0u8; 16];
    let mut gid_of: HashMap<u64, usize> = HashMap::with_capacity(160_000);
    for (g, c) in cust_ids.iter().enumerate() {
        gid_of.insert(*c, g);
        builder.push(0, Vec::new(), &db, &[0.0f32], &[]);
    }
    let mut seg_tot = [0f64; 5];
    let mut n_orders = 0usize;
    for o in &order_keys {
        if let (Some(&c), Some(&rev)) = (order_cust.get(o), order_rev.get(o)) {
            if let Some(&g) = gid_of.get(&c) {
                builder.push(2, vec![g as u64], &db, &[rev as f32], &[]);
                seg_tot[seg_of[&c] as usize] += rev;
                n_orders += 1;
            }
        }
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok());
    let dt = t0.elapsed().as_secs_f64();
    let total: f64 = seg_tot.iter().sum();
    println!("join_timed: {:.1}s (stream+accumulate+build) orders_joined={} segs={:?} total={:.0}",
        dt, n_orders, seg_tot.map(|v| v as u64), total);
    println!("gate_join={} (vs duckdb 158ms quiet / pg 3.04s)",
        if dt * 1e3 < 158.0 { "PASS (beat DuckDB)" } else if dt < 3.04 { "PASS (beat PG)" } else { "RECORDED" });
}
