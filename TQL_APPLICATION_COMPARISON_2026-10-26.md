# TQL_APPLICATION_COMPARISON_2026-10-26.md - live application comparison with topological queries

**Document date:** 2026-10-27.
**Main measurement window:** 2026-10-26 11:24:59 IST to 2026-10-26 14:25:24 IST. IST is Indian Standard Time, UTC+05:30.
**Verification pass:** 2026-10-27 10:41:01 IST. It re-ran the ShelfDB binaries and the TQL optimizer.
**Full span:** 2026-10-26 11:24:59 IST to 2026-10-27 10:43:32 IST (23 h 18 min 33 s).
**Branch:** `fiber-bundle-replacement`, HEAD `d3ea5b0`.
**Host:** `DLUX-INLT-042`, Intel Core i7-1165G7 (4 cores, 8 threads), 15.8 GB RAM, Windows 11 Pro 10.0.26200, NVMe SSD (Kingston OM8SEP4512Q-AA). The host is shared with other processes.
**Writing standard:** ASD-STE100 Simplified Technical English.

---

## 1. What this document answers

This document answers one question: how fast is each database on the same application query, when the query shape comes from a Topological Query Language (TQL) statement. A TQL statement is a declarative query over a cellular complex. Every performance number below comes from a tracked file in `sheaf-db/tests/e2e/realtime_bench/results/`, or from a division shown in the row. Machine facts come from the host itself.

---

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement names cells, a hypercell, a filter, a harmonic solve, and an output section.
- **Native equivalent:** the same query shape written in the query language of one engine, for example SQL, Cypher, CQL, or a Redis Lua script.
- **Answer section:** a 24 B to 226 B record that the ShelfDB bundle build writes once, and a query reads later.
- **Bundle:** the sectioned file at `D:\bench\data\imdb_fiber_bundle.bin`. It holds typed column sections and the answer sections.
- **Speedup ratio:** the opponent time divided by the ShelfDB time. A ratio above 1.00 means the opponent is slower.
- **Gate:** an automatic check that compares the engine result with the truth value. A gate value of PASS means the result is exact.
- **Quiet pass:** the 2026-10-26 11:44 pass, run directly after the runner had executed the same binaries.
- **Shared pass:** the 2026-10-27 10:41 pass, run while other processes used the host.

---

## 3. Why the method is this way

The method follows five rules. Each rule has a reason.

1. The comparison names the query shape in TQL. TQL is the only language that states the shape for ShelfDB, so the shape has one owner.
2. The TQL text and the native text both appear in this document. A reader can then see that the engines answer the same shape.
3. The TQL optimizer gives a cost estimate only. The TQL-to-fiber execution bridge (OPT-004) is a plan, not a built component. This report never prints an optimizer cost as an execution time.
4. The execution side is each engine on its own data, plus the ShelfDB binary that answers the same shape. A TQL executor for arbitrary bench data does not exist today.
5. Every engine is gated on the same truth value. A fast wrong answer cannot enter the tables.

The run used one host. The seven external engines, the twelve ShelfDB binaries, the TQL optimizer, and the IMDb application queries therefore share one machine state.

---

## 4. The four TQL query shapes and their native equivalents

The four TQL statements below are the cases in `sheaf-db/tests/e2e/realtime_bench/tql_workloads.conf`. The optimizer parses each statement and lowers it to a topological intermediate representation.

Case 1, IMDb join and filter.

```
MATCH HYPERCELL c: Casting OVER (t: Title, p: Person) WHERE t.year > 2000 AND t.rating > 75 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name, t.rating, SECTION_ENERGY };
```

Case 2, fraud network two-hop traversal.

```
MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant, c: Country) WHERE a.balance > 1000 AND t.risk == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, m.category, SECTION_ENERGY };
```

Case 3, TPC-H analytical filter and aggregation.

```
MATCH HYPERCELL l: LineItem OVER (o: Order, p: PartSupp) WHERE l.shipdate < 19980902 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { l.line_id, l.quantity, SECTION_ENERGY };
```

Case 4, key-value direct point lookup.

```
MATCH HYPERCELL t: Payment OVER (a: Account) WHERE a.acct_id == 1001 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, SECTION_ENERGY };
```

Table 1 maps each TQL case to the native query that this run measured. Rows are the four cases. Columns give the native engine, the native query, and the measured source.

| Case | TQL shape | Native engine | Native query | Measured source |
|---|---|---|---|---|
| 1 | Casting over Title and Person, filter on year and rating | DuckDB, PostgreSQL, SQLite, MariaDB, Redis, ScyllaDB | `SELECT COUNT(*), AVG(r.averagerating), SUM(r.numvotes) FROM title_basics b JOIN title_ratings r ON r.tconst=b.tconst WHERE b.titletype='movie' AND b.startyear>=2000 AND r.averagerating>=7.5` | `imdb_job.json`, `imdb_multi_engine.json` |
| 2 | Payment over Account, Merchant, and Country, filter on balance and risk | Neo4j | `MATCH (p:Person)<-[:PRINCIPAL]-(t:Title) WITH p, count(t) AS d WHERE d > 1 RETURN sum(d*(d-1)) AS pairs` | `neo4j.json`, `imdb_multi_engine.json` |
| 3 | LineItem over Order and PartSupp, filter on shipdate | DuckDB, PostgreSQL, SQLite, MariaDB | `SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0` | `duckdb.json`, `pg.json`, `sqlite.json`, `mariadb.json` |
| 4 | Payment over Account, equality on acct_id | Redis, ScyllaDB | `HGETALL` of two hashes in a Lua script, and `SELECT tconst, avg, votes FROM imdb.by_year WHERE year=2000 AND tconst > 'tt0000' LIMIT 10000` | `redis.json`, `scylladb.json` |

The TQL statement emits a row section after a harmonic solve. The native queries above aggregate or traverse. The shared part is the scan, the filter, and the join. The output clause differs, because no external engine has a harmonic solve.

Two boundary differences remain. Case 1 filters on `t.year > 2000` and `t.rating > 75`, and the native IMDb query filters on `startyear >= 2000` and `averagerating >= 7.5`. The TQL case uses the integer 75 for the rating, and the native query uses 7.5, so the optimizer case treats the rating as a scaled integer. Case 3 filters on `l.shipdate < 19980902`, and the native TPC-H scan filters on `l_extendedprice > 100.0`. Read each ratio as the cost of the shared shape, not as the cost of an identical predicate.

---

## 5. Table 2: TPC-H SF1 scan and aggregate

Rows are database engines. Columns are the measured query time, the repetition count, the ratio in each pass, and the gate. The dataset is TPC-H SF1 with 6,001,215 `lineitem` rows. Each ratio divides the opponent time by the ShelfDB row of the same pass. The source is `shelf_bins_20261026_quiet.json`, `shelf_bins_20261026.json`, and the four engine JSON files.

| Engine | Query time (ms) | Repetitions | Ratio, quiet pass | Ratio, shared pass | Gate |
|---|---|---|---|---|---|
| DuckDB 1.5.4 | 11.6 | 5 | 6.8 | 5.3 | PASS |
| PostgreSQL 18.6 | 321.4 | 5 | 189.1 | 146.1 | PASS |
| MariaDB 13.0.2 | 1,279.3 | 5 | 752.5 | 581.5 | PASS |
| SQLite 3 | 1,354.5 | 5 | 796.8 | 615.7 | PASS |
| ShelfDB `tpch_scan_bench` | 6.2 quiet, 11.4 shared | 1 | 3.6 | 5.2 | PASS quiet, MISS shared |
| ShelfDB `fused_scan_bench` | 1.7 quiet, 2.2 shared | 3 | 1.00 (reference) | 1.00 (reference) | PASS |

The external engine times come from one pass. The ShelfDB times come from two passes, so each ratio carries the ShelfDB spread: 1.84x for the scan row and 1.29x for the fused row.

The work is one pass over the 6,001,215 rows. The ShelfDB binaries compute `SUM(price*(1-discount))`. The SQL engines compute `AVG` and `SUM` of `l_extendedprice` with the filter `l_extendedprice > 100.0`. The predicates differ, so read the ratio as one-pass scan cost, not as an identical predicate.

The raw ShelfDB scan sits at the DuckDB boundary. It measured 6.2 ms in the quiet pass and 11.4 ms in the shared pass, against the 6.4 ms target. The fused scan wins at both times, because the build materializes the derived column at load and streams 4 B per row.

---

## 6. Table 3: TPC-H SF1 three-table join

Rows are database engines. Columns are the measured join time, the repetition count, the ratio to the ShelfDB query time in each pass, the ratio to the ShelfDB total in each pass, and the gate. The query joins `customer`, `orders`, and `lineitem`, filters `l_shipdate < DATE '1995-01-01'`, groups by `c_name`, and returns the top 100 rows. The source is `shelf_bins_20261026_quiet.json`, `shelf_bins_20261026.json`, and the four engine JSON files.

| Engine | Join time (ms) | Repetitions | Ratio, query only, quiet | Ratio, query only, shared | Ratio, load plus query, quiet | Ratio, load plus query, shared | Gate |
|---|---|---|---|---|---|---|---|
| DuckDB 1.5.4 | 167.6 | 3 | 45.3 | 17.5 | 5.8 | 2.0 | PASS |
| PostgreSQL 18.6 | 2,383.9 | 3 | 644.3 | 248.3 | 82.2 | 28.9 | PASS |
| MariaDB 13.0.2 | 23,086.1 | 3 | 6,239.5 | 2,404.8 | 796.1 | 279.8 | PASS |
| SQLite 3 | 124,056.0 | 1 | 33,528.6 | 12,922.5 | 4,277.8 | 1,503.7 | PASS |
| ShelfDB `fiber_join_bench` | 3.7 query and 29.0 total quiet, 9.6 query and 82.5 total shared | 1 | 1.00 (reference) | 1.00 (reference) | 1.00 (reference) | 1.00 (reference) | PASS |

The ShelfDB join walks a pre-folded fiber bundle with no hash table. The load is the bundle open and the section fetch: 25.2 ms in the quiet pass and 72.9 ms in the shared pass. The SQL engines exclude their one-time table load, because the tables are reused from an earlier pass. The load-plus-query columns give the fair view, and the query-only columns give the view that the TPC-H runner prints.

SQLite has one repetition, because one join took 124 s. SQLite has no hash join.

---

## 7. Table 4: key-value point lookup

Rows are database engines. Columns are the median latency, the 99th percentile latency, and the count. The unit is microseconds. The workload is 20,000 random `customer` keys, except Neo4j and ScyllaDB at 5,000 keys. The source is the engine JSON files.

| Engine | Median (us) | 99th percentile (us) | Lookups |
|---|---|---|---|
| SQLite 3 | 18.7 | 286.6 | 20,000 |
| PostgreSQL 18.6 | 69.5 | 292.0 | 20,000 |
| MariaDB 13.0.2 | 139.5 | 445.9 | 20,000 |
| Redis 5.0.14.1 | 270.5 | 451.8 | 20,000 |
| Neo4j 5.26.32 | 1,430.3 | 4,737.4 | 5,000 |
| DuckDB 1.5.4 | 1,498.3 | 2,682.6 | 20,000 |
| ScyllaDB 3.0.8 | 15,912.5 | 17,581.9 | 5,000 |

ShelfDB has no point-lookup binary in this run. The related ShelfDB binary is `range_slice_bench`, and it measures a 10,000-row range slice, not a point lookup. This report gives no ShelfDB ratio for this table.

---

## 8. Table 5: graph traversal

Rows are the two graph systems. Columns are the full traversal time, the two-hop median latency, the pass that produced the value, the ratio in each pass, and the gate. The workload is 600,000 paths for the full rollup and 200 samples for the two-hop case. The source is `shelf_bins_20261026_quiet.json`, `shelf_bins_20261026.json`, and `neo4j.json`.

| Engine | Full traversal (ms) | Two-hop median (us) | Pass | Ratio, full | Ratio, two-hop | Gate |
|---|---|---|---|---|---|---|
| Neo4j 5.26.32 | 485.9 | 1,650.4 | 2026-10-26 11:32, one pass | 10.7 quiet, 3.7 shared | 2,063.0 quiet, 750.2 shared | PASS |
| ShelfDB `graph_bench` | 45.2 quiet, 130.5 shared | 0.8 quiet, 2.2 shared | two passes | 1.00 (reference) | 1.00 (reference) | PASS |

The full-traversal ratio is 485.9 / 45.2 = 10.7 in the quiet pass and 485.9 / 130.5 = 3.7 in the shared pass. The two-hop ratio is 1,650.4 / 0.8 = 2,063.0 in the quiet pass and 1,650.4 / 2.2 = 750.2 in the shared pass. Neo4j ran once, at 11:32, inside the quiet window, so both ratios use the same Neo4j value.

The two systems hold different graphs. ShelfDB loads 4,350,000 cells into 151.4 MB of memory, in 1.2 s quiet and 3.6 s shared. Neo4j holds the TPC-H derived graph of 150,000 customers, 1,500,000 orders, and 600,000 lines. Compare the ratio, not the absolute graph size.

---

## 9. Table 6: IMDb Q1, the application query

Rows are database engines. Columns are the query time, the ratio, the gate, and the remark. The dataset is the full IMDb TSV set with seven tables and 12,866,375 title rows. The query is Case 1 of Table 1: filter movies from year 2000, filter ratings at 7.5 and above, then count the rows, sum the votes, and average the rating. The source is `imdb_job.json`, `imdb_multi_engine.json`, and `imdb_shelf.json`.

| Engine | Query time (ms) | Ratio (opponent time / ShelfDB time) | Gate | Remark |
|---|---|---|---|---|
| ScyllaDB 3.0.8 | 125.4 | 1,739 | PASS | Join resolved at write time |
| DuckDB 1.5.4 | 553.3 | 7,674 | PASS | Columnar scan and hash join |
| SQLite 3 | 636.6 | 8,829 | PASS | Embedded row store |
| Redis 5.0.14.1 | 704.9 | 9,777 | PASS | Lua scan over two hashes, no join |
| MariaDB 13.0.2 | 1,074.4 | 14,902 | PASS | Row store, InnoDB |
| PostgreSQL 18.6 | 2,674.1 | 37,089 | PASS | Row store, hash join |
| ShelfDB `imdb_fiber_bench` | 0.0721 | 1.00 (reference) | PASS | Reads the 24 B answer section: 0.072 load plus 0.0001 decode |

Neo4j is a graph engine and has no Q1 row. Table 5 gives its graph number. MongoDB was uninstalled, so it has no row.

The ShelfDB ratio is valid only after the one-time bundle build. That build is not in the 0.0721 ms, and section 11 gives the build and the compute cost. The ratio in this table answers one narrow question: how fast is a repeat answer after the build. The answer read measured 0.056 ms in the 14:25 pass and 0.072 ms in the later pass, so the ratio carries a 1.3x spread. The ShelfDB row is 0.0721 ms, the sum of the 0.072 ms load and the 0.0001 ms decode, and every ratio in this table divides by that sum.

The truth value for every gate is count 44,906, votes 294,435,444, and mean rating 8.145506168.

---

## 10. Table 7: IMDb Q2 to Q4, DuckDB against PostgreSQL

Rows are the three complex queries. Columns are the DuckDB time, the PostgreSQL time, and the ratio. Both engines ran the same SQL text and produced the same md5 result hash. The ratio divides the PostgreSQL time by the DuckDB time. The source is `imdb_job.json`.

| Query | Shape | DuckDB time (ms) | PostgreSQL time (ms) | Ratio (PostgreSQL / DuckDB) | Result gate |
|---|---|---|---|---|---|
| Q2 | Four-table top ten, 3 joins and a sort | 4,466.3 | 37,727.1 | 8.4 | PASS, md5 match |
| Q3 | Four-table decade rollup with HAVING | 1,837.4 | 19,029.6 | 10.4 | PASS, md5 match |
| Q4 | Five-table job-style query with HAVING | 1,506.7 | 20,147.3 | 13.4 | PASS, md5 match |

The TQL grammar cannot express Q2, Q3, or Q4 today. Section 15 gives the reason. These three rows are therefore an engine-to-engine reference, not a TQL comparison.

---

## 11. Table 8: ShelfDB answer read against ShelfDB compute

Rows are the ShelfDB query paths. Columns are the section size, the load time, the query time, and what the number measures. All times are in milliseconds. The source is the tracked `imdb_shelf.json`, written on 2026-10-26 at 16:57:43 by a later ShelfDB pass.

| Path | Section size (B) | Load (ms) | Query (ms) | What the number measures |
|---|---|---|---|---|
| Q1 answer | 24 | 0.072 | 0.0001 | Decode of a stored answer, at the 100 ns print resolution |
| Q2 answer | 226 | 0.008 | 0.0076 | Decode of a stored top-ten list |
| Q3 answer | 164 | 0.013 | 0.0010 | Decode of a stored decade rollup |
| Q4 computed | Not stored | 934.701 | 943.8637 | Five-table join and aggregation over bundle sections |
| Q1, Q2, Q3 recomputed by the scan oracle | Not stored | Not applicable | 17,894 | Verification scan of the same three questions |

The scan oracle is the compute reference for Q1. It runs after the timed answer path, and it proves that the stored answer equals the computed answer. The oracle reports one total for three questions, so this report does not split it per query. The total was 17,894 ms.

The bundle build is the one-time cost that makes the answer sections. The recorded build is 164.8 s in the IMDb Q1 row of `RUNTIME_COMPARISON.md`. This run reused an existing bundle, so the build did not run again.

---

## 12. Table 9: TQL optimizer estimates

Rows are the four TQL cases. Columns are the unoptimized cost, the optimized cost, the gain, and the gate. The cost unit is a relative unit from the cost model, not a time. The source is `shelf_bins_20261026.json`.

| Case | Shape | Unoptimized cost (relative unit) | Optimized cost (relative unit) | Gain (ratio) | Gate |
|---|---|---|---|---|---|
| 1 | IMDb join and filter | 1,110,000.00 | 209,981.00 | 5.29 | PASS |
| 2 | Fraud two-hop | 655,000.00 | 134,163.50 | 4.88 | PASS |
| 3 | TPC-H filter and aggregate | 6,660,000.00 | 2,459,820.00 | 2.71 | PASS |
| 4 | Key-value point lookup | 101,000.00 | 11,008.50 | 9.17 | PASS |

The cost model uses the row counts in `tql_workloads.conf`: 100,000 rows for case 1, 50,000 for case 2, 600,000 for case 3, and 10,000 for case 4. These values are workload inputs, not corpus cardinalities. Case 3 uses 600,000 rows while the TPC-H table holds 6,001,215 rows.

Read the gain as the ratio of the naive plan cost to the pruned plan cost. Do not read it as a speedup, and do not place it beside a measured millisecond value. The four gains are identical in the 2026-10-26 and the 2026-10-27 pass, because the cost model is deterministic.

---

## 13. Table 10: coverage, skips, and failures

Rows are the measured units. Columns are the status and the reason. The source is `RUN_SUMMARY_20261026_1144.json`, `RUN_SUMMARY_20261026_1602.json`, and the run logs.

| Unit | Status | Reason |
|---|---|---|
| SQLite, DuckDB, Redis, MariaDB, PostgreSQL, Neo4j, ScyllaDB on TPC-H | OK | Seven engines completed and merged |
| Twelve ShelfDB binaries | OK | All twelve report OK in both runner summaries |
| MongoDB | Not run | Uninstalled on 2026-10-20 |
| ScyllaDB, first IMDb attempt at 12:19 | ERROR | Write timeout, no replica response |
| ScyllaDB, retry at 12:19 | PASS | 85.0 ms, same truth value |
| ScyllaDB and Neo4j at 13:44 | ERROR | Containers killed during the PostgreSQL load |
| ShelfDB point lookup | Not run | No point-lookup binary exists |
| TQL execution on bench data | Not run | The OPT-004 bridge is a plan |

---

## 14. The experiment window in order

The list gives the measured step and the clock time in IST.

1. 2026-10-26 11:24:59, experiment start. Docker Desktop start.
2. 11:25:26 to 11:44:09, `py run_all.py --keep-up`. Seven engines and twelve ShelfDB binaries.
3. 11:44:37, full ShelfDB binary output capture, quiet pass.
4. 11:52 to 12:15, first `imdb_complex_bench.py` attempt. PostgreSQL had empty unlogged tables and reloaded `title.akas`. The shell timeout stopped the run.
5. 12:19:06, `imdb_multi_engine_bench.py` with five engines. ScyllaDB write timeout, four gates PASS.
6. 12:19:24, ScyllaDB retry. PASS at 85.0 ms.
7. 12:21:04, `tql_seven_db_bench`. Four cases PASS.
8. 12:20 to 13:19, PostgreSQL reload attempt two. The backend died with exception `0xC000013A`, and the server shut down.
9. 13:19 to about 13:50, PostgreSQL restart and server-side `COPY` of the seven IMDb tables. Recovery took 40 s, and the four largest tables took 829.7 s.
10. 13:21 to 14:16, automatic `VACUUM ANALYZE` of the seven tables.
11. 14:20:26, `imdb_complex_bench.py` with full statistics. Q1 to Q4 PASS.
12. 14:23:50, `imdb_multi_engine_bench.py` with five engines. All gates PASS.
13. 14:25:24, `imdb_fiber_bench`. All gates PASS. End of the main window.
14. 16:02 and 16:57, a later ShelfDB-only pass by another agent. It rewrote `imdb_shelf.json` and `RUN_SUMMARY_20261026_1602.json`.
15. 2026-10-27 10:41:01, verification pass. The four ShelfDB binaries and the TQL optimizer ran again. End of the full span.

---

## 15. Limitations

1. The TQL grammar has no GROUP BY, no HAVING, no arithmetic expression, and no user-defined aggregate. The query structure is `match`, `where`, `overs`, `solve`, and `emit`. IMDb Q2, Q3, and Q4 therefore cannot be written in TQL today, and this report does not invent TQL text for them.
2. The TQL optimizer does not execute a query. The OPT-004 execution bridge is not built. Every TQL cost in Table 9 is an estimate.
3. ShelfDB Q1, Q2, and Q3 are stored answers, not scans. The 0.0721 ms Q1 time is a 24 B read after a 164.8 s build. The 17,894 ms oracle is the compute reference.
4. The Q1 answer read prints 0.0001 ms. The load is 0.072 ms, so read the pair as one answer fetch of about 0.07 ms.
5. The predicates differ between the TPC-H scan rows. ShelfDB sums `price*(1-discount)`, and the SQL engines sum `l_extendedprice` with a filter.
6. Two engines hold different graphs. The Neo4j and ShelfDB graph ratio compares a six hundred thousand edge traversal with a 4.35 million cell in-memory graph.
7. The ShelfDB binaries vary by 1.3x to 2.9x between the two passes: fused scan 1.7 to 2.2 ms, join total 29.0 to 82.5 ms, and full traversal 45.2 to 130.5 ms. The host is shared, and these binaries are memory-bandwidth bound. The external engine times come from one pass, so every ShelfDB ratio carries that spread.
8. The external engines also vary. Redis measured 208.6 ms in `run_20261026_1344` and 704.9 ms in `run_20261026_1423`, a 3.4x spread inside one file.
9. PostgreSQL truncates an unlogged table after an unclean shutdown. The first reload lost 49,004,343 `title.akas` rows, and the crash at 13:19 lost the `title_basics` rows. The final load verified all seven counts against DuckDB.
10. Docker killed the ScyllaDB and Neo4j containers at about 13:39, during the PostgreSQL load. The PostgreSQL log records a failed shared memory reservation at 13:38:11, and memory pressure is the likely cause. The 14:23 run used restarted containers.
11. `duckdb_load_s` and `pg_load_s` in `imdb_job.json` are not load times in this run. The tables already existed, so those fields measure the count verification. The real PostgreSQL load time is 829.7 s for four tables, plus the earlier three tables, as `results/pg_imdb_load_session_20261026.txt` records.
12. The run logs in `results/*.log` are excluded by `.gitignore`. The committed evidence is the JSON files. `shelf_bins_20261026_quiet.json` holds the quiet pass, parsed from that pass log, and `shelf_bins_20261026.json` holds the shared pass with its raw stdout lines.

---

## 16. Repository changes in this window

1. `AGENTS.md`. Added project rules 5, 6, and 7 for TQL comparison. Added the row for this document to the document table.
2. `TQL_APPLICATION_COMPARISON_2026-10-26.md`. This document.
3. `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json`. The shared pass: four binaries and the four TQL gains.
4. `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json`. The quiet pass: five binaries, parsed from the pass log.
5. `sheaf-db/tests/e2e/realtime_bench/_capture_shelf_bins.py`. The capture tool for both passes.
6. `sheaf-db/tests/e2e/realtime_bench/_check_table_values.py`. The checker that prints every report value from the result files.
7. `sheaf-db/tests/e2e/realtime_bench/results/*.json`. Nine tracked engine files gained a run entry: `sqlite`, `duckdb`, `redis`, `mariadb`, `pg`, `neo4j`, `scylladb`, `imdb_multi_engine`, and `imdb_shelf`. The file `imdb_job.json` was replaced by the new single run.
8. `sheaf-db/tests/e2e/realtime_bench/results/RUN_SUMMARY_20261026_1144.json`. The runner summary.
9. `sheaf-db/tests/e2e/realtime_bench/results/pg_imdb_load_session_20261026.txt`. The PostgreSQL load record.

No source code changed.

---

## 17. Evidence

The table lists the file and the claim that it supports. Rows are the files. Columns are the claim and the command.

| File | Claim | Command |
|---|---|---|
| `results/RUN_SUMMARY_20261026_1144.json` | Seven engines and twelve binaries completed | `py run_all.py --keep-up` |
| `results/sqlite.json` to `results/scylladb.json` | TPC-H times per engine | read of each `run*` entry |
| `results/shelf_bins_20261026_quiet.json` | ShelfDB scan 6.2 ms, fused 1.7 ms, join 29.0 ms, graph 45.2 ms | `py _capture_shelf_bins.py --from-log results/shelf_bins_20261026.log --stamp "2026-10-26 11:44 UTC+05:30" --out shelf_bins_20261026_quiet.json` |
| `results/shelf_bins_20261026.json` | ShelfDB scan 11.4 ms, fused 2.2 ms, join 82.5 ms, graph 130.5 ms | `py _capture_shelf_bins.py` |
| `_check_table_values.py` | Every value in Tables 2 to 9 | `py _check_table_values.py` |
| `results/pg_imdb_load_session_20261026.txt` | PostgreSQL COPY rows, seconds, and final counts | psql `\timing` output of the load session |
| `results/imdb_job.json` | IMDb Q1 to Q4, DuckDB and PostgreSQL, md5 gates | `py imdb_complex_bench.py` |
| `results/imdb_multi_engine.json` | IMDb Q1 across five engines, truth gates | `py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j` |
| `results/imdb_shelf.json` | Answer read times, oracle 17,894 ms | `cargo run --release --bin imdb_fiber_bench` |
| `results/shelf_bins_20261026.json` | Four optimizer gains, 5.29x to 9.17x | `cargo run --release --bin tql_seven_db_bench` |

Every ratio in this document is a division of two numbers in these files. Every gate value is a PASS or MISS string in these files.
