// segment_golden.rs - byte-stability gate for the BCSF segment writer.
//
// The segment file is on the durability boundary. A refactor that changes one
// byte breaks every stored file. This test builds a deterministic complex,
// writes it with the V2 writer, and compares the whole-file FNV-1a hash with
// the captured value. The hash is a golden value, not a computed answer.

use sheaf_storage::bcsf::builder::BcsfBuilder;
use sheaf_storage::bcsf::segment::{write_segment, SEGMENT_FMT_VER_V2, SEGMENT_MAGIC_V2};
use std::path::PathBuf;

/// FNV-1a over the file bytes. Stable across Rust versions and platforms.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn fixture() -> BcsfBuilder {
    let mut builder = BcsfBuilder::new();
    builder.push_slice(0, &[], &[1; 16], &[1.0, 2.0], &[]);
    builder.push_slice(1, &[0], &[2; 16], &[3.0, 4.0], &[0.5]);
    builder.push_slice(1, &[0], &[3; 16], &[5.0, 6.0], &[0.5]);
    builder
}

fn write_fixture(name: &str) -> PathBuf {
    let reader = fixture().finish();
    let path = std::env::temp_dir().join(name);
    write_segment(&reader, &path, 7, 9).expect("segment write");
    path
}

#[test]
fn golden_segment_hash_is_stable() {
    let path = write_fixture("sheaf_segment_golden.bin");
    let bytes = std::fs::read(&path).expect("read segment");

    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(magic, SEGMENT_MAGIC_V2);
    assert_eq!(version, SEGMENT_FMT_VER_V2);

    let hash = fnv1a(&bytes);
    // Golden value captured from the pre-split writer on 2026-11-15.
    assert_eq!(hash, GOLDEN_HASH, "segment bytes changed: {hash:016x}");
    assert_eq!(bytes.len(), GOLDEN_LEN, "segment length changed");
}

/// The golden value is filled after the first recorded run.
const GOLDEN_HASH: u64 = 0xe999_8868_e3fb_320a;
const GOLDEN_LEN: usize = 690;
