// write.rs - V2 segment writer.
//
// The writer assembles one payload buffer from the reader's parts, computes a
// CRC32C per part and one over the whole payload, then writes header,
// directory, payload, and payload CRC. The layout is byte-stable: the golden
// test in tests/segment_golden.rs guards it.

use std::path::Path;

use crate::lsp::ring_wal::crc_hw;

use super::bytes::{f32_bytes, u32_bytes, u64_bytes};
use super::constants::{
    PART_COUNT_V2, PART_DIR_LEN_V2, PART_TAG_DIMS, PART_TAG_DISCRETE, PART_TAG_INDICES,
    PART_TAG_MORPH_CELLS, PART_TAG_MORPH_OFFSETS, PART_TAG_MORPHS, PART_TAG_OFFSETS,
    PART_TAG_STALK_OFFSETS, PART_TAG_STALKS, PART_TAG_VALIDITY, SEGMENT_FMT_VER_V2,
    SEGMENT_HEADER_LEN_V2, SEGMENT_MAGIC_V2,
};
use super::directory::{put_part_dir, PartDirEntry};
use super::zone::zone_map_for;
use crate::bcsf::encoding::{
    encode_delta_u32, encode_delta_u64,
};
use crate::bcsf::reader::BcsfReader;

/// Write a reader to a durable segment file. The V2 format is the default.
pub fn write_segment(
    reader: &BcsfReader,
    path: &Path,
    generation: u64,
    src_fingerprint: u64,
) -> Result<(), String> {
    // A shadow-paging chain has no flat slabs of its own. Flatten it first,
    // because the segment file is a flat format.
    if reader.chain.is_some() {
        let flat = reader.flatten();
        return write_segment(&flat, path, generation, src_fingerprint);
    }
    write_segment_v2(reader, path, generation, src_fingerprint)
}

/// Write one V2 segment file.
fn write_segment_v2(
    reader: &BcsfReader,
    path: &Path,
    generation: u64,
    src_fingerprint: u64,
) -> Result<(), String> {
    let ncells = reader.len() as u64;

    // Assemble the raw part payloads.
    let offsets_raw = u32_bytes(&reader.offsets.offsets);
    let indices_raw = u64_bytes(&reader.indices.indices);
    let stalk_offsets_raw = u32_bytes(&reader.arena.stalk_offsets);
    let morph_cells_raw = u32_bytes(&reader.arena.morph_cells);
    let morph_offsets_raw = u32_bytes(&reader.arena.morph_offsets);
    let discrete_raw = reader.arena.discrete.clone();
    let morphs_raw = f32_bytes(&reader.arena.morphs);
    let stalks_raw = f32_bytes(&reader.arena.stalks);
    let dims_raw = reader.dims.clone();
    let validity_raw = u64_bytes(&reader.validity);

    // Offsets and indices are monotonic, so delta and varint encoding can
    // shrink them. Flag bit 1 tells the reader to decode both parts.
    let mut flags: u32 = 0;
    let offsets_bytes = if reader.offsets.offsets.len() > 1 {
        let mut enc = Vec::new();
        encode_delta_u32(&reader.offsets.offsets, &mut enc);
        if enc.len() < offsets_raw.len() {
            flags |= 0b10;
            enc
        } else {
            offsets_raw
        }
    } else {
        offsets_raw
    };
    let indices_bytes = if reader.indices.indices.len() > 1 {
        let mut enc = Vec::new();
        encode_delta_u64(&reader.indices.indices, &mut enc);
        if enc.len() < indices_raw.len() {
            flags |= 0b10;
            enc
        } else {
            indices_raw
        }
    } else {
        indices_raw
    };

    let parts: [(u32, &[u8]); PART_COUNT_V2] = [
        (PART_TAG_OFFSETS, &offsets_bytes),
        (PART_TAG_INDICES, &indices_bytes),
        (PART_TAG_STALK_OFFSETS, &stalk_offsets_raw),
        (PART_TAG_MORPH_CELLS, &morph_cells_raw),
        (PART_TAG_MORPH_OFFSETS, &morph_offsets_raw),
        (PART_TAG_DISCRETE, &discrete_raw),
        (PART_TAG_MORPHS, &morphs_raw),
        (PART_TAG_STALKS, &stalks_raw),
        (PART_TAG_DIMS, &dims_raw),
        (PART_TAG_VALIDITY, &validity_raw),
    ];

    // Build the payload and the directory in one pass.
    let mut payload: Vec<u8> = Vec::new();
    let mut dir_entries: Vec<PartDirEntry> = Vec::with_capacity(PART_COUNT_V2);
    let mut running_off: u64 = 0;
    for (tag, bytes) in &parts {
        let crc = crc_hw(bytes);
        let (min_val, max_val, null_count) = zone_map_for(*tag, bytes);
        dir_entries.push(PartDirEntry {
            type_tag: *tag,
            off: running_off,
            len: bytes.len() as u64,
            crc,
            min_val,
            max_val,
            null_count,
        });
        payload.extend_from_slice(bytes);
        running_off += bytes.len() as u64;
    }
    let payload_crc = crc_hw(&payload);

    // Header. The CRC covers the first 44 bytes, not the CRC field.
    let mut header: Vec<u8> = Vec::with_capacity(SEGMENT_HEADER_LEN_V2);
    header.extend_from_slice(&SEGMENT_MAGIC_V2.to_le_bytes());
    header.extend_from_slice(&SEGMENT_FMT_VER_V2.to_le_bytes());
    header.extend_from_slice(&ncells.to_le_bytes());
    header.extend_from_slice(&generation.to_le_bytes());
    header.extend_from_slice(&src_fingerprint.to_le_bytes());
    header.extend_from_slice(&(PART_COUNT_V2 as u32).to_le_bytes());
    header.extend_from_slice(&flags.to_le_bytes());
    header.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(header.len(), SEGMENT_HEADER_LEN_V2);
    let header_crc = crc_hw(&header[..44]);
    header[44..48].copy_from_slice(&header_crc.to_le_bytes());

    // Directory.
    let mut directory: Vec<u8> = Vec::with_capacity(PART_COUNT_V2 * PART_DIR_LEN_V2);
    for entry in &dir_entries {
        put_part_dir(&mut directory, entry);
    }
    debug_assert_eq!(directory.len(), PART_COUNT_V2 * PART_DIR_LEN_V2);

    let mut file = std::fs::File::create(path).map_err(|e| format!("{path:?}: {e}"))?;
    use std::io::Write;
    file.write_all(&header).map_err(|e| format!("{path:?}: {e}"))?;
    file.write_all(&directory).map_err(|e| format!("{path:?}: {e}"))?;
    file.write_all(&payload).map_err(|e| format!("{path:?}: {e}"))?;
    file.write_all(&payload_crc.to_le_bytes()).map_err(|e| format!("{path:?}: {e}"))?;
    Ok(())
}
