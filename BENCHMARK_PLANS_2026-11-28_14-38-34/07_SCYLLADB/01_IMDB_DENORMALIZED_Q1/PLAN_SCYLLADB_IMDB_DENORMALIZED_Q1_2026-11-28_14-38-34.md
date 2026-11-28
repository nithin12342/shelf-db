# ScyllaDB against ShelfDB: IMDb denormalized Q1

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Base commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Goal and terms

This plan pairs ScyllaDB with ShelfDB on the denormalized IMDb Q1 case. Topological Query Language (TQL) is the declarative query language of ShelfDB over a cellular complex. Structured Query Language (SQL) is the standard language for relational databases. Cassandra Query Language (CQL) is the query language of ScyllaDB.

The join moves to write time in the ScyllaDB case. The query then reads a clustering range. ShelfDB resolves the join at build time. The plan reports query time and build time as two separate values.

## 2. Original benchmark link

Table 1 answers this question: which source defines the benchmark and the data. Rows are sources. Columns are the source name, the link, and the use in this plan.

| Source name | Link | Use |
|---|---|---|
| IMDb data set | `https://datasets.imdbws.com/` | Source of the title tables |
| Runtime comparison | Local file `RUNTIME_COMPARISON.md` | Measured query time, load time, and truth values |
| TQL application comparison | Local file `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Exact native SQL and the measured ScyllaDB row |
| Tool source | Local file `sheaf-db/tests/e2e/realtime_bench/imdb_scylla_bench.py` | Write-time join, CQL transport, and gate |

## 3. Data set and subset

Table 2 answers this question: which data the pair loads. Rows are data items. Columns are the item, the source path, and the subset limit.

| Data item | Source path | Subset limit |
|---|---|---|
| Title basics | `title.basics.tsv.gz` from the IMDb data set | Movies with a start year of 2000 or later |
| Title ratings | `title.ratings.tsv.gz` from the IMDb data set | Ratings of 7.5 or higher |
| Joined truth set | `imdb_scylla_rows.bin` from `imdb_scylla_prep` | 44,906 rows |
| ScyllaDB table | `imdb.titles` or `imdb_bench.titles` | One partition for movies, clustering by rating and title |

The recorded full source holds seven tables and 12,866,375 title rows. The low-power run extracts two files only. The runner must make sure that the loaded row count matches the subset manifest before it times a query.

## 4. Test-case document and specific cases

Table 3 answers this question: where the exact case lives. Rows are documents. Columns are the document path, the section, and the specific case.

| Document path | Section | Specific case |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Section 4 and section 9, Table 6 | Case 1, IMDb join and filter |
| `RUNTIME_COMPARISON.md` | Section 1 and section 1b | IMDb Q1, two-table filter and aggregate |
| `CONVERSATION_LOG_2026-10-21.md` | Row 8 of the correction table | ScyllaDB IMDb case, 90 millisecond query |
| `recursive test/QUERIES_2026-11-22_15-04-04.md` | Section 3 | Exact IMDb Q1 SQL text |

## 5. Exact query text

The approved documents give exact text. Do not change the predicates.

Native SQL for the relational form, from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`:

```sql
SELECT COUNT(*), AVG(r.averagerating), SUM(r.numvotes) FROM title_basics b JOIN title_ratings r ON r.tconst=b.tconst WHERE b.titletype='movie' AND b.startyear>=2000 AND r.averagerating>=7.5
```

CQL for the denormalized form, from `sheaf-db/tests/e2e/realtime_bench/imdb_scylla_bench.py`:

```sql
SELECT count(*), sum(votes), avg(avg) FROM imdb_bench.titles WHERE grp = 'm' AND avg >= 7.5
```

The same file creates the table with this key definition:

```sql
CREATE TABLE imdb_bench.titles (grp text, year int, avg double, tconst text, votes int, PRIMARY KEY ((grp), avg, tconst))
```

The truth value is count 44,906, votes 294,435,444, and mean rating 8.14550616844068. The runner must gate every engine on these three values.

## 6. Container image and install step

Table 4 answers this question: which image the run uses. Rows are image facts. Columns are the fact and the value.

| Fact | Value |
|---|---|
| Image name | `scylladb/scylla:latest` |
| Pull command | `docker pull scylladb/scylla:latest` |
| Historical image versions | 3.0.8 in `RUNTIME_COMPARISON.md`, 6.2 in `HIGH_VARIANCE_50_TEST_CASES_REPORT.md` |
| Historical start form | `scylladb/scylla --smp 2 --memory 4G` with a data volume |
| Native Windows build | None recorded. Use Docker. |

Start Docker Desktop and make sure that `docker info` reports a Linux server. Pull one image. Do not pull another image while the container runs.

## 7. Low-power limits

Table 5 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing unit allocation | 1 processor | Leaves capacity for the host system |
| Memory allocation | 2 gigabyte | Avoids swap and stutter |
| Benchmark threads | 1 thread | Makes timings comparable |
| Load or build step | Stop after 30 minutes | Prevents a long stalled run |
| Cooldown after the pair | 3 minutes | Lets the processor settle |

Plan command for the container, derived from the documented start form and the master index shape:

```text
docker run --rm --name bench-scylla --cpus=1 --memory=2g --pids-limit=128 -p 9042:9042 scylladb/scylla:latest --smp 1 --memory 2G
```

## 8. Original benchmark command

The source records two commands. Use the exact text.

```text
py imdb_scylla_bench.py
py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j
```

The first command performs the write-time join through `imdb_scylla_prep`, inserts the rows, and times five CQL queries. The second command records the multi-engine IMDb matrix.

## 9. Matching ShelfDB command

```text
cargo run --release --bin imdb_fiber_bench
```

This command reads the answer section from the bundled IMDb fiber file. The recorded build cost is 164.8 seconds. Report that build as a separate value. Do not add it to the query time unless the table column says total time.

## 10. Evidence files

Table 6 answers this question: which files hold the recorded evidence. Rows are files. Columns are the file path and the claim it supports.

| File path | Claim |
|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_scylla.json` | Insert time, query time, count, votes, and mean rating for ScyllaDB |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_multi_engine.json` | IMDb Q1 across five engines |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_shelf.json` | Answer read time and scan oracle |
| `results/imdb_shelf_run_now.json` | Local ShelfDB run detail |

Table 7 answers this question: which files the new run writes. Rows are file names. Columns are the file name and the content. All files sit under `EVIDENCE_2026-11-28_14-38-34/`.

| File name | Content |
|---|---|
| `environment.json` | Host, Docker version, image digest, and subset |
| `image_digest.txt` | Digest of `scylladb/scylla:latest` |
| `subset_manifest.json` | Row counts for the two extracted title files |
| `load_output.txt` | Write-time join and insert output |
| `query_1.txt` to `query_3.txt` | Three measured CQL passes |
| `checksums.txt` | Row count, votes, and mean rating for each pass |
| `shelfdb_output.txt` | Output of `imdb_fiber_bench` |
| `summary.md` | Remote, branch, base commit, image digest, and gate result |

## 11. Acceptance gate

Table 8 answers this question: when the pair passes. Rows are conditions. Columns are the condition, the required result, and the pass or fail state.

| Condition | Required result | Pass or fail |
|---|---|---|
| Row count gate | 44,906 rows in every run | PASS only on match |
| Vote gate | 294,435,444 votes in every run | PASS only on match |
| Mean rating gate | 8.14550616844068 in every run | PASS only on match |
| Resource gate | One container, one processor, 2 gigabyte memory, one thread | PASS only inside the limits |
| Time gate | Query time and build time appear in separate columns | PASS only when both appear |
| Evidence gate | Every file in Table 7 exists and holds output | PASS only when complete |
| Docker gate | If Docker is absent, report `unsupported` with the cause | FAIL for a timing claim |

## Warnings

- **CAUTION:** Do not compare the 90 millisecond query with the 208 millisecond query without naming the run and the image version. The documents record both values.
- **CAUTION:** The ScyllaDB query wins on query time in some runs. The ShelfDB answer read wins after the build. Report both frames.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run for this plan.
