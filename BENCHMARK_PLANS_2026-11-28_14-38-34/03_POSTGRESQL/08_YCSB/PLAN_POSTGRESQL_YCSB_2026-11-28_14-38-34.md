# PostgreSQL versus ShelfDB: Yahoo! Cloud Serving Benchmark plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs the Yahoo! Cloud Serving Benchmark (YCSB) on PostgreSQL and ShelfDB. The extracted document set gives workload names only. It gives no SQL text. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.
- **Unsupported:** a declared result that names a missing input. An unsupported case does not produce a time.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://github.com/brianfrankcooper/YCSB` |
| Alternate runner link | `https://github.com/cmu-db/benchbase` |
| Data set document | `DATASETS.md`, row 3 |
| Test case document | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4, rule 9 |
| Runner document | `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md` |
| Extracted YCSB README | Not present in `attached_bundle_review_2026-11-28_14-19-54/docs/` |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Source | YCSB tool source or BenchBase YCSB benchmark | tool |
| Local source path | `D:\bench\datasets\YCSB\` | path |
| Key count | 100,000 | keys |
| Workloads named in the source | Workloads A through F | workloads |
| Workload count per run | 1 | workload |
| Benchmark threads | 1 | threads |
| Subset rule | Use 100,000 keys. Do not use 1,000,000 keys. | rule |

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case name, the text status, and the source.

| Case name | Text status | Source |
|---|---|---|
| Workload A | Name only. No SQL in the source. | `DATASETS.md` |
| Workload B | Name only. No SQL in the source. | `DATASETS.md` |
| Workload C | Name only. No SQL in the source. | `DATASETS.md` |
| Workload D | Name only. No SQL in the source. | `DATASETS.md` |
| Workload E | Name only. No SQL in the source. | `DATASETS.md` |
| Workload F | Name only. No SQL in the source. | `DATASETS.md` |

The extracted BenchBase README names `ycsb` as a supported benchmark. It does not give the SQL text of the workloads. Do not invent SQL. A case that has no supplied text must report `unsupported` with the cause.

## 6. Container image and low-power limits

Pull one image. Use this command. The runner stays on the host as a Java process, so only one container runs.

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the result directory before the run:

```sh
docker run --rm --name postgres_ycsb --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\ycsb_results:/benchbase/results -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Keys | 100,000 | Keeps the load small |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Clone the runner | `git clone --depth 1 https://github.com/cmu-db/benchbase.git` | BenchBase main README |
| Build the PostgreSQL profile | `./mvnw clean package -P postgres` | BenchBase main README |
| Original benchmark command shape | `java -jar benchbase.jar -b ycsb -c config/postgres/<YCSB configuration file> --create=true --load=true --execute=true` | BenchBase main README |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_ycsb sql "<supplied SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB key-value case | No YCSB implementation exists in ShelfDB. Record `unsupported` with this cause. | This plan |

The BenchBase README gives the exact TPC-C sample configuration path. It does not give the YCSB configuration file name. Do not invent that name. Resolve it from the cloned tool before the run.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_ycsb/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, key count | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Loaded key count and workload name | Record the subset |
| `load.txt` | Tool output from the load phase | Record the load time |
| `result.csv` | One row per operation type with time in microseconds and operation count | Compare the passes |
| `checksums.txt` | Operation counts for each pass | Run the gate |
| `unsupported.csv` | ShelfDB case and cause | Record the declared unsupported case |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Loaded key count equals 100,000 | PASS | Not run |
| Three measured passes complete for the named workload | PASS | Not run |
| Operation counts match across the three passes | PASS | Not run |
| The ShelfDB case reports `unsupported` and a cause | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |

## Warnings

- **CAUTION:** The YCSB README is not in the extracted document set. Do not present the command shape as an extracted exact command.
- **CAUTION:** Do not use 1,000,000 keys on this host.
- **NOTE:** No SQL text is available for Workloads A through F. Do not write that text by hand.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
