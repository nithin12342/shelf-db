#!/usr/bin/env python
"""package_distribution.py - Compile and package standalone SheafDB application.

This script compiles the production SheafDB binaries in release mode and
assembles an installable, self-contained distribution package.
"""

import os
import shutil
import subprocess
import sys
import time
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
MANIFEST = os.path.join(HERE, "sheaf-db", "Cargo.toml")
TARGET_DIR = os.path.join(HERE, "sheaf-db", "target", "release")
DIST_DIR = os.path.join(HERE, "dist", "sheaf-db")
ZIP_OUT = os.path.join(HERE, "dist", "sheaf-db-distribution.zip")

BINARIES = [
    "sheaf_db_app",
]


def run_command(cmd, cwd=None):
    """Run a system command and return stdout or raise an error."""
    res = subprocess.run(cmd, cwd=cwd, shell=True, capture_output=True, text=True)
    if res.returncode != 0:
        print(f"Command failed: {cmd}\n{res.stderr}", file=sys.stderr)
        sys.exit(res.returncode)
    return res.stdout


def build_release():
    """Compile the production binaries in release mode."""
    print("=== Step 1: Compiling application binaries in release profile ===")
    t0 = time.time()
    bin_args = " ".join([f"--bin {b}" for b in BINARIES])
    cmd = f"cargo build --manifest-path \"{MANIFEST}\" {bin_args} --release"
    out = run_command(cmd, cwd=HERE)
    elapsed = time.time() - t0
    print(f"Compilation finished in {elapsed:.2f} seconds.")
    return out


def assemble_package():
    """Assemble the standalone distribution folder structure."""
    print("=== Step 2: Assembling distribution directory structure ===")
    if os.path.exists(DIST_DIR):
        shutil.rmtree(DIST_DIR)
    bin_dir = os.path.join(DIST_DIR, "bin")
    conf_dir = os.path.join(DIST_DIR, "config")
    os.makedirs(bin_dir, exist_ok=True)
    os.makedirs(conf_dir, exist_ok=True)

    exe_suffix = ".exe" if sys.platform == "win32" else ""

    for b in BINARIES:
        src = os.path.join(TARGET_DIR, f"{b}{exe_suffix}")
        if not os.path.exists(src):
            src_no_ext = os.path.join(TARGET_DIR, b)
            if os.path.exists(src_no_ext):
                src = src_no_ext
            else:
                print(f"Error: binary {src} not found.", file=sys.stderr)
                sys.exit(1)
        dst = os.path.join(bin_dir, os.path.basename(src))
        shutil.copy2(src, dst)
        sz_kb = os.path.getsize(dst) / 1024.0
        print(f"  Packaged: {os.path.basename(dst)} ({sz_kb:.1f} KB)")

    src_conf = os.path.join(HERE, "sheaf-db", "tests", "e2e", "realtime_bench", "tql_workloads.conf")
    if os.path.exists(src_conf):
        shutil.copy2(src_conf, os.path.join(conf_dir, "tql_workloads.conf"))
        print("  Packaged: config/tql_workloads.conf")

    readme_content = """# SheafDB Standalone Distribution

SheafDB is a cellular sheaf database for high-performance topological queries.

## Contents
- `bin/sheaf_db_app`: Main standalone database engine executable.
- `config/tql_workloads.conf`: Reference TQL workload configurations.

## Installation
Add the `bin/` directory to your system PATH:

### Windows (PowerShell)
```powershell
$env:Path += ";$PWD\\bin"
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";$PWD\\bin", "User")
```

### Linux / macOS
```bash
export PATH="$PWD/bin:$PATH"
echo 'export PATH="'"$PWD"'/bin:$PATH"' >> ~/.bashrc
```

## Running the Database
Execute the standalone application directly:
```bash
./bin/sheaf_db_app
```
"""
    with open(os.path.join(DIST_DIR, "README.md"), "w", encoding="utf-8") as f:
        f.write(readme_content)

    install_bat = """@echo off
set SCRIPT_DIR=%~dp0
set BIN_DIR=%SCRIPT_DIR%bin
echo Installing SheafDB binaries into current session PATH...
set PATH=%BIN_DIR%;%PATH%
echo SheafDB path configured. Run 'sheaf_db_app' to start.
sheaf_db_app.exe
"""
    with open(os.path.join(DIST_DIR, "install_and_run.bat"), "w", encoding="utf-8") as f:
        f.write(install_bat)

    install_ps1 = """# install_and_run.ps1
$BinDir = Join-Path $PSScriptRoot "bin"
$env:PATH = "$BinDir;$env:PATH"
Write-Host "SheafDB path configured. Executing sheaf_db_app.exe..."
& "$BinDir\\sheaf_db_app.exe"
"""
    with open(os.path.join(DIST_DIR, "install_and_run.ps1"), "w", encoding="utf-8") as f:
        f.write(install_ps1)


def create_archive():
    """Create a distributable zip archive."""
    print("=== Step 3: Generating compressed distribution archive ===")
    os.makedirs(os.path.dirname(ZIP_OUT), exist_ok=True)
    if os.path.exists(ZIP_OUT):
        os.remove(ZIP_OUT)

    with zipfile.ZipFile(ZIP_OUT, "w", zipfile.ZIP_DEFLATED) as zf:
        for root, _, files in os.walk(DIST_DIR):
            for file in files:
                abs_path = os.path.join(root, file)
                rel_path = os.path.relpath(abs_path, os.path.dirname(DIST_DIR))
                zf.write(abs_path, rel_path)

    sz_mb = os.path.getsize(ZIP_OUT) / (1024.0 * 1024.0)
    print(f"Archive created: {ZIP_OUT} ({sz_mb:.2f} MB)")


def verify_distribution():
    """Run a smoke test on the packaged distribution binary."""
    print("=== Step 4: Verifying packaged standalone application ===")
    exe_suffix = ".exe" if sys.platform == "win32" else ""
    app_bin = os.path.join(DIST_DIR, "bin", f"sheaf_db_app{exe_suffix}")

    t0 = time.time()
    out = run_command(f"\"{app_bin}\"", cwd=DIST_DIR)
    elapsed_ms = (time.time() - t0) * 1000.0

    if "=== sheaf-db query result ===" in out and "HARMONIC_BOUNDED" in out:
        print(f"Smoke test PASSED ({elapsed_ms:.1f} ms).")
        print("Verification evidence: Matched hypercells and certified cohomology state.")
    else:
        print("Smoke test FAILED.", file=sys.stderr)
        print(out, file=sys.stderr)
        sys.exit(1)


def main():
    print("Starting automated SheafDB build and packaging pipeline.")
    build_release()
    assemble_package()
    create_archive()
    verify_distribution()
    print("=== Pipeline Complete: All gates passed successfully ===")


if __name__ == "__main__":
    main()
