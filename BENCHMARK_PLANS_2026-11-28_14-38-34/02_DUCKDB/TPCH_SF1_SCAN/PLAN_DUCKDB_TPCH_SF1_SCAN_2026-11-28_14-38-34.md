# DuckDB versus ShelfDB benchmark plan: TPC-H SF1 scan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs DuckDB with ShelfDB on one scan over 6,001,215 Transaction Processing Council benchmark H (TPC-H) lineitem rows. DuckDB runs the recorded Structured Query Language (SQL) text. ShelfDB runs the recorded fused scan binary. This plan does not install software and does not run a benchmark.

## 2. Concept definitions

- **DuckDB:** an embedded columnar database that runs SQL in one process.
- **ShelfDB:** a sheaf database that stores answers in binary cellular storage sections.
- **SQL:** the standard language for relational queries.
- **TQL:** Topological Query Language, a declarative query over a cellular complex.
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
| Truth rows | 6,001,215 `lineitem` rows | `duckdb_bench.py`, line 25 |

## 4. Dataset and subset

Table 2 answers this question: which data does this pair use? Rows are data attributes. Columns are the attribute, the value, and the preparation step.

| Attribute | Value | Preparation step |
|---|---|---|
| Data set | TPC-H scale factor 1 | Generate it locally with `dbgen`. |
| Subset | `lineitem.tbl`, 6,001,215 rows | Run `dbgen -s 1 -f -T L`. |
| Mainframe path | `D:\bench\build\tpch-dbgen\lineitem.tbl` | Read `DATASETS.md`, row 5. |
| Bundle path | `tpch/sf1_tbl/lineitem.tbl` | Extract this path only. |
| Schema | `l_extendedprice DOUBLE, l_discount DOUBLE` | Read `duckdb_bench.py`, line 21. |

## 5. Test-case document and specific cases

Table 3 answers this question: which recorded cases does this pair cover? Rows are sources. Columns are the document path, the specific case, and the recorded result.

| Document path | Specific case | Recorded result |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 5, Table 2 | TPC-H SF1 scan and aggregate | DuckDB 1.5.4, 11.6 ms, 5 repetitions, gate PASS |
| Same table | ShelfDB `tpch_scan_bench` | 6.2 ms quiet, 11.4 ms shared |
| Same table | ShelfDB `fused_scan_bench` | 1.7 ms quiet, 2.2 ms shared |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 2 | Problem 3, analytical column scan | DuckDB 5.60 ms, ShelfDB 3.80 ms un-fused and 2.10 ms fused |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md`, section 3.1, item 3 | Problem 3, loop fusion | Same recorded numbers |
| `recursive test/QUERIES_2026-11-22_15-04-04.md`, section 2 | TPC-H scan and join | Names and source mapping only |

## 6. Exact query text

The recovered text is exact. The SQL text comes from `sheaf-db/tests/e2e/realtime_bench/duckdb_bench.py`, line 76, and from `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 1, case 3.

```sql
SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0
```

The ShelfDB binary computes `SUM(price*(1-discount))`. The TQL case from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, lines 20 to 21, has this text.

```text
MATCH HYPERCELL l: LineItem OVER (o: Order, p: PartSupp) WHERE l.shipdate < 19980902 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { l.line_id, l.quantity, SECTION_ENERGY };
```

The predicates differ. Read the ratio as one-pass scan cost, not as an identical predicate comparison.

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

The tool build commands come from `attached_bundle_review_2026-11-28_14-19-54/docs/tpch-kit-master__tpch-kit-master_README.md`, lines 34 to 40 and 74 to 94.

```bash
git clone https://github.com/gregrahn/tpch-kit.git
cd tpch-kit/dbgen
make MACHINE=LINUX DATABASE=POSTGRESQL
export DSS_CONFIG=$PWD
export DSS_QUERY=$DSS_CONFIG/queries
export DSS_PATH=/data
./dbgen -s 1 -f -T L
```

Table 6 answers this question: which commands produce the two sides of the pair? Rows are sides. Columns are the side, the command, and the expected output.

| Side | Command | Expected output |
|---|---|---|
| Original DuckDB harness | `py duckdb_bench.py` from `sheaf-db/tests/e2e/realtime_bench/` | `results/duckdb.json` with `scan_agg_ms` |
| Matching ShelfDB raw binary | `cargo run --release -p sheaf-engine --bin tpch_scan_bench` | Verbose scan time |
| Matching ShelfDB fused binary | `cargo run --release -p sheaf-engine --bin fused_scan_bench` | `gate_fused_exact=PASS` |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture time, row count, and result checksum for each pass.

## 10. Evidence files

Table 7 answers this question: which files hold the evidence? Rows are files. Columns are the file, the claim, and the action.

| File | Claim | Action |
|---|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/duckdb.json` | Recorded DuckDB time | Read the `scan_agg_ms` entry. |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | Shared pass times | Read the scan entries. |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | Quiet pass times | Read the scan entries. |
| `results/local_docker_benchmark_<timestamp>/02_DUCKDB/TPCH_SF1_SCAN/environment.json` | Image digest and data subset | Write it during the run. |
| Same folder, `pulls.txt` and `container_inspect.txt` | Pull and limit record | Write it during the run. |
| Same folder, `manifest.json`, `load.txt`, `result.csv`, `checksums.txt` | Rows, load, times, checksums | Write it during the run. |
| Same folder, `summary.md` | Remote, branch, top commit, verdict | Write it after the run. |

## 11. Acceptance gate

1. Make sure that one container runs at a time.
2. Make sure that `docker inspect` reports one central processing unit and 2 gigabytes of memory.
3. Make sure that the loaded row count equals 6,001,215 before timing.
4. Keep the warm-up pass outside the timed set.
5. Make sure that each timed result matches the recorded checksum.
6. Write all evidence files from Table 7.
7. Stop the container and wait three minutes before the next pair.
8. Record `unsupported` with the named cause if a step cannot run. Do not write a fake result.

## Warnings

- **CAUTION:** The host is a low-power laptop. Do not run Docker, a release build, and a benchmark together.
- **NOTE:** The SQL predicate and the ShelfDB predicate differ. Do not quote the ratio as an identical query result.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
