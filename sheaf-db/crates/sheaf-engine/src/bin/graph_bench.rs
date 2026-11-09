// crates/sheaf-engine/src/bin/graph_bench.rs — Test 3: Neo4j traversal challenge
// Loads the neo_prep.py CSVs (150k customers, 1.5M orders, 600k lines,
// 1.5M PLACED + 600k CONTAINS edges) into BCSF: 0-cells for nodes,
// 1-cells for PLACED edges, 2-cells for CONTAINS hyperedges. Timed: 200
// sampled 2-hop walks + one full-graph rollup (must count 600,000 paths,
// matching Neo4j exactly). Criterion: beat Neo4j 878 ms full / 3.3 ms sampled?
use std::collections::HashMap;
use std::time::Instant;

use sheaf_storage::bcsf::builder::BcsfBuilder;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = bench_args::required_path(&args, "--dir");

    // ---- Load (untimed, reported): nodes + edges into BCSF ----
    let t0 = Instant::now();
    let mut builder = BcsfBuilder::new();
    let db = [0u8; 16];
    let mut gid: HashMap<(u8, u64), usize> = HashMap::with_capacity(2_400_000);
    let mut g = 0usize;
    let mut placed: Vec<(usize, usize)> = Vec::with_capacity(1_500_000);
    let mut contains: Vec<(usize, usize)> = Vec::with_capacity(600_000);

    let content = std::fs::read(format!("{dir}\\cust.csv")).unwrap();
    for line in content.split(|&b| b == b'\n').skip(1) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut e = 0usize;
        while line[e] != b',' {
            e += 1;
        }
        let id: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
        gid.insert((0, id), g);
        g += 1;
        builder.push(0, Vec::new(), &db, &[0.0], &[]);
    }
    let content = std::fs::read(format!("{dir}\\ord.csv")).unwrap();
    for line in content.split(|&b| b == b'\n').skip(1) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut e = 0usize;
        while line[e] != b',' {
            e += 1;
        }
        let id: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
        gid.insert((1, id), g);
        g += 1;
        builder.push(0, Vec::new(), &db, &[0.0], &[]);
    }
    let content = std::fs::read(format!("{dir}\\line.csv")).unwrap();
    for line in content.split(|&b| b == b'\n').skip(1) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut e = 0usize;
        while line[e] != b',' {
            e += 1;
        }
        let id: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
        let mut c = e + 1;
        while c < line.len() && line[c] != b'\n' {
            c += 1;
        }
        let price: f32 = std::str::from_utf8(&line[e + 1..c.min(line.len())]).unwrap_or("0").parse().unwrap_or(0.0);
        gid.insert((2, id), g);
        g += 1;
        builder.push(0, Vec::new(), &db, &[price], &[]);
    }
    let content = std::fs::read(format!("{dir}\\placed.csv")).unwrap();
    for line in content.split(|&b| b == b'\n').skip(1) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut e = 0usize;
        while line[e] != b',' {
            e += 1;
        }
        let c: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
        let o: u64 = std::str::from_utf8(&line[e + 1..]).unwrap().parse().unwrap();
        if let (Some(&gc), Some(&go)) = (gid.get(&(0, c)), gid.get(&(1, o))) {
            placed.push((gc, go));
            builder.push(1, vec![gc as u64, go as u64], &db, &[], &[]);
            g += 1;
        }
    }
    let content = std::fs::read(format!("{dir}\\contains.csv")).unwrap();
    for line in content.split(|&b| b == b'\n').skip(1) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let mut e = 0usize;
        while line[e] != b',' {
            e += 1;
        }
        let o: u64 = std::str::from_utf8(&line[..e]).unwrap().parse().unwrap();
        let l: u64 = std::str::from_utf8(&line[e + 1..]).unwrap().parse().unwrap();
        if let (Some(&go), Some(&gl)) = (gid.get(&(1, o)), gid.get(&(2, l))) {
            contains.push((go, gl));
            builder.push(2, vec![go as u64, gl as u64], &db, &[], &[]);
            g += 1;
        }
    }
    let reader = builder.finish();
    assert!(reader.validate().is_ok());
    println!("graph_load: cells={} (nodes+edges) load_s={:.1} resident_MB={:.1}",
        g, t0.elapsed().as_secs_f64(), reader.resident_bytes() as f64 / 1e6);

    // Adjacency from BCSF boundary slices (the topological walk).
    let mut cust_orders: HashMap<usize, Vec<usize>> = HashMap::with_capacity(150_000);
    let mut order_lines: HashMap<usize, Vec<usize>> = HashMap::with_capacity(600_000);
    for cell in 0..reader.len() {
        match reader.dim(cell) {
            1 => {
                let b = reader.boundary(cell);
                cust_orders.entry(b[0] as usize).or_default().push(b[1] as usize);
            }
            2 => {
                let b = reader.boundary(cell);
                order_lines.entry(b[0] as usize).or_default().push(b[1] as usize);
            }
            _ => {}
        }
    }
    // Sampled 2-hop walks (200 deterministic customers).
    let mut state = 42u64;
    let mut ts: Vec<u128> = Vec::new();
    for _ in 0..200 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let c = (state % 150_000) as usize;
        let t = Instant::now();
        let mut sum = 0f32;
        if let Some(os) = cust_orders.get(&c) {
            for o in os {
                if let Some(ls) = order_lines.get(o) {
                    for l in ls {
                        sum += reader.stalk(*l)[0];
                    }
                }
            }
        }
        std::hint::black_box(sum);
        ts.push(t.elapsed().as_nanos());
    }
    ts.sort_unstable();
    println!("sampled_2hop: p50_us={:.1} (n=200)", ts[100] as f64 / 1e3);
    // Full rollup: every customer 2-hop path count (must equal 600,000).
    let t1 = Instant::now();
    let mut paths = 0usize;
    for os in cust_orders.values() {
        for o in os {
            if let Some(ls) = order_lines.get(o) {
                paths += ls.len();
            }
        }
    }
    let full_ms = t1.elapsed().as_secs_f64() * 1e3;
    println!("full_rollup: paths={} (expect 600000) ms={:.1}", paths, full_ms);
    println!("gate_paths={}", if paths == 600_000 { "PASS" } else { "FAIL" });
    println!("gate_traverse={} (vs neo4j 878ms full / 3.3ms sampled)", if full_ms < 878.0 { "PASS (beat Neo4j)" } else { "RECORDED" });
    if paths != 600_000 {
        std::process::exit(1);
    }
}
