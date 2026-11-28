# PLAN_INFLUXDB_TSBS_IOT_2026-11-28_14-38-34

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs InfluxDB with ShelfDB on the Internet of Things (IoT) use case of the Time Series Benchmark Suite (TSBS). This plan records commands and gates. This plan does not pull an image. This plan does not run a benchmark.

## 2. Concept definitions

- **Time Series Benchmark Suite (TSBS):** a tool that generates time-series data and queries.
- **Internet of Things (IoT):** a use case that simulates readings from a fleet of trucks.
- **InfluxDB line protocol:** a text format for one reading with tags, fields, and a timestamp.
- **Topological Query Language (TQL):** a declarative query over a cellular complex.

## 3. Original benchmark link and source files

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the use.

| Item | Value | Use |
|---|---|---|
| Original benchmark link | `https://github.com/timescale/tsbs` | Tool source |
| Main README, extracted | `attached_bundle_review_2026-11-28_14-19-54/docs/tsbs-master__tsbs-master_README.md` | Generation and run commands |
| InfluxDB supplement, extracted | `attached_bundle_review_2026-11-28_14-19-54/docs/tsbs-master__tsbs-master_docs_influx.md` | Flags and data format |
| Bundle entry | `shelf_db_all_datasets.zip`, nested `timeseries_tsbs/tsbs-master.zip` | Local tool archive |

## 4. Dataset and subset

Table 2 answers this question: which data subset this pair uses. Rows are properties. Columns are the property, the value, and the source of the value.

| Property | Value | Source of value |
|---|---|---|
| Data family | TSBS time-series readings | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4 |
| Use case | `iot` | TSBS main README, generation example |
| Data format | `influx`, InfluxDB line protocol | TSBS main README, format list |
| Pseudo-random seed | 123 | TSBS main README, generation example |
| Start time | `2016-01-01T00:00:00Z` | TSBS main README, generation example |
| End time for full example | `2016-01-04T00:00:00Z` | TSBS main README, generation example |
| End time for subset | `2016-01-01T02:46:40Z` | Low-power subset in this plan |
| Log interval | `10s` | TSBS main README, generation example |
| Scale for full example | 4,000 trucks | TSBS main README, generation example |
| Scale for subset | 100 trucks | Low-power subset in this plan |
| Target row count | 100,000 rows | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 4 |
| Generated file | `/tmp/influx-data.gz` | TSBS main README, generation example |

The IoT use case also generates diagnostic rows. Make sure that the generated row count does not exceed 100,000. Reduce the scale before the load step when the count exceeds the target. Record the final count in `subset_manifest.json`.

## 5. Test-case document and specific test cases

The exact test-case document path is `attached_bundle_review_2026-11-28_14-19-54/docs/tsbs-master__tsbs-master_README.md`. Appendix I of that document lists the IoT query types.

Table 3 answers this question: which TSBS cases this pair runs. Rows are cases. Columns are the case name, the query type, the description, and the source path.

| Case name | Query type | Description | Source path |
|---|---|---|---|
| Last location | `last-loc` | Fetch the last location of each truck | TSBS main README, Appendix I |
| Low fuel | `low-fuel` | Fetch all trucks with fuel below 10 percent | TSBS main README, Appendix I |
| Average load | `avg-load` | Calculate average load per truck model per fleet | TSBS main README, Appendix I |

The README names these three types in its multi-type query generation example. That example is the source for this case set.

## 6. Exact query text or command

The approved documents give query type names and descriptions. They do not give the generated query text. This plan records names only for the generated queries. Do not invent query text.

Table 4 answers this question: which exact text the approved sources give. Rows are items. Columns are the item, the exact text, and the source path.

| Item | Exact text | Source path |
|---|---|---|
| Multi-type query command | `QUERY_TYPES="last-loc low-fuel avg-load"` | TSBS main README, query generation section |
| Generated query text | Names only from Appendix I | TSBS main README, Appendix I |
| Query runner flags | `-chunk-response-size`, `-urls` | InfluxDB supplement, `tsbs_run_queries_influx` flags |

The InfluxDB supplement gives command names and flags. It does not give a full InfluxDB loader command line. Do not invent a command line. Use the documented names and flags with the subset values from section 9.

The ShelfDB side uses exact TQL text from another approved case. No approved document gives a TQL translation of a TSBS query type. This plan uses the shape below as a time-series shape match. This plan does not claim semantic equality with a TSBS query.

```text
MATCH HYPERCELL t: Tick OVER (s: Symbol)
WHERE t.time >= 20261027091500 AND t.time <= 20261027101500
SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005)
EMIT SECTION { t.price, t.size, SECTION_ENERGY };
```

Source: `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, block "KDB SHAPE 2: one-hour slice - TQL".

## 7. Container image and install step

Table 5 answers this question: which image and tool install this pair uses. Rows are items. Columns are the item, the value, and the source.

| Item | Value | Source |
|---|---|---|
| Container image | `influxdb:2` | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4 |
| Image pull | `docker pull influxdb:2` | Standard Docker command for the named image |
| Tool install | `go get github.com/timescale/tsbs` | TSBS main README, "Installation" |
| Tool build | `cd $GOPATH/src/github.com/timescale/tsbs` then `make` | TSBS main README, "Installation" |
| Default database endpoint | `http://localhost:8086` | InfluxDB supplement, `-urls` flag |

Run the pull and the tool install at execution time only. Capture the image digest with `docker inspect`.

## 8. Low-power limits

Table 6 answers this question: which limits govern this pair. Rows are limits. Columns are the limit, the value, and the source.

| Limit | Value | Source |
|---|---|---|
| Concurrent containers | 1 | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md`, section 2 |
| Container central processing units | 1 | Same plan, section 2 |
| Container memory | 2 GB | Same plan, section 2 |
| Benchmark threads | 1 | Same plan, section 2 |
| Process identifier limit | 128 | Same plan, section 5 |
| Long step stop | 30 minutes | Same plan, section 2 |
| Cooldown between engines | 3 minutes | Same plan, section 2 |
| Data extraction | Selected files only | Same plan, section 2 |

Start the container with these flags: `--cpus=1 --memory=2g --pids-limit=128`. Use a read-only mount for the data directory.

## 9. Original benchmark command

The command below is verbatim from the TSBS main README. It generates data for the IoT use case.

```bash
tsbs_generate_data --use-case="iot" --seed=123 --scale=4000 \
    --timestamp-start="2016-01-01T00:00:00Z" \
    --timestamp-end="2016-01-04T00:00:00Z" \
    --log-interval="10s" --format="timescaledb" \
    | gzip > /tmp/timescaledb-data.gz
```

The main README lists `influx` as a valid format. This plan derives the InfluxDB IoT run by changing the format to `influx` and the output path to `/tmp/influx-data.gz`. The README gives this multi-type query generation command:

```bash
FORMATS="timescaledb" SCALE=4000 SEED=123 \
    TS_START="2016-01-01T00:00:00Z" \
    TS_END="2016-01-04T00:00:01Z" \
    QUERIES=1000 QUERY_TYPES="last-loc low-fuel avg-load" \
    BULK_DATA_DIR="/tmp/bulk_queries" scripts/generate_queries.sh
```

This plan derives the low-power run from the documented parts. The loader is `tsbs_load_influx` with the documented flags `-urls`, `-consistency`, `-replication-factor`, `-backoff`, and `-gzip`. The runner is `tsbs_run_queries_influx` with `-urls` and `-chunk-response-size`. Only the following values change from the documented examples: `FORMATS="influx"`, `--scale=100`, `--timestamp-end="2016-01-01T02:46:40Z"` for data, `--timestamp-end="2016-01-01T02:46:41Z"` for queries, and one worker. Run one warm-up pass without timing. Run three measured passes.

## 10. Matching ShelfDB command

Build the native binary with one build job. Run ShelfDB as a local process. Do not run a ShelfDB container in this pair.

```bash
sheaf-db/target/release/sheaf_db_app.exe --data-file <subset-file> "<exact TQL text from section 6>"
```

The binary and the `--data-file` flag are named in `IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-11-22_09-02-56.md`. Use one thread. Use the same subset file and the same query count as the InfluxDB run. Record the command with the exact TQL text in `shelfdb_output.txt`.

## 11. Evidence files

Write evidence under `results/local_docker_benchmark_<run-timestamp>/influxdb_iot/`.

Table 7 answers this question: which files hold the evidence. Rows are file names. Columns are the file name and the content.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, data subset |
| `image_digest.txt` | Image digest from `docker inspect` |
| `subset_manifest.json` | Row count, seed, time window, scale, checksum |
| `load_output.txt` | Output of `tsbs_load_influx` |
| `query_1.txt` | Output for `last-loc` |
| `query_2.txt` | Output for `low-fuel` |
| `query_3.txt` | Output for `avg-load` |
| `checksums.txt` | Checksums of all output rows |
| `shelfdb_output.txt` | ShelfDB command, TQL text, output, timings |
| `result.csv` | One row per query with measured time |
| `summary.md` | Remote, branch, base commit, data set, tool, gate result |

## 12. Acceptance gate

A pair is complete only when all of the following hold:

1. One container ran at a time.
2. The loaded row count matches `subset_manifest.json`.
3. The result checksum matches the ShelfDB result.
4. The container stayed within one central processing unit and 2 GB memory.
5. Three measured passes ran with one thread after one untimed warm-up.
6. No step ran longer than 30 minutes.
7. No query text or command line was invented.
8. The evidence folder holds commands, outputs, and checksums.
9. The summary names the remote, branch, base commit, data set, tool, and image digest.
10. No token or secret entered Git.

## Warnings

- **CAUTION:** The host is a low-power AMD Ryzen 3 laptop. Run one database at a time.
- **CAUTION:** The InfluxDB supplement does not give a full loader command line. Do not invent one.
- **NOTE:** The TSBS sources give query type names. They do not give generated query text.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
