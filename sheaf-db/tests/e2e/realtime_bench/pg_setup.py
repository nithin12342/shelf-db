"""One-shot: create the bench database and password on the configured PG."""
import psycopg

import bench_paths as P

pg = P.service("pg")
c = psycopg.connect(host=pg.host, port=pg.port, user=pg.user,
                    dbname="postgres", autocommit=True)
c.execute(f'ALTER USER "{pg.user}" PASSWORD %s', (pg.password,))
c.execute("CREATE DATABASE bench")
print("bench db + password ready")
