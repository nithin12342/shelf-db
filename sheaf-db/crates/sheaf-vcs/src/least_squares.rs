// least_squares.rs - least-squares solve on a graph Laplacian (NF-04).
//
// For a 1-complex, the coboundary delta maps a 0-cochain x to edge values
// delta(x)_e = x_v - x_u. Given a 1-cochain b, the global representative is
// the x that minimizes ||delta(x) - b||. It satisfies the normal equations
// delta^T delta x = delta^T b. The operator has a constant kernel, so the
// iterate is mean-centred after every step. The solve is conjugate gradient,
// implemented here in pure Rust.

use std::fmt;

/// An edge from u to v. The coboundary is x_v - x_u.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Edge {
    pub u: usize,
    pub v: usize,
}

/// A one-dimensional cell complex.
#[derive(Clone, Debug)]
pub struct Complex1 {
    pub vertices: usize,
    pub edges: Vec<Edge>,
}

/// Why the solve could not run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SolveError {
    /// An edge endpoint is outside the vertex set.
    BadEdge,
    /// The tolerance is not positive and finite.
    BadTolerance,
    /// The iteration limit is zero.
    BadIterationLimit,
    /// The input cochain has the wrong width.
    BadCochain,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::BadEdge => f.write_str("an edge endpoint is outside the complex"),
            SolveError::BadTolerance => f.write_str("the tolerance must be positive and finite"),
            SolveError::BadIterationLimit => f.write_str("the iteration limit must be positive"),
            SolveError::BadCochain => f.write_str("the cochain width does not match the complex"),
        }
    }
}

impl std::error::Error for SolveError {}

impl Complex1 {
    pub fn new(vertices: usize, edges: Vec<(usize, usize)>) -> Result<Self, SolveError> {
        let edges: Vec<Edge> = edges.into_iter().map(|(u, v)| Edge { u, v }).collect();
        if edges.iter().any(|e| e.u >= vertices || e.v >= vertices) {
            return Err(SolveError::BadEdge);
        }
        Ok(Self { vertices, edges })
    }

    pub fn coboundary(&self, x: &[f32]) -> Result<Vec<f32>, SolveError> {
        if x.len() != self.vertices {
            return Err(SolveError::BadCochain);
        }
        Ok(self.edges.iter().map(|e| x[e.v] - x[e.u]).collect())
    }

    /// The adjoint delta^T: a 1-cochain becomes a 0-cochain.
    pub fn coboundary_t(&self, b: &[f32]) -> Result<Vec<f32>, SolveError> {
        if b.len() != self.edges.len() {
            return Err(SolveError::BadCochain);
        }
        let mut out = vec![0.0f32; self.vertices];
        for (edge, value) in self.edges.iter().zip(b) {
            out[edge.u] -= value;
            out[edge.v] += value;
        }
        Ok(out)
    }

    /// Apply delta^T delta without forming the matrix.
    pub fn laplacian_apply(&self, x: &[f32]) -> Result<Vec<f32>, SolveError> {
        let edge_values = self.coboundary(x)?;
        self.coboundary_t(&edge_values)
    }

    /// Remove the constant kernel by subtracting the mean.
    pub fn center(x: &mut [f32]) {
        if x.is_empty() {
            return;
        }
        let mean = x.iter().sum::<f32>() / x.len() as f32;
        for value in x.iter_mut() {
            *value -= mean;
        }
    }
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

/// Solve delta^T delta x = delta^T b by conjugate gradient.
pub fn least_squares(
    complex: &Complex1,
    b: &[f32],
    tolerance: f32,
    max_iterations: usize,
) -> Result<Vec<f32>, SolveError> {
    if !tolerance.is_finite() || tolerance <= 0.0 {
        return Err(SolveError::BadTolerance);
    }
    if max_iterations == 0 {
        return Err(SolveError::BadIterationLimit);
    }
    let rhs = complex.coboundary_t(b)?;
    let mut x = vec![0.0f32; complex.vertices];
    Complex1::center(&mut x);
    let mut residual = rhs.clone();
    Complex1::center(&mut residual);
    let mut direction = residual.clone();
    let mut residual_norm = dot(&residual, &residual);
    for _ in 0..max_iterations {
        if residual_norm.sqrt() <= tolerance {
            break;
        }
        let applied = complex.laplacian_apply(&direction)?;
        let denominator = dot(&direction, &applied);
        if !denominator.is_finite() || denominator <= f32::EPSILON {
            break;
        }
        let alpha = residual_norm / denominator;
        for (value, step) in x.iter_mut().zip(&direction) {
            *value += alpha * step;
        }
        Complex1::center(&mut x);
        for (value, step) in residual.iter_mut().zip(&applied) {
            *value -= alpha * step;
        }
        Complex1::center(&mut residual);
        let next = dot(&residual, &residual);
        let beta = next / residual_norm;
        for (step, value) in direction.iter_mut().zip(&residual) {
            *step = *value + beta * *step;
        }
        residual_norm = next;
    }
    Ok(x)
}
