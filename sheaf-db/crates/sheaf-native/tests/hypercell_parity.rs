use sheaf_native::hypercell::{solve, solve_fallback, CsrCoboundary};

fn fixture(cols: usize, rows: usize) -> CsrCoboundary {
    let mut m = CsrCoboundary { cols, row_offsets: vec![0], columns: vec![], weights: vec![] };
    for i in 0..rows {
        for j in 0..cols {
            let value = (((i * 17 + j * 7 + 3) % 11) as f32 - 5.0) * 0.125;
            if value != 0.0 { m.columns.push(j); m.weights.push(value); }
        }
        m.row_offsets.push(m.columns.len());
    }
    m
}

// Independent dense Gaussian elimination provides an oracle for the shifted solve.
fn oracle(m: &CsrCoboundary, initial: &[f32], lambda: f64) -> Vec<f64> {
    let n = m.cols;
    let mut a = vec![vec![0.0; n + 1]; n];
    for i in 0..n { a[i][i] = lambda; a[i][n] = lambda * initial[i] as f64; }
    for range in m.row_offsets.windows(2) {
        for k in range[0]..range[1] {
            for l in range[0]..range[1] {
                a[m.columns[k]][m.columns[l]] += m.weights[k] as f64 * m.weights[l] as f64;
            }
        }
    }
    for i in 0..n {
        let pivot = (i..n).max_by(|&j, &k| a[j][i].abs().total_cmp(&a[k][i].abs())).unwrap();
        a.swap(i, pivot);
        let scale = a[i][i];
        for j in i..=n { a[i][j] /= scale; }
        for k in 0..n {
            if k == i { continue; }
            let scale = a[k][i];
            for j in i..=n { a[k][j] -= scale * a[i][j]; }
        }
    }
    a.iter().map(|r| r[n]).collect()
}

#[test]
fn arbitrary_rectangular_shapes_match_dense_oracle_and_fallback() {
    for (cols, rows) in [(2, 1), (6, 3), (8, 2), (15, 7), (3, 11), (17, 5)] {
        let m = fixture(cols, rows);
        let initial: Vec<_> = (0..cols).map(|i| i as f32 * 0.1 - 0.3).collect();
        let expected = oracle(&m, &initial, 0.25);
        let actual = solve(&m, &initial, 0.25, 256, 0.00001).unwrap();
        let fallback = solve_fallback(&m, &initial, 0.25, 256, 0.00001).unwrap();
        assert!(actual.converged && fallback.converged, "{cols}x{rows}");
        for ((a, f), e) in actual.values.iter().zip(&fallback.values).zip(expected) {
            assert!((*a as f64 - e).abs() < 0.00002);
            assert!((a - f).abs() < 0.000001);
        }
        assert!((actual.energy - fallback.energy).abs() < 0.000001);
        assert_eq!(actual.iterations, fallback.iterations);
    }
}

#[test]
fn shifted_solution_is_not_an_unshifted_solver() {
    let m = CsrCoboundary { cols: 2, row_offsets: vec![0, 2], columns: vec![0, 1], weights: vec![2.0, -3.0] };
    let result = solve(&m, &[1.0, 0.0], 1.0, 20, 0.000001).unwrap();
    assert!((result.values[0] - 5.0 / 7.0).abs() < 0.000001);
    assert!((result.values[1] - 3.0 / 7.0).abs() < 0.000001);
    assert!((result.energy - 1.0 / 49.0).abs() < 0.000001);
}

#[test]
fn zero_regularization_preserves_null_component() {
    let m = CsrCoboundary { cols: 2, row_offsets: vec![0, 2], columns: vec![0, 1], weights: vec![1.0, -1.0] };
    let result = solve(&m, &[1.0, 3.0], 0.0, 20, 0.000001).unwrap();
    assert_eq!(result.values, vec![2.0, 2.0]);
    assert!(result.converged);
    assert_eq!(result.energy, 0.0);
}

#[test]
fn empty_rows_and_duplicate_columns_are_valid() {
    let empty = CsrCoboundary { cols: 2, row_offsets: vec![0, 0], columns: vec![], weights: vec![] };
    let result = solve(&empty, &[1.0, 3.0], 1.0, 5, 0.000001).unwrap();
    assert_eq!(result.values, vec![1.0, 3.0]);
    assert_eq!(result.iterations, 0);
    let duplicate = CsrCoboundary { cols: 1, row_offsets: vec![0, 2], columns: vec![0, 0], weights: vec![1.0, 2.0] };
    let result = solve(&duplicate, &[1.0], 1.0, 5, 0.000001).unwrap();
    assert!((result.values[0] - 0.1).abs() < 0.000001);
}

#[test]
fn iteration_exhaustion_is_explicit() {
    let m = fixture(8, 5);
    let result = solve(&m, &[1.0; 8], 0.1, 1, 0.000001).unwrap();
    assert!(!result.converged);
    assert_eq!(result.iterations, 1);
    assert!(result.energy.is_finite());
}

#[test]
fn invalid_sparse_descriptors_and_numbers_return_errors() {
    let valid = fixture(3, 2);
    let mut bad = Vec::new();
    let mut m = valid.clone(); m.cols = usize::MAX; bad.push(m);
    let mut m = valid.clone(); m.row_offsets.clear(); bad.push(m);
    let mut m = valid.clone(); m.row_offsets[0] = 1; bad.push(m);
    let mut m = valid.clone(); m.row_offsets[1] = usize::MAX; bad.push(m);
    let mut m = valid.clone(); m.row_offsets = vec![0, 2, 1, m.columns.len()]; bad.push(m);
    let mut m = valid.clone(); m.columns[0] = m.cols; bad.push(m);
    let mut m = valid.clone(); m.weights[0] = f32::INFINITY; bad.push(m);
    let mut m = valid.clone(); m.weights.pop(); bad.push(m);
    for m in bad {
        assert!(solve(&m, &[0.0; 3], 1.0, 10, 0.000001).is_err());
        assert!(solve_fallback(&m, &[0.0; 3], 1.0, 10, 0.000001).is_err());
    }
    for (lambda, iterations, tol) in [(f32::NAN, 10, 0.1), (-1.0, 10, 0.1), (1.0, usize::MAX, 0.1), (1.0, 0, 0.1), (1.0, 10, 0.0)] {
        assert!(solve(&valid, &[0.0; 3], lambda, iterations, tol).is_err());
    }
    assert!(solve(&valid, &[0.0; 2], 1.0, 10, 0.1).is_err());
    assert!(solve(&valid, &[f32::NAN; 3], 1.0, 10, 0.1).is_err());
}
