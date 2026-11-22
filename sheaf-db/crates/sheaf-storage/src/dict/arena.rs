// dict/arena.rs - the append-only byte arena for dictionary values (MF-06).
//
// One arena holds every text value of one table. A value that is interned twice
// keeps one entry, so the arena length is the number of distinct values, not
// the number of rows.
//
// Each entry starts on a 64-byte boundary and holds a length prefix and the
// bytes:
//
//   offset ->  [ length u32 | value bytes | zero padding to the next boundary ]
//
// The padding is written, not assumed, so the whole arena can be saved and
// loaded as one byte range.
//
// A read checks four things before it returns bytes: the offset is inside the
// arena, the offset is aligned, the length prefix is inside the arena, and the
// value end is inside the arena. A failed check returns InvalidOffset. The
// reader never returns a truncated value.

use super::DictError;

/// The alignment of every entry start.
pub const ALIGNMENT: usize = 64;

/// The bytes of the length prefix.
pub const LENGTH_PREFIX: usize = 4;

/// An append-only byte arena with one stable offset per value.
#[derive(Clone, Debug, Default)]
pub struct ByteArena {
    bytes: Vec<u8>,
    entries: usize,
}

impl ByteArena {
    /// An empty arena.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty arena with room for `capacity` bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { bytes: Vec::with_capacity(capacity), entries: 0 }
    }

    /// The arena bytes, including the length prefixes and the padding.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The number of stored values.
    pub fn entries(&self) -> usize {
        self.entries
    }

    /// True when the arena holds no value.
    pub fn is_empty(&self) -> bool {
        self.entries == 0
    }

    /// The arena length in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// The offset that a new value with this length would get. The caller uses
    /// it for a size check before it appends.
    pub fn next_offset(&self) -> u64 {
        let aligned = align_up(self.bytes.len());
        aligned as u64
    }

    /// Append one value and return its offset. The offset is aligned to 64
    /// bytes and fits in a u32, which is the dictionary column width.
    pub fn append(&mut self, value: &[u8]) -> Result<u32, DictError> {
        let offset = align_up(self.bytes.len());
        let end = offset
            .checked_add(LENGTH_PREFIX)
            .and_then(|v| v.checked_add(value.len()))
            .ok_or(DictError::ArenaFull)?;
        if end > u32::MAX as usize {
            return Err(DictError::ArenaFull);
        }
        // Fill the alignment gap with zeros, so the bytes round trip.
        self.bytes.resize(offset, 0);
        self.bytes.extend_from_slice(&(value.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(value);
        // Pad to the next boundary, so the next entry starts aligned.
        let padded = align_up(self.bytes.len());
        self.bytes.resize(padded, 0);
        self.entries += 1;
        Ok(offset as u32)
    }

    /// Read one value by offset. A bad offset, a bad prefix, or a value that
    /// runs past the arena returns InvalidOffset.
    pub fn get(&self, offset: u32) -> Result<&[u8], DictError> {
        let start = offset as usize;
        if !start.is_multiple_of(ALIGNMENT) {
            return Err(DictError::InvalidOffset(offset));
        }
        let prefix_end = start.checked_add(LENGTH_PREFIX).ok_or(DictError::InvalidOffset(offset))?;
        if prefix_end > self.bytes.len() {
            return Err(DictError::InvalidOffset(offset));
        }
        let length = u32::from_le_bytes(self.bytes[start..prefix_end].try_into().unwrap()) as usize;
        let value_end = prefix_end
            .checked_add(length)
            .ok_or(DictError::InvalidOffset(offset))?;
        if value_end > self.bytes.len() {
            return Err(DictError::InvalidOffset(offset));
        }
        Ok(&self.bytes[prefix_end..value_end])
    }

    /// The length of the value at `offset`, without reading its bytes.
    pub fn value_len(&self, offset: u32) -> Result<usize, DictError> {
        Ok(self.get(offset)?.len())
    }
}

/// Round `value` up to the next 64-byte boundary.
pub(crate) fn align_up(value: usize) -> usize {
    value.div_ceil(ALIGNMENT) * ALIGNMENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_entry_starts_at_offset_zero() {
        let mut arena = ByteArena::new();
        assert_eq!(arena.append(b"alpha").expect("append"), 0);
        assert_eq!(arena.entries(), 1);
        assert_eq!(arena.get(0).expect("read"), b"alpha");
    }

    #[test]
    fn every_entry_starts_on_a_boundary() {
        let mut arena = ByteArena::new();
        let mut offsets = Vec::new();
        for value in [b"a".as_slice(), b"bb".as_slice(), &[0u8; 200], b"".as_slice()] {
            offsets.push(arena.append(value).expect("append"));
        }
        assert_eq!(offsets, vec![0, 64, 128, 384]);
        assert!(arena.bytes().len().is_multiple_of(ALIGNMENT));
        for (offset, value) in offsets.iter().zip([b"a".as_slice(), b"bb", &[0u8; 200], b""]) {
            assert_eq!(arena.get(*offset).expect("read"), value);
        }
    }

    #[test]
    fn a_float_offset_is_rejected() {
        let mut arena = ByteArena::new();
        arena.append(b"alpha").expect("append");
        assert_eq!(arena.get(3), Err(DictError::InvalidOffset(3)));
    }

    #[test]
    fn an_offset_past_the_arena_is_rejected() {
        let mut arena = ByteArena::new();
        arena.append(b"alpha").expect("append");
        assert_eq!(arena.get(64), Err(DictError::InvalidOffset(64)));
        assert_eq!(arena.get(u32::MAX), Err(DictError::InvalidOffset(u32::MAX)));
    }

    #[test]
    fn a_length_that_runs_past_the_arena_is_rejected() {
        let mut arena = ByteArena::new();
        arena.append(b"alpha").expect("append");
        // Claim a value of four billion bytes at an aligned offset.
        arena.bytes[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(arena.get(0), Err(DictError::InvalidOffset(0)));
    }

    #[test]
    fn an_empty_value_round_trips() {
        let mut arena = ByteArena::new();
        let offset = arena.append(b"").expect("append");
        assert_eq!(arena.get(offset).expect("read"), b"");
        assert_eq!(arena.value_len(offset).expect("length"), 0);
    }
}
