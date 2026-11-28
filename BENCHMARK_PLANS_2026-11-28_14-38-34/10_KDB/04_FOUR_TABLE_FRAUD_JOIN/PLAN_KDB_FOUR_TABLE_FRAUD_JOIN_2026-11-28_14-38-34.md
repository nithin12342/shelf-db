# PLAN_KDB_FOUR_TABLE_FRAUD_JOIN_2026-11-28_14-38-34.md

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Status:** Plan only. No image was pulled. No benchmark was run.

## 1. Goal

This plan pairs Kdb+ with ShelfDB on the controlled four-table fraud join. The plan names the source, the data, the case, the text, the commands, and the gate. The plan does not run the benchmark.

## 2. Concept definitions

- **Kdb+:** a column-oriented time-series database from KX Systems.
- **PyKX:** the official Python integration library for Kdb+ and q.
- **q:** the vector and array query language of Kdb+.
- **TQL:** Topological Query Language, a query over a cellular complex.
- **SQL:** Structured Query Language, the standard language for relational databases.
- **Left join:** a join that keeps all rows from the left table.
- **Fiber bundle:** a pre-materialized section of connected entity data.
- **Gate:** an automatic comparison of a result with a truth value.

## 3. Original benchmark link and source files

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the use.

| Item | Value | Use |
|---|---|---|
| Original benchmark link | `bench_kdb_join.py` at the repository root | Committed benchmark script |
| External benchmark URL | None in the approved documents | Do not invent a URL |
| Main comparison | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 6.1 | Join text and timings |
| Raw evidence | `results/KDB_JOIN_RESULTS.json` | Measured values |
| Consolidated text | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact q and SQL text |
| Data catalog | `recursive test/DATASETS_2026-11-22_15-04-04.md`, section 3 | Controlled five-row family |

The script runs the internal q timer over 1,000 iterations. It runs 50 client passes. The documents give the script, not a verbatim command. A derived command is in section 9.

## 4. Dataset and subset

Table 2 answers this question: which data subset this pair uses. Rows are properties. Columns are the property, the value, and the source of the value.

| Property | Value | Source of value |
|---|---|---|
| Data family | Controlled five-row fraud data | `recursive test/DATASETS_2026-11-22_15-04-04.md`, section 3 |
| Account rows (count) | 5 | `bench_kdb_join.py`, lines 9 to 12 |
| Merchant rows (count) | 3 | Same file, lines 14 to 17 |
| Country rows (count) | 2 | Same file, lines 19 to 21 |
| Payment rows (count) | 5 | Same file, lines 23 to 30 |
| Filter | amount greater than 2000 and risk equal to 1 | Same file, line 34 |
| Low-power subset | The same five rows, no larger | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |

The four tables are `Payment`, `Account`, `Merchant`, and `Country`. The script creates all four tables in memory. Do not use the IEEE-CIS fraud file for this pair.

## 5. Test-case document and specific test cases

The exact test-case document path is `KDB_TIMESERIES_COMPARISON_2026-10-27.md`. Section 6.1, Table 6A, names the controlled five-row fraud join.

Table 3 answers this question: which cases this pair runs. Rows are cases. Columns are the case name, the document path, the location, and the status.

| Case name | Document path | Location | Status |
|---|---|---|---|
| Controlled five-row fraud join | `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Section 6.1, Table 6A | Pass, historical |
| KDB four-table fraud join | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Block "KDB FOUR-TABLE FRAUD JOIN" | Exact q and SQL text |
| Fraud network two-hop traversal | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Block "CASE 2" | Related TQL shape only |

The two-hop traversal uses a different predicate. Read it as a shape comparison. It is not an identical query.

## 6. Exact TQL, q, and SQL text

Table 4 answers this question: which language text the approved sources give. Rows are languages. Columns are the language, the text status, and the source path.

| Language | Text status | Source path |
|---|---|---|
| TQL | No exact text for this join. A related shape is quoted below. | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, block "CASE 2" |
| q | Exact text | Same file, block "KDB FOUR-TABLE FRAUD JOIN: q" |
| SQL | Exact text | Same file, block "KDB FOUR-TABLE FRAUD JOIN: SQL" |

q text, verbatim:

```q
select from ((payments lj accounts) lj merchants) lj countries where amount > 2000, risk = 1
```

SQL text, verbatim:

```sql
SELECT p.pay_id, a.name, m.category, c.region, p.amount, p.risk
FROM payment p
JOIN account a ON p.acct_id = a.acct_id
JOIN merchant m ON p.merchant_id = m.merchant_id
JOIN country c ON p.c_code = c.c_code
WHERE p.amount > 2000 AND p.risk = 1;
```

Related TQL shape, verbatim. This shape uses `a.balance > 1000 AND t.risk == 1`. It is not the join filter.

```text
MATCH HYPERCELL t: Payment OVER (a: Account, m: Merchant, c: Country) WHERE a.balance > 1000 AND t.risk == 1 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, m.category, SECTION_ENERGY };
```

Do not invent TQL text for the exact join. Record the absence in `summary.md`.

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
python bench_kdb_join.py
```

The script writes `results/KDB_JOIN_RESULTS.json`. It measures 1,000 internal q iterations and 50 client passes. Run one untimed warm-up pass before the measured passes. Keep one thread.

## 10. Matching ShelfDB command

Run ShelfDB as a local process with the pre-folded fiber bundle. The documents name the executable. They do not give its flags. Record the exact invocation in the evidence file. Do not invent flags.

```bash
cargo run -p sheaf-engine --bin fiber_join_bench --release -- <flags from the run record>
```

Use the same five-row data. Use one build job. Record the command and the output in `shelfdb_output.txt`.

## 11. Evidence files

Write evidence under `results/local_docker_benchmark_<run-timestamp>/kdb_four_table_fraud_join/`.

Table 7 answers this question: which files hold the evidence. Rows are file names. Columns are the file name and the content.

| File name | Content |
|---|---|
| `environment.json` | Host, Python version, PyKX version, data subset |
| `license_gate.txt` | Gate result, q version, install step, `unsupported` cause when present |
| `subset_manifest.json` | Four table row counts, filter, checksum |
| `kdb_output.txt` | Kdb+ command, internal timer, client timer, joined rows |
| `shelfdb_output.txt` | ShelfDB command, fiber bundle path, output, timing |
| `result.csv` | One row per pass with measured time and unit |
| `checksums.txt` | Checksums of joined rows from both engines |
| `summary.md` | Remote, branch, base commit, data set, case, gate result, TQL absence |

## 12. Acceptance gate

A pair is complete only when all of the following hold:

1. The Kdb+ gate returned `Licensed: True` before the run. Otherwise the plan returned `unsupported` with the named cause.
2. One engine ran at a time. One container, one central processing unit, 2 GB memory, and one thread applied.
3. The four table row counts matched `subset_manifest.json`.
4. The joined row checksum matched the ShelfDB result.
5. Three measured passes ran with one thread after one untimed warm-up.
6. No step ran longer than 30 minutes.
7. The q text and the SQL text in section 6 were used verbatim. No text was invented.
8. The related TQL shape was labeled as a shape comparison, not as the join.
9. The evidence folder holds commands, outputs, timings, and checksums.
10. The summary names the remote, branch, base commit, data set, case, and TQL absence.
11. No token or secret entered Git.

## Warnings

- **CAUTION:** Historical timings came from a different host. Read them as history, not as targets.
- **CAUTION:** The five-row join is not the 50-case fraud suite. Do not mix their timings.
- **NOTE:** The approved sources give no exact TQL text for this join. Do not invent it.
- **NOTE:** This is a plan only. No license was activated and no benchmark was run.
