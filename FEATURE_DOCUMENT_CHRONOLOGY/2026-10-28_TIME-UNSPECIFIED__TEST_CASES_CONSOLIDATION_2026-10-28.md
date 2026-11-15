# TEST_CASES_CONSOLIDATION_2026-10-28.md: Consolidating All Test Cases in Sheaf-DB

## Timestamp and Environment Details

- **Document Date:** 2026-10-28
- **Local Time:** 2026-10-28 16:51:00 IST (UTC+05:30)
- **Coordinated Universal Time (UTC):** 2026-10-28 11:21:00 UTC
- **Host Machine:** Windows 11, Intel Core i7-1165G7 @ 2.80GHz (4 cores, 8 threads), 15.8 GB RAM, NVMe PCIe 4.0 x4 SSD
- **Active Git Branch:** `fiber-bundle-replacement`
- **Source of Truth:** `SKELETON.md`

---

## 1. Unit Tests in Crates

The command `cargo test --workspace` executes the following unit test cases.

### 1.1 `sheaf-query` Unit Tests
Location: `crates/sheaf-query/src/ir/optimizer.rs`

| Test Identifier | Target Functionality |
|---|---|
| `ir::optimizer::tests::test_cost_calculation_monotonicity` | Verifies cost model increases with relation size |
| `ir::optimizer::tests::test_explain_costed_output` | Verifies plan explanation text generation |
| `ir::optimizer::tests::test_poset_pruning_invariant` | Verifies down-closure poset pruning preserves bounds |
| `ir::optimizer::tests::test_selectivity_estimation` | Verifies selectivity calculation for topological filters |

### 1.2 `sheaf-storage` Unit Tests
Location: `crates/sheaf-storage/src/lsp/`

| Test Identifier | File Location | Target Functionality |
|---|---|---|
| `lsp::bump_arena::tests::alloc_and_reset` | `bump_arena.rs` | Verifies bump allocator reset behavior |
| `lsp::bump_arena::tests::overflow` | `bump_arena.rs` | Verifies allocator bounds enforcement |
| `lsp::delta_wal::tests::round_trip` | `delta_wal.rs` | Verifies WAL delta serialization round-trip |
| `lsp::delta_wal::tests::torn_tail` | `delta_wal.rs` | Verifies recovery from truncated WAL records |
| `lsp::dense_ts::tests::exactness` | `dense_ts.rs` | Verifies dense time-series exact precision |

### 1.3 `sheaf-engine` Unit Tests
Location: `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs`

| Test Identifier | Target Functionality |
|---|---|
| `imdb_build::imdb_answers::tests::the_q1_answer_round_trips` | Verifies Q1 answer section encoding round-trip |
| `imdb_build::imdb_answers::tests::the_q2_answer_round_trips` | Verifies Q2 aggregate section encoding round-trip |
| `imdb_build::imdb_answers::tests::the_q3_answer_round_trips` | Verifies Q3 decade rollup encoding round-trip |
| `imdb_build::imdb_answers::tests::the_shape_changes_the_generation` | Verifies section generation updates on schema change |
| `imdb_build::imdb_parse::tests::scan_agrees` | Verifies parser token agreement across chunks |

---

## 2. End-to-End (E2E) Phase Gate Test Suites

Location: `sheaf-db/tests/e2e/`

Every phase suite executes a binary with gate assertions.

| Phase Suite | Executed Binary | Target Invariants and Gates |
|---|---|---|
| `phase1_bcsf` | `sheaf-storage/phase1_bench` | `gate_mem_1.5x=PASS`, `gate_lookup_sub15ns=PASS` |
| `phase2_lsp` | `sheaf-engine/phase2_bench` | MVCC snapshot isolation, WAL replay CRC, 50k write throughput |
| `phase2_adv` | `sheaf-engine/adversarial` | Concurrency stress on mem_complex and lock-free shards |
| `phase3_kernel` | `sheaf-engine/phase3_bench` | `gate_hand_delta3`, `gate_converge_25iter`, `gate_energy_1em4`, `gate_solve_8ms`, `gate_par_equiv` |
| `phase3_adv` | `sheaf-engine/phase3_adv` | Ill-conditioned Laplacians, high-noise convergence |
| `phase4_tql` | `sheaf-engine/phase4_bench` | TQL syntax lexing, AST construction, TIR 12-opcode lowering |
| `phase5_opt` | `sheaf-engine/phase5_bench` | E-graph rule application, cost model reduction |
| `phase5_adv` | `sheaf-engine/phase5_adv` | Extreme filter combinations, pushdown stability |
| `phase6_split` | `sheaf-engine/phase6_bench` | Head-to-head multi-client QPS, P99 sub-millisecond latency |
| `phase6_adv` | `sheaf-engine/phase6_adv` | Heavy concurrent latch stress |
| `phase7_dist` | `sheaf-engine/phase7_bench` | Mayer-Vietoris decomposition, zero-hop routing, async Schwarz |
| `fiber_lifecycle`| `run_fiber_lifecycle.py` | 6 fiber lifecycle gates (N1 through N6) |
| `von_neumann` | `physical_limits_probe` | Physical hardware baseline limits (L1 through L18) |

---

## 3. Specialized Performance Benchmark Harnesses

Location: `crates/sheaf-engine/src/bin/` and `crates/sheaf-storage/src/bin/`

| Binary Name | Target Subsystem | Measured Metrics |
|---|---|---|
| `q1_kernel_bench` | IMDb Q1 Query Variants | Variant A (current join) vs B (4 accumulators) vs C (4 threads) vs D (answer fiber) |
| `fused_scan_bench` | Columnar SIMD Scan | `fused_sum_pairs` (AoS) vs `fused_sum_columns` (SoA unit stride) |
| `mt_scan_bench` | Multi-Thread Partition Scan | 1 to 16 threads scaling, memory bus saturation |
| `arrow_scan_bench` | Arrow Slabs and Nulls | Validity bitmap scan vs sparse exception lists |
| `coord_scan_bench` | Coordinate Vectors | Contiguous 64B-aligned 0-cochain inner product sums |
| `hodge_bench` | Hodge Smoothing | Damped Jacobi residual smoothing before PCG solve |
| `p0_gates_bench` | Chebyshev Solver | Degree-K Chebyshev polynomial semi-iteration without preconditioner |
| `bundle_load_bench`| Storage Reader and CRC | Header parse vs section read vs SSE4.2 CRC32C verify |
| `ring_wal_bench` | Ring Buffer WAL | Hardware SSE4.2 CRC vs software CRC, lock-free throughput |
| `segment_bench` | BCSF Persistent Segments| Segment write, open, validate, and corruption rejection |
| `shadow_paging_bench`| BCSF Shadow Paging | O(delta) shadow merge vs full copy |
| `fiber_snap_bench` | Fiber Snapshots | Read consistency across frozen epochs |
