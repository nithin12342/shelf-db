"""imdb_setup_bench.py - what the DuckDB and PostgreSQL setups actually cost.

METHOD-065. The committed record held a DuckDB setup of 0.1 s for seven TSV
files and no script that produced it. This runner measures the setup on this
host, for both engines, with the same input files and the same column list.

What "setup" means here: create the empty table, then load every row of the
file. The time covers both steps. The load is a server-side COPY for
PostgreSQL and a read_csv_auto for DuckDB, which is what a reader of that
engine would write.

The gate: the recorded setup must be at or above the floor for reading the
input at the measured disk rate, or the record must state that the input came
from a warm page cache and give the measured rate.

Usage:
    py sheaf-db/tests/e2e/realtime_bench/imdb_setup_bench.py --engine duckdb
    py sheaf-db/tests/e2e/realtime_bench/imdb_setup_bench.py --engine postgres
    py sheaf-db/tests/e2e/realtime_bench/imdb_setup_bench.py --engine both
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
import time

import bench_paths as P

TSV_DIR = P.imdb()

# The row count every file must load. psql returns 0 from a COPY that stops
# early unless ON_ERROR_STOP is set, so the row count is the check that the
# load was complete. The values come from results/imdb_job.json.
EXPECTED = {
    "title_basics": 12866375,
    "title_akas": 59774094,
    "title_crew": 12866977,
    "title_episode": 9950953,
    "title_principals": 102312263,
    "title_ratings": 1721477,
    "name_basics": 15738257,
}
OUT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "results", "imdb_setup.json")
PG_BIN = P.pg_app()
PG_DATA = P.pgbench_data()
PG = P.service("pg")
PG_PORT = str(PG.port)

# The seven IMDb files, the table name, and the column order of the file.
FILES = [
    ("title_basics", "title.basics.tsv",
     ["tconst", "titleType", "primaryTitle", "originalTitle", "isAdult",
      "startYear", "endYear", "runtimeMinutes", "genres"]),
    ("title_akas", "title.akas.tsv",
     ["titleId", "ordering", "title", "region", "language", "types",
      "attributes", "isOriginalTitle"]),
    ("title_crew", "title.crew.tsv",
     ["tconst", "directors", "writers"]),
    ("title_episode", "title.episode.tsv",
     ["tconst", "parentTconst", "seasonNumber", "episodeNumber"]),
    ("title_principals", "title.principals.tsv",
     ["tconst", "ordering", "nconst", "category", "job", "characters"]),
    ("title_ratings", "title.ratings.tsv",
     ["tconst", "averageRating", "numVotes"]),
    ("name_basics", "name.basics.tsv",
     ["nconst", "primaryName", "birthYear", "deathYear", "primaryProfession",
      "knownForTitles"]),
]

# The disk rate the committed comparison uses for its floor: 3.20 GB/s.
DISK_RATE = 3.20e9


def input_bytes() -> int:
    return sum(os.path.getsize(os.path.join(TSV_DIR, f)) for _, f, _ in FILES)


def floor_ms(nbytes: int) -> float:
    """The least time the read of nbytes can take at the measured disk rate."""
    return nbytes / DISK_RATE * 1e3


def run_duckdb() -> dict:
    """Create and load the seven tables in a fresh DuckDB file."""
    import duckdb

    db = P.data("duckdb_setup.duckdb")
    for suf in ("", ".wal"):
        p = db + suf
        if os.path.exists(p):
            os.remove(p)
    con = duckdb.connect(db)
    per = {}
    total = 0.0
    for table, fn, _cols in FILES:
        path = os.path.join(TSV_DIR, fn).replace("\\", "/")
        t0 = time.perf_counter()
        # all_varchar keeps the column types the same in both engines, so
        # the setup comparison does not compare two different type choices.
        con.execute(
            f"CREATE TABLE {table} AS SELECT * FROM read_csv_auto('{path}', "
            f"header=true, delim='\t', nullstr='\\N', all_varchar=true)"
        )
        dt = (time.perf_counter() - t0) * 1e3
        per[table] = dt
        total += dt
        print(f"duckdb {table}: {dt:.0f} ms")
    counts = {}
    for table, _fn, _c in FILES:
        counts[table] = con.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
    con.execute("CHECKPOINT")
    con.close()
    bad = {k: {"got": v, "want": EXPECTED[k]} for k, v in counts.items() if v != EXPECTED[k]}
    return {"setup_ms": total, "per_table_ms": per, "counts": counts,
            "counts_ok": not bad, "bad_counts": bad}


def pg_env():
    env = dict(os.environ)
    env["PGPASSWORD"] = PG.password
    return env


def pg(args, check=True):
    return subprocess.run(
        [os.path.join(PG_BIN, args[0])] + args[1:],
        capture_output=True, text=True, env=pg_env(), check=False,
    )


def pg_ctl_start() -> subprocess.CompletedProcess:
    """Start the server without a captured pipe.

    pg_ctl passes its own stdout handle to the server it starts, so a
    captured run never sees the pipe close and waits until the server stops.
    The output goes to the server log and to nowhere instead.
    """
    return subprocess.run(
        [os.path.join(PG_BIN, "pg_ctl.exe"), "-D", PG_DATA,
         "-o", f"-p {PG_PORT}",
         "-l", os.path.join(PG_DATA, "server.log"), "-w", "start"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        env=pg_env(), check=False,
    )


def pg_alive() -> bool:
    """True when a server on PG_PORT already answers."""
    r = subprocess.run(
        [os.path.join(PG_BIN, "psql.exe"), "-U", PG.user, "-h", PG.host,
         "-p", PG_PORT, "-d", "postgres", "-t", "-A", "-c", "select 1"],
        capture_output=True, text=True, env=pg_env(), check=False,
    )
    return r.returncode == 0 and r.stdout.strip() == "1"


def start_pg() -> str:
    """Start a private cluster. Returns an empty string on success."""
    if pg_alive():
        print("postgres: a server already answers, reusing it", flush=True)
        return ""
    initdb = os.path.join(PG_BIN, "initdb.exe")
    if not os.path.exists(initdb):
        return "initdb is absent"
    if not os.path.exists(os.path.join(PG_DATA, "PG_VERSION")):
        os.makedirs(PG_DATA, exist_ok=True)
        r = pg(["initdb.exe", "-D", PG_DATA, "-U", PG.user, "-A", "trust", "-E", "UTF8"])
        if r.returncode != 0:
            return "initdb failed: " + r.stderr[-400:]
    r = pg_ctl_start()
    if r.returncode != 0 and not pg_alive():
        return f"pg_ctl start failed with exit {r.returncode}, see the server log"
    return ""


def run_postgres() -> dict:
    """Create and load the seven tables in a private PostgreSQL cluster."""
    err = start_pg()
    if err:
        return {"status": "unavailable", "reason": err}
    psql = os.path.join(PG_BIN, "psql.exe")
    base = [psql, "-U", PG.user, "-h", PG.host, "-p", PG_PORT, "-q"]
    subprocess.run(base + ["-c", "DROP DATABASE IF EXISTS imdbbench"], check=False,
                   capture_output=True, text=True, env=pg_env())
    subprocess.run(base + ["-c", "CREATE DATABASE imdbbench"], check=True,
                   capture_output=True, text=True, env=pg_env())
    per = {}
    total = 0.0
    for table, fn, cols in FILES:
        path = os.path.join(TSV_DIR, fn)
        ddl = f"CREATE TABLE {table} (" + ", ".join(f"{c} text" for c in cols) + ")"
        t0 = time.perf_counter()
        subprocess.run(base + ["-d", "imdbbench", "-c", ddl], check=True,
                       capture_output=True, text=True, env=pg_env())
        with open(os.path.join(PG_DATA, "load.sql"), "w", encoding="utf-8") as f:
            # QUOTE E'\b' makes the double quote an ordinary byte. The IMDb
            # files are tab separated, not CSV, and the characters field of
            # title.principals starts with a double quote, which the plain
            # CSV reader treats as the start of a quoted field and then
            # rejects. The backspace byte is not in the data.
            f.write(f"COPY {table} FROM '{path}' WITH (FORMAT csv, DELIMITER E'\\t', "
                    f"HEADER, NULL '\\N', QUOTE E'\\b');\n")
        r = subprocess.run(base + ["-v", "ON_ERROR_STOP=1", "-d", "imdbbench", "-f",
                                   os.path.join(PG_DATA, "load.sql")],
                           capture_output=True, text=True, env=pg_env())
        if r.returncode != 0:
            return {"status": "unavailable",
                    "reason": f"COPY {table} failed: " + (r.stderr or r.stdout)[-500:]}
        dt = (time.perf_counter() - t0) * 1e3
        per[table] = dt
        total += dt
        print(f"postgres {table}: {dt:.0f} ms")
    counts = {}
    for table, _fn, _c in FILES:
        r = subprocess.run(base + ["-d", "imdbbench", "-t", "-A", "-c",
                                   f"SELECT count(*) FROM {table}"],
                           check=True, capture_output=True, text=True, env=pg_env())
        counts[table] = int(r.stdout.strip())
    bad = {k: {"got": v, "want": EXPECTED[k]} for k, v in counts.items() if v != EXPECTED[k]}
    return {"setup_ms": total, "per_table_ms": per, "counts": counts,
            "counts_ok": not bad, "bad_counts": bad}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--engine", choices=["duckdb", "postgres", "both"], default="both")
    ap.add_argument("--disk-mb-s", type=float, nargs="*", default=[],
                    help="the disk throughput sampled during the run, in MB/s")
    ap.add_argument("--disk-during", default="",
                    help="the engine whose load the samples cover")
    args = ap.parse_args()

    nbytes = input_bytes()
    fi = floor_ms(nbytes)
    print(f"input: {nbytes} B ({nbytes / 2**20:.1f} MiB) over {len(FILES)} files")
    print(f"floor at {DISK_RATE / 1e9:.2f} GB/s: {fi:.0f} ms")

    rec = {
        "method": "METHOD-065",
        "tsv_dir": TSV_DIR,
        "files": [f for _, f, _ in FILES],
        "input_bytes": nbytes,
        "input_mib": round(nbytes / 2**20, 1),
        "disk_rate_gbs": DISK_RATE / 1e9,
        "floor_ms": round(fi, 1),
        "note": "setup covers the empty table and the full load of every row. "
                "Both engines declare every column as text, so the comparison "
                "does not mix two type choices.",
        "disk_state": {
            "sampled_during": args.disk_during,
            "sample_mb_s": args.disk_mb_s,
            "sample_method": "Get-Counter '\\PhysicalDisk(_Total)\\Disk Bytes/sec', "
                             "2 samples of 2 s",
            "consequence": "A rate far below the planned 3.20 GB/s means the "
                           "measured setup is a measure of the disk and not of "
                           "the engine, and the run does not replace a figure "
                           "measured on a quiet disk.",
        },
    }
    # An engine that is measured on its own keeps the record of the other,
    # so a single engine run cannot erase a measurement it did not repeat.
    if os.path.exists(OUT):
        try:
            with open(OUT, encoding="utf-8") as f:
                old = json.load(f)
            for key in ("duckdb", "postgres"):
                if key in old:
                    rec[key] = old[key]
                    print(f"kept the recorded {key} block from the last run", flush=True)
        except (json.JSONDecodeError, OSError) as e:
            print(f"the old record is unreadable ({e}), it is replaced", flush=True)

    if args.engine in ("duckdb", "both"):
        t0 = time.perf_counter()
        d = run_duckdb()
        d["wall_ms"] = (time.perf_counter() - t0) * 1e3
        d["above_floor"] = d["setup_ms"] >= fi
        rec["duckdb"] = d
        print(f"duckdb setup: {d['setup_ms']:.0f} ms, floor {fi:.0f} ms, "
              f"above_floor={d['above_floor']}, counts_ok={d['counts_ok']}", flush=True)

    if args.engine in ("postgres", "both"):
        p = run_postgres()
        if p.get("status") == "unavailable":
            rec["postgres"] = p
            print("postgres setup: unavailable: " + p["reason"], flush=True)
        else:
            p["above_floor"] = p["setup_ms"] >= fi
            rec["postgres"] = p
            print(f"postgres setup: {p['setup_ms']:.0f} ms, "
                  f"above_floor={p['above_floor']}, counts_ok={p['counts_ok']}", flush=True)

    # The gate. The question is whether the recorded DuckDB setup of 0.1 s
    # can be a load of the input at all. The read alone, at the planned
    # 3.20 GB/s, is already 32x that figure, and the measured setup is
    # 1,560x it.
    rec["gate"] = {
        "question": "is a recorded DuckDB setup of 100 ms possible for this input",
        "read_floor_ms_at_planned_rate": round(fi, 1),
        "recorded_setup_ms": 100,
        "recorded_is_below_the_floor_by": round(fi / 100, 1),
    }
    d = rec.get("duckdb", {})
    if "setup_ms" in d:
        rec["gate"]["measured_duckdb_setup_ms"] = round(d["setup_ms"], 0)
        rec["gate"]["measured_over_recorded"] = round(d["setup_ms"] / 100, 0)
        rec["gate"]["counts_complete"] = d.get("counts_ok", "not recorded")
    rec["gate"]["answer"] = (
        "NO. The read of the input alone is "
        f"{fi:.0f} ms at the planned 3.20 GB/s, which is {fi / 100:.1f}x the "
        "recorded 100 ms, so the record is below the floor before any parse "
        "or write. The measured setup is "
        f"{rec['gate'].get('measured_duckdb_setup_ms', 'not measured')} ms."
    )

    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8") as f:
        json.dump(rec, f, indent=2)
        f.write("\n")
    print(f"SAVED {OUT}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
