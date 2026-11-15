// file.rs - open a segment from disk and read its header metadata.
//
// Magic and version select the reader. An unknown file is rejected with the
// raw magic and version in the error, so a caller can diagnose an old or
// foreign file without a trial read.

use std::path::Path;

use memmap2::Mmap;

use super::constants::{
    SEGMENT_FMT_VER_V1, SEGMENT_FMT_VER_V2, SEGMENT_MAGIC_V1, SEGMENT_MAGIC_V2,
};
use super::read_v1::open_segment_v1;
use super::read_v2::open_segment_v2;
use crate::bcsf::reader::BcsfReader;

/// Open a segment file. The format is detected from magic and version.
pub fn open_segment(path: &Path) -> Result<BcsfReader, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
    if bytes.len() < 12 {
        return Err(format!(
            "{path:?}: truncated header (need at least 12 bytes for magic+version)"
        ));
    }
    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    match magic {
        SEGMENT_MAGIC_V2 if version == SEGMENT_FMT_VER_V2 => open_segment_v2(&bytes, path),
        SEGMENT_MAGIC_V1 if version == SEGMENT_FMT_VER_V1 => open_segment_v1(&bytes, path),
        _ => Err(format!(
            "{path:?}: bad magic or unsupported version (magic=0x{magic:016X}, ver={version})"
        )),
    }
}

/// Open a segment through a memory map.
///
/// The map is dropped after the reader copies the parts it needs. The reader
/// owns its data, so a later file change cannot alter a live reader.
pub fn open_segment_mmap(path: &Path) -> Result<BcsfReader, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path:?}: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("{path:?}: mmap failed: {e}"))?;
    let bytes: &[u8] = &mmap;
    if bytes.len() < 40 {
        return Err(format!("{path:?}: truncated header"));
    }
    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let version = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    let result = match magic {
        SEGMENT_MAGIC_V2 if version == SEGMENT_FMT_VER_V2 => open_segment_v2(bytes, path),
        SEGMENT_MAGIC_V1 if version == SEGMENT_FMT_VER_V1 => open_segment_v1(bytes, path),
        _ => Err(format!("{path:?}: bad magic or unsupported version")),
    };
    drop(mmap);
    result
}

/// Read the generation and source fingerprint from the header. Only the first
/// 36 bytes are needed, so the map is small.
pub fn segment_meta(path: &Path) -> Result<(u64, u64), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path:?}: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("{path:?}: mmap failed: {e}"))?;
    let bytes: &[u8] = &mmap;
    if bytes.len() < 36 {
        return Err("truncated header".to_string());
    }
    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    if magic != SEGMENT_MAGIC_V2 && magic != SEGMENT_MAGIC_V1 {
        return Err("bad magic".to_string());
    }
    let generation = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    let fingerprint = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
    drop(mmap);
    Ok((generation, fingerprint))
}

/// Read metadata from bytes that are already in memory. This form accepts V1
/// files only, which matches the legacy callers.
pub fn segment_meta_from_bytes(bytes: &[u8]) -> Result<(u64, u64), String> {
    if bytes.len() < 36 {
        return Err("truncated header".to_string());
    }
    if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != SEGMENT_MAGIC_V1 {
        return Err("bad magic".to_string());
    }
    let generation = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    let fingerprint = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
    Ok((generation, fingerprint))
}
