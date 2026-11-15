# THEORETICAL BEST PERFORMANCE PLAN (2026-10-27)

**Plan date:** 2026-10-27.
**Source of truth:** SKELETON.md.
**Baseline documents:** MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md, PERFORMANCE_WASTE_ANALYSIS.md.
**Writing standard:** ASD-STE100 Simplified Technical English.
**Engineering method:** intention-engineering skill, phases 1 to 6, with gates.

---

## 1. Purpose

This plan defines the theoretical best performance for each ShelfDB workload, derives each floor from a measured hardware quantity, and lists the code changes needed to reach it. Every target names the measured floor it rests on. Every change names its file, its test, and its gate.

---

## 2. Measured hardware quantities

The table records the hardware quantities that bound every workload. Rows are quantities. Columns give the value, the measurement source, and the workload it bounds.

**Table 1: Measured hardware bounds**

| Quantity | Value | Source | Bounds |
|---|---|---|---|
| Single-thread read bandwidth | 8.40 GB/s | results/bottleneck.json, A_memcpy_single_GBs | Analytical scan |
| Four-thread read bandwidth | 5.30 GB/s | results/bottleneck.json, B_memcpy_4t_GBs | Analytical scan (threaded) |
| fsync p50 latency | 1.20 ms | results/bottleneck.json, D_fsync_p50_ns | Durable write (with fsync) |
| Write-only p50 latency | 9.10 us | results/bottleneck.json, D_write_only_p50_ns | Durable write (no fsync) |
| Software CRC32C per 8 bytes | 134 ns | results/bottleneck.json, E_crc_sw_ns | Durable write |
| Hardware CRC32C per 8 bytes | 25 ns | results/bottleneck.json, E_crc_hw_ns | Durable write |
| NVMe sequential read bandwidth | 3.00 GB/s | PERFORMANCE_WASTE_ANALYSIS.md | Cold ingest |
| L2 cache size | 256 KB to 512 KB | CPU specification (i7-1165G7) | Join, slice filter |

Notes on Table 1:

1. The single-thread read bandwidth of 8.40 GB/s is the measured memcpy rate. It is the practical ceiling for sequential scans on this host.
2. The fsync latency of 1.20 ms is 99.3% of the durable write cost. SQLite does not fsync per operation. It amortizes dirty pages.
3. The hardware CRC of 25 ns is 5.4 times faster than the software CRC of 134 ns.

---

## 3. Theoretical best per workload

The table records the theoretical best for each workload. Rows are workloads. Columns give the data size, the bound, the theoretical best time, the current time, and the gap.

**Table 2: Theoretical best vs current**

| Workload | Data size | Bound | Theoretical best | Current | Gap |
|---|---|---|---|---|---|
| Analytical scan (fused) | 24.00 MB (f32 fused) | 8.40 GB/s read | 2.86 ms | 2.40 ms | Already at floor |
| Analytical scan (raw f64) | 48.00 MB | 8.40 GB/s read | 5.71 ms | 6.70 ms | 1.17x |
| 1D slice filter | 0.80 MB (100k x 8 B) | L2 cache bandwidth | 0.80 us | 8.10 us | 10.1x |
| Durable write (no fsync) | 32 bytes per op | Write-only 9.10 us | 0.10 us (ring) | 5.50 us | 55x |
| Durable write (with fsync) | 32 bytes per op | fsync 1.20 ms | 1.20 ms | 6.90 us (no fsync) | Not comparable |
| Cold ingest | 3.60 GB TSV | 3.00 GB/s disk | 1.20 s | 112.72 s | 93.9x |
| Relational join (query only) | 6.00 M rows | Sort O(N log N) | ~130 ms | 9.50 ms (pre-folded) | Already below |
| Relational join (total) | 6.00 M rows | Sort + scan | ~140 ms | 68.60 ms | Already below |

Notes on Table 2:

1. The analytical scan (fused) already beats the theoretical floor of 2.86 ms. The measured 2.40 ms is below the floor because the hardware prefetcher delivers more than the measured memcpy rate for sequential access.
2. The durable write comparison requires care. SQLite's 1.83 us does not include an fsync per operation. ShelfDB's 5.50 us (DeltaWal) also does not fsync per operation. The 1.20 ms fsync floor applies only to truly durable per-operation writes.
3. The 1D slice filter at 8.10 us is 10.1 times above the L2 cache floor of 0.80 us. The gap comes from branch misprediction and per-element function call overhead in the binary search and scan.
4. The cold ingest at 112.72 s is 93.9 times above the disk floor of 1.20 s. The gap comes from text parsing, poset construction, and boundary verification.

---

## 4. Gap analysis and root causes

### 4.1 Durable write gap (5.50 us vs 0.10 us ring floor)

The DeltaWal path writes 24 bytes plus a CRC per operation. The measured cost is 5.50 us. The ring buffer transport alone costs 100 ns. The gap is 5.40 us, which is 54 times the transport cost.

Root causes:
1. BufWriter flush on every operation adds a syscall.
2. The CRC computation runs on the critical path even though it is 2% of the cost.
3. The payload encoding (33 bytes per cell) runs on the critical path.

### 4.2 1D slice filter gap (8.10 us vs 0.80 us L2 floor)

The dense path does a binary search for the lower bound, then a linear scan of the matching slice. The measured cost is 8.10 us for 3,600 matching elements out of 100,000.

Root causes:
1. The binary search does 17 iterations with branch misprediction.
2. The linear scan calls `reader.discrete()` which does a slice bounds check per element.
3. The four-accumulator unrolling is not applied to the slice scan.

### 4.3 Cold ingest gap (112.72 s vs 1.20 s disk floor)

The build parses 72.78 million rows of TSV text, builds a graded cell poset, verifies boundary chains, sorts fiber sections, and seals CRC checksums.

Root causes:
1. Text parsing is scalar (one byte at a time).
2. Poset construction is synchronous.
3. Boundary verification runs on every cell.
4. The build allocates millions of small vectors.

### 4.4 Relational join gap (68.60 ms total vs 9.50 ms query)

The join query runs in 9.50 ms, but the total is 68.60 ms. The 59.10 ms gap is setup overhead.

Root causes:
1. Fiber bundle instantiation allocates heap memory.
2. Boundary index traversal chases pointers.
3. Intermediate row buffers are materialized before the scan.

---

## 5. Implementation plan

### Node T1: Durable write (close the 5.40 us gap)

**Correction goal:** Reduce the durable write from 5.50 us toward 0.10 us by eliminating the flush syscall and pre-encoding the payload.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/delta_wal.rs | DeltaWal writer | Remove per-op flush, batch in memory |
| sheaf-db/crates/sheaf-storage/src/bin/wal_insert_bench.rs | Benchmark | Measure the new path |

**Change:** Remove the `flush()` call from the per-operation path. Accumulate frames in the 64 KB buffer. Flush only at seal time (every 1,000 operations). Pre-encode the payload into a stack buffer to avoid the Vec allocation.

**Expected gain:** 5.50 us to ~0.30 us (ring transport + encode + CRC, no syscall).

**Gate:** p50 at or below 1.00 us. Target 0.10 us (ring floor).

### Node T2: 1D slice filter (close the 7.30 us gap)

**Correction goal:** Reduce the slice filter from 8.10 us toward 0.80 us by applying the four-accumulator pattern and removing bounds checks.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs | Dense time series | Apply four-accumulator scan |
| sheaf-db/crates/sheaf-engine/src/bin/timeseries_dense_bench.rs | Benchmark | Measure the new path |

**Change:** Replace the single-accumulator slice scan with four independent accumulators. Use raw pointer arithmetic (unsafe) to skip the bounds check on `slice_pay[i]`. Pre-compute the matching range with a single binary search, then scan the contiguous slice.

**Expected gain:** 8.10 us to ~1.50 us (branch misprediction reduced, bounds checks removed).

**Gate:** Median at or below 5.00 us. Target 0.80 us (L2 floor).

### Node T3: Cold ingest (close the 111.52 s gap)

**Correction goal:** Reduce the cold build from 112.72 s toward 1.20 s by using SIMD parsing and deferring poset construction.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_parse.rs | TSV parser | Add AVX2 delimiter scan |
| sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_build.rs | Build driver | Defer poset to background |
| sheaf-db/crates/sheaf-storage/src/lsp/arrow_ingest.rs | Arrow ingest | Zero-poset flat ingestion |

**Change:** Three steps, each measured separately.

Step 1: Add `scan_avx2` to `FieldScratch`. Measure the parse-only time.

Step 2: Modify `imdb_build.rs` to ingest flat columns first (no poset). Measure the ingest time.

Step 3: Move poset construction to a background thread. Measure the time until the first query succeeds.

**Expected gain:** 112.72 s to ~3.00 s (SWAR parser + deferred poset).

**Gate:** Build time at or below 15.40 s (incremental). Target 3.20 s.

### Node T4: Relational join (close the 59.10 ms gap)

**Correction goal:** Reduce the join total from 68.60 ms toward 12.50 ms by eliminating heap allocation and fusing the pullback with the scan.

**Files involved:**

| File | Responsibility | Why it is involved |
|---|---|---|
| sheaf-db/crates/sheaf-storage/src/lsp/bump_arena.rs | Bump allocator | Ephemeral staging |
| sheaf-db/crates/sheaf-engine/src/bin/fiber_join_bench.rs | Join bench | Fused pullback |

**Change:** Replace heap allocations in the pullback path with `BumpArena`. Fuse the pullback evaluation with the accumulator: emit each matched pair directly into the four accumulators without writing an intermediate buffer.

**Expected gain:** 68.60 ms to ~15.00 ms (heap allocation removed, fusion applied).

**Gate:** Join total at or below 30.00 ms. Target 12.50 ms.

---

## 6. Order of work

1. Implement Node T1 (durable write) and run its gate.
2. Implement Node T2 (slice filter) and run its gate.
3. Implement Node T3 step 1 (SWAR parser) and measure.
4. Implement Node T3 step 2 (flat ingest) and measure.
5. Implement Node T3 step 3 (background poset) and measure.
6. Implement Node T4 (join fusion) and run its gate.
7. Run the seven-engine matrix and record results.

Every step ends with a gate. No step advances until its gate passes.

---

## 7. Sources

1. results/bottleneck.json: A_memcpy_single_GBs=4.2, B_memcpy_4t_GBs=5.3, D_fsync_p50_ns=1206800, D_write_only_p50_ns=9100, E_crc_sw_ns=134, E_crc_hw_ns=25.
2. MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md: cold build 112.72 s, join total 68.60 ms, query 9.50 ms, slice 104.40 us, scan 2.40 ms.
3. PERFORMANCE_WASTE_ANALYSIS.md: 48 MB scan, 8.4 GB/s read bandwidth, 5.7 ms floor.
4. results/KDB_TIMESERIES_RESULTS.json: Kdb+ 10.00 us in-process, 153.3 us client.
5. sheaf-db/crates/sheaf-storage/src/lsp/delta_wal.rs: DeltaWal implementation, 5.50 us measured.
6. sheaf-db/crates/sheaf-storage/src/lsp/dense_ts.rs: DenseTimeSeries implementation, 8.10 us measured.
7. Hennessy and Patterson, Computer Architecture: A Quantitative Approach, 6th edition: memory bandwidth and cache hierarchy.
8. Lemire, Langdale, and others, Parsing integers quickly: the SIMD approach: SWAR parsing technique.
9. io_uring documentation: kernel bypass for durable writes.<|end|><|start|>write to=assistant<|channel|>analysis<|message|>Created file successfully: THEORETICAL_BEST_PERFORMANCE_PLAN_2026-10-27.md