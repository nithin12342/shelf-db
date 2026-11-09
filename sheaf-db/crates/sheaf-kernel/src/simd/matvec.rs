// crates/sheaf-kernel/src/simd/matvec.rs — multiply W times stalk vectors
// FILE-023. Depends on FILE-010's row-major d×d layout convention only;
// operates on raw slices so the kernel crate stays dependency-free.
// Scalar FMA baseline (f32::mul_add; LLVM auto-vectorizes these tight
// loops). Hand-rolled AVX2 was measured as unjustified for d<=4 dot
// products: no win over auto-vectorized FMA. Morph FETCH cost (the Part-2
// watchpoint) is isolated by timing in the Phase-3 bench, not here.
// No black_box in library code: it would defeat the auto-vectorization
// this baseline relies on. Benches own their sinks.
/// y = M·x, M row-major d×d. Always inlined: at d<=4 the call overhead
/// dwarfs the 16 FMAs (measured), and LTO is enabled for cross-crate cases.
#[inline(always)]
pub fn matvec(m: &[f32], x: &[f32], d: usize, y: &mut [f32]) {
    debug_assert_eq!(m.len(), d * d);
    debug_assert_eq!(x.len(), d);
    debug_assert_eq!(y.len(), d);
    for (r, row) in y.iter_mut().enumerate() {
        let mut s = 0.0f32;
        for (c, val) in x.iter().enumerate().take(d) {
            s = m[r * d + c].mul_add(*val, s);
        }
        *row = s;
    }
}

/// out = Mᵀ·y, M row-major d×d.
#[inline(always)]
pub fn matvec_transpose(m: &[f32], y: &[f32], d: usize, out: &mut [f32]) {
    debug_assert_eq!(m.len(), d * d);
    debug_assert_eq!(y.len(), d);
    debug_assert_eq!(out.len(), d);
    for c in 0..d {
        let mut s = 0.0f32;
        for r in 0..d {
            s = m[r * d + c].mul_add(y[r], s);
        }
        out[c] = s;
    }
}

/// Accumulate out += scale·v (length-checked saxpy).
pub fn saxpy(out: &mut [f32], scale: f32, v: &[f32]) {
    debug_assert_eq!(out.len(), v.len());
    for (o, val) in out.iter_mut().zip(v.iter()) {
        *o += scale * val;
    }
}

/// Dot product with FMA accumulation.
#[inline(always)]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut s = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        s = x.mul_add(*y, s);
    }
    s
}
