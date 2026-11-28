# Dataset usage and execution sequence

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30.
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This document explains how the attached data sets will be used after the per-database plans are approved. It does not run a benchmark. It does not pull an image. It defines the order and the safety limits.

## 2. What download means

Download means two actions:

1. Extract the named data files from `shelf_db_all_datasets.zip`.
2. Pull one database container image from its published registry.

Do not download all 5.22 GB of uncompressed data. Do not pull all database images. Run one database at a time.

## 3. Data extraction order

Table 1 answers this question: which data files are extracted first. Rows are data files. Columns are the archive path, the first use, and the size limit.

| Archive path | First use | Size limit |
|---|---|---|
| `tpch/sf1_tbl/` | TPC-H relational comparison | Scale factor 0.01 for the first run |
| `imdb/title.ratings.tsv.gz` and `title.basics.tsv.gz` | IMDb skewed join | Two files only |
| `ieee_fraud/train_transaction.csv` | Fraud comparison | First 100,000 rows |
| `graph/facebook_combined.txt.gz` | Graph smoke comparison | Full file, small |
| `graph_production/soc-LiveJournal1.txt.gz` | Production graph | Deferred until approval |
| `nosql_ycsb/ycsb-0.17.0.tar.gz` | Key-value comparison | 100,000 operations |
| `timeseries_tsbs/tsbs-master.zip` | Time-series generation | 100,000 rows |
| `tree_json_olap/ClickBench-main.zip` | Analytical comparison | 100,000-row slice |

## 4. Database image order

Pull images in this order:

1. `sqlite` image or native SQLite command line tool.
2. `duckdb` image or native DuckDB command line tool.
3. `postgres:16-alpine`.
4. `mariadb:11-alpine`.
5. `redis:7-alpine`.
6. `valkey/valkey:8-alpine`.
7. `scylladb/scylla:latest`.
8. `neo4j:5-community`.
9. `mongo:7`.
10. Kdb+ image or local license path. Record `unsupported` when the token is absent.
11. `timescale/timescaledb:latest-pg16`.
12. `influxdb:2`.
13. `questdb/questdb:latest`.

Use the exact tag from the per-database plan when it differs from this list.

## 5. Per-pair execution

For each plan file, run these steps:

1. Make sure that the named subset exists and has a checksum file.
2. Start Docker Desktop. Wait until `docker info` reports a Linux server.
3. Pull one image.
4. Start one container with one central processing unit and 2 GB memory.
5. Load the subset.
6. Make sure that the loaded row count matches the manifest.
7. Run one warm-up pass. Do not time it.
8. Run three measured passes with one thread.
9. Capture output rows and a checksum.
10. Run the matching ShelfDB command with the same input.
11. Compare row counts and checksums.
12. Write evidence files.
13. Stop the container. Remove its volume.
14. Wait three minutes before the next pair.

## 6. Evidence

Write evidence under the per-pair folder:

```text
EVIDENCE_<timestamp>/
  environment.json
  image_digest.txt
  subset_manifest.json
  load_output.txt
  query_1.txt
  query_2.txt
  query_3.txt
  checksums.txt
  shelfdb_output.txt
  summary.md
```

The summary must name the remote, branch, top commit, data set, benchmark tool, database image digest, and gate result.

## 7. Acceptance gate

A pair is complete only when all of the following hold:

1. One container ran at a time.
2. The row count matches the manifest.
3. The checksum matches the ShelfDB result.
4. The container stayed within the resource limits.
5. The evidence folder holds command output and checksums.
6. No token or secret entered Git.

## Warnings

- **CAUTION:** The laptop is an AMD Ryzen 3 system with Radeon integrated graphics. Treat it as low power. Do not run two containers at once.
- **CAUTION:** Do not expand the full 5.22 GB bundle before the first small run. The laptop can stall.
- **NOTE:** The per-database pair plans hold the exact commands and query text. Use them in the folder order.
