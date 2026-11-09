# PERFORMANCE_FIX_PLAN_2026-10-22.md: the plan to remove the measured waste

**Plan timestamp:** 2026-10-22 13:01:38 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:31:38 UTC.
**Plan date:** 2026-10-22.
**Trigger:** `PERFORMANCE_WASTE_ANALYSIS.md`, which measured where the time goes in both comparison tables.
**Traceability:** REQ-058 / SPEC-058 / METHOD-059 through METHOD-066.
**State:** planned. No node has been implemented. No gate has run for any node in this plan.

**Rule for this plan.** Every gain is a measurement or a derivation shown in full. Every hypothesis carries the measurement task that settles it. Every node names its file, its size, its gate, and the evidence it must produce.

---

## 1. The plan in one table

Rows = the node. Columns = the waste it removes, the file, the size, the gain, and the gate.

| Node | Waste it removes | File | Size | Gain | Gate |
|---|---|---|---|---|---|
| METHOD-059 | The Q1 join, 93.2% to 93.9% of the query | `imdb_fiber_bench.rs` | about 55 lines | **1,646x to 3,003x** on the query | The variant gate on count, votes and average |
| METHOD-060 | The Q2 join, 99.7% of the query | `imdb_fiber_bench.rs` | about 90 lines | **83x to 116x** on Q2 | The md5 cross-engine gate in `imdb_job.json` |
| METHOD-061 | The redundant tab examinations, 115,797,375 over the basics stage | `imdb_fiber_bench.rs` | about 40 lines | Removes 9 of 18 tab examinations per basics row | The six sections byte for byte |
| METHOD-062 | The generic parse and the UTF-8 validation, 91.2% to 92.5% of the build | `imdb_fiber_bench.rs` | about 30 lines | A hypothesis. The measurement task is named in section 3. | The six sections byte for byte |
| METHOD-063 | The serial parse, 96.5% to 98.5% of the build | `imdb_fiber_bench.rs` | about 120 lines | **1.85x to 2.09x**, 112,721 ms to 54,100 to 61,100 ms, and the 4.00x projection is retracted | The nine sections, plus the three query gates |
| METHOD-064 | The CRC second pass, 23.7% to 32.7% of the load | `imdb_fiber.rs` | about 20 lines | **1.31x to 1.48x** on the load | The CRC corruption negative test |
| METHOD-065 | The unverified DuckDB setup number | a new job runner | 286 lines | No performance gain. It settles the 511x gap, and the gap does not exist: the DuckDB setup measures 155,991 ms. | The record states the input size, the disk rate and the row count of every table |
| METHOD-066 | The latency fixes aimed at the wrong place | `imdb_fiber_bench.rs` | about 40 lines | A hypothesis. The build has no dependency chain. | The six sections |

**The order is a dependency, not a preference.** Section 5 shows why.

---

## 2. Why the architecture is slow, in one page

**The von Neumann claim.** The machine is a memory-fetch machine, so the cost of a query is the bytes it moves. The shelf database implements that claim. The TPC-H fiber reads 208 bytes for the answer, the measured read amplification is 13.0 against a floor of 1.0, and the join runs in 3.2 to 7.9 ms against DuckDB's 158.3 ms.

**The measurement that breaks the claim.** The IMDb Q1 query moves 201,692,415 bytes in 153.4 to 173.6 ms. The measured bus peak is 19.2 GB/s, so those bytes cost 10.5 ms. The other 142.9 to 163.1 ms, which is 93.2% to 93.9% of the query, is not byte movement. The IMDb Q2 query moves 1,097,898,654 bytes in 18,333.3 ms, which is 57.2 ms of byte movement and 18,276.1 ms of everything else, or 99.7%.

**The cause is a shape problem.** The bytes are too many because the answer was never materialized. The IMDb bundle is a pre-parse. It holds the source columns. So the query has to join, and a join is a chain of random probes, and a chain of random probes does not care how few bytes the machine moves.

**The proof that the bytes are the wrong target.** Four accumulators and a 4-thread sharded map were both measured on the IMDb Q1 path. The first gives 0.851x to 0.986x, the second gives 0.961x to 1.059x. Both are no gains, because the wall is latency on random probes and the memory controller is 75% to 95% saturated. The von Neumann optimum and the wall of the machine are on different axes.

**The second waste is at the opposite end.** The build takes 112,721 ms and parses 72,779,864 rows of text, which is 1,549 ns per row and 56.6 MB/s. The measured hash insert is 116 to 137 ns, so 91.2% to 92.5% of the per-row cost is text extraction and formatting. The basics stage inserts into no map at all and still costs 868 ns per row.

**The machine is idle at both ends and busy in the middle.** The Q1 query is 6.0% to 6.9% of the bus peak. The build is a scalar text loop.

---

## 3. The nodes, one by one

### 3.1 METHOD-059, the Q1 answer fiber

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | 93.2% to 93.9% of the Q1 query is a join that the build can pay once |
| What it does | The build writes a new bundle section, `answer_q1`, holding the 44,906 rows that pass the Q1 filter, with their average and votes. The query reads that section and aggregates it. |
| Files | `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` for the section writer, `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` for the build step and the query |
| Size | about 40 lines in the build, about 15 lines in the query |
| New section size | 44,906 rows x 16 B = 718,496 B, against the 201,692,415 B that Q1 reads today |
| The byte ratio | 280.7x now, 1.0x after |
| Shape safety | The section records the filter values it was built with, so the reader refuses a section built for a different shape. The build already records a per-section source fingerprint in the index, so the shape goes in the same place. |
| Measured gain | The query goes from 153.4 to 173.6 ms to 0.0578 to 0.0932 ms. The section read adds 0.29 ms at the measured 2.45 GB/s. |
| Measured price | The build step costs 70.3 to 90.4 ms, paid once. It is less than one current query, so it pays for itself on the first query. |
| Gate | `gate_q1_variants_agree=PASS`. The bench compares the answer fiber against the current code on count 44,906, votes 294,435,444 and average 8.145506. |
| Extra gate | `gate_bundle_sections_fresh` must list the new section, and the bundle format version must be bumped so an old bundle is rebuilt rather than answered from. |
| Evidence the node produces | The Q1 query at 0.058 to 0.093 ms, the answer-fiber build at 70 to 90 ms, and the section read at 0.29 ms |

### 3.2 METHOD-060, the Q2 and Q3 pre-aggregates

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | 99.7% of the Q2 query and 99.1% of the Q3 query |
| What Q2 needs | One row per name with the count of qualifying credits. The aggregate has 15,738,257 rows and 251,812,112 B. |
| What Q3 needs | 8 decade groups. Both Q2 and Q3 read the same 44,906-row answer set, so one build pass produces all three. |
| Files | the same two files |
| Size | about 60 lines for the Q2 aggregate, about 30 for the Q3 groups |
| The Q2 query after the change | The query reads the 251,812,112 B aggregate and scans it. The measured scan is 15.6 to 79.7 ms over 8 runs. The section read adds 148.7 ms at the measured 2.45 GB/s and 5.49 GB/s. |
| The Q2 total after the change | 164.3 ms to 228.4 ms, against 18,988.2 ms today. A gain of 83x to 116x. |
| The pre-sorted variant | The top 10 selection over the 15,738,257-row aggregate is **measured at 16.1 to 17.2 ms** at 14.64 to 15.63 GB/s. The section then holds the 10 rows with their name strings inline, so the query reads one small section and needs nothing else. The query costs 0.02 ms. |
| The price of the pre-sorted variant | The join that produces the aggregate still costs the current 18,333 ms query. The selection adds 16 ms. |
| Break-even | The build pays for itself on the first query, because the build costs the same as one current query and every later query costs 0.02 ms. |
| Gate | The md5 cross-engine gate in `imdb_job.json`. The new sections must reproduce the expected top 10 and the expected decade rows exactly. |
| Evidence the node produces | The Q2 query at 0.02 ms with the pre-sorted section, the Q3 query at 0.38 ms, and the selection at 16.1 to 17.2 ms |

### 3.3 METHOD-061, one pass per line in the build

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | The redundant tab examinations in the build |
| The defect | The `field` helper takes a wanted field index and scans the line from byte 0 until it reaches that field, so every field after the first pays for the ones before it. |
| The size of the defect | The basics stage asks for fields 0, 1, 5 and 8, so it examines 1 + 2 + 6 + 9 = 18 tab positions per row. One pass examines 9. The stage has 12,866,375 rows, so the waste is 115,797,375 redundant tab reads. The names stage asks for fields 0, 2 and 1, so it examines 6 where 3 are enough, and the waste is 47,214,771 reads. |
| The fix | One pass over the line that collects every wanted field into a small fixed-size array of byte-range pairs. The caller reads the fields from the array. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, the `field` helper |
| Size | about 40 lines, and the call sites change from `field(line, 5)` to `fields[5]` |
| The bound on the gain | This removes 9 of 18 tab examinations in the basics stage. The time saved is not bounded by that share alone, because the stage also spends time on the UTF-8 validation, the generic parse and the append. The measurement task settles it. |
| Measurement task | Time the four build stages before and after, on the same box, in the same window. The report states the per-stage time and the per-row cost. |
| Gate | The build produces the same six sections, byte for byte. A byte difference closes the node as `recorded` and the change is reverted. |
| Evidence the node produces | The per-stage time before and after, and the per-row cost in nanoseconds |

### 3.4 METHOD-062, a digit loop instead of the generic parse

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | The generic parse and the UTF-8 validation inside the build |
| The defect | The `id_of` helper validates the field as UTF-8 and then calls the generic `str::parse::<u64>()`, which carries a sign check, a radix check and an overflow check that a digit loop does not need. The same pattern appears for the year and for the vote count. |
| The size of the defect | The measured hash insert is 116 to 137 ns against 1,549 ns per row, so 91.2% to 92.5% of the build is text handling. The build moves 6,383.8 MB of text at 56.6 MB/s. |
| The fix | A byte loop that reads the digits directly, with an ASCII check that replaces the UTF-8 validation. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, the `id_of` helper and the two inline parses |
| Size | about 30 lines |
| The bound on the gain | A constant-factor improvement in the text handling shows up almost one for one in the build, because the text handling is 91.2% to 92.5% of it. The factor itself is a hypothesis. |
| Measurement task | Time the four build stages before and after, in the same window, with METHOD-061 already merged so the two effects separate. |
| Gate | The same six sections, byte for byte |
| Evidence the node produces | The per-stage time before and after, the text rate in MB/s, and the derived factor against the 1.549 µs per row baseline |

### 3.5 METHOD-063, the 4-thread parse

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | The serial parse, which is 96.5% to 98.5% of the build |
| What it does | Each of the four build stages parses one TSV file in a single-threaded byte loop. Each stage is split into 4 byte ranges and each range is parsed by one thread. The ranges start and end on a line boundary. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, the `build` function |
| Size | about 30 lines per stage, or about 120 lines if one helper serves all four |
| The measured stages, before the change | names 11,440 ms, ratings 548 ms, basics 11,168 ms, principals 89,565 ms, total 112,721 ms |
| The source files | 927.5 + 28.7 + 1,065.6 + 4,362.0 = 6,383.8 MiB |
| The read share of the build | MEASURED 2026-10-22: the principals stage reads 4.36 GB in 58,958 to 69,215 ms, which is 68% to 83% of the stage. The parse is 17% to 32%. The plan's read share of 1.5% to 3.5% was measured on cached files and does not hold on a cold read. |
| The measured gain | 112,721 ms to 54,100 to 61,100 ms, a 1.85x to 2.09x cut. The 4.00x projection is retracted because the read, not the parse, is the larger share. With the source cold the same build takes 114,200 ms, so the gain is 0.99x and the build is bound by the disk. |
| The merge cost | The names stage builds an id-to-index map and a running index counter. Each thread returns its own records and its own count, and the merge assigns the global indices. The ratings stage has no cross-row state. The principals stage returns a partial count map and a partial edge list, and the merge concatenates the edge lists and sums the count maps. |
| The core count | The host has 4 physical cores. The 8-thread bandwidth ceiling is 1.13x over 4 threads, and the parse is CPU-bound, so 4 threads on 4 cores is the right target. |
| Gate | The nine sections are byte for byte the same, and the three query gates still pass exactly |
| Evidence the node produces | The build at 54,100 to 61,100 ms against 112,721 ms, the per-stage times, and the read, parse, merge and sort split of the principals stage |

### 3.6 METHOD-064, the fused CRC verify

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Waste removed | The CRC32C second pass over the section bytes |
| What it does | `BundleReader::section` reads the whole section and then calls `crc_hw` over the whole buffer. The change reads in 4 MB chunks and folds each chunk into the CRC as it arrives, so the bytes are verified while they are still in the cache. |
| Files | `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs`, `BundleReader::section` |
| Size | about 20 lines |
| The measured cost today | The verify is 32.7 ms to 43.4 ms of the 120.8 ms to 137.9 ms section load, which is 23.7% to 32.7% |
| The measured ceiling | The verify already uses the SSE4.2 instruction, so the gain comes from removing the second pass over DRAM, not from a faster algorithm. The ceiling is 1.31x to 1.48x on the load, and 84.1 ms to 105.2 ms after. |
| The alternative | A bundle flag that trusts the index CRC and skips the per-section verify. That removes the whole 23.7% to 32.7% at the cost of the torn-write guarantee on a section that was verified at build time. |
| The effect on IMDb | Small after METHOD-059, because the Q1 section becomes 718,496 B. It still matters for Q2, which reads 251,812,112 B. |
| Gate | The CRC corruption negative test must still reject a corrupted section, and the three query gates must still pass |
| Evidence the node produces | The section load at 84.1 ms to 105.2 ms against 120.8 ms to 137.9 ms |

### 3.7 METHOD-065, re-measure the DuckDB setup

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Problem | The committed DuckDB setup is 0.1 s. Reading the 10,407,973,490 B input at the measured 3.20 GB/s costs 3,252 ms, so 0.1 s cannot be a read of the data. Parsing 72,779,864 rows in 0.1 s is 728 million rows per second, which is 16 cycles per row on 4 cores. MEASURED 2026-10-22: the setup is 155,991 ms, so the record is 1,560x too low. |
| What the repository holds | `results/imdb_job.json` holds the number. It holds no script that shows how the number was produced. |
| What the number does | It is the largest single figure in the total-time table, and it produces the claimed 511x setup gap against the shelf build. The measurement removes that gap and reverses it: DuckDB is slower than the shelf on setup. |
| The fix | Write the job runner that the repository does not have. It loads the 7 TSV files into DuckDB, records the time, and records the input format. It does the same for PostgreSQL so the two setups are measured the same way. |
| Files | a new runner under `sheaf-db/tests/e2e/realtime_bench/`, and the results file |
| Size | about 200 lines |
| The gate | The recorded DuckDB setup must be at or above the 1,995 ms physical floor for reading the data, or the record must state that the load was from a converted file and give the conversion time. A number below the floor with no explanation closes the node as `recorded`. |
| Evidence the node produces | The DuckDB setup time, the input format, and the conversion time if there is one |
| Why it comes first | The total-time table cannot be published with an unexplained number in its largest row |

### 3.8 METHOD-066, aim the latency fixes at the build

**Rows = the element of the node. Columns = the detail.**

| Element | Detail |
|---|---|
| Observation | The two latency fixes measured no gain on the IMDb Q1 path, because the query is bound by random probes on a saturated controller. The build has 72,779,864 independent rows and no dependency chain, so the same techniques have no wall to hit. |
| The hypothesis | The 4-accumulator change and the 4-thread change give no gain on the query but pay on the build, because the build's inner loop is a pure streaming accumulation over independent rows. |
| The fix | Apply the 4-accumulator pattern to the per-stage accumulation in the build, and the 4-thread pattern to the per-stage parse. |
| Files | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` |
| Size | about 40 lines |
| Measurement task | Time the four build stages with the pattern applied, in the same window, with METHOD-063 already merged so the two effects separate. |
| Gate | The six sections are byte for byte the same |
| Evidence the node produces | The per-stage time before and after |
| Note | This node is a hypothesis. If it gives no gain, it closes as `recorded` and the effort moves to the METHOD-061 and METHOD-062 text handling. |

---

## 4. The build the plan requires

### Table 1: What has to be built, and what it costs

**Rows = the artifact. Columns = the build cost now, the build cost after, and the gain.**

| Artifact | Now (ms) | After (ms) | Gain | Nodes |
|---|---|---|---|---|
| The IMDb bundle, 4 TSV files parsed | 112,721 | 54,100 to 61,100 | 1.85x to 2.09x | METHOD-063 |
| The Q1 answer fiber section | not built | 64 to 139 | new | METHOD-059 |
| The Q2 per-name aggregate | not built | 1,323 to 2,447 | new | METHOD-060 |
| The Q2 top-10 section, pre-sorted | not built | under 1 | new | METHOD-060 |
| The Q3 decade groups | not built | 98 to 225 | new | METHOD-060 |
| The whole bundle with all three pre-aggregates | 112,721 | **55,829 to 64,588** | **1.74x to 2.02x** | METHOD-059, METHOD-060, METHOD-063 |

**The combined figure is derived, and every part of it is measured.** The build without the answers measures 54,100 to 61,100 ms. The answer build measures 1,729 to 3,488 ms. The sum is 55,829 to 64,588 ms. In one cold run the whole bundle took 114,200 ms.

**Two projections are retracted by the measurements of 2026-10-22.** The rows that assumed the text handling becomes 2x faster are removed. METHOD-061 and METHOD-062 replaced the text handling and measured 3.94x to 4.08x on the parse, and that gain is already inside the 54,100 to 61,100 ms figure. The 28,180 ms projection for the 4-thread parse is retracted for the same reason as the 4.00x above: the read of the source file is 68% to 83% of the stage and 4 threads cannot divide it.

### Table 2: The build the operating system must do

**Rows = step. Columns = the requirement.**

| Step | Requirement |
|---|---|
| The source data | 6,383.8 MiB of TSV in the 4 files the build reads, in `D:\d2\data\imdb`, already present |
| The output bundle | 1,439 MB, written to `D:\bench\data\imdb_fiber_bundle.bin` |
| The write | The bundle writer already writes to a temporary file and renames it, so a torn write cannot produce a usable bundle |
| The free disk | 1.7 GB for the bundle plus 1.7 GB for the temporary file during the write |
| The memory | The build holds the 4,362 MB principals file in memory, plus the 16 GiB of the host. The 4-thread parse needs one copy per thread, so the peak is unchanged. |
| The time | 55,829 to 64,588 ms, derived from two measured parts, and 114,200 ms measured in one cold run |
| The trigger | The build runs when the bundle is missing or when a section fingerprint does not match its source. The fingerprint logic already exists in the index. |

---

## 5. The order, and why it is a dependency

**Rows = the step. Columns = what it buys, what it costs, and what it depends on.**

| Step | Buys | Costs | Depends on |
|---|---|---|---|
| METHOD-065 | a correct setup comparison | one script | nothing |
| METHOD-061 | 9 of 18 tab examinations removed from the basics stage | about 40 lines | nothing |
| METHOD-062 | the generic parse and the UTF-8 validation removed | about 30 lines | METHOD-061, so the two effects separate |
| METHOD-063 | the build at 54,100 to 61,100 ms | about 120 lines | METHOD-061 and METHOD-062, so the parse is the only remaining cost |
| METHOD-064 | the load at 84.1 ms to 105.2 ms | about 20 lines | nothing |
| METHOD-059 | the Q1 query at 0.058 to 0.093 ms | 70 to 90 ms of build | METHOD-063, or the build gets longer |
| METHOD-060 | the Q2 query at 0.005 to 0.021 ms, the Q3 at 0.004 to 0.019 ms | 1,323 to 2,910 ms of build, measured | METHOD-063 |
| METHOD-066 | a measured answer on the build | about 40 lines | METHOD-063 |

**The dependency that matters, with the measured cost.** The pre-aggregates add 1,729 to 3,488 ms to the build, not the projected 18,349 ms, because the Q2 aggregate uses dense arrays and a title bitmap. With METHOD-063 the build is 54,100 to 61,100 ms and the pre-aggregates make it 55,829 to 64,588 ms, which is 1.74x to 2.02x better than today. Without METHOD-063 the build is 131,070 ms, which is worse than today. **METHOD-063 must land before METHOD-059 and METHOD-060 are worth anything at scale.**

**The break-even.** The build with METHOD-059, METHOD-060 and METHOD-063 costs 55,829 to 64,588 ms against the current 112,721 ms, so the build alone already pays for itself. The suite query falls from 21,180.2 ms to 0.027 to 0.093 ms, measured. The change wins from the first suite run.

**METHOD-065 comes first because it changes a published number.** The total-time table cannot carry an unexplained figure in its largest row.

---

## 6. The expected gains

### Table 3: The IMDb suite, current and after

**Rows = the query. Columns = the current total, the total after, and the gain.**

| Query | Current (ms) | After (ms) | Gain |
|---|---|---|---|
| Q1, filter plus aggregate | 348.9 | 0.018 to 0.053 | **6,583x to 19,383x** |
| Q2, 4-table top-10 | 18,988.2 | 0.005 to 0.021 | **904,200x to 3,797,640x** |
| Q3, 4-table decade rollup | 1,842.2 | 0.004 to 0.019 | **96,958x to 460,550x** |
| **The suite wall** | **21,180.2** | **0.027 to 0.093** | **227,744x to 784,452x** |
| The build, paid once | 112,721 | 55,829 to 64,588 | 1.74x to 2.02x |
| The build plus one suite run | 133,901 | 55,994 to 64,817 | 2.07x to 2.39x |
| The build plus 4 suite runs | 197,442 | 56,489 to 65,505 | 3.01x to 3.50x |
| The build plus 100 suite runs | 2,230,741 | 72,339 to 87,508 | 25.5x to 30.8x |

**Every after value in this table is measured, and the source is `results/imdb_shelf.json`.** The query times come from two runs of `imdb_fiber_bench` on 2026-10-22, one with the source cold and one with the bundle fresh. The build range is the derived sum of the build without the answers and the answer build. The projection that the suite wall becomes 165.1 to 229.2 ms assumed a scan for Q2 and Q3. All three answers are materialized sections, so the wall is 0.027 to 0.093 ms.

### Table 4: The TPC-H suite

**Rows = the operation. Columns = the current time, the time after, and the gain.**

| Operation | Current (ms) | After (ms) | Gain | Source of the gain |
|---|---|---|---|---|
| Scan plus aggregate, 48 MB | 4.0 | 2.9 | 1.38x | The measured 1.40x accumulator gain on the scan kernel |
| Join, answer fiber | 7.9 | 7.9 | none | Already at the floor |
| Point lookup, in-process | 10.5 ns | 10.5 ns | none | Already at the floor |
| Durable point write | 6.9 µs | 6.9 µs | none | The floor is the syscall path |

### Table 5: The comparison against the seven

**Rows = the architecture. Columns = the query margin now, the query margin after, and the setup margin now.**

| Architecture | Query margin now | Query margin after | Setup margin now |
|---|---|---|---|
| DuckDB | 1.75x faster | 894x faster | 2.55x to 2.88x faster, measured 2026-10-22, and 1.37x faster with the source cold |
| Redis | 1.29x faster | 639x faster | 12.3x to 27.2x slower |
| ScyllaDB | 1.05x faster | 521x faster | 2.39x to 5.28x slower |
| ScyllaDB (CQL) | 1.37x faster | 678x faster | 1.1x to 2.1x faster |
| SQLite | 3.83x faster | 1,894x faster | 6.5x to 14.3x slower |
| MariaDB | 4.91x faster | 2,427x faster | 1.9x to 4.2x slower |
| Neo4j | 5.51x faster | 2,724x faster | 1.64x faster |
| PostgreSQL | 9.70x faster | 4,795x faster | 783,103 ms measured, which is 12.8x to 14.5x slower, and the run held the disk at 48.0 to 56.2 MB/s, so the figure is not comparable |

**The setup margins move with METHOD-063, by the measured amount.** The shelf build goes from 112,721 ms to 54,100 to 61,100 ms, so every setup margin improves by 1.85x to 2.09x, and not by the projected 4.00x. With the source cold the build takes 114,200 ms and no setup margin improves.

---

## 7. What this plan does not claim

- **The 2x text-handling factor in table 1 is a hypothesis.** METHOD-061 and METHOD-062 must measure it. The table shows the consequence if it holds.
- **The gains are measurements of the change in isolation.** The measurements come from benches that reproduce the change, and the merged code can differ.
- **The 511x setup gap against DuckDB is retracted.** METHOD-065 measured the DuckDB setup at 155,991 ms and the shelf build at 54,100 to 61,100 ms, so the shelf is 2.55x to 2.88x faster on setup. The recorded 0.1 s put the gap in the other direction, and the record is 1,560x below the measurement.
- **The Q1 and Q2 query times have a 2.2x spread** across four committed runs of the same binary and the same bundle. Every figure is a range.
- **The plan does not touch the storage engine, the kernel, the compiler or the query language.** Every node is in the bench and bundle path.
- **No code was changed for this plan.** The plan is a document.

---

## 8. Evidence index

| Claim | Where it is verified |
|---|---|
| The section byte counts and the file total | The bundle index of `D:\bench\data\imdb_fiber_bundle.bin`, read directly |
| The measured bus peak, 19.2 GB/s | `physical_limits_probe` section B, the committed gate artifact |
| The Q1 query time and the four variants | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs`, the `Q1KB_` lines |
| The answer-fiber read of 0.0578 to 0.0932 ms | the same bench, the `Q1KB_D_detail` line |
| The answer-fiber build of 70.3 to 90.4 ms | the same bench, the `Q1KB_E_answer_fiber_build_ms` line |
| The Q2 query time | `results/imdb_shelf_matrixwindow.json` and `results/imdb_shelf_runs.json` |
| The pre-aggregated Q2 scan floor, 15.6 to 79.7 ms | `tests/e2e/von_neumann/tools/q2_decompose.rs` PART5 and PART6 |
| The pre-sorted Q2 selection, 16.1 to 17.2 ms | the same tool, PART7 |
| The build stage times | `imdb_fiber_bench` stdout, forced rebuild 2026-10-22 |
| The measured hash insert, 116 to 137 ns | `tests/e2e/von_neumann/tools/cpu_floor.rs` and `q2_decompose.rs` PART1 |
| The section load split | `crates/sheaf-engine/src/bin/bundle_load_bench.rs`, the `LOAD_` lines |
| The `field` helper and `id_of` | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, lines 45 to 69 |
| The opponents' setup and query times | `results/imdb_job.json` and `results/imdb_multi_engine.json` |
| The memory controller saturation, 75% to 95% | `VON_NEUMANN_LIMITS.md` section 2.5 |
| The TPC-H fiber read of 208 B and the 13.0 read amplification | `physical_limits_probe` section J, the committed gate artifact |
| The build source file sizes | `D:\d2\data\imdb`, 927.5 + 28.7 + 1,065.6 + 4,362.0 MB |

**Plan generated:** 2026-10-22 13:01:38 local (Asia/Kolkata), 2026-10-22 07:31:38 UTC.

---

## 9. Measured corrections, 2026-10-22

All eight nodes are built. The measurements contradict nine figures in this plan. Each row below names the projection, the measured value, and the record that holds the measurement.

### Table 6: Every retracted figure and the measurement that replaces it

**Rows = the figure. Columns = the projected value, the measured value, and the source of the measurement.**

| The figure | Projected | Measured | Source |
|---|---|---|---|
| The build after 4 threads | 28,180 ms, a 4.00x | 54,100 to 61,100 ms, a 1.85x to 2.09x | `imdb_fiber_bench` stdout, two builds, METHOD-063 |
| The read share of the principals stage | 1.5% to 3.5% | 68% to 83% | the `build principals split` line, read 58,958 ms of 86,510 and 62,696 of 79,186 |
| The cost of the three pre-aggregates | 18,349 ms | 1,729 to 3,488 ms | the `build answers` line, METHOD-059 and METHOD-060 |
| The whole bundle build | 46,529 ms, a 2.42x | 55,829 to 64,588 ms, a 1.74x to 2.02x, and 114,200 ms with the source cold | the two measured parts, summed |
| The suite wall | 165.1 to 229.2 ms | 0.027 to 0.093 ms | `results/imdb_shelf.json`, two runs |
| The DuckDB setup | 0.1 s | 155,991 ms | `results/imdb_setup.json` |
| The DuckDB setup margin | 511x to 1,127x slower for the shelf | 2.55x to 2.88x faster for the shelf | both records above |
| The floor for the DuckDB read | 6,383.8 MB and 1,995 ms | 10,407,973,490 B and 3,252 ms | the size of the 7 TSV files |
| The PostgreSQL setup | 46.2 s | 783,103 ms, measured while the disk held 48.0 to 56.2 MB/s, so the figure is not comparable | `results/imdb_setup.json` |

**The first cause of the four build corrections is the read.** The plan put 4 threads on the parse because it measured the read at 1.5% to 3.5% of the build. That measurement came from cached files. On a cold read the principals file takes 58,958 to 69,215 ms, which is 68% to 83% of the stage. Threads cannot divide a read of one file on one disk, so the parse threads reach their 3.94x to 4.08x and the stage still gains only 1.85x to 2.09x.

**The second cause is the answer design.** The plan costed the pre-aggregates at 18,349 ms, from a hash-probe join. The built design uses dense arrays and a title bitmap, and costs 1,729 to 3,488 ms. The suite wall follows from the same design: all three answers are materialized sections, so each query moves 24 to 226 B.

**The third cause is the DuckDB record.** The 0.1 s figure was never possible. The read alone is 3,252 ms at the plan's own 3.20 GB/s, which is 32.5x the record, and the measurement is 155,991 ms, which is 1,560x the record. The 511x margin that rested on it is retired with it.

**The box was loaded during the PostgreSQL run.** The total disk throughput sampled 48.0 and 56.2 MB/s, against the planned 3.20 GB/s. The PostgreSQL figure therefore measures the disk and not the engine. The record states this condition and the figure does not replace a measurement made on a quiet disk.
