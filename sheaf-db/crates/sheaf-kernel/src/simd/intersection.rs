// crates/sheaf-kernel/src/simd/intersection.rs — intersect u64 slices via SIMD
// FILE-022. Pure slice arithmetic. Must never know TQL syntax or storage types.
use super::intersection_avx::intersect_count_avx2;

pub fn intersect_count(a: &[u64], b: &[u64]) -> usize {
    intersect_count_avx2(a, b)
}
