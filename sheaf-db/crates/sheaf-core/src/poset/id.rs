// crates/sheaf-core/src/poset/id.rs — encode CellId dimension tagged identifier
// FILE-001. Must never parse queries or touch floats.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct CellId(pub u64);

impl CellId {
    /// Bits 56-63: dimension tag. Bits 0-55: sequence index.
    pub fn new(dim: u8, index: u64) -> Self {
        debug_assert!(index < (1u64 << 56));
        Self(((dim as u64) << 56) | index)
    }
    pub fn dim(self) -> u8 {
        (self.0 >> 56) as u8
    }
    pub fn index(self) -> u64 {
        self.0 & ((1u64 << 56) - 1)
    }
    pub fn as_u64(self) -> u64 {
        self.0
    }
}
