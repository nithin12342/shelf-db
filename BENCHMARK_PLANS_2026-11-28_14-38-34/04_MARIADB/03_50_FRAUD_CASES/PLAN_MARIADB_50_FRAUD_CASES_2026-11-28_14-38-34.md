# Plan: MariaDB versus ShelfDB on the 50 high-variance fraud cases

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB across the 50 high-variance fraud cases. This document is a plan only. It does not pull an image, load a table, or run a query.

A TQL statement is a declarative query over a cellular complex. SQL is the standard query language for relational databases. A gate is an automatic test that compares a result with a truth value.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the case suite and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| Case suite record | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | 50 case names, matched rows, and times |
| Runner record | `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | MariaDB 11 median and mean across 50 cases |
| Extracted bundle README | `attached_bundle_review_2026-11-28_14-19-54/README.md` | Bundle folder `ieee_fraud/` |
| Fraud data source | `https://www.kaggle.com/c/ieee-fraud-detection/data` | IEEE-CIS fraud comma-separated value (CSV) files |
| Data set record | `DATASETS.md` | 590,540 rows and 20,663 frauds in the extracted set |
| Local runner | `run_cli_manual_suite.py` | Runner command and output record |

The suite uses fraud data. The case document gives matched row counts. It does not give the row identifiers or the SQL text. Take the first 100,000 rows of each selected CSV file as the loading subset. Record the exact subset in the manifest. Do not claim that the published matched row counts apply to a subset. Compute the truth for the subset before timing MariaDB.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which documents name the 50 cases. Rows are documents. Columns are the document path, the section, and the specific cases.

| Document path | Section | Specific cases |
|---|---|---|
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Section 3 | TC-01 to TC-50 (cases 1 to 50) |
| `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | List | The same 50 case names |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Section 4.1 | MariaDB 11 median 2,037.9 microseconds and mean 4,031.1 microseconds |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Section 4.2 | Manual CLI re-benchmark of the 50 cases |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Section 3, TC-02 | MariaDB is the best competitor at 1,875.3 microseconds |

Table 3 answers this question: which cases the suite names. Rows are cases. Columns are the case identifier, the case name, the matched row count from the source, and the best competitor from the source. ShelfDB is not a row in this table because every case names ShelfDB as the measured system.

| Case identifier | Case name | Matched rows (count) | Best competitor |
|---|---|---:|---|
| TC-01 (case 1) | Strict High-Value Risk Equality | 1 | SQLite |
| TC-02 (case 2) | Single Point Exact Account | 1 | MariaDB |
| TC-03 (case 3) | Ultra-High Outlier Amount | 0 | SQLite |
| TC-04 (case 4) | Zero-Match Null Filtering | 0 | SQLite |
| TC-05 (case 5) | Compound Balance Disjunction | 3 | SQLite |
| TC-06 (case 6) | Rooted Merchant Attack Pattern | 0 | SQLite |
| TC-07 (case 7) | Regional European Filtering | 2 | SQLite |
| TC-08 (case 8) | Category String Exact Match | 2 | Redis |
| TC-09 (case 9) | Negative Risk Exclusion | 1 | SQLite |
| TC-10 (case 10) | Multi-Variable Range & Inequality | 2 | SQLite |
| TC-11 (case 11) | Regularization Lambda Scaling 0.0001 | 2 | SQLite |
| TC-12 (case 12) | Regularization Lambda Scaling 0.0005 | 2 | SQLite |
| TC-13 (case 13) | Regularization Lambda Scaling 0.001 | 2 | SQLite |
| TC-14 (case 14) | Regularization Lambda Scaling 0.005 | 2 | SQLite |
| TC-15 (case 15) | Regularization Lambda Scaling 0.02 | 2 | SQLite |
| TC-16 (case 16) | Regularization Lambda Scaling 0.05 | 2 | SQLite |
| TC-17 (case 17) | Regularization Lambda Scaling 0.1 | 2 | SQLite |
| TC-18 (case 18) | Regularization Lambda Scaling 0.25 | 2 | SQLite |
| TC-19 (case 19) | Regularization Lambda Scaling 0.5 | 2 | SQLite |
| TC-20 (case 20) | Regularization Lambda Scaling 1.0 | 2 | SQLite |
| TC-21 (case 21) | Iteration Bound Scaling 1 Iter | 3 | SQLite |
| TC-22 (case 22) | Iteration Bound Scaling 2 Iters | 3 | SQLite |
| TC-23 (case 23) | Iteration Bound Scaling 3 Iters | 3 | SQLite |
| TC-24 (case 24) | Iteration Bound Scaling 5 Iters | 3 | SQLite |
| TC-25 (case 25) | Iteration Bound Scaling 8 Iters | 3 | SQLite |
| TC-26 (case 26) | Iteration Bound Scaling 12 Iters | 3 | SQLite |
| TC-27 (case 27) | Iteration Bound Scaling 18 Iters | 3 | SQLite |
| TC-28 (case 28) | Iteration Bound Scaling 35 Iters | 3 | SQLite |
| TC-29 (case 29) | Iteration Bound Scaling 50 Iters | 3 | SQLite |
| TC-30 (case 30) | Iteration Bound Scaling 100 Iters | 3 | SQLite |
| TC-31 (case 31) | Energy Threshold Certificate 1e-06 | 3 | SQLite |
| TC-32 (case 32) | Energy Threshold Certificate 5e-06 | 3 | SQLite |
| TC-33 (case 33) | Energy Threshold Certificate 1e-05 | 3 | SQLite |
| TC-34 (case 34) | Energy Threshold Certificate 5e-05 | 3 | SQLite |
| TC-35 (case 35) | Energy Threshold Certificate 0.0001 | 3 | SQLite |
| TC-36 (case 36) | Energy Threshold Certificate 0.0005 | 3 | SQLite |
| TC-37 (case 37) | Energy Threshold Certificate 0.001 | 3 | SQLite |
| TC-38 (case 38) | Energy Threshold Certificate 0.005 | 3 | SQLite |
| TC-39 (case 39) | Energy Threshold Certificate 0.01 | 3 | SQLite |
| TC-40 (case 40) | Energy Threshold Certificate 0.05 | 3 | SQLite |
| TC-41 (case 41) | Discrete Only Projection | 2 | SQLite |
| TC-42 (case 42) | Spectral Certification Only | 3 | SQLite |
| TC-43 (case 43) | Cross-Border APAC Region Flow | 0 | SQLite |
| TC-44 (case 44) | Low Balance High Transaction Anomaly | 1 | SQLite |
| TC-45 (case 45) | High Balance Wealth Verification | 1 | Redis |
| TC-46 (case 46) | Gaming Merchant Micro-Payments | 0 | SQLite |
| TC-47 (case 47) | Luxury Low-Risk Channel | 0 | SQLite |
| TC-48 (case 48) | All European Rooted Channels | 2 | SQLite |
| TC-49 (case 49) | Maximum Solver Tightness Stress | 2 | SQLite |
| TC-50 (case 50) | Universal Boundary Full Scan | 5 | SQLite |

## 4. Exact query text

The source gives case names and matched row counts. It gives no SQL text for the 50 cases. The consolidated query file records the same gap. Do not invent SQL for these cases.

Names only apply to every row in Table 3. The cases with a MariaDB row are TC-02 (case 2, Single Point Exact Account), with 1,875.3 microseconds, and the whole-suite MariaDB 11 summary, with 2,037.9 microseconds median and 4,031.1 microseconds mean.

The run team must obtain the 50 case definitions from the recorded suite before timing MariaDB. If the definitions stay absent, write `unsupported` with the cause `no SQL text for the 50 high-variance cases`. Do not write a fake result.

## 5. Container image and install or pull step

Table 4 answers this question: which image and steps the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <subset>:/data:ro mariadb:13.0.2` |
| Fraud data | Extract a subset | Take the first 100,000 rows of each selected IEEE-CIS CSV file |
| Case definitions | Obtain the recorded definitions | Read `run_cli_manual_suite.py` and the case names in Table 3 |

The case document records MariaDB 13.0.2 for TC-02. The manual CLI document records MariaDB 11 for the whole-suite summary. Use one version for one timing row and name it in `environment.json`. Record the image digest.

## 6. Low-power limits

Table 5 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives a spread for each case |
| Stop rule for a load step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 6 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Load data | `mysql --local-infile=1 --host=127.0.0.1 --user=root --password=<password> <load_script.sql>` | Stated row count for each loaded table |
| MariaDB timed case | `mysql --host=127.0.0.1 --user=root --password=<password> -e "<case SQL from the recorded suite>"` | One row per case, with matched rows and time |
| Recorded suite runner | `py run_cli_manual_suite.py` | `results/MANUAL_CLI_EXECUTION_20261027.json` |
| ShelfDB native CLI | `sheaf_db_app.exe` from the release directory | One result per case |

Do not run the whole recorded suite while a container runs. Run the MariaDB cases in one container. Run the ShelfDB cases after the container stops. Make sure that both sides use the same subset and the same truth value.

## 8. Evidence files

Table 7 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Case names, matched rows, and MariaDB row for TC-02 | Section 3 table |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | MariaDB 11 summary across 50 cases | Section 4.1 table |
| `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | The 50 case names | List |
| `results/MANUAL_CLI_EXECUTION_20261027.json` | Native CLI output record | `py run_cli_manual_suite.py` |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, data subset | Written before the pull |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row per case, with time and matched rows | Written after three measured passes |
| `results/local_docker_benchmark_<timestamp>/summary.md` | Remote, branch, and commit | Written after the run |

## 9. Acceptance gate

Table 8 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| Case definitions | All 50 case definitions are present | Pass only when true | Otherwise write `unsupported` with the named cause |
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread | Pass only when true | Record `docker inspect` output |
| Same subset | MariaDB and ShelfDB use the same 100,000-row subset | Pass only when true | Give the subset digest in `manifest.json` |
| Row match | Each case returns the stated matched row count for the subset | Pass only when true | Full-set counts do not apply to a subset |
| MariaDB reference | TC-02 gate value PASS at 1,875.3 microseconds only for the full case data | Pass only when true | Do not apply a full-set time to a subset |
| Evidence completeness | Commands, outputs, and checksums are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

## Warnings

- **CAUTION:** The source gives names, not SQL. Do not invent SQL for the 50 cases.
- **CAUTION:** The 1,875.3 microsecond value belongs to the recorded full case data. Do not quote it for a 100,000-row subset.
- **NOTE:** TC-02 is the only case where MariaDB is the best competitor.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
