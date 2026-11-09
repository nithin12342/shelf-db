# KDB_TIMESERIES_COMPARISON_2026-10-27.md - Kdb+ Live Benchmark and Comparison

**Document date:** 2026-10-27.  
**Experiment window:** 2026-10-27 10:04:23 IST to 2026-10-27 11:01:31 IST (57 min 8 s). IST is Indian Standard Time, UTC+05:30.  
**Host:** Windows 11, Intel Core i7-1165G7 (4 cores, 8 threads), 15.8 GB RAM, NVMe solid-state drive.  
**Writing standard:** ASD-STE100 Simplified Technical English.  
**License status:** KDB-X Community Edition token activated (`kc.lic` installed).  
**Raw evidence sources:** `results/KDB_TIMESERIES_RESULTS.json`, `results/KDB_JOIN_RESULTS.json`, `results/HIGH_VARIANCE_50_RESULTS.json`.

---

## 1. What This Document Answers

This document answers how Kdb+ executes live time-series and relational queries after token installation, what execution times Kdb+ returns, and how Kdb+ compares against DuckDB and ShelfDB. Every number comes from a committed file in the `results/` directory.

---

## 2. Concept Definitions

- **Kdb+:** a column-oriented time-series database produced by KX Systems.
- **PyKX:** the official Python integration library for Kdb+ and q.
- **q:** the vector and array query language of Kdb+.
- **Time-series:** an ordered sequence of data points indexed by time.
- **BCSF:** the Binary Cellular Storage Format of ShelfDB.
- **TQL:** the Topological Query Language of ShelfDB.

---

## 3. License Activation Record

The base64 token provided by the operator was decoded into a 434-byte binary license file. The benchmark script wrote the license to `pykx/lib/kc.lic` and `~/.kx/kc.lic`.

```text
Welcome to KDB-X Community Edition!
Licensed: True
Q version: 5f
```

The embedded Kdb+ engine is fully functional on the host machine.

---

## 4. Time-Series Query Shapes and Native Code

The benchmark measured three core time-series access patterns across 100,000 tick records.

### 4.1 Shape 1: Time Window Filter and Harmonic Solve

- **TQL shape (valid AST):**
  ```text
  MATCH HYPERCELL t: Tick OVER (s: Symbol)
  WHERE t.time >= 20261027090000 AND t.time < 20261027100000
  SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005)
  EMIT SECTION { t.price, t.size, SECTION_ENERGY };
  ```

- **Kdb+ native query (q):**
  ```q
  select avg price, sum size by 0D00:01:00 xbar time from quotes
  ```

- **DuckDB SQL equivalent:**
  ```sql
  SELECT time_bucket(INTERVAL '1 minute', ts) AS b, AVG(price), SUM(size)
  FROM quotes GROUP BY 1;
  ```

### 4.2 Shape 2: Time Range Slice Filter (1-Hour Slice)

- **TQL shape (valid AST):**
  ```text
  MATCH HYPERCELL t: Tick OVER (s: Symbol)
  WHERE t.time >= 20261027091500 AND t.time <= 20261027101500
  SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005)
  EMIT SECTION { t.price, t.size, SECTION_ENERGY };
  ```

- **Kdb+ native query (q):**
  ```q
  select avg price, sum size from quotes where time within (2026.10.27D09:15:00.000000000; 2026.10.27D10:15:00.000000000)
  ```

- **DuckDB SQL equivalent:**
  ```sql
  SELECT AVG(price), SUM(size) FROM quotes
  WHERE ts BETWEEN TIMESTAMP '2026-10-27 09:15:00' AND TIMESTAMP '2026-10-27 10:15:00';
  ```

### 4.3 Shape 3: Exact Timestamp Point Lookup

- **TQL shape (valid AST):**
  ```text
  MATCH HYPERCELL t: Tick OVER (s: Symbol)
  WHERE t.time == 20261027094215
  SOLVE HARMONIC (REGULARIZATION: 0.01, MAX_ITERATIONS: 25, ENERGY_THRESHOLD: 0.005)
  EMIT SECTION { t.price, t.size, SECTION_ENERGY };
  ```

- **Kdb+ native query (q):**
  ```q
  select price, size from quotes where time = 2026.10.27D09:42:15.000000000
  ```

- **DuckDB SQL equivalent:**
  ```sql
  SELECT price, size FROM quotes WHERE ts = TIMESTAMP '2026-10-27 09:42:15';
  ```

---

## 5. Measured Live Time-Series Comparison Table (100,000 Rows)

This table answers how Kdb+, DuckDB, and ShelfDB perform across 100,000 tick records on the host. Every number matches `results/KDB_TIMESERIES_RESULTS.json`.

Rows represent the database engines. Columns give the window aggregation latency, the range slice filter latency, the exact point lookup latency, the full column average scan latency, and the execution status.

| Database Engine | Window Aggregation (us) | Range Slice Filter (us) | Point Lookup (us) | Full Column Average (us) | Status |
|---|---|---|---|---|---|
| Kdb+ 5f (q Internal Engine) | 17,260.0 | 10.0 | 0.0 | 40.0 | Pass |
| Kdb+ 5f (Python Client Interface) | 17,883.0 | 153.3 | 79.4 | 129.5 | Pass |
| ShelfDB BCSF Hot Path | 379.8 | 104.4 | 0.1 | 146.3 | Pass |
| DuckDB 1.5 | 32,375.2 | 2,591.0 | 1,585.9 | 1,532.2 | Pass |

---

## 6. Relational Four-Table Join Comparisons

### 6.1 Table 6A: Controlled 5-Row Fraud Dataset

This table answers how each engine performs on the identical five-row fraud dataset (`Payment` joining `Account`, `Merchant`, and `Country` with filter `amount > 2000 AND risk = 1`). Every number matches `results/KDB_JOIN_RESULTS.json`.

Rows represent the database engines. Columns give the join latency, the measurement timer type, and the execution status.

| Database Engine | Join Latency (us) | Timer Type | Execution Status | Remarks |
|---|---|---|---|---|
| SQLite 3 (In-Memory) | 4.3 | Client Timer | Pass | In-memory relational joins |
| Kdb+ 5f (q Internal Engine) | 7.0 | Internal q Timer | Pass | Keyed left join in q process |
| ShelfDB (Solver Core Only) | 12.0 | Core Solve Timer | Pass | Inner harmonic solve on single hypercell |
| Kdb+ 5f (Python Client Interface) | 23.4 | Client Timer | Pass | Python process IPC evaluation |
| DuckDB 1.5 (In-Memory) | 1,297.3 | Client Timer | Pass | In-memory vectorized table scan |

### 6.2 Table 6B: Suite Medians Across The 50 Test Cases

This table answers how the engines perform across all 50 high-variance test cases. Every number matches the medians in `results/HIGH_VARIANCE_50_RESULTS.json`.

Rows represent the database engines. Columns give the median query latency, the timer scope, and the execution status.

| Database Engine | Median Latency (us) | Timer Scope | Execution Status | Remarks |
|---|---|---|---|---|
| ShelfDB Inner Solver Core | 2.0 | Solver Core Only | Pass | Harmonic relaxation solve on matched cells |
| SQLite 3 | 425.6 | Client Process | Pass | Single-connection SQLite query |
| PostgreSQL 18.6 | 2,133.1 | Client Process | Pass | Relational database connection pool |
| DuckDB 1.5 | 6,378.2 | Client Process | Pass | Vectorized chunk execution |
| ShelfDB Docker Process | 701,949.9 | Full Container Lifecycle | Pass | Includes Docker spawn overhead (702 ms) |

---

## 7. Engineering Findings

1. **Comparable Client Timers:** At the Python client interface, Kdb+ executed the 1-hour time slice filter in 153.3 &mu;s. DuckDB completed the same query in 2,591.0 &mu;s. Kdb+ was 16.9 times faster than DuckDB on client-to-client comparison.
2. **Kdb+ Internal Timer Scope:** Inside the q engine loop (`\t`), Kdb+ completed the slice filter in 10.0 &mu;s. Do not compare the 10.0 &mu;s internal timer directly against the 2,591.0 &mu;s DuckDB client timer.
3. **Controlled Five-Row Relational Join:** On the identical five-row fraud dataset, SQLite completed the in-memory join in 4.3 &mu;s. Kdb+ completed the join in 7.0 &mu;s internally and 23.4 &mu;s at the client. DuckDB took 1,297.3 &mu;s.
4. **ShelfDB Docker Latency:** The median wall time for ShelfDB in `results/HIGH_VARIANCE_50_RESULTS.json` is 701,949.9 &mu;s (702 ms). That time measures the container startup overhead. The internal ShelfDB solver median is 2.0 &mu;s.
