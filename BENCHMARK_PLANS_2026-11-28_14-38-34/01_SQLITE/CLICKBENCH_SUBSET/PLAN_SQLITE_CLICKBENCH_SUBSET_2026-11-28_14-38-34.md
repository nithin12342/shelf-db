# PLAN_SQLITE_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan pairs a 100,000-row ClickBench slice on SQLite with the ShelfDB SQL ingress path. The plan names the source, the data, the cases, the commands, and the gate. The plan does not run the benchmark. The approved documents give no ShelfDB ClickBench binary. This plan records that gap. It does not invent ClickBench query text.

**SQL:** Structured Query Language, the standard language for relational databases.
**ClickBench:** a benchmark for analytical databases that uses one flat hits table.

## 2. Original benchmark source

Table 1 answers this question: which link and command identify the original benchmark. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Original benchmark link | `https://github.com/ClickHouse/ClickBench` | Bundle root `README.md` |
| Result dashboard | `https://benchmark.clickhouse.com/` | Extracted `ClickBench-main/README.md`, line 3 |
| Main run script | `./benchmark.sh` in a system directory | Same file, "How To Add a New Result" |
| Result regeneration script | `./generate-results.sh` | Same file, line 91 |
| Query count (count) | 43 | Same file, line 49 |
| Data source, compressed CSV | `https://datasets.clickhouse.com/hits_compatible/hits.csv.gz` | Same file, line 115 |
| Data source, compressed JSON | `https://datasets.clickhouse.com/hits_compatible/hits.json.gz` | Same file, line 117 |
| Data source, Parquet | `https://datasets.clickhouse.com/hits_compatible/hits.parquet` | Same file, line 118 |

## 3. Data set and subset

Table 2 answers this question: which data and subset the pair uses. Rows are facts. Columns are the fact, the value, and the source.

| Fact | Value | Source |
|---|---|---|
| Data set name | ClickHouse hits compatible | Extracted `ClickBench-main/README.md`, line 113 |
| Full row count (count) | 99,997,497 | Same file, line 62 |
| Low-power subset | first 100,000 rows | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4, item 10 |
| Bundle path | `tree_json_olap/` | Bundle root `README.md` |
| SQLite database file | `D:\bench\data\sqlite\clickbench_100k.db` | Operator path. `PLAN.md`, line 75, names the SQLite data root. |
| Target databases in the bundle | DuckDB, PostgreSQL, MariaDB, ShelfDB | Bundle root `README.md`, tree and nested JSON row |

Download one format only. Decompress it before the load. Use a single file for the load. Record the data size beside the timing.

## 4. Test-case documents and specific cases

Table 3 answers this question: which documents name the cases. Rows are documents. Columns are the path, the specific cases, and the text type.

| Document path | Specific cases | Text type |
|---|---|---|
| Extracted `ClickBench-main/README.md` | Q1 to Q43 by ordinal | Count and method, no SQL text |
| Extracted `ClickBench-main/json/sqlite/README.md` | SQLite for JSON documents, queries 1 to 5 | Method, no SQL text |
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | Section 4 item 10, 100,000-row slice | Data subset rule |
| Bundle root `README.md` | Tree and nested JSON workload | Data link and target list |

The extracted local folder holds no `queries.sql`. The original repository holds that file for each system. Record the ordinal labels Q1 to Q43 only. For the low-power run, use Q1 to Q5 first. Do not write SQL from this plan.

The extracted `json/sqlite/README.md` describes JSONBench. It is a different data set from the hits slice. Do not mix JSONBench timings with ClickBench timings.

## 5. Query text

The local source documents give no SQL text for the 43 ClickBench queries. Record the ordinal names only. Do not invent the SQL.

The original repository supplies `queries.sql` in each system directory. Read that file before a run. Put the exact text into the evidence file. If the file is absent, write `unsupported` with the file name. Do not substitute a query from this plan.

## 6. Container image and low-power limits

Table 4 answers this question: which image and limits apply. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Database image | None. SQLite is embedded. | Extracted `ClickBench-main/json/sqlite/README.md`, line 3 |
| Install step | Precompiled command-line shell from `https://sqlite.org/download.html` | Same line |
| Alternative install | `sqlite3` CLI through Scoop | `sheaf-db/tests/e2e/realtime_bench/PLAN.md`, line 75 |
| Optional isolation image | `alpine:3.20`. Install `sqlite` with `apk add --no-cache sqlite`. | Operator choice. No source document names this image. |
| Pull step for the optional image | `docker pull alpine:3.20` | Operator choice |
| Concurrent containers (count) | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container CPUs (count) | 1 | Same section |
| Container memory (GB) | 2 | Same section |
| Benchmark threads (count) | 1 | Same section. SQLite executes one query in one thread. |
| Maximum step duration (minutes) | 30 | Same section |
| Cooldown (minutes) | 3 | Same section |

## 7. Commands

Table 5 answers this question: which command runs each step. Rows are steps. Columns are the side, the command, and the source.

| Step | Side | Command | Source |
|---|---|---|---|
| Run the full original suite | ClickBench | `./benchmark.sh` from the chosen system directory | Extracted `ClickBench-main/README.md`, "How To Add a New Result" |
| Load the SQLite slice | SQLite | `sqlite3 <database-file> < <load-script.sql>` | Local command shape. The 100,000-row slice supplies the rows. |
| Run one ordinal query | SQLite | `sqlite3 <database-file> ".read <query-file.sql>"` | Local command shape. The SQL text comes from the original `queries.sql`. |
| Run the ShelfDB SQL path | ShelfDB | `cargo run --release -p sheaf-app -- --db <clickbench-db> sql "<SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md`, line 352 |
| Run the ShelfDB TQL path | ShelfDB | `cargo run --release -p sheaf-app -- --db <clickbench-db> tql-file <FILE>` | Same plan, line 353 |

Use one build job. The low-power plan uses `CARGO_BUILD_JOBS=1`. ClickBench normally runs each query three times. Run one warm-up and three measured passes on the low-power laptop.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file and the claim it supports.

| File | Claim |
|---|---|
| Extracted `ClickBench-main/README.md` | Original link, data links, query count, caching rules |
| Extracted `ClickBench-main/json/sqlite/README.md` | SQLite install and single-thread behavior |
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | 100,000-row slice rule |
| Bundle root `README.md` | Bundle path and target databases |
| `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` | ShelfDB SQL and TQL ingress commands |
| `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md` | Application SQL read gate |

## 9. Acceptance gate

1. Make sure that one engine runs at a time.
2. Make sure that the container uses one CPU, 2 GB memory, and one thread.
3. Make sure that the loaded row count equals 100,000.
4. Read the exact SQL from the original `queries.sql`. Make sure that the evidence file holds that text.
5. Make sure that SQLite and ShelfDB return the same checksum for each ordinal query.
6. Record the cold run and the two hot runs in separate columns. Do not average them into one number.
7. Make sure that no query step lasts longer than 30 minutes.
8. Write the command, the output, the query text, the row count, and the checksum into an evidence file. Put the timestamp, the remote, the branch, and the commit in that file.
9. If a source file is absent, write `unsupported` with the file name. Do not invent SQL.

## Warnings

- **CAUTION:** The local bundle holds no `queries.sql`. Do not treat the ordinal labels as query text.
- **CAUTION:** JSONBench and ClickBench use different data. Do not compare a JSONBench number with a hits number.
- **NOTE:** ClickBench rules discourage pre-aggregation. `ADVERSARIAL_ANALYSIS.md`, section 5.1, shows that ShelfDB answers depend on materialized structures. Report that difference with the timing.
