# Plan: SNAP LiveJournal, Neo4j versus ShelfDB

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Result

This plan states the SNAP LiveJournal pair. The master data index names LiveJournal as a graph source. The approved documents give no test case, no Cypher text, and no measured result for it. This plan stays at name level and reports `unsupported` when approval or data is absent.

## 2. Terms

- **LiveJournal graph:** a social network edge list published by Stanford Network Analysis Project (SNAP).
- **Cypher:** the query language for Neo4j.
- **Gate:** a condition that a real result must meet.
- **Unsupported:** an explicit result that states a missing tool, data set, or approval.

## 3. Original benchmark link and dataset

Table 1 answers this question: which original links belong to this pair. Rows are links. Columns are the role and the address.

| Role | Address |
|---|---|
| SNAP LiveJournal data set | `https://snap.stanford.edu/data/soc-LiveJournal1.html` |
| Graph source record | `BENCHMARK_SOURCES.md`, row 6 |

Table 2 answers this question: which data set and subset this pair uses. Rows are items. Columns are the item, the recorded value, and the source.

| Item | Recorded value | Source |
|---|---|---|
| Bundle path | `graph_production/soc-LiveJournal1.txt.gz` | `DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, Table 1 |
| Archive form | Compressed graph file. Size not recorded in the approved sources. | Same table |
| Extraction state | Deferred until approval | Same table |
| Data family | Graph work with SNAP Facebook, LiveJournal, and LDBC | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md`, section 2, item 7 |

Do not extract this file before approval. Do not guess a size. Record the real file size and checksum in the evidence folder after extraction.

## 4. Test cases and exact query text

Table 3 answers this question: which documents name this test case. Rows are documents. Columns are the document, the exact path, and the specific case.

| Document | Exact path | Specific case |
|---|---|---|
| Master index | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/MASTER_INDEX_2026-11-28_14-38-34.md` | Section 4, Table 3, SNAP LiveJournal row |
| Dataset usage sequence | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md` | Table 1, production graph row |
| Dataset bundle investigation | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md` | Section 2, item 7. Names LiveJournal as a graph family only. |
| Benchmark source index | `BENCHMARK_SOURCES.md` | Row 6. Names graph sources for Neo4j. |

The approved documents give the graph name only. They give no specific test case and no exact Cypher text. Do not invent a test case. Do not invent Cypher. The operator must define the case in the evidence folder before a run.

## 5. Container image and pull step

The plan image is `neo4j:5-community`, recorded in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4. Do not pull the image before approval of the data extraction.

Run this pull step when the operator approves the pair.

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
| Load or build step | stop after 30 minute | Prevents a hot laptop. LiveJournal is large. |
| Cooldown after the pair | 3 minute | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which command runs each side of this pair. Rows are roles. Columns are the role, the command, and the status.

| Role | Command | Status |
|---|---|---|
| Original benchmark command | Not recorded in the approved sources. The operator must write the real load and query commands into the evidence folder. | Absent |
| Container run shape | `docker run --rm --name bench-neo4j --cpus=1 --memory=2g --pids-limit=128 -p 7474:7474 -p 7687:7687 neo4j:5-community` | From the execution plan |
| Matching ShelfDB command | Not recorded in the approved sources. Do not use `graph_bench` as a claim of equivalence. | Absent |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the result count, the timing, and the checksum for each pass.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file and the content.

| File | Content |
|---|---|
| `EVIDENCE_2026-11-28_14-38-34/environment.json` | Host, image digest, subset, and approval record |
| `EVIDENCE_2026-11-28_14-38-34/subset_manifest.json` | Real file size, row count, and checksum |
| `EVIDENCE_2026-11-28_14-38-34/load_output.txt` | Real Neo4j load output |
| `EVIDENCE_2026-11-28_14-38-34/query_definition.txt` | The test case that the operator defines |
| `EVIDENCE_2026-11-28_14-38-34/query_1.txt` | First measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_2.txt` | Second measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_3.txt` | Third measured pass |
| `EVIDENCE_2026-11-28_14-38-34/checksums.txt` | Result checksums |
| `EVIDENCE_2026-11-28_14-38-34/shelfdb_output.txt` | ShelfDB output when a matching command exists |
| `EVIDENCE_2026-11-28_14-38-34/summary.md` | Provenance and gate result |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the required evidence, and the outcome.

| Condition | Required evidence | Outcome |
|---|---|---|
| Data extraction has approval | `environment.json` | PASS when met, `unsupported` when not met |
| One container ran at a time | Container inspect output | PASS when met, FAIL when not met |
| Resource limits held | Inspect output with one central processing unit and two gigabyte | PASS when met, FAIL when not met |
| Loaded row count matches the manifest | `load_output.txt` and `subset_manifest.json` | PASS when met, FAIL when not met |
| The operator defined a test case | `query_definition.txt` | PASS when met, `unsupported` when not met |
| Result checksum matches across three passes | `checksums.txt` | PASS when met, FAIL when not met |
| ShelfDB side is present or labeled absent | `shelfdb_output.txt` and `summary.md` | PASS when met, `unsupported` when not met |

A pair is complete only when every row has outcome PASS. When approval, data, a test case, or a matching command is absent, report `unsupported` with the named cause. Do not report a fake result.

## Warnings

- **WARNING:** Do not run Docker, a release build, and the benchmark at the same time. The laptop can stall.
- **CAUTION:** `DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md` defers LiveJournal until approval. Do not extract it early.
- **CAUTION:** The approved documents give no test case. A timing without a stated case is not evidence.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
