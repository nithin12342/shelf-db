# PLAN_SQLITE_TPCH_SF1_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan compares SQLite with ShelfDB on scale factor 1 (SF1) of the Transaction Processing Performance Council H benchmark (TPC-H). The plan names the source, the data, the cases, the commands, and the gate. The plan does not run the benchmark.

**TQL:** Topological Query Language, a declarative query over a cellular complex.
**SQL:** Structured Query Language, the standard language for relational databases.

## 2. Original benchmark source

Table 1 answers this question: which link and tool command identify the original benchmark. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Original benchmark link | `https://github.com/gregrahn/tpch-kit` | Extracted `tpch-kit-master/README.md` |
| Official specification link | `https://www.tpc.org/tpch/` | `DATASETS.md`, row 4 |
| Data generator | `dbgen`. Read `dbgen -h` for the flags. | Extracted `tpch-kit-master/README.md`, "Data generation" |
| Query generation command | `qgen -v -c -d -s 1 > tpch-stream.sql` | Extracted `tpch-kit-master/README.md`, line 85 |
| Environment variables | `DSS_CONFIG`, `DSS_QUERY`, `DSS_PATH` | Extracted `tpch-kit-master/README.md`, "Environment" |

## 3. Data set and subset

Table 2 answers this question: which data and subset the pair uses. Rows are facts. Columns are the fact, the value, and the source.

| Fact | Value | Source |
|---|---|---|
| Data set name | TPC-H scale factor 1 | `DATASETS.md`, row 5 |
| Data size (GB) | 1.101 | `DATASETS.md`, row 5 |
| Lineitem rows (count) | 6,001,215 | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 5 |
| Generated tables | `D:\bench\build\tpch-dbgen\*.tbl` | `DATASETS.md`, row 5 |
| Bundle path | `tpch/sf1_tbl/` | Bundle root `README.md` |
| SQLite database file | `D:\bench\data\sqlite\tpch_sf1.db` | Operator path. `sheaf-db/tests/e2e/realtime_bench/PLAN.md` names `D:\bench\data\sqlite\bench.db`. |

The scan case uses the full `lineitem` table. The join case uses `customer`, `orders`, and `lineitem`. Run a scale factor 0.01 warm-up first. Use scale factor 1 only after that warm-up passes and the laptop stays responsive. This order comes from `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4.

## 4. Test-case documents and specific cases

Table 3 answers this question: which documents name the cases. Rows are documents. Columns are the path, the specific cases, and the text type.

| Document path | Specific cases | Text type |
|---|---|---|
| `recursive test/QUERIES_2026-11-22_15-04-04.md` | TPC-H scan and join family | Exact scan SQL and join description |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Case 3, TPC-H analytical filter | Exact TQL and exact native SQL |
| `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` | Q05, Q06, Q09, Q15 to Q24 | Exact SQL from a plan, not measured |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 2, scan and aggregate, and Table 3, three-table join | Exact scan SQL and join description |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Problem 2, relational join with 6 million lineitem rows | Workload name and timing |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Problem 2, relational join | Workload name and timing |
| `RUNTIME_COMPARISON.md` | Section 3, TPC-H SF1 suite | Metrics and descriptions |
| `ADVERSARIAL_ANALYSIS.md` | Table 3, TPC-H fiber | Measured fiber query |

## 5. Query text

The approved documents give exact text for one scan query and one TQL shape. They give a description for the three-table join. Do not write SQL for the join from this plan.

### 5.1 Exact native SQL for the scan case

The source is `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, case 3, native SQL.

```sql
SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0
```

### 5.2 Exact TQL shape for the scan case

The source is the same file, case 3, TQL. The optimizer does not execute this shape. The execution bridge is a plan.

```text
MATCH HYPERCELL l: LineItem OVER (o: Order, p: PartSupp) WHERE l.shipdate < 19980902 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { l.line_id, l.quantity, SECTION_ENERGY };
```

### 5.3 Join case, description only

`TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 6, gives a description. It joins `customer`, `orders`, and `lineitem`. It filters `l_shipdate < DATE '1995-01-01'`. It groups by `c_name` and returns the top 100 rows. The document gives no exact SQL text for this join. Record the description only.

### 5.4 Predicate difference

The TQL shape filters `l.shipdate < 19980902`. The native SQL filters `l_extendedprice > 100.0`. Read each ratio as the cost of a shared shape, not as an identical predicate. This note comes from `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 4.

## 6. Container image and low-power limits

Table 4 answers this question: which image and limits apply. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Database image | None. SQLite is embedded. | ClickBench `json/sqlite/README.md` |
| Install step | Precompiled command-line shell from `https://sqlite.org/download.html` | Same file |
| Alternative install | `sqlite3` CLI through Scoop | `sheaf-db/tests/e2e/realtime_bench/PLAN.md`, line 75 |
| Optional isolation image | `alpine:3.20`. Install `sqlite` with `apk add --no-cache sqlite`. | Operator choice. No source document names this image. |
| Pull step for the optional image | `docker pull alpine:3.20` | Operator choice |
| Concurrent containers (count) | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container CPUs (count) | 1 | Same section |
| Container memory (GB) | 2 | Same section |
| Benchmark threads (count) | 1 | Same section |
| Maximum step duration (minutes) | 30 | Same section |
| Cooldown (minutes) | 3 | Same section |

## 7. Commands

Table 5 answers this question: which command runs each step. Rows are steps. Columns are the side, the command, and the source.

| Step | Side | Command | Source |
|---|---|---|---|
| Generate the query stream | Original benchmark | `qgen -v -c -d -s 1 > tpch-stream.sql` | Extracted `tpch-kit-master/README.md` |
| Run the stream in SQLite | SQLite | `sqlite3 <database-file> ".read tpch-stream.sql"` | Local command shape. SQL text comes from `qgen`. |
| Run the scan and join matrix | SQLite, historical | `py run_all.py --keep-up` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| Run the fiber join | ShelfDB | `cargo run -p sheaf-engine --bin fiber_join_bench --release` | `ADVERSARIAL_ANALYSIS.md`, section 11 |
| Run the scan | ShelfDB | `cargo run --release --bin tpch_scan_bench` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 2 |
| Run the fused scan | ShelfDB | `cargo run --release --bin fused_scan_bench` | Same table |
| Run one SQL statement | ShelfDB | `cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "<SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md`, line 352 |

Use one build job. The repository record uses `CARGO_BUILD_JOBS=1` and `-j 1`. Do not start a pull while a benchmark runs.

## 8. Recorded baselines

Table 6 answers this question: which historical times the documents record. Rows are engines. ShelfDB is the reference row. Columns are the engine, the workload, the time, the repetition count, and the gate. These values are history from a shared host. Do not use them as targets.

| Engine | Workload | Time (ms) | Repetitions (count) | Gate |
|---|---|---|---|---|
| SQLite 3 | Scan and aggregate, Table 2 | 1,354.5 | 5 | PASS |
| SQLite 3 | Three-table join, Table 3 | 124,056.0 | 1 | PASS |
| ShelfDB `fused_scan_bench` | Scan, quiet pass | 1.7 | 3 | PASS |
| ShelfDB `fiber_join_bench` | Three-table join, query only, quiet pass | 3.7 | 1 | PASS |

## 9. Evidence files

Table 7 answers this question: which files hold the evidence. Rows are files. Columns are the file and the claim it supports.

| File | Claim |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/sqlite.json` | SQLite scan and join times |
| `sheaf-db/tests/e2e/realtime_bench/results/RUN_SUMMARY_20261026_1144.json` | Runner completion |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | ShelfDB scan and join, quiet pass |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | ShelfDB scan and join, shared pass |
| `results/MANUAL_CLI_EXECUTION_20261027.json` | Manual CLI problem 2 timing |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 2 | Five problem workloads |

## 10. Acceptance gate

1. Make sure that one engine runs at a time.
2. Make sure that the container uses one CPU, 2 GB memory, and one thread.
3. Make sure that the loaded `lineitem` count equals 6,001,215.
4. Record the predicate beside each timing. Do not compare different predicates as identical queries.
5. For the join, make sure that the ShelfDB gate reads `gate_fiber_exact=PASS`. `ADVERSARIAL_ANALYSIS.md`, Table 3, records a relative difference of 4.39e-12.
6. Make sure that no load or query step runs longer than 30 minutes.
7. Write the command, the output, the row count, the checksum, and the timing into an evidence file. Put the timestamp, the remote, the branch, and the commit in that file.
8. If a named evidence file is absent, write `unsupported` with the file name. Do not substitute an invented query.

## Warnings

- **CAUTION:** SQLite has no hash join. `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 6, records one repetition for the SQLite join because one run took 124 s.
- **CAUTION:** The recorded ShelfDB binaries vary by 1.3x to 2.9x between passes. The host is shared.
- **NOTE:** A TQL optimizer cost is an estimate. Do not print it as an execution time. The TQL-to-fiber execution bridge is not built.
- **NOTE:** The SQL corpus from `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` is a plan. The documents do not claim that all 24 statements ran on the main branch.
