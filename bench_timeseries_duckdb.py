import duckdb
import time

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
        100
    FROM range(100000) t(i);
""")

# Query 1: Time range slice (1 hour range filter)
timings_slice = []
for _ in range(20):
    t0 = time.perf_counter()
    res = con.execute("""
        SELECT AVG(price), SUM(size) 
        FROM quotes 
        WHERE ts BETWEEN TIMESTAMP '2026-10-27 09:15:00' AND TIMESTAMP '2026-10-27 10:15:00'
    """).fetchall()
    t1 = time.perf_counter()
    timings_slice.append((t1 - t0) * 1e6)

# Query 2: Point lookup at specific timestamp
timings_point = []
for _ in range(20):
    t0 = time.perf_counter()
    res = con.execute("""
        SELECT price, size 
        FROM quotes 
        WHERE ts = TIMESTAMP '2026-10-27 09:42:15'
    """).fetchall()
    t1 = time.perf_counter()
    timings_point.append((t1 - t0) * 1e6)

print(f"DuckDB 100k rows time slice filter: {sorted(timings_slice)[len(timings_slice)//2]:.1f} us (median)")
print(f"DuckDB 100k rows point lookup: {sorted(timings_point)[len(timings_point)//2]:.1f} us (median)")
