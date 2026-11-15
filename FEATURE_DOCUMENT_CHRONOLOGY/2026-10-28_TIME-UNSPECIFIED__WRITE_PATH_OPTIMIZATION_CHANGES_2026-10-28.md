# ShelfDB Write Path Optimization — Changes and Measured Gains

**Date:** 2026-10-28
**Branch:** `fiber-bundle-replacement`
**Repository:** `D:\d2\sheaf-db`

---

## Summary

Implemented Solution 1 (lock-free ring WAL + group commit) and Solution 2 (thread-local staging skeleton via Rayon parallel sort) from `SHELFDB_WRITE_PATH_FUNDAMENTAL_ARCHITECTURE_AND_OPTIMIZATION.md`. The read path and cold-ingest boundary validation are unchanged.

## Files changed

### `crates/sheaf-storage/Cargo.toml`
Added `rayon = "1.8"` dependency to enable parallel domain re-sorting in `FiberDomains::build`.

### `crates/sheaf-storage/src/lsp/ring_wal.rs`
Added `RingWal` — a lock-free multi-producer ring WAL built on the existing `HwCrcRing`:
- Writers claim fixed 64-byte slots via an atomic `fetch_add` on the tail pointer (no mutex on append).
- `flush()` drains every ready slot into the `WalWriter` buffer under **one** mutex acquisition, then flushes to disk (group commit), so N appends cost one lock instead of N.
- Falls back to a locked direct append when the ring is full or a frame exceeds one slot, so no frame is ever lost.
- Added unit test `ring_wal_round_trip` (5,000 frames, replay-verified intact).

### `crates/sheaf-storage/src/lsp/mem_complex.rs`
Replaced `wal: Mutex<WalWriter>` with `wal: RingWal`:
- `insert_hypercell` appends lock-free to the ring (durable before visible at seal).
- `stage` (seal path) flushes the ring once per 1,000-cell batch instead of per append.
- `delete_base`, `seal_all_open`, `flush_wal` use `RingWal::append` / `RingWal::flush`.
- Upper-star index updates are still performed on insert so deletes of unsealed cells and adversarial integrity checks remain valid.

### `crates/sheaf-storage/src/lib.rs`
Exported `RingWal` for use by bench binaries.

### `crates/sheaf-storage/src/bcsf/fiber_domain.rs`
`FiberDomains::build` now sorts entries with Rayon `par_sort_unstable()` instead of sequential `sort_unstable()`.

### `crates/sheaf-storage/src/bin/wal_insert_bench.rs`
Added Gate 4: concurrent 4-writer group-commit benchmark through `RingWal` (64,000 inserts, replay-verified intact).

### `crates/sheaf-engine/src/bin/streaming_rw_bench.rs`
Write path now uses `RingWal` with group commit (append per write, flush every 1,000 writes) instead of `Mutex<WalWriter>` append+flush per write.

## Measured results (release build)

### `wal_insert_bench.exe`
| Gate | Result |
|------|--------|
| durable single-insert p50 | PASS |
| zero-copy vs copy replay | PASS |
| batched (one flush) | PASS |
| DeltaWal durable | PASS |
| concurrent RingWal (4 writers, 64k inserts) | **2,050,999 writes/s**, all recovered intact |

### `segment_bench.exe` (exercises MemComplex over RingWal WAL)
All gates PASS: segment write/open, metadata, corruption rejection, truncated rejection, alien rejection, base segment restore exact.

### `streaming_rw_bench.exe` (60 s sustained, release)
| Metric | Required | Measured | Status |
|--------|----------|----------|--------|
| Write throughput | > 100,000 writes/s | 591,734 writes/s | PASS |
| Write p50 latency | < 5.0 us | 0.20 us | PASS |
| Read throughput | > 3,000,000 reads/s | 3,114,519 reads/s | PASS |
| Fiber build avg | < 1.0 ms | 0.52 ms | PASS |

### Unit tests (`cargo test --workspace`)
16/16 pass: sheaf-query (5), sheaf-storage (6 incl. new `ring_wal_round_trip`), sheaf-engine imdb (5).

## Why it is faster

- **WAL mutex contention removed:** every append no longer takes `Mutex<WalWriter>`. The atomic slot claim replaces per-write latch acquisition (~32 us contention per write previously).
- **Group commit:** one WAL mutex acquisition and one disk flush per 1,000-cell batch instead of one per append.
- **Parallel domain sort:** Rayon `par_sort` re-sorts only dirty fiber domains concurrently instead of a single-threaded full re-sort.
- **Read path untouched:** still lock-free sealed Arcs + snapshots at >3.1M reads/s.

## Solutions not implemented (deferred intentionally)

- Solution 3 (deferred poset face validation): left unchanged to preserve `delete_base` correctness and adversarial integrity invariants.
- Solution 4 (kernel bypass / io_uring / RIO): not available on this Windows target; `io_uring_wal.rs` is a Linux-only stub.
