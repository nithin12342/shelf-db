// C-10 integration test for the compile-time specialization macros.
// Timestamp: 2026-11-22_12-52-18 +05:30
// Remote: origin https://github.com/nithin12342/shelf-db.git
// Branch: combined-stack-feature-improvement

use sheaf_kernel_macros::{rule_table, sheaf_unroll};

/// A slice reduction. The attribute must not change its runtime result.
#[sheaf_unroll(lanes = 4)]
fn sum_lanes(data: &[u64]) -> u64 {
    let mut total = 0u64;
    for &value in data {
        total += value;
    }
    total
}

/// The declared const is replaced by a static table from the emitted header.
#[rule_table]
const SHEAF_RULES: () = ();

// Compile-time assertions. Failure here breaks the build, not a test run.
const _: () = assert!(sum_lanes_sheaf_unroll::LANES == 4usize);
const _: () = assert!(sum_lanes_sheaf_unroll::APPLIED);
const _: () = assert!(sum_lanes_sheaf_unroll::lane_count() == 4usize);
const _: () = assert!(SHEAF_RULES.len() == 6usize);
const _: () = assert!(SHEAF_RULES[0].id == 1u16);
const _: () = assert!(SHEAF_RULES[5].id == 6u16);
const _: () = assert!(SHEAF_RULES[0].name.len() == 20usize);

#[test]
fn unroll_macro_keeps_runtime_result() {
    let data = [1u64, 2u64, 3u64, 4u64, 5u64, 6u64, 7u64];
    let expected: u64 = data.iter().copied().sum();
    assert_eq!(sum_lanes(&data), expected);
    assert_eq!(sum_lanes(&[]), 0u64);
}

#[test]
fn unroll_macro_emits_compile_time_metadata() {
    assert_eq!(sum_lanes_sheaf_unroll::LANES, 4usize);
    assert!(sum_lanes_sheaf_unroll::APPLIED);
    assert_eq!(sum_lanes_sheaf_unroll::lane_count(), 4usize);
}

#[test]
fn rule_table_matches_generated_header() {
    let expected_names = [
        "restriction_pushdown",
        "colimit_assoc",
        "restriction_comp",
        "filter_restrict_commute",
        "identity_restriction",
        "gluing_unique",
    ];
    let expected_theorems = [
        "restriction_pushdown_invariant",
        "colimit_assoc",
        "restriction_comp",
        "filter_restrict_commute",
        "identity_restriction_elim",
        "sheaf_gluing_unique",
    ];

    assert_eq!(SHEAF_RULES.len(), 6usize);
    let mut id_sum = 0u16;
    for (index, rule) in SHEAF_RULES.iter().enumerate() {
        assert_eq!(rule.id, (index + 1) as u16);
        assert_eq!(rule.name, expected_names[index]);
        assert_eq!(rule.theorem, expected_theorems[index]);
        id_sum += rule.id;
    }
    assert_eq!(id_sum, 21u16);
}
