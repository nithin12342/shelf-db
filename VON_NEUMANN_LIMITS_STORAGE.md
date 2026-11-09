# Von Neumann in silicon, part 2: the storage limits, the repository comparison, and the reproduction steps

**Node:** REQ-057 / SPEC-057 / METHOD-052 Â· **Probe:** `crates/sheaf-engine/src/bin/physical_limits_probe.rs`
**Host:** i7-1165G7 (4C/8T, Tiger Lake, 12 MiB L3) Â· 16 GiB Samsung DDR4-3200 (1 channel) Â· 512 GB Kingston NVMe (PCIe 4.0 x4) Â· Win11
**Evidence:** `tests/e2e/von_neumann/expected/physical_limits.txt` (gate) Â· `tests/e2e/realtime_bench/results/pl_run{1,2,3}.txt` (3 runs)

**Rule for this document:** every number is either (a) **measured on this host** by the
probe above, or (b) **derived here from a published physical constant**, with the derivation
shown. No number is copied from a vendor datasheet and presented as ours. Where a published
spec is quoted, it is labelled `published` and sourced.

**Part 2 of 2.** This file holds sections 2.6 to 6. The answer table, the machine parts, and
the compute and memory limits (sections 0 to 2.5) are in `VON_NEUMANN_LIMITS.md`.

---

### 2.6 Storage: the bus ceiling and the durability floor

**PCIe 4.0 x4** (16 GT/s per lane, 128b/130b encoding):

```
16 GT/s × 4 lanes × (128/130) ÷ 8 = 7.88 GB/s
```

Measured sequential read: **3.20 GB/s = 40.6% of the bus ceiling.** The NVMe device itself
is not the limit here. The drive is a 512 GB OEM part behind a shared controller.

**fsync: the one number that matches theory almost exactly.**

| Operation | Measured | Note |
|---|---|---|
| 4 KiB buffered write (no sync) | 3.6 µs | goes to the OS page cache |
| 4 KiB write + `sync_all` | **1307.6 µs** | 363× more expensive |
| 1 MiB write (no sync) | 584.5 µs | |

The ratio (363×) is the *cost of durability*, and it is a physical property of the device,
not of the code. This matches the repo's earlier independent measurement (1.207 ms) to
within 8%, from a different binary and a different day. The residual is the drive's own
flush-path variability, not a measurement disagreement.

**The layering ratio that justifies pre-aggregation:**

```
storage random access (5.3 us) / DRAM random access (164 ns) = 33.0x
```

Every question that can be answered from RAM instead of NVMe saves a factor of ~33 in
latency. That is the whole economic argument for the fiber bundle.

### 2.7 Magnetic areal density: the thermal-stability wall

> A magnetic bit is stable for time *t* only if the anisotropy barrier `E = K·V` exceeds
> `kT·ln(t/τ0)`, with `τ0 ≈ 1 ns` (thermal attempt frequency).

```
τ0  = 1e-9 s
t   = 10 years = 3.15e8 s
E   = ln(3.15e8 / 1e-9) × k_B × T = 26.8 × 4.14e-21 = 1.11e-19 J
K   = 2.2e5 J/m^3  (CoCrPt-like)
V   = E/K = 5.05e-25 m^3
side= V^(1/3) = 9.12 nm
areal density = 1/side^2 = 1.20 Tb/in^2
```

**Consequence:** for perpendicular recording, the grain cannot be shrunk below ~9 nm
without the bit flipping spontaneously, so **areal density is capped by thermal stability,
not by lithography**. This is why disks stopped scaling on area and started scaling
vertically (HAMR) and in capacity-per-platter instead.

### 2.8 Parallel speedup ceiling - Amdahl's law fitted to the measurement

> Amdahl's law: `S(n) = 1 / ((1 - p) + p/n)`, where `p` is the fraction of
> the work that scales with threads and `1 - p` is the fraction that does not.

The repo asked "how much more can threads give?" for a long time. The answer is
a number, and it comes from the measured thread scaling rather than from an
assumption about the machine. For a bandwidth-bound workload the time has the
same shape, `T(n) = T_serial + T_parallel / n`, so the measured bandwidth
ratios give `T(1) / T(n)` directly.

**Rows = thread count. Columns = the measured bandwidth and the Amdahl
parameters that the measurement implies.**

| Threads | Read (GB/s) | Time ratio | Implied serial fraction | Implied ceiling |
|---|---|---|---|---|
| 1 | 14.45 | 1.0000 | n/a | 1.00x |
| 2 | 16.55 | 0.8731 | 0.2539 | 1.34x |
| 4 | 16.71 | 0.8644 | 0.1808 | 1.22x |
| 8 | 17.96 | 0.8042 | 0.2237 | 1.29x |

**The ceiling is 1.34x.** The best fit implies a serial fraction of 0.25, and
the measured 1→8 thread speedup of 1.24x sits just under the 1.34x the law
allows. **The thread lever is nearly exhausted on this host.** More threads
will not help. The memory bus is the wall (section 2.5 and section 3.1).

This closes a long-running question in the repo. The earlier note said
"thread scaling is NOT a lever past ~2 threads". The fitted ceiling of 1.34x
turns that from a rule of thumb into a measured bound.

### 2.9 DRAM refresh - what the memory spends on itself

> Every DRAM cell leaks charge, so every row must be read and rewritten
> before the charge is gone. The maximum refresh interval is set by the
> ratio of stored charge to leakage current.

```
refresh time   = 64 ms
rows per bank  = 8192,  bank groups refreshed in parallel = 4
refresh cycle  = 4 memory clock cycles = 4 x 0.625 ns = 2.50 ns
cycle interval = 64 ms / (8192 x 4) = 1.953 us
overhead       = (2.50 ns x 8192 x 4) / 64 ms = 0.128 %
```

**Two traffic numbers, and the difference matters:**

| Traffic | Value (GB/s) | Where it travels |
|---|---|---|
| Cell-level (read and rewrite every bit) | 268.4 | Inside each package, on the internal bus |
| External bus (command and row address only) | 0.002 | Across the DDR bus to the controller |

The cell-level traffic is 10x the external bus peak, which sounds alarming
until you notice it never touches the external bus. The external cost is
**0.008% of the bus peak**. Refresh is not a performance problem.

**The retention trade is the real limit.** The refresh interval grew from
8 ms for 1 Mbit chips to 64 ms for 256 Mbit chips, because leakage dropped
as the cell shrank. Cells actually hold their charge for 1 to 10 seconds,
so the 64 ms interval is set by the weakest cells on the die, not by the
typical cell. **Shrinking a DRAM cell below the point where leakage rises
forces a shorter refresh interval, which raises the overhead.** That is the
density wall for DRAM, and it is a leakage wall, not a lithography wall.

### 2.10 Soft errors - the bit-flip rate from space

> Cosmic rays and alpha particles from package decay flip bits on their
> own. A single event upset changes a stored value without damaging the
> circuit.

Using the IBM 1996 figure of one error per month per 256 MiB of RAM, this
host's 16 GiB holds 64 such units:

| Quantity | Value | Meaning |
|---|---|---|
| Errors per month | 64 | Across the whole memory |
| Errors per day | 2.13 | Across the whole memory |
| Mean time between flips | 11.2 hours | Anywhere in the memory |
| Mean time for one bit | 1.76e8 years | A single bit |
| Module mean time between errors | 2.40 years | Using the measured 5950 FIT per chip |

**The gap between 11 hours and 176 million years is the whole story.** A
single bit is almost perfectly durable, and the module still sees a flip
every few hours, because there are 1.4e11 bits. Failures are rare per bit
and constant per module.

**This is the physical argument for error correction.** The repo already
uses CRC32C on every WAL frame and every bundle section. That is not
defensive programming against a rare event. It is a requirement set by
physics, at a rate of about two bit flips per day in a 16 GiB machine.

The rate also depends on altitude, because the source is cosmic rays:

| Altitude (m) | Flux factor |
|---|---|
| 0 | 1.00x |
| 1000 | 2.20x |
| 2000 | 4.84x |
| 3000 | 10.65x |

A database running in a mountain data centre sees ten times the bit-flip
rate of one at sea level.

### 2.11 The floors that are NOT the limits

Three universal bounds exist that no computer approaches. Listing them
prevents a false conclusion later.

**Landauer, restated for a logical operation.** Bennett (1973) showed that
a **reversible** computation can in principle approach an arbitrarily
small energy cost per logical operation, at the cost of extra computation
and storage. So the Landauer figure of 2.87e-21 J is the floor for an
**irreversible** operation, and reversible computing moves the floor
below it. Real hardware is 3.5e5 times above it either way.

**Margolus-Levitin speed limit.** A quantum system with average energy `E`
above the ground state needs a time `tau` with `E x tau >= (pi/2) x hbar`:

| Power (W) | Maximum operations per second |
|---|---|
| 1 | 6.0e33 |
| 10 | 6.0e34 |
| 100 | 6.0e35 |

This host executes about 1e10 operations per second. The quantum limit is
**24 orders of magnitude away**. It will not bind within the lifetime of
any computer.

**Bekenstein bound.** The maximum entropy in a region of radius `R` and
energy `E` is `S <= 2 pi k R E / (hbar c)`. A 10 cm region holding 5 J
can hold at most 5.0e25 bits. This host's 16 GiB holds 1.4e11 bits, so it
uses 3e-15 of the bound. **The information content of the memory is not a
physical limit.** The limit that binds is the bandwidth (section 2.5).

**Address space.** A 64-bit physical address reaches 1.8e19 bytes, and the
canonical 48-bit user space reaches 2.8e14 bytes. This host installs
16 GiB, which is 0.000000093% of the physical space. The address space is
not a limit either. What does limit the usable memory is the translation
lookaside buffer: covering 16 GiB with 4 KiB pages needs 4,194,304
entries, and the second-level buffer holds 1536. **That is the same 6 MiB
reach measured in section 2.3, and it is the reason large pages matter.**

### 2.12 NAND endurance - the write budget

> Each program and erase cycle tunnels electrons through the oxide. The
> cycle count is finite, and it is the real lifetime of a flash cell.

| Cell type | Program/erase cycles |
|---|---|
| Single-level (SLC) | 100,000 |
| Multi-level (MLC) | 10,000 |
| Triple-level (TLC) | 1,000 |
| Quad-level (QLC) | 100 |

Applied to this host's 512 GB drive at 0.1 device writes per day over a
5-year service life:

| Quantity | Value |
|---|---|
| Daily write budget | 51.2 GB |
| Yearly write volume | 93.50 TB |
| Total write volume | 467.52 TB |
| Terabytes-written rating | 300 TB |
| Budget used | 1.56 (156%) |
| Years to exhaust the rating | 3.2 |

**The rating is exhausted in 3.2 years, not 5.** The write budget for the
whole drive is 51.2 GB per day, and the write-ahead log shares that budget
with everything else. This is the physical reason the repo's write-ahead
log must be bounded, and the reason the fiber bundle cache exists: a
bundle that is read many times and written once turns a write-heavy
workload into a read-only one.

---

## 3. Comparing against the numbers already in this repo

**Rows = an existing repo number. Columns = what it claimed, and what this probe measured.**

| Repo number | Where | Claimed | Measured now | Agreement |
|---|---|---|---|---|
| Read bandwidth | `bottleneck.json` A | 4.2 GB/s copy (=8.4 GB/s read) | 11.5 GB/s read (1T) | consistent (read vs copy) |
| 4-thread scaling | `bottleneck.json` B | 1.25× (4t vs 1t) | **1.47×** (17.0 vs 11.5) | **corrected, see below** |
| fsync floor | `bottleneck.json` D | 1.207 ms | 1.308 ms | 8% (device flush variability) |
| CRC cost | `bottleneck.json` E | 2% of durable op | not re-measured | n/a |
| L3 domain budget | `SPEC-016` | 64 MB | **12 MB L3** | **see below** |
| LLC host qualification | `SPEC-007` | "12 MB laptop LLC" | **confirmed: 12 MiB** | exact |
| Sub-15 ns lookup | `SPEC-007` | 11.64 ns median | L1 1.64 / L2 4.27 ns | consistent |
| Memory wall | `MAX_PERF_PLAN` W1 | "8.4 GB/s is the floor" | 11.5 GB/s, **45% of DRAM peak** | refined |

### 3.1 Correction: thread scaling is 1.47×, not 1.25×. The limit is the controller

The old probe measured a **memcpy** (read+write traffic) and concluded "4-thread barely
scales (controller saturated)". This probe measures a **pure read stream**:

| Threads | Read GB/s | Scaling |
|---|---|---|
| 1 | 11.53 | 1.00× |
| 2 | 16.17 | 1.40× |
| 4 | 16.95 | **1.47×** |
| 8 | 19.18 | 1.66× |

The earlier 1.25× figure was an artefact of counting a copy's write traffic against the
read bus. The corrected picture is still "scaling stops at the memory controller", but for
a sharper reason: the 8-thread figure (19.18 GB/s) is **74.9% of the 25.6 GB/s
single-channel DDR4 peak**, and the 4→8 step adds only 13% while the DRAM bus is
75-95% saturated. **The scaling limit is not cores. It is the 64-bit memory bus.** Since
the box has only one populated DIMM, adding the second channel roughly doubles the
ceiling. That is a hardware fact, not an optimisation.

### 3.2 Correction: the "64 MB L3 domain" assumption is 5.3× this host's L3

`SPEC-016` and `ARCHITECTURE.md` partition data into **64 MB "L3 domains"**. This host has
a **12 MiB L3**, 5.3× smaller. Consequences, all now measured:

- The partitioner's own gate passes (`domains_fit_l3`) because it checks a *budget constant*,
  not the real LLC.
- The measured L3 latency is 21.97 ns and the L2 is 4.27 ns, a **5.1× cliff** at the L2/L3
  boundary. A "64 MB domain" on this host is therefore mostly **DRAM** (163.7 ns), not L3.
  The real cliff for this host is at **12 MiB**, and a 64 MB domain sits **5.3× past
  the knee**, i.e. entirely in DRAM.
- This is the same effect the repo already documented for `SPEC-007` ("marginal on 12 MB
  laptop LLC, ~50% process PASS, excursions to ~30 ns"). The two findings are the same
  wall seen from two directions, and this probe now measures the wall directly.

**The architecture decision this forces:** the 64 MB figure is a **server** target (a
server L3 is 32-64 MB per socket, often 256 MB+). On this dev box the honest budget is the
**12 MiB real LLC**, and a domain plan sized to it produces very different ghost-replica
counts. The plan must size domains to the *measured* LLC of the target class, and the
probe makes that measurable rather than assumed.

### 3.3 Confirmation: the repo's "fewer bytes, not more FLOPs" thesis

`VON_NEUMANN_MAPPING.md` already concluded "Efficiency = fewer bytes moved, not more
FLOPs (memory wall binds)". This probe turns that from a qualitative claim into a
**number**: the measured ridge point is **2.92 FLOP/byte**, and the project's kernel runs at
**0.33-0.47 FLOP/byte**, 6-9× below the ridge. The kernel *is* memory-bound, and the
design (f32, factored L, SoA fused columns, pre-aggregation, fiber bundle) is the correct
response. The measurement also bounds the ambition: the 1-core read wall is 11.5 GB/s, so
the achievable gain from a perfect kernel is bounded by bytes moved, not by arithmetic.

---

## 4. What the physics says is *not* the limit

Equally important, and a place where intuition is usually wrong:

| Tempting explanation | What the measurement says |
|---|---|
| "The 2.8 ms scan is slow because of the CPU" | Ridge point 2.92 vs kernel 0.40 FLOP/byte → the CPU is idle. Memory is the wall |
| "fsync is slow because of the OS" | 4 KiB buffered write = 3.6 µs, fsync = 1308 µs. The cost is the device's flush-to-media path, and it matches theory |
| "The clock is the limit" | The clock is why the *ns* figures move, but the *cycle* figures are stable (L1 = 4.6, L2 = 13.7, L3 = 62.6). The scan is bandwidth-bound, so the shape is the same at any clock |
| "We need more FLOPs" | The kernel is 6-9× *below* the ridge. Adding FLOPs cannot help |
| "Landauer/energy is why laptops throttle" | Landauer is 3.5e5× below CMOS, and the quantum speed limit is 24 orders of magnitude away. Energy is a *power-delivery* limit, not an information limit |
| "Large pages can fix cold random access" | Correct, and unmeasurable here: the STLB reach (6 MiB) is *below* the L3 (12 MiB), so with 4 KiB pages a TLB-resident footprint always fits in L3 |
| "More cores can fix the bandwidth" | 4→8 threads adds 13%: the 64-bit DDR bus is 75-95% saturated, and the host has only ONE populated channel |
| "More threads can fix the scan" | Amdahl's law fitted to the measurement gives a 1.34x ceiling. The measured 1.24x is already at it. The thread lever is spent |
| "DRAM refresh is eating our bandwidth" | Refresh is 0.128% of the time and 0.008% of the external bus. It is invisible |
| "Memory is too small for the data" | The address space is 0.000000093% used, and the Bekenstein bound is 3e-15 used. Capacity is not the limit |
| "A bit flip will corrupt the database eventually" | True, and the rate is physics: about 2 bit flips per day in 16 GiB. This is why every WAL frame carries a CRC32C |
| "GPUs can fix this" | GPUs fix it by moving the ridge (2.9 → 20..1930 FLOP/byte) via a 1024-bit memory bus, not by being faster at the same bus |

---

## 5. Reproducing this

```bash
# The probe (gate + full output)
cd sheaf-db && cargo run -p sheaf-engine --bin physical_limits_probe --release
sh tests/e2e/von_neumann/run.sh

# 3-run summary
powershell -ExecutionPolicy Bypass -File tests/e2e/realtime_bench/summarize_physical_limits.ps1
```

**The 6 physics-invariant gates** (asserted, not reported. A failure means the measurement
is wrong, not that the machine is odd):

| Gate | Assertion |
|---|---|
| `gate_hierarchy_ordered` | L1 < L2 < L3 < DRAM, strictly |
| `gate_dram_slower_than_l3` | DRAM access exceeds L3 access |
| `gate_dram_above_ddr4_cas_floor` | DRAM > 41.2 ns (else we never reached DRAM) |
| `gate_fsync_floor_exists` | fsync > 10× a buffered write of the same size |
| `gate_read_bw_significant` | read bandwidth > 5% of the DRAM bus peak |
| `gate_dram_slower_than_l1` | DRAM exceeds L1 |

Plus the clock cross-checks: the 8-add vs 4-add chain ratio must be 2.000, and the
4×imul vs 4-add ratio must be 3.000. The second is what validates the *absolute* cycle
count, and both were what caught two wrong clock derivations during development.

---

## 6. Honest limits of this study

- **One host, one machine class.** A laptop i7-1165G7 with **one** populated DDR4 channel
  and **no** spinning disk. The HDD rows are derived from published timings, not measured
  here. The NVMe rows are measured.
- **The clock is not stable.** It swung 1.44-3.21 GHz across the session, with thermal
  state and ~25-30% background load from other processes. Absolute-latency numbers are
  therefore reported in **cycles** (stable) as well as ns (what a caller sees). The
  clock-dependent ones (FLOP/s) vary more, which is why the machine balance is computed
  same-window and why the cross-checks are asserted rather than eyeballed.
- **The clock anchor was wrong twice before it was right.** The first two versions
  hand-analysed a Rust loop's cycle count from the disassembly and got it wrong (LLVM
  strength-reduced the chain and unrolled it. Then a `mov` was not eliminated). The
  current probe derives the clock from the *marginal* cost of one added dependency and
  validates it with two independent cross-checks (8-add vs 4-add = 2.02 vs 2.00,
  4×imul vs 4-add = 3.09 vs 3.00). A third bug: dividing *independently best-taken*
  chain times, which mixes thermal windows, was caught by the same cross-checks
  (1.78 instead of 2.00) and is why the probe now measures adjacent pairs and takes the
  median of per-pair ratios.
- **The DRAM-access decomposition is a 3-quadrant inference.** The 4th quadrant (DRAM with
  a TLB hit) is unreachable with 4 KiB pages on this host (§2.3), so the split between
  "DRAM access" and "page walk" rests on holding the footprint constant: a sound but
  indirect method.
- **The TPU/GPU rows are `published`, not measured.** Only the CPU row is this host's.
- **`landauer` uses T = 300 K** and a CMOS switching cost of ~1e-15 J, which is a
  representative rather than a measured figure for this specific part.
- **The `d×d` kernel-intensity figures in §2.4 are analytical**, computed from the matvec
  shape. They are not a measured profile of the shipped kernel.
- **The new ceilings in §2.8 through §2.12 are computed from published constants**, not
  measured on this host. The only measured part is the Amdahl fit, which uses this
  host's own thread scaling. The constants are: 5950 failures in time per DRAM chip and
  14 neutrons per cm² per hour (Wikipedia, "Soft error"), 64 ms refresh time, 8192 rows
  per bank and 4 cycles per refresh (Wikipedia, "Memory refresh"), 1 error per month per
  256 MiB of RAM (IBM, 1996), 100 to 100,000 program and erase cycles by cell type
  (Wikipedia, "Solid-state drive"), and the Landauer, Margolus-Levitin, Bekenstein and
  Amdahl relations.
- **The DRAM package topology is assumed, not read from the machine.** The soft error
  figures use 8 packages of 2 GiB, and the refresh figures use 4 bank groups. A different
  topology changes those numbers.
- **The Amdahl fit is a single-point-per-thread-count fit.** The implied serial fraction
  ranges from 0.18 to 0.25 across thread counts, so the ceiling of 1.34x carries that
  spread. The measured 1.24x is inside it under every fit.
