// mod.rs - public surface of the durable segment format.
//
// The implementation is split by responsibility so each file stays small:
//   constants  on-disk values and small types
//   bytes      little-endian conversion
//   directory  V2 part directory
//   zone       zone maps and pruning
//   write      V2 writer
//   read_v1    legacy reader
//   read_v2    current reader
//   file       open and metadata entry points
//   sharding   multi-segment cell mapping
//
// The public names below are unchanged from the original single-file module,
// so callers need no edit.

pub mod constants;

mod bytes;
mod directory;
mod file;
mod read_v1;
mod read_v2;
mod sharding;
mod write;
mod zone;

pub use constants::*;
pub use directory::PartDirEntry;
pub use file::{open_segment, open_segment_mmap, segment_meta, segment_meta_from_bytes};
pub use sharding::{SegmentRef, SegmentSet};
pub use write::write_segment;
pub use zone::{zone_map_overlaps, zone_map_overlaps_f32};
