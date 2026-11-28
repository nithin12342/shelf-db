# PostgreSQL versus ShelfDB: LDBC Social Network Benchmark plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs the Linked Data Benchmark Council Social Network Benchmark (LDBC SNB) interactive workload on PostgreSQL and ShelfDB. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.
- **Unsupported:** a declared result that names a missing input. An unsupported case does not produce a time.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://github.com/ldbc/ldbc_snb_interactive_v1_impls` |
| Benchmark specification | `https://github.com/ldbc/ldbc_snb_docs` |
| Data set document | `DATASETS.md`, row 7 |
| Test case document | `attached_bundle_review_2026-11-28_14-19-54/docs/ldbc_snb_interactive_v1_impls-main__ldbc_snb_interactive_v1_impls-main_README.md` |
| PostgreSQL document | `attached_bundle_review_2026-11-28_14-19-54/docs/ldbc_snb_interactive_v1_impls-main__ldbc_snb_interactive_v1_impls-main_postgres_README.md` |
| Small data document | `attached_bundle_review_2026-11-28_14-19-54/docs/ldbc_snb_interactive_v1_impls-main__ldbc_snb_interactive_v1_impls-main_test-data.md` |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Serializer | `CsvMergeForeign` | name |
| Benchmark scale factor | 1 with 250 persons and 1 year | scale |
| Local small data path | `postgres/test-data/` | path |
| Pre-generated alternative | `social_network-csv_merge_foreign-sf*` | name |
| Data source | SURF or Center for Mathematics and Computer Science repository | repository |
| Read queries in the workload | 22 | queries |
| Update queries in the workload | 7 | queries |
| Subset rule | Use the small test data first. Restore the database before each run. | rule |

The generator needs about 250 persons for the parameter script to run. The workload contains updates that persist. A run must start from loaded data.

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case name, the text status, and the source.

| Case name | Text status | Source |
|---|---|---|
| 22 interactive read queries | Names only. Exact SQL not extracted locally. | LDBC SNB main README |
| 7 interactive update queries | Names only. Exact SQL not extracted locally. | LDBC SNB main README |
| Validation mode | Commands in the source. Query text comes from the tool. | LDBC SNB PostgreSQL README |
| Benchmark mode | Commands in the source. Query text comes from the tool. | LDBC SNB PostgreSQL README |

The extracted documents do not give the SQL text of the 22 read queries or the 7 updates. Do not invent SQL. A case that has no supplied text must report `unsupported` with the cause.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the small data directory before the run:

```sh
docker run --rm --name postgres_ldbc --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\ldbc_small_data:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Generated persons | 250 | Meets the parameter script floor |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Set the data path | `export POSTGRES_CSV_DIR=$(pwd)/test-data/` | LDBC SNB PostgreSQL README |
| Build the reference implementations | `mvn clean package -DskipTests -Pcypher,postgres` | LDBC SNB main README |
| Start the container and load once | `docker-compose build && docker-compose up` | LDBC SNB PostgreSQL README |
| Load with the Bash scripts | `scripts/load-in-one-step.sh` | LDBC SNB PostgreSQL README |
| Create validation parameters | `driver/create-validation-parameters.sh` | LDBC SNB main README |
| Run the original validation mode | `driver/validate.sh` | LDBC SNB PostgreSQL README |
| Run the original benchmark mode | `driver/benchmark.sh` | LDBC SNB PostgreSQL README |
| Restore before a repeat run | `scripts/restore-database.sh` | LDBC SNB PostgreSQL README |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_ldbc sql "<supplied SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB graph case | No LDBC implementation exists in ShelfDB. Record `unsupported` with this cause. | This plan |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_ldbc_snb/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Loaded row counts and generated person count | Record the subset |
| `load.txt` | Output of the load script | Record the load time |
| `result.csv` | One row per query with time in milliseconds and result hash | Compare the passes |
| `checksums.txt` | Reference result and observed result per query | Run the gate |
| `unsupported.csv` | ShelfDB case and cause | Record the declared unsupported case |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| The database is restored before each run | PASS | Not run |
| Validation mode passes on the small data | PASS | Not run |
| Three measured passes complete in benchmark mode | PASS | Not run |
| Result hashes match across the three passes | PASS | Not run |
| The ShelfDB case reports `unsupported` and a cause | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |

## Warnings

- **CAUTION:** The workload contains updates. A run without a restore is not comparable with the next run.
- **CAUTION:** The upstream benchmark expects Java 8 and Docker 19 or later. Make sure that the host meets those levels.
- **NOTE:** The SQL text is not in the extracted documents. Do not write that text by hand.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
