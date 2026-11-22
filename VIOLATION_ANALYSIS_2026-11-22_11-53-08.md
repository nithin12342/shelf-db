# Violation analysis

**Timestamp:** 2026-11-22_11-53-08, local time 11:53:08 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `7bdaf1c22246c43f69d05ab70b1a1d572ea14b1f`.
**Scope:** Database functionality only. Neural-network work stays excluded.

---

## 1. Result

I found 12 deviations from ground truth. Ground truth means the approved document that states the requirement. Five deviations need a language or toolchain change. Five deviations need a design change. Two deviations need a new rule from you.

The proof tier, the gRPC tier, the Flight tier, and the journaled DDL follow ground truth. The math tier, the physical page tier, the consensus design, and the metaprogramming policy do not follow ground truth.

---

## 2. Method

1. I read the approved documents. I did not read the protected archive.
2. I compared each requirement with the code from the 16 implementation nodes.
3. I named a deviation only when a document line or a code line supports it.
4. I did not count excluded neural-network work as a database deviation.
5. I recorded the remote, the branch, and the commit hash before I wrote this report.

Table 1 answers this question: which documents give the rules for this analysis.
Rows are documents. Columns are the role and the citation.

| Document | Role | Citation |
|---|---|---|
| `SheafDB_Next_Gen_Technical_Specification.md` | Tier to language map | Lines 63 to 89 |
| Same file | Pillar requirements | Lines 182, 206, 226, 260, 283, 318, 371, 420 |
| `PLAN_2026-11-15_12-09-48.md` | Phase plan with language and file | Lines 125 to 222 |
| `AGENTS.md` | Active project rule | Section 3b |
| `SKELETON.md` | Source of truth for code state | Blocks `missing_feature_plan` and `feature_inventory` |
| `chronology.json` | Document index | Field `reading_scope` |

The instruction says to start at `FEATURE_DOCUMENT_CHRONOLOGY/README.md`. That file is absent. I used `chronology.json` and the root documents instead. This gap is deviation D-12.

---

## 3. Deviation table

Table 2 answers this question: which deviations exist and what each one needs.
Rows are deviations. Columns are the identifier, the area, the ground truth, the code, the gate result, and the fix.

| ID | Area | Ground truth | Code | Gate | Fix |
|---|---|---|---|---|---|
| D-01 | Consensus design | Spec line 318 says the network agreement does not use Raft voting loops | `crates/sheaf-runtime/src/consensus/` and `cluster.rs` line 39 | Fail | Replace Raft with gluing consensus and C++23 Laplacian minimization |
| D-02 | Math kernel | Spec lines 86 and 184 assign inner loops to C++23 | `bounds.rs` line 37, `least_squares.rs` line 104, `h1.rs` line 30 | Fail | Move the computations to C++23 and keep Rust FFI wrappers |
| D-03 | Compression | Spec line 371 requires vector masks, plan lines 146 to 151 assign FSST to C++23 | `crates/sheaf-codec/src/rle.rs` line 29 | Fail | Add `native/kernel-cpp/src/rle.cpp` with a Rust fallback |
| D-04 | Physical page scan | Spec lines 80 and 206 assign physical storage to Zig | `page_scan.rs` lines 17 and 61, `page_stats.rs` | Fail | Move the page decode loop to `native/storage-zig/src/page_scan.zig` |
| D-05 | WAL integrity | Spec line 241 requires BLAKE3 in each framed record, plan lines 157 to 164 assign C11 | `native/wal-c/src/frame.c` lines 18 to 52 uses CRC32C only | Fail | Add `frame_blake3.c` in C11 and keep CRC32C as the fast sector check |
| D-06 | RDMA replication | Spec line 316 requires zero-copy RoCEv2 into registered buffers | `rdma_unsupported.rs` reports only `Unsupported` | Fail | Add a C11 libibverbs transport behind a capability gate |
| D-07 | eBPF mount | Spec line 420 and plan line 220 require an eBPF VFS mount | `native/ebpf/sheaf_vfs.bpf.c` has 52 lines and no build rule | Fail | Compile with clang BPF and libbpf in the Linux container |
| D-08 | Sharding | Spec line 314 requires cut-weight minimization | `sharding/assign.rs` uses modulo placement | Fail | Implement the cut-weight objective in the Rust cover module |
| D-09 | Ingress layout | Plan line 168 requires a new `crates/sheaf-ingress` crate | gRPC is in `sheaf-runtime`, Flight is in `sheaf-app` | Fail | Move both adapters into `crates/sheaf-ingress` |
| D-10 | Procedural macros | Your new instruction says that Rust metaprogramming must be used | No `#[proc_macro]` exists in the source, only three `build.rs` files | Fail | Add a proc-macro crate for unrolled FFI dispatch and rule-table generation |
| D-11 | Mojo | Plan line 78 scores Mojo at zero and line 210 places it in the neural phase | No `.mojo` file exists | Not applicable | Do not add Mojo before you reopen the neural scope and update section 3b |
| D-12 | Document index | `AGENTS.md` says to start at `FEATURE_DOCUMENT_CHRONOLOGY/README.md` | The path does not exist | Fail | Restore the README or update `AGENTS.md` to name `chronology.json` |

---

## 4. Language stack findings

### 4.1 C++23 math kernels

Spec line 86 assigns high-dimensional math to C++23. Spec line 184 says that inner loops in `sheaf-kernel/src/simd/` use C++23. The repository already has eight C++ files. The files cover Chebyshev, Laplacian, Morton tiles, XOR deltas, validity masks, and the hypercell solve.

I wrote three math modules in Rust instead. The first is `crates/sheaf-kernel/src/solvers/bounds.rs`. It computes the spectral interval that Lean proves. The second is `crates/sheaf-vcs/src/least_squares.rs`. It solves the normal equations with conjugate gradient. The third is `crates/sheaf-vcs/src/h1.rs`. It splits a cochain into an exact part and a harmonic class.

These modules work. They still violate the tier rule. I must move the arithmetic into C++23 files under `native/kernel-cpp/src/`. I must keep thin Rust wrappers so that the public API does not change. The gate is bit-level parity with the current Rust result on the same fixtures.

### 4.2 Zig physical storage

Spec line 80 assigns physical storage and BCSF packing to Zig. Spec line 206 names the `sheaf-storage` and BCSF tier. The repository has five Zig source files. They cover the spine, the bump arena, and the BCSF block.

I wrote the new page decode loop in `crates/sheaf-app/src/page_scan.rs`. Line 17 holds the page container. Line 61 runs the scan. This code reads page payloads and checks row hashes. Those actions belong to the physical storage tier. I must move the decode loop into Zig. I can keep the Rust predicate planning code in the app tier. The gate is equality of the skipped page set and the decoded rows.

### 4.3 C11 WAL and hardware paths

Spec line 83 assigns the async WAL and hardware crash consistency to C11. Spec line 241 requires that each framed record carries a BLAKE3 checksum. The plan repeats this in lines 157 to 164.

`native/wal-c/src/frame.c` still uses CRC32C at lines 18 to 52. CRC32C is a fast sector check. It is not the required record checksum. I must add a C11 file `frame_blake3.c` that writes a BLAKE3 digest into the frame header. I must keep CRC32C for the fast path. The gate is that one changed payload byte fails the BLAKE3 check.

RDMA is the second hardware gap. Spec line 316 requires zero-copy replication into registered buffers. My adapter in `rdma_unsupported.rs` reports the missing device. That report is honest. It is not the required transport. I must add a C11 libibverbs adapter. The adapter must report `Unsupported` when the device is absent. The gate is byte parity over the NIC or an explicit unsupported result.

### 4.4 Rust orchestration and ingress

Spec lines 74 and 77 assign ingress and orchestration to Rust. The gRPC adapter and the Flight adapter use Rust. That part follows the stack. The crate layout does not follow plan line 168, which names a new `crates/sheaf-ingress` crate. I placed gRPC in `sheaf-runtime` and Flight in `sheaf-app`. I must move both into `crates/sheaf-ingress`. The gate is that the parity tests keep their names and their results.

### 4.5 Lean 4

Spec line 71 assigns proofs to Lean 4. The five original proof files compile. I added `RuleTable.lean`, `EmitRules.lean`, and `Chebyshev.lean`. The emitter writes a C header and a text manifest. The Rust drift test compares them. This tier follows ground truth. The only gap is D-02, because the runtime mirror of the spectral bound stays in Rust.

### 4.6 Mojo

The approved plan places Mojo in phase H at line 210. Phase H covers neural-network parts. The global instruction excludes neural-network creation and training from this database project. The approved tier table at spec lines 63 to 89 does not name Mojo. The repository has zero `.mojo` files.

Mojo is not suitable for the current database scope. I must not add it before you issue a new explicit instruction. That instruction must reopen the neural scope and update `AGENTS.md` section 3b. Without that instruction, a Mojo tier is an excluded feature and a stack violation.

---

## 5. Procedural macros and Rust metaprogramming

I did not author a procedural macro. I found no `#[proc_macro]` item in the source. I found three build scripts. They are listed in Table 3.

Table 3 answers this question: which build-time generators exist now.
Rows are build scripts. Columns are the crate, the origin, and the output.

| Crate | Origin | Output |
|---|---|---|
| `sheaf-native` | Pre-existing | Probes Zig, clang, and gcc and sets three custom cfgs |
| `sheaf-proofs` | Written by me | Declares the emitted header and manifest as rebuild inputs |
| `sheaf-runtime` | Written by me | Runs `tonic-build` and generates the gRPC client and server |

I used derive macros from serde and prost. External crates supply those macros. I did not write them. Build scripts are Rust metaprogramming at the build level. They do not give compile-time type specialization inside a crate.

Your new instruction says that Rust metaprogramming must be used. I will comply. I propose three database-scoped procedural macros. The first is `#[sheaf_unroll(lanes = N)]`. It generates unrolled calls into the C++ kernels for fixed stalk widths. The second is `#[rule_table]`. It reads the emitted Lean header at compile time and generates a static dispatch table, so the runtime parser in `generated.rs` can go away. The third is `#[ffi_contract]`. It generates Rust wrappers from the headers in `native/include/` and checks arity at compile time.

These macros give compile-time specialization and safer hardware calls. The raw hardware access stays in C++, Zig, and C. That split follows the tier rule. I must update `AGENTS.md` section 3b before I add the macro crate, because the section does not name procedural macros today.

---

## 6. Fix order

1. Update `AGENTS.md` section 3b to name procedural macros and the approved database use. Update the document index so that `chronology.json` is the named start. Gate: the instruction file names the real path and one commit records the edit.
2. Add C++23 files for the spectral interval and the H1 solve. Keep Rust FFI wrappers. Gate: parity with the current Rust output on 100 fixtures.
3. Add a C++23 run-length codec with a Rust fallback. Gate: bit-exact round trip on 1,000 fixture rows.
4. Move the page decode loop into Zig. Gate: equal skipped page sets and equal rows on the 1,600 row fixture.
5. Add the C11 BLAKE3 WAL frame mode. Gate: one changed payload byte fails the record check.
6. Replace Raft with gluing consensus. Gate: two processes reach one global section with zero coboundary.
7. Add the cut-weight objective to the open cover. Gate: lower cut weight than the modulo baseline on the fixture.
8. Add the C11 RDMA transport behind a capability gate. Gate: byte parity or an explicit unsupported result.
9. Compile the BPF program in the Linux container. Gate: clang with the BPF target exits zero and libbpf accepts the object.
10. Move the gRPC and Flight adapters into `crates/sheaf-ingress`. Gate: the parity tests keep their results.
11. Add the proc-macro crate and remove the runtime rule-table parser. Gate: generated code compiles and all parity tests pass.
12. Run the full workspace, Lean, Zig, and container gates again. Gate: no failure and no file over 500 lines.

I must make one commit for each file change. I must push one time after all commits are complete.

---

## 7. Evidence

The ground truth lines and the code paths are named in Table 2 and in Section 4. The node evidence lives in `sheaf-db/tests/e2e/missing_features/nf01/` through `nf16/`. Each directory holds `command.txt`, `output.txt`, and `result.json`. The commit messages repeat the commands and the result lines.

The current top commit is `7bdaf1c22246c43f69d05ab70b1a1d572ea14b1f`. The working tree has one modified file, `SKELETON.md`. No push has happened.

---

## Warnings

- **CAUTION:** Docker Desktop now answers the Docker API. The Linux gates can run. A container can block `io_uring` with a seccomp profile. Use an unconfined profile or privileged mode for that gate.
- **CAUTION:** Mojo needs a new rule and a GPU host. Do not add it to the database scope without your explicit instruction.
- **CAUTION:** The proc-macro crate needs an update to `AGENTS.md` section 3b before the first commit.
- **NOTE:** This analysis does not use the protected archive. The exclusion stays in force.
