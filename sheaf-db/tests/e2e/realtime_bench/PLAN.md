# Real-Time Benchmark Plan: sheaf-db vs 8 Engines

## Objective

Populate sheaf-db plus Neo4j, SQLite, DuckDB, MongoDB, Redis, ScyllaDB, MariaDB, and PostgreSQL with one shared dataset, then run the executable subset of `sheaf-db/COMPARISON_MATRIX.md` live on this machine (i7-1165G7, 16 GB RAM, Windows 11) and record real P50/P99/throughput numbers.

## Environment (verified 2026-10-13)

- Docker 29.8.1 ✅ (ScyllaDB only — no native Windows build exists; data volume on D:, removed after)
- Python 3.11.9 ✅ (harness scripts)
- 16 GB RAM ⚠️ (caps dataset sizes; see Dataset)
- Rust toolchain ✅ (sheaf-db builds already work)
- Package manager: **Scoop** (primary) with `SCOOP=D:\bench\scoop` — no admin, single root dir, clean `scoop uninstall`; winget/choco as fallback where noted
- Disk layout: `D:\bench\apps\` (installs) · `D:\bench\data\<engine>\` (data, deleted per test) · `D:\bench\datasets\` (shared generated data, kept)
- Downloaded datasets (2026-10-13, D: 165 GB free): `D:\bench\datasets\tpch-kit` (dbgen for Tests 2.1/4.1/4.2) · `D:\bench\datasets\YCSB` (workloads for Tests 5.2/6.1); SNAP + IEEE-CIS downloaded by user separately

## Dataset: Fraud-Net (single source of truth)

One generator script emits the dataset in every engine's load format (CSV for Neo4j/DuckDB/SQLite, JSON for MongoDB, HSET stream for Redis, CQL for ScyllaDB, SQL dumps for MariaDB/PostgreSQL, TQL INSERTs for sheaf-db).

| Scale | Accounts | Merchants | Countries | Payments | Purpose |
|-------|----------|-----------|-----------|----------|---------|
| S (smoke) | 1,000 | 200 | 20 | 2,000 | wiring / correctness |
| M (bench) | 100,000 | 5,000 | 200 | 200,000 | all timed runs |
| L (stretch) | 1,000,000 | 20,000 | 500 | 2,000,000 | optional, SQLite/DuckDB/sheaf-db only (16 GB ceiling) |

- Deterministic seed (42) so every engine holds identical rows.
- Discrete fields: ids, names, categories, risk flags. Continuous: 4×f32 stalk payload per cell (4 float columns / array fields elsewhere).

## Per-Test-Case Populations

The fraud-net scales above are the default where a test names no custom shape. The matrix tests below need differently-shaped or larger data; each has its own population. **Laptop scaling rule:** matrix sizes assume server-class hardware; on 16 GB, volumes scale 10× down with shape and ratios preserved, and the scale-down is recorded in the results file next to every number. Partition-count axes are NOT scaled (planning/fan-out overhead is the signal).

| Test | Shape (per engine) | Laptop volume (matrix → scaled) | Generator outputs |
|------|--------------------|---------------------------------|-------------------|
| 1.1 N=5 retrieval | 5-way interactions: User+Device+Merchant+Location+IP | 200k interactions / 1M entities (2M/10M → 10×) | Neo4j CSV (nodes + Transaction intermediates + INVOLVES); TQL INSERTs |
| 1.2 diffusion k=4…8 | scale-free network, Barabási–Albert m=7 (≈15 avg degree) | 100k nodes / ~1.5M edges (unscaled — fits) | Neo4j CSV (edges); sheaf hyperedge TQL |
| 2.1 5-way join | 5 tables (Users, Cards, Devices, Merchants, Terminals) + associative table | 5×100k rows + 200k associative (1M/2M → 10×) | SQL dumps (SQLite/PG/MariaDB), CSV (DuckDB), JSON collections (MongoDB), TQL |
| 2.2 partitions | range-partitioned ledger, partition axis {100, 800, 3200, 6400} | 2M rows total (≈312 rows/part at 6400; counts unscaled, rows fit) | partitioned DDL + loads for PG/MariaDB/SQLite-ATTACH/DuckDB/MongoDB-sharded/ScyllaDB; sheaf uses `phase6_bench` domains |
| 3.1 8R/4W | 0-cells + hypercells pre-populated | 1M + 200k (matches phase2 fixture; unscaled) | SQLite seed DB + sheaf fixture generator |
| 3.2 footprint | reuses T3.1 stores (lookup microbenchmark, no new data) | — | — |
| 4.1 50M scan | single table (id + f32) | 10M rows ≈ 80 MB CSV (50M → 5×; memory ceiling) | CSV (DuckDB/SQLite); stalk slab (sheaf scan micro-bench) |
| 4.2 energy-gated agg | entities + hypercells with W matrices | 1M + 200k (10M/2M → 10×) | per-engine denormalized + sheaf TQL with matrices |
| 5.1 `$lookup` | 5 collections (Customers, Orders, Items, Payments, Shipments) | 5×100k docs (1M → 10×) | MongoDB JSON; SQL dumps; TQL |
| 5.2 128 threads | reuses T5.1 stores (concurrency test, not volume) | — | — |
| 6.1 point lookup | KV pairs (64-bit id → 16 B payload) | 1M timed (+10M stretch optional; load time ~15 min) | Redis HSET stream; sheaf name-index seed |
| 6.2 consistency | reuses T6.1 (5-key groups + Lua vs coboundary) | — | Lua script + TQL |
| 7.1 scatter | rows spread across partition ranges | 5M rows (50M → 10×) | ScyllaDB CQL (partition key spread); sheaf cover fixture |
| 7.2 compaction | reuses T7.1 + sustained write flood | — | flood script + `phase2_bench` |

### Row-count match criteria (setup gate)

After loading, the harness asserts these counts per engine before any timed run:

| Test | Check | Expected |
|------|-------|----------|
| 1.1 | Neo4j `MATCH (t:Transaction) RETURN count(t)`; sheaf cell count | 200k / 200k hypercells |
| 1.2 | Neo4j edge count; sheaf boundary entries | ~1.5M edges both |
| 2.1 | per-table counts + associative count, all engines | 5×100k + 200k |
| 2.2 | rows per partition sum; partition count in catalog | 2M total; {100,800,3200,6400} partitions |
| 3.1 | SQLite rowcount; sheaf `reader.len()` | 1M + 200k |
| 4.1 | DuckDB `COUNT(*)`; scan bench byte total | 10M rows / 80 MB |
| 4.2 | entity + hypercell counts | 1M + 200k |
| 5.1 | per-collection counts | 5×100k docs |
| 6.1 | Redis `DBSIZE`; sheaf index entries | 1M keys |
| 7.1 | ScyllaDB token-range row spread (min/max per range non-empty) | 5M rows, all ranges hit |

Mismatch = setup-gate FAIL; no timed runs until resolved (fixture bug or loader bug, logged as such).

## Engine Setup (headless only — no GUI; one engine at a time; uninstalled after its tests)

| Engine | Install (non-UI, D: drive) | Schema / load path | Remove after test |
|--------|---------------------------|--------------------|-------------------|
| Neo4j | Scoop `extras` bucket (`neo4j-community`) → `D:\bench\scoop`; run `bin\neo4j console`, 4 GB heap cap, APOC off | `LOAD CSV` → nodes + `INVOLVES` edges; index on id | `scoop uninstall neo4j-community` + delete `D:\bench\data\neo4j` |
| SQLite | `sqlite3` CLI via Scoop (KBs, no service) | single file at `D:\bench\data\sqlite\bench.db`, WAL mode, `synchronous=NORMAL` | delete the `.db` + `-wal`/`-shm` files |
| DuckDB | single CLI binary in `D:\bench\apps\duckdb` (no install/service) | CSV COPY into tables | delete binary + working files |
| MongoDB | Scoop (`mongodb-community`) or winget `MongoDB.Server`; `--dbpath D:\bench\data\mongo` | `mongoimport` JSON; compound index per matrix test | uninstall + delete `--dbpath` |
| Redis | Scoop (`redis` port) or winget; config `dir D:\bench\data\redis`, `maxmemory 4gb` | `HSET` per entity + Lua script for Test 6.2 | uninstall + delete dir (dump.rdb/appendonlydir) |
| ScyllaDB | **exception:** no native Windows build — Docker `scylladb/scylla --smp 2 --memory 4G` with `-v D:\bench\data\scylla:/var/lib/scylla` | CQL tables partitioned by entity id | `docker rm -v` + delete volume dir |
| MariaDB | winget/choco `mariadb` with `--datadir=D:\bench\data\mariadb`, InnoDB 2 GB buffer pool | SQL dump load; indexed tables | uninstall + delete datadir |
| PostgreSQL | winget `PostgreSQL.16` (EDB, `--prefix D:\bench\apps\pgsql`) or choco; `--datadir=D:\bench\data\pgsql`, `shared_buffers=2GB` | SQL dump load; B-tree + GIN; partitioned variant for Test 2.2 | uninstall + delete datadir |
| sheaf-db | already builds (native, no install) | TQL INSERT script; needs a **loader binary** (build-mode task — none exists today) | nothing installed (binaries stay; test data under target/ cleaned) |

## Install–Test–Remove Lifecycle (per engine, in test order)

1. **Pre-check:** ≥20 GB free on D:; no other engine installed/running (`scoop list`, service list, `docker ps` all clean).
2. **Install** via package manager to D: (table above); start server with data dir under `D:\bench\data\<engine>\`.
3. **Load** the engine's populations (fraud-net scale + its per-test datasets); run row-count match criteria.
4. **Bench** its mapped matrix tests (3×, CPU% logged).
5. **Export** results to `tests/e2e/realtime_bench/results/`; stop the server.
6. **Uninstall** (`scoop uninstall` / winget / choco) **and delete** `D:\bench\data\<engine>\`; verify removal (binary gone, service gone, dir gone, disk space reclaimed).
7. **Post-check:** D: free space back within 1 GB of pre-check (dataset files under `D:\bench\datasets\` are the only keepers).

Order: SQLite → DuckDB → Redis → MongoDB → MariaDB → PostgreSQL → Neo4j → ScyllaDB → sheaf-db last (native baseline, nothing to install). Rationale: lightest footprint first; heaviest/JVM engines late so their leftovers can't pollute earlier runs.

## Test Execution (live subset of COMPARISON_MATRIX.md)

3× runs each, 60 s cooldown, host CPU% logged alongside (host-qualification precedent: loaded runs annotated, never silently averaged).

| Matrix test | Live procedure |
|-------------|----------------|
| 1.1 N=5 retrieval | Neo4j 5-edge fan-in vs `phase1_bench` lookup; container stats vs `mem_ratio` |
| 1.2 diffusion k=4…8 | Neo4j variable-length Cypher vs `phase3_bench` solve |
| 2.1 5-way join | SQLite/DuckDB/MongoDB/**MariaDB/PostgreSQL** join vs `sheaf_db_app` (join complexity + JSONB vs JSON-раз comparison PG↔MariaDB included) |
| 2.2 partitions | SQLite/DuckDB/MongoDB/ScyllaDB/**MariaDB/PostgreSQL** at 100/800/3200 partitions vs `phase6_bench` (PG declarative partitioning vs MariaDB partitioning) |
| 3.1 8R/4W | SQLite WAL vs `phase2_bench` merge-window |
| 3.2 footprint | SQLite vs `phase1_bench` strided lookups, back-to-back same box |
| 4.1 50M scan | DuckDB `AVG/SUM` vs sheaf-db (**needs new scan micro-bench — build-mode task**) |
| 4.2 energy-gated agg | DuckDB join+UDF vs `sheaf_db_app` energy certificate |
| 5.1 `$lookup` | MongoDB 4-stage pipeline vs sheaf-db incident fetch |
| 5.2 128 threads | MongoDB vs sheaf-db (expect sidestep; record honestly) |
| 6.1 point lookup | Redis loopback vs name-index; `INFO memory` vs 1.220× |
| 6.2 consistency | Redis Lua vs coboundary residual |
| 7.1 scatter | ScyllaDB cross-partition vs `phase7_bench` nerve/loopback |
| 7.2 compaction | ScyllaDB write flood vs `phase2_bench` no-spike gate |

## Outputs

- `sheaf-db/tests/e2e/realtime_bench/` — harness scripts + `results/REALTIME_RESULTS.md` (new file; COMPARISON_MATRIX.md untouched until live data lands, then gains a "live" column)
- Raw per-run logs (3× each) retained next to the summary

## Risks & Mitigations

1. **16 GB RAM + D: disk** — one engine at a time; heap/container caps; scale M for installs. Pre-check ≥20 GB free before each install; post-check space reclaimed.
2. **Background load** — 3× runs + CPU% logs + host-qualified annotations (observed 16→175 µs swings).
3. **Native-vs-native (no more Docker tax)** — all engines except ScyllaDB run as native headless binaries, so comparisons are apples-to-apples on IPC/latency. ScyllaDB keeps its container + disclosure.
4. **Missing build-mode pieces** — TQL bulk loader + 50M scan micro-bench are the only code tasks.
5. **ScyllaDB on 2 cores** — label "laptop-sized topology", not production parity.
6. **Leftover state skewing later runs** — mitigated by the remove-and-verify step (§Lifecycle steps 6–7); a failed post-check blocks the next engine.

## Phase Gates

1. **Setup gate:** all containers healthy + row-counts match per the match-criteria table (scale-S fraud-net + per-test populations).
2. **Smoke gate:** one end-to-end pass of all 14 procedures at scale S, no harness errors.
3. **Bench gate:** 3× scale-M runs with CPU% logs; results file written.
4. **Review gate:** live column added only where 3 stable runs agree (else host-qualified note).

## Explicit Non-Goals

- No changes to sheaf-db algorithms or gates (measurement only, except loader + scan bench).
