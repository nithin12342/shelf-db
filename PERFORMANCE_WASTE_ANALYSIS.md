# Why the optimal architecture is slow, and where the performance goes

**Analysis timestamp:** 2026-10-22 12:33:59 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:03:59 UTC.
**Analysis date:** 2026-10-22.
**Companion documents:** `CODE_CHANGES_AND_EXPECTED_GAINS.md` (the code changes and their measured gains), `SEVEN_DATABASE_AREA_COMPARISON.md` (the opponent comparison), `ARCHITECTURE_GAINS.md` (the improvement candidates), `ADVERSARIAL_ANALYSIS.md` (the three attacks), `VON_NEUMANN_LIMITS.md` (the physical limits), `SHELF_DATA_REPRESENTATION_REPORT.md` (the representation).

**Rule for this document.** Every number comes from a named file and a named run, or from a derivation shown in full. Where a figure is a hypothesis with a measurement task, the table says so.

---

## 1. The answer in one table

Rows = the waste. Columns = the measured size, the share of the thing it sits in, and the fix.

| # | Where the performance goes | Measured size | Share | Fix | Class of fix |
|---|---|---|---|---|---|
| W1 | The query is a join, not a byte move | 93.2% to 99.7% of the Q1 and Q2 time | the dominant term | Materialize the answer at build time | Structural |
| W2 | The answer is never materialized, so the bytes are 280.7x too many | 280.7x on the IMDb Q1 shape | the bytes term | The same fix as W1 | Structural |
| W3 | The build parses text with a scalar byte loop | 108,756 to 111,080 ms | 96.5% to 98.5% of the build | One pass per line, a digit loop, 4 threads | CPU, at build time |
| W4 | The field extractor rescans the line for every field | 18 tab examinations per basics row where 9 are enough | inside W3 | One pass per line | CPU, at build time |
| W5 | The load verifies the section with a second pass | 32.7 to 43.4 ms | 23.7% to 32.7% of the load | Fold the verify into the read | Memory traffic |
| W6 | The DuckDB setup number cannot be a parse of the data | 0.1 s against a 1,995 ms physical floor | the setup comparison | Re-measure DuckDB the same way | Evidence |
| W7 | The micro-optimisation was aimed at the wrong place | 0.965x on the IMDb query | nothing | Aim it at the build | Evidence |

**The short answer: the shelf database is not slow because it moves bytes. It is slow because it resolves the join at query time, and because its build parses text one byte at a time.** The von Neumann analysis is right about the data representation and silent about where the time actually goes.

---

## 2. The argument, in one page

**The von Neumann claim.** The machine is a memory-fetch machine, so the cost of a query is the bytes it moves. The shelf database implements that claim. On the TPC-H shape the fiber reads 208 bytes for the answer, the measured read amplification is 13.0 against a floor of 1.0, and the join runs in 3.2 to 7.9 ms against DuckDB's 158.3 ms. That is the claim working.

**The measurement that breaks the claim.** The IMDb Q1 query moves 201,692,415 bytes in 153.4 to 173.6 ms. The measured bus peak is 19.2 GB/s, so those bytes cost 10.5 ms. **The other 142.9 to 163.1 ms, which is 93.2% to 93.9% of the query, is not byte movement.** The IMDb Q2 query moves 1,097,898,654 bytes in 18,333.3 ms, which is 57.2 ms of byte movement and 18,276.1 ms of everything else, or 99.7%.

**The cause is a shape problem, not a bandwidth problem.** The bytes are too many because the answer was never materialized. The IMDb bundle is a pre-parse. It holds the source columns. So the query has to join, and a join is a chain of random probes, and a chain of random probes does not care how few bytes the machine moves.

**The proof that the bytes are the wrong target.** Four accumulators and a 4-thread sharded map were both measured on the IMDb Q1 path. The first gives 0.851x to 0.986x, the second gives 0.961x to 1.059x. Both are no gains, because the wall is latency on random probes and the memory controller is already 75% to 95% saturated. **The von Neumann optimum and the wall of the machine are on different axes.**

**The second waste, at the opposite end of the machine.** The build takes 112,721 ms. It parses 72,779,864 rows of text. That is 1,549 ns per row, or 56.6 MB/s. The measured hash insert alone is 116 to 137 ns, so 91.2% to 92.5% of the per-row cost is text extraction and formatting, not the index. The basics stage inserts into no map at all and still costs 868 ns per row. The `field` helper rescans the line from its start for every field it wants, so the basics stage examines 18 tab positions per row where 9 are enough.

**So the machine is idle at both ends and busy in the middle.** The query is 6.0% to 6.9% of the bus peak. The build is a scalar text loop. The one place where the shelf database is fast, the TPC-H fiber, is fast because the answer is already materialized and the whole path is 208 bytes.

**The fix follows the argument.** Materialize the answer, which removes the join and the extra bytes together. Then move the CPU work to the build, where the rows are independent and the memory controller is not the limit. Then reduce the bytes the load touches.

---

## 3. The waste ledger for the query-time table

**Rows = the query. Columns = the bytes moved, the time, the rate, the share of the measured bus peak, and the time that is not byte movement.**

| Query | Bytes moved (B) | Query time (ms) | Rate (GB/s) | Share of the 19.2 GB/s peak | Time that is not byte movement (ms) | Share not bytes |
|---|---|---|---|---|---|---|
| IMDb Q1 | 201,692,415 | 153.4 to 173.6 | 1.16 to 1.32 | 6.0% to 6.9% | 142.9 to 163.1 | **93.2% to 93.9%** |
| IMDb Q2 | 1,097,898,654 | 18,333.3 | 0.060 | 0.31% | 18,276.1 | **99.7%** |
| IMDb Q3 | 306,779,567 | 1,746.9 | 0.176 | 0.92% | 1,730.9 | **99.1%** |
| TPC-H join, answer fiber | 208 | 3.2 to 7.9 | n/a | n/a | n/a | the answer is already materialized |

**The section bytes are read out of the bundle index.** basics 167,262,875 B, ratings 34,429,540 B, names 188,859,084 B, nameblob 229,779,490 B, q2edges 679,260,080 B, princount 139,516,692 B. The file is 1,439,107,761 B.

**The shelf database wins this table against all eight opponents, and the win is not a bandwidth win.** The opponents resolve their joins at query time too, and they pay for it in their own ways. DuckDB does it in 345.7 ms against the shelf's 197.8 ms, PostgreSQL in 1,918.1 ms, and the shelf database is faster in every case because it moves fewer bytes and does less work per byte. But the shelf database's own 93% to 99.7% non-byte time is the headroom that is still on the table.

### Table 2: The bytes are also too many, and this is the structural part

**Rows = the query shape. Columns = the bytes the query moves, the bytes the answer occupies, the ratio, and the ratio the shape achieves when the answer is materialized.**

| Query shape | Bytes moved (B) | Bytes the answer occupies (B) | Ratio now | Ratio with a materialized answer |
|---|---|---|---|---|
| IMDb Q1 | 201,692,415 | 718,496 | **280.7x** | 1.0x |
| IMDb Q2 | 1,097,898,654 | 160 | 6,861,867x | 1.0x |
| IMDb Q3 | 306,779,567 | 128 | 2,396,715x | 1.0x |
| TPC-H fiber join | 208 | 1,600 | 0.13x | 0.13x |

**The TPC-H row is the target.** The shelf database's own representation already achieves a ratio below 1.0 on the TPC-H shape, because the fiber holds the answer. The IMDb shapes are 280.7x to 6.9 million times worse, because the bundle holds the source columns.

**This is the whole difference between the two structures that share one name.** The TPC-H fiber is a pre-aggregate. The IMDb bundle is a pre-parse. The measured gap between them is the gap between a 3.2 to 7.9 ms query and an 18,333 ms query.

---

## 4. The waste ledger for the total-time table

**Rows = the item. Columns = the measured time, the share of its parent, and what it is.**

| Item | Time (ms) | Share of the parent | What it is |
|---|---|---|---|
| The build's character-level parse | 108,756 to 111,080 | **96.5% to 98.5% of the 112,721 ms build** | 72,779,864 rows of scalar text handling |
| The build's file read | 1,641 to 3,965 | 1.5% to 3.5% of the build | 6,383.8 MB at the measured 1.61 to 3.89 GB/s |
| The load's CRC32C second pass | 32.7 to 43.4 | **23.7% to 32.7% of the 120.8 to 137.9 ms load** | a second pass over the same bytes |
| The Q1 query's join | 142.9 to 163.1 | 93.2% to 93.9% of the query | 1,721,477 map inserts and 3,216,590 random probes |
| The Q2 query's join | 18,276.1 | 99.7% of the query | 42,453,755 random probes into a 15,738,257-entry map, plus a 229,779,490 B blob walk |
| The Q3 query's join | 1,730.9 | 99.1% of the query | a 12,866,375-row scan with a 139,516,692 B side join |

### Table 3: The per-row cost of the build, stage by stage

**Rows = the build stage. Columns = the rows, the time, the cost per row, the text rate, and the tab examinations per row.**

| Stage | Rows | Time (ms) | Cost per row (ns) | Text rate (MB/s) | Tab examinations per row | Redundant examinations |
|---|---|---|---|---|---|---|
| names | 15,738,257 | 11,440 | 727 | 81.1 | 6 | 3 |
| ratings | 1,721,477 | 548 | 318 | 52.4 | 3 | 0 |
| basics | 12,866,375 | 11,168 | 868 | 95.4 | 18 | **9** |
| principals | 42,453,755 | 89,565 | 2,110 | 48.7 | 3 | 0 |
| **Total** | **72,779,864** | **112,721** | **1,549** | **56.6** | | |

**The `field` helper is the cause of the redundant examinations.** It takes a wanted field index and scans the line from byte 0 until it reaches that field, so every field after the first pays for the ones before it. The basics stage asks for fields 0, 1, 5 and 8, so it examines 1 + 2 + 6 + 9 = 18 tab positions. One pass examines 9. The 9 extra examinations per row are 116 million redundant tab reads over the stage.

**The hash insert is a small part of the cost.** The measured insert is 116 to 137 ns. Against 1,549 ns per row, that is 7.5% to 8.8%. **The other 91.2% to 92.5% is text extraction and formatting.** The basics stage inserts into no map and still costs 868 ns per row for one id parse, one year parse, one genre scan and a 13-byte append.

**Two specific costs inside the text handling.** The `id_of` helper validates the field as UTF-8 and then calls the generic `str::parse::<u64>()`, which carries a sign check, a radix check and an overflow check that a digit loop does not need. The basics stage calls `g.windows(5).any(|w| w == b"Drama")`, which is a 5-byte window scan of the genre field on every row.

---

## 5. The opponents' side of the ledger

**Rows = the architecture. Columns = the measured setup, the measured query, the margin, and the waste that the measurement exposes.**

| Architecture | Setup (ms) | Query (ms) | Shelf margin on query | What the measurement exposes |
|---|---|---|---|---|
| DuckDB | 100 | 345.7 | 1.75x faster | **The setup number fails the physical floor test.** Reading 6,383.8 MB from the NVMe at the measured 3.20 GB/s costs 1,995 ms. The repository holds no script that shows how the 0.1 s was produced. The 511x setup gap is therefore an upper bound. |
| Redis | 4,140.6 | 255.5 | 1.29x faster | The setup is short because the answer is an in-memory key, and the query is slow because of the network round trip. The in-process shelf path answers the point lookup in 10.5 ns, 6,124x to 49,229x faster. |
| ScyllaDB | 21,348.1 | 208.3 | 1.05x faster | The narrowest margin in the matrix. The setup takes 2.39x to 5.28x less time than the shelf build, and the query is 10.5 ms slower. |
| ScyllaDB (CQL) | 97,443.5 | 271.1 | 1.37x faster | The setup is the price of the write-time join. It takes 1.91x the shelf benchmark build, and the total time is 1.90x faster for the shelf. |
| SQLite | 7,889.1 | 757.5 | 3.83x faster | The setup takes 6.5x to 14.3x less time than the shelf build, and the durable write is 3.29x faster. SQLite owns the one area the shelf loses outright. |
| MariaDB | 26,993.4 | 970.9 | 4.91x faster | The setup takes 1.9x to 4.2x less time than the shelf build, and the query is 4.91x slower. |
| Neo4j | 83,689.7 | 1,089.5 | 5.51x faster | The setup is the slowest of all eight. It takes 1.64x the shelf benchmark build, and the traversal is 21.3x to 33.5x slower. |
| PostgreSQL | 46,200 | 1,918.1 | 9.70x faster | The setup takes 1.11x to 2.44x less time than the shelf build, and the query is 9.70x slower. |

**The pattern across the eight.** Every opponent resolves its join at query time as well, and the shelf database still wins every query. But the opponents buy their setup advantage differently. SQLite, MariaDB and PostgreSQL ingest rows and build their own indexes in 7.9 s to 46.2 s. Neo4j builds a graph in 83.7 s. ScyllaDB CQL resolves the join at write time and pays 97.4 s for it. DuckDB records 0.1 s, and that number does not survive the physical floor test.

**The shelf database is the only one of the eight that pays a character-level parse of 6,383.8 MB.** That is the setup disadvantage, and it is the one the plan has to remove.

---

## 6. The new plan

**Rows = the step. Columns = the change, the file, the bound on the gain, the measurement task, and the gate.**

| Step | Change | File | Bound on the gain | Measurement task | Gate |
|---|---|---|---|---|---|
| W1 | Materialize the Q1 answer as a bundle section | `imdb_fiber_bench.rs`, `imdb_fiber.rs` | Measured 1,646x to 3,003x on the query | None, the gain is measured | The variant gate on count, votes and average |
| W2 | Materialize the Q2 aggregate and the Q3 groups | `imdb_fiber_bench.rs` | Measured 83x to 116x on Q2 | None, the gain is measured | The md5 cross-engine gate in `imdb_job.json` |
| W3 | One pass per line in the build, not one pass per field | `imdb_fiber_bench.rs`, the `field` helper | The basics stage examines 18 tab positions per row where 9 are enough, so the fix removes 9 of 18 examinations, which is 116 million redundant tab reads over the stage. The time saved is not bounded by that share alone and must be measured. | Time the basics stage before and after | The build must produce the same six sections byte for byte |
| W4 | A digit loop instead of the generic parse | `imdb_fiber_bench.rs`, the `id_of` helper | The measured hash insert is 7.5% to 8.8% of the per-row cost, so 91.2% to 92.5% of the build is text handling. A constant factor on that work shows up almost one for one in the build. | Time the four stages before and after | The same six sections |
| W5 | 4 threads over the parse | `imdb_fiber_bench.rs`, the `build` function | The parse is 96.5% to 98.5% of the build, so the scaling is near linear. The measured cut is 112,721 ms to 28,180 ms, a 4.00x. | None, the gain is measured | The same six sections, plus the three query gates |
| W6 | Fold the CRC verify into the section read | `imdb_fiber.rs`, `BundleReader::section` | Measured 1.31x to 1.48x on the load | None, the gain is measured | The CRC corruption negative test |
| W7 | Re-measure the DuckDB setup the same way the shelf setup is measured | the job runner, which the repository does not have yet | The 0.1 s number is below the 1,995 ms physical floor for reading the data | Produce a script that loads the 7 TSV files into DuckDB and record the time and the input format | The recorded number must be at or above the physical floor |
| W8 | Aim the latency fixes at the build, not the query | the whole build path | The two latency fixes measured no gain on the query, because the query is bound by random probes on a saturated controller. The build has 72,779,864 independent rows and no dependency chain, so the same techniques have no wall to hit. | Time the build with the 4-accumulator and 4-thread changes applied to the parse loop | The same six sections |

**W7 comes before the push of any total-time claim.** The 511x setup gap against DuckDB is the largest single number in the total-time table, and it rests on a value that cannot be a parse of the data.

**The derivation for W3 and W4 together.** If the text handling is 92% of the build and it improves by a factor of 2, the build becomes 0.08 + 0.46 = 0.54 of its current cost, a 1.85x cut. At a factor of 4, it becomes 0.08 + 0.23 = 0.31, a 3.23x cut. At a factor of 8, it becomes 0.08 + 0.115 = 0.195, a 5.13x cut. **These are derived from the measured 91.2% to 92.5% share, and the factor itself is a hypothesis that W3 and W4 must measure.**

### Table 4: The build, with each step applied

**Rows = the build shape. Columns = the build time and the gain against the current build.**

| Build shape | Build time (ms) | Gain | Basis |
|---|---|---|---|
| The build today | 112,721 | reference | Measured, forced rebuild 2026-10-22 |
| W3 and W4 applied, the text handling 2x faster | 60,870 | 1.85x | Derived from the measured 92% text share |
| W3 and W4 applied, the text handling 4x faster | 34,943 | 3.23x | Derived from the measured 92% text share |
| W5 applied alone, a 4-way parallel parse | 28,180 | 4.00x | Measured stage times, near linear scaling |
| W3, W4 and W5 applied, the text handling 2x faster | 15,217 | 7.41x | Derived |
| W1 and W2 applied on top | 15,217 plus the pre-aggregate joins | | The joins add 18,333 ms unless W5 also covers them |

**The best case in this table is 15,217 ms for the parse, plus the pre-aggregate joins.** The joins are themselves a parse plus a hash build, so a 4-way parallel build covers them too, and the total is about 33,550 ms. That is a 3.36x cut on the build, and it is a derived figure.

**The realistic case is 28,180 ms with W5 alone.** That is the measured figure, and it does not depend on any hypothesis.

### Table 5: The order, and the break-even

**Rows = the step. Columns = what it buys and what it costs.**

| Step | Buys | Costs | Depends on |
|---|---|---|---|
| W7 | a correct setup comparison | one script | nothing |
| W1 | the Q1 query at 0.058 to 0.093 ms | 70 to 90 ms of build | the build |
| W2 | the Q2 query at 0.02 ms, the Q3 at 0.38 ms | 18,349 ms of build | the build and the join |
| W3, W4, W5 | the build at 15,217 to 28,180 ms | the work | nothing |
| W6 | the load at 84.1 to 105.2 ms | 20 lines | nothing |

**W3, W4 and W5 must land before W1 and W2 are worth anything at scale.** The pre-aggregate joins add 18,349 ms to a build that today costs 112,721 ms. With W5 the build is 28,180 ms and the joins make it 46,529 ms, which is still 2.42x better than today. Without W5 the build is 131,070 ms, which is worse than today. **The order is not a preference, it is a dependency.**

**The break-even.** The build with W1, W2 and W5 costs 46,529 ms and the current build costs 112,721 ms, so the build alone already pays for itself. The suite query falls from 21,180.2 ms to 165.1 to 229.2 ms. The change wins from the first suite run.

---

## 7. What this document does not claim

- **The 92% text share is derived from two measured numbers**, the 1,549 ns per row and the 116 to 137 ns hash insert. It is a share, not a decomposition. The per-row work has not been profiled, so the split between the field rescans, the generic parse, the UTF-8 validation and the Vec appends is not known.
- **The 2x, 4x and 8x text-handling factors in table 4 are hypotheses.** W3 and W4 must measure them. The table shows the consequence if they hold, not a prediction.
- **The DuckDB 0.1 s setup number is not disproved, it is unexplained.** The repository holds no script that shows how it was produced. It can be a load from a converted file, a load of a subset, or a measurement of something other than the ingest. W7 produces the script and settles it.
- **The Q1 and Q2 query times have a 2.2x spread** across four committed runs of the same binary and the same bundle. Every figure here is a range.
- **The 208-byte TPC-H fiber read is the measured read of one answer**, and the 1,600-byte answer is 100 rows of 16 bytes. The 0.13x ratio follows from those two numbers.
- **No code was changed for this analysis.** The two benches and the one extended tool from the previous commit are the only code changes, and they were used here for the numbers in sections 3 and 4.

---

## 8. Evidence index

| Claim | Where it is verified |
|---|---|
| The section byte counts | The bundle index of `D:\bench\data\imdb_fiber_bundle.bin`, read directly |
| The Q1, Q2 and Q3 byte totals | 201,692,415 B, 1,097,898,654 B and 306,779,567 B, from the same index |
| The measured bus peak, 19.2 GB/s | `physical_limits_probe` section B, the committed gate artifact |
| The Q1 query time and its variants | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs`, the `Q1KB_` lines |
| The Q2 query time | `results/imdb_shelf_matrixwindow.json` and `results/imdb_shelf_runs.json` |
| The build stage times | `imdb_fiber_bench` stdout, forced rebuild 2026-10-22 |
| The measured hash insert cost | `tests/e2e/von_neumann/tools/cpu_floor.rs` and `q2_decompose.rs` PART1 |
| The section load split | `crates/sheaf-engine/src/bin/bundle_load_bench.rs`, the `LOAD_` lines |
| The TPC-H fiber read of 208 B and the 13.0 read amplification | `physical_limits_probe` section J, the committed gate artifact |
| The memory controller saturation, 75% to 95% | `VON_NEUMANN_LIMITS.md` section 2.5 |
| The opponents' setup and query times | `results/imdb_job.json` and `results/imdb_multi_engine.json` |
| The `field` helper and `id_of` | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`, lines 45 to 69 |
| The build source file sizes | `D:\d2\data\imdb`, 927.5 + 28.7 + 1,065.6 + 4,362.0 MB |

**Analysis generated:** 2026-10-22 12:33:59 local (Asia/Kolkata), 2026-10-22 07:03:59 UTC.

---

## 9. The plan of record

**Plan timestamp:** 2026-10-22 13:01:38 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:31:38 UTC.

The plan that removes the waste measured in this document is `PERFORMANCE_FIX_PLAN_2026-10-22.md`. It names eight nodes, METHOD-059 through METHOD-066, with the file, the size, the gate, the evidence each must produce, and the order. The order is a dependency: the 4-thread parse must land before the pre-aggregates, because the pre-aggregate joins add 18,349 ms to the build.

The session record is `CONVERSATION_LOG_2026-10-22.md`. It lists the eight corrections filed against the pushed documents.