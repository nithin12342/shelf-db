# Plan: Redis key-value point lookup pair

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs the published Redis key-value point lookup with a matching ShelfDB command. The published table gives a shape and timings. It gives no command line. This plan records the shape and states the missing command.

## 2. Pair sources

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the source document.

| Item | Value | Source document |
|---|---|---|
| Original benchmark link | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 7, Table 4 | Tracked local document |
| Original benchmark link | `RUNTIME_COMPARISON.md` section 3, point lookup row | Tracked local document |
| Data set and subset | TPC-H scale factor 1, 20,000 random `customer` keys | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 4 |
| Data set size | 1.101 gigabyte (GB), 6,001,215 `lineitem` rows | `DATASETS.md` row 5 |
| Exact test-case document path | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 7 | Tracked local document |
| Specific test cases | Table 4 row "Redis 5.0.14.1": median 270.5 microseconds, 99th percentile 451.8 microseconds, 20,000 lookups | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 4 |
| Native shape | `HGETALL` of two hashes in a Lua script. Names only. | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 1 row 4 |
| ShelfDB binary name | `range_slice_bench` | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 7 |

## 3. Query text and command status

The approved sources give no exact command line and no exact Lua script text. This plan records names only.

Names only:

- Workload: 20,000 random `customer` key point lookups.
- Redis shape: `HGETALL` of two hashes in a Lua script.
- Truth check: each lookup returns the stored value for the named key.

Do not write a Lua script from memory. Do not present the Table 4 timings as a new run result. The timings are the published record for Redis 5.0.14.1.

## 4. Container image and install step

The plan image is the official Redis server image. The published row names Redis 5.0.14.1. The approved sources name no image tag. Record the pulled image digest. Pull one image at a time.

```text
docker pull redis:7
docker run --rm --name redis --cpus=1 --memory=2g --pids-limit=128 -p 6379:6379 redis:7
```

## 5. Low-power limits

Table 2 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value and unit, and the reason.

| Limit | Value and unit | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the central processing unit (CPU) and memory load low |
| Container CPUs | 1 | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 gigabytes (GB) | Avoids swap and laptop stutter |
| Benchmark threads | 1 | Makes timings comparable and avoids thermal spikes |
| Lookup count | 20,000 lookups | Matches the published Table 4 workload |
| Cooldown between engines | 3 minutes | Lets the CPU settle |

## 6. Commands

Original benchmark command:

```text
Not given in the approved sources.
Names only: 20,000 random customer key point lookups, Redis HGETALL of two hashes in a Lua script.
```

Do not run this pair until the harness command is recovered from the tracked results folder. Recovery is a separate task.

Matching ShelfDB command. The approved sources name the binary but give no full command line. This plan assembles the command from the named binary:

```text
cargo run --release --bin range_slice_bench
```

That binary measures a 10,000-row range slice. It is not a point lookup. Record this result:

```text
unsupported: ShelfDB has no point-lookup binary in the published run
```

Do not compute a Redis to ShelfDB ratio for a point lookup. The published report gives no such ratio.

## 7. Evidence files

Table 3 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file, the claim, and the origin.

| File | Claim it supports | Origin |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 4 | Redis median, 99th percentile, and lookup count | Tracked |
| `sheaf-db/tests/e2e/realtime_bench/results/redis.json` | Published Redis run entries | Tracked |
| `results/local_docker_benchmark_<run_timestamp>/environment.json` | Host, Docker version, image digest, and subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/pulls.txt` | Image pull record | New run |
| `results/local_docker_benchmark_<run_timestamp>/container_inspect.txt` | Applied CPU and memory limits | New run |
| `results/local_docker_benchmark_<run_timestamp>/redis/manifest.json` | Loaded key count and data subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/redis/result.csv` | Measured median, 99th percentile, and count | New run |
| `results/local_docker_benchmark_<run_timestamp>/summary.md` | Remote, branch, top commit, and unsupported ShelfDB row | New run |

Replace `<run_timestamp>` with the real run stamp at execution time.

## 8. Acceptance gate

Table 4 answers this question: what result closes this pair. Rows are gates. Columns are the gate, the expected result, the status at run time, and remarks.

| Gate | Expected result | Status at run time | Remarks |
|---|---|---|---|
| Lookup count | 20,000 lookups complete with no error | NOT RUN | Runner fills PASS or FAIL |
| Value match | Each lookup returns the stored value for its key | NOT RUN | Use a checksum file |
| Latency record | Median and 99th percentile come from this run, not from Table 4 | NOT RUN | Unit is microseconds |
| Thread and container limits | One thread, one CPU, 2 GB memory, one container | NOT RUN | Record values in `environment.json` |
| ShelfDB match | The exact string `unsupported` with the named cause | NOT RUN | Do not compute a point lookup ratio |

## Warnings

- **CAUTION:** Do not copy the published 270.5 microsecond value into a new result file.
- **CAUTION:** The 99th percentile column uses microseconds. Keep the unit in the result table.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
