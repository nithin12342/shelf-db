# PostgreSQL versus ShelfDB: TPC-H scale factor 1 plan

**Timestamp:** 2026-11-28_14-38-34, local time 14:38:34 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `c48907b1f9880679fd979521ba28a3f4a15969e3`.
**Writing standard:** ASD-STE100 Simplified Technical English.

## 1. Result

This plan runs the Transaction Processing Performance Council H (TPC-H) scale factor 1 (SF1) scan and join cases on PostgreSQL and ShelfDB. It is a plan only. No image was pulled and no benchmark was run.

## 2. Concept definitions

- **TQL:** the Topological Query Language of ShelfDB. A TQL statement is a declarative query over a cellular complex.
- **SQL:** Structured Query Language. SQL is the standard query language for relational databases.
- **Gate:** a test that compares a result with the stated truth value.

## 3. Original source

Table 1 answers this question: which sources define this pair. Rows are source items. Columns are the item and the value.

| Item | Value |
|---|---|
| Original benchmark link | `https://github.com/gregrahn/tpch-kit` |
| Specification link | `https://www.tpc.org/tpch/` |
| Data set document | `DATASETS.md`, rows 4 and 5 |
| Test case document | `RUNTIME_COMPARISON.md`, section 3 |
| Exact query document | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt`, case 3 native SQL |
| Runner document | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 2 and Table 3 |
| Tool README | `attached_bundle_review_2026-11-28_14-19-54/docs/tpch-kit-master__tpch-kit-master_dbgen_README` |

## 4. Data set and subset

Table 2 answers this question: which data rows enter this run. Rows are data items. Columns are the item, the value, and the unit.

| Data item | Value | Unit |
|---|---|---|
| Source tables | Eight pipe-delimited tables from `dbgen` | tables |
| Original scale factor | 1 | scale |
| Low-power scale factor | 0.01 first, then 1 | scale |
| Rows in lineitem at scale factor 1 | 6,001,215 | rows |
| Tables for the scan case | lineitem | tables |
| Tables for the join case | customer, orders, lineitem, partsupp | tables |
| Local source path | `D:\bench\build\tpch-dbgen\*.tbl` | path |
| Subset rule | Use scale 0.01 first. Use scale 1 only after the laptop stays responsive. | rule |

## 5. Test cases and exact query text

Table 3 answers this question: which cases run and which text exists. Rows are cases. Columns are the case identifier, the case name, the text status, and the source.

| Case identifier | Case name | Text status | Source |
|---|---|---|---|
| Application case 3 (TQL case 3) | TPC-H analytical filter and aggregation | Exact SQL in the source | `recursive test/TQL_AND_NATIVE_2026-11-22_15-04-04.txt` |
| SQL corpus statement 5 (Q05) | Count of lineitem rows with quantity below 5 | Exact SQL in the source | `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` |
| SQL corpus statement 6 (Q06) | Sum of extended price over lineitem | Exact SQL in the source | `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` |
| SQL corpus statement 24 (Q24) | Count of all lineitem rows | Exact SQL in the source | `recursive test/SQL_CORPUS_2026-11-22_15-04-04.sql` |
| Benchmark suite case | Three-table join of customer, orders, and lineitem | Description only. No exact SQL. | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, Table 3 |

The source gives this exact SQL text for the analytical filter case:

```sql
SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0
```

The source gives this exact SQL text for the count cases:

```sql
SELECT COUNT(*) FROM lineitem WHERE l_quantity < 5;
SELECT COUNT(*) FROM lineitem;
```

The source gives this exact SQL text for the sum case:

```sql
SELECT SUM(l_extendedprice) FROM lineitem;
```

The source gives a description for the three-table join. It names a join of customer, orders, and lineitem. It names a filter on shipdate before 1995-01-01, a group by customer name, and the top 100 rows. Do not invent SQL for this join.

## 6. Container image and low-power limits

Pull one image. Use this command:

```sh
docker pull postgres:18.6
```

Start one container. Change the mount path to the subset directory before the run:

```sh
docker run --rm --name postgres_tpch --cpus=1 --memory=2g --pids-limit=128 -e POSTGRES_PASSWORD=bench -e POSTGRES_USER=bench -e POSTGRES_DB=bench -v D:\bench\build\tpch-dbgen:/data:ro -c max_parallel_workers=0 -c max_parallel_workers_per_gather=0 postgres:18.6
```

Table 4 answers this question: which limits govern this run. Rows are limits. Columns are the limit, the value, and the reason.

| Limit | Value | Reason |
|---|---|---|
| Concurrent database containers | 1 | Keeps the laptop responsive |
| Central processing units for the container | 1 | Leaves capacity for the host system |
| Memory for the container | 2 GB | Avoids swap |
| Benchmark threads | 1 | Makes timings comparable |
| Parallel gather workers | 0 | Enforces one thread in PostgreSQL |
| Maximum time for one load or query step | 30 minutes | Prevents a long stalled run |

## 7. Commands

Table 5 answers this question: which commands run. Rows are steps. Columns are the step, the command, and the source.

| Step | Command | Source |
|---|---|---|
| Build the generator | `make MACHINE=LINUX DATABASE=POSTGRESQL` | TPC-H kit tool README |
| Generate the scale factor 1 data | `./dbgen -s 1` | TPC-H kit tool README |
| Generate the validation query | `./qgen -d 1` | TPC-H kit tool README |
| Original multi-engine runner | `py run_all.py --keep-up` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| Load lineitem from the pipe-delimited file | `psql -h 127.0.0.1 -U bench -d bench -c "\copy lineitem FROM '/data/lineitem.tbl' WITH (FORMAT csv, DELIMITER '|')"` | This plan |
| Measure the exact scan SQL | `psql -h 127.0.0.1 -U bench -d bench -f /data/scan.sql` | This plan |
| Measure the exact count SQL | `psql -h 127.0.0.1 -U bench -d bench -f /data/count.sql` | This plan |
| ShelfDB structured query | `cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "SELECT COUNT(*) FROM lineitem"` | `COMPLETE_EXECUTION_PLAN_2026-11-22_09-02-56.md` |
| ShelfDB scan binary | `cargo run -p sheaf-engine --bin tpch_scan_bench --release` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| ShelfDB fused scan binary | `cargo run -p sheaf-engine --bin fused_scan_bench --release` | `TQL_APPLICATION_COMPARISON_2026-10-26.md`, section 17 |
| ShelfDB fiber join binary | `cargo run -p sheaf-engine --bin fiber_join_bench --release` | `ADVERSARIAL_ANALYSIS.md`, section 11 |

## 8. Evidence files

Table 6 answers this question: which files hold the evidence. Rows are files under `results/local_docker_benchmark_<run timestamp>/postgresql_tpch_sf1/`. Columns are the file, the content, and the use.

| File | Content | Use |
|---|---|---|
| `environment.json` | Host, Docker version, image digest, data subset | Reproduce the run |
| `pulls.txt` | Output of the image pull | Record the image |
| `container_inspect.txt` | Container limits and mount | Record the low-power limits |
| `manifest.json` | Row counts per loaded table | Record the subset |
| `load.txt` | Load commands and output | Record the load time |
| `result.csv` | One row per case with time in milliseconds and checksum | Compare the engines |
| `checksums.txt` | Truth value and observed value for each case | Run the gate |

## 9. Acceptance gate

Table 7 answers this question: which conditions close this pair. Rows are conditions. Columns are the condition, the expected result, and the status. Every status starts as `Not run`.

| Condition | Expected result | Status |
|---|---|---|
| One container runs at a time | PASS | Not run |
| The container uses one central processing unit and 2 GB memory | PASS | Not run |
| One benchmark thread runs | PASS | Not run |
| Loaded lineitem rows match the manifest | PASS | Not run |
| Three measured passes complete for each exact SQL case | PASS | Not run |
| PostgreSQL and ShelfDB checksums match for each case | PASS | Not run |
| No load or query step exceeds 30 minutes | PASS | Not run |
| The three-table join stays a name-only case until SQL text is supplied | PASS | Not run |

## Warnings

- **CAUTION:** The scan SQL and the ShelfDB scan binary use different predicates. Read the ratio as one-pass scan cost, not as an identical query comparison.
- **CAUTION:** Do not load scale factor 1 before the scale 0.01 pass succeeds.
- **NOTE:** The TQL-to-fiber execution bridge is not built. A TQL optimizer cost is an estimate, not an execution time.
- **NOTE:** This is a plan only. No image was pulled and no benchmark was run.
