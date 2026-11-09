# MANIFEST_ROUTER_RESTRUCTURING_PLAN_2026-10-27.md

## 1. Verdict

The proposal helps the network edge. It does not help the database core. The existing sheaf-db engine runs in one process and has no network ingress. The deferred `sheaf-net` crate is the correct home for this work. The core storage, query, and transaction code must stay unchanged in the first phases.

The proposal brings three useful ideas. It brings zero-copy frames, fixed-size aligned structures, and batched lock-free rings. The existing storage crate already uses bump arenas, ring WAL, and io_uring WAL. Those ideas fit the existing style.

The proposal also brings three conflicts. It assumes a single writer, but the existing transaction manager uses face locks and MVCC snapshots. It assumes a connection arena of one million slots, which costs 64 MiB and exceeds the 38 MiB RSS budget. It assumes AF_XDP, which is Linux only, while development runs on Windows.

Recommendation: implement the proposal as an optional, feature-gated crate. Keep the core engine multi-writer. Measure before replacing any concurrency primitive.

---

## 2. Analysis against the existing architecture

### 2.1 What aligns

- Zero heap allocation on the hot path matches the existing zero-copy reader in `crates/sheaf-storage/src/bcsf/reader.rs`.
- Fixed 64-byte ring slots match the existing cache-line alignment discipline.
- Batched writes match the existing ring WAL in `crates/sheaf-storage/src/lsp/ring_wal.rs`.
- io_uring flush matches the existing `crates/sheaf-storage/src/lsp/io_uring_wal.rs`.
- Packed frame headers match the existing descending alignment rule.

### 2.2 What conflicts

| Proposal element | Existing element | Conflict |
|---|---|---|
| Single-writer invariant | `TxnManager` with `FaceLocks` and `Mutex<HashMap>` | Removing locks breaks MVCC conflict detection |
| 1,048,576 connection slots | 38 MiB RSS budget | 64 MiB arena alone exceeds the budget |
| AF_XDP and UMEM | Windows development host | Linux only, needs a mock backend for tests |
| Manifest router on raw frames | In-process TQL pipeline | Frame serialization adds overhead for local calls |
| no_std and unsafe hot path | Safe Rust codebase | Raises review and audit cost |
| 64-packet batches | Single query execution | Batch framing is not needed for local IPC |

### 2.3 Effect on latency

- Local read latency: no improvement. A frame round trip adds serialization cost.
- Local write latency: possible improvement only if single-writer replaces face locks. That change breaks transaction semantics.
- Remote client latency: projected improvement. The proposal removes 32 interrupts per 64-packet batch and cuts wire bytes by about 16 percent.
- Memory: projected regression if the full arena ships. Reduce the session cap or shard the arena.

### 2.4 Decision

Keep the database core as it is. Add the network transport as a sidecar capability. Let the transport call the existing engine through its public APIs.

---

## 3. Scope and non-goals

### 3.1 In scope

- A new crate `sheaf-net`.
- Frame structures with compile-time size checks.
- A connection arena with a configurable session cap.
- Lock-free rings for inter-stage handoff.
- The manifest router and the NACK state machines.
- A bridge that turns a frame into a call to the existing engine.
- A mock UDP backend for Windows tests.
- An AF_XDP backend behind a Linux feature flag.
- Layout, loss, and benchmark gates.

### 3.2 Out of scope

- Replacing `TxnManager` or `FaceLocks`.
- Changing storage layouts or compaction.
- Moving the core to no_std.
- Removing the existing transaction semantics.
- Claiming the unverified 780,000 writes per second number.

---

## 4. Phase plan

### Phase 0. Analysis and gate definition

- Input: this plan.
- Output: approved plan file and traceability entries.
- Exit: review of sections 5, 6, and 7.

### Phase 1. Crate skeleton and layout tests

- Create `sheaf-net` with an empty library.
- Add frame structs with `#[repr(C, packed)]`.
- Add compile-time size assertions.
- Exit: `cargo test -p sheaf-net` passes on Windows.

### Phase 2. Connection arena and rings

- Implement `ConnectionSlot` and `ConnectionArena`.
- Implement `LockFreeRing` and `RingEntry`.
- Add single-producer, single-consumer tests.
- Exit: layout tests and ring stress tests pass.

### Phase 3. Manifest router

- Implement manifest parsing and bitset tracking.
- Implement semantic dispatch by opcode.
- Exit: router unit tests pass, including hard desync.

### Phase 4. NACK state machines

- Implement sender and receiver states.
- Implement selective retransmission by bitmask.
- Exit: 5 percent loss simulation passes.

### Phase 5. Bridge to the engine

- Add a feature-gated module in `sheaf-engine`.
- Map frames to existing read and write paths.
- Keep all storage calls on the existing API.
- Exit: integration test runs one query end to end.

### Phase 6. Egress and compression

- Add the pacer and the static dictionary compressor.
- Exit: compressed frame size meets the 171-byte target in the test corpus.

### Phase 7. AF_XDP backend

- Linux only.
- Replace the mock backend behind a feature flag.
- Exit: runs on a Linux host with an XDP-capable NIC.

### Phase 8. Benchmarks and gates

- Run the three verification suites from the proposal.
- Record measured numbers, not projected numbers.
- Exit: all gates pass or the node reopens.

---

## 5. Existing files to modify

| File | Change | Description |
|---|---|---|
| `sheaf-db/Cargo.toml` | Append member | Add `crates/sheaf-net` to the workspace members list |
| `sheaf-db/crates/sheaf-engine/Cargo.toml` | Append dependency | Add `sheaf-net` as an optional dependency under feature `net` |
| `sheaf-db/crates/sheaf-engine/src/lib.rs` | Append module | Add `#[cfg(feature = "net")] pub mod net;` |
| `SKELETON.md` | Append and modify | Add the NetworkTransport bounded context, file IDs, and folder IDs |
| `ARCHITECTURE.md` | Modify after Phase 5 | Remove the strikethrough on `sheaf-net` only after the gate passes |
| `sheaf-db/crates/sheaf-storage/src/lib.rs` | No change | The bridge uses the existing public API |
| `sheaf-db/crates/sheaf-engine/src/transaction/manager.rs` | No change | Transaction semantics stay intact |
| `sheaf-db/crates/sheaf-engine/src/transaction/lock.rs` | No change | Face locks stay intact |

No existing core file is deleted in this plan.

---

## 6. New files

### 6.1 Crate root

- `sheaf-db/crates/sheaf-net/Cargo.toml`
- `sheaf-db/crates/sheaf-net/src/lib.rs`

### 6.2 Protocol frames

- `sheaf-db/crates/sheaf-net/src/protocol/mod.rs`
- `sheaf-db/crates/sheaf-net/src/protocol/unified_frame.rs`
- `sheaf-db/crates/sheaf-net/src/protocol/manifest_frame.rs`
- `sheaf-db/crates/sheaf-net/src/protocol/nack_frame.rs`
- `sheaf-db/crates/sheaf-net/src/protocol/flags.rs`

### 6.3 Connection arena

- `sheaf-db/crates/sheaf-net/src/arena/mod.rs`
- `sheaf-db/crates/sheaf-net/src/arena/connection_slot.rs`
- `sheaf-db/crates/sheaf-net/src/arena/connection_arena.rs`

### 6.4 Rings

- `sheaf-db/crates/sheaf-net/src/ring/mod.rs`
- `sheaf-db/crates/sheaf-net/src/ring/ring_entry.rs`
- `sheaf-db/crates/sheaf-net/src/ring/lock_free_ring.rs`

### 6.5 Router

- `sheaf-db/crates/sheaf-net/src/router/mod.rs`
- `sheaf-db/crates/sheaf-net/src/router/context_router.rs`
- `sheaf-db/crates/sheaf-net/src/router/dispatch_matrix.rs`

### 6.6 State machines

- `sheaf-db/crates/sheaf-net/src/state/mod.rs`
- `sheaf-db/crates/sheaf-net/src/state/sender.rs`
- `sheaf-db/crates/sheaf-net/src/state/receiver.rs`

### 6.7 Ingress and egress

- `sheaf-db/crates/sheaf-net/src/ingress/mod.rs`
- `sheaf-db/crates/sheaf-net/src/ingress/mock_udp.rs`
- `sheaf-db/crates/sheaf-net/src/ingress/af_xdp.rs`
- `sheaf-db/crates/sheaf-net/src/egress/mod.rs`
- `sheaf-db/crates/sheaf-net/src/egress/pacer.rs`
- `sheaf-db/crates/sheaf-net/src/egress/compressor.rs`

### 6.8 NACK handling

- `sheaf-db/crates/sheaf-net/src/nack/mod.rs`
- `sheaf-db/crates/sheaf-net/src/nack/emitter.rs`
- `sheaf-db/crates/sheaf-net/src/nack/retransmit.rs`

### 6.9 Bridge and tests

- `sheaf-db/crates/sheaf-net/src/bridge/mod.rs`
- `sheaf-db/crates/sheaf-net/src/bridge/db_bridge.rs`
- `sheaf-db/crates/sheaf-net/tests/layout_tests.rs`
- `sheaf-db/crates/sheaf-net/tests/ring_tests.rs`
- `sheaf-db/crates/sheaf-net/tests/nack_tests.rs`

---

## 7. Folder structure changes

```text
sheaf-db/
  crates/
    sheaf-net/                         (new crate)
      Cargo.toml
      src/
        lib.rs
        protocol/
        arena/
        ring/
        router/
        state/
        ingress/
        egress/
        nack/
        bridge/
      tests/
```

The new crate sits beside the other crates. It depends on `sheaf-engine` only through the feature-gated bridge. The core crates do not depend on `sheaf-net`.

---

## 8. Traceability

| New ID | Plain-English name | Maps to |
|---|---|---|
| REQ-060 | Optional network transport requirement | Proposal sections 1 to 8 |
| SPEC-060 | Manifest router and NACK protocol specification | Proposal data structures and state machines |
| FOLDER-021 | Network transport folder | `crates/sheaf-net/src` |
| FILE-115 | Crate root | `crates/sheaf-net/src/lib.rs` |
| FILE-116 | Unified frame header | `protocol/unified_frame.rs` |
| FILE-117 | Manifest frame | `protocol/manifest_frame.rs` |
| FILE-118 | Selective NACK frame | `protocol/nack_frame.rs` |
| FILE-119 | Connection slot | `arena/connection_slot.rs` |
| FILE-120 | Connection arena | `arena/connection_arena.rs` |
| FILE-121 | Ring entry | `ring/ring_entry.rs` |
| FILE-122 | Lock-free ring | `ring/lock_free_ring.rs` |
| FILE-123 | Context router | `router/context_router.rs` |
| FILE-124 | Sender state machine | `state/sender.rs` |
| FILE-125 | Receiver state machine | `state/receiver.rs` |
| FILE-126 | Database bridge | `bridge/db_bridge.rs` |
| METHOD-067 | Layout gate | Compile-time size assertions |
| METHOD-068 | Ring gate | Single-producer single-consumer stress test |
| METHOD-069 | Router gate | Hard desync and bitset completion tests |
| METHOD-070 | NACK gate | 5 percent loss simulation |
| METHOD-071 | Bridge gate | End-to-end query through a frame |
| METHOD-072 | Memory gate | RSS check with the configured session cap |

---

## 9. Verification gates

### 9.1 Layout gate

- `size_of::<UnifiedFrameHeader>() == 24`
- `size_of::<PreFlightManifestFrame>() == 32`
- `size_of::<SelectiveNackFrame>() == 24`
- `size_of::<RingEntry>() == 64`
- `size_of::<ConnectionSlot>() == 64`

### 9.2 Ring gate

- 1,000,000 claim and publish cycles with no lost slots.
- Consumer reads every published slot in order.

### 9.3 Router gate

- Manifest sets the bitset to zero.
- Each data frame sets the correct bit.
- Full bitset commits with zero return packets in the mock.
- Out-of-window sequence triggers a desync signal.

### 9.4 NACK gate

- 5 percent dropped frames produce NACKs for only the missing offsets.
- Return packet rate equals the loss rate.
- Mean latency degrades by less than 150 microseconds.

### 9.5 Memory gate

- With `MAX_SESSIONS = 65,536`, the arena costs 4 MiB.
- Total process RSS stays under 64 MiB in the transport test.

### 9.6 Core regression gate

- All existing tests in `sheaf-core`, `sheaf-storage`, `sheaf-kernel`, `sheaf-query`, and `sheaf-engine` still pass.
- No file outside the listed modification set changes.

---

## 10. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Single-writer assumption breaks transactions | Keep `TxnManager` unchanged and bridge through public APIs |
| AF_XDP unavailable on Windows | Use the mock UDP backend for all default tests |
| Memory budget exceeded | Default to 65,536 sessions and make the cap configurable |
| Unsafe code audit burden | Isolate unsafe code in `ingress/af_xdp.rs` and `ring/lock_free_ring.rs` |
| Unverified performance claims | Label all proposal numbers as projected until measured |
| Scope creep into core | Feature gate `net` and keep the crate optional |

---

## 11. Rollback

Disable the `net` feature. Remove `sheaf-net` from the workspace members list. The core engine returns to its current state. No storage data format changes, so no data migration is needed.
