# Phase-7 distributed E2E gate (SPEC-025..029). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase7_bench --release | tee "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "gate_kahypar_cover=PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "gate_ras_converge=PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "gate_concurrent_edits=PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "gate_nerve_zero_hop=PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "gate_loopback_read=PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
grep -q "PHASE7 E2E: PASS" "tests/e2e/phase7_dist/expected/phase7_result.txt"
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase7_adv --release | tee "tests/e2e/phase7_dist/expected/adv7_result.txt"
grep -q "ADVERSARIAL7: PASS" "tests/e2e/phase7_dist/expected/adv7_result.txt"
echo "PHASE7 E2E: PASS"
