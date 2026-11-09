// crates/sheaf-kernel/src/solvers/harmonic.rs — drive harmonic section relaxation
// FILE-029. Depends on FILE-026 (energy measure) + FILE-028 (solve).
// One call: regularized-solve a stalk assignment, then score its Dirichlet
// energy. The E2E bench gates on the returned triple, nothing else.
use super::tikhonov::{regularized_solve_monitored, RelaxConfig};
use crate::operators::energy::dirichlet_energy;
use crate::operators::laplacian::SheafLaplacian;

#[derive(Clone, Copy, Debug)]
pub struct HarmonicResult {
    pub iters: usize,
    pub rel_residual: f32,
    pub energy: f32,
    pub target_hit: bool,
}

/// Relax x0 toward the harmonic section; x0 is left untouched, solution returned.
/// Stops early when Dirichlet energy reaches cfg.energy_target (checked every
/// cfg.check_every iterations) — the gate in SPEC-012 is stated on energy, so
/// the solver stops on energy, not just on residual tolerance.
pub fn harmonic_relax_target(
    lap: &SheafLaplacian,
    x0: &[f32],
    cfg: RelaxConfig,
) -> (Vec<f32>, HarmonicResult, bool) {
    let mut x = x0.to_vec();
    let mut hit = dirichlet_energy(&lap.ops, &x) <= cfg.energy_target;
    let mut monitor = |it: usize, cur: &[f32]| {
        if it.is_multiple_of(cfg.check_every.max(1)) {
            hit = dirichlet_energy(&lap.ops, cur) <= cfg.energy_target;
            hit
        } else {
            false
        }
    };
    let cg = regularized_solve_monitored(
        lap,
        x0,
        &mut x,
        cfg.lambda,
        super::cg::CgLimits { tol: cfg.tol, max_iter: cfg.max_iter },
        Some(&mut monitor),
        cfg.n_threads,
    );
    let energy = dirichlet_energy(&lap.ops, &x);
    // Final energy check: the monitor may not have been called after the last
    // CG step (e.g., tol reached before check_every aligned). Ensure target_hit
    // reflects the true final energy.
    hit = hit || energy <= cfg.energy_target;
    let r = HarmonicResult { iters: cg.iters, rel_residual: cg.rel_residual, energy, target_hit: hit };
    (x, r, hit)
}
