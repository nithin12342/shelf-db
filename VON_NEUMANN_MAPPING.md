# Math → von Neumann machine: how sheaf theory becomes a program

> **Companion document:** `VON_NEUMANN_LIMITS.md` — the measured physical limits of
> this machine (memory-hierarchy ladder, storage floors, machine balance) and the
> comparison against the numbers derived from published physical constants.
> This document maps *math → machine parts*; that one measures *machine → limits*.

| Von Neumann part | Role | Sheaf-DB use |
|---|---|---|
| Memory (addressed words) | Holds data + instructions | BCSF slabs (`stalks`, `discrete`, `dims`, offsets) |
| CPU (fetch–decode–execute) | Runs one instruction stream | Scan loops, matvec kernels, PCG iteration |
| ALU + FPU | Arithmetic | FMA matvec, dot, saxpy, CRC |
| Control unit + branches | if/loops | Validity checks, convergence tests, cursors |
| Registers | Fastest storage (B) | Stack `[f32;8]` temporaries (`proj`, `delta`, `back`) |
| Cache L1/L2/L3 | KB–MB staging | 65,536-elem morsels; 64 MB L3 domains |
| RAM | GB working set | Full slabs (14–200 MB resident) |
| Disk | Persistent store | WAL frames, fiber snapshot, `.tbl` corpus |

## Translation table: math object → bytes → instructions

| Math | Bytes in memory | Instruction pattern | Machine unit |
|---|---|---|---|
| Stalk F(σ), dim d | d×f32 contiguous (`stalks` slab) | Sequential load | FPU stream, 1 cache line / 4 cells at d=2 |
| Restriction map F | Dense d×d f32, row-major (`morph`) | d² FMA per apply | FPU; O(d) if diagonal (borrowed trick) |
| Cochain x ∈ C⁰ | Flat `Vec<f32>` length n·d | Indexed load/store | RAM stream |
| Coboundary δx | Gather faces → matvec → signed add | `for face: acc += s·M·x[v]` | FPU + integer sign |
| Adjoint δ* | Same blocks, transposed access | `out += s·Mᵀ·δ` | FPU (no extra storage) |
| L = δ*δ (factored) | NEVER stored — two passes over δ blocks | apply δ, then δ* | Saves O((Vd)²) memory + O(Ed³) build writes |
| Lx diagonal (M⁻¹) | Per-cell d×d Cholesky factors | d triangular solves | FPU, O(d³) once per solve |
| Harmonic x (H⁰) | PCG loop state (r, z, p, Ap vectors) | `while ‖r‖/‖b‖ > tol: update` | Branch-on-residual control loop |
| Tikhonov (L+μI)x=μb | Same loop, axpy adds μx | `v → Lx + μx` closure | One extra saxpy per apply |
| δ²=0 certificate | Signs ±1 over sorted boundary | Integer sum, compare 0 | Branch, zero FPU |
| Energy E=Σ‖δ‖² | Reduction accumulator f64 | `acc += dot(δ,δ)` | FPU + single reduction |
| Cohomology class | Not stored (missing capability) | — | Would need rank/nullspace pass |

## Worked example: one factored L-apply as machine flow

```
for op in ops:                        # stream: ops contiguous in memory
  for v in op.faces:                   # gather: x[v*d .. v*d+d] into regs
    proj  = M * x[v]                   # d² FMA, M streamed once
    delta += sign * proj               # d FMA into stack [f32;8] (registers)
  for v in op.faces:                   # second pass, same M (cache-hot)
    back  = Mᵀ * delta                 # d² FMA
    out[v] += sign * back              # scatter-add to RAM
```

- Memory touches per op: 2·|faces| blocks in, 1 out — matches Ω(nnz) bound
- Dense-L alternative would need: O((Vd)²) storage + random access per row — cache-hostile
- Zero heap allocation in the hot loop (stack temporaries only)

## Hierarchy mapping (where each object lives)

```
registers   : proj/delta/back [f32;8], loop cursors, accumulators
L1/L2       : 65,536-elem morsels (mt_scan), one op's faces + M block
L3          : 64 MB contiguous domains + ghost replicas (partition.rs)
RAM         : stalks / discrete / dims / offsets slabs, PCG vectors
disk        : WAL frames (crash log), fiber snapshot (persisted pre-fold)
```

- Rule: hot data streams once per pass; nothing is chased through pointers
- SoA (one buffer per attribute) beats AoS (interleaved) — unit stride, no gather

## Control flow as program (PCG solve)

```
x = x0; r = b - (L+μI)x; z = M⁻¹r; p = z     # setup: 1 apply + 1 precon
while ‖r‖/‖b‖ > tol and it < max:            # branch: convergence test
  Ap = (L+μI)p                                # factored apply (see above)
  α = ⟨r,z⟩/⟨p,Ap⟩; x += αp; r -= αAp         # dots + saxpys
  z = M⁻¹r; β = ⟨r,z⟩/⟨r_old,z_old⟩; p = z+βp  # precondition + update
recheck energy E(x) ≤ target                  # certificate branch
```

- Each iteration: fixed instruction mix, no data-dependent branches in FPU path
- Branches isolated to: convergence test, validity bits, torn-tail CRC (recovery path)

## Word-size choice (f32)

| Option | Traffic | Precision | Verdict |
|---|---|---|---|
| f64 (theory default) | 2x bytes, 2x floor | exact | Unneeded — gates pass at 1e-9..1e-12 |
| f32 (implemented) | Halves every stream | sufficient | Correct von Neumann trade: memory wall binds, ALU doesn't |
| f16 / quantized | Halves again | gate-risky | Revisit only on a failed exactness gate |

## What von Neumann does poorly here (honest limits)

| Weakness | Effect | Mitigation in use / planned |
|---|---|---|
| Sequential bottleneck | One instruction stream per core | Threads + morsel partitions (P1: leaner reduce) |
| Memory wall | Streams cap at ~8 GB/s here | Fewer bytes: fused columns, factored form, f32 |
| Branch cost | Validity/cursor branches stall pipe | Branch-free FPU path; validity hoisted (compact_valid) |
| No bulk parallelism | 6M rows take ms, not µs | GPU/SIMD batching (missing — neural-sheaf-diffusion route) |
| Pointer chasing | Kills prefetch | Flat slabs + sorted cursors, zero linked structures |

## Bottom line

```
Math object   → contiguous words (slabs, SoA, aligned)
Linear map    → streaming FMA loop (never materialized)
Adjoint       → same bytes, transposed access (free)
Solve         → fixed-mix loop + residual branch (PCG)
Certificate   → integer/branch check (δ²=0) or reduction (energy)
Persistence   → sequential log + snapshot files (WAL, fibers)
Efficiency    → fewer bytes moved, not more FLOPs (memory wall binds)
```

**That last line is now a measured number, not a slogan.** The measured roofline ridge
point on this host is **2.92 FLOP/byte**; this project's d×d matvec runs at
**0.33-0.47 FLOP/byte** — 6-9× below the ridge, i.e. structurally memory-bound. See
`VON_NEUMANN_LIMITS.md` §2.4 for the derivation and the same-window measurement.
