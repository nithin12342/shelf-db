# Plan: MariaDB versus ShelfDB on Internet Movie Database (IMDb) query 1 (Q1)

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.

## 1. Goal

This plan compares MariaDB with ShelfDB on IMDb Q1. This document is a plan only. It does not pull an image, load a table, or run a query.

A TQL statement is a declarative query over a cellular complex. SQL is the standard query language for relational databases. A gate is an automatic test that compares a result with a truth value. An answer section is a stored record that a query reads later.

## 2. Original benchmark link and data set

Table 1 answers this question: which sources define the benchmark and the data. Rows are sources. Columns are the source, the link, and the content.

| Source | Link or path | Content |
|---|---|---|
| IMDb data set | `https://datasets.imdbws.com/` | Seven tab-separated value (TSV) tables |
| Extracted bundle README | `attached_bundle_review_2026-11-28_14-19-54/README.md` | Bundle folder `imdb/` |
| Data set record | `DATASETS.md` | Seven compressed TSV files, 1.900 gigabytes recorded |
| Query comparison | `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 6 with MariaDB 13.0.2 |
| Runtime comparison | `RUNTIME_COMPARISON.md` | Section 1 with MariaDB 971 milliseconds |

The full IMDb set holds 12,866,375 `title` rows across seven tables. Q1 uses `title.basics.tsv.gz` and `title.ratings.tsv.gz`. Start from those two tables. If the load exceeds the 30 minute stop rule, take a slice from both files and record the slice size in the manifest. Make sure that the slice keeps the join keys on both sides. Compute the truth for a slice with DuckDB and PostgreSQL before timing MariaDB. The published truth applies to the full set only.

## 3. Exact test-case document path and specific test cases

Table 2 answers this question: which documents name the IMDb Q1 case. Rows are documents. Columns are the document path, the table or section, and the specific case.

| Document path | Table or section | Specific case |
|---|---|---|
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 1, case 1 | IMDb join and filter shape |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 6 | IMDb Q1, MariaDB 1,074.4 milliseconds |
| `TQL_APPLICATION_COMPARISON_2026-10-26.md` | Table 8 | Q1 answer read, 24 bytes |
| `RUNTIME_COMPARISON.md` | Section 1 | IMDb Q1, MariaDB 971 milliseconds |
| `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` | Case 1 | TQL text and native SQL text |
| `recursive test/QUERIES_2026-11-22_15-04-04.md` | Section 3 | Exact Q1 SQL text |

The recorded MariaDB version is 13.0.2. Table 6 records 1,074.4 milliseconds in the 2026-10-26 window. `RUNTIME_COMPARISON.md` records 971 milliseconds in the 2026-10-21 window. Do not mix those two numbers in one row. The truth value is count 44,906, sum of votes 294,435,444, and mean rating 8.14550616844068 for the full set.

## 4. Exact query text

The source gives exact SQL for Q1. Write it verbatim. Do not change the predicates.

Exact SQL for IMDb Q1, from Table 1 case 1 of `TQL_APPLICATION_COMPARISON_2026-10-26.md` and from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`:

```sql
SELECT COUNT(*), AVG(r.averagerating), SUM(r.numvotes) FROM title_basics b JOIN title_ratings r ON r.tconst=b.tconst WHERE b.titletype='movie' AND b.startyear>=2000 AND r.averagerating>=7.5
```

Exact TQL for case 1, from `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`:

```text
MATCH HYPERCELL c: Casting OVER (t: Title, p: Person) WHERE t.year > 2000 AND t.rating > 75 SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005) EMIT SECTION { p.name, t.rating, SECTION_ENERGY };
```

The TQL case filters on `t.year > 2000` and a rating of 75 as a scaled integer. The SQL case filters on `startyear >= 2000` and `averagerating >= 7.5`. Read the ratio as the cost of a shared shape, not as an identical predicate. A TQL optimizer cost is an estimate. Do not state it as an execution time. The TQL-to-fiber execution bridge is not built.

## 5. Container image and install or pull step

Table 3 answers this question: which image and steps the run uses. Rows are components. Columns are the component, the step, and the command.

| Component | Step | Command |
|---|---|---|
| MariaDB | Pull one image | `docker pull mariadb:13.0.2` |
| MariaDB | Start one container | `docker run --rm --name mariadb --cpus=1 --memory=2g --pids-limit=128 -e MARIADB_ROOT_PASSWORD=<password> -v <subset>:/data:ro mariadb:13.0.2` |
| Table schema | Create two tables | Use the column names in the exact SQL: `tconst`, `titletype`, `startyear`, `averagerating`, `numvotes` |
| Data load | Load the two TSV files | `LOAD DATA LOCAL INFILE` for each selected file |

Make sure that the column names match the exact SQL. Do not add a predicate or an index that the source does not name. Record the image digest in `environment.json`.

## 6. Low-power limits

Table 4 answers this question: which limits govern the local run. Rows are limits. Columns are the limit, the value, the unit, and the reason.

| Limit | Value | Unit | Reason |
|---|---:|---|---|
| Database containers | 1 | container | Keeps the laptop responsive |
| Container central processing units | 1 | central processing unit | Leaves capacity for Windows and Docker Desktop |
| Container memory | 2 | gigabytes | Avoids swap |
| Benchmark threads | 1 | thread | Makes timings comparable |
| Warm-up passes | 1 | pass | Populates caches before timing |
| Measured passes | 3 | pass | Gives a spread for each query |
| Stop rule for a build step | 30 | minutes | Prevents a hot laptop |
| Cooldown between engines | 3 | minutes | Lets the processor settle |

## 7. Commands

Table 5 answers this question: which commands the run uses. Rows are steps. Columns are the step, the command, and the expected output.

| Step | Command | Expected output |
|---|---|---|
| Load data | `mysql --local-infile=1 --host=127.0.0.1 --user=root --password=<password> <load_script.sql>` | Row counts for both tables |
| Original recorded runner | `py run_all.py --keep-up` | Seven engine results and twelve ShelfDB binaries |
| Original multi-engine case | `py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j` | IMDb Q1 across five engines |
| MariaDB timed case | `mysql --host=127.0.0.1 --user=root --password=<password> -e "<exact Q1 SQL>"` | Count, mean, and sum with gate PASS |
| ShelfDB answer read | `cargo run --release --bin imdb_fiber_bench` | Answer read, 24 bytes, with three gates |
| ShelfDB scan oracle | The oracle inside `imdb_fiber_bench` | Computed answer for the same three questions |

Do not run either recorded runner on this laptop. Those runners start several engines. Run only the MariaDB case in one container. The one-time bundle build is a separate cost. The recorded build is 164.8 seconds. Report the answer read and the build plus read as two rows.

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files. Columns are the file, the claim, and the command or output that supports it.

| File | Claim | Command or output |
|---|---|---|
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_multi_engine.json` | MariaDB Q1 time and truth gate | `py imdb_multi_engine_bench.py --engines sqlite,mariadb,redis,scylladb,neo4j` |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_job.json` | Q1 to Q4 with md5 gates | `py imdb_complex_bench.py` |
| `sheaf-db/tests/e2e/realtime_bench/results/imdb_shelf.json` | Answer read and scan oracle | `cargo run --release --bin imdb_fiber_bench` |
| `sheaf-db/tests/e2e/realtime_bench/results/pg_imdb_load_session_20261026.txt` | Load rows and seconds | PostgreSQL client output |
| `results/MANUAL_CLI_EXECUTION_20261027.json` | Native CLI cold ingestion case | `py run_cli_manual_suite.py` |
| `results/local_docker_benchmark_<timestamp>/environment.json` | Host, image digest, data subset | Written before the pull |
| `results/local_docker_benchmark_<timestamp>/mariadb/result.csv` | One row with count, mean, sum, and time | Written after three measured passes |

## 9. Acceptance gate

Table 7 answers this question: which conditions make the pair accepted. Rows are gates. Columns are the gate, the expected result, the outcome, and the remark.

| Gate | Expected result | Gate (pass or fail) | Remarks |
|---|---|---|---|
| One container at a time | One MariaDB container, no other database container | Pass only when true | Record `docker ps` output |
| Resource limits | One central processing unit, 2 gigabytes, one thread | Pass only when true | Record `docker inspect` output |
| Truth, full set | Count 44,906, votes 294,435,444, mean 8.14550616844068 | Pass only when true | Use a different truth for a slice |
| MariaDB gate | Gate value PASS in `imdb_multi_engine.json` | Pass only when true | Do not time a wrong answer |
| ShelfDB gate | Three gates PASS in `imdb_shelf.json` | Pass only when true | Label ShelfDB as the reference row |
| Build reporting | Build time and answer read time are separate rows | Pass only when true | The 0.072 millisecond read excludes the build |
| Evidence completeness | Commands, outputs, and checksums are present | Pass only when true | Name the remote, branch, and commit in `summary.md` |

## Warnings

- **CAUTION:** The answer section is a stored result. It is not a scan of the two tables.
- **CAUTION:** Do not mix the 1,074.4 millisecond and the 971 millisecond MariaDB numbers. They come from different runs.
- **CAUTION:** A slice needs its own truth value. The published truth applies to the full set.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
