//! Safe compressed sparse row (CSR) coboundary solve for variable stalk sizes.
//! Energy is ||delta x||^2, not the regularized objective. No dense matrix is built.

#[derive(Clone, Debug)]
pub struct CsrCoboundary {
    pub cols: usize,
    pub row_offsets: Vec<usize>,
    pub columns: Vec<usize>,
    pub weights: Vec<f32>,
}

#[derive(Clone, Debug)]
pub struct SolveResult {
    pub values: Vec<f32>,
    pub energy: f32,
    pub iterations: usize,
    pub converged: bool,
}

const MAX_COLS: usize = 262_144;
const MAX_ROWS: usize = 262_144;
const MAX_NNZ: usize = 1_048_576;
const MAX_WORK: usize = 100_000_000;

fn validate(m: &CsrCoboundary, initial: &[f32], lambda: f32, iters: usize, tol: f32)
    -> Result<(), String>
{
    if m.cols == 0 || m.cols > MAX_COLS || initial.len() != m.cols {
        return Err("invalid column count or initial vector length".into());
    }
    if m.row_offsets.is_empty() || m.row_offsets.len() - 1 > MAX_ROWS
        || m.columns.len() > MAX_NNZ || m.columns.len() != m.weights.len()
    {
        return Err("invalid CSR lengths or resource limit exceeded".into());
    }
    let nnz = m.columns.len();
    if m.row_offsets[0] != 0 || m.row_offsets.last() != Some(&nnz)
        || m.row_offsets.windows(2).any(|w| w[0] > w[1] || w[1] > nnz)
        || m.columns.iter().any(|&c| c >= m.cols)
    {
        return Err("invalid CSR offsets or column index".into());
    }
    if !lambda.is_finite() || lambda < 0.0 || !tol.is_finite() || tol <= 0.0
        || initial.iter().chain(&m.weights).any(|v| !v.is_finite())
    {
        return Err("solve requires finite values, nonnegative regularization, and positive tolerance".into());
    }
    let work = m.cols.checked_add(nnz).and_then(|n| n.checked_add(m.row_offsets.len()))
        .and_then(|n| n.checked_mul(iters));
    let bytes = m.cols.checked_mul(4).and_then(|n| n.checked_mul(size_of::<f64>()));
    if iters == 0 || iters > 4096 || work.is_none_or(|n| n > MAX_WORK)
        || bytes.is_none_or(|n| n > 64 * 1024 * 1024)
    {
        return Err("solve work or scratch limit exceeded".into());
    }
    Ok(())
}

fn zeros<T: Default + Clone>(n: usize) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    out.try_reserve_exact(n).map_err(|_| "cannot allocate solve buffers")?;
    out.resize(n, T::default());
    Ok(out)
}

/// Solve (delta^T delta + lambda I)x = lambda initial with conjugate gradient.
/// A zero lambda preserves the initial null-space component. Duplicate CSR
/// columns add together. Tolerance bounds ||residual|| / max(1, ||lambda initial||).
/// Exhaustion returns a finite result with converged=false, not a false success.
pub fn solve(matrix: &CsrCoboundary, initial: &[f32], regularization: f32,
    max_iterations: usize, tolerance: f32) -> Result<SolveResult, String>
{
    validate(matrix, initial, regularization, max_iterations, tolerance)?;
    #[cfg(sheaf_native_kernel)]
    {
        let mut values = zeros::<f32>(matrix.cols)?;
        let mut scratch = zeros::<f64>(matrix.cols * 4)?;
        let (mut energy, mut iterations, mut converged) = (0.0, 0, 0);
        // SAFETY: validation bounds all CSR indices, lengths, and arithmetic.
        // All pointers borrow live, disjoint buffers for the duration of this call.
        let status = unsafe {
            sheaf_hypercell_solve(matrix.cols, matrix.row_offsets.len() - 1,
                matrix.columns.len(), matrix.row_offsets.as_ptr(), matrix.columns.as_ptr(),
                matrix.weights.as_ptr(), initial.as_ptr(), regularization, max_iterations,
                tolerance, values.as_mut_ptr(), &mut energy, &mut iterations, &mut converged,
                scratch.as_mut_ptr(), scratch.len())
        };
        if status != 0 {
            return Err(format!("native hypercell solve failed with status {status}"));
        }
        Ok(SolveResult { values, energy, iterations, converged: converged != 0 })
    }
    #[cfg(not(sheaf_native_kernel))]
    solve_fallback(matrix, initial, regularization, max_iterations, tolerance)
}

#[cfg(sheaf_native_kernel)]
unsafe extern "C" {
    fn sheaf_hypercell_solve(cols: usize, rows: usize, nnz: usize,
        offsets: *const usize, columns: *const usize, weights: *const f32,
        initial: *const f32, lambda: f32, max_iters: usize, tolerance: f32,
        output: *mut f32, energy: *mut f32, iterations: *mut usize, converged: *mut i32,
        scratch: *mut f64, scratch_len: usize) -> i32;
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// Fuse delta and its adjoint row-by-row. Scratch depends only on columns.
fn apply(m: &CsrCoboundary, x: &[f64], lambda: f64, out: &mut [f64]) {
    for (y, x) in out.iter_mut().zip(x) { *y = lambda * x; }
    for range in m.row_offsets.windows(2) {
        let mut value = 0.0;
        for k in range[0]..range[1] { value += m.weights[k] as f64 * x[m.columns[k]]; }
        for k in range[0]..range[1] { out[m.columns[k]] += m.weights[k] as f64 * value; }
    }
}

/// Exact CSR fallback. It uses the same shifted operator and stopping rule as C++.
pub fn solve_fallback(m: &CsrCoboundary, initial: &[f32], regularization: f32,
    max_iterations: usize, tolerance: f32) -> Result<SolveResult, String>
{
    validate(m, initial, regularization, max_iterations, tolerance)?;
    let mut scratch = zeros::<f64>(m.cols * 4)?;
    let (x, rest) = scratch.split_at_mut(m.cols);
    let (r, rest) = rest.split_at_mut(m.cols);
    let (p, ap) = rest.split_at_mut(m.cols);
    for (x, &v) in x.iter_mut().zip(initial) { *x = v as f64; }
    let lambda = regularization as f64;
    let rhs_norm = initial.iter().map(|&v| (lambda * v as f64).powi(2)).sum::<f64>().sqrt();
    let target = tolerance as f64 * rhs_norm.max(1.0);
    apply(m, x, lambda, ap);
    for i in 0..m.cols { r[i] = lambda * initial[i] as f64 - ap[i]; }
    p.copy_from_slice(r);
    let mut rr = dot(r, r);
    let mut iterations = 0;
    while rr.sqrt() > target && iterations < max_iterations {
        apply(m, p, lambda, ap);
        let pap = dot(p, ap);
        if !pap.is_finite() || pap <= 0.0 { break; }
        let alpha = rr / pap;
        for i in 0..m.cols { x[i] += alpha * p[i]; r[i] -= alpha * ap[i]; }
        iterations += 1;
        let mut next = dot(r, r);
        if !next.is_finite() { return Err("nonfinite conjugate gradient residual".into()); }
        if next.sqrt() <= target {
            // Restart if the recursive residual understates the true residual.
            apply(m, x, lambda, ap);
            for i in 0..m.cols { r[i] = lambda * initial[i] as f64 - ap[i]; }
            next = dot(r, r);
            p.copy_from_slice(r);
        } else {
            let beta = next / rr;
            for i in 0..m.cols { p[i] = r[i] + beta * p[i]; }
        }
        rr = next;
    }
    let mut values = zeros::<f32>(m.cols)?;
    for i in 0..m.cols {
        values[i] = x[i] as f32;
        if !values[i].is_finite() { return Err("solution exceeds finite Float32 range".into()); }
        x[i] = values[i] as f64;
    }
    // Report energy and convergence for the returned Float32 values.
    apply(m, x, lambda, ap);
    for i in 0..m.cols { r[i] = lambda * initial[i] as f64 - ap[i]; }
    let converged = dot(r, r).sqrt() <= target;
    let mut energy = 0.0;
    for range in m.row_offsets.windows(2) {
        let mut value = 0.0;
        for k in range[0]..range[1] { value += m.weights[k] as f64 * x[m.columns[k]]; }
        energy += value * value;
    }
    let energy = energy as f32;
    if !energy.is_finite() { return Err("energy exceeds finite Float32 range".into()); }
    Ok(SolveResult { values, energy, iterations, converged })
}
