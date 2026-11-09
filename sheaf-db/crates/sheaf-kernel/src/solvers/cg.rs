// crates/sheaf-kernel/src/solvers/cg.rs — solve preconditioned conjugate gradient
// FILE-027. Depends on FILE-025 (operator shape) only via the apply closure:
// this file never imports storage or reader types. Block-Jacobi precondition
// enters as flat per-cell inverse blocks; PCG follows Shewchuk's algorithm
// with relative-residual stop. Allocs work vectors once per call.
use crate::simd::matvec::{dot, matvec};

pub struct CgResult {
    pub iters: usize,
    pub rel_residual: f32,
}

/// Stopping rules, shared by every solver entry point.
#[derive(Clone, Copy, Debug)]
pub struct CgLimits {
    pub tol: f32,
    pub max_iter: usize,
}

/// Monitor hook type: receives (iteration, current x), returns true to stop.
pub type Monitor<'a> = Option<&'a mut dyn FnMut(usize, &[f32]) -> bool>;

/// Solve A·x = b from initial guess x (updated in place).
/// `apply(x, out)` must compute out = A·x. `bj_inv` holds per-cell d×d
/// inverse blocks (flat n·d·d) or is empty for unpreconditioned CG.
/// `monitor`, when present, runs every 2nd iteration with (iter, x);
/// returning true stops the solve early (energy-targeted harmonic stop).
pub fn pcg<F>(apply: F, bj_inv: &[f32], b: &[f32], x: &mut [f32], d: usize, lim: CgLimits) -> CgResult
where
    F: Fn(&[f32], &mut [f32]),
{
    pcg_monitored(apply, bj_inv, b, x, d, lim, None)
}

pub fn pcg_monitored<F>(
    apply: F,
    bj_inv: &[f32],
    b: &[f32],
    x: &mut [f32],
    d: usize,
    lim: CgLimits,
    mut monitor: Monitor<'_>,
) -> CgResult
where
    F: Fn(&[f32], &mut [f32]),
{
    let n = b.len();
    assert_eq!(x.len(), n);
    assert!(bj_inv.is_empty() || bj_inv.len() == n * d, "preconditioner shape mismatch");
    let use_bj = !bj_inv.is_empty();
    let mut r = vec![0.0f32; n];
    let mut z = vec![0.0f32; n];
    let mut p = vec![0.0f32; n];
    let mut ap = vec![0.0f32; n];
    apply(x, &mut ap);
    for i in 0..n {
        r[i] = b[i] - ap[i];
    }
    let b_norm = dot(b, b).sqrt().max(1e-30);
    // z = M⁻¹r (block-diagonal apply).
    if use_bj {
        for v in 0..n / d {
            matvec(&bj_inv[v * d * d..(v + 1) * d * d], &r[v * d..(v + 1) * d], d, &mut z[v * d..(v + 1) * d]);
        }
    } else {
        z.copy_from_slice(&r);
    }
    p.copy_from_slice(&z);
    let mut rz_old = dot(&r, &z);
    if rz_old == 0.0 {
        return CgResult { iters: 0, rel_residual: 0.0 };
    }
    for it in 1..=lim.max_iter {
        apply(&p, &mut ap);
        let p_ap = dot(&p, &ap);
        assert!(p_ap > 0.0, "CG breakdown: pᵀAp non-positive (A not SPD?)");
        let alpha = rz_old / p_ap;
        for i in 0..n {
            x[i] += alpha * p[i];
            r[i] -= alpha * ap[i];
        }
        let rel = dot(&r, &r).sqrt() / b_norm;
        if use_bj {
            for v in 0..n / d {
                matvec(&bj_inv[v * d * d..(v + 1) * d * d], &r[v * d..(v + 1) * d], d, &mut z[v * d..(v + 1) * d]);
            }
        } else {
            z.copy_from_slice(&r);
        }
        if rel <= lim.tol {
            return CgResult { iters: it, rel_residual: rel };
        }
        if it.is_multiple_of(2) {
            if let Some(stop) = monitor.as_mut() {
                if stop(it, x) {
                    return CgResult { iters: it, rel_residual: rel };
                }
            }
        }
        let rz_new = dot(&r, &z);
        let beta = rz_new / rz_old;
        for i in 0..n {
            p[i] = z[i] + beta * p[i];
        }
        rz_old = rz_new;
    }
    let rel = dot(&r, &r).sqrt() / b_norm;
    CgResult { iters: lim.max_iter, rel_residual: rel }
}
