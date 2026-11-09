// crates/sheaf-kernel/src/simd/fma_scan.rs — morsel-driven FMA scan kernel
// SYNTHESIS Stage 1b (Choice 1 micro-kernel over Choice 2 layout).
// Depends on nothing (raw slices in, scalar out).
// Processes Arrow-compatible slabs in 65,536-element L2 morsels with 4x
// unrolled accumulation + software prefetch. AVX-512 FMA path when the CPU
// reports it, SSE/unrolled-scalar fallback otherwise (Tiger Lake client
// parts fuse AVX-512 off — the fallback is the honest default there).
// Rounding differs from the scalar baseline (FMA fuses mul+add), so gates
// compare with rel tolerance 1e-6, never bit-exact.
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

pub const MORSEL_ELEMS: usize = 65_536;

/// Which kernel path the CPU takes (for gate diagnostics).
pub fn path_name() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx512f") {
            return "avx512-gather";
        }
    }
    "unrolled-prefetch"
}

/// Fused sum(price * (1 - discount)) over interleaved pairs.
/// `slab` holds rows of width `d`; price col `p`, discount col `q`.
/// Skips no validity bits (caller compacts valid rows first; Stage 1a slab
/// here is fully valid — see arrow_scan_bench).
pub fn fused_sum_pairs(slab: &[f32], d: usize, p: usize, q: usize) -> f64 {
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx512f") {
            unsafe { fused_sum_avx512(slab, d, p, q) }
        } else {
            fused_sum_unrolled(slab, d, p, q)
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        fused_sum_unrolled(slab, d, p, q)
    }
}

/// Portable 4x-unrolled path with prefetch on the next two cache lines.
fn fused_sum_unrolled(slab: &[f32], d: usize, p: usize, q: usize) -> f64 {
    let n = slab.len() / d;
    let mut acc = [0f64; 4];
    let mut r = 0usize;
    while r + 4 <= n {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            // Prefetch ~256 B ahead (4 cache lines) each 4-row step.
            let ahead = (r + 64) * d;
            if ahead < slab.len() {
                _mm_prefetch(slab.as_ptr().add(ahead) as *const i8, _MM_HINT_T0);
            }
        }
        for (k, a) in acc.iter_mut().enumerate() {
            let b = (r + k) * d;
            *a += slab[b + p] as f64 * (1.0 - slab[b + q] as f64);
        }
        r += 4;
    }
    let mut total: f64 = acc.iter().sum();
    while r < n {
        let b = r * d;
        total += slab[b + p] as f64 * (1.0 - slab[b + q] as f64);
        r += 1;
    }
    total
}

/// AVX-512 path: 8-wide FMA over price with (1-discount) precomputed per
/// lane in-register. Horizontal reduce at morsel end. Unsafe: caller
/// guarantees avx512f via runtime detection above.
#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f")]
unsafe fn fused_sum_avx512(slab: &[f32], d: usize, p: usize, q: usize) -> f64 {
    // Gather price/discount lanes (strided by d; gather keeps it correct
    // for any d, at some throughput cost vs unit stride).
    let n = slab.len() / d;
    // NOTE: lanes accumulate in f32, so flush every chunk to an f64 total.
    // A vector f32 accumulator over 375k chunks quantizes (ulp(1e10) = 2048)
    // and drifts ~1e-6; per-chunk flush bounds the error to ~5e-8.
    let mut total = 0f64;
    let one = _mm512_set1_ps(1.0);
    let mut idx = [0i32; 16];
    let mut r = 0usize;
    let mut vec_acc = _mm512_setzero_ps();
    let mut chunk_cnt = 0usize;
    while r + 16 <= n {
        for (k, slot) in idx.iter_mut().enumerate() {
            *slot = ((r + k) * d + p) as i32;
        }
        let pv = _mm512_i32gather_ps(_mm512_loadu_si512(idx.as_ptr() as *const _), slab.as_ptr(), 4);
        for (k, slot) in idx.iter_mut().enumerate() {
            *slot = ((r + k) * d + q) as i32;
        }
        let qv = _mm512_i32gather_ps(_mm512_loadu_si512(idx.as_ptr() as *const _), slab.as_ptr(), 4);
        let factor = _mm512_sub_ps(one, qv);
        vec_acc = _mm512_fmadd_ps(pv, factor, vec_acc);
        chunk_cnt += 1;
        if chunk_cnt == 8 {
            total += _mm512_reduce_add_ps(vec_acc) as f64;
            vec_acc = _mm512_setzero_ps();
            chunk_cnt = 0;
        }
        r += 16;
    }
    if chunk_cnt > 0 {
        total += _mm512_reduce_add_ps(vec_acc) as f64;
    }
    while r < n {
        let b = r * d;
        total += *slab.get_unchecked(b + p) as f64 * (1.0 - *slab.get_unchecked(b + q) as f64);
        r += 1;
    }
    total
}

/// B6: column-wise fused sum over TWO CONTIGUOUS column slices
/// (sum(a[i] * (1 - b[i]))). With SoA as the primary stalk layout the
/// operands are unit-stride, so the kernel needs no gather and no stride
/// arithmetic - the same math as `fused_sum_pairs` on an AoS slab, without
/// the 2x materialization.
pub fn fused_sum_columns(a: &[f32], b: &[f32]) -> f64 {
    debug_assert_eq!(a.len(), b.len(), "column lengths must match");
    let n = a.len();
    let mut acc = 0f64;
    let mut i = 0usize;
    // 4x unrolled with a prefetch 256 B ahead (same shape as the AoS path).
    let mut lanes = [0f64; 4];
    while i + 4 <= n {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            use std::arch::x86_64::{_MM_HINT_T0, _mm_prefetch};
            if i + 64 < n {
                _mm_prefetch(a.as_ptr().add(i + 64) as *const i8, _MM_HINT_T0);
            }
        }
        for k in 0..4 {
            lanes[k] += a[i + k] as f64 * (1.0 - b[i + k] as f64);
        }
        i += 4;
    }
    for l in lanes.iter() {
        acc += *l;
    }
    while i < n {
        acc += a[i] as f64 * (1.0 - b[i] as f64);
        i += 1;
    }
    acc
}
