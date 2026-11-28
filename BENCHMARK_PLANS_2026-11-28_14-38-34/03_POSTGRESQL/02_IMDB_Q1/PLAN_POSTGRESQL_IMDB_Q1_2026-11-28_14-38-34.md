# PostgreSQL versus ShelfDB: Internet Movie Database query 1 plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs the first Internet Movie Database query (Q1) on PostgreSQL and ShelfDB. The query filters movies from year 2000, filters ratings at 7.5 and above, then counts rows, sums votes, and averages the rating. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://datasets.imdbws.com/` |
| Data set document | `DATASETS.md`, row 6 |
| Exact query document | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, case 1 native SQL |
| Test case document | `RUNTIME_COMPARISON.md`, section 1 and section 2 |
| Runner document | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 6 |
| Load record | `sheaf-db/tests/e2e/realtime_bench/results/pg_imdb_load_session_20261026.txt` |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Source files | `title.basics.tsv.gz` and `title.ratings.tsv.gz` | files |
| Full title.basics rows in the recorded case | 12,866,375 | rows |
| Full rating rows in the recorded case | 1,721,477 | rows |
| Low-power subset | First 100,000 rows of each file, joined on title identifier | rows per file |
| Local source path | `D:\d2\data\title.*.tsv.gz` | path |
| Join key | title identifier (`tconst`) | column |
| Subset rule | Use the first 100,000 rows first. Load the full files only after the laptop stays responsive. | rule |

The recorded truth value for the full data set is 44,906 rows, 294,435,444 votes, and 8.145506168 mean rating. The low-power subset will not produce that truth value. The gate compares the two engines on the same subset.

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case identifier, the case name, the text status, and the source.

| Case identifier | Case name | Text status | Source |
|---|---|---|---|
| Application case 1 (TQL case 1) | Internet Movie Database join and filter | Exact SQL in the source | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` |
| Matrix case | First Internet Movie Database query (Q1) filter and aggregate | Exact SQL in the source | `RUNTIME_COMPARISON.md`, section 1 |
| Complex case | Second Internet Movie Database query (Q2) four-table top ten | Names only. TQL grammar cannot express this shape. | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 10 |
| Complex case | Third Internet Movie Database query (Q3) four-table decade rollup | Names only. TQL grammar cannot express this shape. | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 10 |

The source gives this exact SQL text for the first Internet Movie Database query:

```sql
SELECT COUNT(*), AVG(r.averagerating), SUM(r.numvotes) FROM title_basics b JOIN title_ratings r ON r.tconst=b.tconst WHERE b.titletype='movie' AND b.startyear>=2000 AND r.averagerating>=7.5
```

The TQL statement for the same shape is:

```text
MATCH HYPERCELL c: Casting OVER (t: Title, p: Person) WHERE t.year > 2000 AND t.rating > 75 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name, t.rating, SECTION_ENERGY };
```

The TQL statement uses the integer 75 for the rating. The SQL text uses 7.5. Read the ratio as a shape comparison, not as an identical predicate comparison. Do not invent SQL for the second and third Internet Movie Database queries.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the subset directory before the run:

```sh
docker run --rm --name postgres_imdb --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\imdb_subset:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 -c shared_buffers=256MB postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap and shared-memory failure |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Original complex query runner | `py imdb_complex_bench.py` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 14 |
| Original multi-engine runner | `py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| Load the subset | `psql -h 127.0.0.1 -U bench -d bench -f /data/load_subset.sql` | This plan |
| Measure the exact SQL | `psql -h 127.0.0.1 -U bench -d bench -f /data/imdb_q1.sql` | This plan |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_imdb sql "<exact SQL from section 5>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB Internet Movie Database binary | `cargo run -p sheaf-engine --bin imdb_fiber_bench --release` | `ADVERSARIAL_ANALYSIS.md`, section 11 |
| ShelfDB answer binary | `cargo run --release --bin imdb_fiber_bench` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_imdb_q1/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Loaded row counts for both tables | Record the subset |
| `load.txt` | Load commands and output | Record the load time |
| `result.csv` | One row per pass with time in milliseconds and checksum | Compare the engines |
| `checksums.txt` | Row count, vote sum, and mean rating for each engine | Run the gate |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Loaded row counts match the manifest | PASS | Not run |
| Three measured passes complete | PASS | Not run |
| PostgreSQL and ShelfDB return the same row count, vote sum, and mean rating | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |
| The second and third queries stay name-only cases until SQL text is supplied | PASS | Not run |

## Warnings

- **CAUTION:** The recorded PostgreSQL baseline comes from 2026-10-20. It does not share a run window with the other engines.
- **CAUTION:** PostgreSQL truncates an unlogged table after an unclean shutdown. Make sure that every load completes before a query starts.
- **NOTE:** The ShelfDB answer read is a stored answer after a one-time bundle build. The build is not part of the query time.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
