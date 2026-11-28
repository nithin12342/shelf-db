# PLAN_KDB_TIME_WINDOW_AGGREGATION_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan pairs Kdb+ with ShelfDB on the one-minute time window aggregation. The plan names the source, the data, the case, the text, the commands, and the gate. The plan does not run the benchmark.

## 2. Concept definitions

- **Kdb+:** a column-oriented time-series database from KX Systems.
- **PyKX:** the official Python integration library for Kdb+ and q.
- **q:** the vector and array query language of Kdb+.
- **TQL:** Topological Query Language, a query over a cellular complex.
- **SQL:** Structured Query Language, the standard language for relational databases.
- **BCSF:** Binary Cellular Storage Format of ShelfDB.
- **Gate:** an automatic comparison of a result with a truth value.
- **Harmonic solve:** numerical relaxation that minimizes boundary energy.

## 3. Original benchmark link and source files

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the use.

| Item | Value | Use |
|---|---|---|
| Original benchmark link | `bench_kdb_timeseries.py` at the repository root | Committed benchmark script |
| External benchmark URL | None in the approved documents | Do not invent a URL |
| Main comparison | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Shapes, q text, timings |
| Raw evidence | `results/KDB_TIMESERIES_RESULTS.json` | Measured values |
| Consolidated text | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact TQL, q, and SQL text |
| Data catalog | `recursive test/DATASETS_2026-11-22_15-04-04.md` | Local synthetic family |

The script runs the aggregation 25 times at the Python client. It runs 100 iterations inside q for the internal timer. The documents give the script, not a verbatim command. A derived command is in section 9.

## 4. Dataset and subset

Table 2 answers this question: which data subset this pair uses. Rows are properties. Columns are the property, the value, and the source of the value.

| Property | Value | Source of value |
|---|---|---|
| Data family | Local synthetic tick records | `recursive test/DATASETS_2026-11-22_15-04-04.md`, section 3 |
| Row count (rows) | 100,000 | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 4 |
| First timestamp | 2026-10-27 09:00:00 | `bench_kdb_timeseries.py`, line 15 |
| Spacing (seconds) | 1 | Same line |
| Symbols | AAPL, GOOG, MSFT | Same file, line 14 |
| Price formula | 150.0 plus row index modulo 10 times 0.1 | Same file, line 16 |
| Size formula | 100 plus row index modulo 50 | Same file, line 17 |
| Window start | 2026-10-27 09:00:00 | Shape 1 TQL in section 6 |
| Window end | 2026-10-27 10:00:00 | Shape 1 TQL in section 6 |
| Low-power subset | 100,000 rows, no larger | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4, item 8 |

An operator must generate the tick rows in q before the run. The script generates them. Do not generate a larger set.

## 5. Test-case document and specific test cases

The exact test-case document path is `KDB_TIMESERIES_COMPARISON_2026-10-27.md`. Section 4.1 names Shape 1, "Time Window Filter and Harmonic Solve". Section 5 gives the measured row.

Table 3 answers this question: which cases this pair runs. Rows are cases. Columns are the case name, the document path, the location, and the status.

| Case name | Document path | Location | Status |
|---|---|---|---|
| Shape 1, time window filter and harmonic solve | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Section 4.1 | Pass, historical |
| KDB Shape 1, time window filter | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Block "KDB SHAPE 1" | Exact text |
| Window aggregation, 100,000 rows | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Section 5, Table 5 | Measured, historical |

## 6. Exact TQL, q, and SQL text

Table 4 answers this question: which language text the approved sources give. Rows are languages. Columns are the language, the text status, and the source path.

| Language | Text status | Source path |
|---|---|---|
| TQL | Exact text | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, block "KDB SHAPE 1: time window filter - TQL" |
| q | Exact text | Same file, block "KDB SHAPE 1: q" |
| SQL | Exact text | Same file, block "KDB SHAPE 1: SQL" |

TQL text, verbatim:

```text
MATCH HYPERCELL t: Tick OVER (s: Symbol)
WHERE t.time >= 20261027090000 AND t.time < 20261027100000
SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005)
EMIT SECTION { t.price, t.size, SECTION_ENERGY };
```

q text, verbatim:

```q
select avg price, sum size by 0D00:01:00 xbar time from quotes
```

SQL text, verbatim:

```sql
SELECT time_bucket(INTERVAL '1 minute', ts) AS b, AVG(price), SUM(size)
FROM quotes GROUP BY 1;
```

Do not invent text. Use these three forms only.

## 7. License gate and install step

Table 5 answers this question: which gate and install step this pair uses. Rows are items. Columns are the item, the value, and the source.

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

Table 6 answers this question: which limits govern this pair. Rows are limits. Columns are the limit, the value, and the source.

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

The approved documents name the script. They give no verbatim command. This plan derives the following command from the script path.

```bash
python bench_kdb_timeseries.py
```

The script writes `results/KDB_TIMESERIES_RESULTS.json`. It measures 25 client passes and 100 internal q iterations. Run one untimed warm-up pass before the measured passes. Keep one thread.

## 10. Matching ShelfDB command

Run ShelfDB as a local process. Do not run a ShelfDB container in this pair. Use one build job.

```bash
sheaf-db/target/release/sheaf_db_app.exe --data-file <tick-subset-file> "MATCH HYPERCELL t: Tick OVER (s: Symbol) WHERE t.time >= 20261027090000 AND t.time < 20261027100000 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { t.price, t.size, SECTION_ENERGY };"
```

Use the same 100,000-row subset. Use the same pass count as the Kdb+ run. Record the command and the output in `shelfdb_output.txt`.

## 11. Evidence files

Write evidence under `results/local_docker_benchmark_<run-timestamp>/kdb_time_window_aggregation/`.

Table 7 answers this question: which files hold the evidence. Rows are file names. Columns are the file name and the content.

| File name | Content |
|---|---|
| `environment.json` | Host, Python version, PyKX version, data subset |
| `license_gate.txt` | Gate result, q version, install step, `unsupported` cause when present |
| `subset_manifest.json` | Row count, first timestamp, spacing, checksum |
| `kdb_output.txt` | Kdb+ command, internal timer, client timer, result rows |
| `shelfdb_output.txt` | ShelfDB command, TQL text, output, timing |
| `result.csv` | One row per pass with measured time and unit |
| `checksums.txt` | Checksums of result rows from both engines |
| `summary.md` | Remote, branch, base commit, data set, case, gate result |

## 12. Acceptance gate

A pair is complete only when all of the following hold:

1. The Kdb+ gate returned `Licensed: True` before the run. Otherwise the plan returned `unsupported` with the named cause.
2. One engine ran at a time. One container, one central processing unit, 2 GB memory, and one thread applied.
3. The generated row count matched `subset_manifest.json`.
4. The aggregation result checksum matched the ShelfDB result.
5. Three measured passes ran with one thread after one untimed warm-up.
6. No step ran longer than 30 minutes.
7. The text in section 6 was used verbatim. No text was invented.
8. The evidence folder holds commands, outputs, timings, and checksums.
9. The summary names the remote, branch, base commit, data set, and case.
10. No token or secret entered Git.

## Warnings

- **CAUTION:** Historical timings came from a different host. Read them as history, not as targets.
- **CAUTION:** Do not run Docker, a release build, and this benchmark at one time.
- **NOTE:** The TQL shape and the q query are shape equivalents. The documents do not claim identical predicates.
- **NOTE:** This is a plan only. No license was activated and no benchmark was run.
