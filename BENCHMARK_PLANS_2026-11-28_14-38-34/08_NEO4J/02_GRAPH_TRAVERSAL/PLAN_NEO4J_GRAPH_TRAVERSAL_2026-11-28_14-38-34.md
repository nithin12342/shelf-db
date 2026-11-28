# Plan: graph traversal, Neo4j versus ShelfDB

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Result

This plan states the graph traversal pair. Neo4j traverses its derived graph. ShelfDB runs `graph_bench` on its in-memory graph. The two systems hold different graphs, so this plan compares the named workload and the recorded ratios.

## 2. Terms

- **Traversal:** a walk that follows relationships from a node.
- **Path:** one sequence of nodes and relationships in a traversal.
- **Cypher:** the query language for Neo4j.
- **Gate:** a condition that a real result must meet.

## 3. Original benchmark link and dataset

Table 1 answers this question: which original links belong to this pair. Rows are links. Columns are the role and the address.

| Role | Address |
|---|---|
| Source data for the derived graph | `https://www.tpc.org/tpch/` |
| Other recorded graph benchmark source | `https://graphalytics.org/` |
| Live measurement record | `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md`, section 2 |

The approved documents name no external graph suite for this exact traversal. Do not claim that Graphalytics ran. The TPC-H kit is recorded in `DATASETS.md` row 4 and `BENCHMARK_SOURCES.md` row 1.

Table 2 answers this question: which data set and subset this pair uses. Rows are systems. Columns are the system, the subset, and the source.

| System | Subset | Source |
|---|---|---|
| Neo4j | TPC-H derived graph with 150,000 customers, 1,500,000 orders, and 600,000 lines. Workload of 600,000 paths and 200 two-hop samples. | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 8 |
| ShelfDB | In-memory graph of 4,350,000 cells in 151.4 megabyte | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 8 |

Generate scale factor 0.01 first. Use scale factor 1 only after the small run passes. Do not generate more than the named subset.

## 4. Test cases and exact query text

Table 3 answers this question: which documents name this test case. Rows are documents. Columns are the document, the exact path, and the specific case.

| Document | Exact path | Specific case |
|---|---|---|
| Live application comparison | `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 8, Table 5: full traversal and two-hop median |
| Runtime comparison | `RUNTIME_COMPARISON.md` | Section 3: graph full traversal. Section 4: graph traversal row. Section 5: Neo4j slice caveat |
| Live benchmark record | `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md` | Table 1: 600,000-path traversal and two-hop neighbor traversal |
| Adversarial analysis | `ADVERSARIAL_ANALYSIS.md` | Section 6.3: graph traversal over 600,000 paths |

The approved documents give workload names for this case. They give no exact Cypher text for the traversal in these sources. The names are these.

- Full traversal over 600,000 paths.
- Two-hop neighbor traversal over 200 samples.
- Point lookup over 5,000 keys.

Do not invent Cypher text. Do not write a query that the source documents do not give. Record the real command that the operator runs in the evidence folder.

## 5. Container image and pull step

The measured engine version is Neo4j 5.26.32, recorded in `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md`, section 2. The plan image is `neo4j:5-community`, recorded in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4.

Run this pull step before the container starts.

```text
docker pull neo4j:5-community
```

Start one container with this command.

```text
docker run --rm --name bench-neo4j --cpus=1 --memory=2g --pids-limit=128 -p 7474:7474 -p 7687:7687 neo4j:5-community
```

## 6. Low-power limits

Table 4 answers this question: which limits apply to this pair. Rows are limits. Columns are the limit, the allowed value, and the reason.

| Limit | Allowed value | Reason |
|---|---|---|
| Concurrent containers | one container | Keeps the laptop responsive |
| Container central processing units | one central processing unit | Leaves capacity for the host system |
| Container memory | two gigabyte | Avoids swap and laptop stutter |
| Benchmark threads | one thread | Makes timings comparable |
| Load or build step | stop after 30 minute | Prevents a hot laptop |
| Cooldown after the pair | 3 minute | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which command runs each side of this pair. Rows are roles. Columns are the role, the command, and the source.

| Role | Command | Source |
|---|---|---|
| Original benchmark command | `py run_all.py --keep-up` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 14 and section 17 |
| Matching ShelfDB command | `cargo run -p sheaf-engine --bin graph_bench --release` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 8 and section 17 |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the traversal time, the path count, and the result checksum for each pass.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file and the content.

| File | Content |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/neo4j.json` | Neo4j traversal time, path count, and gate |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026_quiet.json` | ShelfDB quiet-pass graph result |
| `sheaf-db/tests/e2e/realtime_bench/results/shelf_bins_20261026.json` | ShelfDB shared-pass graph result |
| `EVIDENCE_2026-11-28_14-38-34/environment.json` | Host, image digest, and subset |
| `EVIDENCE_2026-11-28_14-38-34/load_output.txt` | Graph load output and row counts |
| `EVIDENCE_2026-11-28_14-38-34/query_1.txt` | First measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_2.txt` | Second measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_3.txt` | Third measured pass |
| `EVIDENCE_2026-11-28_14-38-34/checksums.txt` | Path counts and checksums |
| `EVIDENCE_2026-11-28_14-38-34/shelfdb_output.txt` | ShelfDB gate strings |
| `EVIDENCE_2026-11-28_14-38-34/summary.md` | Provenance and gate result |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the required evidence, and the outcome.

| Condition | Required evidence | Outcome |
|---|---|---|
| One container ran at a time | Container inspect output | PASS when met, FAIL when not met |
| Resource limits held | Inspect output with one central processing unit and two gigabyte | PASS when met, FAIL when not met |
| Path count equals 600,000 for the full traversal | `checksums.txt` and `neo4j.json` | PASS when met, FAIL when not met |
| Two-hop sample count equals 200 | `checksums.txt` | PASS when met, FAIL when not met |
| ShelfDB gate `gate_traverse` equals PASS | `shelfdb_output.txt` | PASS when met, FAIL when not met |
| Three measured passes exist | `query_1.txt`, `query_2.txt`, `query_3.txt` | PASS when met, FAIL when not met |

A pair is complete only when every row has outcome PASS. Report `unsupported` with the named cause when the graph subset or the image is absent. Do not report a fake result.

## Warnings

- **WARNING:** Do not run Docker, a release build, and the benchmark at the same time. The laptop can stall.
- **CAUTION:** The two systems hold different graphs. Compare the ratio, not the absolute graph size.
- **CAUTION:** `CONVERSATION_LOG_2026-10-21.md` records an invalid traversal with zero paths after a label-agnostic delete. Make sure that the path count is greater than zero.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
