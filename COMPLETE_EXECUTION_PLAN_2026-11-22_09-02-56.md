# Complete execution plan on this laptop

**Timestamp:** 2026-11-22_09-02-56, local time 09:02:56 +05:30 (Asia/Kolkata).
**Remote:** `origin https://github.com/nithin12342/shelf-db.git`.
**Branch:** `combined-stack-feature-improvement`.
**Top commit at writing:** `1aae55de9bbd3e75fdb2a63b26cff7de4b17669b`.
**Host:** Windows, `x86_64-pc-windows-msvc`.
**Scope:** database functionality only. The PyTorch bridge stays excluded.

---

## 1. Goal

Compile every program in the workspace. Load a data set. Run SQL queries. Convert each query to Topological Query Language (TQL). Optimize the TQL. Measure the optimized time against the baseline time. Record every result with its data set, its query text, and its checksum.

**Acceptance in one sentence.** The plan passes when 24 of 24 SQL queries return the same rows as their TQL form, 24 of 24 optimized runs return the same rows as the baseline runs, the join queries show a plan change, and every number names its command and its result file.

---

## 2. Environment record

Table 1 answers this question: which tool is present before the run starts.
Rows are tools. Columns are the version, the use, and the state.

| Tool | Version | Use | State |
|---|---|---|---|
| Rust compiler | 1.98.0 | All Rust crates | installed |
| Cargo | 1.98.0 | Build and test | installed |
| Zig | 0.17.0 | Storage tier | installed |
| GCC | 16.2.0 (WinLibs) | C WAL tier | installed |
| Clang | not installed | Optional C++ path | fallback to GCC |
| PowerShell | 5.1 or later | Run scripts | installed |
| `psql` | check at run time | PostgreSQL wire test | optional |

**Record the exact values in `environment.json`.** Run each version command and store the output.

---

## 3. Phase 0: compile every program

1. Build the workspace in release mode.
   ```powershell
   cd C:\Users\thela\OneDrive\Desktop\folder\shelf-db\sheaf-db
   cargo build --workspace --release
   ```
2. Build the Zig storage tier.
   ```powershell
   cd native\storage-zig
   zig build
   zig build test
   ```
3. Build the C WAL tier through the crate build script.
   ```powershell
   cd ..\..
   cargo build -p sheaf-native --release
   ```
4. Run the full test suite and keep the `test result:` lines.
   ```powershell
   cargo test --workspace --release 2>&1 | Select-String "test result:"
   ```
5. Confirm the three native archives exist.
   ```powershell
   Get-ChildItem target\release -Recurse -Include sheaf_kernel.lib,sheaf_storage.lib,sheaf_wal.lib
   ```

**Gate 0.** Build exits 0. No test reports `FAILED`. The three archives exist. Record the line counts of the `test result:` output.

**CAUTION:** `cargo test` writes to stderr. PowerShell can report exit code 1 when every test passes. Read the `test result:` lines, not the exit code.

---

## 4. Phase 1: data set

### 4.1 Primary data set: synthetic TPC-H-like

The data set is deterministic and stays inside the repository. A Transaction Processing Performance Council benchmark H (TPC-H) schema gives realistic joins and aggregations. The names follow TPC-H so the queries are portable.

**New file:** `sheaf-db/crates/sheaf-app/src/bin/gen_tpch_like.rs`.

**Generator rules.**

1. Use a xorshift64* random number generator with the fixed seed `0x2545_f491_4f6c_dd1d`. The codec tests already use this generator.
2. Emit Structured Query Language (SQL) files. Do not write a binary format.
3. Use only the supported value types: `BIGINT`, `DOUBLE`, `TEXT`, and `BOOLEAN`. The application has no `DATE` type, so a date is a `BIGINT` day number from 1992-01-01.
4. Make every table smaller than 100,000 rows. The query row limit is 100,000 and the memory limit is 64 MiB.
5. Write one `schema.sql` and one `load.sql`. Split `load.sql` when a file would pass 65,536 bytes, because one request cannot exceed that size.

**Scale factor.** Use scale factor 0.01. It gives 86,805 rows in total.

Table 2 answers this question: which tables the data set has, and how many rows each table holds.
Rows are tables. Columns are the row count, the primary column, and the join key.

| Table | Rows | Primary column | Join key |
|---|---|---|---|
| `region` | 5 | `r_regionkey` | none |
| `nation` | 25 | `n_nationkey` | `n_regionkey` |
| `supplier` | 100 | `s_suppkey` | `s_nationkey` |
| `part` | 2,000 | `p_partkey` | none |
| `partsupp` | 8,000 | `ps_partkey`, `ps_suppkey` | both |
| `customer` | 1,500 | `c_custkey` | `c_nationkey` |
| `orders` | 15,000 | `o_orderkey` | `o_custkey` |
| `lineitem` | 60,175 | `l_orderkey`, `l_linenumber` | `l_orderkey`, `l_partkey`, `l_suppkey` |

**Column set.**

- `region(r_regionkey BIGINT, r_name TEXT, r_comment TEXT)`
- `nation(n_nationkey BIGINT, n_name TEXT, n_regionkey BIGINT, n_comment TEXT)`
- `supplier(s_suppkey BIGINT, s_name TEXT, s_nationkey BIGINT, s_acctbal DOUBLE, s_phone TEXT)`
- `part(p_partkey BIGINT, p_name TEXT, p_brand TEXT, p_type TEXT, p_size BIGINT, p_retailprice DOUBLE)`
- `partsupp(ps_partkey BIGINT, ps_suppkey BIGINT, ps_availqty BIGINT, ps_supplycost DOUBLE)`
- `customer(c_custkey BIGINT, c_name TEXT, c_nationkey BIGINT, c_acctbal DOUBLE, c_mktsegment TEXT, c_active BOOLEAN)`
- `orders(o_orderkey BIGINT, o_custkey BIGINT, o_orderstatus TEXT, o_totalprice DOUBLE, o_orderdate BIGINT, o_orderpriority TEXT)`
- `lineitem(l_orderkey BIGINT, l_partkey BIGINT, l_suppkey BIGINT, l_linenumber BIGINT, l_quantity DOUBLE, l_extendedprice DOUBLE, l_discount DOUBLE, l_tax DOUBLE, l_returnflag TEXT, l_linestatus TEXT, l_shipdate BIGINT)`

**Distribution.** Put a skew in two places. Make 20 percent of the customers hold 60 percent of the orders. Make `l_returnflag` null in 1 percent of the rows. The skew makes predicate pushdown and hash join show a real difference.

**Run the generator.**
```powershell
cargo run --release -p sheaf-app --bin gen_tpch_like -- data\tpch_like_sf001
```

**Manifest.** Write `dataset_manifest.json` with the scale factor, the seed, the table counts, and a SHA-256 checksum of each SQL file.

### 4.2 Optional benchmark data sets

Use one of these only when the download is available. Record the license and the checksum.

1. **TPC-H dbgen.** Build dbgen and run `dbgen -s 0.01 -T l`. This gives the official TPC-H rows. The license limits redistribution, so keep the data outside Git.
2. **IMDb public data set.** The repository already has an IMDb reader in `crates/sheaf-engine/src/bin/shared/imdb_build.rs`. It expects `name.basics.tsv`, `title.ratings.tsv`, and `title.basics.tsv`. Download them from the official IMDb data page and record the checksums.
3. **New York City taxi data.** This gives a large time-series table. Keep a slice under 100,000 rows.

**Rule.** A comparison table names its data set in every row. Do not mix data sets in one table.

---

## 5. Phase 2: load the data

1. Create the database directory.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch sql-file data\tpch_like_sf001\schema.sql
   ```
2. Load the rows. Run each load file in order.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch sql-file data\tpch_like_sf001\load_01.sql
   ```
3. Make sure that every count matches the manifest.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "SELECT COUNT(*) FROM lineitem"
   ```
4. Repeat the count check for all eight tables. Store the output in `load_counts.json`.

**Gate 2.** All eight counts equal the manifest counts. A mismatch stops the run.

**CAUTION:** do not load more than 100,000 rows into one table. A scan over that table returns an explicit row-limit error. That error is a real result, not a pass.

---

## 6. Phase 3: query corpus

Table 3 answers this question: which query each identifier runs, and which optimization it should exercise.
Rows are queries. Columns are the category, the SQL text, and the expected optimization.

| ID | Category | SQL text | Expected optimization |
|---|---|---|---|
| Q01 | Point lookup | `SELECT r_name FROM region WHERE r_regionkey = 2` | Predicate pushdown |
| Q02 | Point lookup | `SELECT c_name FROM customer WHERE c_custkey = 42` | Predicate pushdown |
| Q03 | Range scan | `SELECT c_custkey, c_acctbal FROM customer WHERE c_acctbal > 9000` | Predicate pushdown |
| Q04 | Range scan | `SELECT o_orderkey FROM orders WHERE o_totalprice BETWEEN 100000 AND 200000` | Predicate pushdown |
| Q05 | Count | `SELECT COUNT(*) FROM lineitem WHERE l_quantity < 5` | Predicate pushdown |
| Q06 | Aggregate | `SELECT SUM(l_extendedprice) FROM lineitem` | Full scan |
| Q07 | Aggregate | `SELECT MIN(o_totalprice), MAX(o_totalprice) FROM orders` | Full scan |
| Q08 | Aggregate | `SELECT AVG(c_acctbal) FROM customer` | Full scan |
| Q09 | Group by | `SELECT l_returnflag, SUM(l_extendedprice) FROM lineitem GROUP BY l_returnflag` | Hash aggregation |
| Q10 | Group by | `SELECT c_mktsegment, COUNT(*) FROM customer GROUP BY c_mktsegment ORDER BY c_mktsegment` | Hash aggregation and sort |
| Q11 | Distinct | `SELECT DISTINCT l_returnflag FROM lineitem` | Distinct |
| Q12 | Null filter | `SELECT COUNT(*) FROM lineitem WHERE l_returnflag IS NULL` | Predicate pushdown |
| Q13 | Boolean | `SELECT COUNT(*) FROM customer WHERE c_active = TRUE` | Predicate pushdown |
| Q14 | Order and limit | `SELECT o_orderkey, o_totalprice FROM orders ORDER BY o_totalprice DESC LIMIT 20` | Top-k sort |
| Q15 | Two-table join | `SELECT o.o_orderkey, c.c_name FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey WHERE o.o_totalprice > 400000` | Hash join and pushdown |
| Q16 | Two-table join | `SELECT l.l_orderkey, p.p_name FROM lineitem l INNER JOIN part p ON l.l_partkey = p.p_partkey WHERE l.l_quantity > 40` | Hash join and pushdown |
| Q17 | Two-table join | `SELECT s.s_name, ps.ps_supplycost FROM partsupp ps INNER JOIN supplier s ON ps.ps_suppkey = s.s_suppkey WHERE ps.ps_availqty > 5000` | Hash join |
| Q18 | Three-table join | `SELECT n.n_name, SUM(o.o_totalprice) FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey INNER JOIN nation n ON c.c_nationkey = n.n_nationkey GROUP BY n.n_name` | Two hash joins |
| Q19 | Three-table join | `SELECT r.r_name, COUNT(*) FROM customer c INNER JOIN nation n ON c.c_nationkey = n.n_nationkey INNER JOIN region r ON n.n_regionkey = r.r_regionkey GROUP BY r.r_name` | Two hash joins |
| Q20 | Join and sort | `SELECT c.c_name, o.o_totalprice FROM customer c INNER JOIN orders o ON c.c_custkey = o.o_custkey ORDER BY o.o_totalprice DESC LIMIT 10` | Hash join and top-k |
| Q21 | Join and filter | `SELECT l.l_orderkey FROM lineitem l INNER JOIN orders o ON l.l_orderkey = o.o_orderkey WHERE o.o_orderpriority = '1-URGENT'` | Hash join and pushdown |
| Q22 | Text filter | `SELECT p_name FROM part WHERE p_brand = 'Brand#12'` | Predicate pushdown |
| Q23 | Null join key | `SELECT COUNT(*) FROM orders o INNER JOIN customer c ON o.o_custkey = c.c_custkey` | Null-safe join |
| Q24 | Row limit probe | `SELECT COUNT(*) FROM lineitem` | Explicit row-limit report when the table passes the limit |

**Corpus rule.** Each query uses only the supported SQL surface: one `FROM` source, `INNER JOIN` with `ON`, `WHERE`, `GROUP BY`, `ORDER BY`, `LIMIT`, `OFFSET`, `DISTINCT`, and aggregate calls. The parser rejects `HAVING`, subqueries, `CREATE INDEX`, and `PRIMARY KEY`.

**Store the corpus** in `queries.csv` with columns `id,category,sql,expected_optimization`.

---

## 7. Phase 4: SQL to TQL conversion

1. Run each SQL query and capture the rendered TQL from the `tql` field.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "SELECT ..." > results\run\Q15_sql.json
   ```
2. Extract the `tql` text from the JSON. Store it in `Q15.tql`.
3. Run the TQL text on its own.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch tql-file results\run\Q15.tql > results\run\Q15_tql.json
   ```
4. Compare the `rows` array of the SQL run and the TQL run. Compute one SHA-256 checksum of the canonical rows text for each.
5. Assert that the two checksums are equal.

**TQL shape that the converter must produce.**

```text
MATCH ROWS ("orders": "o")
JOIN ROWS ("customer": "c") ON "o"."o_custkey" = "c"."c_custkey"
WHERE "o"."o_totalprice" > 400000
EMIT ROWS { "o"."o_orderkey", "c"."c_name" };
```

**Native TQL cases.** Add six hand-written TQL queries that have no SQL form. These prove that TQL is a first-class path, not a text rewrite.

1. `MATCH ROWS ("region": "r") EMIT ROWS { "r"."r_name" } ORDER BY "r"."r_name" ASC;`
2. `MATCH ROWS ("part": "p") WHERE "p"."p_size" > 40 EMIT ROWS { "p"."p_partkey", "p"."p_size" };`
3. `MATCH ROWS ("lineitem": "l") WHERE "l"."l_quantity" < 3 EMIT ROWS { "l"."l_orderkey" } LIMIT 100;`
4. A TQL join with `GROUP BY`.
5. A TQL query with `DISTINCT`.
6. A TQL query with `OFFSET`.

**Gate 4.** 24 of 24 SQL queries match their TQL form by checksum. 6 of 6 native TQL queries run without error.

---

## 8. Phase 5: optimization measurement

### 8.1 What the optimizer does

The current build changes three things when optimization is on.

1. It picks a hash index for an equi-join. With `--baseline` it uses a nested loop.
2. It pushes a predicate into the scan plan.
3. It expands a projection alias before execution.

The optimizer does not yet bridge TQL to fiber execution (OPT-004). A TQL optimizer cost is an estimate. It is not an execution time.

### 8.2 Measurement protocol

1. Run five warm-up executions of the query.
2. Run ten measured executions with optimization on. Record `stats.elapsed_us`.
3. Run ten measured executions with `--baseline`. Record `stats.elapsed_us`.
4. Take the median of each set. Discard the warm-up runs.
5. Compute `speedup = baseline_median / optimized_median`. State it with two decimal places.
6. Save the `plan` array from both runs. The plan text proves which rule fired.
7. Assert that the optimized rows equal the baseline rows by checksum.

**Command pair for one query.**
```powershell
cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "<SQL>" > results\run\Q15_optimized.json
cargo run --release -p sheaf-app -- --db data\shelf_tpch --baseline sql "<SQL>" > results\run\Q15_baseline.json
```

**Gate 5.** Every query returns the same checksum on the optimized path and the baseline path. Each of the six join queries shows `Inner hash-index join` in the optimized plan and `Nested loop` in the baseline plan.

### 8.3 Expected direction

Do not promise a number before the run. State the expected direction, then measure.

Table 4 answers this question: which query group should improve, and why.
Rows are query groups. Columns are the group, the expected direction, and the measured median at run time.

| Group | Expected direction | Reason | Measured median (ms) |
|---|---|---|---|
| Point lookup Q01, Q02 | Small gain | Predicate pushdown skips rows | fill at run time |
| Range scan Q03, Q04 | Small gain | Predicate pushdown skips rows | fill at run time |
| Join Q15 to Q21 | Large gain | Hash index replaces nested loop | fill at run time |
| Aggregate Q06 to Q08 | No change | One full scan in both paths | fill at run time |
| Group by Q09, Q10 | No change | Hash aggregation in both paths | fill at run time |

### 8.4 Scale probe

Run Q06, Q15, and Q18 at three table sizes: 10,000, 40,000, and 60,000 lineitem rows. Record the time and the `candidate_rows` value. This shows how the cost grows and where the 100,000 row limit stops the run.

---

## 9. Phase 6: network paths

1. Start the framed request server.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch serve --address 127.0.0.1:7400 --connections 1
   ```
2. Send one length-framed JSON request with a small PowerShell `TcpClient` script. Assert that the response has the same row checksum as the local SQL run.
3. Start the PostgreSQL wire server.
   ```powershell
   cargo run --release -p sheaf-app -- --db data\shelf_tpch pgserve --address 127.0.0.1:5433 --connections 1
   ```
4. Run one query through `psql` when `psql` is installed.
   ```powershell
   psql -h 127.0.0.1 -p 5433 -U any -d any -c "SELECT COUNT(*) FROM lineitem"
   ```
5. When `psql` is absent, run `cargo test -p sheaf-app --test network` and record the result. State that the manual client check is `unsupported` on this host.

**Gate 6.** The framed server returns the same checksum as the local run. The pgwire check passes, or it reports `unsupported` with the named cause.

---

## 10. Phase 7: evidence and report

**Directory:** `results/complete_execution_2026-11-22_09-02-56/`.

Table 5 answers this question: which file holds which evidence.
Rows are files. Columns are the content and the source command.

| File | Content | Source |
|---|---|---|
| `environment.json` | Tool versions, host, OS build | Version commands |
| `dataset_manifest.json` | Scale factor, seed, counts, SQL checksums | Generator |
| `load_counts.json` | Measured table counts | Count queries |
| `queries.csv` | The 24 queries with category and expected optimization | This plan |
| `parity.csv` | SQL checksum, TQL checksum, equal or not | Phase 4 |
| `timing.csv` | Query, baseline median (ms), optimized median (ms), speedup | Phase 5 |
| `plans/Q15_optimized.txt` | The plan lines | Phase 5 |
| `plans/Q15_baseline.txt` | The plan lines | Phase 5 |
| `network.json` | Framed and PostgreSQL wire results | Phase 6 |
| `summary.md` | The five tables above, filled with real values | All phases |
| `checksums.txt` | SHA-256 of every result file | Final step |

**Summary table shape.**

Table 6 answers this question: what the run measured for each query.
Rows are the 24 queries. Columns are the query, the rows returned, the SQL and TQL parity, the baseline median (ms), the optimized median (ms), and the speedup.

**Gate 7.** `summary.md` exists. Every cell in Table 6 holds a measured value or the word `unsupported` with a cause. No cell holds a guess.

---

## 11. Command reference

```powershell
# Build
cd C:\Users\thela\OneDrive\Desktop\folder\shelf-db\sheaf-db
cargo build --workspace --release
cargo test --workspace --release 2>&1 | Select-String "test result:"

# Native tiers
cd native\storage-zig; zig build; zig build test; cd ..\..

# Data set
cargo run --release -p sheaf-app --bin gen_tpch_like -- data\tpch_like_sf001
cargo run --release -p sheaf-app -- --db data\shelf_tpch sql-file data\tpch_like_sf001\schema.sql
cargo run --release -p sheaf-app -- --db data\shelf_tpch sql-file data\tpch_like_sf001\load_01.sql
cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "SELECT COUNT(*) FROM lineitem"

# SQL and TQL
cargo run --release -p sheaf-app -- --db data\shelf_tpch sql "<SQL>"
cargo run --release -p sheaf-app -- --db data\shelf_tpch tql-file <FILE>
cargo run --release -p sheaf-app -- --db data\shelf_tpch --baseline sql "<SQL>"

# Servers
cargo run --release -p sheaf-app -- --db data\shelf_tpch serve --address 127.0.0.1:7400 --connections 1
cargo run --release -p sheaf-app -- --db data\shelf_tpch pgserve --address 127.0.0.1:5433 --connections 1
```

---

## 12. Risks and limits

- **Row limit.** A scan over more than 100,000 rows returns an explicit error. Keep each table under the limit. Record the error as a limit result.
- **Memory limit.** A query cannot exceed 64 MiB. A large join can hit the limit. Reduce the scale factor, not the limit.
- **Request size.** One request cannot exceed 65,536 bytes. Split a large load file.
- **No date type.** Use `BIGINT` day numbers. Do not write `DATE` literals.
- **No index statement.** The parser rejects `CREATE INDEX`. The optimizer builds an in-memory hash index for each join.
- **Optimizer scope.** The optimizer changes join algorithm, pushdown, and alias expansion. It does not rewrite TQL into fibers.
- **Cold cache.** The first run reads from disk. Use warm-up runs and state the run count.
- **PowerShell exit code.** Read the `test result:` lines, not the exit code.
- **CAUTION:** do not compare a synthetic data set against an official benchmark row. Name the data set in every table row.

---

## 13. Completion definition

The plan is complete when all seven gates pass and `summary.md` holds measured values for every query. Then commit the evidence directory and update `SKELETON.md` with a `complete_execution` block that names the run directory, the data set, the query count, and the measured join speedup.
