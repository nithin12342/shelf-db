import time
import pykx as kx
import sqlite3
import duckdb
import json

# 1. KDB+ SETUP & BENCHMARK (5-ROW FRAUD DATASET)
kx.q("""
accounts: ([acct_id: 1001 1002 1003 1004 1005] 
    name: `Alice`Bob`Carol`Dave`Eve; 
    balance: 5000 300 9999 12000 750
);

merchants: ([merchant_id: 2001 2002 2003] 
    category: `grocery`travel`electronics; 
    is_rooted: 010b
);

countries: ([c_code: `US`UK] 
    region: `na`eu
);

payments: ([] 
    pay_id: 90001 90002 90003 90004 90005; 
    acct_id: 1001 1002 1003 1004 1005; 
    merchant_id: 2001 2002 2001 2002 2003; 
    c_code: `US`UK`US`UK`US; 
    amount: 5000 300 9999 12000 750; 
    risk: 1 0 1 0 1
);
""")

# Warmup
kx.q("select from ((payments lj accounts) lj merchants) lj countries where amount > 2000, risk = 1")

# Measure internal q time (1000 iterations)
kdb_internal_us = kx.q(r"\t do[1000; select from ((payments lj accounts) lj merchants) lj countries where amount > 2000, risk = 1]").py() / 1000.0 * 1000.0

# Measure outer client time (per-call in Python)
kdb_eval_times = []
for _ in range(50):
    t0 = time.perf_counter()
    res = kx.q("select from ((payments lj accounts) lj merchants) lj countries where amount > 2000, risk = 1")
    t1 = time.perf_counter()
    kdb_eval_times.append((t1 - t0) * 1e6)
kdb_eval_times.sort()
kdb_client_us = kdb_eval_times[len(kdb_eval_times)//2]

# 2. SQLITE SETUP & BENCHMARK (EXACT SAME 5-ROW FRAUD DATASET)
sq_con = sqlite3.connect(":memory:")
sq_cur = sq_con.cursor()
sq_cur.execute("CREATE TABLE account (acct_id INT PRIMARY KEY, name TEXT, balance INT);")
sq_cur.execute("CREATE TABLE merchant (merchant_id INT PRIMARY KEY, category TEXT, is_rooted INT);")
sq_cur.execute("CREATE TABLE country (c_code TEXT PRIMARY KEY, region TEXT);")
sq_cur.execute("CREATE TABLE payment (pay_id INT PRIMARY KEY, acct_id INT, merchant_id INT, c_code TEXT, amount INT, risk INT);")

sq_cur.executemany("INSERT INTO account VALUES (?, ?, ?)", [
    (1001, "Alice", 5000), (1002, "Bob", 300), (1003, "Carol", 9999), (1004, "Dave", 12000), (1005, "Eve", 750)
])
sq_cur.executemany("INSERT INTO merchant VALUES (?, ?, ?)", [
    (2001, "grocery", 0), (2002, "travel", 1), (2003, "electronics", 0)
])
sq_cur.executemany("INSERT INTO country VALUES (?, ?)", [
    ("US", "na"), ("UK", "eu")
])
sq_cur.executemany("INSERT INTO payment VALUES (?, ?, ?, ?, ?, ?)", [
    (90001, 1001, 2001, "US", 5000, 1),
    (90002, 1002, 2002, "UK", 300, 0),
    (90003, 1003, 2001, "US", 9999, 1),
    (90004, 1004, 2002, "UK", 12000, 0),
    (90005, 1005, 2003, "US", 750, 1)
])
sq_con.commit()

sql_query = """
SELECT p.pay_id, a.name, m.category, c.region, p.amount, p.risk
FROM payment p
JOIN account a ON p.acct_id = a.acct_id
JOIN merchant m ON p.merchant_id = m.merchant_id
JOIN country c ON p.c_code = c.c_code
WHERE p.amount > 2000 AND p.risk = 1;
"""
# Warmup
sq_cur.execute(sql_query).fetchall()

sqlite_times = []
for _ in range(50):
    t0 = time.perf_counter()
    r = sq_cur.execute(sql_query).fetchall()
    t1 = time.perf_counter()
    sqlite_times.append((t1 - t0) * 1e6)
sqlite_times.sort()
sqlite_med_us = sqlite_times[len(sqlite_times)//2]

# 3. DUCKDB SETUP & BENCHMARK (EXACT SAME 5-ROW FRAUD DATASET)
dk_con = duckdb.connect()
dk_con.execute("CREATE TABLE account (acct_id INT, name VARCHAR, balance INT);")
dk_con.execute("CREATE TABLE merchant (merchant_id INT, category VARCHAR, is_rooted INT);")
dk_con.execute("CREATE TABLE country (c_code VARCHAR, region VARCHAR);")
dk_con.execute("CREATE TABLE payment (pay_id INT, acct_id INT, merchant_id INT, c_code VARCHAR, amount INT, risk INT);")

dk_con.executemany("INSERT INTO account VALUES (?, ?, ?)", [
    (1001, "Alice", 5000), (1002, "Bob", 300), (1003, "Carol", 9999), (1004, "Dave", 12000), (1005, "Eve", 750)
])
dk_con.executemany("INSERT INTO merchant VALUES (?, ?, ?)", [
    (2001, "grocery", 0), (2002, "travel", 1), (2003, "electronics", 0)
])
dk_con.executemany("INSERT INTO country VALUES (?, ?)", [
    ("US", "na"), ("UK", "eu")
])
dk_con.executemany("INSERT INTO payment VALUES (?, ?, ?, ?, ?, ?)", [
    (90001, 1001, 2001, "US", 5000, 1),
    (90002, 1002, 2002, "UK", 300, 0),
    (90003, 1003, 2001, "US", 9999, 1),
    (90004, 1004, 2002, "UK", 12000, 0),
    (90005, 1005, 2003, "US", 750, 1)
])

# Warmup
dk_con.execute(sql_query).fetchall()

duck_times = []
for _ in range(50):
    t0 = time.perf_counter()
    r = dk_con.execute(sql_query).fetchall()
    t1 = time.perf_counter()
    duck_times.append((t1 - t0) * 1e6)
duck_times.sort()
duck_med_us = duck_times[len(duck_times)//2]

print("=== CONTROLLED 4-TABLE JOIN BENCHMARK (5-ROW DATASET) ===")
print(f"Kdb+ internal: {kdb_internal_us:.2f} us")
print(f"Kdb+ client: {kdb_client_us:.2f} us")
print(f"SQLite in-memory: {sqlite_med_us:.2f} us")
print(f"DuckDB in-memory: {duck_med_us:.2f} us")

out = {
    "dataset": "5-row fraud dataset",
    "kdb_internal_us": kdb_internal_us,
    "kdb_client_us": kdb_client_us,
    "sqlite_us": sqlite_med_us,
    "duckdb_us": duck_med_us
}
with open("results/KDB_JOIN_RESULTS.json", "w") as f:
    json.dump(out, f, indent=2)
print("Saved results/KDB_JOIN_RESULTS.json")
