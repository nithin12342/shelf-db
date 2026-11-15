# Missing feature implementation plan (database scope)

**Date:** 2026-11-15
**Branch:** `combined-stack-feature-improvement`
**Input audit:** the attached feature audit. It tracks 48 features over 11 pillars: 33 implemented, 6 partial, 9 absent.
**Scope policy:** `AGENTS.md`. This plan covers database functionality only. The PyTorch bridge and all training work stay out of scope. They do not count as unfinished database work.
**Status of this document:** plan only. No node in this document is complete. A node becomes complete only after the gate in section 3 passes.

---

## 1. What this plan answers

The audit gives 15 open features and one code defect. This plan adds one supporting node, the versioned page format, because the codec work needs a versioned container. The set is 17 nodes. Each node names new files, a port and adapter boundary, the invariants, the tests, the evidence file, and the gate.

The order is a dependency order. Each phase builds the input that the next phase needs.

---

## 2. Gap inventory

Table 1 answers this question: which audited feature is open, and what does this plan replace it with.

Rows are open features. Columns are the audit state, the current code location, and the target state.

| ID | Pillar | Feature | Audit state | Current location | Target state |
|---|---|---|---|---|---|
| MF-00 | Compliance | File over 500 lines | Defect | `crates/sheaf-storage/src/bcsf/segment.rs` (666 lines) | Six files, each under 500 lines, byte-identical output |
| MF-01 | 4 | Lock-free in-process nerve routing | Partial, uses `Mutex` queues | `crates/sheaf-runtime/src/nerve.rs` | Bounded lock-free multi-producer multi-consumer queues with a stress test |
| MF-02 | 4 | Thread-per-core OS affinity and NUMA pinning | Model only | `crates/sheaf-runtime/src/affinity.rs` | Real OS thread pinning with mask readback |
| MF-03 | 10 | Concurrent execution of disjoint write waves | Plan only | `crates/sheaf-vcs/src/lca.rs` | Real multi-threaded wave executor with overlap proof |
| MF-04 | 2 | Physical L3 dirty-region partitioning | Model only | `crates/sheaf-runtime/src/affinity.rs`, `sheaf-storage/src/bcsf/partition.rs` | OS L3 cache discovery and a stable partition map |
| MF-05 | 9 | FSST, ALP, frame-of-reference, bit-packing | Absent | none | New codec crate with round-trip and ratio tests |
| MF-06 | 9 | Global dictionary-vector arena sharing | Partial | `sheaf-native` bump arenas | Cross-column string dictionary with one copy per unique value |
| MF-07 | 5 | Page format with explicit codec and version gate | Implicit v1 | `crates/sheaf-app/src/storage.rs` | Versioned page format, unknown version is an error |
| MF-08 | 3 | Pre-registered `io_uring` buffers | Absent | `native/wal-c/src/platform_uring.c` | Fixed-buffer registration on Linux, explicit capability report elsewhere |
| MF-09 | 6 | Lean AOT compilation to C rule tables | Absent | `native/proofs-generated/sheaf_verified_rules.h` (handwritten) | Header and manifest emitted by Lean, drift fails the build |
| MF-10 | 6 | Lean-verified production optimizer | Absent | `crates/sheaf-proofs/src/planner.rs` (toy model) | Rewrite rules in `sheaf-query`, gated by a rule oracle |
| MF-11 | 8 | Cohomological obstruction H1 quotient solver | Partial | `crates/sheaf-vcs/src/diff.rs` | Least-squares global representative plus harmonic obstruction class |
| MF-12 | 7 | Zero-copy RoCEv2 RDMA replication | Absent | none | Replication port, real TCP adapter, explicit RDMA adapter boundary |
| MF-13 | 7 | Distributed cluster consensus | Absent | none | Deterministic Raft engine with a simulated cluster |
| MF-14 | 7 | Canonical open poset cover sharding | Model only | `crates/sheaf-vcs/src/complex.rs` | Shard assignment, ghost cells, cross-shard glue check |
| MF-15 | 11 | Arrow Flight or gRPC daemon | Absent | `crates/sheaf-app` (pgwire only) | Real Flight service under an opt-in feature |
| MF-16 | 11 | eBPF VFS interception | Absent | none | Policy engine plus Linux loader boundary |

**Scope correction.** The audit lists the PyTorch bridge under Pillar 11. That item is training infrastructure. `AGENTS.md` excludes it. This plan does not list it, and its stub state does not count as a database gap.

---

## 3. Definition of done for one node

These rules apply to every node. They follow the new instruction in `AGENTS.md`.

1. New feature code lives in new files. Do not grow an existing file to hold the feature.
2. No file exceeds 500 lines after the change. Run the line count before the commit.
3. Ports are traits. Adapters implement the trait. Callers depend on the trait only.
4. The test calls the production path with real input. A constant inside the code must not produce the expected value.
5. The test compares the real output with a stated expected result.
6. A platform-gated path returns an explicit capability result or error. It does not return a fake value.
7. A declared fallback reports its use through a return value or a log record. It is never silent.
8. Evidence is a file under `sheaf-db/tests/e2e/missing_features/<node>/`. It holds `command.txt`, `output.txt`, and `result.json`.
9. `cargo test` and `cargo clippy -- -D warnings` pass for every touched crate before the commit.
10. One commit per file creation or file modification. The commit message names the node and the test command.

Gate for a node: rules 1 to 10 pass, and the evidence files exist in the working tree.

---

## 4. Phase order

```
Phase 0  Compliance            MF-00
            |
Phase 1  Runtime concurrency   MF-02 -> MF-01 -> MF-03
            |                        |
            |                     MF-04
            v
Phase 2  Storage compression   MF-05 -> MF-06 -> MF-07
            |
Phase 3  WAL hardware          MF-08
            |
Phase 4  Proof pipeline        MF-09 -> MF-10
            |
Phase 5  Topology and cluster  MF-11 -> MF-14 -> MF-12 -> MF-13
            |
Phase 6  Ecosystem             MF-15 -> MF-16
```

Why this order:

- MF-00 removes the file-size violation before new code lands.
- MF-02 gives MF-01 and MF-03 real core identity. A lock-free queue without a pinned consumer is only a faster mutex.
- MF-04 gives the storage layer the real L3 domains that MF-06 and MF-07 use for arena placement.
- MF-10 needs MF-09, because the rewrite gate reads the generated rule table.
- MF-14 needs MF-12 to move shard sections between nodes.
- MF-13 needs MF-12 to carry log entries.
- MF-15 needs MF-07, because Flight streams the encoded page format.

---

## 5. Phase 0: compliance

### MF-00 Split `segment.rs`

**Pillar:** none. **Depends on:** none. **Platform:** all. **Size:** S.

New files, all under `crates/sheaf-storage/src/bcsf/segment/`:

| File | Responsibility |
|---|---|
| `mod.rs` | Public API re-exports only |
| `constants.rs` | Magic values, format versions, part tags, header lengths |
| `header.rs` | Part directory encode and decode |
| `zone.rs` | Zone maps for u32, u64, f32, validity |
| `write.rs` | Segment v1 and v2 writers |
| `read_v2.rs` | Segment v2 reader |
| `read_v1.rs` | Segment v1 reader |
| `file.rs` | `open_segment`, mmap open, metadata read |

Logic:

1. Move code without a semantic change.
2. Keep the public symbol names.
3. Write a fixture segment before the split. Hash the bytes with BLAKE3.
4. Write the same fixture after the split. The hash must match.

Tests:

- The existing segment tests pass without an edit.
- The fixture hash is equal before and after.
- Every new file is under 500 lines.

Evidence: `tests/e2e/missing_features/mf00/{command,output,result.json}` holds the two hashes and the line counts.

---

## 6. Phase 1: runtime concurrency and hardware topology

### MF-02 Real thread affinity

**Pillar:** 4. **Depends on:** none. **Platform:** Windows and Linux real, other targets explicit `Unsupported`. **Size:** M.

New files:

- `crates/sheaf-runtime/src/pinning/mod.rs` defines the port `ThreadPinner` with `pin_current(core) -> Result<Mask, PinError>`, `current_mask()`, and `unpin()`.
- `crates/sheaf-runtime/src/pinning/windows.rs` uses `GetCurrentThread` and `SetThreadAffinityMask`, and reads the mask back with `GetThreadGroupAffinity`.
- `crates/sheaf-runtime/src/pinning/linux.rs` uses `sched_setaffinity` and `sched_getaffinity` from `libc`.
- `crates/sheaf-runtime/src/pinning/unsupported.rs` returns `PinError::Unsupported` with the target name.

Logic:

1. Read the allowed CPU set from the OS. Do not use a hard-coded core count.
2. Reject a core outside the set with `PinError::CoreUnavailable`.
3. Pin the calling thread. Read the mask back. The readback must show exactly one bit.
4. Return the previous mask so the caller can restore it.

Tests:

- Pin the test thread to each allowed core in turn. The readback mask has one bit and equals that core.
- Pin to a core outside the set. The call returns `CoreUnavailable`.
- Restore the previous mask. The readback equals the mask before the test.
- A worker thread pinned to core N reports N through `sched_getcpu` on Linux or the group affinity on Windows.

Evidence: `result.json` holds the allowed set, each readback mask, and the restore mask.

### MF-01 Lock-free nerve router

**Pillar:** 4. **Depends on:** MF-02. **Platform:** all, pure Rust. **Size:** M.

New files:

- `crates/sheaf-runtime/src/nerve_queue.rs` holds a bounded multi-producer multi-consumer ring. Each slot has a sequence number. The enqueue and dequeue operations use `compare_exchange` only.
- `crates/sheaf-runtime/src/nerve_port.rs` defines the port `NerveTransport` with `send`, `recv`, `pending`, and `drain`.
- `crates/sheaf-runtime/src/nerve_lockfree.rs` implements the router on top of the ring. One ring per core.

Logic:

1. The queue capacity is a power of two. Round up at construction. Report the rounded capacity.
2. `send` fails with `QueueFull` when the ring is full. It never blocks and never drops.
3. `recv` fails with `Empty`. It never blocks.
4. The router keeps the `PinningPolicy` mapping from MF-02.

Tests:

- Single producer, single consumer, 100,000 messages: order is exact and no message is lost.
- Eight producers, eight consumers, 50,000 messages each: the multiset of received messages equals the multiset of sent messages.
- A full queue returns `QueueFull`. A subsequent `recv` succeeds.
- A drain returns every message once.

Evidence: `result.json` holds sent count, received count, duplicate count, loss count, and wall time.

### MF-03 Parallel wave executor

**Pillar:** 10. **Depends on:** MF-01, MF-02. **Platform:** all. **Size:** M.

New files:

- `crates/sheaf-vcs/src/write_target.rs` defines the port `CellWriteTarget` with `write_cell(cell) -> Result<(), WriteError>`.
- `crates/sheaf-vcs/src/wave_executor.rs` runs waves with `std::thread::scope`. It maps each wave to the pinned workers from MF-02.
- `crates/sheaf-vcs/src/wave_report.rs` records per-wave thread identifiers, cell counts, and elapsed time.

Logic:

1. Take the wave list from `CollisionInspector::parallel_waves`.
2. Run every cell in a wave concurrently. Cells in different waves run in order.
3. A write error stops the wave, joins every worker, and returns the first error.
4. A cell never runs twice. Assert this inside the executor with an atomic counter.

Tests:

- 64 disjoint cells with eight workers: all 64 writes are applied, and the report shows at least two distinct thread identifiers.
- Cells from one triangle wave set: three waves, one cell each, sequential order.
- An injected error in wave two: the executor returns that error and applies no wave-three write.
- A repeated cell in the input is rejected with `DuplicateCell`.

Evidence: `result.json` holds the thread identifiers, wave sizes, applied count, and elapsed time.

### MF-04 Physical L3 domain discovery

**Pillar:** 2. **Depends on:** MF-02. **Platform:** Windows and Linux real. **Size:** M.

New files:

- `crates/sheaf-storage/src/topology/mod.rs` defines the port `CacheTopology` with `l3_domains() -> Result<Vec<L3Domain>, TopologyError>`.
- `crates/sheaf-storage/src/topology/windows.rs` uses `GetLogicalProcessorInformationEx` with `RelationCache`, level 3, unified cache.
- `crates/sheaf-storage/src/topology/linux.rs` reads `/sys/devices/system/cpu/cpu*/cache/index*/`.
- `crates/sheaf-storage/src/topology/map.rs` maps a BCSF partition to the L3 domain that holds the most of its cells. Ties go to the lowest domain id.

Logic:

1. Query the OS. Do not use a fixed `cores_per_domain`.
2. Reject an empty result with `TopologyError::NoL3Cache`.
3. Check that each reported logical CPU appears in exactly one domain.
4. Keep the partition map deterministic.

Tests:

- Discovery returns at least one domain on this host.
- The union of the domains equals the allowed CPU set, and the intersection of two domains is empty.
- A partition with a known cell set maps to the domain with the largest overlap.
- A tie maps to the lowest domain id.

Evidence: `result.json` holds the domain count, the CPU count, the overlap check, and the partition map.

---

## 7. Phase 2: storage compression

### MF-05 Columnar codec crate

**Pillar:** 9. **Depends on:** none. **Platform:** all, pure Rust. **Size:** L.

New crate `crates/sheaf-codec`:

| File | Responsibility |
|---|---|
| `traits.rs` | `ColumnCodec` port with `id`, `encode`, `decode`, `estimate` |
| `bitpack.rs` | Bit packing at 1 to 64 bits per value |
| `for_.rs` | Frame of reference plus bit packing |
| `alp.rs` | Adaptive lossless decimal floats with exact round trip |
| `fsst.rs` | Symbol table training and string encoding, at most 255 symbols |
| `select.rs` | Chooses the codec by measured encoded size. Ties go to the lowest id |
| `error.rs` | Explicit codec errors |

Logic:

1. `select` encodes each block with every codec that accepts the type. It keeps the smallest result. It does not hard-code a winner.
2. FSST training picks symbols by gain over the current table. It stops at 255 symbols or at zero gain.
3. ALP stores a factor and an exponent per block. It verifies the round trip during encode. When the round trip fails, it returns `NotApplicable` for that block, and `select` records the reason.
4. A decoder rejects an unknown codec id with `UnknownCodec`. It does not guess.

Tests:

- Round trip for 100,000 random `u64`, `i64`, `f32`, and strings. The decoded value equals the input value at bit level.
- Edge values: zero, minimum, maximum, NaN payloads, empty strings, 4 KiB strings.
- FSST table never exceeds 255 symbols.
- The encoded size is at most the raw size for the repeated-string corpus.
- An unknown codec id returns `UnknownCodec`.

Evidence: `result.json` holds the chosen codec, the raw size, the encoded size, and the round-trip result per corpus.

### MF-06 Global dictionary arena

**Pillar:** 9. **Depends on:** MF-05. **Platform:** all. **Size:** M.

New files under `crates/sheaf-storage/src/dict/`:

- `arena.rs` holds one append-only byte arena with 64-byte alignment and a stable offset per value.
- `interner.rs` maps a byte string to an offset. The hash is BLAKE3 over the bytes.
- `column.rs` holds `Dictionary`-encoded column values as 32-bit offsets.

Logic:

1. Intern a value once. A second intern returns the same offset.
2. The arena is shared by every text column in one table.
3. A decode reads the arena by offset and checks the length prefix.
4. A corrupt offset returns `InvalidOffset`. It never returns a truncated value.

Tests:

- 1,000,000 inserts over 10,000 unique values produce 10,000 arena entries.
- Two columns with the same value share one offset.
- Decode returns the original bytes for every unique value.
- A corrupt offset returns `InvalidOffset`.

Evidence: `result.json` holds insert count, unique count, arena bytes, and the decode check.

### MF-07 Versioned page format

**Pillar:** 5. **Depends on:** MF-05, MF-06. **Platform:** all. **Size:** M.

New files:

- `crates/sheaf-app/src/page_format.rs` defines the header: magic, version, codec ids, row count, dictionary id, payload length, and a CRC32C over the body.
- `crates/sheaf-app/src/page_encode.rs` encodes rows through `sheaf-codec`.
- `crates/sheaf-app/src/page_decode.rs` decodes and validates.

Logic:

1. Write version 2 pages for new tables.
2. Read version 1 pages through a declared reader. Record `MigratedFromV1` in the table metadata.
3. Reject an unknown version with `UnsupportedPageVersion`. Do not guess the layout.
4. Verify the CRC32C before decode.
5. Keep the stalk hash check for each row.

Tests:

- Round trip of a 16-row page with every data type. The rows are equal.
- An unknown version byte returns `UnsupportedPageVersion`.
- A flipped payload bit returns `ChecksumMismatch`.
- A version 1 fixture reads with `MigratedFromV1` set.

Evidence: `result.json` holds the version, the CRC result, and the row equality result.

---

## 8. Phase 3: WAL hardware path

### MF-08 Pre-registered `io_uring` buffers

**Pillar:** 3. **Depends on:** none. **Platform:** Linux real, Windows explicit capability report. **Size:** M.

New files:

- `native/include/sheaf_wal_regbuf.h` declares the pool API.
- `native/wal-c/src/registered_buffers.c` holds the pool: allocate, register, submit fixed write, unregister.
- `crates/sheaf-native/src/wal/registered.rs` binds the C API and exposes `RegisteredBuffers` with `register`, `write_fixed`, and `capability`.

Logic:

1. On Linux, call `io_uring_register_buffers` once per pool. Submit `IORING_OP_WRITE_FIXED` with the buffer index.
2. On Windows, `capability()` returns `Capability::Unsupported { reason }`. The WAL keeps the normal write path and reports the capability to the caller.
3. A buffer index outside the pool returns `BufferIndexOutOfRange`.
4. Registration failure returns the raw `errno`.

Tests:

- Pool invariant: every buffer is 64-byte aligned and the count matches the registration.
- A double register returns `AlreadyRegistered`.
- On Linux, a 4 KiB fixed write lands on disk and the CRC32C matches the read-back.
- On Windows, `capability()` returns `Unsupported`, and the caller records it.

Evidence: `result.json` holds the platform, the capability value, the write result, and the CRC result.

---

## 9. Phase 4: proof pipeline

### MF-09 Lean rule emitter

**Pillar:** 6. **Depends on:** none. **Platform:** all. Lean runs when `lake` is installed. **Size:** L.

New files:

- `proofs/RuleTable.lean` defines the rule record, the theorem name, and the invariant flag.
- `proofs/Emit.lean` writes `native/proofs-generated/sheaf_verified_rules.h` and `sheaf_verified_rules.json`.
- `proofs/lakefile.lean` gains an executable target `rules`.
- `crates/sheaf-proofs/build.rs` reads the JSON at build time and fails on a mismatch with `rules.rs`.
- `crates/sheaf-proofs/src/generated_rules.rs` includes the generated table. The handwritten table in `rules.rs` is deleted.

Logic:

1. Lean is the only source of the table.
2. The JSON holds the rule id, name, theorem, and invariant flag.
3. The build script checks the ids are unique, the flags are true, and every theorem name exists in the Lean sources.
4. When Lean is absent, the build uses the committed JSON. It does not regenerate silently. The JSON has a header with the Lean version and the source hash.

Tests:

- The runtime table equals the JSON table.
- A changed JSON entry fails the build with a drift error.
- A missing theorem name fails the build.

Evidence: `result.json` holds the source hash, the JSON hash, the rule count, and the drift result. When Lean is absent, the evidence states `generated_by: committed-manifest`.

### MF-10 Production rewrite optimizer

**Pillar:** 6. **Depends on:** MF-09. **Platform:** all. **Size:** L.

New files:

- `crates/sheaf-query/src/ir/rule_oracle.rs` defines the port `RuleOracle` with `is_verified(id)` and `theorem(id)`.
- `crates/sheaf-query/src/ir/rewrite/mod.rs` defines the rewrite engine over `TirOp`.
- `crates/sheaf-query/src/ir/rewrite/pushdown.rs` moves a restriction below a colimit.
- `crates/sheaf-query/src/ir/rewrite/identity.rs` removes an identity restriction.
- `crates/sheaf-proofs/src/oracle.rs` implements `RuleOracle` on top of the generated table.

Logic:

1. The engine asks the oracle before it applies a rule. An unverified rule returns `RuleNotVerified`.
2. The engine records the applied rule ids in the plan for `EXPLAIN`.
3. The app runs the rewritten plan and the original plan on the same input. The result rows must be equal.

Tests:

- Pushdown: 200 generated TIR plans. The row multiset is equal before and after the rewrite.
- Identity elimination: the plan loses the identity stage and keeps the result.
- An unverified id returns `RuleNotVerified` and leaves the plan unchanged.
- `EXPLAIN` lists the applied rule ids.

Evidence: `result.json` holds the plan count, the equal-result count, and the applied rule ids.

---

## 10. Phase 5: topology and cluster

### MF-11 H1 quotient solver

**Pillar:** 8. **Depends on:** none. **Platform:** all. **Size:** M.

New files:

- `crates/sheaf-vcs/src/cohomology.rs` solves `min ||d0 x - r||^2` for the residual `r` with the normal equations `L0 x = d0^T r`. It uses the kernel CG solver.
- `crates/sheaf-vcs/src/harmonic.rs` returns the harmonic representative `h = r - d0 x` and its energy.
- `crates/sheaf-vcs/src/cycle_space.rs` builds the independent cycle basis and the coordinates of `h` in that basis.

Logic:

1. Compute the residual `r = d0 m`.
2. When `r` is zero, return `Obstruction::None`.
3. Otherwise solve the normal equations and return `h`, `||h||^2`, and the cycle coordinates.
4. Check `d0^T h = 0` within tolerance. A failed check returns `SolverError::NotOrthogonal`.

Tests:

- A line graph has `H1 = 0`. The obstruction is none.
- A triangle with one 1-cell cycle has a nonzero obstruction. The expected value comes from the stated equations.
- Two triangles sharing an edge: the cycle dimension is two and the coordinates are exact.
- The orthogonality check passes for every case.

Evidence: `result.json` holds the residual, the harmonic norm, the cycle dimension, and the orthogonality check.

### MF-14 Open cover sharding

**Pillar:** 7. **Depends on:** MF-11, MF-12. **Platform:** all. **Size:** L.

New files under `crates/sheaf-vcs/src/sharding/`:

- `cover.rs` builds the canonical open cover from the star neighborhoods.
- `assignment.rs` assigns each cell to one or more shards by cover membership.
- `ghost.rs` records boundary cells that two shards share.
- `glue.rs` checks the cross-shard gluing residual.

Logic:

1. Every cell belongs to at least one shard.
2. A cell in two shards exists once per shard and once as a ghost link.
3. A global section restricts to a global section on each shard.
4. A nonzero cross-shard residual returns `GlueConflict` with the cells.

Tests:

- A 1,000-cell complex with four shards: the cover check passes and the ghost count equals the boundary count.
- A section that is global on each shard but not globally returns `GlueConflict`.
- A cell is never lost: the union of the shard cells equals the input cells.

Evidence: `result.json` holds shard count, ghost count, cover check, and glue check.

### MF-12 Replication transport

**Pillar:** 7. **Depends on:** MF-03. **Platform:** all for TCP, Linux with a device for RDMA. **Size:** L.

New files under `crates/sheaf-runtime/src/replication/`:

- `port.rs` defines `ReplicaTransport` with `send_section`, `recv_section`, and `capability`.
- `frame.rs` defines the length-framed section record with a BLAKE3 hash.
- `tcp.rs` implements the transport over TCP with `write_vectored`, so the header and the payload do not copy into one buffer.
- `rdma.rs` returns `Capability::Unsupported { reason }` when no RDMA device is present. On Linux with a device it uses one-sided reads for border cells.

Logic:

1. A frame carries the section id, the byte length, the hash, and the payload.
2. A receiver checks the hash before it exposes the section.
3. A short write retries the remaining bytes. A closed peer returns `PeerClosed`.
4. The RDMA adapter reports its capability. It does not pretend to be TCP.

Tests:

- 10,000 sections over loopback: every hash matches and no section is lost.
- A truncated frame returns `TruncatedFrame`.
- A flipped payload bit returns `HashMismatch`.
- `rdma::capability()` reports the truth on this host.

Evidence: `result.json` holds section count, byte count, throughput, and the capability values.

### MF-13 Consensus engine

**Pillar:** 7. **Depends on:** MF-12. **Platform:** all. **Size:** XL.

New files under `crates/sheaf-runtime/src/consensus/`:

| File | Responsibility |
|---|---|
| `state.rs` | Role, term, voted-for, commit index |
| `log.rs` | Append-only log with term and index checks |
| `rpc.rs` | Request-vote and append-entries messages |
| `driver.rs` | The state machine step function, with no clock and no socket |
| `sim.rs` | Deterministic bus with drop, reorder, duplicate, and partition |
| `snapshot.rs` | Snapshot install and restore |

Logic:

1. The driver is deterministic. Time enters as a logical tick.
2. A candidate needs a majority. A minority cannot commit.
3. A committed entry survives a restart. The log check rejects a gap.
4. A partition heals with the log matching rule. The longer log wins only with a higher term.

Tests:

- Three nodes elect one leader.
- A client entry replicates to all nodes and commits.
- A one-node minority cannot commit.
- Kill and restart a follower. It catches up.
- Partition and heal. No committed entry is lost.
- Duplicate and reordered messages do not break the log matching rule.

Evidence: `result.json` holds the scenario names, the final commit index, and the safety checks.

---

## 11. Phase 6: ecosystem

### MF-15 Arrow Flight daemon

**Pillar:** 11. **Depends on:** MF-07. **Platform:** all. **Size:** L.

New files under `crates/sheaf-app/src/flight/`:

- `mod.rs` wires the service behind the `flight` Cargo feature.
- `service.rs` implements `FlightService`: `do_get`, `do_put`, `get_flight_info`, and `list_flights`.
- `ticket.rs` maps a ticket to a table and a page range.
- `config.rs` holds the listen address.

Logic:

1. `do_get` reads the catalog and streams Arrow record batches from the page format.
2. `do_put` writes a record batch through the existing table build path.
3. An unknown ticket returns `FlightError::UnknownTicket`.
4. The default build does not include the feature. The feature build is the only build that carries Flight.

Tests:

- Start the service on an ephemeral port. A real Arrow client calls `do_get` for a known table. The rows equal the source rows.
- `do_put` then `do_get` returns the written rows.
- An unknown ticket returns `UnknownTicket`.
- `list_flights` returns every table once.

Evidence: `result.json` holds the port, the row counts, and the equality result.

### MF-16 eBPF VFS interception

**Pillar:** 11. **Depends on:** none. **Platform:** Linux real, Windows explicit `Unsupported`. **Size:** L.

New files:

- `native/ebpf/sheaf_vfs.bpf.c` and `native/ebpf/sheaf_vfs.h` hold the tracepoint program and the event record.
- `crates/sheaf-runtime/src/vfs/mod.rs` defines the port `VfsPolicy` with `decide(event) -> Decision`.
- `crates/sheaf-runtime/src/vfs/policy.rs` maps a read or write event to `Passthrough`, `Capture`, or `Reject`, from the open file set and the byte range.
- `crates/sheaf-runtime/src/vfs/linux.rs` loads the program with `aya` and streams events.
- `crates/sheaf-runtime/src/vfs/unsupported.rs` returns `Capability::Unsupported`.

Logic:

1. The policy engine is pure and testable from a recorded trace.
2. The Linux loader attaches to the VFS tracepoint and forwards events.
3. A missing capability returns the reason. It does not fake the capture.

Tests:

- 10,000 recorded events: every decision matches the stated rule.
- A read outside the tracked file returns `Passthrough`.
- On Linux, the loader attaches and reports one event from a real file read.
- On Windows, `capability()` returns `Unsupported`.

Evidence: `result.json` holds the event count, the decision counts, and the capability value.

---

## 12. Test matrix

Table 2 answers this question: which test proves each node.

Rows are nodes. Columns are the test kind, the input, the expected result, and the command.

| Node | Test kind | Input | Expected result | Command |
|---|---|---|---|---|
| MF-00 | Byte parity | Fixed BCSF fixture | Equal BLAKE3 hash | `cargo test -p sheaf-storage segment` |
| MF-01 | Stress | 8 producers, 50k each | No loss, no duplicate | `cargo test -p sheaf-runtime nerve` |
| MF-02 | OS readback | Allowed CPU set | One-bit mask per core | `cargo test -p sheaf-runtime pinning` |
| MF-03 | Concurrency | 64 disjoint cells | All applied, 2+ threads | `cargo test -p sheaf-vcs wave` |
| MF-04 | OS query | Host topology | Domains partition the CPU set | `cargo test -p sheaf-storage topology` |
| MF-05 | Round trip | Random and edge corpora | Bit-exact decode | `cargo test -p sheaf-codec` |
| MF-06 | Dedup | 1M inserts, 10k unique | 10k arena entries | `cargo test -p sheaf-storage dict` |
| MF-07 | Format gate | v1, v2, unknown, corrupt | Read, migrate, reject, reject | `cargo test -p sheaf-app page` |
| MF-08 | Fixed write | 4 KiB WAL record | CRC match, capability truth | `cargo test -p sheaf-native wal` |
| MF-09 | Drift | Lean JSON and runtime table | Equal or build error | `cargo test -p sheaf-proofs` |
| MF-10 | Differential | 200 TIR plans | Equal row multisets | `cargo test -p sheaf-query rewrite` |
| MF-11 | Solve | Line, triangle, two triangles | Exact obstruction | `cargo test -p sheaf-vcs cohomology` |
| MF-12 | Loopback | 10k sections | Hash match, no loss | `cargo test -p sheaf-runtime replication` |
| MF-13 | Simulation | 3 nodes, faults | Safety holds | `cargo test -p sheaf-runtime consensus` |
| MF-14 | Cover | 1,000 cells, 4 shards | Cover and glue pass | `cargo test -p sheaf-vcs sharding` |
| MF-15 | Flight client | Real table over localhost | Equal rows | `cargo test -p sheaf-app --features flight` |
| MF-16 | Trace policy | 10k events | Stated decision each time | `cargo test -p sheaf-runtime vfs` |

---

## 13. Risks and platform limits

Table 3 answers this question: what can block a node.

Rows are risks. Columns are the effect and the control.

| Risk | Effect | Control |
|---|---|---|
| RDMA hardware is absent | No real RoCEv2 measurement | The capability report is the deliverable. The TCP adapter carries the functional test. |
| Windows has no `io_uring` and no eBPF | MF-08 and MF-16 cannot run their kernel paths here | The pure decision and pool logic is tested. The kernel path is `cfg(target_os = "linux")` and reported as unsupported here. |
| Lean and Mathlib are not installed | MF-09 cannot regenerate | The committed manifest is the build input. The drift check still runs. The evidence names the generator state. |
| Arrow Flight adds large dependencies | Longer build, more surface | The feature is opt-in. The default build stays small. |
| Consensus is easy to get wrong | Data loss | The driver is deterministic and the simulation covers partition, restart, and duplicate delivery. |
| Codec changes alter page bytes | Old files unreadable | MF-07 writes a version and MF-05 changes only inside a version. |
| 500-line limit | A node grows past the limit | Split by responsibility before the commit. The line count is part of the gate. |

---

## 14. Execution start

The first three nodes are MF-00, MF-02, and MF-01. They are self-contained, testable on this host, and give the later nodes their foundations.

For each node the sequence is:

1. Add the node to `SKELETON.md` with status `planned` and its traceability tuple.
2. Write the port and the adapter skeleton. Compile it with stubbed bodies.
3. Implement the logic.
4. Run the tests and capture the evidence.
5. Change the `SKELETON.md` status to `e2e_passed`, and attach the evidence path.
6. Commit one file at a time.

No node may skip step 3 or step 4. A green build alone does not close a node.
