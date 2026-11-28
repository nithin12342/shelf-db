# PostgreSQL versus ShelfDB: Institute of Electrical and Electronics Engineers fraud data plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs a four-table fraud join on PostgreSQL and ShelfDB. The source gives exact SQL for a controlled five-row fraud data set. The source gives no SQL for the large fraud file. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.
- **Unsupported:** a declared result that names a missing input. An unsupported case does not produce a time.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://www.kaggle.com/c/ieee-fraud-detection/data` |
| Data set document | `DATASETS.md`, rows 1 and 1b |
| Test case document | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, controlled five-row join table |
| Exact query document | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, lines 70 to 79 |
| Original runner | `bench_kdb_join.py` |
| Runner document | `recursive test/QUERIES_2026-11-22_15-04-04.md`, section 2 |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Large source rows in the source document | 590,540 | rows |
| Large source fraud labels | 20,663 | rows |
| Low-power large subset | First 100,000 rows of each comma-separated file | rows per file |
| Controlled data set | Five rows in payment, five rows in account, three rows in merchant, two rows in country | rows |
| Controlled data source | `bench_kdb_join.py` | path |
| Extracted large data path | `D:\bench\datasets\ieee-fraud\` | path |
| Subset rule | Run the controlled five-row case first. Run the large subset only after the laptop stays responsive. | rule |

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case name, the text status, and the source.

| Case name | Text status | Source |
|---|---|---|
| Controlled five-row relational join | Exact SQL in the source | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` |
| Large fraud file slice | Names only. No exact SQL in the source. | `DATASETS.md` |
| 50 high-variance fraud cases | Names only. No SQL in the source. | `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` |

The source gives this exact SQL text for the controlled five-row case:

```sql
SELECT p.pay_id, a.name, m.category, c.region, p.amount, p.risk
FROM payment p
JOIN account a ON p.acct_id = a.acct_id
JOIN merchant m ON p.merchant_id = m.merchant_id
JOIN country c ON p.c_code = c.c_code
WHERE p.amount > 2000 AND p.risk = 1;
```

The matching query language text for Kdb+ is:

```text
select from ((payments lj accounts) lj merchants) lj countries where amount > 2000, risk = 1
```

The controlled five-row case is not the large fraud file. The exact SQL above does not apply to the large file. Do not invent SQL for the large file or for the 50 named cases.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the controlled data directory before the run:

```sh
docker run --rm --name postgres_ieee_fraud --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\datasets\ieee-fraud:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Large data rows per file | 100,000 | Keeps the load small |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Original runner | `py bench_kdb_join.py` | `bench_kdb_join.py` |
| Load the controlled five-row case | `psql -h 127.0.0.1 -U bench -d bench -f /data/controlled_five_row.sql` | This plan |
| Measure the exact controlled case | `psql -h 127.0.0.1 -U bench -d bench -f /data/join.sql` | This plan |
| Load the large subset | `psql -h 127.0.0.1 -U bench -d bench -f /data/load_subset.sql` | This plan |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_fraud sql "<exact SQL from section 5>"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB fraud application binary | `cargo run -p sheaf-engine --bin sheaf_db_app --release` | `sheaf-db/crates/sheaf-engine/src/bin/sheaf_db_app.rs` |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_ieee_fraud/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Row counts for the controlled case and the large subset | Record the data |
| `load.txt` | Load commands and output | Record the load time |
| `result.csv` | One row per pass with time in microseconds and returned rows | Compare the engines |
| `checksums.txt` | Returned rows and values for the controlled case | Run the gate |
| `unsupported.csv` | Large file case and cause | Record the declared unsupported case |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Controlled data row counts match the source | PASS | Not run |
| Three measured passes complete for the exact controlled SQL | PASS | Not run |
| PostgreSQL and ShelfDB return the same rows and values | PASS | Not run |
| The large file case reports `unsupported` and a cause | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |

## Warnings

- **CAUTION:** The controlled five-row case uses four tables with matched keys. The large fraud file uses a different column layout. Do not carry the SQL across without a supplied mapping.
- **CAUTION:** The recorded Kdb+ result uses 1000 internal iterations. Use one thread for this comparison.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
