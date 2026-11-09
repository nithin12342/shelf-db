# Hardware-aligned planes: what the three-plane architecture changes in sheaf-db

**Document generated:** 2026-10-22, Asia/Kolkata (UTC+05:30)
**Basis commit:** `e8fc5c0` (branch `fiber-bundle-replacement`)
**Subject:** the Hardware-Aligned Horizontally Sliced Architecture, read against this repository
**Host:** Intel Core i7-1165G7, 4 cores and 8 threads, 12 MiB level 3 cache (L3), Windows 11
**Memory and storage:** 16 GiB Samsung DDR4-3200 on one memory channel, 512 GB Kingston Non-Volatile Memory Express (NVMe) drive

**Reading rule.** Every number in this document is read from a committed file in this repository, or derived from a published constant. The derivation appears with the number. Where a number is a projection, the projection and its basis both appear. No measurement was run for this document. No source file was changed.

---

## 0. Answer in one table

Rows = the question. Columns = the answer and the evidence.

| # | Question | Answer | Evidence |
|---|---|---|---|
| Q1 | What is the architecture? | A decomposition by hardware execution profile. It groups work that needs the same silicon into three planes: input-output, compute, and state. | Section 2 |
| Q2 | How much of it exists here today? | Four of the six mechanisms exist in another form. The immutable read slice, the lock-free ring, the pre-aggregated read plane, and core pinning. | Section 3 |
| Q3 | What does it change? | The write path. A sequencer removes the global write-ahead log (WAL) mutex, the global commit-clock mutex, and the per-face index locks. | Section 4.1 |
| Q4 | Does it touch the largest measured gaps? | No. The pre-aggregate is 257x to 569x, the read waste is 7.14x, and the accumulator chain is 1.40x. The architecture changes none of the three. | `ARCHITECTURE_GAINS.md` sections 3.3, 3.4, 3.5 |
| Q5 | Which element conflicts with a recorded decision? | The socket plane. This repository recorded in-process-only operation, because a remote hop turns a 10.5 ns read into about 70 us. | `CONTEXT.md` section 12 |
| Q6 | Which element cannot be checked on this host? | The Linux `io_uring` interface and the Data Plane Development Kit (DPDK) drivers. The host runs Windows 11. | Section 5 |
| Q7 | Which source claim has no measurement? | The branch prediction figure above 99.5 percent, and the sequencer figure of 5 to 10 million operations per second. The write gate here is 4 writers at 50,000 per second. | Section 5 |
| Q8 | What does the architecture not move? | The memory bus, the durable write floor, the DRAM latency, and the parallel speedup ceiling. | Section 8 |
| Q9 | What is the net effect? | A smaller lock surface on the write path and a clearer separation of the planes. The measured wins in this repository remain bytes reductions. | Sections 6 and 8 |

---

## 1. Method and reading rule

### 1.1 The three labels for every number

| Label | Meaning | Example in this document |
|---|---|---|
| measured | Read from a committed result file or a committed gate record | The lookup median of 11.64 ns |
| derived | Computed here from a published constant, with the computation shown | The 534.4 ns page walk |
| projected | An expectation with a stated basis and no measurement behind it | The gain from removing the global WAL mutex |

A number with no label does not appear in this document.

### 1.2 What this document did not do

- It ran no benchmark. Every time comes from a committed file.
- It changed no source file and no configuration.
- It did not decide the three conflicts in section 7. It lists the options and their consequences.

### 1.3 Three terms used throughout

- **Sequencer:** the one thread that applies state changes.
- **Single-producer single-consumer (SPSC) ring:** a queue with one writer and one reader.
- **Front end:** the code that turns stored bytes into a query answer, before any state change.

---

## 2. The plane map

### 2.1 The three planes as they sit over this engine

```
  [ in-process client call ]
             |
             v
  +-------------------------------------------+
  | PLANE 1: FRONT END AND DECODE            |
  | file read, CRC32C verify, section        |
  | decode, TQL tokenize                     |
  +-------------------------------------------+
             |  one command, one cache line
             v
  +-------------------------------------------+
  | PLANE 2: SEQUENCER                       |
  | one thread, one order, no I/O,           |
  | no locks, no allocation                  |
  +-------------------------------------------+
             |  commit record
             v
  +-------------------------------------------+
  | PLANE 3: STATE AND PERSISTENCE           |
  | BCSF base, live bits, Upper-Star index,  |
  | WAL, compaction                           |
  +-------------------------------------------+
             |
             v
  [ a read takes the base directly and never meets the sequencer ]
```

The read path at the bottom already avoids the sequencer in this repository. The base segment is immutable, and a reader takes a view over it.

### 2.2 What carries each plane today

Rows = the role. Columns = the files, the primitive today, the primitive in the architecture, and the state of the change.

| Role | Files today | Primitive today | Primitive in the architecture | State |
|---|---|---|---|---|
| Front end and decode | `bin/shared/fiber_snapshot.rs`, `bin/shared/imdb_fiber.rs`, `sheaf-query/src/syntax/lexer.rs` | whole-file read or section seek, hardware cyclic redundancy check (CRC) | poll and decode on a pinned core | Partly present |
| Sequencer | `sheaf-engine/src/transaction/manager.rs`, `sheaf-storage/src/lsp/mem_complex.rs` | striped mutexes and one global WAL mutex | one writer, one order | Missing |
| State | `sheaf-storage/src/lsp/mem_complex.rs`, `bcsf/reader.rs`, `bcsf/builder.rs` | atomic live bits, atomic generations, a snapshot registry | immutable slices with a copy-on-write publish | Present |
| Persistence | `sheaf-storage/src/lsp/wal.rs`, `lsp/ring_wal.rs`, `lsp/compactor.rs` | a buffered writer with a group-commit header, a hardware CRC32C ring | direct append with kernel bypass | Partly present |
| Core pinning | `sheaf-storage/src/lsp/compactor.rs` line 41 | one pinned core for the compactor | one core per plane | Partly present |

BCSF is the Block Compressed Sparse Fiber, the flat segment format of this engine.

---

## 3. The mechanisms that already exist

Rows = the mechanism in the source architecture. Columns = where it lives, the measured number, and the source.

| Mechanism | Where it lives | Measured number | Source |
|---|---|---|---|
| Immutable read slice | `bcsf/reader.rs` serves views over an immutable segment. `lsp/mem_complex.rs` keeps liveness in atomic bits. | Lookup median 11.64 ns over 1.2 million cells | `SKELETON.md` METHOD-001 |
| Lock-free ring | `lsp/ring_wal.rs` uses a claim ticket, a sequence number, and the SSE4.2 CRC32C instruction. | Append path carries no mutex | `lsp/ring_wal.rs` |
| Pre-aggregated read plane | `lsp/preagg.rs` keeps one accumulator per group. | 938,000 events per second at ingest, 5.3 us to read 5 groups | `SKELETON.md` METHOD-025 |
| O(1) snapshot | A versioned tombstone table replaces the copied bitmap. | Snapshot creation 770.71 us to 0.23 us | `CONTEXT.md` section 13 |
| Core pinning | `lsp/compactor.rs` pins the merge to one core through the core_affinity crate. | Merge rate held 644 to 734 million cells per second inside the merge window | `SKELETON.md` METHOD-003 |
| Range read of the needed bytes | `bin/shared/imdb_fiber.rs` line 208 seeks to one section and verifies its CRC32C. | The 7.14x whole-file waste does not apply to this path | `ARCHITECTURE_GAINS.md` section 3.3 |

**The state plane of the architecture is the design this repository already follows.** The base is immutable. The liveness is a bit. The reader takes a view. The write path is the one part that differs.

---

## 4. The three deltas

### 4.1 Delta 1: the write path becomes a single-writer sequencer

Rows = the step in the current commit path. Columns = the code, the lock or the allocation, and the count.

| Step | Code | Lock or allocation | Count |
|---|---|---|---|
| 1 | `transaction/manager.rs` line 68 | takes the striped face locks in sorted order | one per distinct face |
| 2 | `transaction/manager.rs` line 70 | reads the global commit clock under a mutex | one global mutex |
| 3 | `transaction/manager.rs` line 80 | calls `insert_hypercell` for each write | none |
| 4 | `lsp/mem_complex.rs` line 140 | locks the global WAL writer | one global mutex per inserted cell |
| 5 | `lsp/mem_complex.rs` line 179 | locks the writer shard | one mutex per inserted cell |
| 6 | `lsp/mem_complex.rs` line 182 | copies the boundary into a new vector | one heap allocation per inserted cell |
| 7 | `lsp/mem_complex.rs` line 191 | locks the Upper-Star stripe for each face | one mutex per boundary face |
| 8 | `transaction/manager.rs` line 83 | writes the global commit clock under the mutex | one global mutex |

The star index has 64 stripes, so step 7 takes one lock per face of the boundary. A hypercell with three faces takes three stripe locks.

The current path and the proposed path differ in shape:

```
  CURRENT PATH, four writers

  writer 1 --+
  writer 2 --+--> global WAL mutex --> shard mutex --> star stripe mutex
  writer 3 --+                                         (one per face)
  writer 4 --+

  PROPOSED PATH, one sequencer

  writer 1 --+
  writer 2 --+--> command ring, one cache line per command --> sequencer
  writer 3 --+                                                        |
  writer 4 --+                                          WAL append, shard write,
                                                            star update
```

The sequencer removes steps 1, 2, 4, 5, 7, and 8 from the writer threads. The writers then only build a command and publish it.

**The correctness question.** The commit path today uses a face clock to detect a conflict, and it aborts the second writer. A sequencer owns one order, so it can run the same check in one place. The striped face locks then become unnecessary, because only one thread takes them.

**The cost.** The sequencer becomes a single point of serialization. The write gate in `SPEC.md` requires 4 writers at 50,000 hyperedges per second, so the sequencer must hold that rate. No measurement of a sequencer exists in this repository.

### 4.2 Delta 2: the front end becomes a decode plane

The front end of this repository already does the work the architecture assigns to its input-output plane, except for the socket.

| Work | Code today | Note |
|---|---|---|
| Read the file | `bin/shared/fiber_snapshot.rs` line 45 calls `std::fs::read`, which reads the whole file | The query needs 201,692,415 bytes and the read moves 1,439,107,807 bytes |
| Read only the needed bytes | `bin/shared/imdb_fiber.rs` line 208 seeks and reads one section | The sectioned reader already removes the 7.14x waste |
| Verify the bytes | `bin/shared/imdb_fiber.rs` line 222 checks the CRC32C of the section | A hardware instruction, not a software loop |
| Turn text into a query | `sheaf-query/src/syntax/lexer.rs` | The parser runs on the caller thread today |

Two facts shape this delta. First, the whole-file read still exists in the first reader, so the 7.14x waste is not gone from every path. Second, the architecture names a socket and this repository does not have one.

### 4.3 Delta 3: the storage plane gets its own thread and its own ring

| Property | Today | Source |
|---|---|---|
| WAL writer | a 64 KiB buffered writer over a pre-allocated 8 MiB file | `lsp/wal.rs` lines 57 and 58 |
| Durability point | the flush writes the valid length and its CRC, so a torn header is visible | `lsp/wal.rs` line 75 |
| Device sync | the flush does not call a device sync | `lsp/wal.rs` line 75 |
| Durable write cost | 1307.6 us for a 4 KiB sync | `VON_NEUMANN_LIMITS.md` section 0 |
| Epoch append path | median 2.5 us per append, 20,000 epochs replay in order | `SKELETON.md` METHOD-026 |
| Lock-free ring | present, with a claim ticket and hardware CRC32C | `lsp/ring_wal.rs` |

**The durability floor stays.** A kernel-bypass append removes the system call and the page cache copy. It does not remove the cost of the device flush, which this host measures at 1307.6 us. The current design avoids that cost by not syncing on every commit, and by paying the floor only at a group-commit point.

---

## 5. The silicon claims against the measured ladder

Rows = the claim in the source architecture. Columns = the measured value on this host, and the relation.

| Claim in the source architecture | Measured value on this host | Relation |
|---|---|---|
| A handoff across the L3 costs 25 ns | L3 latency is 21.97 ns, 62.6 cycles | Consistent |
| A handoff between sibling threads costs 1 to 3 ns | Level 1 data cache (L1d) latency is 1.64 ns, 4.6 cycles | Consistent, and only for a pair that shares one core |
| The code loop fits in 16 KiB of level 1 instruction cache (L1i) | L1i is 32 KiB per core | Consistent |
| Instruction misses fall below 0.2 percent | not measured on this host | Unverified |
| Branch prediction stays above 99.5 percent | not measured on this host | Unverified |
| One sequencer core handles 5 to 10 million operations per second | the write gate is 4 writers at 50,000 hyperedges per second | Unverified, and 25x to 50x the gate |
| The domain budget is 64 MB | the L3 on this host is 12 MiB | Conflict, the budget is 5.3 times the real cache |

Three observations follow from the ladder.

1. **Sibling threads share one core.** Two threads on the two hardware threads of one physical core share the execution resources and the L1d. A pair that shares L1d also shares the issue width. On a 4 core host the compute plane then has one core, not two.
2. **The L3 cliff is steep here.** The measured L2 to L3 step is 4.27 ns to 21.97 ns, a factor of 5.1 (`VON_NEUMANN_LIMITS.md` section 1.1). A domain that is sized to 64 MB sits in DRAM at 163.7 ns on this host.
3. **The host runs Windows 11.** The `io_uring` interface does not exist there, and DPDK needs a Linux kernel with specific drivers. `CONTEXT.md` section 3 states that the build order never compiles to an imaginary target.

**A note on the bimodal inlining.** The source architecture inlines the business logic in the input-output worker when the queue is empty. That rule applies to a read only. A write that runs in the input-output worker creates a second writer, and the one-writer order of the state disappears.

---

## 6. The gain table

Rows = the candidate change. Columns = the measured baseline, the expected gain, the basis, and the confidence.

| Candidate | Measured baseline today | Expected gain | Basis of the expectation | Confidence |
|---|---|---|---|---|
| Remove the global WAL mutex | commit path 99th percentile (P99) 143 to 435 us | not measured | the mutex serializes every writer | none |
| Remove the global commit-clock mutex | same baseline | not measured | one global mutex is taken twice per commit | none |
| Remove the per-face star lock | 64 stripes, one lock per face | not measured | the lock count grows with the boundary size | none |
| Remove the per-insert allocation | not measured | not measured | `boundary.to_vec()` copies the boundary for every insert | none |
| One cache line per command in the ring | the ring carries a fixed slot with a 52 byte payload limit | handoff 1 to 3 ns | L1d latency is 1.64 ns | medium |
| Read the base off the sequencer | lookup median 11.64 ns | no change | this already happens | high |
| Range read of the needed sections | 1,439,107,807 bytes read for 201,692,415 bytes needed | 7.14x less read traffic | measured | high |
| Pre-aggregate the query shape | 12,531 ms for the four-table top-10 query | 257x to 569x | measured | high |
| Break the accumulator chain | 5.28 ms for the one-lane scan | 1.40x | measured | high |
| Direct append to the drive | the WAL appends into a 64 KiB buffer | not measured | a device sync costs 1307.6 us and the buffer already amortizes it | low |
| Pin one core per plane | the compactor is pinned to one core | not measured | no instruction miss rate was measured on this host | none |
| Inline a read in the front end | not measured | not measured | the rule is sound for a read and breaks the one-writer rule for a write | none |

**The table has one pattern.** Every row with a measured gain is a bytes reduction. Every row that removes a lock has no measurement yet. The architecture removes locks. The largest measured gains in this repository come from moving fewer bytes.

---

## 7. The three conflicts

Each conflict gives the architecture element, the recorded decision with its source, and two options with their consequence. This document does not choose an option.

### 7.1 Conflict 1: the socket plane against the in-process decision

| Item | Content |
|---|---|
| Architecture element | An input-output plane that terminates a network protocol, decrypts, and grants pacing tokens |
| Recorded decision | The engine runs in process. A remote hop turns a 10.5 ns read into about 70 us, so the plan deleted the client and server split. `CONTEXT.md` sections 11 and 12 |
| Option A | Keep the socket plane for remote access. The nanosecond class of win does not cross a socket, so only the millisecond class of win survives the hop. |
| Option B | Keep the engine in process and give the front end a different job. The front end then decodes and verifies bytes, and the plane split still holds for the local case. |

### 7.2 Conflict 2: the kernel-bypass storage plane on a Windows host

| Item | Content |
|---|---|
| Architecture element | A storage plane that appends to the drive with `io_uring` and `O_DIRECT`, and polls with DPDK |
| Recorded decision | The host runs Windows 11. The build order never compiles to an imaginary target. `CONTEXT.md` section 3 |
| Option A | Add a Linux server build that uses `io_uring` and direct append. The durable write floor of 1307.6 us still applies, and the gain needs a measurement on that host. |
| Option B | Keep one portable storage path for every host. The buffered writer with the group-commit header stays, and the epoch append path at a median of 2.5 us stays the durable append. |

### 7.3 Conflict 3: one global sequencer against the distributed rules

| Item | Content |
|---|---|
| Architecture element | One sequencer for the whole state, so every mutation has one order |
| Recorded decision | Border cells merge by minimum Dirichlet energy, with no two-phase commit and no barriers. The design is a CRDT gluing over an asynchronous Schwarz iteration. `SPEC-026` and `SPEC-027` |
| Option A | One sequencer per domain. Each domain keeps its own order, and the border merge rules still apply across domains. |
| Option B | Keep the current multi-writer commit path with the face clock and the striped face locks. The distributed rules stay unchanged, and the global lock surface stays. |

---

## 8. The ceilings this architecture does not move

Rows = the ceiling. Columns = the floor or the measured value, and the effect on the planes.

| Ceiling | Floor or measured value | Effect on the planes |
|---|---|---|
| Memory bandwidth | 25.6 GB/s for one channel, 11.5 GB/s at one thread | Pinning a core does not raise the bus ceiling |
| Parallel speedup | 1.34x ceiling fitted to the measured scaling, 1.24x measured | More threads do not help, and a plane per core spends the same budget |
| Durable write | 1307.6 us for a 4 KiB device sync | A direct append does not remove the sync |
| DRAM random access | 163.7 ns cold | A pinned core does not shorten the access |
| Page walk | 46.6 ns measured, 534.4 ns for a four-level walk at 4 KiB pages | Large pages need a privilege the host does not grant |
| Soft error rate | 2.13 errors per day in 16 GiB | Every frame needs a checksum, and the CRC32C ring already carries one |
| NAND endurance | 3.2 years at 0.1 device writes per day | The write budget for the whole drive is 51.2 GB per day |
| Quantum and Landauer limits | 6.0e33 operations per second at 1 watt, 2.87e-21 J per bit | Not binding, so the planes do not change them |

**The largest remaining gap is not a ceiling.** The measured database scan reaches 42.3 percent of the memory ceiling at one thread and 80.8 percent at four threads. The missing time sits in a single accumulator inside `sheaf-kernel/src/simd/mt_scan.rs`. Four lanes on the same bytes run 1.47x faster. `SKELETON.md` METHOD-053

---

## 9. What this document does not claim

- It does not claim a gain from any lock removal. No measurement of a sequencer exists in this repository.
- It does not claim that the branch prediction figure or the operations-per-second figure from the source architecture holds here. Both are unmeasured on this host.
- It does not decide the three conflicts in section 7.
- It does not claim that the architecture solves the three largest measured gaps. It does not.

---

## 10. Evidence index

Rows = the claim. Columns = the file that holds the evidence.

| Claim | File |
|---|---|
| Lookup median 11.64 ns, memory 1.220x raw | `SKELETON.md` METHOD-001 |
| Commit path P99 143 to 435 us, conflict abort path | `SKELETON.md` METHOD-007 |
| Merge rate 644 to 734 million cells per second in the merge window, P99 write 47 to 137 us | `SKELETON.md` METHOD-003 |
| Pre-aggregate ingest 938,000 events per second, readout 5.3 us | `SKELETON.md` METHOD-025 |
| Epoch append median 2.5 us, 20,000 epochs replay in order | `SKELETON.md` METHOD-026 |
| Read amplification 13.0 from the fiber against 6001.2 from a base scan, 208 B against 24,004,860 B | `SKELETON.md` METHOD-055 |
| Latency ladder, 25.6 GB/s ceiling, 1307.6 us sync, 12 MiB L3, Amdahl 1.34x against 1.24x | `VON_NEUMANN_LIMITS.md` sections 0 and 1.1 |
| Kernel utilization 42.3 percent at one thread, accumulator 1.47x | `SKELETON.md` METHOD-053 |
| Read waste 7.14x, pre-aggregate 257x to 569x, accumulator 1.40x, page walk 534.4 ns | `ARCHITECTURE_GAINS.md` sections 3.3, 3.4, 3.5, 3.6 |
| In-process decision, 10.5 ns to about 70 us over a loopback | `CONTEXT.md` section 12 |
| Snapshot 770.71 us to 0.23 us, shadow merge 1 ms against 6 ms | `CONTEXT.md` section 13 |
| Global WAL mutex, per-insert allocation, per-face star lock | `sheaf-db/crates/sheaf-storage/src/lsp/mem_complex.rs` lines 140, 182, 191 |
| Global commit-clock mutex, striped face locks | `sheaf-db/crates/sheaf-engine/src/transaction/manager.rs` lines 40, 68, 70, 83 |
| Buffered WAL writer, group-commit header, no device sync | `sheaf-db/crates/sheaf-storage/src/lsp/wal.rs` lines 57, 58, 75 |
| Lock-free ring with claim ticket and hardware CRC32C | `sheaf-db/crates/sheaf-storage/src/lsp/ring_wal.rs` |
| Whole-file read | `sheaf-db/crates/sheaf-engine/src/bin/shared/fiber_snapshot.rs` line 45 |
| Section seek and CRC32C verify | `sheaf-db/crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` lines 208 and 222 |
| Core pinning of the compactor | `sheaf-db/crates/sheaf-storage/src/lsp/compactor.rs` line 41 |

**Document generated:** 2026-10-22, Asia/Kolkata (UTC+05:30).
