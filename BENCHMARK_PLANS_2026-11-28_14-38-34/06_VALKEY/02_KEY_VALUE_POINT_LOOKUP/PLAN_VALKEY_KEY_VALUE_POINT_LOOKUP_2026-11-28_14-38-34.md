# Plan: Valkey key-value point lookup pair

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs a Valkey key-value point lookup with a matching ShelfDB command. The approved documents name this workload for Valkey. They give no measured Valkey result and no exact Valkey command text.

## 2. Pair sources

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the source document.

| Item | Value | Source document |
|---|---|---|
| Original benchmark link | `https://github.com/redis/memtier_benchmark` | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/MASTER_INDEX_2026-11-28_14-38-34.md` Table 3 |
| Workload assignment | Valkey, point lookup with memtier | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 row 6 |
| Data set and subset | Generated keys, 100,000 keys | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` section 4 item 9 |
| Shape reference | 20,000 key point lookups on Redis 5.0.14.1. Reference only, not a Valkey result. | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 4 |
| Exact test-case document path | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 row 6 | Tracked local document |
| Exact test-case document path | `attached_bundle_review_2026-11-28_14-19-54/docs/memtier_benchmark-master__memtier_benchmark-master_README.md` | Local extracted README, excluded from Git |
| Specific test cases | Point lookup with memtier and the read-only GET command | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 row 6 |
| ShelfDB binary name | `range_slice_bench` | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 7 |
| Published Valkey result | None in the approved documents | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 |

## 3. Query text and command status

The approved sources give no exact Valkey point lookup command. The memtier README documents the `__key__` placeholder at lines 196 and 250. The plan command in section 6 uses that documented placeholder. The plan command is not a quote from a source.

Names only:

- Workload: point lookup with memtier on Valkey.
- Reference shape: Redis point lookup in the published comparison.
- Truth check: each lookup returns the stored value for the named key.

Do not copy the published Redis median of 270.5 microseconds into a Valkey result file.

## 4. Container image and install step

The approved sources name no Valkey image. The plan image is the official Valkey server image. This is a plan choice, not a sourced fact. Record the pulled image digest. Pull one image at a time.

```text
docker pull valkey/valkey:latest
docker run --rm --name valkey --cpus=1 --memory=2g --pids-limit=128 -p 6379:6379 valkey/valkey:latest
```

## 5. Low-power limits

Table 2 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value and unit, and the reason.

| Limit | Value and unit | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the central processing unit (CPU) and memory load low |
| Container CPUs | 1 | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 gigabytes (GB) | Avoids swap and laptop stutter |
| Benchmark threads | 1 | Makes timings comparable and avoids thermal spikes |
| Key count | 100,000 keys | Follows the low-power key-value subset rule |
| Cooldown between engines | 3 minutes | Lets the CPU settle |

## 6. Commands

Original benchmark command. This command uses the documented `__key__` placeholder and the low-power values. The command text is a plan, not a quote from a source:

```text
memtier_benchmark --uri=redis://127.0.0.1:6379/0 --command="GET __key__" --test-time=60 -t 1 -c 1 --key-maximum=99999 --json-out-file=results/local_docker_benchmark_<run_timestamp>/valkey/point_lookup.json
```

The `redis` scheme in the uniform resource identifier selects the Redis protocol. Valkey answers on that protocol.

Matching ShelfDB command. The approved sources name the binary but give no full command line. This plan assembles the command from the named binary:

```text
cargo run --release --bin range_slice_bench
```

That binary measures a 10,000-row range slice. It is not a point lookup. Record this result:

```text
unsupported: ShelfDB has no point-lookup binary in the published run
```

Do not compute a Valkey to ShelfDB ratio for a point lookup. The published report gives no such ratio.

## 7. Evidence files

Table 3 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file, the claim, and the origin.

| File | Claim it supports | Origin |
|---|---|---|
| `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 | Valkey workload assignment | Tracked |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 4 | Redis reference shape and counts | Tracked |
| `results/local_docker_benchmark_<run_timestamp>/environment.json` | Host, Docker version, image digest, and subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/pulls.txt` | Image pull record | New run |
| `results/local_docker_benchmark_<run_timestamp>/container_inspect.txt` | Applied CPU and memory limits | New run |
| `results/local_docker_benchmark_<run_timestamp>/valkey/manifest.json` | Loaded key count and data subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/valkey/point_lookup.json` | Measured median, 99th percentile, and count | New run |
| `results/local_docker_benchmark_<run_timestamp>/summary.md` | Remote, branch, top commit, and unsupported ShelfDB row | New run |

Replace `<run_timestamp>` with the real run stamp at execution time. Promote the result into a tracked `valkey.json` only after the gates pass.

## 8. Acceptance gate

Table 4 answers this question: what result closes this pair. Rows are gates. Columns are the gate, the expected result, the status at run time, and remarks.

| Gate | Expected result | Status at run time | Remarks |
|---|---|---|---|
| Lookup count | 100,000 lookups complete with no error | NOT RUN | Runner fills PASS or FAIL |
| Value match | Each lookup returns the stored value for its key | NOT RUN | Use a checksum file |
| Latency record | Median and 99th percentile come from this Valkey run | NOT RUN | Unit is microseconds |
| New baseline | No number comes from the Redis reference row | NOT RUN | Keep the two files separate |
| Thread and container limits | One thread, one CPU, 2 GB memory, one container | NOT RUN | Record values in `environment.json` |
| ShelfDB match | The exact string `unsupported` with the named cause | NOT RUN | Do not compute a point lookup ratio |

## Warnings

- **CAUTION:** Do not copy the published 270.5 microsecond value into a Valkey result file.
- **CAUTION:** The 99th percentile column uses microseconds. Keep the unit in the result table.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
