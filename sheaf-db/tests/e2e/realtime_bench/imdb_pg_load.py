"""imdb_pg_load.py — load PostgreSQL's imdb DB from the already-loaded DuckDB
copy, using DuckDB's postgres extension (C++ path).

Why: psycopg3 text COPY splits input per line (measured: 474s user CPU for
~1GB -> ~1 MB/s effective). DuckDB streams TSV and writes rows through the
postgres extension at C++ speed.

PG tables are UNLOGGED on purpose: the earlier LOGGED load triggered a WAL
checkpoint storm ("checkpoints are occurring too frequently", 270s checkpoint
writes). Benchmarks read the same tables afterwards.
"""
import time

import duckdb
import psycopg

import bench_paths as P

DDB = P.data("duckdb", "imdb.duckdb")
_pg = P.service("pg")
PG = {"host": _pg.host, "port": _pg.port, "user": _pg.user, "dbname": "imdb"}
TABLES = ["title_basics", "title_akas", "title_crew", "title_episode",
          "title_principals", "title_ratings", "name_basics"]
DDL = {
    "title_basics": "(tconst TEXT, titletype TEXT, primarytitle TEXT, originaltitle TEXT, isadult SMALLINT, startyear INT, endyear INT, runtimeminutes INT, genres TEXT)",
    "title_akas": "(titleid TEXT, ordering INT, title TEXT, region TEXT, language TEXT, types TEXT, attributes TEXT, isoriginaltitle SMALLINT)",
    "title_crew": "(tconst TEXT, directors TEXT, writers TEXT)",
    "title_episode": "(tconst TEXT, parenttconst TEXT, seasonnumber INT, episodenumber INT)",
    "title_principals": "(tconst TEXT, ordering INT, nconst TEXT, category TEXT, job TEXT, characters TEXT)",
    "title_ratings": "(tconst TEXT PRIMARY KEY, averagerating REAL, numvotes INT)",
    "name_basics": "(nconst TEXT PRIMARY KEY, primaryname TEXT, birthyear INT, deathyear INT, primaryprofession TEXT, knownfortitles TEXT)",
}


def main():
    pg = psycopg.connect(**PG, autocommit=True, connect_timeout=15)
    cur = pg.cursor()
    cur.execute("SET synchronous_commit = off")
    cur.execute("SET maintenance_work_mem = '1GB'")
    for t in TABLES:
        cur.execute(f"DROP TABLE IF EXISTS {t}")
        cur.execute(f"CREATE UNLOGGED TABLE {t} {DDL[t]}")
    print("pg: 7 unlogged tables created", flush=True)

    con = duckdb.connect(DDB)
    con.execute("INSTALL postgres; LOAD postgres;")
    con.execute(f"ATTACH 'postgresql://{PG['user']}@{PG['host']}:{PG['port']}/imdb' AS pg (TYPE postgres)")
    for t in TABLES:
        t0 = time.perf_counter()
        con.execute(f"INSERT INTO pg.{t} SELECT * FROM {t}")
        cur.execute(f"SELECT COUNT(*) FROM {t}")
        n = cur.fetchone()[0]
        src = con.execute(f"SELECT COUNT(*) FROM {t}").fetchall()[0][0]
        assert n == src, (t, n, src)
        print(f"pg loaded {t}: {n} rows in {time.perf_counter() - t0:.1f}s", flush=True)
    con.close()
    pg.close()
    print("PG-LOAD-COMPLETE", flush=True)


if __name__ == "__main__":
    main()
