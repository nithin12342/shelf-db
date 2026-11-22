// bounds.rs - the Chebyshev spectral interval used by the kernel (NF-02).
//
// The interval is [mu, 1.05 * lambda_max + mu]. The Lean file
// proofs/Chebyshev.lean proves that its center and radius reconstruct the
// endpoints, that the radius is positive, that the shifted centre ratio is
// above one, and that the Chebyshev polynomials increase there. This module
// computes the same quantities and rejects any input for which the proof's
// preconditions do not hold.

use std::fmt;

/// The safety margin applied to the power-iteration estimate.
pub const SAFETY: f64 = 1.05;

/// Why the interval cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoundsError {
    /// The shift is not a positive finite number.
    InvalidShift,
    /// The eigenvalue estimate is not a positive finite number.
    InvalidLambda,
}

impl fmt::Display for BoundsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BoundsError::InvalidShift => f.write_str("the regularization shift must be positive and finite"),
            BoundsError::InvalidLambda => f.write_str("the eigenvalue estimate must be positive and finite"),
        }
    }
}

impl std::error::Error for BoundsError {}

/// The proven spectral interval and its derived iteration parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpectralInterval {
    pub lower: f64,
    pub upper: f64,
    pub center: f64,
    pub radius: f64,
    pub sigma: f64,
}

impl SpectralInterval {
    /// Build the interval. Every returned field satisfies a Lean theorem.
    pub fn of(mu: f64, lambda_max: f64) -> Result<Self, BoundsError> {
        if !mu.is_finite() || mu <= 0.0 {
            return Err(BoundsError::InvalidShift);
        }
        if !lambda_max.is_finite() || lambda_max <= 0.0 {
            return Err(BoundsError::InvalidLambda);
        }
        let lower = mu;
        let upper = SAFETY * lambda_max + mu;
        let center = (upper + lower) / 2.0;
        let radius = (upper - lower) / 2.0;
        let sigma = center / radius;
        Ok(Self { lower, upper, center, radius, sigma })
    }

    /// True when a value lies in the proven interval.
    pub fn contains(&self, value: f64) -> bool {
        value >= self.lower && value <= self.upper
    }

    /// The Chebyshev polynomial T_degree(sigma), by the Lean recurrence.
    pub fn chebyshev_t(&self, degree: usize) -> f64 {
        let x = self.sigma;
        let mut previous = 1.0f64;
        let mut current = x;
        match degree {
            0 => previous,
            1 => current,
            _ => {
                for _ in 1..degree {
                    let next = 2.0 * x * current - previous;
                    previous = current;
                    current = next;
                }
                current
            }
        }
    }

    /// The residual bound 1 / T_degree(sigma). It decreases with degree.
    pub fn residual_bound(&self, degree: usize) -> Result<f64, BoundsError> {
        let value = self.chebyshev_t(degree);
        if !value.is_finite() || value <= 0.0 {
            return Err(BoundsError::InvalidLambda);
        }
        Ok(1.0 / value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_inputs_are_rejected() {
        assert_eq!(SpectralInterval::of(0.0, 1.0).err(), Some(BoundsError::InvalidShift));
        assert_eq!(SpectralInterval::of(f64::NAN, 1.0).err(), Some(BoundsError::InvalidShift));
        assert_eq!(SpectralInterval::of(1.0, 0.0).err(), Some(BoundsError::InvalidLambda));
    }

    #[test]
    fn center_and_radius_reconstruct_the_endpoints() {
        let interval = SpectralInterval::of(0.25, 4.0).expect("interval");
        assert!((interval.center - interval.radius - interval.lower).abs() < 1e-12);
        assert!((interval.center + interval.radius - interval.upper).abs() < 1e-12);
        assert!(interval.sigma > 1.0);
    }
}
