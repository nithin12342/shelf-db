"""run_all.py — one-command full-matrix bench. Keeps installs + data, never deletes.

Usage: py run_all.py [--engines sqlite,duckdb,pg] [--fresh] [--keep-up]
                     [--no-shelf] [--shelf-only]
                     [--data DIR] [--build DIR] [--datasets DIR] [--imdb DIR]
                     [--pg-app DIR] [--mariadb-app DIR] [--redis-server EXE]
                     [--mongod EXE] [--pgbench-data DIR]
The roots come from ~/.sheaf_bench.json, or from the flags above. The
repository holds no absolute path.
--fresh: delete sqlite/duckdb DB files first (true reload; default reuses =
  fast). --keep-up: leave servers running after. Results merge into existing
  JSONs as runN_dated entries (history preserved, never clobbered).
"""
import argparse
import datetime
import json
import os
import socket
import subprocess
import sys
import time

import bench_paths as P

HERE = os.path.dirname(os.path.abspath(__file__))
RES = str(P.RESULTS)
IST = datetime.timezone(datetime.timedelta(hours=5, minutes=30))


def set_roots(a):
    """Write the CLI roots into the environment for every child process."""
    for attr, env in (("data", "SHEAF_DATA"), ("build", "SHEAF_BUILD"),
                      ("datasets", "SHEAF_DATASETS"), ("imdb", "SHEAF_IMDB"),
                      ("pg_app", "SHEAF_PG_APP"), ("mariadb_app", "SHEAF_MARIADB_APP"),
                      ("redis_server", "SHEAF_REDIS_SERVER"), ("mongod", "SHEAF_MONGOD"),
                      ("pgbench_data", "SHEAF_PGBENCH_DATA")):
        value = getattr(a, attr)
        if value:
            os.environ[env] = value


def stamp():
    return datetime.datetime.now(IST).strftime("%Y-%m-%d %H:%M %Z")


def wait_port(host, port, timeout=120):
    t0 = time.time()
    while time.time() - t0 < timeout:
        try:
            socket.create_connection((host, port), timeout=2).close()
            return True
        except OSError:
            time.sleep(2)
    return False


def wait_ready(e, name):
    """Poll engine-level readiness (port-open is not ready). Returns True."""
    if e.get("ready_poll"):
        t0 = time.time()
        while time.time() - t0 < e.get("wait", 120):
            r = sh(e["ready_poll"], timeout=30)
            if r.returncode == 0:
                print(f"{name}: ready ({r.stdout.strip()[-60:]})")
                return True
            time.sleep(5)
        return False
    if e.get("ready_wait"):
        print(f"{name}: port open, ready-wait {e['ready_wait']}s (DB recovery)")
        time.sleep(e["ready_wait"])
    return True


def sh(cmd, timeout=3600):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True,
                          cwd=HERE, timeout=timeout)


# MongoDB (document store) was uninstalled on 2026-10-20: its $lookup join
# over the IMDb data measured 183,618.6 ms against ShelfDB's 142.7 ms
# (1,287x). ScyllaDB (CQL, wide-column) takes the NoSQL slot: it moves the
# join to write time and answers from a clustering-range scan.
def engines():
    """The engine matrix. Every path and port comes from the bench config."""
    redis = P.service("redis")
    mariadb = P.service("mariadb")
    pg = P.service("pg")
    neo4j = P.service("neo4j")
    scylla = P.service("scylla")
    return {
        "sqlite": {"bench": "py sqlite_bench.py", "json": "sqlite.json"},
        "duckdb": {"bench": "py duckdb_bench.py", "json": "duckdb.json"},
        "redis": {"bench": "py redis_bench.py", "json": "redis.json",
                  "pexec": [P.redis_server(), "--dir", P.data("redis"),
                            "--maxmemory", "4gb", "--maxmemory-policy", "noeviction",
                            "--port", str(redis.port)],
                  "host": redis.host, "port": redis.port, "wait": 30,
                  "stop_im": "redis-server.exe"},
        "scylladb": {"bench": "py scylla_bench.py", "json": "scylladb.json",
                     "start": "docker start bench-scylla", "host": scylla.host,
                     "port": scylla.port, "wait": 220, "ready_wait": 150,
                     "stop_cmd": "docker stop bench-scylla"},
        "mariadb": {"bench": "py sql_bench.py mariadb", "json": "mariadb.json",
                    "pexec": [P.mariadb_app("mysqld.exe"),
                              "--datadir=" + P.data("mariadb"),
                              f"--port={mariadb.port}",
                              "--innodb-buffer-pool-size=2G", "--console"],
                    "host": mariadb.host, "port": mariadb.port, "wait": 60,
                    "stop_im": "mysqld.exe"},
        "pg": {"bench": "py sql_bench.py pg", "json": "pg.json",
               "pexec": [P.pg_app("pg_ctl.exe"), "-D", P.data("pgsql"),
                         "-o", f"-p {pg.port} -c shared_buffers=2GB -c synchronous_commit=on",
                         "-l", P.data("pgsql", "logfile"), "start"],
               "host": pg.host, "port": pg.port, "wait": 180,
               "ready_poll": f'"{P.pg_app("pg_isready.exe")}" -h {pg.host} -p {pg.port}',
               "stop_cmd": f'"{P.pg_app("pg_ctl.exe")}" -D "{P.data("pgsql")}" stop'},
        "neo4j": {"bench": "py neo4j_bench.py", "json": "neo4j.json",
                  "start": "docker start bench-neo4j", "host": neo4j.host,
                  "port": neo4j.port, "wait": 150, "ready_wait": 75,
                  "stop_cmd": "docker stop bench-neo4j"},
    }


ORDER = ["sqlite", "duckdb", "redis", "mariadb", "pg", "neo4j", "scylladb"]


def shelf_bins():
    """The shelf binaries and the data paths each one needs."""
    tpch = P.build("tpch-dbgen")
    lineitem = P.build("tpch-dbgen", "lineitem.tbl")
    snap = P.data("fiber_snapshot.bin")
    snap_bad = P.data("fiber_snapshot.bad")
    return [
        ("sheaf-engine", "tpch_scan_bench", [f"--lineitem={lineitem}"]),
        ("sheaf-engine", "fused_scan_bench", [f"--lineitem={lineitem}"]),
        ("sheaf-engine", "fiber_join_bench", [f"--dir={tpch}", f"--snap={snap}"]),
        ("sheaf-engine", "fiber_snap_bench",
         [f"--dir={tpch}", f"--snap={snap}", f"--snap-bad={snap_bad}"]),
        ("sheaf-engine", "range_slice_bench", []),
        ("sheaf-engine", "graph_bench", [f"--dir={P.data('neo4j', 'import')}"]),
        ("sheaf-engine", "mt_bench", []),
        ("sheaf-storage", "wal_insert_bench", []),
        ("sheaf-engine", "epoch_wal_bench", []),
        ("sheaf-engine", "p0_gates_bench", []),
        ("sheaf-engine", "real_load_bench",
         [f"--fraud={P.datasets('ieee-fraud', 'train_transaction.csv')}",
          f"--fb={P.datasets('facebook_combined.txt')}"]),
        ("sheaf-engine", "imdb_fiber_bench",
         [f"--imdb-dir={P.imdb()}",
          f"--bundle={P.data('imdb_fiber_bundle.bin')}",
          f"--baseline={P.RESULTS / 'imdb_job.json'}",
          f"--output={P.RESULTS / 'imdb_shelf.json'}"]),
    ]


HEADLINES = {"sqlite.json": ("join_ms", "scan_agg_ms"), "duckdb.json": ("join_ms", "scan_agg_ms"),
             "redis.json": ("point_lookup_us", "lua_5key_us"), "mongodb.json": ("lookup4_indexed_ms", "mixed_32t"),
             "mariadb.json": ("join_ms", "scan_agg_ms"), "pg.json": ("join_ms", "scan_agg_ms"),
             "neo4j.json": ("full_traversal_ms", "two_hop_us"), "scylladb.json": ("point_lookup_us", "range_slice_ms")}


def merge_result(path, prior):
    """Wrap a fresh single-run output with prior multi-run history."""
    try:
        new = json.load(open(path))
    except Exception as e:
        return {"error": f"bench output unreadable: {e}", "prior_kept": bool(prior)}
    runs = {k: v for k, v in (prior or {}).items() if k.startswith("run")}
    tag = f"run{len(runs) + 1}_{datetime.datetime.now(IST).strftime('%Y%m%d')}"
    if runs or not any(k.startswith("run") for k in new):
        merged = dict(prior or {})
        merged[tag] = new
        merged[tag]["note"] = f"runner {stamp()}"
        merged["note"] = f"{len(runs) + 1} runs; latest {tag}."
        return merged
    return new


def docker_ok():
    r = sh("docker info", timeout=60)
    return r.returncode == 0


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--engines", default=",".join(ORDER))
    ap.add_argument("--fresh", action="store_true")
    ap.add_argument("--keep-up", action="store_true")
    ap.add_argument("--no-shelf", action="store_true")
    ap.add_argument("--shelf-only", action="store_true")
    ap.add_argument("--data")
    ap.add_argument("--build")
    ap.add_argument("--datasets")
    ap.add_argument("--imdb")
    ap.add_argument("--pg-app")
    ap.add_argument("--mariadb-app")
    ap.add_argument("--redis-server")
    ap.add_argument("--mongod")
    ap.add_argument("--pgbench-data")
    a = ap.parse_args()
    set_roots(a)
    E = engines()
    want = [e for e in a.engines.split(",") if e in E]
    if a.fresh:
        for f in (P.data("sqlite", "bench.db"), P.data("duckdb", "bench.duckdb")):
            if os.path.exists(f):
                os.remove(f)
                print(f"fresh: deleted {f}")
    summary = {}
    started = []
    have_docker = docker_ok() if not a.shelf_only else True
    if not a.shelf_only and not have_docker:
        print("PREFLIGHT: docker daemon down — neo4j/scylladb will SKIP (no fake numbers)")
    if not a.shelf_only:
        for name in ORDER:
            if name not in want:
                continue
            e = E[name]
            print(f"\n===== {name} =====")
            if name in ("neo4j", "scylladb") and not have_docker:
                print(f"{name}: SKIP (docker daemon down)"); summary[name] = "SKIP-NO-DOCKER"; continue
            if e.get("pexec"):
                subprocess.Popen(e["pexec"], cwd=HERE, stdout=subprocess.DEVNULL,
                                 stderr=subprocess.DEVNULL,
                                 creationflags=subprocess.DETACHED_PROCESS)
                time.sleep(3)
                ok = wait_port(e["host"], e["port"], e.get("wait", 120))
                if ok:
                    ok = wait_ready(e, name)
            elif e.get("start"):
                sh(e["start"])
                ok = wait_port(e["host"], e["port"], e.get("wait", 120))
                if ok:
                    ok = wait_ready(e, name)
            else:
                ok = True
            if not ok:
                print(f"{name}: server failed to start, SKIP"); summary[name] = "SERVER-FAIL"; continue
            started.append(name)
            prior = None
            jp = os.path.join(RES, e["json"])
            if os.path.exists(jp):
                try:
                    prior = json.load(open(jp))
                except Exception:
                    prior = None
            r = sh(e["bench"])
            print((r.stdout or "")[-600:])
            if r.returncode != 0:
                print(f"{name}: BENCH-FAIL\n{(r.stdout or '')[-800:]}\n{(r.stderr or '')[-400:]}")
                summary[name] = "BENCH-FAIL"; continue
            try:
                merged = merge_result(jp, prior)
                json.dump(merged, open(jp, "w"), indent=2)
                summary[name] = "OK"
            except Exception as ex:
                summary[name] = f"MERGE-FAIL {ex}"
    if not a.no_shelf:
        print("\n===== shelf-db bins =====")
        manifest = str(P.SHEAF_DB / "Cargo.toml")
        subprocess.run(["cargo", "build", "--manifest-path", manifest, "--release",
                        "--bins", "-p", "sheaf-engine", "-p", "sheaf-storage"],
                       cwd=HERE, timeout=3600)
        for crate, b, extra in shelf_bins():
            cmd = ["cargo", "run", "--manifest-path", manifest, "-p", crate,
                   "--bin", b, "--release", "--"] + extra
            r = subprocess.run(cmd, capture_output=True, text=True, cwd=HERE, timeout=3600)
            out = (r.stdout or "")[-300:]
            print(f"-- {b}: {out.strip().splitlines()[-1] if out.strip() else 'FAIL'}")
            summary[f"shelf:{b}"] = "OK" if r.returncode == 0 else "FAIL"
            if r.returncode != 0:
                print((r.stderr or "")[-400:])
    if not a.keep_up:
        for name in started:
            e = E[name]
            if e.get("stop_im"):
                sh(f'taskkill /F /IM {e["stop_im"]} >nul 2>&1')
            elif e.get("stop_cmd"):
                sh(e["stop_cmd"])
        print("servers stopped (installs + data kept)")
    print("\n===== HEADLINES =====")
    for name in ORDER:
        if name not in want:
            continue
        try:
            d = json.load(open(os.path.join(RES, E[name]["json"])))
            rk = [k for k in d if k.startswith("run")][-1]
            cols = HEADLINES[E[name]["json"]]
            vals = {c: (d[rk].get(c, {}).get("p50", d[rk].get(c, {}).get("ms", d[rk].get(c)))) for c in cols}
            print(f"{name} [{rk}]: {vals}")
        except Exception as ex:
            print(f"{name}: unreadable ({ex})")
    sp = os.path.join(RES, f"RUN_SUMMARY_{datetime.datetime.now(IST).strftime('%Y%m%d_%H%M')}.json")
    json.dump({"stamp": stamp(), "summary": summary}, open(sp, "w"), indent=2)
    print("SAVED", sp)


if __name__ == "__main__":
    sys.exit(main())
