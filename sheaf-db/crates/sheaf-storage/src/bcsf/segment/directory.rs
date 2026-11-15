// directory.rs - the V2 self-describing part directory.
//
// Each entry records the part tag, its offset and length in the payload, a
// CRC32C over the part, and the zone-map fields. A reader can therefore skip
// a part without reading it and reject a corrupt part in isolation.

/// One V2 directory entry.
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

/// Append one directory entry in the on-disk order.
pub(crate) fn put_part_dir(h: &mut Vec<u8>, e: &PartDirEntry) {
    h.extend_from_slice(&e.type_tag.to_le_bytes());
    h.extend_from_slice(&e.off.to_le_bytes());
    h.extend_from_slice(&e.len.to_le_bytes());
    h.extend_from_slice(&e.crc.to_le_bytes());
    h.extend_from_slice(&e.min_val.to_le_bytes());
    h.extend_from_slice(&e.max_val.to_le_bytes());
    h.extend_from_slice(&e.null_count.to_le_bytes());
}

/// Read one directory entry. The caller guarantees that 48 bytes are present.
pub(crate) fn read_part_dir(bytes: &[u8], at: usize) -> PartDirEntry {
    PartDirEntry {
        type_tag: u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()),
        off: u64::from_le_bytes(bytes[at + 4..at + 12].try_into().unwrap()),
        len: u64::from_le_bytes(bytes[at + 12..at + 20].try_into().unwrap()),
        crc: u32::from_le_bytes(bytes[at + 20..at + 24].try_into().unwrap()),
        min_val: u64::from_le_bytes(bytes[at + 24..at + 32].try_into().unwrap()),
        max_val: u64::from_le_bytes(bytes[at + 32..at + 40].try_into().unwrap()),
        null_count: u64::from_le_bytes(bytes[at + 40..at + 48].try_into().unwrap()),
    }
}
