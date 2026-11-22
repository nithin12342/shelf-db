// Timestamp: 2026-11-22_11-53-08 +05:30
// math.rs - Rust port for the C++23 math kernels (C-02).
//
// The C++ tier owns the arithmetic. This module builds the C descriptor from
// validated Rust slices and reports which adapter produced each result. The
// portable fallback is explicit: its `source` field names "rust-fallback".
// No caller can mistake a fallback result for a native result.

use crate::kernel::{ComplexSpec, KernelError};

/// The interval proved in proofs/Chebyshev.lean.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectralInterval {
    pub lower: f64,
    pub upper: f64,
    pub center: f64,
    pub radius: f64,
    pub sigma: f64,
    pub source: &'static str,
}

/// The harmonic split of one 1-cochain.
#[derive(Clone, Debug, PartialEq)]
pub struct H1Solution {
    pub representative: Vec<f32>,
    pub obstruction: Vec<f32>,
    pub residual_norm: f32,
    pub source: &'static str,
}

/// Build the spectral interval. Invalid numeric inputs are explicit errors.
pub fn spectral_interval(mu: f64, lambda_max: f64) -> Result<SpectralInterval, KernelError> {
    if !mu.is_finite() || mu <= 0.0 || !lambda_max.is_finite() || lambda_max <= 0.0 {
        return Err(KernelError::InvalidArgument);
    }
    #[cfg(sheaf_native_kernel)]
    {
        let mut lower = 0.0f64;
        let mut upper = 0.0f64;
        let mut center = 0.0f64;
        let mut radius = 0.0f64;
        let mut sigma = 0.0f64;
        let code = unsafe {
            crate::abi::sheaf_spectral_interval(
                mu,
                lambda_max,
                &mut lower,
                &mut upper,
                &mut center,
                &mut radius,
                &mut sigma,
            )
        };
        if code != crate::abi::SHEAF_KERNEL_OK {
            return Err(KernelError::InvalidArgument);
        }
        Ok(SpectralInterval { lower, upper, center, radius, sigma, source: "cpp23" })
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        let lower = mu;
        let upper = 1.05 * lambda_max + mu;
        let center = (upper + lower) * 0.5;
        let radius = (upper - lower) * 0.5;
        Ok(SpectralInterval {
            lower,
            upper,
            center,
            radius,
            sigma: center / radius,
            source: "rust-fallback",
        })
    }
}

/// Evaluate the Chebyshev polynomial at sigma for the given degree.
pub fn chebyshev_t(sigma: f64, degree: u32) -> f64 {
    #[cfg(sheaf_native_kernel)]
    {
        unsafe { crate::abi::sheaf_chebyshev_t(sigma, degree) }
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        let mut previous = 1.0f64;
        let mut current = sigma;
        if degree == 0 {
            return previous;
        }
        if degree == 1 {
            return current;
        }
        for _ in 1..degree {
            let next = 2.0 * sigma * current - previous;
            previous = current;
            current = next;
        }
        current
    }
}

/// Split a 1-cochain into its exact part and its harmonic class.
pub fn h1_solve(
    complex: &ComplexSpec<'_>,
    b: &[f32],
    tolerance: f32,
    max_iterations: u32,
) -> Result<H1Solution, KernelError> {
    complex.validate()?;
    if b.len() != complex.edge_floats()
        || !tolerance.is_finite()
        || tolerance <= 0.0
        || max_iterations == 0
    {
        return Err(KernelError::InvalidArgument);
    }
    #[cfg(sheaf_native_kernel)]
    {
        let mut x = vec![0.0f32; complex.vertex_floats()];
        let mut obstruction = vec![0.0f32; b.len()];
        let mut residual = 0.0f32;
        let descriptor = crate::kernel::native::descriptor(complex);
        let code = unsafe {
            crate::abi::sheaf_h1_solve(
                &descriptor,
                b.as_ptr(),
                tolerance,
                max_iterations,
                x.as_mut_ptr(),
                obstruction.as_mut_ptr(),
                &mut residual,
            )
        };
        if code != crate::abi::SHEAF_KERNEL_OK {
            return Err(KernelError::InvalidArgument);
        }
        Ok(H1Solution { representative: x, obstruction, residual_norm: residual, source: "cpp23" })
    }
    #[cfg(not(sheaf_native_kernel))]
    {
        let solution = h1_reference(complex, b, tolerance, max_iterations)?;
        Ok(solution)
    }
}

#[cfg(not(sheaf_native_kernel))]
fn h1_reference(
    complex: &ComplexSpec<'_>,
    b: &[f32],
    tolerance: f32,
    max_iterations: u32,
) -> Result<H1Solution, KernelError> {
    let n = complex.vertex_floats();
    let d = complex.stalk_dim as usize;
    let mut rhs = vec![0.0f32; n];
    let mut x = vec![0.0f32; n];
    let mut residual = vec![0.0f32; n];
    let mut direction = vec![0.0f32; n];
    let mut applied = vec![0.0f32; n];
    let mut edges = vec![0.0f32; b.len()];
    adjoint(complex, b, &mut rhs);
    center(&mut rhs);
    residual.copy_from_slice(&rhs);
    direction.copy_from_slice(&rhs);
    let mut norm = dot(&residual, &residual);
    for _ in 0..max_iterations {
        if norm.sqrt() <= tolerance {
            break;
        }
        coboundary(complex, &direction, &mut edges);
        adjoint(complex, &edges, &mut applied);
        let denominator = dot(&direction, &applied);
        if !denominator.is_finite() || denominator <= f32::EPSILON {
            break;
        }
        let alpha = norm / denominator;
        for i in 0..n {
            x[i] += alpha * direction[i];
            residual[i] -= alpha * applied[i];
        }
        center(&mut x);
        center(&mut residual);
        let next = dot(&residual, &residual);
        let beta = next / norm;
        for i in 0..n {
            direction[i] = residual[i] + beta * direction[i];
        }
        norm = next;
    }
    coboundary(complex, &x, &mut edges);
    let obstruction: Vec<f32> = b.iter().zip(&edges).map(|(value, exact)| value - exact).collect();
    let residual_norm = obstruction.iter().map(|value| value * value).sum::<f32>().sqrt();
    let _ = d;
    Ok(H1Solution { representative: x, obstruction, residual_norm, source: "rust-fallback" })
}

#[cfg(not(sheaf_native_kernel))]
fn coboundary(complex: &ComplexSpec<'_>, x: &[f32], edge: &mut [f32]) {
    let d = complex.stalk_dim as usize;
    for (index, (&src, &dst)) in complex.edge_src.iter().zip(&complex.edge_dst).enumerate() {
        for k in 0..d {
            let left = x[src as usize * d + k];
            let right = x[dst as usize * d + k];
            let left = complex.r_src.map_or(left, |r| left * r[index * d + k]);
            let right = complex.r_dst.map_or(right, |r| right * r[index * d + k]);
            edge[index * d + k] = right - left;
        }
    }
}

#[cfg(not(sheaf_native_kernel))]
fn adjoint(complex: &ComplexSpec<'_>, edge: &[f32], out: &mut [f32]) {
    out.fill(0.0);
    let d = complex.stalk_dim as usize;
    for (index, (&src, &dst)) in complex.edge_src.iter().zip(&complex.edge_dst).enumerate() {
        for k in 0..d {
            let value = edge[index * d + k];
            let left = complex.r_src.map_or(value, |r| value * r[index * d + k]);
            let right = complex.r_dst.map_or(value, |r| value * r[index * d + k]);
            out[src as usize * d + k] -= left;
            out[dst as usize * d + k] += right;
        }
    }
}

#[cfg(not(sheaf_native_kernel))]
fn center(values: &mut [f32]) {
    if values.is_empty() {
        return;
    }
    let mean = values.iter().sum::<f32>() / values.len() as f32;
    for value in values.iter_mut() {
        *value -= mean;
    }
}

#[cfg(not(sheaf_native_kernel))]
fn dot(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}
