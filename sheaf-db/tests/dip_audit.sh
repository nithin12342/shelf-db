#!/bin/sh
# tests/dip_audit.sh — enforce inward-only import arrows (ARCHITECTURE.md line 4).
# Fails (exit 1) on any violation. Run after every phase gate.
# Rule: domain <- application <- infrastructure <- interfaces.
# Cargo-level edges allowed: engine->{core,storage,kernel,query},
# query->{core}, storage->{core}. Everything else must go through the
# documented waivers (ARCHITECTURE.md, now superseded per A4).
set -e
cd "$(dirname "$0")/.."
FAIL=0

# 1. Crate-level: no undeclared cross-crate imports (cargo enforces declared
# deps at build time; this catches undeclared ones that slipped via extern).
echo "--- crate-level edges ---"
for c in sheaf-core sheaf-storage sheaf-kernel sheaf-query sheaf-engine; do
  echo "crate $c declares:"
  grep -A10 '\[dependencies\]' "crates/$c/Cargo.toml" | grep -o 'sheaf-[a-z]*' | sort -u || true
done
# sheaf-core and sheaf-kernel must depend on nothing (pure math).
if grep -A10 '\[dependencies\]' crates/sheaf-core/Cargo.toml | grep -q 'sheaf-'; then
  echo "VIOLATION: sheaf-core depends on another sheaf crate"; FAIL=1
fi
if grep -A10 '\[dependencies\]' crates/sheaf-kernel/Cargo.toml | grep -q 'sheaf-'; then
  echo "VIOLATION: sheaf-kernel depends on another sheaf crate"; FAIL=1
fi
# sheaf-query may only depend on sheaf-core.
if grep -A10 '\[dependencies\]' crates/sheaf-query/Cargo.toml | grep 'sheaf-' | grep -qv 'sheaf-core'; then
  echo "VIOLATION: sheaf-query depends beyond sheaf-core"; FAIL=1
fi
# sheaf-storage may depend on sheaf-core (layout) + sheaf-kernel (W-D3:
# bin-only harness use, phase1_bench sorted-scan check; lib never imports it).
if grep -A10 '\[dependencies\]' crates/sheaf-storage/Cargo.toml | grep 'sheaf-' | grep -qv -e 'sheaf-core' -e 'sheaf-kernel'; then
  echo "VIOLATION: sheaf-storage depends beyond core+kernel(W-D3)"; FAIL=1
fi

# 2. Folder-level: pure-math folders must not import orchestration.
echo "--- folder-level arrows ---"
if grep -rn 'operators\|solvers' crates/sheaf-kernel/src/simd/ ; then
  echo "VIOLATION: simd/ imports orchestration"; FAIL=1
fi
if grep -rn '::ir\|ir::' crates/sheaf-query/src/syntax/ ; then
  echo "VIOLATION: syntax/ imports ir/"; FAIL=1
fi
if grep -rn 'sheaf_' crates/sheaf-core/src/ ; then
  echo "VIOLATION: sheaf-core imports a sheaf crate"; FAIL=1
fi
# Storage LIB code must never import kernel/query/engine (bins are harness
# and exempt; W-D3 covers the phase1_bench kernel use).
if grep -rn 'sheaf_kernel\|sheaf_query\|sheaf_engine' crates/sheaf-storage/src/bcsf/ crates/sheaf-storage/src/lsp/ crates/sheaf-storage/src/index/ ; then
  echo "VIOLATION: storage lib imports kernel/query/engine"; FAIL=1
fi

if [ "$FAIL" -eq 0 ]; then
  echo "DIP AUDIT: PASS"
else
  echo "DIP AUDIT: FAIL"
  exit 1
fi
