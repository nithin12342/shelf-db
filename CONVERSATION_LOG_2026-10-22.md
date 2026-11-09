# CONVERSATION_LOG_2026-10-22.md - session record: the seven-database area comparison, the code changes, and the performance waste plan

**Session timestamp:** 2026-10-22 13:01:38 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:31:38 UTC.
**Session date:** 2026-10-22.
**Branch:** `fiber-bundle-replacement`. **Base commit:** `e8fc5c0` at the session start.
**Commits in the session, in order:** `b582cc1`, `1c2623d`, `af79df7`, and the plan commit that carries this file.

| # | Event | Evidence | Outcome |
|---|---|---|---|
| 1 | The seven-database area comparison was written | `SEVEN_DATABASE_AREA_COMPARISON.md` | Every current value read from a committed result file, every projected value from a measured floor |
| 2 | A 2.2x run spread was found in the shelf's own IMDb numbers | `results/imdb_shelf_runs.json` | Four committed runs archived: Q1 159.4 to 349.6 ms, Q2 10,804.2 to 21,493.8 ms, Q3 1,472.1 to 2,539.3 ms |
| 3 | The ScyllaDB (CQL) load was corrected | `results/imdb_scylla.json` insert_ms 97,443.5 | 24.80 s was wrong. The CQL row on total time moved from 0.49x slower to 1.90x faster, and the headline from 7 of 8 losses to 6 of 8 |
| 4 | The DuckDB setup gap was corrected | `ARCHITECTURE_GAINS.md` section 3.2 | 181x was a speedup ratio, not a gap. The gap is 282x |
| 5 | The PostgreSQL insert margin was corrected | `results/pg.json` and `results/wal_insert.json` | 15,159x was wrong. It is 15.16x, because both values are in microseconds |
| 6 | The tool baselines were given a source | `tools/q1_decompose.rs`, `q2_decompose.rs`, `q2_projection.rs` | Every printed baseline now names its result file and its run key, so the numbers cannot drift again |
| 7 | `VON_NEUMANN_LIMITS.md` was split | Two files, 392 and 404 lines | The file was 774 lines, over the 500-line SPEC-022 gate |
| 8 | The bundle reader was measured for the first time | `crates/sheaf-engine/src/bin/bundle_load_bench.rs` | **The 7.14x whole-file read waste does not exist.** The reader is section-scoped and pays 14.02% of the file |
| 9 | The section load was decomposed | the same bench | The read is 62.5% to 69.2% and the CRC32C verify is 23.7% to 32.7% of the load |
| 10 | The IMDb Q1 kernel was compared across four variants | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs` | **The 1.40x accumulator gain does not reach the IMDb path.** It measures 0.851x to 0.986x there |
| 11 | A 4-thread sharded Q1 was measured | the same bench | 0.961x to 1.059x. No gain. The node was rejected and the bench kept as the evidence |
| 12 | The answer fiber was measured on the real bundle | the same bench | 0.0578 to 0.0932 ms against 153.4 to 173.6 ms, a 1,646x to 3,003x gain, and the build that produces it costs 70.3 to 90.4 ms |
| 13 | The pre-sorted Q2 selection was measured | `tools/q2_decompose.rs` PART7 | 16.1 to 17.2 ms over 15,738,257 rows, so the pre-sorted section is 160 B and the query is 0.0013 ms |
| 14 | The code changes were stated with their gates | `CODE_CHANGES_AND_EXPECTED_GAINS.md` | Six changes, C1 to C6, with C6 rejected |
| 15 | The performance waste was decomposed for both tables | `PERFORMANCE_WASTE_ANALYSIS.md` | 93.2% to 99.7% of the query time is not byte movement, and 96.5% to 98.5% of the build is text handling |
| 16 | The build was decomposed row by row | the same document | 72,779,864 rows at 1,549 ns per row and 56.6 MB/s, with the measured hash insert at 116 to 137 ns |
| 17 | The fix plan was written | `PERFORMANCE_FIX_PLAN_2026-10-22.md` | Eight nodes, METHOD-059 to METHOD-066, with the order as a dependency |

## Corrections filed against the pushed documents

| # | Claim | Where it was | The measurement that disproves it | The correction |
|---|---|---|---|---|
| 1 | The bundle reader reads the whole file, a 7.14x waste | `ARCHITECTURE_GAINS.md` section 3.3 | `bundle_load_bench.rs`: the Q1 sections are 14.02% of the file and the open costs 0.068 ms to 0.442 ms | The reader is section-scoped. The real cost is the CRC32C verify at 23.7% to 32.7% of the load |
| 2 | The 4-accumulator fix takes the IMDb Q1 query from 197.8 ms to 141.3 ms | `ARCHITECTURE_GAINS.md` section 4, `SEVEN_DATABASE_AREA_COMPARISON.md` table 11 | `q1_kernel_bench.rs`: 0.851x to 0.986x on the IMDb Q1 path over 4 runs | The projection is the answer fiber at 0.058 ms to 0.093 ms |
| 3 | The ScyllaDB (CQL) load is 24.80 s | `ADVERSARIAL_ANALYSIS.md` section 6.1 | `results/imdb_scylla.json` insert_ms 97,443.5 | 97.44 s. The CQL total-time row moves from 0.49x slower to 1.90x faster |
| 4 | The DuckDB setup gap after the parallel build is 181x | `ARCHITECTURE_GAINS.md` section 3.2 | 28,180 ms against DuckDB's 100 ms load | 282x. The 181x was the build speedup ratio |
| 5 | The PostgreSQL insert margin is 15,159x | `SEVEN_DATABASE_AREA_COMPARISON.md` | 104.6 µs against 6.9 µs | 15.16x. Both values are in microseconds |
| 6 | The ScyllaDB range-slice margin is 2,741x | `SEVEN_DATABASE_AREA_COMPARISON.md` table 3 | `results/scylladb.json` range_slice_ms 56.7 to 105.5 over 10 runs | 567x to 1,055x. The 2,741x belongs to the separate 274.1 ms IMDb range slice |
| 7 | The Q2 floor is 22.0 ms to 48.7 ms | `ADVERSARIAL_ANALYSIS.md` section 4.4 | 8 runs of `q2_decompose.rs` PART5 | 15.6 ms to 79.7 ms. The box is often loaded, and the range is the honest result |
| 8 | The DuckDB setup of 0.1 s is a parse of the IMDb data | `results/imdb_job.json` | The NVMe floor for 6,383.8 MB is 1,995 ms at the measured 3.20 GB/s. 72,779,864 rows in 0.1 s is 16 cycles per row on 4 cores | Unexplained, not disproved. The repository holds no script for it. METHOD-065 produces the script |

## Measured results this session

| Case | Result | Gate |
|---|---|---|
| The IMDb Q1 answer fiber | 0.0578 to 0.0932 ms against 153.4 to 173.6 ms, all four variants agreeing on count 44,906, votes 294,435,444, average 8.145506 | PASS |
| The IMDb bundle build, forced | names 11,440 ms, ratings 548 ms, basics 11,168 ms, principals 89,565 ms, total 112,721 ms | PASS, 6 sections byte for byte |
| The bundle section load | 120.8 ms to 137.9 ms for 201,692,415 B, 14.02% of the file | PASS |
| The pre-sorted Q2 selection | 16.1 to 17.2 ms over 15,738,257 rows | PASS |
| The pre-aggregated Q2 scan | 15.6 ms to 79.7 ms over 8 runs | PASS |
| The TPC-H fiber join | 3.2 ms best, 7.9 ms measured 2026-10-22 | PASS |
| The von Neumann physical-limits gate | 19 gates | PASS |
| The dependency-inversion audit | crate edges and folder arrows | PASS |
| Clippy, the whole workspace | 0 errors, 0 warnings | PASS |
| The six standalone analysis tools | all run | PASS |
| The 100-plus check script | every cited value re-read from the committed result files, every margin re-derived | PASS |

## Decision recorded

**The shelf database is not slow because it moves bytes.** 93.2% to 99.7% of the query time is a join, and 96.5% to 98.5% of the build is a scalar text loop. The von Neumann analysis is right about the data representation and silent about where the time goes.

**The two structures that share the name "fiber" are the whole story.** The TPC-H fiber is a pre-aggregate and reads 208 bytes for the answer. The IMDb bundle is a pre-parse and holds the source columns, so the byte ratio against the answer is 280.7x instead of 0.13x. The fix is to materialize the answer.

**The micro-optimisation was aimed at the wrong end of the machine.** The accumulator and the sharded map both measured no gain on the query, because the query is bound by random probes on a saturated controller. The build has 72,779,864 independent rows and no dependency chain, so the same techniques have no wall to hit. METHOD-066 tests that.

**The setup comparison rests on an unexplained number.** DuckDB's 0.1 s is below the 1,995 ms physical floor for reading the data, and the repository holds no script for it. METHOD-065 produces the script and comes first, because the total-time table cannot carry an unexplained figure in its largest row.

**The order is a dependency.** The pre-aggregate joins add 18,349 ms to the build. With the 4-way parallel parse the build is 46,529 ms, which is 2.42x better than today. Without it the build is 131,070 ms, which is worse than today.

## Method notes kept for the next session

| Note | Why it matters |
|---|---|
| The shelf's own IMDb query times have a 2.2x spread across four runs of the same binary and the same bundle | Every published figure is a range, and every table names the run it uses |
| The box is 15% to 30% loaded most of the time, and the clock swings with the thermal state | A single point value is not a measurement here. Take the median of at least 3 runs |
| `Measure-Object -Line` counts non-empty lines only | The SPEC-022 500-line gate must be checked with a raw line count |
| A PowerShell script read as ANSI mangles every non-ASCII character in it | Write the script with the write tool, then substitute the characters by code point |
| `Set-Content` on a PowerShell array collapses the newlines | Use the write tool, or `[System.IO.File]::WriteAllLines` |
| `git commit -m` with a here-string fails in PowerShell | Write the message to a file and use `git commit -F` |
| The measurement of a tool's own behaviour is not a measurement of the production path | The 7.14x read waste was measured in the analysis tool. Read the production source before claiming anything about it |

**Record generated:** 2026-10-22 13:01:38 local (Asia/Kolkata), 2026-10-22 07:31:38 UTC.
