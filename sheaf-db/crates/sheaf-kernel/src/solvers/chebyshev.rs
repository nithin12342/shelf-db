// crates/sheaf-kernel/src/solvers/chebyshev.rs — P0: Chebyshev Tikhonov filter
// Fixed-μ bypass for BJ-PCG: degree-K Chebyshev semi-iteration needs only
// matrix-free applies (no dots/reductions per step, no preconditioner).
// Bounds [μ, λmax+μ]; λmax via power iteration (+5% safety margin —
// overestimating keeps the interval covering the spectrum; underestimating
// can diverge). f32 throughout, matching the crate.
use super::super::operators::laplacian::SheafLaplacian;

/// Largest eigenvalue of L by power iteration (deterministic start).
pub fn lambda_max_power(lap: &SheafLaplacian, iters: usize) -> f32 {
    let n = lap.n * lap.d;
    let mut v = vec![0.0f32; n];
    for (i, s) in v.iter_mut().enumerate() {
        *s = (((i * 2654435761) % 1000) as f32) / 1000.0 + 0.5;
    }
    let mut w = vec![0.0f32; n];
    let mut lam = 0.0f32;
    for _ in 0..iters {
        let mut norm = 0.0f32;
        for &x in &v {
            norm += x * x;
        }
        norm = norm.sqrt().max(1e-30);
        for s in v.iter_mut() {
            *s /= norm;
        }
        lap.apply_into(&v, &mut w);
        lam = 0.0;
        for (a, b) in v.iter().zip(w.iter()) {
            lam += a * b;
        }
        std::mem::swap(&mut v, &mut w);
    }
    lam.max(1e-12)
}

/// Solve (L+μI)x = b by Chebyshev semi-iteration (Saad form).
/// Returns iterations used (= degree). Caller builds b = μ·x₀ as usual.
pub fn chebyshev_tikhonov(
    lap: &SheafLaplacian,
    b: &[f32],
    mu: f32,
    lmax: f32,
    degree: usize,
    x: &mut [f32],
) -> usize {
    assert!(mu > 0.0, "Chebyshev Tikhonov needs positive mu");
    let n = lap.n * lap.d;
    assert_eq!(b.len(), n);
    assert_eq!(x.len(), n);
    let lmin = mu;
    let lmax = (lmax * 1.05 + mu).max(lmin * 1.01);
    let dc = (lmax + lmin) / 2.0;
    let c = (lmax - lmin) / 2.0;
    let mut r = vec![0.0f32; n];
    let mut ap = vec![0.0f32; n];
    let mut x_prev = vec![0.0f32; n];
    // Three-term recurrence from T_{k+1} = 2tT_k − T_{k-1} with the
    // optimal shifted polynomials R_k(λ) = T_k((dc−λ)/c)/T_k(dc/c):
    //   ρ_0 = 1/σ, ρ_k = 1/(2σ − ρ_{k-1}), β_k = ρ_{k-1}ρ_k,
    //   α_0 = 1/dc, α_k = (1+β_k)/dc, σ = dc/c.
    // (An earlier revision used β_k = (c·α_{k-1}/2)², which equals ρ_{k-1}²
    // instead of ρ_{k-1}ρ_k — close but inexact; it stalled at rel 0.04.)
    let sigma = dc / c.max(1e-30);
    let mut rho_old = 1.0 / sigma;
    let alpha0 = 1.0 / dc;
    // k = 0: x_1 = x_0 + r_0/dc (x_prev = x_0 via blessed copy, no lint).
    lap.apply_into(x, &mut ap);
    for ((ri, bi), (ai, xi)) in r.iter_mut().zip(b.iter()).zip(ap.iter().zip(x.iter())) {
        *ri = bi - (ai + mu * xi);
    }
    x_prev.copy_from_slice(x);
    for (xi, ri) in x.iter_mut().zip(r.iter()) {
        *xi += alpha0 * ri;
    }
    // k ≥ 1: x_{k+1} = x_k + α_k·r_k + β_k·(x_k − x_{k-1}), single-pass shift.
    for _ in 1..degree {
        lap.apply_into(x, &mut ap);
        for ((ri, bi), (ai, xi)) in r.iter_mut().zip(b.iter()).zip(ap.iter().zip(x.iter())) {
            *ri = bi - (ai + mu * xi);
        }
        let rho_new = 1.0 / (2.0 * sigma - rho_old);
        let beta = rho_old * rho_new;
        let alpha = (1.0 + beta) / dc;
        for i in 0..n {
            let old = x[i];
            x[i] += alpha * r[i] + beta * (old - x_prev[i]);
            x_prev[i] = old;
        }
        rho_old = rho_new;
    }
    degree
}
