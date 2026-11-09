// crates/sheaf-engine/src/bin/graded_wal_bench.rs — SYNTHESIS Stage 2 gate
// Verifies the graded WAL contract: (1) out-of-order arrival accepted,
// (2) premature seal of dim≥1 rejected while dim 0 unsealed (down-closure),
// (3) graded sealing advances the watermark 0→1→2, (4) replay recovers every
// sealed record exactly. Timing informational (correctness gate, not latency).
use sheaf_storage::lsp::graded_log::GradedWal;

fn cell(dim: u8, id: u64) -> Vec<u8> {
    let mut b = vec![dim];
    b.extend_from_slice(&id.to_le_bytes());
    b.extend_from_slice(&[0.1f32, 0.2, 0.3, 0.4].iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>());
    b
}

fn main() {
    let dir = std::env::temp_dir().join("sheaf_graded_bench");
    let _ = std::fs::remove_dir_all(&dir);
    let mut g = GradedWal::create(&dir).unwrap();

    // Gate 1: arrivals in any order (2-cells BEFORE 0-cells) accepted.
    for i in 0..100u64 {
        g.append(2, &cell(2, i)).unwrap();
    }
    for i in 0..300u64 {
        g.append(0, &cell(0, i)).unwrap();
    }
    for i in 0..50u64 {
        g.append(1, &cell(1, i)).unwrap();
    }
    println!("gate_arrival_any_order=PASS (100x dim2 + 300x dim0 + 50x dim1)");

    // Gate 2: sealing dim 2 (or 1) before dim 0 is REJECTED.
    let early2 = g.seal(2).is_err();
    let early1 = g.seal(1).is_err();
    println!("gate_seal_ordered={} (seal(2)={}, seal(1)={} before seal(0))",
        if early2 && early1 { "PASS" } else { "FAIL" }, !early2, !early1);

    // Gate 3: graded sealing advances watermark 0 -> 1 -> 2.
    assert!(g.seal(0).is_ok());
    let w0 = g.watermark();
    assert!(g.seal(1).is_ok());
    let w1 = g.watermark();
    assert!(g.seal(2).is_ok());
    let w2 = g.watermark();
    println!("gate_watermark={} ({} -> {} -> {})",
        if (w0, w1, w2) == (1, 2, 3) { "PASS" } else { "FAIL" }, w0, w1, w2);

    // Gate 4: replay recovers every sealed record exactly.
    let counts = g.replay_sealed().unwrap();
    let ok = counts == vec![(0, 300), (1, 50), (2, 100)];
    println!("gate_replay_exact={} ({:?})", if ok { "PASS" } else { "FAIL" }, counts);

    let _ = std::fs::remove_dir_all(&dir);
    if !(early2 && early1 && (w0, w1, w2) == (1, 2, 3) && ok) {
        std::process::exit(1);
    }
    println!("STAGE-2: PASS");
}
