// traits.rs - the column codec port and the value model (MF-05).
//
// A block of one column has one type. A codec declares which types it accepts,
// encodes a block to bytes, decodes the bytes, and states a size estimate.
// The selector in select.rs depends on this port only, so a new codec is a new
// adapter with no change to the caller.

use crate::error::CodecError;

/// The value type of a column block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnType {
    /// Unsigned 64-bit integers.
    U64,
    /// Signed 64-bit integers.
    I64,
    /// 32-bit floats.
    F32,
    /// Length-delimited byte strings.
    Bytes,
}

impl ColumnType {
    /// The type name for an error message.
    pub fn name(self) -> &'static str {
        match self {
            ColumnType::U64 => "u64",
            ColumnType::I64 => "i64",
            ColumnType::F32 => "f32",
            ColumnType::Bytes => "bytes",
        }
    }
}

/// A borrowed block of column values.
#[derive(Clone, Copy, Debug)]
pub enum ColumnData<'a> {
    U64(&'a [u64]),
    I64(&'a [i64]),
    F32(&'a [f32]),
    Bytes(&'a [Vec<u8>]),
}

impl<'a> ColumnData<'a> {
    /// The type of the block.
    pub fn column_type(&self) -> ColumnType {
        match self {
            ColumnData::U64(_) => ColumnType::U64,
            ColumnData::I64(_) => ColumnType::I64,
            ColumnData::F32(_) => ColumnType::F32,
            ColumnData::Bytes(_) => ColumnType::Bytes,
        }
    }

    /// The number of values in the block.
    pub fn len(&self) -> usize {
        match self {
            ColumnData::U64(v) => v.len(),
            ColumnData::I64(v) => v.len(),
            ColumnData::F32(v) => v.len(),
            ColumnData::Bytes(v) => v.len(),
        }
    }

    /// True when the block holds no value.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The size of the block with no encoding: eight bytes per number and one
    /// length prefix plus the bytes per string.
    pub fn raw_bytes(&self) -> usize {
        match self {
            ColumnData::U64(v) => v.len() * 8,
            ColumnData::I64(v) => v.len() * 8,
            ColumnData::F32(v) => v.len() * 4,
            ColumnData::Bytes(v) => v.iter().map(|s| s.len() + 4).sum(),
        }
    }
}

/// A decoded block. The variant follows the codec that produced it.
#[derive(Clone, Debug, PartialEq)]
pub enum DecodedColumn {
    U64(Vec<u64>),
    I64(Vec<i64>),
    F32(Vec<f32>),
    Bytes(Vec<Vec<u8>>),
}

impl DecodedColumn {
    /// The type of the block.
    pub fn column_type(&self) -> ColumnType {
        match self {
            DecodedColumn::U64(_) => ColumnType::U64,
            DecodedColumn::I64(_) => ColumnType::I64,
            DecodedColumn::F32(_) => ColumnType::F32,
            DecodedColumn::Bytes(_) => ColumnType::Bytes,
        }
    }

    /// The number of values.
    pub fn len(&self) -> usize {
        match self {
            DecodedColumn::U64(v) => v.len(),
            DecodedColumn::I64(v) => v.len(),
            DecodedColumn::F32(v) => v.len(),
            DecodedColumn::Bytes(v) => v.len(),
        }
    }

    /// True when the block holds no value.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The u64 values, or an error when the block has another type.
    pub fn as_u64(&self) -> Result<&[u64], CodecError> {
        match self {
            DecodedColumn::U64(v) => Ok(v),
            other => Err(CodecError::UnsupportedType(other.column_type().name())),
        }
    }

    /// The i64 values, or an error when the block has another type.
    pub fn as_i64(&self) -> Result<&[i64], CodecError> {
        match self {
            DecodedColumn::I64(v) => Ok(v),
            other => Err(CodecError::UnsupportedType(other.column_type().name())),
        }
    }

    /// The f32 values, or an error when the block has another type.
    pub fn as_f32(&self) -> Result<&[f32], CodecError> {
        match self {
            DecodedColumn::F32(v) => Ok(v),
            other => Err(CodecError::UnsupportedType(other.column_type().name())),
        }
    }

    /// The byte strings, or an error when the block has another type.
    pub fn as_bytes(&self) -> Result<&[Vec<u8>], CodecError> {
        match self {
            DecodedColumn::Bytes(v) => Ok(v),
            other => Err(CodecError::UnsupportedType(other.column_type().name())),
        }
    }
}

/// The codec port. A codec is stateless, so every codec is shareable across
/// worker threads.
pub trait ColumnCodec: Send + Sync {
    /// The codec id. It is stable on disk, so an id is never reused.
    fn id(&self) -> u8;

    /// The codec name for a report.
    fn name(&self) -> &'static str;

    /// True when the codec accepts this column type.
    fn accepts(&self, ty: ColumnType) -> bool;

    /// Encode one block. The call returns NotApplicable when the codec cannot
    /// represent the block without loss.
    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError>;

    /// Decode one block. The call returns Corrupt on a truncated or bad buffer.
    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError>;

    /// A planning estimate of the encoded size in bytes. It is an estimate,
    /// never a promise.
    fn estimate(&self, data: &ColumnData<'_>) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_follow_the_type() {
        let u = vec![1u64, 2, 3];
        let i = vec![-1i64, 2];
        let f = vec![1.0f32, 2.0, 3.0, 4.0];
        let s = vec![b"ab".to_vec(), b"".to_vec()];
        assert_eq!(ColumnData::U64(&u).raw_bytes(), 24);
        assert_eq!(ColumnData::I64(&i).raw_bytes(), 16);
        assert_eq!(ColumnData::F32(&f).raw_bytes(), 16);
        assert_eq!(ColumnData::Bytes(&s).raw_bytes(), 10);
        assert_eq!(ColumnData::Bytes(&s).len(), 2);
        assert!(!ColumnData::Bytes(&s).is_empty());
        assert_eq!(ColumnData::U64(&[]).column_type(), ColumnType::U64);
    }

    #[test]
    fn decoded_accessors_check_the_type() {
        let block = DecodedColumn::U64(vec![1, 2]);
        assert_eq!(block.as_u64().expect("u64"), &[1, 2]);
        assert_eq!(block.len(), 2);
        assert_eq!(
            block.as_i64(),
            Err(CodecError::UnsupportedType("u64"))
        );
    }
}
