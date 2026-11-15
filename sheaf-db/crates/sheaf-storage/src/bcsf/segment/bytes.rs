// bytes.rs - little-endian byte conversion for the segment format.
//
// One place owns the wire encoding. The writer and both readers call these
// helpers, so an encoding change cannot drift between them.

/// Encode a u32 slice as little-endian bytes.
pub(crate) fn u32_bytes(v: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Encode a u64 slice as little-endian bytes.
pub(crate) fn u64_bytes(v: &[u64]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 8);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Encode an f32 slice as little-endian bytes.
pub(crate) fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// Decode little-endian u32 values. A trailing partial word is ignored, which
/// matches the legacy reader behaviour.
pub(crate) fn u32s(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect()
}

/// Decode little-endian u64 values.
pub(crate) fn u64s(b: &[u8]) -> Vec<u64> {
    b.as_chunks::<8>().0.iter().map(|c| u64::from_le_bytes(*c)).collect()
}

/// Decode little-endian f32 values.
pub(crate) fn f32s(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect()
}
