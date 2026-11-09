# DETAILED_LOSS_ANALYSIS_2026-10-23.md - comprehensive technical analysis of ShelfDB performance deficits

**Document date:** 2026-10-23.
**Execution timestamp:** 2026-10-23 17:50:00 local (Asia/Kolkata, UTC+05:30), which is 2026-10-23 12:20:00 UTC.
**Branch:** `fiber-bundle-replacement`.
**Writing standard:** ASD-STE100 Simplified Technical English.

---

## 1. Concept definitions

- **Speedup ratio:** The opponent time divided by the ShelfDB time.
- **Latency reduction margin:** The percentage of execution time that ShelfDB saves.
- **Write-ahead log (WAL):** An append-only file that records database mutations before commit.
- **Fiber bundle section:** A contiguous memory buffer that stores typed column vectors.
- **SIMD (Single Instruction Multiple Data):** CPU instructions that process multiple data values simultaneously.

---

## 2. Summary of measured deficits

ShelfDB wins analytical queries on pre-indexed data. However, ShelfDB loses in four specific areas:
1. SQLite wins on durable single-point writes.
2. DuckDB wins on raw relational joins without fiber sections.
3. DuckDB wins on analytical scans when loop accumulators remain un-fused.
4. Relational and key-value databases win on initial cold ingest time.

The table below lists every measured loss from the live runs on 2026-10-23.

#### Table 1: Summary of measured ShelfDB losses
Each row shows one workload where ShelfDB lost. Each column shows measured times and performance deficits.

| Opponent database | Workload name | Dataset size | Opponent time | ShelfDB time | Deficit ratio | Latency margin (%) | Status |
|---|---|---|---|---|---|---|---|
| SQLite | Durable single-point write (p50) | 10,000 inserts | 1.70 µs | 6.90 µs | 4.06x slower | -305.88% | Permanent trade-off |
| DuckDB | Raw join without fiber bundle | 6M lineitem rows | 205.00 ms | 2,500.00 ms | 12.20x slower | -1119.51% | Architectural boundary |
| DuckDB | Analytical scan without fused loop | 6M lineitem rows | 5.60 ms | 7.60 ms | 1.36x slower | -35.71% | Solved by fused scan (4.0 ms) |
| DuckDB / Redis | Initial cold build and ingest | 3.6 GB IMDb | 0.10 s to 22.60 s | 51.10 s to 112.72 s | Slower ingest | -126.11% to -400.00% | Structural build trade-off |

---

## 3. Deep technical analysis of each loss

### 3.1 Loss 1: Durable single-point writes (Lost to SQLite)

#### The measurement
In our live benchmark run on 2026-10-23:
- SQLite completed single-row durable writes in **1.70 µs** at median (p50).
- ShelfDB completed single-row durable writes in **6.90 µs** at median (p50).
- SQLite ran **4.06 times faster** than ShelfDB. ShelfDB exhibited a **-305.88% latency margin**.

#### Architectural root cause
1. **SQLite single-process page design:**
   SQLite writes directly to memory-mapped database pages. It uses a single process mutex. It does not validate complex multi-dimensional cell topologies.
2. **ShelfDB write-ahead log overhead:**
   ShelfDB must record cell descriptors, dual stalks (discrete attributes plus continuous embedding vectors), and boundary incidence records.
   For every write-ahead log frame, ShelfDB performs three sequential operations:
   - It formats an append-only frame header with sequence numbers and payload lengths.
   - It computes an explicit CRC32C checksum over the entire frame.
   - It synchronizes the file buffer to persistent storage.

#### Engineering verdict
This deficit represents an intentional architectural trade-off. ShelfDB guarantees topological structure integrity and continuous vector embeddings. SQLite targets minimal single-record transactional overhead. ShelfDB will not beat SQLite on single-row scalar writes.

---

### 3.2 Loss 2: Raw relational joins without fiber sections (Lost to DuckDB)

#### The measurement
In our live benchmark run on 2026-10-23:
- DuckDB completed the join across 6,001,215 lineitem rows in **205.00 ms** at median (and 158.30 ms in quiet runs).
- ShelfDB un-indexed raw join completed in **2,500.00 ms**.
- DuckDB ran **12.20 to 15.80 times faster** than ShelfDB. ShelfDB exhibited a **-1119.51% latency margin**.

#### Architectural root cause
1. **DuckDB dynamic hash-join engine:**
   DuckDB builds vectorized radix hash tables in parallel across all CPU cores. It sizes hash tables dynamically using data statistics. It probes memory with SIMD vector instructions.
2. **ShelfDB reliance on topological indexing:**
   ShelfDB does not contain a general-purpose dynamic relational hash-join builder.
   When ShelfDB executes with pre-folded fiber bundle sections (`fiber_join_bench`), it completes in **3.20 ms to 7.90 ms** (beating DuckDB by 25.9x).
   However, if a query forces ShelfDB to join un-indexed flat tables, ShelfDB falls back to generic relational scan loops.

#### Engineering verdict
ShelfDB is a topological sheaf database. It achieves high execution speed by compiling joins into pre-materialized boundary sections. On ad-hoc raw flat tables without topological indexes, DuckDB remains the superior engine.

---

### 3.3 Loss 3: Analytical scans without accumulator fusion (Lost to DuckDB)

#### The measurement
In our live benchmark run on 2026-10-23:
- DuckDB completed analytical scans and sum aggregations over 6M rows in **5.60 ms** (p50).
- ShelfDB baseline un-fused scan completed in **7.60 ms**.
- DuckDB ran **1.36 times faster** than ShelfDB. ShelfDB exhibited a **-35.71% latency margin**.

#### Architectural root cause
1. **Instruction dependency stalls:**
   In the baseline BCSF reader loop, each iteration adds the column value to a single CPU register accumulator.
   The CPU pipeline must wait for the addition to complete before executing the next addition.
   This dependency chain creates pipeline stalls on modern out-of-order processors.
2. **DuckDB chunk vectorization:**
   DuckDB evaluates chunks of 2,048 values using AVX2 instructions that execute multiple additions per CPU cycle.

#### Engineering remedy and verification
We resolved this loss in [`sheaf-db/crates/sheaf-engine/src/bin/fused_scan_bench.rs`](sheaf-db/crates/sheaf-engine/src/bin/fused_scan_bench.rs).
By unrolling the scan loop into four independent register accumulators (`acc0`, `acc1`, `acc2`, `acc3`), ShelfDB eliminates instruction stalls.
With loop fusion, ShelfDB completes the identical scan in **4.00 ms**, which is **1.40 times faster than DuckDB**.

---

### 3.4 Loss 4: Initial full build and ingest time

#### The measurement
In our live benchmark run on 2026-10-23:
- SQLite loaded its tables in **1.91 s**.
- Redis loaded 1,000,000 keys in **22.60 s**.
- DuckDB loaded in **0.10 s to 156.00 s**.
- ShelfDB required **51.10 s to 112.72 s** to build the complete fiber bundle for the 3.6 GB IMDb dataset.

#### Architectural root cause
1. **Relational ingestion simplicity:**
   Relational databases parse text lines and append binary records directly into page buffers or column vectors.
2. **ShelfDB topological construction complexity:**
   During ingest, ShelfDB performs heavy mathematical operations:
   - It constructs a graded cell poset from raw entity records.
   - It builds dual-stalk descriptors (discrete fields plus Float32 continuous stalks).
   - It validates boundary incidence chains to guarantee that $\delta^2 = 0$.
   - It sorts and pre-indexes topological fiber sections.
   - It computes CRC32C checksums over every section buffer.

#### Engineering verdict
The longer build time pays for the fast query execution time. The pre-folded bundle allows ShelfDB to answer IMDb Q1 in 0.019 ms and Q2 in 0.009 ms. Applications that demand sub-second real-time bulk ingest must use standard relational or key-value stores.

---

## 4. Architectural Summary

ShelfDB has clear boundaries where it wins and where it loses:
- **Where ShelfDB wins:** Topological graph traversals, pre-indexed fiber joins, in-memory point lookups, and wide contiguous range reads.
- **Where ShelfDB loses:** Single-row durable write latency, raw ad-hoc relational joins, and initial whole-complex build latency.

---

## 5. Verification evidence

All measurements in this document come from live executions on 2026-10-23:
- SQLite measurements: [`sheaf-db/tests/e2e/realtime_bench/results/sqlite.json`](sheaf-db/tests/e2e/realtime_bench/results/sqlite.json)
- DuckDB measurements: [`sheaf-db/tests/e2e/realtime_bench/results/duckdb.json`](sheaf-db/tests/e2e/realtime_bench/results/duckdb.json)
- Redis measurements: [`sheaf-db/tests/e2e/realtime_bench/results/redis.json`](sheaf-db/tests/e2e/realtime_bench/results/redis.json)
- Run summary: [`sheaf-db/tests/e2e/realtime_bench/results/RUN_SUMMARY_20261023_1735.json`](sheaf-db/tests/e2e/realtime_bench/results/RUN_SUMMARY_20261023_1735.json)
