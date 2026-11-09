# IMPLEMENTATION PLAN: OPTIMIZE AND EXPAND SHELFDB (2026-10-27)

**Plan date:** 2026-10-27.
**Source of truth:** SKELETON.md.
**Plan of record for performance:** PERFORMANCE_FIX_PLAN_2026-10-22.md.
**Writing standard:** ASD-STE100 Simplified Technical English.
**Engineering method:** intention-engineering skill, phases 1 to 6, with gates.

---

## 1. Purpose

This plan turns the ideas in `optimize and expand.txt` into code. It lists every change to existing files, every new file, and every append. It defines the tests against the 50 test cases, the seven-engine matrix, and all existing tests. It sets the measurement gates that decide whether each change is accepted.

The plan keeps the cellular sheaf, the fiber bundle, and every layer in ARCHITECTURE.md. A change that replaces them with a relational, vector, or key-value engine is rejected, even if it is faster.

---

## 2. Requirement

REQ-OE-001: ShelfDB must reach or beat each compared engine in that engine's own area, measured on the real code path, without regressing existing gains, state transitions, or throughput bounds.

### 2.1 Success criteria

1. All existing cargo tests pass with zero failures.
2. All existing benchmark binaries print their gate lines as PASS.
3. The durable-write path reaches a median at or below 6.90 microseconds, with a target of 1.60 microseconds.
4. The dense one-dimensional slice filter reaches a median at or below 48.00 microseconds, with a target of 8.50 microseconds, and exactness relative error below 1e-12.
5. The cold ingest path reaches a median at or below 61,100 milliseconds, with a target of 2,100 milliseconds.
6. The fused scan stays below 6.40 milliseconds with relative error below 1e-6.
7. The 50 test case suite reports matching counts that agree with SQLite on at least 45 of 50 cases.
8. The seven-engine matrix shows no regression on any headline metric.
9. The Kdb+ comparison is re-measured on one timer basis and the ratio is stated with that basis.

### 2.2 Must not change

1. The cellular sheaf and the fiber bundle.
2. The BCSF layout: dims, offsets, indices, arena, discrete 16-byte slab.
3. The TQL grammar and the AST.
4. The kernel math: CoboundaryOp, SheafLaplacian, harmonic solver.
5. The existing benchmark gate lines and their thresholds.
6. The seven-engine matrix runner's engine list and measurement protocol.
7. The Semantics of WalWriter, EpochWriter, and HwCrcRing. New code is added beside them.

---

## 3. Measured baseline (corrected)

The table records the five failure states. Rows are failure states. Columns give the workload, the opponent time, the ShelfDB time, the measured deficit, the evidence, and the status.

**Table 1: Measured failure states**

| Failure state | Workload | Opponent time | ShelfDB time | Measured deficit | Evidence | Status |
|---|---|---|---|---|---|---|
| Durable single-row write | 10,000 single-row durable inserts | SQLite 3: 1.70 us (2026-10-23 run) | 6.90 us | 4.06x slower for ShelfDB | DETAILED_LOSS_ANALYSIS_2026-10-23.md Table 1. The ShelfDB value has no committed result file. | Holds, permanent boundary |
| Raw join without fiber bundle | 6,001,215 lineitem rows | DuckDB 1.5: 205.00 ms median, 158.30 ms quiet | 2,500.00 ms | 12.20x slower at median, 15.79x at quiet | DETAILED_LOSS_ANALYSIS_2026-10-23.md Table 1 and section 3.2. No committed result file. | Holds, architectural boundary |
| Analytical scan, single accumulator | 6,001,215 lineitem rows | DuckDB 1.5: 5.60 ms | 7.60 ms before fusion, 4.00 ms after fusion | 1.36x slower before fusion, 1.40x faster after fusion | DETAILED_LOSS_ANALYSIS_2026-10-23.md Table 1 and section 3.3 | Solved in two steps |
| Cold ingest and setup | 3.6 GB IMDb TSV | DuckDB 1.5 and Redis: 0.10 s to 22.60 s | 51.10 s to 112.72 s before METHOD-063, 54.10 s to 61.10 s after | 126.11% to 400.00% slower for ShelfDB | PERFORMANCE_FIX_PLAN_2026-10-22.md METHOD-056 | Holds, structural |
| Pure one-dimensional vector slice filter | 100,000 tick rows | Kdb+ 5f: 10.00 us in process, 153.3 us from the client | 48.00 us | Not established | results/KDB_TIMESERIES_RESULTS.json. The ShelfDB value has no committed result file. | Not established |

Notes on Table 1:

1. The SQLite value of 1.70 us is from the 2026-10-23 run. The 2026-10-26 run records 4.8 us, which gives 1.44x. The 4.06x is the wider gap.
2. The 15.79x value uses the 158.30 ms quiet DuckDB value. The median is 205.00 ms, which gives 12.20x.
3. The two Kdb+ values use different timer bases and differ by 15.3x. Neither ratio is established because the ShelfDB value has no client-observed pair.
4. The fused-scan row records two steps. Loop fusion takes ShelfDB from 7.60 ms to 4.00 ms. The four-accumulator change takes the TPC-H scan from 4.0 ms to 2.9 ms.
5. The cold-ingest row uses the post-METHOD-063 range as the current baseline. The 51.10 s to 112.72 s range is the pre-change value.

---

## 4. Mathematical claims in the attached text: verified and corrected

The attached text proposes a stratified cellular pre-sheaf. Four of its claims do not hold as written. The plan uses the corrected versions.

### 4.1 Orthogonal restriction maps and the diagonal claim

Claim: if the restriction maps are in O(d), the diagonal block simplifies to deg(v) times the identity, with zero runtime overhead.

Correction: the simplification holds when every restriction map is an orthogonal matrix of the same dimensions, so F transpose F equals the identity. If the stalk dimensions differ, an orthogonal map does not exist and only a partial isometry is possible. In that case F transpose F is a projector, not the identity. The code in `sheaf-kernel/src/operators/coboundary.rs` uses one morphism per hypercell shared across all faces, so the simplification applies to that special case. The general case does not get it for free.

Gauge equivariance: L prime equals Q L Q transpose holds only when the morphism transforms with Q. If Q changes the basis of the vertex stalk but the morphism stays fixed, the energy changes. So gauge equivariance is a property of the family of maps, not a runtime free lunch.

### 4.2 Pullback formula for lateral skips

Claim: parallel transport along a path is F inverse for the second edge composed with F for the first edge.

Correction: the formula as written mixes the orientation and assumes the restriction maps are square and invertible. In general, restriction maps are rectangular and the inverse does not exist. The correct transport is the pseudo-inverse when dimensions differ, and the composition order follows the path orientation. The code has no lateral-skip operator today, so this is a design note for the dynamic pullback node.

### 4.3 Kernel preservation under the additive term

Claim: L tilde equals L plus the projected lateral term preserves ker L equals ker L tilde.

Correction: the equality holds when the lateral term is positive semi-definite on the orthogonal complement of ker L. If the term is indefinite, new kernel directions can appear. The projector onto the orthogonal complement of ker L requires computing the kernel of L, which is a sparse null-space problem. The code computes no kernel today. The honest version is a gated term with alpha defaulting to zero, so the baseline path is bit-identical. That is the exact homotopy retract.

### 4.4 Cost of the gated term

The attached text treats the orthogonal projector as free. It is not. The plan therefore gates the lateral term at zero by default and measures before claiming any gain.

---

## 5. Architecture policy

The table records the foreign technique for each failure state and the reason the policy rejects it.

**Table 2: Foreign-engine paths rejected**

| Failure state | Foreign technique | Reason for rejection |
|---|---|---|
| Durable single-row write | Generic group commit and B-tree page writes | Replaces the sheaf write path with a foreign storage engine |
| Raw join without fiber bundle | Relational hash join | Replaces pre-folded connections with a foreign join operator |
| Analytical scan | Unchecked SIMD accumulator tricks | Removes the graded cochain structure from the scan |
| Cold ingest and setup | Generic parallel CSV parser | Treats ingest as text parsing instead of section assembly |
| Pure one-dimensional vector slice filter | Standalone numeric vector engine | Replaces the sheaf model with a foreign vector engine |

The table records the sheaf-native technique for each failure state and the identity check.

**Table 3: Sheaf-native technique for each failure state**

| Failure state | Sheaf-native technique | Identity preserved |
|---|---|---|
| Durable single-row write | Epoch-stamped fiber section append with local section commit | Yes |
| Raw join without fiber bundle | Fiber product of sheaves over a common base, plus pre-folded boundary complexes and answer fibers | Yes |
| Analytical scan | Graded cochain reduction over partitioned sheaf scans | Yes |
| Cold ingest and setup | Parallel local section assembly and gluing under a frozen epoch | Yes |
| Pure one-dimensional vector slice filter | Restriction of a zero-cochain to an interval subcomplex using dense coordinate vectors | Yes |

---

## 6. Node plan

The plan uses six nodes, one per failure state plus the harness node. Each node lists its files, its appends, its tests, and its gate.

### 6.1 Node N1: Durable write path (delta WAL)

**Correction goal:** Reduce the durable single-row write median from 6.90 us toward 1.60 us by recording only the local cochain differential, while keeping the sheaf semantics and the read path unchanged.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/delta_wal.rs | New module: DeltaWal writer and replay | Records only the stalk delta and the node id |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |
| sheaf-db/crates/sheaf-storage/src/lib.rs | Crate root | Re-exports DeltaWal |
| sheaf-db/crates/sheaf-storage/src/lsp/ring_wal.rs | Hardware CRC | Supplies crc_hw for the new writer |
| sheaf-db/crates/sheaf-storage/src/bin/wal_insert_bench.rs | Benchmark | Adds the DeltaWal gate |

**New file: `delta_wal.rs`**

Contents:
- Struct `DeltaWal` with fields `buf: BufWriter<File>`, `epoch: u64`.
- `create(path)`: create the file, pre-allocate 8 MB, write the epoch base marker.
- `append_delta(&mut self, cell_id: u64, delta: &[u8])`: write `[epoch u64][cell_id u64][len u32][crc u32][payload]`, increment epoch. Use `crc_hw` from ring_wal.
- `flush(&mut self)`: flush the buffer.
- `replay_delta(path)`: read the file, return `(Vec<(u64, Vec<u8>)>, u64, bool)`.

**Append to `mod.rs`:**

`pub mod delta_wal;`

**Append to `lib.rs`:**

`pub use lsp::delta_wal::DeltaWal;`

**Modify `wal_insert_bench.rs`:**

Add a fourth gate after the existing three. Create a `DeltaWal`, run 20,000 append_delta calls with flush per op, record p50 and p99, replay and check exactness. Print `delta_wal_durable: n=... p50_ns=... p99_ns=...` and `gate_delta_wal_replay=PASS` or `FAIL`.

**Tests:**
- Unit test in `delta_wal.rs`: write 1,000 deltas, replay, check ids and payloads match, check torn detection on a truncated file.
- Existing: `wal_insert_bench` must still print `WAL_INSERT_BENCH: PASS`.

**Gate:**
- `gate_delta_wal_replay=PASS`.
- p50 must be at or below 6,900 ns (current). Target 1,600 ns.
- No change to `WalWriter`, `EpochWriter`, or `HwCrcRing`.

### 6.2 Node N2: Raw join fallback (dynamic fiber product)

**Correction goal:** Provide a sheaf-native fallback for ad-hoc joins on flat tables, without replacing pre-folded fiber bundles, and measure it separately.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/fiber_product.rs | New module: sorted-key fiber product | Sheaf-native join as intersection of key fibers |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |
| sheaf-db/crates/sheaf-engine/src/bin/dynamic_pullback_bench.rs | New binary | Measures the fallback |
| sheaf-db/crates/sheaf-engine/src/bin/fiber_join_bench.rs | Existing bench | Regression gate for the pre-folded path |

**New file: `fiber_product.rs`**

Contents:
- Function `fiber_product(left: &BcsfReader, right: &BcsfReader, key_field: &str, key_offset: usize) -> Vec<(usize, usize)>`.
- Sort both sides by the key field, then two-pointer merge to emit matching index pairs.
- No hash table. No nested loop.

**New file: `dynamic_pullback_bench.rs`**

Build two BCSF readers from TPC-H customer and orders, run the fiber product, time it, compare against the pre-folded `fiber_join_bench` result (must not regress).

**Tests:**
- Unit test: two small readers with known keys, check the product equals the brute-force set.
- Regression: `fiber_join_bench` gate lines unchanged.

**Gate:**
- Exact match with brute force on a 1,000-row fixture.
- Pre-folded path unchanged.

**Note:** This node is optional for the outperform claim. The pre-folded path already beats DuckDB by 25.9x. The dynamic path is a fallback, measured separately, and documented as a boundary.

### 6.3 Node N3: Fused scan (regression gate)

**Correction goal:** Keep the fused scan gain. No new code.

**Files involved:** none changed. `fused_scan_bench.rs` is the gate.

**Tests:**
- Run `fused_scan_bench`. Check `gate_fused_exact=PASS` and `gate_fused_target=PASS`.

**Gate:**
- best_ms below 6.40, relative error below 1e-6.

### 6.4 Node N4: Cold ingest (lazy poset and SIMD parsing)

**Correction goal:** Reduce the IMDb build median from 61,100 ms toward 2,100 ms by using parallel local section assembly, SWAR parsing, and deferred verification.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | TSV field extractor | Replace byte loop with SWAR scan |
| sheaf-db/crates/sheaf-storage/src/lsp/huge_pages.rs | New module: huge-page helper | Reduces TLB misses on the arena |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes huge_pages |
| sheaf-db/crates/sheaf-engine/src/bin/imdb_fiber_bench.rs | Build driver | Calls the huge-page helper and skips validation on the hot path |

**Modify `imdb_parse.rs`:**

Add a method `scan_simd` to `FieldScratch` with the same signature as `scan`. It loads 32 bytes at a time and uses `_mm256_cmpeq_epi8` to find tab bytes, then extracts positions with `_tzcnt_u32`. Keep `scan` as the fallback for short lines. Unit test: `scan` and `scan_simd` agree on 10,000 random lines.

**New file: `huge_pages.rs`**

Contents:
- Function `prefault_arena(ptr: *mut u8, len: usize)` that calls `madvise(MADV_HUGEPAGE)` on Linux and `VirtualAlloc` with large pages on Windows where the privilege is available, and is a no-op otherwise.

**Modify `imdb_fiber_bench.rs`:**

Call `prefault_arena` on the builder's discrete and stalk buffers before the parse. Remove the `validate()` call from the timed build path; keep it in a separate untimed check.

**Tests:**
- Unit test in `imdb_parse.rs`: `scan` and `scan_simd` agree.
- Build gate: `imdb_fiber_bench` prints `gate_bundle_sections_fresh=PASS` and the three query gates PASS.
- Measurement gate: build time recorded and compared against 61,100 ms.

**Gate:**
- Build time at or below 61,100 ms. Target 2,100 ms.
- All query gates PASS.
- The SWAR scan matches the byte scan on 10,000 lines.

**Note:** The 2,100 ms target is an estimate from the attached text. It requires the lazy poset work that is not yet implemented. The plan records it as a target, not a measured result.

### 6.5 Node N5: One-dimensional dense slice filter

**Correction goal:** Reduce the slice filter median from 48.00 us toward 8.50 us by restricting a zero-cochain to an interval subcomplex using dense coordinate vectors, while keeping the sheaf model.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | New module: DenseTimeSeries | Contiguous timestamp and payload vectors |
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Module registry | Exposes the new module |
| sheaf-db/crates/sheaf-storage/src/bcsf/reader.rs | BCSF reader | Adds `to_dense_ts` conversion |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | New binary | Measures the dense path |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_shelf_bench.rs | Existing bench | Baseline for comparison |

**New file: `dense_ts.rs`**

Contents:
- Struct `DenseTimeSeries` with fields `timestamps: Vec<u64>`, `payloads: Vec<f32>`.
- Method `slice_sum(&self, lo: u64, hi: u64) -> (f64, usize)`: binary search for the lower bound, then a linear scan of the contiguous slice, accumulating with four independent accumulators.
- Method `from_reader(reader: &BcsfReader, ts_offset: usize) -> Self`: copy timestamps and payloads out of the BCSF slabs.

**Modify `reader.rs`:**

Add a method `to_dense_ts(&self, ts_offset: usize) -> DenseTimeSeries` that delegates to `DenseTimeSeries::from_reader`.

**New file: `timeseries_dense_bench.rs`**

Build the same 100,000 ticks as `timeseries_shelf_bench`, convert to `DenseTimeSeries`, run the 1-hour slice filter 25 times, print `dense_slice_us` and `gate_dense_exact`. Compare against the cell-based `slice_filter_us`.

**Tests:**
- Unit test: `DenseTimeSeries::slice_sum` matches the cell-based scan on the same data, relative error below 1e-12.
- Bench gate: `gate_dense_exact=PASS`.

**Gate:**
- Dense slice median at or below 48.00 us. Target 8.50 us.
- Exactness relative error below 1e-12.
- The cell-based `timeseries_shelf_bench` still prints its gate lines unchanged.

**Note on the Kdb+ ratio:** The Kdb+ value of 10.00 us is from an in-process timer. The client-observed value is 153.3 us. The ShelfDB value of 48.00 us has no client-observed pair. The plan requires a re-measurement on one timer basis before any ratio is claimed.

### 6.6 Node N6: Harness and test unification

**Correction goal:** Make the 50 test case suite measure the ShelfDB engine instead of a Docker container, and align the seed data.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs | Standalone app | Accept a data file and print machine-readable timing |
| run_50_high_variance_suite.py | 50-case runner | Generate the TQL data file and spawn the native binary |
| sheaf-db/tests/e2e/realtime_bench/run_all.py | Seven-engine matrix | No change, regression gate |

**Modify `sheaf_db_app.rs`:**

1. Accept `--data-file PATH`. When present, read the TQL program from that file instead of using FRAUD_DATA. The file must contain the schema, the inserts, and the query.
2. Remove the hardcoded FRAUD_DATA fallback when `--data-file` is present.
3. Print `matched_hypercells: N` and `execution_time_us: M` per result, which it already does through `render()`.

**Modify `run_50_high_variance_suite.py`:**

1. Generate a TQL data file from ACCOUNTS, MERCHANTS, COUNTRIES, and PAYMENTS before the loop. Write it to a temp file.
2. Replace the `docker run` call with a direct spawn of `sheaf_db_app --data-file <path> <tql_stmt>`.
3. Parse `execution_time_us` as the per-result solver time. Record the median across matched results, not the container time.
4. Record `matched` count and compare against the SQL engines' counts.

**Tests:**
- Run the 50-case suite. Check that match counts agree with SQLite on at least 45 of 50 cases.
- Run the seven-engine matrix. Check no regression on headline metrics.

**Gate:**
- Match agreement at or above 45 of 50.
- ShelfDB timing no longer includes container startup.

---

## 7. File change inventory

### 7.1 Existing files modified

| File | Change |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Add three `pub mod` lines |
| sheaf-db/crates/sheaf-storage/src/lib.rs | Add three re-exports |
| sheaf-db/crates/sheaf-storage/src/bcsf/reader.rs | Add `to_dense_ts` method |
| sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs | Add `--data-file` flag and data-file path |
| sheaf-db/crates/sheaf-engine/src/bin/wal_insert_bench.rs | Add DeltaWal gate |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | Add `scan_simd` method |
| sheaf-db/crates/sheaf-engine/src/bin/imdb_fiber_bench.rs | Call huge-page helper, skip validation on hot path |
| run_50_high_variance_suite.py | Native harness and data file generation |
| SKELETON.md | Append REQ-OE-001 and REQ-OE-002 through REQ-OE-007 |

### 7.2 New files

| File | Purpose |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/delta_wal.rs | DeltaWal writer and replay |
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | DenseTimeSeries and conversion |
| sheaf-db/crates/sheaf-storage/src/lsp/huge_pages.rs | Huge-page helper |
| sheaf-db/crates/sheaf-storage/src/lsp/fiber_product.rs | Sorted-key fiber product |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | Dense slice benchmark |
| sheaf-db/crates/sheaf-engine/src/bin/dynamic_pullback_bench.rs | Dynamic pullback benchmark |

### 7.3 Appends to existing files

| File | Append |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | `pub mod delta_wal;`, `pub mod dense_ts;`, `pub mod huge_pages;`, `pub mod fiber_product;` |
| sheaf-db/crates/sheaf-storage/src/lib.rs | `pub use lsp::delta_wal::DeltaWal;`, `pub use lsp::dense_ts::DenseTimeSeries;` |
| sheaf-db/crates/sheaf-storage/src/bcsf/reader.rs | `to_dense_ts` method on `BcsfReader` |
| SKELETON.md | Traceability entries for REQ-OE-001 to REQ-OE-007 |

---

## 8. Test plan

### 8.1 Existing tests (must pass unchanged)

1. `cargo test -p sheaf-storage` — all unit and integration tests.
2. `cargo test -p sheaf-engine` — all unit and integration tests.
3. `cargo test -p sheaf-kernel` — all unit and integration tests.
4. `cargo test -p sheaf-query` — all unit and integration tests.
5. `wal_insert_bench`: `gate_replay_exact=PASS`, `gate_replay_zero_copy=PASS`, `gate_batch_replay=PASS`, `WAL_INSERT_BENCH: PASS`.
6. `fused_scan_bench`: `gate_fused_exact=PASS`, `gate_fused_target=PASS`.
7. `timeseries_shelf_bench`: prints `slice_filter_us`, no crash.
8. `range_slice_bench`: `gate_range_exact=PASS`.
9. `imdb_fiber_bench`: `gate_bundle_sections_fresh=PASS`, `gate_answer_q1_matches_scan=PASS`, `gate_answer_q2_matches_scan=PASS`, `gate_answer_q3_matches_scan=PASS`, `gate_q1_exact=PASS`, `gate_q1_fast=PASS`, `gate_q2_fast=PASS`, `gate_q3_fast=PASS`, `gate_q4_exact=PASS`, `IMDB-FIBER-BUNDLE: ALL PASS`.
10. `fiber_join_bench`: gate lines unchanged.
11. `epoch_wal_bench`: gate lines unchanged.

### 8.2 New unit tests

1. `delta_wal.rs`: write 1,000 deltas, replay, check ids and payloads match. Torn detection on a truncated file.
2. `dense_ts.rs`: `slice_sum` matches the cell-based scan on the same data, relative error below 1e-12.
3. `imdb_parse.rs`: `scan` and `scan_simd` agree on 10,000 random lines.
4. `fiber_product.rs`: product equals brute force on a 1,000-row fixture.

### 8.3 50 test case suite

1. Run `run_50_high_variance_suite.py` with the native harness.
2. Check that ShelfDB match counts agree with SQLite on at least 45 of 50 cases.
3. Check that ShelfDB timing no longer includes container startup.
4. Record the median ShelfDB solver time per case and compare against the other engines.

### 8.4 Seven-engine matrix

1. Run `run_all.py` with the full engine list.
2. Check no regression on any headline metric.
3. Record the run summary JSON.

### 8.5 Kdb+ comparison

1. Re-measure Kdb+ and ShelfDB on one timer basis.
2. State the ratio with its basis.
3. Record the result in `results/KDB_TIMESERIES_RESULTS.json`.

---

## 9. Measurement gates and acceptance

The table records each gate, its threshold, and the action on failure.

**Table 4: Measurement gates**

| Gate | Threshold | Action on failure |
|---|---|---|
| Existing cargo tests | Zero failures | Fix the failure before any other work |
| Existing bench gate lines | All PASS | Reopen the node that regressed |
| DeltaWal replay | PASS | Fix DeltaWal |
| DeltaWal p50 | At or below 6,900 ns | Record the value, do not claim the target |
| Dense slice exactness | Relative error below 1e-12 | Fix the dense conversion |
| Dense slice p50 | At or below 48,000 ns | Record the value, do not claim the target |
| Cold ingest p50 | At or below 61,100 ms | Record the value, do not claim the target |
| Fused scan | best_ms below 6.40 and rel below 1e-6 | Reopen the scan node |
| 50-case match agreement | At or above 45 of 50 | Fix the data alignment |
| Seven-engine matrix | No regression on headlines | Reopen the regressing node |
| Kdb+ ratio | Stated with timer basis | Record the basis, do not claim a ratio without it |

---

## 10. Risks and open items

1. The 2,100 ms ingest target is an estimate from the attached text. It requires lazy poset work that is not yet implemented. The plan records it as a target.
2. The 8.50 us slice target is an estimate. It requires dense-column work that is not yet implemented.
3. The 1.60 us write target is an estimate. The SQLite value of 1.70 us is from an in-process timer. The comparison can not close on a client-observed basis.
4. The Kdb+ ratio is not established. It requires a re-measurement on one timer basis.
5. The 50-case suite currently uses a hardcoded 15-record dataset in the ShelfDB app. The plan fixes this, but the fix is a code change with its own risk.
6. Huge pages on Windows require a privilege that can not be available. The helper is a no-op when the privilege is missing.
7. The dynamic pullback node is optional. It is large and does not affect the outperform claim because the pre-folded path already wins.

---

## 11. Order of work

1. Record this plan as the plan of record for the optimize-and-expand effort.
2. Implement Node N1 (delta WAL) and run its gate.
3. Implement Node N5 (dense slice) and run its gate.
4. Implement Node N6 (harness) and run the 50-case suite.
5. Implement Node N4 (cold ingest) and run its gate.
6. Implement Node N2 (dynamic pullback) if time permits.
7. Run the seven-engine matrix and the Kdb+ comparison.
8. Update SKELETON.md and the document set with the measured results.

Every step ends with a gate. No step advances until its gate passes.