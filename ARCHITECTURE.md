# ARCHITECTURE + FILE DESIGN (Architecture → File Design gates)

DIP layers: `domain/` (pure math incl. slice arithmetic, depends on nothing) → `application/` (orchestration incl. operators/solvers, depends on domain traits) → `infrastructure/` (BCSF/WAL/executor impls) → `interfaces/` (TQL text, .section, client API).
Rule: import arrows point inward only (enforced by tests/dip_audit.sh — crate edges + folder arrows; violation = return to Planning).

## Folder tree (Phase 1 output; no file contents yet)

```
sheaf-db/
  Cargo.toml (workspace: sheaf-core, sheaf-storage, sheaf-kernel, sheaf-query, sheaf-engine — sheaf-distrib/sheaf-net deferred, see as-built delta A4)
  crates/sheaf-core/src/poset/          FOLDER-001  domain/TopologyCore
  crates/sheaf-core/src/stalk/          FOLDER-002  domain/TopologyCore
  crates/sheaf-core/src/morphism/       FOLDER-003  domain/TopologyCore
  crates/sheaf-storage/src/bcsf/         FOLDER-004  infrastructure/StorageEngine
  crates/sheaf-storage/src/lsp/          FOLDER-005  infrastructure/StorageEngine
  crates/sheaf-storage/src/index/        FOLDER-006  infrastructure/StorageEngine
  crates/sheaf-kernel/src/simd/          FOLDER-007  domain/SheafKernel (pure u64/f32 slice arithmetic, no storage/OS)
  crates/sheaf-kernel/src/operators/     FOLDER-008  application/SheafKernel (trait orchestration over domain primitives)
  crates/sheaf-kernel/src/solvers/       FOLDER-009  application/SheafKernel
  crates/sheaf-query/src/syntax/         FOLDER-010  interfaces+TQLCompiler
  crates/sheaf-query/src/ir/             FOLDER-011  application/TQLCompiler (TIR + validator + optimizer consolidated; FOLDER-012 merged here, see as-built delta A3)
  ~~crates/sheaf-engine/src/catalog/~~       FOLDER-013  DEFERRED — schema registry never built; TQL DEFINE statements serve as catalog (see as-built delta A5)
  ~~crates/sheaf-engine/src/executor/~~      FOLDER-014  DEFERRED — bench binaries serve as executor; api/result.rs serves as output (see as-built delta A5)
  crates/sheaf-engine/src/transaction/   FOLDER-015  application/EngineRuntime
  crates/sheaf-engine/src/api/           FOLDER-016  interfaces/EngineRuntime
  crates/sheaf-engine/src/corpus.rs      (bench-corpus helper, no FOLDER; E2E harness, see as-built delta A1)
  ~~crates/sheaf-distrib/src/~~               FOLDER-017  DEFERRED — implemented at storage level as bcsf/cover.rs + bcsf/partition.rs (see as-built delta A4)
  ~~crates/sheaf-net/src/~~                     FOLDER-018  DEFERRED — loopback proves protocol; no separate crate (see as-built delta A4)
  tests/e2e/phase1_bcsf/  phase2_lsp/  phase3_kernel/  phase4_tql/  phase5_opt/  phase6_split/  phase7_dist/
```

Each `tests/e2e/phaseN_<ctx>/` holds `fixtures/` (real inputs) + `expected/` (artifact next phase reads) + idempotent `run.sh` invoking `cargo run --bin <interface>`.

## File responsibilities + must-never (Phase 2 output; SRP: all statements unique)

| FILE | Responsibility (≤7 words) | Must never |
|---|---|---|
| poset/id.rs | encode CellId dimension tagged identifier | parse queries or touch floats |
| poset/cell.rs | define graded cell descriptors | store stalks or matrices |
| poset/incidence.rs | store codim-1 boundaries virtualize composites | materialize full simplex closure |
| poset/ordering.rs | enforce poset order acyclicity | execute linear algebra |
| stalk/discrete.rs | store exact discrete attribute payloads | do float matvec |
| stalk/continuous.rs | store aligned continuous embedding vectors | do string matching |
| stalk/aligned_buf.rs | allocate 64B aligned float buffers | define schema semantics |
| stalk/bundle.rs | unify discrete plus continuous handle | persist or compact |
| morphism/projection.rs | evaluate discrete projection masks | multiply float matrices |
| morphism/matrix.rs | store restriction weight matrices | filter strings |
| morphism/compound.rs | combine pi plus W morphism | own storage layout |
| bcsf/offsets.rs | index cell to incidence slice | mutate on write path |
| bcsf/indices.rs | store sorted boundary identifiers | hold floats |
| bcsf/arena.rs | pack stalks plus morphism weights | parse or plan |
| bcsf/reader.rs | serve zero-copy snapshot views | write or lock |
| bcsf/builder.rs | assemble immutable BCSF segments | serve reads or compact |
| bcsf/name_lookup.rs | find cells by discrete value | scan full BCSF |
| bcsf/partition.rs | split hypergraph L3 domains | solve linear systems |
| bcsf/cover.rs | assign hyperedges to domains | cut hyperedges by hash |
| lsp/mem_complex.rs | append lock-free live poset | serve frozen reads |
| lsp/wal.rs | log mutations with CRC | execute queries |
| lsp/snapshot.rs | isolate MVCC read snapshots | compact segments |
| lsp/compactor.rs | merge MemComplex into BCSF | accept client traffic |
| lsp/preagg.rs | maintain O(1) group accumulators | serve ad-hoc aggregates |
| lsp/epoch_wal.rs | epoch-validated append-only headers | rewrite headers in place |
| ~~index/tifi.rs~~ | ~~map attribute values to cells~~ | ~~scan full BCSF~~ |
| ~~index/partition.rs~~ | ~~partition hypergraph NUMA domains~~ | ~~solve linear systems~~ |
| simd/intersection.rs | intersect u64 slices via SIMD | know TQL syntax |
| simd/mt_scan.rs | partitioned multi-threaded scan reduce | allocate per scan |
| simd/matvec.rs | multiply W times stalk vectors | touch discrete masks |
| operators/coboundary.rs | compute delta residuals per cell | own solver iteration |
| operators/laplacian.rs | assemble sparse sheaf Laplacian | parse queries |
| operators/energy.rs | score Dirichlet energy coherence | emit client output |
| solvers/cg.rs | solve preconditioned conjugate gradient | define schema |
| solvers/tikhonov.rs | regularize Laplacian linear solves | manage transactions |
| solvers/harmonic.rs | drive harmonic section relaxation | tokenize text |
| syntax/lexer.rs | tokenize TQL topological keywords | typecheck dims |
| syntax/parser.rs | parse TQL into AST | execute vectors |
| syntax/expr_parser.rs | parse WHERE expressions | touch DDL syntax |
| syntax/define_parser.rs | parse DEFINE statements | touch query syntax |
| syntax/ast.rs | represent match restrict emit | optimize plans |
| ir/tir.rs | lower AST to TIR graph | run SIMD |
| ir/validator.rs | reject dim cycles mismatches | choose plan cost |
| ir/optimizer.rs | prune TIR via poset structure | invent new opcodes (tripwire: a 13th opcode requires a SPEC-018 amendment + METHOD node, never a silent addition) |
| ~~catalog/schema.rs~~ | ~~register cell type schemas~~ | ~~execute pipelines~~ |
| ~~catalog/index_meta.rs~~ | ~~track segment offset metadata~~ | ~~serve clients~~ |
| ~~executor/physical_plan.rs~~ | ~~represent executable plan steps~~ | ~~run threads~~ |
| ~~executor/pipeline.rs~~ | ~~run batched vectorized pipeline~~ | ~~parse TQL~~ |
| ~~executor/context.rs~~ | ~~hold scratch execution buffers~~ | ~~allocate per cell~~ |
| transaction/manager.rs | manage txn begin commit | evaluate energy |
| transaction/lock.rs | lock overlapping lattice subcomplexes | store stalks |
| ~~api/client.rs~~ | ~~accept client query sessions~~ | ~~compute Laplacian~~ |
| api/result.rs | serialize section plus energy | open sockets |
| engine/bin-shared/corpus.rs | hold shared E2E bench corpus | link into library |
| ~~distrib/cover.rs~~ | ~~partition poset into overlapping subcomplexes~~ | ~~cut hyperedges by hash~~ |
| ~~distrib/border.rs~~ | ~~represent replicated border fiber slices~~ | ~~own interior cells~~ |
| ~~distrib/schwarz.rs~~ | ~~diffuse border residuals without barriers~~ | ~~call global allreduce~~ |
| ~~distrib/crdt.rs~~ | ~~merge border edits by energy~~ | ~~take distributed locks~~ |
| ~~net/nerve.rs~~ | ~~route queries along nerve graph~~ | ~~use central coordinator~~ |
| ~~net/rdma.rs~~ | ~~read remote border arenas one-sided~~ | ~~wake remote CPU per read~~ |
| ~~net/gossip.rs~~ | ~~exchange border residuals asynchronously~~ | ~~block on barriers~~ |
| ~~net/protocol.rs~~ | ~~frame cross-node sync bytes~~ | ~~parse TQL text~~ |

SRP check: all responsibility statements distinct (recounted post as-built deltas; struck rows excluded). Line gate: `wc -l` must show every file ≤500 lines before Code Skeleton entry — enforced; parser.rs and phase4_bench.rs were split (expr_parser.rs, define_parser.rs, engine/corpus.rs) to comply.

## As-built deltas (2026-10-13 audit: text reconciled to code, no code deleted)

A1. Bench binaries + harness helpers carry no FILE IDs (precedent: phase2_bench, adversarial). `engine/corpus.rs` holds the shared Phase-4 corpus extracted from phase4_bench.rs for the SPEC-022 line gate.
A2. `syntax/parser.rs` exceeded 500 lines → split into parser.rs (374) + expr_parser.rs (WHERE logic) + define_parser.rs (DEFINE logic). Same code, same gates.
A3. Optimizer consolidated: the planned 4-file e-graph (egraph/rules_boundary/rules_pushdown/cost) was never needed — poset pruning is a single-pass rewrite. Implemented as `ir/optimizer.rs` (FILE-038 covers it; METHOD-010 verified). FOLDER-012 merged into FOLDER-011.
A4. Distributed implemented at storage level, not as separate crates: `bcsf/partition.rs` (FILE-021, L3 domains + ghosts) + `bcsf/cover.rs` (FILE-049, greedy hyperedge cover). sheaf-distrib/sheaf-net crates, border/schwarz/crdt/nerve/rdma/gossip/protocol files: DEFERRED. Waivers W-D1/W-D2 are moot (no cross-crate arrows exist); retained for history.
A5. EngineRuntime catalog/executor/client never built: TQL DEFINE statements serve as catalog, bench binaries serve as executor, api/result.rs serves as output. METHOD-016 (head-to-head benchmark) remains stub — analytic comparison in PERFORMANCE.md/PERFORMANCE_PARTITIONED.md (local sheaf-db runs + internet benchmarks) stands in until a PG/Neo4j harness exists.
ROADMAP (no code yet): a deployable service needs, in order: (1) executor pipeline over TIR (un-defer FILE-041/042), (2) client sessions (FILE-046), (3) catalog registry (FILE-039/040), (4) multi-node fabric (FOLDER-017/018). Each step re-enters the state machine at File Design with its own METHOD node; the shipped kernel/compiler/storage are the stable foundation, not the finished database.
A6. `index/` holds only mod.rs (stub). TIFI (FILE-020) implemented as `bcsf/name_lookup.rs` (METHOD-011 verified); `index/partition.rs` implemented as `bcsf/partition.rs` (METHOD-015 verified).

## Part-2 forward deltas (2026-10-08, from Phase-1 adversarial findings)

1. Compactor merges via `BcsfBuilder::extend_from_reader` bulk slice copies with SIMD offset shifts + tombstone-block skips — never cell-by-cell `push` (METHOD-017, parent FILE-048).
2. Phase-2 gates are P99-gated (writes <200 µs, cold-read degradation <5% during merges); medians are informational only.
3. Compactor uses cache-isolated streaming writes (dedicated cores and/or non-temporal stores) to protect reader L3 lines.
4. Phase-3 watchpoint: sparse-morph `binary_search` (~18 branches) isolated in Laplacian bench; stride-offset replacement triggers on branch-stall evidence only.
5. Registry discipline (F1): snapshot registers pin+gen atomically under one lock; install holds the same lock across min+prune — the prune set can never cover a live snapshot.
6. Merge-window gating (F5): reader rate inside the merge call vs a thermally-adjacent post baseline (P1 is warmup); 32k-cell `extend_range` chunks with inter-chunk yields bound burst bandwidth theft.

## Distributed plan deltas (Phase-7, from 10/10 review — HISTORICAL: all items below deferred per as-built delta A4; retained to show intent, not state)

1. Covers, not cuts: `distrib/cover.rs` partitions by open subcomplexes (KaHyPar-minimized borders); ≥95% hyperedges interior (METHOD-019).
2. No barriers: `distrib/schwarz.rs` runs local PCG <10µs and diffuses border residuals async; an `MPI_Allreduce` anywhere fails the gate (METHOD-020).
3. No 2PC: `distrib/crdt.rs` merges border edits to min-Dirichlet-energy sections; interior deletes stay zero-network (METHOD-021).
4. No coordinator: `net/nerve.rs` routes local queries zero-hop and slices spanning queries on nerve edges (METHOD-022).
5. No RPC on hot path: `net/rdma.rs` reads border arenas one-sided; 1.2µs gate is HW-qualified (RoCEv2 required, loopback proves protocol) — same host-qualification precedent as SPEC-007 (METHOD-023).

## Documented DIP waivers (SUPERSEDED by as-built delta A4 — no cross-crate arrows exist; retained for history)

- W-D3 (live): `sheaf-storage` crate declares `sheaf-kernel` in [dependencies], but only `src/bin/phase1_bench.rs` (harness) imports it (sorted-scan check); lib code under bcsf/lsp/index never does — enforced by tests/dip_audit.sh. Rationale: cargo has no per-bin dependencies; splitting the bench into its own crate for one import is churn without layer benefit.

Strict inward-only import would force a new domain-trait layer for two arrows whose cross-layer coupling is inherent, not accidental. Both waived explicitly (FOLDER-014 `application+infrastructure` sets the precedent); the audit script enforces that every outward arrow has a matching entry here, so waivers cannot accumulate silently.

- W-D1: `distrib/cover.rs` (FILE-049) → `index/partition.rs` (FILE-021). Rationale: cover construction consumes KaHyPar partition-map output types directly; interposing a domain trait would add a file to launder a genuine data dependency.
- W-D2: `distrib/border.rs` (FILE-050) → `bcsf/reader.rs` (FILE-015). Rationale: border slices are views over reader snapshots; re-wrapping the reader in an application trait duplicates a zero-cost view for layer purity alone.

## As-built deltas (continued)

A7. Theoretical-maximum program (2026-10-14, planned, unbuilt): bottleneck-probe measurements (bandwidth ~8.4 GB/s read, fsync 1.2 ms, 4-thread scaling ~1.25x, checksum ~2% of ops) motivate FILE-060/061/062 (P1/P3/P4 builds). P2/P5/P6 modify existing files (name_lookup, partition) with no new FILE IDs. Traceability: REQ-030..035 / SPEC-030..035 / METHOD-024..029 (stubs). Predictions are stretch targets with hard-gate parity floors; see MAX_PERF_PLAN.md.
A8: Von Neumann physical-limits probe (2026-10-21, built + verified): a new bench-only bounded context `HardwarePhysics` (FOLDER-020) with 5 files - FILE-099 `bin/physical_limits_probe.rs` (the probe) and FILE-100/101/102/103/104 `bin/shared/phys_mem.rs` / `phys_derive.rs` / `phys_store.rs` / `phys_workload.rs` / `phys_ceilings.rs` (measurement core, physical-floor derivation, storage core, kernel-utilization measurement, fundamental ceilings). It measures this host's memory hierarchy and storage limits and prints the DERIVED physical floors beside them, so the comparison is measured-vs-derived rather than asserted. It touches no engine code and imports no crate (pure std + inline asm), so it adds no DIP edge. Gate: `tests/e2e/von_neumann/run.sh` (6 physics-invariant ordering gates, 2 clock cross-checks, 2 kernel-utilization gates, 4 ceiling gates). Report: `VON_NEUMANN_LIMITS.md`. Two repo corrections filed there: thread scaling is 1.47x (not 1.25x - the old figure counted a memcpy's write traffic), and the "64 MB L3 domain" of SPEC-016 is 5.3x this host's real 12 MiB L3, so domains must be sized to the measured LLC of the target class. Traceability: REQ-057 / SPEC-057 / METHOD-052, METHOD-053, METHOD-054.
A7. Theoretical-maximum program (2026-10-14, planned, unbuilt): bottleneck-probe measurements (bandwidth ~8.4 GB/s read, fsync 1.2 ms, 4-thread scaling ~1.25x, checksum ~2% of ops) motivate FILE-060/061/062 (P1/P3/P4 builds). P2/P5/P6 modify existing files (name_lookup, partition) with no new FILE IDs. Traceability: REQ-030..035 / SPEC-030..035 / METHOD-024..029 (stubs). Predictions are stretch targets with hard-gate parity floors; see MAX_PERF_PLAN.md.

## As-built deltas (continued, 2026-10-22)

A9. Performance-waste measurement context (2026-10-22, built + verified): two bench-only
binaries in the existing `HardwarePhysics` bounded context (FOLDER-020), plus a `SPEC-058` plan.
- `crates/sheaf-engine/src/bin/bundle_load_bench.rs` (FILE-115): decomposes the IMDb bundle
  section load into the raw read, the CRC32C verify, and the buffer zero-fill, and reports the
  share of the file that a section-scoped read pays for.
- `crates/sheaf-engine/src/bin/q1_kernel_bench.rs` (FILE-116): compares four variants of the
  IMDb Q1 kernel on the real bundle sections, with a gate that all four agree on count, votes
  and average.
Both import only the existing `shared/imdb_fiber.rs` and `sheaf_storage::lsp::ring_wal::crc_hw`.
Neither adds a crate edge, so the dependency-inversion audit stays clean. Two findings that
change the design, not just the report: (1) the bundle reader is already section-scoped, so the
previously claimed 7.14x whole-file read waste does not exist, and the real load cost is the
CRC32C second pass at 23.7% to 32.7%. (2) The 1.40x accumulator gain measured on the TPC-H
scan kernel does not reach the IMDb path, where the same change measures 0.851x to 0.986x.
Traceability: REQ-058 / SPEC-058 / METHOD-059..066, planned.

A10. Planned build and load path changes (2026-10-22, planned, unbuilt). The plan of record is
`PERFORMANCE_FIX_PLAN_2026-10-22.md`. No file in the storage engine, the kernel, the compiler or
the query language changes. Every node is in the bench and bundle path:
- `shared/imdb_fiber.rs` (FILE-074) gains a section writer for the answer fiber and the
  pre-aggregates, and `BundleReader::section` folds the CRC32C verify into the read.
- `imdb_fiber_bench.rs` (the bin that owns `shared/imdb_fiber.rs` and the build function) gains
  the answer-fiber and pre-aggregate build steps, a one-pass-per-line field extractor, a digit
  loop in place of the generic parse, and a 4-thread split over the parse.
- The `field` helper, which today scans the line from byte 0 for every field it wants, is
  replaced by a single pass that collects every wanted field. The basics stage examines 18 tab
  positions per row where 9 are enough.
- The bundle format version is bumped so an old bundle is rebuilt rather than answered from.
The dependency that governs the order: the pre-aggregate joins add 18,349 ms to the build, so
the 4-thread parse (112,721 ms to 28,180 ms) must land before the pre-aggregates are worth
anything at scale. Traceability: REQ-058 / SPEC-058 / METHOD-059..066.

## Failure-state disposition (2026-10-27)

The table below records each measured failure state, whether a modification is required, and the architecture action.

Rows are failure states. Columns are opponent, measured deficit, modification required, and architecture action.

| Failure state | Opponent | Measured deficit | Modification required | Architecture action |
|---|---|---|---|---|
| Durable single-point write | SQLite 3 | 4.06x slower (1.70 us vs 6.90 us) | No | Keep epoch WAL for batched writes. Do not chase SQLite on scalar inserts. |
| Raw join without fiber bundle | DuckDB 1.5 | 15.79x slower (158.30 ms vs 2,500.00 ms) | No | Keep pre-folded fiber bundles. Document as an architectural boundary. |
| Analytical scan, single accumulator | DuckDB 1.5 | Solved: 4.00 ms vs 5.60 ms | No | Keep the four-accumulator fused scan. |
| Cold ingest and setup | DuckDB 1.5 and Redis | 51.10 s to 112.72 s build; bundle cache under 1 s on the second build | Yes | Add bundle cache, incremental section rewrite, and partial rebuild. |
| Pure 1D vector slice filter | Kdb+ 5f | 4.80x slower (10.00 us vs 48.00 us) | Optional | Add a dense 0-cochain vector path only if time-series workloads are in scope. |

Notes:

- The SQLite and DuckDB raw-join losses are permanent boundaries. No modification can remove them without changing the engine identity.
- The analytical scan loss is already removed by loop fusion.
- The ingest loss is structural but reducible. The bundle cache and partial rebuild nodes cover it.
- The Kdb+ loss is a specialization gap, not a correctness gap.

## Foreign-engine paths considered and rejected (2026-10-27)

These paths are listed for contrast only. They are not accepted unless they are recast as sheaf operations.

The table below records a foreign-engine path for each failure state. Each path names the lower bound it rests on. None of these paths is accepted as a replacement for the sheaf.

Rows are failure states. Columns are opponent, mathematical path, modification required, and verification gate.

| Failure state | Opponent | Mathematical path | Modification required | Verification gate |
|---|---|---|---|---|
| Durable single-point write | SQLite 3 | Group commit amortizes the fsync: per-write cost equals T_append plus T_fs divided by N. A fixed-size descriptor and hardware CRC lower T_append. | Yes | p50 under 1.70 us on a quiet host |
| Raw join without fiber bundle | DuckDB 1.5 | Radix hash join in expected O(n + m) time with vectorized probe. | Yes | Join time under 158.30 ms on 6,001,215 rows |
| Analytical scan, single accumulator | DuckDB 1.5 | Already 4.00 ms. Further SIMD reduction moves toward bytes divided by bandwidth. | Optional | Scan time stays under 5.60 ms |
| Cold ingest and setup | DuckDB 1.5 and Redis | SIMD TSV parse and parallel I/O move toward input bytes divided by disk bandwidth. | Yes | Build time under the measured opponent setup |
| Pure 1D vector slice filter | Kdb+ 5f | Dense 0-cochain arrays with SIMD filter remove cell-header overhead. | Yes | Slice time under 10.00 us on 100,000 rows |

Notes:

- "Mathematically possible" means the target sits inside a known lower bound. It does not mean the target is measured today.
- Each path needs a measurement on the real code path before it can be claimed.
- The raw-join path must stay an optional operator. It must not replace pre-folding, or the sheaf identity changes.
- The Kdb+ path is a specialization. It does not change the sheaf model.

## Sheaf-native resolution policy (2026-10-27)

The cellular sheaf, the fiber bundle, and every layer in this document stay in place. A fix that replaces them with a relational, vector, or key-value engine is rejected, even if it is faster.

Rows are failure states. Columns are foreign technique rejected, sheaf-native technique accepted, and identity preserved.

| Failure state | Foreign technique rejected | Sheaf-native technique accepted | Identity preserved |
|---|---|---|---|
| Durable single-point write | Generic group commit and B-tree page writes | Epoch-stamped fiber section append with local section commit | Yes |
| Raw join without fiber bundle | Relational hash join | Fiber product of sheaves over a common base, plus pre-folded boundary complexes and answer fibers | Yes |
| Analytical scan | Unchecked SIMD accumulator tricks | Graded cochain reduction over partitioned sheaf scans | Yes |
| Cold ingest and setup | Generic parallel CSV parser | Parallel local section assembly and gluing under a frozen epoch | Yes |
| Pure 1D vector slice filter | Standalone numeric vector engine | Restriction of a 0-cochain to an interval subcomplex using dense coordinate vectors | Yes |

Notes:

- The raw-join boundary remains. The sheaf does not promise ad-hoc joins on flat tables. It promises pre-folded connections.
- The Kdb+ boundary remains for pure numeric vectors. The sheaf adds structure, and that structure has a cost.
- Every accepted technique must be verified on the real code path before it is claimed.