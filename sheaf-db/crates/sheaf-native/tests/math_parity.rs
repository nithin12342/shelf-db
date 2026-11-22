// Timestamp: 2026-11-22_11-53-08 +05:30
// math_parity.rs - C++23 math kernel parity gate (C-02).
//
// The test drives the production math port with real input. It compares the
// native result with an independent Rust computation. The fallback result must
// name "rust-fallback" in its source field. No result is hard-coded.

use sheaf_native::kernel::ComplexSpec;
use sheaf_native::math::{chebyshev_t, h1_solve, spectral_interval};
use sheaf_native::native_kernel_linked;

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

fn reference_interval(mu: f64, lambda: f64) -> (f64, f64, f64, f64, f64) {
    let upper = 1.05 * lambda + mu;
    let lower = mu;
    let center = (upper + lower) * 0.5;
    let radius = (upper - lower) * 0.5;
    (lower, upper, center, radius, center / radius)
}

fn reference_t(sigma: f64, degree: u32) -> f64 {
    let mut previous = 1.0;
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

fn line_complex() -> (ComplexSpec<'static>, Vec<f32>) {
    let src: &'static [u32] = &[0, 1];
    let dst: &'static [u32] = &[1, 2];
    let complex = ComplexSpec { num_vertices: 3, stalk_dim: 1, edge_src: src, edge_dst: dst, r_src: None, r_dst: None };
    let b = vec![-1.5f32, 2.25];
    (complex, b)
}

#[test]
fn one_hundred_spectral_intervals_match_the_reference() {
    assert!(native_kernel_linked(), "the C++23 kernel must be linked on this host");
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let mut native_count = 0usize;
    for _ in 0..100 {
        let mu = 0.01 + 4.0 * rng.unit();
        let lambda = 0.1 + 100.0 * rng.unit();
        let got = spectral_interval(mu, lambda).expect("interval");
        let expected = reference_interval(mu, lambda);
        assert!((got.lower - expected.0).abs() < 1e-10);
        assert!((got.upper - expected.1).abs() < 1e-10);
        assert!((got.center - expected.2).abs() < 1e-10);
        assert!((got.radius - expected.3).abs() < 1e-10);
        assert!((got.sigma - expected.4).abs() < 1e-10);
        if got.source == "cpp23" {
            native_count += 1;
        }
    }
    if native_kernel_linked() {
        assert_eq!(native_count, 100);
    } else {
        assert_eq!(native_count, 0);
    }
    assert!(spectral_interval(0.0, 1.0).is_err());
}

#[test]
fn chebyshev_values_match_the_reference() {
    let sigma = 1.75;
    for degree in 0..32 {
        let got = chebyshev_t(sigma, degree);
        let expected = reference_t(sigma, degree);
        assert!((got - expected).abs() < 1e-8, "degree {degree}");
    }
}

#[test]
fn an_exact_cochain_has_zero_harmonic_class() {
    let (complex, b) = line_complex();
    let solution = h1_solve(&complex, &b, 1e-6, 512).expect("h1 solve");
    assert!(solution.residual_norm < 1e-3, "{}", solution.residual_norm);
    assert_eq!(solution.representative.len(), 3);
    assert_eq!(solution.obstruction.len(), 2);
    if native_kernel_linked() {
        assert_eq!(solution.source, "cpp23");
    } else {
        assert_eq!(solution.source, "rust-fallback");
    }
    let reconstructed: Vec<f32> = solution
        .representative
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    for (got, want) in reconstructed.iter().zip(&b) {
        assert!((got - want).abs() < 1e-3, "got {got}, want {want}");
    }
}

#[test]
fn a_cycle_carries_a_nonzero_harmonic_class() {
    let src: &'static [u32] = &[0, 1, 2];
    let dst: &'static [u32] = &[1, 2, 0];
    let complex = ComplexSpec { num_vertices: 3, stalk_dim: 1, edge_src: src, edge_dst: dst, r_src: None, r_dst: None };
    let b = vec![1.0f32, 1.0, 1.0];
    let solution = h1_solve(&complex, &b, 1e-6, 512).expect("h1 solve");
    assert!(solution.residual_norm > 1.0, "{}", solution.residual_norm);
    let mut adjoint = vec![0.0f32; 3];
    for (index, (&source, &target)) in src.iter().zip(dst.iter()).enumerate() {
        adjoint[source as usize] -= solution.obstruction[index];
        adjoint[target as usize] += solution.obstruction[index];
    }
    for value in adjoint {
        assert!(value.abs() < 1e-4, "delta^T h must vanish: {value}");
    }
}
