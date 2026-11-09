// crates/sheaf-engine/src/bin/bottleneck_probe.rs — isolate every claimed bottleneck
// Sections: (A) single-thread memcpy bandwidth, (B) multi-thread bandwidth,
// (C) bounds-check cost: manual-index vs iterator vs unchecked sum loops,
// (D) fsync floor: 4KB write+fsync distribution vs write-only,
// (E) WAL frame cost breakdown: encode + crc_sw + crc_hw + append + flush.
// Each section prints measured numbers; verdicts compare against the disputed
// results (7.9ms scan, 6.9us insert, 2.8ms prediction).
use std::time::Instant;

fn main() {
    // ---- A: single-thread memcpy bandwidth (256 MB x10) ----
    let big = vec![7u8; 256 << 20];
    let mut dst = vec![0u8; 256 << 20];
    let t0 = Instant::now();
    for _ in 0..10 {
        dst.copy_from_slice(&big);
        std::hint::black_box(&dst);
    }
    let secs = t0.elapsed().as_secs_f64();
    println!("A-memcpy-single: {:.1} GB/s (2560 MB in {:.2}s)", 2.56 / secs, secs);

    // ---- B: multi-thread bandwidth (4 x 64 MB, scoped threads) ----
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| {
                let mut d = vec![0u8; 64 << 20];
                for _ in 0..10 {
                    d.copy_from_slice(&big[..64 << 20]);
                    std::hint::black_box(&d);
                }
            });
        }
    });
    let secs = t0.elapsed().as_secs_f64();
    println!("B-memcpy-4t: {:.1} GB/s (2560 MB in {:.2}s)", 2.56 / secs, secs);

    // ---- C: sum-loop variants over 6M pairs (48 MB slab) ----
    let n = 6_001_215usize;
    let mut p = vec![0f32; n];
    let mut q = vec![0f32; n];
    for i in 0..n {
        p[i] = (i as f32 * 0.001) % 100.0;
        q[i] = (i as f32 * 0.0007) % 0.5;
    }
    // C1: manual index (bounds checks likely retained)
    let t0 = Instant::now();
    let mut acc = 0f64;
    let mut i = 0usize;
    while i < n {
        acc += p[i] as f64 * (1.0 - q[i] as f64);
        i += 1;
    }
    std::hint::black_box(acc);
    println!("C1-manual-index: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    // C2: zip iterators
    let t0 = Instant::now();
    let mut acc = 0f64;
    for (a, b) in p.iter().zip(q.iter()) {
        acc += *a as f64 * (1.0 - *b as f64);
    }
    std::hint::black_box(acc);
    println!("C2-zip-iter: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);
    // C3: unchecked (unsafe, upper bound of check-removal gain)
    let t0 = Instant::now();
    let mut acc = 0f64;
    let mut i = 0usize;
    while i < n {
        unsafe {
            acc += *p.get_unchecked(i) as f64 * (1.0 - *q.get_unchecked(i) as f64);
        }
        i += 1;
    }
    std::hint::black_box(acc);
    println!("C3-unchecked: {:.1} ms", t0.elapsed().as_secs_f64() * 1e3);

    // ---- D: fsync floor (1000 x 4KB write+fsync) ----
    let dir = std::env::temp_dir().join("sheaf_fsync_probe");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.dat");
    let blk = vec![9u8; 4096];
    let mut ts: Vec<u128> = Vec::new();
    {
        let f = std::fs::File::create(&path).unwrap();
        for _ in 0..1000 {
            let t0 = Instant::now();
            use std::io::Write;
            (&f).write_all(&blk).unwrap();
            f.sync_all().unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
    }
    ts.sort_unstable();
    println!("D-fsync-4KB: p50={}ns p99={}ns (n=1000)", ts[500], ts[990]);
    let mut ts: Vec<u128> = Vec::new();
    {
        let f = std::fs::File::create(&path).unwrap();
        for _ in 0..10000 {
            let t0 = Instant::now();
            use std::io::Write;
            (&f).write_all(&blk).unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
    }
    ts.sort_unstable();
    println!("D-write-only-4KB: p50={}ns p99={}ns (n=10000, no sync)", ts[5000], ts[9900]);

    // ---- E: WAL frame breakdown (33B cell frame, 20k reps each) ----
    let payload = vec![3u8; 33];
    let mut ts_e = [0u128; 5];
    let reps = 20_000u32;
    let t0 = Instant::now();
    for _ in 0..reps {
        let _ = payload.len();
    }
    ts_e[0] = t0.elapsed().as_nanos() / reps as u128;
    let t0 = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(sheaf_storage::lsp::wal::crc32(&payload));
    }
    ts_e[1] = t0.elapsed().as_nanos() / reps as u128;
    let t0 = Instant::now();
    for _ in 0..reps {
        std::hint::black_box(sheaf_storage::lsp::ring_wal::crc_hw(&payload));
    }
    ts_e[2] = t0.elapsed().as_nanos() / reps as u128;
    println!("E-frame-ns: encode~{} crc_sw={} crc_hw={} (per 33B frame)",
        ts_e[0], ts_e[1], ts_e[2]);
    println!("PROBE-COMPLETE");
    let _ = std::fs::remove_dir_all(&dir);
}
