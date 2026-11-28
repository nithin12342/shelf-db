# MongoDB versus ShelfDB plan: TSBS time-series workload

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan describes the Time Series Benchmark Suite (TSBS) workload that compares MongoDB with ShelfDB. This plan does not install software. This plan does not run a benchmark.

## 2. Concept definitions

- **Time Series Benchmark Suite (TSBS)** is a set of Go programs that generate data and measure database speed.
- **ShelfDB** is the topological database in this repository.
- **Time-series** is an ordered sequence of data points indexed by time.
- **Topological Query Language (TQL)** is a declarative query over a cellular complex.
- **Binary Cellular Storage Format (BCSF)** is the physical storage format of ShelfDB.
- **Dev ops** is a TSBS use case that models nine monitored systems.
- **FlatBuffer** is the serialized data format that TSBS uses for MongoDB.

## 3. Source map

Table 1 answers this question: which document holds each fact for this pair. Rows are documents. Columns are the document, the location, and the fact that the plan uses.

| Document | Location | Fact |
|---|---|---|
| `attached_bundle_review_2026-11-28_14-19-54/docs/tsbs-master__tsbs-master_README.md` | Installation, use-case table, data generation, query generation, Appendix I | Install steps, exact example commands, query type names, MongoDB dev ops support |
| `attached_bundle_review_2026-11-28_14-19-54/docs/tsbs-master__tsbs-master_docs_mongo.md` | Sections "Data format", `tsbs_load_mongo`, `tsbs_run_queries_mongo` | FlatBuffer schema and exact MongoDB flag names |
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | Line 47 | Generate 100,000 tick rows. Do not generate 10 million rows. |
| `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md` | Lines 33 and 48 | Extract `timeseries_tsbs/tsbs-master.zip`. Use image `mongo:7`. |
| `KDB_TIMESERIES_COMPARISON_2026-10-27.md` | Sections 4 and 5 | Four ShelfDB query shapes and the measured 100,000-row results |
| `sheaf-db/crates/sheaf-engine/src/bin/timeseries_shelf_bench.rs` | Lines 8 to 18 and 35 to 143 | ShelfDB fixture and the four queries that the binary runs |

## 4. Original benchmark link

The original link is `https://github.com/timescale/tsbs`. The extracted documents live in `attached_bundle_review_2026-11-28_14-19-54/docs/`. The attached archive is `shelf_db_all_datasets.zip`. Use only the extracted documentation. Do not extract the full data bundle on the laptop.

## 5. Data set and subset

Table 2 answers this question: which data set and subset belong to this pair. Rows are attributes. Columns are the attribute, the value, and the source.

| Attribute | Value | Source |
|---|---|---|
| Generator | TSBS `tsbs_generate_data` | TSBS README, data generation |
| Use case | `cpu-only` | TSBS README, use-case table |
| Reason for the use case | MongoDB supports dev ops. MongoDB does not support the IoT use case. | TSBS README, use-case table |
| PRNG seed | `123` | TSBS README, data generation example |
| Start time | `2016-01-01T00:00:00Z` | TSBS README, data generation example |
| Row limit | 100,000 tick rows | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, line 47 |
| Storage format for MongoDB | FlatBuffer, as the `mongo.fbs` schema defines | TSBS MongoDB supplemental guide |
| Exact scale value | Not named in the approved documents. Do not invent one. | No source |

Make sure that the generator emits 100,000 rows. Reduce the scale or the time span until the count reaches 100,000. Write the chosen scale in `subset_manifest.json`. Do not use the example value of 4,000 for this low-power run.

## 6. Exact test cases and exact text

Table 3 answers this question: which exact text the source gives. Rows are actions. Columns are the action, the exact path, and the exact text or the name. Where the source gives no MongoDB command, the row names the case only.

| Action | Exact source path | Exact text or name |
|---|---|---|
| Install TSBS | TSBS README, section "Installation" | `go get github.com/timescale/tsbs`, then `cd $GOPATH/src/github.com/timescale/tsbs`, then `make` |
| Generate data example | TSBS README, lines 145 to 149 | `tsbs_generate_data --use-case="iot" --seed=123 --scale=4000 --timestamp-start="2016-01-01T00:00:00Z" --timestamp-end="2016-01-04T00:00:00Z" --log-interval="10s" --format="timescaledb" \| gzip > /tmp/timescaledb-data.gz` |
| Generate queries example | TSBS README, lines 189 to 193 | `tsbs_generate_queries --use-case="iot" --seed=123 --scale=4000 --timestamp-start="2016-01-01T00:00:00Z" --timestamp-end="2016-01-04T00:00:01Z" --queries=1000 --query-type="breakdown-frequency" --format="timescaledb" \| gzip > /tmp/timescaledb-queries-breakdown-frequency.gz` |
| Run queries example | TSBS README, lines 332 to 334 | `cat /tmp/queries/timescaledb-cpu-max-all-eight-hosts-queries.gz \| gunzip \| tsbs_run_queries_timescaledb --workers=8 --postgres="host=localhost user=postgres sslmode=disable"` |
| MongoDB load flags | TSBS MongoDB supplemental guide | `-url`, default `localhost:27017`. `-write-timeout`, default `10s`. `-document-per-event`, default `false`. |
| MongoDB query flags | TSBS MongoDB supplemental guide | `-url`, default `localhost:27017`. `-read-timeout`, default `10s`. |
| MongoDB query document text | TSBS MongoDB supplemental guide | Not given. Do not invent query documents. |
| Dev ops query type names | TSBS README, Appendix I | `single-groupby-1-1-1`, `single-groupby-1-1-12`, `single-groupby-1-8-1`, `single-groupby-5-1-1`, `single-groupby-5-1-12`, `single-groupby-5-8-1`, `cpu-max-all-1`, `cpu-max-all-8`, `double-groupby-1`, `double-groupby-5`, `double-groupby-all`, `high-cpu-all`, `high-cpu-1`, `lastpoint`, `groupby-orderby-limit`. Names only. |
| ShelfDB query shapes | `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 4 | Window aggregation, range slice filter, exact point lookup, full column average. Section 4 holds the exact TQL, q, and SQL text. |
| ShelfDB binary | `sheaf-db/crates/sheaf-engine/src/bin/timeseries_shelf_bench.rs` | Binary name `timeseries_shelf_bench`. No command line is given in the source. |

**CAUTION:** The three verbatim source examples use the IoT use case and the `timescaledb` format. MongoDB does not support IoT. Do not run those examples unchanged for MongoDB. The extracted documents give no verbatim MongoDB command line. Use only the documented flag names. Record any adapted command in the evidence file.

## 7. Container image and install step

1. Pull the image from the approved image list: `docker pull mongo:7`.
2. Start one container with the approved shape: `docker run --rm --name mongo --cpus=1 --memory=2g --pids-limit=128 -p 27017:27017 -d mongo:7`.
3. Build the TSBS programs on the host with the three commands in Table 3. Do not build them inside the MongoDB container.
4. Generate the data on the host. Mount the generated file into the container as a read-only file when the loader needs it.
5. Run the TSBS loader and the TSBS query runner against `localhost:27017`. Use the documented flag names from Table 3.
6. Do not start a second container. Do not store a password in Git.

## 8. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, and the source.

| Limit | Value | Source |
|---|---|---|
| Concurrent database containers | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container central processing units | 1 | Same section |
| Container memory | 2 gigabytes (2 GB) | Same section |
| Benchmark threads | 1 worker | Same section |
| Data rows | 100,000 | Same document, line 47 |
| Measured passes | 3, after one untimed warm-up | `DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 5 |
| Cooldown between pairs | 3 minutes | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |

Use one worker in `tsbs_run_queries_mongo`. Do not use the example value of eight workers. Stop a load or a query step after 30 minutes.

## 9. Original command and matching ShelfDB command

Table 5 answers this question: which command runs each side. Rows are sides. Columns are the side, the command, and the source or the derivation.

| Side | Command | Source or derivation |
|---|---|---|
| MongoDB, original benchmark command | No verbatim MongoDB command line is given. The tool names are `tsbs_generate_data`, `tsbs_load_mongo`, and `tsbs_run_queries_mongo`. Use the documented flags in Table 3. | TSBS README and MongoDB supplemental guide |
| ShelfDB, matching command | `cd sheaf-db`, then `cargo run --release --bin timeseries_shelf_bench -p sheaf-engine` | No verbatim command appears in the source. The binary name comes from `sheaf-db/crates/sheaf-engine/src/bin/timeseries_shelf_bench.rs`. This is a derived Cargo invocation. |

Run one warm-up pass. Do not time it. Run three measured passes. Use the same data file for all passes.

## 10. Evidence files

Write evidence under `02_TSBS_TIME_SERIES/EVIDENCE_2026-11-28_14-38-34/`. Use this layout:

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
kdb_reference.md
summary.md
```

Copy the ShelfDB reference values from `KDB_TIMESERIES_COMPARISON_2026-10-27.md`, section 5, into `kdb_reference.md`. The `summary.md` file must name the remote, the branch, the top commit, the data set, the generator seed, the image digest, and the gate result.

## 11. Acceptance gate

Table 6 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and a pass or fail result that the runner fills in.

| Condition | Expected result | Result |
|---|---|---|
| Concurrent containers | 1 |  |
| Container central processing units | 1 |  |
| Container memory | 2 GB |  |
| Query workers | 1 |  |
| TSBS rows loaded | 100,000, with the count in `subset_manifest.json` |  |
| ShelfDB ticks built | 100,000, as the assertion in the binary requires |  |
| MongoDB output | Row count and checksum recorded for each of the three passes |  |
| ShelfDB output | Four timings printed for each of the three passes |  |
| Like-for-like checksum between MongoDB and ShelfDB | `unsupported`. TSBS generates dev ops data. The ShelfDB binary builds its own ticks. The corpora differ. |  |
| Speed ratio between MongoDB and ShelfDB | Do not report one. The corpora differ. |  |
| Evidence files | All files in section 10 are present |  |
| Secrets in Git | None |  |

## Warnings

- **CAUTION:** Do not compare the TSBS dev ops corpus with the ShelfDB in-memory tick fixture as a same-box ratio. The rule in `RUNTIME_COMPARISON.md` requires one data set per comparison row.
- **CAUTION:** The ShelfDB binary builds its fixture in memory. It reads no external file. A like-for-like run needs a code change. This plan does not authorize one.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
