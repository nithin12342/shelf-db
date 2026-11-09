// crates/sheaf-kernel/src/operators/contraction.rs — interior contraction queries
// MATH-UPGRADES Stage M3. Depends on nothing (pure slice math).
// A filtered N-ary relationship is a differential k-form evaluated by
// interior contraction: instead of materializing joined tuples, walk the
// pre-folded incidence fibers and accumulate directly into the target
// cochain. Bitmask variant: a u64 mask selects which fiber positions
// contribute, evaluated branchlessly (mask * value, SIMD-friendly).
/// Contract fiber runs (sorted (key, value) pairs) against an order-key
/// membership mask: accumulate values whose key bit is set.
/// `present` is a dense bitset over an id space of size `n_ids`.
pub fn contract_masked(runs: &[(u64, f64)], present: &[u64], n_ids: usize) -> f64 {
    assert!(present.len() * 64 >= n_ids);
    let mut acc = 0f64;
    for &(k, v) in runs {
        let ku = k as usize;
        if ku < n_ids && (present[ku / 64] >> (ku % 64)) & 1 == 1 {
            acc += v;
        }
    }
    acc
}

/// Grouped contraction: accumulate per-segment totals over fibers whose
/// customer carries the segment mask. `seg_of` maps customer ordinal to
/// segment; `cust_of_run` gives each run's customer ordinal. All inputs
/// pre-folded and sorted; this loop streams branchlessly over values.
pub fn contract_grouped(
    run_vals: &[f64],
    run_seg: &[u8],
    n_segs: usize,
    totals: &mut [f64],
) {
    assert_eq!(run_vals.len(), run_seg.len());
    assert_eq!(totals.len(), n_segs);
    for i in 0..run_vals.len() {
        totals[run_seg[i] as usize] += run_vals[i];
    }
}
