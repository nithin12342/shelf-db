use crate::wal::{crc32c::crc32c, WalStats, MAX_PAYLOAD};
use std::fs::File;
use std::io::Read;
use std::path::Path;

pub fn replay(
    path: &Path,
    stats: WalStats,
    mut visit: impl FnMut(u64, &[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut offset = 0u64;
    let mut sequence = 0u64;
    let mut previous_epoch = 0;
    while offset < stats.valid_end {
        if stats.valid_end - offset < 32 {
            return Err("short committed log header".into());
        }
        let mut header = [0u8; 32];
        file.read_exact(&mut header).map_err(|e| e.to_string())?;
        let u32_at = |at| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
        let u64_at = |at| u64::from_le_bytes(header[at..at + 8].try_into().unwrap());
        let epoch = u64_at(12);
        let len = u32_at(20) as usize;
        if u32_at(0) != 0x4c57_4853
            || u32_at(28) != crc32c(&header[..28])
            || u64_at(4) != sequence
            || len > MAX_PAYLOAD
            || epoch < previous_epoch
            || epoch > stats.epoch
            || 32 + len as u64 > stats.valid_end - offset
        {
            return Err("invalid committed log record".into());
        }
        let mut payload = vec![0; len];
        file.read_exact(&mut payload).map_err(|e| e.to_string())?;
        if crc32c(&payload) != u32_at(24) {
            return Err("committed log payload checksum mismatch".into());
        }
        visit(epoch, &payload)?;
        previous_epoch = epoch;
        sequence += 1;
        offset += 32 + len as u64;
    }
    if sequence != stats.records_recovered || (sequence > 0 && sequence - 1 != stats.last_seq) {
        return Err("committed log sequence does not match its epoch header".into());
    }
    Ok(())
}
