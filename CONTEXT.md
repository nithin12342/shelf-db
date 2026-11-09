# CONTEXT — Cellular Sheaf Database (sheaf-db)

Source: `7 Database Paradigms Breakdown - Gemini Conversation (2026-10-08_12-09-49).md` (14 turns, saved `Thu Oct 08 2026 12:09:49 GMT+0530`).
Method: `@intention-engineering` — Planning output. No implementation detail below is actionable until `SKELETON.md` gates pass.

## 1. Business goal (one paragraph)

Build a personal-project Rust database + query language that natively expresses N-dimensional nodes/edges/relations/hyper-relations via cellular sheaf theory, beating SQL/graph/document/wide-column/KV on higher-order (N>=4) queries — not by out-tuning their home turf (B-Trees, Redis hashes, columnar scans) but by changing the computation to topological consistency + sparse linear algebra ("zigzag on land", Turn 3).

## 2. Scope: in / out

In: relational, graph (1-D complex), tree (poset ideal), document (atomic payload), wide-column-sparse, KV (0-D cells) unified under one poset + stalk model; native harmonic/sheaflet search (`min x^T L_F x`) instead of Lucene/HNSW add-on.
Out: multimodal/file-blob store (Turn 2 — separate file storage); generic full-text (BM25/tokenizer/fuzzy automata) as Phase-1 target (Turn 3 — deferred, then rebuilt natively as harmonic search, not inverted index).

## 3. Hard constraints (from conversation)

- Rust only (memory layout, zero-cost, fearless concurrency). Turn 2.
- Every file <=500 lines; ~60 files across 5 crates (sheaf-distrib/sheaf-net deferred to storage-level modules per ARCHITECTURE as-built delta A4). Turn 7.
- E2E-only verification: real binary + real fixture → `.section` artifact; no mocked unit tests (skill `language-profiles/rust.md`, Turn 5/8).
- Hardware-first build order: BCSF → SIMD kernel → hand-written API + benchmark vs PG/Neo4j → TQL compiler. Never compile to an imaginary target. Turn 10.
- Dedicated vectorized kernel mandatory (not Volcano `next()->Tuple`); 1024-cell batches, L1/L2 residency; CPU→GPU offload only at subcomplex granularity. Turn 10.
- Idempotent scripts, one node = one commit with compiler + runtime evidence (skill principles #8/#9).

## 4. Math foundation (agreed, Turns 3-4 + Turn 13 fixes)

- Base: graded poset `(P, <=)`; 0-cells entities, 1-cells binary edges, k-cells N-ary hyper-relations; `σ<=τ` = incidence. Regular CW + locally Eulerian + canonical bit-parity `[σ:τ]=(-1)^i` so `δ²=0` (Turn 13 Gap 1 fix; pairwise `Δx` alone is insufficient for N>=3). `δ`/`∂` are defined only between adjacent dimensions (codim 1): a k-cell pointing directly at 0-cells is evaluated as a composite chain map through virtual codim-1 faces generated on the fly via canonical sorted-subset indexing — no intermediate cells stored, `δ²=0` preserved algebraically.
- Stalk: dual `F(σ) = (F_disc, F_cont)`; discrete = byte/bitmask/enum exact payload; continuous ≅ R^d SIMD f32 embeddings. Turn 4 Vuln 1.
- Morphism: typed `F_{σ<=τ} = (π_{σ<=τ}, W_{σ<=τ})`; `π` zero-copy mask, `W` dense/sparse col-major matrix.
- Operators: coboundary `(δx)(τ) = Σ_{σ◁τ} [σ:τ]·F_{σ<=τ}(x_σ)`; Laplacian `L_F = δ^T δ` (PSD only with Gap-1 fix); energy `E(x) = x^T L_F x + λ||x-x0||² <= ε` (soft sheaf, Turn 4 Vuln 2); cohomology `H^0 = ker δ`; solver = Block-Jacobi PCG `M=diag(L_F)`, `κ(M⁻¹L)<=15`, 8–18 iters (Turn 13 Gap 2 fix; unconditioned CG stalls).
- Storage invariant: pruned poset (maximal + demanded cells only; lazy sub-faces; sorted boundary IDs, AVX-512 intersect) — never full `O(2^N)` simplicial closure (Turn 4 Vuln 3). Down-closure `τ∈P,σ<=τ⇒σ∈P` enforced via Upper-Star tombstones, not BCSF shifts (Turn 13 Gap 3).
- Lookup invariant: attribute queries go through TIFI — indexed values as (-1)-dim virtual roots → contiguous `CellId` slice (Turn 13 Gap 4; BCSF alone is O(N) scan).
- Partition invariant: 64 MB L3 domains via KaHyPar/METIS k-way hypergraph partition + Ghost Fiber replicas; kernel never does cross-socket per-cell fetch (Turn 13 Gap 5).

## 5. Hardened architecture (Turn 4 + Turn 13)

```
TQL Compiler (parse→Eulerian-validate→TIR→E-graph π-pushdown+∂∂=0→12-opcode plan)
LSP (MemComplex lock-free/star-cascade/TIFI/MVCC + BCSFv2 64B blocks/live-masks/M⁻¹ + NUMA compactor via extend_from_reader bulk merges with streaming writes)
SIMD Kernel (FMA coboundary + BJ-PCG + certified .section emit)
Distributed (Phase-7 planned: Mayer-Vietoris covers → RAS async diffusion → cohomology-CRDT gluing → Čech-nerve routing → RDMA border reads; no hash cuts, no barriers, no 2PC, no coordinator, no hot-path RPC)
```

## 6. Hardware mapping (Turn 14, summary)

NVMe: WAL + frozen BCSF (10–50 µs, 7 GB/s seq; no traversal) → RAM dual-buffer MemComplex/BaseComplex flat arrays + NUMA/ghosts (60–100 ns) → L3 32–96 MB subcomplex → L2 ~1 MB W+M⁻¹ (~3 ns) → L1 32–48 KB boundary+bitmask (~1 ns) → AVX-512 zmm FMA coboundary/energy/PCG (~0.5–12 ns) → GPU only for whole-subcomplex sparse linalg. Budget ~30–150 ns/subcomplex vs PG 5–50 µs / Neo4j 2–10 µs.

## 7. Six-phase plan with gates (Turn 5; details in SPEC.md)

1. BCSF in-memory (<1.5× bytes, sub-15 ns lookup) → 2. LSP+ACID (4W@50k/s + 8R bulk-merged, P99 write <200 µs, cold-read P99 degradation <5% during merges — tail-gated per Part-2, never median) → 3. Kernel (10k noisy nodes → E≤1e-4, ≤25 iters, <8 ms; morph-fetch watchpoint armed) → 4. TQL frontend (50 valid <50 µs, 20 malformed pinpointed) → 5. E-graph optimizer (N=6 O(2⁶)→single slice per EXPLAIN) → 6. Head-to-head (N≥4 ≥5× QPS, P99 sub-ms; e.g. 1.8 ms vs PG 38 ms / Neo4j 52 ms — analytic comparison in PERFORMANCE.md stands in, live harness environment-blocked) → 7. Distributed (planned, SPEC-025..029: covers → Schwarz → CRDT gluing → nerve routing → RDMA; E2E `tests/e2e/phase7_dist/`).

## 8. Language + verifiable output (Turns 8/9/11)

TQL: DDL `DEFINE CELL/HYPERCELL TYPE + MORPHISM`, DML `INSERT CELL/HYPERCELL OVER BOUNDARY`, DQL `MATCH ... WHERE ... RESTRICT TO BOUNDARY ... SOLVE HARMONIC(λ,MAX_ITER,ENERGY_THRESHOLD) EMIT SECTION`. Lowers to 12-opcode plan (`BCSF_SCAN→DISCRETE_FILTER→GATHER→APPLY_MASK→VEC_GATHER→LOAD_MORPHISMS→BUILD_LAPLACIAN→SOLVE_PCG→EVAL_ENERGY→FILTER_THRESHOLD→MATERIALIZE→YIELD`); discrete bitmask chain can skip `SOLVE_PCG`.
`.section` YAML: `status, execution_time_us, subcomplex{dim,cell_count,cells}, discrete_payload, spectral_metrics{dirichlet_energy,iterations,cohomology_state}, harmonic_section{stalk_projections}`.

## 9. Top risks

R1 Orientation PSD breakage → mitigated by Eulerian validator (Gap 1). R2 Solver stall on hubs → BJ-PCG (Gap 2). R3 Delete corruption → tombstones (Gap 3). R4 O(N) attribute scans → TIFI (Gap 4). R5 NUMA stall → partition+ghosts (Gap 5). R6 Scope creep into per-cell GPU/JIT before Phase 3 gates pass → blocked by build-order gate (Turn 10). R7 Compactor evicts reader L3 → bulk merges with cache-isolated streaming writes only (Part-2). R8 Sparse-morph branches stall SpMV → watchpoint armed, stride-offset trigger on evidence (Part-2). R9 No RDMA HW on dev laptop → loopback proves protocol, 1.2µs gate HW-qualified (SPEC-029). R10 Async Schwarz divergence under partition → bounded residual exchange + energy-min gluing contains it (SPEC-026/027).

## 10. Theoretical-maximum program (2026-10-14, planned)

Exhaustive bottleneck probing (bottleneck_probe, results in results/bottleneck.json) fixed three walls: memory bandwidth (~8.4 GB/s true read; 4-thread scaling only ~1.25x, controller-capped), fsync floor (~1.2 ms; neither side syncs per op), checksum materiality (~2% of durable ops, Amdahl). Refuted two hypotheses: bounds-check cost (all variants within noise) and thread-scaling gains. Response: six proposals P1-P6 (parallel partitioned scan; validity-free nulls; incremental pre-aggregation; append-only epoch WAL headers; MPHF/branchless index; bitmask adjacency), each with hard parity gates + stretch targets, ordered P1+P3, P4, P2/P6, P5. Full proposal + falsification criteria: tests/e2e/realtime_bench/MAX_PERF_PLAN.md. New traceability: REQ-030..035 / SPEC-030..035 / METHOD-024..029 (stubs). Business goal unchanged: wins must come from structural advantages (pre-fold/stream/pre-aggregate), never from contesting physics floors.

## 11. Fiber lifecycle + write-time correctness (2026-10-20, planned)

**Trigger.** Production-readiness review of the fiber (pre-fold) layer. Measured facts: bundle build is O(dataset) per run (TPC-H 1.8 s; IMDb Q2+Q3 178.96 s = 164% of a clean release build); fibers are a frozen snapshot with **no epoch stamp anywhere in storage**; `compactor.rs` renumbers live cells (`remap_base`), so fiber keys dangle after compaction; a build racing a compaction can produce a torn fiber whose CRC still validates (no version stamp).

**Decision.** Scope A (N1-N4 + N6 + N8). Axis B (release-profile restructure) is separate and gated by a 3-part bar. Plan of record: `FIBER_LIFECYCLE_PLAN.md`. Traceability: REQ-036..042 / SPEC-036..042 / METHOD-030..037.

**Axis A (runtime bundle).** Extract the pre-fold into `crates/sheaf-engine/src/bin/shared/fiber.rs` (METHOD-030), then add a fingerprint-bound bundle cache (epoch + row counts + schema version + bundle kind): unchanged data rebuilds in <1 s, any fingerprint change is a MISS (never a stale hit), cached answers must equal cold-build answers.

**Correctness under writes.** Epoch-stamped bundles with fail-loud validation and an always-correct scan fallback (METHOD-032); per-64 MB-domain incremental maintenance so a write re-sorts only its dirty domain (METHOD-033); frozen-epoch builds under the compactor's LSM fencing (METHOD-034); a concurrent read/write stress gate of 10,000 mixed ops where the fiber answer must equal the fallback answer every time (METHOD-035). The naive scan path stays permanently as the correctness oracle.

**Axis B (compile-time, gated).** Fat-LTO clean release build measured at 109.3 s (fresh `CARGO_TARGET_DIR`; warm 0.19 s; incremental 0.8 s; release artifacts 0.05 GB). thin-LTO + `codegen-units=16` is allowed only with all three of: clean build <=55 s, bit-identical Phase-3 kernel results, identical measured-query timings across all 11 `SHELF_BINS`. Any miss closes the node as `recorded` with the profile untouched.

**Production shape (decided).** In-process only; scale horizontally by partitioning into multiple copies (Phase-6/7 cover partitioning, 64 MB L3 domains, ghost replicas, CRDT energy-merge, zero-hop nerve routing). The client/server plan was deleted; remote access would erase the ns-class wins (10.5 ns -> ~70 us over loopback) while leaving ms-class wins intact.

**Conversation record.** IMDb complex suite (Q1 filter+agg, Q2 4-table top-10, Q3 4-table rollup+HAVING) built with md5-cross-checked PG==DuckDB baselines; two silent SQL dialect divergences found by that gate (DuckDB `/` true division vs PG integer division; DuckDB `CAST(double->INT)` rounds vs PG truncates); ShelfDB fiber counterpart gates all PASS (Q1 count/votes/avg exact; Q3 all 8 decade groups exact; Q2 all 10 names+counts exact); 9-category IMDb comparison PASS (ShelfDB 143 ms; Redis 199; ScyllaDB 266; DuckDB 346; SQLite 353; Neo4j 385; MariaDB 1040; PostgreSQL 1918; MongoDB 183,619). Engineering fixes found: psycopg3 text COPY is per-line CPU-bound (474 s/GB) -> DuckDB postgres extension loader (195k rows/s); PG WAL checkpoint storm -> UNLOGGED tables; Redis cannot join (needs denormalization); Neo4j duplicate credit rows inflate pair counts 3.2x; Neo4j `count(*)` over 2-hop expansion exhausts the 4.2 GiB heap -> degree-form aggregation.

## 12. Comparison repair, NoSQL challenger, and cached fiber bundles (2026-10-21)

**Trigger.** A run of the 8-engine matrix produced a summary table in which "MongoDB wins" sat next to "ShelfDB 143 ms". Adversarial re-check showed those were **different corpora**: MongoDB's 29.7 ms was the synthetic 5x100k set (200-row $lookup); ShelfDB's 143 ms was IMDb (12.9M + 1.7M rows). On the **same IMDb data** MongoDB's query measured **183,618.6 ms** vs ShelfDB **142.7 ms** (1,287x). The verdict was an artifact of the comparison, not a result. MongoDB was uninstalled; **ScyllaDB (CQL) is the NoSQL challenger** (it moves the join to write time and answers the same truth from a clustering-range scan: 90 ms query, exact, 1,991x MongoDB, but 28.1 s build vs ShelfDB's 0.38 s total). Rule adopted: **every row in a comparison table must name its dataset.**

**Root cause of the stale Neo4j number.** The IMDb bench deleted all Neo4j nodes to load its own graph, wiping the TPC-H-derived graph that `neo4j_bench` measures (traversal "8.6 ms" over an empty DB, paths 0). The delete is now scoped to the `Title`/`Person` labels; Neo4j re-imported (2.25M nodes / 2.1M rels) and re-benched: traversal 1,398.5 ms, paths 600,000.

**Fiber-bundle work (Axis A, the 179 s/run problem).**
- **B3** zero-copy bundle load: read once, borrow the bytes, decode on the fly; integrity switched from the byte-wise IEEE CRC32 (150-250 ms per 42.5 MB) to the repo's SSE4.2 CRC32C (8.5-23 ms). Gate rewritten to a **same-run ratio** after three measurement bugs (dead-code-eliminated reference CRC, warm-vs-cold second read, cross-run baseline) - see CONVERSATION_LOG_2026-10-21.md.
- **B2** IMDb sectioned bundle: the 178.96 s build is written once (51.1 s) into a container with per-section CRC32C; a run fetches only the sections its query needs. Q1 240 ms, Q2 ~10 s, Q3 ~1.2 s (10-65x), all exact.
- **B1** TPCH fiber bench now uses the cached bundle: 4,200 ms pre-fold per run -> 24.8-54.5 ms (77-169x), totals exact.

**Method discipline enforced by the gates (all recorded, none hidden):** same-run ratios instead of cross-run baselines; build and query reported separately everywhere; a cached bundle counted as an INDEX BUILD (B13 gate policy); every table row names its dataset; Rust-first (the IMDb ETL moved into `imdb_scylla_prep.rs`; Python kept only for CQL transport, since no Rust CQL driver is allowed without a new crate).

**Production shape (unchanged, now evidenced):** in-process only - the ns-class wins (10.5 ns) die over a socket - with scale-out by partitioning into multiple copies (Phase-6/7 covers, ghost replicas, CRDT gluing, zero-hop routing).

## 13. Fiber lifecycle wave 2 — the remaining plan nodes (2026-10-21)

Rows = node, Columns = the capability and the number that proves it.

| Node | Capability | Proof |
|---|---|---|
| B4 | Versioned tombstones: a snapshot copies NO liveness state | snapshot creation 770.71 us -> **0.23 us** (O(n) -> O(1)) |
| B5 | Shadow paging: the merge copies only the delta when nothing is dropped | shadow merge **1 ms** vs copy **6 ms**; chained lookup 5.57 ns (flat 5.86 ns) |
| B6 | SoA is the primary stalk layout: column reads are views | transpose allocates **0 bytes** (was 48 MB); scan 7.9 -> 5.6 ms |
| B8 | Durable segment descriptor: a segment opens without the builder | 2,500 cells round-trip identical; corrupt/truncated/alien rejected |
| B9 | The value->cells index survives process exit | a 2nd process reloads and re-serializes it byte-identically; scan parity |
| B10 | The L3 domain plan + ghost map survive process exit | the reloaded map still satisfies `domains_fit_l3` + `ghosts_cover_boundaries` |
| B11 | A table opens BY NAME and the schema version gates every plan | a v5 plan against a v4 table is refused |
| B12 | A changed input rewrites only its section | 260 KB re-encoded, 780 KB copied (of 1.04 MB) |
| B14 | Zero-copy WAL replay | 4.8 ms -> 3.5 ms, 20,000 frames identical |
| N3 | A fiber built at epoch E refuses to answer at E+1 | mismatch -> refused; the scan fallback matches exactly |
| N4 | Per-domain incremental fiber update | 1,000 writes -> 2 re-sorts (the dirty domains) |
| N5 | The compactor exposes the frozen epoch of each merge | a concurrent build is byte-equal to a quiet-box build |
| N6 | The fiber lifecycle gate | 10,000 mixed ops, 0 mismatches, lag <= 2 generations |

Invariants that must hold after any change in this area: the reader's accessors go through
`resolve()` (a chained reader has empty slabs); a delta is a FRAGMENT (its boundary ids point at
the base) and is validated with `validate_fragment`; the morph-cell index is the builder's OWN
index; a stale bundle is rebuilt, never answered from; a stale fiber falls back to a scan, and the
scan must return the same aggregate.

## 14. Von Neumann physical limits — measured vs derived (2026-10-21)

**Trigger.** The question "what are the physical limits of this architecture" needed an answer
that was *measured*, not asserted. The repo already had a bottleneck probe (bandwidth, fsync,
CRC) but it measured a memcpy, not the memory hierarchy, and it never separated DRAM latency
from page-walk cost.

**Method.** New probe `crates/sheaf-engine/src/bin/physical_limits_probe.rs` (REQ-057 /
SPEC-057 / METHOD-052, FILE-099..102) measures this host's actual limits and prints the
DERIVED physical floors beside them. Report: `VON_NEUMANN_LIMITS.md`. Gate:
`tests/e2e/von_neumann/run.sh` (6 physics-invariant gates + 2 clock cross-checks).

**Key numbers (3 runs, medians; host: i7-1165G7, 16 GiB DDR4-3200 single channel, 512 GB NVMe):**

| Quantity | Measured | Derived floor | Source |
|---|---|---|---|
| L1 / L2 / L3 latency | 1.64 / 4.27 / 21.97 ns (4.6 / 13.7 / 62.6 cycles) | — | pointer chase |
| DRAM random access | 163.7 ns (cold, 128 MiB) | 41.2 ns | DDR4-3200 CL22 tRCD+tCL+tRP |
| DRAM access component | 92.6 ns | 41.2 ns | 3-quadrant decomposition |
| Page walk (4 KiB pages) | 46.6 ns | — | footprint-controlled experiment |
| Read bandwidth 1T / 4T | 11.5 / 17.0 GB/s | 25.6 GB/s (1ch peak) | pure read stream |
| Machine balance (ridge) | 2.92 FLOP/byte | — | same-window compute+bandwidth |
| fsync 4 KiB | 1307.6 µs | — | device flush path |
| NVMe seq read | 3.20 GB/s | 7.88 GB/s | PCIe 4.0 x4 bus |
| Landauer | — | 2.87e-21 J/bit | kT·ln2, 300 K |
| Si signal flight (10 mm) | — | 120 ps | c/3.6 |
| HDD areal density (10 yr) | — | 1.20 Tb/in² | K·V ≥ kT·ln(t/τ0) |

**Two repo corrections filed (see VON_NEUMANN_LIMITS.md §3):**
1. **Thread scaling is 1.47×, not 1.25×.** The old 1.25× came from counting a *memcpy*'s
   write traffic against the read bus. A pure read stream gives 11.5 → 17.0 GB/s (1T → 4T);
   4→8 adds only 13% because the 64-bit DDR bus is 75-95% saturated. The wall is the
   **memory bus**, and this host has only ONE populated channel.
2. **The "64 MB L3 domain" in SPEC-016/ARCHITECTURE is 5.3× this host's real 12 MiB L3.**
   The measured L2→L3 cliff is 5.1×, so a 64 MB domain on this host is mostly DRAM. Same
   wall `SPEC-007` already documented from the other side ("12 MB laptop LLC"), now
   measured directly. The 64 MB figure is a SERVER target; domains should be sized to the
   measured LLC of the target class.

**One design thesis confirmed numerically.** `VON_NEUMANN_MAPPING.md` said "fewer bytes
moved, not more FLOPs". Now it is a number: the measured ridge point is 2.92 FLOP/byte and
this project's d×d matvec runs at 0.33-0.47 FLOP/byte — 6-9× below the ridge, i.e.
structurally memory-bound. f32, the factored Laplacian, SoA fused columns, pre-aggregation
and the fiber bundle are all the right response, and the 1-core wall is 11.5 GB/s.

**Methodology corrections found by the gates (worth keeping).** Three measurement bugs were
caught by the probe's own cross-checks, not by inspection: (a) the pointer chase kept the
chain in a side array, so it measured CACHE latency while claiming DRAM; (b) the clock was
derived from a hand-analysed Rust loop that LLVM had strength-reduced and unrolled; (c) the
clock ratio divided independently-best-taken chain times, mixing thermal windows. The
final probe derives the clock from the MARGINAL cost of one added dependency and validates
it with two independent cycle-count cross-checks, measured on adjacent pairs and aggregated
by median.

**Does the database reach the memory limit? No, and the cause is known.**
The probe measures the real storage-engine scan kernel against the memory ceiling in the
SAME window, on the same 48.0 MB of TPC-H lineitem data. Utilization: **42.3% at one
thread, 55.5% at two, 80.8% at four** (METHOD-053, FILE-103, `shared/phys_workload.rs`).
The memory ceiling reaches 22.4 GB/s at four threads, so memory can feed the kernel faster
than the kernel consumes. The gap is the **single f64 accumulator** in the partition scan
(`sheaf-kernel/src/simd/mt_scan.rs`): every row waits for the previous row's multiply-add,
so the memory idles. A controlled 1-lane vs 4-lane experiment on the same bytes shows
**1.47x** from breaking that chain alone, with the aggregate unchanged (rel diff 2.1e-7,
which is double-precision rounding over 6e6 additions). The fused structure-of-arrays
kernel already uses four lanes and is 1.33x faster at one thread. **This is a loop-shape
fix, not a hardware limit, and it is the next change to make.**

**The remaining ceilings, now computed rather than assumed** (METHOD-054, FILE-104,
`shared/phys_ceilings.rs`):

| Ceiling | Result | Consequence for the design |
|---|---|---|
| Amdahl parallel speedup | 1.34x ceiling, fitted to this host's own thread scaling; measured 1.24x | **The thread lever is spent.** Do not plan around more threads. |
| DRAM refresh | 0.128% of time, 0.008% of the external bus | Invisible. The density wall is cell leakage, not the bus. |
| Soft errors from cosmic rays | 64 errors per month in 16 GiB, 2.13 per day; one bit survives 1.76e8 years but the module fails every 11.2 hours | **Error correction is mandatory, at a rate set by physics.** Every WAL frame already carries a CRC32C. |
| NAND endurance | 300 terabytes-written rating exhausted in 3.2 years at 0.1 device writes per day | The write budget is 51.2 GB per day for the whole drive. The fiber bundle turns a write-heavy load into a read-only one. |
| Landauer, Margolus-Levitin, Bekenstein, address space | 3.5e5x, 1e24x, 3e-15, 1e-7% away | Not binding. Do not design against them. |

## 15. Performance waste and the fix plan (2026-10-22)

**Trigger.** The question "why is the mathematically optimal architecture slow, and where is the
performance wasted" needed an answer that was measured, for both comparison tables. The
measurements are in `PERFORMANCE_WASTE_ANALYSIS.md`. The plan of record is
`PERFORMANCE_FIX_PLAN_2026-10-22.md`, timestamped 2026-10-22 13:01:38 local (Asia/Kolkata, UTC+05:30), which is 2026-10-22 07:31:38 UTC. The session record is
`CONVERSATION_LOG_2026-10-22.md`. Traceability: REQ-058 / SPEC-058 / METHOD-059..066.

**The answer.** The shelf database is not slow because it moves bytes. It is slow because it
resolves the join at query time, and because its build parses text one byte at a time. The von
Neumann analysis is right about the data representation and silent about where the time goes.

**The query-time ledger.** The section byte counts are read out of the bundle index directly.

| Query | Bytes moved (B) | Query time (ms) | Share of the 19.2 GB/s bus peak | Time that is not byte movement | Share not bytes |
|---|---|---|---|---|---|
| IMDb Q1 | 201,692,415 | 153.4 to 173.6 | 6.0% to 6.9% | 142.9 to 163.1 ms | **93.2% to 93.9%** |
| IMDb Q2 | 1,097,898,654 | 18,333.3 | 0.31% | 18,276.1 ms | **99.7%** |
| IMDb Q3 | 306,779,567 | 1,746.9 | 0.92% | 1,730.9 ms | **99.1%** |
| TPC-H join, answer fiber | 208 | 3.2 to 7.9 | n/a | n/a | the answer is already materialized |

**The total-time ledger.**

| Item | Time (ms) | Share of the parent | What it is |
|---|---|---|---|
| The build's character-level parse | 108,756 to 111,080 | **96.5% to 98.5% of the 112,721 ms build** | 72,779,864 rows of scalar text handling |
| The build's file read | 1,641 to 3,965 | 1.5% to 3.5% of the build | 6,383.8 MB at the measured 1.61 to 3.89 GB/s |
| The load's CRC32C second pass | 32.7 to 43.4 | **23.7% to 32.7% of the 120.8 to 137.9 ms load** | a second pass over the same bytes |
| The Q1 query's join | 142.9 to 163.1 | 93.2% to 93.9% of the query | 1,721,477 map inserts, 3,216,590 random probes |
| The Q2 query's join | 18,276.1 | 99.7% of the query | 42,453,755 probes into a 15,738,257-entry map |

**The structural cause.** The byte ratio against the answer is 280.7x on the IMDb Q1 shape and
0.13x on the TPC-H fiber. The TPC-H fiber is a pre-aggregate and holds the answer. The IMDb
bundle is a pre-parse and holds the source columns, so every query re-derives the answer, and a
join is a chain of random probes that does not care how few bytes the machine moves. The two
structures share one name and one set of claims, and the difference between them is the whole
story.

**The build, row by row.** 72,779,864 rows in 112,721 ms, which is 1,549 ns per row and
56.6 MB/s of text. The measured hash insert is 116 to 137 ns, so 91.2% to 92.5% of the per-row
cost is text extraction and formatting. The basics stage inserts into no map at all and still
costs 868 ns per row. The `field` helper scans the line from byte 0 for every field it wants, so
the basics stage examines 18 tab positions per row where 9 are enough, which is 115,797,375
redundant tab reads over the stage.

**The plan, eight nodes, with the order as a dependency.**

| Node | Waste it removes | Gain | Gate |
|---|---|---|---|
| METHOD-065 | the unverified DuckDB setup number | no performance gain. It settles the 511x gap. | the recorded number must be at or above the physical floor |
| METHOD-061 | the redundant tab examinations | removes 9 of 18 per basics row | the six sections byte for byte |
| METHOD-062 | the generic parse and the UTF-8 validation | a hypothesis, 91.2% to 92.5% of the build | the six sections byte for byte |
| METHOD-063 | the serial parse | **4.00x**, 112,721 ms to 28,180 ms | the six sections, plus the three query gates |
| METHOD-064 | the CRC second pass | **1.31x to 1.48x** on the load | the CRC corruption negative test |
| METHOD-059 | the Q1 join | **1,646x to 3,003x** on the query | the variant gate on count, votes and average |
| METHOD-060 | the Q2 and Q3 joins | **83x to 116x** on Q2 | the md5 cross-engine gate |
| METHOD-066 | the latency fixes aimed at the wrong end | a hypothesis. The build has no dependency chain. | the six sections |

**The dependency that matters.** The pre-aggregate joins add 18,349 ms to the build. With
METHOD-063 the build is 46,529 ms, which is 2.42x better than today. Without it the build is
131,070 ms, which is worse than today. METHOD-063 must land before METHOD-059 and METHOD-060
are worth anything at scale.

**Two corrections to the design thesis, both measured.** The 1.40x accumulator gain measured on
the TPC-H scan kernel does not reach the IMDb path, where the same change measures 0.851x to
0.986x and a 4-thread sharded map measures 0.961x to 1.059x. The wall on that path is latency
on random probes, and the memory controller is 75% to 95% saturated. The build has 72,779,864
independent rows and no dependency chain, so the same techniques have no wall to hit there.

**The setup comparison rests on an unexplained number.** DuckDB's recorded 0.1 s is below the
1,995 ms physical floor for reading 6,383.8 MB at the measured 3.20 GB/s. The repository holds
no script that shows how it was produced, so the 511x setup gap is an upper bound until
METHOD-065 runs.