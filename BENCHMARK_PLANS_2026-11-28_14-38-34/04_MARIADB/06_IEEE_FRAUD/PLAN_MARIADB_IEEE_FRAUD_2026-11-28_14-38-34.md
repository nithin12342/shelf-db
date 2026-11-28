# Plan: MariaDB versus ShelfDB on the Institute of Electrical and Electronics Engineers (IEEE) fraud join

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB on the IEEE fraud four-table join. This document is a plan only. It does not pull an image, load a table, or run a query.

A TQL statement is a declarative query over a cellular complex. SQL is the standard query language for relational databases. A gate is an automatic test that compares a result with a truth value.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the benchmark and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| IEEE-CIS fraud data | `https://www.kaggle.com/c/ieee-fraud-detection/data` | Competition CSV files |
| Data set record | `DATASETS.md` | Extracted set with 590,540 rows and 20,663 frauds |
| Bundle README | `attached_bundle_review_2026-11-28_14-19-54/README.md` | Bundle folder `ieee_fraud/` |
| Kdb join script | `bench_kdb_join.py` | Controlled five-row fraud data and join |
| Time-series comparison | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Four-table fraud join record |
| Query text record | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact SQL and TQL text |
| Case suite | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Fraud case names and matched rows |

The exact SQL join uses a controlled five-row data set. That data set holds five accounts, three merchants, two countries, and five payments. Use it for the exactness gate. The IEEE-CIS CSV files are a separate, larger data set. Take the first 100,000 rows of each selected CSV file for the scaling case. Record the subset in the manifest. Do not apply the controlled five-row truth to the IEEE-CIS CSV subset.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which documents name the fraud cases. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | `KDB FOUR-TABLE FRAUD JOIN: SQL` | Four-table join on `payment`, `account`, `merchant`, `country` |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | `KDB FOUR-TABLE FRAUD JOIN: q` | Same join in q |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Case 2 | TQL fraud network two-hop traversal |
| `bench_kdb_join.py` | Lines 7 to 31 | Controlled five-row fraud data |
| `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Fraud join section | Four-table join record |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Section 3 | Fraud cases TC-01 to TC-50 (cases 1 to 50) |

The specific cases are the four-table fraud join, the TQL case 2 shape, and the 50 named cases. The 50 cases have names only. See `04_MARIADB/03_50_FRAUD_CASES/PLAN_MARIADB_50_FRAUD_CASES_2026-11-28_14-38-34.md` for that list. Do not repeat those cases in this timing row.

## 4. Exact query text

The source gives exact SQL for the four-table fraud join. Write it verbatim. Do not change the predicates.

Exact SQL for the four-table fraud join, from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`:

```sql
SELECT p.pay_id, a.name, m.category, c.region, p.amount, p.risk
FROM payment p
JOIN account a ON p.acct_id = a.acct_id
JOIN merchant m ON p.merchant_id = m.merchant_id
JOIN country c ON p.c_code = c.c_code
WHERE p.amount > 2000 AND p.risk = 1;
```

Exact TQL for case 2, from the same file:

```text
MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant, c: Country) WHERE a.balance > 1000 AND t.risk == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, m.category, SECTION_ENERGY };
```

Table 1 of `TQL_APPLICATION_COMPARISON_2026-10-26.md` names Cypher as the native equivalent of TQL case 2. It does not name this SQL join as that equivalent. Do not claim that the two statements are the same query. The TQL case filters on `a.balance > 1000`. The SQL join filters on `p.amount > 2000 AND p.risk = 1`. Read them as two distinct shapes. A TQL optimizer cost is an estimate. Do not state it as an execution time. The TQL-to-fiber execution bridge is not built.

## 5. Container image and install or pull step

Table 3 answers this question: which image and steps the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <subset>:/data:ro mariadb:13.0.2` |
| Controlled data | Create four tables | `account`, `merchant`, `country`, `payment` |
| Controlled data | Insert five rows | Use the values in `bench_kdb_join.py`, lines 7 to 31 |
| IEEE-CIS subset | Extract a subset | Take the first 100,000 rows of each selected CSV file |

Make sure that the column names match the exact SQL. Use `pay_id`, `acct_id`, `merchant_id`, `c_code`, `amount`, and `risk` for `payment`. Record the image digest in `environment.json`.

## 6. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Exactness data rows | 5 | row | Matches the controlled data set |
| Scaling subset | 100,000 | rows per comma-separated value file | Keeps load time within the stop rule |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives a spread for each case |
| Stop rule for a load step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Load controlled data | `mysql --host=127.0.0.1 --user=root --password=<password> <controlled_load.sql>` | Five payment rows and matching dimension rows |
| MariaDB timed join | `mysql --host=127.0.0.1 --user=root --password=<password> -e "<exact SQL from section 4>"` | Two rows from the controlled data |
| Recorded Kdb reference | `py bench_kdb_join.py` | Kdb, SQLite, and DuckDB timings |
| ShelfDB matching command | `unsupported` | Error: no fraud four-table join binary is named in the approved documents or in `sheaf-db/crates/sheaf-engine/src/bin/` |

The recorded script `bench_kdb_join.py` needs Kdb+ and the pykx library. Kdb+ needs a license token. If the token is absent, write `unsupported` with the cause `Kdb+ license token absent`. Do not write a fake Kdb+ time. The ShelfDB row stays `unsupported` until a fraud join binary is named. Do not use `run_cli_manual_suite.py` as a substitute. That runner covers the 50 named cases, not this SQL join.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact SQL and TQL text | Consolidated text file |
| `bench_kdb_join.py` | Controlled five-row data | Lines 7 to 31 |
| `DATASETS.md` | IEEE-CIS row and fraud counts | Data set record |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, data subset | Written before the pull |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row per measured pass, with checksum | Written after three measured passes |
| `results/local_docker_benchmark_<timestamp>/shelfdb/unsupported.txt` | Cause of the ShelfDB gap | Written when the command is not present |

## 9. Acceptance gate

Table 7 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| Controlled row count | Five payment rows are loaded | Pass only when true | Compare with `bench_kdb_join.py` |
| Join result | Two rows, `pay_id` 90001 and 90003 | Pass only when true | Derived from `amount > 2000` and `risk = 1` on the five recorded rows |
| Join checksum | Checksum of the two returned rows | Pass only when true | Write the checksum in `checksums.txt` |
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread | Pass only when true | Record `docker inspect` output |
| Kdb reference | PASS, or `unsupported` with the license cause | Pass only when the status is explicit | Do not write a fake time |
| ShelfDB gap | `unsupported` with the named cause | Pass only when the error is explicit | Do not return a fake result |
| Evidence completeness | Commands, outputs, and checksums are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

The two expected rows come from the recorded data. Payment 90001 has amount 5,000 and risk 1. Payment 90003 has amount 9,999 and risk 1. Payment 90004 has amount 12,000 but risk 0. The other two payments fail the amount predicate or the risk predicate.

## Warnings

- **CAUTION:** The exact SQL join and TQL case 2 are different shapes. Do not compare their times directly.
- **CAUTION:** The controlled five-row truth does not apply to the IEEE-CIS CSV subset.
- **NOTE:** Kdb+ needs a license. A missing token gives `unsupported` with the named cause.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
