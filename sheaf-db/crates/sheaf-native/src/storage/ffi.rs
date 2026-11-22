//! ffi - Rust mirror of native/include/sheaf_storage.h.
//!
//! Compiles only when build.rs linked the Zig library (cfg
//! sheaf_native_storage). Struct layout is part of the ABI: field order,
//! types, and the 64-byte spine alignment must match the C header.

/// Cells per spine block.
pub const SHEAF_SPINE_CELLS: usize = 16;
/// BLAKE3-128 digest width.
pub const SHEAF_HASH_BYTES: usize = 16;
/// Bounding-box dimensions in a block header.
pub const SHEAF_COORD_DIMS: usize = 4;
/// "BCSF" little-endian.
pub const SHEAF_BCSF_MAGIC: u32 = 0x4653_4342;
/// Sentinel for an unused spine slot.
pub const SHEAF_SPINE_EMPTY: u32 = 0xFFFF_FFFF;

/// Struct-of-arrays spine, exactly 512 bytes on a 64-byte boundary.
#[repr(C, align(64))]
pub struct SheafSpineSoA {
    pub cell_ids: [u32; SHEAF_SPINE_CELLS],
    pub stalk_hashes: [[u8; SHEAF_HASH_BYTES]; SHEAF_SPINE_CELLS],
    pub byte_offsets: [u64; SHEAF_SPINE_CELLS],
    pub flags_modes: [u32; SHEAF_SPINE_CELLS],
}

/// Bump arena over caller memory. No system allocator is involved.
#[repr(C)]
pub struct SheafArena {
    pub base: *mut u8,
    pub capacity: usize,
    pub used: usize,
}

/// 64-byte BCSF block header with a coordinate bounding box.
#[repr(C)]
pub struct SheafBcsfBlockHeader {
    pub magic: u32,
    pub cell_count: u32,
    pub coord_min: [f32; SHEAF_COORD_DIMS],
    pub coord_max: [f32; SHEAF_COORD_DIMS],
    pub payload_offset: u64,
    pub payload_bytes: u64,
    pub dims: u32,
    pub reserved: u32,
}

extern "C" {
    pub fn sheaf_arena_init(arena: *mut SheafArena, buffer: *mut u8, capacity: usize);
    pub fn sheaf_arena_alloc(arena: *mut SheafArena, size: usize, alignment: usize) -> *mut u8;
    pub fn sheaf_arena_reset(arena: *mut SheafArena);
    pub fn sheaf_arena_used(arena: *const SheafArena) -> usize;

    pub fn sheaf_spine_pack(
        out: *mut SheafSpineSoA,
        cell_ids: *const u32,
        hashes: *const u8,
        byte_offsets: *const u64,
        flags_modes: *const u32,
        count: u32,
    ) -> i32;
    pub fn sheaf_spine_count(spine: *const SheafSpineSoA) -> u32;
    pub fn sheaf_page_row_ranges(
        spine: *const SheafSpineSoA,
        payload: *const u8,
        payload_len: usize,
        offsets: *mut u64,
        lengths: *mut u32,
        capacity: u32,
    ) -> i32;
    pub fn sheaf_spine_get(
        spine: *const SheafSpineSoA,
        slot: u32,
        cell_id: *mut u32,
        hash_out: *mut u8,
        offset_out: *mut u64,
        flags_out: *mut u32,
    ) -> i32;

    pub fn sheaf_bcsf_pack_header(
        out: *mut SheafBcsfBlockHeader,
        mins: *const f32,
        maxs: *const f32,
        dims: u32,
        cell_count: u32,
        payload_offset: u64,
        payload_bytes: u64,
    ) -> i32;
    pub fn sheaf_bcsf_block_matches(
        header: *const SheafBcsfBlockHeader,
        query_min: *const f32,
        query_max: *const f32,
        dims: u32,
    ) -> i32;
}