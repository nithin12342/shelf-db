// h1.rs - the first cohomology quotient solver (NF-04).
//
// A 1-cochain b splits into an exact part delta(x) and a harmonic part h.
// The exact part comes from the least-squares global representative x. The
// harmonic part h = b - delta(x) is the obstruction class: it is orthogonal
// to im(delta), so delta^T h is zero. A nonzero h means the boundary data
// cannot come from any global section.

use crate::least_squares::{Complex1, SolveError};

/// The quotient of a 1-cochain into an exact part and a harmonic class.
#[derive(Clone, Debug)]
pub struct H1Solution {
    /// The mean-centred global representative.
    pub representative: Vec<f32>,
    /// The harmonic obstruction class, b - delta(x).
    pub obstruction: Vec<f32>,
    /// The Euclidean norm of the obstruction.
    pub residual_norm: f32,
}

impl H1Solution {
    /// The exact part delta(x).
    pub fn exact(&self, complex: &Complex1) -> Result<Vec<f32>, SolveError> {
        complex.coboundary(&self.representative)
    }
}

/// Decompose b into its exact and harmonic parts.
pub fn solve_h1(
    complex: &Complex1,
    b: &[f32],
    tolerance: f32,
    max_iterations: usize,
) -> Result<H1Solution, SolveError> {
    let sources: Vec<u32> = complex.edges.iter().map(|edge| edge.u as u32).collect();
    let targets: Vec<u32> = complex.edges.iter().map(|edge| edge.v as u32).collect();
    let spec = sheaf_native::kernel::ComplexSpec {
        num_vertices: complex.vertices as u32,
        stalk_dim: 1,
        edge_src: &sources,
        edge_dst: &targets,
        r_src: None,
        r_dst: None,
    };
    let native = sheaf_native::math::h1_solve(
        &spec,
        b,
        tolerance,
        max_iterations.min(u32::MAX as usize) as u32,
    )
    .map_err(|_| SolveError::BadCochain)?;
    Ok(H1Solution {
        representative: native.representative,
        obstruction: native.obstruction,
        residual_norm: native.residual_norm,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_exact_cochain_has_no_obstruction() {
        let complex = Complex1::new(3, vec![(0, 1), (1, 2)]).expect("complex");
        let exact = complex.coboundary(&[-1.0, 2.0, 0.5]).expect("coboundary");
        let solution = solve_h1(&complex, &exact, 1e-6, 512).expect("solve");
        assert!(solution.residual_norm < 1e-4, "{}", solution.residual_norm);
    }

    #[test]
    fn a_cycle_carries_a_nonzero_obstruction() {
        let complex = Complex1::new(3, vec![(0, 1), (1, 2), (2, 0)]).expect("complex");
        let cycle = vec![1.0, 1.0, 1.0];
        let solution = solve_h1(&complex, &cycle, 1e-6, 512).expect("solve");
        assert!(solution.residual_norm > 0.5, "{}", solution.residual_norm);
        let adjoint = complex.coboundary_t(&solution.obstruction).expect("adjoint");
        for value in adjoint {
            assert!(value.abs() < 1e-4, "the obstruction must be harmonic: {value}");
        }
    }
}
