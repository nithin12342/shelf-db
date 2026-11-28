# PLAN_SQLITE_BENCHBASE_TPCC_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan pairs the BenchBase TPC-C workload on SQLite with the ShelfDB write path. The plan names the source, the data, the cases, the commands, and the gate. The plan does not run the benchmark. The approved documents give no ShelfDB transaction runner for TPC-C. This plan records that gap. It does not claim that the ShelfDB command replays the TPC-C transaction mix.

**TQL:** Topological Query Language, a declarative query over a cellular complex.
**SQL:** Structured Query Language, the standard language for relational databases.
**TPC-C:** Transaction Processing Performance Council C, an online transaction benchmark.

## 2. Original benchmark source

Table 1 answers this question: which link and command identify the original benchmark. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Original benchmark link | `https://github.com/cmu-db/benchbase` | Bundle root `README.md` |
| Official specification link | `https://www.tpc.org/tpcc/` | `BENCHMARK_SOURCES.md`, row 1 |
| Build command | `./mvnw clean package -P sqlite` | Extracted `benchbase-main/README.md`, "How to Build" |
| Documented run command | `java -jar benchbase.jar -b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true` | Same file, Quickstart |
| Help command | `java -jar benchbase.jar -h` | Same file |

The extracted README gives the postgres sample path. It does not give a SQLite sample path. Read `config/sqlite/` in the unpacked distribution and name the file that you use. Do not write a configuration path that you did not read.

## 3. Data set and subset

Table 2 answers this question: which data and subset the pair uses. Rows are facts. Columns are the fact, the value, and the source.

| Fact | Value | Source |
|---|---|---|
| Benchmark name | `tpcc` | Extracted `benchbase-main/README.md`, work bench list |
| Table count (count) | 9 | `BENCHMARK_WIDENING_REPORT.md`, TPC-C row |
| Transaction types (count) | 5 | Same row |
| Warehouse size (MB) | about 100 | Same row |
| Bundle path | `oltp_btree_tpcc/` | Bundle root `README.md` |
| Local data root | `D:\bench\data\sqlite\bench.db` | `sheaf-db/tests/e2e/realtime_bench/PLAN.md`, line 75 |
| Subset | the smallest warehouse count that finishes in 30 minutes | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |

`BENCHMARK_WIDENING_REPORT.md` gives no warehouse count and no measured TPC-C result for ShelfDB. Record the warehouse count that you load. Do not compare a run with an unrecorded warehouse count. BenchBase generates the data through its loader. No manual download is required.

## 4. Test-case documents and specific cases

Table 3 answers this question: which documents name the cases. Rows are documents. Columns are the path, the specific cases, and the text type.

| Document path | Specific cases | Text type |
|---|---|---|
| `BENCHMARK_WIDENING_REPORT.md` | TPC-C, five transactions over nine tables | Counts and workload name |
| Extracted `benchbase-main/README.md` | `tpcc` benchmark, Quickstart and Usage Guide | Command text, no SQL text |
| Extracted `benchbase-main/docker/README.md` | `benchbase` and `benchbase-{profile}` images | Image and run commands |
| `sheaf-db/tests/e2e/realtime_bench/PLAN.md` | Matrix test 3.1, SQLite WAL against `phase2_bench` | Workload name |
| `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md` | Application write and read gates | Commands and expected output |

The approved documents name the benchmark `tpcc` and the count of five transaction types. They give no SQL text and no transaction names for the local run. Record the benchmark name only. Do not invent the five SQL statements.

## 5. Query text

The source documents give no SQL text for the TPC-C transactions. Record the benchmark name `tpcc` only. Do not write SQL for this pair.

The ShelfDB side uses the write path. `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md`, line 162, gives one exact write statement that the repository executed:

```sql
INSERT INTO t VALUES (1, 'alpha'), (2, 'beta')
```

That statement is a smoke gate. It is not a TPC-C transaction. Use it to make sure that the ShelfDB write path works. Do not use it as a TPC-C workload.

## 6. Container image and low-power limits

Table 4 answers this question: which image and limits apply. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Pull step | `docker pull benchbase.azurecr.io/benchbase` | Extracted `benchbase-main/docker/README.md`, line 83 |
| Local build step | `BENCHBASE_PROFILES='sqlite' ./docker/benchbase/build-full-image.sh` | Same file, line 21 and line 25 |
| Run step | `docker run --rm --env BENCHBASE_PROFILE='sqlite' -v results:/benchbase/results benchbase.azurecr.io/benchbase --help` | Same file, line 86, with the profile changed |
| Database image for SQLite | None. SQLite is embedded. | ClickBench `json/sqlite/README.md` |
| SQLite install | `sqlite3` CLI through Scoop | `sheaf-db/tests/e2e/realtime_bench/PLAN.md`, line 75 |
| Concurrent containers (count) | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container CPUs (count) | 1 | Same section |
| Container memory (GB) | 2 | Same section |
| Benchmark threads (count) | 1 | Same section |
| Maximum step duration (minutes) | 30 | Same section |
| Cooldown (minutes) | 3 | Same section |

The profile image name follows the pattern in the extracted README. That README names `benchbase.azurecr.io/benchbase-postgres` for the postgres profile. Make sure that the SQLite profile image exists before you use it. If the image is absent, write `unsupported` with the image name.

## 7. Commands

Table 5 answers this question: which command runs each step. Rows are steps. Columns are the side, the command, and the source.

| Step | Side | Command | Source |
|---|---|---|---|
| Build the SQLite distribution | SQLite | `./mvnw clean package -P sqlite` | Extracted `benchbase-main/README.md`, "How to Build" |
| Load and run TPC-C | SQLite | `java -jar benchbase.jar -b tpcc -c <sqlite-tpcc-configuration> --create=true --load=true --execute=true` | Same README. Use the configuration file that you read from `config/sqlite/`. |
| Run with Maven | SQLite | `mvn clean compile exec:java -P sqlite -Dexec.args="-b tpcc -c <sqlite-tpcc-configuration> --create=true --load=true --execute=true"` | Same README, "How to Run with Maven" |
| Check the ShelfDB write path | ShelfDB | `cargo run -q -p sheaf-app -- --db smoke_db sql "INSERT INTO t VALUES (1, 'alpha'), (2, 'beta')"` | `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md`, line 161 |
| Measure the ShelfDB write path | ShelfDB | `cargo run -p sheaf-engine --bin phase2_bench --release` | `sheaf-db/tests/e2e/realtime_bench/PLAN.md`, matrix test 3.1 |
| Read back the smoke rows | ShelfDB | `cargo run -q -p sheaf-app -- --db smoke_db sql "SELECT id, name FROM t ORDER BY id"` | Same inventory, line 162 |

Use one build job. The low-power plan uses `CARGO_BUILD_JOBS=1`. BenchBase is multi-threaded by configuration. Set one thread in the configuration that you use.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file and the claim it supports.

| File | Claim |
|---|---|
| Extracted `benchbase-main/README.md` | Build, run, and Docker commands |
| Extracted `benchbase-main/docker/README.md` | Image names and pull command |
| `BENCHMARK_WIDENING_REPORT.md` | TPC-C table and transaction counts |
| `sheaf-db/tests/e2e/realtime_bench/PLAN.md` | SQLite WAL against `phase2_bench` |
| `sheaf-db/tests/e2e/realtime_bench/results/wal_insert.json` | ShelfDB write latency record |
| `sheaf-db/tests/e2e/realtime_bench/results/epoch.json` | ShelfDB epoch write record |
| `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md` | Application write and read gates |

## 9. Acceptance gate

1. Make sure that one engine runs at a time.
2. Make sure that the container uses one CPU, 2 GB memory, and one thread.
3. Record the warehouse count, the configuration file name, and the thread count before the run.
4. Make sure that the BenchBase load completes and that the five transaction types execute.
5. Make sure that the ShelfDB smoke write returns `"affected":2` and that the read returns two rows.
6. Record the ShelfDB write timing as a write-path proxy. Do not label it a TPC-C result.
7. If no ShelfDB TPC-C runner exists, write `unsupported` with the named gap. Do not return a TPC-C number for ShelfDB.
8. Make sure that no load or run step lasts longer than 30 minutes.
9. Write the command, the output, the configuration name, and the timing into an evidence file. Put the timestamp, the remote, the branch, and the commit in that file.

## Warnings

- **CAUTION:** No approved document gives a ShelfDB TPC-C transaction runner. A write benchmark is not a TPC-C run.
- **CAUTION:** TPC-C updates persist. Restore the database state before each measured run.
- **NOTE:** The extracted BenchBase README names the postgres sample configuration. The SQLite configuration must be read from the distribution.
