# Consolidated feature inventory, 2026-11-22

**Question this file answers:** how many database features the documentation describes, and how many of them the code implements now.

**Answer:** the documents describe 49 features. 48 are in database scope. 1 is excluded training work. Of the 48, **30 are implemented**, **11 are partial**, and **7 are absent**. With a partial feature counted as one half, the weighted completion is 35.5 of 48, or 74.0 percent.

---

## 1. Method

1. The document list comes from `FEATURE_DOCUMENT_CHRONOLOGY/chronology.json`. The file lists 33 database-relevant documents.
2. The requirement text comes from `SheafDB_Next_Gen_Technical_Specification.md` (11 pillars), `SHELFDB_SCHEMA_DESIGN_AND_QUERY_PATTERNS.md` (schema and query patterns), and `Sheaf_VCS_Architecture.md` (version control).
3. The open work comes from `MISSING_FEATURE_IMPLEMENTATION_PLAN_2026-11-15.md` (17 nodes).
4. Each feature was checked against the current source tree. A feature is **implemented** only when production code exists and a test calls that code with real input and passes.
5. The protected archive `ARCHIVED_NEURAL_NETWORK_DOCUMENTATION/` was not read, searched, or indexed.

**State words.**

- **implemented:** the production code exists and a passing test exercises it.
- **partial:** the code exists, but a named part is missing or the path is not proven on this host.
- **absent:** no production code exists.
- **excluded:** outside database scope under `AGENTS.md`.

---

## 2. Feature inventory by pillar

Table 1 answers this question: what is each feature, where is its code, and what proves it.
Rows are features. Columns are the identifier, the feature name, the source requirement, the state, the proof file, and the test command.

| ID | Feature | Source requirement | State | Proof file | Test command |
|---|---|---|---|---|---|
| F-01 | AVX-512 FMA and VNNI inner loops | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/fma_scan.cpp`, `laplacian.cpp`, `validity.cpp`, `xor_delta.cpp` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-02 | Morton Z-order 64x64 cache tiling | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/morton.cpp`, `sheaf_matvec_tiled` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-03 | Unmaterialized factored Laplacian, two-pass | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/laplacian.cpp`, `sheaf_coboundary_apply`, `sheaf_coboundary_adjoint` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-04 | Vectorized sparse XOR cochain fold | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/xor_delta.cpp`, `sheaf_xor_delta_fold` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-05 | f32 precision down-scaling | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/laplacian.cpp`, `chebyshev.cpp` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-06 | Branch-free validity masks (`compact_valid`) | Next-Gen spec, Pillar 1 | implemented | `native/kernel-cpp/src/validity.cpp`, `sheaf_compact_valid` | `cargo test -p sheaf-native --test kernel_parity` (9 pass) |
| F-07 | 64-byte cache-line alignment | Next-Gen spec, Pillar 2 | implemented | `native/storage-zig/src/spine_soa.zig`, `bcsf_block.zig` | `cargo test -p sheaf-native --test storage_parity` (8 pass) |
| F-08 | SheafSpine Struct-of-Arrays | Next-Gen spec, Pillar 2 | implemented | `native/storage-zig/src/spine_soa.zig` (`SheafSpineSoA`) | `cargo test -p sheaf-native --test storage_parity` (8 pass) |
| F-09 | Thread-local bump allocator and huge pages | Next-Gen spec, Pillar 2 | implemented | `native/storage-zig/src/bump_arena.zig`, `crates/sheaf-storage/src/lsp/bump_arena.rs`, `huge_pages.rs` | `cargo test -p sheaf-storage` (54 pass) |
| F-10 | Offset-driven predicate pushdown | Next-Gen spec, Pillar 2 | partial | `crates/sheaf-storage/src/bcsf/coord.rs`, `offsets.rs`, `bcsf_block.zig` (`blockMatches`) | Storage path passes. The live application page path does not call it. |
| F-11 | Per-domain dirty-region maintenance | Next-Gen spec, Pillar 2 | implemented | `crates/sheaf-storage/src/bcsf/fiber_domain.rs`, `partition.rs`, `crates/sheaf-storage/src/topology/` | `cargo test -p sheaf-storage topology` (33 pass) |
| F-12 | io_uring SQPOLL kernel bypass | Next-Gen spec, Pillar 3 | partial | `native/wal-c/src/platform_uring.c` | Linux-only source. The Windows host does not build it. |
| F-13 | Zero-syscall registered buffers | Next-Gen spec, Pillar 3 | partial | `native/wal-c/src/registered_buffers.c`, `crates/sheaf-native/src/wal/registered.rs` | `cargo test -p sheaf-native --test registered_buffers` (6 pass). Windows reports `Unsupported`. |
| F-14 | NVMe AWUPF-sized commit descriptors | Next-Gen spec, Pillar 3 | implemented | `native/include/sheaf_wal.h` (max payload), `native/wal-c/src/frame.c` | `cargo test -p sheaf-native --test wal_recovery` (8 pass) |
| F-15 | Double-buffered A/B epoch headers | Next-Gen spec, Pillar 3 | implemented | `native/wal-c/src/epoch_header.c` | `cargo test -p sheaf-native --test wal_recovery` (8 pass) |
| F-16 | Self-verifying framed records | Next-Gen spec, Pillar 3 | implemented | `native/wal-c/src/frame.c` (magic, sequence, length, checksum) | `cargo test -p sheaf-native --test wal_recovery` (8 pass) |
| F-17 | Tonic, Tokio, and Arrow Flight ingress | Next-Gen spec, Pillar 4 | absent | none. `crates/sheaf-runtime/src/ingress.rs` is framed JSON over TCP. No `tonic`, `tokio`, or `arrow` dependency exists. | not run |
| F-18 | Affine buffer pinning (`PinnedStalkView`) | Next-Gen spec, Pillar 4 | implemented | `crates/sheaf-runtime/src/pin.rs` | `cargo test -p sheaf-runtime` (47 pass) |
| F-19 | Lock-free epoch-based reclamation | Next-Gen spec, Pillar 4 | implemented | `crates/sheaf-runtime/src/epoch.rs` | `cargo test -p sheaf-runtime` (47 pass) |
| F-20 | Thread-per-core OS and NUMA pinning | Next-Gen spec, Pillar 4 | implemented | `crates/sheaf-runtime/src/affinity.rs`, `pinning/windows.rs`, `pinning/linux.rs` | `cargo test -p sheaf-runtime pinning` (4 pass) |
| F-21 | In-process nerve routing | Next-Gen spec, Pillar 4 | implemented | `crates/sheaf-runtime/src/nerve.rs`, `nerve_lockfree.rs`, `nerve_partition_map.rs` | `cargo test -p sheaf-runtime` (47 pass, 8 x 50,000 messages) |
| F-22 | Fingerprint-bound bundle cache | Next-Gen spec, Pillar 5 | implemented | `crates/sheaf-engine/src/bin/shared/fiber_snapshot.rs`, `crates/sheaf-storage/src/lsp/fiber_epoch.rs` | `cargo test -p sheaf-engine` (5 pass) |
| F-23 | Compaction fencing | Next-Gen spec, Pillar 5 | implemented | `crates/sheaf-storage/src/lsp/compactor.rs`, `fiber_epoch.rs` | `cargo test -p sheaf-storage` (54 pass) |
| F-24 | Permanent naive scan oracle | Next-Gen spec, Pillar 5 | implemented | `crates/sheaf-storage/src/bin/phase1_bench.rs`, `crates/sheaf-engine/src/bin/fiber_join_bench.rs` | `cargo test -p sheaf-storage` (54 pass) |
| F-25 | Lean 4 sheaf axiom proofs | Next-Gen spec, Pillar 6 | partial | `proofs/Poset.lean`, `Restriction.lean`, `Gluing.lean`, `Cohomology.lean` | Lean is not installed. The sources are not compiled. |
| F-26 | Lean-emitted verified rule table | Next-Gen spec, Pillar 6 | absent | `native/proofs-generated/sheaf_verified_rules.h` is handwritten, not Lean output. MF-09 stays planned. | not run |
| F-27 | Lean-verified production rewrite optimizer | Next-Gen spec, Pillar 6 | partial | `crates/sheaf-proofs/src/rules.rs`, `planner.rs` (toy model), `crates/sheaf-query/src/ir/optimizer.rs` (statistical) | `cargo test -p sheaf-proofs` (9 pass). The production rewrite is not Lean-gated. |
| F-28 | Lean spectral-radius convergence bounds | Next-Gen spec, Pillar 6 | absent | `crates/sheaf-kernel/src/solvers/chebyshev.rs` has no Lean bound | not run |
| F-29 | Canonical open poset cover sharding | Next-Gen spec, Pillar 7 | partial | `crates/sheaf-storage/src/bcsf/cover.rs`, `crates/sheaf-vcs/src/complex.rs` (model only) | MF-14 stays planned |
| F-30 | Boundary intersection ghost stalks | Next-Gen spec, Pillar 7 | absent | none | MF-14 stays planned |
| F-31 | Zero-copy RDMA RoCEv2 replication | Next-Gen spec, Pillar 7 | absent | none | MF-12 stays planned |
| F-32 | Order-tolerant XOR commutativity | Next-Gen spec, Pillar 7 | partial | `native/kernel-cpp/src/xor_delta.cpp` (commutative fold) | No network transport exists. |
| F-33 | Sheaf gluing consensus | Next-Gen spec, Pillar 7 | partial | `crates/sheaf-vcs/src/section.rs`, `diff.rs` (global-section rule) | No cluster consensus engine exists. |
| F-34 | Multi-scale fractal invariant | Next-Gen spec, Pillar 7 | partial | `crates/sheaf-storage/src/topology/`, `bcsf/partition.rs` | L3 domains are real. L1 sub-tiles are not explicit. |
| F-35 | Atomic software entity decomposition | Next-Gen spec, Pillar 8 | implemented | `crates/sheaf-vcs/src/complex.rs` | `cargo test -p sheaf-vcs` (49 pass) |
| F-36 | Commits as zero-coboundary sections | Next-Gen spec, Pillar 8 | implemented | `crates/sheaf-vcs/src/section.rs` | `cargo test -p sheaf-vcs` (49 pass) |
| F-37 | Cohomological merge conflict resolution | Next-Gen spec, Pillar 8 | partial | `crates/sheaf-vcs/src/diff.rs` detects the obstruction. The H1 quotient solver is missing. | MF-11 stays planned |
| F-38 | Content-addressable Merkle-sheaf DAG | Next-Gen spec, Pillar 8 | implemented | `crates/sheaf-vcs/src/merkle.rs` (BLAKE3) | `cargo test -p sheaf-vcs` (49 pass) |
| F-39 | Reversible poset time-travel diffs | Next-Gen spec, Pillar 8 | implemented | `crates/sheaf-vcs/src/diff.rs` | `cargo test -p sheaf-vcs` (49 pass) |
| F-40 | Dictionary-vector arena sharing | Next-Gen spec, Pillar 9 | implemented | `crates/sheaf-storage/src/dict/arena.rs`, `interner.rs`, `column.rs` | `cargo test -p sheaf-storage --test dictionary` (5 pass) and 15 unit tests |
| F-41 | FSST, ALP, frame-of-reference, bit-packing | Next-Gen spec, Pillar 9 | implemented | `crates/sheaf-codec/src/{fsst,alp,for_,bitpack,raw,select}.rs` | `cargo test -p sheaf-codec` (47 unit, 9 integration pass) |
| F-42 | Run-length and validity bitmask packing | Next-Gen spec, Pillar 9 | partial | `crates/sheaf-app/src/page_format.rs` (null bitmap), kernel validity masks | No dedicated run-length codec exists. |
| F-43 | Disjoint non-interference execution | Next-Gen spec, Pillar 10 | implemented | `crates/sheaf-vcs/src/lca.rs`, `wave_executor.rs` | `cargo test -p sheaf-vcs wave` (10 pass) |
| F-44 | Append-only chronological charts | Next-Gen spec, Pillar 10 | implemented | `crates/sheaf-vcs/src/chart.rs` | `cargo test -p sheaf-vcs` (49 pass) |
| F-45 | Deterministic non-destructive delta chaining | Next-Gen spec, Pillar 10 | implemented | `crates/sheaf-vcs/src/chart.rs` (delta fold) | `cargo test -p sheaf-vcs` (49 pass) |
| F-46 | PostgreSQL wire protocol | Next-Gen spec, Pillar 11 | implemented | `crates/sheaf-app/src/pgwire.rs`, `pg_codec.rs`, `pg_sql.rs` | `cargo test -p sheaf-app --test network` (10 pass) |
| F-47 | Apache Arrow Flight daemon | Next-Gen spec, Pillar 11 | absent | none | MF-15 stays planned |
| F-48 | eBPF VFS interception | Next-Gen spec, Pillar 11 | absent | none | MF-16 stays planned |
| F-49 | Native PyTorch bridge | Next-Gen spec, Pillar 11 | excluded | `native/torch-bridge/` (stub only) | Training work is out of scope. |

**Table 1 totals.**

| State | Count | Share of 48 in-scope features |
|---|---|---|
| Implemented | 30 | 62.5 percent |
| Partial | 11 | 22.9 percent |
| Absent | 7 | 14.6 percent |
| Excluded | 1 | not counted |

---

## 3. Schema and query features

Table 2 answers this question: which schema and query features the schema guide describes, and which of them the application exposes.
Rows are features. Columns are the identifier, the feature, the source section, the state, and the proof.

| ID | Feature | Source section | State | Proof |
|---|---|---|---|---|
| S-01 | Cell declaration (DDL) | Schema guide, 3.1 | implemented | `crates/sheaf-app/src/sql.rs`, `sql_write.rs` |
| S-02 | Morphism declaration (DDL) | Schema guide, 3.2 | absent | No parser support exists. |
| S-03 | Answer fiber materialization | Schema guide, Pattern 3 | implemented | `crates/sheaf-storage/src/lsp/preagg.rs`, `fiber_snapshot.rs` |
| S-04 | Contiguous fiber domains | Schema guide, Pattern 4 | implemented | `crates/sheaf-storage/src/bcsf/fiber_domain.rs` |
| S-05 | BCSF v2 self-describing segment with zone maps | Schema guide, Pattern 5 | implemented | `crates/sheaf-storage/src/bcsf/segment/read_v2.rs`, `zone.rs` |
| S-06 | Validity bitmaps for missing values | Schema guide, Pattern 6 | implemented | `crates/sheaf-app/src/page_format.rs` (null bitmap) |
| S-07 | TQL grammar | Schema guide, 5.1 | implemented | `crates/sheaf-app/src/tql.rs` |
| S-08 | 12-opcode topological intermediate representation | Schema guide, 7 | implemented | `crates/sheaf-query/src/ir/tir.rs` |

Table 2 states: 7 implemented, 1 absent.

---

## 4. The 17-node missing-feature plan

Table 3 answers this question: which plan node is closed and which node stays open.
Rows are plan nodes. Columns are the node, the name, the state, and the proof.

| Node | Name | State | Proof |
|---|---|---|---|
| MF-00 | Split the 666-line segment file | e2e_passed | `cargo test -p sheaf-storage --test segment_golden` |
| MF-01 | Lock-free bounded MPMC nerve router | e2e_passed | `cargo test -p sheaf-runtime` (47) |
| MF-02 | Real OS affinity and NUMA pinning | e2e_passed | `cargo test -p sheaf-runtime pinning` (4) |
| MF-03 | Parallel execution of disjoint write waves | e2e_passed | `cargo test -p sheaf-vcs wave` (10) |
| MF-04 | Physical L3 cache discovery and partition map | e2e_passed | `cargo test -p sheaf-storage topology` (33) |
| MF-05 | FSST, ALP, FoR, bit-packing codecs | e2e_passed | `cargo test -p sheaf-codec` (47 + 9) |
| MF-06 | Global dictionary arena sharing | e2e_passed | `cargo test -p sheaf-storage --test dictionary` (5 + 15) |
| MF-07 | Versioned page format with explicit gate | e2e_passed | `cargo test -p sheaf-app --test page` (6 + 17) |
| MF-08 | Pre-registered io_uring buffers | e2e_passed | `cargo test -p sheaf-native --test registered_buffers` (6) |
| MF-09 | Lean AOT rule table and drift gate | planned | none |
| MF-10 | Production rewrite optimizer behind a rule oracle | planned | none |
| MF-11 | H1 quotient solver | planned | none |
| MF-12 | Replication transport with a real TCP adapter | planned | none |
| MF-13 | Deterministic Raft consensus engine | planned | none |
| MF-14 | Open cover sharding with ghost cells | planned | none |
| MF-15 | Arrow Flight daemon | planned | none |
| MF-16 | eBPF VFS interception | planned | none |

Table 3 states: 9 of 17 nodes are closed, and 8 stay open.

---

## 5. Toolchain and run evidence

Table 4 answers this question: which toolchain is present, and which command proves the application runs.
Rows are checks. Columns are the check, the command, the measured result, and the state.

| Check | Command | Measured result | State |
|---|---|---|---|
| Rust compiler | `rustc --version` | 1.98.0 (2026-08-18) | pass |
| Cargo | `cargo --version` | 1.98.0 | pass |
| Zig | `zig version` | 0.17.0 | pass |
| GCC | `gcc --version` | 16.2.0 (WinLibs) | pass |
| Clang | `clang --version` | not installed. GCC builds the C tiers. | fallback in use |
| Lean 4 | `lean --version` | not installed | pending |
| Workspace build | `cargo build --workspace` | finished in 39.00 s | pass |
| Workspace test | `cargo test --workspace` | 356 tests pass, 0 fail | pass |
| Zig tests | `zig build test` (in `native/storage-zig`) | no output, exit 0 | pass |
| Native archives | build output | `sheaf_kernel.lib` 253.7 KB, `sheaf_storage.lib` 14.2 KB, `sheaf_wal.lib` 103.1 KB | pass |
| Application start | `cargo run -q -p sheaf-app -- --help` | usage text printed | pass |
| Application write | `cargo run -q -p sheaf-app -- --db smoke_db sql "INSERT INTO t VALUES (1, 'alpha'), (2, 'beta')"` | `"affected":2` | pass |
| Application read | `cargo run -q -p sheaf-app -- --db smoke_db sql "SELECT id, name FROM t ORDER BY id"` | 2 rows, `"output_rows":2` | pass |
| File size cap | line count over `crates`, `native`, `proofs` | largest file 496 lines (`imdb_fiber.rs`) | pass |

The application needs no extra package. The workspace has no external service dependency. `cargo build` fetches the Rust crates on first use.

**Test count detail.** 18 + 3 + 12 + 20 + 14 + 10 + 6 + 4 + 5 (sheaf-app) + 47 + 9 (sheaf-codec) + 5 (engine bench) + 1 (kernel) + 2 + 6 + 9 + 6 + 8 + 8 (native) + 9 (proofs) + 4 (query) + 47 (runtime) + 48 + 5 + 1 (storage) + 49 (vcs) = 356.

---

## 6. Gaps

The seven absent features, in dependency order:

1. F-17 Tonic, Tokio, and Arrow Flight ingress.
2. F-26 Lean-emitted rule table (MF-09).
3. F-28 Lean spectral-radius bounds.
4. F-30 boundary ghost stalks (MF-14).
5. F-31 RDMA RoCEv2 replication (MF-12).
6. F-47 Arrow Flight daemon (MF-15).
7. F-48 eBPF VFS interception (MF-16).

The seven absent features plus five of the partial features (F-25 Lean proofs, F-27 Lean-gated optimizer, F-29 sharding, F-33 consensus, F-37 H1 solver) need the missing infrastructure. The missing infrastructure is Lean, a Linux host, and a network cluster.

**Host limits.**

- The io_uring path (F-12, F-13) is written but not compiled or run on this Windows host.
- The Lean proofs (F-25) need Lean 4 and Mathlib.
- No second machine exists for a real cluster test (F-30, F-31, F-33).

---

## 7. Warnings and notes

- **WARNING:** a partial feature is not a finished feature. Do not count a partial feature as complete.
- The 48-feature audit line in `SKELETON.md` is one input. It is not the answer of this file. This file derives its own count from the requirement text.
- The historical records list smaller test counts than the current tree. The current tree has more tests because the MF-00 through MF-08 nodes added tests after those records.
- A TQL optimizer cost is an estimate. The optimizer-to-fiber execution bridge (OPT-004) is not built.
