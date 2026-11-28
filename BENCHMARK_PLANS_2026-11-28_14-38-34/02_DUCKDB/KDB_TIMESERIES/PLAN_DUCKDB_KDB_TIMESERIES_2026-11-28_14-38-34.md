# DuckDB versus ShelfDB benchmark plan: Kdb time-series shapes

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs DuckDB with ShelfDB on the three recorded Kdb time-series shapes and the full column average. DuckDB runs the recorded Structured Query Language (SQL) text. ShelfDB runs the recorded slice path. This pair does not run Kdb. This plan does not install software and does not run a benchmark.

## 2. Concept definitions

- **DuckDB:** an embedded columnar database that runs SQL in one process.
- **ShelfDB:** a sheaf database that stores answers in binary cellular storage sections.
- **Time-series:** an ordered sequence of data points indexed by time.
- **SQL:** the standard language for relational queries.
- **Gate:** an automatic comparison of an engine result with a truth value.

## 3. Pair identity

Table 1 answers this question: what identifies this pair? Rows are attributes. Columns are the attribute, the recorded value, and the source.

| Attribute | Recorded value | Source |
|---|---|---|
| Pair | DuckDB versus ShelfDB | This plan |
| Benchmark family | Time-series benchmark suite (TSBS) | Master index, Table 3 |
| Original link | `https://github.com/timescale/tsbs` | Master index, Table 3 |
| Comparison document | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Section 5 |
| Row count | 100,000 tick rows | Section 5 of that document |

## 4. Dataset and subset

Table 2 answers this question: which data does this pair use? Rows are data attributes. Columns are the attribute, the value, and the preparation step.

| Attribute | Value | Preparation step |
|---|---|---|
| Data set | 100,000 generated tick rows | Generate the rows in the DuckDB session. |
| Columns | `sym VARCHAR`, `ts TIMESTAMP`, `price DOUBLE`, `size INT` | Read `bench_timeseries_duckdb.py`, lines 5 to 12. |
| Generation text | `TIMESTAMP '2026-10-27 09:00:00' + (i * INTERVAL 1 SECOND)` | Read `bench_timeseries_duckdb.py`, lines 14 to 22. |
| Low-power rule | 100,000 rows, not 10 million rows | Read `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4.8. |
| TSBS tool | Go programs for data and query generation | Read extracted TSBS README, lines 103 to 113. |

## 5. Test-case document and specific cases

Table 3 answers this question: which recorded cases does this pair cover? Rows are cases. Columns are the case, the document section, and the recorded DuckDB result in microseconds.

| Case | Document section | DuckDB result (us) |
|---|---|---|
| One-minute window aggregation | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 4.1 and section 5 | 32,375.2 |
| One-hour range slice | Same document, section 4.2 and section 5 | 2,591.0 |
| Exact timestamp point lookup | Same document, section 4.3 and section 5 | 1,585.9 |
| Full column average | Same document, section 5 | 1,532.2 |
| Controlled five-row fraud join | Same document, section 6.1 | 1,297.3 |

The fraud join row is a separate relational case. Keep it in a separate table from the time-series rows. The raw evidence file is `results/KDB_TIMESERIES_RESULTS.json`. The join evidence file is `results/KDB_JOIN_RESULTS.json`.

## 6. Exact query text

The SQL text is exact. It comes from `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, sections 4.1 to 4.3, and from `bench_kdb_timeseries.py`, lines 89, 96, 103, and 110.

```sql
SELECT time_bucket(INTERVAL '1 minute', ts) AS b, AVG(price), SUM(size)
FROM quotes GROUP BY 1;
```

```sql
SELECT AVG(price), SUM(size) FROM quotes
WHERE ts BETWEEN TIMESTAMP '2026-10-27 09:15:00' AND TIMESTAMP '2026-10-27 10:15:00';
```

```sql
SELECT price, size FROM quotes WHERE ts = TIMESTAMP '2026-10-27 09:42:15';
```

```sql
SELECT AVG(price) FROM quotes;
```

Use these four texts. Report the client timer scope with each number. Do not compare an internal timer against a client timer.

## 7. Container image and install step

Table 4 answers this question: which image and install step provide DuckDB? Rows are items. Columns are the item, the value, and the reason.

| Item | Value | Reason |
|---|---|---|
| Image | `python:3.12-slim` | DuckDB has no server image. It runs in one process. |
| Pull step | `docker pull python:3.12-slim` | Pull one image at a time. |
| Install step | `pip install duckdb` | Install the embedded library. |
| Version record | Record `duckdb.__version__` in `environment.json` | The documents name 1.5.4. |
| License gate | Kdb needs a token | This pair does not run Kdb. Do not install PyKX. |

## 8. Low-power limits

Table 5 answers this question: which limits govern the local run? Rows are limits. Columns are the limit, the value, and the command or action.

| Limit | Value | Command or action |
|---|---|---|
| Concurrent containers | 1 | Stop all other containers first. |
| Central processing unit | 1 | Use `--cpus=1`. |
| Memory | 2 gigabytes | Use `--memory=2g`. |
| Threads | 1 | Use `SET threads=1;` in the DuckDB session. |
| Long step | 30 minutes maximum | Stop the step at the limit. |
| Cooldown | 3 minutes | Wait before the next pair. |

## 9. Commands

Table 6 answers this question: which commands produce the two sides of the pair? Rows are sides. Columns are the side, the command, and the expected output.

| Side | Command | Expected output |
|---|---|---|
| Original DuckDB harness | `py bench_timeseries_duckdb.py` | Median slice and point lookup times |
| Original combined harness | `py bench_kdb_timeseries.py` | `results/KDB_TIMESERIES_RESULTS.json`, needs a Kdb token |
| Matching ShelfDB binary | `cargo run --release -p sheaf-engine --bin range_slice_bench` | Range slice time over 10,000 rows |

The `range_slice_bench` binary measures a 10,000-row range slice. It does not run the same 100,000-row tick query. Report that difference with the ratio. Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture time, row count, and result checksum for each pass.

## 10. Evidence files

Table 7 answers this question: which files hold the evidence? Rows are files. Columns are the file, the claim, and the action.

| File | Claim | Action |
|---|---|---|
| `results/KDB_TIMESERIES_RESULTS.json` | Recorded DuckDB and Kdb times | Read the `duckdb_us` entry. |
| `bench_timeseries_duckdb.py` | DuckDB query text and generation text | Read lines 5 to 49. |
| `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 5 | Recorded comparison table | Read the DuckDB row. |
| `results/local_docker_benchmark_<timestamp>/02_DUCKDB/KDB_TIMESERIES/environment.json` | Image digest and row count | Write it during the run. |
| Same folder, `pulls.txt` and `container_inspect.txt` | Pull and limit record | Write it during the run. |
| Same folder, `manifest.json`, `load.txt`, `result.csv`, `checksums.txt` | Rows, load, times, checksums | Write it during the run. |
| Same folder, `summary.md` | Remote, branch, top commit, verdict | Write it after the run. |

## 11. Acceptance gate

1. Make sure that one container runs at a time.
2. Make sure that `docker inspect` reports one central processing unit and 2 gigabytes of memory.
3. Make sure that the generator produces 100,000 rows before timing.
4. Make sure that each query returns the recorded result checksum.
5. Keep the warm-up pass outside the timed set.
6. Report the client timer scope with every number.
7. Make sure that the ShelfDB range slice workload difference is stated beside the ratio.
8. Write all evidence files from Table 7.
9. Stop the container and wait three minutes before the next pair.
10. Record `unsupported` with the named cause if a query cannot run. Do not write a fake result.

## Warnings

- **CAUTION:** The host is a low-power laptop. Do not run Docker, a release build, and a benchmark together.
- **CAUTION:** Kdb needs a license token. This pair does not use Kdb and does not need that token.
- **NOTE:** The ShelfDB slice binary uses 10,000 rows. The DuckDB query uses 100,000 rows.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
