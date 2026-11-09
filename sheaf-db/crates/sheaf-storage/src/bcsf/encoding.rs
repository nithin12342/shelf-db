// crates/sheaf-storage/src/bcsf/encoding.rs — delta + varint encoding
// for sorted boundary indices and monotonic cell offsets.
//
// Sorted boundary indices (u64) and monotonic offsets (u32) compress well
// with delta encoding: store the first value absolute, then each subsequent
// value as (cur - prev). The deltas are small and encode in few varint
// bytes. For an index with N entries averaging gap G, raw storage is
// N * 8 bytes; delta+varint is roughly N * ceil(log2(G)/7) bytes, often
// 2-5x smaller.
//
// Layout (delta+varint block):
//   [count u64LE][first u64LE (absolute)][varint deltas...]
//
// Varint: LEB128, 7 bits per byte, continuation bit in MSB.

/// Encode a slice of u64 values as LEB128 varints into `out`.
pub fn encode_varint_u64(values: &[u64], out: &mut Vec<u8>) {
    for &v in values {
        let mut x = v;
        loop {
            let mut byte = (x & 0x7F) as u8;
            x >>= 7;
            if x != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if x == 0 {
                break;
            }
        }
    }
}

/// Decode LEB128 varints from `data` starting at `pos`, writing count values
/// into `out`. Returns the new position.
pub fn decode_varint_u64(data: &[u8], mut pos: usize, count: usize, out: &mut Vec<u64>) -> Result<usize, String> {
    for _ in 0..count {
        if pos >= data.len() {
            return Err("varint: truncated".to_string());
        }
        let mut val: u64 = 0;
        let mut shift: u32 = 0;
        loop {
            if pos >= data.len() {
                return Err("varint: truncated".to_string());
            }
            let byte = data[pos];
            pos += 1;
            val |= ((byte & 0x7F) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
            shift += 7;
            if shift > 63 {
                return Err("varint: overflow".to_string());
            }
        }
        out.push(val);
    }
    Ok(pos)
}

/// Encode sorted u64 values as delta+varint. First value is absolute.
pub fn encode_delta_u64(sorted: &[u64], out: &mut Vec<u8>) {
    out.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
    if sorted.is_empty() {
        return;
    }
    out.extend_from_slice(&sorted[0].to_le_bytes());
    let mut prev = sorted[0];
    let mut deltas: Vec<u64> = Vec::with_capacity(sorted.len() - 1);
    for &v in sorted.iter().skip(1) {
        deltas.push(v - prev);
        prev = v;
    }
    encode_varint_u64(&deltas, out);
}

/// Decode delta+varint block from `data` starting at `pos`. Returns the
/// decoded values and the new position.
pub fn decode_delta_u64(data: &[u8], mut pos: usize) -> Result<(Vec<u64>, usize), String> {
    if pos + 8 > data.len() {
        return Err("delta: truncated count".to_string());
    }
    let count = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
    pos += 8;
    let mut out: Vec<u64> = Vec::with_capacity(count);
    if count == 0 {
        return Ok((out, pos));
    }
    if pos + 8 > data.len() {
        return Err("delta: truncated first".to_string());
    }
    let first = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
    pos += 8;
    out.push(first);
    if count == 1 {
        return Ok((out, pos));
    }
    let mut deltas: Vec<u64> = Vec::with_capacity(count - 1);
    pos = decode_varint_u64(data, pos, count - 1, &mut deltas)?;
    let mut prev = first;
    for &d in &deltas {
        prev = prev.wrapping_add(d);
        out.push(prev);
    }
    Ok((out, pos))
}

/// Encode monotonic u32 offsets as delta+varint. First value absolute.
pub fn encode_delta_u32(sorted: &[u32], out: &mut Vec<u8>) {
    out.extend_from_slice(&(sorted.len() as u64).to_le_bytes());
    if sorted.is_empty() {
        return;
    }
    out.extend_from_slice(&(sorted[0] as u32).to_le_bytes());
    let mut prev = sorted[0];
    let mut deltas: Vec<u64> = Vec::with_capacity(sorted.len() - 1);
    for &v in sorted.iter().skip(1) {
        deltas.push(v as u64 - prev as u64);
        prev = v;
    }
    encode_varint_u64(&deltas, out);
}

/// Decode delta+varint u32 block.
pub fn decode_delta_u32(data: &[u8], mut pos: usize) -> Result<(Vec<u32>, usize), String> {
    if pos + 8 > data.len() {
        return Err("delta u32: truncated count".to_string());
    }
    let count = u64::from_le_bytes(data[pos..pos + 8].try_into().unwrap()) as usize;
    pos += 8;
    let mut out: Vec<u32> = Vec::with_capacity(count);
    if count == 0 {
        return Ok((out, pos));
    }
    if pos + 4 > data.len() {
        return Err("delta u32: truncated first".to_string());
    }
    let first = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
    pos += 4;
    out.push(first);
    if count == 1 {
        return Ok((out, pos));
    }
    let mut deltas: Vec<u64> = Vec::with_capacity(count - 1);
    pos = decode_varint_u64(data, pos, count - 1, &mut deltas)?;
    let mut prev: u64 = first as u64;
    for &d in &deltas {
        prev = prev.wrapping_add(d);
        out.push(prev as u32);
    }
    Ok((out, pos))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_round_trip() {
        let vals: Vec<u64> = vec![0, 1, 127, 128, 300, 10000, u32::MAX as u64, u64::MAX];
        let mut out = Vec::new();
        encode_varint_u64(&vals, &mut out);
        let mut decoded: Vec<u64> = Vec::new();
        let end = decode_varint_u64(&out, 0, vals.len(), &mut decoded).unwrap();
        assert_eq!(end, out.len());
        assert_eq!(decoded, vals);
    }

    #[test]
    fn delta_u64_round_trip() {
        let sorted: Vec<u64> = vec![100, 105, 107, 200, 201, 1000, u64::MAX];
        let mut out = Vec::new();
        encode_delta_u64(&sorted, &mut out);
        let (decoded, end) = decode_delta_u64(&out, 0).unwrap();
        assert_eq!(end, out.len());
        assert_eq!(decoded, sorted);
    }

    #[test]
    fn delta_u32_round_trip() {
        let sorted: Vec<u32> = vec![0, 10, 20, 100, 1000, u32::MAX];
        let mut out = Vec::new();
        encode_delta_u32(&sorted, &mut out);
        let (decoded, end) = decode_delta_u32(&out, 0).unwrap();
        assert_eq!(end, out.len());
        assert_eq!(decoded, sorted);
    }

    #[test]
    fn delta_empty() {
        let mut out = Vec::new();
        encode_delta_u64(&[], &mut out);
        let (decoded, end) = decode_delta_u64(&out, 0).unwrap();
        assert_eq!(end, out.len());
        assert!(decoded.is_empty());
    }
}
