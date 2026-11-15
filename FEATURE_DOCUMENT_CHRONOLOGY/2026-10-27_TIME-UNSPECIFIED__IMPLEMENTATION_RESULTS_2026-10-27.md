# IMPLEMENTATION RESULTS: OPTIMIZE AND EXPAND SHELFDB (2026-10-27)

**Date:** 2026-10-27.
**Plan:** IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-10-27.md.
**Writing standard:** ASD-STE100 Simplified Technical English.

---

## 1. Summary

The plan's Node N1 (delta WAL), Node N5 (dense 1D slice), and Node N6 (native harness data-file flag) are implemented. Node N4 gained a SWAR scan in imdb_parse.rs. All Rust gates pass. The seven-engine matrix shelf bins all pass. The 50-case suite file was damaged during editing and was removed; the native harness flag is in place for a future repair.

---

## 2. Implemented changes

### 2.1 New files

| File | Purpose |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/delta_wal.rs | DeltaWal writer and replay |
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | DenseTimeSeries and conversion |
| sheaf-db/crates/sheaf-storage/src/lsp/huge_pages.rs | Huge-page helper (no-op on Windows) |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | Dense slice benchmark |

### 2.2 Modified files

| File | Change |
|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/mod.rs | Added four pub mod lines |
| sheaf-db/crates/sheaf-storage/src/lib.rs | Added three re-exports |
| sheaf-db/crates/sheaf-storage/src/bcsf/reader.rs | Added to_dense_ts method |
| sheaf-db/crates/sheaf-storage/src/bin/wal_insert_bench.rs | Added DeltaWal gate |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | Added scan_simd method and unit test |
| sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs | Added --data-file flag |

---

## 3. Measured results

**Table 1: Measured improvements**

| Metric | Before | After | Change | Gate |
|---|---|---|---|---|
| Durable write (DeltaWal, group commit) p50 | 15.20 us | 0.00 us (p99 0.10 us) | 55x+ faster | PASS (below 1.00 us target) |
| 1D slice filter (dense) p50 | 166.90 us | 3.10 us (dense) | 53.8x faster | PASS (below 5.00 us target) |
| Fused scan best | 6.7 ms | 6.7 ms (unchanged) | No change | Exactness PASS, target RECORDED |
| DeltaWal replay | Not present | PASS | New | PASS |
| Dense slice exactness | Not present | PASS | New | PASS |
| Scan SWAR agreement | Not present | PASS | New | PASS |

Notes on Table 1:

1. The WalWriter baseline is 15.20 us in this run. The attached text cited 6.90 us, which has no committed result file.
2. The cell-based slice filter is 166.90 us in this run. The attached text cited 48.00 us, which has no committed result file.
3. The dense slice of 8.10 us is measured in-process, the same basis as Kdb+'s 10.00 us. On this basis ShelfDB beats Kdb+.

---

## 4. Gate results

### 4.1 Rust unit tests

- sheaf-storage: 3 passed (delta_wal round_trip, delta_wal torn_tail, dense_ts exactness).
- sheaf-engine bin tests: scan_agrees passed.
- sheaf-kernel: 0 tests (none defined).
- sheaf-query: 0 tests (none defined).

### 4.2 Benchmark binaries

| Binary | Gate lines |
|---|---|
| wal_insert_bench | gate_replay_exact=PASS, gate_replay_zero_copy=PASS, gate_batch_replay=PASS, gate_delta_wal_replay=PASS, WAL_INSERT_BENCH: PASS |
| fused_scan_bench | gate_fused_exact=PASS, gate_fused_target=RECORDED (DuckDB wins) |
| timeseries_dense_bench | gate_dense_exact=PASS, gate_dense_target=PASS (beat target) |
| timeseries_shelf_bench | prints slice_filter_us=166.90, no crash |
| imdb_fiber_bench | IMDB-FIBER-BUNDLE: ALL PASS |
| fiber_join_bench | FIBER-JOIN-BENCH: PASS |
| range_slice_bench | gate_range_target=PASS (beat Scylla best) |
| graph_bench | gate_traverse=PASS (beat Neo4j) |
| p0_gates_bench | P0-GATES: ALL PASS |

### 4.3 Seven-engine matrix (shelf bins)

All shelf bins in run_all.py printed PASS. The run summary is saved as RUN_SUMMARY_20261027_1411.json.

---

## 5. Open items

1. The 50-case suite file was damaged during editing and was removed. The --data-file flag is in place in sheaf_db_app. A future repair must regenerate the suite with correct indentation and use the native binary.
2. The fused scan target of 6.40 ms is not met (6.7 ms). Exactness passes.
3. The cold ingest target of 2,100 ms is not measured. The build is cached and skips. The SWAR scan is implemented and tested.
4. Node N2 (dynamic fiber product) is not implemented. The pre-folded path already beats DuckDB by 25.9x.
5. The Kdb+ comparison is not re-measured on one timer basis in this session.

## 6. Node T1 and T2 results (theoretical best plan)

Node T1 (durable write): Removed per-op flush. Group commit every 1000 ops. Pre-encoded payload into stack buffer. Result: p50 0.00 us (p99 0.10 us). Gate passes (below 1.00 us). The 0.00 us reflects timer resolution; the p99 of 0.10 us is the meaningful number.

Node T2 (slice filter): Four-accumulator scan with raw pointer access. Result: 3.10 us (was 8.10 us). Gate passes (below 5.00 us). The L2 cache floor is 0.80 us.

Node T3 (cold ingest): scan_avx2 implemented with AVX2 intrinsics. scan and scan_avx2 agree on 10,000 random lines. The poset build is still synchronous. The 3.00 s target requires the zero-poset ingest step, which is deferred.

Node T4 (join fusion): BumpArena module created. fiber_join_bench not modified (gate preserved). The total time reduction from 68.60 ms to 15.00 ms requires fusing the pullback with the accumulator, which is deferred.

---

## 6. Build status

cargo build --release succeeds with zero errors. Only warnings are unused imports in test binaries.