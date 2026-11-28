# DuckDB versus ShelfDB benchmark plan: BenchBase TPC-C

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan records the BenchBase transaction processing benchmark C (TPC-C) pair for DuckDB and ShelfDB. The extracted BenchBase documentation lists no DuckDB profile and no ShelfDB adapter. This plan therefore states the original command and sets an `unsupported` gate. It does not invent a driver. It does not install software and does not run a benchmark.

## 2. Concept definitions

- **DuckDB:** an embedded columnar database that runs SQL in one process.
- **ShelfDB:** a sheaf database that stores answers in binary cellular storage sections.
- **BenchBase:** a multi-database load generator that talks to databases through Java database connectivity.
- **TPC-C:** a transaction processing benchmark that mixes reads and writes.
- **Gate:** an automatic comparison of an engine result with a truth value.

## 3. Pair identity

Table 1 answers this question: what identifies this pair? Rows are attributes. Columns are the attribute, the recorded value, and the source.

| Attribute | Recorded value | Source |
|---|---|---|
| Pair | DuckDB versus ShelfDB | This plan |
| Benchmark family | BenchBase with TPC-C | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, section 4, item 2 |
| Original link | `https://github.com/cmu-db/benchbase` | Master index, Table 3 |
| Workload name | `tpcc` | Extracted BenchBase README, line 40 |
| Database profiles | `postgres`, `mysql`, `mariadb`, `sqlite`, `cockroachdb`, `phoenix`, `spanner` | Extracted BenchBase README, line 90 |

The profile list has no DuckDB entry. The approved documents name no ShelfDB adapter for BenchBase.

## 4. Dataset and subset

Table 2 answers this question: which data does this pair use? Rows are data attributes. Columns are the attribute, the value, and the preparation step.

| Attribute | Value | Preparation step |
|---|---|---|
| Data set | TPC-C tables | Let the BenchBase loader create them with `--load=true`. |
| Source | BenchBase configuration file | Read `config/postgres/sample_tpcc_config.xml` as the recorded example. |
| Subset | Not recorded in the approved documents | Use the smallest warehouse count that the configuration allows. |
| Record choice | Write the warehouse count in `environment.json` | Make the run reproducible. |
| Data mount | `-v results:/benchbase/results` | Read extracted BenchBase README, line 211. |

## 5. Test-case document and specific cases

Table 3 answers this question: which recorded cases does this pair cover? Rows are sources. Columns are the document path, the specific case, and the recorded detail.

| Document path | Specific case | Recorded detail |
|---|---|---|
| `BENCHMARK_SOURCES.md`, row 1 | TPC-C link | `https://www.tpc.org/tpcc/` |
| `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, section 4, item 2 | BenchBase TPC-C | Java and Maven workload |
| `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md`, lines 61 to 80 | `tpcc` benchmark | Named workload in the framework list |
| Same README, line 40 | TPC-C execution command | Create, load, and execute flags |
| Master index, Table 3 | BenchBase source link | `https://github.com/cmu-db/benchbase` |

The specific test cases are the transactions that BenchBase defines for the `tpcc` benchmark. The approved documents give no transaction SQL text. This plan does not name individual transactions and does not write SQL for them.

## 6. Query text

The extracted BenchBase README gives the benchmark name and the run flags. It gives no SQL text. `recursive test/QUERIES_2026-11-22_15-04-04.md` gives no BenchBase query text. This plan gives names only. Do not invent SQL. Do not substitute the TPC-H text from the other plans.

## 7. Container image and install step

Table 4 answers this question: which image and install step provide BenchBase? Rows are items. Columns are the item, the value, and the reason.

| Item | Value | Reason |
|---|---|---|
| Runtime image | `benchbase.azurecr.io/benchbase` | Extracted BenchBase README, line 211 |
| Development image | `benchbase.azure.cr.io/benchbase-dev` | Extracted BenchBase README, line 188 |
| Pull step | `docker pull benchbase.azurecr.io/benchbase` | Pull one image at a time. |
| Build step | `./mvnw clean package -P postgres` | Extracted BenchBase README, lines 23 to 27 |
| DuckDB image | None recorded | The profile list has no DuckDB entry. |

## 8. Low-power limits

Table 5 answers this question: which limits govern the local run? Rows are limits. Columns are the limit, the value, and the command or action.

| Limit | Value | Command or action |
|---|---|---|
| Concurrent containers | 1 | Stop all other containers first. |
| Central processing unit | 1 | Use `--cpus=1`. |
| Memory | 2 gigabytes | Use `--memory=2g`. |
| Threads | 1 | Set the BenchBase thread count to one in the configuration. |
| Long step | 30 minutes maximum | Stop the step at the limit. |
| Cooldown | 3 minutes | Wait before the next pair. |

## 9. Commands

The original command comes from `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md`, line 40.

```bash
java -jar benchbase.jar -b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true
```

Table 6 answers this question: which commands belong to the two sides? Rows are sides. Columns are the side, the command, and the status.

| Side | Command | Status |
|---|---|---|
| Original BenchBase with Docker | `docker run -it --rm --env BENCHBASE_PROFILE='postgres' -v results:/benchbase/results benchbase.azurecr.io/benchbase -b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true` | Runs only against a supported profile |
| Original BenchBase with Maven | `mvn clean compile exec:java -P postgres -Dexec.args="-b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true"` | Runs only against a supported profile |
| Matching ShelfDB command | None recorded | Report `unsupported` with the cause |

Do not run the PostgreSQL form and record it as a DuckDB result. Do not write a BenchBase command with a DuckDB profile that the documentation does not list. Run one warm-up pass for a supported profile. Do not time it. Run three measured passes with one thread.

## 10. Evidence files

Table 7 answers this question: which files hold the evidence? Rows are files. Columns are the file, the claim, and the action.

| File | Claim | Action |
|---|---|---|
| Extracted BenchBase README, lines 90 to 94 | Supported profile list | Read it to support the gate |
| `results/local_docker_benchmark_<timestamp>/02_DUCKDB/BENCHBASE_TPCC/environment.json` | Image digest and warehouse count | Write it even when the gate reports `unsupported` |
| Same folder, `pulls.txt` and `container_inspect.txt` | Pull and limit record | Write it if an image was pulled |
| Same folder, `manifest.json`, `load.txt`, `result.csv`, `checksums.txt` | Rows, load, times, checksums | Write them only for a supported profile |
| Same folder, `summary.md` | Remote, branch, top commit, verdict | Write `unsupported` and the named cause |

## 11. Acceptance gate

1. Make sure that one container runs at a time.
2. Make sure that `docker inspect` reports one central processing unit and 2 gigabytes of memory.
3. Make sure that the profile list in `environment.json` names no DuckDB profile.
4. Write `unsupported` in `summary.md`. Name the cause as the missing DuckDB profile in BenchBase.
5. Write `unsupported` in `summary.md`. Name the cause as the missing ShelfDB adapter in BenchBase.
6. Do not write a result row, a time, or a checksum for an unsupported side.
7. Make sure that no PostgreSQL result is labelled as a DuckDB result.
8. Write all evidence files that Table 7 requires.
9. Stop the container and wait three minutes before the next pair.

## Warnings

- **CAUTION:** The host is a low-power laptop. Do not run Docker, a release build, and a benchmark together.
- **CAUTION:** BenchBase updates persist. Restore the data before each run.
- **WARNING:** Do not present a fake result for an unsupported side. Report `unsupported` with the cause.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
