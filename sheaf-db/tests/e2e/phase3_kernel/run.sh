# Phase-3 sheaf-kernel E2E gate (SPEC-010/011/012). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase3_bench --release | tee "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "gate_hand_delta3=PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "gate_converge_25iter=PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "gate_energy_1em4=PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "gate_solve_8ms=PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "gate_par_equiv=PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
grep -q "PHASE3 E2E: PASS" "tests/e2e/phase3_kernel/expected/phase3_result.txt"
echo "PHASE3 E2E: PASS"
