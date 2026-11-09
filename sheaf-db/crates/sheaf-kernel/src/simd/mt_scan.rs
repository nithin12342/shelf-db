// crates/sheaf-kernel/src/simd/mt_scan.rs — partitioned multi-threaded scan
// MAX-PERF P1 (METHOD-024, FILE-060). Depends on nothing (raw slices).
// Splits rows into contiguous per-thread partitions; each thread accumulates
// its fused sum into a private f64; single final reduction. No atomics on
// the hot path (one join per thread). Expectation from W1 (bandwidth wall):
// ~4-6 ms on 48 MB (bandwidth-split, not 8x — controller-capped).
pub fn partitioned_sum(p: &[f32], q: &[f32], n_threads: usize) -> f64 {
    assert_eq!(p.len(), q.len(), "column length mismatch");
    assert!((1..=16).contains(&n_threads), "thread count 1..=16");
    let n = p.len();
    if n == 0 {
        return 0.0;
    }
    let chunk = n.div_ceil(n_threads);
    let mut partials = vec![0f64; n_threads];
    std::thread::scope(|s| {
        for (t, slot) in partials.iter_mut().enumerate() {
            let start = (t * chunk).min(n);
            let end = ((t + 1) * chunk).min(n);
            let (pp, qq) = (&p[start..end], &q[start..end]);
            s.spawn(move || {
                let mut acc = 0f64;
                for (a, b) in pp.iter().zip(qq.iter()) {
                    acc += *a as f64 * (1.0 - *b as f64);
                }
                *slot = acc;
            });
        }
    });
    partials.iter().sum()
}
