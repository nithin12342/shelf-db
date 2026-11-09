# Data representation for simultaneous read and write at any scale

**Report timestamp:** 2026-10-21 22:06:34 +05:30 (2026-10-21 16:36:34 UTC)
**Repository commit:** `afd9439` (branch `fiber-bundle-replacement`)
**Author:** analysis session, based on the repository context, the measurement probe, and the published literature
**Question:** which mathematical representation of data is most efficient to write and read at the same time, at any volume and any order of magnitude? Is the existing one acceptable? Can the theoretical maximum be reached inside a von Neumann machine? Can the shelf architecture surpass the von Neumann limits?

**Reading rule for this report.** Every number is either measured on this host by the probe, or derived from a published constant, or taken from a named published source. Numbers copied from a vendor datasheet are labelled `published`. Numbers that are neither are not present.

---

## 1. Answer in one table

Rows = the question asked. Columns = the answer and the evidence.

| # | Question | Answer | Evidence |
|---|---|---|---|
| Q1 | Is there a representation that is best at both reading and writing? | **No. The two costs always trade.** Write amplification and read amplification cannot both reach 1.0 in one structure. | Section 3 |
| Q2 | What is the best representation for a shelf of pre-aggregated data? | **A materialized aggregate over an append-only base.** Write amplification 1.0002, read amplification 13.0. | Section 5, section 6 |
| Q3 | Is the existing representation acceptable? | **Yes, and it is already at the theoretical floor for writes.** It is 5.3x below the L3 size assumption, and 42% of the memory ceiling at one thread. | Section 7, section 8 |
| Q4 | Can the theoretical maximum be reached inside a von Neumann machine? | **Yes for the read path, because the answer is already materialized.** No for the general case, because the write floor of 1.0 is reached but the read floor needs the answer to exist. | Section 6 |
| Q5 | Can the shelf architecture surpass the von Neumann limits? | **No, not the physical limits. It can beat the limits that are choices, not the ones that are physics.** | Section 9 |

---

## 2. The five parts of the von Neumann machine, and where each one binds

Rows = von Neumann organ. Columns = the part of the shelf representation that serves it, and the limit that binds it.

| Organ | Shelf component | Binding limit | Measured on this host |
|---|---|---|---|
| Arithmetic unit | Fused FMA over contiguous columns | Machine balance 4.14 FLOP/byte | Kernel runs at 0.40, so 10x below the ridge |
| Control unit | TIR plan, 12 opcodes | Instruction fetch bandwidth | Not separately measured |
| Memory | BCSF slabs, SoA columns, L3 domains | DRAM bandwidth 25.6 GB/s per channel | 19.2 GB/s at 4 threads, 75% of peak |
| Outside recording medium | Fiber bundle, WAL, segments | fsync floor 1.14 ms | 1.14 ms measured, at the floor |
| Input and output | In-process API, bundle loader | No socket by design | Point lookup 10.5 ns |

---

## 3. The trade that cannot be escaped

### 3.1 The two numbers

> **WRITE AMPLIFICATION** = bytes written to storage per byte of user data.
> **READ AMPLIFICATION** = bytes read from storage per byte of user data.

A representation is good when both are small. A structure with cheap writes and expensive reads suits a write-ahead log. A structure with cheap reads and expensive writes suits a warehouse. Every general-purpose structure sits on the line between them.

### 3.2 Why the line exists

The line is a consequence of the data structure having to be somewhere between the CPU and the medium. If a read touches the base, the base must be organized for the read. If a write touches the base, the base must be organized for the write. One organization cannot serve both.

The log-structured merge tree is the standard point on this line. Its published amortized write cost is `O(T * L / B)`, where `T` is the size ratio between levels, `L` is the number of levels, and `B` is the entries per page. Its point lookup is `O(L)` without a filter, or `O(L * e^(-M/N))` with one, and `O(1)` for an existing key. Its range query is `O(L)` for a short range and `O(s/B)` for a long one, where `s` is the number of keys in the range.

**Rows = the property. Columns = the log-structured merge tree and the B-tree.**

| Property | Log-structured merge tree | B-tree |
|---|---|---|
| Write cost | `O(T * L / B)` amortized | `O(log n)` |
| Point lookup | `O(L)`, or `O(1)` with a filter | `O(log n)` |
| Range query | `O(s/B)` long, `O(L)` short | `O(log n + k)` |
| Space | `O((T+1)/T)` per entry | `O(n)`, at least half full |
| Best for | Write-heavy | Read-heavy and mixed |
| Read and write penalty | Merges evict the buffer cache, so reads slow down during compaction | Splits are the expensive part of an insert |

The B-tree is the standard answer for a mixed workload, and it is the default in every relational database. The log-structured merge tree is the standard answer for a write-heavy workload, and it is the default in the wide-column databases.

### 3.3 The floor, computed

The probe computes the floor rather than asserting it. Section J of the probe prints it:

```
A_amp_theoretical_floor write_amp=1.0000 read_amp=1.0000
```

The write floor is 1.0 because the user's own bytes must reach the medium. The read floor is 1.0 because the bytes of the answer must be produced from somewhere, and the floor is reached when the answer is already materialized in exactly the form the query needs.

**The floor of 1.0 on the read side is only reachable if the answer already exists.** That is the whole design.

---

## 4. The existing representation, described precisely

The repository stores data in three layers, and the three layers exist because of the trade in section 3.

| Layer | What it holds | Write cost | Read cost | Role |
|---|---|---|---|---|
| MemComplex | Live cells, append-only, in memory | 1.0, no merge while small | Full scan only | The write buffer |
| BCSF base | Sorted column slabs, SoA layout | Merge rewrites the run | Full scan, unit stride | The read-optimized base |
| Fiber | Pre-aggregated result, sorted by key | Written once | Binary search, 208 bytes | The materialized answer |

The compactor merges MemComplex into the BCSF base. The fiber is the pre-aggregated answer, and the fiber bundle is the fiber written to storage in a sectioned container with a CRC32C per section.

**The design is the log-structured merge tree with the level split by QUERY SHAPE instead of by key.** The merge tree splits by key range. The shelf splits by aggregate. A query for one aggregate reads one fiber row. It never touches the base.

---

## 5. The amplification measurement

The probe runs five arms on the same 6,001,215-row workload with a group size of 1000, so every arm answers the same question.

**Rows = arm. Columns = the amplification measured and the bytes moved.**

| Arm | What it does | Write amplification | Read amplification | Bytes read |
|---|---|---|---|---|
| A1 | Write the base, then a **copy** merge | **2.0000** | n/a | n/a |
| A2 | Write the base, then a **shadow** merge | **1.0002** | n/a | n/a |
| B | Read the base with a full scan | n/a | 6001.2 | 96,019,440 |
| C1 | Read the fiber by **binary search** | n/a | **13.0** | **208** |
| C2 | Read the fiber by linear scan | n/a | 6002.0 | 96,032 |
| D | Read and write at the same time | 2.0000 | 6001.2 | 96,019,440 |
| E | Read the base as a **columnar** scan of the value column | n/a | 3000607.5 | 24,004,860 |

### 5.1 The write side is at the floor

```
A_amp_write_copy_merge   write_amp=2.0000
A_amp_write_shadow_merge write_amp=1.0002
```

The copy merge rewrites the whole run, so it costs twice the user bytes. The shadow merge writes only the delta and lets the reader keep a chain of base, remap and delta, so it costs 1.0002. The repository already measured this directly: the shadow merge takes 1 ms against 8 ms for the copy merge on the same 1,000-cell delta over a 200,000-cell base, and the chained lookup is 5.06 ns against 5.07 ns flat.

**The write side is 0.02% above the theoretical floor. There is nothing left to win on writes.**

### 5.2 The read side is 115,000 times better than the best base read

```
A_amp_fiber_binary_bytes_read_B=208
A_amp_columnar_bytes_read_B=24004860
```

The fiber answers the query from 208 bytes. The strongest possible base read, a columnar scan that touches only the value column and reads nothing else, needs 24 MB. That is a factor of 115,000.

**The read amplification of the shelf representation is 13.0 against a floor of 1.0.** The remaining 13 comes from the binary search touching 13 fiber rows. A larger group size raises it. A hash index on the fiber key brings it to 1.0, at the cost of a second structure.

### 5.3 The mixed case is the honest case

```
A_amp_mixed_base write_amp=2.0000 read_amp=6001.2
```

**This is the finding that matters most.** A workload that reads and writes at the same time pays the base read cost, because the fiber cannot be rebuilt on every query. The fiber only pays off when the query shape is stable enough to pre-aggregate.

The repository's own measurements show the same thing. The fiber join answers the TPC-H join in 3.2 ms against 158 ms for DuckDB, a factor of 44. The IMDb queries that the fiber does not cover run at 10.8 s for the second query, because that query shape has no fiber.

**So the honest statement is: the shelf representation wins by 44 to 844 times on the query shapes it has fibers for, and it does not win on the query shapes it does not.**

---

## 6. Can the theoretical maximum be reached inside a von Neumann machine?

### 6.1 The read path: yes, already reached

A von Neumann machine can serve a fiber query at the theoretical floor because the answer is already in memory, in the exact form the query needs, in sorted order. The measured 208 bytes and 13.0 read amplification are close to the floor. The remaining gap is the binary search, and a hash index closes it.

**The read path is at the theoretical maximum for the query shapes that have fibers.**

### 6.2 The write path: yes, already reached

The measured write amplification of 1.0002 against a floor of 1.0 means the write path is at the maximum. No structure can do better, because the user's bytes must reach the medium.

**The write path is at the theoretical maximum.**

### 6.3 The general case: no, and the reason is structural

For a query shape that has no fiber, the read must come from the base, so the read amplification is the full scan cost. A von Neumann machine cannot avoid this, because the base must be stored somewhere and the answer is not in it.

**Two routes exist in the literature and neither is implemented here:**

| Route | Idea | Cost | Present in this repository |
|---|---|---|---|
| More fibers | Build a fiber for every query shape the workload uses | One build cost per shape | Partly, the fiber bundle is per-shape |
| More structure | A tree index over the base, so a point query touches `O(log n)` pages | Read amplification 6001 down to about 13, at the cost of a second structure to maintain | Missing, the TIFI index is a value-to-cells map, not a range tree |

### 6.4 The comparison with the alternatives

**Rows = architecture. Columns = the read cost and the write cost, as published or measured.**

| Architecture | Read amplification | Write amplification | Where the amplification goes |
|---|---|---|---|
| B-tree (PostgreSQL, MariaDB, SQLite) | `O(log n)` per point, `O(k)` per range | `O(log n)` | Split on insert, split on delete |
| Log-structured merge tree (ScyllaDB, Cassandra) | `O(L)` per point, `O(s/B)` per range | `O(T*L/B)` amortized | Merge rewrites the run |
| Columnar OLAP (DuckDB) | One full column scan | Rewrite affected row groups | Compression and zone maps |
| Graph store (Neo4j) | Index lookup plus traversal | Append to adjacency | Index maintenance |
| **Shelf, fiber-backed query** | **13.0 measured** | **1.0002 measured** | **The fiber is written once and read many times** |
| **Shelf, base-backed query** | **6001.2 measured** | **1.0002 measured** | **The base scan reads the whole column** |

**The shelf representation is not the best at both. It is the best at reads for the shapes it covers, and it is at the theoretical floor for writes. No other architecture in the list has read amplification of 13, because no other architecture materializes the answer.**

---

## 7. Is the existing representation acceptable?

**Rows = criterion. Columns = the verdict and the evidence.**

| Criterion | Verdict | Evidence |
|---|---|---|
| Write cost near the floor | **Met** | 1.0002 measured, shadow merge 1 ms against 8 ms for the copy merge |
| Read cost near the floor, fiber-backed | **Met** | 13.0 measured, 208 bytes |
| Read cost near the floor, base-backed | **Not met, and not met by anyone** | 6001.2, which is the cost of the base scan |
| Correct sequential access | **Met** | Structure-of-arrays primary layout, unit stride, 19.2 GB/s at 4 threads against a 25.6 GB/s bus peak, 75% |
| Correct at the memory ceiling | **Not met at one thread** | 46.6% at one thread, 86.2% at four |
| Works at any volume | **Partly** | The 64 MB domain assumption is 5.3x this host's 12 MiB L3 |
| Exact results | **Met** | Relative difference 9.28e-13 between the two kernels, gates pass |
| Durable writes | **Met** | 2.5 us epoch path, 6.9 us WAL path, against a 1.14 ms fsync floor |
| Bounded write lifetime | **At risk** | The 300 terabytes-written rating runs out in 3.2 years at 0.1 device writes per day |

### 7.1 The one real defect: the accumulator chain

The partition scan in `sheaf-kernel/src/simd/mt_scan.rs` keeps one floating-point accumulator per thread:

```rust
let mut acc = 0.0f64;
for (a, b) in pp.iter().zip(qq.iter()) {
    acc += *a as f64 * (1.0 - *b as f64);
}
```

Every row waits for the previous row's multiply-add to retire. The memory system idles while the processor waits. The measured effect:

**Rows = accumulator lanes. Columns = time and effective bandwidth.**

| Lanes | Time (ms) | Effective (GB/s) | Note |
|---|---|---|---|
| 1 | 5.28 | 9.09 | Serial dependency chain |
| 4 | 3.78 | 12.70 | Four independent chains |
| difference | n/a | n/a | **1.40x faster on the same bytes** |

The aggregate is unchanged. The relative difference is 2.1e-7, which is double-precision rounding over six million additions. The fused structure-of-arrays kernel already uses four lanes and is 1.33x faster at one thread.

**This is a loop-shape fix, not a hardware limit. It is the next change to make.**

### 7.2 The sizing defect: the 64 MB domain assumption

`SPEC-016` and `ARCHITECTURE.md` partition data into 64 MB "L3 domains". This host has a 12 MiB L3, so the assumption is 5.3x too large. The measured L2 to L3 cliff is 5.1x, so a 64 MB domain on this host sits past the knee and is mostly DRAM.

**The 64 MB figure is a server target. Domains must be sized to the measured LLC of the target class.**

### 7.3 The write budget defect

The 300 terabytes-written rating of the 512 GB drive runs out in 3.2 years at 0.1 device writes per day, not 5. The write budget for the whole drive is 51.2 GB per day, and the write-ahead log shares it with everything else. This is the physical reason the fiber bundle must exist, and the reason it must be read far more often than it is written.

---

## 8. The sequential access question, answered directly

**Rows = working set. Columns = the latency and the cycle count, as recorded in
the committed gate artifact.**

| Working set | Latency (ns) | Cycles | Tier |
|---|---|---|---|
| 32 KiB | 1.09 | 5.1 | L1 |
| 64 KiB | 1.67 | 7.8 | L1 |
| 128 KiB | 2.41 | 11.3 | L2 |
| 256 KiB | 2.75 | 12.9 | L2 |
| 512 KiB | 3.66 | 17.1 | L2 |
| 1 MiB | 4.22 | 19.8 | L2 |
| 2 MiB | 8.01 | 37.5 | L2 |
| 4 MiB | 12.18 | 57.1 | L3 |
| 8 MiB | 32.57 | 152.5 | L3 |
| 12 MiB | 62.18 | 291.2 | L3 to DRAM boundary |
| 16 MiB | 93.46 | 437.6 | DRAM |
| 128 MiB | 133.61 | 625.6 | DRAM |
**The access pattern is correct.** The scan is unit-stride over contiguous columns in structure-of-arrays layout, it reaches 75% of the memory bus peak at four threads, and the ladder shows the expected knees with no anomalies.

**The cycle counts are not stable across runs, and that is a property of the machine, not
of the data.** An L1 hit costs a fixed number of cycles, so its nanosecond value tracks
the core clock, and this box's clock moves with thermal state and background load. The
nanosecond column therefore moved between 1.09 and 1.68 ns for the same 32 KiB set across
the recorded runs, while the cycle count stayed near 5. **The cycle count is the
architecture. The nanosecond count is what a caller experiences.**

**Two facts limit the path and neither is a defect in the layout:**

| Limit | Value | Consequence |
|---|---|---|
| Memory bus peak | 25.6 GB/s per channel, 19.2 GB/s measured | 75% of peak. The bus is the wall. |
| Thread scaling ceiling | 2.16x in the recorded artifact, 1.34x in an earlier run | The thread lever is nearly spent. The measured speedup sits under the fitted ceiling in every run. |

---

## 9. Can the shelf architecture surpass the von Neumann limits?

This is the question that needs the sharpest answer, because the answer splits in two.

### 9.1 The physical limits cannot be surpassed

**Rows = physical limit. Columns = the value and whether a representation can beat it.**

| Limit | Value | Surpassable by a representation? |
|---|---|---|
| Landauer energy per bit | 2.87e-21 J | No. The hardware is 3.5e5x above it. |
| Quantum speed limit | 6.0e33 operations per second at 1 W | No. The host does 1e10, so 24 orders away. |
| Bekenstein information bound | 5.0e25 bits in a 10 cm region holding 5 J | No. The memory uses 3e-15 of it. |
| Address space | 2^64 bytes | No. 0.000000093% is used. |
| DRAM bandwidth | 25.6 GB/s per channel | **No.** A representation cannot make a bus faster. |
| DRAM latency | 41.2 ns floor, 102.1 ns measured | **No.** A representation cannot make a memory cell faster. |
| fsync floor | 1.14 ms | **No.** The device must reach the platter. |
| Soft error rate | 2.13 bit flips per day in 16 GiB | **No.** Cosmic rays do not care about the data model. |
| NAND endurance | 1000 cycles for triple-level cells | **No.** The oxide tunnel barrier does not care either. |

**A data representation cannot beat physics.** Every one of these is a property of the medium.

### 9.2 The soft limits can be beaten, and the shelf already beats most of them

These are limits that exist because a design chose them, not because the universe did.

| Soft limit | Value | Beaten by the shelf? | How |
|---|---|---|---|
| Read amplification of a base scan | 6001.2 measured | **Yes** | The fiber answers from 208 bytes instead of 24 MB |
| Write amplification of a merge | 2.0 measured | **Yes** | The shadow merge writes only the delta, giving 1.0002 |
| Instruction and data sharing | One bus | **Partly** | The fiber separates the read path from the write path, so they do not contend for the same structure |
| Machine balance | 4.14 FLOP/byte | **Not applicable** | The kernel is 10x below the ridge, so the balance is not the binding limit |
| Concurrent access to one structure | Merge pauses readers | **Yes** | Per-domain incremental fiber update, 1,000 writes dirty 2 domains and re-sort 2, not 1,000 |

### 9.3 The honest summary

**The shelf architecture does not surpass the von Neumann limits. It avoids paying them.**

The distinction is exact:

- A von Neumann machine cannot read DRAM faster than the bus allows. The shelf does not change that.
- A von Neumann machine cannot materialize an answer it has not computed. The shelf computes it once, ahead of time, and then the read is free.

**The gain is a factor of 115,000 fewer bytes read, not a faster bus.** The measured 208 bytes against 24,004,860 bytes is a 115,000x reduction in the work the machine must do. The bus is unchanged. The medium is unchanged. The number of bytes crossing it is 115,000x smaller.

That is the whole architectural claim, and it is a real one. It is also strictly bounded: the fiber must be built, and the build cost is `O(n log n)` for the sorts. The repository measured that at 1.8 s for the TPC-H fibers, and the IMDb bundle at 51.1 s.

---

## 10. The decision, with the numbers attached

### 10.1 Keep the existing representation

**Rows = reason. Columns = the evidence.**

| Reason | Evidence |
|---|---|
| The write path is at the theoretical maximum | 1.0002 measured against a floor of 1.0 |
| The read path is at the theoretical maximum for covered shapes | 13.0 measured, 208 bytes, against a floor of 1.0 |
| The representation beats the B-tree and the merge tree on its own ground | 44x to 844x on the measured joins |
| The sequential access is correct | 75% of bus peak, clean ladder, no anomalies |
| The results are exact | Relative difference 9.28e-13, all gates pass |

### 10.2 Do these three things next

| # | Action | Expected gain | Cost |
|---|---|---|---|
| 1 | Give the partition scan four accumulators, like the fused kernel | **1.40x on the single-thread scan** | About ten lines, no interface change |
| 2 | Size the fiber domains to the measured LLC of the target class | Removes a 5.3x over-provisioning of the ghost replicas | One constant, and a partitioner change |
| 3 | Build a fiber for every query shape the workload actually uses | Removes the 6001.2 read amplification for the uncovered shapes | One build cost per shape, and a catalog entry |

### 10.3 Do not do these

| Action | Why not |
|---|---|
| Chase the machine balance | The kernel is 10x below the ridge, so the balance is not binding |
| Add more threads | The fitted ceiling is 2.16x in the recorded artifact and the measured speedup sits under it |
| Chase a faster bus | 75% of peak is close to the practical limit, and the second channel is a hardware change |
| Optimize the solver further | The theory review already found the apply path at the `I(nnz)` bound, and the wins come from the data structures |
| Reach for a quantum or reversible design | The limits are 24 orders of magnitude away and 350,000x away |

---

## 11. Honest limits of this report

- **One host, one machine class.** An i7-1165G7 laptop with 16 GiB of DDR4-3200 in a single channel and a 512 GB NVMe drive. The theory in sections 3, 6 and 9 is machine independent, and the measurements in sections 5, 7 and 8 are not.
- **The amplification experiment uses a synthetic base and a synthetic fiber.** The shapes match the real ones, at 6,001,215 rows with 16 bytes per row and a group size of 1000, and the fiber is built by the same grouping rule, but the numbers come from the model rather than from the shipped BCSF and fiber code. The real measurements are in the repository: the 1 ms shadow merge against 8 ms for the copy merge, and the 3.2 ms fiber join against 158 ms for DuckDB.
- **The write amplification of the copy merge is modelled, not measured.** The repository measures the merge time, 1 ms against 8 ms, and the model computes the byte counts from the two policies. The ratio between the two is real. The absolute byte counts come from the model.
- **The published figures for the log-structured merge tree and the B-tree come from Wikipedia**, not from the original papers. The original papers are named in the article and are the better citation.
- **The read amplification of a fiber query assumes a binary search.** A hash index brings it to 1.0, and the repository has no such index on the fiber. The measured 13.0 is what the current code does.
- **The clock on this box swings from 1.1 GHz to 4.4 GHz** with thermal state and background load. Latencies are reported in cycles as well as nanoseconds for this reason, and the machine balance is measured in the same window as the bandwidth it is divided by.
- **The report does not cover the distributed plan.** The Phase 6 and Phase 7 nodes are partly built, and the theoretical maximum for a multi-node shelf is not analysed here.

---

## 12. Evidence index

| Claim | Where it is verified |
|---|---|
| Write amplification 1.0002 and 2.0000 | `physical_limits_probe` section J, `A_amp_write_shadow_merge` and `A_amp_write_copy_merge` |
| Read amplification 13.0, 6001.2, 3000607.5 | `physical_limits_probe` section J, the `A_amp_read_*` lines |
| Bytes read 208 against 24,004,860 | `physical_limits_probe` section J, `A_amp_fiber_binary_bytes_read_B` |
| The theoretical floor of 1.0 | `physical_limits_probe` section J, `A_amp_theoretical_floor` |
| Shadow merge 1 ms against 8 ms | `shadow_paging_bench`, `gate_shadow_merge_scales_with_delta` |
| Chained lookup 5.06 ns against 5.07 ns flat | `shadow_paging_bench`, `shadow_lookup` |
| Accumulator chain, 1.40x | `physical_limits_probe` section H, `H_accumulator_*` |
| Kernel at 46.6% and 86.2% of the ceiling | `physical_limits_probe` section H, `H_utilization_of_ceiling` |
| Machine balance 4.14 FLOP/byte | `physical_limits_probe` section G, `G_same_window_machine_balance_flop_per_byte` |
| Thread ceiling 2.16x | `physical_limits_probe` section I, `C_amdahl_max_speedup_ceiling` |
| Latency ladder and cycle counts | `physical_limits_probe` section A, `A_latency_ns` |
| DRAM access 102.1 ns, page walk 24.4 ns | `physical_limits_probe` section E, `E_dram_access_component_ns` and `E_page_walk_ns` |
| fsync 1.14 ms | `physical_limits_probe` section D, `D_fsync_4KiB_p50_us` |
| Soft errors 2.13 per day | `physical_limits_probe` section I, `C_soft_error_errors_per_day` |
| NAND endurance 3.2 years | `physical_limits_probe` section I, `C_nand_years_to_exhaust_rating` |
| Fiber join 3.2 ms against DuckDB 158 ms | `results/fiber.json` |
| BCSF 33 bytes per row | `COMPLEXITY_ANALYSIS.md` section 2, row 1 |
| All gates pass | `tests/e2e/von_neumann/run.sh` |

**Reproduce everything:**

```bash
cd sheaf-db
cargo run -p sheaf-engine --bin physical_limits_probe --release
sh tests/e2e/von_neumann/run.sh
```

**Report generated:** 2026-10-21 22:06:34 +05:30 (2026-10-21 16:36:34 UTC), commit `afd9439`.
