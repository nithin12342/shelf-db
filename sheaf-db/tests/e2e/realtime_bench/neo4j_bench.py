"""Neo4j E2E bench: point lookups, 2-hop traversals, deep paths. Emits JSON.
Usage: py neo4j_bench.py   (requires the configured Neo4j server, database neo4j imported)
"""
import json
import os
import random
import time

from neo4j import GraphDatabase

import bench_paths as P

OUT = str(P.RESULTS / "neo4j.json")
SEED = 42


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    neo = P.service("neo4j")
    d = GraphDatabase.driver(neo.uri("bolt"), auth=(neo.user, neo.password))
    res = {}
    with d.session() as s:
        res["neo4j_version"] = s.run("CALL dbms.components() YIELD versions RETURN versions[0] AS v").single()["v"]
        res["counts"] = {
            "customers": s.run("MATCH (c:Customer) RETURN count(c) AS n").single()["n"],
            "orders": s.run("MATCH (o:Order) RETURN count(o) AS n").single()["n"],
            "lines": s.run("MATCH (l:Line) RETURN count(l) AS n").single()["n"],
        }
        print(res["counts"])
        s.run("CREATE INDEX cust_id IF NOT EXISTS FOR (c:Customer) ON (c.custkey)").consume()
        s.run("CREATE INDEX ord_id IF NOT EXISTS FOR (o:Order) ON (o.orderkey)").consume()

        rng = random.Random(SEED)
        keys = rng.sample(range(1, 150001), 5000)
        ts = []
        for k in keys:
            a = time.perf_counter_ns()
            list(s.run("MATCH (c:Customer {custkey: $k}) RETURN c.name", k=k))
            ts.append((time.perf_counter_ns() - a) / 1000.0)
        res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

        # 2-hop: customer -> orders -> lines, aggregate spend for sampled customers
        samp = rng.sample(range(1, 150001), 200)
        ts = []
        for k in samp:
            a = time.perf_counter_ns()
            list(s.run("MATCH (c:Customer {custkey: $k})-[:PLACED]->(o)-[:CONTAINS]->(l) RETURN sum(l.price)", k=k))
            ts.append((time.perf_counter_ns() - a) / 1000.0)
        res["two_hop_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

        # deep traversal: all 2-hop paths rolled up (bounded aggregation over full graph)
        ts = []
        for _ in range(3):
            a = time.perf_counter_ns()
            n = s.run("MATCH (c:Customer)-[:PLACED]->(o:Order)-[:CONTAINS]->(l:Line) RETURN count(l) AS n").single()["n"]
            ts.append((time.perf_counter_ns() - a) / 1e6)
        res["full_traversal_ms"] = {"p50": round(pct(ts, 0.5), 1), "paths": n, "n": len(ts)}
    d.close()
    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
