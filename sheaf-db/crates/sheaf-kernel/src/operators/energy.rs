// crates/sheaf-kernel/src/operators/energy.rs — score Dirichlet energy coherence
// FILE-026. Depends on FILE-024. E(x) = Σ_τ ||δ_τ(x)||² over hypercells.
// Pure measurement: never mutates state, never solves.
use super::coboundary::CoboundaryOp;

/// Total Dirichlet energy of a 0-cell stalk assignment.
pub fn dirichlet_energy(ops: &[CoboundaryOp], stalks: &[f32]) -> f32 {
    let mut total = 0.0f32;
    for op in ops {
        total += op.cell_energy(stalks);
    }
    total
}

/// H¹-proxy inconsistency score: E(x)/‖x‖² (dimensionless). Zero means a
/// global section; positive measures disagreement the harmonic projection
/// removes. Compares across solves without a Betti computation.
pub fn inconsistency(energy: f32, norm_sq: f64) -> f64 {
    if norm_sq <= 0.0 {
        return f64::INFINITY;
    }
    energy as f64 / norm_sq
}
