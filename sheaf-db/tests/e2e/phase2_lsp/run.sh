# Phase-2 LSP E2E gate (SPEC-008/009, P99-gated). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase2_bench --release | tee "tests/e2e/phase2_lsp/expected/phase2_result.txt"
grep -q "gate_write_p99_200us=PASS" "tests/e2e/phase2_lsp/expected/phase2_result.txt"
grep -q "gate_compact_nospike_95pct=PASS" "tests/e2e/phase2_lsp/expected/phase2_result.txt"
grep -q "gate_commit_p99_2ms=PASS" "tests/e2e/phase2_lsp/expected/phase2_result.txt"
grep -q "PHASE2 E2E: PASS" "tests/e2e/phase2_lsp/expected/phase2_result.txt"
echo "PHASE2 E2E: PASS"
