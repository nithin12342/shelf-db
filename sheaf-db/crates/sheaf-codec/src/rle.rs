// rle.rs - run-length codec (NF-13, spec Pillar 9).
//
// A run is one value repeated one or more times. The codec stores each run as a
// length and one value. It is lossless, including the bit pattern of a float.
//
// Layout: tag u8 | count u32 | runs u32 | (run_len u32, value)*
// A value is 8 bytes for u64 and i64, 4 bytes of bits for f32, and a u32 length
// plus the bytes for a string.
//
// The selector measures this codec against every other codec. RLE does not win
// by assumption. It wins on a block with long constant runs.

use crate::error::{take_u32, take_u8, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The id of the run-length codec.
pub const RLE_ID: u8 = 5;

const TAG_U64: u8 = 0;
const TAG_I64: u8 = 1;
const TAG_F32: u8 = 2;
const TAG_BYTES: u8 = 3;

/// The fixed header: tag, count, and run count.
const HEADER_LEN: usize = 9;

/// A codec that stores constant runs as one value and one length.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunLengthCodec;

impl RunLengthCodec {
    fn encode_u64(&self, values: &[u64]) -> Vec<u8> {
        let runs = runs_u64(values);
        let mut out = Vec::with_capacity(HEADER_LEN + runs.len() * 12);
        out.push(TAG_U64);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (len, value) in runs {
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    fn encode_i64(&self, values: &[i64]) -> Vec<u8> {
        let runs = runs_i64(values);
        let mut out = Vec::with_capacity(HEADER_LEN + runs.len() * 12);
        out.push(TAG_I64);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (len, value) in runs {
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    fn encode_f32(&self, values: &[f32]) -> Vec<u8> {
        let runs = runs_f32(values);
        let mut out = Vec::with_capacity(HEADER_LEN + runs.len() * 8);
        out.push(TAG_F32);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (len, value) in runs {
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        out
    }

    fn encode_bytes(&self, values: &[Vec<u8>]) -> Vec<u8> {
        let runs = runs_bytes(values);
        let body: usize = runs.iter().map(|(_, value)| 4 + value.len()).sum();
        let mut out = Vec::with_capacity(HEADER_LEN + body + runs.len() * 4);
        out.push(TAG_BYTES);
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (len, value) in runs {
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&(value.len() as u32).to_le_bytes());
            out.extend_from_slice(&value);
        }
        out
    }
}

fn runs_u64(values: &[u64]) -> Vec<(u32, u64)> {
    let mut runs: Vec<(u32, u64)> = Vec::new();
    for &value in values {
        match runs.last_mut() {
            Some((len, last)) if *last == value => *len += 1,
            _ => runs.push((1, value)),
        }
    }
    runs
}

fn runs_i64(values: &[i64]) -> Vec<(u32, i64)> {
    let mut runs: Vec<(u32, i64)> = Vec::new();
    for &value in values {
        match runs.last_mut() {
            Some((len, last)) if *last == value => *len += 1,
            _ => runs.push((1, value)),
        }
    }
    runs
}

fn runs_f32(values: &[f32]) -> Vec<(u32, f32)> {
    let mut runs: Vec<(u32, f32)> = Vec::new();
    for &value in values {
        match runs.last_mut() {
            Some((len, last)) if last.to_bits() == value.to_bits() => *len += 1,
            _ => runs.push((1, value)),
        }
    }
    runs
}

fn runs_bytes(values: &[Vec<u8>]) -> Vec<(u32, Vec<u8>)> {
    let mut runs: Vec<(u32, Vec<u8>)> = Vec::new();
    for value in values {
        match runs.last_mut() {
            Some((len, last)) if last == value => *len += 1,
            _ => runs.push((1, value.clone())),
        }
    }
    runs
}

impl ColumnCodec for RunLengthCodec {
    fn id(&self) -> u8 {
        RLE_ID
    }

    fn name(&self) -> &'static str {
        "rle"
    }

    fn accepts(&self, ty: ColumnType) -> bool {
        matches!(
            ty,
            ColumnType::U64 | ColumnType::I64 | ColumnType::F32 | ColumnType::Bytes
        )
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        Ok(match data {
            ColumnData::U64(values) => self.encode_u64(values),
            ColumnData::I64(values) => self.encode_i64(values),
            ColumnData::F32(values) => self.encode_f32(values),
            ColumnData::Bytes(values) => self.encode_bytes(values),
        })
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let tag = take_u8(bytes, 0)?;
        let count = take_u32(bytes, 1)? as usize;
        let run_count = take_u32(bytes, 5)? as usize;
        let mut at = HEADER_LEN;
        match tag {
            TAG_U64 => {
                let mut out = Vec::with_capacity(count);
                for _ in 0..run_count {
                    let len = take_u32(bytes, at)? as usize;
                    at += 4;
                    let end = at + 8;
                    if end > bytes.len() {
                        return Err(CodecError::Corrupt("truncated rle u64 value"));
                    }
                    let value = u64::from_le_bytes(bytes[at..end].try_into().unwrap());
                    at = end;
                    push_run(&mut out, len, count, value)?;
                }
                finish(out, count, "rle u64 count")
            }
            TAG_I64 => {
                let mut out = Vec::with_capacity(count);
                for _ in 0..run_count {
                    let len = take_u32(bytes, at)? as usize;
                    at += 4;
                    let end = at + 8;
                    if end > bytes.len() {
                        return Err(CodecError::Corrupt("truncated rle i64 value"));
                    }
                    let value = i64::from_le_bytes(bytes[at..end].try_into().unwrap());
                    at = end;
                    push_run(&mut out, len, count, value)?;
                }
                finish(out, count, "rle i64 count")
            }
            TAG_F32 => {
                let mut out = Vec::with_capacity(count);
                for _ in 0..run_count {
                    let len = take_u32(bytes, at)? as usize;
                    at += 4;
                    let end = at + 4;
                    if end > bytes.len() {
                        return Err(CodecError::Corrupt("truncated rle f32 value"));
                    }
                    let value = f32::from_bits(u32::from_le_bytes(bytes[at..end].try_into().unwrap()));
                    at = end;
                    push_run(&mut out, len, count, value)?;
                }
                finish(out, count, "rle f32 count")
            }
            TAG_BYTES => {
                let mut out = Vec::with_capacity(count);
                for _ in 0..run_count {
                    let len = take_u32(bytes, at)? as usize;
                    at += 4;
                    let value_len = take_u32(bytes, at)? as usize;
                    at += 4;
                    let end = at
                        .checked_add(value_len)
                        .ok_or(CodecError::Corrupt("rle string length overflow"))?;
                    if end > bytes.len() {
                        return Err(CodecError::Corrupt("truncated rle string value"));
                    }
                    let value = bytes[at..end].to_vec();
                    at = end;
                    push_run(&mut out, len, count, value)?;
                }
                finish(out, count, "rle bytes count")
            }
            _ => Err(CodecError::Corrupt("bad rle type tag")),
        }
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        match data {
            ColumnData::U64(values) => HEADER_LEN + runs_u64(values).len() * 12,
            ColumnData::I64(values) => HEADER_LEN + runs_i64(values).len() * 12,
            ColumnData::F32(values) => HEADER_LEN + runs_f32(values).len() * 8,
            ColumnData::Bytes(values) => {
                let runs = runs_bytes(values);
                HEADER_LEN + runs.iter().map(|(_, value)| 8 + value.len()).sum::<usize>()
            }
        }
    }
}

fn push_run<T: Clone>(out: &mut Vec<T>, len: usize, count: usize, value: T) -> Result<(), CodecError> {
    let next = out
        .len()
        .checked_add(len)
        .ok_or(CodecError::Corrupt("rle run length overflow"))?;
    if next > count {
        return Err(CodecError::Corrupt("rle run length passes the block count"));
    }
    out.extend(std::iter::repeat_n(value, len));
    Ok(())
}

fn finish<T>(out: Vec<T>, count: usize, reason: &'static str) -> Result<DecodedColumn, CodecError>
where
    DecodedColumn: From<Vec<T>>,
{
    if out.len() != count {
        return Err(CodecError::Corrupt(reason));
    }
    Ok(DecodedColumn::from(out))
}

impl From<Vec<u64>> for DecodedColumn {
    fn from(values: Vec<u64>) -> Self {
        DecodedColumn::U64(values)
    }
}

impl From<Vec<i64>> for DecodedColumn {
    fn from(values: Vec<i64>) -> Self {
        DecodedColumn::I64(values)
    }
}

impl From<Vec<f32>> for DecodedColumn {
    fn from(values: Vec<f32>) -> Self {
        DecodedColumn::F32(values)
    }
}

impl From<Vec<Vec<u8>>> for DecodedColumn {
    fn from(values: Vec<Vec<u8>>) -> Self {
        DecodedColumn::Bytes(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_constant_u64_block_round_trips_and_shrinks() {
        let codec = RunLengthCodec;
        let values = vec![7u64; 10_000];
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        assert_eq!(bytes.len(), HEADER_LEN + 12);
        assert_eq!(bytes.len(), codec.estimate(&ColumnData::U64(&values)));
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_u64().expect("u64"), values.as_slice());
    }

    #[test]
    fn a_signed_block_round_trips() {
        let codec = RunLengthCodec;
        let values = vec![i64::MIN, i64::MIN, 0, i64::MAX];
        let bytes = codec.encode(&ColumnData::I64(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_i64().expect("i64"), values.as_slice());
    }

    #[test]
    fn floats_round_trip_at_bit_level() {
        let codec = RunLengthCodec;
        let values = vec![
            0.0f32,
            -0.0,
            -0.0,
            f32::NAN,
            f32::NAN,
            f32::from_bits(0x7fc0_0001),
            f32::INFINITY,
        ];
        let bytes = codec.encode(&ColumnData::F32(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        let got = decoded.as_f32().expect("f32");
        for (a, b) in values.iter().zip(got.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "bit-level equality");
        }
    }

    #[test]
    fn strings_round_trip() {
        let codec = RunLengthCodec;
        let values = vec![
            b"repeat".to_vec(),
            b"repeat".to_vec(),
            b"".to_vec(),
            b"tail".to_vec(),
        ];
        let bytes = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
    }

    #[test]
    fn an_empty_block_round_trips() {
        let codec = RunLengthCodec;
        let values: Vec<u64> = Vec::new();
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        assert_eq!(bytes.len(), HEADER_LEN);
        let decoded = codec.decode(&bytes).expect("decode");
        assert_eq!(decoded.as_u64().expect("u64"), &[] as &[u64]);
    }

    #[test]
    fn a_truncated_value_is_rejected() {
        let codec = RunLengthCodec;
        let values = vec![1u64; 4];
        let bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        assert_eq!(
            codec.decode(&bytes[..bytes.len() - 1]),
            Err(CodecError::Corrupt("truncated rle u64 value"))
        );
    }

    #[test]
    fn a_run_past_the_count_is_rejected() {
        let codec = RunLengthCodec;
        let values = vec![1u64; 2];
        let mut bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        bytes[9..13].copy_from_slice(&9u32.to_le_bytes());
        assert_eq!(
            codec.decode(&bytes),
            Err(CodecError::Corrupt("rle run length passes the block count"))
        );
    }

    #[test]
    fn a_short_total_is_rejected() {
        let codec = RunLengthCodec;
        let values = vec![1u64, 2];
        let mut bytes = codec.encode(&ColumnData::U64(&values)).expect("encode");
        bytes[1..5].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            codec.decode(&bytes),
            Err(CodecError::Corrupt("rle u64 count"))
        );
    }
}
