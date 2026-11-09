# Von Neumann physical-limits E2E gate (SPEC-057 / METHOD-052).
# Idempotent: re-running overwrites the expected output.
# Measures this host's real memory-hierarchy and storage limits, derives the
# physical floors from published constants, and asserts the physics-invariant
# ordering gates. The report (VON_NEUMANN_LIMITS.md) quotes these numbers.
set -e
cd "$(dirname "$0")/../../.."
mkdir -p tests/e2e/von_neumann/expected

cargo run --manifest-path Cargo.toml -p sheaf-engine --bin physical_limits_probe --release \
  | tee "tests/e2e/von_neumann/expected/physical_limits.txt"

# The probe must complete.
grep -q "PHYSICAL-LIMITS-PROBE: COMPLETE" "tests/e2e/von_neumann/expected/physical_limits.txt"
# Physics-invariant gates (ordering / identity, not performance targets).
grep -q "gate_hierarchy_ordered=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_dram_slower_than_l3=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_dram_above_ddr4_cas_floor=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_fsync_floor_exists=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_read_bw_significant=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_dram_slower_than_l1=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"

# Clock cross-checks: the 8-add vs 4-add chain ratio must be 2.0, and the
# 4x-imul vs 4-add ratio must be 3.0. The second validates the ABSOLUTE
# cycle count, which is what every "FLOP/cycle" claim rests on.
grep -q "C_clock_crosscheck_ok=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"

# The derived floors must be present (the report cites them by name).
grep -q "E_landauer_J_per_bit=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "E_dram_floor_ns " "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "E_machine_balance_1t_flop_per_byte=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "E_dram_only_estimate_ns=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "E_hdd_grain_nm=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "E_stlb_reach_MiB_at_1536_entries=" "tests/e2e/von_neumann/expected/physical_limits.txt"

# The same-window roofline pair (the machine balance must be measured in
# the same thermal window as the bandwidth it is divided by).
grep -q "G_same_window_machine_balance_flop_per_byte=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "G_kernel_intensity_flop_per_byte d=4" "tests/e2e/von_neumann/expected/physical_limits.txt"

# Section H: the real database kernel against the memory ceiling, measured
# in the same window. A kernel cannot beat the ceiling it reads from, and
# the two kernels must agree on the aggregate.
grep -q "gate_kernel_below_ceiling=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_kernels_agree=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "H_utilization_of_ceiling" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "H_accumulator_results_agree=true" "tests/e2e/von_neumann/expected/physical_limits.txt"

# Section I: the fundamental and storage ceilings. The gates assert the
# physics is self-consistent, and the derived numbers must be present.
grep -q "gate_amdahl_serial_fraction_below_one=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_amdahl_ceiling_above_measured=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_refresh_overhead_under_one_pct=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_soft_error_bit_outlives_module=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_amdahl_max_speedup_ceiling=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_refresh_overhead_pct=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_soft_error_dim_mtbf_years=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_op_floor_margolus_levitin_W=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_bekenstein_max_bits=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_address_canonical_max_bytes=" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "C_nand_years_to_exhaust_rating=" "tests/e2e/von_neumann/expected/physical_limits.txt"

# Section J: the amplification experiment. The read and write costs must
# both be at or above the physical floor of 1.0, the fiber must read far
# fewer bytes than any base read, and the shadow merge must cost less
# storage than the copy merge.
grep -q "gate_write_amp_at_least_one=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_read_amp_at_least_one=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_fiber_reads_fewer_bytes_than_base=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_fiber_read_amp_far_below_base=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "gate_shadow_merge_cheaper_than_copy=PASS" "tests/e2e/von_neumann/expected/physical_limits.txt"
grep -q "A_amp_theoretical_floor" "tests/e2e/von_neumann/expected/physical_limits.txt"

echo "VON NEUMANN LIMITS E2E: PASS"
