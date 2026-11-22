// Timestamp: 2026-11-22_11-53-08 +05:30
// frame.rs - replication frames with a checksum (NF-05).
//
// Frame layout, all little-endian:
//   sequence u64 | cell u64 | length u32 | checksum u32 | patch bytes
// A frame over the limit, a truncated frame, or a checksum mismatch is an
// explicit error. No partial frame is ever returned as a delta.

use super::port::{ReplicationError, Result};
use super::SectionDelta;

/// The largest accepted patch. Deliberately small for this tier.
pub const MAX_PATCH_BYTES: usize = 1024 * 1024;

const HEADER: usize = 8 + 8 + 4 + 4;

/// A CRC32 over the patch bytes. The polynomial is the reflected 0xEDB88320.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Encode a delta as one frame.
pub fn encode(delta: &SectionDelta) -> Result<Vec<u8>> {
    if delta.xor_patch.len() > MAX_PATCH_BYTES {
        return Err(ReplicationError::Framing("patch exceeds the frame limit".into()));
    }
    let mut out = Vec::with_capacity(HEADER + delta.xor_patch.len());
    out.extend_from_slice(&delta.sequence.to_le_bytes());
    out.extend_from_slice(&delta.cell_id.to_le_bytes());
    out.extend_from_slice(&(delta.xor_patch.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc32(&delta.xor_patch).to_le_bytes());
    out.extend_from_slice(&delta.xor_patch);
    Ok(out)
}

/// Decode one frame. A short buffer or a bad checksum is an explicit error.
pub fn decode(frame: &[u8]) -> Result<SectionDelta> {
    if frame.len() < HEADER {
        return Err(ReplicationError::Framing("frame is shorter than its header".into()));
    }
    let sequence = u64::from_le_bytes(frame[0..8].try_into().unwrap());
    let cell_id = u64::from_le_bytes(frame[8..16].try_into().unwrap());
    let length = u32::from_le_bytes(frame[16..20].try_into().unwrap()) as usize;
    let checksum = u32::from_le_bytes(frame[20..24].try_into().unwrap());
    if length > MAX_PATCH_BYTES {
        return Err(ReplicationError::Framing("patch exceeds the frame limit".into()));
    }
    let end = HEADER.checked_add(length).ok_or_else(|| {
        ReplicationError::Framing("frame length overflow".into())
    })?;
    if frame.len() != end {
        return Err(ReplicationError::Framing("frame length does not match its header".into()));
    }
    let patch = &frame[HEADER..end];
    if crc32(patch) != checksum {
        return Err(ReplicationError::Framing("frame checksum mismatch".into()));
    }
    Ok(SectionDelta::new(sequence, cell_id, patch.to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_delta_round_trips() {
        let delta = SectionDelta::new(7, 42, vec![1, 2, 3, 255]);
        let bytes = encode(&delta).expect("encode");
        assert_eq!(bytes.len(), HEADER + 4);
        assert_eq!(decode(&bytes).expect("decode"), delta);
    }

    #[test]
    fn a_truncated_frame_is_rejected() {
        let delta = SectionDelta::new(7, 42, vec![1, 2, 3, 255]);
        let bytes = encode(&delta).expect("encode");
        assert_eq!(
            decode(&bytes[..bytes.len() - 1]).err(),
            Some(ReplicationError::Framing("frame length does not match its header".into()))
        );
    }

    #[test]
    fn a_corrupt_patch_is_rejected() {
        let delta = SectionDelta::new(7, 42, vec![1, 2, 3, 255]);
        let mut bytes = encode(&delta).expect("encode");
        *bytes.last_mut().unwrap() ^= 1;
        assert_eq!(
            decode(&bytes).err(),
            Some(ReplicationError::Framing("frame checksum mismatch".into()))
        );
    }
}
