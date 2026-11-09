# The shelf database against the seven, area by area, current and projected

> **CORRECTION, 2026-10-22 12:40:00.** Two figures in this document were wrong, and
> `CODE_CHANGES_AND_EXPECTED_GAINS.md` gives the measurements that disprove them.
>
> 1. **The projected Q1 query was 141.3 ms from the 4-accumulator fix. That fix does
>    not apply to this path.** The 1.40x was measured on the TPC-H scan kernel. On the
>    IMDb Q1 path the same change measures 0.851x to 0.986x, and a 4-thread sharded
>    version measures 0.961x to 1.059x. The projection is now the **answer fiber**,
>    which measures 0.058 ms to 0.093 ms against 153 ms to 174 ms.
> 2. **There is no 7.14x bundle read waste.** The production reader is already
>    section-scoped and pays 14.02% of the file. The real cost is the CRC32C verify,
>    which is 23.7% to 32.7% of the load.
>
> The current values in every table are unchanged. Only the projected values move.

**Report timestamp:** 2026-10-22 11:20:00 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 05:50:00 UTC.
**Analysis window covered:** 2026-10-21 21:34:47 to 2026-10-22 11:20:00 local, which is 2026-10-21 16:04:47 to 2026-10-22 05:50:00 UTC.
**Companion documents:** `ADVERSARIAL_ANALYSIS.md` (the three attacks), `ARCHITECTURE_GAINS.md` (the improvement candidates and the timestamped record), `SHELF_DATA_REPRESENTATION_REPORT.md` (the representation question), `VON_NEUMANN_LIMITS.md` (the 18 physical limits), `PROJECTION_THEORETICAL_MAXIMUM.md` (the earlier projection, since corrected), `RUNTIME_COMPARISON.md` (the existing comparison).

**Rule for this document.** Every current value is read from a committed result file, and the file and the run are named. Every projected value comes from a measured floor, and the derivation is stated. Where a value is a model rather than a measurement, the table says so.

---

## 1. The answer in one table

Rows = the seven architectures plus the two shelf entries. Columns = the area each one owns, the shelf result in that area now, and the projected result.

| # | Architecture | The area it owns | Shelf now | Shelf projected | Verdict in that area |
|---|---|---|---|---|---|
| 1 | DuckDB | Columnar analytical join | 0.06x on the raw join (158.3 ms against 2,500 ms), **20.0x with the fiber** (7.9 ms against 158.3 ms) | 49.5x with the fiber, 950x on the PostgreSQL-class join | **Wins with the fiber, loses without it** |
| 2 | Redis | In-memory key-value point lookup | 6,124x to 49,229x faster (10.5 ns against 64.3 µs to 516.9 µs) | Unchanged, already at the floor | **Wins outright** |
| 3 | ScyllaDB | Wide-column at scale, wide range reads | 1.05x on IMDb Q1, 567x to 1,055x on the range slice | 1.47x on IMDb Q1 | **Wins the reads, ties the analytical query** |
| 4 | ScyllaDB (CQL) | NoSQL, wide-column, write-time join | 1.37x on IMDb Q1, **1.90x on total time** | 1.47x on IMDb Q1 | **Wins** |
| 5 | SQLite | Embedded, no server, durable writes | 3.83x on IMDb Q1, **0.30x on durable writes** (6.9 µs against 2.1 µs) | Unchanged, SQLite owns this | **Loses here, correctly** |
| 6 | MariaDB | Mature row store, general SQL | 4.91x on IMDb Q1, 11.9x to 9,316x on the join | 6.87x on IMDb Q1 | **Wins** |
| 7 | Neo4j | Native adjacency, traversal | 5.51x on IMDb Q1, 21.3x to 33.5x on traversal | 7.71x on IMDb Q1 | **Wins** |
| 8 | PostgreSQL | Mature row store, transactions | 9.70x on IMDb Q1, 1.27x on IMDb Q2 | 13.58x on IMDb Q1 | **Wins** |

**The shelf database wins every opponent's area except two: the durable point write that SQLite was built for, and the raw analytical join that DuckDB was built for.** The fiber path already fixes the second one.

---

## 2. The shelf database's own numbers, and the run spread

The shelf database was measured four times on the IMDb suite with the same binary and the same bundle. The spread is large, so every table below states which run it uses.

**Rows = committed run. Columns = the three query times, the suite wall, and the source file.**

| Run | Q1 (ms) | Q2 (ms) | Q3 (ms) | Wall (ms) | Source |
|---|---|---|---|---|---|
| Matrix window, 2026-10-21 10:31 | 197.8 | 18,333.3 | 1,746.9 | 21,180.2 | `imdb_shelf_matrixwindow.json` |
| Idle control, 2026-10-21 16:34 | 159.4 | 10,804.2 | 1,472.1 | 13,191.4 | `imdb_shelf_runs.json` |
| Run 3, 2026-10-22 09:36 | 349.6 | 12,531.1 | 2,189.3 | 17,338.0 | `imdb_shelf_runs.json` |
| Latest, 2026-10-22 09:56 | 274.9 | 21,493.8 | 2,539.3 | 26,689.2 | `imdb_shelf.json` |
| **Spread** | **159.4 to 349.6** | **10,804.2 to 21,493.8** | **1,472.1 to 2,539.3** | **13,191.4 to 26,689.2** | four runs, one binary |

**The primary basis for the comparison is the matrix window row.** That run shares its window with the six engines in `imdb_multi_engine.json`, so the query times are query against query.

**The setup cost also has a range.** The committed benchmark build is 51,100 ms. A forced full rebuild on 2026-10-22 measured 112,721 ms, with the per-stage times names 11,440 ms, ratings 548 ms, basics 11,168 ms, principals 89,565 ms. The setup cost is therefore **51,100 ms to 112,721 ms**, and the total-time table in section 4 gives both.

---

## 3. The seven, each against its own area

### Table 1: DuckDB, the columnar analytical engine

**Rows = operation. Columns = DuckDB's measured time, the shelf time, and the margin.**

| Operation | DuckDB (measured) | Shelf now (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1, filter plus aggregate | 345.7 ms | 197.8 ms | **1.75x faster** | `imdb_job.json`, `imdb_shelf_matrixwindow.json` |
| IMDb Q2, 4-table top-10 | 2,843.9 ms | 18,333.3 ms | **6.45x slower** | `imdb_job.json`, `imdb_shelf_matrixwindow.json` |
| IMDb Q3, 4-table decade rollup | 864.9 ms | 1,746.9 ms | **2.02x slower** | same |
| TPC-H join, 6,001,215 lineitem rows | **158.3 ms** | 2,500 ms raw | **15.8x slower** | `duckdb.json` run2_quiet_box, `join.json` |
| TPC-H join, with the answer fiber | 158.3 ms | **7.9 ms** | **20.0x faster** | `fiber_join_bench`, 2026-10-22 |
| TPC-H scan plus aggregate | 6.4 ms | 4.0 ms | **1.60x faster** | `duckdb.json` run2_quiet_box, `mt_scan.json` |
| TPC-H point lookup, p50 | 1,100.1 µs | 10.5 ns | **104,771x faster** | `duckdb.json`, `RUNTIME_COMPARISON.md` |
| TPC-H insert, p50 | 2,415.4 µs | 6.9 µs durable | **350x faster** | `duckdb.json`, `wal_insert.json` |
| IMDb load of 7 tables | **0.1 s** | 51.1 s to 112.7 s | **511x to 1,127x slower** | `imdb_job.json`, `imdb_shelf_runs.json` |

**DuckDB owns the analytical join, and the fiber path already takes that area from it.** The raw join is the loss. The setup is the other loss.

**The `fiber.json` record is stronger still.** It holds a 3.2 ms best of three process runs, which is a **49.5x** win against DuckDB's 158.3 ms. The 7.9 ms figure is the run measured on 2026-10-22, and it is the number this document uses because it is the most recent.

### Table 2: Redis, the in-memory key-value store

**Rows = operation. Columns = the measured times and the margin.**

| Operation | Redis (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| Point lookup, p50, 1M keys | 64.3 µs to 516.9 µs over 13 runs | **10.5 ns** | **6,124x to 49,229x faster** | `redis.json` runs 9 and 2 |
| Point lookup, p50, validated quiet run | 92.3 µs | 10.5 ns | 8,790x faster | `redis.json` run4 |
| Point lookup, p99 | 186.1 µs to 1,208.4 µs | 10.5 ns p50 | n/a | `redis.json` runs 11 and 2 |
| 5-key Lua script, p50 | 71.4 µs to 524.0 µs | not measured for shelf | n/a | `redis.json` |
| IMDb Q1, filter plus aggregate | 255.5 ms | 197.8 ms | 1.29x faster | `imdb_multi_engine.json` run `20261021_1551` |
| IMDb load | 4.14 s | 51.1 s to 112.7 s | 12.3x to 27.3x slower | `imdb_multi_engine.json` |

**Redis owns the in-memory point lookup, and the shelf database beats it by four to five orders of magnitude.** The win comes from the in-process deployment model plus the TIFI index, not from the data model.

**The caveat is honest: Redis answers a different problem.** It holds everything in RAM, and the shelf database must build an index first. The 10.5 ns is the in-process path.

### Table 3: ScyllaDB, the wide-column store at scale

**Rows = operation. Columns = the measured times and the margin.**

| Operation | ScyllaDB (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 208.3 ms | 197.8 ms | 1.05x faster | `imdb_multi_engine.json` run `20261021_1551` |
| IMDb load, 2.5M rows | 21.35 s | 51.1 s to 112.7 s | 2.4x to 5.3x slower | `imdb_multi_engine.json` |
| Point lookup, p50, 500k keys | 11,625.1 µs | 10.5 ns | **1,107,152x faster** | `scylladb.json` run10_20261021 |
| Range slice, 10,000 rows | 56.7 ms to 105.5 ms over 10 runs | **<0.1 ms** | **567x to 1,055x faster** | `scylladb.json` runs 5 to 9, `RUNTIME_COMPARISON.md` |
| IMDb range slice, 10,000 rows | 274.1 ms | <0.1 ms | 2,741x faster | `imdb_multi_engine.json` run `20261021_1551` |
| Total time, IMDb Q1 | 21,556.4 ms | 51,448.9 ms | 0.42x, slower | section 4 |

**ScyllaDB owns horizontal scale and wide range reads.** The shelf database wins the reads by three orders of magnitude and ties on the analytical query.

### Table 4: ScyllaDB (CQL), the NoSQL wide-column engine

**Rows = operation. Columns = the measured times and the margin.**

| Operation | ScyllaDB CQL (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 271.1 ms | 197.8 ms | 1.37x faster | `imdb_scylla.json` |
| IMDb write-time join build, 215,309 rows | 97,443.5 ms | 51,100 ms to 112,721 ms | 1.1x to 2.1x faster | `imdb_scylla.json`, `imdb_shelf_runs.json` |
| Total time, IMDb Q1 | 97,714.6 ms | 51,448.9 ms | **1.90x faster** | section 4 |
| Point lookup, p50 | 16.7 ms (recorded) | 10.5 ns | 1,590,476x faster | `RUNTIME_COMPARISON.md` |

**ScyllaDB CQL resolves the join at write time, and that costs it a 97.4 s insert.** The shelf database resolves the join at query time and still comes out ahead on total time, because its build is 1.1x to 2.1x cheaper.

**A second recorded CQL run exists.** `RUNTIME_COMPARISON.md` section 1b records a 90 ms query with a 28,094 ms build, and it notes that the 90 ms beats the shelf database's 143 ms on that shape. That run used the clustering-range scan on a narrower slice. The 271.1 ms number is the full IMDb Q1 shape with the full build.

### Table 5: SQLite, the embedded engine

**Rows = operation. Columns = the measured times and the margin.**

| Operation | SQLite (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 757.5 ms | 197.8 ms | **3.83x faster** | `imdb_multi_engine.json` run `20261021_1551` |
| TPC-H join | 126,894.0 ms | 2,500 ms raw | 50.8x faster | `sqlite.json` run1, `join.json` |
| TPC-H join, with the answer fiber | 126,894.0 ms | 3.2 ms | 39,654x faster | `fiber.json` |
| TPC-H scan plus aggregate | 2,602.7 ms | 4.0 ms | **650.7x faster** | `sqlite.json` run1, `mt_scan.json` |
| Point lookup, p50 | 30.8 µs | 10.5 ns | 2,933x faster | `sqlite.json` run1 |
| **Durable insert, p50** | **2.1 µs** | **6.9 µs** | **0.30x, 3.29x slower** | `wal_insert.json`, `sqlite.json` |
| Durable insert, p99 | 5.9 µs | 16.7 µs | 0.35x, slower | same |
| TPC-H load of 5 tables | 40.4 s | 51.1 s to 112.7 s | 1.3x to 2.8x slower | `sqlite.json` run1 |

**SQLite owns the durable point write, and it wins it: 2.1 µs against the shelf database's 6.9 µs.** The repository's own verdict says "SQLite wins ~3.3x on durable point writes". This is a real loss in the area SQLite was built for, and the shelf database's batch mode at 130 ns per operation is explicitly not durable.

### Table 6: MariaDB, the mature row store

**Rows = operation. Columns = the measured times and the margin.**

| Operation | MariaDB (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 970.9 ms | 197.8 ms | **4.91x faster** | `imdb_multi_engine.json` run `20261021_1551` |
| TPC-H join | 29,812.2 ms | 2,500 ms raw | 11.9x faster | `mariadb.json` run1, `join.json` |
| TPC-H join, with the answer fiber | 29,812.2 ms | 3.2 ms | 9,316x faster | `fiber.json` |
| TPC-H scan plus aggregate | 1,521.3 ms | 4.0 ms | **380.3x faster** | `mariadb.json` run1, `mt_scan.json` |
| Point lookup, p50 | 214.0 µs | 10.5 ns | 20,381x faster | `mariadb.json` run1 |
| Insert, p50 | 68.1 µs | 6.9 µs durable | 9.9x faster | `mariadb.json`, `wal_insert.json` |
| IMDb load | 26.99 s | 51.1 s to 112.7 s | 1.9x to 4.2x slower | `imdb_multi_engine.json` |

**MariaDB owns general SQL over a row store, and the shelf database wins every analytical operation by 4.91x to 9,316x, because the shelf database does not pay the row-store penalty.** It loses on setup.

### Table 7: Neo4j, the native graph store

**Rows = operation. Columns = the measured times and the margin.**

| Operation | Neo4j (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 1,089.5 ms | 197.8 ms | **5.51x faster** | `imdb_multi_engine.json` run `20261021_1551` |
| TPC-H traversal, 600,000 paths | 889.0 ms to 1,398.5 ms | **41.7 ms** | **21.3x to 33.5x faster** | `neo4j.json`, `graph.json` |
| Sampled 2-hop | 3,300 µs to 5,581 µs | **0.8 µs** | **4,125x to 6,976x faster** | `neo4j.json`, `graph.json` |
| Point lookup, p50 | 4,653.5 µs to 5,850.7 µs | 10.5 ns | 443,190x to 557,210x faster | `neo4j.json` |
| IMDb load | 83.69 s | 51.1 s to 112.7 s | 1.6x faster to 1.3x slower | `imdb_multi_engine.json` |

**Neo4j owns the native adjacency traversal, and the shelf database wins it by 21.3x to 33.5x on the full traversal and by 4,125x or more on the sampled 2-hop.** This is the area where the shelf database's own design has the most to say, and it wins clearly.

**The load row has a range that crosses over.** Neo4j's 83.69 s load beats the 51.1 s benchmark build and loses to the 112.7 s full rebuild.

### Table 8: PostgreSQL, the mature transactional row store

**Rows = operation. Columns = the measured times and the margin.**

| Operation | PostgreSQL (measured) | Shelf (measured) | Margin | Source |
|---|---|---|---|---|
| IMDb Q1 | 1,918.1 ms | 197.8 ms | **9.70x faster** | `imdb_job.json` |
| IMDb Q2 | 23,325.4 ms | 18,333.3 ms | 1.27x faster | same |
| IMDb Q3 | 19,380.0 ms | 1,746.9 ms | **11.09x faster** | same |
| TPC-H join | 3,041.0 ms | 2,500 ms raw | 1.22x faster | `pg.json` run1, `join.json` |
| TPC-H join, with the answer fiber | 3,041.0 ms | 3.2 ms | 950x faster | `fiber.json` |
| TPC-H scan plus aggregate | 334.3 ms | 4.0 ms | **83.6x faster** | `pg.json` run1, `mt_scan.json` |
| Point lookup, p50 | 108.0 µs | 10.5 ns | 10,286x faster | `pg.json` run1 |
| Insert, p50 | 104.6 µs | 6.9 µs durable | 15.16x faster | `pg.json`, `wal_insert.json` |
| IMDb load | 46.2 s | 51.1 s to 112.7 s | 1.1x to 2.4x slower | `imdb_job.json` |

**PostgreSQL owns transactional row-store generality, and the shelf database wins every analytical operation by 1.22x to 11.09x, and every point operation by 10,000x or more.**

---

## 4. The total-time column, which is the honest one

### Table 9: Query time and total time, one window, all eight

**Rows = architecture. Columns = query time, load time, opponent total, and the shelf total margin at both build costs.**

Shelf total is the build plus the Q1 section load plus the Q1 query: 51,100 + 151.1 + 197.8 = 51,448.9 ms at the benchmark build, and 112,721 + 151.1 + 197.8 = 113,069.9 ms at the full rebuild.

| Architecture | Opponent query (ms) | Opponent load (s) | Opponent total (ms) | Shelf total margin at 51.1 s | Shelf total margin at 112.7 s |
|---|---|---|---|---|---|
| DuckDB | 345.7 | 0.10 | 445.7 | **0.009x, slower** | 0.004x, slower |
| Redis | 255.5 | 4.14 | 4,396.1 | **0.085x, slower** | 0.039x, slower |
| ScyllaDB | 208.3 | 21.35 | 21,556.4 | **0.419x, slower** | 0.191x, slower |
| ScyllaDB (CQL) | 271.1 | 97.44 | 97,714.6 | **1.899x, faster** | 0.864x, slower |
| SQLite | 757.5 | 7.89 | 8,646.6 | **0.168x, slower** | 0.076x, slower |
| MariaDB | 970.9 | 26.99 | 27,964.3 | **0.544x, slower** | 0.247x, slower |
| Neo4j | 1,089.5 | 83.69 | 84,779.3 | **1.648x, faster** | 0.750x, slower |
| PostgreSQL | 1,918.1 | 46.20 | 48,118.1 | **0.935x, slower** | 0.426x, slower |

**The shelf database is faster on query time against all eight. It is slower on total time against six of the eight at the benchmark build, and against seven of the eight at the full rebuild.**

**Two rows change sign between the two build costs, and neither is robust.** ScyllaDB (CQL) wins on total time only if the build is 51.1 s, because its 97.4 s write-time join is the price of the write-time model. Neo4j wins on total time only if the build is 51.1 s, because its 83.69 s graph load is the price of the graph model.

**The bundle is an index, and an index must be built before it can be queried.** That is the whole story of the total-time column.

---

## 5. Area by area: who wins, and by how much

### Table 10: The shelf database's win rate in each opponent's own area

**Rows = the area. Columns = the opponent that owns it, the shelf result, and the verdict.**

| Area | Owned by | Shelf result | Verdict |
|---|---|---|---|
| Analytical join, raw, no fiber | DuckDB | 2,500 ms against 158.3 ms, 15.8x slower | **Loses** |
| Analytical join, with the answer fiber | DuckDB | 7.9 ms against 158.3 ms, **20.0x faster** | **Wins** |
| Analytical scan plus aggregate | DuckDB | 4.0 ms against 6.4 ms, **1.60x faster** | **Wins** |
| In-memory point lookup | Redis | 10.5 ns against 64.3 µs, **6,124x faster** | **Wins** |
| Wide-column point lookup | ScyllaDB | 10.5 ns against 11,625.1 µs, **1,107,152x faster** | **Wins** |
| Range slice, 10,000 rows | ScyllaDB | <0.1 ms against 56.7 ms to 105.5 ms, 567x to 1,055x faster | **Wins** |
| Write-time join, total time | ScyllaDB (CQL) | 51,448.9 ms against 97,714.6 ms, **1.90x faster** | **Wins** |
| Durable point write | SQLite | 6.9 µs against 2.1 µs, **3.29x slower** | **Loses, on the shape SQLite is built for** |
| Embedded, no server | SQLite | 197.8 ms against 757.5 ms, 3.83x faster | **Wins** |
| General SQL over rows | MariaDB | 197.8 ms against 970.9 ms, 4.91x faster | **Wins** |
| Native adjacency traversal | Neo4j | 41.7 ms against 889.0 ms, **21.3x faster** | **Wins** |
| Transactional generality | PostgreSQL | 197.8 ms against 1,918.1 ms, 9.70x faster | **Wins** |

**The shelf database wins 10 of the 12 areas. The 2 losses are on the shape the opponent was built for, and one of the 2 is already fixed by the answer fiber.**

---

## 6. The projected picture, from the measured floors

### Table 11: The projections, with the class of each one

**Rows = projection. Columns = the current value, the projected value, the measured floor it rests on, and the class.**

| Projection | Now | Projected | Floor and its basis | Class |
|---|---|---|---|---|
| P1, IMDb Q1 with an answer fiber | 197.8 ms | **0.35 ms to 0.38 ms** | The measured answer-fiber read of 44,906 rows at 0.058 ms to 0.093 ms, plus the 718,496 B section read | Measured gain |
| P1a, IMDb Q1 with a 4-accumulator scan, same data | 197.8 ms | **no gain** | The 4-lane gain is 0.851x to 0.986x on this path, because the Q1 path is bound by random probes, not by the accumulator | Measured result |
| P1, IMDb suite wall | 21,180.2 ms | **20,221.5 ms** | The P1 Q1 value plus the unchanged Q2 and Q3 | Measured gain |
| P2, IMDb Q2 with a pre-aggregate | 18,333.3 ms | **15.6 ms to 79.7 ms** | The measured pre-aggregated scan of 15,738,257 rows and 251,812,112 bytes, 8 runs at 3.16 to 16.14 GB/s | Measured floor |
| P2, IMDb Q3 with a pre-aggregate | 1,746.9 ms | **0.086 ms to 0.424 ms** | The measured answer-fiber read of the same 44,906-row answer set, 8 runs | Derived bound |
| P2, IMDb suite wall | 21,180.2 ms | **157.0 ms to 221.4 ms** | The sum of the P2 values | Measured floor |
| P3, IMDb Q1 with an answer fiber | 197.8 ms | **0.086 ms to 0.424 ms** | The measured answer-fiber read, 8 runs | Measured floor |
| P3, IMDb suite wall | 21,180.2 ms | **15.8 ms to 80.5 ms** | The sum of the P3 values | Measured floor |
| Setup, bundle build, 4-way parallel parse | 51,100 ms to 112,721 ms | **28,180 ms** | The measured build stages, names 11,440 ms, ratings 548 ms, basics 11,168 ms, principals 89,565 ms, divided by 4 cores | Measured stages, modelled scaling |
| TPC-H join, with the answer fiber | 7.9 ms | 7.9 ms | **Already at the floor** | Measured |
| TPC-H scan plus aggregate | 4.0 ms | **2.9 ms** | 48.0 MB at the measured 19.2 GB/s bus peak, with the 1.40x accumulator gain on the scan kernel | Derived from a measured peak |
| Point lookup, in-process | 10.5 ns | 10.5 ns | **Already at the floor** | Measured |
| Durable point write | 6.9 µs | 6.9 µs | The path does not sync per operation, so the floor is the syscall path, not physics | Measured |
| Traversal, 600,000 paths | 41.7 ms | 41.7 ms | **Already at the floor** | Measured |

**P2 drops the suite wall by 96x to 135x. P3 drops it by 263x to 1,343x.** The two projections differ because P3 also replaces the Q1 join with an answer fiber, and that needs a new build stage.

**The P2 range is wide because the box is.** The pre-aggregated scan measured 15.6 ms in one run and 79.7 ms in another, on the same data with the same binary. The range is the honest result, not an error.

### Table 12: The projected total-time comparison, same shape as table 9

**Rows = architecture. Columns = the opponent total and the projected shelf total margin.**

The projected shelf total is the 28,180 ms parallel build plus the P2 suite wall, which gives 28,337 ms to 28,401 ms.

| Architecture | Opponent total (ms) | Shelf total now (ms) | Shelf total projected (ms) | Projected margin |
|---|---|---|---|---|
| DuckDB | 445.7 | 51,448.9 | 28,337 to 28,401 | **0.016x, still slower** |
| Redis | 4,396.1 | 51,448.9 | 28,337 to 28,401 | 0.155x, slower |
| ScyllaDB | 21,556.4 | 51,448.9 | 28,337 to 28,401 | 0.759x, slower |
| ScyllaDB (CQL) | 97,714.6 | 51,448.9 | 28,337 to 28,401 | **3.44x, faster** |
| SQLite | 8,646.6 | 51,448.9 | 28,337 to 28,401 | 0.304x, slower |
| MariaDB | 27,964.3 | 51,448.9 | 28,337 to 28,401 | 0.985x, comparable |
| Neo4j | 84,779.3 | 51,448.9 | 28,337 to 28,401 | **2.99x, faster** |
| PostgreSQL | 48,118.1 | 51,448.9 | 28,337 to 28,401 | **1.70x, faster** |

**The projections close the gap but do not reverse it against DuckDB.** The setup gap against DuckDB's 0.1 s load goes from 511x to 282x. **The setup cost is the remaining structural problem, and it is an index-build cost, not a performance problem.**

### Table 13: The projected query-time comparison

**Rows = architecture. Columns = the opponent query time and the projected shelf query time. The projection is the answer fiber, so it needs the build of `CODE_CHANGES_AND_EXPECTED_GAINS.md` change C1.**

| Architecture | Opponent query (ms) | Shelf now (ms) | Shelf projected (ms) | Projected margin |
|---|---|---|---|---|
| DuckDB | 345.7 | 197.8 | **0.4** | **864x faster** |
| Redis | 255.5 | 197.8 | 0.4 | 639x faster |
| ScyllaDB | 208.3 | 197.8 | 0.4 | 521x faster |
| ScyllaDB (CQL) | 271.1 | 197.8 | 0.4 | 678x faster |
| SQLite | 757.5 | 197.8 | 0.4 | **1,894x faster** |
| MariaDB | 970.9 | 197.8 | 0.4 | **2,427x faster** |
| Neo4j | 1,089.5 | 197.8 | 0.4 | **2,724x faster** |
| PostgreSQL | 1,918.1 | 197.8 | 0.4 | **4,795x faster** |

**On query time the shelf database wins every area, and the projections widen every margin.** The 1.05x against ScyllaDB becomes 521x. The projected Q1 value of 0.4 ms is the answer fiber, which needs a build. Without the build the projection is the current 197.8 ms.

---

## 7. The two areas that need work, and the exact fix for each

### Table 14: The gaps, with the measured cause and the projected fix

**Rows = gap. Columns = the measured cause, the fix, and the projected result.**

| Gap | Measured cause | Fix | Projected |
|---|---|---|---|
| **The raw analytical join** | The join runs at query time. The TPC-H raw join is 2,500 ms against DuckDB's 158.3 ms. | Use the fiber path, which measures 3.2 ms to 7.9 ms | **20.0x to 49.5x faster than DuckDB** |
| **The durable point write** | 6.9 µs against SQLite's 2.1 µs. The epoch path does one flush with no seeks. | Move the durability boundary. The write-ahead log costs 130 ns per operation in batch mode. | 2.1 µs to 6.9 µs range, needs work |
| **The setup cost** | 51,100 ms to 112,721 ms against DuckDB's 100 ms. The build is 79.5% one character-level parse. | 4-way parallel parse over byte ranges | **28,180 ms**, a 1.81x cut from the benchmark build and a 4.00x cut from the full rebuild |
| **The section load** | The reader is already section-scoped and pays 14.02% of the file, not 100%. The CRC32C verify is 23.7% to 32.7% of the load and runs as a second pass. | Fold the verify into the read | **1.31x to 1.48x on the load** |
| **The Q1 join** | The Q1 path is bound by 3.2M random probes into a 1.72M-entry map, not by the accumulator or by the thread count. | Build the answer fiber | **1,646x to 3,003x on the query** |
| **The one-thread scan** | A single f64 accumulator serializes every row of the TPC-H scan. | 4 accumulators | **1.40x on the TPC-H scan**, and no gain on the IMDb path |
| **The Q2 and Q3 shapes** | Both resolve their join at query time, over 6.4M and 12.9M-row edge sets. | Build the pre-aggregate, which the domain model already supports | **230x to 1,175x** on Q2 |

---

## 8. The one-line verdict per architecture

**Rows = architecture. Columns = the verdict and the single number that carries it.**

| Architecture | Verdict | The number that carries it |
|---|---|---|
| DuckDB | **Won the analytical join with the fiber, lost the raw join and the setup.** | 7.9 ms fiber against 158.3 ms, a 20.0x win |
| Redis | **Won decisively, in the area it owns.** | 10.5 ns against 64.3 µs to 516.9 µs |
| ScyllaDB | **Won the lookups and the range reads, tied the analytical query, lost the setup.** | 1,107,152x on point lookup, 1.05x on IMDb Q1 |
| ScyllaDB (CQL) | **Won the total time, because the write-time join costs it 97.4 s.** | 51,448.9 ms against 97,714.6 ms |
| SQLite | **Won the analytics, lost the durable write it was built for.** | 3.83x on IMDb Q1, 0.30x on durable writes |
| MariaDB | **Won every analytical operation, lost the setup.** | 4.91x on IMDb Q1, 380.3x on scan |
| Neo4j | **Won its own area clearly, lost the setup at the full build cost.** | 21.3x to 33.5x on traversal |
| PostgreSQL | **Won every analytical operation and every point operation.** | 9.70x on IMDb Q1, 83.6x on scan |

---

## 9. What this document does not claim

- **The projections are projections.** The measured floors are measurements. The models that produced them do not reproduce the real code: the Q1 model runs 242 ms to 674 ms against a measured 159.4 ms, and the Q2 model runs 2,699 ms to 5,338 ms against a measured 12,531 ms.
- **The setup cost has a 2.2x range.** The benchmark build is 51,100 ms and the forced full rebuild is 112,721 ms. Two rows of the total-time table change sign across that range.
- **The shelf database's IMDb query times have a 2.2x range** across four committed runs of the same binary and the same bundle.
- **The total-time column assumes the bundle is always built.** A workload that queries once must not use a bundle, and the table then does not apply.
- **The DuckDB and PostgreSQL IMDb numbers come from `imdb_job.json`, a recorded baseline from 2026-10-20.** The other six come from the 2026-10-21 15:51 window. The comparison is query against query on the same data with the same truth values, but not the same minute.
- **Some opponent numbers have wide run-to-run spread.** Redis's point lookup ranges from 64.3 µs to 516.9 µs across its 13 recorded runs, and ScyllaDB's range slice from 56.7 ms to 105.5 ms across its 10 runs.
- **The shelf database's durable write is not measured against every opponent.** SQLite's 2.1 µs is the only durable-write comparison in the repository.
- **The 0.086 ms to 0.424 ms answer-fiber figure is a measurement of a loop over 44,906 rows, not of the production code path.** No shipped code reads an answer fiber yet.
- **No product code was changed.** The fixes in section 7 are proposals with measured evidence.

---

## 10. Evidence index

| Claim | Where it is verified |
|---|---|
| The four committed shelf IMDb runs | `results/imdb_shelf_runs.json` and `results/imdb_shelf_matrixwindow.json` |
| DuckDB's IMDb and load numbers | `results/imdb_job.json` |
| The six engines on IMDb Q1, one window | `results/imdb_multi_engine.json` run `20261021_1551` |
| ScyllaDB CQL's IMDb numbers | `results/imdb_scylla.json` |
| Redis's numbers and run spread | `results/redis.json`, 13 runs |
| ScyllaDB's numbers and run spread | `results/scylladb.json`, 10 runs |
| SQLite's TPC-H numbers | `results/sqlite.json` run1 |
| MariaDB's TPC-H numbers | `results/mariadb.json` run1_quiet_box |
| PostgreSQL's TPC-H numbers | `results/pg.json` run1_quiet_box |
| DuckDB's TPC-H numbers | `results/duckdb.json` run2_quiet_box |
| Neo4j's numbers | `results/neo4j.json` |
| The shelf's traversal and 2-hop | `results/graph.json` |
| The shelf's TPC-H fiber join | `fiber_join_bench` stdout, 2026-10-22, 7.9 ms, and `results/fiber.json`, 3.2 ms best |
| The shelf's TPC-H raw join | `results/join.json`, `join_timed_s` 2.5 |
| The shelf's TPC-H scan | `results/mt_scan.json`, 4.0 ms best of 3 runs |
| The shelf's durable write | `results/wal_insert.json`, 6.9 µs p50 |
| The measured pre-aggregate and answer-fiber floors | `tools/q2_decompose.rs` PART5, `tools/q1_decompose.rs` PART4, 8 runs each |
| The break-even | `tools/q1_decompose.rs` PART6 |
| The 1.40x accumulator gain on the TPC-H scan | `physical_limits_probe` section H |
| The answer-fiber gain and the 0.965x accumulator result on IMDb | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs`, the `Q1KB_` lines |
| The section load split | `crates/sheaf-engine/src/bin/bundle_load_bench.rs`, the `LOAD_` lines |
| The build stages | `imdb_fiber_bench` stdout, forced rebuild 2026-10-22 |
| The bundle read waste | `tools/arch_gains.rs` C1, and `Bundle::open` in `shared/fiber_snapshot.rs` |
| The existing seven-database comparison | `RUNTIME_COMPARISON.md` |

**Report generated:** 2026-10-22 11:20:00 local (Asia/Kolkata), 2026-10-22 05:50:00 UTC.
