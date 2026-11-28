# ScyllaDB against ShelfDB: YCSB workloads

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Goal and terms

This plan pairs ScyllaDB with ShelfDB on the Yahoo! Cloud Serving Benchmark (YCSB) workloads. Topological Query Language (TQL) is the declarative query language of ShelfDB over a cellular complex. Key-value (KV) describes a store that reads a row by a key.

The approved documents name workloads A to F. They give no exact YCSB command and no workload property file. This plan records names only. The runner must not invent a workload file or a command.

## 2. Original benchmark link

Table 1 answers this question: which source defines the benchmark and the data. Rows are sources. Columns are the source name, the link, and the use in this plan.

| Source name | Link | Use |
|---|---|---|
| YCSB tool | `https://github.com/brianfrankcooper/YCSB` | Original benchmark source for the ScyllaDB row |
| Benchmark sources | Local file `BENCHMARK_SOURCES.md` | Links YCSB to `results/scylladb.json` |
| Dataset list | Local file `DATASETS.md` | Names the local YCSB source directory |
| Widening report | Local file `BENCHMARK_WIDENING_REPORT.md` | Names the six workloads and the metric shape |

## 3. Data set and subset

Table 2 answers this question: which data the pair loads. Rows are data items. Columns are the item, the local source path, and the subset limit.

| Data item | Local source path | Subset limit |
|---|---|---|
| YCSB source | `D:\bench\datasets\YCSB\` | 0.036 gigabyte, manual clone |
| Bundled archive | `nosql_ycsb/ycsb-0.17.0.tar.gz` from `shelf_db_all_datasets.zip` | Extract this archive only |
| Generated records | Directory chosen by the runner | 100,000 operations and 100,000 keys |
| Record size | Workload property in the source tool | 1 kilobyte, as named in the widening report |

The widening report names 10 million to 1 billion records as the published scale. The low-power run uses 100,000 operations. The runner must make sure that the loaded key count matches the subset manifest before it times a workload.

## 4. Test-case document and specific cases

Table 3 answers this question: where the exact cases live. Rows are documents. Columns are the document path, the section, and the specific cases.

| Document path | Section | Specific cases |
|---|---|---|
| `BENCHMARK_WIDENING_REPORT.md` | Section 2, YCSB row, and section 3, phase 1b | Workloads A, B, C, D, E, and F |
| `DATASETS.md` | Row 3 | YCSB suite source and local directory |
| `recursive test/DATASETS_2026-11-22_15-04-04.md` | Section 2 | YCSB source as a benchmark tool |
| `BENCHMARK_SOURCES.md` | Row 7 | Wide-column ScyllaDB mapped to YCSB |

The documents give workload names only. They do not give the property text for workloads A to F. Do not copy property text from the tool source into a result claim.

## 5. Exact query text or names only

The approved documents give no exact query text and no exact YCSB command. The tool source link is the only source for workload definitions. This plan names workloads A, B, C, D, E, and F. It does not write a workload property file.

The runner must record operations per second, median latency, and 99th percentile latency for each named workload. It must record the workload name as the source gives it.

## 6. Container image and install step

Table 4 answers this question: which image the run uses. Rows are image facts. Columns are the fact and the value.

| Fact | Value |
|---|---|
| Database image name | `scylladb/scylla:latest` |
| Pull command | `docker pull scylladb/scylla:latest` |
| Tool source step | Extract `nosql_ycsb/ycsb-0.17.0.tar.gz` from the attached bundle |
| Historical image versions | 3.0.8 in `RUNTIME_COMPARISON.md`, 6.2 in other result documents |
| Native Windows build | None recorded. Use Docker. |

Start Docker Desktop and make sure that `docker info` reports a Linux server. Pull one image. Extract only the named YCSB archive. Do not pull another image while the container runs.

## 7. Low-power limits

Table 5 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing unit allocation | 1 processor | Leaves capacity for the host system |
| Memory allocation | 2 gigabytes of memory | Avoids swap and stutter |
| Benchmark threads | 1 thread | Makes timings comparable |
| Load or workload step | Stop after 30 minutes | Prevents a long stalled run |
| Cooldown after the pair | 3 minutes | Lets the processor settle |

Plan command for the container, derived from the documented start form and the master index shape:

```text
docker run --rm --name bench-scylla --cpus=1 --memory=2g --pids-limit=128 -p 9042:9042 scylladb/scylla:latest --smp 1 --memory 2G
```

## 8. Original benchmark command

The approved documents give no exact YCSB command. They name the tool and the workloads. Write `unavailable` in the command field of the evidence file. Do not invent `bin/ycsb load` or `bin/ycsb run` text.

## 9. Matching ShelfDB command

The widening report names a planned ShelfDB KV shape for YCSB. The repository gives no ShelfDB YCSB runner. Record `unsupported` for workloads A to F with this cause: no ShelfDB YCSB runner exists in the approved documents.

The related ShelfDB KV shape is the range slice binary:

```text
cargo run --release --bin range_slice_bench
```

Do not present this binary as a YCSB equivalent. It measures a 10,000-row slice, not workloads A to F.

## 10. Evidence files

Table 6 answers this question: which files hold the recorded evidence. Rows are files. Columns are the file path and the claim it supports.

| File path | Claim |
|---|---|
| `BENCHMARK_SOURCES.md` | YCSB link and `results/scylladb.json` mapping |
| `DATASETS.md` | Local YCSB source directory and size |
| `BENCHMARK_WIDENING_REPORT.md` | Workload names A to F and the metric shape |
| `sheaf-db/tests/e2e/realtime_bench/results/scylladb.json` | Recorded ScyllaDB KV and slice metrics |
| `attached_bundle_review_2026-11-28_14-19-54/docs/` | Local extracted tool README files, not tracked |

Table 7 answers this question: which files the new run writes. Rows are file names. Columns are the file name and the content. All files sit under `EVIDENCE_2026-11-28_14-38-34/`.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, archive path, and subset |
| `image_digest.txt` | Digest of `scylladb/scylla:latest` |
| `subset_manifest.json` | 100,000 keys, 100,000 operations, and the load checksum |
| `load_output.txt` | Loader output and loaded key count |
| `query_1.txt` to `query_3.txt` | Three measured passes for each named workload |
| `checksums.txt` | Key count and checksum for each pass |
| `shelfdb_output.txt` | The `unsupported` line for workloads A to F and the range slice output |
| `summary.md` | Remote, branch, base commit, image digest, and gate result |

## 11. Acceptance gate

Table 8 answers this question: when the pair passes. Rows are conditions. Columns are the condition, the required result, and the pass or fail state.

| Condition | Required result | Pass or fail |
|---|---|---|
| Subset gate | Loaded count equals the manifest count of 100,000 keys | PASS only on match |
| Workload-name gate | Names A to F appear and no invented property file appears | PASS only when true |
| Command gate | The evidence names the command as `unavailable` | PASS only when recorded |
| Metric gate | Operations per second, median, and 99th percentile appear for each workload | PASS only when complete |
| ShelfDB gate | Workloads A to F report `unsupported` with the named cause | PASS only on clear cause |
| Resource gate | One container, one processor, 2 gigabytes of memory, one thread | PASS only inside the limits |
| Evidence gate | Every file in Table 7 exists and holds output | PASS only when complete |
| Docker gate | If Docker is absent, report `unsupported` with the cause | FAIL for a timing claim |

## Warnings

- **CAUTION:** Do not write YCSB property text from memory. The approved documents do not give that text.
- **CAUTION:** Do not compare 100,000 operations with the published 10 million to 1 billion record scale. Name the subset in every row.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run for this plan.
