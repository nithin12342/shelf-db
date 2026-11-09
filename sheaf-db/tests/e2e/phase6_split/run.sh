# Phase-6 hypergraph split E2E gate (SPEC-016). Idempotent: overwrites expected output.
set -e
cd "$(dirname "$0")/../../.."
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase6_bench --release | tee "tests/e2e/phase6_split/expected/phase6_result.txt"
grep -q "gate_hypergraph_split=PASS" "tests/e2e/phase6_split/expected/phase6_result.txt"
grep -q "PHASE6 E2E: PASS" "tests/e2e/phase6_split/expected/phase6_result.txt"
cargo run --manifest-path Cargo.toml -p sheaf-engine --bin phase6_adv --release | tee "tests/e2e/phase6_split/expected/adv6_result.txt"
grep -q "ADVERSARIAL6: PASS" "tests/e2e/phase6_split/expected/adv6_result.txt"
echo "PHASE6 E2E: PASS"
