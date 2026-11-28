# Consolidated database and test-case index

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `6edd0698ae57d91be09a9ddcb9d4ffa13a4de610`.

## 1. Result

Thirteen databases have separate plan folders. Every database is compared with ShelfDB. The folder tree holds 59 pair plans. Each pair plan names the benchmark source, the data subset, the test-case document, the query text status, the container image, the low-power limits, the commands, the evidence files, and the acceptance gate.

## 2. Database index

Table 1 answers this question: which database is compared with ShelfDB and which documents hold its cases. Rows are databases. Columns are the database, the comparison target, the benchmark source link, the test-case document, and the query text document.

| Database | Compared with | Benchmark source | Test-case document | Query text document |
|---|---|---|---|---|
| SQLite | ShelfDB | [BenchBase](https://github.com/cmu-db/benchbase), [ClickBench](https://github.com/ClickHouse/ClickBench) | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, `TQL_APPLICATION_COMPARISON_2026-10-26.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt), [`SQL corpus`](../recursive%20test/SQL_CORPUS_2026-11-22_15-04-04.sql) |
| DuckDB | ShelfDB | [TPC-H kit](https://github.com/gregrahn/tpch-kit), [ClickBench](https://github.com/ClickHouse/ClickBench) | `RUNTIME_COMPARISON.md`, `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt), [`SQL corpus`](../recursive%20test/SQL_CORPUS_2026-11-22_15-04-04.sql) |
| PostgreSQL | ShelfDB | [TPC-H kit](https://github.com/gregrahn/tpch-kit), [BenchBase](https://github.com/cmu-db/benchbase), [LDBC SNB](https://github.com/ldbc/ldbc_snb_interactive_v1_impls) | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt), [`SQL corpus`](../recursive%20test/SQL_CORPUS_2026-11-22_15-04-04.sql) |
| MariaDB | ShelfDB | [TPC-H kit](https://github.com/gregrahn/tpch-kit), [BenchBase](https://github.com/cmu-db/benchbase), [ClickBench](https://github.com/ClickHouse/ClickBench) | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt), [`SQL corpus`](../recursive%20test/SQL_CORPUS_2026-11-22_15-04-04.sql) |
| Redis | ShelfDB | [YCSB](https://github.com/brianfrankcooper/YCSB), [memtier](https://github.com/redis/memtier_benchmark) | `RUNTIME_COMPARISON.md`, `TQL_APPLICATION_COMPARISON_2026-10-26.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt) |
| Valkey | ShelfDB | [memtier](https://github.com/redis/memtier_benchmark) | `RUNTIME_COMPARISON.md` | Plan folder only. No exact Valkey command in the approved documents. |
| ScyllaDB | ShelfDB | [YCSB](https://github.com/brianfrankcooper/YCSB) | `RUNTIME_COMPARISON.md`, `TQL_APPLICATION_COMPARISON_2026-10-26.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt) |
| Neo4j | ShelfDB | [LDBC SNB](https://github.com/ldbc/ldbc_snb_interactive_v1_impls), [SNAP Facebook](https://snap.stanford.edu/data/egonets-Facebook.html), [SNAP LiveJournal](https://snap.stanford.edu/data/soc-LiveJournal1.html) | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, `RUNTIME_COMPARISON.md`, `ADVERSARIAL_ANALYSIS.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt) |
| MongoDB | ShelfDB | [TSBS](https://github.com/timescale/tsbs) | `RUNTIME_COMPARISON.md`, `mongo_bench.py`, TSBS MongoDB guide | `mongo_bench.py` has exact PyMongo calls. |
| Kdb+ | ShelfDB | Local Kdb benchmark scripts | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | [`TQL and native text`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt), `bench_kdb_timeseries.py`, `bench_kdb_join.py` |
| TimescaleDB | ShelfDB | [TSBS](https://github.com/timescale/tsbs) | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, TSBS TimescaleDB guide | TSBS gives query type names, not generated query text. |
| InfluxDB | ShelfDB | [TSBS](https://github.com/timescale/tsbs) | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, TSBS InfluxDB guide | TSBS gives query type names, not generated query text. |
| QuestDB | ShelfDB | [TSBS](https://github.com/timescale/tsbs) | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, TSBS QuestDB guide | TSBS gives query type names, not generated query text. |

## 3. Pair plan index

Table 2 answers this question: which pair plans exist for each database. Rows are databases. Columns are the database, the pair count, and the plan links.

| Database | Pairs (count) | Plan links |
|---|---|---|
| SQLite | 5 | [README](01_SQLITE/README_SQLITE_2026-11-28_14-38-34.md), [TPC-H](01_SQLITE/TPCH_SF1/PLAN_SQLITE_TPCH_SF1_2026-11-28_14-38-34.md), [IMDb Q1](01_SQLITE/IMDB_Q1/PLAN_SQLITE_IMDB_Q1_2026-11-28_14-38-34.md), [50 fraud](01_SQLITE/FRAUD_50/PLAN_SQLITE_FRAUD_50_2026-11-28_14-38-34.md), [BenchBase](01_SQLITE/BENCHBASE_TPCC/PLAN_SQLITE_BENCHBASE_TPCC_2026-11-28_14-38-34.md), [ClickBench](01_SQLITE/CLICKBENCH_SUBSET/PLAN_SQLITE_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md) |
| DuckDB | 9 | [README](02_DUCKDB/README_DUCKDB_2026-11-28_14-38-34.md), [TPC-H scan](02_DUCKDB/TPCH_SF1_SCAN/PLAN_DUCKDB_TPCH_SF1_SCAN_2026-11-28_14-38-34.md), [TPC-H join](02_DUCKDB/TPCH_SF1_JOIN/PLAN_DUCKDB_TPCH_SF1_JOIN_2026-11-28_14-38-34.md), [IMDb Q1](02_DUCKDB/IMDB_Q1/PLAN_DUCKDB_IMDB_Q1_2026-11-28_14-38-34.md), [Q2](02_DUCKDB/IMDB_Q2/PLAN_DUCKDB_IMDB_Q2_2026-11-28_14-38-34.md), [Q3](02_DUCKDB/IMDB_Q3/PLAN_DUCKDB_IMDB_Q3_2026-11-28_14-38-34.md), [50 fraud](02_DUCKDB/FRAUD_50/PLAN_DUCKDB_FRAUD_50_2026-11-28_14-38-34.md), [ClickBench](02_DUCKDB/CLICKBENCH_SUBSET/PLAN_DUCKDB_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md), [Kdb time series](02_DUCKDB/KDB_TIMESERIES/PLAN_DUCKDB_KDB_TIMESERIES_2026-11-28_14-38-34.md), [BenchBase](02_DUCKDB/BENCHBASE_TPCC/PLAN_DUCKDB_BENCHBASE_TPCC_2026-11-28_14-38-34.md) |
| PostgreSQL | 8 | [README](03_POSTGRESQL/README_POSTGRESQL_2026-11-28_14-38-34.md), [TPC-H](03_POSTGRESQL/01_TPCH_SF1/PLAN_POSTGRESQL_TPCH_SF1_2026-11-28_14-38-34.md), [IMDb Q1](03_POSTGRESQL/02_IMDB_Q1/PLAN_POSTGRESQL_IMDB_Q1_2026-11-28_14-38-34.md), [50 fraud](03_POSTGRESQL/03_50_FRAUD_CASES/PLAN_POSTGRESQL_50_FRAUD_CASES_2026-11-28_14-38-34.md), [BenchBase](03_POSTGRESQL/04_BENCHBASE_TPCC/PLAN_POSTGRESQL_BENCHBASE_TPCC_2026-11-28_14-38-34.md), [ClickBench](03_POSTGRESQL/05_CLICKBENCH_SUBSET/PLAN_POSTGRESQL_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md), [LDBC](03_POSTGRESQL/06_LDBC_SNB/PLAN_POSTGRESQL_LDBC_SNB_2026-11-28_14-38-34.md), [IEEE fraud](03_POSTGRESQL/07_IEEE_FRAUD/PLAN_POSTGRESQL_IEEE_FRAUD_2026-11-28_14-38-34.md), [YCSB](03_POSTGRESQL/08_YCSB/PLAN_POSTGRESQL_YCSB_2026-11-28_14-38-34.md) |
| MariaDB | 6 | [README](04_MARIADB/README_MARIADB_2026-11-28_14-38-34.md), [TPC-H](04_MARIADB/01_TPCH_SF1/PLAN_MARIADB_TPCH_SF1_2026-11-28_14-38-34.md), [IMDb Q1](04_MARIADB/02_IMDB_Q1/PLAN_MARIADB_IMDB_Q1_2026-11-28_14-38-34.md), [50 fraud](04_MARIADB/03_50_FRAUD_CASES/PLAN_MARIADB_50_FRAUD_CASES_2026-11-28_14-38-34.md), [BenchBase](04_MARIADB/04_BENCHBASE_TPCC/PLAN_MARIADB_BENCHBASE_TPCC_2026-11-28_14-38-34.md), [ClickBench](04_MARIADB/05_CLICKBENCH_SUBSET/PLAN_MARIADB_CLICKBENCH_SUBSET_2026-11-28_14-38-34.md), [IEEE fraud](04_MARIADB/06_IEEE_FRAUD/PLAN_MARIADB_IEEE_FRAUD_2026-11-28_14-38-34.md) |
| Redis | 4 | [README](05_REDIS/README_REDIS_2026-11-28_14-38-34.md), [YCSB](05_REDIS/01_YCSB/PLAN_REDIS_YCSB_2026-11-28_14-38-34.md), [memtier](05_REDIS/02_MEMTIER_GET_SET/PLAN_REDIS_MEMTIER_GET_SET_2026-11-28_14-38-34.md), [point lookup](05_REDIS/03_KEY_VALUE_POINT_LOOKUP/PLAN_REDIS_KEY_VALUE_POINT_LOOKUP_2026-11-28_14-38-34.md), [TQL case 4](05_REDIS/04_TQL_CASE_4/PLAN_REDIS_TQL_CASE_4_2026-11-28_14-38-34.md) |
| Valkey | 2 | [README](06_VALKEY/README_VALKEY_2026-11-28_14-38-34.md), [memtier](06_VALKEY/01_MEMTIER_GET_SET/PLAN_VALKEY_MEMTIER_GET_SET_2026-11-28_14-38-34.md), [point lookup](06_VALKEY/02_KEY_VALUE_POINT_LOOKUP/PLAN_VALKEY_KEY_VALUE_POINT_LOOKUP_2026-11-28_14-38-34.md) |
| ScyllaDB | 6 | [README](07_SCYLLADB/README_SCYLLADB_2026-11-28_14-38-34.md), [IMDb](07_SCYLLADB/01_IMDB_DENORMALIZED_Q1/PLAN_SCYLLADB_IMDB_DENORMALIZED_Q1_2026-11-28_14-38-34.md), [IEEE fraud](07_SCYLLADB/02_IEEE_FRAUD/PLAN_SCYLLADB_IEEE_FRAUD_2026-11-28_14-38-34.md), [YCSB](07_SCYLLADB/03_YCSB/PLAN_SCYLLADB_YCSB_2026-11-28_14-38-34.md), [point lookup](07_SCYLLADB/04_KEY_VALUE_POINT_LOOKUP/PLAN_SCYLLADB_KEY_VALUE_POINT_LOOKUP_2026-11-28_14-38-34.md), [range slice](07_SCYLLADB/05_RANGE_SLICE/PLAN_SCYLLADB_RANGE_SLICE_2026-11-28_14-38-34.md), [TQL case 4](07_SCYLLADB/06_TQL_CASE_4/PLAN_SCYLLADB_TQL_CASE_4_2026-11-28_14-38-34.md) |
| Neo4j | 5 | [README](08_NEO4J/README_NEO4J_2026-11-28_14-38-34.md), [IMDb Q2](08_NEO4J/01_IMDB_Q2_CYPHER/PLAN_NEO4J_IMDB_Q2_CYPHER_2026-11-28_14-38-34.md), [traversal](08_NEO4J/02_GRAPH_TRAVERSAL/PLAN_NEO4J_GRAPH_TRAVERSAL_2026-11-28_14-38-34.md), [Facebook](08_NEO4J/03_SNAP_FACEBOOK_TWO_HOP/PLAN_NEO4J_SNAP_FACEBOOK_TWO_HOP_2026-11-28_14-38-34.md), [LiveJournal](08_NEO4J/04_SNAP_LIVEJOURNAL/PLAN_NEO4J_SNAP_LIVEJOURNAL_2026-11-28_14-38-34.md), [LDBC](08_NEO4J/05_LDBC_SNB_INTERACTIVE/PLAN_NEO4J_LDBC_SNB_INTERACTIVE_2026-11-28_14-38-34.md) |
| MongoDB | 2 | [README](09_MONGODB/README_MONGODB_2026-11-28_14-38-34.md), [mixed threads](09_MONGODB/01_MIXED_32_THREAD/PLAN_MONGODB_MIXED_32_THREAD_2026-11-28_14-38-34.md), [TSBS](09_MONGODB/02_TSBS_TIME_SERIES/PLAN_MONGODB_TSBS_TIME_SERIES_2026-11-28_14-38-34.md) |
| Kdb+ | 6 | [README](10_KDB/README_KDB_2026-11-28_14-38-34.md), [window](10_KDB/01_TIME_WINDOW_AGGREGATION/PLAN_KDB_TIME_WINDOW_AGGREGATION_2026-11-28_14-38-34.md), [range](10_KDB/02_RANGE_SLICE/PLAN_KDB_RANGE_SLICE_2026-11-28_14-38-34.md), [point](10_KDB/03_EXACT_POINT_LOOKUP/PLAN_KDB_EXACT_POINT_LOOKUP_2026-11-28_14-38-34.md), [join](10_KDB/04_FOUR_TABLE_FRAUD_JOIN/PLAN_KDB_FOUR_TABLE_FRAUD_JOIN_2026-11-28_14-38-34.md), [50 cases](10_KDB/05_HIGH_VARIANCE_50/PLAN_KDB_HIGH_VARIANCE_50_2026-11-28_14-38-34.md), [slice](10_KDB/06_ONE_DIMENSIONAL_VECTOR_SLICE/PLAN_KDB_ONE_DIMENSIONAL_VECTOR_SLICE_2026-11-28_14-38-34.md) |
| TimescaleDB | 2 | [README](11_TIMESCALEDB/README_TIMESCALEDB_2026-11-28_14-38-34.md), [dev ops](11_TIMESCALEDB/PAIR_01_TSBS_DEVOPS/PLAN_TIMESCALEDB_TSBS_DEVOPS_2026-11-28_14-38-34.md), [internet of things](11_TIMESCALEDB/PAIR_02_TSBS_IOT/PLAN_TIMESCALEDB_TSBS_IOT_2026-11-28_14-38-34.md) |
| InfluxDB | 2 | [README](12_INFLUXDB/README_INFLUXDB_2026-11-28_14-38-34.md), [dev ops](12_INFLUXDB/PAIR_01_TSBS_DEVOPS/PLAN_INFLUXDB_TSBS_DEVOPS_2026-11-28_14-38-34.md), [internet of things](12_INFLUXDB/PAIR_02_TSBS_IOT/PLAN_INFLUXDB_TSBS_IOT_2026-11-28_14-38-34.md) |
| QuestDB | 2 | [README](13_QUESTDB/README_QUESTDB_2026-11-28_14-38-34.md), [dev ops](13_QUESTDB/PAIR_01_TSBS_DEVOPS/PLAN_QUESTDB_TSBS_DEVOPS_2026-11-28_14-38-34.md), [internet of things](13_QUESTDB/PAIR_02_TSBS_IOT/PLAN_QUESTDB_TSBS_IOT_2026-11-28_14-38-34.md) |

## 4. Exact query status

1. Exact SQL, TQL, Cypher, and q text is in [`TQL_AND_NATIVE_2026-11-22_15-04-04.txt`](../recursive%20test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt).
2. The 24 SQL statements are in [`SQL_CORPUS_2026-11-22_15-04-04.sql`](../recursive%20test/SQL_CORPUS_2026-11-22_15-04-04.sql).
3. The 50 fraud cases have names only in [`CASE_NAMES_2026-11-22_15-04-04.md`](../recursive%20test/CASE_NAMES_2026-11-22_15-04-04.md).
4. TSBS gives query type names. It does not give generated query text.
5. YCSB and memtier give workload names. They do not give exact commands in the approved documents.
6. Do not invent query text. Record names only and mark the exact text as absent.

## Warnings

- **CAUTION:** Some pair plans record `unsupported` for ShelfDB. The approved documents name no matching ShelfDB runner for those cases.
- **NOTE:** These are plans. No image was pulled and no benchmark was run.
- **NOTE:** The protected neural-network archive stayed excluded.
