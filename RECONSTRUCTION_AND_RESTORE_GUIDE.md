# Repository Reconstruction and Restore Guide

This document describes how to reconstruct the SheafDB workspace from the archive `d2_git_backup.zip`.

---

## 1. Archive Contents

The archive contains:
* Complete Git history in the `.git` directory.
* All 368 source code files, benchmark scripts, and documentation files tracked by Git.
* This reconstruction guide.

The archive excludes:
* The `.env` credential file.
* The 10 GB benchmark datasets in `data/`.
* Rust build artifacts in `sheaf-db/target/`.
* Local analysis JSON files (`fiber-bundle-build-avoidance-plan.json`, `html-conversation-to-timestamped-markdown.json`, `von-neumann-architecture-physical-limits.json`).

---

## 2. Reconstruction Steps

### Step 1: Extract the Archive

Extract `d2_git_backup.zip` to your target directory (for example, `d:\d2`):

```powershell
Expand-Archive -Path "$HOME\Downloads\d2_git_backup.zip" -DestinationPath "d:\d2" -Force
```

### Step 2: Make Sure that the Git Tree is Clean

Open a terminal in the target directory and run:

```powershell
cd d:\d2
git status
git log -n 5 --oneline
```

The working tree must report clean on branch `fiber-bundle-replacement`.

### Step 3: Recreate the Environment Configuration

Copy `.env.example` to `.env` and fill in your personal GitHub credentials:

```powershell
Copy-Item .env.example .env
```

Edit `.env` to supply:
* `PERSONAL_GITHUB_TOKEN` (your GitHub personal access token).
* `PERSONAL_GITHUB_REPO` (your remote repository, e.g. `nithin12342/shelf-theory-db`).

### Step 4: Rebuild the Rust Engine

Compile the Sheaf database engine:

```powershell
cd d:\d2\sheaf-db
cargo build --release
cargo test
```

### Step 5: Restore Benchmark Datasets (Optional)

If you plan to run the IMDB or LDBC benchmarks:
1. Create directory `d:\d2\data\imdb`.
2. Download the official IMDB dump files from `https://datasets.imdbws.com/`:
   * `name.basics.tsv.gz`
   * `title.akas.tsv.gz`
   * `title.basics.tsv.gz`
   * `title.crew.tsv.gz`
   * `title.episode.tsv.gz`
   * `title.principals.tsv.gz`
   * `title.ratings.tsv.gz`
3. Extract each `.tsv.gz` file into `d:\d2\data\imdb\`.
4. See `DATASETS.md` for download URLs for IEEE-CIS fraud and SNAP Facebook datasets.

---

## 3. Verification Commands

To verify that the workspace runs correctly:

```powershell
# Run manual CLI suite
python run_cli_manual_suite.py

# Push changes using the personal push script (requires .env)
python push_personal.py
```
