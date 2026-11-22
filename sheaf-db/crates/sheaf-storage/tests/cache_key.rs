// cache_key.rs - integration test for the 128-bit bundle cache key (NF-16).
//
// The test calls the production constructor. It states the expected result for
// every case: equal inputs give equal keys, and a change in one field gives a
// different key.

use sheaf_storage::lsp::cache_key::BundleKey;

#[test]
fn the_production_key_matches_an_independent_blake3_computation() {
    let epoch = 42u64;
    let rows = 60_175u64;
    let schema = 7u32;
    let kind = 2u32;

    let key = BundleKey::of(epoch, rows, schema, kind);

    let mut hasher = blake3::Hasher::new();
    hasher.update(&epoch.to_le_bytes());
    hasher.update(&rows.to_le_bytes());
    hasher.update(&schema.to_le_bytes());
    hasher.update(&kind.to_le_bytes());
    let digest = hasher.finalize();

    assert_eq!(key.as_bytes(), &digest.as_bytes()[..16]);
    assert_eq!(key.to_hex().len(), 32);
}

#[test]
fn four_field_changes_give_four_distinct_keys() {
    let base = BundleKey::of(42, 60_175, 7, 2);
    let keys = [
        BundleKey::of(43, 60_175, 7, 2),
        BundleKey::of(42, 60_176, 7, 2),
        BundleKey::of(42, 60_175, 8, 2),
        BundleKey::of(42, 60_175, 7, 3),
    ];
    for key in keys {
        assert_ne!(base, key);
    }
}

#[test]
fn the_key_is_idempotent_across_calls() {
    let first = BundleKey::of(1, 2, 3, 4);
    let second = BundleKey::of(1, 2, 3, 4);
    assert_eq!(first, second);
    assert_eq!(first.as_bytes(), second.as_bytes());
}
