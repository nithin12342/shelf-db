//! storage_parity - the Zig tier and the Rust fallback must agree.
//!
//! Every test runs against each linked backend. Cross-backend tests pack
//! with one adapter and read with the other, which proves the wire format
//! is identical, not merely self-consistent.

use sheaf_native::storage::{
    BumpArena, FallbackStorage, SpineEntry, StorageBackend, StorageError, BCSF_MAGIC,
    HASH_BYTES, SPINE_BYTES, SPINE_CELLS,
};

fn backends() -> Vec<Box<dyn StorageBackend>> {
    let mut v: Vec<Box<dyn StorageBackend>> = vec![Box::new(FallbackStorage)];
    #[cfg(sheaf_native_storage)]
    v.push(Box::new(sheaf_native::storage::NativeStorage));
    v
}

fn entry(i: usize) -> SpineEntry {
    let mut hash = [0u8; HASH_BYTES];
    for (j, b) in hash.iter_mut().enumerate() {
        *b = ((i * 7 + j) % 256) as u8;
    }
    SpineEntry {
        cell_id: 100 + i as u32,
        stalk_hash: hash,
        byte_offset: (i * 64) as u64,
        flags_mode: i as u32,
    }
}

#[test]
fn spine_roundtrips_every_slot() {
    let entries: Vec<SpineEntry> = (0..5).map(entry).collect();
    for be in backends() {
        let packed = be.spine_pack(&entries).unwrap();
        assert_eq!(be.spine_count(&packed), 5, "{} count", be.name());
        for (i, e) in entries.iter().enumerate() {
            assert_eq!(be.spine_get(&packed, i), Some(*e), "{} slot {i}", be.name());
        }
        assert_eq!(be.spine_get(&packed, 5), None, "{} tail", be.name());
        assert_eq!(be.spine_get(&packed, SPINE_CELLS), None, "{} range", be.name());
    }
}

#[test]
fn spine_capacity_and_empty_block() {
    for be in backends() {
        let empty = be.spine_pack(&[]).unwrap();
        assert_eq!(be.spine_count(&empty), 0, "{} empty", be.name());
        let full: Vec<SpineEntry> = (0..SPINE_CELLS).map(entry).collect();
        let packed = be.spine_pack(&full).unwrap();
        assert_eq!(be.spine_count(&packed), SPINE_CELLS, "{} full", be.name());
        let too_many: Vec<SpineEntry> = (0..SPINE_CELLS + 1).map(entry).collect();
        assert_eq!(be.spine_pack(&too_many), Err(StorageError::InvalidArgument), "{} over", be.name());
    }
}

#[test]
fn spine_wire_format_is_identical_across_backends() {
    let entries: Vec<SpineEntry> = (0..7).map(entry).collect();
    let reference = FallbackStorage.spine_pack(&entries).unwrap();
    for be in backends() {
        let packed = be.spine_pack(&entries).unwrap();
        assert_eq!(packed, reference, "{} bytes differ from fallback", be.name());
        // Read the fallback bytes through this backend.
        assert_eq!(be.spine_count(&reference), 7, "{} cross count", be.name());
        assert_eq!(be.spine_get(&reference, 3), Some(entries[3]), "{} cross get", be.name());
    }
}

#[test]
fn bcsf_pushdown_accepts_and_rejects() {
    let mins = [0.0f32, 10.0];
    let maxs = [5.0f32, 20.0];
    for be in backends() {
        let h = be.bcsf_pack_header(&mins, &maxs, 2, 16, 4096, 8192).unwrap();
        assert!(be.bcsf_block_matches(&h, &[4.0, 19.0], &[9.0, 30.0], 2), "{} hit", be.name());
        assert!(!be.bcsf_block_matches(&h, &[6.0, 10.0], &[9.0, 20.0], 2), "{} miss", be.name());
        // Touching boundaries count (closed intervals).
        assert!(be.bcsf_block_matches(&h, &[5.0, 20.0], &[8.0, 25.0], 2), "{} touch", be.name());
        // Dimension mismatch fails closed.
        assert!(!be.bcsf_block_matches(&h, &[4.0, 19.0], &[9.0, 30.0], 3), "{} dims", be.name());
        // Corrupt magic fails closed.
        let mut bad = h;
        bad[0] ^= 0xFF;
        assert!(!be.bcsf_block_matches(&bad, &[4.0, 19.0], &[9.0, 30.0], 2), "{} magic", be.name());
    }
}

#[test]
fn bcsf_rejects_inverted_and_out_of_range_dims() {
    for be in backends() {
        assert_eq!(
            be.bcsf_pack_header(&[5.0], &[0.0], 1, 1, 0, 0),
            Err(StorageError::InvalidArgument),
            "{} inverted", be.name()
        );
        assert_eq!(
            be.bcsf_pack_header(&[0.0], &[1.0], 0, 1, 0, 0),
            Err(StorageError::InvalidArgument),
            "{} zero dims", be.name()
        );
        assert_eq!(
            be.bcsf_pack_header(&[0.0], &[1.0], 5, 1, 0, 0),
            Err(StorageError::InvalidArgument),
            "{} big dims", be.name()
        );
    }
}

#[test]
fn bcsf_wire_format_is_identical_across_backends() {
    let mins = [1.0f32, 2.0, 3.0];
    let maxs = [4.0f32, 5.0, 6.0];
    let reference = FallbackStorage.bcsf_pack_header(&mins, &maxs, 3, 9, 128, 256).unwrap();
    // The magic lands little-endian at byte 0.
    assert_eq!(u32::from_le_bytes(reference[0..4].try_into().unwrap()), BCSF_MAGIC);
    for be in backends() {
        let h = be.bcsf_pack_header(&mins, &maxs, 3, 9, 128, 256).unwrap();
        assert_eq!(h, reference, "{} header bytes differ", be.name());
    }
}

#[repr(C, align(64))]
struct AlignedBuf([u8; 256]);

#[test]
fn arena_honors_alignment_capacity_and_reset() {
    for be in backends() {
        let mut buf = AlignedBuf([0u8; 256]);
        let mut arena = BumpArena::new(be.as_ref(), &mut buf.0);
        let a = arena.alloc(10, 1).unwrap();
        a[0] = 1;
        assert_eq!(arena.used(), 10, "{} used after a", be.name());
        // The next 64-byte request skips to the next cache line.
        let b = arena.alloc(64, 64).unwrap();
        assert_eq!(b.as_ptr() as usize % 64, 0, "{} alignment", be.name());
        assert_eq!(arena.used(), 128, "{} used after b", be.name());
        // Exhaustion leaves the bump pointer untouched.
        let before = arena.used();
        assert!(arena.alloc(4096, 8).is_none(), "{} exhaust", be.name());
        assert_eq!(arena.used(), before, "{} used stable", be.name());
        // Bad alignment is rejected.
        assert!(arena.alloc(8, 3).is_none(), "{} bad align", be.name());
        arena.reset();
        assert_eq!(arena.used(), 0, "{} reset", be.name());
    }
}

#[test]
fn packed_spine_is_exactly_512_bytes() {
    for be in backends() {
        let packed = be.spine_pack(&[entry(0)]).unwrap();
        assert_eq!(packed.len(), SPINE_BYTES, "{} size", be.name());
    }
}