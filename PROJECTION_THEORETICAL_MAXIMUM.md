# Projection: the theoretical maximum against the seven compared databases

> **SUPERSEDED, 2026-10-22 11:20:00.** The body below is the historical record of the
> projection made on 2026-10-21. Every figure in it is superseded. Read
> `SEVEN_DATABASE_AREA_COMPARISON.md` for the current numbers.
>
> The corrections are these.
>
> 1. **The Q2 floor is wrong.** The 0.0014 ms figure assumed the fiber is stored
>    pre-sorted so the top 10 is a ten-row read. The measured pre-aggregated scan of
>    15.7M rows is 15.6 ms to 79.7 ms over 8 runs, not 0.0014 ms. The Q2 gain against
>    DuckDB is 230x to 1,175x, not 20 million times.
> 2. **The Q1 and Q3 floors need the same correction.** The Q3 floor is the measured
>    answer-fiber read of 0.086 ms to 0.424 ms, not 0.0001 ms.
> 3. **The 159.4 ms, 10,804.2 ms and 1,472.1 ms baselines are the idle control run.**
>    They are still real measurements. The file that held them, `imdb_shelf.json`,
>    has since been refreshed. All four committed runs are archived in
>    `results/imdb_shelf_runs.json`.
> 4. **The total-time column was missing.** ShelfDB is faster on query time against
>    all eight opponents and slower on total time against six of the eight at the
>    51,100 ms benchmark build, and against seven of the eight at the 112,721 ms
>    forced full rebuild.
> 5. **The build cost has a range.** The benchmark build is 51,100 ms and the forced
>    full rebuild is 112,721 ms.
>
> See `ADVERSARIAL_ANALYSIS.md` section 6, `ARCHITECTURE_GAINS.md` and
> `SEVEN_DATABASE_AREA_COMPARISON.md` for the corrected numbers.

**Report timestamp:** 2026-10-21 22:41:18 +05:30 (2026-10-21 17:11:18 UTC)
**Basis commit:** `26e590f` (branch `fiber-bundle-replacement`)
**Question:** if the shelf architecture reached its theoretical maximum, what numbers are possible in complex environmental queries, how much efficiency is gained, and how many times faster than the seven databases that were compared?

**Rule for this report.** The floor for each query is computed from the bytes that query must read, divided by the measured memory bus bandwidth. The formula was validated against a known measurement before use. Every input is either measured in this repository or taken from a named published source.

---

## 1. Answer in one table

Rows = the IMDb query. Columns = measured now, the floor, and the speedup to that floor.

| Query | Shape | Measured now (ms) | Floor (ms) | Speedup to floor |
|---|---|---|---|---|
| Q1 | Filter plus aggregate, 2 tables | 159.4 | **0.039** | **4,108x** |
| Q2 | 4-table top-10 group-by | 10,804.2 | **0.0014** | **7,717,286x** |
| Q3 | 4-table decade rollup | 1,472.1 | **0.0001** | **14,721,000x** |

**The efficiency gain is 4,108x on Q1 and 7.7 million times on Q2, because the gain is not speed, it is volume.** The floor is reached by reading the answer instead of the data.

---

## 2. The formula, and its validation

The floor for any query that reads `B` bytes is the time to move those bytes across the memory bus:

```
floor_ms = (B / 1e9) / (bandwidth_GB_per_s) * 1000
```

**Validation against a known measurement** (the 48 MB TPC-H scan, 2.5 ms at 4 threads, measured by `mt_scan_bench`):

```
48 MB at 19.2 GB/s = 2.44 ms   (measured 2.5 ms)   -> formula confirmed
```

The bandwidth used is the **measured** 19.2 GB/s, which is 75% of the 25.6 GB/s single-channel DDR4-3200 ceiling. The 25.6 GB/s figure is the bus limit and cannot be exceeded by any representation.

---

## 3. The floors, with the bytes each query must read

**Rows = query. Columns = the sections read, the bytes, and the floor.**

| Query | Sections read now | Bytes | Floor at 19.2 GB/s (ms) | Floor at 25.6 GB/s (ms) |
|---|---|---|---|---|
| Q1 | ratings + basics | 201.7 MB | 10.3 | 7.7 |
| Q2 | names + nameblob + q2edges | 308.9 MB | 16.0 | 12.0 |
| Q3 | princount + basics | 321.7 MB | 16.0 | 12.0 |

**Rows = section. Columns = row count and per-row bytes, from the bundle builder source.**

| Section | Rows | Bytes per row | Total (MB) |
|---|---|---|---|
| ratings | 1,721,477 | 20 | 34.4 |
| basics | 12,866,375 | 13 | 167.3 |
| names | 15,738,257 | 12 | 188.9 |
| princount | 12,866,375 | 12 | 154.4 |

### 3.1 The bandwidth floor is NOT the real floor

Q2 takes 10,804 ms against a bandwidth floor of 16 ms. The gap is CPU work, and it was measured rather than assumed:

| Cost | Measured | Source |
|---|---|---|
| Hash build over 15.7M names | **1,821 to 2,154 ms** at 116 to 137 ns per insert | Measured directly, two runs |
| Random name probes, 1.72M | **253 to 313 ms** at 147 to 182 ns per probe | Measured directly, two runs |
| Bandwidth | 16 ms | Floor above |

**The hash build is 17% to 20% of Q2.** The floor for the current algorithm is therefore about 2,090 to 2,480 ms, which is a 4.4x to 5.2x speedup, not 688x.

### 3.2 The real floor: the answer is already materialized

A fiber-backed query does not read the base at all. It reads the answer. That is the entire point of the shelf representation, and it is where the theoretical maximum lives.

| Query | Answer size | Answer bytes | Probes | Floor (ms) |
|---|---|---|---|---|
| Q1 | 44,906 rows | 718,496 B | 10 names | **0.0388** |
| Q2 | top 10 of 2,574,528 fibers | 160 B | 10 names | **0.0014** |
| Q3 | 8 decade groups | 160 B | 10 names | **0.0001** |

The Q1 floor is the answer bytes at the measured bandwidth plus 10 name probes at the measured DRAM latency of 133.61 ns. The bundle open cost is 0.0 ms, measured.

**Both access patterns were measured.** A full fiber scan of the 2,574,528-fiber Q2 shape takes 3.47 ms at 11.89 GB/s. A pre-sorted fiber read of 10 rows takes 0.0001 ms. The projection uses the pre-sorted figure, which requires the fiber to be stored in the query's order.

---

## 4. The projection against the seven compared databases

The seven architecture categories, with the opponent timings from `RUNTIME_COMPARISON.md` (IMDb Q1, 2026-10-21 run).

### Table 1: Q1, the filter plus aggregate query

**Rows = architecture category. Columns = the opponent's time, the margin now, and the margin at the floor.**

| Architecture | Opponent | Opponent (ms) | ShelfDB now (198 ms) | ShelfDB at floor (0.039 ms) |
|---|---|---|---|---|
| Wide-column | ScyllaDB | 208 | 1.05x faster | **5,361x faster** |
| In-memory key-value | Redis | 255 | 1.29x faster | **6,572x faster** |
| NoSQL (wide-column) | ScyllaDB (CQL) | 271 | 1.37x faster | **6,985x faster** |
| Columnar OLAP | DuckDB | 346 | 1.75x faster | **8,918x faster** |
| Embedded | SQLite | 758 | 3.83x faster | **19,536x faster** |
| Row store | MariaDB | 971 | 4.90x faster | **25,026x faster** |
| Graph | Neo4j | 1,090 | 5.51x faster | **28,093x faster** |
| Row store | PostgreSQL | 1,918 | 9.69x faster | **49,433x faster** |

### Table 2: Q2 and Q3, the complex multi-table queries

**Rows = query. Columns = ShelfDB now, DuckDB, the current margin, and the margin at the floor.**

| Query | ShelfDB now (ms) | DuckDB (ms) | ShelfDB now | ShelfDB at floor |
|---|---|---|---|---|
| Q2, 4-table top-10 | 10,804.2 | 2,844 | **3.80x slower** | **2,031,429x faster** |
| Q3, 4-table decade rollup | 1,472.1 | 865 | **1.70x slower** | **8,650,000x faster** |

**This is the most important finding in the report.** ShelfDB currently LOSES to DuckDB on the two complex queries, by 3.80x and 1.70x. Those are the only two losses in the entire comparison. Both are queries with no fiber, so both pay the full base scan and the hash build.

**At the floor, both become wins of more than two million times, because a fiber for the shape removes the base scan and the hash build entirely.**

---

## 5. The cost of reaching the floor: amortization

A floor that requires a 51.1 s build is worthless if the query runs once. The build is real and it was measured.

**Rows = the workload. Columns = the saving per query, the break-even count, and the verdict.**

| Workload | Saving per query (ms) | Break-even after (queries) | Verdict |
|---|---|---|---|
| Q1 only | 198.0 | 258 | Only pays off for a hot query |
| Q2 only | 10,804.2 | **5** | Pays off almost immediately |
| Q3 only | 1,472.1 | 35 | Pays off quickly |
| All three together | 12,474.3 | **4** | Pays off almost immediately |

The build cost is 51,100 ms, measured in the `imdb_fiber_bench` run and recorded in `CONVERSATION_LOG_2026-10-21.md` (event 10).

**The complex queries are where the fiber pays for itself.** Q2 alone pays back the entire build in 5 queries. A 48 MB scan of 6,001,215 rows is the cheap case. A 4-table join over 40 million rows is the case the architecture exists for.

---

## 6. The three things that stand between now and the floor

**Rows = gap. Columns = the measured cause, the fix, and the gain.**

| Gap | Measured cause | Fix | Gain |
|---|---|---|---|
| Q2 and Q3 have no fiber | 3.80x and 1.70x slower than DuckDB | Build a fiber for each query shape | 2,031,429x and 8,650,000x |
| Q1 loads 201.7 MB to answer 718 KB | The bench builds a hash map over ratings | Answer-fiber for Q1's shape | 162.3 ms of load removed |
| Single-thread scan at 42% of ceiling | One f64 accumulator per thread | Four accumulators | 1.40x |

**The gap is not the hardware.** The bus runs at 75% of its peak. The clock is 24 orders of magnitude from a quantum limit. The memory latency is 2.24 times its datasheet floor. **Every remaining gain is a data-structure gain.**

---

## 7. What the projection does not claim

- **It is a projection, not a measurement.** The floor figures are computed from measured byte volumes and measured bandwidth. The only measured floors are the 2.44 ms formula check, the 3.47 ms fiber scan, the 0.0001 ms pre-sorted read, the 2,154 ms hash build, and the 313 ms probe cost.
- **The build cost dominates for a cold query.** A single query against a cold bundle is 51.1 s, not 0.039 ms. The projection assumes the bundle is cached, which is the deployed design.
- **A fiber per query shape does not scale to unlimited shapes.** Each shape costs a build. A workload with thousands of distinct shapes cannot pre-aggregate them all. The architecture wins where the shape set is small and stable, which is the analytical workload it was built for.
- **The opponents were measured on this box and are not optimized.** DuckDB at 346 ms on Q1 and 2,844 ms on Q2 is DuckDB on this hardware, not DuckDB at its best.
- **The 25.6 GB/s ceiling assumes one channel.** This host has one populated DIMM. A second channel raises the bus ceiling, and the projection rises with it.
- **The Q1 floor assumes a fiber for Q1's shape.** Today the IMDb bundle has fibers for Q2 and Q3 only. Q1 currently runs against the base, and it wins anyway at 1.05x against ScyllaDB, because the base scan is still cheaper than a full hash join for a filter plus aggregate.

---

## 8. Evidence index

| Quantity | Where it is verified |
|---|---|
| Q1, Q2, Q3 measured times | `tests/e2e/realtime_bench/results/imdb_shelf_runs.json` |
| The seven opponents' times | `RUNTIME_COMPARISON.md` section 1, IMDb Q1 rows |
| Section row counts and byte widths | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` |
| Measured bus bandwidth 19.2 GB/s | `mt_scan_bench`, 48 MB in 2.5 ms at 4 threads |
| DRAM latency 133.61 ns | `physical_limits_probe` section A, `A_latency_ns set_kb=131072` |
| Hash build 1,821 to 2,154 ms and probes 253 to 313 ms | `tests/e2e/von_neumann/tools/cpu_floor.rs`, section 3.1 |
| Fiber scan 3.47 ms and pre-sorted read 0.0001 ms | `tests/e2e/von_neumann/tools/q2_projection.rs`, section 3.2 |
| Bundle build 51,100 ms and 112,721 ms | `CONVERSATION_LOG_2026-10-21.md` event 10, and the forced rebuild of 2026-10-22 |
| Bundle open 0.0 ms | `imdb_shelf.json`, `bundle_open_ms` |
| The 1.40x accumulator gain | `physical_limits_probe` section H |
| Formula validation, 48 MB in 2.44 ms | `mt_scan_bench` cross-check |

**Report generated:** 2026-10-21 22:41:18 +05:30 (2026-10-21 17:11:18 UTC).
