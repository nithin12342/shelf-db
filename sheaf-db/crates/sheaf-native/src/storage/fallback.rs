//! fallback - pure-Rust storage adapter, always available.
//!
//! It writes the identical wire format as the Zig tier: the 512-byte
//! spine with cache-line-separated arrays, and the 64-byte BCSF header
//! with an inverted box on inactive dimensions. Byte encoding is
//! little-endian, matching the extern structs on the supported hosts.

use super::{
    BCSF_HEADER_BYTES, BCSF_MAGIC, COORD_DIMS, HASH_BYTES, SPINE_BYTES, SPINE_CELLS,
    SPINE_EMPTY, SpineEntry, StorageBackend, StorageError,
};

// Spine field offsets (bytes). Each starts on a cache-line boundary.
const OFF_IDS: usize = 0;
const OFF_HASHES: usize = 64;
const OFF_OFFSETS: usize = 320;
const OFF_FLAGS: usize = 448;

// BCSF header field offsets (bytes).
const H_MAGIC: usize = 0;
const H_COUNT: usize = 4;
const H_MIN: usize = 8;
const H_MAX: usize = 24;
const H_POFF: usize = 40;
const H_PBYTES: usize = 48;
const H_DIMS: usize = 56;

fn put_u32(buf: &mut [u8], at: usize, v: u32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn get_u32(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(buf[at..at + 4].try_into().unwrap())
}
fn put_u64(buf: &mut [u8], at: usize, v: u64) {
    buf[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn get_u64(buf: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(buf[at..at + 8].try_into().unwrap())
}
fn put_f32(buf: &mut [u8], at: usize, v: f32) {
    buf[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn get_f32(buf: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(buf[at..at + 4].try_into().unwrap())
}

/// Pure-Rust storage backend. Stateless and shareable across workers.
pub struct FallbackStorage;

impl StorageBackend for FallbackStorage {
    fn name(&self) -> &'static str {
        "rust-fallback"
    }

    fn spine_pack(&self, entries: &[SpineEntry]) -> Result<[u8; SPINE_BYTES], StorageError> {
        if entries.len() > SPINE_CELLS {
            return Err(StorageError::InvalidArgument);
        }
        let mut out = [0u8; SPINE_BYTES];
        for (i, e) in entries.iter().enumerate() {
            put_u32(&mut out, OFF_IDS + i * 4, e.cell_id);
            out[OFF_HASHES + i * HASH_BYTES..OFF_HASHES + (i + 1) * HASH_BYTES]
                .copy_from_slice(&e.stalk_hash);
            put_u64(&mut out, OFF_OFFSETS + i * 8, e.byte_offset);
            put_u32(&mut out, OFF_FLAGS + i * 4, e.flags_mode);
        }
        // Mark the tail empty. Hashes, offsets, and flags stay zero so no
        // stale bytes leak, matching the Zig pack exactly.
        for i in entries.len()..SPINE_CELLS {
            put_u32(&mut out, OFF_IDS + i * 4, SPINE_EMPTY);
        }
        Ok(out)
    }

    fn spine_count(&self, packed: &[u8; SPINE_BYTES]) -> usize {
        let mut n = 0;
        while n < SPINE_CELLS {
            if get_u32(packed, OFF_IDS + n * 4) == SPINE_EMPTY {
                break;
            }
            n += 1;
        }
        n
    }

    fn spine_get(&self, packed: &[u8; SPINE_BYTES], slot: usize) -> Option<SpineEntry> {
        if slot >= SPINE_CELLS {
            return None;
        }
        let cell_id = get_u32(packed, OFF_IDS + slot * 4);
        if cell_id == SPINE_EMPTY {
            return None;
        }
        let mut stalk_hash = [0u8; HASH_BYTES];
        stalk_hash.copy_from_slice(&packed[OFF_HASHES + slot * HASH_BYTES..][..HASH_BYTES]);
        Some(SpineEntry {
            cell_id,
            stalk_hash,
            byte_offset: get_u64(packed, OFF_OFFSETS + slot * 8),
            flags_mode: get_u32(packed, OFF_FLAGS + slot * 4),
        })
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
        let mut out = [0u8; BCSF_HEADER_BYTES];
        put_u32(&mut out, H_MAGIC, BCSF_MAGIC);
        put_u32(&mut out, H_COUNT, cell_count);
        for i in 0..COORD_DIMS {
            if i < d {
                if mins[i] > maxs[i] {
                    return Err(StorageError::InvalidArgument); // inverted input box
                }
                put_f32(&mut out, H_MIN + i * 4, mins[i]);
                put_f32(&mut out, H_MAX + i * 4, maxs[i]);
            } else {
                // Inactive dimensions get an inverted box so they never
                // reject a query by accident.
                put_f32(&mut out, H_MIN + i * 4, f32::NEG_INFINITY);
                put_f32(&mut out, H_MAX + i * 4, f32::INFINITY);
            }
        }
        put_u64(&mut out, H_POFF, payload_offset);
        put_u64(&mut out, H_PBYTES, payload_bytes);
        put_u32(&mut out, H_DIMS, dims);
        Ok(out)
    }

    fn bcsf_block_matches(
        &self,
        header: &[u8; BCSF_HEADER_BYTES],
        query_min: &[f32],
        query_max: &[f32],
        dims: u32,
    ) -> bool {
        if get_u32(header, H_MAGIC) != BCSF_MAGIC {
            return false;
        }
        let hd = get_u32(header, H_DIMS);
        if hd == 0 || hd as usize > COORD_DIMS || dims != hd {
            return false;
        }
        let d = dims as usize;
        if query_min.len() < d || query_max.len() < d {
            return false;
        }
        for i in 0..d {
            let bmin = get_f32(header, H_MIN + i * 4);
            let bmax = get_f32(header, H_MAX + i * 4);
            // Closed intervals: touching boundaries count as a match.
            if query_max[i] < bmin || query_min[i] > bmax {
                return false;
            }
        }
        true
    }

    fn arena_alloc(
        &self,
        buffer: &mut [u8],
        used: &mut usize,
        size: usize,
        align: usize,
    ) -> Option<usize> {
        if align == 0 || (align & (align - 1)) != 0 {
            return None;
        }
        if size == 0 {
            return Some(0); // zero-size reads get the base, no bump
        }
        // Align against the absolute buffer address, exactly like the Zig
        // arena, so both backends agree on any host alignment.
        let base = buffer.as_ptr() as usize;
        let current = base + *used;
        let aligned = (current + align - 1) & !(align - 1);
        let pad = aligned - base;
        let cap = buffer.len();
        if pad > cap || size > cap - pad {
            return None; // exhaustion leaves the bump pointer untouched
        }
        *used = pad + size;
        Some(pad)
    }
}