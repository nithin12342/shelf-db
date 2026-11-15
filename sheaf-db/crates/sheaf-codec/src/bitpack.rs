// bitpack.rs - bit packing at one to 64 bits per value (MF-05).
//
// Values are written least significant bit first, with no padding between
// values. A block of `count` values at `bits` bits per value occupies
// ceil(count * bits / 8) bytes. A bit width of zero is legal and means that
// every value is zero, so the block occupies no byte.

use crate::error::{take_u32, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The number of bits needed to hold `value`. Zero needs zero bits.
pub fn bits_needed(value: u64) -> u32 {
    if value == 0 {
        0
    } else {
        64 - value.leading_zeros()
    }
}

/// The packed length in bytes.
pub fn packed_len(count: usize, bits: u32) -> usize {
    (count * bits as usize).div_ceil(8)
}

/// Pack `values` at `bits` bits each. A value that does not fit is masked, so
/// the caller must pass a width that holds every value.
pub fn pack(values: &[u64], bits: u32, out: &mut Vec<u8>) {
    debug_assert!(bits <= 64);
    if bits == 0 {
        return;
    }
    let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
    let mut acc: u128 = 0;
    let mut held: u32 = 0;
    for &value in values {
        acc |= ((value & mask) as u128) << held;
        held += bits;
        while held >= 8 {
            out.push(acc as u8);
            acc >>= 8;
            held -= 8;
        }
    }
    if held > 0 {
        out.push(acc as u8);
    }
}

/// Unpack `count` values at `bits` bits each.
pub fn unpack(bytes: &[u8], count: usize, bits: u32) -> Result<Vec<u64>, CodecError> {
    if bits > 64 {
        return Err(CodecError::Corrupt("bit width above 64"));
    }
    if bytes.len() < packed_len(count, bits) {
        return Err(CodecError::Corrupt("truncated packed block"));
    }
    if bits == 0 {
        return Ok(vec![0; count]);
    }
    let mask: u128 = if bits == 64 { u64::MAX as u128 } else { (1u128 << bits) - 1 };
    let mut out = Vec::with_capacity(count);
    let mut acc: u128 = 0;
    let mut held: u32 = 0;
    let mut at = 0usize;
    for _ in 0..count {
        while held < bits {
            acc |= (bytes[at] as u128) << held;
            at += 1;
            held += 8;
        }
        out.push((acc & mask) as u64);
        acc >>= bits;
        held -= bits;
    }
    Ok(out)
}

/// The id of the plain bit-packed u64 codec.
pub const BITPACK_ID: u8 = 1;

/// A codec that stores a u64 block at the narrowest bit width that holds the
/// largest value.
#[derive(Clone, Copy, Debug, Default)]
pub struct BitPackCodec;

impl ColumnCodec for BitPackCodec {
    fn id(&self) -> u8 {
        BITPACK_ID
    }

    fn name(&self) -> &'static str {
        "bitpack"
    }

    fn accepts(&self, ty: ColumnType) -> bool {
        ty == ColumnType::U64
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        let ColumnData::U64(values) = data else {
            return Err(CodecError::UnsupportedType(data.column_type().name()));
        };
        let max = values.iter().copied().max().unwrap_or(0);
        let bits = bits_needed(max);
        let mut out = Vec::with_capacity(5 + packed_len(values.len(), bits));
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.push(bits as u8);
        pack(values, bits, &mut out);
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let count = take_u32(bytes, 0)? as usize;
        let bits = *bytes.get(4).ok_or(CodecError::Corrupt("truncated bit width"))? as u32;
        let values = unpack(&bytes[5..], count, bits)?;
        Ok(DecodedColumn::U64(values))
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        match data {
            ColumnData::U64(values) => {
                let bits = bits_needed(values.iter().copied().max().unwrap_or(0));
                5 + packed_len(values.len(), bits)
            }
            _ => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_width_counts_the_largest_value() {
        assert_eq!(bits_needed(0), 0);
        assert_eq!(bits_needed(1), 1);
        assert_eq!(bits_needed(2), 2);
        assert_eq!(bits_needed(255), 8);
        assert_eq!(bits_needed(256), 9);
        assert_eq!(bits_needed(u64::MAX), 64);
    }

    #[test]
    fn every_width_round_trips() {
        let mut state = 0x1234_5678_9abc_def0u64;
        for bits in 1..=64u32 {
            let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
            let mut values = Vec::new();
            for _ in 0..257 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                values.push(state & mask);
            }
            let mut buffer = Vec::new();
            pack(&values, bits, &mut buffer);
            assert_eq!(buffer.len(), packed_len(values.len(), bits), "width {bits}");
            assert_eq!(unpack(&buffer, values.len(), bits).expect("unpack"), values);
        }
    }

    #[test]
    fn a_zero_width_block_occupies_no_byte() {
        let values = vec![0u64; 100];
        let mut buffer = Vec::new();
        pack(&values, 0, &mut buffer);
        assert!(buffer.is_empty());
        assert_eq!(unpack(&buffer, 100, 0).expect("unpack"), values);
    }

    #[test]
    fn a_truncated_block_is_rejected() {
        let values = vec![1u64, 2, 3];
        let mut buffer = Vec::new();
        pack(&values, 2, &mut buffer);
        assert_eq!(
            unpack(&buffer[..buffer.len() - 1], 3, 2),
            Err(CodecError::Corrupt("truncated packed block"))
        );
    }

    #[test]
    fn codec_round_trips_a_full_range_block() {
        let codec = BitPackCodec;
        let values = vec![0u64, 1, u64::MAX, 7, 1024];
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        assert_eq!(bytes.len(), codec.estimate(&ColumnData::U64(&values)));
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_u64().expect("u64"), values.as_slice());
    }

    #[test]
    fn codec_rejects_another_type() {
        let codec = BitPackCodec;
        let values = vec![1i64, 2];
        assert_eq!(
            codec.encode(&ColumnData::I64(&values)),
            Err(CodecError::UnsupportedType("i64"))
        );
    }
}
