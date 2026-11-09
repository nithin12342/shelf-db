"""One-shot: correct undirected triangle count on facebook edges; patch duckdb_real.json."""
import json
import time

import duckdb

import bench_paths as P

OUT = str(P.RESULTS / "duckdb_real.json")
FB = P.datasets("facebook_combined.txt.gz")

con = duckdb.connect()
con.execute("CREATE TABLE edge(a INT, b INT)")
con.execute(f"COPY edge FROM '{FB}' (DELIMITER ' ')")
print("a>b rows:", con.execute("SELECT COUNT(*) FROM edge WHERE a>b").fetchone()[0])
t0 = time.perf_counter_ns()
tri = con.execute("""SELECT COUNT(*) FROM edge e1
                     JOIN edge e2 ON e2.a=e1.a AND e2.b>e1.b
                     JOIN edge e3 ON e3.a=e1.b AND e3.b=e2.b""").fetchone()[0]
ms = round((time.perf_counter_ns() - t0) / 1e6, 1)
print("triangles:", tri, "ms:", ms)
with open(OUT) as f:
    d = json.load(f)
d["fb_triangles"] = {"n": tri, "ms": ms,
                     "note": "corrected undirected wedge query; earlier 0 was an orientation bug in the query, not the data"}
with open(OUT, "w") as f:
    json.dump(d, f, indent=2)
print("JSON corrected")
