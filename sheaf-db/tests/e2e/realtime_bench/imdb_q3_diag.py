"""imdb_q3_diag.py — why do PG and DuckDB disagree on Q3?
Compares: (1) join fan-out rows, (2) per-decade counts, (3) type of
startyear in each engine, (4) NULL handling of the WHERE predicates.
"""
import duckdb
import psycopg

import bench_paths as P

DDB = P.data("duckdb", "imdb.duckdb")
_pg = P.service("pg")
PG = {"host": _pg.host, "port": _pg.port, "user": _pg.user, "dbname": "imdb"}

con = duckdb.connect(DDB)
pg = psycopg.connect(**PG, autocommit=True, connect_timeout=15)
cur = pg.cursor()

print("--- column types ---")
cur.execute("SELECT column_name, data_type FROM information_schema.columns "
            "WHERE table_name='title_basics' ORDER BY ordinal_position")
print("PG  title_basics:", cur.fetchall())
print("DDB title_basics:", con.execute("DESCRIBE title_basics").fetchall())

Q = """SELECT COUNT(*) FROM title_basics b
       JOIN title_ratings r ON r.tconst = b.tconst
       JOIN title_principals p ON p.tconst = b.tconst
       JOIN name_basics n ON n.nconst = p.nconst
       WHERE b.genres LIKE '%Drama%' AND r.numvotes>10000 AND b.startyear>=1950"""
d = con.execute(Q).fetchall()[0][0]
cur.execute(Q)
p = cur.fetchone()[0]
print(f"\njoin fan-out rows: duckdb={d} pg={p}  (equal={d == p})")

G = """SELECT (b.startyear/10)*10 AS decade, COUNT(*) c FROM title_basics b
       JOIN title_ratings r ON r.tconst = b.tconst
       JOIN title_principals p ON p.tconst = b.tconst
       JOIN name_basics n ON n.nconst = p.nconst
       WHERE b.genres LIKE '%Drama%' AND r.numvotes>10000 AND b.startyear>=1950
       GROUP BY decade ORDER BY decade"""
print("\nduckdb decades:", con.execute(G).fetchall()[:12])
print("pg     decades:", cur.execute(G).fetchall()[:12])

print("\n--- predicate selectivity ---")
for cond in ["b.genres LIKE '%Drama%'", "r.numvotes>10000", "b.startyear>=1950"]:
    q = ("SELECT COUNT(*) FROM title_basics b JOIN title_ratings r ON r.tconst=b.tconst "
         "JOIN title_principals p ON p.tconst=b.tconst JOIN name_basics n ON n.nconst=p.nconst "
         f"WHERE {cond}")
    print(f"  {cond:26s} duckdb={con.execute(q).fetchall()[0][0]:>12} pg={cur.execute(q).fetchone()[0]:>12}")

print("\n--- nulls ---")
for tbl, col in [("title_basics", "startyear"), ("title_basics", "genres"),
                  ("title_ratings", "numvotes"), ("name_basics", "birthyear")]:
    q = f"SELECT COUNT(*) FILTER (WHERE {col} IS NULL), COUNT(*) FROM {tbl}"
    try:
        dq = con.execute(q).fetchall()[0]
    except Exception as e:
        dq = ("err", str(e)[:40])
    try:
        cur.execute(q)
        pq = cur.fetchone()
    except Exception as e:
        pq = ("err", str(e)[:40])
    print(f"  {tbl}.{col:12s} nulls duckdb={dq} pg={pq}")
pg.close()
con.close()
