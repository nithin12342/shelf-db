// read_v1.rs - reader for the legacy V1 segment format.
//
// V1 stores a fixed nine-part table with u32 directory offsets and one payload
// CRC. The reader stays because existing files must remain readable.

use std::path::Path;

use crate::lsp::ring_wal::crc_hw;

use super::bytes::{f32s, u32s, u64s};
use super::constants::{
    HEADER_LEN, PART_COUNT, SEGMENT_FMT_VER_V1, SEGMENT_MAGIC_V1,
};
use super::constants::Part;
use crate::bcsf::arena::StalkArena;
use crate::bcsf::indices::IncidenceIndices;
use crate::bcsf::offsets::CellOffsets;
use crate::bcsf::reader::BcsfReader;

/// Open a V1 segment from bytes already in memory.
pub(crate) fn open_segment_v1(bytes: &[u8], path: &Path) -> Result<BcsfReader, String> {
    if bytes.len() < HEADER_LEN {
        return Err(format!("{path:?}: truncated header"));
    }
    if u64::from_le_bytes(bytes[..8].try_into().unwrap()) != SEGMENT_MAGIC_V1 {
        return Err(format!("{path:?}: bad magic (not a segment)"));
    }
    let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    if ver != SEGMENT_FMT_VER_V1 {
        return Err(format!("{path:?}: format version {ver} != {SEGMENT_FMT_VER_V1}"));
    }
    let ncells = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;

    // The nine part descriptors follow the header prefix.
    let mut off = 20usize;
    let mut parts = [Part::default(); PART_COUNT];
    for part in parts.iter_mut() {
        part.off = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
        part.len = u64::from_le_bytes(bytes[off + 8..off + 16].try_into().unwrap());
        off += 16;
    }
    let crc = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let payload = &bytes[HEADER_LEN..];
    if crc_hw(payload) != crc {
        return Err(format!("{path:?}: payload crc mismatch (torn write?)"));
    }

    let slice = |part: Part| -> Result<&[u8], String> {
        let start = part.off as usize;
        let end = start
            .checked_add(part.len as usize)
            .ok_or("part overflow")?;
        if end > payload.len() {
            return Err("part outside payload".to_string());
        }
        Ok(&payload[start..end])
    };

    let offsets = CellOffsets { offsets: u32s(slice(parts[0])?) };
    let indices = IncidenceIndices { indices: u64s(slice(parts[1])?) };
    let placeholder_validity = vec![0; ncells.div_ceil(64).max(1)];
    let arena = StalkArena {
        stalk_offsets: u32s(slice(parts[2])?),
        stalks: f32s(slice(parts[7])?),
        morph_cells: u32s(slice(parts[3])?),
        morph_offsets: u32s(slice(parts[4])?),
        morphs: f32s(slice(parts[6])?),
        discrete: slice(parts[5])?.to_vec(),
        validity: placeholder_validity.clone(),
    };
    let dims = slice(parts[8])?.to_vec();
    let reader = BcsfReader {
        dims,
        offsets,
        indices,
        arena,
        chain: None,
        validity: placeholder_validity,
    };
    if reader.len() != ncells {
        return Err(format!("{path:?}: cell count mismatch"));
    }
    reader.validate().map_err(|e| format!("{path:?}: {e}"))?;
    Ok(reader)
}
