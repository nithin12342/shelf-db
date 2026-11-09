# Phase-3 adversarial E2E gate (dense-oracle + edge cases). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase3_adv --release | tee "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b1_dense_agreement=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b2_symmetry=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b3_apply_energy_consistency=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b4_early_stop=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b5_active_edges=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b6_par_determinism=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b7_bj_inverse=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "gate_adv_b8_buffer_contracts=PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
grep -q "ADVERSARIAL3: PASS" "tests/e2e/phase3_adv/expected/adv3_result.txt"
echo "ADVERSARIAL3 E2E: PASS"
