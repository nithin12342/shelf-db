# MongoDB versus ShelfDB plan: mixed 32-thread workload

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan describes the mixed 32-thread workload that compares MongoDB with ShelfDB. This plan does not install software. This plan does not run a benchmark.

## 2. Concept definitions

- **ShelfDB** is the topological database in this repository.
- **Topological Query Language (TQL)** is a declarative query over a cellular complex.
- **Binary Cellular Storage Format (BCSF)** is the physical storage format of ShelfDB.
- **Throughput** is the number of operations that finish in one second.
- **Median latency (p50)** is the latency that half of the operations stay below.
- **99th percentile latency (p99)** is the latency that 99 percent of the operations stay below.

## 3. Source map

Table 1 answers this question: which document holds each fact for this pair. Rows are documents. Columns are the document, the location, and the fact that the plan uses.

| Document | Location | Fact |
|---|---|---|
| `RUNTIME_COMPARISON.md` | Section 3, row "mixed 32-thread", and section 4, row "Mixed 32-thread" | ShelfDB 217,522 operations per second. MongoDB 4,875 operations per second. ShelfDB wins by about 45 times. |
| `sheaf-db/tests/e2e/realtime_bench/results/mongodb.json` | Run `run8_20261015`, field `mixed_32t` | MongoDB 4,875 operations per second, 16,000 operations, median 5,455 microseconds, 99th percentile 18,382.6 microseconds. |
| `sheaf-db/tests/e2e/realtime_bench/results/mt.json` | Top level | ShelfDB 217,522 operations per second, 32,000 operations, median 5.8 microseconds, 99th percentile 1,472.9 microseconds. |
| `sheaf-db/tests/e2e/realtime_bench/results/REALTIME_RESULTS.md` | Section 6 and fair-rerun table Test 5 | MongoDB 2,497 operations per second, median 9.47 milliseconds. Keep this value separate from 4,875 operations per second. |
| `sheaf-db/tests/e2e/realtime_bench/FAIR_RERUN_PLAN.md` | Test 5, and the execution record for Test 5 | 32 operating system threads. 50 percent reads and 50 percent writes. 30-second cap. Source gate: exceed 2,497 operations per second. |
| `sheaf-db/tests/e2e/realtime_bench/mongo_bench.py` | Lines 2 and 84 to 109 | Exact harness command and exact MongoDB operations. |
| `sheaf-db/crates/sheaf-engine/src/bin/mt_bench.rs` | Lines 1 to 18 and 41 to 75 | ShelfDB fixture, thread count, and operation count. |

## 4. Original benchmark link

No external benchmark tool owns this workload. The source is a local harness in this repository. The file `BENCHMARK_SOURCES.md` names the public results page at `https://github.com/nithin12342/shelf-theory-db/blob/main/sheaf-db/tests/e2e/realtime_bench/results/REALTIME_RESULTS.md`. Write this page in the evidence summary. Do not substitute Yahoo! Cloud Serving Benchmark (YCSB) for this local harness.

## 5. Data set and subset

The harness loads five collections into the `bench` database. Test 5 uses only the `payments` collection.

Table 2 answers this question: which collections hold the subset. Rows are collections. Columns are the collection, the document count, and the use in Test 5.

| Collection | Documents (count) | Use in Test 5 |
|---|---|---|
| `customers` | 100,000 | Fixture only. Test 5 does not read it. |
| `orders` | 100,000 | Fixture only. Test 5 does not read it. |
| `items` | 100,000 | Fixture only. Test 5 does not read it. |
| `payments` | 100,000 | Read target and write target. |
| `shipments` | 100,000 | Fixture only. Test 5 does not read it. |

## 6. Exact test cases and exact text

Table 3 answers this question: which exact command or operation the source gives. Rows are actions. Columns are the action, the exact path, and the exact text. Where the source gives no command, the row names the case only.

| Action | Exact source path | Exact text or name |
|---|---|---|
| Run the MongoDB harness | `sheaf-db/tests/e2e/realtime_bench/mongo_bench.py`, line 2 | `py mongo_bench.py` |
| Read operation | Same file, line 94 | `d.payments.find_one({"_id": i})` |
| Write operation | Same file, line 98 | `d.payments.update_one({"_id": i}, {"$set": {"method": "card"}})` |
| Worker shape | Same file, lines 84 to 105 | 32 workers. 500 operations per worker. Random identifier from 1 to 100,000. Half of the operations are reads. Half are writes. |
| Test case name | `FAIR_RERUN_PLAN.md`, Test 5 | "MongoDB 32-thread concurrency challenge". |
| ShelfDB fixture | `sheaf-db/crates/sheaf-engine/src/bin/mt_bench.rs`, lines 14 to 18 | 100,000 zero-cells. 10,000 two-cells. 32 threads. 1,000 operations per thread. 30-second cap. No command line is given in the source. |

Do not add query text that the source does not give. Do not change the read operation or the write operation.

## 7. Container image and install step

1. The recorded server version is MongoDB 9.0.2. The sources are `REALTIME_RESULTS.md` line 3 and the field `mongo_version` in `mongodb.json`.
2. Pull the image that matches the recorded version: `docker pull mongo:9.0.2`.
3. Start one container with the approved shape: `docker run --rm --name mongo --cpus=1 --memory=2g --pids-limit=128 -p 27017:27017 -d mongo:9.0.2`.
4. Set `mongo_host` to `127.0.0.1` and `mongo_port` to `27017` in the machine configuration file. The harness reads these values through `bench_paths.py`.
5. Install Python and PyMongo on the host only if they are absent. The import statement at line 10 of `mongo_bench.py` names PyMongo. The command at line 2 uses `py`, the Windows Python launcher.
6. Do not install a second MongoDB server on the host. Do not store a password in Git.

## 8. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, and the source.

| Limit | Value | Source |
|---|---|---|
| Concurrent database containers | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container central processing units | 1 | Same section |
| Container memory | 2 gigabytes (2 GB) | Same section |
| Benchmark threads | 1 | Same section |
| Measured passes | 3, after one untimed warm-up | `DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 5 |
| Cooldown between pairs | 3 minutes | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |

**CAUTION:** Keep 32 threads for this pair. The thread count is the workload. Run the 32 threads inside one container with one central processing unit. Record this deviation from the one-thread limit in `summary.md`. Do not report a same-box ratio against the recorded 32-thread value.

## 9. Original command and matching ShelfDB command

Table 5 answers this question: which command runs each side. Rows are sides. Columns are the side, the command, and the source or the derivation.

| Side | Command | Source or derivation |
|---|---|---|
| MongoDB, original benchmark command | `cd sheaf-db/tests/e2e/realtime_bench`, then `py mongo_bench.py` | Line 2 of `mongo_bench.py` |
| ShelfDB, matching command | `cd sheaf-db`, then `cargo run --release --bin mt_bench -p sheaf-engine` | No verbatim command appears in the source. The binary name comes from `sheaf-db/crates/sheaf-engine/src/bin/mt_bench.rs`. This is a derived Cargo invocation. |

Run one warm-up pass. Do not time it. Run three measured passes. Use the same container and the same fixture for all passes.

## 10. Evidence files

Write evidence under `01_MIXED_32_THREAD/EVIDENCE_2026-11-28_14-38-34/`. Use this layout:

```text
environment.json
image_digest.txt
subset_manifest.json
load_output.txt
query_1.txt
query_2.txt
query_3.txt
checksums.txt
shelfdb_output.txt
mongodb_run8_reference.json
shelfdb_mt_reference.json
summary.md
```

Copy the recorded reference values from `results/mongodb.json` and `results/mt.json` into the two reference files. The `summary.md` file must name the remote, the branch, the top commit, the data set, the image digest, the thread deviation, and the gate result.

## 11. Acceptance gate

Table 6 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and a pass or fail result that the runner fills in.

| Condition | Expected result | Result |
|---|---|---|
| Concurrent containers | 1 |  |
| Container central processing units | 1 |  |
| Container memory | 2 GB |  |
| Documents in each of five collections | 100,000 |  |
| MongoDB operations in each measured pass | 16,000, with no error |  |
| ShelfDB operations in each measured pass | 32,000, with no error |  |
| Deadlock or starvation | None |  |
| Median and 99th percentile latency | Record both sides with the data set name |  |
| Checksum comparison | Not applicable. The two sides do different work per operation. Record this exception. |  |
| Evidence files | All files in section 10 are present |  |
| Secrets in Git | None |  |

## Warnings

- **CAUTION:** Keep the MongoDB runs separate. The file `mongodb.json` holds values of 2,497, 3,218, and 4,875 operations per second. RUNTIME_COMPARISON uses 4,875. Do not average these values.
- **CAUTION:** The ShelfDB path reads a BCSF slab and appends a write-ahead log frame. The MongoDB path reads and writes a document. Do not equate the two operations.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
