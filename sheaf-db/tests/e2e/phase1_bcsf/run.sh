# Phase-1 BCSF E2E gate (SPEC-007). Idempotent: re-running overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-storage --bin phase1_bench --release | tee "tests/e2e/phase1_bcsf/expected/phase1_result.txt"
grep -q "gate_mem_1.5x=PASS" "tests/e2e/phase1_bcsf/expected/phase1_result.txt"
grep -q "gate_lookup_sub15ns=PASS" "tests/e2e/phase1_bcsf/expected/phase1_result.txt"
echo "PHASE1 E2E: PASS"
