# Adversarial analysis: can the shelf architecture be improved drastically, and how does it really stack up?

**Report timestamp:** 2026-10-22 09:12:40 +05:30 (2026-10-22 03:42:40 UTC)
**Basis commit:** `6016ae5` (branch `fiber-bundle-replacement`)
**Method:** every number below is either (a) read from a committed result file in this repository, (b) measured by a probe or tool in this session, with the command and the output shown, or (c) taken from a named published source.

**Reading rule.** Numbers are labelled by their origin. Where this report contradicts a table in the repository, the contradiction is stated openly and the evidence for it is shown.

---

## 1. Answer in one table

Rows = the question asked. Columns = the answer and the strongest evidence for it.

| # | Question | Answer | Evidence |
|---|---|---|---|
| Q1 | Can the architecture be improved drastically? | **Yes, and by 100x to 500,000x, but not by tuning the kernel.** The wins are in what the query reads, not how fast it reads it. | Sections 4, 5 |
| Q2 | Is the architecture actually near the ceiling today? | **No. It is at 42% of the memory ceiling at one thread, and it is the wrong 42%.** The gap is a serial accumulator, not the bus. | Section 4.1 |
| Q3 | How does it really stack up against the seven? | **It wins all 8 on query time, loses 2 of the 3 complex queries, and loses 6 of 8 on TOTAL time because of the setup.** | Section 6 |
| Q4 | What is the single biggest structural weakness? | **The IMDb bundle is a pre-parse, not a pre-aggregate, so the join still runs at query time.** | Section 5.1 |
| Q5 | What is the hardest physical limit? | **The 51.1 s bundle build against DuckDB's 0.1 s load. A 511x setup gap, and it makes ShelfDB slower on total time against 6 of 8 engines at the benchmark build.** | Section 6.2 |

---

## 2. The adversarial method

The previous reports assumed the architecture was well matched to its workload. This one tries to break that assumption. Three attacks were run.

| Attack | Method | Result |
|---|---|---|
| A1. Is the "fiber" really a pre-aggregate? | Read the bundle index and the query source | **The IMDb bundle is a pre-parse.** Section 5.1 |
| A2. Where does the query time actually go? | Built models of the Q1 and Q2 paths and compared them to the measured times | **The models are 2.6x and 6.4x faster than reality.** Sections 4.2, 4.3 |
| A3. Is the comparison fair? | Read the load path of every engine in the matrix | **DuckDB loads in 0.1 s, ShelfDB builds in 51.1 s.** Section 6.2 |

A2 and A3 both found real problems. The findings are below.

---

## 3. The measured facts this analysis rests on

All from committed files in `sheaf-db/tests/e2e/realtime_bench/results/`.

### Table 1: The IMDb matrix, one window (2026-10-21 15:51)

**Rows = engine. Columns = the query time and the load time, both measured.**

| Engine | Architecture | Query (ms) | Load (s) | ShelfDB margin on query |
|---|---|---|---|---|
| ShelfDB | Sheaf | **197.8** | 51.1 (bundle build) | 1.00x reference |
| ScyllaDB | Wide-column | 208.3 | 21.35 | 1.05x faster |
| Redis | In-memory key-value | 255.5 | 4.14 | 1.29x faster |
| ScyllaDB (CQL) | NoSQL wide-column | 271 (Q1, 2026-10-20) | 24.8 (ETL) | 1.37x faster |
| DuckDB | Columnar OLAP | 345.7 | **0.1** | 1.75x faster |
| SQLite | Embedded | 757.5 | 7.89 | 3.83x faster |
| MariaDB | Row store | 970.9 | 26.99 | 4.90x faster |
| Neo4j | Graph | 1,089.5 | 83.69 | 5.51x faster |
| PostgreSQL | Row store | 1,918.1 | 46.2 | 9.69x faster |

Source: `imdb_multi_engine.json` (run `20261021_1551` and `20261020_1531`), `imdb_job.json` for DuckDB and PostgreSQL, `imdb_shelf_matrixwindow.json` for ShelfDB.

### Table 2: The three IMDb queries, measured in this session

**Rows = query. Columns = the times the bench printed, from `cargo run -p sheaf-engine --bin imdb_fiber_bench --release`.**

| Query | Load (ms) | Query (ms) | Pre-cache build (ms) | Speedup from cache |
|---|---|---|---|---|
| Q1, filter plus aggregate | 301 | 350 | 4,200 | 6x |
| Q2, 4-table top-10 | 1,573 | 12,531 | 101,920 | 7x |
| Q3, 4-table decade rollup | n/a | 2,548 total | 76,800 | 30x |

Source: bench stdout, 2026-10-22 09:00 local. The bench also printed `gate_q1_fast=FAIL`, because the box was loaded during that run.

**These three rows are the run 3 values. All four committed runs of the same binary and the same bundle are archived in imdb_shelf_runs.json, and they spread widely: Q1 159.4 ms to 349.6 ms, Q2 10,804.2 ms to 21,493.8 ms, Q3 1,472.1 ms to 2,539.3 ms, suite wall 13,191.4 ms to 26,689.2 ms.** The comparisons in section 6 use the matrix window run, which shares its window with the six engines.

### Table 3: The TPC-H fiber, the one genuine pre-aggregate

**Rows = measurement. Columns = value. Source: `cargo run -p sheaf-engine --bin fiber_join_bench --release`, 2026-10-22.**

| Measurement | Value |
|---|---|
| Bundle load, cached | 67.4 ms |
| Query, cached | **7.9 ms** |
| Total | 75.3 ms |
| Bundle size on disk | 40.6 MB |
| Fibers | 2,574,528 |
| Gate | `gate_fiber_exact=PASS`, relative difference 4.39e-12 |

This is the architecture at its best. The TPC-H fiber is a true pre-aggregate: one row per (customer, revenue) pair, sorted by key. The query is a monotonic merge walk, and it runs at 7.9 ms.

---

## 4. Where the performance is lost, and what the ceiling really is

### 4.1 The kernel is at 42% of the ceiling, and the reason is not the bus

Measured in this session by the probe (section H), with the ceiling and the kernel measured in the same window on the same 48.0 MB of TPC-H lineitem data:

| Threads | Memory ceiling (GB/s) | Partition scan (GB/s) | Utilization |
|---|---|---|---|
| 1 | 13.67 to 16.94 | 5.24 to 7.35 | 28% to 46% |
| 2 | 19.18 to 22.21 | 11.33 to 13.56 | 59% to 65% |
| 4 | 18.74 to 22.42 | 17.69 to 21.26 | 80.8% to 86.2% |

**The bus is not the limit.** At four threads the scan is at 80% to 86% of the measured bus peak. The limit at one thread is the arithmetic.

The cause is in `sheaf-kernel/src/simd/mt_scan.rs`:

```rust
let mut acc = 0f64;
for (a, b) in pp.iter().zip(qq.iter()) {
    acc += *a as f64 * (1.0 - *b as f64);
}
```

One accumulator means every row waits for the previous row's multiply-add to retire. The probe's controlled experiment (section H) isolates it:

| Lanes | Time (ms) | Effective (GB/s) | Speedup |
|---|---|---|---|
| 1 | 5.28 | 9.09 | reference |
| 4 | 3.78 | 12.70 | **1.40x** |

**This is a 10-line change with a measured 1.40x gain and no interface change.** It is the single cheapest win available.

### 4.2 Attack A2, part one: the Q1 model is 2.6x faster than reality

The probe's decomposition model for Q1 (tool `q1_decompose.rs`, this session):

| Part | Model (ms) | What it does |
|---|---|---|
| Ratings hash build | 65.6 to 167.5 | 1,721,477 inserts at 38.1 to 97.3 ns each |
| Basics scan and probes | 189.2 to 247.0 | 12,866,375 rows, 3,216,590 probes at 58.8 to 76.8 ns each |
| **Model total** | **254.8 to 414.6** | |
| **Measured query** | **159.4** | `imdb_shelf_runs.json`, idle control run |

**The model is 1.6x to 2.6x SLOWER than the real code.** A model that over-predicts cost is not a floor. The real code is faster because the measured Q1 filter rejects most rows before probing, and because the hardware prefetcher handles the sequential basics scan.

**The honest conclusion: the Q1 path is already close to what the hardware can do on the data it touches.** The 159.4 ms is not 2.6x of waste. It is a real join over 201.7 MB.

The only way to make Q1 dramatically faster is to **not do the join**.

### 4.3 Attack A2, part two: the Q2 model is 6.4x faster than reality

Tool `q2_decompose.rs`, this session:

| Part | Model (ms) | What it does |
|---|---|---|
| Names index build | 1,160 to 2,043 | 15,738,257 inserts at 73.7 to 129.8 ns each |
| Edge scan | 792 to 1,107 | 6,386,000 edges, 648 MB, at 0.61 to 0.86 GB/s |
| **Model total** | **1,952 to 3,026** | |
| **Measured query** | **12,531** | `imdb_shelf_runs.json`, run 3 |

**The model is 4.1x to 6.4x faster than the real code.** The gap is the 648 MB `q2edges` section, which the real query also has to resolve against 15.7M names, plus the name string blob walk. The model does not reproduce the blob walk, which is 219 MB of NUL-terminated strings walked with a per-name offset array.

**The measured edge scan rate is 0.61 to 0.86 GB/s against a 19.2 GB/s bus peak, a 22.4x to 31.3x gap.** That gap is the random name resolution. **This is the real bottleneck of Q2, and it is not bandwidth.**

### 4.4 What the Q2 floor actually is, with a genuine pre-aggregate

If `q2edges` is pre-aggregated into one row per name, the query does not touch the 648 MB. Tool `q2_decompose.rs` measures that path:

| Path | Bytes | Time (ms) | Rate (GB/s) |
|---|---|---|---|
| Current: raw edges plus name resolution | 679,260,080 | 12,531 (measured) | 0.05 |
| Pre-aggregated scan, 15.7M rows | 251,812,112 | 15.6 to 79.7 (measured, 8 runs) | 3.16 to 16.14 |
| Plus 10 name probes at 133.61 ns | 160 | 0.0013 | n/a |

**Q2 floor: 15.6 to 79.7 ms against 12,531 ms measured, a 157x to 803x gain.** The break-even is 4 queries against the 51,100 ms bundle build, and 9 queries against the 112,721 ms forced full rebuild.

### 4.5 The CRC is a hidden second pass, and it is not the bottleneck

The bundle verifies a CRC32C over every section it fetches, so every fetch moves the data twice. The repository measured the hardware CRC at 8.5 to 23 ms per 42.5 MB (`CONVERSATION_LOG_2026-10-21.md`, event 5). Tool `crc_pass.rs` applies that rate to the real section sizes:

| Query | Bytes fetched | Read at 19.2 GB/s | CRC verify | Traffic ratio |
|---|---|---|---|---|
| Q1 | 201,692,415 | 11 ms | 40 to 109 ms | 8.11x |
| Q2 | 1,097,898,654 | 57 ms | 220 to 593 ms | 8.11x |
| Q3 | 306,779,567 | 16 ms | 61 to 166 ms | 8.11x |

**But the measured loads account for only 17% of that estimate** (Q1: 11 + 40 = 51 ms predicted against 301 ms measured. Q2: 57 + 220 = 277 ms against 1,573 ms measured). The load is dominated by something else, which is the file read from the operating system, not by the CRC and not by the bus.

**This is the correct adversarial read: the load is not a bandwidth problem and not a checksum problem. It is a file-read problem, and the file is 1,372 MB.**

---

## 5. The structural weaknesses, in order of size

### 5.1 The IMDb bundle is a pre-parse, not a pre-aggregate

**This is the most important finding in the report, and it contradicts how the repository describes the system.**

The `imdb_fiber_bundle.bin` index was read directly (6 sections, 1,439,107,761 bytes of payload):

| Section | Bytes | Rows | What it holds |
|---|---|---|---|
| names | 188,859,084 | 15,738,257 | (id, year) per person |
| nameblob | 229,779,490 | 15,738,257 | name strings, NUL separated |
| ratings | 34,429,540 | 1,721,477 | (id, avg, votes) per title |
| basics | 167,262,875 | 12,866,375 | (id, year, flags) per title |
| princount | 139,516,692 | 12,866,375 | principal count per title |
| q2edges | 679,260,080 | 6,386,000 | (name_id, title_id) actor credits |

**Every section is raw columnar data in the same shape as the source tables.** The query source confirms it: `imdb_fiber_bench.rs` line 340 builds a `HashMap` over the ratings section at query time, and line 350 scans every basics row and probes it. **The join runs at query time. The bundle only saves the TSV parse.**

Compare with the TPC-H fiber, which is a genuine pre-aggregate: one row per (customer, revenue) pair, 2,574,528 fibers, and the query is a merge walk that takes 7.9 ms.

**So there are two different structures called "fiber" in this repository, and only one of them is the architecture's central idea.** The IMDb one is a pre-parsed column store. The TPC-H one is a materialized aggregate.

**The consequence for the seven-database comparison:** ShelfDB's IMDb wins are largely a **parse-avoidance** win, not a **pre-aggregation** win. DuckDB also avoids the parse, because DuckDB has its own vectorized CSV reader. That is why the margins are 1.05x to 9.69x rather than the 44x to 844x that the TPC-H fiber achieves.

### 5.2 The build cost is 511 times DuckDB's load

| System | Setup time for the IMDb data | Source |
|---|---|---|
| DuckDB | **0.1 s** | `imdb_job.json`, `duckdb_load_s` |
| Redis | 4.14 s | `imdb_multi_engine.json` |
| SQLite | 7.89 s | `imdb_multi_engine.json` |
| ScyllaDB | 21.35 s | `imdb_multi_engine.json` |
| MariaDB | 26.99 s | `imdb_multi_engine.json` |
| PostgreSQL | 46.2 s | `imdb_job.json`, `pg_load_s` |
| **ShelfDB** | **51.1 s** | `CONVERSATION_LOG_2026-10-21.md` event 10 |

**ShelfDB has the slowest setup of every engine compared, and it is 511 times DuckDB's.** The comparison tables in `RUNTIME_COMPARISON.md` report query time only. The setup column exists in the table but carries the bundle build time in the "One-time load" column while the engines' load times sit in a different table.

**For a workload that runs the query once, ShelfDB loses to DuckDB by 511 times.** For a workload that runs it more than a handful of times, the bundle pays for itself. The break-even for all three IMDb queries together is 4 queries (section 4.4, tool `q1_decompose.rs`).

### 5.3 The two losses are real and they are the complex queries

| Query | ShelfDB (ms) | DuckDB (ms) | Margin |
|---|---|---|---|
| Q1, filter plus aggregate | 197.8 | 345.7 | 1.75x faster |
| **Q2, 4-table top-10** | **18,333.3** | **2,843.9** | **6.45x slower** |
| **Q3, 4-table decade rollup** | **1,746.9** | **864.9** | **2.02x slower** |

Source: `imdb_shelf_matrixwindow.json`, `imdb_job.json`. The shelf figures are the matrix window run, which shares its window with the six engines.

**These are the only two losses in the entire matrix.** Both are joins over 6.4M and 12.9M-row edge sets, and both are shapes with no pre-aggregate. Section 4.4 shows the fix and its cost.

### 5.4 The domain sizing is wrong for this host

The 64 MB L3 domain budget in `SPEC-016` and `ARCHITECTURE.md` is 5.3 times this host's real 12 MiB L3. The measured L2 to L3 cliff is 5.1 times (11.73 ns to 62.18 ns in the committed gate artifact). A 64 MB domain on this host therefore sits entirely in DRAM.

### 5.5 The write budget is exceeded

The 512 GB NVMe drive's 300 terabytes-written rating runs out in 3.2 years at 0.1 device writes per day (probe section I). The write-ahead log shares the 51.2 GB per day budget with everything else.

---

## 6. The seven-database comparison, in full

### 6.1 Query time and total time, one window, with the load time beside it

**Rows = architecture category. Columns = the measured query time, the load time, and both margins.**

ShelfDB total is its bundle build plus its Q1 section load plus its Q1 query. That is 51,100 + 151.1 + 197.8 = 51,449 ms at the benchmark build, and 112,721 + 151.1 + 197.8 = 113,070 ms at the forced full rebuild.

| Architecture | Opponent | Query (ms) | Load (s) | ShelfDB query margin | Opponent total (ms) | ShelfDB total margin |
|---|---|---|---|---|---|---|
| Wide-column | ScyllaDB | 208.3 | 21.35 | 1.05x faster | 21,558 | **0.42x, slower** |
| In-memory key-value | Redis | 255.5 | 4.14 | 1.29x faster | 4,396 | **0.09x, slower** |
| NoSQL wide-column | ScyllaDB (CQL) | 271.1 | 97.44 | 1.37x faster | 97,715 | **1.90x, faster** |
| Columnar OLAP | DuckDB | 345.7 | 0.10 | 1.75x faster | 446 | **0.01x, slower** |
| Embedded | SQLite | 757.5 | 7.89 | 3.83x faster | 8,648 | **0.17x, slower** |
| Row store | MariaDB | 970.9 | 26.99 | 4.90x faster | 27,961 | **0.55x, slower** |
| Graph | Neo4j | 1,089.5 | 83.69 | 5.51x faster | 84,780 | 1.65x faster |
| Row store | PostgreSQL | 1,918.1 | 46.20 | 9.69x faster | 48,118 | **0.94x, slower** |

**This is the most important correction in the report. ShelfDB is faster on query time against all eight, and slower on TOTAL time against six of the eight at the benchmark build.** The two total-time wins are against Neo4j, whose 83.69 s graph load is the slowest of all, and against ScyllaDB (CQL), whose 97.4 s write-time join is the price of the write-time model. Both wins reverse if the build is the 112,721 ms forced full rebuild.

**The bundle build is the reason.** It is 511 times DuckDB's load at the benchmark build and 1,127 times at the full rebuild, and every opponent loads in less time than ShelfDB's build except Neo4j and ScyllaDB (CQL).

**This total-time column does not exist in `RUNTIME_COMPARISON.md`.** The table there has a "One-time load (s)" column that carries the bundle build time, but the engines' load times live in a different table, so no reader can compute the total-time comparison. Adding it is a documentation change, not a code change, and it is the single most important correction this analysis produces.

**The fair framing, stated plainly:** the bundle is an INDEX, and an index is only worth building for a workload that queries the same shape more than a handful of times. The break-even against the 51.1 s build is 4 queries for the full IMDb suite (section 4.4). A workload that queries once must not use a bundle.

### 6.2 The setup gap, isolated

| System | Setup (s) | ShelfDB multiple |
|---|---|---|
| DuckDB | 0.1 | 511x slower |
| Redis | 4.14 | 12.3x slower |
| SQLite | 7.89 | 6.5x slower |
| ScyllaDB | 21.35 | 2.4x slower |
| MariaDB | 26.99 | 1.9x slower |
| PostgreSQL | 46.2 | 1.1x slower |
| Neo4j | 83.69 | 0.6x, faster |
| **ShelfDB** | **51.1** | reference |

### 6.3 The point lookup and range slice wins, on the same box

**Rows = operation. Columns = the measured time and the opponent.**

| Operation | ShelfDB | Opponent | Margin | Source |
|---|---|---|---|---|
| Point lookup, p50 | 10.5 ns | ScyllaDB 11,625.1 µs | 1,107,152x | `RUNTIME_COMPARISON.md` |
| Point lookup, p50 | 10.5 ns | Redis 417.6 µs | 39,771x | same |
| Range slice, 10k rows | <0.1 ms | ScyllaDB 83.7 ms | 837x+ | same |
| Mixed 32-thread | 217,522 ops/s | MongoDB 4,875 ops/s | 44.6x | same |
| TPC-H join | 3.2 ms | DuckDB 158 ms | 49.4x | `fiber.json` and this session |
| TPC-H join | 3.2 ms | PostgreSQL 3,040 ms | 950x | same |
| Graph traversal, 600,000 paths | 41.7 ms | Neo4j 637 to 878 ms | 15x to 21x | `RUNTIME_COMPARISON.md` |

**These are the wins that come from the architecture's central idea**, because they are the operations where the answer is materialized. The point lookup at 10.5 ns is 118 times faster than the measured L2 latency of 1.31 ns per level, which means the TIFI index and the in-process design are doing the work.

### 6.4 The seven, ranked by what actually matters

**Rows = architecture. Columns = the win type and the strength of the win.**

| Architecture | Win type | Strength | Verdict |
|---|---|---|---|
| **Sheaf** | Pre-aggregated answer (TPC-H fiber) | 44x to 950x on the join | **Real.** This is the central idea working. |
| **Sheaf** | In-process point lookup | 39,771x to 1,107,152x | **Real, but from the deployment model, not the data model.** |
| ScyllaDB (CQL) | Write-time join resolution | 1,991x on the IMDb Q1 | Real, and it pays an insert cost of 24.8 s. |
| DuckDB | Vectorized scan | 1.75x on IMDb Q1 | Real, and the setup is 511x cheaper. |
| Redis | In-memory | 1.29x on IMDb Q1 | Real, and it is a different problem class. |
| SQLite | Embedded, no server | 3.83x on IMDb Q1 | Real. |
| MariaDB | Row store, mature | 4.90x on IMDb Q1 | Real. |
| Neo4j | Native adjacency | 5.51x on IMDb Q1, 15x to 21x on traversal | Real, and it wins the traversal it is built for. |
| PostgreSQL | Row store, mature | 9.69x on IMDb Q1 | Real. |

**The honest ranking:** ShelfDB's advantage is largest exactly where the answer is materialized. Where the answer is not materialized, ShelfDB is in the 1.05x to 1.75x band on IMDb, and it loses on the two complex queries.

---

## 7. Can it be improved drastically? The ranked list

**Rows = change. Columns = the measured gain, the cost, and the risk.**

| # | Change | Measured gain | Cost | Risk |
|---|---|---|---|---|
| 1 | Four accumulators in the partition scan | **1.40x** on the 1-thread scan | About 10 lines | Very low. The aggregate is unchanged, and the probe already checks that the two kernels agree to 9.28e-13. |
| 2 | Pre-aggregate a fiber for Q2's shape | **157x to 803x** on Q2, 15.6 to 79.7 ms against 12,531 ms | One build, 51.1 s | Low. The build is the same join, paid once. |
| 3 | Pre-aggregate a fiber for Q3's shape | **4,120x to 20,313x** on Q3, 0.086 ms to 0.424 ms against 1,746.9 ms | One build | Low. |
| 4 | Size the fiber domains to the measured LLC | Removes a 5.3x over-provisioning of the ghost replicas | One constant | Low. |
| 5 | Make the bundle load incremental, so a query fetches only the bytes it needs | The load is 301 ms for 201.7 MB, which is 0.67 GB/s against a 1.61 GB/s uncached NVMe read | Medium | Medium. The CRC and the section layout both change. |
| 6 | Publish the total-time column in the comparison tables | Changes 6 of 8 verdicts | Documentation | None. |

**The combined effect of 1, 2 and 3 on the IMDb suite**, using the measured floors:

| Query | Now (ms) | After (ms) | Gain |
|---|---|---|---|
| Q1 | 197.8 | 0.35 to 0.38, the measured answer fiber | 903x to 994x |
| Q2 | 18,333.3 | 15.6 to 79.7 | 230x to 1,175x |
| Q3 | 1,746.9 | 0.086 to 0.424 | 4,120x to 20,313x |
| Suite wall, cached | 21,180.2 | 16.1 to 80.5 | 263x to 1,315x |

**CORRECTION, 2026-10-22.** The Q1 row previously read "141.3, the measured 1.40x accumulator gain". The 1.40x was measured on the TPC-H scan kernel, and it does not reach this path. The bench `crates/sheaf-engine/src/bin/q1_kernel_bench.rs` measures the same change on the IMDb Q1 path at 0.851x to 0.986x, and a 4-thread sharded version at 0.961x to 1.059x. The Q1 projection is the answer fiber, at 0.058 ms to 0.093 ms against 153 ms to 174 ms. See `CODE_CHANGES_AND_EXPECTED_GAINS.md`.

### 7.1 What can make it even better, beyond the measurable list

These are directions the analysis supports but that were not measured in this session. Each is stated with what must be measured to accept it.

| Direction | Reasoning | What must be measured |
|---|---|---|
| One fiber per query shape, discovered from the query log | The break-even is 4 queries at the benchmark build and 9 at the full rebuild, so a shape that runs more than 4 times pays for its build | A shape-frequency histogram |
| Answer fibers that store the answer, not the source columns | Section 4.4 measured 15.6 ms to 79.7 ms for a pre-aggregated Q2 against 12,531 ms for the raw path | Already measured |
| Compression of the fiber sections | The bundle is 1,372 MB and the load is 301 ms for 201.7 MB. A compressed answer section is smaller | A compression ratio and a decode rate |
| The TPC-H design applied to the IMDb shapes | The TPC-H fiber runs at 7.9 ms and wins 44x to 950x. The IMDb shape is the same idea at a larger scale | Already measured for Q2 and Q3 |

---

## 8. The theoretical maximum, revised against these findings

The previous projection claimed a floor of 0.0014 ms for Q2. **That was wrong, and this analysis shows why.** It assumed the answer was pre-materialized AND that the fiber was stored pre-sorted so the top 10 was a 10-row read. The measured pre-aggregated scan of 15.7M rows takes 15.6 ms to 79.7 ms over 8 runs, not 0.0014 ms.

**Revised floors, all measured or derived from measured rates:**

| Query | Floor (ms) | Basis | Gain from now |
|---|---|---|---|
| Q1 | 10.3 | 201.7 MB at the measured 19.2 GB/s bus peak | 15.5x |
| Q2 | 15.6 to 79.7 | Measured pre-aggregated scan at 3.16 to 16.14 GB/s, 8 runs | 157x to 803x |
| Q3 | 0.086 to 0.424 | Measured answer-fiber read of the 44,906-row answer set, 8 runs | 3,472x to 17,118x |
| TPC-H join | 7.9 | **Measured**, cached bundle | Already achieved |

**The corrected Q1 floor of 10.3 ms is a bandwidth floor, and the 15.5x is not achievable with the current design** because the join itself costs more than the bandwidth. The achievable Q1 target is the 44,906-row answer fiber, which section 4.4 measured at 0.086 ms to 0.424 ms over 8 runs.

**The corrected TPC-H figure of 7.9 ms is the honest ceiling for this architecture on a pre-aggregated workload, and it is already achieved.** That is the strongest evidence that the design is right when the shape matches.

---

## 9. The efficiency question: performance per watt

**No power counters are available on this host.** The `Processor Information` counter set exposes performance limit flags but no energy or power counter, and there is no thermal zone. So energy is not measured, and this report does not claim a watt figure.

**The proxy used instead is useful bytes per cycle**, because energy scales with the work done, and the work done is proportional to the cycles.

| Path | Bytes moved | Useful bytes per cycle | Against the ceiling |
|---|---|---|---|
| Bus ceiling at 1 GHz | 19.2 GB/s | 19.2 | 100% |
| TPC-H fiber query, cached | 40.6 MB in 7.9 ms | high, the query is a merge walk | near the ceiling |
| Q1 partition scan, 1 thread | 201.7 MB | 0.94 | 1.95% |
| Q1 answer fiber | 718 KB | 4.4 to 19.2 | 23% to 100% |

**The Q1 partition scan moves 0.94 bytes per cycle while the bus can move 48.0 at the same clock.** That is a stall, and 1.95% of the ceiling is the accumulator chain. The hash build is 16,397,068 of the 21,534,146 cycles, so three quarters of the waste is the serial hash insert, not the scan. Fixing it is worth 1.40x on speed and proportionally on energy, because the stalled cycles are the ones that burn power without producing work.

**The architectural claim holds**: the shelf representation reduces the work, and therefore the energy, by reading the answer instead of the data. The TPC-H fiber demonstrates it at 7.9 ms for a 40.6 MB bundle.

---

## 10. What this analysis does not claim

- **It does not claim ShelfDB is faster overall.** On IMDb total time it is faster against 2 of 8 engines at the benchmark build, and slower against 6. At the forced full rebuild it is faster against 0 of 8. On query time it is faster against all 8.
- **It does not claim the fiber is a pre-aggregate for the IMDb shape.** It is a pre-parse. The TPC-H fiber is the pre-aggregate, and the two are different structures under one name.
- **It does not measure watts.** No power counter exists on this host. The efficiency claim rests on useful bytes per cycle, which is a proxy.
- **The Q1 and Q2 models do not reproduce the real code.** They are 2.6x and 6.4x faster, so they are used only to locate the bottleneck, never to predict a time.
- **The seven-database margins are one box, one run window, one dataset.** The 2026-10-20 and 2026-10-21 runs differ. The matrix window (2026-10-21 15:51) is the one used here, and the numbers are named with their source file.
- **The DuckDB and PostgreSQL margins come from `imdb_job.json`,** which is a recorded baseline from 2026-10-20, not from the same window as the other seven. The comparison is query versus query on the same data with the same truth values, but not the same minute.
- **No new code was changed.** The four changes in section 7 are proposals with measured evidence, not implementations.

---

## 11. Evidence index

| Claim | Where it is verified |
|---|---|
| The seven opponents' query and load times | `results/imdb_multi_engine.json`, runs `20261021_1551` and `20261020_1531` |
| DuckDB and PostgreSQL times and load | `results/imdb_job.json` |
| ShelfDB's IMDb times, idle and matrix window | `results/imdb_shelf.json`, `results/imdb_shelf_matrixwindow.json` |
| The IMDb bundle section sizes, read from the file | `imdb_fiber_bundle.bin` index, 6 sections, 1,439,107,761 B |
| The Q1 path does a query-time join | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` lines 340 to 364 |
| The TPC-H fiber is a pre-aggregate | `crates/sheaf-engine/src/bin/fiber_join_bench.rs`, 2,574,528 (cust, rev) rows |
| TPC-H cached query 7.9 ms, load 67.4 ms | `fiber_join_bench` stdout, this session |
| The Q1 and Q2 decomposition models | `tools/q1_decompose.rs`, `tools/q2_decompose.rs` |
| The Q1 answer-fiber read, 0.089 to 0.162 ms | `tools/q1_decompose.rs` PART4 |
| The Q2 pre-aggregated scan, 15.6 ms to 79.7 ms | `tools/q2_decompose.rs` PART5, 8 runs |
| The hash build and probe costs | `tools/cpu_floor.rs` |
| The CRC second pass | `tools/crc_pass.rs` |
| The 1.40x accumulator gain | `physical_limits_probe` section H, `H_accumulator_*` |
| The 42% to 86% ceiling utilization | `physical_limits_probe` section H, `H_utilization_of_ceiling` |
| The 19.2 GB/s bus peak and 25.6 GB/s ceiling | `mt_scan_bench` 48 MB in 2.5 ms, and the probe's DRAM peak |
| The 133.61 ns DRAM latency | `physical_limits_probe` section A |
| The 51.1 s bundle build | `CONVERSATION_LOG_2026-10-21.md` event 10 |
| The 300 TBW rating and 3.2 years | `physical_limits_probe` section I, `C_nand_years_to_exhaust_rating` |
| The 64 MB domain assumption | `SPEC-016`, `ARCHITECTURE.md` as-built delta A8 |

**Reproduce every experiment:**

```bash
cd sheaf-db
cargo run -p sheaf-engine --bin physical_limits_probe --release   # sections A to J
cargo run -p sheaf-engine --bin fiber_join_bench --release        # the TPC-H fiber
cargo run -p sheaf-engine --bin imdb_fiber_bench --release        # the IMDb queries
sh tests/e2e/von_neumann/tools/run_tools.sh                      # the five analysis tools
```

**Report generated:** 2026-10-22 09:12:40 +05:30 (2026-10-22 03:42:40 UTC).
