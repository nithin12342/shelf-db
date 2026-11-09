"""DuckDB E2E bench: load TPC-H SF1, run timed queries, emit results JSON.
Usage: py duckdb_bench.py   (requires the duckdb pip package)
"""
import json
import os
import random
import time

import duckdb

import bench_paths as P

DATA = P.build("tpch-dbgen")
DB = P.data("duckdb", "bench.duckdb")
OUT = str(P.RESULTS / "duckdb.json")
SEED = 42

COLS = {
    "customer": "c_custkey INT, c_name VARCHAR, c_address VARCHAR, c_nationkey INT, c_phone VARCHAR, c_acctbal DOUBLE, c_mktsegment VARCHAR, c_comment VARCHAR",
    "orders": "o_orderkey INT, o_custkey INT, o_orderstatus VARCHAR, o_totalprice DOUBLE, o_orderdate DATE, o_orderpriority VARCHAR, o_clerk VARCHAR, o_shippriority INT, o_comment VARCHAR",
    "lineitem": "l_orderkey INT, l_partkey INT, l_suppkey INT, l_linenumber INT, l_quantity DOUBLE, l_extendedprice DOUBLE, l_discount DOUBLE, l_tax DOUBLE, l_returnflag VARCHAR, l_linestatus VARCHAR, l_shipdate DATE, l_commitdate DATE, l_receiptdate DATE, l_shipinstruct VARCHAR, l_shipmode VARCHAR, l_comment VARCHAR",
    "part": "p_partkey INT, p_name VARCHAR, p_mfgr VARCHAR, p_brand VARCHAR, p_type VARCHAR, p_size INT, p_container VARCHAR, p_retailprice DOUBLE, p_comment VARCHAR",
    "partsupp": "ps_partkey INT, ps_suppkey INT, ps_availqty INT, ps_supplycost DOUBLE, ps_comment VARCHAR",
}
EXPECTED = {"customer": 150000, "orders": 1500000, "lineitem": 6001215, "part": 200000, "partsupp": 800000}


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(DB), exist_ok=True)
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    con = duckdb.connect(DB)
    for t, cols in COLS.items():
        try:
            n = con.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
        except Exception:
            n = -1
        if n == EXPECTED[t]:
            print(f"{t}: already loaded ({n}), skipping")
        else:
            a = time.perf_counter()
            con.execute(f"DROP TABLE IF EXISTS {t}")
            con.execute(f"CREATE TABLE {t} ({cols})")
            con.execute(f"COPY {t} FROM '{DATA}\\{t}.tbl' (DELIMITER '|')")
            print(f"{t}: loaded in {time.perf_counter() - a:.1f}s")
    counts = {t: con.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0] for t in COLS}
    assert counts == EXPECTED, counts
    res = {"counts": counts, "duckdb_version": duckdb.__version__}

    rng = random.Random(SEED)
    keys = rng.sample(range(1, 150001), 20000)
    ts = []
    for k in keys:
        a = time.perf_counter_ns()
        con.execute("SELECT * FROM customer WHERE c_custkey=?", [k]).fetchone()
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 2), "p99": round(pct(ts, 0.99), 2), "n": len(ts)}

    jq = """SELECT c_name, SUM(l_extendedprice) FROM customer JOIN orders ON o_custkey=c_custkey
            JOIN lineitem ON l_orderkey=o_orderkey WHERE l_shipdate < DATE '1995-01-01'
            GROUP BY c_name ORDER BY 2 DESC LIMIT 100"""
    ts = []
    for _ in range(3):
        a = time.perf_counter_ns()
        rows = con.execute(jq).fetchall()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["join_ms"] = {"p50": round(pct(ts, 0.5), 1), "rows": len(rows), "n": len(ts)}

    ts = []
    for _ in range(5):
        a = time.perf_counter_ns()
        con.execute("SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0").fetchone()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["scan_agg_ms"] = {"p50": round(pct(ts, 0.5), 1), "n": len(ts)}

    ts = []
    for i in range(10000):
        a = time.perf_counter_ns()
        con.execute("INSERT INTO part VALUES (?,?,?,?,?,?,?,?,?)",
                    [300000 + i, "bench", "mfgr", "brand", "type", 1, "box", 9.99, ""])
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["insert_us"] = {"p50": round(pct(ts, 0.5), 2), "p99": round(pct(ts, 0.99), 2), "n": len(ts)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
