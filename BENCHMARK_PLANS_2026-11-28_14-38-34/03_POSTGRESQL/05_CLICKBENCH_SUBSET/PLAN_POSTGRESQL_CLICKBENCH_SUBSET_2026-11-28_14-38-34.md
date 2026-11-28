# PostgreSQL versus ShelfDB: ClickBench subset plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs a small slice of the ClickBench clickstream workload on PostgreSQL and ShelfDB. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.
- **Unsupported:** a declared result that names a missing input. An unsupported case does not produce a time.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://github.com/ClickHouse/ClickBench` |
| Benchmark site | `https://benchmark.clickhouse.com/` |
| Data set document | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4, rule 10 |
| Test case document | `attached_bundle_review_2026-11-28_14-19-54/docs/ClickBench-main__ClickBench-main_README.md` |
| Data download link | `https://datasets.clickhouse.com/hits_compatible/hits.csv.gz` |
| Query definition in the tool | `queries.sql` in each system directory of the upstream tool |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Full table rows in the benchmark | 99,997,497 | rows |
| Low-power subset | First 100,000 rows of `hits.csv.gz` | rows |
| Table shape | One flat table | tables |
| Local subset path | `D:\bench\clickbench_subset\hits.csv` | path |
| Query count in the full workload | 43 | queries |
| Query count in this subset | First 3 queries of the 43-query workload | queries |
| Repetitions per query | 3 | runs |
| Subset rule | Decompress the file before load. Load one file. Do not split the file. | rule |

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case name, the text status, and the source.

| Case name | Text status | Source |
|---|---|---|
| First query of the 43-query workload | Name only. Text not extracted locally. | Upstream `queries.sql` |
| Second query of the 43-query workload | Name only. Text not extracted locally. | Upstream `queries.sql` |
| Third query of the 43-query workload | Name only. Text not extracted locally. | Upstream `queries.sql` |

The extracted README states that each system directory holds `queries.sql` with 43 queries. The extracted local document set does not contain that file. Do not invent SQL. A case that has no supplied text must report `unsupported` with the cause.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the subset directory before the run:

```sh
docker run --rm --name postgres_clickbench --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\clickbench_subset:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Loaded rows | 100,000 | Keeps the load small |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Download the data | `curl -LO https://datasets.clickhouse.com/hits_compatible/hits.csv.gz` | ClickBench main README |
| Decompress the data | `gzip -d hits.csv.gz` | This plan |
| Original main script | `./benchmark.sh` | ClickBench main README |
| Original query loop | `./run.sh` | ClickBench main README |
| Original sub-task scripts | `./install`, `./load`, `./query` | ClickBench main README |
| Load the subset | `psql -h 127.0.0.1 -U bench -d bench -f /data/create.sql` | This plan |
| Measure one named case | `psql -h 127.0.0.1 -U bench -d bench -f /data/<query name>.sql` | This plan after text is supplied |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_clickbench sql "<supplied SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_clickbench_subset/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Loaded row count and column names | Record the subset |
| `load.txt` | Load commands and output | Record the load time |
| `result.csv` | One row per run with time in milliseconds and checksum | Compare the engines |
| `checksums.txt` | Truth value and observed value for each query | Run the gate |
| `unsupported.csv` | Query name and cause for each missing text | Record declared unsupported cases |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Loaded row count equals 100,000 | PASS | Not run |
| Three measured passes complete for each query with supplied text | PASS | Not run |
| PostgreSQL and ShelfDB checksums match for each query | PASS | Not run |
| Each query with no text reports `unsupported` and a cause | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |

## Warnings

- **CAUTION:** The full workload holds 99,997,497 rows. Do not load it on this host.
- **CAUTION:** Do not create extra indexes or pre-aggregated tables for this pair. The upstream rules exclude them.
- **NOTE:** The query text is not in the extracted documents. Do not write that text by hand.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
