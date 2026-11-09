"""ScyllaDB E2E bench: KV table, point reads, range scatter, write-flood reads.
Usage: py scylla_bench.py   (requires the configured ScyllaDB server)
"""
import json
import os
import random
import time

from cassandra.cluster import Cluster
from cassandra.query import BatchStatement

import bench_paths as P

OUT = str(P.RESULTS / "scylladb.json")
SEED = 42
NKEYS = 500_000


def pct(times, p):
    s = sorted(times)
    return s[min(len(s) - 1, int(p * len(s)))]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    scylla = P.service("scylla")
    cl = Cluster([scylla.host], port=scylla.port)
    se = cl.connect()
    res = {"scylla_version": se.execute("SELECT release_version FROM system.local").one()[0]}
    se.execute("CREATE KEYSPACE IF NOT EXISTS bench WITH replication={'class':'SimpleStrategy','replication_factor':1}")
    se.set_keyspace("bench")
    n = 0
    try:
        n = se.execute("SELECT COUNT(*) FROM kv").one()[0]
    except Exception:
        pass
    if n == NKEYS:
        print(f"already loaded ({n}), skipping")
    else:
        se.execute("DROP TABLE IF EXISTS kv")
        se.execute("CREATE TABLE kv (rng INT, id INT, payload TEXT, PRIMARY KEY (rng, id))")
        a = time.perf_counter()
        batch, count = BatchStatement(), 0
        for i in range(1, NKEYS + 1):
            batch.add("INSERT INTO kv (rng, id, payload) VALUES (%s,%s,%s)", (i % 50, i, "x" * 48))
            count += 1
            if count % 200 == 0:
                se.execute(batch)
                batch = BatchStatement()
        if len(batch) > 0:
            se.execute(batch)
        res["load_s"] = round(time.perf_counter() - a, 1)
        print(f"loaded in {res['load_s']}s")
    n = se.execute("SELECT COUNT(*) FROM kv").one()[0]
    assert n == NKEYS, n
    res["counts"] = {"kv": n}

    rng = random.Random(SEED)
    keys = [(rng.randrange(50), rng.randint(1, NKEYS)) for _ in range(5000)]
    stmt = se.prepare("SELECT payload FROM kv WHERE rng=? AND id=?")
    ts = []
    for rk, i in keys:
        a = time.perf_counter_ns()
        list(se.execute(stmt, (rk, i)))
        ts.append((time.perf_counter_ns() - a) / 1000.0)
    res["point_lookup_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    # scatter analog: one range's full slice
    a = time.perf_counter_ns()
    rows = list(se.execute("SELECT id FROM kv WHERE rng=7"))
    res["range_slice_ms"] = {"ms": round((time.perf_counter_ns() - a) / 1e6, 1), "rows": len(rows)}

    # write flood + concurrent read p99
    def flood():
        for j in range(20000):
            se.execute("INSERT INTO kv (rng, id, payload) VALUES (%s,%s,%s)", (j % 50, NKEYS + j + 1, "y" * 48))
    import threading
    th = threading.Thread(target=flood)
    th.start()
    ts = []
    while th.is_alive():
        a = time.perf_counter_ns()
        list(se.execute(stmt, (3, 12345)))
        ts.append((time.perf_counter_ns() - a) / 1000.0)
        if len(ts) > 2000:
            break
    th.join()
    res["flood_read_us"] = {"p50": round(pct(ts, 0.5), 1), "p99": round(pct(ts, 0.99), 1), "n": len(ts)}

    with open(OUT, "w") as f:
        json.dump(res, f, indent=2)
    print(json.dumps(res, indent=2))
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
