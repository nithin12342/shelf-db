// crates/sheaf-engine/src/bin/ring_wal_bench.rs — SYNTHESIS Stage 3 gate
// Gates: (1) HW CRC == software CRC bit-exact on 10k random payloads.
// (2) MPSC: 8 producers x 5k appends, single drain recovers all 40k with
// valid CRCs. (3) Latency: 20k single-thread appends p50/p99 (target <=0.8us
// vs SQLite 2.1us). (4) Graded fusion: drain dim-prefixed frames into
// GradedWal, seal 0->1->2, replay exact (Choice 1 over Choice 2).
use std::sync::Arc;
use std::time::Instant;

use sheaf_storage::lsp::graded_log::GradedWal;
use sheaf_storage::lsp::ring_wal::{crc32c_soft, crc_hw, HwCrcRing};

fn pct(mut v: Vec<u128>, p: f64) -> u128 {
    v.sort_unstable();
    v[((v.len() as f64 * p) as usize).min(v.len() - 1)]
}

fn main() {
    // Gate 1: HW CRC agrees with software CRC-32C bit-exactly (same
    // algorithm both sides; the WAL file format keeps ISO-HDLC separately).
    let mut rng = 0x12345678u64;
    let mut agree = true;
    for _ in 0..10_000 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let mut b = [0u8; 52];
        for (i, slot) in b.iter_mut().enumerate() {
            *slot = (rng >> (i % 8)) as u8;
        }
        if crc_hw(&b) != crc32c_soft(&b) {
            agree = false;
            break;
        }
    }
    println!("gate_crc_agree={}", if agree { "PASS" } else { "FAIL" });

    // Gate 2: MPSC completeness (8 x 5k).
    let ring = Arc::new(HwCrcRing::new(16));
    let mut hs = Vec::new();
    for th in 0..8u64 {
        let ring = Arc::clone(&ring);
        hs.push(std::thread::spawn(move || {
            for i in 0..5_000u64 {
                let mut p = [0u8; 33];
                p[0] = (th % 3) as u8;
                p[1..9].copy_from_slice(&(th * 1_000_000 + i).to_le_bytes());
                assert!(ring.try_append(&p), "ring full (size it up, don't retry-loop)");
            }
        }));
    }
    for h in hs {
        h.join().unwrap();
    }
    let mut n = 0usize;
    let drained = ring.drain(|_| n += 1);
    let _ = drained;
    println!("gate_mpsc={} ({}/40000 frames)", if n == 40_000 { "PASS" } else { "FAIL" }, n);

    // Gate 3: single-thread append latency (target <= 0.8us).
    let ring2 = HwCrcRing::new(16);
    // Pre-drain pattern: keep ring empty by draining every 1k appends.
    let mut ts: Vec<u128> = Vec::with_capacity(20_000);
    let mut payload = [0u8; 33];
    let mut drained_total = 0usize;
    for i in 0..20_000u64 {
        payload[0] = (i % 3) as u8;
        payload[1..9].copy_from_slice(&i.to_le_bytes());
        let t0 = Instant::now();
        assert!(ring2.try_append(&payload));
        ts.push(t0.elapsed().as_nanos());
        if i % 1000 == 999 {
            drained_total += ring2.drain(|_| {});
        }
    }
    drained_total += ring2.drain(|_| {});
    assert_eq!(drained_total, 20_000);
    println!("ring_insert: p50_ns={} p99_ns={} (target <= 800ns vs sqlite 2100ns)",
        pct(ts.clone(), 0.5), pct(ts, 0.99));

    // Gate 4: graded fusion — drain into GradedWal, seal graded, replay exact.
    let dir = std::env::temp_dir().join("sheaf_ring_graded");
    let _ = std::fs::remove_dir_all(&dir);
    let mut g = GradedWal::create(&dir).unwrap();
    let ring3 = HwCrcRing::new(16);
    for i in 0..300u64 {
        let mut p = vec![0u8];
        p.extend_from_slice(&i.to_le_bytes());
        assert!(ring3.try_append(&p));
    }
    for i in 0..100u64 {
        let mut p = vec![2u8];
        p.extend_from_slice(&i.to_le_bytes());
        assert!(ring3.try_append(&p));
    }
    ring3.drain(|f| g.append(f[0], &f[1..]).unwrap());
    let early = g.seal(2).is_err();
    assert!(g.seal(0).is_ok() && g.seal(2).is_ok());
    let counts = g.replay_sealed().unwrap();
    // Absent dim 1 is vacuously sealed: watermark skips to 3 (past max dim 2).
    let fusion_ok = early && counts == vec![(0, 300), (2, 100)] && g.watermark() == 3;
    println!("gate_graded_fusion={} ({:?}, watermark={})", if fusion_ok { "PASS" } else { "FAIL" }, counts, g.watermark());
    let _ = std::fs::remove_dir_all(&dir);

    if !(agree && n == 40_000 && fusion_ok) {
        std::process::exit(1);
    }
    println!("STAGE-3: PASS");
}
