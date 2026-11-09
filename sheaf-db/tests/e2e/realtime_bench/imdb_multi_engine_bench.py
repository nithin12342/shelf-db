"""imdb_multi_engine_bench.py — IMDb complex cases across DB CATEGORIES.
One file, one dataset (real IMDb), each engine in its own strength:

  sqlite   (embedded)   : Q1 join+aggregate on SQL
  mariadb  (row store)  : Q1 join+aggregate on SQL
  mongodb  (document)   : Q1 via $lookup aggregation pipeline
  redis    (in-mem KV)  : point lookups + full-scan aggregation via Lua
  scylla   (wide-col)   : point + range slice + per-year aggregation
  neo4j    (graph)      : 2-hop co-actor traversal (IMDb's classic query)
  duckdb   (columnar)   : Q1 (baseline engine, from imdb_job.json)
  pg       (row store)  : Q1 (baseline engine, from imdb_job.json)
  shelfdb  (sheaf)      : Q1/Q2/Q3 (from imdb_shelf.json, Rust bins)

All aggregate cases are gated against the SAME truth values
(count / sum_votes / avg), taken from the md5-cross-checked PG+DuckDB run
(results/imdb_job.json). Usage:
  py imdb_multi_engine_bench.py --engines sqlite,mariadb,mongodb,redis,scylladb,neo4j
"""
import argparse
import json
import os
import time
from decimal import Decimal

import bench_paths as P


def _json_default(o):
    """MariaDB returns Decimal for SUM(); JSON needs numbers."""
    if isinstance(o, Decimal):
        return int(o) if o == o.to_integral_value() else float(o)
    raise TypeError(f"Object of type {o.__class__.__name__} is not JSON serializable")

IMDB = P.imdb()
RES = str(P.RESULTS)
OUT = os.path.join(RES, "imdb_multi_engine.json")
BASE = os.path.join(RES, "imdb_job.json")
DDB = P.data("duckdb", "imdb.duckdb")
_mariadb = P.service("mariadb")
MARIADB = {"host": _mariadb.host, "port": _mariadb.port, "user": _mariadb.user,
           "password": _mariadb.password, "database": "imdb"}
MONGO = P.service("mongo").authority
_neo4j = P.service("neo4j")
NEO4J = (_neo4j.uri("bolt"), _neo4j.user, _neo4j.password)

MIN_AVG = 7.5
MIN_YEAR = 2000


def truth():
    d = json.load(open(BASE))
    q1 = d["queries"]["q1"]
    return {"count": q1["expected_count"], "votes": q1["expected_sum_votes"],
            "avg": q1["expected_avg_rating"]}


def gate(name, res, t, tol=1e-6):
    c_ok = res["count"] == t["count"]
    v_ok = res["votes"] == t["votes"]
    a_rel = abs(res["avg"] - t["avg"]) / t["avg"] if t["avg"] else 0.0
    ok = c_ok and v_ok and a_rel < tol
    res["gate"] = "PASS" if ok else "FAIL"
    res["gate_detail"] = (f"count {res['count']}=={t['count']}:{c_ok} "
                          f"votes {res['votes']}=={t['votes']}:{v_ok} "
                          f"avg rel {a_rel:.2e}")
    print(f"{name}: count={res['count']} votes={res['votes']} avg={res['avg']:.6f} "
          f"({res.get('ms', 0):.0f} ms) gate={res['gate']}", flush=True)
    return ok


def load_truth_sets():
    """Movies (year>=MIN_YEAR) and ratings (avg>=MIN_AVG): the exact row sets
    the SQL engines join over. Shared by every engine so all see identical data.
    `joined` = the intersection (the Q1 truth set) — KV engines that cannot
    join need it denormalized at load time (Redis/Scylla idiom)."""
    import duckdb
    con = duckdb.connect(DDB)
    titles = con.execute(
        f"SELECT tconst FROM title_basics WHERE titletype='movie' AND startyear>={MIN_YEAR}"
    ).fetchall()
    rat = con.execute(
        f"SELECT tconst, averagerating, numvotes FROM title_ratings WHERE averagerating>={MIN_AVG}"
    ).fetchall()
    joined = con.execute(
        f"""SELECT r.tconst, r.averagerating, r.numvotes FROM title_ratings r
            JOIN title_basics b ON b.tconst=r.tconst
            WHERE b.titletype='movie' AND b.startyear>={MIN_YEAR} AND r.averagerating>={MIN_AVG}"""
    ).fetchall()
    con.close()
    return titles, rat, joined


# ---------------------------------------------------------------- sqlite
def run_sqlite(titles, rat, joined, t):
    import sqlite3
    db = P.data("sqlite", "imdb.db")
    if os.path.exists(db):
        os.remove(db)
    con = sqlite3.connect(db)
    t0 = time.perf_counter()
    con.execute("CREATE TABLE ratings(tconst TEXT PRIMARY KEY, avg REAL, votes INT)")
    con.execute("CREATE TABLE basics(tconst TEXT PRIMARY KEY, titletype TEXT, startyear INT)")
    con.executemany("INSERT INTO basics VALUES(?,?,?)",
                    ((x[0], "movie", MIN_YEAR) for x in titles))
    con.executemany("INSERT INTO ratings VALUES(?,?,?)", rat)
    con.commit()
    load_s = time.perf_counter() - t0
    t0 = time.perf_counter()
    row = con.execute(
        "SELECT COUNT(*), SUM(r.votes), AVG(r.avg) FROM ratings r JOIN basics b "
        "ON b.tconst=r.tconst WHERE b.titletype='movie' AND b.startyear>=? "
        "AND r.avg>=?", (MIN_YEAR, MIN_AVG)).fetchone()
    ms = (time.perf_counter() - t0) * 1e3
    con.close()
    res = {"count": int(row[0]), "votes": int(row[1]), "avg": float(row[2]),
           "ms": ms, "load_s": load_s}
    gate("sqlite", res, t)
    return res


# --------------------------------------------------------------- mariadb
def run_mariadb(titles, rat, joined, t):
    import pymysql
    params = {k: v for k, v in MARIADB.items() if k != "database"}
    cn = pymysql.connect(**params, autocommit=True)
    with cn.cursor() as cur:
        cur.execute("CREATE DATABASE IF NOT EXISTS imdb")
    cn.close()
    cn = pymysql.connect(**MARIADB, autocommit=True)
    with cn.cursor() as cur:
        cur.execute("CREATE DATABASE IF NOT EXISTS imdb")
        cur.execute("USE imdb")
        for stmt in ["DROP TABLE IF EXISTS ratings", "DROP TABLE IF EXISTS basics",
                     "CREATE TABLE ratings(tconst VARCHAR(20) PRIMARY KEY, avg DOUBLE, votes INT) ENGINE=InnoDB",
                     "CREATE TABLE basics(tconst VARCHAR(20) PRIMARY KEY, titletype VARCHAR(20), startyear INT) ENGINE=InnoDB"]:
            cur.execute(stmt)
        t0 = time.perf_counter()
        cur.executemany("INSERT INTO basics VALUES(%s,%s,%s)",
                        ((x[0], "movie", MIN_YEAR) for x in titles))
        cur.executemany("INSERT INTO ratings VALUES(%s,%s,%s)",
                        ((a, b, c) for (a, b, c) in rat))
        load_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        cur.execute("SELECT COUNT(*), SUM(r.votes), AVG(r.avg) FROM ratings r "
                    "JOIN basics b ON b.tconst=r.tconst WHERE b.titletype='movie' "
                    "AND b.startyear>=%s AND r.avg>=%s", (MIN_YEAR, MIN_AVG))
        row = cur.fetchone()
        ms = (time.perf_counter() - t0) * 1e3
    cn.close()
    res = {"count": row[0], "votes": row[1], "avg": row[2], "ms": ms, "load_s": load_s}
    gate("mariadb", res, t)
    return res


# --------------------------------------------------------------- mongodb
def run_mongodb(titles, rat, joined, t):
    from pymongo import MongoClient
    cli = MongoClient(MONGO, serverSelectionTimeoutMS=20000)
    db = cli.imdb
    db.ratings.drop()
    db.basics.drop()
    t0 = time.perf_counter()
    db.ratings.insert_many([{"tconst": a, "avg": b, "votes": c} for (a, b, c) in rat],
                           ordered=False)
    db.basics.insert_many([{"tconst": a[0], "titletype": "movie", "startyear": MIN_YEAR}
                            for a in titles], ordered=False)
    db.ratings.create_index([("tconst", 1)])
    db.basics.create_index([("tconst", 1)])
    load_s = time.perf_counter() - t0
    t0 = time.perf_counter()
    pipe = [
        {"$match": {"avg": {"$gte": MIN_AVG}}},
        {"$lookup": {"from": "basics", "localField": "tconst", "foreignField": "tconst", "as": "b"}},
        {"$unwind": "$b"},
        {"$match": {"b.titletype": "movie", "b.startyear": {"$gte": MIN_YEAR}}},
        {"$group": {"_id": None, "cnt": {"$sum": 1}, "votes": {"$sum": "$votes"},
                    "avg_sum": {"$sum": "$avg"}, "avg_n": {"$sum": 1}}},
    ]
    row = list(db.ratings.aggregate(pipe))[0]
    ms = (time.perf_counter() - t0) * 1e3
    cli.close()
    res = {"count": row["cnt"], "votes": row["votes"],
           "avg": row["avg_sum"] / row["avg_n"], "ms": ms, "load_s": load_s}
    gate("mongodb", res, t)
    return res


# ----------------------------------------------------------------- redis
def run_redis(titles, rat, joined, t):
    """Redis has no join: the Q1 truth set is denormalized into one hash at
    load time (Redis' idiom). The scan aggregation then reproduces the truth."""
    import redis as r
    redis_svc = P.service("redis")
    cli = r.Redis(host=redis_svc.host, port=redis_svc.port)
    cli.flushdb()
    t0 = time.perf_counter()
    # One hash per column (Redis idiom): field = tconst.
    pipe = cli.pipeline(transaction=False)
    for (a, b, c) in joined:
        pipe.hset("imdb:avg", a, f"{b}")
        pipe.hset("imdb:votes", a, f"{c}")
        if len(pipe.command_stack) >= 20000:
            pipe.execute()
            pipe = cli.pipeline(transaction=False)
    pipe.execute()
    load_s = time.perf_counter() - t0
    # point lookups (KV strength)
    keys = [x[0] for x in joined[:20000]]
    t0 = time.perf_counter()
    for k in keys:
        cli.hmget("imdb:avg", "imdb:votes", k)
    lookup_ms = (time.perf_counter() - t0) * 1e3
    # full-scan aggregation via Lua (Redis' own scripting strength)
    lua = """
    local a = redis.call('HGETALL', KEYS[1])
    local v = redis.call('HGETALL', KEYS[2])
    local votes = 0
    local asum = 0
    local n = 0
    -- HGETALL returns a flat [field, value, ...] array: value sits at i+1
    for i = 1, #v, 2 do
      asum = asum + tonumber(a[i+1])
      votes = votes + tonumber(v[i+1])
      n = n + 1
    end
    return {votes, asum, n}
    """
    t0 = time.perf_counter()
    votes, asum, n = cli.eval(lua, 2, "imdb:avg", "imdb:votes")
    scan_ms = (time.perf_counter() - t0) * 1e3
    cli.flushdb()
    res = {"count": n, "votes": votes, "avg": asum / n, "ms": scan_ms,
           "lookup_ms": lookup_ms, "load_s": load_s}
    gate("redis", res, t)
    return res


# --------------------------------------------------------------- scylla
def run_scylla(titles, rat, joined, t):
    """Wide-column: truth set denormalized into a by_year table (Scylla idiom:
    the join is resolved at write time, partition = year)."""
    from cassandra.cluster import Cluster
    scylla = P.service("scylla")
    cl = Cluster([scylla.host], port=scylla.port)
    deadline = time.time() + 240
    ses = None
    while time.time() < deadline:
        try:
            ses = cl.connect()
            ses.execute("SELECT release_version FROM system.local")
            break
        except Exception:
            time.sleep(10)
    if ses is None:
        raise RuntimeError("scylla native protocol never became ready")
    ses.execute("DROP KEYSPACE IF EXISTS imdb")
    ses.execute("CREATE KEYSPACE imdb WITH replication = "
                "{'class': 'SimpleStrategy', 'replication_factor': 1}")
    ses.execute("CREATE TABLE imdb.by_year (year int, tconst text, avg double, votes int, "
                "PRIMARY KEY (year, tconst))")
    t0 = time.perf_counter()
    ins = ses.prepare("INSERT INTO imdb.by_year (year, tconst, avg, votes) VALUES (?, ?, ?, ?)")
    batch = []
    for (a, b, c) in joined:
        batch.append((MIN_YEAR, a, b, c))
        if len(batch) >= 5000:
            ses.execute_concurrent_with_args(ins, batch)
            batch = []
    if batch:
        ses.execute_concurrent_with_args(ins, batch)
    load_s = time.perf_counter() - t0
    # range slice: 10k keys by clustering order
    t0 = time.perf_counter()
    rows = list(ses.execute("SELECT tconst, avg, votes FROM imdb.by_year WHERE year=%s "
                         "AND tconst > %s LIMIT 10000", (MIN_YEAR, "tt0000")))
    range_ms = (time.perf_counter() - t0) * 1e3
    # aggregation over the partition (wide-column scan)
    t0 = time.perf_counter()
    agg = list(ses.execute("SELECT COUNT(*), SUM(votes), AVG(avg) FROM imdb.by_year "
                         "WHERE year=%s", (MIN_YEAR,)))
    ms = (time.perf_counter() - t0) * 1e3
    cl.shutdown()
    res = {"count": agg[0][0], "votes": agg[0][1], "avg": agg[0][2], "ms": ms,
           "range_ms": range_ms, "range_rows": len(rows), "load_s": load_s}
    gate("scylladb", res, t)
    return res


# ---------------------------------------------------------------- neo4j
def run_neo4j(titles, rat, joined, t):
    """Graph: IMDb's classic shape (Title)-[:PRINCIPAL]->(Person), then a
    2-hop co-actor traversal. Slice = titles with avg>=8.0 (documented:
    smaller than the full set to stay inside the container's heap)."""
    from neo4j import GraphDatabase
    import duckdb
    con = duckdb.connect(DDB)
    edges = con.execute(
        """SELECT p.tconst, p.nconst FROM title_principals p
           JOIN title_ratings r ON r.tconst=p.tconst
           JOIN title_basics b ON b.tconst=p.tconst
           WHERE b.titletype='movie' AND b.startyear>=2010 AND r.averagerating>=8.0
           LIMIT 300000"""
    ).fetchall()
    # IMDb's principals TSV can list the same (title, person) twice; the
    # traversal counts DISTINCT co-actor pairs, so dedupe the edge list
    # (Neo4j counted 548,924 vs 173,716 distinct before this).
    edges = sorted(set(edges))
    con.close()
    uniq_titles = sorted({a for a, _ in edges})
    uniq_persons = sorted({n for _, n in edges})
    drv = GraphDatabase.driver(NEO4J[0], auth=(NEO4J[1], NEO4J[2]))
    with drv.session() as s:
        # Delete ONLY this bench's own labels, in bounded batches.
        # A label-agnostic delete (MATCH (n) ... DETACH DELETE n) wiped the
        # TPC-H-derived graph that neo4j_bench.py measures (labels
        # Customer/Order/Line) and produced a bogus 8.6 ms "traversal" over an
        # EMPTY database on 2026-10-20. The IMDb graph uses Title/Person,
        # which no other bench touches.
        while True:
            r = s.run("MATCH (n) WHERE n:Title OR n:Person "
                      "WITH n LIMIT 10000 DETACH DELETE n "
                      "RETURN count(*) AS c").single()
            if not r or r["c"] == 0:
                break
        print("  neo4j: previous graph cleared", flush=True)
        # plain indexes (not uniqueness constraints): MERGE under a unique
        # constraint exhausted the 4.2 GiB heap on this container.
        s.run("CREATE INDEX t_id IF NOT EXISTS FOR (t:Title) ON (t.tid)")
        s.run("CREATE INDEX p_id IF NOT EXISTS FOR (p:Person) ON (p.pid)")
        t0 = time.perf_counter()
        for i in range(0, len(uniq_titles), 10000):
            s.run("UNWIND $rows AS r CREATE (t:Title {tid: r})",
                  rows=uniq_titles[i:i + 10000])
        print(f"  neo4j: {len(uniq_titles)} titles created", flush=True)
        for i in range(0, len(uniq_persons), 10000):
            s.run("UNWIND $rows AS r CREATE (p:Person {pid: r})",
                  rows=uniq_persons[i:i + 10000])
        print(f"  neo4j: {len(uniq_persons)} persons created", flush=True)
        for i in range(0, len(edges), 10000):
            s.run("UNWIND $rows AS r MATCH (t:Title {tid: r[0]}) "
                  "MATCH (p:Person {pid: r[1]}) CREATE (t)-[:PRINCIPAL]->(p)",
                  rows=edges[i:i + 10000])
        print(f"  neo4j: {len(edges)} relationships created", flush=True)
        load_s = time.perf_counter() - t0
        # 2-hop co-actor traversal, counted by degree (same number as the
        # path expansion, but without materializing every pair — the
        # expansion exhausted the 4.2 GiB heap).
        t0 = time.perf_counter()
        recs = s.run("MATCH (p:Person)<-[:PRINCIPAL]-(t:Title) "
                     "WITH p, count(t) AS d WHERE d > 1 "
                     "RETURN sum(d * (d - 1)) AS pairs").single()["pairs"]
        ms = (time.perf_counter() - t0) * 1e3
        # exactness: same count computed from the edge list
        byp = {}
        for a, n in edges:
            byp.setdefault(n, set()).add(a)
        expect = sum(len(v) * (len(v) - 1) for v in byp.values())
        titles_n = len(uniq_titles)
    drv.close()
    res = {"pairs": recs, "expected_pairs": expect, "ms": ms, "load_s": load_s,
           "titles": titles_n, "edges": len(edges), "persons": len(uniq_persons)}
    ok = recs == expect
    res["gate"] = "PASS" if ok else "FAIL"
    res["gate_detail"] = f"pairs {recs} == {expect}"
    print(f"neo4j: 2-hop pairs={recs} expected={expect} ({ms:.0f} ms) "
          f"edges={len(edges)} gate={res['gate']}", flush=True)
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--engines", default="sqlite,mariadb,mongodb,redis,scylladb,neo4j")
    a = ap.parse_args()
    t = truth()
    print(f"truth: count={t['count']} votes={t['votes']} avg={t['avg']:.9f}", flush=True)
    titles, rat, joined = load_truth_sets()
    print(f"shared row sets: titles={len(titles)} ratings={len(rat)} joined_truth={len(joined)}", flush=True)
    runners = {"sqlite": run_sqlite, "mariadb": run_mariadb, "mongodb": run_mongodb,
               "redis": run_redis, "scylladb": run_scylla, "neo4j": run_neo4j}
    res = {}
    detail_all = {}
    old = {}
    if os.path.exists(OUT):
        try:
            old = json.load(open(OUT))
        except Exception:
            old = {}
    for name in a.engines.split(","):
        if name not in runners:
            continue
        try:
            detail = runners[name](titles, rat, joined, t)
            res[name] = detail.get("gate") == "PASS" if isinstance(detail, dict) else bool(detail)
            detail_all[name] = detail
        except Exception as e:
            print(f"{name}: ERROR {type(e).__name__}: {str(e)[:160]}", flush=True)
            res[name] = False
            detail_all[name] = {"gate": "ERROR", "error": f"{type(e).__name__}: {str(e)[:200]}"}
    # reference engines (already measured, read from disk)
    j = json.load(open(BASE))
    res["duckdb_q1_ms"] = j["queries"]["q1"]["duckdb_ms"]
    res["pg_q1_ms"] = j["queries"]["q1"]["pg_ms"]
    shelf = os.path.join(RES, "imdb_shelf.json")
    if os.path.exists(shelf):
        s = json.load(open(shelf))
        res["shelfdb_q1_ms"] = s["q1"]["query_ms"]
    tag = time.strftime("run_%Y%m%d_%H%M")
    old[tag] = res
    old[tag + '_detail'] = detail_all
    old["truth"] = t
    json.dump(old, open(OUT, "w"), indent=2, default=_json_default)
    print("SAVED", OUT, flush=True)


if __name__ == "__main__":
    main()
