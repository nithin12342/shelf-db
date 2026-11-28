# Plan: MariaDB versus ShelfDB on BenchBase Transaction Processing Performance Council Benchmark C (TPC-C)

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB on the BenchBase TPC-C workload. This document is a plan only. It does not pull an image, build BenchBase, load a table, or run a transaction.

BenchBase is a multi-database benchmark framework that uses Java Database Connectivity (JDBC). A gate is an automatic test that compares a result with a truth value. Online transaction processing (OLTP) is a workload of short transactions.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the benchmark and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| BenchBase | `https://github.com/cmu-db/benchbase` | Source, profiles, and workload definitions |
| BenchBase main README | `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md` | Build, profile list, and run command |
| BenchBase Docker README | `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_docker_README.md` | Container build and run scripts |
| BenchBase container README | `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_docker_benchbase_README.md` | Prebuilt image names |
| Bundle README | `attached_bundle_review_2026-11-28_14-19-54/README.md` | Bundle folder `oltp_btree_tpcc/` |
| TPC-C rule source | `https://www.tpc.org/tpcc/` | Official benchmark specification |

BenchBase generates the TPC-C tables with its own loader. The data set is not a download. The extracted bundle names the folder `oltp_btree_tpcc/`. Use one warehouse and one terminal for the low-power run. Make sure that those values appear in the configuration file. Record the values in the manifest. Do not use the default warehouse count if it exceeds the laptop limits.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which document names the TPC-C case. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md` | Benchmark list | TPC-C |
| `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md` | Quickstart | `tpcc` execution with `sample_tpcc_config.xml` |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Problem 1 | Durable single-row write, not TPC-C |
| `BENCHMARK_SOURCES.md` | Row 1 | TPC-C link for MariaDB |

The specific case is the `tpcc` benchmark named by BenchBase. The approved documents give no TPC-C result number for MariaDB and no TPC-C result number for ShelfDB. The durable single-row write in the five problem workloads is a different case. Do not use it as a TPC-C result.

## 4. Exact query text

The extracted README names the benchmark and the run command. It does not give the TPC-C transaction SQL. BenchBase generates the transactions from its own definitions. Do not invent SQL for this case.

Names only apply here. The named units are `tpcc`, the `mariadb` build profile, and the sample TPC-C configuration file. The run team must read the exact configuration file from the unpacked BenchBase distribution. The README shows `config/postgres/sample_tpcc_config.xml` for PostgreSQL. Do not assume that the same path and the same values work for the MariaDB profile.

## 5. Container image and install or pull step

Table 3 answers this question: which image and tools the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <results>:/benchbase/results mariadb:13.0.2` |
| BenchBase | Build the MariaDB profile on the host | `git clone --depth 1 https://github.com/cmu-db/benchbase.git` then `cd benchbase` then `./mvnw clean package -P mariadb` |
| BenchBase | Unpack the distribution | `cd target` then `tar xvzf benchbase-mariadb.tgz` then `cd benchbase-mariadb` |

The README lists `mariadb` as a supported Maven profile. The container scripts can build a `benchbase-mariadb` image. Do not run that image in this plan. A second container breaks the one-container limit. Run BenchBase on the host against the one MariaDB container. Record the image digest in `environment.json`.

## 6. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Benchmark terminals | 1 | terminal | Keeps transaction load low |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives a spread for the workload |
| Stop rule for a load step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Show BenchBase help arguments | `java -jar benchbase.jar -h` | Supported argument list |
| Original README command, PostgreSQL profile | `java -jar benchbase.jar -b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true` | TPC-C load and execution |
| MariaDB command, same shape | `java -jar benchbase.jar -b tpcc -c <mariadb_sample_config> --create=true --load=true --execute=true` | TPC-C load and execution in one MariaDB container |
| Recorded one-step Maven form | `mvn clean compile exec:java -P postgres -Dexec.args="-b tpcc -c config/postgres/sample_tpcc_config.xml --create=true --load=true --execute=true"` | Same benchmark without packaging |
| ShelfDB matching command | `unsupported` | Error: no TPC-C binary is named in the approved documents or in `sheaf-db/crates/sheaf-engine/src/bin/` |

Read `<mariadb_sample_config>` from the unpacked distribution. Do not write a guessed path. Do not use `ring_wal_bench` or `fiber_join_bench` as a TPC-C substitute. Those binaries measure a write path and a join, not the TPC-C transaction mix.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `attached_bundle_review_2026-11-28_14-19-54/docs/benchbase-main__benchbase-main_README.md` | Profile list and original run command | Extracted README |
| `benchbase-<profile>/results/` | Throughput and latency per transaction type | BenchBase execution output |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, configuration values | Written before the run |
| `results/local_docker_benchmark_<timestamp>/mariadb/load.txt` | Load rows and seconds | BenchBase load output |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row per measured pass | Written after three measured passes |
| `results/local_docker_benchmark_<timestamp>/shelfdb/unsupported.txt` | Cause of the ShelfDB gap | Written when the command is not present |

## 9. Acceptance gate

Table 7 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread, one terminal | Pass only when true | Record `docker inspect` output and the configuration file |
| Load gate | BenchBase reports a completed TPC-C load | Pass only when true | Give the loaded row counts |
| Truth gate | BenchBase reports no failed transaction | Pass only when true | Do not time a failed run |
| ShelfDB gap | `unsupported` with the named cause | Pass only when the error is explicit | Do not return a fake result |
| Evidence completeness | Commands, outputs, and configuration values are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

## Warnings

- **CAUTION:** Build BenchBase on the host. Do not start a second container.
- **CAUTION:** The README shows a PostgreSQL sample configuration. Read the MariaDB sample from the unpacked distribution.
- **NOTE:** ShelfDB has no named TPC-C binary. The plan records `unsupported` with a clear error.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
