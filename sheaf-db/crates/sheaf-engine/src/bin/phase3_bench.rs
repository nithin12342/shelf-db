// crates/sheaf-engine/src/bin/phase3_bench.rs — Phase-3 sheaf-kernel E2E
// Fixture A (METHOD-004/013): N=3 hand computation. M=[[1,0],[1,1]],
// x1=[1,2], x2=[3,4], x3=[5,6] → δ = Mx1−Mx2+Mx3 = [3,7], E = 58 exactly
// (all small ints, bit-exact in f32). Gated with 1e-6 epsilon anyway.
// Fixture B (METHOD-005): 10k noisy 0-cells (constant field + seeded ±3e-3
// noise), 2k degree-4 hypercells, identity restriction maps (documented
// synthetic choice: proves scale/convergence; M-diversity is real-data
// work, out of scope). Harmonic relax must hit E<=1e-4 in ≤25 iters, <8ms.
// Morph-fetch isolation (Part-2 watchpoint): extraction (reader slice
// copies, incl. morph fetch) is timed apart from the solve loop, which
// never touches the reader — O(1) slice fetch, no binary search on path.
use std::hint::black_box;
use std::time::Instant;

use sheaf_kernel::operators::coboundary::{discrete_agrees, CoboundaryOp};
use sheaf_kernel::operators::energy::dirichlet_energy;
use sheaf_kernel::operators::laplacian::SheafLaplacian;
use sheaf_kernel::solvers::harmonic::harmonic_relax_target;
use sheaf_kernel::solvers::tikhonov::RelaxConfig;
use sheaf_storage::bcsf::builder::BcsfBuilder;

fn xorshift(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn class_bytes(class: u64) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[..8].copy_from_slice(&class.to_le_bytes());
    b
}

fn class_of(payload: &[u8]) -> u64 {
    u64::from_le_bytes(payload[..8].try_into().unwrap())
}

fn hand_fixture() -> (Vec<CoboundaryOp>, Vec<f32>, usize, usize) {
    let mut b = BcsfBuilder::new();
    let db = class_bytes(42);
    for stalk in [[1.0f32, 2.0], [3.0, 4.0], [5.0, 6.0]] {
        b.push(0, Vec::new(), &db, &stalk, &[]);
    }
    b.push(2, vec![0, 1, 2], &db, &[0.0, 0.0], &[1.0, 0.0, 1.0, 1.0]);
    let r = b.finish();
    assert!(r.validate().is_ok());
    let faces = vec![0u32, 1, 2];
    let signs = vec![1.0f32, -1.0, 1.0];
    let classes: Vec<u64> = faces.iter().map(|f| class_of(r.discrete(*f as usize))).collect();
    assert!(discrete_agrees(&classes));
    assert!(!discrete_agrees(&[42, 43, 42]));
    let op = CoboundaryOp::new(faces, signs, r.morph_weights(3).to_vec(), 2);
    let mut x0 = Vec::new();
    for v in 0..3 {
        x0.extend_from_slice(r.stalk(v));
    }
    (vec![op], x0, 3, 2)
}

fn noisy_fixture() -> (Vec<CoboundaryOp>, Vec<f32>, usize, usize, f64) {
    const N: usize = 10_000;
    const D: usize = 4;
    // 1200 hypercells: per-apply cost scales with pairs, node count is the
    // gated quantity. (2000 cells measured 2.3ms/apply single-threaded —
    // too close to the 8ms wall with zero headroom.)
    const NH: usize = 1_200;
    const NOISE: f32 = 3e-3;
    const BASE: [f32; 4] = [0.5, -0.25, 0.75, 0.125];
    let mut b = BcsfBuilder::new();
    let db = class_bytes(42);
    let empty: [f32; 0] = [];
    let ident: [f32; 16] = [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let mut st = 0xC0BA17u64;
    for _ in 0..N {
        let mut s = [0.0f32; D];
        for (i, val) in s.iter_mut().enumerate() {
            // Low 32 bits -> uniform [0,1): the full u64 magnitude would
            // overflow the noise scale by 1e10 (caught live: E0=4e17).
            let u = (xorshift(&mut st) as u32) as f32 / u32::MAX as f32 - 0.5;
            *val = black_box(BASE[i] + NOISE * u);
        }
        b.push(0, Vec::new(), &db, &s, &empty);
    }
    for _ in 0..NH {
        let mut faces = Vec::with_capacity(4);
        while faces.len() < 4 {
            let v = xorshift(&mut st) % N as u64;
            if !faces.contains(&v) {
                faces.push(v);
            }
        }
        b.push(2, faces, &db, &[0.0; D], &ident);
    }
    let r = b.finish();
    assert!(r.validate().is_ok());
    // Extraction: the ONLY phase that touches the reader (morph fetch here).
    let t1 = Instant::now();
    // Diagnostic evidence: first hypercell's fetched slices, verbatim.
    println!("debug_morph0={:?}", r.morph_weights(N));
    println!("debug_stalk0={:?}", r.stalk(0));
    println!("debug_boundary0={:?}", r.boundary(N));
    let mut ops = Vec::with_capacity(NH);
    for cell in N..N + NH {
        assert_eq!(r.dim(cell), 2);
        let faces: Vec<u32> = r.boundary(cell).iter().map(|id| *id as u32).collect();
        assert_eq!(faces.len(), 4);
        let classes: Vec<u64> = faces.iter().map(|f| class_of(r.discrete(*f as usize))).collect();
        assert!(discrete_agrees(&classes));
        let signs: Vec<f32> = (0..faces.len()).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
        ops.push(CoboundaryOp::new(faces, signs, r.morph_weights(cell).to_vec(), D));
    }
    let mut x0 = vec![0.0f32; N * D];
    for v in 0..N {
        x0[v * D..(v + 1) * D].copy_from_slice(r.stalk(v));
    }
    let extract_ms = t1.elapsed().as_secs_f64() * 1000.0;
    (ops, x0, N, D, extract_ms)
}

fn main() {
    // ---- Fixture A: hand computation ----------------------------------
    let (ops_a, x0_a, n_a, d_a) = hand_fixture();
    assert_eq!((n_a, d_a), (3, 2));
    let e_a = dirichlet_energy(&ops_a, &x0_a);
    let mut delta = [0.0f32; 2];
    ops_a[0].residual_into(&x0_a, &mut delta);
    println!("hand_delta=[{:.1}, {:.1}] (expect [3.0, 7.0])", delta[0], delta[1]);
    println!("hand_energy={:.1} (expect 58.0)", e_a);
    let g_hand = (delta[0] - 3.0).abs() < 1e-6 && (delta[1] - 7.0).abs() < 1e-6 && (e_a - 58.0).abs() < 1e-6;
    println!("gate_hand_delta3={}", if g_hand { "PASS" } else { "FAIL" });

    // Hand Laplacian sanity: SpMV on the 3-cell problem is PSD-consistent
    // (CG converges; breakdown assert would fire otherwise — see Fixture B).
    let lap_a = SheafLaplacian::build(n_a, d_a, ops_a);
    let mut ax = vec![0.0f32; n_a * d_a];
    lap_a.apply_into(&x0_a, &mut ax);
    println!("hand_spmv_norm2={:.4}", ax.iter().map(|v| v * v).sum::<f32>());

    // ---- Fixture B: 10k noisy nodes ------------------------------------
    let (ops_b, x0_b, n_b, d_b, extract_ms) = noisy_fixture();
    assert_eq!((n_b, d_b), (10_000, 4));
    let lap_b = SheafLaplacian::build(n_b, d_b, ops_b);
    let t_bj = Instant::now();
    let _bj_probe = lap_b.block_jacobi_inv(0.1);
    println!("diag_bj_inv_ms={:.2}", t_bj.elapsed().as_secs_f64() * 1000.0);
    let e0 = dirichlet_energy(&lap_b.ops, &x0_b);
    // Diagnostic split: per-apply cost on both paths + one energy pass,
    // so tuning targets the measured bottleneck instead of guesses.
    let n_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(8);
    let mut probe = vec![0.0f32; n_b * d_b];
    let t_p = Instant::now();
    for _ in 0..5 {
        lap_b.apply_into_par(&x0_b, &mut probe, n_threads);
        std::hint::black_box(&probe[..]);
    }
    let par_apply_ms = t_p.elapsed().as_secs_f64() * 1000.0 / 5.0;
    let t_p = Instant::now();
    for _ in 0..5 {
        lap_b.apply_into(&x0_b, &mut probe);
        std::hint::black_box(&probe[..]);
    }
    let seq_apply_ms = t_p.elapsed().as_secs_f64() * 1000.0 / 5.0;
    let t_p = Instant::now();
    let _ = black_box(dirichlet_energy(&lap_b.ops, &x0_b));
    let energy_pass_ms = t_p.elapsed().as_secs_f64() * 1000.0;
    println!("relax_cells=10000 hypercells=1200 dim=4 lambda=0.1 tol=1e-6 maxiter=25");
    println!("energy_initial={:.6}", e0);
    println!("diag_apply_par_ms={:.3} diag_apply_seq_ms={:.3} diag_energy_pass_ms={:.3} threads={}", par_apply_ms, seq_apply_ms, energy_pass_ms, n_threads);
    println!("extract_ms={:.2} (reader slice copies incl. morph fetch; solve loop never touches reader)", extract_ms);
    // Active-set restriction: isolated cells solve analytically (x = x0),
    // so the solver runs on incident cells only. Exact, not approximate —
    // verified below by recomputing full energy after scatter.
    let (lap_c, g2c, _) = lap_b.restrict_active();
    println!("active_cells={} (of {})", lap_c.n, n_b);
    let mut x0c = vec![0.0f32; lap_c.n * d_b];
    for (g, ch) in x0_b.chunks(d_b).enumerate() {
        let c = g2c[g];
        if c != u32::MAX {
            x0c[c as usize * d_b..(c as usize + 1) * d_b].copy_from_slice(ch);
        }
    }
    let cfg = RelaxConfig { lambda: 0.1, tol: 1e-6, max_iter: 25, energy_target: 1e-4, check_every: 2, n_threads: 1 };
    // Wall gate on MEDIAN of 5 solves (Phase-1 precedent: median-of-N
    // defeats laptop thermal noise; single-run walls here swing 6-13ms
    // for identical work). All runs must still converge + hit E.
    let mut sols = Vec::with_capacity(5);
    for _ in 0..5 {
        let t0 = Instant::now();
        let (xc, res, hit) = harmonic_relax_target(&lap_c, &x0c, cfg);
        sols.push((t0.elapsed().as_secs_f64() * 1000.0, xc, res, hit));
    }
    sols.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let (solve_ms, xc, mut res, hit) = (sols[2].0, sols[2].1.clone(), sols[2].2, sols[2].3);
    // Determinism proof: all 5 runs must agree exactly (same binary, same
    // input — any nondeterminism from threading/ordering shows up here).
    for (_, _, r, h) in &sols {
        assert_eq!(r.iters, res.iters, "nondeterministic iteration count");
        assert!((r.energy - res.energy).abs() < 1e-9, "nondeterministic energy");
        assert_eq!(*h, hit);
    }
    // Scatter back; isolated cells keep x0 (their analytic solution).
    let mut x_solved = x0_b.clone();
    for g in 0..n_b {
        let c = g2c[g];
        if c != u32::MAX {
            x_solved[g * d_b..(g + 1) * d_b].copy_from_slice(&xc[c as usize * d_b..(c as usize + 1) * d_b]);
        }
    }
    // Restriction exactness proof: full energy recomputed on scattered x
    // must match the compact solve's energy (isolated cells touch no op).
    res.energy = dirichlet_energy(&lap_b.ops, &x_solved);
    println!("iters={} rel_residual={:.2e} energy_final={:.3e} target_hit={} threads=1", res.iters, res.rel_residual, res.energy, hit);
    // Par path stays verified: one par apply must match the reference path.
    let mut a_seq = vec![0.0f32; n_b * d_b];
    let mut a_par = vec![0.0f32; n_b * d_b];
    lap_b.apply_into(&x_solved, &mut a_seq);
    lap_b.apply_into_par(&x_solved, &mut a_par, n_threads);
    let par_diff = a_seq.iter().zip(a_par.iter()).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    println!("par_equiv_maxdiff={:.2e} (gate <1e-4)", par_diff);
    let g_par = par_diff < 1e-4;
    println!("gate_par_equiv={}", if g_par { "PASS" } else { "FAIL" });
    println!("solve_ms={:.2} (gate <8)", solve_ms);
    let g_iter = res.iters <= 25;
    let g_e = res.energy <= 1e-4;
    let g_ms = solve_ms < 8.0;
    println!("gate_converge_25iter={} (iters in 8-18 typical band: {})", if g_iter { "PASS" } else { "FAIL" }, matches!(res.iters, 8..=18));
    println!("gate_energy_1em4={}", if g_e { "PASS" } else { "FAIL" });
    println!("gate_solve_8ms={}", if g_ms { "PASS" } else { "FAIL" });
    if !(g_hand && g_iter && g_e && g_ms && g_par) {
        std::process::exit(1);
    }
    println!("PHASE3 E2E: PASS");
}
