# Attached dataset bundle investigation

**Timestamp:** 2026-11-28_14-19-54, local time 14:19:54 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `3c7394681e48f21ecbe706bbe286bb46f0e24dde`.

## 1. Result

The attached archive is `shelf_db_all_datasets.zip`. It holds 30 entries. The compressed size is 3.57 GB. The uncompressed size is 5.22 GB. The archive has one root README. It has no other root Markdown file. Six nested archives hold tool documentation and README files.

I extracted the root README and the Markdown files from the nested tool archives. I did not extract the large dataset files. That choice keeps the Ryzen 3 laptop responsive.

## 2. Root README

The root README names this bundle as a unified benchmark dataset bundle. It groups workloads as follows:

1. Relational OLAP with TPC-H.
2. Relational skewed joins with IMDb.
3. Relational online transaction processing with BenchBase.
4. Wide-column and tabular work with IEEE-CIS fraud data.
5. Key-value work with YCSB and memtier.
6. Time-series work with TSBS.
7. Graph work with SNAP Facebook, LiveJournal, and LDBC.
8. Tree and nested JSON work with ClickBench.

## 3. Nested tool archives

Table 1 answers this question: which nested archives hold tool documentation. Rows are nested archives. Columns are the archive, the main README, and the benchmark role.

| Nested archive | Main README | Benchmark role |
|---|---|---|
| `tpch-kit-master.zip` | `tpch-kit-master/README.md` | Build `dbgen` and `qgen` |
| `benchbase-main.zip` | `benchbase-main/README.md` | TPC-C, SEATS, Twitter, and YCSB |
| `memtier_benchmark-master.zip` | `memtier_benchmark-master/README.md` | Redis and Valkey load generation |
| `tsbs-master.zip` | `tsbs-master/README.md` | Time-series generation and load |
| `ldbc_snb_interactive_v1_impls-main.zip` | `ldbc_snb_interactive_v1_impls-main/README.md` | Interactive graph queries |
| `ClickBench-main.zip` | `ClickBench-main/README.md` | Analytical and nested JSON queries |

## 4. What the README files say

1. TPC-H kit builds `dbgen` with `make MACHINE=LINUX DATABASE=POSTGRESQL`.
2. BenchBase runs through Java and Maven. It supports TPC-C and other online transaction workloads.
3. memtier generates Redis and Memcache protocol traffic with pipelining and latency histograms.
4. TSBS supports Akumuli, Cassandra, ClickHouse, CrateDB, InfluxDB, MongoDB, QuestDB, SiriDB, TimescaleDB, Timestream, and VictoriaMetrics.
5. LDBC SNB provides reference implementations for Neo4j, PostgreSQL, GraphDB, DuckDB, TigerGraph, and Umbra. Its workload contains updates. A run must restore the database first.
6. ClickBench focuses on reproducibility. Its dataset and queries cover clickstream and web analytics.

## 5. Local extraction

I wrote the extracted documentation under `attached_bundle_review_2026-11-28_14-19-54/`. That folder stays local. The repository `.gitignore` must exclude it before any commit.

## Warnings

- **CAUTION:** Do not extract all 5.22 GB on this laptop before the benchmark plan is approved. The disk and central processing unit load can make the laptop twitch.
- **CAUTION:** LDBC updates persist. Restore the database before each LDBC run.
- **NOTE:** This report uses only README and Markdown files from the bundle. It does not use the protected neural-network archive.
