# ScyllaDB against ShelfDB: IEEE fraud cases

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Goal and terms

This plan pairs ScyllaDB with ShelfDB on the IEEE-CIS fraud cases. Topological Query Language (TQL) is the declarative query language of ShelfDB over a cellular complex. Structured Query Language (SQL) is the standard language for relational databases. Key-value (KV) describes a store that reads a row by a key.

The approved documents give case names for this family. They do not give SQL text for the 50 cases. This plan records names only. The runner must not invent query text.

## 2. Original benchmark link

Table 1 answers this question: which source defines the benchmark and the data. Rows are sources. Columns are the source name, the link, and the use in this plan.

| Source name | Link | Use |
|---|---|---|
| IEEE-CIS fraud data | `https://www.kaggle.com/competitions/ieee-fraud-detection/data` | Source of the transaction data |
| Benchmark sources | Local file `BENCHMARK_SOURCES.md` | Links the fraud data to the comparison pages |
| High-variance case report | Local file `HIGH_VARIANCE_50_TEST_CASES_REPORT.md` | ScyllaDB 6.2 latency across the 50 cases |
| 50-case result document | Local file `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Case names and matched row counts |

## 3. Data set and subset

Table 2 answers this question: which data the pair loads. Rows are data items. Columns are the item, the local source path, and the subset limit.

| Data item | Local source path | Subset limit |
|---|---|---|
| Fraud zip | `D:\d2\data\ieee-fraud-detection.zip` | 0.124 gigabyte, manual download |
| Extracted fraud data | `D:\bench\datasets\ieee-fraud\` | 1.355 gigabyte, 590,540 rows |
| Transaction file for the low-power run | `ieee_fraud/train_transaction.csv` from the attached bundle | First 100,000 rows |
| Identity file | `ieee_fraud/train_identity.csv` from the attached bundle | Matching 100,000 rows only when a case needs identity fields |

The full extracted set holds 20,663 fraud rows. The low-power run uses the first 100,000 transaction rows. The runner must make sure that the loaded count matches the subset manifest before it times a case.

## 4. Test-case document and specific cases

Table 3 answers this question: where the exact cases live. Rows are documents. Columns are the document path, the section, and the specific cases.

| Document path | Section | Specific cases |
|---|---|---|
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Section 3 | TC-01 Strict High-Value Risk Equality, TC-02 Single Point Exact Account, TC-03 Ultra-High Outlier Amount, TC-04 Zero-Match Null Filtering, TC-50 Universal Boundary Full Scan |
| `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | List items 1 to 4 and 50 | The same five case names |
| `HIGH_VARIANCE_50_TEST_CASES_REPORT.md` | Section 1 and section 5 | ScyllaDB 6.2 summary and ten representative cases |

The source gives names and matched row counts. It gives no SQL text for these cases. This plan names the cases and does not write SQL for them.

## 5. Exact query text or names only

The approved documents give no exact query text for TC-01, TC-02, TC-03, TC-04, or TC-50. Do not create query text from the case names. The four-table fraud join in `KDB_TIMESERIES_COMPARISON_2026-10-27.md` belongs to the Kdb+ pair. Do not use it in this ScyllaDB pair.

The runner must load the named cases through the recorded runner. It must record the case name, the matched row count, the query time, and the checksum for each case.

## 6. Container image and install step

Table 4 answers this question: which image the run uses. Rows are image facts. Columns are the fact and the value.

| Fact | Value |
|---|---|
| Image name | `scylladb/scylla:latest` |
| Pull command | `docker pull scylladb/scylla:latest` |
| Historical image version | 6.2 in `HIGH_VARIANCE_50_TEST_CASES_REPORT.md` |
| Historical start form | `scylladb/scylla --smp 2 --memory 4G` with a data volume |
| Native Windows build | None recorded. Use Docker. |

Start Docker Desktop and make sure that `docker info` reports a Linux server. Pull one image. Do not pull another image while the container runs.

## 7. Low-power limits

Table 5 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing unit allocation | 1 processor | Leaves capacity for the host system |
| Memory allocation | 2 gigabytes of memory | Avoids swap and stutter |
| Benchmark threads | 1 thread | Makes timings comparable |
| Load or case step | Stop after 30 minutes | Prevents a long stalled run |
| Cooldown after the pair | 3 minutes | Lets the processor settle |

Plan command for the container, derived from the documented start form and the master index shape:

```text
docker run --rm --name bench-scylla --cpus=1 --memory=2g --pids-limit=128 -p 9042:9042 scylladb/scylla:latest --smp 1 --memory 2G
```

## 8. Original benchmark command

The 50-case result document records one runner command. The approved documents give no ScyllaDB-specific command for the fraud cases. Record that fact in the evidence. Do not invent a CQL command.

```text
py run_cli_manual_suite.py
```

The high-variance report records ScyllaDB 6.2 with a mean latency of 13,694.0 microseconds, a median latency of 13,425.8 microseconds, and a 95th percentile latency of 22,885.8 microseconds. Those numbers come from an earlier host and an earlier image. Do not join them to a new run.

## 9. Matching ShelfDB command

```text
py run_cli_manual_suite.py
```

The same runner produces the ShelfDB row for the same case names. Run it with one thread. Record the case name, the matched row count, the latency, and the checksum for each pass.

## 10. Evidence files

Table 6 answers this question: which files hold the recorded evidence. Rows are files. Columns are the file path and the claim it supports.

| File path | Claim |
|---|---|
| `results/HIGH_VARIANCE_50_RESULTS.json` | Raw per-engine latency for the 50 cases |
| `results/MANUAL_CLI_EXECUTION_20261027.json` | Runner output for the 50 cases and five workloads |
| `HIGH_VARIANCE_50_TEST_CASES_REPORT.md` | ScyllaDB 6.2 summary and resource telemetry |
| `BENCHMARK_RESULTS_50_TEST_CASES_2026-10-28.md` | Case names, matched rows, and ShelfDB latency |
| `recursive test/CASE_NAMES_2026-11-22_15-04-04.md` | Consolidated case names |

Table 7 answers this question: which files the new run writes. Rows are file names. Columns are the file name and the content. All files sit under `EVIDENCE_2026-11-28_14-38-34/`.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, and subset |
| `image_digest.txt` | Digest of `scylladb/scylla:latest` |
| `subset_manifest.json` | First 100,000 transaction rows and the row checksum |
| `load_output.txt` | Loader output and loaded row count |
| `query_1.txt` to `query_3.txt` | Three measured passes for the named cases |
| `checksums.txt` | Matched row count and checksum for each case |
| `shelfdb_output.txt` | Output of the ShelfDB runner |
| `summary.md` | Remote, branch, base commit, image digest, and gate result |

## 11. Acceptance gate

Table 8 answers this question: when the pair passes. Rows are conditions. Columns are the condition, the required result, and the pass or fail state.

| Condition | Required result | Pass or fail |
|---|---|---|
| Subset gate | Loaded count equals the manifest count of 100,000 rows | PASS only on match |
| Case-name gate | The five named cases appear in every pass | PASS only when all appear |
| No-invention gate | No SQL text is created from a case name | PASS only when no text is created |
| Checksum gate | Row count and checksum match the ShelfDB output | PASS only on match |
| Resource gate | One container, one processor, 2 gigabytes of memory, one thread | PASS only inside the limits |
| Evidence gate | Every file in Table 7 exists and holds output | PASS only when complete |
| Docker gate | If Docker is absent, report `unsupported` with the cause | FAIL for a timing claim |

## Warnings

- **CAUTION:** Do not treat the five named cases as SQL. The source gives names only.
- **CAUTION:** The recorded ScyllaDB 6.2 numbers and the new image can differ. Name the image version in every result row.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run for this plan.
