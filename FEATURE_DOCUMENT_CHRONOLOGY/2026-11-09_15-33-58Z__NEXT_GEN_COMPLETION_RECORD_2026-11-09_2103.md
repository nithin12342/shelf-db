# Next-Generation Stack Completion Record — 2026-11-09 21:03:58 +0530

**Branch:** `combined-stack-feature-improvement`
**Remote:** `origin` (`https://github.com/nithin12342/shelf-db.git`)
**Plan of record:** `SheafDB_Next_Gen_Technical_Specification.md` (606 lines)
**Status at last push:** `5765339` Add C11 WAL tier build module with io_uring selection
**Author of this record:** Crouton Alpha session (continuation of a prior agent session)

---

## 1. Purpose

This document records the state of the work described in the plan at the
moment the operator asked for a pause. It lists what is done, what is
partially done, and what remains. It is the hand-over note for the next
session.

---

## 2. Prior agent work found on this branch

The prior agent left 55 commits ahead of `origin/main`, 40 files changed,
4883 insertions, 4 deletions at the last push. The branch was present
locally and pushed to `origin/combined-stack-feature-improvement`.

| Tier | Content | State |
|---|---|---|
| Tier 1 kernel (C++23) | `sheaf-db/native/kernel-cpp/src/` — 8 files: `laplacian.cpp`, `morton.cpp`, `chebyshev.cpp`, `xor_delta.cpp`, `validity.cpp`, `cpu_features.cpp`, `internal.hpp`, plus `Makefile` | Written. Compile faults found and fixed in this session |
| Tier 2 storage (Zig) | `sheaf-db/native/storage-zig/` — `src/spine_soa.zig`, `src/bump_arena.zig`, `src/bcsf_block.zig`, `src/exports.zig`, `src/tests.zig`, `build.zig` | Written. Compile faults found and fixed in this session |
| Tier 2b WAL (C11) | `sheaf-db/native/wal-c/src/` — 6 files: `frame.c`, `epoch_header.c`, `wal_slab.c`, `platform_posix.c`, `platform_windows.c`, `platform_uring.c`, `wal_internal.h` | Written. Not yet verified end to end on this machine |
| ABI headers | `sheaf-db/native/include/sheaf_kernel.h`, `sheaf_storage.h`, `sheaf_wal.h` | Written |
| Rust FFI crate | `sheaf-db/crates/sheaf-native/` — `build.rs`, `build/kernel.rs`, `build/storage.rs`, `build/wal.rs`, `build/toolchain.rs`, `src/abi.rs`, `src/kernel/mod.rs`, `src/kernel/native.rs`, `src/kernel/fallback.rs`, `tests/kernel_parity.rs` | Written. Blocked on compile faults fixed in this session |

---

## 3. Toolchain installs performed in this session

All three packages were installed with winget on Windows (x86_64):

| Package | ID | Version | Purpose |
|---|---|---|---|
| Zig | `zig.zig` | 0.17.0 | C++23 kernel via `zig c++`, C via `zig cc`, storage tier via `zig build-lib` |
| WinLibs/MinGW | `BrechtSanders.WinLibs.POSIX.UCRT` | gcc/g++ 16.2.0 | Plain `gcc` / `g++` C and C++ compilers |
| LLVM | `LLVM.LLVM` | 23.1.5 | `clang` / `clang++` |

Verification:

- `zig version` prints `0.17.0`.
- `gcc --version` prints `16.2.0`.
- `clang++ --version` prints `clang version 23.1.5`.
- After install, the shell PATH must be reloaded from the Machine and
  User environment. The shell does not refresh PATH by itself.

---

## 4. Faults found and fixed in this session (7 files modified)

Each fix is a minimum diff inside the faulty file. All fixes keep the
fallback rule: a missing toolchain never fails the build.

| # | File | Fault | Fix |
|---|---|---|---|
| 1 | `sheaf-db/crates/sheaf-native/build.rs` | `canonicalize()` returns a `\\?\` verbatim path. Zig clang mis-resolves quoted includes from such paths (`internal.hpp` not found) | Strip the `\\?\` prefix in `strip_verbatim()` before any tier uses the path |
| 2 | `sheaf-db/crates/sheaf-native/build/kernel.rs` | Compile command had no include path for the kernel `src` dir | Add `-I kernel-cpp/src` next to `-I include` |
| 3 | `sheaf-db/crates/sheaf-native/build/storage.rs` | `zig build-lib` received `-femit-bin` as a separate argument; zig expects `-femit-bin=<path>` in one argument | Pass one argument `format!("-femit-bin={}", lib.display())` |
| 4 | `sheaf-db/native/kernel-cpp/src/internal.hpp` | Missing `<cmath>` for `std::sqrt`; AVX-512 helper uses `_mm_popcnt_u32` without the `popcnt` feature | Add `#include <cmath>`; add `popcnt` to the `SHEAF_TARGET_AVX512` attribute |
| 5 | `sheaf-db/native/kernel-cpp/src/cpu_features.cpp` | `_xgetbv` needs the `xsave` target feature | Mark `probe_caps()` with `__attribute__((target("xsave")))` on GNU/clang |
| 6 | `sheaf-db/native/kernel-cpp/src/xor_delta.cpp` | `sheaf_xor_delta_encode` inlines AVX-512 intrinsics in an unmarked function | Move the loop into a marked helper `encode_avx512()` and call it |
| 7 | `sheaf-db/native/storage-zig/src/spine_soa.zig` | Zig 0.17 rejects a `count` parameter that shadows the `count` function | Rename the `pack` parameter `count` to `n_cells` |

---

## 5. Build evidence so far

Passing results before the pause:

- `cargo test -p sheaf-native --features force-fallback` — 9 tests passed.
- `cargo test -p sheaf-native` (fallback only) — 9 tests passed.
- `cargo clippy -p sheaf-native --all-targets -- -D warnings` — clean.
- After fixes 1, 2, 4, 5, 6: the C++ kernel tier compiles (zig c++).
- After fix 7 and the manual zig run: the Zig storage tier compiles to
  `sheaf_storage.lib` with `-target x86_64-windows-gnu`.
- With zig c++ on Windows + MSVC rustc, the link of the kernel objects
  fails with `LNK1143 invalid or corrupt file` (GNU objects vs MSVC
  linker). This is the reason the next session should try `clang++`
  (MSVC target) instead of `zig c++` for the kernel tier.

A test run with `SHEAF_KERNEL_CXX=clang++` was started but was
interrupted by the operator before it finished. No final
native-link result is claimed in this record.

---

## 6. Remaining work from the plan (11 pillars)

| Pillar | Content | State |
|---|---|---|
| 1. Vector compute (C++23) | Factors, tiles, Chebyshev, XOR, validity, CPUID | Code written. Native link not yet proven with clang++ |
| 2. Storage (Zig BCSF) | Spine SoA, bump arena, block headers | Code written. Rust `build/storage.rs` fixed. End-to-end link not yet proven |
| 3. WAL (C11) | Framing, A/B slots, slab, POSIX/Windows/uring backends | Code written. Not yet verified on this machine |
| 4. Rust ingress/EBR/LCA-CW | Tokio/Tonic/Arrow Flight, affine leases, thread-per-core | Not started. No files |
| 5. Fiber lifecycle | Fingerprint cache, compaction fencing | Not started on this branch |
| 6. Lean 4 proofs | Sheaf axioms, TIR rewrites, H1 obstruction | Not started. No files |
| 7. Distributed invariants | Open-cover partitioning, ghost stalks, RDMA | Not started. No files |
| 8. Merkle-sheaf VCS | Character-level cells, DAG, time travel | Not started. No files |
| 9. Compression | FSST/Alp/BitPacking, arena sharing | Not started. No files |
| 10. LCA-CW writes | Chronological time charts | Not started. No files |
| 11. Bridges | PyTorch C++/CUDA, pgwire, Arrow Flight, eBPF VFS | Not started. No files |

Roadmap phases from the plan: Phase 1 (kernel) — partially done.
Phase 2 (Zig + C WAL) — partially done. Phase 3 (Rust ingress) —
not started. Phase 4 (Lean + PyTorch) — not started.

`SKELETON.md` has not been updated for the new stack. The project
rule requires the current status in `SKELETON.md` (source of truth).
Update it in the next session.

File-size rule: every new file is under 500 lines. The largest file
is about 377 lines.

---

## 7. Next steps for the next session

1. Reload PATH: Machine, User, plus `C:\Program Files\LLVM\bin`.
2. Re-run: `cargo clean -p sheaf-native`, then
   `cargo test -p sheaf-native` with `SHEAF_KERNEL_CXX=clang++`.
   A passing result must name the linked native cfg flags.
3. Prove each native archive loads: run `zig build test` in
   `storage-zig`, and compile the WAL C files with `clang`.
4. Update `SKELETON.md` with the `sheaf-native` crate and the three
   native tiers. File Design and Code Skeleton gates need this update.
5. Commit each file creation or modification in one commit (§3.2).
6. Push once with `git push` when the build passes (§3.1).
