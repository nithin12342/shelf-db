// dict/mod.rs - the global dictionary arena (MF-06, spec Pillar 9).
//
// A text column stores 32-bit offsets, not bytes. One arena per table holds
// every distinct value once, so twelve columns that repeat the same country
// names share one copy and one offset.
//
// Module map:
//
// ```text
// arena     append-only 64-byte aligned byte arena with a stable offset
// interner  maps a value to an offset, keyed by the BLAKE3 hash of the value
// column    a dictionary-encoded column that holds 32-bit offsets
// ```
//
// The port boundary is the offset. A column never owns arena bytes, and the
// arena never knows which column wrote a value.

use std::fmt;

pub mod arena;
pub mod column;
pub mod interner;

pub use arena::{ByteArena, ALIGNMENT, LENGTH_PREFIX};
pub use column::DictColumn;
pub use interner::Interner;

/// Why a dictionary operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DictError {
    /// The offset is not aligned, is outside the arena, or carries a length
    /// that runs past the arena. The reader never returns a truncated value.
    InvalidOffset(u32),
    /// The arena reached the 4 GiB limit of a 32-bit offset.
    ArenaFull,
    /// The column holds no value at this position.
    IndexOutOfRange(usize),
}

impl fmt::Display for DictError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DictError::InvalidOffset(offset) => {
                write!(f, "dictionary offset {offset} is not a valid entry")
            }
            DictError::ArenaFull => write!(f, "the dictionary arena reached the 32-bit limit"),
            DictError::IndexOutOfRange(index) => {
                write!(f, "the dictionary column holds no value at index {index}")
            }
        }
    }
}

impl std::error::Error for DictError {}
