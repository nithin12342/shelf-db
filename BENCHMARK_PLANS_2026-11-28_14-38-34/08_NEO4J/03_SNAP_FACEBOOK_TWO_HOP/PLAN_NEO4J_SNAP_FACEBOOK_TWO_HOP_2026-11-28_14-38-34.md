# Plan: SNAP Facebook two-hop, Neo4j versus ShelfDB

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Result

This plan states the SNAP Facebook two-hop pair. Neo4j loads the extracted Facebook edge list. The approved documents do not record a ShelfDB command for this graph. This plan records that gap and does not claim a match.

## 2. Terms

- **Two-hop:** a traversal that reaches a node through exactly two relationships.
- **Cypher:** the query language for Neo4j.
- **Edge list:** a text file with one pair of node identifiers on each line.
- **Gate:** a condition that a real result must meet.

## 3. Original benchmark link and dataset

Table 1 answers this question: which original links belong to this pair. Rows are links. Columns are the role and the address.

| Role | Address |
|---|---|
| SNAP Facebook data set | `https://snap.stanford.edu/data/ego-Facebook.html` |
| Alternate link in the master index | `https://snap.stanford.edu/data/egonets-Facebook.html` |
| Graph source record | `BENCHMARK_SOURCES.md`, row 6 |

Table 2 answers this question: which data set and subset this pair uses. Rows are items. Columns are the item, the recorded value, and the source.

| Item | Recorded value | Source |
|---|---|---|
| Archive | `D:\d2\data\facebook_combined.txt.gz`, 0.0002 gigabyte | `DATASETS.md`, row 2 |
| Extracted edge list | `D:\bench\datasets\facebook_combined.txt`, 0.0009 gigabyte | `DATASETS.md`, row 2b |
| Nodes | 4,039 | `DATASETS.md`, row 2b |
| Edges | 88,234 | `DATASETS.md`, row 2b |
| Bundle path | `graph/facebook_combined.txt.gz` | `DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, Table 1 |

Extract only this file from `shelf_db_all_datasets.zip`. Do not extract the full bundle.

## 4. Test cases and exact query text

Table 3 answers this question: which documents name this test case. Rows are documents. Columns are the document, the exact path, and the specific case.

| Document | Exact path | Specific case |
|---|---|---|
| Data set index | `DATASETS.md` | Row 2, used by Facebook two-hop and triangle tests |
| Benchmark source index | `BENCHMARK_SOURCES.md` | Row 6, Facebook edges for the graph engine, and the local data file row |
| Execution plan | `BENCHMARK_EXECUTION_PLAN_2026-11-28_14-19-54.md` | Section 4, item 7. Section 6, Table 2, row 7: Neo4j Facebook two-hop |
| Master index | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/MASTER_INDEX_2026-11-28_14-38-34.md` | Section 4, Table 3, SNAP Facebook row |
| Dataset bundle investigation | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md` | Section 2, item 7: graph work with SNAP Facebook |

The specific test case is the Facebook two-hop test. The triangle test is also named in `DATASETS.md` row 2. This plan does not run the triangle test.

The approved documents give a workload name only. They give no exact Cypher text for this case. Do not invent Cypher. Record the real command that the operator runs in the evidence folder.

## 5. Container image and pull step

The plan image is `neo4j:5-community`, recorded in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4. The recorded live engine version is Neo4j 5.26.32, recorded in `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md`, section 2.

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

Table 5 answers this question: which command runs each side of this pair. Rows are roles. Columns are the role, the command, and the status.

| Role | Command | Status |
|---|---|---|
| Original benchmark command | Not recorded in the approved sources. Use the workload name `FB 2-hop` from `DATASETS.md` row 2 and record the real command at run time. | Name only |
| Container run shape | `docker run --rm --name bench-neo4j --cpus=1 --memory=2g --pids-limit=128 -p 7474:7474 -p 7687:7687 neo4j:5-community` | From the execution plan |
| Matching ShelfDB command | Not recorded. A proposed shape is `cargo run -p sheaf-engine --bin graph_bench --release` on a graph built from this edge list. The operator must build that graph and record it. | Proposed, not measured |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the two-hop result count, the timing, and the checksum for each pass.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file and the content.

| File | Content |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/neo4j.json` | Existing Neo4j records for other cases. No Facebook row exists today. |
| `EVIDENCE_2026-11-28_14-38-34/environment.json` | Host, image digest, and subset |
| `EVIDENCE_2026-11-28_14-38-34/subset_manifest.json` | Node count, edge count, and file checksum |
| `EVIDENCE_2026-11-28_14-38-34/load_output.txt` | Real Neo4j load output |
| `EVIDENCE_2026-11-28_14-38-34/query_1.txt` | First measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_2.txt` | Second measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_3.txt` | Third measured pass |
| `EVIDENCE_2026-11-28_14-38-34/checksums.txt` | Two-hop result checksums |
| `EVIDENCE_2026-11-28_14-38-34/shelfdb_output.txt` | ShelfDB output when an operator creates the command |
| `EVIDENCE_2026-11-28_14-38-34/summary.md` | Provenance and gate result |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the required evidence, and the outcome.

| Condition | Required evidence | Outcome |
|---|---|---|
| One container ran at a time | Container inspect output | PASS when met, FAIL when not met |
| Resource limits held | Inspect output with one central processing unit and two gigabyte | PASS when met, FAIL when not met |
| Loaded node count equals 4,039 | `load_output.txt` and `subset_manifest.json` | PASS when met, FAIL when not met |
| Loaded edge count equals 88,234 | `load_output.txt` and `subset_manifest.json` | PASS when met, FAIL when not met |
| Two-hop checksum matches across three passes | `checksums.txt` | PASS when met, FAIL when not met |
| ShelfDB side is labeled proposed or recorded | `shelfdb_output.txt` and `summary.md` | PASS when met, FAIL when not met |

A pair is complete only when every row has outcome PASS. Report `unsupported` with the named cause when the edge list or the image is absent. Do not report a fake result. Do not claim a ShelfDB match when no ShelfDB command exists.

## Warnings

- **WARNING:** Do not run Docker, a release build, and the benchmark at the same time. The laptop can stall.
- **CAUTION:** `DATASETS.md` gives two different source addresses for the Facebook graph. Use the address in row 2 first. Record the address that you used.
- **CAUTION:** The approved sources give no Cypher text and no ShelfDB command for this case. Do not invent either one.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
