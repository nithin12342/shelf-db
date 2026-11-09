import time
import pykx as kx
import duckdb

print("=== KDB+ (PyKX) & DUCKDB TIME-SERIES BENCHMARK ===")
print("PyKX Licensed:", kx.licensed)
print("Q version:", kx.q('.z.K'))

# 1. SETUP KDB+ DATA
# Generate 100,000 tick rows in q
kx.q("""
n: 100000;
quotes: ([] 
    sym: n?`AAPL`GOOG`MSFT;
    time: 2026.10.27D09:00:00.000000000 + (til n) * 1000000000;
    price: 150.0 + (til n) mod 10 * 0.1;
    size: 100 + (til n) mod 50
);
""")

count_q = kx.q('count quotes').py()
print(f"Kdb+ loaded {count_q} quotes.")

# 2. BENCHMARK KDB+
# Warmup
kx.q("select avg price, sum size by 0D00:01:00 xbar time from quotes")

# Query 1: Window Aggregation (1-minute buckets)
kdb_window_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = kx.q("select avg price, sum size by 0D00:01:00 xbar time from quotes")
    t1 = time.perf_counter()
    kdb_window_times.append((t1 - t0) * 1e6)

# Query 2: Range Slice Filter (1-hour slice)
kdb_slice_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = kx.q("select avg price, sum size from quotes where time within (2026.10.27D09:15:00.000000000; 2026.10.27D10:15:00.000000000)")
    t1 = time.perf_counter()
    kdb_slice_times.append((t1 - t0) * 1e6)

# Query 3: Point Lookup
kdb_point_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = kx.q("select price, size from quotes where time = 2026.10.27D09:42:15.000000000")
    t1 = time.perf_counter()
    kdb_point_times.append((t1 - t0) * 1e6)

# Query 4: Vectorized Cumulative moving average / scan
kdb_scan_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = kx.q("exec avg price from quotes")
    t1 = time.perf_counter()
    kdb_scan_times.append((t1 - t0) * 1e6)

# Also measure native q timing via \t in q itself (excluding Python bridge overhead)
q_window_internal_ms = kx.q(r"\t do[100; select avg price, sum size by 0D00:01:00 xbar time from quotes]").py() / 100.0 * 1000.0
q_slice_internal_ms = kx.q(r"\t do[100; select avg price, sum size from quotes where time within (2026.10.27D09:15:00.000000000; 2026.10.27D10:15:00.000000000)]").py() / 100.0 * 1000.0
q_point_internal_ms = kx.q(r"\t do[100; select price, size from quotes where time = 2026.10.27D09:42:15.000000000]").py() / 100.0 * 1000.0
q_scan_internal_ms = kx.q(r"\t do[100; exec avg price from quotes]").py() / 100.0 * 1000.0

# 3. DUCKDB RUN
con = duckdb.connect()
con.execute("""
    CREATE TABLE quotes (
        sym VARCHAR,
        ts TIMESTAMP,
        price DOUBLE,
        size INT
    );
""")
con.execute("""
    INSERT INTO quotes
    SELECT 
        'AAPL',
        TIMESTAMP '2026-10-27 09:00:00' + (i * INTERVAL 1 SECOND),
        150.0 + (i % 10) * 0.1,
        100 + (i % 50)
    FROM range(100000) t(i);
""")

duck_window_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = con.execute("SELECT time_bucket(INTERVAL '1 minute', ts) as b, AVG(price), SUM(size) FROM quotes GROUP BY 1").fetchall()
    t1 = time.perf_counter()
    duck_window_times.append((t1 - t0) * 1e6)

duck_slice_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = con.execute("SELECT AVG(price), SUM(size) FROM quotes WHERE ts BETWEEN TIMESTAMP '2026-10-27 09:15:00' AND TIMESTAMP '2026-10-27 10:15:00'").fetchall()
    t1 = time.perf_counter()
    duck_slice_times.append((t1 - t0) * 1e6)

duck_point_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = con.execute("SELECT price, size FROM quotes WHERE ts = TIMESTAMP '2026-10-27 09:42:15'").fetchall()
    t1 = time.perf_counter()
    duck_point_times.append((t1 - t0) * 1e6)

duck_scan_times = []
for _ in range(25):
    t0 = time.perf_counter()
    res = con.execute("SELECT AVG(price) FROM quotes").fetchall()
    t1 = time.perf_counter()
    duck_scan_times.append((t1 - t0) * 1e6)

med = lambda l: sorted(l)[len(l)//2]

print("\n--- RESULTS (100,000 ROWS) ---")
print(f"Query 1: 1-Min Bucket Aggregation:")
print(f"  Kdb+ (q internal engine): {q_window_internal_ms:.1f} us")
print(f"  Kdb+ (via Python IPC/eval): {med(kdb_window_times):.1f} us")
print(f"  DuckDB: {med(duck_window_times):.1f} us")

print(f"\nQuery 2: 1-Hour Time Slice Filter:")
print(f"  Kdb+ (q internal engine): {q_slice_internal_ms:.1f} us")
print(f"  Kdb+ (via Python IPC/eval): {med(kdb_slice_times):.1f} us")
print(f"  DuckDB: {med(duck_slice_times):.1f} us")

print(f"\nQuery 3: Exact Timestamp Point Lookup:")
print(f"  Kdb+ (q internal engine): {q_point_internal_ms:.1f} us")
print(f"  Kdb+ (via Python IPC/eval): {med(kdb_point_times):.1f} us")
print(f"  DuckDB: {med(duck_point_times):.1f} us")

print(f"\nQuery 4: Full Column Scan & Average:")
print(f"  Kdb+ (q internal engine): {q_scan_internal_ms:.1f} us")
print(f"  Kdb+ (via Python IPC/eval): {med(kdb_scan_times):.1f} us")
print(f"  DuckDB: {med(duck_scan_times):.1f} us")

# Save JSON results
import json
out = {
    "kdb_internal_us": {
        "window_agg": q_window_internal_ms,
        "slice_filter": q_slice_internal_ms,
        "point_lookup": q_point_internal_ms,
        "column_avg": q_scan_internal_ms
    },
    "kdb_eval_us": {
        "window_agg": med(kdb_window_times),
        "slice_filter": med(kdb_slice_times),
        "point_lookup": med(kdb_point_times),
        "column_avg": med(kdb_scan_times)
    },
    "duckdb_us": {
        "window_agg": med(duck_window_times),
        "slice_filter": med(duck_slice_times),
        "point_lookup": med(duck_point_times),
        "column_avg": med(duck_scan_times)
    }
}
with open("results/KDB_TIMESERIES_RESULTS.json", "w") as f:
    json.dump(out, f, indent=2)
print("\nWrote results/KDB_TIMESERIES_RESULTS.json successfully.")
