//! fallback - portable scalar Rust adapter for the Tier 1 port.
//!
//! This adapter keeps the workspace buildable and testable on machines
//! without a C++ toolchain. It implements the same mathematics as the
//! C++23 library, in the same operation order where the order affects
//! floating-point results. The tiled matvec uses plain accumulation here;
//! the parity tests allow a small f32 tolerance for reordering differences.

use super::{ComplexSpec, KernelBackend, KernelError};

/// Scalar adapter. It holds no state, so many workers can share one value.
pub struct FallbackKernel;

#[inline]
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// Spread the bits of x into even positions (same network as morton.cpp).
fn spread_bits(x: u32) -> u64 {
    let mut v = x as u64;
    v = (v | (v << 16)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v << 8)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v << 4)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v << 2)) & 0x3333_3333_3333_3333;
    v = (v | (v << 1)) & 0x5555_5555_5555_5555;
    v
}

fn compact_bits(mut v: u64) -> u32 {
    v &= 0x5555_5555_5555_5555;
    v = (v | (v >> 1)) & 0x3333_3333_3333_3333;
    v = (v | (v >> 2)) & 0x0F0F_0F0F_0F0F_0F0F;
    v = (v | (v >> 4)) & 0x00FF_00FF_00FF_00FF;
    v = (v | (v >> 8)) & 0x0000_FFFF_0000_FFFF;
    v = (v | (v >> 16)) & 0x0000_0000_FFFF_FFFF;
    v as u32
}

impl KernelBackend for FallbackKernel {
    fn name(&self) -> &'static str {
        "rust-fallback"
    }

    fn coboundary(
        &self, cx: &ComplexSpec<'_>, s: &[f32], edge_out: &mut [f32],
    ) -> Result<(), KernelError> {
        cx.validate()?;
        let d = cx.stalk_dim as usize;
        if s.len() != cx.vertex_floats() || edge_out.len() != cx.edge_floats() {
            return Err(KernelError::InvalidArgument);
        }
        for (e, (&src, &dst)) in cx.edge_src.iter().zip(cx.edge_dst).enumerate() {
            let src = &s[src as usize * d..][..d];
            let dst = &s[dst as usize * d..][..d];
            let out = &mut edge_out[e * d..][..d];
            match (cx.r_src, cx.r_dst) {
                // Identity restriction: oriented difference of stalk vectors.
                (None, None) => {
                    for j in 0..d {
                        out[j] = dst[j] - src[j];
                    }
                }
                (Some(r_src), Some(r_dst)) => {
                    let block = e * d * d;
                    for i in 0..d {
                        let row_dst = &r_dst[block + i * d..][..d];
                        let row_src = &r_src[block + i * d..][..d];
                        out[i] = dot(row_dst, dst) - dot(row_src, src);
                    }
                }
                _ => return Err(KernelError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn coboundary_adjoint(
        &self, cx: &ComplexSpec<'_>, edge_vec: &[f32], vertex_out: &mut [f32],
    ) -> Result<(), KernelError> {
        cx.validate()?;
        let d = cx.stalk_dim as usize;
        if edge_vec.len() != cx.edge_floats() || vertex_out.len() != cx.vertex_floats() {
            return Err(KernelError::InvalidArgument);
        }
        vertex_out.fill(0.0);
        for (e, (&src, &dst)) in cx.edge_src.iter().zip(cx.edge_dst).enumerate() {
            let v = &edge_vec[e * d..][..d];
            let src_off = src as usize * d;
            let dst_off = dst as usize * d;
            match (cx.r_src, cx.r_dst) {
                (None, None) => {
                    for j in 0..d {
                        vertex_out[dst_off + j] += v[j];
                        vertex_out[src_off + j] -= v[j];
                    }
                }
                (Some(r_src), Some(r_dst)) => {
                    // Transpose scatter: out += R^T v walks R row by row.
                    let block = e * d * d;
                    for i in 0..d {
                        let w = v[i];
                        let row_dst = &r_dst[block + i * d..][..d];
                        let row_src = &r_src[block + i * d..][..d];
                        for j in 0..d {
                            vertex_out[dst_off + j] += w * row_dst[j];
                            vertex_out[src_off + j] -= w * row_src[j];
                        }
                    }
                }
                _ => return Err(KernelError::InvalidArgument),
            }
        }
        Ok(())
    }

    fn laplacian_apply(
        &self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32], out: &mut [f32],
    ) -> Result<(), KernelError> {
        if scratch_edges.len() < cx.edge_floats() {
            return Err(KernelError::ScratchTooSmall);
        }
        self.coboundary(cx, s, scratch_edges)?;
        self.coboundary_adjoint(cx, scratch_edges, out)
    }

    fn energy(&self, cx: &ComplexSpec<'_>, s: &[f32], scratch_edges: &mut [f32]) -> f32 {
        if self.coboundary(cx, s, scratch_edges).is_err() {
            return -1.0;
        }
        scratch_edges.iter().map(|v| v * v).sum()
    }

    fn xor_fold(
        &self, base: &[u8], deltas: &[&[u8]], out: &mut [u8],
    ) -> Result<(), KernelError> {
        if out.len() != base.len() || deltas.iter().any(|d| d.len() != base.len()) {
            return Err(KernelError::InvalidArgument);
        }
        out.copy_from_slice(base);
        for delta in deltas {
            for (o, d) in out.iter_mut().zip(delta.iter()) {
                *o ^= *d;
            }
        }
        Ok(())
    }

    fn xor_encode(
        &self, base: &[u8], target: &[u8], delta_out: &mut [u8],
    ) -> Result<(), KernelError> {
        if base.len() != target.len() || delta_out.len() != base.len() {
            return Err(KernelError::InvalidArgument);
        }
        for ((o, b), t) in delta_out.iter_mut().zip(base).zip(target) {
            *o = b ^ t;
        }
        Ok(())
    }

    fn morton_encode(&self, x: u32, y: u32) -> u64 {
        spread_bits(x) | (spread_bits(y) << 1)
    }

    fn morton_decode(&self, key: u64) -> (u32, u32) {
        (compact_bits(key), compact_bits(key >> 1))
    }

    fn matvec_tiled(
        &self, matrix: &[f32], rows: u32, cols: u32, x: &[f32], out: &mut [f32],
    ) -> Result<(), KernelError> {
        let (r, c) = (rows as usize, cols as usize);
        if matrix.len() != r * c || x.len() != c || out.len() != r {
            return Err(KernelError::InvalidArgument);
        }
        // Plain accumulation. Tiling in the native adapter only changes the
        // addition order, which the parity tolerance covers.
        for i in 0..r {
            out[i] = dot(&matrix[i * c..][..c], x);
        }
        Ok(())
    }

    fn compact_valid(&self, values: &[f32], mask_bits: &[u64], out: &mut [f32]) -> usize {
        let count = values.len();
        let words = count.div_ceil(64);
        if mask_bits.len() < words || out.len() < count {
            return 0;
        }
        let mut kept = 0;
        for (i, &v) in values.iter().enumerate() {
            if (mask_bits[i / 64] >> (i % 64)) & 1 == 1 {
                out[kept] = v;
                kept += 1;
            }
        }
        kept
    }

    fn chebyshev_scratch_floats(&self, cx: &ComplexSpec<'_>) -> usize {
        3 * cx.vertex_floats() + cx.edge_floats()
    }

    fn chebyshev_solve(
        &self, cx: &ComplexSpec<'_>, b: &[f32], x: &mut [f32],
        max_iters: u32, tol: f32, lambda_min: f32, lambda_max: f32,
        scratch: &mut [f32],
    ) -> Result<u32, KernelError> {
        cx.validate()?;
        let n = cx.vertex_floats();
        if b.len() != n || x.len() != n {
            return Err(KernelError::InvalidArgument);
        }
        if scratch.len() < self.chebyshev_scratch_floats(cx) {
            return Err(KernelError::ScratchTooSmall);
        }
        if !(lambda_max > 0.0) || lambda_min < 0.0 || lambda_max < lambda_min || !(tol > 0.0) {
            return Err(KernelError::InvalidArgument);
        }

        // Split the arena: [ r | x_prev | lx | edge cochain ].
        let (r, rest) = scratch.split_at_mut(n);
        let (x_prev, rest) = rest.split_at_mut(n);
        let (lx, edge_scratch) = rest.split_at_mut(n);

        let center = 0.5 * (lambda_max + lambda_min);
        let half_width = 0.5 * (lambda_max - lambda_min);
        let b_norm = dot(b, b).sqrt();
        let target = tol * b_norm.max(1e-30);
        let target2 = target * target;

        let mut alpha_prev = 1.0 / center;
        let mut first = true;
        for k in 0..max_iters {
            self.laplacian_apply(cx, x, edge_scratch, lx)?;
            for i in 0..n {
                r[i] = b[i] - lx[i];
            }
            if dot(r, r) <= target2 {
                return Ok(k);
            }
            let (beta, alpha) = if first {
                (0.0, alpha_prev)
            } else {
                let t = 0.5 * half_width * alpha_prev;
                let beta = t * t;
                (beta, 1.0 / (center - beta / alpha_prev))
            };
            for i in 0..n {
                let old = x[i];
                x[i] = old + alpha * r[i] + beta * (old - x_prev[i]);
                x_prev[i] = old;
            }
            alpha_prev = alpha;
            first = false;
        }
        Err(KernelError::NoConvergence)
    }
}