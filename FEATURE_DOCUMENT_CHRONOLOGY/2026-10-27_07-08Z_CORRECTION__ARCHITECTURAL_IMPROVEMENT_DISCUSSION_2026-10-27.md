# ARCHITECTURAL_IMPROVEMENT_DISCUSSION_2026-10-27.md

## Timestamp

- Date: 2026-10-27
- Correction recorded: 2026-10-27 12:38 IST
- Original session wall-clock time: not recorded
- Repository: D:\d2
- Source of truth: SKELETON.md

## Purpose

This document records the architectural improvement discussion for ShelfDB. It records the measured failure states, the disposition of each state, the foreign-engine paths that are rejected, and the sheaf-native paths that are accepted.

## Scope

This document does not change code. It records a design discussion and an architecture policy. The policy keeps the cellular sheaf, the fiber bundle, and every layer in ARCHITECTURE.md in place.

## Measured failure states

The table records the five failure states that the review examined. Rows are failure states. Columns give the workload, the opponent time, the ShelfDB time, the measured deficit, the evidence, and the status. The opponent value comes first in every row, and each time cell states its unit.

**Table 1: Measured failure states, with workload and direction**

| Failure state | Workload | Opponent time | ShelfDB time | Measured deficit | Evidence | Status |
|---|---|---|---|---|---|---|
| Durable single-point write | 10,000 single-row durable inserts | SQLite 3: 1.70 us | 6.90 us | 4.06x slower for ShelfDB | `DETAILED_LOSS_ANALYSIS_2026-10-23.md` Table 1. The SQLite value is in `results/sqlite.json` `insert_us` p50. The ShelfDB value has no committed result file. | Holds, permanent |
| Raw join without fiber bundle | 6,001,215 lineitem rows | DuckDB 1.5: 205.00 ms median, 158.30 ms quiet | 2,500.00 ms | 12.20x slower for ShelfDB at the median, 15.79x at the quiet value | `DETAILED_LOSS_ANALYSIS_2026-10-23.md` Table 1 and section 3.2. No committed result file. | Holds, architectural boundary |
| Analytical scan, single accumulator | 6,001,215 lineitem rows | DuckDB 1.5: 5.60 ms | 7.60 ms before fusion, 4.00 ms after fusion | 1.36x slower for ShelfDB before fusion. 1.40x faster for ShelfDB after fusion. | `DETAILED_LOSS_ANALYSIS_2026-10-23.md` Table 1 and section 3.3. `CODE_CHANGES_AND_EXPECTED_GAINS.md` C5 records the step from 4.0 ms to 2.9 ms. | Solved in two steps |
| Cold ingest and setup | 3.6 GB IMDb TSV | DuckDB 1.5 and Redis: 0.10 s to 22.60 s | 51.10 s to 112.72 s | Latency margin 126.11% to 400.00% for ShelfDB | Opponent times: `DETAILED_LOSS_ANALYSIS_2026-10-23.md` Table 1. Build times: `PERFORMANCE_FIX_PLAN_2026-10-22.md` METHOD-056, 51,100 ms and 112,721 ms. | Holds, structural |
| Pure 1D vector slice filter | 100,000 tick rows | Kdb+ 5f: 10.00 us in process, 153.3 us from the client | 48.00 us | Not established | Kdb+ values: `results/KDB_TIMESERIES_RESULTS.json`. The ShelfDB value has no committed result file. | Not established |

Notes on Table 1:

1. The 4.06x value uses the 2026-10-23 SQLite value of 1.70 us. The 2026-10-26 run records 4.8 us for SQLite, which gives 1.44x. The 4.06x is the wider gap of the two runs.
2. The 15.79x value in `ARCHITECTURE.md` uses the 158.30 ms quiet DuckDB value. This table gives the 205.00 ms median as the primary value and names the quiet case beside it.
3. The two Kdb+ values use different timer bases and differ by 15.3x. The in-process timer gives ShelfDB 4.80x slower. The client-observed time gives ShelfDB 3.19x faster. The 48.00 us ShelfDB value has no client-observed pair, so neither ratio is established.
4. The fused-scan row records two steps. Loop fusion takes ShelfDB from 7.60 ms to 4.00 ms. The four-accumulator change takes the TPC-H scan from 4.0 ms to 2.9 ms.

## Disposition of each failure state

Rows are failure states. Columns are the modification required and the architecture action.

**Table 2: Disposition of each failure state**

| Failure state | Modification required | Architecture action |
|---|---|---|
| Durable single-point write | No | Keep epoch WAL for batched writes. Do not chase SQLite on scalar inserts. |
| Raw join without fiber bundle | No | Keep pre-folded fiber bundles. Document as an architectural boundary. |
| Analytical scan, single accumulator | No | Keep the four-accumulator fused scan. |
| Cold ingest and setup | Yes | Add bundle cache, incremental section rewrite, and partial rebuild. |
| Pure 1D vector slice filter | No code change yet | Measure Kdb+ and ShelfDB on one timer basis before any dense-vector work. |

## Foreign-engine paths considered and rejected

The table records a foreign technique for each failure state and the reason that the policy rejects it.

**Table 3: Foreign-engine paths rejected**

| Failure state | Foreign technique | Reason for rejection |
|---|---|---|
| Durable single-point write | Generic group commit and B-tree page writes | Replaces the sheaf write path with a foreign storage engine |
| Raw join without fiber bundle | Relational hash join | Replaces pre-folded connections with a foreign join operator |
| Analytical scan | Unchecked SIMD accumulator tricks | Removes the graded cochain structure from the scan |
| Cold ingest and setup | Generic parallel CSV parser | Treats ingest as text parsing instead of section assembly |
| Pure 1D vector slice filter | Standalone numeric vector engine | Replaces the sheaf model with a foreign vector engine |

## Sheaf-native resolution policy

The cellular sheaf, the fiber bundle, and every layer in ARCHITECTURE.md stay in place. A fix that replaces them with a relational, vector, or key-value engine is rejected, even if it is faster.

Rows are failure states. Columns are the sheaf-native technique and the identity check.

**Table 4: Sheaf-native technique for each failure state**

| Failure state | Sheaf-native technique | Identity preserved |
|---|---|---|
| Durable single-point write | Epoch-stamped fiber section append with local section commit | Yes |
| Raw join without fiber bundle | Fiber product of sheaves over a common base, plus pre-folded boundary complexes and answer fibers | Yes |
| Analytical scan | Graded cochain reduction over partitioned sheaf scans | Yes |
| Cold ingest and setup | Parallel local section assembly and gluing under a frozen epoch | Yes |
| Pure 1D vector slice filter | Restriction of a 0-cochain to an interval subcomplex using dense coordinate vectors | Yes |

## Policy notes

- The raw-join boundary remains. The sheaf does not promise ad-hoc joins on flat tables. It promises pre-folded connections.
- The Kdb+ boundary remains unproven for pure numeric vectors. The measurement must use one timer basis.
- Every accepted technique must be verified on the real code path before it is claimed.
- "Mathematically possible" means the target sits inside a known lower bound. It does not mean the target is measured today.
- Every ratio must state its direction. Write "slower for ShelfDB" or "faster for ShelfDB".
- Every row must name its timer basis. An in-process timer and a client-observed time are different clocks.
- A value with no committed result file is a record, not a measurement. Mark it in the evidence column.

## Theoretical thought experiment (2026-10-27)

This section applies sheaf and fiber-bundle concepts to each failure state. It asks whether a sheaf-native path can reach the opponent, in theory, without changing other gains.

Rows are failure states. Columns are the sheaf concept, the thought experiment, the theoretical outcome, and the non-impact.

**Table 5: Theoretical sheaf-native path for each failure state**

| Failure state | Sheaf concept | Thought experiment | Theoretical outcome | Non-impact |
|---|---|---|---|---|
| Durable single-point write | Local section and epoch stamp | A write appends to a local fiber section. The CRC covers the whole section. The WAL frame is per section. Cost per write tends to one cache-line store plus one branch as the section grows. | Can approach hardware store latency | Read path and query results stay unchanged |
| Raw join without fiber bundle | Fiber product of sheaves | Index each sheaf by restriction to the key base with TIFI. The join is the intersection of sorted key fibers. Cost tends to input size plus output size. | Can reach linear time in theory | Pre-folding stays the default path |
| Analytical scan | Graded cochain reduction | Each dimension level reduces in parallel. Accumulators are stalks of a reduction sheaf. Cost tends to bytes divided by bandwidth. | Can reach memory bandwidth | Existing scan gates stay green |
| Cold ingest and setup | Gluing of local sections | Each byte range is a local section. Assembly is parallel and gluing is associative for disjoint keys. Cost tends to input bytes divided by disk bandwidth. | Can reach disk bandwidth | Bundle format and query gates stay unchanged |
| Pure 1D vector slice filter | Restriction of a 0-cochain | Time is a totally ordered poset. The slice is restriction to an interval. Dense coordinate vectors remove cell-header overhead. Cost tends to n times load plus compare, divided by bandwidth. | Can remove the cell-header overhead in theory. The 4.80x ratio is not established. | Sheaf model and other workloads stay unchanged |

### Literature concepts used

- Sheaf: a structure that attaches local data to open sets and glues them into global data.
- Fiber bundle: a space that looks locally like a product of a base and a fiber.
- Fiber product: the categorical pullback of two sheaves over a common base.
- Restriction map: the map from a global section to a local section.
- Graded cochain: data attached to cells of a given dimension.
- Local section: data on one open set before gluing.
- Gluing: the assembly of local sections into a global section.
- Epoch stamp: a version marker that makes a build reproducible and rejectable.

### Theoretical conclusion

Each of the four established failure states has a sheaf-native path that can reach the opponent in theory. The paths do not replace the sheaf with a foreign engine. They use operations that the sheaf already defines: section, restriction, fiber product, gluing, and cochain reduction.

The Kdb+ path stays a hypothesis. The ratio that motivates it is not established.

The paths are thought experiments. They need measurement on the real code path before they can be claimed.

## Conceptual status (2026-10-27)

The four established failure states are conceptually solved by sheaf-native operations. The operations are section, restriction, fiber product, gluing, and graded cochain reduction. The solutions do not replace the sheaf with a foreign engine.

The Kdb+ failure state is not established. The remaining gap is a measurement gap, not a design gap.

The solutions remain theoretical. They need measurement on the real code path before they can be claimed as gains.

## Research review (2026-10-27)

This section records the research reviewed for a mathematical framework that can improve the failure states without changing other gains.

Rows are sources. Columns are the year and the finding.

**Table 6: Reviewed sources**

| Source | Year | Finding |
|---|---|---|
| Ayzenberg et al., "Sheaf theory: from deep geometry to deep learning" (arXiv 2502.15476) | 2025 | Surveys sheaf theory in data science and computer science. It supports local-to-global data modeling. Link: https://arxiv.org/html/2502.15476v1 |
| Hansen and Ghrist, "Sheaf Neural Networks" (arXiv 2012.06333) | 2020 | Builds neural networks on the sheaf Laplacian. It supports heterophilic and signed relations. Link: https://www.alphaxiv.org/abs/2012.06333 |
| Barbero et al., "Sheaf Neural Networks with Connection Laplacians" (PMLR) | 2022 | Extends sheaf neural networks with connection Laplacians. It supports richer relational structure. Link: https://proceedings.mlr.press/v196/barbero22a/barbero22a.pdf |
| Wei and Wei, "Persistent sheaf Laplacians" (arXiv 2112.10906) | 2021 | Fuses data types across scales with sheaf Laplacians. It supports multiscale analysis. Link: https://arxiv.org/abs/2112.10906 |
| Mansourbeigi, "Sheaf Theory Approach to Distributed Applications" (USU) | 2017 | Models distributed applications with sheaves. It supports local-to-global consistency. Link: https://digitalcommons.usu.edu/cgi/viewcontent.cgi?article=1006&context=computer_science_stures |
| "Fiber Bundle Learning: A Topological Framework" (MDPI) | 2023 | Models datasets as fiber bundles. It supports feature invariants. Link: https://www.mdpi.com/2813-9542/3/2/12/pdf |
| Hernández and Sánchez-Soto, "The Sheaf Laplacian: A Topological Framework for Data Fusion and Consensus in Distributed Sensing Networks" (arXiv 2606.19529) | 2026 | Applies the sheaf Laplacian to data fusion and consensus in distributed sensing. It supports structured multi-modal data. Link: https://arxiv.org/html/2606.19529v1 |
| Phillips, "Going Beyond the Data as the Patching (Sheaving) of Local..." (PMC) | 2018 | Uses sheaf patching for data integration. It supports local-to-global assembly. Link: https://pmc.ncbi.nlm.nih.gov/articles/PMC6189483/ |

### Conclusion of the review

No single reviewed paper proves a performance gain for the five failure states. The reviewed work supports the existing sheaf model. It supports local-to-global consistency, multiscale analysis, and structured data integration. It does not support replacing the sheaf with a foreign engine.

The accepted framework remains the cellular sheaf and the fiber bundle. Improvements must be expressed as sheaf operations. Each improvement must be measured on the real code path before it is claimed.

## Correction record (2026-10-27)

This section records the corrections to the first version of this document.

1. The Kdb+ row gave only the 10.00 us in-process value and claimed a 4.80x loss. It now gives both timer bases and marks the ratio as not established.
2. The cold-ingest row gave only the ShelfDB build time. It now gives the opponent time, 0.10 s to 22.60 s, and the percentage deficit, 126.11% to 400.00%.
3. The analytical-scan row merged the pre-fusion loss and the post-fusion gain into one pair. It now separates the 7.60 ms loss from the 4.00 ms gain.
4. The raw-join row used the 158.30 ms quiet value and called the deficit 15.79x. It now gives the 205.00 ms median as the primary value and names the quiet case.
5. Every row now gives the opponent value first, the workload, the evidence, and the direction of the ratio.
6. Every table now has a title, and the lead-in states what the rows and the columns are.

## Evidence

- ARCHITECTURE.md, section "Failure-state disposition (2026-10-27)"
- ARCHITECTURE.md, section "Foreign-engine paths considered and rejected (2026-10-27)"
- ARCHITECTURE.md, section "Sheaf-native resolution policy (2026-10-27)"
- DETAILED_LOSS_ANALYSIS_2026-10-23.md, Table 1 and sections 3.1 to 3.4
- CODE_CHANGES_AND_EXPECTED_GAINS.md, change C5
- PERFORMANCE_FIX_PLAN_2026-10-22.md, section 9 and METHOD-056
- SKELETON.md, traceability chain for REQ-057 and REQ-058
- sheaf-db/tests/e2e/realtime_bench/results/sqlite.json, `insert_us` p50
- results/KDB_TIMESERIES_RESULTS.json, in-process and client-observed slice times
- arXiv 2606.19529, checked on 2026-10-27. The title and the authors match Table 6.

Four ShelfDB values in Table 1 have no committed result file: 6.90 us, 2,500.00 ms, 4.00 ms, and 48.00 us. `DETAILED_LOSS_ANALYSIS_2026-10-23.md` records them, and `CODE_CHANGES_AND_EXPECTED_GAINS.md` records the 4.00 ms value. The build times are in METHOD-056.

The next measurement must write each of those values to a tracked result file. The table marks them until then.
