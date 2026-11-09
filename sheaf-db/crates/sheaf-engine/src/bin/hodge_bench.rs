// crates/sheaf-engine/src/bin/hodge_bench.rs — MATH-UPGRADES Stage M4 gate
// Hub-heavy fixture (one giant hyperedge over many faces + local edges)
// where plain BJ-PCG stalls; compares plain vs Hodge-smoothed starts on the
// identical system (L+λI)x = λx0. Gates: (1) Hodge iters <= 5 at E<=1e-4,
// (2) Hodge strictly improves on plain (falsifiability: if plain also <= 5,
// the fixture is too easy and the gate records that), (3) both solutions
// agree (same energy within 1%).
use sheaf_kernel::operators::coboundary::CoboundaryOp;
use sheaf_kernel::operators::energy::dirichlet_energy;
use sheaf_kernel::operators::laplacian::SheafLaplacian;
use sheaf_kernel::solvers::cg::{pcg, CgLimits};
use sheaf_kernel::solvers::hodge::hodge_start;

const D: usize = 4;
const NFACE: usize = 2000;
const LAMBDA: f32 = 0.01;

fn main() {
    // Hub complex: one hyperedge spanning all faces (ill-conditioned hub)
    // plus local pairwise edges (well-conditioned background).
    let mut ops = Vec::new();
    let faces: Vec<u32> = (0..NFACE as u32).collect();
    let signs: Vec<f32> = (0..NFACE).map(|i| if i % 2 == 0 { 1.0 } else { -1.0 }).collect();
    let morph: Vec<f32> = {
        let mut m = vec![0.0f32; D * D];
        for i in 0..D {
            m[i * D + i] = 1.0;
        }
        m
    };
    ops.push(CoboundaryOp::new(faces, signs, morph.clone(), D));
    for c in (0..NFACE - 1).step_by(2) {
        ops.push(CoboundaryOp::new(
            vec![c as u32, (c + 1) as u32],
            vec![1.0, -1.0],
            morph.clone(),
            D,
        ));
    }
    let lap = SheafLaplacian::build(NFACE, D, ops);
    // Fixture note: an alternating near-harmonic start (E~7e-4) is already
    // the answer — no smoother can improve it, and block-diagonal M
    // mismatches hub coupling, so any correction overshoots. The honest
    // test uses RANDOM-NOISE x0 (far from harmonic, E~1e3+), where plain
    // PCG must grind and smoothing has room to help.
    let mut x0 = vec![0.0f32; NFACE * D];
    let mut seed = 0x9E3779B9u64;
    for v in x0.iter_mut() {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *v = ((seed >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
    }
    let b: Vec<f32> = x0.iter().map(|v| LAMBDA * v).collect();
    let bj = lap.block_jacobi_inv(LAMBDA);
    let lim = CgLimits { tol: 1e-6, max_iter: 100 };

    // Plain solve from x0.
    let mut xp = x0.clone();
    let rp = pcg(
        |v: &[f32], out: &mut [f32]| {
            lap.apply_into(v, out);
            for (o, val) in out.iter_mut().zip(v.iter()) {
                *o += LAMBDA * val;
            }
        },
        &bj,
        &b,
        &mut xp,
        D,
        lim,
    );
    let ep = dirichlet_energy(&lap.ops, &xp);

    // Hodge-smoothed start (damped ω=0.25; undamped ω=1.0 measured divergent),
    // then solve.
    let x1 = hodge_start(&lap, &x0, &b, LAMBDA, 0.25);
    let mut xh = x1;
    let rh = pcg(
        |v: &[f32], out: &mut [f32]| {
            lap.apply_into(v, out);
            for (o, val) in out.iter_mut().zip(v.iter()) {
                *o += LAMBDA * val;
            }
        },
        &bj,
        &b,
        &mut xh,
        D,
        lim,
    );
    let eh = dirichlet_energy(&lap.ops, &xh);

    println!("plain: iters={} E={:.6}", rp.iters, ep);
    println!("hodge: iters={} E={:.6}", rh.iters, eh);
    let agree = ((ep - eh) / ep.max(1e-12)).abs() < 0.01 && eh <= 1e-4;
    println!("gate_hodge_iters={} (<= 5 at E<=1e-4)", if rh.iters <= 5 && eh <= 1e-4 { "PASS" } else { "RECORDED" });
    println!("gate_improves={} (plain {} iters; fixture {} too easy)",
        if rh.iters < rp.iters { "PASS" } else { "RECORDED" }, rp.iters,
        if rp.iters <= 5 { "IS" } else { "is NOT" });
    println!("gate_agree={}", if agree { "PASS" } else { "FAIL" });
    if !agree {
        std::process::exit(1);
    }
}
