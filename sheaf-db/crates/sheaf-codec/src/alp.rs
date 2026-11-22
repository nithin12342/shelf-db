// alp.rs - adaptive lossless decimal floats (MF-05).
//
// A decimal value such as 12.34 is stored as the integer 1234 and the scale
// 100. The codec picks one factor and one exponent for the whole block, so a
// column of money or sensor readings becomes a block of small integers.
//
// The codec is lossless by construction, not by hope. Encode rebuilds every
// value from the chosen digits and compares the bit pattern. If one value does
// not come back bit for bit, the codec returns NotApplicable and the selector
// records the reason.
//
// Layout: count u32 | factor u8 | exponent u8 | base i64 | bits u8 | packed
// The decoded value is (base + delta) / (factor * 10^exponent).

use crate::bitpack::{bits_needed, pack, packed_len, unpack};
use crate::error::{take_u32, CodecError};
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

/// The id of the ALP codec.
pub const ALP_ID: u8 = 3;

/// Header bytes before the packed digits.
const HEADER_LEN: usize = 4 + 1 + 1 + 8 + 1;
/// The candidate factors. A larger factor needs fewer digits but overflows
/// sooner, so all three are measured.
const FACTORS: [u8; 3] = [1, 10, 100];
/// The largest exponent that keeps 100 * 10^10 inside a u64 scale.
const MAX_EXPONENT: u8 = 10;
/// The largest magnitude that an i64 digit can hold.
const DIGIT_LIMIT: f64 = 9.2e18;

/// One candidate framing of a block.
struct Candidate {
    factor: u8,
    exponent: u8,
    base: i64,
    bits: u8,
    deltas: Vec<u64>,
}

impl Candidate {
    /// The encoded size of this framing.
    fn size(&self) -> usize {
        HEADER_LEN + packed_len(self.deltas.len(), self.bits as u32)
    }

    /// The scale that the decoder divides by.
    fn scale(&self) -> f64 {
        self.factor as f64 * 10f64.powi(self.exponent as i32)
    }

    /// Rebuild one value from the stored digits, exactly as the decoder does.
    fn value(&self, digits: i64) -> f32 {
        (digits as f64 / self.scale()) as f32
    }

    /// True when every value comes back with the same bit pattern.
    ///
    /// A small framing is not automatically a correct framing. A factor of one
    /// and an exponent of zero turns 3.14 into the digit 3, so this check runs
    /// before a framing is accepted.
    fn exact(&self, values: &[f32]) -> bool {
        if self.deltas.len() != values.len() {
            return false;
        }
        self.deltas.iter().zip(values.iter()).all(|(&delta, &value)| {
            let digits = (self.base as i128 + delta as i128) as i64;
            self.value(digits).to_bits() == value.to_bits()
        })
    }
}

/// Try to frame `values` with one factor and one exponent. Returns None when a
/// value needs a digit outside the i64 range.
fn frame(values: &[f32], factor: u8, exponent: u8) -> Option<Candidate> {
    let scale = factor as f64 * 10f64.powi(exponent as i32);
    let mut digits: Vec<i64> = Vec::with_capacity(values.len());
    for &v in values {
        let scaled = (v as f64) * scale;
        if !scaled.is_finite() || scaled.abs() >= DIGIT_LIMIT {
            return None;
        }
        digits.push(scaled.round() as i64);
    }
    let base = digits.iter().copied().min().unwrap_or(0);
    let deltas: Vec<u64> = digits.iter().map(|&d| (d as i128 - base as i128) as u64).collect();
    let bits = bits_needed(deltas.iter().copied().max().unwrap_or(0)) as u8;
    Some(Candidate { factor, exponent, base, bits, deltas })
}

/// A codec that stores a float block as decimal digits and a scale.
#[derive(Clone, Copy, Debug, Default)]
pub struct AlpCodec;

impl AlpCodec {
    /// The smallest framing that rebuilds every value bit for bit.
    ///
    /// Every factor and exponent pair is tried. The candidate with the fewest
    /// bytes that passes the exactness check wins. A tie keeps the framing that
    /// was tried first, so the choice is deterministic.
    fn best_frame(values: &[f32]) -> Option<Candidate> {
        if values.iter().any(|v| !v.is_finite()) {
            return None;
        }
        let mut best: Option<Candidate> = None;
        for factor in FACTORS {
            for exponent in 0..=MAX_EXPONENT {
                let Some(candidate) = frame(values, factor, exponent) else {
                    continue;
                };
                if !candidate.exact(values) {
                    continue;
                }
                let better = match &best {
                    None => true,
                    Some(current) => candidate.size() < current.size(),
                };
                if better {
                    best = Some(candidate);
                }
            }
        }
        best
    }
}

impl ColumnCodec for AlpCodec {
    fn id(&self) -> u8 {
        ALP_ID
    }

    fn name(&self) -> &'static str {
        "alp"
    }

    fn accepts(&self, ty: ColumnType) -> bool {
        ty == ColumnType::F32
    }

    fn encode(&self, data: &ColumnData<'_>) -> Result<Vec<u8>, CodecError> {
        let ColumnData::F32(values) = data else {
            return Err(CodecError::UnsupportedType(data.column_type().name()));
        };
        if values.is_empty() {
            let mut out = Vec::with_capacity(HEADER_LEN);
            out.extend_from_slice(&0u32.to_le_bytes());
            out.push(1);
            out.push(0);
            out.extend_from_slice(&0i64.to_le_bytes());
            out.push(0);
            return Ok(out);
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(CodecError::NotApplicable("a value is not a finite decimal"));
        }
        let candidate = Self::best_frame(values).ok_or(CodecError::NotApplicable(
            "no decimal framing rebuilds the block bit for bit",
        ))?;

        let mut out = Vec::with_capacity(candidate.size());
        out.extend_from_slice(&(values.len() as u32).to_le_bytes());
        out.push(candidate.factor);
        out.push(candidate.exponent);
        out.extend_from_slice(&candidate.base.to_le_bytes());
        out.push(candidate.bits);
        pack(&candidate.deltas, candidate.bits as u32, &mut out);

        // Verify the exact round trip before the bytes leave the encoder.
        let decoded = self.decode(&out)?;
        let restored = decoded.as_f32()?;
        let exact = restored.len() == values.len()
            && restored.iter().zip(values.iter()).all(|(a, b)| a.to_bits() == b.to_bits());
        if !exact {
            return Err(CodecError::NotApplicable("the round trip is not bit exact"));
        }
        Ok(out)
    }

    fn decode(&self, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
        let count = take_u32(bytes, 0)? as usize;
        if bytes.len() < HEADER_LEN {
            return Err(CodecError::Corrupt("truncated alp header"));
        }
        let factor = bytes[4];
        let exponent = bytes[5];
        if factor == 0 || exponent > MAX_EXPONENT {
            return Err(CodecError::Corrupt("bad alp scale"));
        }
        let base = i64::from_le_bytes(bytes[6..14].try_into().unwrap());
        let bits = bytes[14] as u32;
        let deltas = unpack(&bytes[HEADER_LEN..], count, bits)?;

        let scale = factor as f64 * 10f64.powi(exponent as i32);
        let values = deltas
            .iter()
            .map(|&d| ((base as i128 + d as i128) as f64 / scale) as f32)
            .collect();
        Ok(DecodedColumn::F32(values))
    }

    fn estimate(&self, data: &ColumnData<'_>) -> usize {
        match data {
            ColumnData::F32(values) => match Self::best_frame(values) {
                Some(candidate) => candidate.size(),
                None => HEADER_LEN,
            },
            _ => HEADER_LEN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(values: &[f32]) -> Vec<f32> {
        let codec = AlpCodec;
        let bytes = codec.encode(&ColumnData::F32(values)).expect("encode");
        let decoded = codec.decode(&bytes).expect("decode");
        decoded.as_f32().expect("f32").to_vec()
    }

    #[test]
    #[allow(clippy::approx_constant)] // 3.14159 is a decimal input, not the PI constant.
    fn decimal_values_round_trip_exactly() {
        let values = vec![0.1f32, 0.2, 3.14159, -2.5, 1e-6, 1234.5];
        let got = round_trip(&values);
        for (a, b) in values.iter().zip(got.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn a_money_column_packs_to_a_few_bits_per_value() {
        let codec = AlpCodec;
        // A money column holds whole cents. The value is the correctly rounded
        // quotient, so a scale of one hundred rebuilds every cent exactly.
        let values: Vec<f32> = (0..1000).map(|cents| (1000 + cents) as f32 / 100.0).collect();
        let bytes = codec.encode(&ColumnData::F32(&values)).expect("encode");
        assert!(
            bytes.len() < values.len() * 4 / 2,
            "the block must be smaller than raw: {} vs {}",
            bytes.len(),
            values.len() * 4
        );
        let got = codec.decode(&bytes).expect("decode");
        let got = got.as_f32().expect("f32");
        for (a, b) in values.iter().zip(got.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn zero_and_integers_round_trip() {
        let got = round_trip(&[0.0, 1.0, 2.0, -3.0]);
        assert_eq!(got, vec![0.0, 1.0, 2.0, -3.0]);
    }

    #[test]
    fn a_non_finite_value_is_declined() {
        let codec = AlpCodec;
        for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let values = vec![1.0f32, special];
            assert_eq!(
                codec.encode(&ColumnData::F32(&values)),
                Err(CodecError::NotApplicable("a value is not a finite decimal"))
            );
        }
    }

    #[test]
    fn negative_zero_is_declined_because_the_bit_pattern_differs() {
        let codec = AlpCodec;
        let values = vec![-0.0f32, 1.0];
        // Every framing rebuilds -0.0 as +0.0, so no framing is bit exact.
        assert_eq!(
            codec.encode(&ColumnData::F32(&values)),
            Err(CodecError::NotApplicable(
                "no decimal framing rebuilds the block bit for bit"
            ))
        );
    }

    #[test]
    fn an_empty_block_round_trips() {
        let codec = AlpCodec;
        let bytes = codec.encode(&ColumnData::F32(&[])).expect("encode");
        assert!(codec.decode(&bytes).expect("decode").is_empty());
    }

    #[test]
    fn a_bad_scale_is_rejected() {
        let codec = AlpCodec;
        let values = vec![1.0f32, 2.0];
        let mut bytes = codec.encode(&ColumnData::F32(&values)).expect("encode");
        bytes[4] = 0;
        assert_eq!(codec.decode(&bytes), Err(CodecError::Corrupt("bad alp scale")));
    }

    #[test]
    fn codec_rejects_an_integer_block() {
        let codec = AlpCodec;
        let values = vec![1u64];
        assert_eq!(
            codec.encode(&ColumnData::U64(&values)),
            Err(CodecError::UnsupportedType("u64"))
        );
    }
}
