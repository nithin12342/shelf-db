# ScyllaDB against ShelfDB: key-value point lookup

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Goal and terms

This plan pairs ScyllaDB with ShelfDB on a key-value (KV) point lookup. A point lookup reads one row by its full key. Topological Query Language (TQL) is the declarative query language of ShelfDB over a cellular complex. Cassandra Query Language (CQL) is the query language of ScyllaDB.

A ShelfDB point-lookup binary does not exist in the recorded application run. This plan records that fact. It does not create a fake ShelfDB point-lookup time.

## 2. Original benchmark link

Table 1 answers this question: which source defines the benchmark and the data. Rows are sources. Columns are the source name, the link, and the use in this plan.

| Source name | Link | Use |
|---|---|---|
| YCSB tool | `https://github.com/brianfrankcooper/YCSB` | Original tool link for the ScyllaDB KV row |
| memtier tool | `https://github.com/redis/memtier_benchmark` | Tool link for KV load generation |
| Benchmark sources | Local file `BENCHMARK_SOURCES.md` | Maps the wide-column row to YCSB |
| ScyllaDB bench source | Local file `sheaf-db/tests/e2e/realtime_bench/scylla_bench.py` | Exact CQL and lookup loop |

## 3. Data set and subset

Table 2 answers this question: which data the pair loads. Rows are data items. Columns are the item, the local source path, and the subset limit.

| Data item | Local source path | Subset limit |
|---|---|---|
| KV table | `bench.kv` created by `scylla_bench.py` | 500,000 rows |
| Lookup keys | Selected by seed 42 in `scylla_bench.py` | 5,000 random keys for ScyllaDB |
| Application KV set | `results/scylladb.json` and `results/redis.json` | 5,000 keys for ScyllaDB, 20,000 keys for other engines |
| Payload | INSERT text in `scylla_bench.py` | 48 characters |

The two recorded sets use different key counts. Do not join them into one timing row. The runner must make sure that the loaded row count matches the subset manifest before it times a lookup.

## 4. Test-case document and specific cases

Table 3 answers this question: where the exact case lives. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 7, Table 4 | Key-value point lookup, median and 99th percentile, 5,000 lookups for ScyllaDB |
| `RUNTIME_COMPARISON.md` | Section 3 and section 4 | ScyllaDB point lookup median and ShelfDB in-process point lookup |
| `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md` | Table 1, ScyllaDB row | Point key lookup with 5,000 keys |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 13, Table 10 | ShelfDB point lookup not run |

Recorded values differ by run. `TQL_APPLICATION_COMPARISON_2026-10-26.md` gives 15,912.5 microseconds median and 17,581.9 microseconds at the 99th percentile for ScyllaDB 3.0.8. `RUNTIME_COMPARISON.md` gives 11,625.1 microseconds for the median row and 10.5 nanoseconds for the ShelfDB in-process path. Name the source with every value.

## 5. Exact query text

The source script gives exact CQL. Use it without change.

Table definition from `sheaf-db/tests/e2e/realtime_bench/scylla_bench.py`:

```sql
CREATE TABLE kv (rng INT, id INT, payload TEXT, PRIMARY KEY (rng, id))
```

Prepared lookup from the same file:

```sql
SELECT payload FROM kv WHERE rng=? AND id=?
```

Insert form from the same file:

```sql
INSERT INTO kv (rng, id, payload) VALUES (%s,%s,%s)
```

## 6. Container image and install step

Table 4 answers this question: which image the run uses. Rows are image facts. Columns are the fact and the value.

| Fact | Value |
|---|---|
| Image name | `scylladb/scylla:latest` |
| Pull command | `docker pull scylladb/scylla:latest` |
| Historical image versions | 3.0.8 in `TQL_APPLICATION_COMPARISON_2026-10-26.md`, 6.2 in other result documents |
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
| Load or lookup step | Stop after 30 minutes | Prevents a long stalled run |
| Cooldown after the pair | 3 minutes | Lets the processor settle |

Plan command for the container, derived from the documented start form and the master index shape:

```text
docker run --rm --name bench-scylla --cpus=1 --memory=2g --pids-limit=128 -p 9042:9042 scylladb/scylla:latest --smp 1 --memory 2G
```

## 8. Original benchmark command

The source script gives the exact command.

```text
py scylla_bench.py
```

The multi-engine runner also records a point-lookup workload. Use its exact documented form when the pair runs inside the matrix:

```text
py run_all.py --keep-up
```

## 9. Matching ShelfDB command

The application comparison states that ShelfDB has no point-lookup binary. Write `unsupported` for a direct point-lookup command. Give this cause: no ShelfDB point-lookup binary exists in the recorded application run.

The related binary measures a range slice. Do not use it as a point lookup.

```text
cargo run --release --bin range_slice_bench
```

The 10.5 nanosecond ShelfDB value in `RUNTIME_COMPARISON.md` is an in-process measurement. It is not the output of a command in this pair. Do not present it as a new run.

## 10. Evidence files

Table 6 answers this question: which files hold the recorded evidence. Rows are files. Columns are the file path and the claim it supports.

| File path | Claim |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/scylladb.json` | ScyllaDB median, 99th percentile, count, and range metrics |
| `sheaf-db/tests/e2e/realtime_bench/results/redis.json` | KV lookup context for the other engine |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | ShelfDB binaries and optimizer output |
| `sheaf-db/tests/e2e/realtime_bench/results/RUN_SUMMARY_20261026_1144.json` | Runner coverage and skip record |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 4 and Table 10 values |

Table 7 answers this question: which files the new run writes. Rows are file names. Columns are the file name and the content. All files sit under `EVIDENCE_2026-11-28_14-38-34/`.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, and subset |
| `image_digest.txt` | Digest of `scylladb/scylla:latest` |
| `subset_manifest.json` | 500,000 rows and 5,000 lookup keys with the checksum |
| `load_output.txt` | Loader output and loaded row count |
| `query_1.txt` to `query_3.txt` | Three measured passes with median and 99th percentile |
| `checksums.txt` | Payload checksum for each lookup set |
| `shelfdb_output.txt` | The `unsupported` line and the range slice output |
| `summary.md` | Remote, branch, base commit, image digest, and gate result |

## 11. Acceptance gate

Table 8 answers this question: when the pair passes. Rows are conditions. Columns are the condition, the required result, and the pass or fail state.

| Condition | Required result | Pass or fail |
|---|---|---|
| Row gate | 500,000 rows and 5,000 lookup keys match the manifest | PASS only on match |
| CQL gate | The prepared lookup text matches the source | PASS only on match |
| Metric gate | Median and 99th percentile appear with the key count | PASS only when complete |
| ShelfDB gate | The absence of a point-lookup binary is reported as `unsupported` | PASS only on clear cause |
| No-fake gate | No range slice result is labelled as a point lookup | PASS only when true |
| Resource gate | One container, one processor, 2 gigabytes of memory, one thread | PASS only inside the limits |
| Evidence gate | Every file in Table 7 exists and holds output | PASS only when complete |
| Docker gate | If Docker is absent, report `unsupported` with the cause | FAIL for a timing claim |

## Warnings

- **CAUTION:** Do not compare 5,000-key and 20,000-key rows without naming the count. The documents use both counts.
- **CAUTION:** Do not turn the in-process nanosecond value into a command result. The source reports it as in-process only.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run for this plan.
