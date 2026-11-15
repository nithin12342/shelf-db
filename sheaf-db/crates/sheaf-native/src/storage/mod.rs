//! storage - the port between Rust orchestration and the Zig physical tier.
//!
//! Dependency inversion mirrors the kernel tier:
//!
//! ```text
//! engine / query code
//!        |  depends on the trait only
//!        v
//! storage::StorageBackend  (port)
//!        |---------------------------|
//!        v                           v
//! storage::native::NativeStorage  storage::fallback::FallbackStorage
//! (Zig library via C ABI)         (pure Rust, always available)
//! ```
//!
//! Both adapters pack the identical 512-byte spine and 64-byte BCSF
//! header, so a block written by one reads back through the other.

pub mod fallback;
#[cfg(sheaf_native_storage)]
pub mod ffi;
#[cfg(sheaf_native_storage)]
pub mod native;

pub use fallback::FallbackStorage;
#[cfg(sheaf_native_storage)]
pub use native::NativeStorage;

/// Cells per spine block.
pub const SPINE_CELLS: usize = 16;
/// BLAKE3-128 digest width in bytes.
pub const HASH_BYTES: usize = 16;
/// Bounding-box dimensions in a block header.
pub const COORD_DIMS: usize = 4;
/// Packed spine size in bytes (eight cache lines).
pub const SPINE_BYTES: usize = 512;
/// Packed BCSF block header size in bytes.
pub const BCSF_HEADER_BYTES: usize = 64;
/// Sentinel marking an unused spine slot.
pub const SPINE_EMPTY: u32 = u32::MAX;
/// "BCSF" little-endian magic.
pub const BCSF_MAGIC: u32 = 0x4653_4342;

/// One populated spine slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpineEntry {
    pub cell_id: u32,
    pub stalk_hash: [u8; HASH_BYTES],
    pub byte_offset: u64,
    pub flags_mode: u32,
}

/// Errors the storage port can return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    /// A count, dimension, or pointer argument is out of contract.
    InvalidArgument,
    /// The bump arena cannot satisfy the requested size and alignment.
    ArenaExhausted,
}

/// The physical storage port. Implementations are stateless and shareable.
pub trait StorageBackend {
    fn name(&self) -> &'static str;

    /// Pack up to 16 entries into one 512-byte spine block.
    fn spine_pack(&self, entries: &[SpineEntry]) -> Result<[u8; SPINE_BYTES], StorageError>;

    /// Number of populated slots, stopping at the first sentinel.
    fn spine_count(&self, packed: &[u8; SPINE_BYTES]) -> usize;

    /// Read one slot. None when the slot is empty or out of range.
    fn spine_get(&self, packed: &[u8; SPINE_BYTES], slot: usize) -> Option<SpineEntry>;

    /// Pack a 64-byte BCSF block header with a coordinate bounding box.
    fn bcsf_pack_header(
        &self,
        mins: &[f32],
        maxs: &[f32],
        dims: u32,
        cell_count: u32,
        payload_offset: u64,
        payload_bytes: u64,
    ) -> Result<[u8; BCSF_HEADER_BYTES], StorageError>;

    /// Predicate pushdown: true when the query box intersects the block box
    /// on every active dimension. An invalid header never matches.
    fn bcsf_block_matches(
        &self,
        header: &[u8; BCSF_HEADER_BYTES],
        query_min: &[f32],
        query_max: &[f32],
        dims: u32,
    ) -> bool;

    /// Bump-allocate `size` bytes at `align` inside `buffer`, advancing the
    /// `used` cursor. Returns the byte offset of the region, or None when
    /// the buffer is exhausted or the alignment is not a power of two.
    fn arena_alloc(
        &self,
        buffer: &mut [u8],
        used: &mut usize,
        size: usize,
        align: usize,
    ) -> Option<usize>;
}

/// A safe bump arena over caller memory, dispatched through a backend.
/// The buffer stays pinned for the arena lifetime, which keeps the
/// affine-lease rule intact: no system allocator touches these bytes.
pub struct BumpArena<'a, B: StorageBackend + ?Sized> {
    backend: &'a B,
    buffer: &'a mut [u8],
    used: usize,
}

impl<'a, B: StorageBackend + ?Sized> BumpArena<'a, B> {
    pub fn new(backend: &'a B, buffer: &'a mut [u8]) -> Self {
        Self { backend, buffer, used: 0 }
    }

    /// Carve a region. The returned slice borrows the arena buffer.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<&mut [u8]> {
        let off = self.backend.arena_alloc(self.buffer, &mut self.used, size, align)?;
        Some(&mut self.buffer[off..off + size])
    }

    /// Reset the bump pointer. The buffer contents are untouched.
    pub fn reset(&mut self) {
        self.used = 0;
    }

    /// Bytes consumed so far, including alignment padding.
    pub fn used(&self) -> usize {
        self.used
    }
}

/// True when this build links the compiled Zig storage library.
pub const fn native_storage_linked() -> bool {
    cfg!(sheaf_native_storage)
}