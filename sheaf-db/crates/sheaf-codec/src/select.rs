// select.rs - the codec selector (MF-05).
//
// The selector measures every codec that accepts the block type and keeps the
// smallest result. Ties go to the lower codec id, because the comparison is
// strict and the registry is in ascending id order. No codec is assumed to win.
//
// A codec that declines the block is not an error. Its reason is recorded on
// the choice, so a reader can see why the block went to another codec.

use crate::alp::AlpCodec;
use crate::bitpack::BitPackCodec;
use crate::error::CodecError;
use crate::for_::FrameOfReferenceCodec;
use crate::fsst::FsstCodec;
use crate::raw::RawCodec;
use crate::traits::{ColumnCodec, ColumnData, ColumnType, DecodedColumn};

static RAW: RawCodec = RawCodec;
static BITPACK: BitPackCodec = BitPackCodec;
static FOR: FrameOfReferenceCodec = FrameOfReferenceCodec;
static ALP: AlpCodec = AlpCodec;
static FSST: FsstCodec = FsstCodec;

/// The registry, in ascending id order.
static REGISTRY: [&'static dyn ColumnCodec; 5] = [&RAW, &BITPACK, &FOR, &ALP, &FSST];

/// Every registered codec, in ascending id order.
pub fn codecs() -> &'static [&'static dyn ColumnCodec] {
    &REGISTRY
}

/// Find a codec by id. An unknown id is an error, never a guess.
pub fn codec_by_id(id: u8) -> Result<&'static dyn ColumnCodec, CodecError> {
    REGISTRY
        .iter()
        .find(|codec| codec.id() == id)
        .copied()
        .ok_or(CodecError::UnknownCodec(id))
}

/// Decode a block with the codec that the id names.
pub fn decode(id: u8, bytes: &[u8]) -> Result<DecodedColumn, CodecError> {
    codec_by_id(id)?.decode(bytes)
}

/// The winner of a selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CodecChoice {
    /// The id of the winning codec.
    pub codec_id: u8,
    /// The name of the winning codec.
    pub codec_name: &'static str,
    /// The encoded block.
    pub encoded: Vec<u8>,
    /// The raw size of the block with no encoding.
    pub raw_bytes: usize,
    /// The codecs that declined the block, with the reason.
    pub rejected: Vec<(u8, String)>,
}

impl CodecChoice {
    /// The encoded size in bytes.
    pub fn encoded_bytes(&self) -> usize {
        self.encoded.len()
    }

    /// The bytes saved against the raw form. A negative value means the raw
    /// form is smaller, which happens when every codec adds a header.
    pub fn saved_bytes(&self) -> i64 {
        self.raw_bytes as i64 - self.encoded.len() as i64
    }
}

/// Encode a block with every codec that accepts its type and keep the smallest
/// result. Returns the smallest choice, or the reason when no codec accepts it.
pub fn select(data: &ColumnData<'_>) -> Result<CodecChoice, CodecError> {
    let mut best: Option<CodecChoice> = None;
    let mut rejected: Vec<(u8, String)> = Vec::new();

    for codec in REGISTRY.iter() {
        if !codec.accepts(data.column_type()) {
            continue;
        }
        match codec.encode(data) {
            Ok(encoded) => {
                let candidate = CodecChoice {
                    codec_id: codec.id(),
                    codec_name: codec.name(),
                    encoded,
                    raw_bytes: data.raw_bytes(),
                    rejected: Vec::new(),
                };
                let better = match &best {
                    None => true,
                    // The strict comparison keeps the lower id on a tie.
                    Some(current) => candidate.encoded.len() < current.encoded.len(),
                };
                if better {
                    best = Some(candidate);
                }
            }
            Err(err) => rejected.push((codec.id(), err.to_string())),
        }
    }

    match best {
        Some(mut choice) => {
            choice.rejected = rejected;
            Ok(choice)
        }
        None => Err(CodecError::NotApplicable("no registered codec accepts the block")),
    }
}

/// The codecs that accept a type, in ascending id order.
pub fn codecs_for(ty: ColumnType) -> Vec<u8> {
    REGISTRY
        .iter()
        .filter(|codec| codec.accepts(ty))
        .map(|codec| codec.id())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_is_in_ascending_id_order() {
        let ids: Vec<u8> = codecs().iter().map(|c| c.id()).collect();
        assert_eq!(ids, vec![0, 1, 2, 3, 4]);
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn an_unknown_id_is_rejected() {
        assert_eq!(
            codec_by_id(200).err(),
            Some(CodecError::UnknownCodec(200))
        );
        assert_eq!(decode(200, &[0]).err(), Some(CodecError::UnknownCodec(200)));
    }

    #[test]
    fn a_narrow_u64_block_goes_to_the_smallest_codec() {
        let values: Vec<u64> = (1_000..2_000).collect();
        let choice = select(&ColumnData::U64(&values)).expect("select");
        // Ten bits per value beats the raw eight bytes per value.
        assert_eq!(choice.codec_name, "for");
        assert!(choice.saved_bytes() > 0);
        let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
        assert_eq!(decoded.as_u64().expect("u64"), values.as_slice());
    }

    #[test]
    fn a_spread_u64_block_stays_raw() {
        let values = vec![0u64, u64::MAX, 7, 1 << 40];
        let choice = select(&ColumnData::U64(&values)).expect("select");
        assert_eq!(choice.codec_name, "raw");
        assert_eq!(choice.encoded_bytes(), choice.raw_bytes + 5);
        let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
        assert_eq!(decoded.as_u64().expect("u64"), values.as_slice());
    }

    #[test]
    fn a_decimal_block_goes_to_alp() {
        let values: Vec<f32> = (0..500).map(|cents| (10_000 + cents) as f32 / 100.0).collect();
        let choice = select(&ColumnData::F32(&values)).expect("select");
        assert_eq!(choice.codec_name, "alp");
        let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
        let got = decoded.as_f32().expect("f32");
        for (a, b) in values.iter().zip(got.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn a_non_finite_float_block_records_the_alp_reason() {
        let values = vec![f32::NAN, 1.0];
        let choice = select(&ColumnData::F32(&values)).expect("select");
        assert_eq!(choice.codec_name, "raw");
        assert!(
            choice.rejected.iter().any(|(id, reason)| *id == 3 && reason.contains("declined")),
            "the alp reason must be recorded"
        );
    }

    #[test]
    fn a_repeated_string_block_goes_to_fsst() {
        let phrase = b"sheaf database column store ".to_vec();
        let values: Vec<Vec<u8>> = (0..100).map(|_| phrase.clone()).collect();
        let choice = select(&ColumnData::Bytes(&values)).expect("select");
        assert_eq!(choice.codec_name, "fsst");
        assert!(choice.saved_bytes() > 0);
        let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
        assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
    }

    #[test]
    fn accepted_types_are_reported_per_codec() {
        assert_eq!(codecs_for(ColumnType::U64), vec![0, 1, 2]);
        assert_eq!(codecs_for(ColumnType::I64), vec![0, 2]);
        assert_eq!(codecs_for(ColumnType::F32), vec![0, 3]);
        assert_eq!(codecs_for(ColumnType::Bytes), vec![0, 4]);
    }
}
