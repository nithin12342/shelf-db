//! native - FFI adapter that forwards the storage port to the Zig library.
//!
//! Compiles only when build.rs linked libsheaf_storage (cfg
//! sheaf_native_storage). Every unsafe block is bounded: pointers come
//! from live Rust buffers validated first, and structs cross the boundary
//! through aligned locals so no unaligned reference is ever formed.

use super::{
    BCSF_HEADER_BYTES, COORD_DIMS, HASH_BYTES, SPINE_BYTES, SPINE_CELLS, SpineEntry,
    StorageBackend, StorageError,
};
use crate::storage::ffi;

/// FFI adapter. Stateless, so core-pinned workers share it freely.
pub struct NativeStorage;

// Copy the packed bytes into an aligned spine local. read_unaligned avoids
// forming a 64-byte-aligned reference from a 1-aligned byte array.
fn spine_local(packed: &[u8; SPINE_BYTES]) -> ffi::SheafSpineSoA {
    // SAFETY: SheafSpineSoA is 512 bytes of plain integers; any bit pattern
    // is valid, and read_unaligned imposes no alignment requirement.
    unsafe { std::ptr::read_unaligned(packed.as_ptr() as *const ffi::SheafSpineSoA) }
}

fn header_local(packed: &[u8; BCSF_HEADER_BYTES]) -> ffi::SheafBcsfBlockHeader {
    // SAFETY: the header is 64 bytes of plain scalars; any bits are valid.
    unsafe { std::ptr::read_unaligned(packed.as_ptr() as *const ffi::SheafBcsfBlockHeader) }
}

impl StorageBackend for NativeStorage {
    fn name(&self) -> &'static str {
        "zig-native"
    }

    fn spine_pack(&self, entries: &[SpineEntry]) -> Result<[u8; SPINE_BYTES], StorageError> {
        if entries.len() > SPINE_CELLS {
            return Err(StorageError::InvalidArgument);
        }
        let ids: Vec<u32> = entries.iter().map(|e| e.cell_id).collect();
        let offsets: Vec<u64> = entries.iter().map(|e| e.byte_offset).collect();
        let flags: Vec<u32> = entries.iter().map(|e| e.flags_mode).collect();
        let mut hashes = [0u8; SPINE_CELLS * HASH_BYTES];
        for (i, e) in entries.iter().enumerate() {
            hashes[i * HASH_BYTES..(i + 1) * HASH_BYTES].copy_from_slice(&e.stalk_hash);
        }
        // SAFETY: out is an aligned zeroed local; the input slices hold
        // `entries.len()` elements and hashes holds 16 * 16 bytes.
        let mut out: ffi::SheafSpineSoA = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            ffi::sheaf_spine_pack(
                &mut out,
                ids.as_ptr(),
                hashes.as_ptr(),
                offsets.as_ptr(),
                flags.as_ptr(),
                entries.len() as u32,
            )
        };
        if rc != 0 {
            return Err(StorageError::InvalidArgument);
        }
        // SAFETY: the struct is exactly SPINE_BYTES; transmute_copy reads
        // its bytes without an alignment requirement.
        Ok(unsafe { std::mem::transmute_copy::<ffi::SheafSpineSoA, [u8; SPINE_BYTES]>(&out) })
    }

    fn spine_count(&self, packed: &[u8; SPINE_BYTES]) -> usize {
        let spine = spine_local(packed);
        // SAFETY: spine is a live aligned local for this call only.
        unsafe { ffi::sheaf_spine_count(&spine) as usize }
    }

    fn spine_get(&self, packed: &[u8; SPINE_BYTES], slot: usize) -> Option<SpineEntry> {
        if slot >= SPINE_CELLS {
            return None;
        }
        let spine = spine_local(packed);
        let mut cell_id = 0u32;
        let mut hash = [0u8; HASH_BYTES];
        let mut offset = 0u64;
        let mut flags = 0u32;
        // SAFETY: every out-pointer references a live local; the C side
        // writes one slot or returns nonzero for an empty slot.
        let rc = unsafe {
            ffi::sheaf_spine_get(
                &spine,
                slot as u32,
                &mut cell_id,
                hash.as_mut_ptr(),
                &mut offset,
                &mut flags,
            )
        };
        if rc != 0 {
            return None;
        }
        Some(SpineEntry { cell_id, stalk_hash: hash, byte_offset: offset, flags_mode: flags })
    }

    fn bcsf_pack_header(
        &self,
        mins: &[f32],
        maxs: &[f32],
        dims: u32,
        cell_count: u32,
        payload_offset: u64,
        payload_bytes: u64,
    ) -> Result<[u8; BCSF_HEADER_BYTES], StorageError> {
        let d = dims as usize;
        if d == 0 || d > COORD_DIMS || mins.len() < d || maxs.len() < d {
            return Err(StorageError::InvalidArgument);
        }
        // SAFETY: out is an aligned zeroed local; mins/maxs hold >= dims
        // elements, validated above.
        let mut out: ffi::SheafBcsfBlockHeader = unsafe { std::mem::zeroed() };
        let rc = unsafe {
            ffi::sheaf_bcsf_pack_header(
                &mut out,
                mins.as_ptr(),
                maxs.as_ptr(),
                dims,
                cell_count,
                payload_offset,
                payload_bytes,
            )
        };
        if rc != 0 {
            return Err(StorageError::InvalidArgument);
        }
        // SAFETY: the header is exactly BCSF_HEADER_BYTES.
        Ok(unsafe {
            std::mem::transmute_copy::<ffi::SheafBcsfBlockHeader, [u8; BCSF_HEADER_BYTES]>(&out)
        })
    }

    fn bcsf_block_matches(
        &self,
        packed: &[u8; BCSF_HEADER_BYTES],
        query_min: &[f32],
        query_max: &[f32],
        dims: u32,
    ) -> bool {
        let d = dims as usize;
        if d == 0 || d > COORD_DIMS || query_min.len() < d || query_max.len() < d {
            return false;
        }
        let header = header_local(packed);
        // SAFETY: header is a live aligned local; query slices hold >= dims
        // elements, validated above.
        unsafe { ffi::sheaf_bcsf_block_matches(&header, query_min.as_ptr(), query_max.as_ptr(), dims) == 1 }
    }

    fn arena_alloc(
        &self,
        buffer: &mut [u8],
        used: &mut usize,
        size: usize,
        align: usize,
    ) -> Option<usize> {
        let mut arena = ffi::SheafArena {
            base: buffer.as_mut_ptr(),
            capacity: buffer.len(),
            used: *used,
        };
        // SAFETY: arena.base points at the live buffer for this call; the
        // C side only bumps within capacity and never frees the buffer.
        let ptr = unsafe { ffi::sheaf_arena_alloc(&mut arena, size, align) };
        *used = arena.used;
        if ptr.is_null() {
            return None;
        }
        Some(ptr as usize - buffer.as_ptr() as usize)
    }
}