// constants.rs - on-disk constants for the durable segment format (B8).
//
// V1 is the legacy fixed nine-part layout. V2 is the current self-describing
// layout with type tags, per-part CRCs, and zone maps. Both values stay public
// because older files remain readable.

/// Legacy V1 magic, ASCII "SGSMNT01".
pub const SEGMENT_MAGIC_V1: u64 = 0x5347_534D_4E54_3031;
/// Current V2 magic, ASCII "SGSMNT02".
pub const SEGMENT_MAGIC_V2: u64 = 0x5347_534D_4E54_3032;
/// Legacy format version.
pub const SEGMENT_FMT_VER_V1: u32 = 1;
/// Current format version.
pub const SEGMENT_FMT_VER_V2: u32 = 2;

/// V2 header length: magic, version, cells, generation, fingerprint, part
/// count, flags, header CRC.
pub const SEGMENT_HEADER_LEN_V2: usize = 48;
/// V2 directory entry length: tag, offset, length, CRC, min, max, null count.
pub const PART_DIR_LEN_V2: usize = 48;

/// Legacy alias for the V1 magic.
pub const SEGMENT_MAGIC: u64 = SEGMENT_MAGIC_V1;
/// Legacy alias for the V1 version.
pub const SEGMENT_FMT_VER: u32 = SEGMENT_FMT_VER_V1;
/// Number of parts in a V1 file.
pub const PART_COUNT: usize = 9;
/// V1 header length.
pub const HEADER_LEN: usize = 8 + 4 + 8 + 8 + 8 + PART_COUNT * 16 + 4;

/// Part tag: cell offsets.
pub const PART_TAG_OFFSETS: u32 = 0;
/// Part tag: incidence indices.
pub const PART_TAG_INDICES: u32 = 1;
/// Part tag: stalk offsets.
pub const PART_TAG_STALK_OFFSETS: u32 = 2;
/// Part tag: morphism cells.
pub const PART_TAG_MORPH_CELLS: u32 = 3;
/// Part tag: morphism offsets.
pub const PART_TAG_MORPH_OFFSETS: u32 = 4;
/// Part tag: discrete stalks.
pub const PART_TAG_DISCRETE: u32 = 5;
/// Part tag: morphism weights.
pub const PART_TAG_MORPHS: u32 = 6;
/// Part tag: continuous stalks.
pub const PART_TAG_STALKS: u32 = 7;
/// Part tag: cell dimensions.
pub const PART_TAG_DIMS: u32 = 8;
/// Part tag: validity bitmap.
pub const PART_TAG_VALIDITY: u32 = 9;

/// V2 part count, including the optional validity bitmap.
pub const PART_COUNT_V2: usize = 10;

/// Segment format version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentFormat {
    V1,
    V2,
}

/// A part location in a V1 file.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Part {
    pub off: u64,
    pub len: u64,
}
