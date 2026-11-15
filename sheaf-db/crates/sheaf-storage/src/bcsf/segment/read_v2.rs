// read_v2.rs - reader for the V2 segment format.
//
// The reader validates the header CRC, the whole-payload CRC, and every part
// CRC before it exposes any data. A corrupt file is rejected as a whole. It is
// never half-loaded.

use std::path::Path;

use crate::lsp::ring_wal::crc_hw;

use super::bytes::{f32s, u32s, u64s};
use super::constants::{
    PART_DIR_LEN_V2, PART_TAG_DIMS, PART_TAG_DISCRETE, PART_TAG_INDICES, PART_TAG_MORPH_CELLS,
    PART_TAG_MORPH_OFFSETS, PART_TAG_MORPHS, PART_TAG_OFFSETS, PART_TAG_STALK_OFFSETS,
    PART_TAG_STALKS, PART_TAG_VALIDITY, SEGMENT_HEADER_LEN_V2,
};
use super::directory::{read_part_dir, PartDirEntry};
use crate::bcsf::arena::StalkArena;
use crate::bcsf::encoding::{decode_delta_u32, decode_delta_u64};
use crate::bcsf::indices::IncidenceIndices;
use crate::bcsf::offsets::CellOffsets;
use crate::bcsf::reader::BcsfReader;

/// Open a V2 segment from bytes already in memory.
pub(crate) fn open_segment_v2(bytes: &[u8], path: &Path) -> Result<BcsfReader, String> {
    if bytes.len() < SEGMENT_HEADER_LEN_V2 {
        return Err(format!("{path:?}: truncated V2 header"));
    }
    let ncells = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;
    let part_count = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let delta_enabled = flags & 0b10 != 0;
    let header_crc = u32::from_le_bytes(bytes[44..48].try_into().unwrap());

    if crc_hw(&bytes[..44]) != header_crc {
        return Err(format!("{path:?}: V2 header crc mismatch"));
    }

    let dir_start = SEGMENT_HEADER_LEN_V2;
    let dir_end = dir_start + part_count * PART_DIR_LEN_V2;
    if bytes.len() < dir_end + 4 {
        return Err(format!("{path:?}: truncated V2 directory"));
    }
    let mut dir_entries: Vec<PartDirEntry> = Vec::with_capacity(part_count);
    for i in 0..part_count {
        dir_entries.push(read_part_dir(bytes, dir_start + i * PART_DIR_LEN_V2));
    }

    // The payload CRC is the last four bytes of the file.
    let file_len = bytes.len();
    let payload_crc = u32::from_le_bytes(bytes[file_len - 4..file_len].try_into().unwrap());
    let payload = &bytes[dir_end..file_len - 4];
    if crc_hw(payload) != payload_crc {
        return Err(format!("{path:?}: V2 payload crc mismatch (torn write?)"));
    }

    let get_part = |tag: u32| -> Result<Vec<u8>, String> {
        let entry = dir_entries
            .iter()
            .find(|e| e.type_tag == tag)
            .ok_or_else(|| format!("{path:?}: missing part type {tag}"))?;
        let start = entry.off as usize;
        let end = start
            .checked_add(entry.len as usize)
            .ok_or("part overflow")?;
        if end > payload.len() {
            return Err("part outside payload".to_string());
        }
        let part = &payload[start..end];
        if crc_hw(part) != entry.crc {
            return Err(format!("{path:?}: part {tag} crc mismatch (corrupt part)"));
        }
        Ok(part.to_vec())
    };

    let offsets_bytes = get_part(PART_TAG_OFFSETS)?;
    let indices_bytes = get_part(PART_TAG_INDICES)?;
    let stalk_offsets_bytes = get_part(PART_TAG_STALK_OFFSETS)?;
    let morph_cells_bytes = get_part(PART_TAG_MORPH_CELLS)?;
    let morph_offsets_bytes = get_part(PART_TAG_MORPH_OFFSETS)?;
    let discrete_bytes = get_part(PART_TAG_DISCRETE)?;
    let morphs_bytes = get_part(PART_TAG_MORPHS)?;
    let stalks_bytes = get_part(PART_TAG_STALKS)?;
    let dims_bytes = get_part(PART_TAG_DIMS)?;
    let validity_bytes = get_part(PART_TAG_VALIDITY)?;

    // Delta-decode offsets and indices when the flag is set.
    let offsets_vec = if delta_enabled {
        decode_delta_u32(&offsets_bytes, 0)
            .map(|(decoded, _)| decoded)
            .map_err(|e| format!("{path:?}: {e}"))?
    } else {
        u32s(&offsets_bytes)
    };
    let indices_vec = if delta_enabled {
        decode_delta_u64(&indices_bytes, 0)
            .map(|(decoded, _)| decoded)
            .map_err(|e| format!("{path:?}: {e}"))?
    } else {
        u64s(&indices_bytes)
    };

    let offsets = CellOffsets { offsets: offsets_vec };
    let indices = IncidenceIndices { indices: indices_vec };
    let arena = StalkArena {
        stalk_offsets: u32s(&stalk_offsets_bytes),
        stalks: f32s(&stalks_bytes),
        morph_cells: u32s(&morph_cells_bytes),
        morph_offsets: u32s(&morph_offsets_bytes),
        morphs: f32s(&morphs_bytes),
        discrete: discrete_bytes,
        validity: u64s(&validity_bytes),
    };
    let validity = u64s(&validity_bytes);
    let reader = BcsfReader {
        dims: dims_bytes,
        offsets,
        indices,
        arena,
        chain: None,
        validity,
    };
    if reader.len() != ncells {
        return Err(format!("{path:?}: cell count mismatch"));
    }
    reader.validate().map_err(|e| format!("{path:?}: {e}"))?;
    Ok(reader)
}
