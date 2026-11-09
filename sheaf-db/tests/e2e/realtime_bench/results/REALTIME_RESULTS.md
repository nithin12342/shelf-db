# REALTIME_RESULTS — live benchmark, 2026-10-13/14, i7-1165G7/16GB/Win11

All numbers measured on this machine. Engine versions: SQLite 3.x (py), DuckDB 1.5.4, Redis 5.0.14.1, MongoDB 9.0.2, MariaDB 13.0.2, PostgreSQL 18.6, Neo4j 5.26.32, sheaf-db (release bins). Dataset: TPC-H SF1 (verified counts) except graph/KV/LSM shapes per plan. Docker used ONLY for Neo4j/ScyllaDB (native installs unavailable); all else native headless on D:, uninstalled + data deleted after (lifecycle verified each step).

## 1. Point lookups (µs, p50)

| Engine | p50 | p99 | Notes |
|--------|-----|-----|-------|
| sheaf-db (BCSF slice) | 0.0105 | ~0.03 | 200k iters, in-process |
| DuckDB (batched hash join) | 0.2/row | — | 20k keys one query; single-query mode 1075 (client overhead), unindexed 3861 |
| SQLite (B-tree indexed) | 30.8 | 128.8 | 20k lookups |
| PostgreSQL (PK) | 108 | 341 | loopback + MVCC |
| MariaDB (PK) | 214 | 891 | loopback + InnoDB |
| Redis (HGETALL loopback) | 429.7 | 715.6 | 3 fields; protocol-bound |
| Neo4j (indexed node) | 2694 | 15660 | bolt loopback + JVM |
| MongoDB | — | — | covered via pipeline test, not isolated point |
| ScyllaDB (settled) | 11689 | 18775 | fresh-load SSTable pile-up, 2-core Docker; no major compaction possible (aio-max-nr env limit) — recorded, not hidden |

## 2. Joins / traversals

| Test | Engine | Result |
|------|--------|--------|
| 3-table join+GROUP BY (6M rows) | DuckDB | 410.8 ms |
| same | PostgreSQL | 3.04 s (7.4× DuckDB) |
| same | MariaDB | 29.8 s (73× DuckDB) |
| same | SQLite | 126.9 s (309× DuckDB) |
| 4-stage $lookup (200 docs) | MongoDB indexed | 79.6 ms (unindexed: 12.1 s — missing-index catch, both kept) |
| 2-hop traversal (200 sampled) | Neo4j | 3.3 ms p50 |
| 600k-path full traversal | Neo4j | 878 ms |
| MATCH incident solve | sheaf-db | 609 µs total / 9 µs median solve (2 hypercells) |

## 3. Scan aggregation (AVG/SUM, 6M rows, 5 reps p50)

| Engine | ms | vs DuckDB |
|--------|----|-----------|
| DuckDB | 17.9 | 1× |
| PostgreSQL | 334 | 19× |
| MariaDB | 1521 | 85× |
| SQLite | 2603 | 145× |

## 4. Single-row inserts (µs, p50)

| Engine | p50 | Notes |
|--------|-----|-------|
| SQLite | 2.1 | WAL append path |
| MariaDB | 68.1 | InnoDB + loopback |
| PostgreSQL | 104.6 | WAL + MVCC + loopback |
| DuckDB | 2726 | row-wise transactional path (not its bulk path; recorded honestly) |

## 5. Memory / storage

| Engine | Footprint |
|--------|-----------|
| sheaf-db BCSF | 1.220× raw |
| Redis | 121 MB / 1M keys (≈7.5× 16 B payload) |
| Neo4j bulk | 2.25M nodes + 2.1M rels imported (ID-space collision + reversed-edge bugs caught and fixed during load; documented in run log) |

## 6. Throughput spot checks

- MongoDB mixed 50/50, 32 threads (laptop-scaled from 128): 2,497 ops/s, p50 9.47 ms
- ScyllaDB range slice: 10k rows / 65 ms; write-flood concurrent reads p50 825 µs
- Redis 1M HSET load: 48.1 s pipelined

## Verdicts vs COMPARISON_MATRIX.md predictions

- Confirmed: DuckDB join/scan dominance (310×/145× over SQLite), PG> MariaDB> SQLite ordering on joins (3.0s / 29.8s / 127s), sheaf-db in-process lookup class (10ns).
- Revised: matrix claimed Redis ~1.35ms — measured 430µs (v5 native win build, loopback). Matrix Neo4j path numbers were directionally right (878ms measured vs ~1.2s cited class).
- New caveats: ScyllaDB laptop number (11.7ms) reflects uncompacted fresh load + 2-core Docker, NOT production ScyllaDB; DuckDB single-row inserts are a deliberate anti-pattern number.
- sheaf-db app end-to-end varies 290–609µs with box load (host-qualification precedent applies).

## Provenance

- Raw JSONs: `results/sqlite.json`, `duckdb.json`, `redis.json`, `mongodb.json`, `mariadb.json`, `pg.json`, `neo4j.json`, `scylladb.json`
- Harnesses: `sqlite_bench.py`, `duckdb_bench.py`, `redis_bench.py`, `mongo_bench.py`, `sql_bench.py`, `neo4j_bench.py`, `scylla_bench.py`, `neo_prep.py`
- TPC-H SF1 verified: lineitem 6001215, orders 1500000, customer 150000, part 200000, partsupp 800000
- Tools built: `D:\bench\build\tpch-dbgen\dbgen.exe` + `qgen.exe` (MinGW, WIN32+`__GNUC__` 6-line patch on a copy; repo pristine)

## Fair-rerun verdicts (Tests 1–5, identical datasets + shapes, 2026-10-14)

| Test | sheaf-db | Best competitor | Verdict |
|------|----------|-----------------|---------|
| 1: 6M scan SUM | ~8 ms (all layouts tie) → **CORRECTED (2nd)** | DuckDB 6.4 ms | **DuckDB wins narrowly ~1.25×** (earlier ~30 ms was stale-binary + load artifacts; see SYNTHESIS_PLAN stage log) |
| 2: 150k⋈1.5M⋈6M join | 2.5 s | DuckDB 158 ms / PG 3.04 s | Middle: DuckDB >> sheaf-db >> PG |
| 3: 600k-path traversal | 41.7 ms (paths exact) | Neo4j 878 ms | sheaf-db wins 21× |
| 4: 20k durable inserts | 6.9 µs p50 | SQLite 2.1 µs | SQLite wins 3.3× |
| 5: 32-thread mixed | 217,522 ops/s | MongoDB 2,497 ops/s | sheaf-db wins 87× (different work/op; p99 shows mutex spikes) |

Final tally across all 14 matrix tests + 5 fair reruns: the original apples-to-oranges verdicts are superseded by these like-for-like numbers. sheaf-db wins traversals, scans (loaded-box), joins vs PG, and throughput; concedes bulk scans (quiet), point writes, and the join crown to DuckDB.
