// for_.rs - frame of reference plus bit packing (MF-05).
//
// A block of integers is stored as the smallest value plus the bit-packed
// distance from it. Close values need few bits, so a narrow block packs to a
// few bytes per hundred values.
//
// The header records the type, the count, the base, and the bit width, so the
// decoder needs no external metadata:
//   tag u8 (0 = u64, 1 = i64) | count u32 | base i64 | bits u8 | packed deltas

use crate::bitpack::{bits_needed, pack, packed_len, unpack};
use crate::error::{take_u32, take_u8, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The id of the frame-of-reference codec.
pub const FOR_ID: u8 = 2;

/// The tag for a u64 block.
const TAG_U64: u8 = 0;
/// The tag for an i64 block.
const TAG_I64: u8 = 1;
/// Header bytes before the packed block.
const HEADER_LEN: usize = 1 + 4 + 8 + 1;

/// A codec that stores integers as a base value and bit-packed deltas.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameOfReferenceCodec;

impl FrameOfReferenceCodec {
    /// Frame an i64 block: the base is the minimum and each delta is the exact
    /// distance, which always fits in a u64.
    fn frame_i64(values: &[i64]) -> (i64, Vec<u64>) {
        let base = values.iter().copied().min().unwrap_or(0);
        let deltas = values
            .iter()
            .map(|&v| (v as i128 - base as i128) as u64)
            .collect();
        (base, deltas)
    }

    /// Frame a u64 block.
    fn frame_u64(values: &[u64]) -> (u64, Vec<u64>) {
        let base = values.iter().copied().min().unwrap_or(0);
        let deltas = values.iter().map(|&v| v.wrapping_sub(base)).collect();
        (base, deltas)
    }
}

impl ColumnCodec for FrameOfReferenceCodec {
    fn id(&self) -> u8 {
        FOR_ID
    }

    fn name(&self) -> &'static str {
        "for"
    }

    fn accepts(&self, ty: ColumnType) -> bool {
        matches!(ty, ColumnType::U64 | ColumnType::I64)
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        let (tag, count, base_bytes, deltas) = match data {
            ColumnData::U64(values) => {
                let (base, deltas) = Self::frame_u64(values);
                (TAG_U64, values.len(), base.to_le_bytes(), deltas)
            }
            ColumnData::I64(values) => {
                let (base, deltas) = Self::frame_i64(values);
                (TAG_I64, values.len(), base.to_le_bytes(), deltas)
            }
            other => return Err(CodecError::UnsupportedType(other.column_type().name())),
        };

        let max = deltas.iter().copied().max().unwrap_or(0);
        let bits = bits_needed(max);
        let mut out = Vec::with_capacity(HEADER_LEN + packed_len(count, bits));
        out.push(tag);
        out.extend_from_slice(&(count as u32).to_le_bytes());
        out.extend_from_slice(&base_bytes);
        out.push(bits as u8);
        pack(&deltas, bits, &mut out);
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let tag = take_u8(bytes, 0)?;
        let count = take_u32(bytes, 1)? as usize;
        if bytes.len() < HEADER_LEN {
            return Err(CodecError::Corrupt("truncated frame header"));
        }
        let base_bytes: [u8; 8] = bytes[5..13].try_into().unwrap();
        let bits = bytes[13] as u32;
        let deltas = unpack(&bytes[HEADER_LEN..], count, bits)?;

        match tag {
            TAG_U64 => {
                let base = u64::from_le_bytes(base_bytes);
                Ok(DecodedColumn::U64(deltas.iter().map(|&d| base.wrapping_add(d)).collect()))
            }
            TAG_I64 => {
                let base = i64::from_le_bytes(base_bytes);
                Ok(DecodedColumn::I64(
                    deltas.iter().map(|&d| (base as i128 + d as i128) as i64).collect(),
                ))
            }
            other => {
                let _ = other;
                Err(CodecError::Corrupt("bad frame tag"))
            }
        }
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        let (count, max_delta) = match data {
            ColumnData::U64(values) => {
                let (_, deltas) = Self::frame_u64(values);
                (values.len(), deltas.iter().copied().max().unwrap_or(0))
            }
            ColumnData::I64(values) => {
                let (_, deltas) = Self::frame_i64(values);
                (values.len(), deltas.iter().copied().max().unwrap_or(0))
            }
            _ => return HEADER_LEN,
        };
        HEADER_LEN + packed_len(count, bits_needed(max_delta))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_narrow_u64_block_packs_small() {
        let codec = FrameOfReferenceCodec;
        let values: Vec<u64> = (1_000_000..1_000_100).collect();
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        // One million to one million plus 99 needs seven bits per value.
        assert_eq!(bytes.len(), HEADER_LEN + packed_len(100, 7));
        assert!(bytes.len() < values.len() * 8 / 2);
        assert_eq!(
            codec.decode(&bytes).expect("decode").as_u64().expect("u64"),
            values.as_slice()
        );
    }

    #[test]
    fn a_signed_block_round_trips_its_extremes() {
        let codec = FrameOfReferenceCodec;
        let values = vec![i64::MIN, -5, 0, 5, i64::MAX];
        let bytes = codec.encode(&ColumnData::I64(&values)).expect("encode");
        assert_eq!(
            codec.decode(&bytes).expect("decode").as_i64().expect("i64"),
            values.as_slice()
        );
    }

    #[test]
    fn an_unsigned_block_round_trips_its_extremes() {
        let codec = FrameOfReferenceCodec;
        let values = vec![0u64, 1, u64::MAX, u64::MAX - 1];
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        assert_eq!(
            codec.decode(&bytes).expect("decode").as_u64().expect("u64"),
            values.as_slice()
        );
    }

    #[test]
    fn an_empty_block_round_trips() {
        let codec = FrameOfReferenceCodec;
        let bytes = codec.encode(&ColumnData::U64(&[])).expect("encode");
        assert_eq!(bytes.len(), HEADER_LEN);
        assert!(codec.decode(&bytes).expect("decode").is_empty());
    }

    #[test]
    fn a_bad_tag_is_rejected() {
        let codec = FrameOfReferenceCodec;
        let values = vec![1u64, 2, 3];
        let mut bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        bytes[0] = 9;
        assert_eq!(
            codec.decode(&bytes),
            Err(CodecError::Corrupt("bad frame tag"))
        );
    }

    #[test]
    fn a_truncated_header_is_rejected() {
        let codec = FrameOfReferenceCodec;
        assert_eq!(
            codec.decode(&[0u8, 1, 0, 0]),
            Err(CodecError::Corrupt("truncated u32"))
        );
    }

    #[test]
    fn codec_rejects_a_float_block() {
        let codec = FrameOfReferenceCodec;
        let values = vec![1.0f32];
        assert_eq!(
            codec.encode(&ColumnData::F32(&values)),
            Err(CodecError::UnsupportedType("f32"))
        );
    }
}
