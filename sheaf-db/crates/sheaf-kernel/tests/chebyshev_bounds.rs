// Timestamp: 2026-11-22_11-53-08 +05:30
// chebyshev_bounds.rs - check the Rust estimator against the Lean proofs (NF-02).
//
// The test calls the production constructor with 100 deterministic inputs.
// It asserts the statements proved in proofs/Chebyshev.lean: the endpoints are
// reconstructed, the radius is positive, the sigma ratio is above one, the
// interval contains sampled eigenvalues, and the residual bound decreases
// with the polynomial degree.

use sheaf_kernel::solvers::bounds::SpectralInterval;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[test]
fn one_hundred_intervals_satisfy_the_proven_invariants() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    for case in 0..100 {
        let mu = 0.01 + 4.0 * rng.unit();
        let lambda = 0.1 + 100.0 * rng.unit();
        let interval = SpectralInterval::of(mu, lambda).expect("interval");

        assert!((interval.center - interval.radius - interval.lower).abs() < 1e-10, "case {case}");
        assert!((interval.center + interval.radius - interval.upper).abs() < 1e-10, "case {case}");
        assert!(interval.lower < interval.center && interval.center < interval.upper, "case {case}");
        assert!(interval.radius > 0.0, "case {case}");
        assert!(interval.sigma > 1.0, "case {case}");

        // A sampled Rayleigh value inside the estimated spectrum is contained.
        let fraction = rng.unit();
        let eigenvalue = mu + fraction * (interval.upper - mu);
        assert!(interval.contains(eigenvalue), "case {case}");
        assert!(!interval.contains(mu - 1e-6), "case {case}");
        assert!(!interval.contains(interval.upper + 1e-6), "case {case}");
    }
}

#[test]
fn the_residual_bound_decreases_with_degree() {
    let interval = SpectralInterval::of(0.5, 12.0).expect("interval");
    let mut previous = interval.residual_bound(1).expect("bound");
    for degree in 2..=24 {
        let current = interval.residual_bound(degree).expect("bound");
        assert!(current < previous, "degree {degree}: {current} must be below {previous}");
        previous = current;
    }
    assert!(previous < 1e-3, "the residual bound must shrink with degree");
}

#[test]
fn the_chebyshev_values_are_positive_and_increasing() {
    let interval = SpectralInterval::of(0.25, 3.0).expect("interval");
    let mut previous = interval.chebyshev_t(0);
    for degree in 1..=24 {
        let current = interval.chebyshev_t(degree);
        assert!(current > 0.0, "degree {degree}");
        assert!(current > previous, "degree {degree}");
        previous = current;
    }
}
