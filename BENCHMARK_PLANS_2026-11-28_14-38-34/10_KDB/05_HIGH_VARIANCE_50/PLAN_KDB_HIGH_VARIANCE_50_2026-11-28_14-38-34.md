# PLAN_KDB_HIGH_VARIANCE_50_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan pairs Kdb+ with ShelfDB on the 50 high-variance fraud cases. The plan names the source, the data, the cases, the commands, and the gate. The plan does not run the benchmark. The approved documents give no Kdb+ runner for these 50 cases. This plan records that gap.

## 2. Concept definitions

- **Kdb+:** a column-oriented time-series database from KX Systems.
- **PyKX:** the official Python integration library for Kdb+ and q.
- **q:** the vector and array query language of Kdb+.
- **TQL:** Topological Query Language, a query over a cellular complex.
- **SQL:** Structured Query Language, the standard language for relational databases.
- **Fraud case:** one named high-variance query over the IEEE-CIS fraud data.
- **Gate:** an automatic comparison of a result with a truth value.

## 3. Original benchmark link and source files

Table 1 answers this question: which link and sources define this pair. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Original data link | `https://www.kaggle.com/c/ieee-fraud-detection/data` | `DATASETS.md`, row 1 |
| Original runner command | `py run_cli_manual_suite.py` | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 1 |
| Raw evidence | `results/MANUAL_CLI_EXECUTION_20261027.json` | Same section |
| Suite medians | `results/HIGH_VARIANCE_50_RESULTS.json` | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, Table 6B |
| Case names | `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | `recursive test/README_2026-11-22_15-04-04.md` |
| Query sources | `recursive test/QUERIES_2026-11-22_15-04-04.md` | Same README |
| Kdb+ runner | Absent in the repository | No approved document names one |

The original runner measured ShelfDB, SQLite, PostgreSQL, DuckDB, MariaDB, and Redis. It did not measure Kdb+. Do not claim a Kdb+ result from that runner.

## 4. Dataset and subset

Table 2 answers this question: which data and subset this pair uses. Rows are facts. Columns are the fact, the value, and the source.

| Fact | Value | Source |
|---|---|---|
| Data set name | IEEE-CIS fraud detection | `DATASETS.md`, row 1 |
| Archive size (GB) | 0.124 | Same row |
| Extracted size (GB) | 1.355 | `DATASETS.md`, row 1b |
| Extracted rows (count) | 590,540 | Same row |
| Fraud rows (count) | 20,663 | Same row |
| Archive path | `D:\d2\data\ieee-fraud-detection.zip` | `DATASETS.md`, row 1 |
| Extracted path | `D:\bench\datasets\ieee-fraud\` | `DATASETS.md`, row 1b |
| Log-in gate | Kaggle account required for the archive | `DATASETS.md`, row 1 |
| Low-power subset | First 100,000 rows of the IEEE-CIS CSV files | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4, item 6 |

Use the 100,000-row subset. Do not load all 590,540 rows on the low-power laptop.

## 5. Test-case document and specific test cases

The exact test-case document path is `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`. Section 3 lists all 50 cases. The consolidated name list is in `recursive test/CASE_NAMES_2026-11-22_15-04-04.md`.

Table 3 answers this question: which documents name the cases. Rows are documents. Columns are the path, the specific cases, and the text type.

| Document path | Specific cases | Text type |
|---|---|---|
| `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | Test case TC-01 to test case TC-50 | Names only |
| `recursive test/QUERIES_2026-11-22_15-04-04.md` | 50 high-variance cases | Names only |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Section 3, all 50 cases | Names, matched rows, timings |
| `MANUAL_CLI_BENCHMARK_AND_FIVE_FAILURES_2026-10-27.md` | Section 4.2, all 50 cases | Names, matched rows, timings |
| `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Table 6B, suite medians | Timings only, no Kdb+ row |

Table 4 answers this question: which 50 cases the pair runs. Rows are cases. Columns are the test case identifier and the case name. The names come from the consolidated list.

| Test case identifier | Case name |
|---|---|
| Test case TC-01 | Strict High-Value Risk Equality |
| Test case TC-02 | Single Point Exact Account |
| Test case TC-03 | Ultra-High Outlier Amount |
| Test case TC-04 | Zero-Match Null Filtering |
| Test case TC-05 | Compound Balance Disjunction |
| Test case TC-06 | Rooted Merchant Attack Pattern |
| Test case TC-07 | Regional European Filtering |
| Test case TC-08 | Category String Exact Match |
| Test case TC-09 | Negative Risk Exclusion |
| Test case TC-10 | Multi-Variable Range & Inequality |
| Test case TC-11 | Regularization Lambda Scaling 0.0001 |
| Test case TC-12 | Regularization Lambda Scaling 0.0005 |
| Test case TC-13 | Regularization Lambda Scaling 0.001 |
| Test case TC-14 | Regularization Lambda Scaling 0.005 |
| Test case TC-15 | Regularization Lambda Scaling 0.02 |
| Test case TC-16 | Regularization Lambda Scaling 0.05 |
| Test case TC-17 | Regularization Lambda Scaling 0.1 |
| Test case TC-18 | Regularization Lambda Scaling 0.25 |
| Test case TC-19 | Regularization Lambda Scaling 0.5 |
| Test case TC-20 | Regularization Lambda Scaling 1.0 |
| Test case TC-21 | Iteration Bound Scaling 1 Iter |
| Test case TC-22 | Iteration Bound Scaling 2 Iters |
| Test case TC-23 | Iteration Bound Scaling 3 Iters |
| Test case TC-24 | Iteration Bound Scaling 5 Iters |
| Test case TC-25 | Iteration Bound Scaling 8 Iters |
| Test case TC-26 | Iteration Bound Scaling 12 Iters |
| Test case TC-27 | Iteration Bound Scaling 18 Iters |
| Test case TC-28 | Iteration Bound Scaling 35 Iters |
| Test case TC-29 | Iteration Bound Scaling 50 Iters |
| Test case TC-30 | Iteration Bound Scaling 100 Iters |
| Test case TC-31 | Energy Threshold Certificate 1e-06 |
| Test case TC-32 | Energy Threshold Certificate 5e-06 |
| Test case TC-33 | Energy Threshold Certificate 1e-05 |
| Test case TC-34 | Energy Threshold Certificate 5e-05 |
| Test case TC-35 | Energy Threshold Certificate 0.0001 |
| Test case TC-36 | Energy Threshold Certificate 0.0005 |
| Test case TC-37 | Energy Threshold Certificate 0.001 |
| Test case TC-38 | Energy Threshold Certificate 0.005 |
| Test case TC-39 | Energy Threshold Certificate 0.01 |
| Test case TC-40 | Energy Threshold Certificate 0.05 |
| Test case TC-41 | Discrete Only Projection |
| Test case TC-42 | Spectral Certification Only |
| Test case TC-43 | Cross-Border APAC Region Flow |
| Test case TC-44 | Low Balance High Transaction Anomaly |
| Test case TC-45 | High Balance Wealth Verification |
| Test case TC-46 | Gaming Merchant Micro-Payments |
| Test case TC-47 | Luxury Low-Risk Channel |
| Test case TC-48 | All European Rooted Channels |
| Test case TC-49 | Maximum Solver Tightness Stress |
| Test case TC-50 | Universal Boundary Full Scan |

## 6. Exact TQL, q, and SQL text

Table 5 answers this question: which language text the approved sources give for these 50 cases. Rows are languages. Columns are the language, the text status, and the source path.

| Language | Text status | Source path |
|---|---|---|
| TQL | No exact text for the 50 cases | `recursive test/CASE_NAMES_2026-11-22_15-04-04.md`, names only |
| q | No exact text for the 50 cases | `recursive test/QUERIES_2026-11-22_15-04-04.md`, names only |
| SQL | No exact text for the 50 cases | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 3 |

Do not invent TQL, q, or SQL text for test case TC-01 to test case TC-50. A name is not a query.

A separate case has exact text. The controlled five-row fraud join is not one of the 50 cases. Its q and SQL text live in `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, block "KDB FOUR-TABLE FRAUD JOIN". That case belongs to `PLAN_KDB_FOUR_TABLE_FRAUD_JOIN_2026-11-28_14-38-34.md`. Do not mix the two data sets.

## 7. License gate and install step

Table 6 answers this question: which gate and install step this pair uses. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Container image | None. Kdb+ runs as an embedded engine in the Python process. | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 3 |
| License gate | KDB-X Community Edition token. Write `unsupported` when the token is absent. | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 6 |
| Install step | `pip install pykx`. This command is derived from the recorded PyKX 4.2.0 install. | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, engineering record |
| License placement | `pykx/lib/kc.lic` and `~/.kx/kc.lic` | Same document, section 3 |
| Gate output | `Licensed: True`, q version 5f | Same document, section 3 |
| Image pull | Not applicable. No image exists in the approved sources. | Same document |

Do not place the token in Git. Write the gate result into `license_gate.txt`. If the gate fails, write `unsupported` with the cause and stop this plan.

## 8. Low-power limits

Table 7 answers this question: which limits govern this pair. Rows are limits. Columns are the limit, the value, and the source.

| Limit | Value | Source |
|---|---|---|
| Concurrent database containers (count) | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container central processing units (count) | 1 | Same section |
| Container memory (GB) | 2 | Same section |
| Benchmark threads (count) | 1 | Same section |
| Process identifier limit (count) | 128 | Same document, section 5 |
| Long step stop (minutes) | 30 | Same document, section 2 |
| Cooldown between engines (minutes) | 3 | Same section |
| Data extraction | Selected files only | Same section |

Kdb+ uses one Python process. An isolation container keeps these limits. Run one engine at a time. Start a container with `--cpus=1 --memory=2g --pids-limit=128`. Mount the data directory read-only.

## 9. Original benchmark command

The runner command is verbatim from `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 1.

```bash
py run_cli_manual_suite.py
```

The runner writes `results/MANUAL_CLI_EXECUTION_20261027.json`. It records one row per case. The Kdb+ side has no committed runner. Write `unsupported` with the cause `NO_KDB_50_CASE_RUNNER`. Do not substitute a Kdb+ result from the time-series script or the five-row join script.

## 10. Matching ShelfDB command

Run ShelfDB as a local process. Use one build job. Run one case at a time with the case name beside the timing.

```bash
py run_cli_manual_suite.py
```

```bash
sheaf-db/target/release/sheaf_db_app.exe --data-file <fraud-subset-file> "<exact TQL text for the selected case>"
```

The approved sources give no TQL text for the 50 cases. Keep the second command as a shape until a runner and case text are committed. Record the exact invocation in `shelfdb_output.txt`.

## 11. Evidence files

Write evidence under `results/local_docker_benchmark_<run-timestamp>/kdb_high_variance_50/`.

Table 8 answers this question: which files hold the evidence. Rows are file names. Columns are the file name and the content.

| File name | Content |
|---|---|
| `environment.json` | Host, Python version, PyKX version, data subset |
| `license_gate.txt` | Gate result, q version, install step, `unsupported` cause when present |
| `subset_manifest.json` | Row count, checksum, first 100,000 rows |
| `kdb_output.txt` | `unsupported` cause or Kdb+ runner output with one row per case |
| `shelfdb_output.txt` | ShelfDB command, case name, output, timing |
| `result.csv` | One row per case with matched rows, time, and unit |
| `checksums.txt` | Checksums of result rows per case |
| `summary.md` | Remote, branch, base commit, data set, runner gap, gate result |

## 12. Acceptance gate

A pair is complete only when all of the following hold:

1. The Kdb+ gate returned `Licensed: True` first. Otherwise the plan returned `unsupported` with the named cause.
2. One engine ran at a time. One container, one central processing unit, 2 GB memory, and one thread applied.
3. The subset held 100,000 rows and matched `subset_manifest.json`.
4. The evidence names all 50 cases. It does not attach a case name to another case timing.
5. Each case matched the row count in `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 3.
6. Three measured passes ran with one thread after one untimed warm-up.
7. No step ran longer than 30 minutes.
8. No TQL, q, or SQL text was invented. The runner gap is named when no runner exists.
9. No five-row join timing entered this 50-case record.
10. The summary names the remote, branch, base commit, data set, and runner gap.
11. No token or secret entered Git.

## Warnings

- **CAUTION:** The original 50-case run did not measure Kdb+. Do not claim that it did.
- **CAUTION:** The controlled five-row join is a different data set. Do not mix its timings with these cases.
- **NOTE:** The 50-case documents give names and timings. They give no query text.
- **NOTE:** A missing Kdb+ runner gives `unsupported`, not a fake result.
- **NOTE:** This is a plan only. No license was activated and no benchmark was run.
