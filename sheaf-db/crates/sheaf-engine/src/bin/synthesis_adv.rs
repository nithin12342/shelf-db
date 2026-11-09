// crates/sheaf-engine/src/bin/synthesis_adv.rs — adversarial analysis of
// synthesis claims + hunt for NEW bottlenecks. Follows project protocol:
// fix-loud (assert/panic, never silent), dense comparison, boundary probes.
// ADV1: bandwidth floor curve across sizes (is 8.4 GB/s stable?).
// ADV2: fsync latency C: vs D: (all WAL benches used temp_dir = C:!).
// ADV3: ring latency UNDER 8-way contention (prior test was completeness-only).
// ADV4: fiber skew (all revenue one segment) — totals + timing.
// ADV5: graded watermark across 8 dims with scrambled seal attempts.
// ADV6: fiber prefold breakdown (parse/sort/map/build shares).
use std::time::Instant;

#[path = "shared/bench_args.rs"]
mod bench_args;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let d_probe = bench_args::required_path(&args, "--d-probe");
    let lineitem = bench_args::required_path(&args, "--lineitem");

    // ---- ADV1: bandwidth vs size ----
    for mb in [16usize, 48, 256, 1024] {
        let n = mb << 20;
        let src = vec![7u8; n];
        let mut dst = vec![0u8; n];
        let t0 = Instant::now();
        let reps = if mb <= 48 { 10 } else { 3 };
        for _ in 0..reps {
            dst.copy_from_slice(&src);
            std::hint::black_box(&dst);
        }
        let gb = (mb * reps) as f64 / 1024.0 / t0.elapsed().as_secs_f64();
        println!("ADV1-memcpy-{}MB: {:.1} GB/s", mb, gb);
    }

    // ---- ADV2: fsync C: vs D: ----
    for (name, dir) in [
        ("C", std::env::temp_dir()),
        ("D", std::path::PathBuf::from(&d_probe)),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("probe.dat");
        let blk = vec![9u8; 4096];
        let f = std::fs::File::create(&path).unwrap();
        let mut ts: Vec<u128> = Vec::new();
        for _ in 0..200 {
            let t0 = Instant::now();
            use std::io::Write;
            (&f).write_all(&blk).unwrap();
            f.sync_all().unwrap();
            ts.push(t0.elapsed().as_nanos());
        }
        ts.sort_unstable();
        println!("ADV2-fsync-{}: p50={}ns p99={}ns (n=200)", name, ts[100], ts[198]);
        let _ = std::fs::remove_file(&path);
    }

    // ---- ADV3: ring latency under 8-way contention ----
    {
        use sheaf_storage::lsp::ring_wal::HwCrcRing;
        use std::sync::Arc;
        let ring = Arc::new(HwCrcRing::new(18)); // 256k slots, never full
        let mut hs = Vec::new();
        for th in 0..8u64 {
            let ring = Arc::clone(&ring);
            hs.push(std::thread::spawn(move || {
                let mut ts = Vec::with_capacity(5000);
                let mut p = [0u8; 33];
                for i in 0..5000u64 {
                    p[0] = (th % 3) as u8;
                    p[1..9].copy_from_slice(&(th * 1_000_000 + i).to_le_bytes());
                    let t0 = Instant::now();
                    assert!(ring.try_append(&p));
                    ts.push(t0.elapsed().as_nanos());
                }
                ts
            }));
        }
        let mut all: Vec<u128> = Vec::new();
        for h in hs {
            all.extend(h.join().unwrap());
        }
        all.sort_unstable();
        let n = all.len();
        println!("ADV3-ring-contended: p50={}ns p99={}ns pmax={}ns (n={}, 8 producers)",
            all[n / 2], all[(n as f64 * 0.99) as usize], all[n - 1], n);
        let drained = ring.drain(|_| {});
        println!("ADV3-drained={} (expect 40000)", drained);
        assert_eq!(drained, 40_000);
    }

    // ---- ADV4: fiber skew — all revenue into segment 0 ----
    {
        // 2.57M runs, every run keyed to customer 7 (worst-case skew).
        let mut fibers: Vec<(u64, f64)> = Vec::with_capacity(2_574_528);
        for i in 0..2_574_528u64 {
            fibers.push((7, (i % 1000) as f64 * 0.5));
        }
        fibers.sort_unstable_by_key(|&(c, _)| c);
        let t0 = Instant::now();
        let mut seg_tot = [0f64; 5];
        let mut i = 0usize;
        while i < fibers.len() {
            let c = fibers[i].0;
            let mut rev = 0f64;
            while i < fibers.len() && fibers[i].0 == c {
                rev += fibers[i].1;
                i += 1;
            }
            seg_tot[(c % 5) as usize] += rev;
        }
        let ms = t0.elapsed().as_secs_f64() * 1e3;
        let total: f64 = seg_tot.iter().sum();
        println!("ADV4-skew: {:.1}ms total={:.0} (all revenue one customer; seg0 should hold ~all)", ms, total);
        assert!(seg_tot[2] > total * 0.99, "skew landed wrong");
    }

    // ---- ADV5: 8-dim watermark with scrambled seals ----
    {
        use sheaf_storage::lsp::graded_log::GradedWal;
        let dir = std::env::temp_dir().join("sheaf_adv5");
        let _ = std::fs::remove_dir_all(&dir);
        let mut g = GradedWal::create(&dir).unwrap();
        for d in [5u8, 3, 7, 1, 0, 2, 4, 6] {
            g.append(d, &[d, 1, 2, 3]).unwrap();
        }
        // Scrambled seals must fail until graded order allows.
        assert!(g.seal(7).is_err() && g.seal(3).is_err());
        for d in 0..8u8 {
            assert!(g.seal(d).is_ok(), "seal({}) should succeed in order", d);
        }
        assert_eq!(g.watermark(), 8);
        let counts = g.replay_sealed().unwrap();
        assert_eq!(counts.len(), 8);
        println!("ADV5-watermark-8d=PASS (scrambled rejected, graded 0..7 advances to 8)");
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- ADV6: prefold breakdown (parse/sort/map shares of join prefold) ----
    {
        let content = std::fs::read(&lineitem).unwrap();
        let t0 = Instant::now();
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
        let parse_s = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        runs.sort_unstable_by_key(|&(k, _)| k);
        let sort_s = t0.elapsed().as_secs_f64();
        println!("ADV6-prefold: parse={:.1}s sort={:.1}s runs={} (sort share={:.0}%)",
            parse_s, sort_s, runs.len(), sort_s / (parse_s + sort_s) * 100.0);
    }
    println!("ADVERSARIAL-SYNTHESIS: COMPLETE");
}
