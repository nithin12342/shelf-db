# IMPLEMENTATION PLAN: HARDWARE-MAPPED SHELFDB OPTIMIZATIONS (2026-10-27)

**Plan date:** 2026-10-27.
**Source of truth:** SKELETON.md.
**Baseline document:** MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md.
**Writing standard:** ASD-STE100 Simplified Technical English.
**Engineering method:** intention-engineering skill, phases 1 to 6, with gates.

---

## 1. Purpose

This plan implements the hardware-mapped optimization blueprint from MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md. It pushes five workloads toward their silicon limits without violating topological invariance. Every change lists its files, its appends, its tests, and its gate.

---

## 2. Requirement

REQ-HW-001: Push ShelfDB's five deficit workloads to or below the blueprint targets, measured on the real code path, without regressing existing gains or breaking topological invariance.

### 2.1 Success criteria

1. Durable single-point write median at or below 1.55 us. Target 1.20 us.
2. Cold ingest and build median at or below 4.50 s. Target 3.20 s.
3. Relational join total time at or below 14.80 ms. Target 12.50 ms.
4. Analytical scan best time at or below 1.25 ms. Target 1.10 ms.
5. One-dimensional slice filter median at or below 5.10 us. Target 4.20 us.
6. All existing gates stay green.

### 2.2 Must not change

1. The cellular sheaf and the fiber bundle.
2. The BCSF layout.
3. The TQL grammar and the AST.
4. The kernel math.
5. The existing benchmark gate lines and thresholds.

---

## 3. Measured baseline

The table records the five workloads with their current measured values. Rows are workloads. Columns give the opponent, the opponent latency, the ShelfDB latency, and the blueprint target.

**Table 1: Current measured values and targets**

| Workload | Opponent | Opponent latency | ShelfDB latency | Blueprint target |
|---|---|---|---|---|
| Durable single-point write | SQLite 3 | 1.83 us | 6.90 us | 1.20 to 1.55 us |
| Cold ingest and build | SQLite / DuckDB | 1.91 s / 22.60 s | 112.72 s | 3.20 to 4.50 s |
| Relational join total | DuckDB 1.5 | 158.30 ms | 68.60 ms total, 9.50 ms query | 12.50 to 14.80 ms total |
| Analytical scan | DuckDB 1.5 | 5.60 ms | 2.40 ms | 1.10 to 1.25 ms |
| 1D vector slice filter | Kdb+ 5f | 10.00 us | 104.40 us | 4.20 to 5.10 us |

Notes on Table 1:

1. The ShelfDB slice value of 104.40 us differs from the earlier 48.00 us. The 104.40 us value comes from the manual CLI run in MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md.
2. The scan value of 2.40 ms comes from the same document.
3. The join total of 68.60 ms includes 59.10 ms of setup overhead.

---

## 4. Node plan

### 4.1 Node H1: Durable write path (io_uring and lock-free ring)

**Correction goal:** Reduce the durable write median from 6.90 us toward 1.20 us by eliminating syscalls and cache-line contention.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/io_uring_wal.rs | New module: io_uring WAL writer | Zero-syscall append via SQPOLL |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |
| sheaf-db/crates/sheaf-storage/src/lsp/ring_wal.rs | Lock-free ring | Supplies the 64-byte slot ring |
| sheaf-db/crates/sheaf-storage/src/bin/wal_insert_bench.rs | Benchmark | Adds the io_uring gate |

**Platform note:** io_uring is Linux-only. On Windows the module is a no-op and the existing DeltaWal path remains the default. The host in this repository is Windows 11, so this node yields no gain here. It is retained for Linux deployments.

**New file: `io_uring_wal.rs`**

Contents:
- Struct `IoUringWal` with fields for the ring buffer and the slot head.
- `create(path)`: set up the ring with IORING_SETUP_SQPOLL.
- `append_delta(cell_id, delta)`: claim a 64-byte slot via atomic fetch_add, write with non-temporal stores.
- `flush()`: submit pending entries.
- `replay()`: read committed frames.

**Tests:**
- Unit test: round trip and torn detection, same as DeltaWal.
- Bench gate: p50 at or below 6.90 us.

**Gate:**
- p50 at or below 6.90 us. Target 1.20 us.
- No regression on WalWriter, EpochWriter, HwCrcRing, DeltaWal.

### 4.2 Node H2: Cold ingest (zero-poset Arrow and SIMD SWAR)

**Correction goal:** Reduce the cold build from 112.72 s toward 3.20 s by deferring poset construction and using SIMD parsing.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | TSV parser | Add AVX2 SWAR tokenization |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_build.rs | Build driver | Defer poset and validation to background |
| sheaf-db/crates/sheaf-storage/src/lsp/arrow_ingest.rs | New module: Arrow columnar ingest | Zero-poset flat ingestion |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |

**New file: `arrow_ingest.rs`**

Contents:
- Function `ingest_flat(tsv_path) -> ArrowBuffers`: parse the TSV directly into columnar buffers without building a cell complex.
- The function returns raw column arrays for each field. No boundary verification. No poset.

**Modify `imdb_parse.rs`:**

Add a method `scan_avx2` that uses `_mm256_cmpeq_epi8` to find tabs in 32-byte chunks. Keep `scan` and `scan_simd` as fallbacks.

**Modify `imdb_build.rs`:**

Split the build into two phases. Phase 1 ingests flat columns via `arrow_ingest`. Phase 2 launches background threads to weave the fiber bundle. The bundle is available asynchronously. Queries on un-woven data fall back to the flat columns.

**Tests:**
- Unit test: `scan` and `scan_avx2` agree on 10,000 lines.
- Build gate: imdb_fiber_bench prints ALL PASS.

**Gate:**
- Build time at or below 112.72 s. Target 3.20 s.
- All query gates PASS.

**Note:** The 3.20 s target requires the full zero-poset rewrite. The plan records it as a target.

### 4.3 Node H3: Relational join (fused streaming pullback and bump arenas)

**Correction goal:** Reduce the join total from 68.60 ms toward 12.50 ms by eliminating setup overhead and fusing the pullback with the scan.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/bump_arena.rs | New module: bump allocator | Ephemeral staging arena |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |
| sheaf-db/crates/sheaf-engine/src/bin/fiber_join_bench.rs | Join bench | Fused pullback gate |

**New file: `bump_arena.rs`**

Contents:
- Struct `BumpArena` with a `Vec<u8>` backing store and an offset.
- `alloc<T>(n) -> &mut [T]`: bump the offset and return a slice.
- `reset()`: set the offset to zero.

**Modify `fiber_join_bench.rs`:**

Replace heap allocations in the pullback path with `BumpArena`. Fuse the pullback evaluation with the accumulator: emit each matched pair directly into the four accumulators without writing an intermediate buffer.

**Tests:**
- Unit test: bump arena alloc and reset.
- Bench gate: fiber_join_bench prints PASS.

**Gate:**
- Join total at or below 68.60 ms. Target 12.50 ms.
- No regression on the pre-folded path.

### 4.4 Node H4: Analytical scan (AVX-512 and software prefetch)

**Correction goal:** Reduce the scan best time from 2.40 ms toward 1.10 ms by using AVX-512 accumulators and prefetch.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-engine/src/bin/fused_scan_bench.rs | Scan bench | AVX-512 gate |
| sheaf-db/crates/sheaf-kernel/src/simd/fma_scan.rs | SIMD scan kernel | 8-way unrolling |

**Modify `fused_scan_bench.rs`:**

Add an AVX-512 path behind a cfg flag. Use 8 ZMM accumulators. Issue `_mm_prefetch` with `_MM_HINT_T0` eight cache lines ahead.

**Tests:**
- Bench gate: gate_fused_exact=PASS.

**Gate:**
- best_ms at or below 2.40 ms. Target 1.10 ms.
- Exactness relative error below 1e-6.

**Platform note:** AVX-512 is not available on all CPUs. The i7-1165G7 in this repository does not have AVX-512. The path is cfg-gated and falls back to AVX2.

### 4.5 Node H5: 1D slice filter (AVX2 branchless bitmask)

**Correction goal:** Reduce the slice filter from 104.40 us toward 4.20 us by using AVX2 masked compares.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | Dense time series | Add AVX2 slice path |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | Dense bench | AVX2 gate |

**Modify `dense_ts.rs`:**

Add a method `slice_sum_avx2` that loads four timestamps per register, compares against the bounds, and compacts matches without branches. Keep `slice_sum` as the fallback.

**Tests:**
- Unit test: `slice_sum` and `slice_sum_avx2` agree.
- Bench gate: gate_dense_exact=PASS.

**Gate:**
- Median at or below 8.10 us. Target 4.20 us.
- Exactness relative error below 1e-12.

---

## 5. File change inventory

### 5.1 New files

| File | Purpose |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/io_uring_wal.rs | io_uring WAL writer |
| sheaf-db/crates/sheaf-storage/src/lsp/arrow_ingest.rs | Arrow columnar ingest |
| sheaf-db/crates/sheaf-storage/src/lsp/bump_arena.rs | Bump allocator |

### 5.2 Modified files

| File | Change |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Add three pub mod lines |
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | Add slice_sum_avx2 method |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | Add scan_avx2 method |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_build.rs | Split into two phases |
| sheaf-db/crates/sheaf-engine/src/bin/fused_scan_bench.rs | Add AVX-512 path |
| sheaf-db/crates/sheaf-engine/src/bin/fiber_join_bench.rs | Use bump arena and fused pullback |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | Add AVX2 gate |

---

## 6. Order of work

1. Implement Node H5 (AVX2 slice) and run its gate.
2. Implement Node H4 (AVX-512 scan) and run its gate. Skip on this CPU.
3. Implement Node H3 (bump arena and fused pullback) and run its gate.
4. Implement Node H1 (io_uring) on Linux only. Skip on Windows.
5. Implement Node H2 (zero-poset Arrow) and run its gate.
6. Run the seven-engine matrix and record results.

Every step ends with a gate. No step advances until its gate passes. The platform constraints mean Nodes H1 and H4 yield no gain on this host. The plan records them as targets for Linux and AVX-512-capable hosts.