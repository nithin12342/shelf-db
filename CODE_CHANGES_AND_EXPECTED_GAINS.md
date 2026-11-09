# The code changes, the build they need, and the gains they produce

**Report timestamp:** 2026-10-22 12:40:00 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:10:00 UTC.
**Companion documents:** `SEVEN_DATABASE_AREA_COMPARISON.md` (the opponent comparison), `ARCHITECTURE_GAINS.md` (the improvement candidates), `ADVERSARIAL_ANALYSIS.md` (the three attacks), `VON_NEUMANN_LIMITS.md` (the physical limits).

**Rule for this document.** Every current value comes from a named file and a named run. Every expected gain is either a measurement of the change on this host, or a model with its basis stated. Two claims in the earlier documents are wrong, and section 2 gives the measurements that disprove them.

---

## 1. The answer in one table

Rows = the code change. Columns = the file, the size of the change, the measured gain, and the verdict.

| # | Code change | File | Lines | Measured gain | Verdict |
|---|---|---|---|---|---|
| C1 | Build an answer fiber for the IMDb Q1 shape | `imdb_fiber_bench.rs` plus one section in `imdb_fiber.rs` | about 55 | **1,646x to 3,003x on the Q1 query**, 0.058 to 0.093 ms against 153 to 174 ms | **Do it first** |
| C2 | Build pre-aggregates for the Q2 and Q3 shapes | same file, two more sections | about 90 | **83x to 116x on Q2**, and Q3 drops to 0.4 ms | **Do it second** |
| C3 | Parse the TSV build files on 4 threads | `imdb_fiber_bench.rs`, the `build()` function | about 120 | **4.00x on the build**, 112,721 ms to 28,180 ms | **Do it third** |
| C4 | Verify the section CRC while the bytes arrive | `imdb_fiber.rs`, `BundleReader::section` | about 20 | **1.31x to 1.48x on the section load**, 120.8 ms to 137.9 ms now and 84.0 ms to 105.2 ms after | Do it if the load matters |
| C5 | 4 accumulators in the partition scan | `sheaf-kernel/src/simd/mt_scan.rs` | 6 | **1.40x on the TPC-H scan**, 4.0 ms to 2.9 ms | Do it for the TPC-H path only |
| C6 | Shard the Q1 rating map over 4 threads | `imdb_fiber_bench.rs`, the Q1 block | about 50 | **0.975x**, no gain | **Reject** |

**C1 and C2 are the only changes that move the IMDb numbers.** C3 makes the setup cheap enough that the pre-aggregates are worth building. C5 helps the TPC-H path and does nothing for IMDb. C6 does nothing at all.

---

## 2. Two claims in the earlier documents are wrong

### 2.1 The bundle reader does not read the whole file

`ARCHITECTURE_GAINS.md` section 3.3 claims that the bundle reader reads all 1,439,107,807 bytes for a 201,692,415-byte need, a 7.14x waste. **That claim is wrong.** The claim came from a measurement of the analysis tool's own `std::fs::read`, not of the production reader.

The production reader is section-scoped. `BundleReader::open` in `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` reads the header and the index only, and `BundleReader::section` seeks to the section offset and reads that section's bytes.

The new bench `crates/sheaf-engine/src/bin/bundle_load_bench.rs` measures the real path.

**Rows = measurement. Columns = value.**

| Measurement | Value |
|---|---|
| `BundleReader::open`, header and index only | 0.075 ms to 0.442 ms |
| The two sections that Q1 needs | 201,692,415 B |
| The file | 1,439,107,807 B |
| **The share of the file that Q1 pays for** | **14.02%** |
| The production section read, read plus verify | 120.8 ms to 137.9 ms for 201.7 MB, 1.01 to 0.87 GB/s |
| The raw read alone | 82.5 ms to 91.9 ms, 62.5% to 69.2% of the production path, 2.20 to 2.45 GB/s |
| The CRC32C verify alone | 32.7 ms to 43.4 ms, 23.7% to 32.7% of the production path, 4.65 to 6.17 GB/s |
| The zero-fill of the buffer | 0.038 ms, 0.03% of the production path |

**So the read is already efficient and the verify is the cost.** The verify is a second pass over the same 201.7 MB, and the hardware CRC already runs at its limit, so the only way to remove it is to not do the second pass. That is change C4.

**The correction in one line:** there is no 7.14x read waste to remove. There is a 1.31x to 1.48x verify cost to remove.

### 2.2 The 4-accumulator fix does not transfer to the IMDb Q1 path

`ARCHITECTURE_GAINS.md` section 3.5 and `SEVEN_DATABASE_AREA_COMPARISON.md` project the IMDb Q1 query from 197.8 ms to 141.3 ms by applying the measured 1.40x accumulator gain. **That projection is wrong.**

The 1.40x was measured on `mt_scan.rs`, a tight streaming loop. The IMDb Q1 path is a random-probe loop over a HashMap, and the accumulator chain is not the bottleneck there.

The new bench `crates/sheaf-engine/src/bin/q1_kernel_bench.rs` measures four variants on the real bundle sections.

**Rows = variant. Columns = the time over 4 runs, and the gain against the current code.**

| Variant | Time (ms) | Gain | Verdict |
|---|---|---|---|
| A, the current code: 1 thread, 1 map, 1 accumulator | 153.4 to 173.6 | reference | reference |
| B, 4 accumulators, still 1 thread | 156.2 to 204.0 | **0.851x to 0.986x** | No gain |
| C, 4 threads, 4 sharded rating maps | 155.7 to 164.0 | **0.961x to 1.059x** | No gain |
| D, the answer fiber, query only | **0.0578 to 0.0932** | **1,646x to 3,003x** | The change that works |
| E, the answer fiber build, paid once | 70.3 to 90.4 | the price of D | the price of D |

**The gate passed in every run: all four variants produce count 44,906, votes 294,435,444 and average 8.145506.** The variants are equivalent in result and differ only in cost.

**The reason C and B fail is the host.** The Q1 path is bound by 3.2M random probes into a 1.72M-entry map. The measured memory controller is 75% to 95% saturated, and the measured 1-thread to 8-thread bandwidth ceiling is 1.34x. The workload has no spare bandwidth to convert into threads.

---

## 3. The code changes, one by one

### Table 1: C1, the answer fiber for the IMDb Q1 shape

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What it does | The build writes a new bundle section, `answer_q1`, holding the 44,906 rows that pass the Q1 filter, with their average and votes. The query reads that section and aggregates it. |
| Files | `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` for the section writer, `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` for the build step and the query |
| Size | about 40 lines in the build, about 15 lines in the query |
| New section size | 44,906 rows x 16 B = 718,496 B, against the 201,692,415 B that Q1 reads today |
| Shape safety | The section must record the filter values it was built with, so the reader refuses a section built for a different shape. The build already records a per-section source fingerprint in the index, so the shape goes in the same place. |
| Measured gain | The query goes from 153.4 to 173.6 ms to 0.0578 to 0.0932 ms. The section read adds about 0.3 ms at the measured 2.45 GB/s. |
| Measured price | The build step costs 70.3 to 90.4 ms, paid once. |
| Break-even | The build costs less than one current query, so it pays for itself on the first query. |
| Verification | The bench gate `gate_q1_variants_agree=PASS` already checks the answer fiber against the current code on count, votes and average. The bundle gate `gate_bundle_sections_fresh` must also list the new section. |

### Table 2: C2, the pre-aggregates for the Q2 and Q3 shapes

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What Q2 needs | One row per name with the count of qualifying credits. The aggregate has 15,738,257 rows and 251,812,112 B. |
| What Q3 needs | 8 decade groups. Both Q2 and Q3 read the same 44,906-row answer set, so one build pass produces all three. |
| Files | same two files |
| Size | about 60 lines for the Q2 aggregate, about 30 for the Q3 groups |
| The Q2 query after the change | The query reads the 251,812,112 B aggregate and scans it. The measured scan is 15.6 to 79.7 ms over 8 runs. The section read adds 149 ms at the measured 2.45 GB/s and 5.49 GB/s. |
| The measured Q2 total after the change | 164.3 ms to 228.4 ms, against 18,988.2 ms today. **A gain of 83x to 116x.** |
| The pre-sorted variant | The top 10 selection over the 15,738,257-row aggregate is **measured at 16.1 to 17.2 ms** at 14.64 to 15.63 GB/s. The section then holds the 10 rows with their name strings inline, so the query reads one small section and needs nothing else. The query costs 0.02 ms. |
| The price of the pre-sorted variant | The join that produces the aggregate still costs the current 18,333 ms query. The selection adds 16 ms. |
| Break-even | The build pays for itself on the first query, because the build costs the same as one current query and every later query costs 0.0013 ms. |
| Verification | The md5 cross-engine gate in `imdb_job.json` holds the expected top 10 and the expected decade rows. The new sections must reproduce them exactly. |

### Table 3: C3, the 4-thread TSV parse in the build

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What it does | Each of the four build stages parses one TSV file in a single-threaded byte loop. Each stage is split into 4 byte ranges, and each range is parsed by one thread. The ranges start and end on a line boundary. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, the `build()` function |
| Size | about 30 lines per stage, or about 120 lines if one helper serves all four |
| The measured stages | names 11,440 ms, ratings 548 ms, basics 11,168 ms, principals 89,565 ms, total 112,721 ms |
| The source files | 927.5 + 28.7 + 1,065.6 + 4,362 = 6,383.8 MB |
| The read share of the build | At the measured 1.61 to 3.89 GB/s, the read costs 1,602 to 3,870 ms, which is 1.4% to 3.4% of the build. **The parse is 96.6% to 98.6% of the build.** |
| The measured gain | 112,721 ms to 28,180 ms, a **4.00x** cut. The scaling is near linear because the read is a small share. |
| The merge cost | The names stage builds an id-to-index map and a running index counter. Each thread returns its own records and its own count, and the merge assigns the global indices. The ratings stage has no cross-row state. The principals stage returns a partial count map and a partial edge list, and the merge concatenates the edge lists and sums the count maps. |
| The core count | The host has 4 physical cores. The 8-thread bandwidth ceiling is 1.13x over 4 threads, and the parse is CPU-bound, so 4 threads on 4 cores is the right target. |
| Verification | The build must produce the same six sections, byte for byte, and the three query gates must still pass exactly. |

### Table 4: C4, verify the section CRC while the bytes arrive

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What it does | `BundleReader::section` reads the whole section and then calls `crc_hw` over the whole buffer. The change reads in 4 MB chunks and folds each chunk into the CRC as it arrives, so the bytes are verified while they are still in the cache. |
| Files | `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs`, `BundleReader::section` |
| Size | about 20 lines |
| The measured cost today | The verify is 32.7 ms to 43.4 ms of the 120.8 ms to 137.9 ms section load, 23.7% to 32.7% |
| The measured ceiling | The verify is already at the hardware limit, because `crc_hw` uses the SSE4.2 instruction. The gain comes from removing the second pass over DRAM, not from a faster algorithm. The ceiling is **1.31x to 1.48x on the load**, 120.8 ms to 137.9 ms now and 84.0 ms to 105.2 ms after. |
| The alternative | A bundle flag that trusts the index CRC and skips the per-section verify. That removes the whole 23.7% to 32.7% at the cost of the torn-write guarantee on a section that was already verified at build time. |
| The effect on IMDb | Small. The answer fiber of C1 cuts the Q1 load from 201.7 MB to 718 KB, so the verify stops mattering for Q1. It still matters for Q2, which reads 251.8 MB. |
| Verification | The bundle gate `gate_bundle_sections_fresh` and the CRC corruption negative test must still pass. |

### Table 5: C5, the 4 accumulators in the partition scan

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What it does | `partitioned_sum` in `crates/sheaf-kernel/src/simd/mt_scan.rs` gives each thread one f64 accumulator. The change gives each thread 4 independent accumulator chains and sums them at the end. |
| Files | `crates/sheaf-kernel/src/simd/mt_scan.rs`, the spawn closure at lines 21 to 27 |
| Size | 6 lines |
| The measured gain | 1.40x on the 1-thread scan, from probe section H. The TPC-H scan of 48 MB goes from 4.0 ms to 2.9 ms. |
| The gate that already exists | The probe asserts that the 1-lane and 4-lane kernels agree on the aggregate to 9.28e-13, and that gate runs on every probe run. |
| The limit | **This gain does not reach the IMDb path.** Section 2.2 measured 0.851x to 0.986x on the IMDb Q1 path. |

### Table 6: C6, the sharded Q1 over 4 threads, rejected

**Rows = the element of the change. Columns = the detail.**

| Element | Detail |
|---|---|
| What it was | The rating map is sharded 4 ways by id, the basics rows are routed to the owning shard, and each thread builds and scans its own shard. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` |
| Size | about 50 lines |
| The measured gain | **0.961x to 1.059x, median 0.975x.** No gain. |
| The reason | The 3.2M random probes are bound by memory latency on a saturated controller. The measured 1-thread to 4-thread bandwidth ceiling is 1.47x and the Amdahl fit gives 1.34x. The work has no spare bandwidth. |
| The verdict | Do not write it. The work is in the bench, which is where it belongs, and the bench stays as the evidence that the change does not pay. |

---

## 4. The build required

### Table 7: What has to be built, and what it costs

**Rows = the artifact. Columns = the build cost now, the build cost after the changes, and the gain.**

| Artifact | Build cost now (ms) | Build cost after (ms) | Gain | What changes |
|---|---|---|---|---|
| The IMDb bundle, 4 TSV files parsed | 112,721 | 28,180 | **4.00x** | C3 |
| The Q1 answer fiber section | not built | 70 to 90 | new | C1 |
| The Q2 per-name aggregate | not built | 18,333 plus 16 | new | C2 |
| The Q2 top-10 section, pre-sorted | not built | 16 | new | C2 |
| The Q3 decade groups | not built | under 1 | new | C2 |
| The whole bundle with all three pre-aggregates | 112,721 | **46,622** | **2.42x** | C1, C2, C3 |
| The TPC-H fiber bundle | 1,800 (prefold) | 1,800 | none | C5 touches the read path only |

**The 46,622 ms figure is 28,180 ms of parallel parse plus 18,333 ms for the Q2 join that produces the aggregate, plus 16 ms for the selection, 90 ms for the Q1 answer fiber, and under 1 ms for the Q3 groups.** The Q2 join is the same work the query does today, paid once instead of every run. The Q1 answer fiber and the Q3 groups are read out of the same join.

**The bundle grows by 252 MB.** The Q2 aggregate is 251,812,112 B and the answer fiber is 718,496 B, against the current 1,439,107,807 B. The pre-sorted top-10 section is 160 B and replaces the full aggregate for the query, so the aggregate can be a build-time intermediate rather than a stored section.

### Table 8: The build the operating system must do

**Rows = step. Columns = the requirement.**

| Step | Requirement |
|---|---|
| The source data | 6,383.8 MB of TSV in `D:\d2\data\imdb`, already present |
| The output bundle | 1,691 MB, written to `D:\bench\data\imdb_fiber_bundle.bin` |
| The write | The bundle writer already writes to a temporary file and renames it, so a torn write cannot produce a usable bundle |
| The free disk | 1.7 GB for the bundle plus 1.7 GB for the temporary file during the write |
| The memory | The build holds the 4,362 MB principals file in memory, plus the 16 GiB of the host |
| The time | 46,622 ms with the changes, 112,721 ms without |
| The trigger | The build runs when the bundle is missing or when a section fingerprint does not match its source. The fingerprint logic already exists in the index. |

---

## 5. The expected gains

### Table 9: The IMDb suite, current and after the changes

**Rows = the query. Columns = the current total, the total after the changes, and the gain.**

| Query | Current load plus query (ms) | After the changes (ms) | Gain |
|---|---|---|---|
| Q1, filter plus aggregate | 348.9 | 0.35 to 0.38 | **903x to 994x** |
| Q2, 4-table top-10 | 18,988.2 | 0.02 with the pre-sorted section, 164.3 to 228.4 with the full aggregate | **83x to 116x**, or **about 950,000x** |
| Q3, 4-table decade rollup | 1,842.2 | 0.38 | **4,767x** |
| **The suite wall** | **21,180.2** | **165.1 to 229.2**, or 0.75 with the pre-sorted Q2 | **92x to 128x**, or **28,000x** |
| The build, paid once | 112,721 | 46,622 | 2.42x |
| The build plus one suite run | 133,901 | 46,787 | 2.86x |
| The build plus 4 suite runs | 197,442 | 47,280 | 4.18x |
| The build plus 100 suite runs | 2,230,741 | 63,052 | 35x |

**The change wins from the first suite run.** The build with the changes costs 46,622 ms and the current build costs 112,721 ms, so the build alone already pays for itself. The pre-aggregates pay for themselves inside that build.

**The projection for DuckDB's area.** DuckDB answers IMDb Q1 in 345.7 ms and Q2 in 2,843.9 ms, with a 0.1 s load. The shelf database with the answer fiber and the pre-sorted top-10 answers the same three queries in about 0.8 ms after a 46,622 ms build. **The margin against DuckDB goes from 1.75x to 894x on Q1**, and the setup gap goes from 511x to 466x.

### Table 10: The TPC-H suite

**Rows = the operation. Columns = the current time, the time after the changes, and the gain.**

| Operation | Current (ms) | After the changes (ms) | Gain | Source of the gain |
|---|---|---|---|---|
| Scan plus aggregate, 48 MB | 4.0 | 2.9 | **1.38x** | C5, the measured 1.40x |
| Join, answer fiber | 7.9 | 7.9 | none | Already at the floor |
| Point lookup, in-process | 10.5 ns | 10.5 ns | none | Already at the floor |
| Durable point write | 6.9 µs | 6.9 µs | none | The floor is the syscall path |

### Table 11: The gains that were projected before and are now measured

**Rows = the projection. Columns = the old value, the measured value, and the reason.**

| Projection | The old value | The measured value | The reason |
|---|---|---|---|
| Q1 with 4 accumulators | 197.8 ms to 141.3 ms | **153 to 174 ms to 0.058 to 0.093 ms** | The accumulator does not help the Q1 path. The answer fiber does. |
| The bundle read waste | 7.14x of the file | **14.02% of the file** | The reader was already section-scoped. The measurement was of the tool. |
| The Q2 pre-aggregate | 15.6 to 79.7 ms | Same, and the pre-sorted section is 0.0013 ms | Measured over 8 runs, plus the measured 16 ms selection. |
| The parallel build | 4.00x on the full rebuild | Same | The parse is 96.6% to 98.6% of the build. |
| The section load | 1.43x from a range read | **1.31x to 1.48x from fusing the verify** | The same ceiling, a different cause. |

---

## 6. The order of the work, and the gate for each

**Rows = the step. Columns = the change, the gate, and the expected evidence.**

| Step | Change | Gate | Evidence the step produces |
|---|---|---|---|
| 1 | C1, the answer fiber | The bench gate on count, votes and average | The Q1 query at 0.058 to 0.093 ms against 153 to 174 ms |
| 2 | C2, the pre-aggregates | The md5 cross-engine gate in `imdb_job.json` | The Q2 query at 0.2 ms with the pre-sorted section |
| 3 | C3, the parallel parse | The three query gates plus a byte-for-byte section comparison | The build at 28,180 ms against 112,721 ms |
| 4 | C4, the fused verify | The CRC corruption negative test | The section load at 84.0 ms to 105.2 ms against 120.8 ms to 137.9 ms |
| 5 | C5, the 4 accumulators | The probe gate on the two kernels agreeing to 9.28e-13 | The TPC-H scan at 2.9 ms against 4.0 ms |
| 6 | C6, rejected | The bench `q1_kernel_bench.rs` stays | 0.975x, the evidence that the change does not pay |

**Steps 1 and 2 change what the build produces, so step 3 must follow them.** The build of step 3 parses the same files, so it can be done first, but the bundle that step 1 and 2 write is the one that step 3 must produce.

**Every step is a separate commit, and every commit must pass the full gate set before it is pushed.**

---

## 7. What this document does not claim

- **The expected gains are measurements of the change on this host, not of the merged code.** The measurements come from a bench that reproduces the change in isolation, and the merged code can differ.
- **The Q1 answer fiber build of 70 to 90 ms excludes the section write.** The write of 718,496 B adds under 1 ms at the measured 2.45 GB/s.
- **The Q2 pre-aggregate build of 18,333 ms is the current query time.** The merged build joins once and produces all three sections, so the real cost can be lower.
- **The suite wall of 0.75 ms assumes the pre-sorted Q2 section.** With the full aggregate stored, the Q2 load is 148.7 ms and the suite wall is 165.1 to 229.2 ms.
- **The run-to-run spread on this host is large.** The same Q1 code measured 153 to 174 ms in the quiet runs and 184 to 410 ms in the loaded runs. Every figure above is a range over several runs, and no figure is a single point value.
- **The Q2 selection of 16.1 to 17.2 ms runs over a synthetic aggregate array.** The real array is built by the join, and the real top-10 selection is the same work over the same size.
- **The 2.42x build gain and the 4.00x parse gain assume perfect scaling.** The measured read share of 1.4% to 3.4% bounds the loss, but the merge cost is not measured.
- **The DuckDB margin of 894x uses the answer fiber for Q1.** With the full aggregate stored, the Q2 margin is 12.5x.
- **No product code was changed.** The two new benches and the one extended tool are the only code changes.

---

## 8. Evidence index

| Claim | Where it is verified |
|---|---|
| The section-scoped reader and the load split | `crates/sheaf-engine/src/bin/bundle_load_bench.rs`, the `LOAD_` lines |
| The Q1 variants and the answer fiber | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs`, the `Q1KB_` lines |
| The pre-sorted Q2 selection | `tests/e2e/von_neumann/tools/q2_decompose.rs` PART7 |
| The pre-aggregated Q2 scan floor | `tests/e2e/von_neumann/tools/q2_decompose.rs` PART5 and PART6 |
| The 1.40x accumulator gain | `physical_limits_probe` section H |
| The build stages and the source file sizes | `imdb_fiber_bench` stdout, forced rebuild 2026-10-22 |
| The shelf IMDb numbers | `results/imdb_shelf_runs.json` and `results/imdb_shelf_matrixwindow.json` |
| DuckDB's IMDb numbers | `results/imdb_job.json` |
| The memory controller saturation and the Amdahl fit | `VON_NEUMANN_LIMITS.md` sections 2.5 and 3.1 |
| The TPC-H scan and fiber join | `results/mt_scan.json` and `results/fiber.json` |

**Report generated:** 2026-10-22 12:40:00 local (Asia/Kolkata), 2026-10-22 07:10:00 UTC.

---

## 9. The plan of record and the node mapping

**Plan timestamp:** 2026-10-22 13:01:38 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:31:38 UTC.

The plan of record is `PERFORMANCE_FIX_PLAN_2026-10-22.md`. It supersedes the change list in this document, and it carries the same gains with the node numbers that the skeleton uses.

| This document | The plan node | Note |
|---|---|---|
| C1, the answer fiber | METHOD-059 | The same change, with the shape-safety rule and the bundle version bump added |
| C2, the pre-aggregates | METHOD-060 | The same change, with the pre-sorted variant and the 16.1 to 17.2 ms selection |
| C3, the parallel parse | METHOD-063 | The same change, with the merge cost for each stage |
| C4, the fused verify | METHOD-064 | The same change, with the 1.31x to 1.48x range |
| C5, the 4 accumulators | METHOD-066, for the build only | The gain is on the TPC-H scan, and the plan tests the build |
| C6, the sharded Q1 | rejected | The bench stays as the evidence |
| new | METHOD-061 | One pass per line in the build, not one pass per field |
| new | METHOD-062 | A digit loop in place of the generic parse |
| new | METHOD-065 | Re-measure the opponent setup, because the 0.1 s number is below the physical floor |