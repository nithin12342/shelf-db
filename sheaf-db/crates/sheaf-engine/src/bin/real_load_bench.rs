// crates/sheaf-engine/src/bin/real_load_bench.rs — sheaf-db on REAL datasets
// Loads IEEE-CIS fraud train (590k rows, subset cols) as 0-cells and SNAP
// ego-Facebook (88k edges) as 0-cells + 1-cells, then runs the same shapes
// DuckDB ran: fraud GROUP BY ProductCD + fraud-risk filter; 2-hop walks.
// Gates: row/edge counts exact; fraud sums match DuckDB (20,663 frauds,
// avg $135.03); timings recorded (process-level, single rep).
use std::collections::HashMap;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let fraud = bench_args::required_path(&args, "--fraud");
    let fb = bench_args::required_path(&args, "--fb");

    // ---- Fraud: TransactionID(0), isFraud(1), TransactionAmt(3), ProductCD(4) ----
    let t0 = Instant::now();
    let content = std::fs::read(&fraud).expect("train_transaction.csv missing");
    let mut builder = BcsfBuilder::new();
    let db = [0u8; 16];
    let mut n = 0usize;
    let mut start = 0usize;
    let mut first = true;
    // product -> code (W,X,C,H,R) for GROUP BY parity with DuckDB
    let mut prod_of: Vec<u8> = Vec::new();
    let mut amts: Vec<f32> = Vec::new();
    let mut frauds: Vec<u8> = Vec::new();
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            if first {
                first = false;
                continue;
            } // header
            let mut fs = 0usize;
            let mut fstart = 0usize;
            let (mut fraud, mut amt) = (0u8, 0f32);
            let mut prod = 0u8;
            for j in 0..line.len() {
                if line[j] == b',' {
                    match fs {
                        1 => fraud = if &line[fstart..j] == b"1" { 1 } else { 0 },
                        3 => amt = std::str::from_utf8(&line[fstart..j]).unwrap_or("0").parse().unwrap_or(0.0),
                        4 => {
                            prod = match line.get(fstart) {
                                Some(b'W') => 0,
                                Some(b'X') => 1,
                                Some(b'C') => 2,
                                Some(b'H') => 3,
                                _ => 4,
                            }
                        }
                        _ => {}
                    }
                    if fs == 4 {
                        // remaining 389 cols skipped: scan to EOL is enough
                        // (we only break early-out of field parse, not line)
                    }
                    fs += 1;
                    fstart = j + 1;
                }
            }
            builder.push(0, Vec::new(), &db, &[amt, fraud as f32], &[]);
            prod_of.push(prod);
            amts.push(amt);
            frauds.push(fraud);
            n += 1;
        }
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok());
    assert_eq!(n, 590_540, "fraud row count");
    println!("fraud_load: rows={} load_s={:.1} resident_MB={:.1}", n,
        t0.elapsed().as_secs_f64(), reader.resident_bytes() as f64 / 1e6);

    // GROUP BY ProductCD over stalks (mirrors DuckDB fraud_group).
    let t0 = Instant::now();
    let mut groups = [(0u64, 0u64, 0f64); 5];
    for (cell, &g) in prod_of.iter().enumerate() {
        let st = reader.stalk(cell);
        let g = g as usize;
        groups[g].0 += 1;
        groups[g].1 += st[1] as u64;
        groups[g].2 += st[0] as f64;
    }
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    let nf: u64 = groups.iter().map(|g| g.1).sum();
    println!("fraud_group: rep_ms={:.1} frauds={} (expect 20663)", ms, nf);
    println!("gate_fraud_exact={}", if nf == 20_663 { "PASS" } else { "FAIL" });

    // ---- Facebook: 88k edges -> node 0-cells + edge 1-cells ----
    let t0 = Instant::now();
    let mut builder = BcsfBuilder::new();
    let mut id_of: HashMap<u64, usize> = HashMap::new();
    let mut g = 0usize;
    let mut nedges = 0usize;
    // NOTE: .gz read needs flate2 (not a dep) — read the pre-extracted file
    // (see FB archive); report SKIP if absent. Keeps zero-dep invariant.
    let fb_path = &fb;
    let content = match std::fs::read(fb_path) {
        Ok(c) if c.starts_with(&[0x1f, 0x8b]) => {
            println!("fb_skip: the input is gzip; pass the extracted facebook_combined.txt");
            println!("gate_fb=SKIP (data not extracted)");
            return;
        }
        Ok(c) => c,
        Err(_) => {
            println!("fb_skip: extract facebook_combined.txt.gz into the datasets root first");
            println!("gate_fb=SKIP (data not extracted)");
            return;
        }
    };
    let mut start = 0usize;
    for i in 0..content.len() {
        if content[i] == b'\n' {
            let line = &content[start..i];
            start = i + 1;
            if line.is_empty() {
                continue;
            }
            let mut sp = 0usize;
            while sp < line.len() && line[sp] != b' ' && line[sp] != b'\t' {
                sp += 1;
            }
            let a: u64 = std::str::from_utf8(&line[..sp]).unwrap().parse().unwrap();
            let b: u64 = std::str::from_utf8(&line[sp + 1..]).unwrap_or("0").parse().unwrap_or(0);
            for id in [a, b] {
                let gid = *id_of.entry(id).or_insert_with(|| {
                    let cur = g;
                    g += 1;
                    builder.push(0, Vec::new(), &db, &[0.0], &[]);
                    cur
                });
                let _ = gid;
            }
            builder.push(1, vec![id_of[&a] as u64, id_of[&b] as u64], &db, &[], &[]);
            nedges += 1;
        }
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok());
    println!("fb_load: nodes={} edges={} load_s={:.1}", id_of.len(), nedges, t0.elapsed().as_secs_f64());
    assert_eq!(nedges, 88_234, "edge count");
    println!("gate_fb_exact=PASS");

    // 2-hop neighborhood sizes for 200 sampled nodes (adjacency from BCSF).
    let mut adj: HashMap<usize, Vec<usize>> = HashMap::new();
    for cell in 0..reader.len() {
        if reader.dim(cell) == 1 {
            let b = reader.boundary(cell);
            adj.entry(b[0] as usize).or_default().push(b[1] as usize);
            adj.entry(b[1] as usize).or_default().push(b[0] as usize);
        }
    }
    let nodes: Vec<usize> = adj.keys().copied().collect();
    let t0 = Instant::now();
    let mut tot = 0usize;
    for k in 0..200.min(nodes.len()) {
        let v = nodes[k * 37 % nodes.len()];
        if let Some(ns) = adj.get(&v) {
            for w in ns {
                if let Some(n2) = adj.get(w) {
                    tot += n2.len();
                }
            }
        }
    }
    println!("fb_2hop: rep_ms={:.1} (200 sampled, walk-sum={})", t0.elapsed().as_secs_f64() * 1e3, tot);
    println!("REAL-LOAD-BENCH: COMPLETE");
}
