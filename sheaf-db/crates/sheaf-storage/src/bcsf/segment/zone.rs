// zone.rs - zone maps for the V2 part directory and range pruning.
//
// A zone map stores the minimum and maximum value of a part as u64 bit
// patterns, plus a null count for the validity bitmap. A query uses the zone
// map to skip a part without reading it.

use super::constants::{
    PART_TAG_INDICES, PART_TAG_MORPH_CELLS, PART_TAG_MORPH_OFFSETS, PART_TAG_MORPHS,
    PART_TAG_OFFSETS, PART_TAG_STALK_OFFSETS, PART_TAG_STALKS, PART_TAG_VALIDITY,
};
use super::directory::PartDirEntry;

/// Zone map for a u32 array. It scans the array because the input order is not
/// guaranteed after delta decoding.
fn zone_map_u32(v: &[u32]) -> (u64, u64, u64) {
    if v.is_empty() {
        return (0, 0, 0);
    }
    let mut mn = v[0] as u64;
    let mut mx = mn;
    for &x in v.iter().skip(1) {
        let x = x as u64;
        if x < mn {
            mn = x;
        }
        if x > mx {
            mx = x;
        }
    }
    (mn, mx, 0)
}

/// Zone map for a sorted u64 array. The first and last values are the bounds.
fn zone_map_u64(v: &[u64]) -> (u64, u64, u64) {
    match (v.first(), v.last()) {
        (Some(&first), Some(&last)) => (first, last, 0),
        _ => (0, 0, 0),
    }
}

/// Zone map for an f32 array, compared by bit pattern.
fn zone_map_f32(v: &[f32]) -> (u64, u64, u64) {
    if v.is_empty() {
        return (0, 0, 0);
    }
    let mut mn = f32::to_bits(v[0]) as u64;
    let mut mx = mn;
    for &x in v.iter().skip(1) {
        let b = f32::to_bits(x) as u64;
        if b < mn {
            mn = b;
        }
        if b > mx {
            mx = b;
        }
    }
    (mn, mx, 0)
}

/// Count zero bits in the validity words.
fn zone_map_validity_count(bytes: &[u8]) -> u64 {
    let words = super::bytes::u64s(bytes);
    words.iter().map(|w| (!w).count_ones() as u64).sum()
}

/// Pick the zone-map statistics for a part from its type tag.
pub(crate) fn zone_map_for(tag: u32, bytes: &[u8]) -> (u64, u64, u64) {
    match tag {
        PART_TAG_OFFSETS | PART_TAG_STALK_OFFSETS | PART_TAG_MORPH_CELLS
        | PART_TAG_MORPH_OFFSETS => zone_map_u32(&super::bytes::u32s(bytes)),
        PART_TAG_INDICES => zone_map_u64(&super::bytes::u64s(bytes)),
        PART_TAG_MORPHS | PART_TAG_STALKS => zone_map_f32(&super::bytes::f32s(bytes)),
        PART_TAG_VALIDITY => (0, 0, zone_map_validity_count(bytes)),
        // Discrete stalks and dims carry no numeric range.
        _ => (0, 0, 0),
    }
}

/// True when a part may hold a value in [lo, hi]. False means the reader can
/// skip the part. The check is conservative: it never skips a possible match.
pub fn zone_map_overlaps(entry: &PartDirEntry, lo: u64, hi: u64) -> bool {
    if lo > hi {
        return false;
    }
    !(entry.max_val < lo || entry.min_val > hi)
}

/// The f32 form of the overlap check. It compares bit patterns, so the result
/// agrees with the stored ordering.
pub fn zone_map_overlaps_f32(entry: &PartDirEntry, lo: f32, hi: f32) -> bool {
    if lo > hi {
        return false;
    }
    let lo_bits = f32::to_bits(lo) as u64;
    let hi_bits = f32::to_bits(hi) as u64;
    !(entry.max_val < lo_bits || entry.min_val > hi_bits)
}
