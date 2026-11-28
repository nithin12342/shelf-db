# DuckDB versus ShelfDB benchmark plan: TPC-H SF1 join

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs DuckDB with ShelfDB on the three-table Transaction Processing Council benchmark H (TPC-H) scale factor 1 join. DuckDB runs the recorded Structured Query Language (SQL) text. ShelfDB walks a pre-folded fiber bundle. This plan does not install software and does not run a benchmark.

## 2. Concept definitions

- **DuckDB:** an embedded columnar database that runs SQL in one process.
- **ShelfDB:** a sheaf database that stores answers in binary cellular storage sections.
- **Fiber bundle:** a pre-materialized section of connected entity data.
- **SQL:** the standard language for relational queries.
- **Gate:** an automatic comparison of an engine result with a truth value.

## 3. Pair identity

Table 1 answers this question: what identifies this pair? Rows are attributes. Columns are the attribute, the recorded value, and the source.

| Attribute | Recorded value | Source |
|---|---|---|
| Pair | DuckDB versus ShelfDB | This plan |
| Benchmark family | Transaction Processing Council benchmark H (TPC-H) | `BENCHMARK_SOURCES.md`, row 3 |
| Original link | `https://www.tpc.org/tpch/` | `BENCHMARK_SOURCES.md`, row 1 |
| Tool link | `https://github.com/gregrahn/tpch-kit` | Master index, Table 3 |
| Scale factor | 1, named SF1 | `DATASETS.md`, row 5 |
| Join shape | `customer`, `orders`, and `lineitem` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 6 |

## 4. Dataset and subset

Table 2 answers this question: which data does this pair use? Rows are data attributes. Columns are the attribute, the value, and the preparation step.

| Attribute | Value | Preparation step |
|---|---|---|
| Data set | TPC-H scale factor 1 | Generate it locally with `dbgen`. |
| Subset | `customer.tbl`, `orders.tbl`, and `lineitem.tbl` | Run `dbgen -s 1`. |
| Row counts | 150,000 customers, 1,500,000 orders, 6,001,215 line items | Read `duckdb_bench.py`, line 25. |
| Mainframe path | `D:\bench\build\tpch-dbgen\*.tbl` | Read `DATASETS.md`, row 5. |
| Bundle path | `tpch/sf1_tbl/` | Extract this path only. |

## 5. Test-case document and specific cases

Table 3 answers this question: which recorded cases does this pair cover? Rows are sources. Columns are the document path, the specific case, and the recorded result.

| Document path | Specific case | Recorded result |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 6, Table 3 | TPC-H SF1 three-table join | DuckDB 1.5.4, 167.6 ms, 3 repetitions, gate PASS |
| Same table | ShelfDB `fiber_join_bench` | 3.7 ms query and 29.0 ms total, quiet pass |
| Same table | ShelfDB `fiber_join_bench` | 9.6 ms query and 82.5 ms total, shared pass |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 2 | Problem 2, relational join over 6 million line items | DuckDB 158.30 ms, ShelfDB 8.70 ms |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md`, section 3.1, item 2 | Problem 2, raw join and fiber join | DuckDB 158.30 ms, ShelfDB 2,500 ms raw and 5.50 ms fiber |
| `RUNTIME_COMPARISON.md`, section 3 | TPC-H SF1 suite join | DuckDB 419 ms, ShelfDB fiber join 3.6 ms |
| `recursive test/QUERIES_2026-11-22_15-04-04.md`, section 2 | TPC-H scan and join | Names and source mapping only |

## 6. Exact query text

The join text is exact. It comes from `sheaf-db/tests/e2e/realtime_bench/duckdb_bench.py`, lines 63 to 65. The description in `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 6, matches it.

```sql
SELECT c_name, SUM(l_extendedprice) FROM customer JOIN orders ON o_custkey=c_custkey
        JOIN lineitem ON l_orderkey=o_orderkey WHERE l_shipdate < DATE '1995-01-01'
        GROUP BY c_name ORDER BY 2 DESC LIMIT 100
```

The ShelfDB binary walks a pre-folded fiber bundle with no hash table. The SQL engines exclude their one-time table load because the tables come from an earlier pass. Report the query time and the load-plus-query time as separate columns.

## 7. Container image and install step

Table 4 answers this question: which image and install step provide DuckDB? Rows are items. Columns are the item, the value, and the reason.

| Item | Value | Reason |
|---|---|---|
| Image | `python:3.12-slim` | DuckDB has no server image. It runs in one process. |
| Pull step | `docker pull python:3.12-slim` | Pull one image at a time. |
| Install step | `pip install duckdb` | Install the embedded library. |
| Version record | Record `duckdb.__version__` in `environment.json` | The documents name 1.5.4. |
| Data mount | `-v <subset directory>:/data:ro` | Mount only the named subset. |

## 8. Low-power limits

Table 5 answers this question: which limits govern the local run? Rows are limits. Columns are the limit, the value, and the command or action.

| Limit | Value | Command or action |
|---|---|---|
| Concurrent containers | 1 | Stop all other containers first. |
| Central processing unit | 1 | Use `--cpus=1`. |
| Memory | 2 gigabytes | Use `--memory=2g`. |
| Threads | 1 | Use `SET threads=1;` in the DuckDB session. |
| Long step | 30 minutes maximum | Stop the step at the limit. |
| Cooldown | 3 minutes | Wait before the next pair. |

## 9. Commands

The tool build commands come from `attached_bundle_review_2026-11-28_14-19-54/docs/tpch-kit-master__tpch-kit-master_README.md`, lines 34 to 40.

```bash
git clone https://github.com/gregrahn/tpch-kit.git
cd tpch-kit/dbgen
make MACHINE=LINUX DATABASE=POSTGRESQL
export DSS_CONFIG=$PWD
export DSS_QUERY=$DSS_CONFIG/queries
export DSS_PATH=/data
./dbgen -s 1
```

Table 6 answers this question: which commands produce the two sides of the pair? Rows are sides. Columns are the side, the command, and the expected output.

| Side | Command | Expected output |
|---|---|---|
| Original DuckDB harness | `py duckdb_bench.py` from `sheaf-db/tests/e2e/realtime_bench/` | `results/duckdb.json` with `join_ms` |
| Matching ShelfDB binary | `cargo run --release -p sheaf-engine --bin fiber_join_bench` | Cached query near 7.9 ms and `gate_fiber_exact=PASS` |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the query time, the bundle load time, the row count, and the result checksum for each pass.

## 10. Evidence files

Table 7 answers this question: which files hold the evidence? Rows are files. Columns are the file, the claim, and the action.

| File | Claim | Action |
|---|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/duckdb.json` | Recorded DuckDB join time | Read the `join_ms` entry. |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | Shared pass join time and load | Read the join entries. |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | Quiet pass join time and load | Read the join entries. |
| `ADVERSARIAL_ANALYSIS.md`, Table 3 in section 3 | Cached 7.9 ms query and 40.6 MB bundle | Read the recorded values. |
| `results/local_docker_benchmark_<timestamp>/02_DUCKDB/TPCH_SF1_JOIN/environment.json` | Image digest and data subset | Write it during the run. |
| Same folder, `pulls.txt` and `container_inspect.txt` | Pull and limit record | Write it during the run. |
| Same folder, `manifest.json`, `load.txt`, `result.csv`, `checksums.txt` | Rows, load, times, checksums | Write it during the run. |
| Same folder, `summary.md` | Remote, branch, top commit, verdict | Write it after the run. |

## 11. Acceptance gate

1. Make sure that one container runs at a time.
2. Make sure that `docker inspect` reports one central processing unit and 2 gigabytes of memory.
3. Make sure that the loaded row counts match Table 2 before timing.
4. Keep the warm-up pass outside the timed set.
5. Make sure that the 100 returned rows match the recorded checksum.
6. Report query time and load-plus-query time in separate columns.
7. Write all evidence files from Table 7.
8. Stop the container and wait three minutes before the next pair.
9. Record `unsupported` with the named cause if a step cannot run. Do not write a fake result.

## Warnings

- **CAUTION:** The host is a low-power laptop. Do not run Docker, a release build, and a benchmark together.
- **NOTE:** DuckDB excludes its table load. ShelfDB includes a bundle load. Report both views.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
