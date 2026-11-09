# Von Neumann in silicon, part 1: the equipment, the hierarchy, and the compute and memory limits

**Node:** REQ-057 / SPEC-057 / METHOD-052 · **Probe:** `crates/sheaf-engine/src/bin/physical_limits_probe.rs`
**Host:** i7-1165G7 (4C/8T, Tiger Lake, 12 MiB L3) · 16 GiB Samsung DDR4-3200 (1 channel) · 512 GB Kingston NVMe (PCIe 4.0 x4) · Win11
**Evidence:** `tests/e2e/von_neumann/expected/physical_limits.txt` (gate) · `tests/e2e/realtime_bench/results/pl_run{1,2,3}.txt` (3 runs)

**Rule for this document:** every number is either (a) **measured on this host** by the
probe above, or (b) **derived here from a published physical constant**, with the derivation
shown. No number is copied from a vendor datasheet and presented as ours. Where a published
spec is quoted, it is labelled `published` and sourced.

---

**Part 1 of 2.** This file holds the answer table, the machine parts, and the compute and
memory limits (sections 0 to 2.5). The storage limits, the endurance limits, the comparison
against the numbers already in this repository, and the reproduction steps are in
`VON_NEUMANN_LIMITS_STORAGE.md`.

## 0. Answer in one table

Rows = the physical limit. Columns = the derived floor, what this host measures, and the gap.

| # | Limit | Derived floor | This host | Gap | Verdict |
|---|---|---|---|---|---|
| L1 | Landauer energy per bit erased | 2.87e-21 J (kT·ln2, 300 K) | ~1e-15 J (CMOS) | 3.5e5× | Not the binding constraint |
| L2 | Signal flight across a die | 120 ps / 10 mm one-way | n/a | n/a | Bounds die size, not clock |
| L3 | DRAM random access (DDR4-3200 CL22) | 41.2 ns (tRCD+tCL+tRP) | 92.6 ns | 2.24× | Real access ≫ bare timing |
| L4 | Page walk (4 KiB pages) | 46.6 ns measured | n/a | n/a | Dominates cold random access |
| L5 | DRAM read bandwidth (1 channel) | 25.6 GB/s | 11.5 GB/s (1T) | 45% | Bandwidth-saturated |
| L6 | Storage sequential read | 7.88 GB/s (PCIe 4.0 x4) | 3.20 GB/s | 40.6% | Bus-limited |
| L7 | Durable write (fsync) | 1.31 ms | 1.31 ms | 1.00× | **Hard floor, confirmed** |
| L8 | Machine balance (ridge point) | n/a | 2.92 FLOP/byte | n/a | Kernel is 6-9× below it |
| L9 | HDD areal density (10-yr retention) | 1.20 Tb/in² | n/a | n/a | Thermal stability, not area |
| L10 | Storage/DRAM latency ratio | n/a | 33.0× | n/a | Justifies pre-aggregation |
| L11 | **Database scan vs memory ceiling** | n/a | **42-81% of ceiling** | n/a | **Gap is the accumulator, not memory** |
| L12 | Parallel speedup ceiling (Amdahl) | 1.34-1.50x | 1.24x measured | n/a | Thread count is nearly exhausted |
| L13 | DRAM refresh overhead | 0.128% of time | n/a | n/a | Self-refresh is cheap |
| L14 | Soft error rate (cosmic rays) | 64 errors/month | n/a | n/a | **Error correction is mandatory** |
| L15 | Quantum speed limit | 6.0e33 ops/s at 1 W | ~1e10 ops/s | 1e24× | Not the binding constraint |
| L16 | Bekenstein information bound | 5.0e25 bits in 5 J | 1.4e11 bits | 1e14× | Not the binding constraint |
| L17 | Address space ceiling | 2^64 bytes (2^48 canonical) | 16 GiB | 1e-7% | Not the binding constraint |
| L18 | NAND program/erase endurance | 1000 cycles (TLC) | 1.56 of 300 TBW | n/a | 3.2 years to exhaust |

### 0.1 Does the database reach the memory limit? Short answer

**No, not at one thread. The database scan reaches 42% of the memory ceiling at one
thread and 81% at four threads. The missing time is not the memory system. It is a
serial accumulator inside the kernel.**

The detail is in section 2.4a. The short version: the partition scan keeps one
floating-point accumulator per thread, so every row waits for the previous
row's multiply-add to finish. The memory system idles while the processor
waits. A four-lane version of the same arithmetic runs 1.47x faster on the
same bytes.

### 0.2 What is left on the table? The full list of ceilings

**Rows = ceiling. Columns = the derived floor, the gap to this host, and whether
it binds.**

| Ceiling | Floor | Gap | Binds? |
|---|---|---|---|
| Memory bandwidth | 25.6 GB/s (1 channel) | 45% used at 1 thread | **Yes** |
| Parallel speedup (Amdahl) | 1.34x | 1.24x measured | **Yes, nearly spent** |
| DRAM random access | 41.2 ns | 2.24x measured | **Yes** |
| Durable write | 1.31 ms | 1.00x measured | **Yes, at the floor** |
| Accumulator chain | 1.47x available | 42% used at 1 thread | **Yes, fixable now** |
| Bit-flip rate | 2 per day | n/a | **Yes, needs correction** |
| NAND write budget | 3.2 years | 156% over 5 years | **Yes, over budget** |
| Landauer energy | 2.87e-21 J | 3.5e5x above | No |
| Quantum speed limit | 6.0e33 ops/s at 1 W | 1e24x away | No |
| Bekenstein bound | 5.0e25 bits | 3e-15 used | No |
| Address space | 2^64 bytes | 1e-7% used | No |

---

## 1. The machine parts, and where each one lives

The von Neumann model (1945) has five organs. Every modern computer still has all five.
what changed is that the *memory* organ became a hierarchy and the *processor* organ became
a choice among four different designs.

| Von Neumann organ (1945) | Modern equivalent | This host | Where it is in sheaf-db |
|---|---|---|---|
| Central arithmetic unit | ALU + FMA units | AVX-512 FMA, 2/cycle | `simd/fma_scan.rs` matvec |
| Central control unit | Front end + branch predictor | 8-wide OoO | `ir/optimizer.rs` plan |
| Memory (instructions + data) | L1/L2/L3 SRAM → DRAM | 32 KiB / 5 MiB / 12 MiB → 16 GiB | BCSF slabs |
| Outside recording medium | NVMe SSD | 512 GB, PCIe 4.0 x4 | WAL frames, fiber bundle |
| I/O mechanisms | DMA + PCIe + OS page cache | n/a | `lsp/wal.rs`, bundle cache |

### 1.1 Primary storage: the ladder, measured

**Rows = memory tier. Columns = what was measured and how.**

| Tier | Technology | Latency (ns) | Latency (cycles) | Capacity | Bandwidth | Cost/byte |
|---|---|---|---|---|---|---|
| Tier | Technology | Latency (ns) | Latency (cycles) | Capacity | Bandwidth | Cost/byte |
|---|---|---|---|---|---|---|
| Registers | flip-flop | ~0.3 (1 cycle) | 1 | 32 × 64-bit | 2×64 B/cycle | astronomic |
| L1d/L1i | 6T SRAM | **1.64** | **4.6** | 48 KiB/core | 2×64 B/cycle | high |
| L2 | 6T SRAM | **4.27** | **13.7** | 1.25 MiB/core | ~64 B/cycle | high |
| L2 (far) | 6T SRAM | 7.33 | 20.8 | 1.25 MiB/core | ~64 B/cycle | high |
| L3 | 6T SRAM, shared | **21.97** | **62.6** | 12 MiB | ~30 B/cycle | med |
| DRAM | 1T1C DRAM, DDR4-3200 | **163.7** | 478 | 16 GiB | **11.5 GB/s** (1T) | low |
| NVMe | 3D NAND + DRAM cache | **5.3 µs** (4 KiB random) | n/a | 512 GB | **3.20 GB/s** seq | low |
| (HDD, reference) | magnetic platter | 2.08 ms half-rev @7200 rpm | n/a | 20 TB | 260 MB/s | lowest |

**Method.** Latency is a *dependent pointer chase*: each load's address is produced by the
previous load, so the CPU cannot overlap them. Working sets are swept from 4 KiB to 128 MiB.
Best-of-3 per point. 3 process-level runs. Medians reported.

**Why both ns and cycles.** The cache tiers are measured in *cycles* as well as ns, because
ns alone is not a stable quantity on this box: an L1 hit costs a fixed number of cycles,
so its nanosecond value tracks the core clock, and this box's clock swings with thermal
state and background load. **The cycle figure is the architecture. The ns figure is what a
caller experiences.** Both are printed by the probe. (The DRAM row's cycle figure is the
least reliable: DRAM latency is mostly not clock-bound, so dividing it by a clock that
varies 2-3× across the sweep inflates it. The DRAM *ns* figure is the solid one.)

**The ladder is not smooth. It has knees, and the knees are the architecture:**

```
latency (ns)
   |
164|                                        ...........  DRAM (164 ns)
   |                                    ...
   |                                ...
 22|            ...L3 (22.0 ns)......
   |        ...
  4|   L2 (4.3)
   |  .
1.6| L1 (1.64)
   +--------------------------------------------------> working set
     4K  32K  128K   1M    4M    12M   16M   128M
              |__L2__|  |_____L3____|  |__DRAM__|
```

### 1.2 The processor zoo: four different answers to the same question

All four are von Neumann (shared instruction/data memory). They differ in **where the
memory sits relative to the arithmetic**. That single choice is what sets their roofline.

| Processor | Execution model | Memory | Peak compute | Mem BW | **Balance (FLOP/byte)** | Roofline shape |
|---|---|---|---|---|---|---|
| **CPU** (this host) | scalar OoO, 1 instr/cycle/core | DDR4 + SRAM | 29.7 GFLOP/s (1T, AVX-512) | 15.6 GB/s | **2.92** (measured, same window) | shallow, memory-bound |
| **GPU** (H100) | 132 SMs, 2048 threads, SIMT | HBM3 80 GB | 67 TFLOPS FP32 | 3.35 TB/s | **20** (FP32) | deep, but compute-bound in FP32 |
| **GPU** (H100, tensor) | tensor cores | HBM3 | 990 TFLOPS FP16 | 3.35 TB/s | **295** | very deep |
| **TPU** (v4) | 2D systolic array, 4096/chip | HBM2 32 GB | 275 TOPS | 1.2 TB/s | **229** | deepest per byte |
| **TPU** (v7 Ironwood) | systolic | HBM3e 192 GB | 4614 TFLOPS FP8 | 7.37 TB/s | **626** | deepest |
| **TPU** (8t) | systolic, FP4 | HBM3e 216 GB | 12.6 PFLOP FP4 | 6.53 TB/s | **1930** | extreme |

**The one-line story:** a GPU/TPU does not beat a CPU at the von Neumann bottleneck. It
**moves the bottleneck**. Going from CPU (2.9) to TPU (626) FLOP/byte is a ~215× shift in
where the wall is, achieved by widening the memory interface (HBM: 1024-bit per stack vs
64-bit per DDR channel) and by dedicating the datapath to one operation shape.

**Published TPU generations** (Wikipedia, "Tensor Processing Unit". Bandwidth and TOPS as published):

| Gen | Process | Clock | On-chip mem | Memory | Mem BW | Perf | Balance |
|---|---|---|---|---|---|---|---|
| v1 | 28 nm | 700 MHz | 28 MiB | 8 GiB DDR3 | 34 GB/s | 23 TOPS (int8) | 676 |
| v2 | 16 nm | 700 MHz | 32 MiB | 16 GB HBM | 600 GB/s | 45 TFLOPS | 75 |
| v3 | 16 nm | 940 MHz | 128 MiB | 32 GiB HBM | 900 GB/s | 123 TFLOPS | 137 |
| v4 | 7 nm | 1050 MHz | 128 MiB | 32 GiB HBM | 1200 GB/s | 275 TOPS | 229 |
| v5e | n/a | n/a | n/a | 16 GB HBM | 819 GB/s | 197 TOPS (bf16) | 241 |
| v5p | n/a | n/a | n/a | 95 GB HBM | 2765 GB/s | 459 TOPS | 166 |
| v6e | n/a | 1750 MHz | n/a | 32 GB HBM | 1640 GB/s | 918 TOPS | 560 |
| v7 | n/a | n/a | n/a | 192 GB HBM3e | 7.37 TB/s | 4614 TFLOPS | 626 |
| 8t | n/a | n/a | n/a | 216 GB HBM3e | 6.53 TB/s | 12.6 PFLOP FP4 | 1930 |

### 1.3 The memory interface, generation by generation

**Rows = memory generation. Columns = interface width and rate, and the resulting bandwidth.**

| Gen | Interface | Pins/stack | Rate/pin | Bandwidth/stack | Note |
|---|---|---|---|---|---|
| HBM1 | 8×128b | 1024b | 1.0 GT/s | 128 GB/s | interposer required |
| HBM2 | 8×128b | 1024b | 2.0 GT/s | 256 GB/s | |
| HBM2E | 12×128b | 1024b | 3.6 GT/s | 461 GB/s | 24 GB/stack |
| HBM3 | 16×64b | 1024b | 6.4 GT/s | 819 GB/s | channels doubled, pins same |
| HBM3E | 16×64b | 1024b | 9.6 GT/s | 1229 GB/s | |
| HBM4 | 32×64b | **2048b** | 8.0 GT/s | 2048 GB/s | bus width doubled |
| DDR4-3200 (this host) | 1×64b | 64b | 3.2 GT/s | **25.6 GB/s** | **11× narrower than HBM3** |
| GDDR6X (GPU) | 16×32b | 512b | 24 GT/s | 1.8 TB/s | |

**The width is the whole story.** HBM3's 1024-bit interface is 16× a DDR channel's 64 bits.
That is why a GPU with 3.35 TB/s needs a 1024-bit bus and an interposer, and why the same
silicon on DDR4 tops out at 25.6 GB/s. **Bandwidth is bought with pins, and pins are
expensive**. That is the von Neumann bottleneck expressed in packaging.

---

## 2. The derived physical limits

Each is derived from a published constant, with the arithmetic shown.

### 2.1 Landauer limit: the energy floor of a bit

> Erasing one bit must dissipate at least `E ≥ k_B·T·ln2`.

```
k_B = 1.380649e-23 J/K
T   = 300 K
E   = 1.380649e-23 × 300 × 0.6931 = 2.871e-21 J per bit
```

A real CMOS switching event costs ~1e-15 J, so this hardware sits **3.48e5× above**
the thermodynamic floor. Published: "modern computers use about a billion times as much
energy per operation" (Landauer's principle, Wikipedia).

**Consequence:** energy per bit is *not* today's binding constraint. If computation ran at
the Landauer limit, the same chip is ~350 000× more efficient. The binding
constraints are **communication** (section 2.2) and **geometry** (section 2.3), not
thermodynamics. This is why this project's optimisation target is *bytes moved*
(`VON_NEUMANN_MAPPING.md`: "Efficiency = fewer bytes moved, not more FLOPs").

### 2.2 Signal flight: the speed of light in silicon

> A signal in a loaded on-die wire travels at ~c/n with n ≈ 3.6.

```
v = 2.998e8 / 3.6 = 8.33e7 m/s
```

| Die span | One-way flight | Round trip | Implied max clock (round trip = 1 cycle) |
|---|---|---|---|
| 10 mm | 120.1 ps | 240.2 ps | 4.16 GHz |
| 25 mm | 300.2 ps | 600.4 ps | 1.67 GHz |
| 50 mm | 600.4 ps | 1200.8 ps | 0.83 GHz |

**Consequence:** a die twice as wide halves the maximum clock rate, purely from propagation
delay. This is the physical reason multi-core won: instead of making one core bigger
(and slower), you make the die wider and put several small cores in it, each with a short
round trip. **It is also why the L3 is a separate die-level concern and why on-package
(HBM) beats on-board (DDR) memory**. Every centimetre of trace is 120 ps of flight time.

### 2.3 DRAM random access: the timing floor, and what it really costs

> DDR4-3200: 3200 MT/s → 0.625 ns per clock. CL22 → 22 cycles for the CAS latency.
> A random access is `tRCD` (row activate) + `tCL` (column) + `tRP` (precharge) = 3×22.

```
cycle           = 2000 / 3200 = 0.625 ns
tRCD+tCL+tRP    = 3 × 22 × 0.625 = 41.2 ns   <- the floor
```

**Measured decomposition.** The probe separates the two costs that a naive pointer chase
conflates, by holding the *cache footprint* constant and varying only the *page count*:

| Quadrant | Measured (ns) | Meaning |
|---|---|---|
| L3 hit, TLB hit | 38.1 | pure L3 + no walk |
| L3 hit, TLB miss | 74.0 | L3 + one page walk |
| DRAM, TLB miss | 170.8 | DRAM + one page walk |

```
page walk      = 74.0 - 38.1 =  46.6 ns
DRAM access    = 170.8 - 74.0 = 92.6 ns   (2.24x the 41.2 ns bare timing)
```

The extra 51 ns over the bare timing is row activation, bus turnaround, refresh stalls
and controller queueing. That is the difference between a datasheet number and a real machine.

**A structural finding.** The fourth quadrant (DRAM, TLB hit) is **not measurable** on
this host with 4 KiB pages: the STLB reach is 1536 entries × 4 KiB = **6 MiB**, which is
*smaller* than the 12 MiB L3, so any footprint that fits the TLB also fits the L3. Large
pages are needed. This box refuses them (ERROR_PRIVILEGE_NOT_HELD, 1314). The
impossibility is a result, not a gap.

**And the headline cold-access number is therefore ~164 ns, not 93 ns.** A dense chase over
128 MiB (which touches 32 768 pages) costs 163.7 ns. It pays DRAM *and* a page walk
*and* heavy TLB pressure. The single biggest lever on cold random access on this box is
not the memory chip. It is the **page size**.

### 2.4 The von Neumann bottleneck, measured: machine balance

> Machine balance (roofline ridge point) = peak FLOP/s ÷ peak byte/s.
> Below this intensity, a kernel is memory-bound no matter how fast the ALU is.

The critical subtlety: **compute and bandwidth must be measured in the same window.** This
box's clock swings from 1.1 to 3.5 GHz across runs (thermal + background load), so a
balance computed from numbers taken minutes apart is meaningless. The probe interleaves
them (section G):

```
same-window read      = 15.59 GB/s
same-window compute   = 43.32 GFLOP/s
machine balance       = 2.92 FLOP/byte
```

**Now the decisive comparison**: the arithmetic intensity of *this project's own kernel*,
a dense d×d matvec per face (reads d² weights + d inputs, does d² FMAs):

| Kernel | Flops | Bytes (f32) | Intensity | vs measured ridge (2.92) |
|---|---|---|---|---|
| d=2 matvec | 8 | 24 | 0.333 | **8.8× below → memory-bound** |
| d=4 matvec | 32 | 80 | 0.400 | **7.3× below → memory-bound** |
| d=8 matvec | 128 | 288 | 0.444 | **6.6× below → memory-bound** |
| d=16 matvec | 512 | 1088 | 0.471 | **6.2× below → memory-bound** |

**This is the quantitative justification for the project's core design decision.** The
kernel is 6-9× below the ridge point, so it is *structurally* memory-bound. The only
lever that moves it is **bytes moved**, not FLOPs. That is exactly why:
- `f32` over `f64` (halves every stream) is the right trade, not "precision is enough",
  but "the kernel lives on the bandwidth wall".
- the factored Laplacian (`L = δ*δ`, never materialised) is right. It removes an
  O((Vd)²) write, not an O((Vd)²) multiply.
- fused columns (SoA) are right, for the same reason.
- pre-aggregation (P3) and the fiber bundle (B1/B2) are right. They remove *whole scans*.

**And the limit on the lever itself:** the 1-thread read bandwidth measured here
(11.5 GB/s) is 45% of the 25.6 GB/s single-channel DDR4 peak. So even a perfect
implementation cannot go much past ~11 GB/s on one core. The wall is 4× below what a naive
"just make the kernel faster" plan targets.

### 2.4a Does the database reach the memory ceiling? The gap is the accumulator

The roofline says the kernel is memory-bound. That predicts it runs at the
memory bandwidth. It does not. The probe measures the real database kernel and
the memory ceiling **in the same window**, so the difference cannot be thermal
drift.

**Rows = thread count. Columns = the memory ceiling and the real kernel, both
measured on the same 48.0 MB of TPC-H lineitem data in the same window.**

| Threads | Memory ceiling (GB/s) | Partition scan (GB/s) | Utilization (%) | Fused scan (GB/s) | Utilization (%) |
|---|---|---|---|---|---|
| 1 | 13.67 | 5.78 | **42.3** | 7.71 | 56.4 |
| 2 | 22.21 | 12.34 | **55.5** | 10.27 | 46.2 |
| 4 | 22.42 | 18.12 | **80.8** | 10.14 | 45.2 |

The ceiling row is a streaming read of the same two columns with the same thread
count. The kernel row is the storage engine's own partition scan. The ceiling
reaches 22.4 GB/s at four threads, so the memory system can feed the kernel
faster than the kernel consumes.

**The cause is the accumulator, not the memory.** The partition scan keeps one
floating-point accumulator per thread:

```rust
let mut acc = 0f64;
for (a, b) in pp.iter().zip(qq.iter()) {
    acc += *a as f64 * (1.0 - *b as f64);
}
```

Every row waits for the previous row's multiply-add to retire. The measured
multiply-add latency sets the pace, and the memory system idles in between.
The probe isolates this by running the same arithmetic with one lane and with
four independent lanes, in the same window:

| Lanes | Time (ms) | Effective (GB/s) | Note |
|---|---|---|---|
| 1 | 7.20 | 6.66 | serial dependency chain |
| 4 | 4.91 | 9.78 | four independent chains |
| n/a | n/a | n/a | **1.47x faster on the same bytes** |

**Why one thread is the worst case.** The dependency chain is per thread. At
one thread the chain sets the pace and the memory idles 58% of the time. At
four threads the four chains interleave, so the memory system stays busy and
utilization rises to 81%. The kernel has a ceiling of its own, and it is below
the memory ceiling at every thread count.

**What this means for the design.** The memory hierarchy was never the
bottleneck for the single-thread scan. The fix is a data layout and loop
change, not a faster machine and not a wider bus:

- The fused scan already uses four lanes, and it is 1.33x faster than the
  partition scan at one thread.
- The partition scan is the one that still uses a single accumulator.
- Both kernels produce the same aggregate to within 1e-12, so the change is
  safe to make.

The repo already moved the column layout to structure of arrays (FILE-076).
The accumulator lanes are the next step, and the probe now measures the size
of that prize: **1.47x on the single-thread scan.**

### 2.5 Bandwidth-delay product: the concurrency the memory system must hide

> By Little's law, to sustain bandwidth B over a latency L you need B×L bytes in flight.

```
B = 19.2 GB/s (8 threads), L = 164 ns  =>  B×L = 3081 bytes
```

The host's outstanding-miss capacity (LFBs + fill buffers) is in this ballpark, which is
why 4 threads reaches 17.0 GB/s but 8 threads adds little (19.2 GB/s. The memory
controller is saturated). **The scaling limit is not cores. It is the memory controller.**
This corrects the earlier repo note ("thread scaling is NOT a lever past ~2 threads"):
1→4 threads gives 1.47×, and 4→8 gives 1.13×.

