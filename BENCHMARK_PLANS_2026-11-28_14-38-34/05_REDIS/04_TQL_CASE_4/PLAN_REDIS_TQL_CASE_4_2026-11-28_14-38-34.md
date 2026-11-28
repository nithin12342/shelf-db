# Plan: Redis TQL case 4 pair

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan pairs Topological Query Language (TQL) case 4 with its native Redis shape. A TQL statement is a declarative query over a cellular complex. The approved sources give the exact TQL text. They give no exact Redis script text.

## 2. Pair sources

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item, the value, and the source document.

| Item | Value | Source document |
|---|---|---|
| Original benchmark link | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 4, case 4 | Tracked local document |
| Original benchmark link | `sheaf-db/tests/e2e/realtime_bench/tql_workloads.conf`, named in the comparison | Tracked local file |
| External tool link | None for case 4 in the approved sources | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 4 |
| Data set and subset | Key-value workload with 10,000 rows. This is a workload input, not a corpus count. | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 12 |
| Shape cells | Payment over Account, equality on `acct_id` 1001 | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 4, case 4 |
| Exact test-case document path | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` lines 26 to 30 | Tracked local file |
| Specific test cases | Case 4, key-value direct point lookup, and optimizer Table 9 case 4 | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 9 |
| Native Redis shape | `HGETALL` of two hashes in a Lua script. Names only. | `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 1 row 4 |
| ShelfDB runner | `tql_seven_db_bench` | `TQL_APPLICATION_COMPARISON_2026-10-26.md` section 17 |

## 3. Exact query text

The source gives this exact TQL case 4 text. Do not edit it.

```text
MATCH HYPERCELL t: Payment OVER (a: Account) WHERE a.acct_id == 1001 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, SECTION_ENERGY };
```

The source gives this exact Structured Query Language (SQL) text for case 4. The named engine for that text is ScyllaDB, not Redis:

```text
SELECT tconst, avg, votes FROM imdb.by_year WHERE year=2000 AND tconst > 'tt0000' LIMIT 10000
```

The source gives no exact Redis Lua script text. Names only for the Redis side: `HGETALL` of two hashes in a Lua script. Do not write a Lua script from memory.

## 4. Container image and install step

The Redis native shape needs a server. The plan image is the official Redis server image. The published sources name Redis 5.0.14.1 but no image tag. The TQL optimizer step runs as a local binary. Record the pulled image digest.

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
| Workload rows | 10,000 rows | Follows the case 4 workload input |
| Cooldown between engines | 3 minutes | Lets the CPU settle |

## 6. Commands

Original benchmark command. The command is the exact TQL text in section 3. The native Redis shape is a name only. Do not invent a command line for the Lua script.

Matching ShelfDB command. This exact command comes from the comparison evidence table:

```text
cargo run --release --bin tql_seven_db_bench
```

That runner lowers each TQL case and reports optimizer costs. The TQL-to-fiber execution bridge, named OPT-004, is not built. A cost from that runner is an estimate. Do not state an estimate as an execution time.

## 7. Evidence files

Table 3 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file, the claim, and the origin.

| File | Claim it supports | Origin |
|---|---|---|
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact case 4 TQL and native SQL text | Tracked |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` Table 9 | Case 4 estimates and gain | Tracked |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | Shared pass optimizer costs | Tracked |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | Quiet pass optimizer costs | Tracked |
| `results/local_docker_benchmark_<run_timestamp>/environment.json` | Host, Docker version, image digest, and subset | New run |
| `results/local_docker_benchmark_<run_timestamp>/redis/result.csv` | Native Redis shape record with no invented script | New run |
| `results/local_docker_benchmark_<run_timestamp>/summary.md` | Remote, branch, top commit, and estimate note | New run |

Replace `<run_timestamp>` with the real run stamp at execution time.

## 8. Acceptance gate

Table 4 answers this question: what result closes this pair. Rows are gates. Columns are the gate, the expected result, the status at run time, and remarks.

| Gate | Expected result | Status at run time | Remarks |
|---|---|---|---|
| TQL text | Text equals section 3, character for character | NOT RUN | Runner fills PASS or FAIL |
| Unoptimized estimate | 101,000 relative unit from Table 9 | NOT RUN | Estimate only |
| Optimized estimate | 11,008.50 relative unit from Table 9 | NOT RUN | Estimate only |
| Gain | 9.17 from Table 9 | NOT RUN | Divide unoptimized by optimized |
| Estimate label | Each cost carries the word `estimate` | NOT RUN | No execution time from this runner |
| Native Redis shape | `HGETALL` of two hashes in a Lua script, names only | NOT RUN | No invented script text |
| Bridge status | `OPT-004 not built` recorded | NOT RUN | No point lookup execution claim |

## Warnings

- **CAUTION:** Do not place a TQL cost beside a measured millisecond value.
- **CAUTION:** Do not copy the Table 9 gain into a new timing result.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
