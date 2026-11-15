// raw.rs - the unframed baseline codec (MF-05).
//
// Every block has a raw form: a type tag, a count, then the values in a
// fixed-width little-endian field, or a length prefix plus the bytes for a
// string. The selector measures the raw size against the other codecs, so a
// block that does not compress is still stored and still reported.
//
// The raw form is not a silent fallback. The selector records which codec won.
//
// Layout: tag u8 (0 u64, 1 i64, 2 f32, 3 bytes) | count u32 | body.

use crate::error::{take_u32, take_u8, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The id of the raw codec.
pub const RAW_ID: u8 = 0;

/// Type tags in the raw layout.
const TAG_U64: u8 = 0;
const TAG_I64: u8 = 1;
const TAG_F32: u8 = 2;
const TAG_BYTES: u8 = 3;

/// The header length: tag plus count.
const HEADER_LEN: usize = 5;

/// A codec that stores values with no encoding.
#[derive(Clone, Copy, Debug, Default)]
pub struct RawCodec;

impl ColumnCodec for RawCodec {
    fn id(&self) -> u8 {
        RAW_ID
    }

    fn name(&self) -> &'static str {
        "raw"
    }

    fn accepts(&self, _ty: ColumnType) -> bool {
        true
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        let mut out = Vec::with_capacity(HEADER_LEN + data.raw_bytes());
        let tag = match data {
            ColumnData::U64(_) => TAG_U64,
            ColumnData::I64(_) => TAG_I64,
            ColumnData::F32(_) => TAG_F32,
            ColumnData::Bytes(_) => TAG_BYTES,
        };
        out.push(tag);
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        match data {
            ColumnData::U64(values) => {
                for &v in values.iter() {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            ColumnData::I64(values) => {
                for &v in values.iter() {
                    out.extend_from_slice(&v.to_le_bytes());
                }
            }
            ColumnData::F32(values) => {
                for &v in values.iter() {
                    out.extend_from_slice(&v.to_bits().to_le_bytes());
                }
            }
            ColumnData::Bytes(values) => {
                for value in values.iter() {
                    out.extend_from_slice(&(value.len() as u32).to_le_bytes());
                    out.extend_from_slice(value);
                }
            }
        }
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let tag = take_u8(bytes, 0)?;
        let count = take_u32(bytes, 1)? as usize;
        let body = bytes.get(HEADER_LEN..).ok_or(CodecError::Corrupt("truncated raw header"))?;
        match tag {
            TAG_U64 => {
                if body.len() != count * 8 {
                    return Err(CodecError::Corrupt("raw u64 length does not match the count"));
                }
                Ok(DecodedColumn::U64(
                    body.chunks_exact(8).map(|c| u64::from_le_bytes(c.try_into().unwrap())).collect(),
                ))
            }
            TAG_I64 => {
                if body.len() != count * 8 {
                    return Err(CodecError::Corrupt("raw i64 length does not match the count"));
                }
                Ok(DecodedColumn::I64(
                    body.chunks_exact(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect(),
                ))
            }
            TAG_F32 => {
                if body.len() != count * 4 {
                    return Err(CodecError::Corrupt("raw f32 length does not match the count"));
                }
                Ok(DecodedColumn::F32(
                    body.chunks_exact(4)
                        .map(|c| f32::from_bits(u32::from_le_bytes(c.try_into().unwrap())))
                        .collect(),
                ))
            }
            TAG_BYTES => {
                let mut at = 0usize;
                let mut out = Vec::with_capacity(count);
                for _ in 0..count {
                    let len = take_u32(body, at)? as usize;
                    at += 4;
                    let end = at
                        .checked_add(len)
                        .ok_or(CodecError::Corrupt("raw string length overflow"))?;
                    if end > body.len() {
                        return Err(CodecError::Corrupt("truncated raw string"));
                    }
                    out.push(body[at..end].to_vec());
                    at = end;
                }
                Ok(DecodedColumn::Bytes(out))
            }
            _ => Err(CodecError::Corrupt("bad raw type tag")),
        }
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        HEADER_LEN + data.raw_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip_with_no_encoding() {
        let codec = RawCodec;
        let u = vec![0u64, 1, u64::MAX];
        let bytes = codec.encode(&ColumnData::U64(&u)).expect("encode u64");
        assert_eq!(bytes.len(), HEADER_LEN + 24);
        assert_eq!(codec.decode(&bytes).expect("decode").as_u64().expect("u64"), u.as_slice());

        let i = vec![i64::MIN, 0, i64::MAX];
        let bytes = codec.encode(&ColumnData::I64(&i)).expect("encode i64");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_i64().expect("i64"), i.as_slice());
    }

    #[test]
    fn floats_round_trip_at_bit_level() {
        let codec = RawCodec;
        let values = vec![0.0f32, -0.0, 1.5, f32::NAN, f32::INFINITY, f32::from_bits(0x7fc0_0001)];
        let bytes = codec.encode(&ColumnData::F32(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        let got = decoded.as_f32().expect("f32");
        for (a, b) in values.iter().zip(got.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "bit-level equality");
        }
    }

    #[test]
    fn strings_round_trip() {
        let codec = RawCodec;
        let values = vec![b"".to_vec(), b"a".to_vec(), vec![0u8; 4096]];
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
    }

    #[test]
    fn a_truncated_string_is_rejected() {
        let codec = RawCodec;
        let values = vec![b"abc".to_vec()];
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        assert_eq!(
            codec.decode(&bytes[..bytes.len() - 1]),
            Err(CodecError::Corrupt("truncated raw string"))
        );
    }

    #[test]
    fn a_bad_tag_is_rejected() {
        let codec = RawCodec;
        let values = vec![1u64];
        let mut bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        bytes[0] = 7;
        assert_eq!(codec.decode(&bytes), Err(CodecError::Corrupt("bad raw type tag")));
    }
}
