# ScyllaDB against ShelfDB: Topological Query Language case 4

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Goal and terms

This plan pairs ScyllaDB with ShelfDB on case 4 of the Topological Query Language (TQL) application comparison. TQL is the declarative query language of ShelfDB over a cellular complex. Structured Query Language (SQL) is the standard language for relational databases. Cassandra Query Language (CQL) is the query language of ScyllaDB. Key-value (KV) describes a store that reads a row by a key.

The TQL statement gives a query shape. The optimizer gives a cost estimate only. The TQL-to-fiber execution bridge, named OPT-004 in the source documents, is not built. This plan does not state an optimizer cost as an execution time.

## 2. Original benchmark link

Table 1 answers this question: which source defines the benchmark and the data. Rows are sources. Columns are the source name, the link, and the use in this plan.

| Source name | Link | Use |
|---|---|---|
| memtier tool | `https://github.com/redis/memtier_benchmark` | KV tool link for the Redis side of case 4 |
| YCSB tool | `https://github.com/brianfrankcooper/YCSB` | KV tool link for the ScyllaDB side |
| Application comparison | Local file `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Exact TQL text, native text, and optimizer result |
| Consolidated text | Local file `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact case 4 text |

## 3. Data set and subset

Table 2 answers this question: which data the pair loads. Rows are data items. Columns are the item, the local source path, and the subset limit.

| Data item | Local source path | Subset limit |
|---|---|---|
| IMDb truth table | `imdb.by_year` from `imdb_multi_engine_bench.py` | Year 2000, keys above `tt0000`, 10,000 rows |
| TQL workload input | `sheaf-db/tests/e2e/realtime_bench/tql_workloads.conf` | 10,000 rows for case 4 |
| Redis hashes | Two hashes read by a Lua script | Two hashes for the same account shape |
| ScyllaDB evidence | `results/scylladb.json` | Recorded KV and range metrics |

The source notes a boundary difference. The TQL case uses equality on `acct_id` 1001. The native CQL text uses a range scan on the IMDb by-year table. Read the ratio as a shape comparison. The runner must make sure that the loaded row count matches the subset manifest before it times a query.

## 4. Test-case document and specific cases

Table 3 answers this question: where the exact case lives. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 4, case 4 | Payment over Account, equality on account identifier |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 9, Table 9 | Case 4 optimizer cost estimate |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Lines 26 to 30 | Exact TQL and native text |
| `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md` | Table 2 | Point key lookup optimizer row with a 9.17 gain |

## 5. Exact query text

The source gives exact text. Use it without change.

TQL statement for case 4:

```text
MATCH HYPERCELL t: Payment OVER (a: Account) WHERE a.acct_id == 1001 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { a.acct_id, SECTION_ENERGY };
```

Native text for Redis and ScyllaDB from Table 1 of the application comparison:

```text
HGETALL of two hashes in a Lua script
SELECT tconst, avg, votes FROM imdb.by_year WHERE year=2000 AND tconst > 'tt0000' LIMIT 10000
```

The bound-value CQL form in `imdb_multi_engine_bench.py` uses bound values:

```sql
SELECT tconst, avg, votes FROM imdb.by_year WHERE year=? AND tconst > ? LIMIT 10000
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
| Load or query step | Stop after 30 minutes | Prevents a long stalled run |
| Cooldown after the pair | 3 minutes | Lets the processor settle |

Plan command for the container, derived from the documented start form and the master index shape:

```text
docker run --rm --name bench-scylla --cpus=1 --memory=2g --pids-limit=128 -p 9042:9042 scylladb/scylla:latest --smp 1 --memory 2G
```

## 8. Original benchmark command

The application comparison gives the exact multi-engine command.

```text
py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j
```

The same document gives the exact optimizer command.

```text
cargo run --release --bin tql_seven_db_bench
```

Run the two commands as two separate steps. The first produces the native query time. The second produces the optimizer estimate.

## 9. Matching ShelfDB command

```text
cargo run --release --bin tql_seven_db_bench
```

This command parses case 4 and runs the statistical cost optimizer. The recorded unoptimized cost is 101,000.00 relative unit. The recorded optimized cost is 11,008.50 relative unit. The gain is 9.17 as a ratio. These values are estimates. Do not print them in a millisecond column.

For execution on bench data, write `unsupported` with this cause: the OPT-004 TQL-to-fiber execution bridge is not built. Do not create an execution time for the TQL statement.

## 10. Evidence files

Table 6 answers this question: which files hold the recorded evidence. Rows are files. Columns are the file path and the claim it supports.

| File path | Claim |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_multi_engine.json` | Native ScyllaDB range result and row count |
| `sheaf-db/tests/e2e/realtime_bench/results/scylladb.json` | Recorded ScyllaDB KV and range metrics |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | Case 4 optimizer costs and gain |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | Quiet pass values |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Exact case 4 text |

Table 7 answers this question: which files the new run writes. Rows are file names. Columns are the file name and the content. All files sit under `EVIDENCE_2026-11-28_14-38-34/`.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, and subset |
| `image_digest.txt` | Digest of `scylladb/scylla:latest` |
| `subset_manifest.json` | Year 2000 partition and 10,000 row slice bounds |
| `load_output.txt` | Loader output and loaded row count |
| `query_1.txt` to `query_3.txt` | Three measured native query passes |
| `checksums.txt` | Row count and checksum for each pass |
| `shelfdb_output.txt` | Optimizer output and the `unsupported` execution line |
| `summary.md` | Remote, branch, base commit, image digest, and gate result |

## 11. Acceptance gate

Table 8 answers this question: when the pair passes. Rows are conditions. Columns are the condition, the required result, and the pass or fail state.

| Condition | Required result | Pass or fail |
|---|---|---|
| Topological query parse gate | Case 4 parses and passes validation with the exact text | PASS only on match |
| Cost gate | Unoptimized 101,000.00 and optimized 11,008.50 relative unit | PASS only on match |
| Gain gate | Gain equals 9.17 as a ratio and appears in a relative column | PASS only on match |
| Native gate | CQL returns 10,000 rows with the recorded checksum | PASS only on match |
| Execution gate | TQL execution reports `unsupported` with the OPT-004 cause | PASS only on clear cause |
| No-fake gate | No optimizer cost appears as an execution time | PASS only when true |
| Resource gate | One container, one processor, 2 gigabytes of memory, one thread | PASS only inside the limits |
| Evidence gate | Every file in Table 7 exists and holds output | PASS only when complete |
| Docker gate | If Docker is absent, report `unsupported` with the cause | FAIL for a timing claim |

## Warnings

- **CAUTION:** The TQL predicate and the native predicate differ. Read the gain as a shape cost, not as an identical query result.
- **CAUTION:** Do not use the 9.17 gain as a speedup. The source states that the optimizer does not execute a query.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run for this plan.
