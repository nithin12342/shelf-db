# Code change map — every change → every file (committed, pending, planned)

| # | Commit | Files | Lines | Purpose | State |
|---|---|---|---|---|---|
| 1 | `b79d89a` | `tests/e2e/realtime_bench/run_all.py` (NEW) | ~200 | One-command matrix: servers, sequential benches, runN merge, summary | Local only |
| 2 | `82bc053` | `crates/sheaf-engine/src/bin/fused_scan_bench.rs` (NEW) | ~90 | Overhaul A: fused revenue column scan | Local only |
| 3 | `fe53e6f` | `run_all.py` (edit) | +45/−24 | SHELF_BINS+=fused, docker ready_wait, fail output | Local only |
| 4 | `8a56243` | `crates/sheaf-engine/src/bin/fiber_snap_bench.rs` (NEW) | ~230 | Overhaul C: fiber snapshot persist + load | Local only |
| 5 | `6f4a466` | `crates/sheaf-storage/src/bcsf/name_lookup.rs` (edit) | +21 | Overhaul D: `NameIndex::range` interval API (BE bounds) | Local only |
| 5 | `6f4a466` | `crates/sheaf-engine/src/bin/range_slice_bench.rs` (NEW) | 81 | Overhaul D: 10k-slice bench vs Scylla | Local only |
| 5 | `6f4a466` | `run_all.py` (edit) | +3/−1 | SHELF_BINS+=snapshot+range bins | Local only |
| 6 | `298edd8` | `tests/e2e/realtime_bench/mongo_bench.py` (edit) | +20/−5 | Truthful unindexed/indexed pair (index-drop fix) | Local only |
| 6 | `298edd8` | `results/{8 engines}.json` + `RUN_SUMMARY_*.json` (edits) | +642/−12 | run4/run5 histories, repairs, retraction notes | Local only |
| 7 | `35d2173` + `019b0b4` | laplacian/tikhonov + `fused_scan_bench.rs` | hoist/scale/diag/cache + 4-way unroll (1.7 ms) | Committed |
| 8 | `0d863b2` + `88fa66f` | chebyshev/components/energy/p0 + `fiber_snap_bench.rs` crc | P0 triple gate + snapshot crc framing | Committed |
| 9 | `471741a` | `p0_gates_bench.rs` stress fixtures | d8 exact, giant 2000-face, ill-conditioned converge | Committed |
| 10 | (docs) | `COMPLEXITY_ANALYSIS.md` (NEW) + map sync | 15 formulas vs wall, 10 probes, unroll update | Uncommitted |

## Uncommitted (working tree)

| File | Type | Action needed |
|---|---|---|
| `AGENTS.md` (NEW) | Response-format rule | Commit if wanted |
| `SHEAF_THEORY_REVIEW.md` (NEW) | Theory efficiency review + related-theory appendix | Commit if wanted |
| `VON_NEUMANN_MAPPING.md` (NEW) | Math→machine translation | Commit if wanted |
| `CODE_CHANGE_MAP.md` (NEW, this file) | Change inventory | Commit if wanted |
| `facebook_combined.txt.gz`, `ieee-fraud-detection.zip` | Data archives | Leave untracked (data, not code) |
| `D:\bench\data\fiber_snapshot.bin` | Build artifact (outside repo) | Keep on disk (speeds reruns) |

## Planned (not started)

| Item | Target file(s) | Source |
|---|---|---|
| Hoist Mᵀδ in apply loop | `sheaf-kernel/.../operators/laplacian.rs` | Review §constants |
| Scale-not-recompute MᵀM in build | same | Review §constants |
| Cache M⁻¹ across same-λ solves | same + `solvers/tikhonov.rs` | Review §constants |
| P0 Chebyshev Tikhonov filter | new bin + `solvers/` | Review part 2 |
| P0 H¹ curl residual metric | `solvers/hodge.rs` or `operators/energy.rs` | Review part 2 |
| P0 lumped diagonal Hodge-⋆ | `operators/` | Review part 2 |
| P1 Morse shrink, per-stalk QR, projection sparsifier | new modules | Review part 2 |
| Runner shakedown (docker E2E, --fresh/--engines/--keep-up) | `run_all.py` | Session notes |
| Push 6 local commits | remote `personal/main` | Blocked: window + no auth |

## Divergence

```
local main    : 6 commits ahead (b79d89a..298edd8)
remote        : untouched since revert 03aaec0 (0 pushed this session section)
reverted pair : 8684aa2 (bad rematch) / 03aaec0 (revert) — history only
```

## Session 2026-10-22 (analysis, benches, and the fix plan)

| # | Commit | Files | Lines | Purpose | State |
|---|---|---|---|---|---|
| 19 | `b582cc1` | `SEVEN_DATABASE_AREA_COMPARISON.md` (NEW) + 4 docs (edits) + `results/imdb_shelf_runs.json` (NEW) + 3 tools (edits) | about 900 | The area-by-area comparison, the four-run archive, and the tool provenance | Committed |
| 20 | `1c2623d` | `CODE_CHANGES_AND_EXPECTED_GAINS.md` (NEW) + `bundle_load_bench.rs` (NEW) + `q1_kernel_bench.rs` (NEW) + 4 docs (edits) | about 700 | The code changes C1 to C6 with the measured gains, and the two benches that measured them | Committed |
| 21 | `af79df7` | `PERFORMANCE_WASTE_ANALYSIS.md` (NEW) + `SKELETON.md` (edit) | about 230 | Where the performance goes, for both comparison tables | Committed |
| 22 | this commit | `PERFORMANCE_FIX_PLAN_2026-10-22.md` (NEW) + `CONVERSATION_LOG_2026-10-22.md` (NEW) + 8 docs (edits) | about 800 | The fix plan METHOD-059 to METHOD-066, and the session record | Committed |

## Planned, from `PERFORMANCE_FIX_PLAN_2026-10-22.md`

| Node | Target file(s) | Lines | Source | Gate |
|---|---|---|---|---|
| METHOD-065, re-measure the opponent setup | a new runner under `tests/e2e/realtime_bench/` | about 200 | The 0.1 s DuckDB number is below the 1,995 ms physical floor | the recorded number is at or above the floor |
| METHOD-061, one pass per line | `crates/sheaf-engine/src/bin/imdb_fiber_bench.rs` | about 40 | The `field` helper rescans the line for every field | the six sections byte for byte |
| METHOD-062, a digit loop | the same file | about 30 | The generic `str::parse` and the UTF-8 validation | the six sections byte for byte |
| METHOD-063, 4 threads over the parse | the same file | about 120 | The parse is 96.5% to 98.5% of the build | the six sections, plus the three query gates |
| METHOD-064, the fused CRC verify | `crates/sheaf-engine/src/bin/shared/imdb_fiber.rs` | about 20 | The verify is 23.7% to 32.7% of the load | the CRC corruption negative test |
| METHOD-059, the Q1 answer fiber | the same two files | about 55 | 93.2% to 93.9% of the Q1 query is a join | the variant gate on count, votes and average |
| METHOD-060, the Q2 and Q3 pre-aggregates | the same two files | about 90 | 99.7% of the Q2 query is a join | the md5 cross-engine gate |
| METHOD-066, the latency fixes on the build | the same file | about 40 | The same patterns measured no gain on the query | the six sections byte for byte |