# Plan: LDBC SNB interactive, Neo4j versus ShelfDB

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Result

This plan states the LDBC Social Network Benchmark (SNB) Interactive pair. Neo4j runs the reference Cypher implementation. The approved documents do not record a ShelfDB LDBC implementation. This plan records that gap and gives the driver commands from the attached bundle README files.

## 2. Terms

- **LDBC SNB:** the Social Network Benchmark of the Linked Data Benchmark Council.
- **Interactive workload:** the read and update workload of LDBC SNB. It holds 22 read queries and 7 update queries.
- **Cypher:** the query language for Neo4j.
- **Time compression ratio:** the driver target value for an audited run. The driver goal is the lowest value that keeps the schedule audit.
- **Gate:** a condition that a real result must meet.

## 3. Original benchmark link and dataset

Table 1 answers this question: which original links belong to this pair. Rows are links. Columns are the role and the address.

| Role | Address |
|---|---|
| LDBC SNB Interactive implementations | `https://github.com/ldbc/ldbc_snb_interactive_v1_impls` |
| LDBC SNB specification | `https://ldbcouncil.org/benchmarks/snb/` |
| Pre-generated data sets | `https://hdl.handle.net/11112/e6e00558-a2c3-9214-473e-04a16de09bf8` |
| SNB Datagen | `https://github.com/ldbc/ldbc_snb_datagen_hadoop` |

Table 2 answers this question: which data set and subset this pair uses. Rows are items. Columns are the item, the recorded value, and the source.

| Item | Recorded value | Source |
|---|---|---|
| Neo4j data set | `social_network-csv_composite-longdateformatter-sf*`, from the pre-generated repository | Cypher README, section "Using pre-generated data sets" |
| Local generator archive | `D:\d2\data\ldbc_snb_datagen_spark-main.zip`, 0.029 gigabyte | `DATASETS.md`, row 7 |
| Small test data | `cypher/test-data/` | Main README, section "Test data set" |
| First subset | Small test data set. Do not use scale factor 100. | Main README and low-power limits |
| Serializer | `CsvComposite` with `LongDateFormatter` | Cypher README, section "Generating the data set" |

The main README records that scale factor 100 needs 24 gigabyte and about 4 hour to generate on one machine. Do not generate that size on this laptop.

## 4. Test cases and exact query text

Table 3 answers this question: which documents name this test case. Rows are documents. Columns are the document, the exact path, and the specific case.

| Document | Exact path | Specific case |
|---|---|---|
| Attached main README | `attached_bundle_review_2026-11-28_14-19-54/docs/ldbc_snb_interactive_v1_impls-main__ldbc_snb_interactive_v1_impls-main_README.md` | Section "Driver modes": create validation parameters, validate, benchmark. Section "Developer's guide": 22 reads and 7 updates. |
| Attached Cypher README | `attached_bundle_review_2026-11-28_14-19-54/docs/ldbc_snb_interactive_v1_impls-main__ldbc_snb_interactive_v1_impls-main_cypher_README.md` | Sections "Loading the data set", "Create validation parameters", "Validate", "Benchmark", and "Reload between runs" |
| Master index | `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/MASTER_INDEX_2026-11-28_14-38-34.md` | Section 4, Table 3, LDBC SNB row |
| Dataset bundle investigation | `DATASET_BUNDLE_INVESTIGATION_2026-11-28_14-19-54.md` | Section 4, item 5, and the warning on persistent updates |

The attached README files name the query families. They do not print the exact Cypher text. The names are these.

- 22 read queries. The set holds complex reads and short reads.
- 7 update queries.
- Three driver modes: create validation parameters, validate, benchmark.
- Backup and restore scripts around each run.

Do not invent Cypher text. Do not copy query text from a source outside the attached README files. When the operator runs a query, record the real text in the evidence folder.

## 5. Container image and pull step

The plan image is `neo4j:5-community`, recorded in `BENCHMARK_PLANS_2026-11-28_14-38-34/00_INDEX/DATASET_USAGE_AND_EXECUTION_SEQUENCE_2026-11-28_14-38-34.md`, section 4. The Cypher README states that the Neo4j version is set in `cypher/scripts/vars.sh`. The recorded live engine version in this repository is Neo4j 5.26.32, recorded in `LIVE_SEVEN_DATABASE_BENCHMARK_AND_LOSSES_2026-10-23.md`, section 2.

The Cypher README requires Bash, Java 8, Docker 19 or later, and free space in `NEO4J_CONTAINER_ROOT`. The main README states that the project uses Java 11. Make sure that both toolchains are present before the run.

Run these steps in order.

```text
git clone https://github.com/ldbc/ldbc_snb_interactive_v1_impls
cd ldbc_snb_interactive_v1_impls
scripts/build.sh
docker pull neo4j:5-community
```

Use this Maven build only when the operator builds a subset.

```text
mvn clean package -DskipTests -Pcypher,postgres
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
| Warmup duration | short smoke run | The audited 30 minute warmup is out of scope on this laptop |
| Load or build step | stop after 30 minute | Prevents a hot laptop |
| Cooldown after the pair | 3 minute | Lets the processor settle |

The main README recommends multi-threaded benchmark runs. Do not use that mode here. Use one thread to keep the laptop responsive and the timings comparable.

## 7. Commands

Table 5 answers this question: which command runs each side of this pair. Rows are roles. Columns are the role, the command, and the source.

| Role | Command | Source |
|---|---|---|
| Build the project | `scripts/build.sh` | Main README, section "Building the project" |
| Set the data paths | `export NEO4J_VANILLA_CSV_DIR=`pwd`/test-data/vanilla` and `export NEO4J_CONVERTED_CSV_DIR=`pwd`/test-data/converted` | Cypher README, section "Running the benchmark" |
| Load the data | `scripts/load-in-one-step.sh` | Cypher README, section "Loading the data set" |
| Original benchmark command, validation parameters | `driver/create-validation-parameters.sh` | Main README, driver mode 1, and Cypher README |
| Original benchmark command, validate | `driver/validate.sh` | Main README, driver mode 2, and Cypher README |
| Original benchmark command, benchmark | `driver/benchmark.sh` | Main README, driver mode 3, and Cypher README |
| Restore before each run | `scripts/backup-database.sh` then `scripts/restore-database.sh` | Main README, section "Notes", and Cypher README |
| Optional native dump path | `scripts/backup-neo4j.sh` then `scripts/restore-neo4j.sh` | Cypher README, section "Reload between runs" |
| Matching ShelfDB command | Not recorded in the approved sources. `graph_bench` is a shape-level smoke test only and is not an LDBC implementation. | Gap recorded |

Run one warm-up pass. Do not time it. Run three measured passes with one thread. Capture the validation result, the query count, the update count, and the checksum for each pass.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence for this pair. Rows are files. Columns are the file and the content.

| File | Content |
|---|---|
| `EVIDENCE_2026-11-28_14-38-34/environment.json` | Host, image digest, Java versions, and subset |
| `EVIDENCE_2026-11-28_14-38-34/subset_manifest.json` | Data set name, scale factor, row counts, and checksum |
| `EVIDENCE_2026-11-28_14-38-34/load_output.txt` | Real output of `scripts/load-in-one-step.sh` |
| `EVIDENCE_2026-11-28_14-38-34/query_definition.txt` | Real Cypher text recorded by the operator |
| `EVIDENCE_2026-11-28_14-38-34/query_1.txt` | First measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_2.txt` | Second measured pass |
| `EVIDENCE_2026-11-28_14-38-34/query_3.txt` | Third measured pass |
| `EVIDENCE_2026-11-28_14-38-34/checksums.txt` | Validation result and result checksums |
| `EVIDENCE_2026-11-28_14-38-34/restore_output.txt` | Real backup and restore output |
| `EVIDENCE_2026-11-28_14-38-34/shelfdb_output.txt` | ShelfDB output when a matching command exists |
| `EVIDENCE_2026-11-28_14-38-34/summary.md` | Provenance and gate result |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the required evidence, and the outcome.

| Condition | Required evidence | Outcome |
|---|---|---|
| One container ran at a time | Container inspect output | PASS when met, FAIL when not met |
| Resource limits held | Inspect output with one central processing unit and two gigabyte | PASS when met, FAIL when not met |
| Data restored before each pass | `restore_output.txt` | PASS when met, FAIL when not met |
| Validation passes | `driver/validate.sh` output in `checksums.txt` | PASS when met, FAIL when not met |
| 22 read queries ran | `query_1.txt`, `query_2.txt`, `query_3.txt` | PASS when met, FAIL when not met |
| 7 update queries ran | `query_1.txt`, `query_2.txt`, `query_3.txt` | PASS when met, FAIL when not met |
| Result checksum matches across three passes | `checksums.txt` | PASS when met, FAIL when not met |
| ShelfDB equivalence is claimed only when a binary exists | `shelfdb_output.txt` and `summary.md` | PASS when met, `unsupported` when not met |

A pair is complete only when every row has outcome PASS. When Java, the image, the data set, or a matching ShelfDB command is absent, report `unsupported` with the named cause. Do not report a fake result.

## Warnings

- **WARNING:** Do not run Docker, a release build, and the benchmark at the same time. The laptop can stall.
- **CAUTION:** LDBC updates persist. Restore the database before each run.
- **CAUTION:** The attached README files do not print the exact Cypher text. Do not invent it.
- **CAUTION:** The main README describes an audited run with a 30 minute warmup and a 2 hour benchmark. This plan does not claim an audited result.
- **NOTE:** This is a plan only. No image was pulled and no benchmark ran.
