# Plan: Valkey memtier GET SET pair

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs the memtier GET and SET workload on Valkey with a matching ShelfDB command. Valkey speaks the Redis protocol. The memtier tool needs no Valkey-specific command syntax. The approved documents name no measured Valkey result.

## 2. Pair sources

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the source document.

| Item | Value | Source document |
|---|---|---|
| Original benchmark link | `https://github.com/redis/memtier_benchmark` | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/MASTER_INDEX_2026-11-28_14-38-34.md` Table 3 |
| Valkey benchmark order | Order 6, point lookup with memtier | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 row 6 |
| Data set and subset | Generated keys and values, 100,000 keys | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` section 4 item 9 |
| Exact test-case document path | `attached_bundle_review_2026-11-28_14-19-54/docs/memtier_benchmark-master__memtier_benchmark-master_README.md` | Local extracted README, excluded from Git |
| Specific test cases | SET and GET commands plus the built-in read and write ratio workload | Memtier README lines 10, 16, and 231 |
| ShelfDB harness name | `streaming_rw_bench` | `CONTINUOUS_STREAMING_READ_WRITE_BENCHMARK_2026-10-28.md` section 4 |
| Published Valkey result | None in the approved documents | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` Table 2 |

## 3. Exact command text from the source

The memtier README gives this exact command example at line 231:

```text
memtier_benchmark --command="SET foo __data__" --command="SET bar __data__" --command="GET foo"
```

The memtier README gives this exact rate-limited example at line 788:

```text
memtier_benchmark --ratio=1:0 --test-time=60 --rate-limiting=100 -t 2 -c 50 --key-pattern=P:P --key-maximum 1000000
```

That example uses two threads and one million keys. Do not use those values on this host. The plan command below uses documented options with the low-power values. The option `--uri` is documented at README line 174. The option `--json-out-file` is documented at README line 180.

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

Original benchmark command. This command uses documented memtier options with the low-power values. The command text is a plan, not a quote from the README:

```text
memtier_benchmark --uri=redis://127.0.0.1:6379/0 --ratio=1:1 --test-time=60 -t 1 -c 1 --key-maximum=100000 --json-out-file=results/local_docker_benchmark_<run_timestamp>/valkey/memtier_get_set.json
```

The `redis` scheme in the uniform resource identifier selects the Redis protocol. Valkey answers on that protocol.

Matching ShelfDB command. The approved sources name the binary but give no full command line. This plan assembles the command from the named binary:

```text
cargo run --release --bin streaming_rw_bench
```

The ShelfDB harness measures continuous mixed read and write work. It does not speak the memtier protocol. Do not compute a ratio from the two tools. Record each tool result in its own row.

## 7. Evidence files

Table 3 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file, the claim, and the origin.

| File | Claim it supports | Origin |
|---|---|---|
| `CONTINUOUS_STREAMING_READ_WRITE_BENCHMARK_2026-10-28.md` | ShelfDB harness name and prior mixed-work results | Tracked |
| `results/local_docker_benchmark_<run_timestamp>/environment.json` | Host, Docker version, image digest, and subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/pulls.txt` | Image pull record | New run |
| `results/local_docker_benchmark_<run_timestamp>/container_inspect.txt` | Applied CPU and memory limits | New run |
| `results/local_docker_benchmark_<run_timestamp>/valkey/manifest.json` | Loaded key count and data subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/valkey/memtier_get_set.json` | Measured operations per second and latencies | New run |
| `results/local_docker_benchmark_<run_timestamp>/valkey/checksums.txt` | Values read back after write | New run |
| `results/local_docker_benchmark_<run_timestamp>/summary.md` | Remote, branch, top commit, and shape note | New run |

Replace `<run_timestamp>` with the real run stamp at execution time. Promote the result into a tracked `valkey.json` only after the gates pass.

## 8. Acceptance gate

Table 4 answers this question: what result closes this pair. Rows are gates. Columns are the gate, the expected result, the status at run time, and remarks.

| Gate | Expected result | Status at run time | Remarks |
|---|---|---|---|
| JSON summary | File exists and reports zero connection errors | NOT RUN | Runner fills PASS or FAIL |
| Key count | Loaded key count equals 100,000 from the manifest | NOT RUN | Runner fills PASS or FAIL |
| Value match | Each sampled GET returns the value that SET wrote | NOT RUN | Use the checksum file |
| New baseline | Every number comes from this Valkey run | NOT RUN | Do not copy a Redis number |
| Thread and container limits | One thread, one CPU, 2 GB memory, one container | NOT RUN | Record values in `environment.json` |
| ShelfDB harness | `streaming_rw_bench` output exists with its own shape named | NOT RUN | Do not compute a cross-tool ratio |

## Warnings

- **CAUTION:** Do not use the README example with two threads and one million keys.
- **CAUTION:** Do not reuse the numbers in `sheaf-db/tests/e2e/realtime_bench/results/redis.json` as a Valkey baseline.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
