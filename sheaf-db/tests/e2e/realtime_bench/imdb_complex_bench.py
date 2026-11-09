"""imdb_complex_bench.py — new complex test cases (JOB-family on real IMDb).
3 queries of rising complexity over 7 real tables; PG (row) vs DuckDB
(columnar) baselines with cross-engine md5 result-hash gates.
ShelfDB side: imdb_fiber_bench (Rust) reproduces Q1 totals from this file.
OUT: results/imdb_job.json (single run; runner merge comes later).
"""
import hashlib
import json
import os
import time

import duckdb
import psycopg

import bench_paths as P

OUT = str(P.RESULTS / "imdb_job.json")
IMDB = P.imdb()
DDB = P.data("duckdb", "imdb.duckdb")
_pg = P.service("pg")
PG = {"host": _pg.host, "port": _pg.port, "user": _pg.user, "dbname": "imdb"}

TABLES = ["title.basics", "title.akas", "title.crew", "title.episode",
          "title.principals", "title.ratings", "name.basics"]

DDL = {
    "title_basics": "(tconst TEXT, titletype TEXT, primarytitle TEXT, originaltitle TEXT, isadult SMALLINT, startyear INT, endyear INT, runtimeminutes INT, genres TEXT)",
    "title_akas": "(titleid TEXT, ordering INT, title TEXT, region TEXT, language TEXT, types TEXT, attributes TEXT, isoriginaltitle SMALLINT)",
    "title_crew": "(tconst TEXT, directors TEXT, writers TEXT)",
    "title_episode": "(tconst TEXT, parenttconst TEXT, seasonnumber INT, episodenumber INT)",
    "title_principals": "(tconst TEXT, ordering INT, nconst TEXT, category TEXT, job TEXT, characters TEXT)",
    "title_ratings": "(tconst TEXT PRIMARY KEY, averagerating REAL, numvotes INT)",
    "name_basics": "(nconst TEXT PRIMARY KEY, primaryname TEXT, birthyear INT, deathyear INT, primaryprofession TEXT, knownfortitles TEXT)",
}

QUERIES = {
    # Q1: 2-table filter + aggregate (correctness anchor)
    "q1": """SELECT COUNT(*), AVG(r.averagerating), SUM(r.numvotes) FROM title_basics b
             JOIN title_ratings r ON r.tconst = b.tconst
             WHERE b.titletype='movie' AND b.startyear>=2000 AND r.averagerating>=7.5""",
    # Q2: 4-table top-k (join order stress)
    "q2": """SELECT n.primaryname, COUNT(*) AS c FROM title_principals p
             JOIN title_basics b ON b.tconst = p.tconst
             JOIN title_ratings r ON r.tconst = p.tconst
             JOIN name_basics n ON n.nconst = p.nconst
             WHERE p.category IN ('actor','actress') AND n.birthyear>=1970 AND r.averagerating>=8.0
             GROUP BY n.primaryname ORDER BY c DESC, n.primaryname ASC LIMIT 10""",
    # Q3: 4-table decade rollup with HAVING (aggregation stress).
    # Dialect divergences found by the md5 gate (both real, both silent):
    #  (1) DuckDB '/' is true division (1951.0) vs PostgreSQL integer division
    #      (1950)        -> 59 decades in DuckDB vs 8 in PG.
    #  (2) DuckDB CAST(double AS INTEGER) ROUNDS (195.5 -> 196) while the
    #      integer path TRUNCATES (1955/10 -> 195) -> 9 decades vs 8.
    # Dialect-neutral form: FLOOR on a float division, then cast.
    # Join fan-out was identical in both engines (207,381 rows).
    "q3": """SELECT CAST(FLOOR(b.startyear / 10.0) AS INTEGER) * 10 AS decade, COUNT(*), AVG(r.averagerating) FROM title_basics b
             JOIN title_ratings r ON r.tconst = b.tconst
             JOIN title_principals p ON p.tconst = b.tconst
             JOIN name_basics n ON n.nconst = p.nconst
             WHERE b.genres LIKE '%Drama%' AND r.numvotes>10000 AND b.startyear>=1950
             GROUP BY decade HAVING COUNT(*)>500 ORDER BY decade""",
    # Q4: 5-table JOB complex query (multi-attribute filters + join order stress)
    "q4": """SELECT n.primaryname, COUNT(*) AS movie_count, AVG(r.averagerating) AS avg_rating, SUM(r.numvotes) AS total_votes
             FROM title_basics b
             JOIN title_ratings r ON r.tconst = b.tconst
             JOIN title_principals p ON p.tconst = b.tconst
             JOIN name_basics n ON n.nconst = p.nconst
             WHERE b.titletype = 'movie'
               AND b.startyear >= 1980
               AND r.averagerating >= 7.8
               AND r.numvotes >= 50000
               AND p.category IN ('actor', 'actress')
               AND n.birthyear >= 1950
             GROUP BY n.primaryname
             HAVING COUNT(*) >= 5
             ORDER BY movie_count DESC, avg_rating DESC, n.primaryname ASC
             LIMIT 10""",
}


def norm(rows):
    out = []
    for r in rows:
        out.append("|".join("NULL" if v is None else
                            (f"{v:.4f}" if isinstance(v, float) else str(v)) for v in r))
    return sorted(out)


def h(rows):
    return hashlib.md5("\n".join(norm(rows)).encode()).hexdigest()


def main():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    res = {"tsv_dir": IMDB, "tables": TABLES}
    # ---- DuckDB load (skip if counts match) ----
    t0 = time.perf_counter()
    con = duckdb.connect(DDB)
    counts = {}
    for t in TABLES:
        f = os.path.join(IMDB, t + ".tsv")
        tbl = t.replace(".", "_")
        n = con.execute(f"SELECT COUNT(*) FROM {tbl}").fetchall()[0][0] \
            if tbl in [r[0] for r in con.execute("SHOW TABLES").fetchall()] else -1
        if n < 0:
            con.execute(f"""CREATE TABLE {tbl} AS FROM read_csv('{f}', delim='\t',
                header=true, nullstr='\\N', quote='', sample_size=200000)""")
            n = con.execute(f"SELECT COUNT(*) FROM {tbl}").fetchall()[0][0]
            print(f"duckdb loaded {tbl}: {n}", flush=True)
        counts[tbl] = n
    res["duckdb_load_s"] = round(time.perf_counter() - t0, 1)
    res["counts"] = counts
    # ---- PG load (skip if counts match; create db on first run) ----
    t0 = time.perf_counter()
    try:
        pg = psycopg.connect(**PG, autocommit=True)
    except Exception:
        admin = dict(PG);
        admin["dbname"] = "postgres"
        pg = psycopg.connect(**admin, autocommit=True)
        pg.cursor().execute("CREATE DATABASE imdb")
        pg.close()
        pg = psycopg.connect(**PG, autocommit=True)
        print("pg: created database imdb", flush=True)
    cur = pg.cursor()
    # Load-time settings: bulk load must not be WAL-bound (observed:
    # "checkpoints are occurring too frequently" + 107s checkpoint writes
    # made the copy ~5x slower). UNLOGGED tables skip WAL entirely for
    # the bulk load; documented as a load-time concession, not a
    # steady-state one (queries run on the same tables afterwards).
    cur.execute("SET synchronous_commit = off")
    cur.execute("SET maintenance_work_mem = '1GB'")
    cur.execute("SET work_mem = '256MB'")
    for t in TABLES:
        tbl = t.replace(".", "_")
        cur.execute(f"CREATE UNLOGGED TABLE IF NOT EXISTS {tbl} {DDL[tbl]}")
        cur.execute(f"SELECT COUNT(*) FROM {tbl}")
        if cur.fetchone()[0] == 0:
            f = os.path.join(IMDB, t + ".tsv")
            with open(f, "rb") as fh:
                with cur.copy(f"COPY {tbl} FROM STDIN WITH (FORMAT csv, DELIMITER E'\\t', NULL '\\N', HEADER true, QUOTE E'\\b')") as cp:
                    while True:
                        b = fh.read(8 << 20)
                        if not b:
                            break
                        cp.write(b)
            print(f"pg loaded {tbl}", flush=True)
        cur.execute(f"SELECT COUNT(*) FROM {tbl}")
        assert cur.fetchone()[0] == counts[tbl], (tbl, "count mismatch pg vs duckdb")
    res["pg_load_s"] = round(time.perf_counter() - t0, 1)
    print(f"counts verified: {counts}", flush=True)
    # ---- Queries both engines, hash gates ----
    res["queries"] = {}
    for qn, q in QUERIES.items():
        t0 = time.perf_counter()
        drows = con.execute(q).fetchall()
        dms = (time.perf_counter() - t0) * 1e3
        t0 = time.perf_counter()
        cur.execute(q)
        prows = cur.fetchall()
        pms = (time.perf_counter() - t0) * 1e3
        dh, ph = h(drows), h(prows)
        assert dh == ph, (qn, "PG vs DuckDB result mismatch", len(drows), len(prows))
        assert len(drows) > 0, (qn, "empty result")
        res["queries"][qn] = {"rows": len(drows), "md5": dh,
                              "duckdb_ms": round(dms, 1), "pg_ms": round(pms, 1)}
        if qn == "q1":
            # expected values for the ShelfDB fiber counterpart (Rust bin)
            res["queries"][qn]["expected_count"] = int(drows[0][0])
            res["queries"][qn]["expected_avg_rating"] = float(drows[0][1])
            res["queries"][qn]["expected_sum_votes"] = int(drows[0][2])
            print(f"q1 expected: count={drows[0][0]} avg={drows[0][1]} votes={drows[0][2]}", flush=True)
        if qn == "q3":
            # expected rows for the ShelfDB fiber counterpart (decade, cnt, avg)
            res["queries"][qn]["expected_rows"] = [
                [int(r[0]), int(r[1]), float(r[2])] for r in drows]
            # compact form for the Rust counterpart (one line, ';'-separated)
            res["queries"][qn]["expected_rows_compact"] = ";".join(
                f"{int(r[0])},{int(r[1])},{float(r[2]):.10f}" for r in drows)
            print(f"q3 expected: {[(r[0], r[1]) for r in drows]}", flush=True)
        if qn == "q2":
            # expected rows for the ShelfDB fiber counterpart (name, count)
            res["queries"][qn]["expected_rows_compact"] = ";".join(
                f"{r[0]},{int(r[1])}" for r in drows)
            print(f"q2 expected: {[(r[0], r[1]) for r in drows]}", flush=True)
        if qn == "q4":
            res["queries"][qn]["expected_rows_compact"] = ";".join(
                f"{r[0]},{int(r[1])}" for r in drows)
            print(f"q4 expected: {[(r[0], r[1]) for r in drows]}", flush=True)
        print(f"{qn}: rows={len(drows)} md5={dh[:12]} duck={dms:.0f}ms pg={pms:.0f}ms", flush=True)
    res["gate_cross_engine"] = "PASS"
    res["dialect_note"] = ("Two silent dialect divergences found by the md5 cross-engine "
                           "gate: (1) DuckDB '/' is true division vs PostgreSQL integer "
                           "division (8 decades vs 59); (2) DuckDB CAST(double AS INT) "
                           "rounds vs PostgreSQL truncation (8 vs 9). Fixed with "
                           "CAST(FLOOR(startyear/10.0) AS INTEGER)*10. Join fan-out "
                           "identical in both engines (207,381 rows).")
    json.dump(res, open(OUT, "w"), indent=2)
    print("SAVED", OUT)


if __name__ == "__main__":
    main()
