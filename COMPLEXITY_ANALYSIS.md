# Core architecture: complexity formulas vs measured wall — with adversarial analysis

| Symbol | Meaning | Value here |
|---|---|---|
| n | cells (0-cells = unknowns) | 6 (b₀) … 6,001,215 (scan) |
| m | Σ\|faces\| over hypercells (coboundary nnz) | 1,998 (p0) … 88,234 (FB) … 6M+ (TPC-H) |
| d | stalk dim (≤8) | 1 (KV) … 2 (scans/fibers) |
| κ | κ((L+μI)) = λmax/λmin | ~43 (p0 fixture, μ=0.1) |
| BW | measured single-thread read BW | 8.4 GB/s (`bottleneck.json`) |
| f | clock | ~4 GHz (i7-1165G7 turbo) |

## 1. Component formulas and predictions

| # | Component | Time formula | Space formula | Predicts (this hardware) |
|---|---|---|---|---|
| 1 | BCSF load (parse .tbl) | O(rows × fields), char-level | 33 B/row: 8 stalk + 16 discrete + 4 stalk_off + 1 dim + 4 offsets | 6M rows ≈ 198 MB resident |
| 2 | Scan SUM (stride-2) | O(n) + floor bytes/BW | O(1) extra | 48 MB / 8.4 = 5.7 ms floor |
| 3 | Fused scan (4 B/row) | O(n) + floor bytes/BW | O(1) extra (+24 MB slab) | 24 MB / 8.4 = 2.86 ms floor |
| 4 | L build | O(m·d³) | O(n·d²) diag + op blocks | p0: 1998·8 = 16k FMA ≈ 4 µs |
| 5 | L apply (factored) | O(m·d²) | O(n·d) vectors, zero temp alloc | p0: 1998·2·4 = 16k FMA ≈ 2 µs flops |
| 6 | PCG solve | O(√κ·m·d²·log(1/ε)) iters×apply | 4·n·d + n·d² (r,z,p,Ap + M⁻¹) | κ=43 → ~46 iters unprecond (tol 1e-6) |
| 7 | Chebyshev deg-K | K applies + 25 power | 3·n·d (x,x_prev,r,Ap... 4 bufs) | err ≈ 2ρ^K, ρ=0.7355 → K=40: ~3e-6 |
| 8 | Power λmax (25 it) | 25 applies | O(n·d) | Underconverges ~0.5% on path graphs (ratio 1−O(1/n²)) |
| 9 | Fiber prefold | O(n log n) 3 sorts | 3 sorted vecs | 6M+1.5M+150k ≈ 1.8–2.6 s |
| 10 | Fiber walk | O(fibers) streaming | O(1) cursors | 2.57M × ~6 cyc ≈ 4 ms |
| 11 | Range slice | 2·O(log V) + O(k) reads | O(k) ranges | 38 compares + 10k reads ≈ 20–50 µs |
| 12 | WAL durable op | O(payload) + 2 seeks + 2 flushes | 64 KB buffer | ≈ syscall floor × 4 ≈ 7 µs |
| 13 | Epoch durable op | O(payload) + 1 flush, 0 seeks | forward only | ≈ syscall floor ≈ 2 µs |
| 14 | b₀ union-find | O(n·α(n)) | O(n) parents | Instant at any tested n |
| 15 | Graph rollup | O(paths × len) | O(1) per path | 600k × ~6 ops ≈ 40–70 ms |
| 16 | Clean release build (fat LTO, 37 bins) | O(1) per bin + link | 0.05 GB artifacts | **109.3 s** (fresh CARGO_TARGET_DIR); warm 0.19 s; 1-bin touch 0.8 s |

## 2. Prediction vs measurement

| # | Predicted | Measured | Match? |
|---|---|---|---|
| 1 | 198 MB resident | 198.0 MB (`tpch_scan`); fraud 19.5 MB = 33 B/row ✓ | ✅ EXACT (byte accounting) |
| 2 | 5.7 ms floor | ~8 ms quiet (old loop) | ⚠️ +40% — accumulator latency chain (see §4) |
| 3 | 2.86 ms floor | 5.3 ms (LLVM ~2-way) → **1.7 ms** (4 explicit chains) | ✅ EXCEEDED — effective 14 GB/s reads (RAM read BW, not R+W copy floor) |
| 4 | ~4 µs flops | Not isolated (inside 1.8 s prefold-dominated load) | ➖ unmeasured at scale |
| 5 | ~2 µs flops/apply | ~54 µs/iter (2.0 ms / 37 iters) | ❌ 25x — overhead-dominated (see §4) |
| 6 | ~46 iters (unprecond bound) | 37 iters (BJ-PCG) | ✅ consistent (preconditioner cuts ~20%) |
| 7 | ~3e-6 at K=40 | 8.76e-6 | ✅ same order (3x = f32 rounding) |
| 8 | ~0.5% low | λmax 3.978 vs analytic 3.99999 | ✅ as predicted; ×1.05 margin covers (4.18 > 4.0) |
| 9 | seconds (sort-bound) | 1.8–2.6 s | ✅ |
| 10 | ~4 ms | 3.2–4.3 ms quiet / 5–9.5 loaded | ✅ |
| 11 | 20–50 µs | <50 µs (<0.1 ms display) | ✅ |
| 12 | ~7 µs | 6.9 quiet / 12.3 loaded | ✅ |
| 13 | ~2 µs | 1.7 / 1.8 / 2.0 µs (3 runs) | ✅ |
| 14 | ~0 | instant | ✅ |
| 15 | 40–70 ms | 41.7 quiet / 67.9 loaded | ✅ |

## 3. Adversarial probes (tried to break each formula)

| Probe | Attack | Result |
|---|---|---|
| A1 | d=8 max: apply ×16, diag ×64 vs d=2 | FORMULA HOLDS, evidence absent — no E2E fixture above d=2. Untested quadrant |
| A2 | Giant hyperedge (\|faces\|=1000) | Hoist saves 999 transposes by construction; no such fixture exists. Untested |
| A3 | Inconsistent maps (κ → 10⁴) | Bound says iters ×15; energy_target early-stop caps wall instead of speed. No ill-conditioned E2E. Untested |
| A4 | Empty ops / isolated cells | `restrict_active` removes analytically (x=x₀ exact); B5 gates it. Covered |
| A5 | n=1 trivial solve | 4 vector allocs + closure dispatch dwarf 1 FMA. Overhead regime, formula needs +c term |
| A6 | Load ×4 (50% CPU) | All walls inflate 2–4x uniformly (fused 5.3→24). Environmental, not formula failure |
| A7 | Power estimator on expander (fast mixing) | Converges in <25 iters; margin harmless. Only path-like graphs need the margin |
| A8 | f32 vs f64 accumulation | rel 1e-9..1e-12 on all sum gates; rounding visible only in Chebyshev (3x). Validated |
| A9 | Snapshot bit-rot (stale prefold) | No checksum/version in snapshot format — torn write replays as garbage, not error. REAL GAP (no gate) |
| A10 | BE-key overflow (key ≥ 2⁶⁴) | Impossible by type (u64). Non-issue |

## 4. Mismatch autopsy (every ❌/⚠️ explained)

| Mismatch | Cause | Math |
|---|---|---|
| Scan +40–85% over floor | Accumulator latency chain: `acc +=` serializes 4-cycle f64 adds | Serial model: 6M×4/4G = 6 ms; measured 5.3 → LLVM ~2-way unrolled; floor unreachable with single accumulator |
| Apply 25x over flops | Small-n overhead regime: dots/saxpys/closure/dispatch per iter | 54 µs/iter = ~2 µs flops + ~52 µs O(n) passes + fixed costs on n=4000 |
| Fused tied old loop at load | Contention ceiling (~1.3 GB/s both) replaces BW floor | Equal bytes-independent walls; byte-count only matters quiet |
| Crossover unknown | Flops dominate only when m·d² ≫ per-iter overhead (~n=10⁵+, untested) | Predict: kernel never ran that scale; DB-level wins come from structures, not solver scale |

## 5. Bottom line

```
Space formulas : EXACT (33 B/row verified twice, byte-for-byte)
Time formulas  : correct for work counted; wall = work + overhead + load
Overhead law   : small-n → fixed costs dominate (measured 25x on n=4000)
Latency law    : single-accumulator sums pay chain latency (measured +85%);
                 4-way unroll reaches 1.7 ms (BEATS the naive floor estimate —
                 floor was R+W copy BW; pure reads go faster)
Load law       : 2–4x uniform inflation, host-qualified (measured 5.3→24)
Untested       : d=8, giant hyperedges, ill-conditioned κ, n≈10⁵ solves
Real gap found : snapshot has no checksum (A9) — recommend u64 magic+len+crc
                 header before it becomes a silent-corruption vector
```
## A9 close-out (2026-10-21) — snapshot integrity

| Item | Before | After |
|---|---|---|
| Format | no checksum | `[MAGIC][fmt_ver][len][crc32]` (fiber_snap) and a sectioned container with per-section CRC32C (imdb bundle) |
| Torn write | replayed as garbage | rejected: "crc mismatch (torn write?)" — fail loud, never half-load |
| Stale source data | undetectable | section-level CRC + rebuild-on-missing; a fingerprint-bound cache is the open item (B8) |
| Gate | none | `gate_snap_corrupt_reject=PASS` on a flipped byte (both bundles) |

## Compile-time axis (N0/N8)

| Metric | Value | Method |
|---|---|---|
| Clean release build (fat LTO, 37 bins, `codegen-units=1`) | **109.3 s** | fresh `CARGO_TARGET_DIR`, cargo/rustc 1.98.1 |
| Warm incremental (no work) | 0.19 s | same tree |
| Incremental after 1 bin touched | 0.8 s | incl. process spawn |
| Release-only artifacts | 0.05 GB | main `target/` = 0.87 GB (debug + incremental) |
| Runtime fiber bundle build (IMDb, per run, pre-cache) | 178.96 s | now 51.1 s once + cached queries |

| Runtime fiber bundle build (IMDb, per run, pre-cache) | 178.96 s | now 51.1 s once + cached queries. Measured 112,721 ms for the forced full rebuild, of which 96.5% to 98.5% is the character-level parse | Plan: `PERFORMANCE_FIX_PLAN_2026-10-22.md` METHOD-061/062/063, 112,721 ms to 28,180 ms |