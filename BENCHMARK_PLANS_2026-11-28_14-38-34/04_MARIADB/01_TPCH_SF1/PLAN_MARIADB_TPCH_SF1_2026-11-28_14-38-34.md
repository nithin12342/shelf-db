# Plan: MariaDB versus ShelfDB on Transaction Processing Performance Council Benchmark H (TPC-H), scale factor 1

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB on scale factor 1 (SF1). This document is a plan only. It does not pull an image, load a table, or run a query.

A TQL statement is a declarative query over a cellular complex. SQL is the standard query language for relational databases. A gate is an automatic test that compares a result with a truth value.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the benchmark and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| TPC-H kit | `https://github.com/gregrahn/tpch-kit` | `dbgen` and `qgen` tools |
| Official specification | `https://www.tpc.org/tpch/` | Benchmark rules and reference data |
| Extracted kit README | `attached_bundle_review_2026-11-28_14-19-54/docs/tpch-kit-master__tpch-kit-master_README.md` | Build and generation commands |
| Extracted generator README | `attached_bundle_review_2026-11-28_14-19-54/docs/tpch-kit-master__tpch-kit-master_dbgen_README` | Command-line arguments for `dbgen` and `qgen` |
| Data set record | `DATASETS.md` | SF1 tables, 1.101 gigabytes |
| Bundle folder | `shelf_db_all_datasets.zip`, folder `tpch/sf1_tbl/` | Generated SF1 tables |

The data set is the eight SF1 tables. The measured cases use `lineitem` with 6,001,215 rows. The three-table case also uses `orders` and `customer`. The TQL case also names `partsupp`. The SQL corpus in `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` uses `region`, `nation`, `part`, and `supplier`. Load those tables from the same SF1 generation. Do not mix a 0.01 scale subset with SF1 in one timing row.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which documents name the SF1 cases. Rows are documents. Columns are the document path, the table or section, and the specific case.

| Document path | Table or section | Specific case |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 2 | TPC-H SF1 scan and aggregate |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 3 | TPC-H SF1 three-table join |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 1, case 3 | TQL analytical filter shape |
| `RUNTIME_COMPARISON.md` | Section 3 | MariaDB join, 52,444 ms |
| `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` | Statements Q05, Q06, Q12, Q16, Q21, Q24 | Line-item count, sum, null filter, join, urgent-order filter, total count |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Case 3 | TQL text and native SQL text |

The recorded MariaDB version is 13.0.2. Table 2 records 1,279.3 milliseconds for the scan and aggregate. Table 3 records 23,086.1 milliseconds for the join. `RUNTIME_COMPARISON.md` records 52,444 milliseconds for the join in a different run. Do not mix those two join numbers in one row.

## 4. Exact query text

The source gives exact text for the scan and aggregate. It gives a description only for the three-table join. Do not invent SQL for that join.

Exact SQL for the scan and aggregate, from Table 1 case 3 of `TQL_APPLICATION_COMPARISON_2026-10-26.md`:

```sql
SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0
```

Exact TQL for case 3, from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`:

```text
MATCH HYPERCELL l: LineItem OVER (o: Order, p: PartSupp) WHERE l.shipdate < 19980902 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { l.line_id, l.quantity, SECTION_ENERGY };
```

Description for the three-table join, quoted from Table 3 of `TQL_APPLICATION_COMPARISON_2026-10-26.md`:

> The query joins `customer`, `orders`, and `lineitem`, filters `l_shipdate < DATE '1995-01-01'`, groups by `c_name`, and returns the top 100 rows.

The ShelfDB scan computes `SUM(price*(1-discount))`. The SQL engines compute `AVG` and `SUM` of `l_extendedprice` with the filter `l_extendedprice > 100.0`. Read each ratio as one-pass scan cost, not as an identical predicate. A TQL optimizer cost is an estimate. Do not state it as an execution time. The TQL-to-fiber execution bridge is not built.

## 5. Container image and install or pull step

Table 3 answers this question: which image and tools the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <subset>:/data:ro mariadb:13.0.2` |
| TPC-H kit | Install build tools | `sudo apt-get install git make gcc` |
| TPC-H kit | Clone and build | `git clone https://github.com/gregrahn/tpch-kit.git` then `cd tpch-kit/dbgen` then `make MACHINE=LINUX DATABASE=POSTGRESQL` |

Make sure that the chosen `DATABASE` value matches MariaDB. The README names POSTGRESQL in its example and tells the reader to see `Makefile` and `tpcd.h` for valid values. Read those files in the cloned kit. Do not assume that the example dialect works on MariaDB. Record the image digest in `environment.json`.

## 6. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives a spread for each query |
| Stop rule for a load step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Generate SF1 data | `dbgen -s 1` from `tpch-kit/dbgen` | Eight `.tbl` files in `DSS_PATH` |
| Generate all queries | `qgen -v -c -d -s 1 > tpch-stream.sql` | SQL text for 22 queries |
| Load data | `mysql --host=127.0.0.1 --user=root --password=<password> <load_script.sql>` | Stated row count per table |
| Original recorded runner | `py run_all.py --keep-up` | Seven engine results and twelve ShelfDB binaries |
| MariaDB timed case | `mysql --host=127.0.0.1 --user=root --password=<password> -e "SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0"` | One row with two aggregate values |
| ShelfDB scan | `cargo run --release --bin tpch_scan_bench` | Scan timing from the tracked result file |
| ShelfDB fused scan | `cargo run --release --bin fused_scan_bench` | Fused scan timing from the tracked result file |
| ShelfDB join | `cargo run --release --bin fiber_join_bench --dir=D:\bench\build\tpch-dbgen --snap=D:\bench\data\fiber_snapshot.bin` | Load, query, and total milliseconds |

Do not run `py run_all.py --keep-up` on this laptop. That runner starts seven engines. Run only the MariaDB case in one container. The two `--dir` and `--snap` paths come from `run_cli_manual_suite.py`. Make sure that those paths exist before the run.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/mariadb.json` | MariaDB scan and join times | Read of each `run*` entry |
| `sheaf-db/tests/e2e/realtime_bench/results/duckdb.json`, `sqlite.json`, `pg.json` | Opponent times in the same table | Read of each `run*` entry |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | ShelfDB scan 6.2 ms, fused 1.7 ms, join total 29.0 ms | `py _capture_shelf_bins.py --from-log results/shelf_bins_20261026.log --stamp "2026-10-26 11:44 UTC+05:30" --out shelf_bins_20261026_quiet.json` |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | ShelfDB scan 11.4 ms, fused 2.2 ms, join total 82.5 ms | `py _capture_shelf_bins.py` |
| `results/MANUAL_CLI_EXECUTION_20261027.json` | Native CLI problem workloads | `py run_cli_manual_suite.py` |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, data subset | Written before the pull |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row per query, with times and checksum | Written after three measured passes |

## 9. Acceptance gate

Table 7 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread | Pass only when true | Record `docker inspect` output |
| Row count | 6,001,215 `lineitem` rows at SF1 | Pass only when true | Compare with the generator manifest |
| Scan gate | Gate value PASS in `mariadb.json` | Pass only when true | Do not time a wrong answer |
| Join gate | Top 100 rows and gate value PASS in `mariadb.json` | Pass only when true | Give the checksum in `checksums.txt` |
| ShelfDB reference | `fused_scan_bench` and `fiber_join_bench` report PASS | Pass only when true | Label ShelfDB as the reference row |
| Evidence completeness | Commands, outputs, and checksums are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

## Warnings

- **CAUTION:** The host is an entry-level laptop. Run one database at a time.
- **CAUTION:** Do not mix the 1,279.3 ms and the 52,444 ms MariaDB join numbers. They come from different runs.
- **NOTE:** The three-table join has a description only. The plan does not invent SQL for it.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
