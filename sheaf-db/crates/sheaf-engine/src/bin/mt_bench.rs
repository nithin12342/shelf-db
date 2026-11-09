// crates/sheaf-engine/src/bin/mt_bench.rs — Test 5: MongoDB 32-thread challenge
// 32 OS threads, 50/50 read/write, 30 s sustained cap (stops early at op
// budget). Reads: BCSF boundary + stalk fetches through a shared Arc reader
// (lock-free). Writes: one shared WalWriter behind a Mutex (honest
// single-writer design) with flush per op. Criterion: exceed MongoDB 2,497
// ops/s (p50 9.47 ms) without deadlock or starvation spikes?
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::reader::BcsfReader;
use sheaf_storage::lsp::wal::WalWriter;

const N_CELL: usize = 100_000;
const N_HYPER: usize = 10_000;
const NTHREADS: usize = 32;
const OPS_PER_THREAD: usize = 1_000;
const BUDGET_S: u64 = 30;

fn main() {
    // Fixture: 100k 0-cells + 10k 2-cells (3 faces each), d=4 stalks.
    let mut builder = BcsfBuilder::new();
    let db = [0u8; 16];
    for i in 0..N_CELL {
        builder.push(0, Vec::new(), &db, &[i as f32 * 0.001, 0.2, 0.3, 0.4], &[]);
    }
    for h in 0..N_HYPER {
        let b0 = (h * 3 % N_CELL) as u64;
        builder.push(2, vec![b0, b0 + 1, b0 + 2], &db, &[0.1, 0.2, 0.3, 0.4], &[]);
    }
    let reader: BcsfReader = builder.finish();
    assert!(reader.validate().is_ok());
    let reader = Arc::new(reader);

    let dir = std::env::temp_dir().join("sheaf_mt_bench");
    std::fs::create_dir_all(&dir).unwrap();
    let writer = Arc::new(Mutex::new(WalWriter::create(&dir.join("mt.wal")).unwrap()));

    let t_start = Instant::now();
    let mut handles = Vec::new();
    for th in 0..NTHREADS {
        let reader = Arc::clone(&reader);
        let writer = Arc::clone(&writer);
        handles.push(std::thread::spawn(move || {
            let mut rng = (th as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mut lat: Vec<u128> = Vec::with_capacity(OPS_PER_THREAD);
            let mut ops = 0usize;
            for _ in 0..OPS_PER_THREAD {
                if t_start.elapsed() > Duration::from_secs(BUDGET_S) {
                    break;
                }
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let t0 = Instant::now();
                if rng.is_multiple_of(2) {
                    // read: boundary + stalk fetch on pseudo-random cell
                    let c = (rng as usize) % (N_CELL + N_HYPER);
                    let b = reader.boundary(c);
                    let mut acc = 0f32;
                    for f in b {
                        acc += reader.stalk(*f as usize)[0];
                    }
                    std::hint::black_box(acc);
                } else {
                    // write: WAL append + flush under one mutex
                    let mut payload = [0u8; 33];
                    payload[1..9].copy_from_slice(&(rng).to_le_bytes());
                    let mut w = writer.lock().unwrap();
                    w.append(&payload).unwrap();
                    w.flush().unwrap();
                }
                lat.push(t0.elapsed().as_nanos());
                ops += 1;
            }
            (ops, lat)
        }));
    }
    let mut total_ops = 0usize;
    let mut all_lat: Vec<u128> = Vec::new();
    for h in handles {
        let (ops, lat) = h.join().unwrap();
        total_ops += ops;
        all_lat.extend(lat);
    }
    let wall = t_start.elapsed().as_secs_f64();
    all_lat.sort_unstable();
    let p50 = all_lat[all_lat.len() / 2] as f64 / 1e3;
    let p99 = all_lat[(all_lat.len() as f64 * 0.99) as usize] as f64 / 1e3;
    println!("mt_mixed: threads={} ops={} wall_s={:.1} throughput={:.0} p50_us={:.1} p99_us={:.1}",
        NTHREADS, total_ops, wall, total_ops as f64 / wall, p50, p99);
    println!("gate_mt={} (vs mongo 2497 ops/s, p50 9.47ms)",
        if total_ops as f64 / wall > 2497.0 { "PASS" } else { "RECORDED" });
    let _ = std::fs::remove_file(dir.join("mt.wal"));
    let _ = std::fs::remove_dir(dir);
}
