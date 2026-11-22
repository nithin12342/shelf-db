#!/usr/bin/env bash
# compile_ebpf.sh - build the eBPF VFS interception program (C-07).
#
# Timestamp: 2026-11-22_12-48-16 +05:30
# Remote: origin https://github.com/nithin12342/shelf-db.git
# Branch: combined-stack-feature-improvement
# Base commit: 3832065581f3ff4240f4d267c38bcde8fea1c24e
#
# The script compiles native/ebpf/sheaf_vfs.bpf.c with the clang BPF target.
# It checks three gates in order: clang, the BPF backend, and the libbpf
# headers. Each missing gate prints UNSUPPORTED with the named missing tool.
# The script never prints PASS when a gate is absent.

set -euo pipefail
cd "$(dirname "$0")/.."

SOURCE="native/ebpf/sheaf_vfs.bpf.c"
OUTPUT="${1:-native/ebpf/sheaf_vfs.bpf.o}"

unsupported() {
  echo "UNSUPPORTED: eBPF VFS compile cannot run. Missing: $1"
  exit 0
}

if ! command -v clang >/dev/null 2>&1; then
  unsupported "clang"
fi

if ! clang -print-targets 2>/dev/null | grep -Eq '(^|[[:space:]])bpf(el|eb)?([[:space:]]|$)'; then
  unsupported "clang BPF target"
fi

# clang with -target bpf does not add the host multiarch include directory.
# linux/types.h needs asm/types.h from there, so find it once and reuse it.
MULTIARCH_INCLUDE=""
for candidate in \
    "/usr/include/$(clang -print-multiarch 2>/dev/null)" \
    "/usr/include/$(uname -m)-linux-gnu"; do
  if [ -n "$candidate" ] && [ -f "$candidate/asm/types.h" ]; then
    MULTIARCH_INCLUDE="$candidate"
    break
  fi
done

BPF_FLAGS=(-target bpf)
if [ -n "$MULTIARCH_INCLUDE" ]; then
  BPF_FLAGS+=("-I" "$MULTIARCH_INCLUDE")
fi

# A syntax-only include probe proves that bpf/bpf_helpers.h is reachable.
# linux/types.h comes first because the libbpf helper declarations use __u32
# and __u64 from that header.
if ! printf '#include <linux/types.h>\n#include <bpf/bpf_helpers.h>\nint probe(void) { return 0; }\n' \
    | clang "${BPF_FLAGS[@]}" -x c -fsyntax-only - >/dev/null 2>&1; then
  unsupported "libbpf headers (bpf/bpf_helpers.h)"
fi

if [ ! -f "$SOURCE" ]; then
  unsupported "BPF source $SOURCE"
fi

clang "${BPF_FLAGS[@]}" -O2 -g -Wall -Werror -c "$SOURCE" -o "$OUTPUT"
echo "PASS: compiled $SOURCE to $OUTPUT"
