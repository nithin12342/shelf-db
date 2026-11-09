# Benchmark sources — datasets and our comparison pages

| # | Type (our DBs) | Dataset link | Our comparison page |
|---|---|---|---|
| 1 | Row RDBMS (PG, MariaDB) | TPC-H: `https://www.tpc.org/tpch/`, TPC-C: `https://www.tpc.org/tpcc/` | `sheaf-db/tests/e2e/realtime_bench/results/pg.json`, `.../mariadb.json` |
| 2 | Embedded (SQLite) | TPC-H (above); no official embedded suite | `.../results/sqlite.json` |
| 3 | Columnar OLAP (DuckDB) | TPC-H; TPC-DS `https://www.tpc.org/tpcds/`; ClickBench `https://github.com/ClickHouse/ClickBench` | `.../results/duckdb.json` |
| 4 | In-memory KV (Redis) | YCSB `https://github.com/brianfrankcooper/YCSB`; memtier `https://github.com/RedisLabs/memtier_benchmark` | `.../results/redis.json` |
| 5 | Document (MongoDB) | YCSB (above); IEEE fraud `https://www.kaggle.com/competitions/ieee-fraud-detection/data` | `.../results/mongodb.json`, `.../duckdb_real.json` |
| 6 | Graph (Neo4j) | LDBC SNB `https://ldbcouncil.org/benchmarks/snb/`; FB edges `https://snap.stanford.edu/data/ego-Facebook.html`; Graphalytics `https://graphalytics.org/` | `.../results/neo4j.json` |
| 7 | Wide-column (ScyllaDB) | YCSB (above) | `.../results/scylladb.json` |

## Our pages

| Page | Link |
|---|---|
| Repo (verified live) | `https://github.com/nithin12342/shelf-theory-db` |
| Results synthesis | `https://github.com/nithin12342/shelf-theory-db/blob/main/sheaf-db/tests/e2e/realtime_bench/results/REALTIME_RESULTS.md` |
| Run log | `https://github.com/nithin12342/shelf-theory-db/blob/main/sheaf-db/tests/e2e/realtime_bench/results/RUN_LOG.md` |
| Per-engine JSONs | `.../results/<engine>.json` (8 files, same folder) |

## Local data files (downloaded/generated from above)

| File | Source |
|---|---|
| `D:\bench\build\tpch-dbgen\*.tbl` | TPC-H dbgen, SF1 (built from tpc.org sources) |
| `D:\bench\datasets\ieee-fraud\*.csv` | Kaggle IEEE-CIS competition dump |
| `D:\bench\datasets\facebook_combined.txt` | SNAP ego-Facebook (extracted from .gz) |
| `D:\bench\datasets\imdb*` | Planned (Phase 2a): `https://datasets.imdbws.com/` |
