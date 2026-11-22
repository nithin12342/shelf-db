// h1.rs - integration test for the H1 quotient solver (NF-04).
//
// Four complexes: a line, a triangle, two glued triangles, and a triangle with
// a nonzero cycle class. The expected result is stated for each case. An exact
// cochain must have zero obstruction; a cycle must have a nonzero class that
// is orthogonal to the image of the coboundary.

use sheaf_vcs::h1::{solve_h1, H1Solution};
use sheaf_vcs::least_squares::Complex1;

fn solve(vertices: usize, edges: Vec<(usize, usize)>, b: Vec<f32>) -> (Complex1, H1Solution) {
    let complex = Complex1::new(vertices, edges).expect("complex");
    let solution = solve_h1(&complex, &b, 1e-6, 1024).expect("solve");
    (complex, solution)
}

#[test]
fn a_line_represents_an_exact_cochain_with_zero_obstruction() {
    let complex = Complex1::new(3, vec![(0, 1), (1, 2)]).expect("complex");
    let expected = vec![-1.5, 2.25, 0.75];
    let b = complex.coboundary(&expected).expect("coboundary");
    let solution = solve_h1(&complex, &b, 1e-6, 1024).expect("solve");
    assert!(solution.residual_norm < 1e-4, "{}", solution.residual_norm);
    let mut centered = expected.clone();
    sheaf_vcs::least_squares::Complex1::center(&mut centered);
    for (got, want) in solution.representative.iter().zip(&centered) {
        assert!((got - want).abs() < 1e-3, "got {got}, want {want}");
    }
    let reconstructed = complex.coboundary(&solution.representative).expect("exact");
    for (got, want) in reconstructed.iter().zip(&b) {
        assert!((got - want).abs() < 1e-3);
    }
}

#[test]
fn a_triangle_with_zero_cycle_sum_is_exact() {
    let complex = Complex1::new(3, vec![(0, 1), (1, 2), (2, 0)]).expect("complex");
    let b = vec![2.0, -0.5, -1.5];
    assert!((b.iter().sum::<f32>()).abs() < 1e-6, "the cycle sum must be zero");
    let solution = solve_h1(&complex, &b, 1e-6, 1024).expect("solve");
    assert!(solution.residual_norm < 1e-3, "{}", solution.residual_norm);
}

#[test]
fn two_glued_triangles_represent_an_exact_cochain() {
    let edges = vec![(0, 1), (1, 2), (2, 0), (1, 3), (3, 2), (2, 1)];
    let complex = Complex1::new(4, edges).expect("complex");
    let x = vec![0.5, -1.0, 2.0, 0.25];
    let b = complex.coboundary(&x).expect("coboundary");
    let solution = solve_h1(&complex, &b, 1e-6, 1024).expect("solve");
    assert!(solution.residual_norm < 1e-3, "{}", solution.residual_norm);
    let mut centered = x.clone();
    sheaf_vcs::least_squares::Complex1::center(&mut centered);
    for (got, want) in solution.representative.iter().zip(&centered) {
        assert!((got - want).abs() < 1e-3, "got {got}, want {want}");
    }
}

#[test]
fn a_nonzero_cycle_sum_reports_a_harmonic_obstruction() {
    let (complex, solution) = solve(
        3,
        vec![(0, 1), (1, 2), (2, 0)],
        vec![1.0, 1.0, 1.0],
    );
    assert!(solution.residual_norm > 1.0, "{}", solution.residual_norm);
    for value in &solution.obstruction {
        assert!(value.abs() > 0.1, "the class must be nonzero on the cycle");
    }
    let adjoint = complex.coboundary_t(&solution.obstruction).expect("adjoint");
    for value in adjoint {
        assert!(value.abs() < 1e-4, "delta^T h must vanish: {value}");
    }
}

#[test]
fn bad_inputs_are_rejected() {
    let complex = Complex1::new(2, vec![(0, 1)]).expect("complex");
    assert!(solve_h1(&complex, &[1.0], 0.0, 100).is_err());
    assert!(solve_h1(&complex, &[1.0], 1e-6, 0).is_err());
    assert!(solve_h1(&complex, &[1.0, 2.0], 1e-6, 100).is_err());
    assert!(Complex1::new(2, vec![(0, 5)]).is_err());
}
