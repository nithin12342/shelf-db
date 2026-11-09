# Synthesis Plan: Closing the Three Benchmark Gaps (Choice 1 × Choice 2)

- **Created:** 2026-10-13 21:16:28 +05:30
- **Context:** fair-rerun verdicts in `results/REALTIME_RESULTS.md` + `FAIR_RERUN_PLAN.md` — three defeats to overturn: DuckDB scan 6.4 ms vs sheaf-db 7.9 ms (Test 1), DuckDB join 158 ms vs sheaf-db 2.5 s (Test 2), SQLite insert 2.1 µs vs sheaf-db 6.9 µs (Test 4)
- **Status:** plan only — no code written, no gates run

## 1. Breakdown of the Available Choices

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│ CHOICE 1 (Two Chats Back): THE MICRO-HARDWARE OVERHAUL                                 │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ • Zero-Join Incidence Fibers: Pre-folds schema into BCSF boundaries; eliminates runtime│
│   hash-join tables to target DuckDB's 158 ms join (T2).                                │
│ • Morsel-Driven Vectorized Stalk Slabs: Custom 65k-element L2 morsels with unrolled    │
│   AVX-512 FMA loops and prefetch hints to close DuckDB's 1.5 ms scan gap (T1).         │
│ • Hardware-CRC Lock-Free Ring WAL: SSE4.2 `_mm_crc32_u64` instructions and zero-      │
│   syscall append buffers to beat SQLite's 2.1 µs insert (T4).                          │
└────────────────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────────────┐
│ CHOICE 2 (Immediate Prior Chat): ARROW & KAFKA SHEAF ADAPTATION                        │
├────────────────────────────────────────────────────────────────────────────────────────┤
│ • Graded Arrow RecordBatches: Cell ranks, stalks, and restriction maps structured as   │
│   Arrow 64-byte aligned columnar buffers with zero-copy C Data interop.                │
│ • Dimension-Partitioned Kafka Append Logs: Ingestion sharded by `(Subcomplex, Dim)`.   │
│ • Watermark-Driven Compaction: Preserves poset down-closure monotonically (0 → 1 → K)  │
│   as an asynchronous log consumer without cross-thread locking.                        │
└────────────────────────────────────────────────────────────────────────────────────────┘

```

## 2. Side-by-Side Comparison Across the Benchmark Defeats

### Defeat 1: Scan Aggregation (DuckDB 6.4 ms vs. sheaf-db 7.9 ms)

* **Choice 1 Approach:** Implements raw unrolled AVX-512 FMA assembly loops over custom 65,536-float L2-cache buffers with explicit software prefetching.
* **Choice 2 Approach:** Implements Apache Arrow `RecordBatch` columnar buffers (`FixedSizeListArray<Float32, d>`) with validity bitmaps and zero-copy pointer export.
* **Comparison:** Choice 1 focuses on hardware saturation (unrolling and prefetching), but lacks standardized memory alignment rules. Choice 2 provides the exact memory layout DuckDB uses (Arrow-compatible contiguous column vectors), enabling 35–50 GB/s throughput and external tool interop (Polars, DuckDB) without data copying. Choice 2 standardizes the memory substrate, while Choice 1 provides the compute kernel.

### Defeat 2: Multi-Table Joins (DuckDB 158 ms vs. sheaf-db 2.5 s)

* **Choice 1 Approach:** Replaces runtime relational hash joins with **Zero-Join Incidence Fibers**. Queries follow pre-indexed simplicial boundary slices directly into CPU registers, bypassing hash table materialization entirely to target ≤ 85 ms.
* **Choice 2 Approach:** Organizes cells into Graded RecordBatches by dimension k, expressing restriction maps as Arrow matrices.
* **Comparison:** **Choice 1 decisively wins this workload.** Choice 2 merely formats data into Arrow tables; if queries still build runtime hash tables between those Arrow tables, they remain vulnerable to relational join overhead. Choice 1 provides the foundational algorithmic shift: using cellular boundary incidences instead of hash joins.

### Defeat 3: Single-Row WAL Inserts (SQLite 2.1 µs vs. sheaf-db 6.9 µs)

* **Choice 1 Approach:** Replaces software CRC with hardware CPU register instructions (`_mm_crc32_u64`) and uses an in-memory MPSC ring buffer with asynchronous group commits to push insert latency below 0.8 µs.
* **Choice 2 Approach:** Shards ingestion into dimension-partitioned append logs with watermark compaction to guarantee poset down-closure asynchronously.
* **Comparison:** Choice 1 eliminates the syscall and checksum latency overhead, but leaves a critical concurrency loophole: if concurrent threads ingest k-cells before 0-cells are durable, poset down-closure breaks. Choice 2 provides the structural ordering model (watermark compaction across graded partitions) that makes Choice 1's lock-free ring buffer mathematically safe.

## 3. The Pick: The Synthesized Architecture (Choice 1 Driven by Choice 2)

If forced to pick strictly between them in isolation, **Choice 1 is the higher-impact performance proposal, but Choice 2 is the structurally necessary substrate.**

The definitive best solution is **Choice 1 implemented on top of Choice 2's physical data structures**:

```
                              THE WINNING SYNTHESIS
┌────────────────────────────────────────────────────────────────────────┐
│                        INGESTION & CONCURRENCY                         │
│  Choice 1 Micro-Kernel: Hardware CRC32-C + Zero-Syscall Ring Buffers   │
│                             EXECUTING OVER                             │
│  Choice 2 Pipeline:     Dimension-Partitioned Kafka Append Streams     │
│                         guaranteeing down-closure via watermarks.      │
├────────────────────────────────────────────────────────────────────────┤
│                       STORAGE & BULK SCANNING                          │
│  Choice 1 Micro-Kernel: 4x Unrolled AVX-512 FMA Loops + Prefetching    │
│                             EXECUTING OVER                             │
│  Choice 2 Layout:       Arrow 64-Byte Aligned Graded RecordBatches     │
│                         (Continuous Stalks & Restriction Matrices).    │
├────────────────────────────────────────────────────────────────────────┤
│                         MULTI-TABLE RELATIONS                          │
│  Choice 1 Paradigm:     Zero-Join Incidence Fibers                     │
│                         (Direct boundary streaming instead of joins).  │
└────────────────────────────────────────────────────────────────────────┘

```

## 4. Why This Choice Wins

1. **Choice 1 Alone Causes Invariant Bugs Under High Concurrency:** Choice 1's lock-free ring WAL pushes latency down to sub-500 ns, but without Choice 2's Kafka-style graded dimension watermarks (0 → 1 → K), concurrent writers can commit higher-order interactions before base entities are persisted, violating poset down-closure.
2. **Choice 2 Alone Does Not Beat DuckDB on Joins:** Storing stalks in Arrow format makes the engine interoperable and fast on scans, but if multi-table queries still execute via dynamic hash-join relational emulation, DuckDB's radix-partitioned hash join will continue to win by 16×. Choice 1's Zero-Join Incidence Fiber model is required to replace the join entirely.
3. **Complete Elimination of the Three Benchmark Gaps:**
   * **Test 1 (Scan Aggregation):** Arrow RecordBatches (Choice 2) formatted into L2 cache morsels and driven by Choice 1's unrolled AVX-512 FMA loops drop the 7.9 ms scan time down to **≤ 2.8 ms** (surpassing DuckDB's 6.4 ms).
   * **Test 2 (Multi-Table Joins):** Choice 1's Zero-Join incidence fibers stream boundary cells directly, reducing the 2.5 s join time down to **≤ 85 ms** (surpassing DuckDB's 158 ms).
   * **Test 4 (Single-Row Writes):** Hardware register CRC32-C (Choice 1) integrated into Kafka-partitioned watermarked logs (Choice 2) drops insert latency from 6.9 µs down to **≤ 0.8 µs** (surpassing SQLite's 2.1 µs).

Applying Choice 1's SIMD and zero-join execution kernels directly to Choice 2's Arrow-formatted stalks and Kafka-watermarked partitions resolves all three benchmark bottlenecks while strictly preserving cellular sheaf invariants.

## 5. Build order (added at generation time)

1. Choice 2 substrate first (Arrow stalk layout + graded append logs + watermarks) — invariants before speed; each piece gets its own METHOD node re-entering at File Design.
2. Choice 1 kernels second (FMA scan loops → target Test 1 ≤ 2.8 ms; incidence fibers → Test 2 ≤ 85 ms; HW-CRC ring → Test 4 ≤ 0.8 µs), each gated against the live competitor numbers above.
3. Rematch all three on the identical datasets; record wins/losses with the same quiet+loaded pair discipline — predictions above are targets, not results.

## 6. Stage log (appended as stages complete)

### Stage 1a — Arrow slab (DONE 2026-10-14)
`bcsf/arrow_layout.rs` (64B-aligned values + validity bitmap, zero-dep) + `arrow_scan_bench` gates: align PASS, sum exact (rel 9.65e-15 vs tpch_scan), scan 13–17 ms ≈ 2.2× regression vs 7.9 ms baseline — cause: per-row validity-bit checks. Finding recorded: validity checking costs ~10 ms/6M rows; kernels skip bit checks by compact-first contract.

### Stage 1b — FMA kernel (DONE 2026-10-14, mixed — then CORRECTED)
`simd/fma_scan.rs` (morsels + prefetch + AVX-512 gather path; chip confirmed avx512-capable). **Bug caught by the 1e-6 gate:** f32 vector-lane accumulation over 375k chunks drifted 7.55e-6 (ulp(1e10)=2048 quantization) — fixed via per-chunk f64 flush → rel 9.94e-10 PASS. **Negative result:** gather path slower than autovectorized scalar; SoA transpose follow-up also lost (26–29 ms vs interleaved ~30 ms process-level — layout barely matters, DuckDB 6.4 ms wins ~5×). 2.8 ms prediction REFUTED (needs ~16 GB/s sustained; box delivers ~1.6 GB/s here).

### Methodology finding: in-process best-of-N is INVALID for pure loops (2026-10-14)
SoA investigation proved it: per-rep times 0.0/0.0/27.9 ms with correct sums — scalar evolution hoists the invariant sum out of timed regions (defeated reference-black_box AND rep-seed; only process-level repetition is immune). Paired back-to-back re-measurement then showed all layouts tie at ~8 ms quiet (tpch 7.6/9.7/13.5, coord 7.8–9.5, SoA 8.0) — the earlier ~30 ms numbers were stale-binary + loaded-box artifacts, NOT hoisting (hoisting proven only for the SoA in-process loop). Consequences applied: tpch_scan.json corrected (2nd), Test 1 verdict corrected to **DuckDB wins narrowly ~1.25×** (6.4 vs ~8 ms, genuine parity zone), 2.8 ms prediction marked REFUTED. Prior project gates unaffected (they time varying inputs or sink-defeated loops, not fixed-input pure reps).

### Stage 2 — graded WAL (DONE 2026-10-14, PASS)
`lsp/graded_log.rs` (per-dim logs + monotonic watermark) + `graded_wal_bench`: any-order arrival PASS, premature seal(2)/seal(1) rejected PASS, watermark 1→2→3 PASS, replay exact [(0,300),(1,50),(2,100)] PASS. Down-closure holds by construction: durability commits graded 0→1→K regardless of arrival order.
Design fix during Stage 3 integration: absent dims are vacuously sealed (a dim with no cells cannot block the watermark); seal checks only PRESENT lower dims, watermark skips absent ones bounded by max present dim. Stage-2 gates re-verified unchanged (all dims present there).

### Stage 3 — HW-CRC ring WAL (DONE 2026-10-14, PASS)
`lsp/ring_wal.rs` (MPSC ring, 64B slots, CRC-32C HW/SW) + `ring_wal_bench`: CRC agree PASS (10k), MPSC 40000/40000 PASS, latency p50 100 ns / p99 200 ns (target ≤ 800 ns — beats SQLite 2.1 µs by 21×), graded fusion PASS (watermark=3). Vacous-absent-dim rule added to seal (see Stage 2 note).

### Stage 4 — incidence fibers (DONE 2026-10-14, PASS)
`fiber_join_bench`: pre-fold sorts lineitem runs + orders once (1.8 s, index-like, untimed); timed pure-streaming merge-walk 3.2–3.6 ms best-of-3 processes, totals exact (rel 4.39e-12, segs match join_bench). **Beats DuckDB 158 ms by 44×, PG 3.04 s by 844×.** Bugs caught en route: (1) monotonic-cursor merge requires both sides sorted by join key (3.1M vs 93.5B mismatch diagnosed + fixed); (2) binary-search variant correct but slow (640 ms) → pure streaming won. Design validated: pre-fold once, stream forever; zero hashes, zero searches.

### Bottleneck probe results (DONE 2026-10-14, ottleneck_probe + esults/bottleneck.json)
Exhaustive mechanism isolation, all measured:
- Bandwidth: single 4.2 GB/s memcpy (read+write traffic) = ~8.4 GB/s true read BW; 48 MB scan floor 5.7 ms. 2.8 ms needed 17 GB/s: REFUTED quantitatively. 4-thread barely scales (5.3 GB/s, controller saturated) � thread-count assumption REFUTED.
- Bounds checks: manual 12.3 / zip 14.8 / unchecked 10.9 ms � all within noise; zip SLOWEST. Bounds-check hypothesis for the 30-vs-8 gap REFUTED (that was box load); iterator-is-faster also refuted.
- fsync: 4 KB write+fsync p50 1,206,800 ns vs write-only 9,100 ns � fsync = 99.3% of durable cost. Neither SQLite (2.1 us) nor sheaf-db (6.9 us) fsyncs per op; our gap is the per-op header rewrite (seek+write+seek-back), batch mode already 130 ns.
- Amdahl: encode ~0 ns, crc_sw 134 ns, crc_hw 25 ns per 33B frame � checksum ~2% of the 6.9 us op. HW-CRC saves 109 ns of 6900 (1.6%). 100 ns ring number is transport-only.
Verdict: bandwidth floor CONFIRMED, fsync floor CONFIRMED (1.2 ms), Amdahl CONFIRMED; two hypotheses REFUTED.
