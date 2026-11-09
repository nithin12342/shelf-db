# Max-Performance Task Plan (P1–P6)

- **Created:** 2026-10-14 (IST)
- **Basis:** bottleneck-probe measurements (`results/bottleneck.json`); proposal in conversation 2026-10-14
- **Traceability:** REQ-030..035 / SPEC-030..035 / METHOD-024..029 (stubs in SKELETON.md)
- **Status:** plan only — no code written

## Walls (measured, not assumed)

| # | Wall | Measurement | Consequence |
|---|------|-------------|-------------|
| W1 | Read bandwidth ~8.4 GB/s single-thread; 4-thread ~1.25× (controller-capped) | `bottleneck_probe` A/B | 48 MB scan floor ≈ 5.7 ms; thread scaling is NOT a lever past ~2 threads |
| W2 | fsync ~1.2 ms vs write-only ~9 µs (99.3%) | probe D | Durability floor; both sides avoid per-op sync — compare amortized paths only |
| W3 | Checksum ~2% of durable op (134 ns sw / 25 ns hw of 6.9 µs) | probe E | HW-CRC saves ~1.6%; never the lever |
| R1 | Bounds checks within noise (manual 12.3 / zip 14.8 / unchecked 10.9 ms) | probe C | Layout micro-tuning closed; stop |
| R2 | Zip slowest — iterator-is-faster refuted | probe C | Write plain loops; let LLVM vectorize |

## Tasks (priority order)

### P1 — Parallel partitioned scan (METHOD-024, FILE-060 `simd/mt_scan.rs`)
2–4 threads, per-thread f64 accumulators, single final reduction over the 6M-row slab. Hard gate: exact + ≤ 8 ms parity. Stretch: ≤ 6.4 ms (DuckDB quiet). Expectation from W1: ~4–6 ms (bandwidth-split, not 8×).

### P3 — Incremental pre-aggregation (METHOD-025, FILE-061 `lsp/preagg.rs`)
Per-segment accumulators O(1) at ingest under graded ordering; GROUP BY reads O(#groups). Hard gate: totals exact vs 93,574,920,897. Memory per (group × measure) reported. This dissolves repeated aggregation — biggest structural payoff.

### P4 — Append-only epoch WAL (METHOD-026, FILE-062 `lsp/epoch_wal.rs`)
Epoch-validated headers (no seek-back rewrite); group-commit amortization. Hard gate: correctness (replay exact, down-closure) + durable single-op in SQLite's 2.1 µs class; report sync and amortized separately (W2 honesty rule).

### P2/P6 — Hygiene (no new files)
Validity-free nulls (sparse exception lists in `name_lookup`) + bitmask adjacency (in `partition`). Hard gates: exactness + measured deltas only.

### P5 — MPHF index (lowest priority, diminishing returns)
Only after P1–P4. Hard gate: exactness + ≤ 10.5 ns parity; stretch ~3–5 ns.

## Discipline (unchanged)

Process-level repetition only; quiet+loaded pairs; clippy `-D warnings` clean; ≤500-line files; one commit per task; predictions are stretch targets, misses recorded with mechanism (per precedent: 2.8 ms REFUTED, hoisting finding, CRC-layer finding).
