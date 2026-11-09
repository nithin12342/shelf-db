# Phase-5 optimizer + validator E2E gate (SPEC-013/015/019). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase5_bench --release | tee "tests/e2e/phase5_opt/expected/phase5_result.txt"
grep -q "gate_eulerian_validator=PASS" "tests/e2e/phase5_opt/expected/phase5_result.txt"
grep -q "gate_optimizer_explain=PASS" "tests/e2e/phase5_opt/expected/phase5_result.txt"
grep -q "gate_statistical_optimizer=PASS" "tests/e2e/phase5_opt/expected/phase5_result.txt"
grep -q "gate_name_lookup_parity=PASS" "tests/e2e/phase5_opt/expected/phase5_result.txt"
grep -q "PHASE5 E2E: PASS" "tests/e2e/phase5_opt/expected/phase5_result.txt"
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase5_adv --release | tee "tests/e2e/phase5_opt/expected/adv5_result.txt"
grep -q "ADVERSARIAL5: PASS" "tests/e2e/phase5_opt/expected/adv5_result.txt"
echo "PHASE5 E2E: PASS"
