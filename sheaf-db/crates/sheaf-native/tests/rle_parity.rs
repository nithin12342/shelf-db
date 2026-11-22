// rle_parity.rs - C++23 codec parity gate (C-03).
//
// The test calls the production port with a 1,000-value block. It compares the
// native bytes with an independent Rust encoder and checks the round trip.

use sheaf_native::codec::{rle_decode_u64, rle_encode_u64};
use sheaf_native::native_kernel_linked;

fn reference(values: &[u64]) -> Vec<u8> {
    let mut runs: Vec<(u32, u64)> = Vec::new();
    for &value in values {
        match runs.last_mut() {
            Some((length, last)) if *last == value => *length += 1,
            _ => runs.push((1, value)),
        }
    }
    let mut out = Vec::new();
    out.push(0u8);
    out.extend_from_slice(&(values.len() as u32).to_le_bytes());
    out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
    for (length, value) in runs {
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

#[test]
fn one_thousand_values_match_the_reference_and_round_trip() {
    assert!(native_kernel_linked(), "the C++23 kernel must be linked");
    let mut values = Vec::with_capacity(1000);
    for index in 0..1000u64 {
        values.push(index / 7);
    }
    let (encoded, source) = rle_encode_u64(&values).expect("encode");
    assert_eq!(source, "cpp23");
    assert_eq!(encoded, reference(&values));
    let (decoded, source) = rle_decode_u64(&encoded, values.len()).expect("decode");
    assert_eq!(source, "cpp23");
    assert_eq!(decoded, values);
    assert!(rle_decode_u64(&encoded[..encoded.len() - 1], values.len()).is_err());
}
