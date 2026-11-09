# Math Upgrades Implementation Plan (M1–M4)

- **Created:** 2026-10-14 (IST)
- **Source theory:** user-supplied §§1–4 (graded poset + sheaf functor + cochain/Laplacian + Mayer-Vietoris; diagnosis of the two defeats; four modifications)
- **Status:** plan only — no code written, no gates run
- **Standing results to beat:** Test 1 DuckDB 6.4 ms (sheaf ~30 ms) · Test 4 SQLite 2.1 µs (sheaf 6.9 µs durable / 100 ns ring-transport) · Test 2 sheaf-fibers 3.2 ms (already beating DuckDB 158 ms) · kernel PCG currently 2 iters post-fix on 10k-noisy fixture

## Gate discipline (applies to every stage)

- **Hard gate (must pass):** bit-exact or tolerance-exact results vs the existing implementation on identical data + no regression vs the standing number + `cargo clippy --all-targets -- -D warnings` clean + new files ≤500 lines.
- **Stretch goal (recorded, never gating):** the headline prediction (≤2.5 ms / <0.5 µs / sub-ms / 3–5 iters). Misses are recorded with cause like all prior negative results (cf. 2.8 ms REFUTED, Test 1 hoisting correction).
- **Timing method:** process-level repetition only (SYNTHESIS_PLAN methodology finding — in-process best-of-N is invalid for pure loops). Single timed rep per process, 3 processes, best taken.
- **Commits:** one cherry-pickable commit per stage (code + gates + plan-log entry).

## Stage M1 — Coordinate isomorphism (scan)

**Theory:** $C^0(P;\mathcal{F}_a) \cong \mathbb{R}^{|P_0|}$ per attribute; aggregation $= \langle \mathbf{1}, \mathbf{v}_a \rangle$.
**Build:** `bcsf/coord.rs` — per-attribute dense 64B-aligned vecs (reuse `AlignedBuf` from `arrow_layout.rs`; one vec per attribute, row = cell ordinal, append-only). New bin `coord_scan_bench.rs`: load lineitem price/discount into two coord vecs (load untimed/reported), timed inner-product sum × 3 processes.
**Bandwidth arithmetic (honest):** 6M×2×4 B = 48 MB; box delivers ~1.6 GB/s on this pattern (measured) → floor ≈ 30 ms; needs ~19 GB/s sustained for 2.5 ms. SoA already measured 26–29 ms on identical traffic.
**Hard gate:** sum exact (rel < 1e-9 vs 218102223881.86) + time ≤ 30 ms (no regression vs best standing sheaf-db scan).
**Stretch:** ≤ 6.4 ms (beat DuckDB quiet); ≤ 2.5 ms only if bandwidth cooperates.

## Stage M2 — Lazy coproduct ingestion (writes)

**Theory:** ingestion $= O(1)$ free-monoid append ($P_{t+1} = P_t \amalg \mathrm{Free}(\tau)$); canonical quotient $q$ runs at compaction.
**Build:** `lsp/colimit.rs` — background projection: drain sealed graded logs → sorted incidence + BCSF segment via existing `BcsfBuilder`; new bin `colimit_bench.rs`.
**Hard gate:** append path unchanged (ring 100 ns already beats 0.5 µs target — re-verify) + colimit output `validate()` clean + query parity: fiber-join totals on colimit-built BCSF match 93,574,920,897 exactly.
**Stretch:** end-to-end durable insert < 0.5 µs (append + group-commit amortized); SQLite 2.1 µs comparison repeated on identical 20k-insert workload.
**Note:** the 0.5 µs prediction is credible only amortized (group commit); per-op fsync physics (~µs on NVMe) bounds the synchronous case — the gate measures and reports both, like Test 4 did (6.9 µs sync / 130 ns batched).

## Stage M3 — Exterior contraction (joins)

**Theory:** $N$-ary relations as $k$-forms $\omega \in \bigwedge^k C^0$; evaluation as interior contraction $\iota_v$ over sorted boundary arrays with SIMD bitmask masking.
**Build:** `operators/contraction.rs` — contraction kernel over the fiber arrays from Stage 4 (sorted, contiguous); new bin `contract_join_bench.rs` running the identical GROUP BY segment workload (totals must match 93,574,920,897).
**Hard gate:** totals exact + time ≤ 3.2 ms (no regression vs fiber baseline).
**Stretch:** sub-ms (needs ~3× over fibers; bitmask-vs-branch + register-resident boundaries are the mechanism; if it lands 1–3 ms that is still a decisive DuckDB-scale win, recorded as such).

## Stage M4 — Hodge preconditioning (solvers)

**Theory:** $C^k = \ker(L_k) \oplus \mathrm{im}(\delta) \oplus \mathrm{im}(\delta^*)$; project out high-frequency boundary noise before PCG.
**Build:** `solvers/hodge.rs` — one δ/δ* smoothing projection producing the PCG start vector; new bin `hodge_bench.rs`.
**Fixture (required for falsifiability):** current BJ-PCG already converges in 2 iters on the 10k-noisy fixture, so "18 → 3–5" is untestable there. New hub-heavy fixture (power-law degree, 100k nodes) where BJ-PCG demonstrably stalls (>18 iters or misses E ≤ 1e-4); gate = Hodge start brings it to ≤ 5 iters at E ≤ 1e-4 with bit-exact section vs a converged reference.
**Hard gate:** exactness + ≤ 5 iters on the hub fixture + no regression on the 10k fixture (still ≤ 25-iter gate, still E ≤ 1e-4).
**Stretch:** none beyond the gate — the gate IS the strong claim here.

## Build order

M1 → M2 → M3 → M4 (read paths, then write path + compaction, then joins, then solvers; M4 independent and could parallelize, kept sequential for clean bisection). After all four: full rematch (Tests 1–5 + R1–R3 proposal) and quiet+loaded pairs per standing discipline. Predictions above are targets, not results.

## Stage log

### M1 — coordinate vectors (DONE, PASS hard gate)
`bcsf/coord.rs` + `coord_scan_bench`: 64B-aligned, sum exact, ~8 ms process-level (parity with all layouts). 2.5 ms stretch REFUTED (bandwidth floor).

### M2 — lazy colimit (DONE, PASS all gates)
`lsp/colimit.rs` + `colimit_bench`: ordered seal, validate, boundary parity 14999==14999, frame roundtrip. Vacuous-absent-dim rule added during Stage 3 integration.

### M3 — exterior contraction (DONE, mixed)
`operators/contraction.rs` (masked + grouped kernels) + `contract_join_bench`: totals exact (rel 4.39e-12), 7.6 ms single-rep. Hard gate (≤ 3.2 ms fibers) MISSED — per-row indexed scatter costs ~2.4× vs per-customer dense accumulation (real finding). Still beats DuckDB 158 ms (21×) and PG 3.04 s (400×). Sub-ms stretch REFUTED on this hardware. Fibers (Stage 4 legacy result, 3.2 ms) remain the fastest join path.

### M4 — Hodge preconditioning (DONE, NEGATIVE)
`solvers/hodge.rs` (damped Jacobi start, ω param) + `hodge_bench` (hub fixture: 2000-face hyperedge + local edges). Formulations tried: (1) alternating near-harmonic start → undamped ω=1.0 DIVERGED (E=1272 vs 7e-4); damped ω=0.25 still hurt (E=45.7) — block-diagonal M mismatches hub coupling and damages an already-good start. (2) random-noise start → plain 8 iters / E=0.0164 vs hodge 8 iters / E=0.0165: IDENTICAL, no effect. Neither reaches E ≤ 1e-4 (regularized solution sits at E≈0.016 — gate target unreachable on this fixture regardless of start). Verdict: smoother is dead weight everywhere tested; BJ-PCG alone does the job. Code kept as documented experimental negative (correct, tested, clippy-clean), not wired into any solve path.
