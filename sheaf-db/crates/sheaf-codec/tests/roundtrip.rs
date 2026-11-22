// roundtrip.rs - integration test for every codec and the selector (MF-05).
//
// The test drives the production path. It calls select, which measures every
// codec that accepts the block, and then decode, which is the same entry point
// that a reader uses. Each case states its expected result: the decoded block
// must equal the input, byte for byte.
//
// The random generator is a local xorshift64* so the crate keeps zero external
// dependencies. The seed is fixed, so a failure is repeatable.

use sheaf_codec::fsst::{train, FsstCodec, MAX_SYMBOLS};
use sheaf_codec::{
    codec_by_id, decode, select, CodecError, ColumnCodec, ColumnData, ColumnType, DecodedColumn,
    RLE_ID,
};

/// The number of random values per type.
const COUNT: usize = 100_000;

/// A xorshift64* generator. It is deterministic and needs no crate.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn below(&mut self, bound: usize) -> usize {
        (self.next_u64() % bound as u64) as usize
    }
}

/// Encode through the selector and decode through the registry.
fn round_trip(data: &ColumnData<'_>) -> DecodedColumn {
    let choice = select(data).expect("the selector must accept a supported block");
    let codec = codec_by_id(choice.codec_id).expect("the chosen id must be registered");
    assert_eq!(codec.id(), choice.codec_id);
    assert_eq!(
        choice.encoded.len(),
        choice.encoded_bytes(),
        "the reported size must match the buffer"
    );
    decode(choice.codec_id, &choice.encoded).expect("the chosen codec must decode its own buffer")
}

#[test]
fn one_hundred_thousand_u64_round_trip() {
    let mut rng = Rng::new(0x5eed_0001);
    let mut values: Vec<u64> = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        values.push(match index % 5 {
            // A narrow block, a wide block, and a full-range block.
            0 => rng.next_u64() % 128,
            1 => 1_000_000 + rng.next_u64() % 4096,
            2 => rng.next_u64(),
            3 => u64::MAX - rng.next_u64() % 1024,
            _ => rng.next_u64() >> rng.below(64),
        });
    }

    let decoded = round_trip(&ColumnData::U64(&values));
    assert_eq!(
        decoded.column_type(),
        ColumnType::U64,
        "the decoded type must match the input type"
    );
    assert_eq!(decoded.as_u64().expect("u64 values"), values.as_slice());
}

#[test]
fn one_hundred_thousand_i64_round_trip() {
    let mut rng = Rng::new(0x5eed_0002);
    let mut values: Vec<i64> = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        values.push(match index % 5 {
            0 => (rng.next_u64() % 128) as i64 - 64,
            1 => -1_000_000 - (rng.next_u64() % 4096) as i64,
            2 => rng.next_u64() as i64,
            3 => i64::MIN + (rng.next_u64() % 1024) as i64,
            _ => i64::MAX - (rng.next_u64() % 1024) as i64,
        });
    }

    let decoded = round_trip(&ColumnData::I64(&values));
    assert_eq!(decoded.column_type(), ColumnType::I64);
    assert_eq!(decoded.as_i64().expect("i64 values"), values.as_slice());
}

#[test]
fn one_hundred_thousand_f32_round_trip_at_bit_level() {
    let mut rng = Rng::new(0x5eed_0003);
    let mut values: Vec<f32> = Vec::with_capacity(COUNT);
    for _ in 0..COUNT {
        // The raw bits include NaN, infinity, zero, and subnormal values.
        values.push(f32::from_bits(rng.next_u64() as u32));
    }

    let decoded = round_trip(&ColumnData::F32(&values));
    assert_eq!(decoded.column_type(), ColumnType::F32);
    let got = decoded.as_f32().expect("f32 values");
    assert_eq!(got.len(), values.len());
    for (index, (a, b)) in values.iter().zip(got.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "value {index} differs at bit level");
    }
}

#[test]
fn one_hundred_thousand_decimal_f32_round_trip_through_alp() {
    // Whole cents are the case that ALP is built for. The selector must choose
    // it, and it must rebuild every value bit for bit.
    let values: Vec<f32> = (0..COUNT)
        .map(|cents| (10_000 + cents as u64) as f32 / 100.0)
        .collect();
    let choice = select(&ColumnData::F32(&values)).expect("select");
    assert_eq!(choice.codec_name, "alp", "a cents column must choose alp");
    let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
    let got = decoded.as_f32().expect("f32 values");
    for (index, (a, b)) in values.iter().zip(got.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "value {index} differs at bit level");
    }
}

#[test]
fn one_hundred_thousand_strings_round_trip() {
    let mut rng = Rng::new(0x5eed_0004);
    let alphabet = b"sheafdb ";
    let mut values: Vec<Vec<u8>> = Vec::with_capacity(COUNT);
    for index in 0..COUNT {
        let len = match index % 4 {
            0 => 0,
            1 => rng.below(8),
            2 => rng.below(64),
            _ => 256 + rng.below(1024),
        };
        let mut text = Vec::with_capacity(len);
        for _ in 0..len {
            text.push(alphabet[rng.below(alphabet.len())]);
        }
        values.push(text);
    }

    let decoded = round_trip(&ColumnData::Bytes(&values));
    assert_eq!(decoded.column_type(), ColumnType::Bytes);
    assert_eq!(decoded.as_bytes().expect("string values").to_vec(), values);
}

#[test]
fn edge_values_round_trip_for_every_type() {
    let u64_values = vec![0u64, 1, u64::MAX, u64::MAX - 1, 1 << 63, 7];
    let decoded = round_trip(&ColumnData::U64(&u64_values));
    assert_eq!(decoded.as_u64().expect("u64"), u64_values.as_slice());

    let i64_values = vec![0i64, 1, -1, i64::MIN, i64::MAX, i64::MAX - 1];
    let decoded = round_trip(&ColumnData::I64(&i64_values));
    assert_eq!(decoded.as_i64().expect("i64"), i64_values.as_slice());

    let f32_values = vec![
        0.0f32,
        -0.0,
        f32::MIN,
        f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::from_bits(0x7fc0_0001),
        f32::from_bits(0xffc0_0001),
    ];
    let decoded = round_trip(&ColumnData::F32(&f32_values));
    let got = decoded.as_f32().expect("f32");
    for (index, (a, b)) in f32_values.iter().zip(got.iter()).enumerate() {
        assert_eq!(a.to_bits(), b.to_bits(), "edge value {index} differs");
    }

    let string_values = vec![
        Vec::new(),
        vec![0u8],
        vec![0xffu8; 4096],
        b"the quick brown fox jumps over the lazy dog".to_vec(),
        b"the quick brown fox jumps over the lazy dog".to_vec(),
    ];
    let decoded = round_trip(&ColumnData::Bytes(&string_values));
    assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), string_values);
}

#[test]
fn a_repeated_corpus_keeps_the_table_under_the_cap_and_the_block_under_raw() {
    let phrase = b"the quick brown fox jumps over the lazy dog ";
    let values: Vec<Vec<u8>> = (0..1000).map(|_| phrase.to_vec()).collect();
    let table = train(&values);
    assert!(table.len() <= MAX_SYMBOLS, "the table must stay under 255 symbols");
    assert!(!table.is_empty(), "a repeated corpus must train at least one symbol");

    let codec = FsstCodec;
    let raw = ColumnData::Bytes(&values).raw_bytes();
    let encoded = codec.encode(&ColumnData::Bytes(&values)).expect("encode");
    assert!(
        encoded.len() <= raw,
        "fsst must not grow a repeated corpus: {} vs {}",
        encoded.len(),
        raw
    );
    let decoded = codec.decode(&encoded).expect("decode");
    assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
}

#[test]
fn every_unknown_codec_id_is_reported() {
    for id in [6u8, 77, 200, 255] {
        assert_eq!(
            codec_by_id(id).err(),
            Some(CodecError::UnknownCodec(id)),
            "id {id} must be rejected"
        );
        assert_eq!(
            decode(id, &[0, 0, 0, 0]).err(),
            Some(CodecError::UnknownCodec(id))
        );
    }
}

#[test]
fn a_constant_block_selects_the_run_length_codec() {
    let values: Vec<Vec<u8>> = (0..50_000).map(|_| b"same".to_vec()).collect();
    let choice = select(&ColumnData::Bytes(&values)).expect("select");
    assert_eq!(choice.codec_id, RLE_ID);
    assert_eq!(choice.codec_name, "rle");
    assert_eq!(choice.encoded_bytes(), 9 + 4 + 4 + 4);
    let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
    assert_eq!(decoded.as_bytes().expect("bytes").to_vec(), values);
}

#[test]
fn a_constant_numeric_block_stays_smaller_under_frame_of_reference() {
    let values = vec![42u64; 50_000];
    let choice = select(&ColumnData::U64(&values)).expect("select");
    assert_eq!(choice.codec_id, 2);
    assert_eq!(choice.codec_name, "for");
    assert!(
        choice.encoded_bytes() < 21,
        "frame of reference stores a zero bit width, so it beats run-length here"
    );
    let decoded = decode(choice.codec_id, &choice.encoded).expect("decode");
    assert_eq!(decoded.as_u64().expect("u64"), values.as_slice());
}

#[test]
fn every_codec_rejects_a_truncated_buffer() {
    for codec in sheaf_codec::codecs() {
        let empty: Result<DecodedColumn, CodecError> = codec.decode(&[]);
        assert!(
            empty.is_err(),
            "codec {} must reject an empty buffer",
            codec.name()
        );
    }
}
