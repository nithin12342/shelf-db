# Plan: MariaDB versus ShelfDB on ClickBench, a 100,000-row subset

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB on a ClickBench subset. This document is a plan only. It does not pull an image, download the data, load a table, or run a query.

ClickBench is a benchmark for analytical databases. A TQL statement is a declarative query over a cellular complex. SQL is the standard query language for relational databases. A gate is an automatic test that compares a result with a truth value.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the benchmark and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| ClickBench | `https://github.com/ClickHouse/ClickBench` | Benchmark scripts and query set |
| Result dashboard | `https://benchmark.clickhouse.com/` | Published results |
| ClickBench main README | `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md` | Rules, data links, and driver scripts |
| MariaDB ColumnStore note | `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_mariadb-columnstore_README.md` | Recorded load failure |
| MariaDB and DuckDB note | `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_mariadb-duckdb_README.md` | Experimental storage engine |
| JSONBench MariaDB note | `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_json_mariadb_README.md` | JSON workload configuration |
| Execution plan | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | 100,000-row ClickBench slice |

The full ClickBench data set is `hits.csv.gz` at `https://datasets.clickhouse.com/hits_compatible/hits.csv.gz`. It holds 99,997,497 rows in one flat table. Use a 100,000-row slice for this laptop. Download and decompress the slice before the load. Record the slice digest in the manifest. Do not compare a 100,000-row time with a published 99,997,497-row time.

The extracted documentation names three MariaDB entries. `mariadb-columnstore` failed the full load with error 1030. `mariadb-duckdb` is an experimental plug-in that uses MariaDB 11.4.13 and plug-in release 2.0.1. `json_mariadb` is JSONBench on Bluesky events, not the `hits` table. The extracted documentation has no plain MariaDB `hits` entry. Use the main ClickBench driver with the MariaDB default configuration. Record the chosen variant in `environment.json`.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which documents name the ClickBench cases. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md` | Overview | 43-query workload on `hits` |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md` | How to add a result | `queries.sql` with 43 queries |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md` | Caching | One cold run and two hot runs per query |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_mariadb-columnstore_README.md` | Full text | `LOAD DATA LOCAL INFILE` failure |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_mariadb-duckdb_README.md` | Full text | Plug-in version and checksum note |
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | Section 4, item 10 | 100,000-row slice |

The specific cases are query 1 (Q1) to query 43 (Q43) in `queries.sql`. Table 3 answers this question: which query groups the benchmark names. Rows are groups. Columns are the group, the identifier, and the statement from the source.

| Group | Identifier | Statement from the source |
|---|---|---|
| Full scan and filtered scan | Q1 to Q43 (queries 1 to 43) | The workload tests full scan, filtered scan, index lookup, and relational operations |
| Cold runs | First run of each query | The driver restarts the server before the first run |
| Hot runs | Second and third run of each query | The driver takes the smaller of the two hot timings |

## 4. Exact query text

The ClickBench README states that `queries.sql` holds 43 queries. The extracted documentation does not include that file. The approved documents give no SQL text for those queries. Do not invent SQL for Q1 to Q43.

Names only apply to every query in Table 3. The run team must read `queries.sql` from the cloned ClickBench repository. If that file is absent, write `unsupported` with the cause `no ClickBench query text in the provided files`. Do not write a fake result.

The ColumnStore load error is recorded in the extracted README. If the run selects ColumnStore and the load fails, record `unsupported` with error 1030. Do not record a zero or a guessed time.

## 5. Container image and install or pull step

Table 4 answers this question: which image and steps the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB, default path | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB, plug-in path | Pull one image only when that path is selected | `docker pull mariadb:11.4.13` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <results>:/benchbase/results mariadb:13.0.2` |
| ClickBench | Clone the repository | `git clone https://github.com/ClickHouse/ClickBench.git` |
| ClickBench | Read the selected entry scripts | Read `install`, `start`, `check` (readiness), `load`, `query`, `data-size`, and `stop` in the entry directory |

The plug-in README pins MariaDB 11.4.13 and plug-in release 2.0.1. It states that packages exist for amd64 only. Make sure that the host uses amd64 before that path. Pull one image for one run. Record the image digest in `environment.json`.

## 6. Low-power limits

Table 5 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Data slice | 100,000 | rows | Keeps load time within the stop rule |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives one cold and two hot runs per query |
| Stop rule for a load step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 6 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Original full driver | `./benchmark.sh` from the selected entry directory | Load time, data size, and 43 timing lines |
| Original sub-steps | `./install`, `./start`, `./check` (readiness), `./load`, `./query`, `./stop` | One result per step |
| Single-process setting | `BENCH_CONCURRENT_DURATION=0` | Skips the concurrent query test |
| Data slice | Set `BENCH_DATA_DIR` to the 100,000-row slice directory | One flat table with 100,000 rows |
| Query run | Feed one line of `queries.sql` into `./query` | Runtime in seconds on the last line of stderr |
| JSONBench example, not this pair | `cd clickhouse` then `./benchmark.sh 1` | JSONBench result, not a `hits` result |
| Cloud driver, not this pair | `run-benchmark.sh <system> <size>` | Cloud run, outside the laptop plan |
| ShelfDB matching command | `unsupported` | Error: no ClickBench binary is named in the approved documents or in `sheaf-db/crates/sheaf-engine/src/bin/` |

Do not use `fused_scan_bench` as a ClickBench result. That binary reads prepared data from the TPC-H path. Do not use the cloud driver on the laptop.

## 8. Evidence files

Table 7 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `<entry>/results/<date>/<machine>.json` | Load time, data size, and 43 query timings | ClickBench driver output |
| `<entry>/queries.sql` | Exact query text | Read from the cloned repository |
| `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_mariadb-columnstore_README.md` | ColumnStore load failure | Extracted README |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, slice digest, chosen variant | Written before the run |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row per query, with cold and hot times | Written after three measured passes |
| `results/local_docker_benchmark_<timestamp>/shelfdb/unsupported.txt` | Cause of the ShelfDB gap | Written when the command is not present |

## 9. Acceptance gate

Table 8 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| Query text | `queries.sql` is present | Pass only when true | Otherwise write `unsupported` with the named cause |
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread | Pass only when true | Record `docker inspect` output |
| Row count | The table holds 100,000 rows | Pass only when true | Compare with the slice manifest |
| Query gate | Every query returns a result or `null` for a failure | Pass only when true | Do not fill a failure with zero |
| ColumnStore gate | `unsupported` with error 1030 if that variant fails | Pass only when the error is explicit | Do not record a fake time |
| ShelfDB gap | `unsupported` with the named cause | Pass only when the error is explicit | Do not return a fake result |
| Evidence completeness | Commands, outputs, and checksums are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

## Warnings

- **CAUTION:** The 100,000-row slice is not the published ClickBench data set. Name the slice in every row.
- **CAUTION:** The extracted documentation has no plain MariaDB `hits` entry. Name the chosen variant.
- **NOTE:** The 43 query statements are not in the provided files. The plan does not invent SQL.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
