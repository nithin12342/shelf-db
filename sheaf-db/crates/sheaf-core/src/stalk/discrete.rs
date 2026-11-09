// crates/sheaf-core/src/stalk/discrete.rs — store exact discrete attribute payloads
// FILE-005. Must never do float matvec.
#[derive(Clone, Copy, Debug)]
pub struct DiscreteStalk {
    /// Fixed 16-byte exact payload (ids, enums, bitmasks). Zero-cost compare.
    pub bytes: [u8; 16],
}

impl DiscreteStalk {
    pub fn from_u64(v: u64) -> Self {
        let mut b = [0u8; 16];
        b[..8].copy_from_slice(&v.to_le_bytes());
        Self { bytes: b }
    }
    pub fn eq_bytes(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}
