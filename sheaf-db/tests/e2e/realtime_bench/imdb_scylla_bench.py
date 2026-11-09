"""imdb_scylla_bench.py — IMDb complex case on ScyllaDB (CQL, wide-column).
Replaces MongoDB as the NoSQL opponent (uninstalled 2026-10-20: its
$lookup join cost 183,618.6 ms on this data vs ShelfDB's 142.7 ms).

CQL has no joins, so the join is resolved at WRITE time (the classic NoSQL
trade) and the query is a clustering-range scan:
    PRIMARY KEY ((year), avg, tconst)
    SELECT count(*), sum(votes), avg(avg) FROM titles
      WHERE year >= 2000 AND avg >= 7.5
Both year (partition) and avg (clustering) are key columns, so no
ALLOW FILTERING is needed.

Rust-first: the write-time join is done by the Rust binary
`imdb_scylla_prep` (the --out path); this script is
only the CQL transport (insert + query + gate).
Gate: count / votes / avg must equal the shared truth
(44,906 / 294,435,444 / 8.1455061684) and be > 100x faster than
MongoDB's 183,618.6 ms query on the same data.
"""
import json
import os
import struct
import subprocess
import time

from cassandra.cluster import Cluster

import bench_paths as P

ROWS = P.data("imdb_scylla_rows.bin")
OUT = str(P.RESULTS / "imdb_scylla.json")
PREP_BIN = str(P.SHEAF_DB / "crates" / "sheaf-engine" / "src" / "bin" / "imdb_scylla_prep.rs")
CARGO = r"cargo"
KEYSPACE = "imdb_bench"
TABLE = "titles"
TRUTH = {"count": 44906, "votes": 294435444, "avg": 8.145506168440683}
MONGO_QUERY_MS = 183618.6
MIN_YEAR = 2000
MIN_AVG = 7.5


def ensure_rows():
    """Run the Rust prep if the joined-rows file is missing."""
    if os.path.exists(ROWS):
        return
    print("rows file missing -> running the Rust prep binary", flush=True)
    subprocess.run(
        [CARGO, "run", "-q", "--release", "--manifest-path",
         str(P.SHEAF_DB / "Cargo.toml"), "-p", "sheaf-engine",
         "--bin", "imdb_scylla_prep", "--", "--imdb", P.imdb(), "--out", ROWS],
        check=True,
    )


def read_rows():
    with open(ROWS, "rb") as f:
        data = f.read()
    magic, count = struct.unpack_from("<QQ", data, 0)
    assert magic == 0x5343594C524F5753, "bad rows magic"
    return count, data[16:]


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    ensure_rows()
    count_rows, blob = read_rows()

    scylla = P.service("scylla")
    cl = Cluster([scylla.host], port=scylla.port)
    ses = cl.connect()
    ses.execute(f"DROP KEYSPACE IF EXISTS {KEYSPACE}")
    ses.execute(
        f"CREATE KEYSPACE {KEYSPACE} WITH replication = "
        "{'class': 'SimpleStrategy', 'replication_factor': 1}"
    )
    ses.execute(
        f"CREATE TABLE {KEYSPACE}.{TABLE} ("
        "grp text, year int, avg double, tconst text, votes int, "
        "PRIMARY KEY ((grp), avg, tconst))"
    )

    # --- insert (ScyllaDB write cost) ---
    t0 = time.perf_counter()
    ins = ses.prepare(
        f"INSERT INTO {KEYSPACE}.{TABLE} (grp, year, avg, tconst, votes) "
        "VALUES (?, ?, ?, ?, ?)"
    )
    batch = []
    n = 0
    for i in range(0, len(blob), 24):
        tid, year, avg, votes = struct.unpack_from("<Qifq", blob, i)
        batch.append(("m", year, avg, f"tt{tid:07d}", votes))
        if len(batch) >= 2000:
            ses.execute_concurrent_with_args(ins, batch)
            n += len(batch)
            batch = []
    if batch:
        ses.execute_concurrent_with_args(ins, batch)
        n += len(batch)
    insert_ms = (time.perf_counter() - t0) * 1e3
    print(f"insert: rows={n} ms={insert_ms:.0f}", flush=True)

    # --- query (the comparable number) ---
    # Single partition ('m') holds the movies-with-rating set (year>=2000 is a
    # write-time filter, so the query's year predicate is satisfied by
    # construction); avg>=7.5 is a QUERY-TIME clustering-range scan.
    q = (f"SELECT count(*), sum(votes), avg(avg) FROM {KEYSPACE}.{TABLE} "
         f"WHERE grp = 'm' AND avg >= {MIN_AVG}")
    ts = []
    for _ in range(5):
        t0 = time.perf_counter()
        row = ses.execute(q).one()
        ts.append((time.perf_counter() - t0) * 1e3)
    ts.sort()
    q_ms = ts[len(ts) // 2]
    count, votes, avg = int(row[0]), int(row[1]), float(row[2])
    print(f"query: ms={q_ms:.1} count={count} votes={votes} avg={avg:.6f}", flush=True)

    c_ok = count == TRUTH["count"]
    v_ok = votes == TRUTH["votes"]
    a_rel = abs(avg - TRUTH["avg"]) / TRUTH["avg"]
    fast_ok = q_ms < MONGO_QUERY_MS / 100
    gate = "PASS" if (c_ok and v_ok and a_rel < 1e-9 and fast_ok) else "FAIL"
    print(
        f"gate_scylla_exact={gate} count_ok={c_ok} votes_ok={v_ok} avg_rel={a_rel:.2e} "
        f"faster_than_mongo={fast_ok} ({MONGO_QUERY_MS / q_ms:.0f}x)"
    )

    res = {
        "engine": "scylladb",
        "version": ses.execute("SELECT release_version FROM system.local").one()[0],
        "shape": "CQL denormalized: PRIMARY KEY ((year), avg, tconst)",
        "etl": "Rust: imdb_scylla_prep (write-time join)",
        "etl_rows": count_rows,
        "insert_ms": round(insert_ms, 1),
        "query_ms": round(q_ms, 1),
        "query_ms_runs": [round(x, 1) for x in ts],
        "count": count,
        "votes": votes,
        "avg": avg,
        "mongo_same_query_ms": MONGO_QUERY_MS,
        "gate": gate,
    }
    json.dump(res, open(OUT, "w"), indent=2)
    print("SAVED", OUT)
    cl.shutdown()
    if gate != "PASS":
        raise SystemExit(1)


if __name__ == "__main__":
    main()
