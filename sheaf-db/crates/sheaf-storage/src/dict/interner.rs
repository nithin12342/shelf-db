// dict/interner.rs - maps a byte string to one arena offset (MF-06).
//
// The key is the BLAKE3 hash of the value. A 32-byte hash is smaller than most
// values, so the map stays small, and two different values that share a hash
// are told apart by the bytes in the arena. The bytes are the source of truth.
//
// One interner serves every text column of one table. A value that appears in
// two columns is stored once and returns one offset to both.

use std::collections::HashMap;

use super::arena::ByteArena;
use super::DictError;

/// The hash length in bytes.
pub const HASH_LEN: usize = 32;

/// A content-addressed value table over one byte arena.
#[derive(Clone, Debug, Default)]
pub struct Interner {
    arena: ByteArena,
    buckets: HashMap<[u8; HASH_LEN], Vec<u32>>,
    interned: u64,
    reused: u64,
}

impl Interner {
    /// An empty interner.
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty interner with room for `capacity` arena bytes.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { arena: ByteArena::with_capacity(capacity), ..Self::default() }
    }

    /// Store `value` and return its offset. A value that is already stored
    /// returns the offset of the existing entry.
    pub fn intern(&mut self, value: &[u8]) -> Result<u32, DictError> {
        let hash = blake3::hash(value);
        let key: [u8; HASH_LEN] = *hash.as_bytes();
        if let Some(offsets) = self.buckets.get(&key) {
            // A hash match is not a value match, so compare the bytes.
            for &offset in offsets {
                if self.arena.get(offset)? == value {
                    self.reused += 1;
                    return Ok(offset);
                }
            }
        }
        let offset = self.arena.append(value)?;
        self.buckets.entry(key).or_default().push(offset);
        self.interned += 1;
        Ok(offset)
    }

    /// Read the value at `offset`.
    pub fn resolve(&self, offset: u32) -> Result<&[u8], DictError> {
        self.arena.get(offset)
    }

    /// The number of distinct values.
    pub fn entries(&self) -> usize {
        self.arena.entries()
    }

    /// The arena length in bytes, including the prefixes and the padding.
    pub fn arena_bytes(&self) -> usize {
        self.arena.len()
    }

    /// The number of calls that stored a new value.
    pub fn interned(&self) -> u64 {
        self.interned
    }

    /// The number of calls that returned an existing offset.
    pub fn reused(&self) -> u64 {
        self.reused
    }

    /// The stored values in offset order.
    pub fn values(&self) -> Result<Vec<Vec<u8>>, DictError> {
        let mut out = Vec::with_capacity(self.entries());
        let mut at = 0usize;
        while at < self.arena.len() {
            let offset = at as u32;
            let value = self.arena.get(offset)?;
            at = super::arena::align_up(at + super::LENGTH_PREFIX + value.len());
            out.push(value.to_vec());
        }
        Ok(out)
    }

    /// The arena bytes, for a save or a page body.
    pub fn arena(&self) -> &ByteArena {
        &self.arena
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_intern_returns_the_same_offset() {
        let mut dict = Interner::new();
        let first = dict.intern(b"canada").expect("first intern");
        let second = dict.intern(b"canada").expect("second intern");
        assert_eq!(first, second);
        assert_eq!(dict.entries(), 1);
        assert_eq!(dict.interned(), 1);
        assert_eq!(dict.reused(), 1);
        assert_eq!(dict.resolve(first).expect("resolve"), b"canada");
    }

    #[test]
    fn distinct_values_get_distinct_offsets() {
        let mut dict = Interner::new();
        let a = dict.intern(b"canada").expect("a");
        let b = dict.intern(b"mexico").expect("b");
        assert_ne!(a, b);
        assert_eq!(dict.entries(), 2);
        assert_eq!(dict.resolve(a).expect("a"), b"canada");
        assert_eq!(dict.resolve(b).expect("b"), b"mexico");
    }

    #[test]
    fn a_corrupt_offset_is_reported() {
        let mut dict = Interner::new();
        dict.intern(b"canada").expect("intern");
        assert_eq!(dict.resolve(7), Err(DictError::InvalidOffset(7)));
    }

    #[test]
    fn values_walk_every_entry_once() {
        let mut dict = Interner::new();
        for value in [b"aa".as_slice(), b"bbbb", b"", b"cc"] {
            dict.intern(value).expect("intern");
        }
        assert_eq!(
            dict.values().expect("values"),
            vec![b"aa".to_vec(), b"bbbb".to_vec(), Vec::new(), b"cc".to_vec()]
        );
        assert_eq!(dict.arena_bytes() % super::super::ALIGNMENT, 0);
    }
}
