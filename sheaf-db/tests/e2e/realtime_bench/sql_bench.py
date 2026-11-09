"""Shared SQL bench for MariaDB / PostgreSQL: TPC-H SF1 load + timed queries.
Usage: py sql_bench.py mariadb  |  py sql_bench.py pg
Expects the configured MariaDB and PostgreSQL servers.
"""
import json
import os
import random
import sys
import time

import bench_paths as P

DATA = P.build("tpch-dbgen")
OUTDIR = str(P.RESULTS)
SEED = 42

SCHEMA_COMMON = {
    "customer": "(c_custkey INT PRIMARY KEY, c_name VARCHAR(64), c_address VARCHAR(128), c_nationkey INT, c_phone VARCHAR(32), c_acctbal DOUBLE, c_mktsegment VARCHAR(32), c_comment VARCHAR(256))",
    "orders": "(o_orderkey INT PRIMARY KEY, o_custkey INT, o_orderstatus CHAR(1), o_totalprice DOUBLE, o_orderdate DATE, o_orderpriority VARCHAR(32), o_clerk VARCHAR(64), o_shippriority INT, o_comment VARCHAR(256))",
    "lineitem": "(l_orderkey INT, l_partkey INT, l_suppkey INT, l_linenumber INT, l_quantity DOUBLE, l_extendedprice DOUBLE, l_discount DOUBLE, l_tax DOUBLE, l_returnflag CHAR(1), l_linestatus CHAR(1), l_shipdate DATE, l_commitdate DATE, l_receiptdate DATE, l_shipinstruct VARCHAR(64), l_shipmode VARCHAR(32), l_comment VARCHAR(256))",
    "part": "(p_partkey INT PRIMARY KEY, p_name VARCHAR(128), p_mfgr VARCHAR(64), p_brand VARCHAR(32), p_type VARCHAR(64), p_size INT, p_container VARCHAR(32), p_retailprice DOUBLE, p_comment VARCHAR(256))",
    "partsupp": "(ps_partkey INT, ps_suppkey INT, ps_availqty INT, ps_supplycost DOUBLE, ps_comment VARCHAR(256))",
}
EXPECTED = {"customer": 150000, "orders": 1500000, "lineitem": 6001215, "part": 200000, "partsupp": 800000}


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def connect(engine):
    svc = P.service(engine)
    if engine == "mariadb":
        import pymysql
        return pymysql.connect(host=svc.host, port=svc.port, user=svc.user,
                               password=svc.password, database="bench", local_infile=True)
    import psycopg
    return psycopg.connect(host=svc.host, port=svc.port, user=svc.user,
                           password=svc.password, dbname="bench")


def main():
    engine = sys.argv[1]
    os.makedirs(OUTDIR, exist_ok=True)
    con = connect(engine)
    cur = con.cursor()
    res = {"engine": engine}
    float_t = "DOUBLE PRECISION" if engine == "pg" else "DOUBLE"
    schema = {t: s.replace("DOUBLE", float_t) for t, s in SCHEMA_COMMON.items()}
    try:
        cur.execute("SELECT VERSION()")
        res["version"] = cur.fetchone()[0]
    except Exception as e:
        res["version"] = str(e)
    print("server:", res["version"][:80])

    counts = {}
    for t in SCHEMA_COMMON:
        try:
            cur.execute(f"SELECT COUNT(*) FROM {t}")
            n = cur.fetchone()[0]
        except Exception:
            con.rollback()
            n = -1
        if n == EXPECTED[t]:
            print(f"{t}: already loaded ({n}), skipping")
        else:
            a = time.perf_counter()
            cur.execute(f"DROP TABLE IF EXISTS {t}")
            cur.execute(f"CREATE TABLE {t} {schema[t]}")
            if engine == "mariadb":
                fpath = f"{DATA}\\{t}.tbl".replace("\\", "/")
                cur.execute(f"LOAD DATA LOCAL INFILE '{fpath}' INTO TABLE {t} FIELDS TERMINATED BY '|'")
            else:
                with open(f"{DATA}\\{t}.tbl", "r", encoding="utf-8") as f:
                    with cur.copy(f"COPY {t} FROM STDIN (DELIMITER '|')") as cp:
                        for line in f:
                            cp.write(line)
            con.commit()
            print(f"{t}: loaded in {time.perf_counter() - a:.1f}s")
            cur.execute(f"SELECT COUNT(*) FROM {t}")
            n = cur.fetchone()[0]
        counts[t] = n
    assert counts == EXPECTED, counts
    res["counts"] = counts
    cur.execute("CREATE INDEX IF NOT EXISTS idx_orders_cust ON orders(o_custkey)" if engine == "mariadb"
            else "CREATE INDEX IF NOT EXISTS idx_orders_cust ON orders(o_custkey)")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_lineitem_order ON lineitem(l_orderkey)" if engine == "mariadb"
            else "CREATE INDEX IF NOT EXISTS idx_lineitem_order ON lineitem(l_orderkey)")
    con.commit()

    rng = random.Random(SEED)
    keys = rng.sample(range(1, 150001), 20000)
    ph = "%s" if engine == "mariadb" else "%s"
    ts = []
    for k in keys:
        a = time.perf_counter_ns()
        cur.execute(f"SELECT * FROM customer WHERE c_custkey={ph}", (k,))
        cur.fetchone()
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    jq = """SELECT c_name, SUM(l_extendedprice) FROM customer JOIN orders ON o_custkey=c_custkey
            JOIN lineitem ON l_orderkey=o_orderkey WHERE l_shipdate < DATE '1995-01-01'
            GROUP BY c_name ORDER BY 2 DESC LIMIT 100"""
    ts = []
    for _ in range(3):
        a = time.perf_counter_ns()
        cur.execute(jq)
        rows = cur.fetchall()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["join_ms"] = {"p50": round(pct(ts, 0.5), 1), "rows": len(rows), "n": len(ts)}

    ts = []
    for _ in range(5):
        a = time.perf_counter_ns()
        cur.execute("SELECT AVG(l_extendedprice), SUM(l_extendedprice) FROM lineitem WHERE l_extendedprice > 100.0")
        cur.fetchone()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["scan_agg_ms"] = {"p50": round(pct(ts, 0.5), 1), "n": len(ts)}

    ts = []
    for i in range(10000):
        a = time.perf_counter_ns()
        cur.execute("INSERT INTO part VALUES (" + ",".join([ph] * 9) + ")",
                    (300000 + i, "bench", "mfgr", "brand", "type", 1, "box", 9.99, ""))
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    con.commit()
    res["insert_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    out = os.path.join(OUTDIR, f"{engine}.json")
    with open(out, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", out)


if __name__ == "__main__":
    main()
