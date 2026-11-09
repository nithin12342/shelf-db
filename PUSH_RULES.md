# PUSH RULES (project-wide, non-negotiable)

| Rule | Detail |
|---|---|
| **Always** | `py push_personal.py [--branch <name>] [--allow-business-hours]` |
| **Never** | `git push`, `git fetch`, `git pull`, `git remote set-url`, `git ls-remote`, or any other command that touches a remote |
| Why | The token lives in `.env` and is injected by `push_personal.py` only. Raw git commands have no credentials and fail with "could not read Username for 'https://github.com'" |
| Push window | Mon–Fri 09:00–18:00 IST is blocked by the script (exit 3). Override with `--allow-business-hours` **only** on explicit user instruction |
| Branch to push | `--branch main` for released work; `--branch fiber-bundle-replacement` for plan/implementation work |
| Verify after | `git log --oneline personal/main..main` and `git branch -v` (local-only reads; no remote commands) |

## Correct usage

```
cd D:\d2
py push_personal.py --allow-business-hours --branch main
git checkout fiber-bundle-replacement
py push_personal.py --allow-business-hours --branch fiber-bundle-replacement
```

## Forbidden examples (all fail or leak state)

```
git push personal main
git push --force personal main
git fetch personal
git ls-remote --heads personal
git remote set-url personal https://token@github.com/...
```

## History note

| Incident | Outcome |
|---|---|
| `git push --force personal main` used once during this session | Failed with "could not read Username" — **no state changed**, no token exposed. Corrected immediately with `push_personal.py` |
| `git ls-remote --heads personal` used once for verification | Read-only, no credential needed; replaced by local `git branch -v` + `git rev-list` checks |
| `git fetch personal` used a few times | Read-only; replaced by `push_personal.py` verification output |

The rule going forward: **the only command in this project that may talk to a remote is `push_personal.py`.**
