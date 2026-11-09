# Phase-4 TQL front-end E2E gate (SPEC-017/018 + .section emit). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase4_bench --release | tee "tests/e2e/phase4_tql/expected/tql_result.txt"
grep -q "gate_parse_50us=PASS" "tests/e2e/phase4_tql/expected/tql_result.txt"
grep -q "gate_malformed_pinpoint=PASS" "tests/e2e/phase4_tql/expected/tql_result.txt"
grep -q "gate_tir_golden=PASS" "tests/e2e/phase4_tql/expected/tql_result.txt"
grep -q "gate_section_emit=PASS" "tests/e2e/phase4_tql/expected/tql_result.txt"
grep -q "PHASE4 E2E: PASS" "tests/e2e/phase4_tql/expected/tql_result.txt"
echo "PHASE4 E2E: PASS"
