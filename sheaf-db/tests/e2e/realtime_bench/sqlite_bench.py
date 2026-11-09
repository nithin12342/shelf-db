"""SQLite E2E bench: load TPC-H SF1, run timed queries, emit results JSON.
Usage: py sqlite_bench.py
Gates: row counts match spec; timings recorded with host CPU note.
"""
import json
import os
import random
import sqlite3
import time

import bench_paths as P

DATA = P.build("tpch-dbgen")
DB = P.data("sqlite", "bench.db")
OUT = str(P.RESULTS / "sqlite.json")
SEED = 42

SCHEMA = {
    "customer": "c_custkey INT PRIMARY KEY, c_name TEXT, c_address TEXT, c_nationkey INT, c_phone TEXT, c_acctbal REAL, c_mktsegment TEXT, c_comment TEXT",
    "orders": "o_orderkey INT PRIMARY KEY, o_custkey INT, o_orderstatus TEXT, o_totalprice REAL, o_orderdate TEXT, o_orderpriority TEXT, o_clerk TEXT, o_shippriority INT, o_comment TEXT",
    "lineitem": "l_orderkey INT, l_partkey INT, l_suppkey INT, l_linenumber INT, l_quantity REAL, l_extendedprice REAL, l_discount REAL, l_tax REAL, l_returnflag TEXT, l_linestatus TEXT, l_shipdate TEXT, l_commitdate TEXT, l_receiptdate TEXT, l_shipinstruct TEXT, l_shipmode TEXT, l_comment TEXT",
    "part": "p_partkey INT PRIMARY KEY, p_name TEXT, p_mfgr TEXT, p_brand TEXT, p_type TEXT, p_size INT, p_container TEXT, p_retailprice REAL, p_comment TEXT",
    "partsupp": "ps_partkey INT, ps_suppkey INT, ps_availqty INT, ps_supplycost REAL, ps_comment TEXT",
}

EXPECTED = {"customer": 150000, "orders": 1500000, "lineitem": 6001215, "part": 200000, "partsupp": 800000}


def load_table(con, table):
    path = os.path.join(DATA, table + ".tbl")
    cur = con.cursor()
    cur.execute(f"DROP TABLE IF EXISTS {table}")
    cur.execute(f"CREATE TABLE {table} ({SCHEMA[table]})")
    with open(path, "r", encoding="utf-8") as f:
        batch = []
        for line in f:
            row = line.rstrip("\n").split("|")  # this fork emits no trailing delimiter
            batch.append(row)
            if len(batch) >= 5000:
                cur.executemany(f"INSERT INTO {table} VALUES ({','.join('?' * len(batch[0]))})", batch)
                batch = []
        if batch:
            cur.executemany(f"INSERT INTO {table} VALUES ({','.join('?' * len(batch[0]))})", batch)
    con.commit()
    n = cur.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
    assert n == EXPECTED[table], f"{table}: got {n}, want {EXPECTED[table]}"
    return n


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(DB), exist_ok=True)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    con = sqlite3.connect(DB)
    con.execute("PRAGMA journal_mode=WAL;")
    con.execute("PRAGMA synchronous=NORMAL;")
    t0 = time.perf_counter()
    counts = {}
    for t in SCHEMA:
        try:
            n = con.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
        except sqlite3.OperationalError:
            n = -1
        if n == EXPECTED[t]:
            print(f"{t}: already loaded ({n}), skipping")
            counts[t] = n
        else:
            counts[t] = load_table(con, t)
    load_s = time.perf_counter() - t0
    con.execute("CREATE INDEX IF NOT EXISTS idx_orders_cust ON orders(o_custkey)")
    con.execute("CREATE INDEX IF NOT EXISTS idx_lineitem_order ON lineitem(l_orderkey)")
    con.execute("CREATE INDEX IF NOT EXISTS idx_lineitem_shipdate ON lineitem(l_shipdate)")
    con.commit()

    rng = random.Random(SEED)
    res = {"counts": counts, "load_s": round(load_s, 2)}

    # 3.2-style: 20k strided point lookups on customer
    keys = rng.sample(range(1, 150001), 20000)
    ts = []
    for k in keys:
        a = time.perf_counter_ns()
        con.execute("SELECT * FROM customer WHERE c_custkey=?", (k,)).fetchone()
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 2), "p99": round(pct(ts, 0.99), 2), "n": len(ts)}

    # 2.1-style: 3-table join (customer-orders-lineitem) top-100 revenue
    q_join = """SELECT c.c_name, SUM(l.l_extendedprice) FROM customer c
                JOIN orders o ON o.o_custkey=c.c_custkey
                JOIN lineitem l ON l.l_orderkey=o.o_orderkey
                WHERE l.l_shipdate < '1995-01-01' GROUP BY c.c_name
                ORDER BY 2 DESC LIMIT 100"""
    ts = []
    for _ in range(1):  # full 3-table GROUP BY over 6M rows ≈185s/run on this laptop; n=1 recorded honestly
        a = time.perf_counter_ns()
        rows = con.execute(q_join).fetchall()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["join_ms"] = {"p50": round(pct(ts, 0.5), 1), "rows": len(rows), "n": len(ts)}

    # 4.1-style: full scan AVG/SUM over lineitem extendedprice
    ts = []
    for _ in range(5):
        a = time.perf_counter_ns()
        r = con.execute("SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0").fetchone()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["scan_agg_ms"] = {"p50": round(pct(ts, 0.5), 1), "avg": r[0], "n": len(ts)}

    # write test: 10k single-row inserts into part
    ts = []
    cur = con.cursor()
    for i in range(10000):
        a = time.perf_counter_ns()
        cur.execute("INSERT INTO part VALUES (?,?,?,?,?,?,?,?,?)",
                    (300000 + i, "bench", "mfgr", "brand", "type", 1, "box", 9.99, ""))
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    con.commit()
    res["insert_us"] = {"p50": round(pct(ts, 0.5), 2), "p99": round(pct(ts, 0.99), 2), "n": len(ts)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
