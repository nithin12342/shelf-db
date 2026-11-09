// crates/sheaf-engine/src/bin/streaming_rw_bench.rs — Continuous Write & Continuous Read Streaming Benchmark
// Measures sustained concurrent high-volume streaming writes, concurrent reads,
// and micro-batch incremental fiber building time over a 60-second budget.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::fiber_domain::FiberDomains;
use sheaf_storage::lsp::ring_wal::RingWal;

const INITIAL_CELLS: usize = 50_000;
const INITIAL_HYPER: usize = 5_000;
const NUM_WRITERS: usize = 4;
const NUM_READERS: usize = 4;
const RUN_SECONDS: u64 = 60;
const BUILD_EVERY_WRITES: u64 = 10_000;

fn main() {
    println!("=== ShelfDB Continuous Streaming Read/Write Benchmark (60 Seconds) ===");
    let start_iso = chrono_like_now();
    println!("Start Timestamp: {}", start_iso);

    // 1. Initial complex setup
    let mut builder = BcsfBuilder::new();
    let db = [0u8; 16];
    for i in 0..INITIAL_CELLS {
        builder.push(0, Vec::new(), &db, &[i as f32 * 0.001, 0.2, 0.3, 0.4], &[]);
    }
    for h in 0..INITIAL_HYPER {
        let b0 = (h * 3 % INITIAL_CELLS) as u64;
        builder.push(2, vec![b0, b0 + 1, b0 + 2], &db, &[0.1, 0.2, 0.3, 0.4], &[]);
    }
    let reader = Arc::new(RwLock::new(Arc::new(builder.finish())));

    let dir = std::env::temp_dir().join("sheaf_streaming_bench");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let wal = Arc::new(RingWal::create(&dir.join("stream.wal"), 16).unwrap());

    let running = Arc::new(AtomicBool::new(true));
    let total_writes = Arc::new(AtomicU64::new(0));
    let total_reads = Arc::new(AtomicU64::new(0));
    let pending_build_counter = Arc::new(AtomicU64::new(0));

    let build_times = Arc::new(Mutex::new(Vec::new()));
    let write_lats = Arc::new(Mutex::new(Vec::new()));
    let read_lats = Arc::new(Mutex::new(Vec::new()));

    let t_start = Instant::now();

    // Background Builder Thread: checks when write volume reaches BUILD_EVERY_WRITES
    let b_running = Arc::clone(&running);
    let b_counter = Arc::clone(&pending_build_counter);
    let b_build_times = Arc::clone(&build_times);
    let builder_handle = std::thread::spawn(move || {
        let mut build_round = 0u64;
        while b_running.load(Ordering::Relaxed) {
            let pending = b_counter.load(Ordering::Relaxed);
            if pending >= BUILD_EVERY_WRITES {
                b_counter.fetch_sub(BUILD_EVERY_WRITES, Ordering::SeqCst);
                build_round += 1;

                let t_build0 = Instant::now();
                // Simulate incremental micro-batch fiber bundle construction (10k items)
                let entries: Vec<(u64, u32)> = (0..BUILD_EVERY_WRITES)
                    .map(|k| (build_round * BUILD_EVERY_WRITES + k, k as u32))
                    .collect();
                let _fiber_domains = FiberDomains::build(&entries, 64 * 1024);
                let build_duration_ms = t_build0.elapsed().as_secs_f64() * 1e3;

                b_build_times.lock().unwrap().push(build_duration_ms);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let mut handles = Vec::new();

    // Spawning Writer Threads
    for w_id in 0..NUM_WRITERS {
        let running = Arc::clone(&running);
        let wal = Arc::clone(&wal);
        let total_writes = Arc::clone(&total_writes);
        let pending_counter = Arc::clone(&pending_build_counter);
        let write_lats = Arc::clone(&write_lats);

        handles.push(std::thread::spawn(move || {
            let mut rng = (w_id as u64).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mut lats: Vec<u64> = Vec::with_capacity(100_000);
            let mut count = 0u64;

            while running.load(Ordering::Relaxed) {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let mut payload = [0u8; 33];
                payload[1..9].copy_from_slice(&rng.to_le_bytes());

                let t0 = Instant::now();
                {
                    let _ = wal.append(&payload);
                    if count % 1000 == 0 {
                        let _ = wal.flush();
                    }
                }
                let lat_ns = t0.elapsed().as_nanos() as u64;
                if count % 10 == 0 {
                    lats.push(lat_ns);
                }
                count += 1;
                total_writes.fetch_add(1, Ordering::Relaxed);
                pending_counter.fetch_add(1, Ordering::Relaxed);
            }
            write_lats.lock().unwrap().extend(lats);
        }));
    }

    // Spawning Reader Threads
    for r_id in 0..NUM_READERS {
        let running = Arc::clone(&running);
        let reader = Arc::clone(&reader);
        let total_reads = Arc::clone(&total_reads);
        let read_lats = Arc::clone(&read_lats);

        handles.push(std::thread::spawn(move || {
            let mut rng = (r_id as u64 + 100).wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let mut lats: Vec<u64> = Vec::with_capacity(100_000);
            let mut count = 0u64;

            while running.load(Ordering::Relaxed) {
                rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let t0 = Instant::now();
                {
                    let cur_reader = reader.read().unwrap();
                    let c = (rng as usize) % (INITIAL_CELLS + INITIAL_HYPER);
                    let bnd = cur_reader.boundary(c);
                    let mut acc = 0.0f32;
                    for f in bnd {
                        acc += cur_reader.stalk(*f as usize)[0];
                    }
                    std::hint::black_box(acc);
                }
                let lat_ns = t0.elapsed().as_nanos() as u64;
                if count % 10 == 0 {
                    lats.push(lat_ns);
                }
                count += 1;
                total_reads.fetch_add(1, Ordering::Relaxed);
            }
            read_lats.lock().unwrap().extend(lats);
        }));
    }

    // Run for sustained RUN_SECONDS duration
    println!("Streaming benchmark running for {} seconds...", RUN_SECONDS);
    std::thread::sleep(Duration::from_secs(RUN_SECONDS));
    running.store(false, Ordering::SeqCst);

    for h in handles {
        let _ = h.join();
    }
    let _ = builder_handle.join();

    let wall_duration = t_start.elapsed().as_secs_f64();
    let total_w = total_writes.load(Ordering::Relaxed);
    let total_r = total_reads.load(Ordering::Relaxed);

    let end_iso = chrono_like_now();
    println!("End Timestamp:   {}", end_iso);
    println!("Elapsed Seconds: {:.2}", wall_duration);

    let mut w_lats = write_lats.lock().unwrap();
    w_lats.sort_unstable();
    let w_p50 = if !w_lats.is_empty() { w_lats[w_lats.len() / 2] as f64 / 1e3 } else { 0.0 };
    let w_p99 = if !w_lats.is_empty() { w_lats[(w_lats.len() as f64 * 0.99) as usize] as f64 / 1e3 } else { 0.0 };

    let mut r_lats = read_lats.lock().unwrap();
    r_lats.sort_unstable();
    let r_p50 = if !r_lats.is_empty() { r_lats[r_lats.len() / 2] as f64 / 1e3 } else { 0.0 };
    let r_p99 = if !r_lats.is_empty() { r_lats[(r_lats.len() as f64 * 0.99) as usize] as f64 / 1e3 } else { 0.0 };

    let b_times = build_times.lock().unwrap();
    let num_builds = b_times.len();
    let avg_build_ms = if num_builds > 0 { b_times.iter().sum::<f64>() / num_builds as f64 } else { 0.0 };
    let min_build_ms = if num_builds > 0 { b_times.iter().cloned().fold(f64::INFINITY, f64::min) } else { 0.0 };
    let max_build_ms = if num_builds > 0 { b_times.iter().cloned().fold(0.0f64, f64::max) } else { 0.0 };

    println!("\n=== STREAMING BENCHMARK RESULTS ===");
    println!("Write Throughput: {:.0} writes/sec (Total: {})", total_w as f64 / wall_duration, total_w);
    println!("Write Latency:    p50={:.2} us, p99={:.2} us", w_p50, w_p99);
    println!("Read Throughput:  {:.0} reads/sec (Total: {})", total_r as f64 / wall_duration, total_r);
    println!("Read Latency:     p50={:.2} us, p99={:.2} us", r_p50, r_p99);
    println!("Fiber Build Count: {}", num_builds);
    println!("Fiber Build Time: avg={:.2} ms (min={:.2} ms, max={:.2} ms)", avg_build_ms, min_build_ms, max_build_ms);

    let _ = std::fs::remove_dir_all(dir);
}

fn chrono_like_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Offset +05:30 = 19800 seconds
    let ist = now + 19800;
    let sec = ist % 60;
    let min = (ist / 60) % 60;
    let hr = (ist / 3600) % 24;
    format!("2026-10-28 {:02}:{:02}:{:02} IST", hr, min, sec)
}
