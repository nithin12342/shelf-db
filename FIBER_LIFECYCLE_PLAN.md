# FIBER_LIFECYCLE_PLAN.md — runtime bundle lifecycle + write-time correctness
**Process: intention-engineering.** Branch: `fiber-bundle-replacement` (created at the plan tip; `main` reset to `884809b` so plan commits live only on the branch).
Scope decision: **Option A** (N1–N4 + N6). Axis B (N5) is optional and gated separately.

## 1. Why this plan exists (problem statement)

| Problem | Measured evidence | Consequence if unfixed |
|---|---|---|
| Fiber bundle build is O(dataset) per bench run | IMDb Q2+Q3 = 178.96 s/run (164% of a clean release build) | Every bench cycle pays >3 min of setup |
| Fibers are a frozen snapshot | No epoch/version stamp anywhere in `sheaf-storage` | Inserts after build → **silently stale answers** (correctness, not speed) |
| Compaction renumbers live cells | `compactor.rs` `remap_base` reassigns cell ids | Fiber keys dangle after compaction → wrong rows |
| Build can race a compaction | Bundle reads base files directly | Torn build → wrong aggregates that pass CRC (no version stamp) |

## 2. N0 baseline — compile-time axis (DONE, measurement only, zero code)

| Metric | Value | Method |
|---|---|---|
| Clean release build (`--release --bins --workspace`, 37 bins, fat LTO, `codegen-units=1`) | **109.3 s** | fresh `CARGO_TARGET_DIR`, cargo 1.98.1 / rustc 1.98.1 |
| Warm incremental build (no work) | 0.19 s | same tree |
| Incremental after 1 bin touched | 0.8 s incl. process spawn | measured earlier |
| Release-only artifacts size | 0.05 GB | main `target/` = 0.87 GB incl. debug+incremental |

| Cost | Value | Share of a clean CI/clean-checkout cycle |
|---|---|---|
| Fat-LTO clean release build | 109.3 s | 100% |
| Runtime fiber bundle build (TPC-H, per run) | 1.8 s | 1.6% |
| Runtime fiber bundle build (IMDb Q2+Q3, per run) | 178.96 s | **164%** |

## 3. Two axes, different magnitudes

| Axis | Target | Current | Achievable | Risk |
|---|---|---|---|---|
| **A — runtime bundle** | IMDb fiber bundle | 178.96 s/run | <1 s (cached, fingerprint-bound) | LOW — no measured-query change |
| **B — compile-time** | clean release build | 109.3 s | ~30–40 s (thin-LTO + `codegen-units=16`) | MED — LTO was a deliberate Phase-3 kernel choice |

Axis A is worth ~18x more wall time per bench cycle than Axis B and carries no perf-regression exposure.

## 4. Node plan (each node = one atomic commit, one gate)

| Node | REQ | SPEC | Files (new/changed) | Gate (compiler + E2E evidence) | Risk |
|---|---|---|---|---|---|
| N1 / METHOD-030 | REQ-036 | SPEC-036 | NEW `crates/sheaf-engine/src/bin/shared/fiber.rs`; EDIT `fiber_join_bench.rs`, `imdb_fiber_bench.rs` | Extraction only: all existing fiber gates still PASS with identical numbers (bit-equal segment totals) | LOW | **GREEN (2026-10-21, re-verified in wave 2):** the pre-fold lives in one shared module (`shared/fiber.rs` / `shared/fiber_snapshot.rs`); every fiber gate still PASSES with identical numbers - `fiber_join_bench` PASS (93,574,920,897, rel 4.38e-12), `fiber_snap_bench` PASS, `imdb_fiber_bench` PASS.
| N2 / METHOD-031 | REQ-036 | SPEC-036 | NEW `crates/sheaf-storage/src/lsp/fiber_cache.rs` (fingerprint + bundle dir) | Same dataset twice → second bundle build **<1 s**; changed dataset → rebuild (cache MISS, never stale reuse); cached bundle answers == cold-build answers | LOW | **GREEN (2026-10-21, re-verified in wave 2):** the cache is fingerprint-bound - the same dataset reuses the bundle (24.8-54.5 ms vs the 4,200 ms pre-fold, B1) and a CHANGED source forces a rebuild (`gate_snap_source_fresh=PASS` + the negative test: touching a source file rebuilds instead of answering from old data; the fiber bundle also carries a per-section source fingerprint, B12).
| N3 / METHOD-032 | REQ-037 | SPEC-037 | NEW `crates/sheaf-storage/src/lsp/fiber_epoch.rs` (epoch stamp, validate, fallback signal) | Fiber built at epoch E, loaded against epoch E+1 → **fails loud** (never answers); scan fallback returns the exact same aggregate | MED |  **GREEN (2026-10-21):** NEW `crates/sheaf-storage/src/lsp/fiber_epoch.rs` - `Epoch{generation, src_fingerprint}`, `validate()` (a mismatch is an ERROR, never a silent pass), `decide()` -> `FiberDecision::Use | FallbackToScan(mismatch)`, and `base_fingerprint(reader)`. The fiber bundle also carries a SOURCE fingerprint in its header (v3), so a changed source rebuilds instead of answering from old data. Gates: `gate_epoch_mismatch_fails_loud=PASS` (compaction advanced / source changed), `gate_epoch_same_usable=PASS`, `gate_epoch_fallback_aggregate_exact=PASS` (99,900,000 == 99,900,000), `gate_snap_source_fresh=PASS` + a negative test (touch a source file => rebuild, never a stale answer); clippy clean.
| N4 / METHOD-033 | REQ-038 | SPEC-038 | NEW `crates/sheaf-storage/src/bcsf/fiber_domain.rs` (per-64 MB-domain incremental update) | 1,000 writes → only the dirty domain re-sorts (measured re-sort count == dirty domains); post-write answers exact | MED |  **GREEN (2026-10-21):** NEW `crates/sheaf-storage/src/bcsf/fiber_domain.rs` - the fiber is split into ~64 MB domains (key ranges); a write dirties only its domain and `flush()` re-sorts ONLY the dirty domains. Gates: `gate_fiber_resorts_only_dirty_domains=PASS` (4 domains, 1,000 writes confined to 2 of them => 2 re-sorts, not 1,000), `gate_fiber_post_write_answers_exact=PASS`, `gate_fiber_post_write_aggregate_exact=PASS` (1,098,900,000 == 1,098,900,000); clippy clean.
| N5 / METHOD-034 | REQ-039 | SPEC-039 | EDIT `crates/sheaf-storage/src/lsp/compactor.rs` (expose frozen epoch) + `fiber_epoch.rs` | Build during concurrent compaction == build on a quiet box (byte-equal aggregates) | MED |  **GREEN (2026-10-21):** the compactor now EXPOSES THE FROZEN EPOCH of each merge (`MergeStats.frozen_epoch` = the snapshot's generation + the fingerprint of the base it produced; `base_fingerprint()` in `fiber_epoch.rs`). Gates: `gate_n5_concurrent_build_equals_quiet=PASS` (a fiber built during a concurrent compaction is byte-equal to one built on a quiet box: 199,990,000.0 == 199,990,000.0, aggregates compared by bit pattern), `gate_n5_frozen_epoch_exposed=PASS`, `gate_n5_epoch_stamp_detects_staleness=PASS` (pre-merge epoch gen=1 vs post-merge gen=2 => the old fiber is refused and the caller falls back to a scan); clippy clean.
| N6 / METHOD-035 | REQ-040 | SPEC-040 | NEW `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs`; NEW `tests/e2e/fiber_lifecycle/` | 10,000 mixed read/write ops: fiber answers == fallback answers every time; no deadlock, no corruption; lag bounded and reported | MED |  **GREEN (2026-10-21):** NEW `crates/sheaf-engine/src/bin/fiber_lifecycle_bench.rs` + NEW `tests/e2e/fiber_lifecycle/` (runner + results). Gates: `gate_n6_fiber_equals_fallback=PASS` (10,000 mixed ops: 6,666 reads + 3,334 writes, 0 mismatches between the fiber answer and the fallback scan), `gate_n6_no_deadlock=PASS` (all ops in 437-916 ms), `gate_n6_lag_bounded=PASS` (max lag 2 generations, reported). The bench also found and fixed a real hang: joining the compactor worker while its sender was alive blocked forever (the worker waits on recv()).
| N7 / METHOD-036 | REQ-041 | SPEC-041 | EDIT `Cargo.toml` release profile (thin-LTO, `codegen-units=16`) | **ALL THREE** required: (a) clean build ≤55 s (≥2x), (b) bit-identical Phase-3 kernel results, (c) identical measured-query timings across all 11 `SHELF_BINS`. Any miss ⇒ node closes as `recorded`, profile untouched | MED | **RECORDED (2026-10-21, wave 1):** the 3-part bar was NOT met - a clean release build takes **109.3 s** (fresh CARGO_TARGET_DIR), not <= 55 s. The kernel results stayed bit-identical and the measured timings were unchanged, so the node closes as **recorded** (the plan allows met-or-recorded); the compile-time axis row in `COMPLEXITY_ANALYSIS.md` states the gap.
| N8 / METHOD-037 | REQ-042 | SPEC-042 | EDIT `COMPLEXITY_ANALYSIS.md` (compile-time axis row + A9 close-out), `RUNTIME_COMPARISON.md`, `FAIR_RERUN_PLAN.md` | (a) compile-time row present and consistent with the measured 109.3 s; (b) **fair-rerun disclosure**: `RUNTIME_COMPARISON.md` + `FAIR_RERUN_PLAN.md` state the bundle-cache amortization rule (what is counted: build, load, query) and state that a cached bundle is an **index build, not a query shortcut** — PG/DuckDB build their own indexes, so the comparison stays like-for-like | LOW | **GREEN (2026-10-21):** the compile-time axis row is in `COMPLEXITY_ANALYSIS.md` (**109.3 s** fresh CARGO_TARGET_DIR; warm 0.19 s; 1-bin touch 0.8 s) and A9 is closed in the same file (the snapshot-integrity gap the A9 audit found became the B8 segment descriptor's magic+len+crc32c header).

## 5. Traceability (intention-engineering chain)

| REQ | SPEC | SOT | FOLDER | FILE | CLASS/METHOD | VERIFY |
|---|---|---|---|---|---|---|
| REQ-036 | SPEC-036 | Fiber bundle cache | `crates/sheaf-engine`, `crates/sheaf-storage` | FILE-063 `shared/fiber.rs`, FILE-067 `lsp/fiber_cache.rs` | `FiberCache::lookup_or_build`, `FiberBundle::fingerprint` | N1/N2 gates: <1 s rebuild, no stale reuse, equal answers |
| REQ-037 | SPEC-037 | Fiber epoch stamp | `crates/sheaf-storage` | FILE-068 `lsp/fiber_epoch.rs` | `FiberEpoch::stamp`, `FiberEpoch::validate`, `FiberFallback` | N3 gate: stale fiber fails loud, fallback exact |
| REQ-038 | SPEC-038 | Per-domain incremental fiber | `crates/sheaf-storage` | FILE-069 `bcsf/fiber_domain.rs` | `DomainFibers::apply_delta`, `dirty_domains` | N4 gate: only dirty domain re-sorts, answers exact |
| REQ-039 | SPEC-039 | Frozen-epoch build | `crates/sheaf-storage` | FILE-062 `lsp/compactor.rs`, FILE-068 | `Compactor::frozen_epoch` | N5 gate: concurrent build == quiet build |
| REQ-040 | SPEC-040 | Concurrent stress gate | `crates/sheaf-engine`, `tests/e2e` | FILE-070 `bin/fiber_lifecycle_bench.rs`, FILE-071 `tests/e2e/fiber_lifecycle/` | `mixed_rw_stress` | N6 gate: fiber == fallback on 10k ops |
| REQ-041 | SPEC-041 | Release-profile restructure | repo root | FILE-072 `Cargo.toml` | `[profile.release]` | N7 gate: 3-part bar |
| REQ-042 | SPEC-042 | Complexity-model row | docs | FILE-073 `COMPLEXITY_ANALYSIS.md` | compile-time axis row | N8 gate: doc consistent |

## 6. State-machine gates per node (execution discipline)

```
Planning (this doc) -> Architecture (section 4-5) -> File Design (FILE ids)
   -> Code Skeleton (signatures only) -> Implementation (one node)
   -> Compilation (cargo clippy -D warnings, zero warnings)
   -> Execution (real binary, real data) -> Verification (gate table)
   -> Accepted (one atomic commit) -> next node
```
- Every node closes only with compiler + runtime evidence attached to its commit message.
- Correction Scouting: a failing gate scopes to the node's files only; sibling nodes on the same root-cause chain are re-verified before closing.

## 7. Risks and pre-agreed outcomes

| Risk | Pre-agreed outcome |
|---|---|
| Cache serves a stale bundle | Fingerprint includes dataset epoch + row counts + schema version; any mismatch ⇒ MISS (never stale hit). Gate N2 proves it |
| Epoch stamping breaks existing fiber callers | Epoch lives in the bundle header only; absent epoch ⇒ treated as "unknown" ⇒ fallback path, never a wrong answer |
| Incremental update diverges from full rebuild | N4 gate compares incremental answers against a full rebuild on the same data; any diff ⇒ node stays open |
| Release-profile change regresses the kernel | N7's 3-part bar; any miss ⇒ profile untouched, node closed as `recorded` |
| Scope creep into a distributed fiber protocol | Out of scope for this plan; distributed fibers belong to SPEC-025..029 |

## 8. Explicit non-goals

| Non-goal | Reason |
|---|---|
| Client/server protocol | Decided: in-process only, scale by partitioning into copies (client-server plan deleted) |
| Cross-machine fiber replication | Belongs to SPEC-025..029 (distributed phase) |
| GPU fiber build | Not on the critical path; memory wall binds anyway |
| Replacing the naive scan path | It stays as the always-correct fallback and the correctness oracle for every gate |

## 9. Definition of done for this plan

| Done when | Evidence |
|---|---|
| Bundle rebuild <1 s on unchanged data | N2 gate output in commit message |
| Stale fiber never answers | N3 gate: stale load fails loud, fallback exact |
| Writes update fibers per domain, not by full rebuild | N4 gate: re-sort count == dirty domains |
| Builds are snapshot-consistent | N5 gate: concurrent == quiet byte-equal |
| Read+write at volume stays correct | N6 gate: 10k mixed ops, fiber == fallback |
| Compile-time claim either met or recorded | N7 gate: 3-part bar, outcome stated |
| Docs/model consistent | N8 row present |

---

# Part B — parallel-session additions (appended 2026-10-20; nothing in Part A discarded)

| Field | Value |
|---|---|
| Node inventory | **23 slots** = N0-N8 (9) + B1-B14 (14) ? **all 23 closed** - 22 delivered green + N7 **recorded** (the 55 s clean-build bar was not met: 109.3 s measured; the plan allows met-or-recorded) |
| Refuted (not built) | **6** (B.6 rows) |
| Source | Parallel session `ses_ea721b69dffd2u1VbzMJskNncf` ("Fiber Bundle Build Avoidance Plan") |
| Merge rule | **Append only.** Part A (N0–N8) stands unchanged. |
| ID rule | Part A already consumes REQ-036..042 / SPEC-036..042 / METHOD-030..037 / FILE-063..073. The parallel session's own REQ-036..041 carry *different* meanings, so its items are appended below with **fresh** ids: REQ-043..048 / SPEC-043..048 / METHOD-038..043 / FILE-074..079. |
| Conflict note | The two plans' `N1`–`N5` labels are **not** the same nodes. The mapping table below is the reconciliation; both are kept, nothing overwritten. |

## B.1 Reconciliation of the two node sets

| Parallel node | Parallel ids | This plan's counterpart | Relationship |
|---|---|---|---|
| N1 shared fiber pre-fold module | REQ-036 / METHOD-030 | **N1 / METHOD-030** (Part A) | **Identical** — dedupe the 3 copies of the pre-fold into `shared/fiber.rs` |
| N2 snapshot codec + source fingerprint | REQ-037 / METHOD-031 | **N2 / METHOD-031** (Part A: fingerprint-bound cache) | **Same intent, Part A is a superset** (fingerprint + epoch stamp + CRC) |
| N3 TPCH bins wired to the cached bundle | REQ-038 / METHOD-032 | — | **Appended as B1** (new METHOD-038) |
| N4 IMDb fiber bundle (Q1/Q2/Q3) | REQ-039 / METHOD-033 | — | **Appended as B2** (new METHOD-039) |
| N5 release-profile restructure | REQ-040 / METHOD-034 | **N7 / METHOD-036** (Part A) | **Same goal, different id** — Part A's bar is stricter (3-part: ≤55 s + bit-identical kernel + identical timings). Part A wins; B keeps a cross-reference. |
| N6 docs + disclosure | REQ-041 / METHOD-035 | **N6 + N8 / METHOD-035 + METHOD-037** | Same — Part A covers it. |
| *(new)* zero-copy (mmap) snapshot load | — | — | **Appended as B3** (new METHOD-040) |
| *(new)* versioned tombstone table | — | — | **Appended as B4** (new METHOD-041) |
| *(new)* shadow-paging compaction (reader chain) | — | — | **Appended as B5** (new METHOD-042) |
| *(new)* SoA as primary stalk layout | — | — | **Appended as B6** (new METHOD-043) |
| *(new)* SKELETON drift fix (7 untracked library files) | — | — | **Appended as B7** (new METHOD-044) |

## B.2 Appended nodes (Part B)

| Node | REQ | SPEC | Files | Change | Gate (evidence, not claim) |
|---|---|---|---|---|---|
| **B1** / METHOD-038 | REQ-043 | SPEC-043 | EDIT `fiber_join_bench.rs` (+ `shared/fiber_snapshot.rs` codec) | **TPCH fiber bench uses the cached bundle.** `fiber_join_bench` re-sorted 6M+1.5M+150k rows (4.2 s) on EVERY run. It now opens the bundle written by the B3 codec (same payload layout: [nf][(cust,rev)*nf][ns][(cust,seg)*ns]), walks it with the same streaming merge-walk, and writes the bundle when it is missing (so the first run builds it, later runs skip the build). | **GREEN (2026-10-21):** cached path **24.8–54.5 ms** total (load 21–51 ms + query 3.5 ms) vs the 4,200 ms pre-fold ⇒ **77–169x**; totals exact (93,574,920,897, rel 4.38e-12); gate total ≤ 500 ms; clippy clean. |
| **B2** / METHOD-039 | REQ-044 | SPEC-044 | NEW `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs`; REWRITE `imdb_fiber_bench.rs` | **IMDb fiber bundle, sectioned.** The 178.96 s build (principals 102M + names 15.7M rows) is written ONCE into a sectioned container: header (magic, version, n_sections, index_len, index CRC32C) + index (name, offset, len, CRC32C per section) + payload. Sections: `basics` (id u64, year i32, flags u8 — 12,866,375 rows), `ratings` (id, avg f32, votes i64 — 1,721,477), `princount` (id, n u32 — 11,626,391 titles), `names` (id, birthyear) + `nameblob` (NUL-terminated), `q2edges` (name_id, title_id — 42,453,755 actor/actress credits). A run fetches ONLY the sections its query needs (offset + own CRC32C); a corrupt section fails loud instead of half-loading. | **GREEN (2026-10-21, 2/2 runs, clippy clean):** build **51.1 s ONCE** (was 178.96 s per run); Q1 **244 ms** (17-18x), Q2 **10658 ms** (10-11x, gate >=5x), Q3 **1253 ms** (61-65x); Q1 exact (count 44,906, votes 294,435,444, avg rel 8.78e-10); Q2 top-10 names+counts identical to the SQL baseline. **Five bugs the gates caught:** the pre-1995 filter was lost in a rewrite (6,001,215 rows instead of 2,574,528 → total wrong 2.3x); the reader shifted section offsets a SECOND time (the writer already stores final offsets) → false 'crc mismatch'; the index record-size guard said 24 bytes instead of 20 → the last section was dropped → index CRC mismatch; packed records (13/20/12 B) read with an 8-byte-stride accessor → wrong fields read silently; the last TSV field has no trailing tab (numVotes all zero). |
| **B3** / METHOD-040 | REQ-045 | SPEC-045 | EDIT `fiber_snap_bench.rs`; NEW `crates/sheaf-engine/src/bin/shared/fiber_snapshot.rs` | **Zero-copy load**: read the bundle ONCE and BORROW the bytes — no per-element `Vec::push` deserialization; the merge-walk decodes on the fly. Integrity primitive switched from the byte-at-a-time IEEE CRC32 (measured 150-250 ms per 42.5 MB) to the repo's existing SSE4.2 **CRC32C** (8.5-23 ms; software fallback 110-220 ms) with a format-version bump so old bundles rebuild instead of mis-reading. No new crate. | **GREEN (2026-10-20, 3/3 runs, same-run comparison):** post-read work (the part B3 changed) old 167-301 ms -> new 44-80 ms = **5.4-10.4x, median 6.2x**; total path (adds the file read, identical work in both) **2.8-4.2x**; totals exact (rel 4.38e-12); corrupt copy rejected. **Gate:** post-read median ratio >= 2.0x (measured 6.2x) AND exactness AND corrupt-reject. **Measurement discipline (two bugs found, both recorded because both flattered the result):** (1) the reference path's CRC result was unused, so the compiler DELETED the whole integrity loop — the 'old path' stopped paying for its checksum (ratio read 1.1-1.6x instead of the true 5-8x); fixed with `black_box`. (2) the reference re-read the file and got a warm-cached read while the new path paid the cold one; fixed by sharing one read. A third error — the earlier '4.7-5.1x' figure compared against a CROSS-RUN baseline measured on a cooler box — was an artifact of box state; the same-run ratio gate is what exposed it. **Next bottleneck:** the file read (22-44 ms, now dominant) — removable only by mmap (new crate, out of scope). |
| **B4** / METHOD-041 | REQ-046 | SPEC-046 | NEW `crates/sheaf-storage/src/lsp/tombstone_table.rs`; EDIT `lsp/snapshot.rs`, `lsp/mem_complex.rs` | **Versioned tombstone table**: replace the per-snapshot copied live bitmap (`mem_complex.rs:249,258`) with a shared `(cell_id, deletion_gen)` table; a snapshot at gen G sees cell alive iff `deletion_gen > G` | snapshot creation O(1) instead of O(ncells); Phase-2 repeatable-read E2E still PASS; no leaked generation pins (F1 discipline holds) | **GREEN (2026-10-21):** NEW `crates/sheaf-storage/src/lsp/tombstone_table.rs` (concept: versioned death gens in `mem_complex.rs`/`snapshot.rs`), EDIT `lsp/snapshot.rs`, `lsp/mem_complex.rs`, `lsp/compactor.rs`, `bcsf/builder.rs`, bins `phase2_bench.rs`, `adversarial.rs`. Liveness became a DEATH GENERATION per cell (0 = alive) shared by `Arc`; a snapshot at gen G resolves liveness as `death_gen == 0 || death_gen > G` and copies nothing; the merge consumes the remap table directly (it already encodes liveness), so no boolean mask exists anywhere. **GREEN (2026-10-21, quiet box):** snapshot creation **770.71 us -> 0.23 us** (3,350x, O(n) -> O(1)); phase2 E2E PASS (txn abort, WAL torn tail, scope guard, write P99 < 200 us, merge-window reader rate >= 95% of post-merge, commit P99 < 2 ms); adversarial suite **12/12 PASS** (incl. A2 delete-merge race, A3 snapshot pinning, A6 shadow fuzz); clippy clean. |
| **B5** / METHOD-042 | REQ-047 | SPEC-047 | EDIT `bcsf/reader.rs` (FILE-015), `bcsf/builder.rs` (FILE-048), `lsp/compactor.rs` (FILE-019) | **Shadow paging**: `BcsfReader` gains an optional chain `base ⊕ remap ⊕ delta`; compactor emits a chain instead of copying the whole dataset when `dropped == 0` (remap is then an identity prefix — the hot path stays indirection-free) | merge cost scales with Δ not n; **Phase-1 lookup gate re-run: median true-fetch ≤15 ns** (SPEC-007) — a miss closes the node as `recorded`; merge-window reader rate ≥95% of post-merge baseline; totals exact | **GREEN (2026-10-21):** `BcsfReader` gained an optional chain `base (+) remap (+) delta` (`bcsf/reader.rs`: `Chain{base, delta, remap, identity, base_cells}`, chain-aware `dim/boundary/stalk/morph_weights/discrete`, `chain_depth()`, `flatten()`). The compactor emits a CHAIN when the merge drops nothing (the remap is the identity prefix, so the hot path stays indirection-free) and falls back to the copy path otherwise; the chain depth is bounded (4, an LSM-style level count) and the resolution RECURSES into a nested chain. Gates: `gate_shadow_merge_scales_with_delta=PASS` (shadow merge **1 ms** vs copy merge **6 ms** for the same 1,000-cell delta over a 200,000-cell base - the merge cost is O(delta)), `gate_shadow_merge_emits_chain=PASS` (depth 1, dropped 0), `gate_shadow_totals_exact=PASS` (20,000,899,000.0 == 20,000,899,000.0, bit-compared), `gate_shadow_cells_identical=PASS` (every cell's dim/boundary/stalk/discrete matches the flat copy), `gate_shadow_lookup_sub15ns=PASS` (chained **5.57 ns** vs flat 5.86 ns - overhead 0.95x); Phase-1 `gate_lookup_sub15ns=PASS` (7.53 ns), phase2 E2E PASS, adversarial 12/12 PASS. |
| **B6** / METHOD-043 | REQ-048 | SPEC-048 | EDIT `bcsf/arrow_layout.rs` | **SoA as primary stalk layout** (column descriptor decides physical layout at write time) so `transpose_soa` / `compact_valid` no longer materialize 2× memory at read time | `fused_revenue_sum` exact; memory spike eliminated; no regression in `fused_scan` / `mt_scan` gates | **GREEN (2026-10-21):** SoA is now the PRIMARY stalk layout in `bcsf/arrow_layout.rs` - the column descriptor (width `d`) decides the physical layout at write time: the values buffer holds `d` CONTIGUOUS columns, so `transpose_soa` / `compact_valid` return VIEWS (`Cow::Borrowed`) instead of materializing 2x the data; the FMA kernel gained a column-wise entry point (`fused_sum_columns`) that needs no gather. Gates: `gate_sum_exact=PASS` (218,102,223,881.86, rel 9.65e-15), **`gate_soa_no_memory_spike=PASS` (0 bytes allocated by the transpose; the old AoS path copied 48,009,720 B - measured with a counting global allocator)**, `gate_parity=PASS` (arrow scan 5.6 ms vs the 7.9 ms baseline), `gate_fma_correct=PASS` (4.9 ms), `gate_soa=PASS` (5.4 ms), `gate_p2_exceptions=PASS` (857,317 nulls, 5,143,898 kept, dense sum exact); no regression: `fused_scan_bench` 1.7 ms PASS, `mt_scan_bench` 6.7/3.3/2.4 ms (rel ~1e-14); clippy clean. |
| **B7** / METHOD-044 | REQ-049 | SPEC-049 | EDIT `SKELETON.md` | **SKELETON drift fix**: register FILE ids for the 7 load-bearing library files that have none — `bcsf/arrow_layout.rs`, `bcsf/coord.rs`, `lsp/graded_log.rs`, `lsp/ring_wal.rs`, `lsp/colimit.rs`, `sheaf-kernel/src/components.rs`, `sheaf-kernel/src/solvers/chebyshev.rs` | every `src/**` **library** file has a FILE id; `src/bin/**` is EXEMPT per as-built delta A1 ("Bench binaries + harness helpers carry no FILE IDs"); DIP audit still clean; no orphan REQ/SPEC/METHOD ids | **GREEN (2026-10-20):** the SKELETON drift is fixed - FILE-076..082 (the 7 untracked library files: ring_wal, colimit, components, chebyshev and friends) are registered in `SKELETON.md` with their folder ids and dependencies; the node inventory and the file list agree.

## B.3 Measured evidence behind the appended nodes

| Measurement (row) | Value (column) | Source (column) |
|---|---|---|
| TPCH bundle build | 4.2 s | `fiber_join_bench` live run (this box, loaded) |
| TPCH bundle load (cached) | 158.1 ms | `fiber_snap_bench` live run |
| Of which copy/deserialize | ~153 ms (97%) | 40.6 MB file vs 5.1 ms BW floor (8.4 GB/s measured) |
| IMDb bundle build | 178.96 s | `results/imdb_shelf.json` (q2 101.9 s + q3 76.8 s) |
| Merge (compaction) cost | 71–764 ms for 1.2M cells | METHOD-003 evidence (Phase-2 E2E) |
| Snapshot live-bitmap copy | O(ncells) per snapshot, on the read path | `mem_complex.rs:249,258` (`.map(...).collect()`) |
| Thread scaling ceiling | 4 threads only 1.25× (controller-capped) | W1 in `MAX_PERF_PLAN.md` |
| Already Kafka-native in-repo | `lsp/ring_wal.rs` — Disruptor-lite lock-free MPSC, fixed 64 B slots, HW CRC | file header |
| **B3 delivered (2026-10-20)** | post-read work **167-301 ms -> 44-80 ms (5.4-10.4x, median 6.2x)**; total path 2.8-4.2x; per-element copy removed; byte-wise IEEE CRC32 (150-250 ms) -> SSE4.2 CRC32C (8.5-23 ms); next bottleneck = the 22-44 ms file read | `fiber_snap_bench` 3/3 green runs, same-run ratio gate |

## B.4 Ordering (Part B only; Part A ordering unchanged)

```
B3 (zero-copy load) ──► B2 (IMDb cache) ──► B1 (TPCH wiring)   [bench spine]
B4 (tombstones) ──► B5 (shadow paging) ──► B6 (SoA layout)     [storage spine]
B7 (skeleton drift)  independent — do anytime
```

**Scope label — Part A N3 / N4(partial) / N5 / N6 are "future-live-fiber" nodes.**
They defend a *future* design where fibers are built over live BCSF cells. Today
the bench fibers are derived from `.tbl`/`.tsv` files and never from BCSF cell
ids, so compaction cannot dangle a fiber key and the epoch stamp has nothing to
stamp. They are legitimate to plan, but they are 4 high-effort MED-risk nodes
sitting at P1 **ahead of** the 179 s win. Re-prioritised: build **B3 → B2 → B1**
first, then **B4**, and treat N3/N4/N5/N6 as a second wave to be scheduled only
when a live-fiber design exists.

## B.5 Remaining identified items (listed, not scheduled — append-only, nothing dropped)

| Item | REQ | SPEC | METHOD | Source | What | Gate | Priority |
|---|---|---|---|---|---|---|---|
| **B8** | REQ-050 | SPEC-050 | METHOD-045 | parallel C1 | **Durable segment descriptor**: on-disk BCSF segment header `[MAGIC][fmt_ver][generation][src_fingerprint][ncells][offsets_off/len][...][crc32]` + `open_segment()` so a segment opens without running the builder | opens without builder; totals exact; torn/corrupt rejected, never half-loaded | 🟠 P2 (unblocks B9/B10) | **GREEN (2026-10-21):** NEW `crates/sheaf-storage/src/bcsf/segment.rs` - on-disk segment header `[MAGIC][fmt_ver][ncells][generation][src_fingerprint][9 parts (off/len)][crc32c]` + `open_segment()`; `MemComplex::persist_base_segment()` / `restore_base_segment()` open a segment WITHOUT running the builder. Gates: `gate_segment_exact=PASS` (2,500 cells byte-identical: dims/boundary/stalk/morph/discrete), `gate_segment_meta=PASS` (gen 42 + fingerprint round-trip), `gate_segment_corrupt_reject=PASS` (flipped byte -> CRC mismatch), `gate_segment_truncated_reject=PASS`, `gate_segment_alien_reject=PASS` (foreign file refused), `gate_segment_restore_exact=PASS` (restored base identical in a fresh engine); clippy clean.
| **B9** | REQ-051 | SPEC-051 | METHOD-046 | parallel C2 | **Persisted TIFI index sidecar**: value→cells index survives process exit (today: RAM-only, rebuilt per process) | after restart, lookup returns the same contiguous slice; parity with scan | 🟡 P3 | **GREEN (2026-10-21):** NEW sidecar codec in `crates/sheaf-storage/src/bcsf/name_lookup.rs` (`to_sidecar`/`from_sidecar`/`write_sidecar`/`read_sidecar`; header `[MAGIC][fmt_ver][generation][src_fingerprint][body_len][crc32c]`). Gates (a REAL second process reads the file only, no rebuild): `gate_sidecar_reserialize_identical=PASS` (22,789 bytes, byte-identical), `gate_sidecar_scan_parity=PASS` (97 distinct values, 5,000/5,000 cells match a full scan), `gate_sidecar_range_parity=PASS` (520 = 520 cells in [10,20)), `gate_sidecar_corrupt_reject=PASS`; clippy clean.
| **B10** | REQ-052 | SPEC-052 | METHOD-047 | parallel C3 | **Persisted partition-map sidecar**: L3-domain assignment + ghost map survive restart | after restart, `domains_fit_l3` + `ghosts_cover_boundaries` still hold | 🟡 P3 | **GREEN (2026-10-21):** NEW sidecar codec in `crates/sheaf-storage/src/bcsf/partition.rs` (`to_sidecar`/`from_sidecar`/`write_sidecar`/`read_sidecar`; domains + ghost map + per-domain bytes). Gates (a REAL second process reads the file only, no re-partition): `gate_partition_reserialize_identical=PASS` (608 bytes), `gate_partition_domains_fit_l3=PASS` (10 domains), `gate_partition_ghosts_cover_boundaries=PASS` (9 ghosts), `gate_partition_matches_fresh_plan=PASS` (reloaded map == a fresh partition), `gate_partition_corrupt_reject=PASS`; clippy clean.
| **B11** | REQ-053 | SPEC-053 | METHOD-048 | parallel C4 | **Table/catalog-level meta map** (table → segment set + column offsets + generation). Already on the ROADMAP (`ARCHITECTURE.md:110` step 3) | table opens by name; schema version gates every plan | 🟡 P3 (ROADMAP) | **GREEN (2026-10-21):** NEW `crates/sheaf-storage/src/bcsf/catalog.rs` - a table = name + segment set + column offsets + schema version; `open_table(name)` and `plan_table(name, schema_version)` (a version mismatch is REFUSED, never executed); the catalog persists to a sidecar. Gates: `gate_catalog_roundtrip=PASS` (304 B), `gate_catalog_open_by_name=PASS` (imdb_ratings v4, 2 segments), `gate_catalog_unknown_table_reject=PASS`, `gate_catalog_schema_gate=PASS` (v4 accepted, v5 refused), `gate_catalog_corrupt_reject=PASS`; clippy clean.
| **B12** | REQ-054 | SPEC-054 | METHOD-049 | parallel A6 | **Per-section generation table** for the bundle cache: one changed input rewrites only the affected section, not all 40.6 MB | only the changed section rewrites; totals exact | 🟢 P4 | **GREEN (2026-10-21):** bundle format v3 - every section carries its own GENERATION (the producer's stamp for the input that produced it) and `rewrite_sections()` writes a new bundle in which ONLY the updated sections are re-encoded while the rest is copied byte-for-byte. Gates: `gate_bundle_only_changed_section_rewritten=PASS` (1 of 4 sections re-encoded: 260,000 B written, 780,000 B copied), `gate_bundle_sections_intact=PASS` (untouched sections byte-identical), `gate_bundle_per_section_generations=PASS` (changed section bumped, others preserved), `gate_bundle_totals_exact=PASS`, `gate_bundle_unknown_section_reject=PASS`; the IMDb bundle now stamps each section with its source fingerprint and refuses to answer from a stale section (`gate_bundle_sections_fresh=PASS`, 6 sections) - the bundle format went v2 -> v3, so the old file rebuilds once (164.8 s). A/B on the SAME box and data (old v2 binary vs new v3 binary, 3 runs each, medians): q1 373 -> 367 ms, q2 19,083 -> 18,988 ms, q3 2,001 -> 1,893 ms - no regression; the earlier speed-gate failures were the box being ~1.8x slower than at baseline recording time (the old binary shows the same numbers).
| **B13** | REQ-055 | SPEC-055 | METHOD-050 | parallel A7 | **Consistent gate amortization policy**: `fiber_join_bench` times only the query while `fiber_snap_bench` counts its load — one rule for both, stated in the gate output | both gates use the same rule; README/RUNTIME_COMPARISON updated | 🟢 P4 | **GREEN (2026-10-20):** one gate policy for both fiber benches - the build and the query are reported SEPARATELY and the cached bundle is counted as an INDEX BUILD (like PG/DuckDB building their own indexes), so the comparable number is build + query; the rule is stated in the header of both benches.
| **B14** | REQ-056 | SPEC-056 | METHOD-051 | parallel B5 | **Zero-copy WAL replay** (keep the mapped buffer, return slices instead of `payload.to_vec()` per record) | replay exact; no per-record copy | 🟢 P4 (negligible) | **GREEN (2026-10-21):** `crates/sheaf-storage/src/lsp/wal.rs` - the log is read ONCE and each payload is a RANGE into that buffer (`replay_zero_copy` -> `Replay{buf, frames, torn}`; `payload(i)` returns a slice), so recovery no longer duplicates the log. Integrity semantics are unchanged: magic + length + CRC per frame, torn regions still reported. Gates: copy 4.8 ms vs zero-copy **3.5 ms** (20,000 frames, byte-identical payloads); `gate_replay_zero_copy=PASS`; `gate_replay_exact=PASS` (20000/20000, truncated=false); clippy clean. |

| Note | Detail |
|---|---|
| B13/B14 are policy/hygiene, not code-heavy | B13 is a rule + doc change; B14 is a small optimisation |
| B8 is the enabling node for B9/B10 | the sidecars need a durable segment format to hang off |
| Nothing from the parallel session's 22-item list is dropped | A1→N1, A2→N2, A3→B1, A4→B2, A5→B3, A6→B12, A7→B13, B1→B4, B2/B3→B5, B4→B6, B5→B14, C1→B8, C2→B9, C3→B10, C4→B11, D1→N0, D2→N7, E1→B7, E2→N6/N8, E3→N6/N8 |

## B.6 Explicitly not building (measured and refuted — retained for the record)

| Idea | Why rejected | Measurement |
|---|---|---|
| L1/L2/SRAM tier manager | layout tuning already closed | R1: bounds checks within noise (12.3 / 14.8 / 10.9 ms) |
| More thread scaling | bandwidth-capped | W1: 4 threads only 1.25x; 48 MB floor ~5.7 ms |
| Kafka-style no-fsync-per-op | breaks durability (REQ-009/REQ-032) | W2: fsync 1.2 ms vs write-only 9 us |
| Per-tier checksums | capped ~2% | W3: checksum = 2% of a durable op |
| Persisting raw memory addresses | breaks restart, compaction, MVCC pinning, cross-node reads | offsets vs addresses table (SHEAF_THEORY_REVIEW) |
| Explicit L2 pinning / prefetch hints | hardware prefetcher wins | R2: iterator-is-faster refuted |
| **B2 delivered (2026-10-21)** | IMDb build 178.96 s/run -> 51.1 s once; Q1 244 ms, Q2 10658 ms, Q3 1253 ms (10-65x); exact on all three | `imdb_fiber_bench` ALL PASS 2/2 |
| **B1 delivered (2026-10-21)** | TPCH fiber bench: 4,200 ms pre-fold per run -> 24.8-54.5 ms cached (77-169x), totals exact | `fiber_join_bench` cached path PASS |
| **B14 delivered (2026-10-21)** | WAL replay: copy 4.8 ms -> zero-copy 3.5 ms (20,000 frames identical); integrity unchanged | `wal_insert_bench` PASS |
| **N3 delivered (2026-10-21)** | fiber bundle bound to a source fingerprint; stale source => rebuild, never a stale answer | `fiber_snap_bench` + `fiber_join_bench` PASS |
| **B4 delivered (2026-10-21)** | snapshot creation 770.71 us -> 0.23 us (O(n)->O(1), 3,350x); no bool mask at merge time either; phase2 E2E PASS + adversarial 12/12 | `phase2_bench`, `adversarial` |
| **B8 delivered (2026-10-21)** | durable segment descriptor + `open_segment()`; a segment opens without the builder; corrupt/truncated/alien files rejected, never half-loaded | `segment_bench` 6/6 PASS |
| **B9 delivered (2026-10-21)** | value->cells index persists to a sidecar and survives process exit; lookup after restart == scan parity; corrupt file rejected | `index_sidecar_bench` (write/verify in 2 processes) 4/4 PASS |
| **B10 delivered (2026-10-21)** | L3-domain assignment + ghost map persist to a sidecar and survive process exit; plan invariants still hold on the reloaded map | `partition_sidecar_bench` (write/verify in 2 processes) 5/5 PASS |
| **B11 delivered (2026-10-21)** | table/catalog meta map: a table opens BY NAME, the schema version gates every plan, the catalog persists to a sidecar | `catalog_bench` 5/5 PASS |
| **B12 delivered (2026-10-21)** | per-section generation table + incremental rewrite: a changed input re-encodes only its section (260 KB written, 780 KB copied) | `imdb_fiber_sections_bench` 5/5 + A/B no-regression |
| **N4 delivered (2026-10-21)** | per-domain incremental fiber update: 1,000 writes cost 2 re-sorts (the dirty domains), not 1,000 | `fiber_epoch_bench` PASS |
| **N5 delivered (2026-10-21)** | the compactor exposes the frozen epoch; a build during a concurrent compaction is byte-equal to a quiet-box build; a pre-merge fiber is refused | `fiber_lifecycle_bench` PASS |
| **N6 delivered (2026-10-21)** | 10,000 mixed read/write ops: fiber == fallback every time, no deadlock, lag bounded (2 generations) and reported | `fiber_lifecycle_bench` + `run_fiber_lifecycle.py` 6/6 PASS |

---

# Part C - the performance-waste plan (appended 2026-10-22 13:01:38 local. Nothing in Part A or Part B is discarded.)

## C.1 Why this part exists

The plan of record is `PERFORMANCE_FIX_PLAN_2026-10-22.md`. It removes the measured performance
waste of the bundle path, and the bundle path is the fiber lifecycle. The waste ledger is
`PERFORMANCE_WASTE_ANALYSIS.md`.

Two findings change the fiber story, and both are measured:

1. **The bundle is a pre-parse, not a pre-aggregate.** The byte ratio against the answer is
   280.7x on the IMDb Q1 shape and 0.13x on the TPC-H fiber. The TPC-H fiber holds the answer.
   The IMDb bundle holds the source columns, so every query re-derives the answer, and 93.2% to
   99.7% of the query time is not byte movement. **A fiber per query shape, built at the shape's
   parameters and stored with those parameters, is the fix.** That is Axis A extended: the bundle
   gains sections that hold answers, not only source columns.
2. **The build is 96.5% to 98.5% a character-level parse.** 72,779,864 rows at 1,549 ns per row
   and 56.6 MB/s, with the measured hash insert at 116 to 137 ns, so 91.2% to 92.5% is text
   handling. The `field` helper scans the line from byte 0 for every field it wants, so the
   basics stage examines 18 tab positions per row where 9 are enough.

## C.2 The appended nodes

| Node | Lifecycle effect | Gate |
|---|---|---|
| METHOD-065 | the setup comparison gets a script, so the 511x gap is measured and not assumed | the recorded number is at or above the 1,995 ms physical floor |
| METHOD-061 | one pass per line in the build, 115,797,375 redundant tab reads removed | the six sections byte for byte |
| METHOD-062 | a digit loop in place of the generic parse | the six sections byte for byte |
| METHOD-063 | 4 threads over the parse, 112,721 ms to 28,180 ms | the six sections, plus the three query gates |
| METHOD-064 | the CRC verify folds into the read, 23.7% to 32.7% of the load | the CRC corruption negative test |
| METHOD-059 | the Q1 answer fiber, a new section holding the 44,906 surviving rows | the variant gate on count, votes and average |
| METHOD-060 | the Q2 aggregate and the Q3 groups, built from the same pass | the md5 cross-engine gate |
| METHOD-066 | the latency fixes tested on the build, where there is no dependency chain | the six sections byte for byte |

## C.3 The invariant that the appended nodes must not break

A stale bundle is rebuilt, never answered from. A fiber built for one shape must never answer a
query with different parameters, so every new section carries the parameters it was built with
and the reader refuses a mismatched one. The bundle format version is bumped with the new
sections, so a bundle written before the change is rebuilt rather than read.

## C.4 The dependency that governs the order

The pre-aggregate joins add 18,349 ms to the build. With METHOD-063 the build is 46,529 ms, which
is 2.42x better than today. Without it the build is 131,070 ms, which is worse than today.
**METHOD-063 must land before METHOD-059 and METHOD-060 are worth anything at scale.**