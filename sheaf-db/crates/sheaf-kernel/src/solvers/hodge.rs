// crates/sheaf-kernel/src/solvers/hodge.rs — Hodge smoothing projection
// MATH-UPGRADES Stage M4. Depends on operators (Laplacian) + simd (matvec).
// One DAMPED Jacobi residual-smoothing step toward (L+λI)x = b:
//   x0' = x0 - ω·M^-1 ((L+λI)x0 - b),  M^-1 = per-cell dxd blocks.
// Damping is load-bearing: ω=1.0 diverges on high-strain starts (measured
// E=1272 vs 7e-4 — full-step overshoot), ω≈0.25 smooths (multigrid
// Richardson practice). Theory: M^-1(L+λI) damps high-frequency
// (boundary-noise) error components first (Hodge: exact/curled parts
// separate); the smoothed start cuts the Krylov iterations the generic PCG
// needs on ill-conditioned (hub-heavy) complexes. Same M^-1 convention as
// cg.rs (per-cell dxd row-major).
use crate::operators::laplacian::SheafLaplacian;
use crate::simd::matvec::matvec;

pub fn hodge_start(lap: &SheafLaplacian, x0: &[f32], b: &[f32], lambda: f32, omega: f32) -> Vec<f32> {
    let (n, d) = (lap.n, lap.d);
    assert_eq!(x0.len(), n * d, "x0 shape");
    assert_eq!(b.len(), n * d, "rhs shape");
    let mut ax = vec![0.0f32; n * d];
    lap.apply_into(x0, &mut ax);
    for (o, v) in ax.iter_mut().zip(x0.iter()) {
        *o += lambda * v;
    }
    let bj = lap.block_jacobi_inv(lambda);
    assert_eq!(bj.len(), n * d * d, "preconditioner shape");
    let mut x1 = x0.to_vec();
    let mut r = vec![0.0f32; d];
    let mut z = vec![0.0f32; d];
    for v in 0..n {
        for k in 0..d {
            r[k] = ax[v * d + k] - b[v * d + k];
        }
        matvec(&bj[v * d * d..(v + 1) * d * d], &r, d, &mut z);
        for k in 0..d {
            x1[v * d + k] -= omega * z[k];
        }
    }
    x1
}
