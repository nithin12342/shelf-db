# Plan: IMDb Q2 Cypher, Neo4j versus ShelfDB

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Result

This plan states the IMDb Q2 Cypher pair. Neo4j runs the exact Cypher text recorded in the approved documents. ShelfDB reads its stored IMDb Q2 answer section. The two output shapes differ, so this plan compares cost and records both results.

## 2. Terms

- **Cypher:** the query language for Neo4j.
- **Topological Query Language (TQL):** a declarative query over a cellular complex.
- **Answer section:** a stored result that the ShelfDB bundle build writes once.
- **Gate:** a condition that a real result must meet.

## 3. Original benchmark link and dataset

Table 1 answers this question: which original links belong to this pair. Rows are links. Columns are the role and the address.

| Role | Address |
|---|---|
| IMDb data set | `https://datasets.imdbws.com/` |
| Native comparison record | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 4, case 2 |

The IMDb dump is a manual download. `DATASETS.md` row 6 records the seven `title.*.tsv.gz` files and `name.basics.tsv.gz`. The recorded local path is `D:\d2\data\`.

Table 2 answers this question: which data set and subset this pair uses. Rows are systems. Columns are the system, the subset, and the source.

| System | Subset | Source |
|---|---|---|
| Neo4j | Person and Title nodes with PRINCIPAL relationships from the IMDb name and title tables | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 4 |
| ShelfDB | The IMDb fiber bundle. Section `q2edges` holds actor credits. `ADVERSARIAL_ANALYSIS.md` records 6,386,000 edges for the measured bundle. | `ADVERSARIAL_ANALYSIS.md`, section 5.1 |

Extract only the files named in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, Table 1. Do not extract the full bundle.

## 4. Test cases and exact query text

Table 3 answers this question: which documents name this test case. Rows are documents. Columns are the document, the exact path, and the specific case.

| Document | Exact path | Specific case |
|---|---|---|
| Live application comparison | `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 4, Table 1, case 2 native query. Section 17 evidence row for `imdb_multi_engine_bench.py` |
| Recursive test query index | `recursive test/QUERIES_2026-11-22_15-04-04.md` | Section 3, the IMDb Q2 Cypher text |
| Consolidated query text | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | `[CASE 2: native Cypher]` |
| Adversarial analysis | `ADVERSARIAL_ANALYSIS.md` | Section 5.3, IMDb Q2 complex query |

The exact Cypher text is this statement.

```cypher
MATCH (p:Person)<-[:PRINCIPAL]-(t:Title) WITH p, count(t) AS d WHERE d > 1 RETURN sum(d*(d-1)) AS pairs
```

Do not change the text. Do not add a clause. `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 15, states that TQL cannot express Q2 today. The ShelfDB result is a stored Q2 answer section, not a Cypher pairs result.

## 5. Container image and pull step

The measured engine version is Neo4j 5.26.32, recorded in `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 5. The plan image is `neo4j:5-community`, recorded in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4.

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
| Original benchmark command | `py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| Matching ShelfDB command | `cargo run -p sheaf-engine --bin imdb_fiber_bench --release` | `ADVERSARIAL_ANALYSIS.md`, section 11 |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the real query time, the load time, and the result checksum for each pass.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file and the content.

| File | Content |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/neo4j.json` | Recorded Neo4j Cypher result and timing |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_multi_engine.json` | Multi-engine result and gate strings |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_shelf.json` | ShelfDB Q2 answer read and oracle timing |
| `EVIDENCE_2026-11-28_14-38-34/environment.json` | Host, image digest, and subset |
| `EVIDENCE_2026-11-28_14-38-34/query_1.txt` | First measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_2.txt` | Second measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_3.txt` | Third measured pass |
| `EVIDENCE_2026-11-28_14-38-34/checksums.txt` | Result checksums |
| `EVIDENCE_2026-11-28_14-38-34/shelfdb_output.txt` | ShelfDB output and gate strings |
| `EVIDENCE_2026-11-28_14-38-34/summary.md` | Provenance and gate result |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the required evidence, and the outcome.

| Condition | Required evidence | Outcome |
|---|---|---|
| One container ran at a time | Container inspect output | PASS when met, FAIL when not met |
| Resource limits held | Inspect output with one central processing unit and two gigabyte | PASS when met, FAIL when not met |
| Row count matches the subset manifest | `subset_manifest.json` and `load_output.txt` | PASS when met, FAIL when not met |
| Neo4j result matches the recorded value | `checksums.txt` and `neo4j.json` | PASS when met, FAIL when not met |
| ShelfDB Q2 answer matches its scan oracle | `shelfdb_output.txt` and `imdb_shelf.json` | PASS when met, FAIL when not met |
| Three measured passes exist | `query_1.txt`, `query_2.txt`, `query_3.txt` | PASS when met, FAIL when not met |

A pair is complete only when every row has outcome PASS. Report `unsupported` with the named cause when the IMDb subset or the image is absent. Do not report a fake result.

## Warnings

- **WARNING:** Do not run Docker, a release build, and the benchmark at the same time. The laptop can stall.
- **CAUTION:** The Cypher pairs result and the ShelfDB Q2 answer section have different output shapes. Record both. Do not present them as the same answer.
- **CAUTION:** `RUNTIME_COMPARISON.md` section 7 marks the DuckDB setup figure as an upper bound. Do not quote it as a measured load time.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
