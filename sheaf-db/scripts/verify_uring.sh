#!/usr/bin/env bash
# verify_uring.sh - NF-14 Linux verification.
# Timestamp: 2026-11-22_09-02-56 +05:30
# Remote: origin https://github.com/nithin12342/shelf-db.git
# Branch: combined-stack-feature-improvement
# Base commit: 3b7e332

set -euo pipefail
cd "$(dirname "$0")/.."

if [ "$(uname -s)" != "Linux" ]; then
  echo "UNSUPPORTED: io_uring fixed-buffer writes require Linux."
  exit 0
fi

cargo test -p sheaf-native --test uring_linux -- --nocapture
