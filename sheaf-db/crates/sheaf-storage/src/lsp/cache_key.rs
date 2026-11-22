// Timestamp: 2026-11-22_11-53-08 +05:30
// cache_key.rs - exact 128-bit bundle cache key (NF-16, spec Pillar 5).
//
// The cache key is BLAKE3 over four fixed-width fields, truncated to 128 bits:
//
//   key = BLAKE3(epoch_id || row_count || schema_version || bundle_kind)[0..16]
//
// Every field is little-endian and fixed width. A change in one field cannot
// land on the bytes of another field, so two different bundles cannot share a
// key by construction of the input, only by a 128-bit hash collision.
//
// The key identifies a pre-folded fiber bundle. A write that advances the epoch
// changes the key. A schema change changes the key. A different bundle kind for
// the same data changes the key.

use std::fmt;

/// The length of a bundle cache key in bytes.
pub const KEY_BYTES: usize = 16;

/// A 128-bit content key for a pre-folded bundle.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BundleKey([u8; KEY_BYTES]);

impl BundleKey {
    /// Build the key from the four fields that define a bundle.
    pub fn of(
        epoch_id: u64,
        row_count: u64,
        schema_version: u32,
        bundle_kind: u32,
    ) -> BundleKey {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&epoch_id.to_le_bytes());
        hasher.update(&row_count.to_le_bytes());
        hasher.update(&schema_version.to_le_bytes());
        hasher.update(&bundle_kind.to_le_bytes());
        let digest = hasher.finalize();
        let mut key = [0u8; KEY_BYTES];
        key.copy_from_slice(&digest.as_bytes()[..KEY_BYTES]);
        BundleKey(key)
    }

    /// The key bytes.
    pub fn as_bytes(&self) -> &[u8; KEY_BYTES] {
        &self.0
    }

    /// The key as 32 lowercase hexadecimal characters.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(KEY_BYTES * 2);
        for byte in self.0.iter() {
            use std::fmt::Write;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }
}

impl fmt::Debug for BundleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BundleKey({})", self.to_hex())
    }
}

impl fmt::Display for BundleKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_inputs_give_the_same_key() {
        let a = BundleKey::of(7, 1_000, 3, 1);
        let b = BundleKey::of(7, 1_000, 3, 1);
        assert_eq!(a, b);
        assert_eq!(a.to_hex().len(), 32);
    }

    #[test]
    fn each_field_changes_the_key() {
        let base = BundleKey::of(7, 1_000, 3, 1);
        let epoch = BundleKey::of(8, 1_000, 3, 1);
        let rows = BundleKey::of(7, 1_001, 3, 1);
        let schema = BundleKey::of(7, 1_000, 4, 1);
        let kind = BundleKey::of(7, 1_000, 3, 2);
        for (name, key) in [("epoch", epoch), ("rows", rows), ("schema", schema), ("kind", kind)] {
            assert_ne!(base, key, "a change in {name} must change the key");
        }
        let all = [base, epoch, rows, schema, kind];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "keys {i} and {j} must differ");
            }
        }
    }

    #[test]
    fn the_key_is_the_truncated_blake3_of_the_fixed_width_fields() {
        let mut expected = blake3::Hasher::new();
        expected.update(&7u64.to_le_bytes());
        expected.update(&1_000u64.to_le_bytes());
        expected.update(&3u32.to_le_bytes());
        expected.update(&1u32.to_le_bytes());
        let digest = expected.finalize();
        let key = BundleKey::of(7, 1_000, 3, 1);
        assert_eq!(key.as_bytes(), &digest.as_bytes()[..KEY_BYTES]);
    }

    #[test]
    fn the_hex_form_is_stable() {
        let key = BundleKey::of(0, 0, 0, 0);
        assert_eq!(key.to_hex(), key.to_string());
        assert_eq!(format!("{key:?}"), format!("BundleKey({})", key.to_hex()));
    }
}
