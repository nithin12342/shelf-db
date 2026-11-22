# Missing feature implementation plan

**Timestamp:** 2026-11-22_09-02-56, local time 09:02:56 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `1aae55de9bbd3e75fdb2a63b26cff7de4b17669b`.
**Input:** `results/CONSOLIDATED_FEATURE_INVENTORY_2026-11-22.md`.
**Scope:** database functionality only. The PyTorch bridge stays excluded.

---

## 1. What this plan answers

The inventory finds 18 open database features. 11 are partial and 7 are absent. This plan puts them into 16 nodes. Each node has one owner, one port, one adapter, one test, and one gate. The order is a dependency order.

Table 1 answers this question: which inventory feature each node closes, and in which phase.

Rows are nodes. Columns are the phase, the node, the inventory features closed, and the dependency.

| Phase | Node | Closes | Depends on |
|---|---|---|---|
| 1 Proof pipeline | NF-01 | F-25, F-26 | Lean toolchain |
| 1 Proof pipeline | NF-02 | F-28 | NF-01 |
| 1 Proof pipeline | NF-03 | F-27 | NF-01 |
| 2 Distributed | NF-04 | F-37 | none |
| 2 Distributed | NF-05 | F-32 | NF-04 |
| 2 Distributed | NF-06 | F-31 | NF-05 |
| 2 Distributed | NF-07 | F-33 | NF-05 |
| 2 Distributed | NF-08 | F-29, F-30, F-34 | NF-04, NF-05 |
| 3 Ecosystem | NF-09 | F-47 | NF-10 |
| 3 Ecosystem | NF-10 | F-17 | NF-09 |
| 3 Ecosystem | NF-11 | F-48 | none |
| 4 Storage and query | NF-12 | F-10 | none |
| 4 Storage and query | NF-13 | F-42 | none |
| 4 Storage and query | NF-14 | F-12, F-13 | Linux host |
| 4 Storage and query | NF-15 | S-02 | none |
| 4 Storage and query | NF-16 | F-22 | none |

**Dependency graph.**

```text
NF-01 --+--> NF-02
        +--> NF-03

NF-04 --> NF-05 --+--> NF-06
                  +--> NF-07
                  +--> NF-08

NF-09 <--> NF-10

NF-12   NF-13   NF-14   NF-15   NF-16   (independent)
```

---

## 2. Gate for every node

These rules apply to all 16 nodes. They repeat the project rules with the node steps.

1. New feature code lives in new files. Do not grow an existing file to hold a new feature.
2. No file exceeds 500 lines. Run the line count before the commit.
3. A port is a trait. An adapter implements the trait. A caller depends on the trait only.
4. The test calls the production path with real input.
5. The test compares the real output with a stated expected result.
6. A constant in the code must not produce the expected result.
7. A declared fallback reports its use and returns an explicit result.
8. A platform-gated path returns `Unsupported` with a named cause. It does not return a fake value.
9. `cargo test` and `cargo clippy -- -D warnings` pass for every touched crate.
10. The evidence directory holds `command.txt`, `output.txt`, and `result.json`.
11. One commit for one file. The commit message names the node and the test command.
12. The commit message and the evidence file name the remote, the branch, and the top commit.

**Evidence path.** `sheaf-db/tests/e2e/missing_features/<node>/`.

---

## 3. Phase 1: proof pipeline

### NF-01 Lean AOT rule table and drift gate

- **Goal:** compile the Lean rule proofs and emit a C header plus a manifest. Fail the build when the runtime table differs from the manifest.
- **New files:**
  - `proofs/RuleTable.lean`
  - `proofs/Emit.lean`
  - `crates/sheaf-proofs/build.rs`
  - `crates/sheaf-proofs/src/generated.rs`
  - `crates/sheaf-proofs/src/oracle.rs`
  - `native/proofs-generated/sheaf_verified_rules.generated.h`
  - `tools/check_lean_drift.ps1`
- **Port:** `RuleOracle { fn is_verified(&self, rule: RuleId) -> bool; fn digest(&self) -> [u8; 32]; }`.
- **Adapters:** `GeneratedRules` reads the emitted manifest. `HandwrittenRules` is the current table and reports `FallbackReason::LeanNotBuilt`.
- **Invariant:** the runtime digest equals the manifest digest. A rule with no proof is rejected.
- **Test:** `cargo test -p sheaf-proofs --test lean_drift`. Compile the emitted header with gcc and check one known rule id.
- **Gate:** `lake build` exits 0, the header compiles, the drift test passes, and the fallback reports its reason.
- **Host requirement:** Lean 4 v4.22.0 and Mathlib.

### NF-02 Lean spectral-radius convergence bounds

- **Goal:** prove the Chebyshev interval bounds and check the runtime estimate against the proven interval.
- **New files:**
  - `proofs/Chebyshev.lean`
  - `crates/sheaf-kernel/src/solvers/bounds.rs`
  - `crates/sheaf-kernel/tests/chebyshev_bounds.rs`
- **Port:** `SpectralBounds { fn interval(&self, n: usize, diagonal_sum: f64) -> Result<(f64, f64)> }`.
- **Invariant:** the computed interval sits inside the proven interval. Energy decreases monotonically.
- **Test:** 100 random symmetric positive-definite matrices. Assert the interval containment and the monotone energy.
- **Gate:** Lean proof compiles and the Rust test passes.

### NF-03 Production rewrite optimizer behind a rule oracle

- **Goal:** apply TIR rewrites only when the oracle marks the rule verified.
- **New files:**
  - `crates/sheaf-query/src/ir/rewrite.rs`
  - `crates/sheaf-query/src/ir/rule_oracle.rs`
  - `crates/sheaf-query/tests/rewrite_equivalence.rs`
- **Port:** `RuleOracle`. The adapter is `GeneratedRules` from NF-01.
- **Invariant:** a rewrite preserves the output row multiset. An unverified rule is rejected with `RewriteError::Unverified`.
- **Test:** 200 generated plans. Compare the row multiset before and after. Assert that one unverified rule is rejected.
- **Gate:** the equivalence test passes and the rejection test passes.

---

## 4. Phase 2: distributed topology

### NF-04 H1 quotient solver

- **Goal:** return a least-squares global representative plus the harmonic obstruction class.
- **New files:**
  - `crates/sheaf-vcs/src/h1.rs`
  - `crates/sheaf-vcs/src/least_squares.rs`
  - `crates/sheaf-vcs/tests/h1.rs`
- **Invariant:** the residual norm is below the stated tolerance. The harmonic class is orthogonal to the image of the coboundary.
- **Test:** four cases: one line, one triangle, two glued triangles, and one case with a nonzero obstruction.
- **Gate:** all four cases pass, and the obstruction case reports a nonzero class.

### NF-05 Replication transport with a real TCP adapter

- **Goal:** move sections between nodes over loopback TCP with order-tolerant XOR deltas.
- **New files:**
  - `crates/sheaf-runtime/src/replication/mod.rs`
  - `crates/sheaf-runtime/src/replication/port.rs`
  - `crates/sheaf-runtime/src/replication/tcp_adapter.rs`
  - `crates/sheaf-runtime/src/replication/frame.rs`
  - `crates/sheaf-runtime/tests/replication.rs`
- **Port:** `ReplicationTransport { fn send(&self, section: &Section) -> Result<Ack>; fn receive(&self) -> Result<Section>; }`.
- **Invariant:** 10,000 sections over loopback produce the same BLAKE3 hash on both ends. A reordered delivery produces the same final state.
- **Test:** send 10,000 sections, shuffle the receive order, compare the final hash. Add a truncated-frame error case.
- **Gate:** hash equality and the error case pass.

### NF-06 RDMA adapter boundary

- **Goal:** define the RDMA port and report a real capability result.
- **New files:**
  - `crates/sheaf-runtime/src/replication/rdma_port.rs`
  - `crates/sheaf-runtime/src/replication/rdma_unsupported.rs`
- **Port:** `RdmaTransport`.
- **Invariant:** on a host without RDMA verbs, the adapter returns `Capability::Unsupported { reason }`. It never returns a fake success.
- **Test:** assert the capability report on this host. Assert that the error names the missing device or library.
- **Gate:** the unsupported report is explicit and named.

### NF-07 Deterministic Raft consensus engine

- **Goal:** a deterministic Raft engine over the NF-05 transport.
- **New files:**
  - `crates/sheaf-runtime/src/consensus/mod.rs`
  - `crates/sheaf-runtime/src/consensus/log.rs`
  - `crates/sheaf-runtime/src/consensus/state.rs`
  - `crates/sheaf-runtime/src/consensus/node.rs`
  - `crates/sheaf-runtime/src/consensus/cluster.rs`
  - `crates/sheaf-runtime/tests/raft.rs`
- **Invariant:** one leader per term. A committed entry is present on a majority. A minority cannot commit.
- **Test:** five scenarios: election, replication, minority write, restart, and partition heal.
- **Gate:** all five scenarios pass with a fixed seed.

### NF-08 Open cover sharding with ghost cells

- **Goal:** assign cells to shards along the poset cover and replicate only the boundary.
- **New files:**
  - `crates/sheaf-vcs/src/sharding/mod.rs`
  - `crates/sheaf-vcs/src/sharding/cover.rs`
  - `crates/sheaf-vcs/src/sharding/ghost.rs`
  - `crates/sheaf-vcs/src/sharding/assign.rs`
  - `crates/sheaf-vcs/src/sharding/glue.rs`
  - `crates/sheaf-vcs/tests/sharding.rs`
- **Invariant:** every cell belongs to at least one shard. A ghost cell equals the boundary restriction. A glue conflict is reported, not hidden.
- **Test:** 1,000 cells over 4 shards. Check the cover, the ghost equality, and one forced glue conflict.
- **Gate:** the cover check, the ghost check, and the conflict report pass.

---

## 5. Phase 3: ecosystem

### NF-10 Tonic and Tokio gRPC ingress

- **Goal:** serve the same request model over gRPC.
- **New files:**
  - `crates/sheaf-runtime/src/grpc/mod.rs`
  - `crates/sheaf-runtime/src/grpc/service.rs`
  - `crates/sheaf-runtime/src/grpc/proto/sheaf.proto`
  - `crates/sheaf-runtime/tests/grpc_parity.rs`
- **Port:** `QueryService`.
- **Invariant:** one request produces the same response bytes on the framed TCP path and on the gRPC path.
- **Test:** send 100 requests on both paths and compare the response bodies.
- **Gate:** parity test passes. The dependency is opt-in behind a `grpc` feature.
- **Dependency:** `tonic`, `tokio`, `prost`.

### NF-09 Arrow Flight daemon

- **Goal:** serve columnar result sets with Arrow Flight.
- **New files:**
  - `crates/sheaf-app/src/flight/mod.rs`
  - `crates/sheaf-app/src/flight/server.rs`
  - `crates/sheaf-app/src/flight/producer.rs`
  - `crates/sheaf-app/tests/flight.rs`
- **Port:** `FlightService`.
- **Invariant:** a `do_get` returns the same rows as the SQL path. A `do_put` round trip returns the same rows.
- **Test:** a real Flight client calls `do_get` and `do_put` on localhost.
- **Gate:** both calls pass. The feature is opt-in behind a `flight` feature.
- **Dependency:** `arrow`, `arrow-flight`, `tonic`, `tokio`.

### NF-11 eBPF VFS interception

- **Goal:** a policy engine plus a Linux loader boundary.
- **New files:**
  - `native/ebpf/sheaf_vfs.bpf.c`
  - `crates/sheaf-native/src/ebpf/mod.rs`
  - `crates/sheaf-native/src/ebpf/policy.rs`
  - `crates/sheaf-native/src/ebpf/loader_unsupported.rs`
  - `crates/sheaf-native/tests/ebpf_policy.rs`
- **Port:** `VfsPolicy { fn decide(&self, event: &VfsEvent) -> Decision }`.
- **Invariant:** every recorded event gets an explicit allow or deny decision. On Windows the loader returns `Unsupported`.
- **Test:** 10,000 recorded events. Assert one stated decision for each event and the explicit unsupported report.
- **Gate:** the decision test and the capability test pass.

---

## 6. Phase 4: storage and query completeness

### NF-12 Live predicate pushdown wiring

- **Goal:** skip pages before decode when the predicate cannot match.
- **New files:**
  - `crates/sheaf-app/src/page_scan.rs`
  - `crates/sheaf-app/src/page_predicate.rs`
  - `crates/sheaf-app/tests/page_pushdown.rs`
- **Invariant:** the skipped page set equals the set that a full scan would reject. No skipped page holds a matching row.
- **Test:** 1,000 pages with a known value distribution. Compare the skip set with brute force.
- **Gate:** set equality and zero false skips.

### NF-13 Run-length codec

- **Goal:** add a run-length codec with a stable id.
- **New files:**
  - `crates/sheaf-codec/src/rle.rs`
  - `crates/sheaf-codec/tests/rle.rs`
- **Invariant:** bit-exact round trip. Codec id 5. The selector picks run-length only when it rebuilds the block bit for bit.
- **Test:** random and edge corpora, including one long run and one alternating block.
- **Gate:** round trip passes and the selector rule holds.

### NF-14 io_uring verification on Linux

- **Goal:** build and run the fixed-buffer write path on a Linux host.
- **New files:**
  - `crates/sheaf-native/tests/uring_linux.rs`
  - `scripts/verify_uring.ps1` (Windows guard) and `scripts/verify_uring.sh` (Linux run)
- **Invariant:** a fixed 4 KiB write returns the CRC32C of the written bytes. The ring uses `IORING_SETUP_SQPOLL`.
- **Test:** gated by `cfg(target_os = "linux")`. On Windows the test reports `Unsupported` with the named cause.
- **Gate:** the Linux run passes. The Windows report is explicit.

### NF-15 Morphism declaration DDL

- **Goal:** add a `DEFINE MORPHISM` statement.
- **New files:**
  - `crates/sheaf-app/src/ddl_morphism.rs`
  - `crates/sheaf-app/tests/morphism_ddl.rs`
- **Invariant:** a morphism references existing cells. A bad reference is an error.
- **Test:** create two cells, create one morphism, then reject one unknown cell.
- **Gate:** the accept case and the reject case pass.

### NF-16 Exact fingerprint cache key

- **Goal:** replace the 64-bit composite with the specified 128-bit key.
- **New files:**
  - `crates/sheaf-storage/src/lsp/cache_key.rs`
  - `crates/sheaf-storage/tests/cache_key.rs`
- **Invariant:** the key is `BLAKE3(epoch_id || row_count || schema_version || bundle_kind)` truncated to 128 bits. A change in one field changes the key.
- **Test:** four field changes, four different keys. One repeated input gives one key.
- **Gate:** the four-key test passes.

---

## 7. Toolchain prerequisites

Table 2 answers this question: which toolchain each node needs, and which toolchain is present now.

Rows are toolchains. Columns are the use, the nodes, and the current state.

| Toolchain | Used by | Nodes | State now |
|---|---|---|---|
| Rust 1.98.0 | All Rust nodes | NF-01 to NF-16 | installed |
| Zig 0.17.0 | Storage tiers | NF-12, NF-16 | installed |
| GCC 16.2.0 | C tiers | NF-01, NF-14 | installed |
| Lean 4 v4.22.0 + Mathlib | Proof pipeline | NF-01, NF-02, NF-03 | not installed |
| Linux with io_uring | Fixed-buffer WAL | NF-14 | not available |
| Two hosts or one loopback pair | Cluster tests | NF-05, NF-06, NF-07 | loopback only |
| `tonic`, `tokio`, `arrow`, `arrow-flight` | Ecosystem | NF-09, NF-10 | not added |

**Install order.** Install Lean first, because phase 1 blocks phase 1 only. Install the ecosystem crates when phase 3 starts. Use a Linux virtual machine or a second laptop for NF-14.

---

## 8. Completion definition

The plan is complete when all 16 nodes are `e2e_passed`. A node is `e2e_passed` only when all 12 gate rules pass and the evidence directory exists. Then `SKELETON.md` moves each node from `planned` to `e2e_passed`, and the inventory count becomes 46 implemented and 2 excluded or host-gated.

**Expected final count.**

| State | Count |
|---|---|
| Implemented | 42 |
| Host-gated, reported as unsupported | 3 |
| Excluded | 1 |
| Absent | 3 |

The host-gated features are F-12, F-13, and F-31. The remaining absent features are F-26 and F-28 if Lean is not installed, and F-30 if a second host is not available. Each one must report its reason.

---

## 9. Warnings and notes

- **CAUTION:** do not start a node before its dependency is `e2e_passed`. A partial cluster test hides real failures.
- The Lean pipeline needs Mathlib. A failed Mathlib download stops NF-01, NF-02, and NF-03.
- NF-14 needs a Linux host. A Windows run must return `Unsupported`, not a pass.
- NF-09 and NF-10 add external crates. Run `cargo deny` or `cargo audit` after the add.
- A TQL optimizer cost is an estimate. The optimizer-to-fiber execution bridge (OPT-004) is not built.
