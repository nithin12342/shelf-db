// crates/sheaf-kernel/src/operators/laplacian.rs — assemble sparse sheaf Laplacian
// FILE-025. Depends on FILE-024. Matrix-free L = δᵀδ over 0-cell stalks:
// unknowns are the dim-0 prefix (flat n·d), hypercells contribute operators
// only. Convention (enforced at build): every face id < n, i.e. fixtures
// store all 0-cells first. apply() needs zero allocation: stack buffers.
use super::coboundary::{CoboundaryOp, MAX_D};
use crate::simd::matvec::{matvec, matvec_transpose};
use std::sync::Arc;

/// Fingerprint for cached block-Jacobi factors.
type BjKey = (u32, usize, usize, usize, u32);

pub struct SheafLaplacian {
    pub n: usize,
    pub d: usize,
    pub ops: Vec<CoboundaryOp>,
    /// Per-cell Σ_τ M_τᵀM_τ blocks (flat n·d·d), unregularized.
    pub diag: Vec<f32>,
    /// Per-op: morph is exactly diagonal → O(d) fast path in apply.
    /// Exact-zero test only (no tolerance invention); recomputed by build().
    pub diag_only: Vec<bool>,
    /// Cached block-Jacobi factors + fingerprint (see block_jacobi_cached).
    /// Key = (λ bits, n, d, ops len, Σdiag bits).
    cached_bj: std::cell::RefCell<Option<(BjKey, Arc<Vec<f32>>)>>,
    scratch_pool: std::cell::RefCell<Vec<Vec<f32>>>,
}

impl SheafLaplacian {
    pub fn build(n: usize, d: usize, ops: Vec<CoboundaryOp>) -> Self {
        assert!(matches!(d, 1..=MAX_D));
        let mut diag = vec![0.0f32; n * d * d];
        let mut mtm = [0.0f32; MAX_D * MAX_D];
        let mut diag_only = Vec::with_capacity(ops.len());
        for op in &ops {
            assert_eq!(op.d, d, "mixed stalk dims in one Laplacian");
            // MᵀM once per op (morph is per-op, not per-face): identical block
            // is added into every face's diagonal — previously recomputed
            // per face at O(d³) each.
            for r in 0..d {
                for c in 0..d {
                    let mut s = 0.0f32;
                    for k in 0..d {
                        // NOTE (B7 precedent): mul_add(a,b,s) includes +s —
                        // never write `s += a.mul_add(b,s)`.
                        s = op.morph[k * d + r].mul_add(op.morph[k * d + c], s);
                    }
                    mtm[r * MAX_D + c] = s;
                }
            }
            // Diagonal morph ⇒ MᵀM diagonal too, so the O(d) fast path in
            // apply is exact. Exact-zero test only (no tolerance invented).
            let mut morph_diag = true;
            for r in 0..d {
                for c in 0..d {
                    if r != c && op.morph[r * d + c] != 0.0 {
                        morph_diag = false;
                    }
                }
            }
            diag_only.push(morph_diag);
            for face in &op.faces {
                assert!((*face as usize) < n, "face id outside 0-cell prefix");
                let base = (*face as usize) * d * d;
                for r in 0..d {
                    for c in 0..d {
                        diag[base + r * d + c] += mtm[r * MAX_D + c];
                    }
                }
            }
        }
        Self { n, d, ops, diag, diag_only, cached_bj: std::cell::RefCell::new(None), scratch_pool: std::cell::RefCell::new(Vec::new()) }
    }

    /// out = L·x (overwrite contract: zeroes out first, so solver loops
    /// cannot accumulate stale values — a stale-accumulation bug here once
    /// drove CG residual to 1.14 instead of converging. The 160 KB memset
    /// is negligible next to the SpMV itself.)
    ///
    /// Parallel twin below shares the per-op body through accumulate_op so
    /// the two paths cannot silently diverge.
    pub fn apply_into(&self, x: &[f32], out: &mut [f32]) {
        debug_assert_eq!(x.len(), self.n * self.d);
        debug_assert_eq!(out.len(), self.n * self.d);
        for slot in out.iter_mut() {
            *slot = 0.0;
        }
        for (i, op) in self.ops.iter().enumerate() {
            Self::accumulate_op(self.d, op, self.diag_only[i], x, out);
        }
    }

    /// One hypercell's δᵀδ contribution, added into out.
    /// Mᵀ·delta is loop-invariant (same morph + delta for every face):
    /// computed once, sign-scaled per face — previously (|faces|-1) wasted
    /// d×d matvecs per op per apply. Diagonal morphs take the O(d) path.
    fn accumulate_op(d: usize, op: &CoboundaryOp, is_diag: bool, x: &[f32], out: &mut [f32]) {
        let mut proj = [0.0f32; MAX_D];
        let mut delta = [0.0f32; MAX_D];
        let mut back = [0.0f32; MAX_D];
        for slot in delta[..d].iter_mut() {
            *slot = 0.0;
        }
        if is_diag {
            for (face, sign) in op.faces.iter().zip(op.signs.iter()) {
                let base = (*face as usize) * d;
                for i in 0..d {
                    delta[i] += sign * op.morph[i * d + i] * x[base + i];
                }
            }
            for i in 0..d {
                back[i] = op.morph[i * d + i] * delta[i];
            }
            for (face, sign) in op.faces.iter().zip(op.signs.iter()) {
                let base = (*face as usize) * d;
                for i in 0..d {
                    out[base + i] += sign * back[i];
                }
            }
            return;
        }
        for (face, sign) in op.faces.iter().zip(op.signs.iter()) {
            let base = (*face as usize) * d;
            matvec(&op.morph, &x[base..base + d], d, &mut proj[..d]);
            for (slot, val) in delta[..d].iter_mut().zip(proj[..d].iter()) {
                *slot += sign * val;
            }
        }
        matvec_transpose(&op.morph, &delta[..d], d, &mut back[..d]);
        for (face, sign) in op.faces.iter().zip(op.signs.iter()) {
            let base = (*face as usize) * d;
            for (slot, val) in out[base..base + d].iter_mut().zip(back[..d].iter()) {
                *slot += sign * val;
            }
        }
    }

    /// Threaded twin of apply_into: fixed contiguous chunks, per-thread
    /// buffers, ordered reduce — deterministic for a given thread count.
    /// Used by the solver hot loop; apply_into remains the reference path.
    pub fn apply_into_par(&self, x: &[f32], out: &mut [f32], n_threads: usize) {
        debug_assert_eq!(x.len(), self.n * self.d);
        debug_assert_eq!(out.len(), self.n * self.d);
        for slot in out.iter_mut() {
            *slot = 0.0;
        }
        let d = self.d;
        let out_len = out.len();
        let chunk = (self.ops.len() + n_threads - 1) / n_threads.max(1);
        let op_chunks: Vec<_> = self.ops.chunks(chunk).collect();
        let flag_chunks: Vec<_> = self.diag_only.chunks(chunk).collect();
        let num_actual_threads = op_chunks.len();

        let mut pool = self.scratch_pool.borrow_mut();
        while pool.len() < num_actual_threads {
            pool.push(vec![0.0f32; out_len]);
        }
        for buf in pool.iter_mut().take(num_actual_threads) {
            if buf.len() < out_len {
                buf.resize(out_len, 0.0);
            }
            buf[..out_len].fill(0.0);
        }

        let (active_bufs, _) = pool.split_at_mut(num_actual_threads);

        std::thread::scope(|s| {
            let mut handles = Vec::new();
            for (buf, (c, flags)) in active_bufs.iter_mut().zip(op_chunks.into_iter().zip(flag_chunks.into_iter())) {
                let local = &mut buf[..out_len];
                handles.push(s.spawn(move || {
                    for (op, is_diag) in c.iter().zip(flags.iter()) {
                        Self::accumulate_op(d, op, *is_diag, x, local);
                    }
                }));
            }
            for h in handles {
                h.join().unwrap();
            }
        });

        for buf in active_bufs.iter() {
            for (slot, val) in out.iter_mut().zip(buf[..out_len].iter()) {
                *slot += val;
            }
        }
    }

    /// Restrict to the active set (cells incident to ≥1 hypercell).
    /// Isolated cells solve analytically (x = x₀ exactly: empty rows give
    /// (0+λI)x = λx₀), so excluding them is exact, not approximate. Returns
    /// the compact Laplacian plus both index maps. On sparse fixtures this
    /// removes most unknowns (measured: 10k → ~3.3k) from every O(n) pass.
    pub fn restrict_active(&self) -> (SheafLaplacian, Vec<u32>, Vec<u32>) {
        let mut active = vec![false; self.n];
        for op in &self.ops {
            for f in &op.faces {
                active[*f as usize] = true;
            }
        }
        let mut g2c = vec![u32::MAX; self.n];
        let mut c2g = Vec::new();
        for (g, on) in active.iter().enumerate() {
            if *on {
                g2c[g] = c2g.len() as u32;
                c2g.push(g as u32);
            }
        }
        let ops = self
            .ops
            .iter()
            .map(|op| {
                CoboundaryOp::new(
                    op.faces.iter().map(|f| g2c[*f as usize]).collect(),
                    op.signs.clone(),
                    op.morph.clone(),
                    op.d,
                )
            })
            .collect();
        (SheafLaplacian::build(c2g.len(), self.d, ops), g2c, c2g)
    }
    /// Cached twin of block_jacobi_inv: same factors, recomputed only when
    /// the fingerprint changes. Key = (λ bits, n, d, ops len, Σdiag), all
    /// bit/order-deterministic for unchanged data, so a hit is exact — never
    /// a stale approximation. External mutation of the pub fields changes
    /// Σdiag/ops-len and safely misses. RefCell keeps the &self API (zero
    /// ripple into E2E callers); one borrow per solve, never per iteration.
    pub fn block_jacobi_cached(&self, lambda: f32) -> Arc<Vec<f32>> {
        let mut sum = 0.0f32;
        for &v in &self.diag {
            sum += v;
        }
        let key = (lambda.to_bits(), self.n, self.d, self.ops.len(), sum.to_bits());
        if let Some((k, factors)) = self.cached_bj.borrow().as_ref() {
            if *k == key {
                return Arc::clone(factors);
            }
        }
        let factors = Arc::new(self.block_jacobi_inv(lambda));
        *self.cached_bj.borrow_mut() = Some((key, Arc::clone(&factors)));
        factors
    }
    /// Block-Jacobi factors: per-cell inverse of (D_v + λI), flat n·d·d.
    /// Cholesky (D_v + λI is SPD by construction): half the flops of
    /// Gauss-Jordan with no pivot branches. Asserts definiteness.
    pub fn block_jacobi_inv(&self, lambda: f32) -> Vec<f32> {
        let (n, d) = (self.n, self.d);
        let mut inv = vec![0.0f32; n * d * d];
        // Lower-triangular factor in scratch, reused per cell (no allocs).
        let mut l = [0.0f32; MAX_D * MAX_D];
        let mut y = [0.0f32; MAX_D];
        for v in 0..n {
            let src = &self.diag[v * d * d..(v + 1) * d * d];
            // Factor A = LLᵀ.
            let mut ok = true;
            for k in 0..d {
                let mut s = src[k * d + k] + lambda;
                for j in 0..k {
                    s -= l[k * MAX_D + j] * l[k * MAX_D + j];
                }
                if s <= 1e-18 {
                    ok = false;
                    break;
                }
                l[k * MAX_D + k] = s.sqrt();
                for i in k + 1..d {
                    let mut t = src[i * d + k];
                    for j in 0..k {
                        t -= l[i * MAX_D + j] * l[k * MAX_D + j];
                    }
                    l[i * MAX_D + k] = t / l[k * MAX_D + k];
                }
            }
            assert!(ok, "non-PD diagonal block (should be impossible for D+λI)");
            // Invert column by column: solve L·y = e_j, then Lᵀ·x = y.
            for j in 0..d {
                y.iter_mut().enumerate().for_each(|(i, s)| *s = if i == j { 1.0 } else { 0.0 });
                for i in 0..d {
                    let mut s = y[i];
                    for k in 0..i {
                        s -= l[i * MAX_D + k] * y[k];
                    }
                    y[i] = s / l[i * MAX_D + i];
                }
                for i in (0..d).rev() {
                    let mut s = y[i];
                    for k in i + 1..d {
                        s -= l[k * MAX_D + i] * inv[v * d * d + k * d + j];
                    }
                    inv[v * d * d + i * d + j] = s / l[i * MAX_D + i];
                }
            }
        }
        inv
    }
}
