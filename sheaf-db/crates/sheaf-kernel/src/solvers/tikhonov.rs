// crates/sheaf-kernel/src/solvers/tikhonov.rs — regularize Laplacian linear solves
// FILE-028. Depends on FILE-027. Minimizes E(x) + λ||x−x₀||², i.e. solves
// (L + λI)x = λx₀ via BJ-PCG. λ > 0 cures the Laplacian's zero mode
// (constant sections) and bounds κ; the bench reports the λ it used.
use super::cg::{pcg_monitored, CgLimits, CgResult, Monitor};
use crate::operators::laplacian::SheafLaplacian;

/// Full relaxation configuration in one struct (clippy: ≤7 args everywhere).
#[derive(Clone, Copy, Debug)]
pub struct RelaxConfig {
    pub lambda: f32,
    pub tol: f32,
    pub max_iter: usize,
    pub energy_target: f32,
    pub check_every: usize,
    pub n_threads: usize,
}

/// Solve the Tikhonov-regularized system from initial guess x (in place).
/// Returns iterations and relative residual of the REGULATED system.
pub fn regularized_solve(
    lap: &SheafLaplacian,
    x0: &[f32],
    x: &mut [f32],
    lambda: f32,
    tol: f32,
    max_iter: usize,
) -> CgResult {
    regularized_solve_monitored(lap, x0, x, lambda, CgLimits { tol, max_iter }, None, 1)
}

/// Monitored twin: the monitor closure receives each checked iterate and
/// returns true to stop early (energy-targeted harmonic stop). n_threads
/// selects apply_into_par (1 = reference single-thread path).
pub fn regularized_solve_monitored(
    lap: &SheafLaplacian,
    x0: &[f32],
    x: &mut [f32],
    lambda: f32,
    lim: CgLimits,
    monitor: Monitor<'_>,
    n_threads: usize,
) -> CgResult {
    assert!(lambda > 0.0, "Tikhonov needs positive lambda");
    assert_eq!(x0.len(), lap.n * lap.d);
    assert_eq!(x.len(), lap.n * lap.d);
    let bj = lap.block_jacobi_cached(lambda);
    let d = lap.d;
    let mut b = vec![0.0f32; x0.len()];
    for (dst, src) in b.iter_mut().zip(x0.iter()) {
        *dst = lambda * src;
    }
    if n_threads <= 1 {
        pcg_monitored(
            |v, out| {
                lap.apply_into(v, out);
                for (o, val) in out.iter_mut().zip(v.iter()) {
                    *o += lambda * val;
                }
            },
            &bj,
            &b,
            x,
            d,
            lim,
            monitor,
        )
    } else {
        pcg_monitored(
            |v, out| {
                lap.apply_into_par(v, out, n_threads);
                for (o, val) in out.iter_mut().zip(v.iter()) {
                    *o += lambda * val;
                }
            },
            &bj,
            &b,
            x,
            d,
            lim,
            monitor,
        )
    }
}
