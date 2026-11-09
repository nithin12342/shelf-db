"""_capture_shelf_bins.py - write the ShelfDB binary numbers to tracked JSON.

Two modes.

  live mode runs the binaries and records their stdout.
  log mode parses a saved capture log, so a pass that ran earlier still gets a
  tracked record even when the raw log is not tracked.

Usage:
  py _capture_shelf_bins.py
  py _capture_shelf_bins.py --from-log results/shelf_bins_20261026.log \
      --stamp "2026-10-26 11:44 UTC+05:30" \
      --out results/shelf_bins_20261026_quiet.json

The regexes are the same in both modes, so a log pass and a live pass produce
the same fields.
"""
import argparse
import datetime
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SHEAF = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
RES = os.path.join(HERE, "results")
IST = datetime.timezone(datetime.timedelta(hours=5, minutes=30))
TPCH = r"D:\bench\build\tpch-dbgen"
SNAP = r"D:\bench\data\fiber_snapshot.bin"
NEO = r"D:\bench\data\neo4j\import"

BENCHES = {
    "tpch_scan_bench": lambda: [f"--lineitem={TPCH}\\lineitem.tbl"],
    "fused_scan_bench": lambda: [f"--lineitem={TPCH}\\lineitem.tbl"],
    "fiber_join_bench": lambda: [f"--dir={TPCH}", f"--snap={SNAP}"],
    "range_slice_bench": lambda: [],
    "graph_bench": lambda: [f"--dir={NEO}"],
}


def num(pattern, text):
    m = re.search(pattern, text)
    return float(m.group(1)) if m else None


def extract(name, body):
    """Return the recorded fields for one binary, from its stdout."""
    if name == "tpch_scan_bench":
        return {
            "rep_ms": num(r"rep_ms=([0-9.]+)", body),
            "load_s": num(r"load_s=([0-9.]+)", body),
            "resident_MB": num(r"resident_MB=([0-9.]+)", body),
            "sum": num(r"sum=([0-9.]+)", body),
            "target_duckdb_ms": num(r"target_duckdb_ms=([0-9.]+)", body),
            "gate": "PASS" if "gate_scan=PASS" in body else "MISS",
        }
    if name == "fused_scan_bench":
        return {
            "best_ms": num(r"best_ms=([0-9.]+)", body),
            "median_ms": num(r"median_ms=([0-9.]+)", body),
            "load_s": num(r"load_s=([0-9.]+)", body),
            "fused_MB": num(r"fused_MB=([0-9.]+)", body),
            "sum": num(r"sum=([0-9.]+)", body),
            "gate": "PASS" if "gate_fused_target=PASS" in body else "MISS",
        }
    if name == "fiber_join_bench":
        m = re.search(r"load_ms=([0-9.]+) query_ms=([0-9.]+) total_ms=([0-9.]+)", body)
        return {
            "load_ms": float(m.group(1)) if m else None,
            "query_ms": float(m.group(2)) if m else None,
            "total_ms": float(m.group(3)) if m else None,
            "total_sum": num(r"total=([0-9.]+) ", body),
            "gate": "PASS" if "FIBER-JOIN-BENCH: PASS" in body else "FAIL",
        }
    if name == "range_slice_bench":
        return {
            "cells": num(r"cells=([0-9]+)", body),
            "best_ms": num(r"best_ms=([0-9.]+)", body),
            "rows": num(r"rows=([0-9]+)", body),
            "gate": "PASS" if "gate_range_target=PASS" in body else "MISS",
        }
    if name == "graph_bench":
        return {
            "cells": num(r"cells=([0-9]+)", body),
            "load_s": num(r"load_s=([0-9.]+)", body),
            "resident_MB": num(r"resident_MB=([0-9.]+)", body),
            "sampled_2hop_p50_us": num(r"p50_us=([0-9.]+)", body),
            "full_paths": num(r"paths=([0-9]+)", body),
            "full_ms": num(r"expect [0-9]+\) ms=([0-9.]+)", body),
            "gate": "PASS" if "gate_traverse=PASS" in body else "MISS",
        }
    return {}


def parse_log(text):
    """Split a capture log into per-binary blocks and extract each one."""
    out = {}
    text = text.lstrip("\ufeff")
    parts = re.split(r"^===== (\S+) =====\s*$", text, flags=re.M)
    for i in range(1, len(parts), 2):
        name, body = parts[i], parts[i + 1]
        out[name] = extract(name, body)
        out[name]["log_block"] = body.strip().splitlines()
    return out


def run(name):
    cmd = ["cargo", "run", "--release", "--bin", name, "--"] + BENCHES[name]()
    r = subprocess.run(cmd, cwd=SHEAF, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"{name}: FAIL {r.stderr[-300:]}")
        sys.exit(1)
    return r.stdout


def live():
    out = {}
    for name in BENCHES:
        body = run(name)
        out[name] = extract(name, body)
        out[name]["stdout"] = body.strip().splitlines()
    tql_out = run_tql()
    out["tql_seven_db_bench"] = tql_cases(tql_out)
    out["tql_seven_db_bench"]["stdout"] = tql_out.strip().splitlines()
    return out


def run_tql():
    cmd = ["cargo", "run", "--release", "--bin", "tql_seven_db_bench"]
    r = subprocess.run(cmd, cwd=SHEAF, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"tql_seven_db_bench: FAIL {r.stderr[-300:]}")
        sys.exit(1)
    return r.stdout


def tql_cases(text):
    cases = []
    for block in text.split("--- Case ")[1:]:
        cases.append({
            "case": int(block.split(":", 1)[0]),
            "unoptimized": num(r"Unoptimized estimated cost: ([0-9.]+)", block),
            "optimized": num(r"Optimized estimated cost:\s+([0-9.]+)", block),
            "gain": num(r"Optimizer efficiency gain:\s+([0-9.]+)x", block),
            "pass": "status: PASS" in block,
        })
    return {"cases": cases, "all_pass": "TQL-SEVEN-DB-BENCH: ALL PASS" in text}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--from-log", help="capture log to parse, instead of running")
    ap.add_argument("--stamp", help="stamp text for the parsed pass")
    ap.add_argument("--out", default="shelf_bins_20261026.json")
    a = ap.parse_args()

    if a.from_log:
        if not a.stamp:
            sys.exit("--stamp is required with --from-log")
        text = open(a.from_log, encoding="utf-8", errors="replace").read()
        out = {"stamp": a.stamp, "source": os.path.basename(a.from_log)}
        out.update(parse_log(text))
    else:
        out = {"stamp": datetime.datetime.now(IST).strftime("%Y-%m-%d %H:%M:%S %Z"),
               "host": "DLUX-INLT-042"}
        out.update(live())

    path = a.out if os.path.isabs(a.out) else os.path.join(RES, a.out)
    json.dump(out, open(path, "w"), indent=2)
    print("SAVED", path)
    print(json.dumps({k: {kk: vv for kk, vv in v.items()
                          if kk not in ("stdout", "log_block")}
                      for k, v in out.items() if isinstance(v, dict)}, indent=1))


if __name__ == "__main__":
    main()
