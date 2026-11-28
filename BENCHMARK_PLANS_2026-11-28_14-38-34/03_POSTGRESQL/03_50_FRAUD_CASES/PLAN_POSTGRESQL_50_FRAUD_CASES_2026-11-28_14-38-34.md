# PostgreSQL versus ShelfDB: 50 fraud cases plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan names the 50 high-variance fraud cases for a PostgreSQL versus ShelfDB comparison. The source gives case names and measured times. The source gives no SQL text for these cases. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.
- **Unsupported:** a declared result that names a missing input. An unsupported case does not produce a time.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://www.kaggle.com/competitions/ieee-fraud-detection/data` |
| Data set document | `DATASETS.md`, rows 1 and 1b |
| Test case document | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 3 |
| Case name document | `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` |
| Original runner | `run_cli_manual_suite.py` |
| Evidence record | `results/MANUAL_CLI_EXECUTION_20261027.json` |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Source archive | `D:\d2\data\ieee-fraud-detection.zip` | path |
| Extracted rows in the source | 590,540 | rows |
| Fraud rows in the source | 20,663 | rows |
| Low-power subset | First 100,000 rows of each comma-separated file | rows per file |
| Extracted path | `D:\bench\datasets\ieee-fraud\` | path |
| Subset rule | Use the first 100,000 rows first. Make sure that the fraud label count is recorded. | rule |

## 5. Test cases and exact query text

Table 3 answers this question: which cases run. Rows are cases. Columns are the case identifier and the case name. No case has SQL text in the source.

| Case identifier | Case name |
|---|---|
| Test case 1 (TC-01) | Strict High-Value Risk Equality |
| Test case 2 (TC-02) | Single Point Exact Account |
| Test case 3 (TC-03) | Ultra-High Outlier Amount |
| Test case 4 (TC-04) | Zero-Match Null Filtering |
| Test case 5 (TC-05) | Compound Balance Disjunction |
| Test case 6 (TC-06) | Rooted Merchant Attack Pattern |
| Test case 7 (TC-07) | Regional European Filtering |
| Test case 8 (TC-08) | Category String Exact Match |
| Test case 9 (TC-09) | Negative Risk Exclusion |
| Test case 10 (TC-10) | Multi-Variable Range and Inequality |
| Test case 11 (TC-11) | Regularization Lambda Scaling 0.0001 |
| Test case 12 (TC-12) | Regularization Lambda Scaling 0.0005 |
| Test case 13 (TC-13) | Regularization Lambda Scaling 0.001 |
| Test case 14 (TC-14) | Regularization Lambda Scaling 0.005 |
| Test case 15 (TC-15) | Regularization Lambda Scaling 0.02 |
| Test case 16 (TC-16) | Regularization Lambda Scaling 0.05 |
| Test case 17 (TC-17) | Regularization Lambda Scaling 0.1 |
| Test case 18 (TC-18) | Regularization Lambda Scaling 0.25 |
| Test case 19 (TC-19) | Regularization Lambda Scaling 0.5 |
| Test case 20 (TC-20) | Regularization Lambda Scaling 1.0 |
| Test case 21 (TC-21) | Iteration Bound Scaling 1 Iteration |
| Test case 22 (TC-22) | Iteration Bound Scaling 2 Iterations |
| Test case 23 (TC-23) | Iteration Bound Scaling 3 Iterations |
| Test case 24 (TC-24) | Iteration Bound Scaling 5 Iterations |
| Test case 25 (TC-25) | Iteration Bound Scaling 8 Iterations |
| Test case 26 (TC-26) | Iteration Bound Scaling 12 Iterations |
| Test case 27 (TC-27) | Iteration Bound Scaling 18 Iterations |
| Test case 28 (TC-28) | Iteration Bound Scaling 35 Iterations |
| Test case 29 (TC-29) | Iteration Bound Scaling 50 Iterations |
| Test case 30 (TC-30) | Iteration Bound Scaling 100 Iterations |
| Test case 31 (TC-31) | Energy Threshold Certificate 1e-06 |
| Test case 32 (TC-32) | Energy Threshold Certificate 5e-06 |
| Test case 33 (TC-33) | Energy Threshold Certificate 1e-05 |
| Test case 34 (TC-34) | Energy Threshold Certificate 5e-05 |
| Test case 35 (TC-35) | Energy Threshold Certificate 0.0001 |
| Test case 36 (TC-36) | Energy Threshold Certificate 0.0005 |
| Test case 37 (TC-37) | Energy Threshold Certificate 0.001 |
| Test case 38 (TC-38) | Energy Threshold Certificate 0.005 |
| Test case 39 (TC-39) | Energy Threshold Certificate 0.01 |
| Test case 40 (TC-40) | Energy Threshold Certificate 0.05 |
| Test case 41 (TC-41) | Discrete Only Projection |
| Test case 42 (TC-42) | Spectral Certification Only |
| Test case 43 (TC-43) | Cross-Border Asia Pacific Region Flow |
| Test case 44 (TC-44) | Low Balance High Transaction Anomaly |
| Test case 45 (TC-45) | High Balance Wealth Verification |
| Test case 46 (TC-46) | Gaming Merchant Micro-Payments |
| Test case 47 (TC-47) | Luxury Low-Risk Channel |
| Test case 48 (TC-48) | All European Rooted Channels |
| Test case 49 (TC-49) | Maximum Solver Tightness Stress |
| Test case 50 (TC-50) | Universal Boundary Full Scan |

The source document gives matched row counts and measured times for each case. The source names are the exact case names. The expanded names in Table 3 keep the abbreviations readable. The source gives no SQL text and no TQL text for the 50 cases. Do not invent SQL. A case that has no supplied text must report `unsupported` with the cause.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the subset directory before the run:

```sh
docker run --rm --name postgres_fraud50 --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\datasets\ieee-fraud:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run after the missing query text is supplied. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Original runner | `py run_cli_manual_suite.py` | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md`, section 1 |
| Load the subset | `psql -h 127.0.0.1 -U bench -d bench -f /data/load_subset.sql` | This plan |
| Measure one named case | `psql -h 127.0.0.1 -U bench -d bench -f /data/<case name>.sql` | This plan after text is supplied |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_fraud sql "<supplied SQL>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB fraud application binary | `cargo run -p sheaf-engine --bin sheaf_db_app --release` | `sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs` |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_50_fraud_cases/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Row count and fraud label count | Record the subset |
| `load.txt` | Load commands and output | Record the load time |
| `result.csv` | One row per case with time in microseconds and matched rows | Compare the engines |
| `checksums.txt` | Matched rows for each case | Run the gate |
| `unsupported.csv` | Case identifier and cause for each missing text | Record declared unsupported cases |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Loaded rows match the manifest | PASS | Not run |
| All 50 case names appear in `result.csv` or `unsupported.csv` | PASS | Not run |
| Each case with supplied text has three measured passes | PASS | Not run |
| PostgreSQL and ShelfDB matched row counts agree for each case | PASS | Not run |
| Each case with no text reports `unsupported` and a cause | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |

## Warnings

- **CAUTION:** A case with no SQL text is not complete. Do not record a time for it.
- **CAUTION:** The named cases include solver parameters. The PostgreSQL side cannot run those parameters without a declared mapping from the supplied text.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
