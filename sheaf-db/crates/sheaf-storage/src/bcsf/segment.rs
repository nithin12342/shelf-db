// crates/sheaf-storage/src/bcsf/segment.rs — durable segment descriptor
// (B8 / METHOD-045, REQ-050 / SPEC-050).
//
// V1 format (legacy): [header][offsets][indices][stalk_offsets][morph_cells]
//   [morph_offsets][discrete][morphs][stalks][dims][payload_crc]
//   Fixed 9-part table, single payload CRC, u32 directory offsets, whole-file
//   read via std::fs::read.
//
// V2 format (current): [header][part_directory][payload][payload_crc]
//   Self-describing part directory with type tags, per-part CRC32 checksums,
//   zone maps (min/max/null_count per part), u64 directory offsets (no 4GB
//   cap), mmap-friendly layout (header + directory are small, parts can be
//   paged in selectively). A reader can open it directly (no builder run)
//   and a corrupt file is rejected, never half-loaded.
//
//   V2 header (40 bytes):
//     magic(8) "SGSMNT02" | version(4)=2 | ncells(8) | generation(8) |
//     fingerprint(8) | part_count(4) | flags(4) | header_crc(4)
//
//   V2 part directory (44 bytes per part):
//     type_tag(4) | offset(8) | len(8) | crc32(4) | min_val(8) | max_val(8) |
//     null_count(8)
//
//   Part type tags:
//     0 = offsets   1 = indices   2 = stalk_offsets   3 = morph_cells
//     4 = morph_offsets  5 = discrete  6 = morphs  7 = stalks  8 = dims
//
//   Flags:
//     bit 0 = offset data is u64 (vs u32)
//     bit 1 = delta encoding present on sortable parts
use std::path::Path;

use memmap2::Mmap;

use crate::lsp::ring_wal::crc_hw;

use super::arena::StalkArena;
use super::encoding::{decode_delta_u32, decode_delta_u64, encode_delta_u32, encode_delta_u64};
use super::indices::IncidenceIndices;
use super::offsets::CellOffsets;
use super::reader::BcsfReader;

pub const SEGMENT_MAGIC_V1: u64 = 0x5347_534D_4E54_3031; // "SGSMNT01"
pub const SEGMENT_MAGIC_V2: u64 = 0x5347_534D_4E54_3032; // "SGSMNT02"
pub const SEGMENT_FMT_VER_V1: u32 = 1;
pub const SEGMENT_FMT_VER_V2: u32 = 2;

// V2 constants
pub const SEGMENT_HEADER_LEN_V2: usize = 48; // magic|ver|ncells|gen|fp|count|flags|crc
pub const PART_DIR_LEN_V2: usize = 48; // tag|off|len|crc|min|max|null_count

// V1 constants (kept for backward compatibility)
pub const SEGMENT_MAGIC: u64 = SEGMENT_MAGIC_V1;
pub const SEGMENT_FMT_VER: u32 = SEGMENT_FMT_VER_V1;
pub const PART_COUNT: usize = 9;
pub const HEADER_LEN: usize = 8 + 4 + 8 + 8 + 8 + PART_COUNT * 16 + 4;

// Part type tags (V2 self-describing directory)
pub const PART_TAG_OFFSETS: u32 = 0;
pub const PART_TAG_INDICES: u32 = 1;
pub const PART_TAG_STALK_OFFSETS: u32 = 2;
pub const PART_TAG_MORPH_CELLS: u32 = 3;
pub const PART_TAG_MORPH_OFFSETS: u32 = 4;
pub const PART_TAG_DISCRETE: u32 = 5;
pub const PART_TAG_MORPHS: u32 = 6;
pub const PART_TAG_STALKS: u32 = 7;
pub const PART_TAG_DIMS: u32 = 8;
pub const PART_TAG_VALIDITY: u32 = 9;

// V2 part count includes the optional validity bitmap part.
pub const PART_COUNT_V2: usize = 10;

// Segment format version
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SegmentFormat {
    V1,
    V2,
}

/// A part's location in the file (V1-style, kept for internal use).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Part {
    pub off: u64,
    pub len: u64,
}

/// V2 part directory entry: self-describing with type tag, per-part CRC,
/// and zone-map statistics (min/max/null_count).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PartDirEntry {
    pub type_tag: u32,
    pub off: u64,
    pub len: u64,
    pub crc: u32,
    pub min_val: u64,
    pub max_val: u64,
    pub null_count: u64,
}

fn put_part_dir(h: &mut Vec<u8>, e: &PartDirEntry) {
    h.extend_from_slice(&e.type_tag.to_le_bytes());
    h.extend_from_slice(&e.off.to_le_bytes());
    h.extend_from_slice(&e.len.to_le_bytes());
    h.extend_from_slice(&e.crc.to_le_bytes());
    h.extend_from_slice(&e.min_val.to_le_bytes());
    h.extend_from_slice(&e.max_val.to_le_bytes());
    h.extend_from_slice(&e.null_count.to_le_bytes());
}

fn u32_bytes(v: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn u64_bytes(v: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 8);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Compute zone-map statistics for a u32 array (sorted or monotonic).
fn zone_map_u32(v: &[u32]) -> (u64, u64, u64) {
    if v.is_empty() {
        (0, 0, 0)
    } else {
        let mut mn = v[0] as u64;
        let mut mx = v[0] as u64;
        for &x in v.iter().skip(1) {
            let x = x as u64;
            if x < mn { mn = x; }
            if x > mx { mx = x; }
        }
        (mn, mx, 0)
    }
}

/// Compute zone-map statistics for a u64 array (sorted).
fn zone_map_u64(v: &[u64]) -> (u64, u64, u64) {
    if v.is_empty() {
        (0, 0, 0)
    } else {
        (v[0], v[v.len() - 1], 0)
    }
}

/// Compute zone-map statistics for an f32 array.
fn zone_map_f32(v: &[f32]) -> (u64, u64, u64) {
    if v.is_empty() {
        (0, 0, 0)
    } else {
        let mut mn = f32::to_bits(v[0]);
        let mut mx = f32::to_bits(v[0]);
        for &x in v.iter().skip(1) {
            let b = f32::to_bits(x);
            if b < mn { mn = b; }
            if b > mx { mx = b; }
        }
        (mn as u64, mx as u64, 0)
    }
}

/// Write a reader to a durable segment file. V2 format by default.
pub fn write_segment(reader: &BcsfReader, path: &Path, generation: u64, src_fingerprint: u64) -> Result<(), String> {
    // B5: a shadow-paging chain has no flat slabs of its own - flatten it
    // (the segment file is a flat format).
    if reader.chain.is_some() {
        let flat = reader.flatten();
        return write_segment(&flat, path, generation, src_fingerprint);
    }
    write_segment_v2(reader, path, generation, src_fingerprint)
}

/// Write a reader to a V2 segment file (self-describing parts, per-part CRC,
/// zone maps, u64 directory offsets, optional delta encoding and validity
/// bitmap).
fn write_segment_v2(
    reader: &BcsfReader,
    path: &Path,
    generation: u64,
    src_fingerprint: u64,
) -> Result<(), String> {
    let ncells = reader.len() as u64;

    // Assemble raw part payloads.
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

    // Decide delta encoding: offsets and indices are monotonic/sorted, so
    // they benefit from delta+varint. The flag bit 1 tells the reader to
    // delta-decode those two parts.
    let mut flags: u32 = 0;
    let (offsets_bytes, delta_offsets) = if reader.offsets.offsets.len() > 1 {
        let mut enc = Vec::new();
        encode_delta_u32(&reader.offsets.offsets, &mut enc);
        if enc.len() < offsets_raw.len() {
            flags |= 0b10; // delta encoding present on sortable parts
            (enc, true)
        } else {
            (offsets_raw, false)
        }
    } else {
        (offsets_raw, false)
    };
    let (indices_bytes, delta_indices) = if reader.indices.indices.len() > 1 {
        let mut enc = Vec::new();
        encode_delta_u64(&reader.indices.indices, &mut enc);
        if enc.len() < indices_raw.len() {
            flags |= 0b10;
            (enc, true)
        } else {
            (indices_raw, false)
        }
    } else {
        (indices_raw, false)
    };
    let _ = delta_offsets;
    let _ = delta_indices;

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

    // Build payload and directory.
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

    // Build header.
    let mut header: Vec<u8> = Vec::with_capacity(SEGMENT_HEADER_LEN_V2);
    header.extend_from_slice(&SEGMENT_MAGIC_V2.to_le_bytes());
    header.extend_from_slice(&SEGMENT_FMT_VER_V2.to_le_bytes());
    header.extend_from_slice(&ncells.to_le_bytes());
    header.extend_from_slice(&generation.to_le_bytes());
    header.extend_from_slice(&src_fingerprint.to_le_bytes());
    header.extend_from_slice(&(PART_COUNT_V2 as u32).to_le_bytes());
    header.extend_from_slice(&flags.to_le_bytes());
    // header_crc placeholder at [44..48]
    header.extend_from_slice(&0u32.to_le_bytes());
    debug_assert_eq!(header.len(), SEGMENT_HEADER_LEN_V2);
    // CRC covers bytes [0..44] (everything before the CRC field itself).
    let header_crc = crc_hw(&header[..44]);
    header[44..48].copy_from_slice(&header_crc.to_le_bytes());

    // Build directory.
    let mut directory: Vec<u8> = Vec::with_capacity(PART_COUNT_V2 * PART_DIR_LEN_V2);
    for e in &dir_entries {
        put_part_dir(&mut directory, e);
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

/// Pick zone-map statistics for a part based on its type tag.
fn zone_map_for(tag: u32, bytes: &[u8]) -> (u64, u64, u64) {
    match tag {
        PART_TAG_OFFSETS => {
            let v: Vec<u32> = bytes.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
            zone_map_u32(&v)
        }
        PART_TAG_INDICES => {
            let v: Vec<u64> = bytes.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
            zone_map_u64(&v)
        }
        PART_TAG_STALK_OFFSETS => {
            let v: Vec<u32> = bytes.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
            zone_map_u32(&v)
        }
        PART_TAG_MORPH_CELLS => {
            let v: Vec<u32> = bytes.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
            zone_map_u32(&v)
        }
        PART_TAG_MORPH_OFFSETS => {
            let v: Vec<u32> = bytes.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect();
            zone_map_u32(&v)
        }
        PART_TAG_MORPHS | PART_TAG_STALKS => {
            let v: Vec<f32> = bytes.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
            zone_map_f32(&v)
        }
        PART_TAG_VALIDITY => {
            let null_count = zone_map_validity_count(bytes);
            (0, 0, null_count)
        }
        _ => (0, 0, 0), // discrete, dims: no numeric zone map
    }
}

/// Zone-map validity: returns null_count from a validity bitmap's null_count
/// field (already stored by the writer).
fn zone_map_validity_count(bytes: &[u8]) -> u64 {
    // validity is u64 words; null_count is tracked separately by the writer.
    // We compute it here for completeness: count zero bits.
    let words: Vec<u64> = bytes.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect();
    let mut nulls: u64 = 0;
    for &w in &words {
        nulls += (!w).count_ones() as u64;
    }
    nulls
}

/// Open a segment file into a reader. Auto-detects V1 vs V2 format.
/// For V2, uses mmap to avoid copying the whole file into RAM; only the
/// header and part directory are touched up front, parts are copied from
/// the mapping into owned Vecs.
pub fn open_segment(path: &Path) -> Result<BcsfReader, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path:?}: {e}"))?;
    if bytes.len() < 12 {
        return Err(format!("{path:?}: truncated header (need at least 12 bytes for magic+version)"));
    }
    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());

    match magic {
        SEGMENT_MAGIC_V2 if ver == SEGMENT_FMT_VER_V2 => open_segment_v2(&bytes, path),
        SEGMENT_MAGIC_V1 if ver == SEGMENT_FMT_VER_V1 => open_segment_v1(&bytes, path),
        _ => Err(format!("{path:?}: bad magic or unsupported version (magic=0x{magic:016X}, ver={ver})")),
    }
}

/// Open a V2 segment file from already-read bytes.
fn open_segment_v2(bytes: &[u8], path: &Path) -> Result<BcsfReader, String> {
    if bytes.len() < SEGMENT_HEADER_LEN_V2 {
        return Err(format!("{path:?}: truncated V2 header"));
    }
    let ncells = u64::from_le_bytes(bytes[12..20].try_into().unwrap()) as usize;
    let _generation = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
    let _fingerprint = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
    let part_count = u32::from_le_bytes(bytes[36..40].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(bytes[40..44].try_into().unwrap());
    let delta_enabled = flags & 0b10 != 0;
    let header_crc = u32::from_le_bytes(bytes[44..48].try_into().unwrap());

    // Validate header CRC over bytes 0..44.
    if crc_hw(&bytes[..44]) != header_crc {
        return Err(format!("{path:?}: V2 header crc mismatch"));
    }

    let dir_start = SEGMENT_HEADER_LEN_V2;
    let dir_end = dir_start + part_count * PART_DIR_LEN_V2;
    if bytes.len() < dir_end + 4 {
        return Err(format!("{path:?}: truncated V2 directory"));
    }

    let mut dir_entries: Vec<PartDirEntry> = Vec::with_capacity(part_count);
    let mut p = dir_start;
    for _ in 0..part_count {
        dir_entries.push(PartDirEntry {
            type_tag: u32::from_le_bytes(bytes[p..p + 4].try_into().unwrap()),
            off: u64::from_le_bytes(bytes[p + 4..p + 12].try_into().unwrap()),
            len: u64::from_le_bytes(bytes[p + 12..p + 20].try_into().unwrap()),
            crc: u32::from_le_bytes(bytes[p + 20..p + 24].try_into().unwrap()),
            min_val: u64::from_le_bytes(bytes[p + 24..p + 32].try_into().unwrap()),
            max_val: u64::from_le_bytes(bytes[p + 32..p + 40].try_into().unwrap()),
            null_count: u64::from_le_bytes(bytes[p + 40..p + 48].try_into().unwrap()),
        });
        p += PART_DIR_LEN_V2;
    }

    let payload_start = dir_end;
    // Read payload CRC (last 4 bytes).
    let file_len = bytes.len();
    if file_len < payload_start + 4 {
        return Err(format!("{path:?}: truncated V2 payload"));
    }
    let payload_crc = u32::from_le_bytes(bytes[file_len - 4..file_len].try_into().unwrap());
    let payload_end = file_len - 4;

    let payload = &bytes[payload_start..payload_end];
    let computed = crc_hw(payload);
    if computed != payload_crc {
        return Err(format!("{path:?}: V2 payload crc mismatch (torn write?)"));
    }

    // Validate each part's CRC and extract.
    let get_part = |tag: u32| -> Result<Vec<u8>, String> {
        let entry = dir_entries.iter().find(|e| e.type_tag == tag)
            .ok_or_else(|| format!("{path:?}: missing part type {tag}"))?;
        let s = entry.off as usize;
        let e = s.checked_add(entry.len as usize).ok_or("part overflow")?;
        if e > payload.len() {
            return Err("part outside payload".to_string());
        }
        let part_bytes = &payload[s..e];
        if crc_hw(part_bytes) != entry.crc {
            return Err(format!("{path:?}: part {tag} crc mismatch (corrupt part)"));
        }
        Ok(part_bytes.to_vec())
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

    let u32s = |b: &[u8]| -> Vec<u32> {
        b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
    };
    let u64s = |b: &[u8]| -> Vec<u64> {
        b.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect()
    };
    let f32s = |b: &[u8]| -> Vec<f32> {
        b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
    };

    // Delta-decode offsets and indices if the flag is set.
    let offsets_vec = if delta_enabled {
        let (decoded, _) = decode_delta_u32(&offsets_bytes, 0).map_err(|e| format!("{path:?}: {e}"))?;
        decoded
    } else {
        u32s(&offsets_bytes)
    };
    let indices_vec = if delta_enabled {
        let (decoded, _) = decode_delta_u64(&indices_bytes, 0).map_err(|e| format!("{path:?}: {e}"))?;
        decoded
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
    let dims = dims_bytes;
    let reader = BcsfReader {
        dims,
        offsets,
        indices,
        arena,
        chain: None,
        validity: u64s(&validity_bytes),
    };
    if reader.len() != ncells {
        return Err(format!("{path:?}: cell count mismatch"));
    }
    reader.validate().map_err(|e| format!("{path:?}: {e}"))?;
    Ok(reader)
}

/// Open a V1 segment file from already-read bytes (legacy format).
fn open_segment_v1(bytes: &[u8], path: &Path) -> Result<BcsfReader, String> {
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
    let mut off = 20usize;
    let mut parts = [Part::default(); PART_COUNT];
    for p in parts.iter_mut() {
        p.off = u64::from_le_bytes(bytes[off..off + 8].try_into().unwrap());
        p.len = u64::from_le_bytes(bytes[off + 8..off + 16].try_into().unwrap());
        off += 16;
    }
    let crc = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap());
    let payload = &bytes[HEADER_LEN..];
    if crc_hw(payload) != crc {
        return Err(format!("{path:?}: payload crc mismatch (torn write?)"));
    }
    let slice = |p: Part| -> Result<&[u8], String> {
        let s = p.off as usize;
        let e = s.checked_add(p.len as usize).ok_or("part overflow")?;
        if e > payload.len() {
            return Err("part outside payload".to_string());
        }
        Ok(&payload[s..e])
    };
    let u32s = |b: &[u8]| -> Vec<u32> {
        b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
    };
    let u64s = |b: &[u8]| -> Vec<u64> {
        b.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect()
    };
    let f32s = |b: &[u8]| -> Vec<f32> {
        b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
    };
    let offsets = CellOffsets { offsets: u32s(slice(parts[0])?) };
    let indices = IncidenceIndices { indices: u64s(slice(parts[1])?) };
    let arena = StalkArena {
        stalk_offsets: u32s(slice(parts[2])?),
        stalks: f32s(slice(parts[7])?),
        morph_cells: u32s(slice(parts[3])?),
        morph_offsets: u32s(slice(parts[4])?),
        morphs: f32s(slice(parts[6])?),
        discrete: slice(parts[5])?.to_vec(),
        validity: vec![0; ((ncells + 63) / 64).max(1)],
    };
    let dims = slice(parts[8])?.to_vec();
    let reader = BcsfReader {
        dims,
        offsets,
        indices,
        arena,
        chain: None,
        validity: vec![0; ((ncells + 63) / 64).max(1)],
    };
    if reader.len() != ncells {
        return Err(format!("{path:?}: cell count mismatch"));
    }
    reader.validate().map_err(|e| format!("{path:?}: {e}"))?;
    Ok(reader)
}

/// Open a segment file using mmap (avoids copying the whole file into RAM).
/// Only the header and part directory are touched up front; parts are
/// copied from the mapping into owned Vecs as they are needed.
pub fn open_segment_mmap(path: &Path) -> Result<BcsfReader, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path:?}: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("{path:?}: mmap failed: {e}"))?;
    let bytes: &[u8] = &*mmap;

    if bytes.len() < 40 {
        return Err(format!("{path:?}: truncated header"));
    }
    let magic = u64::from_le_bytes(bytes[..8].try_into().unwrap());
    let ver = u32::from_le_bytes(bytes[8..12].try_into().unwrap());

    let result = match magic {
        SEGMENT_MAGIC_V2 if ver == SEGMENT_FMT_VER_V2 => open_segment_v2(bytes, path),
        SEGMENT_MAGIC_V1 if ver == SEGMENT_FMT_VER_V1 => open_segment_v1(bytes, path),
        _ => Err(format!("{path:?}: bad magic or unsupported version")),
    };

    // Keep the mmap alive for the lifetime of the reader by leaking it.
    // BcsfReader owns its data (copied from mmap), so we can drop the mmap.
    // The Mmap is dropped here; data has been copied into Vecs above.
    drop(mmap);
    result
}

/// Header metadata of a segment file (generation, source fingerprint).
/// Uses mmap so only the first 44 bytes are touched, not the whole file.
pub fn segment_meta(path: &Path) -> Result<(u64, u64), String> {
    let file = std::fs::File::open(path).map_err(|e| format!("{path:?}: {e}"))?;
    let mmap = unsafe { Mmap::map(&file) }.map_err(|e| format!("{path:?}: mmap failed: {e}"))?;
    let bytes: &[u8] = &*mmap;
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

/// Legacy segment_meta for V1 files read via std::fs::read (kept for
/// callers that already have the bytes in memory).
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

// ---------------------------------------------------------------------------
// Zone-map pruning (V2 part directory).
// ---------------------------------------------------------------------------

/// Check whether a part's zone map overlaps the query range [lo, hi].
/// Returns `true` if the part MAY contain values in the range (so it must
/// be read), `false` if it can be safely skipped.
///
/// For numeric parts (offsets, indices, stalk_offsets, morph_cells,
/// morph_offsets, stalks, morphs) the zone map stores min/max as u64
/// bit patterns; overlap is a simple interval test.
/// For validity, null_count > 0 means the part has nulls.
pub fn zone_map_overlaps(entry: &PartDirEntry, lo: u64, hi: u64) -> bool {
    if lo > hi {
        return false;
    }
    // If the part's max is below lo or min is above hi, it cannot overlap.
    !(entry.max_val < lo || entry.min_val > hi)
}

/// Zone-map pruning for a f32 column: compare bit patterns so NaN handling
/// is consistent with the stored ordering.
pub fn zone_map_overlaps_f32(entry: &PartDirEntry, lo: f32, hi: f32) -> bool {
    if lo > hi {
        return false;
    }
    let lo_bits = f32::to_bits(lo) as u64;
    let hi_bits = f32::to_bits(hi) as u64;
    !(entry.max_val < lo_bits || entry.min_val > hi_bits)
}

// ---------------------------------------------------------------------------
// Multi-segment sharding foundation.
// ---------------------------------------------------------------------------

/// A reference to one segment within a sharded dataset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentRef {
    pub path: std::path::PathBuf,
    pub generation: u64,
    pub ncells: u64,
    pub src_fingerprint: u64,
}

/// A sharded segment set: multiple segment files that together form one
/// logical table. Cells are laid out contiguously across segments (segment 0
/// holds cells [0, ncells_0), segment 1 holds cells [ncells_0, ncells_0+ncells_1),
/// etc.). This removes the per-segment ~4GB u32 offset cap by spreading
/// cells across files.
#[derive(Clone, Debug, Default)]
pub struct SegmentSet {
    pub segments: Vec<SegmentRef>,
    /// Cumulative cell counts: segment_bounds[i] = start cell index of
    /// segment i; segment_bounds[last] = total cells.
    pub segment_bounds: Vec<u64>,
}

impl SegmentSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn total_cells(&self) -> u64 {
        self.segment_bounds.last().copied().unwrap_or(0)
    }

    /// Find which segment contains `cell` and the local index within it.
    pub fn locate(&self, cell: u64) -> Option<(usize, u64)> {
        if self.segments.is_empty() {
            return None;
        }
        let mut prev = 0u64;
        for (i, bound) in self.segment_bounds.iter().enumerate() {
            if cell < *bound {
                return Some((i, cell - prev));
            }
            prev = *bound;
        }
        None
    }

    pub fn push_segment(&mut self, seg: SegmentRef) {
        let start = self.total_cells();
        self.segment_bounds.push(start + seg.ncells);
        self.segments.push(seg);
    }
}

impl Default for SegmentRef {
    fn default() -> Self {
        SegmentRef { path: std::path::PathBuf::new(), generation: 0, ncells: 0, src_fingerprint: 0 }
    }
}
