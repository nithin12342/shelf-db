# Sheaf theory vs this implementation — efficiency review

| # | Source | Used for |
|---|---|---|
| 1 | Hansen, Ghrist — spectral sheaf theory (jakobhansen.org) | Sheaf Laplacian, harmonic extension, sparsification |
| 2 | Hansen — gentle introduction / learning sheaves | Block form L, restriction-map cost |
| 3 | Hansen, Ghrist — approximate spectral sheaf (2019) | (1±ε) sparsification, O(ε⁻²n log n) edges |
| 4 | Robinson — SampTA 2013 / PySheaf | Sparse local coboundary blocks |
| 5 | Bodnar et al. — Neural Sheaf Diffusion (2022) | GPU sparse apply, diagonal restriction cuts O(d²)→O(d) |
| 6 | Oudot (MPRI) / Scythe discrete-Morse literature | H^k via Morse reduction |
| 7 | Nemirovsky–Yudin | First-order lower bound Ω(√κ log(1/ε)) |

## Theory points (brief)

- Cellular sheaf: stalks F(σ) + restriction maps, δᵏx = Σ[σ:τ]F(x), δ²=0, Hᵏ=ker δᵏ/im δᵏ⁻¹
- Sheaf Laplacian: Δ = δ*δ + δδ*, graph L = δ*δ, ker L = H⁰
- L-apply cost: O(m) = O(E·d²); explicit L build: O(E·d³); rule: keep factored, never form L dense
- Solvers: Euler O(m)/step; CG O(√κ·m); PCG fewer steps; dense Cholesky O((Vd)³)
- Tikhonov (L+μI)x = μx₀: μ>0 removes nullspace, cuts κ, same solvers run faster
- Hodge Hᵏ: SVD O(n³d³), Lanczos O(m)/matvec, or Morse reduction to critical cells
- Lower bounds: apply Ω(nnz); CG optimal among Krylov methods

## Implementation map (this repo)

| Component | File | Choice | Cost |
|---|---|---|---|
| L build | `sheaf-kernel/.../operators/laplacian.rs` | Dense d×d morphs, per-cell ΣMᵀM diag | O(Σ\|faces\|·d³) |
| L apply | same, `apply_into` | Matrix-free δᵀδ, zero-alloc, stack [f32;8] | O(Σ\|faces\|·d²) |
| L apply par | same, `apply_into_par` | Thread-local + full reduce per iter | +O(threads·n·d) mem/iter |
| Preconditioner | same, `block_jacobi_inv` | Per-cell inv(D+λI), Cholesky, rebuilt per solve | O(n·d³) per solve |
| Harmonic | `solvers/harmonic.rs` | Delegates to Tikhonov BJ-PCG + energy gate | Same as Tikhonov |
| Tikhonov | `solvers/tikhonov.rs` | (L+λI)x=λx₀ via BJ-PCG, tol/max_iter/energy_target | 1 apply + 1 M⁻¹ + dots/saxpys per iter |
| δ²=0 check | `sheaf-query/.../ir/validator.rs` | Signed boundary sum O(\|boundary\|), codim-1 scope | No D₂D₁ product |
| Hᵏ/Betti | `solvers/hodge.rs` + `api/result.rs` | None — one damped-Richardson probe (unwired) + energy certificate | — |
| Precision | everywhere | f32 (E2E gates: rel 1e-9..1e-12 achieved) | Half the memory of f64 |

## Verdicts — most efficient or not

| Component | Verdict | Reason |
|---|---|---|
| L apply (matrix-free δᵀδ) | **Optimal** | Matches Ω(nnz) bound; never forms L dense — exactly the theory prescription |
| Tikhonov formulation | **Optimal** | Standard (L+μI)x=μx₀; μ>0 cuts κ; nothing faster in this class |
| PCG + block-Jacobi | **Optimal method, improvable constants** | CG optimal among Krylov; block-Jacobi is the right cheap M⁻¹ for block structure |
| Harmonic via Tikhonov | **Optimal** | μ→0 limit is the textbook route; λ>0 cures the H⁰ zero-mode |
| δ²=0 check | **Near-optimal (scoped)** | O(\|boundary\|) vs O(n³) matrix product; valid only via codim-1 restriction (documented) |
| L build | **Improvable** | Recomputes MᵀM per face; dense d×d even for scalar/identity morphs |
| Per-face apply loop | **Improvable** | Recomputes Mᵀδ per face; hoist to 1× + sign-scale |
| Preconditioner rebuild | **Improvable** | Rebuilt every solve; cache across solves with same λ |
| Parallel apply | **Works, heavy** | Full thread-local + reduce per PCG iter; partition-by-cell or morsel-static is leaner |
| Hᵏ cohomology | **Missing capability** | No Betti/rank/Lanczos/Morse — certificate-only; theory offers three routes |
| GPU / sparsification | **Missing (out of scope)** | Neural-sheaf-diffusion GPU batching, Hansen–Ghrist sparsification not implemented |
| f32 precision | **Deliberate, validated** | Gates pass at 1e-9..1e-12; halves traffic; revisit only if a gate fails |

## Bottom line (part 1)

```
Solver architecture : theory-optimal (factored apply, PCG, Tikhonov form)
Constant factors    : 3 hoist/cache fixes open (build, apply loop, M⁻¹ cache)
Missing vs theory   : H^k computation, GPU, sparsification
DB benchmark wins   : from data structures (fibers, fused columns, BCSF),
                      NOT from beating solver bounds — correctly so,
                      since Ω(nnz) / Ω(√κ) say you cannot.
```

## Related theories — comparison and borrow list

| # | Theory | Standard cost | In our repo? | Borrow (actionable) |
|---|---|---|---|---|
| 1 | DEC / combinatorial Hodge Δₖ | O(nnz)/matvec, O(n³) eig | MISS (only δ/L/E) | Lumped diagonal Hodge-⋆: geometry at zero solver cost |
| 2 | Discrete Morse (Forman) | Linear coreduction; optimal matching NP-hard | MISS | Coreduction front-end: shrink cells before PCG, Betti on tiny Morse boundary |
| 3 | Connection Laplacian / VDM / sync | nd×nd, top-eig O(nnz·d) | MISS | Per-stalk QR re-orthonormalization when maps near-orthogonal → fewer PCG iters |
| 4 | GSP (Chebyshev filters) | O(K·m), no eigendecomposition | MISS (docs only) | Chebyshev Tikhonov filter K=20–50 bypasses PCG for fixed μ; f32/GPU-friendly |
| 5 | HodgeRank | LS O(m√κ) + curl O(\|T\|) | PARTIAL (smoother only, unwired) | Edge-residual + triangle-curl norm = H¹ signal with no Betti solver |
| 6 | Persistent homology | O(n³) worst; H₀ O(nα(n)) | MISS | Union-find H₀ shortcut + clear/twist ordering IF Betti is ever added |
| 7 | FEEC (mixed Hodge Laplacian) | Assembly + indefinite saddle solve | MISS | Stay lumped/identity mass; diagonal Hodge-⋆ only if geometry demands |
| 8 | Near-linear Laplacian solvers (ST/KOSZ) | Õ(m log n) for SDD only | MISS | Hierarchy idea only — sheaf blocks aren't SDD; block-Jacobi PCG stays |
| 9 | Tarski lattice Laplacian | Local gossip O(iters·m), no spectra | MISS (nominal) | Fallback consensus for quantized/non-linear stalks, reuse adjacency loops |
| 10 | Reff sparsification (Spielman–Srivastava) | O(n log n/ε²) edges | MISS | Scalar-projection (trace/norm) sampling for preconditioner only |

## Overlap status (grep-verified)

| Area | Status | Evidence |
|---|---|---|
| Consistency energy E=Σ‖δ‖² | HIT (measured, gated) | `operators/energy.rs:7`, phase3_adv:185, phase3_bench:127 |
| Hodge smoothing | PARTIAL (dead weight) | `solvers/hodge.rs`, verdict in MATH_UPGRADES_PLAN.md:41 |
| Curl / ranking metric | MISS | Comment only (`hodge.rs:8`); no metric, no ranking quality test |
| Chebyshev / spectral filter | MISS | Zero code hits |
| Morse / persistence / sync / sparsify / FEEC / gossip | MISS | Zero code hits (false hits documented: `Morsel`, `collapse`, `reduction`) |

## Bottom line (part 2 — borrowing priority)

```
P0 (drop-in, no solver change) : Chebyshev Tikhonov filter; H¹ curl residual;
                                 lumped diagonal Hodge-⋆
P1 (small build changes)       : Morse coreduction front-end; per-stalk QR;
                                 scalar-projection sparsifier for M⁻¹
P2 (new capability)            : Union-find H₀ + clear/twist IF Betti added;
                                 Tarski fallback for non-linear stalks
DO NOT borrow                  : Dense eig, cubic persistence reduction,
                                 ST/KOSZ core (non-SDD blocks), full FEEC saddle
```

## Implementation status (2026-10-15)

| Item | Status | Evidence |
|---|---|---|
| Hoist Mᵀδ, per-op MᵀM, diag fast path, M⁻¹ cache | DONE | `operators/laplacian.rs`, phase3+B7 green, iters=2 |
| Chebyshev Tikhonov filter | DONE | `solvers/chebyshev.rs`, rel 8.8e-6 vs PCG |
| H¹ curl/inconsistency proxy | DONE | `operators/energy.rs::inconsistency`, 20x drop gated |
| b₀ union-find | DONE | `components.rs`, ==3 gated |
| Lumped diagonal Hodge-⋆ | DONE (equivalent) | Mass already identity; diag-morph O(d) path added |
| Scalar sparsifier for M⁻¹ | FOLDED into M⁻¹ cache | M⁻¹ already block-diagonal; rebuild was the cost |
| Per-stalk QR | REJECTED | Replacing M by Q changes the sheaf — valid only for orthogonal-by-construction maps |
| Tarski fallback | REJECTED | No lattice/order-valued stalks in this codebase (f32 linear) |
| Morse coreduction | OPEN | Research-grade; unsafe to half-build (wrong collapse = silent wrong homology) |
