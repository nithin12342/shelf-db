# Low-power Docker benchmark execution plan

**Timestamp:** 2026-11-28_14-19-54, local time 14:19:54 +05:30.
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `3c7394681e48f21ecbe706bbe286bb46f0e24dde`.

## 1. Goal

This plan runs the database comparisons one database at a time. It keeps the laptop responsive. The host is an AMD Ryzen 3 7000 series machine with Radeon integrated graphics. Treat it as an entry-level laptop. Do not run two database containers at once. Do not run a container and a release build at once.

## 2. Hardware limits

Table 1 answers this question: which limits govern the local run. Rows are limits. Columns are the value and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps central processing unit and memory load low |
| Container CPUs | 1 | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 GB | Avoids swap and laptop stutter |
| Benchmark threads | 1 | Makes timings comparable and avoids thermal spikes |
| Data extraction | Selected files only | Avoids 5.22 GB of uncompressed data at once |
| Long build or load step | Stop after 30 minutes | Prevents a hot laptop and long stalled runs |
| Cooldown between engines | 3 minutes | Lets the central processing unit settle |

## 3. Source of truth

Use these files for data and queries:

1. `recursive test/DATASETS_2026-11-22_15-04-04.md` for dataset names and sizes.
2. `recursive test/QUERIES_2026-11-22_15-04-04.md` for query sources.
3. `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` for the 24 SQL statements.
4. `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` for TQL, SQL, and q text.
5. `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` for the 50 case names.

Do not invent query text. If a source gives a workload name only, record the name only.

## 4. Data preparation

1. Keep `shelf_db_all_datasets.zip` intact. Do not extract all datasets.
2. Extract only `tpch/sf1_tbl/` when the TPC-H phase starts.
3. Use scale factor 0.01 first. Generate it with `dbgen` from the nested TPC-H kit.
4. Use scale factor 1 only after the 0.01 run passes and the laptop stays responsive.
5. For IMDb, start from `title.ratings.tsv.gz` and `title.basics.tsv.gz`.
6. For fraud work, start from the first 100,000 rows of the IEEE-CIS CSV files.
7. For graph work, start from `graph/facebook_combined.txt.gz` only.
8. For time-series work, generate 100,000 tick rows with TSBS. Do not generate 10 million rows.
9. For key-value work, use 100,000 keys with memtier. Do not use 1 million keys.
10. For ClickBench, select a 100,000-row slice. Do not load the full clickstream data.

## 5. Docker setup

1. Start Docker Desktop. Wait until `docker info` reports a Linux server.
2. Set Docker Desktop to two central processing units and 3 GB memory.
3. Stop all running containers before each engine.
4. Remove the previous engine container and its named volume after each run.
5. Pull one image at a time. Do not start a pull while a benchmark runs.
6. Use this shape for every container:
   `docker run --rm --name <engine> --cpus=1 --memory=2g --pids-limit=128 ...`
7. Mount only the subset data directory. Use a read-only mount for data.
8. Use `--network=none` when the engine does not need a network.

## 6. Engine order

Table 2 answers this question: which engine order the run uses. Rows are engines. Columns are the order, the engine, the workload, and the exit action.

| Order | Engine | Workload | Exit action |
|---|---|---|---|
| 1 | SQLite | TPC-H and IMDb SQL | Stop process and save timing |
| 2 | DuckDB | TPC-H scan and join | Stop process and save timing |
| 3 | PostgreSQL | TPC-H and IMDb SQL | Stop container and remove volume |
| 4 | MariaDB | TPC-H and IMDb SQL | Stop container and remove volume |
| 5 | Redis | Point lookup with memtier | Stop container and remove volume |
| 6 | Valkey | Point lookup with memtier | Stop container and remove volume |
| 7 | Neo4j | Facebook two-hop and LDBC read | Stop container and remove volume |
| 8 | ClickHouse | ClickBench slice | Stop container and remove volume |
| 9 | TimescaleDB | TSBS tick slice | Stop container and remove volume |
| 10 | InfluxDB | TSBS tick slice | Stop container and remove volume |
| 11 | QuestDB | TSBS tick slice | Stop container and remove volume |
| 12 | ScyllaDB | Wide-column and fraud slice | Stop container and remove volume |
| 13 | MongoDB | Nested JSON slice | Stop container and remove volume |
| 14 | ShelfDB | Same queries through local binaries | Stop process and save timing |

Use Kdb+ only after the license check. If the token is absent, record `unsupported` with the named cause.

## 7. Per-engine procedure

Run these steps for each engine in Table 2.

1. Write `environment.json` with host, Docker version, image digest, and data subset.
2. Pull or load one image.
3. Start one container with the limits from section 5.
4. Load one data subset.
5. Make sure that the loaded row count matches the subset manifest.
6. Run one warm-up pass. Do not time it.
7. Run three measured passes. Use one thread.
8. Capture query time, load time, and result checksum.
9. Compare the checksum with the truth value from the query source.
10. Write `result.csv` with one row per query.
11. Stop the container. Remove its volume.
12. Wait three minutes before the next engine.

## 8. Evidence layout

Write evidence under a new folder with the run timestamp. Use this shape:

```text
results/local_docker_benchmark_<timestamp>/
  environment.json
  pulls.txt
  container_inspect.txt
  <engine>/manifest.json
  <engine>/load.txt
  <engine>/result.csv
  <engine>/checksums.txt
  summary.md
```

Include the remote, branch, and top commit in `summary.md`.

## 9. Acceptance gate

A run is accepted only when all of the following hold:

1. One engine runs at a time.
2. Every query returns the stated row count and checksum.
3. Each container stays within one central processing unit and 2 GB memory.
4. No load or query step runs longer than 30 minutes.
5. The laptop stays responsive during the run.
6. The evidence folder holds commands, outputs, and checksums.

## Warnings

- **WARNING:** Do not run Docker, a release build, and a benchmark suite at the same time. The laptop can stall.
- **CAUTION:** LDBC updates persist. Restore the data before each LDBC run.
- **CAUTION:** Docker Desktop can keep Windows Subsystem for Linux memory after a container stops. Restart Docker Desktop between engine groups when memory stays high.
- **NOTE:** This is a plan only. No database image was pulled and no benchmark was run.
