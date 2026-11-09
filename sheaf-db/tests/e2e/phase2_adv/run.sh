# Phase-2 adversarial E2E gate (out-of-box correctness suite, SPEC-008/009).
# Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin adversarial --release | tee "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a1_remap_identity=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a2_delete_merge_race=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a3_snapshot_pinned=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a3_final_equality=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a4_wal_bitflip=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a5a_txn_overlap_xor=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a5b_txn_disjoint=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a5c_txn_empty=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a5d_txn_abort_clean=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a6_shadow_fuzz=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a7_prune_churn_idempotent=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "adv_a8_degenerates=PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
grep -q "ADVERSARIAL: PASS" "tests/e2e/phase2_adv/expected/adv_result.txt"
echo "ADVERSARIAL E2E: PASS"
