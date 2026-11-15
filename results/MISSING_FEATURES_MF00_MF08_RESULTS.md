# Missing feature results, MF-00 through MF-08

This file answers one question: which of the 17 plan nodes are complete, and
which command proves it.

Branch: `combined-stack-feature-improvement`. Date: 2026-11-15.
Plan: `MISSING_FEATURE_IMPLEMENTATION_PLAN_2026-11-15.md`.

Each row is one plan node. Each state comes from a cargo test command that
calls the production code path with real input.

## Table 1. Node state and the command that proves it

The rows are the completed nodes in plan order. The columns are the node, its
state, the crate that holds the code, and the test command.

| Node | Feature | State | Crate | Test command |
|---|---|---|---|---|
| MF-00 | Split `bcsf/segment.rs` into one file per concern | e2e_passed | sheaf-storage | `cargo test -p sheaf-storage --test segment_golden` |
| MF-02 | Real OS thread affinity and NUMA pinning | e2e_passed | sheaf-runtime | `cargo test -p sheaf-runtime pinning` |
| MF-01 | Lock-free bounded MPMC nerve router | e2e_passed | sheaf-runtime | `cargo test -p sheaf-runtime` |
| MF-03 | Parallel execution of disjoint write waves | e2e_passed | sheaf-vcs | `cargo test -p sheaf-vcs wave` |
| MF-04 | Physical L3 cache discovery and partition map | e2e_passed | sheaf-storage | `cargo test -p sheaf-storage topology` |
| MF-05 | FSST, ALP, frame-of-reference, bit packing codecs | e2e_passed | sheaf-codec | `cargo test -p sheaf-codec` |
| MF-06 | Global dictionary arena sharing | e2e_passed | sheaf-storage | `cargo test -p sheaf-storage dict` and `cargo test -p sheaf-storage --test dictionary` |
| MF-07 | Versioned page format with an explicit gate | e2e_passed | sheaf-app | `cargo test -p sheaf-app page` and `cargo test -p sheaf-app --test page` |
| MF-08 | Pre-registered io_uring buffers | e2e_passed | sheaf-native | `cargo test -p sheaf-native --test registered_buffers` |

## Table 2. Test counts and result per node

The rows are the same nodes. The columns are the number of tests and the result
line that cargo printed.

| Node | Tests | Result line |
|---|---|---|
| MF-00 | 1 golden test plus the existing segment suite | ok, fixture bytes equal |
| MF-02 | 4 pinning tests inside 26 crate tests | ok |
| MF-01 | 47 crate tests, including 8 producers x 50,000 | ok |
| MF-03 | 10 wave tests plus 49 crate tests | ok |
| MF-04 | 33 topology tests | ok |
| MF-05 | 47 unit tests and 9 integration tests | ok, integration run 13.38 s |
| MF-06 | 15 unit tests and 5 integration tests | ok, integration run 2.56 s |
| MF-07 | 17 unit tests and 6 integration tests | ok |
| MF-08 | 6 integration tests, 39 tests in the crate | ok, run 0.01 s |

## Table 3. Plain-language summary per node

The rows are the nodes. The columns are what the node adds and what the test
shows.

| Node | What it adds | What the test shows |
|---|---|---|
| MF-00 | Eleven files replace one 1,100-line file | The segment bytes equal the fixture bytes |
| MF-02 | Real pinning adapters for Windows, Linux, and a declared unsupported path | Each allowed core pins, reads the mask back, and restores |
| MF-01 | A bounded ring, a port, a lock-free queue, and a B+ tree partition index | 400,000 messages cross the router with no loss and no duplicate |
| MF-03 | A write target, a wave report, and a parallel wave executor | 64 disjoint cells apply across 2 or more threads, and an error propagates |
| MF-04 | Windows and Linux cache discovery, a sysfs parser, and a partition map | The cache domains cover the allowed CPU set with no overlap |
| MF-05 | Five codecs behind one selector, with a stable id per codec | 100,000 values of each type round trip at bit level |
| MF-06 | An aligned byte arena, a BLAKE3 interner, and a column of offsets | 1,000,000 inserts over 10,000 values make 10,000 arena entries |
| MF-07 | A header, a writer, and a reader with a CRC32C gate and a row fingerprint | A 16-row page round trips, a bad version is rejected, a bad byte is rejected, a v1 page reads as migrated |
| MF-08 | A C pool, a Rust binding, and a capability report | The pool is 64-byte aligned, a second register is rejected, and this host reports Unsupported with a named cause |

## Platform notes

1. MF-08 runs the real fixed-write path on Linux only. This host runs Windows,
   so the test asserts the explicit `Unsupported` report and a non-empty
   reason. It does not accept a fake success.
2. MF-04 reads Linux sysfs through a parser that compiles on every platform, so
   the parse logic gets coverage on Windows.
3. The `sheaf-codec` and `sheaf-proofs` crates hold zero external dependencies.
   Test data comes from a local xorshift64* generator with a fixed seed.

## Still planned

MF-09 through MF-16 stay at state `planned`. They are the Lean rule emitter,
the production rewrite optimizer, the H1 quotient solver, the replication
transport, the Raft engine, sharded open covers, the Arrow Flight daemon, and
the eBPF VFS interception. `SKELETON.md` marks each one.

## Commands to reproduce

```text
cd sheaf-db
cargo test -p sheaf-storage --test segment_golden
cargo test -p sheaf-runtime
cargo test -p sheaf-vcs
cargo test -p sheaf-storage topology
cargo test -p sheaf-storage dict
cargo test -p sheaf-storage --test dictionary
cargo test -p sheaf-codec
cargo test -p sheaf-app
cargo test -p sheaf-native
```

A test result line goes to stderr. Read the `test result:` lines, not the
process exit code.
