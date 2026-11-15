//! sheaf-codec - the columnar codec crate (MF-05, spec Pillar 9).
//!
//! One port, one adapter per encoding, and one selector that measures them.
//!
//! ```text
//! traits   ColumnCodec port, ColumnData, DecodedColumn
//! error    explicit codec errors
//! bitpack  bit packing at one to 64 bits, and the plain bit-packed codec
//! for_     frame of reference plus bit packing, for u64 and i64
//! alp      adaptive lossless decimal floats, for f32
//! fsst     symbol table training and string encoding, for bytes
//! raw      the unframed baseline codec for every type
//! select   measures every accepting codec and keeps the smallest result
//! ```
//!
//! Codec ids are stable on disk: 0 raw, 1 bitpack, 2 frame of reference,
//! 3 ALP, and 4 FSST. An id is never reused, and an unknown id is an error.

pub mod alp;
pub mod bitpack;
pub mod error;
pub mod for_;
pub mod fsst;
pub mod raw;
pub mod select;
pub mod traits;

pub use alp::{AlpCodec, ALP_ID};
pub use bitpack::{BitPackCodec, BITPACK_ID};
pub use error::CodecError;
pub use for_::{FrameOfReferenceCodec, FOR_ID};
pub use fsst::{train, FsstCodec, SymbolTable, FSST_ID, MAX_SYMBOLS};
pub use raw::{RawCodec, RAW_ID};
pub use select::{codec_by_id, codecs, codecs_for, decode, select, CodecChoice};
pub use traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};
