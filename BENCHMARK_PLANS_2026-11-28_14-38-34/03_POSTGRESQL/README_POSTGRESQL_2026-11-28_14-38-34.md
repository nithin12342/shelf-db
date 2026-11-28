# PostgreSQL benchmark plan index

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This folder holds eight plan files. Each plan compares PostgreSQL with ShelfDB on one data set and one workload. This folder does not install software and does not run a benchmark.

## 2. Concept definitions

- **Topological Query Language (TQL):** the declarative query language of ShelfDB. A TQL statement names cells, a filter, a solve, and an output section.
- **Structured Query Language (SQL):** the standard query language for relational databases.
- **Gate:** a test that compares a result with a stated truth value.

## 3. Pair index

Table 1 answers this question: which plan file belongs to each pair. Rows are pairs. Columns are the folder, the plan file, the benchmark link, and the query text status.

| Pair | Folder | Plan file | Benchmark link | Query text status |
|---|---|---|---|---|
| TPC-H scale factor 1 | `01_TPCH_SF1/` | `PLAN_POSTGRESQL_TPCH_SF1_2026-11-28_14-38-34.md` | `https://github.com/gregrahn/tpch-kit` | Exact SQL for scan and count. Join by name only. |
| Internet Movie Database query 1 | `02_IMDB_Q1/` | `PLAN_POSTGRESQL_IMDB_Q1_2026-11-28_14-38-34.md` | `https://datasets.imdbws.com/` | Exact SQL in the source. |
| 50 fraud cases | `03_50_FRAUD_CASES/` | `PLAN_POSTGRESQL_50_FRAUD_CASES_2026-11-28_14-38-34.md` | `https://www.kaggle.com/competitions/ieee-fraud-detection/data` | Case names only. No SQL text in the source. |
| BenchBase TPC-C | `04_BENCHBASE_TPCC/` | `PLAN_POSTGRESQL_BENCHBASE_TPCC_2026-11-28_14-38-34.md` | `https://github.com/cmu-db/benchbase` | Tool-generated transactions. Names only. |
| ClickBench subset | `05_CLICKBENCH_SUBSET/` | `PLAN_POSTGRESQL_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md` | `https://github.com/ClickHouse/ClickBench` | Query names only. Text not extracted locally. |
| LDBC Social Network Benchmark | `06_LDBC_SNB/` | `PLAN_POSTGRESQL_LDBC_SNB_2026-11-28_14-38-34.md` | `https://github.com/ldbc/ldbc_snb_interactive_v1_impls` | 22 read queries and 7 updates. Names only. |
| Institute of Electrical and Electronics Engineers fraud data | `07_IEEE_FRAUD/` | `PLAN_POSTGRESQL_IEEE_FRAUD_2026-11-28_14-38-34.md` | `https://www.kaggle.com/c/ieee-fraud-detection/data` | Exact SQL for controlled five-row join only. |
| Yahoo! Cloud Serving Benchmark | `08_YCSB/` | `PLAN_POSTGRESQL_YCSB_2026-11-28_14-38-34.md` | `https://github.com/brianfrankcooper/YCSB` | Workload names only. No SQL text in the source. |

## 4. Shared limits

Every plan uses the same limits. Table 2 answers this question: which limits apply to every pair. Rows are limits. Columns are the limit and the value.

| Limit | Value |
|---|---|
| Concurrent database containers | 1 |
| Central processing units for the container | 1 |
| Memory for the container | 2 GB |
| Benchmark threads | 1 |
| Measured passes | 3 |
| Maximum time for one load or query step | 30 minutes |
| Cooldown between pairs | 3 minutes |

## 5. Evidence layout

Every plan writes evidence under `results/local_docker_benchmark_<run timestamp>/postgresql_<pair>/`. Each evidence folder holds `environment.json`, `pulls.txt`, `container_inspect.txt`, `manifest.json`, `load.txt`, `result.csv`, and `checksums.txt`. The summary file names the remote, the branch, and the top commit.

## Warnings

- **CAUTION:** Run one database container at a time. The host is an entry-level laptop.
- **CAUTION:** Do not run a container and a release build at the same time.
- **NOTE:** No image was pulled and no benchmark was run for these plans.
- **NOTE:** Do not invent SQL text. When a source gives a name only, record the name only.
