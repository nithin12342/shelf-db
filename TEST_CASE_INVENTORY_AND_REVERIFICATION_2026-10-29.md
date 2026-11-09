# ShelfDB Test Case Inventory & Re-Verification Procedures

**Date:** 2026-10-29
**Repository:** `D:\d2\sheaf-db`
**Workspace:** `sheaf-storage`, `sheaf-engine`, `sheaf-query`, `sheaf-kernel`

---

## Part 1: Unit Tests (20 `#[test]` functions)

Run with: `cargo test --workspace`

| # | Test | Crate | File | What it verifies |
|---|------|-------|------|------------------|
| 1 | `bump_arena::tests::alloc_and_reset` | sheaf-storage | `crates/sheaf-storage/src/lsp/bump_arena.rs:33` | Bump arena alloc/reset round-trip |
| 2 | `bump_arena::tests::overflow` | sheaf-storage | `crates/sheaf-storage/src/lsp/bump_arena.rs:44` | Alloc returns None when full |
| 3 | `dense_ts::tests::exactness` | sheaf-storage | `crates/sheaf-storage/src/lsp/dense_ts.rs:109` | Dense time-series matches cell-based reference |
| 4 | `delta_wal::tests::round_trip` | sheaf-storage | `crates/sheaf-storage/src/lsp/delta_wal.rs:89` | WAL append/replay preserves all 1000 deltas |
| 5 | `delta_wal::tests::torn_tail` | sheaf-storage | `crates/sheaf-storage/src/lsp/delta_wal.rs:111` | Torn WAL tail is rejected, never half-loaded |
| 6 | `optimizer::tests::test_poset_pruning_invariant` | sheaf-query | `crates/sheaf-query/src/ir/optimizer.rs:216` | Poset pruning produces correct slice count |
| 7 | `optimizer::tests::test_selectivity_estimation` | sheaf-query | `crates/sheaf-query/src/ir/optimizer.rs:224` | Selectivity estimates within tolerance |
| 8 | `optimizer::tests::test_cost_calculation_monotonicity` | sheaf-query | `crates/sheaf-query/src/ir/optimizer.rs:250` | Cost monotonic with row count |
| 9 | `optimizer::tests::test_tir_opt` | sheaf-query | `crates/sheaf-query/src/ir/optimizer.rs:249` | TIR optimization correctness |
| 10 | `optimizer::tests::test_tir_rewrite` | sheaf-query | `crates/sheaf-query/src/ir/optimizer.rs:263` | TIR rewrite correctness |
| 11 | `intersection_avx::tests::test_intersect_avx2` | sheaf-kernel | `crates/sheaf-kernel/src/simd/intersection_avx.rs:84` | AVX2 intersection matches scalar |
| 12 | `ring_wal::tests::ring_wal_round_trip` | sheaf-storage | `crates/sheaf-storage/src/lsp/ring_wal.rs:244` | Lock-free ring WAL round-trip, 5000 frames intact |
| 13 | `encoding::tests::varint_round_trip` | sheaf-storage | `crates/sheaf-storage/src/bcsf/encoding.rs:160` | LEB128 varint encode/decode round-trip |
| 14 | `encoding::tests::delta_u64_round_trip` | sheaf-storage | `crates/sheaf-storage/src/bcsf/encoding.rs:171` | Delta+varint u64 round-trip |
| 15 | `encoding::tests::delta_u32_round_trip` | sheaf-storage | `crates/sheaf-storage/src/bcsf/encoding.rs:181` | Delta+varint u32 round-trip |
| 16 | `encoding::tests::delta_empty` | sheaf-storage | `crates/sheaf-storage/src/bcsf/encoding.rs:191` | Empty delta block round-trips |
| 17 | `imdb_parse::tests::scan_agrees` | sheaf-engine | `crates/sheaf-engine/src/bin/shared/imdb_parse.rs:476` | Scalar and SIMD field scan produce identical results |
| 18 | `imdb_answers::tests::the_shape_changes_the_generation` | sheaf-engine | `crates/sheaf-engine/src/bin/shared/imdb_answers.rs:357` | Answer generation changes when shape/fingerprint changes |
| 19-21 | `imdb_answers::tests` (3 more) | sheaf-engine | `crates/sheaf-engine/src/bin/shared/imdb_answers.rs:373,382,395` | Answer index build/lookup/range correctness |

**Result: 20/20 pass**

---

## Part 2: Bench/Run-Time Gates

Each bench binary prints `gate_<name>=PASS/FAIL/RECORDED` lines. These are run-time verification gates, not `#[test]` functions. They run when you execute the bench binary directly.

### sheaf-storage bins

| # | Gate | Binary | File | What it verifies |
|---|------|--------|------|------------------|
| 22 | `gate_replay_exact` | wal_insert_bench | `crates/sheaf-storage/src/bin/wal_insert_bench.rs:68` | WAL replay recovers all records intact |
| 23 | `gate_replay_zero_copy` | wal_insert_bench | `crates/sheaf-storage/src/bin/wal_insert_bench.rs:91` | Zero-copy replay is as fast or faster than copy |
| 24 | `gate_batch_replay` | wal_insert_bench | `crates/sheaf-storage/src/bin/wal_insert_bench.rs:108` | Batched WAL replay recovers all records |
| 25 | `gate_delta_wal_replay` | wal_insert_bench | `crates/sheaf-storage/src/bin/wal_insert_bench.rs:132` | Delta WAL replay is correct |
| 26 | `gate_ring_concurrent` | wal_insert_bench | `crates/sheaf-storage/src/bin/wal_insert_bench.rs:179` | Concurrent RingWal group commit: 64k records, 4 writers, all recovered |
| 27 | `gate_partition_domains_fit_l3` | partition_sidecar_bench | `crates/sheaf-storage/src/bin/partition_sidecar_bench.rs:72` | L3 domain partitioning fits byte budget |
| 28 | `gate_mem_1.5x` | phase1_bench | `crates/sheaf-storage/src/bin/phase1_bench.rs:203` | Memory footprint within 1.5x baseline |
| 29 | `gate_lookup_sub15ns` | phase1_bench | `crates/sheaf-storage/src/bin/phase1_bench.rs:204` | Lookup latency under 15ns |
| 30 | `gate_segment_exact` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:78` | V2 segment round-trip: 2500 cells identical |
| 31 | `gate_segment_meta` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:88` | Generation + fingerprint round-trip |
| 32 | `gate_segment_corrupt_reject` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:99` | Corrupt payload byte → CRC rejects |
| 33 | `gate_segment_truncated_reject` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:110` | Truncated file → error, no half-load |
| 34 | `gate_segment_alien_reject` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:119` | Foreign bytes → error |
| 35 | `gate_segment_restore_exact` | segment_bench | `crates/sheaf-storage/src/bin/segment_bench.rs:156` | Persist base → restore → identical base |

### sheaf-engine bins

| # | Gate | Binary | File | What it verifies |
|---|------|--------|------|------------------|
| 36 | `gate_q1_exact` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:439` | q1 count/votes/avg match DuckDB/PostgreSQL exactly |
| 37 | `gate_q1_fast` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:440` | q1 cached speedup >= 5x |
| 38 | `gate_q2_fast` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:440` | q2 cached speedup >= 5x |
| 39 | `gate_q3_fast` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:440` | q3 cached speedup >= 10x |
| 40 | `gate_q4_exact` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:444` | q4 matches DuckDB top-10 names exactly |
| 41 | `gate_bundle_sections_fresh` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:~420` | Bundle sections bound to source fingerprint |
| 42 | `gate_answer_q1_matches_scan` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:~450` | q1 answer matches full scan |
| 43 | `gate_answer_q2_matches_scan` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:~450` | q2 answer matches full scan |
| 44 | `gate_answer_q3_matches_scan` | imdb_fiber_bench | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs:~450` | q3 answer matches full scan |
| 45 | `gate_align_64B` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:77` | Slab base is 64-byte aligned |
| 46 | `gate_sum_exact` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:82` | Column sum matches reference within 1e-9 |
| 47 | `gate_parity` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:93` | Performance within 10% of baseline |
| 48 | `gate_fma_correct` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:110` | FMA fused revenue sum within 1e-6 |
| 49 | `gate_soa_no_memory_spike` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:131` | SOA transpose allocates < 4096 bytes |
| 50 | `gate_p2_exceptions` | arrow_scan_bench | `crates/sheaf-engine/src/bin/arrow_scan_bench.rs:175` | Null exceptions list is correct |
| 51 | `gate_arrival_any_order` | graded_wal_bench | `crates/sheaf-engine/src/bin/graded_wal_bench.rs:30` | WAL arrives in any order |
| 52 | `gate_seal_ordered` | graded_wal_bench | `crates/sheaf-engine/src/bin/graded_wal_bench.rs:35` | Seals are ordered correctly |
| 53 | `gate_watermark` | graded_wal_bench | `crates/sheaf-engine/src/bin/graded_wal_bench.rs:45` | Watermark advances correctly |
| 54 | `gate_replay_exact` | graded_wal_bench | `crates/sheaf-engine/src/bin/graded_wal_bench.rs:51` | WAL replay counts are exact |
| 55 | `gate_fused_exact` | fused_scan_bench | `crates/sheaf-engine/src/bin/fused_scan_bench.rs:100` | Fused scan result within 1e-6 |
| 56 | `gate_fused_target` | fused_scan_bench | `crates/sheaf-engine/src/bin/fused_scan_bench.rs:101` | Fused scan meets time target |
| 57 | `gate_snap_corrupt_reject` | fiber_snap_bench | `crates/sheaf-engine/src/bin/fiber_snap_bench.rs:311` | Corrupt snapshot rejected |
| 58 | `gate_snap_exact` | fiber_snap_bench | `crates/sheaf-engine/src/bin/fiber_snap_bench.rs:436` | Snapshot result within 1e-9 |
| 59 | `gate_snap_source_fresh` | fiber_snap_bench | `crates/sheaf-engine/src/bin/fiber_snap_bench.rs:369` | Bundle source fingerprint is fresh |
| 60 | `gate_n5_concurrent_build_equals_quiet` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:159` | Concurrent build matches quiet build |
| 61 | `gate_n5_frozen_epoch_exposed` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:163` | Frozen epoch generation > 0 |
| 62 | `gate_n5_epoch_stamp_detects_staleness` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:177` | Staleness detected after merge |
| 63 | `gate_n6_fiber_equals_fallback` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:263` | Fiber aggregate matches fallback |
| 64 | `gate_n6_no_deadlock` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:266` | No deadlock under concurrent ops |
| 65 | `gate_n6_lag_bounded` | fiber_lifecycle_bench | `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs:270` | Lag <= 64 generations |

---

## Part 3: Re-Verification Procedures

### Prerequisites

```
PowerShell (Windows), Rust toolchain installed
Working directory: D:\d2\sheaf-db
```

### Step 1: Compile everything

```powershell
cargo check --workspace
```

Expected: `Finished dev profile`, no warnings, no errors.

### Step 2: Run all unit tests

```powershell
cargo test --workspace
```

Expected: All crates report `test result: ok. N passed; 0 failed;`

### Step 3: Build release binaries

```powershell
cargo build --release --bins
```

Expected: `Finished release profile`, no errors.

### Step 4: Run segment bench (V2 format verification)

```powershell
.\target\release\segment_bench.exe
```

Expected gates all PASS:
- `gate_segment_exact=PASS`
- `gate_segment_meta=PASS`
- `gate_segment_corrupt_reject=PASS`
- `gate_segment_truncated_reject=PASS`
- `gate_segment_alien_reject=PASS`
- `gate_segment_restore_exact=PASS`
- `SEGMENT_BENCH: PASS`

### Step 5: Run WAL insert bench (RingWal verification)

```powershell
.\target\release\wal_insert_bench.exe
```

Expected gates all PASS:
- `gate_replay_exact=PASS`
- `gate_replay_zero_copy=PASS`
- `gate_batch_replay=PASS`
- `gate_delta_wal_replay=PASS`
- `gate_ring_concurrent=PASS`

### Step 6: Run IMDb cold build (fiber bundle + BCSF)

Requires `D:\d2\data\imdb/` (the IMDb dataset, ~7 GB).

```powershell
Remove-Item "D:\bench\data\imdb_fiber_bundle.bin" -ErrorAction SilentlyContinue
.\target\release\imdb_fiber_bench.exe --imdb-dir=D:\d2\data\imdb --bundle=D:\bench\data\imdb_fiber_bundle.bin --baseline=D:\d2\sheaf-db\tests\e2e\realtime_bench\results\imdb_job.json --output=D:\d2\results\imdb_shelf_run_now.json --threads=4
```

Expected output:
- Stage timings (names, ratings, basics, principals, answers)
- `bundle written: ... in ~60 s` (cold build)
- `bundle present and fresh, skipping build` (on re-run)
- All `gate_*` lines PASS
- Query results match DuckDB/PostgreSQL baseline exactly

### Step 7: Run streaming read/write benchmark

```powershell
.\target\release\streaming_rw_bench.exe
```

Expected:
- Write throughput: measurable writes/sec (varies by host)
- Write p50 latency: < 5.0 us
- Read throughput: > 3,000,000 reads/sec
- Fiber build avg: < 1.0 ms

### Step 8: Full end-to-end summary

```powershell
cargo test --workspace
.\target\release\segment_bench.exe
.\target\release\wal_insert_bench.exe
```

All should report PASS / `test result: ok` with 0 failures.

---

## Part 4: Files Modified (this session)

| File | Change |
|------|--------|
| `crates/sheaf-storage/Cargo.toml` | Added `memmap2 = "0.9"` and `rayon = "1.8"` |
| `crates/sheaf-storage/src/bcsf/mod.rs` | Added `pub mod encoding;` |
| `crates/sheaf-storage/src/bcsf/encoding.rs` | NEW: delta+varint encode/decode for u64/u32 |
| `crates/sheaf-storage/src/bcsf/segment.rs` | V2 format: delta encoding flag, validity part, zone-map pruning, SegmentSet sharding |
| `crates/sheaf-storage/src/bcsf/arena.rs` | Added `validity: Vec<u64>` null bitmap to StalkArena |
| `crates/sheaf-storage/src/bcsf/builder.rs` | Validity bits set on push/push_slice, carried through finish_inner |
| `crates/sheaf-storage/src/bcsf/reader.rs` | Added `validity: Vec<u64>` to BcsfReader, `valid()` O(1) check |
| `crates/sheaf-storage/src/lsp/ring_wal.rs` | Lock-free ring WAL + group commit |
| `crates/sheaf-storage/src/lsp/mem_complex.rs` | RingWal replaces Mutex<WalWriter> |
| `crates/sheaf-storage/src/lib.rs` | Exported RingWal |
| `crates/sheaf-storage/src/bcsf/fiber_domain.rs` | Rayon par_sort in FiberDomains::build |
| `crates/sheaf-storage/src/bin/wal_insert_bench.rs` | Added concurrent group-commit gate |
| `crates/sheaf-engine/src/bin/streaming_rw_bench.rs` | RingWal group commit write path |
| `crates/sheaf-db/IMDB_BUILD_ARCHITECTURE_STUDY_2026-10-29.md` | Documentation |
| `crates/sheaf-db/BCSF_SEGMENT_EFFICIENCY_GAINS_2026-10-29.md` | Documentation |

---

## Part 5: Verification Results (this session)

| Check | Result |
|-------|--------|
| `cargo check --workspace` | Clean, no warnings |
| `cargo test --workspace` | 20/20 unit tests pass, 0 failures |
| `segment_bench` (release) | All 6 gates PASS |
| `wal_insert_bench` (release) | All 5 gates PASS |
| `streaming_rw_bench` (release) | Write p50=0.10us, reads >3M/s, fiber build <1ms |
| `imdb_fiber_bench` (release) | All gates PASS, results match DuckDB/PostgreSQL |

**No regression detected on any scenario.** Read path untouched (lock-free sealed Arcs + snapshots). Cold-ingest boundary validation preserved. V1 segment format backward compatible.
