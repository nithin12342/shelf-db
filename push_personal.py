#!/usr/bin/env python3
"""
push_personal.py - Secure push tool to personal GitHub repository.

Bypasses Windows Credential Manager without exposing personal tokens
in `.git/config` permanently. Supports loading credentials from a local
`.env` file or environment variables.
"""

import argparse
import datetime
import os
import re
import subprocess
import sys
from pathlib import Path


def load_env_file(filepath: Path) -> dict:
    """Simple parser for key=value pairs in a .env file."""
    env_vars = {}
    if not filepath.exists():
        return env_vars

    with open(filepath, "r", encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            if "=" in line:
                key, val = line.split("=", 1)
                env_vars[key.strip()] = val.strip().strip("\"'")
    return env_vars


def run_cmd(cmd: list, cwd: Path, check: bool = True, capture: bool = False):
    """Run a subprocess command safely."""
    res = subprocess.run(
        cmd,
        cwd=cwd,
        check=check,
        text=True,
        capture_output=capture,
    )
    return res


def get_current_branch(cwd: Path) -> str:
    """Returns the name of the currently checked-out branch, default to 'main'."""
    try:
        res = run_cmd(["git", "branch", "--show-current"], cwd=cwd, capture=True)
        branch = res.stdout.strip()
        return branch if branch else "main"
    except Exception:
        return "main"


def is_git_repo(cwd: Path) -> bool:
    """Check if the directory is inside a git repository."""
    try:
        res = run_cmd(["git", "rev-parse", "--is-inside-work-tree"], cwd=cwd, check=False, capture=True)
        return res.returncode == 0
    except FileNotFoundError:
        print("[!] Git executable not found on PATH.", file=sys.stderr)
        sys.exit(1)


def sanitize_repo_url(url: str) -> str:
    """Normalize repo name or URL into standard https://github.com/owner/repo format."""
    url = url.strip()
    # Match owner/repo pattern e.g., 'username/my-repo'
    if re.match(r"^[\w.-]+/[\w.-]+$", url):
        return f"https://github.com/{url}.git"
    if not url.endswith(".git") and "github.com" in url:
        return f"{url}.git"
    return url


# Push window policy: remotes are written only outside Mon-Fri 09:00-18:00
# local time. Weekday uses datetime.weekday() (Mon=0 .. Sun=6); the end hour
# is exclusive, so 18:00 onward is allowed.
BUSINESS_DAYS = {0, 1, 2, 3, 4}
BUSINESS_START_HOUR = 9
BUSINESS_END_HOUR = 18


def is_business_hours(now=None) -> bool:
    """True when pushes are blocked: Mon-Fri 09:00 (incl) to 18:00 (excl)."""
    now = now or datetime.datetime.now()
    return now.weekday() in BUSINESS_DAYS and BUSINESS_START_HOUR <= now.hour < BUSINESS_END_HOUR


def main():
    parser = argparse.ArgumentParser(
        description="Push local repository to personal GitHub using a Personal Access Token."
    )
    parser.add_argument(
        "--token",
        help="GitHub Personal Access Token (can also be set via GITHUB_TOKEN in env or .env file)",
    )
    parser.add_argument(
        "--repo",
        help="Target repository URL or 'username/repo' (can also be set via GITHUB_REPO)",
    )
    parser.add_argument(
        "--branch",
        help="Branch to push (defaults to current branch or 'main')",
    )
    parser.add_argument(
        "--remote",
        default="personal",
        help="Remote name to configure (default: 'personal')",
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Force push (--force-with-lease)",
    )
    parser.add_argument(
        "--allow-business-hours",
        action="store_true",
        help="Override the Mon-Fri 09:00-18:00 push block (use deliberately)",
    )
    parser.add_argument(
        "--dir",
        default=".",
        help="Repository directory (default: current directory)",
    )

    args = parser.parse_args()

    repo_dir = Path(args.dir).resolve()
    if not repo_dir.exists():
        print(f"[!] Directory not found: {repo_dir}", file=sys.stderr)
        sys.exit(1)

    # 1. Ensure it's a git repo
    if not is_git_repo(repo_dir):
        print(f"[*] Directory '{repo_dir}' is not a git repository yet. Initializing...")
        run_cmd(["git", "init"], cwd=repo_dir)

    # 2. Read configuration from CLI > environment > .env file
    env_file = repo_dir / ".env"
    file_vars = load_env_file(env_file)

    token = (
        args.token
        or os.environ.get("PERSONAL_GITHUB_TOKEN")
        or os.environ.get("GITHUB_TOKEN")
        or file_vars.get("PERSONAL_GITHUB_TOKEN")
        or file_vars.get("GITHUB_TOKEN")
    )

    repo_target = (
        args.repo
        or os.environ.get("PERSONAL_GITHUB_REPO")
        or os.environ.get("GITHUB_REPO")
        or file_vars.get("PERSONAL_GITHUB_REPO")
        or file_vars.get("GITHUB_REPO")
    )

    if not token:
        print(
            "[!] Error: No GitHub token provided.\n"
            "    Provide it via:\n"
            "      1. Argument: --token <token>\n"
            "      2. Environment variable: PERSONAL_GITHUB_TOKEN\n"
            "      3. Inside .env file: PERSONAL_GITHUB_TOKEN=ghp_xxxx",
            file=sys.stderr,
        )
        sys.exit(1)

    if not repo_target:
        print(
            "[!] Error: No target repository provided.\n"
            "    Provide it via:\n"
            "      1. Argument: --repo username/repository\n"
            "      2. Inside .env file: PERSONAL_GITHUB_REPO=username/repository",
            file=sys.stderr,
        )
        sys.exit(1)

    clean_repo_url = sanitize_repo_url(repo_target)

    # Extract user and repo path from https://github.com/<owner>/<repo>.git
    match = re.search(r"github\.com[/:]([\w.-]+)/([\w.-]+?)(?:\.git)?$", clean_repo_url)
    if not match:
        print(f"[!] Could not parse repository path from: {clean_repo_url}", file=sys.stderr)
        sys.exit(1)

    owner, repo_name = match.groups()
    authed_url = f"https://{token}@github.com/{owner}/{repo_name}.git"

    # Branch determination
    target_branch = args.branch or get_current_branch(repo_dir)

    print(f"[*] Target repository: https://github.com/{owner}/{repo_name}")
    print(f"[*] Target branch:     {target_branch}")
    print(f"[*] Remote name:       {args.remote}")

    # 2b. Push window gate: refuse before touching any remote state.
    now = datetime.datetime.now()
    if is_business_hours(now) and not args.allow_business_hours:
        print(
            f"[!] Push blocked: business hours are Mon-Fri "
            f"{BUSINESS_START_HOUR:02d}:00-{BUSINESS_END_HOUR:02d}:00 local time "
            f"(now: {now.strftime('%A %H:%M')}).\n"
            f"    Nothing was pushed and no remote was modified.\n"
            f"    Re-run off-hours, or pass --allow-business-hours to override deliberately.",
            file=sys.stderr,
        )
        sys.exit(3)

    # 3. Setup / update remote without persisting credentials permanently
    remotes_res = run_cmd(["git", "remote"], cwd=repo_dir, capture=True)
    existing_remotes = remotes_res.stdout.split()

    if args.remote in existing_remotes:
        run_cmd(["git", "remote", "set-url", args.remote, clean_repo_url], cwd=repo_dir)
    else:
        run_cmd(["git", "remote", "add", args.remote, clean_repo_url], cwd=repo_dir)

    # 4. Push directly with one-time authenticated URL or config override
    # We bypass Windows Credential Manager with -c credential.helper=""
    # and pass the authed URL directly to avoid persisting the token in .git/config!
    push_cmd = [
        "git",
        "-c", "credential.helper=",
        "push",
        authed_url,
        f"HEAD:{target_branch}",
    ]
    if args.force:
        push_cmd.append("-f")

    print("[*] Pushing commits to personal repository...")
    try:
        run_cmd(push_cmd, cwd=repo_dir)
        print(f"\n[+] Successfully pushed to https://github.com/{owner}/{repo_name} ({target_branch})!")
    except subprocess.CalledProcessError as e:
        print("\n[!] Push failed. Check token permissions (must have 'repo' scope) or branch conflict.", file=sys.stderr)
        sys.exit(e.returncode)


if __name__ == "__main__":
    main()
