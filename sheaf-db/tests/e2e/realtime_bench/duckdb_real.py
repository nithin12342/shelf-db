"""DuckDB on REAL data: IEEE-CIS fraud aggregations + Facebook graph queries.
Usage: py duckdb_real.py   (duckdb pip package; reads the datasets root)
"""
import json
import os
import time

import duckdb

import bench_paths as P

OUT = str(P.RESULTS / "duckdb_real.json")
FRAUD = P.datasets("ieee-fraud", "train_transaction.csv")
FB = P.datasets("facebook_combined.txt.gz")


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    con = duckdb.connect()
    res = {"duckdb_version": duckdb.__version__}

    # --- fraud: real 590k x 394-col rows ---
    a = time.perf_counter()
    con.execute(f"CREATE TABLE fraud AS SELECT * FROM read_csv('{FRAUD}', header=true)")
    res["fraud_load_s"] = round(time.perf_counter() - a, 1)
    n = con.execute("SELECT COUNT(*), SUM(isFraud), AVG(TransactionAmt) FROM fraud").fetchone()
    res["fraud_counts"] = {"rows": n[0], "frauds": n[1], "avg_amt": round(n[2], 2)}
    print("fraud rows:", n[0], "frauds:", n[1])

    q_fraud = """SELECT ProductCD, COUNT(*), SUM(isFraud), AVG(TransactionAmt)
                 FROM fraud GROUP BY ProductCD ORDER BY 3 DESC"""
    ts = []
    for _ in range(3):
        a = time.perf_counter_ns()
        rows = con.execute(q_fraud).fetchall()
        ts.append((time.perf_counter_ns() - a) / 1e6)
    res["fraud_group_ms"] = {"p50": round(pct(ts, 0.5), 1), "groups": len(rows), "n": len(ts)}

    q_risk = """SELECT card4, AVG(TransactionAmt) FROM fraud WHERE isFraud=1 GROUP BY card4"""
    a = time.perf_counter_ns()
    rows = con.execute(q_risk).fetchall()
    res["fraud_risk_ms"] = {"ms": round((time.perf_counter_ns() - a) / 1e6, 1), "groups": len(rows)}

    # --- facebook: real 88k-edge graph ---
    a = time.perf_counter()
    con.execute(f"CREATE TABLE edge(a INT, b INT)")
    con.execute(f"COPY edge FROM '{FB}' (DELIMITER ' ')")
    res["fb_load_s"] = round(time.perf_counter() - a, 1)
    res["fb_edges"] = con.execute("SELECT COUNT(*) FROM edge").fetchone()[0]

    # 2-hop neighborhood size for 200 sampled nodes (recursive CTE)
    import random
    rng = random.Random(42)
    nodes = con.execute("SELECT DISTINCT a FROM edge USING SAMPLE 200").fetchall()
    nodes = [r[0] for r in nodes]
    ts = []
    for v in nodes:
        a = time.perf_counter_ns()
        con.execute(f"""WITH RECURSIVE hop(n, d) AS (
          SELECT {v}, 0 UNION
          SELECT CASE WHEN e.a=h.n THEN e.b ELSE e.a END, h.d+1 FROM hop h
          JOIN edge e ON e.a=h.n OR e.b=h.n WHERE h.d < 2)
          SELECT COUNT(DISTINCT n) FROM hop""").fetchone()
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["fb_2hop_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    # triangle count (clustering signal)
    a = time.perf_counter_ns()
    tri = con.execute("""SELECT COUNT(*) FROM edge e1 JOIN edge e2 ON e1.b=e2.a
                         JOIN edge e3 ON e3.a=e2.b AND e3.b=e1.a""").fetchone()[0]
    res["fb_triangles"] = {"n": tri, "ms": round((time.perf_counter_ns() - a) / 1e6, 1)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
