// crates/sheaf-engine/src/bin/p0_gates_bench.rs — P0 borrows gate
// Three review-driven additions, one binary, three gates:
// (1) Chebyshev Tikhonov filter vs BJ-PCG reference (agree < 1e-3, f32);
// (2) H¹-proxy inconsistency E/‖x‖² strictly drops after harmonic solve;
// (3) b₀ union-find == 3 on a two-component + isolated fixture.
// Fixture: path graph n=2000, d=2, identity morphs (SPD, deterministic).
use std::time::Instant;

use sheaf_kernel::components::betti_zero;
use sheaf_kernel::operators::coboundary::CoboundaryOp;
use sheaf_kernel::operators::energy::{dirichlet_energy, inconsistency};
use sheaf_kernel::operators::laplacian::SheafLaplacian;
use sheaf_kernel::solvers::cg::CgLimits;
use sheaf_kernel::solvers::chebyshev::{chebyshev_tikhonov, lambda_max_power};
use sheaf_kernel::solvers::tikhonov::regularized_solve_monitored;

const N: usize = 2000;
const D: usize = 2;
const MU: f32 = 0.1;
const DEGREE: usize = 40;

fn main() {
    // ---- Fixture: path-graph sheaf, identity restriction maps ----
    let mut ops = Vec::new();
    for i in 0..N - 1 {
        ops.push(CoboundaryOp::new(
            vec![i as u32, (i + 1) as u32],
            vec![1.0, -1.0],
            vec![1.0, 0.0, 0.0, 1.0],
            D,
        ));
    }
    let lap = SheafLaplacian::build(N, D, ops);
    // Smooth signal + deterministic noise (same seed convention as benches).
    let mut x0 = vec![0.0f32; N * D];
    for i in 0..N {
        for k in 0..D {
            let s = ((i * (k + 3) + 7) % 101) as f32 / 101.0 - 0.5;
            x0[i * D + k] = ((i as f32) / (N as f32) * std::f32::consts::TAU).sin() + 0.3 * s;
        }
    }
    let mut b = vec![0.0f32; N * D];
    for (dst, src) in b.iter_mut().zip(x0.iter()) {
        *dst = MU * src;
    }

    // ---- Gate 1: Chebyshev vs PCG reference ----
    let t0 = Instant::now();
    let mut x_pcg = vec![0.0f32; N * D];
    let res = regularized_solve_monitored(
        &lap, &x0, &mut x_pcg, MU, CgLimits { tol: 1e-6, max_iter: 500 }, None, 1,
    );
    let pcg_ms = t0.elapsed().as_secs_f64() * 1e3;
    let t0 = Instant::now();
    let lmax = lambda_max_power(&lap, 25);
    let mut x_cheb = vec![0.0f32; N * D];
    let used = chebyshev_tikhonov(&lap, &b, MU, lmax, DEGREE, &mut x_cheb);
    let cheb_ms = t0.elapsed().as_secs_f64() * 1e3;
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for (a, c) in x_pcg.iter().zip(x_cheb.iter()) {
        num += ((*a as f64) - (*c as f64)).powi(2);
        den += (*a as f64).powi(2);
    }
    let rel = (num / den).sqrt();
    println!("p0_cheb: pcg_iters={} pcg_ms={:.1} cheb_applies={} cheb_ms={:.1} lmax={:.3} rel={:.2e}",
        res.iters, pcg_ms, 25 + 1 + used, cheb_ms, lmax, rel);
    println!("gate_cheb_agree={}", if rel < 1e-3 { "PASS" } else { "FAIL" });

    // ---- Gate 2: inconsistency strictly drops ----
    let norm_sq: f64 = x0.iter().map(|v| (*v as f64).powi(2)).sum();
    let before = inconsistency(dirichlet_energy(&lap.ops, &x0), norm_sq);
    let norm_h: f64 = x_pcg.iter().map(|v| (*v as f64).powi(2)).sum();
    let after = inconsistency(dirichlet_energy(&lap.ops, &x_pcg), norm_h);
    println!("p0_incon: before={:.4e} after={:.4e}", before, after);
    println!("gate_incon_drop={}", if after < before { "PASS" } else { "FAIL" });

    // ---- Gate 3: b₀ == 3 (two components + one isolated cell) ----
    let faces = vec![vec![0u32, 1, 2], vec![3u32, 4]];
    let b0 = betti_zero(6, &faces);
    println!("p0_betti0: b0={} (expect 3)", b0);
    println!("gate_betti0={}", if b0 == 3 { "PASS" } else { "FAIL" });

    // ---- Gate 4: d=8 stalk dim (max supported) applies correctly ----
    // Identity morphs on a path: (Lx)_v = deg(v)·x_v − Σ_neigh x_u (per stalk
    // coordinate). Verified against hand neighbor-differences.
    let nd8 = 500usize;
    let mut ops8 = Vec::new();
    for i in 0..nd8 - 1 {
        let mut morph = vec![0.0f32; 64];
        for k in 0..8 {
            morph[k * 8 + k] = 1.0;
        }
        ops8.push(CoboundaryOp::new(vec![i as u32, (i + 1) as u32], vec![1.0, -1.0], morph, 8));
    }
    let lap8 = SheafLaplacian::build(nd8, 8, ops8);
    let x8: Vec<f32> = (0..nd8 * 8).map(|i| (i % 37) as f32 * 0.25).collect();
    let mut y8 = vec![0.0f32; nd8 * 8];
    lap8.apply_into(&x8, &mut y8);
    let mut worst = 0.0f64;
    for v in 0..nd8 {
        let deg = (if v > 0 { 1 } else { 0 }) + (if v + 1 < nd8 { 1 } else { 0 });
        for k in 0..8 {
            let mut expect = deg as f32 * x8[v * 8 + k];
            if v > 0 {
                expect -= x8[(v - 1) * 8 + k];
            }
            if v + 1 < nd8 {
                expect -= x8[(v + 1) * 8 + k];
            }
            worst = worst.max((y8[v * 8 + k] - expect).abs() as f64);
        }
    }
    println!("p0_d8: worst_abs_err={:.2e} (expect < 1e-4)", worst);
    println!("gate_d8={}", if worst < 1e-4 { "PASS" } else { "FAIL" });

    // ---- Gate 5: giant hyperedge (2000 faces, one op) ----
    // Constant field + zero-sum signs ⇒ δ = 0 ⇒ Lx = 0 exactly. Exercises
    // the hoisted transpose path at scale (1 transpose, not 2000).
    let ng = 2000usize;
    let faces_g: Vec<u32> = (0..ng as u32).collect();
    let signs_g: Vec<f32> = (0..ng).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
    let lap_g = SheafLaplacian::build(
        ng, 2, vec![CoboundaryOp::new(faces_g, signs_g, vec![1.0, 0.0, 0.0, 1.0], 2)]);
    let x_g = vec![3.5f32; ng * 2];
    let mut y_g = vec![0.0f32; ng * 2];
    let t0 = Instant::now();
    lap_g.apply_into(&x_g, &mut y_g);
    let giant_ms = t0.elapsed().as_secs_f64() * 1e3;
    let max_out: f32 = y_g.iter().map(|v| v.abs()).fold(0.0, f32::max);
    println!("p0_giant: faces={} apply_ms={:.2} max_out={:.2e} (expect 0)", ng, giant_ms, max_out);
    println!("gate_giant={}", if max_out < 1e-4 { "PASS" } else { "FAIL" });

    // ---- Gate 6: ill-conditioned μ=1e-4 (κ ≈ 4e4) still converges ----
    let mu_ill = 1e-4f32;
    let mut x_ill = vec![0.0f32; N * D];
    let res_ill = regularized_solve_monitored(
        &lap, &x0, &mut x_ill, mu_ill, CgLimits { tol: 1e-6, max_iter: 3000 }, None, 1,
    );
    println!("p0_ill: mu={} iters={} rel={:.2e} (bound ~200+ iters at κ≈4e4)",
        mu_ill, res_ill.iters, res_ill.rel_residual);
    println!("gate_ill={}", if res_ill.rel_residual < 1e-3 { "PASS" } else { "FAIL" });

    if rel >= 1e-3
        || after >= before
        || b0 != 3
        || worst >= 1e-4
        || max_out >= 1e-4
        || res_ill.rel_residual >= 1e-3
    {
        std::process::exit(1);
    }
    println!("P0-GATES: ALL PASS");
}
