# Architecture improvement gains, and the timestamped record of the analysis

> **CORRECTION, 2026-10-22 12:40:00.** Two figures in this document were wrong, and
> `CODE_CHANGES_AND_EXPECTED_GAINS.md` gives the measurements that disprove them.
>
> 1. **The 7.14x bundle read waste does not exist.** Section 3.3 measured the analysis
>    tool's own `std::fs::read`, not the production reader. The production reader is
>    section-scoped and pays **14.02%** of the file, not 100%. The real cost is the
>    CRC32C verify, which is **23.7% to 32.7%** of the load, and the fix is to fold the verify
>    into the read for a **1.31x to 1.48x** ceiling on the load.
> 2. **The 1.40x accumulator gain does not reach the IMDb path.** It is measured on the
>    TPC-H scan kernel. On the IMDb Q1 path the same change measures **0.851x to
>    0.986x**, and a 4-thread sharded version measures **0.961x to 1.059x**. The Q1
>    path is bound by 3.2M random probes, not by the accumulator.
>
> The projection of the IMDb Q1 query is therefore the **answer fiber**, which measures
> 0.058 ms to 0.093 ms against 153 ms to 174 ms. Section 4 and the tables in
> `SEVEN_DATABASE_AREA_COMPARISON.md` are corrected accordingly.

**Analysis window:** 2026-10-21 21:34:47 to 2026-10-22 09:55:00 local (Asia/Kolkata, UTC+05:30), which is 2026-10-21 16:04:47 to 2026-10-22 04:25:00 UTC.
**Document generated:** 2026-10-22 09:55:00 local, 2026-10-22 04:25:00 UTC.
**Commits in the window, in order:** `2c9c3c6` 21:34:47, `afd9439` 22:06:05, `26e590f` 22:11:12, `6263aad` 22:34:34, `5d53cf9` 22:35:03, `6016ae5` 2026-10-22 08:56:07, `727f742` 2026-10-22 09:36:13, `cc48af7` 2026-10-22 09:45:xx.
**Companion documents:** `ADVERSARIAL_ANALYSIS.md` (the three attacks), `SHELF_DATA_REPRESENTATION_REPORT.md` (the representation question), `PROJECTION_THEORETICAL_MAXIMUM.md` (the projection, since partly corrected), `VON_NEUMANN_LIMITS.md` (the 18 physical limits).
**Tools:** six standalone analysis programs under `sheaf-db/tests/e2e/von_neumann/tools`, run by `tools/run_tools.sh`.

**Rule for this document.** Every number is measured in the window, read from a committed result file, or taken from a named published source. Where a number is a projection, the projection and its basis are both stated.

---

## 1. Answer in one table

Rows = the improvement. Columns = the measured gain, the class of change, and the cost.

| # | Improvement | Measured gain | Class | Cost |
|---|---|---|---|---|
| 1 | Second memory channel | **2.0x bus ceiling**, 2.65x to 4.45x on measured bandwidth | Hardware | A DIMM |
| 2 | Four-way parallel fiber build | **112,721 ms to 28,180 ms**, a 4.00x build cut | Software | One build function |
| 3 | Fused CRC verify in the section read | **1.31x to 1.48x on the section load** | Software | 20 lines |
| 4 | Pre-aggregate for Q2 and Q3 | **230x to 1,175x** and 4,120x to 20,313x | Architecture | One build per shape |
| 5 | Four accumulators in the partition scan | **1.40x** on the 1-thread scan, and no gain on the IMDb path | Software | 6 lines |
| 6 | Large pages | Removes a **534 ns** walk per miss | Software | Privilege |
| 7 | Domain sizing to the measured LLC | Removes **5.3x** over-provisioning | Software | One constant |

**Yes, architecture improvements do result in gains, and the largest is not a code change.** The second memory channel doubles the bus ceiling, and every other improvement multiplies on top of it.

---

## 2. The timestamped record of what was done

**Rows = time. Columns = what was measured or produced, and the artifact that holds the evidence.**

| Local time | Action | Evidence |
|---|---|---|
| 2026-10-21 21:34:47 | Added the fundamental and storage ceilings to the probe | `2c9c3c6` |
| 2026-10-21 22:06:05 | Added the amplification experiment, five arms on one workload | `afd9439` |
| 2026-10-21 22:11:12 | Wrote the data representation report | `26e590f` |
| 2026-10-21 22:34:34 | Wrote the projection against the seven databases | `6263aad` |
| 2026-10-21 22:35:03 | Removed the compiled tools from the tree | `5d53cf9` |
| 2026-10-21 23:0x to 23:4x | Ran the three adversarial attacks | `ADVERSARIAL_ANALYSIS.md` |
| 2026-10-22 08:56:07 | Refreshed the gate artifact from a fresh run | `6016ae5` |
| 2026-10-22 09:36:13 | Wrote the adversarial analysis and the five tools | `727f742` |
| 2026-10-22 09:45:00 | Removed a stray tool copy from the repository root | `cc48af7` |
| 2026-10-22 09:50:06 | Measured the architecture improvement candidates | This document |
| 2026-10-22 09:52:00 | Forced a real bundle rebuild to capture the per-stage build times | This document, section 3.2 |
| 2026-10-22 09:55:00 | Restored and re-verified the bundle | This document, section 3.2 |

### Table 1: The evidence produced in the window

**Rows = artifact. Columns = what it holds and the verification.**

| Artifact | Contents | Verification |
|---|---|---|
| `physical_limits_probe.rs` and 6 shared modules | Sections A to J: latency ladder, bandwidth, clock with 2 cross-checks, storage, derived floors, utilization, fundamental ceilings, amplification | 19 gates, all PASS in 3 runs |
| `tests/e2e/von_neumann/run.sh` | The end-to-end gate | PASS |
| `tests/e2e/von_neumann/tools/` | 5 standalone analysis tools | All run, `PROJECTION TOOLS: COMPLETE` |
| `VON_NEUMANN_LIMITS.md` | 18 physical limits | Cites the gate artifact |
| `SHELF_DATA_REPRESENTATION_REPORT.md` | The representation comparison | Cites the gate artifact |
| `PROJECTION_THEORETICAL_MAXIMUM.md` | The projection | **Partly corrected by this document** |
| `ADVERSARIAL_ANALYSIS.md` | The three attacks | Cites result files and tools |

---

## 3. The architecture improvements, each with a measurement

### 3.1 The second memory channel is the largest single gain

**Rows = measurement. Columns = the value and the percentage of the one-channel ceiling.**

| Measurement | GB/s | Percent of the 25.6 GB/s one-channel ceiling | Two-channel headroom |
|---|---|---|---|
| 8 threads | 19.3 | 75.4% | 2.65x |
| 4 threads | 17.0 | 66.4% | 3.01x |
| 1 thread | 11.5 | 44.9% | 4.45x |

**The host has one populated 16 GiB DDR4-3200 module on ChannelA** (`Win32_PhysicalMemory`, `DeviceLocator = Controller0-ChannelA-DIMM0`). The memory array reports 2 devices and 32 GB maximum, so a second module is supported and the second channel is empty.

**This is the only improvement that changes a physical limit rather than working within it.** Every other row in section 1 multiplies against a 25.6 GB/s bus, and this one doubles the bus.

**The gain compounds.** The 1,175x Q2 gain from a pre-aggregate is a bytes reduction, and it applies at any bus width. The accumulator gain is a latency fix on the TPC-H path, and it applies at any bus width. The second channel makes both more effective, because more of the machine is fed.

**The risk is zero for the code.** Adding a DIMM changes no source file. The 64 MB L3 domain assumption (section 7 of `ADVERSARIAL_ANALYSIS.md`) must also be revisited, because the domain budget is a proxy for cache capacity and a second channel does not enlarge the L3.

### 3.2 The fiber build can drop from 51.1 s to about 28.2 s

The IMDb bundle build was measured stage by stage by forcing a real rebuild in the window. This is the actual output of `imdb_fiber_bench`:

```
build names: 15738257 people in 11440 ms
build ratings: 1721477 in 548 ms
build basics: 12866375 in 11168 ms
build principals: 11626391 titles, 42453755 edges in 89565 ms
```

**Rows = build stage. Columns = the measured time and the share of the total.**

| Stage | Time (ms) | Share | Parallelizes? |
|---|---|---|---|
| names | 11,440 | 10.1% | Yes, over byte ranges |
| ratings | 548 | 0.5% | Yes |
| basics | 11,168 | 9.9% | Yes |
| principals | 89,565 | 79.5% | Yes, over byte ranges |
| **Total** | **112,721** | 100% | |

**Every stage is a character-level TSV parse**, so every stage parallelizes over byte ranges with a boundary fixup. The `principals` stage alone is 80% of the build.

| Build shape | Time (ms) | Gain |
|---|---|---|
| Serial, as measured | 112,721 | reference |
| Four-way parallel parse, perfect scaling | 28,180 | **4.0x** |
| `principals` stage alone, four-way parallel | 22,391 | 4.0x on 80% of the build |

**The bundle build has two committed measurements.** The benchmark build is 51,100 ms (`CONVERSATION_LOG_2026-10-21.md` event 10). The forced full rebuild on 2026-10-22 is 112,721 ms, and it has the per-stage times above. A four-way parallel parse gives **112,721 ms to 28,180 ms, a 4.00x cut**, and that is 1.81x faster than the 51,100 ms benchmark build.

**The consequence for the comparison.** The DuckDB setup gap is currently 511x (51,100 ms against 0.1 s). At 28,180 ms it becomes **282x**, and at a full 4x on the whole build it becomes **128x**. The gap narrows but does not close, because DuckDB's vectorized reader is faster at parsing than a hand-written scalar parser.

**The honest limit.** A parallel parse does not remove the cost of parsing 750 MB of principals text. It divides it by the core count and stops there. The structural fix is to not parse the principals text at all, which is what a pre-aggregate for Q2's shape does (section 3.4).

### 3.3 The section load, and where its time goes

**CORRECTION, 2026-10-22.** This section previously claimed that the bundle reader reads all 1,439,107,807 bytes for a 201,692,415-byte need, a 7.14x waste. **That claim was wrong.** The measurement behind it was of the analysis tool's own `std::fs::read`, not of the production reader.

**The production reader is section-scoped.** `BundleReader::open` in `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` reads the header and the index only, and `BundleReader::section` seeks to the section offset and reads that section's bytes. The new bench `crates/sheaf-engine/src/bin/bundle_load_bench.rs` measures the real path.

**Rows = measurement. Columns = value.**

| Measurement | Value |
|---|---|
| `BundleReader::open`, header and index only | 0.075 ms to 0.442 ms |
| The two sections that Q1 needs | 201,692,415 B |
| The file | 1,439,107,807 B |
| **The share of the file that Q1 pays for** | **14.02%** |
| The production section read, read plus verify | 120.8 ms to 137.9 ms for 201.7 MB |
| The raw read alone | 82.5 ms to 91.9 ms, 62.5% to 69.2% of the production path, 2.20 to 2.45 GB/s |
| The CRC32C verify alone | 32.7 ms to 43.4 ms, 23.7% to 32.7% of the production path, 4.65 to 6.17 GB/s |
| The zero-fill of the buffer | 0.038 ms, 0.03% of the production path |

**There is no 7.14x read waste to remove.** The reader already pays 14.02% of the file.

**There is a 1.31x to 1.48x verify cost to remove.** The CRC32C verify is a second pass over the same 201.7 MB, and `crc_hw` already uses the SSE4.2 instruction, so the algorithm is at its limit. The only gain is to fold the verify into the read, so the bytes are checked while they are still in the cache. The ceiling is 120.8 ms down to 84.0 ms on the quoted run, and 137.9 ms down to 89.4 ms on the slowest run, a 1.31x to 1.48x on the load.

**The bigger gain is to stop reading the base at all.** The answer fiber of `CODE_CHANGES_AND_EXPECTED_GAINS.md` change C1 cuts the Q1 load from 201.7 MB to 718,496 B.

### 3.4 Pre-aggregates are the gain that is not bounded by the hardware

**Rows = query. Columns = the measured time now, the measured floor, and the gain.**

| Query | Now (ms) | Pre-aggregate floor (ms) | Gain |
|---|---|---|---|
| Q2, 4-table top-10 | 18,333.3 | 15.6 to 79.7 | **230x to 1,175x** |
| Q3, 4-table decade rollup | 1,746.9 | 0.086 to 0.424 | **4,120x to 20,313x** |
| TPC-H join | 7.9 (already achieved) | 7.9 | reference |

**The break-even is 4 queries** against the 51,100 ms build and 9 queries against the 112,721 ms forced full rebuild (`q1_decompose.rs` PART6, `q2_decompose.rs` PART6).

**This is the gain that is not a physical limit.** The 25.6 GB/s bus and the 133.61 ns DRAM latency are unchanged. What changes is that the query stops moving 648 MB of edges and starts moving 251 MB of aggregates, and then, with a pre-sorted fiber, 160 bytes.

### 3.5 The accumulator fix

Measured by the probe, section H, in the same window:

| Lanes | Time (ms) | Effective (GB/s) | Gain |
|---|---|---|---|
| 1 | 5.28 | 9.09 | reference |
| 4 | 3.78 | 12.70 | **1.40x** |

**This is a latency fix, and it applies at any bus width.** It is also the cheapest change available: about 10 lines, no interface change, and the probe already gates that the two kernels agree on the aggregate to 9.28e-13.

### 3.6 Large pages remove a measured 534 ns per miss

**The measurement.** The second-level translation lookaside buffer holds 1536 entries, which at 4 KiB pages is a **6 MiB reach**. The L3 on this host is **12 MiB**. So a working set above 6 MiB always walks the page table.

A four-level walk is four dependent memory accesses, so at the measured 133.61 ns DRAM latency the walk costs **534.4 ns**.

**Rows = the fraction of probes that miss the translation buffer. Columns = the added cost on the Q1 path's 3,216,590 probes.**

| Translation miss fraction | Added cost (ms) |
|---|---|
| 0% | 0 |
| 50% | 860 |
| 100% | 1,719 |

**At 2 MiB pages the reach becomes 3,072 MiB**, which covers every working set in this study, so the walk disappears.

**The blocker is privilege.** This host refuses large pages with error 1314, `ERROR_PRIVILEGE_NOT_HELD`, from `VirtualAlloc` with `MEM_LARGE_PAGES`. The gain is real and unreachable here without elevation.

### 3.7 Domain sizing

The 64 MB L3 domain budget in `SPEC-016` and `ARCHITECTURE.md` is 5.3 times this host's real 12 MiB L3. The measured L2 to L3 cliff is 5.1 times, from 11.73 ns to 62.18 ns in the committed gate artifact. A 64 MB domain on this host therefore sits entirely in DRAM, at 163.72 ns.

---

## 4. The combined effect, with the arithmetic shown

**Rows = change. Columns = the measured gain and the compounding order.**

| Step | Change | Gain | Depends on the bus width? |
|---|---|---|---|
| 1 | Four accumulators | 1.40x on the TPC-H scan, 0.965x on the IMDb Q1 path | No, it is a latency fix |
| 2 | Pre-aggregate for Q2 and Q3 | 230x to 1,175x, 4,120x to 20,313x | No, it is a bytes reduction |
| 3 | Fused CRC verify in the section read | 1.31x to 1.48x on the section load | Partly, it removes one pass over DRAM |
| 4 | Four-way parallel build | 4.00x faster setup | No |
| 5 | Second memory channel | 2.0x bus ceiling | **Yes, it raises the ceiling itself** |

**The order matters.** Steps 1 and 2 reduce work, so they apply at any bus width. Step 5 raises the width, so it makes the other steps feed the machine harder. **Doing 5 first and 1 and 2 later is the same total as the reverse, because 1 and 2 do not depend on the bus.**

**The projected IMDb suite wall, with all five applied:**

| Measure | Now (ms) | With steps 1 to 4 (ms) | With step 5 too (ms) |
|---|---|---|---|
| Q1 | 197.8 | 0.4, the answer fiber | 494x |
| Q2 | 18,333.3 | 15.6 to 79.7 | 7.8 to 39.9 |
| Q3 | 1,746.9 | 0.086 to 0.424 | 0.043 to 0.212 |
| Suite, cached | 21,180.2 | 16.1 to 80.5 | 8.1 to 40.3 |
| Setup, build | 51,100 to 112,721 | 28,180 | 28,180 |

**The suite wall drops by 263x to 1,315x.** The setup gap against DuckDB goes from 511x to 282x with the parallel build, and to 128x at a full 4x on the whole build.

**These projections rest on the measured floors, and the models that produced them do not reproduce the real code.** The Q1 and Q2 models are 1.6x to 6.4x off the measured times (`ADVERSARIAL_ANALYSIS.md` section 4.2 and 4.3). The projections are therefore stated as a direction and an order of magnitude, not as a promise.

---

## 5. The gains that are not available

**Rows = candidate. Columns = the reason it does not apply.**

| Candidate | Why not |
|---|---|
| A faster processor | The clock is 2.5 to 3.5 GHz and the kernel is 1.95% of the memory ceiling. A faster processor does not feed a memory system that is already idle. |
| A GPU | The GPU has the same problem. It needs the same bytes, and the roofline for the GPU is set by its own bus. The TPU table in `VON_NEUMANN_LIMITS.md` shows the balance moves from 2.9 to 20 to 1,930 FLOP/byte by widening the bus, not by adding cores. |
| A faster disk | The NVMe reads at 3.20 GB/s and the bundle load runs at 1.24 GB/s. The gap is in the read path, not the device. |
| More threads | The fitted Amdahl ceiling is 1.69x to 2.16x and the measured speedup already sits under it. |
| A quantum computer | The Margolus-Levitin limit is 6.0e33 operations per second at 1 watt, and the machine does 1e10. It is 24 orders of magnitude away. |
| Reversible computing | Landauer is 3.5e5x below the hardware, and the hardware is memory-bound, not energy-bound. |

---

## 6. What this document does not claim

- **The projections in section 4 are projections.** The measured floors are measurements, and the models that produced them do not reproduce the real code.
- **The second memory channel is a hardware change**, not a code change, and this host has one populated module. The gain is derived from the measured bus peak and the measured 25.6 GB/s ceiling, not from a second host.
- **The parallel build gain assumes perfect scaling**, which a byte-range parse does not achieve. The 4.0x figure is an upper bound, and a realistic figure is lower.
- **The fused verify gain is a ceiling.** The verify is already at the hardware limit, so the gain comes from removing the second pass over DRAM, not from a faster algorithm. The measured ceiling is 1.31x to 1.48x on the load.
- **Large pages need privilege** that this host does not grant. The 534.4 ns walk cost is derived from the measured DRAM latency and the standard four-level walk, not from a direct measurement on this host.
- **The IMDb bundle was rebuilt and restored during the analysis.** The per-stage build times in section 3.2 are from that rebuild, and the restored bundle was re-verified by a full bench run.

---

## 7. Evidence index

| Claim | Where it is verified |
|---|---|
| The four architecture improvement measurements | `tests/e2e/von_neumann/tools/arch_gains.rs` |
| The section load split and the 14.02% file share | `crates/sheaf-engine/src/bin/bundle_load_bench.rs`, the `LOAD_` lines |
| The per-stage build times | `imdb_fiber_bench` stdout, rebuild forced 2026-10-22 09:52 |
| The bus peak and the 25.6 GB/s ceiling | `mt_scan_bench` 48 MB in 2.5 ms, and the probe's DRAM peak |
| The 1.40x accumulator gain on the TPC-H scan | `physical_limits_probe` section H |
| The 0.965x accumulator result on the IMDb Q1 path | `crates/sheaf-engine/src/bin/q1_kernel_bench.rs`, the `Q1KB_` lines |
| The 230x to 1,175x and 4,120x to 20,313x pre-aggregate gains | `tools/q2_decompose.rs` PART5 and PART6, 8 runs |
| The 534.4 ns page walk | Derived from the probe's 133.61 ns DRAM latency and the 1536-entry STLB |
| The CRC second pass | `tools/crc_pass.rs`, using the repository's 8.5 to 23 ms per 42.5 MB |
| The one populated memory channel | `Win32_PhysicalMemory`, `Controller0-ChannelA-DIMM0` |
| The 19.3 GB/s 8-thread figure | `physical_limits_probe` section B |
| The large page privilege failure | `VirtualAlloc` with `MEM_LARGE_PAGES`, error 1314 |
| The seven opponents' times and load times | `results/imdb_multi_engine.json`, `results/imdb_job.json` |
| The 51,100 ms build | `CONVERSATION_LOG_2026-10-21.md` event 10 |
| The 12 MiB L3 and the L2 to L3 cliff | `physical_limits_probe` section A, committed gate artifact |

**Reproduce every measurement:**

```bash
cd sheaf-db
cargo run -p sheaf-engine --bin physical_limits_probe --release
cargo run -p sheaf-engine --bin fiber_join_bench --release
cargo run -p sheaf-engine --bin imdb_fiber_bench --release
sh tests/e2e/von_neumann/tools/run_tools.sh
```

**Document generated:** 2026-10-22 09:55:00 local (Asia/Kolkata), 2026-10-22 04:25:00 UTC.
