#!/usr/bin/env python
"""N6 E2E runner: the fiber lifecycle gate (10,000 mixed read/write ops).

Runs the Rust bench (`fiber_lifecycle_bench`) and asserts every gate line is
PASS. The bench itself is the source of truth; this runner only turns a
gate failure into a non-zero exit code and records the result under
`results/`.

Rows = gate, Columns = verdict.
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
# tests/e2e/fiber_lifecycle -> tests/e2e -> tests -> sheaf-db -> workspace root
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
RESULTS = os.path.join(HERE, "results")
BIN = os.path.join(REPO, "sheaf-db", "target", "release", "fiber_lifecycle_bench.exe")
if not os.path.exists(BIN):
    BIN = os.path.join(REPO, "sheaf-db", "target", "release", "fiber_lifecycle_bench")

GATES = [
    "gate_n5_concurrent_build_equals_quiet",
    "gate_n5_frozen_epoch_exposed",
    "gate_n5_epoch_stamp_detects_staleness",
    "gate_n6_fiber_equals_fallback",
    "gate_n6_no_deadlock",
    "gate_n6_lag_bounded",
]


def main() -> int:
    if not os.path.exists(BIN):
        print(f"binary not found: {BIN}", file=sys.stderr)
        print("build it first: cargo build --release --bin fiber_lifecycle_bench", file=sys.stderr)
        return 2
    proc = subprocess.run(
        [BIN], cwd=os.path.join(REPO, "sheaf-db"), capture_output=True, text=True
    )
    out = proc.stdout
    print(out)
    verdicts = {}
    for line in out.splitlines():
        for gate in GATES:
            if line.startswith(gate + "="):
                verdicts[gate] = "PASS" if "=PASS" in line or " PASS" in line else "FAIL"
    missing = [g for g in GATES if g not in verdicts]
    failed = [g for g, v in verdicts.items() if v != "PASS"]
    os.makedirs(RESULTS, exist_ok=True)
    with open(os.path.join(RESULTS, "fiber_lifecycle.json"), "w", encoding="utf-8") as f:
        json.dump({"gates": verdicts, "missing": missing, "exit_code": proc.returncode}, f, indent=2)
    if missing or failed:
        print(f"FAIL: missing={missing} failed={failed}")
        return 1
    print(f"PASS: {len(verdicts)}/{len(GATES)} gates")
    return 0


if __name__ == "__main__":
    sys.exit(main())
