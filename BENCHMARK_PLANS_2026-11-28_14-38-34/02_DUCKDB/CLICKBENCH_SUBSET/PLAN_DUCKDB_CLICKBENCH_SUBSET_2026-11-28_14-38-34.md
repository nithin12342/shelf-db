# DuckDB versus ShelfDB benchmark plan: ClickBench subset

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs DuckDB with ShelfDB on a ClickBench data subset. The benchmark publishes 43 Structured Query Language (SQL) queries in one file. The approved documents give no query text for those queries. This plan records the query count only. It does not invent SQL. It does not install software and does not run a benchmark.

## 2. Concept definitions

- **DuckDB:** an embedded columnar database that runs SQL in one process.
- **ShelfDB:** a sheaf database that stores answers in binary cellular storage sections.
- **ClickBench:** an analytical benchmark over one flat clickstream table.
- **SQL:** the standard language for relational queries.
- **Gate:** an automatic comparison of an engine result with a truth value.

## 3. Pair identity

Table 1 answers this question: what identifies this pair? Rows are attributes. Columns are the attribute, the recorded value, and the source.

| Attribute | Recorded value | Source |
|---|---|---|
| Pair | DuckDB versus ShelfDB | This plan |
| Benchmark family | ClickBench | `BENCHMARK_SOURCES.md`, row 3 |
| Original link | `https://github.com/ClickHouse/ClickBench` | Master index, Table 3 |
| Result dashboard | `https://benchmark.clickhouse.com/` | Extracted ClickBench README, line 3 |
| Query count | 43 queries in `queries.sql` | Extracted ClickBench README, line 49 and line 83 |

## 4. Dataset and subset

Table 2 answers this question: which data does this pair use? Rows are data attributes. Columns are the attribute, the value, and the preparation step.

| Attribute | Value | Preparation step |
|---|---|---|
| Data set | ClickBench hits, 99,997,497 rows in the full set | Read extracted ClickBench README, line 62. |
| Source file | `hits.parquet` | Download from `https://datasets.clickhouse.com/hits_compatible/hits.parquet`. |
| Low-power subset | First 100,000 rows | Read `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4.10. |
| Table shape | One flat table | Read extracted ClickBench README, line 60. |
| Data mount | `-v <subset directory>:/data:ro` | Mount the subset only. |

## 5. Test-case document and specific cases

Table 3 answers this question: which recorded cases does this pair cover? Rows are sources. Columns are the document path, the specific case, and the recorded detail.

| Document path | Specific case | Recorded detail |
|---|---|---|
| `BENCHMARK_SOURCES.md`, row 3 | ClickBench for the columnar engine | Names the original benchmark link |
| `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, section 4, item 6 | ClickBench workload | Analytical and nested JSON queries |
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4.10 | ClickBench subset | 100,000 rows, not the full set |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md`, line 83 | The 43 queries in `queries.sql` | The file holds the query text |
| `master index`, Table 3 | ClickBench source link | `https://github.com/ClickHouse/ClickBench` |

The specific test cases are the 43 numbered queries in the upstream `queries.sql` file. The approved documents do not name individual queries. This plan does not name them. The operator must read that file at run time and must record the selected query numbers in `manifest.json`.

## 6. Query text

The extracted documentation gives the file names and the query count. It gives no SQL text. `recursive test/QUERIES_2026-11-22_15-04-04.md` does not list ClickBench text. This plan gives names only. The operator must use the exact text from `queries.sql`. Do not write new SQL. Do not use pre-aggregated tables, because the extracted ClickBench README, line 156, discourages them.

## 7. Container image and install step

Table 4 answers this question: which image and install step provide DuckDB? Rows are items. Columns are the item, the value, and the reason.

| Item | Value | Reason |
|---|---|---|
| Image | `python:3.12-slim` | DuckDB has no server image. It runs in one process. |
| Pull step | `docker pull python:3.12-slim` | Pull one image at a time. |
| Install step | `pip install duckdb` | Install the embedded library. |
| Version record | Record `duckdb.__version__` in `environment.json` | The documents name 1.5.4. |
| Upstream scripts | `install`, `load`, `query`, `create.sql`, `queries.sql`, `run.sh`, `benchmark.sh` | Read extracted ClickBench README, lines 78 to 85. |

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

Table 6 answers this question: which commands produce the two sides of the pair? Rows are sides. Columns are the side, the command, and the expected output.

| Side | Command | Expected output |
|---|---|---|
| Original DuckDB benchmark | `./benchmark.sh` from one ClickBench system directory | Load time, data size, and 43 runtime lines |
| Original query loop | `./run.sh` from the same directory | Three runs for each query |
| Matching ShelfDB command | `cargo run --release -p sheaf-app -- --db data\shelf_clickbench sql "<exact text from queries.sql>"` | One result and checksum per query |

The ShelfDB command uses the SQL interface shape from `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md`, line 148. The operator must put exact query text in the placeholder. The command must not carry invented SQL. Run one warm-up pass. Do not time it. Use the second and third runs as the hot times, as the extracted ClickBench README, lines 163 to 172, instructs. Capture time, row count, and result checksum for each query.

## 10. Evidence files

Table 7 answers this question: which files hold the evidence? Rows are files. Columns are the file, the claim, and the action.

| File | Claim | Action |
|---|---|---|
| `BENCHMARK_SOURCES.md`, row 3 | DuckDB link to ClickBench | Read the recorded link. |
| Extracted ClickBench README, lines 78 to 85 | Required output format | Read the load, size, and runtime rules. |
| `results/local_docker_benchmark_<timestamp>/02_DUCKDB/CLICKBENCH_SUBSET/environment.json` | Image digest, data subset, query numbers | Write it during the run. |
| Same folder, `pulls.txt` and `container_inspect.txt` | Pull and limit record | Write it during the run. |
| Same folder, `manifest.json`, `load.txt`, `result.csv`, `checksums.txt` | Rows, load, times, checksums | Write it during the run. |
| Same folder, `summary.md` | Remote, branch, top commit, verdict | Write it after the run. |

## 11. Acceptance gate

1. Make sure that one container runs at a time.
2. Make sure that `docker inspect` reports one central processing unit and 2 gigabytes of memory.
3. Make sure that the loaded row count equals 100,000 before timing.
4. Make sure that every selected query returns a result and a checksum.
5. Keep the warm-up pass outside the timed set.
6. Use the second and third runs for the hot time.
7. Make sure that the SQL text comes from `queries.sql` and not from this plan.
8. Write all evidence files from Table 7.
9. Stop the container and wait three minutes before the next pair.
10. Record `unsupported` with the named cause if a query cannot run. Do not write a fake result.

## Warnings

- **CAUTION:** The host is a low-power laptop. Do not run Docker, a release build, and a benchmark together.
- **CAUTION:** The 100,000-row subset is not the full 99,997,497-row set. Name the subset with each number.
- **NOTE:** No ShelfDB ClickBench harness appears in the approved documents. The SQL interface is the only recorded path.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
