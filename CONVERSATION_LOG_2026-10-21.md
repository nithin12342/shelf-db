# CONVERSATION_LOG_2026-10-21.md — session record: comparison repair, NoSQL challenger, fiber-bundle cache

| # | Event | Evidence | Outcome |
|---|---|---|---|
| 1 | Full 8-engine matrix run (run_all.py) | `results/RUN_SUMMARY_20261020_2116.json` | 8/8 engines OK, 10/11 shelf bins OK, 1 FAIL (fiber_snap) |
| 2 | Neo4j traversal showed 8.6 ms with paths 0 | `results/neo4j.json` run9 (counts 0/0/0) | **INVALID** — my IMDb bench had wiped the graph (`MATCH (n) DETACH DELETE n`) |
| 3 | Neo4j re-import + re-bench | 2.25M nodes / 2.1M rels, traversal 1,398.5 ms, paths 600,000 | ✅ valid number restored |
| 4 | Root cause of (2) fixed | `imdb_multi_engine_bench.py` delete now scoped to `Title`/`Person` labels | future runs cannot wipe the bench graph |
| 5 | B3 (zero-copy bundle load) built | `shared/fiber_snapshot.rs` | per-element copy removed; CRC32C instead of byte-wise IEEE CRC32 |
| 6 | B3 gate had 3 measurement bugs | see below | gate rewritten to a same-run ratio |
| 7 | MongoDB uninstalled | `scoop uninstall mongodb` + data dir deleted | ScyllaDB takes the NoSQL slot |
| 8 | ScyllaDB IMDb case built + run | `imdb_scylla_prep.rs` (Rust) + `imdb_scylla_bench.py` (CQL transport) | query **90 ms**, exact PASS, **1,991x** faster than MongoDB's 183,618.6 ms |
| 9 | Cross-dataset comparison error found | MongoDB 29.7 ms was the synthetic corpus; IMDb is 183,618.6 ms | table corrected; rule: every row names its dataset |
| 10 | B2 (IMDb sectioned bundle) built | `shared/imdb_fiber.rs` + `imdb_fiber_bench.rs` | 178.96 s build → 51.1 s once; Q1 240 ms, Q2 ~10 s, Q3 ~1.2 s; exact |
| 11 | B1 (TPCH bench wired to the cached bundle) | `fiber_join_bench.rs` cached path | 4,200 ms pre-fold → 24.8–54.5 ms (77–169x), totals exact |
| 12 | B13 gate policy + N8 model row | headers in both fiber benches; `COMPLEXITY_ANALYSIS.md` | build/query always separate; compile-time axis 109.3 s recorded; A9 closed |

## Adversarial findings (all found by gates, all recorded)

| Finding | How it surfaced | Correction |
|---|---|---|
| Reference path's CRC was **dead-code eliminated** (result unused) | ratio read 1.1–1.6x, implausible vs the earlier 158 ms load | `black_box` the CRC result; true ratio 5.4–10.4x |
| Reference re-read the file (warm cache) vs the new path (cold) | ratio swung 0.9x–1.9x between runs | share ONE read between both paths |
| Cross-run baseline (cooler box) inflated the win to "4.7–5.1x" | same-run ratio disagreed | same-run median-of-3 ratio gate; the 4.7x claim retracted |
| Pre-1995 row filter lost in a rewrite | bundle had 6,001,215 fibers instead of 2,574,528; total wrong by 2.3x | filter restored; exactness gate caught it |
| The 42.5 MB buffer **clone** was inside the timed region | verify 24.2 ms vs raw CRC 8.5 ms | clone removed; `Bundle::as_bytes` borrow |
| Last TSV field has **no trailing tab** | all `numVotes` came out 0 (Scylla case) | parse the remainder after the last separator |
| Packed records (13/20/12 B) read with an 8-byte-stride accessor | would read wrong fields silently | per-record-type accessors |
| Neo4j benchmark wiped by another bench | traversal 8.6 ms, paths 0 | label-scoped delete |
| CQL rejects range predicates on the partition key | "Only EQ and IN relation are supported" | single partition + clustering on `avg` |

## Measured results this session (B1 done, B2 done, B3 done)

| Case | Result | Gate |
|---|---|---|
| ScyllaDB IMDb Q1 | 90 ms query, 24.8 s insert, 3.3 s ETL, exact 44,906 / 294,435,444 / 8.145506 | ✅ PASS (1,991x vs MongoDB) |
| B3 bundle load | post-read work 167–301 ms → 44–80 ms (5.4–10.4x); total path 2.8–4.2x | ✅ PASS 3/3 |
| B2 IMDb bundle | sectioned, per-section CRC32C, demand-loaded | see results/imdb_shelf.json |

## Decision recorded

| Decision | Value |
|---|---|
| Deployment model | **In-process only** (no client/server) — the ns-class wins (10.5 ns) die over a socket |
| Scale-out | Partition + shard into multiple in-process copies (Phase-6/7 cover, ghosts, CRDT, zero-hop routing) |
| NoSQL challenger | ScyllaDB (CQL), MongoDB uninstalled |
| Rust-first | Heavy work in Rust; Python only for CQL transport (no Rust driver without a new crate) |

## Wave 2 — the remaining plan nodes (B4, B5, B6, B8–B12, B14, N3–N6)

All 13 previously-open nodes were implemented, gated and committed in this wave.
Rows = node, Columns = what changed, the measured evidence, and the gate verdict.

| Node | What changed | Measured evidence | Gate |
|---|---|---|---|
| B14 | Zero-copy WAL replay (`lsp/wal.rs`): the log is read once, payloads are RANGES into that buffer (`replay_zero_copy` -> `Replay{buf, frames, torn}`) | copy 4.8 ms -> zero-copy **3.5 ms**, 20,000 frames byte-identical | `gate_replay_zero_copy=PASS`, `gate_replay_exact=PASS` |
| N3 | Epoch stamp: `lsp/fiber_epoch.rs` (`Epoch`, `validate`, `decide`, `base_fingerprint`) + the fiber bundle's source fingerprint in its header (v3) | mismatched epoch -> refused; the scan fallback returns the same aggregate (99,900,000 == 99,900,000); a touched source forces a rebuild | `gate_epoch_mismatch_fails_loud=PASS`, `gate_epoch_fallback_aggregate_exact=PASS`, `gate_snap_source_fresh=PASS` |
| B4 | Versioned tombstones: every cell carries a DEATH GENERATION (0 = alive) shared by `Arc`; a snapshot resolves liveness as `death_gen == 0 \|\| death_gen > snap_gen` and copies nothing; the merge consumes the remap table directly | snapshot creation **770.71 us -> 0.23 us** (3,350x, O(n) -> O(1)); no bool mask at merge time | phase2 E2E PASS, adversarial 12/12 PASS |
| B8 | Durable segment descriptor (`bcsf/segment.rs`): on-disk header + `open_segment()`; `MemComplex::persist_base_segment` / `restore_base_segment` | 2,500 cells round-trip byte-identical; gen + fingerprint round-trip; corrupt/truncated/alien files rejected; a restored base is identical in a fresh engine | `segment_bench` 6/6 PASS |
| B9 | Persisted TIFI sidecar (`bcsf/name_lookup.rs`): the value->cells index serializes and reloads | a SECOND process reloads the file only and re-serializes it byte-identically (22,789 B); 97 values / 5,000 cells match a full scan; range parity 520 = 520 | `index_sidecar_bench` 4/4 PASS |
| B10 | Persisted partition-map sidecar (`bcsf/partition.rs`): L3 domains + ghost map | a second process reloads it; `domains_fit_l3` PASS (10 domains), `ghosts_cover_boundaries` PASS (9 ghosts), reloaded map == a fresh partition | `partition_sidecar_bench` 5/5 PASS |
| B11 | Table/catalog meta map (`bcsf/catalog.rs`): open a table BY NAME; the schema version gates every plan; the catalog persists | open by name PASS; a v5 plan against a v4 table is REFUSED; unknown table rejected | `catalog_bench` 5/5 PASS |
| B12 | Bundle format v3: every section carries its own GENERATION; `rewrite_sections()` re-encodes only the changed section | 1 of 4 sections rewritten (260,000 B written, 780,000 B copied); untouched sections byte-identical; the IMDb bundle stamps each section with its source fingerprint | `imdb_fiber_sections_bench` 5/5 PASS + A/B no regression |
| B5 | Shadow paging: `BcsfReader` gained an optional chain `base (+) remap (+) delta`; the compactor emits a chain when the merge drops nothing (identity remap, depth bounded at 4) | shadow merge **1 ms** vs copy merge **6 ms** for the same 1,000-cell delta over a 200,000-cell base; chained lookup **5.57 ns** vs flat 5.86 ns (overhead 0.95x) | `shadow_paging_bench` 6/6 PASS, phase1 7.53 ns PASS, phase2 PASS, adversarial 12/12 |
| B6 | SoA is the PRIMARY stalk layout (`bcsf/arrow_layout.rs`): the values buffer holds `d` contiguous columns, so `transpose_soa` / `compact_valid` return VIEWS; the kernel gained a column-wise entry point | the transpose allocates **0 bytes** (the old AoS path copied 48,009,720 B, measured with a counting allocator); the scan improved 7.9 -> 5.6 ms; `fused_revenue_sum` exact (rel 9.65e-15) | `arrow_scan_bench` PASS, `fused_scan_bench` 1.7 ms PASS, `mt_scan_bench` no regression |
| N4 | Per-domain incremental fiber update (`bcsf/fiber_domain.rs`): the fiber is split into ~64 MB domains; only dirty domains re-sort | 1,000 writes over 4 domains dirty 2 of them -> **2 re-sorts**, not 1,000; post-write answers + aggregate exact | `fiber_epoch_bench` PASS |
| N5 | The compactor EXPOSES THE FROZEN EPOCH of each merge (`MergeStats.frozen_epoch`) | a build during a concurrent compaction is byte-equal to a quiet-box build (199,990,000.0 == 199,990,000.0, bit-compared); a pre-merge fiber is refused (gen 1 vs gen 2) | `fiber_lifecycle_bench` PASS |
| N6 | Fiber lifecycle gate: `fiber_lifecycle_bench.rs` + `tests/e2e/fiber_lifecycle/` | 10,000 mixed ops (6,666 reads + 3,334 writes): **0** fiber-vs-fallback mismatches, no deadlock, max lag 2 generations | `fiber_lifecycle_bench` + runner 6/6 PASS |

### Adversarial findings in wave 2 (all found by gates, all corrected)

| Finding | How it surfaced | Correction |
|---|---|---|
| The builder recorded the REMAP index as the morph-cell index instead of its own | "merged base invalid: morph cell out of range" (only visible when the remap is not the identity) | the builder now records its own cell index |
| The chain resolution did not recurse into a nested chain (the base's slabs are empty when the base is itself a chain) | panic "index out of bounds: the len is 0" in the adversarial A2 delete-merge race | `resolve()` recurses until it reaches a flat reader |
| A delta is a FRAGMENT: its boundary ids point at BASE cells, so it is not a valid standalone reader | "merged base invalid: dangling boundary id" | `validate_fragment(max_id)` + `finish_fragment()` |
| Joining the compactor worker while its sender was alive waited forever | the lifecycle bench hung with 0 CPU | drop the sender before joining (documented in the bench) |
| The N3 header grew 24 -> 32 bytes, but the bench hardcoded the payload offset 24 | "capacity overflow" in the reference path (it read a garbage fiber count) | the payload offset now uses `fiber_snapshot::HEADER_LEN` |
| The IMDb speed gates flapped on this box | q1 406-460 ms vs a 420 ms threshold | A/B on the SAME box and data: the pre-change binary gives q1 373 ms / q2 19,083 ms / q3 2,001 ms, the new one 367 / 18,988 / 1,893 -> no regression; the box is ~1.8x slower than at baseline recording time |
| A stale container (java/neo4j) was still running and made every measurement noisy | 50% CPU on the box, timings 1.8x off | the stray process was killed; the measurements were repeated |

### Wave-2 regression status

| Suite | Result |
|---|---|
| shelf bins (`run_all.py` list) | **11/11 PASS** |
| phase2 E2E (MVCC) | PASS |
| adversarial suite | 12/12 PASS |
| phase1 lookup gate | 7.53 ns (budget 15 ns) PASS |
| `fused_scan_bench` / `mt_scan_bench` | PASS, no regression |
