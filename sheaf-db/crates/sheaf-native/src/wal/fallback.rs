//! fallback - pure-Rust WAL implementing the full C11 wire format.
//!
//! This is not a stub. It writes the same 32-byte CRC32C record frames and
//! the same 4096-byte A/B epoch slots as wal-c, so a log it writes recovers
//! through the native tier and vice versa. The crash-recovery tests rely
//! on that identity. Durability uses File::sync_all at the commit boundary.

use super::{WalBackend, WalError, WalStats, MAX_PAYLOAD, SLOT_BYTES};
use crate::wal::crc32c::crc32c;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

const REC_MAGIC: u32 = 0x4C57_4853; // "SHWL"
const REC_HEADER_BYTES: u64 = 32;
const SLOT_MAGIC: u32 = 0x4445_4853; // "SHED"
const SLOT_VERSION: u32 = 1;

fn put_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}
fn get_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn put_u64(b: &mut [u8], at: usize, v: u64) {
    b[at..at + 8].copy_from_slice(&v.to_le_bytes());
}
fn get_u64(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(b[at..at + 8].try_into().unwrap())
}

fn read_at(f: &mut File, off: u64, buf: &mut [u8]) -> std::io::Result<()> {
    f.seek(SeekFrom::Start(off))?;
    f.read_exact(buf)
}
fn write_at(f: &mut File, off: u64, buf: &[u8]) -> std::io::Result<()> {
    f.seek(SeekFrom::Start(off))?;
    f.write_all(buf)
}
fn file_len(f: &File) -> std::io::Result<u64> {
    Ok(f.metadata()?.len())
}

#[derive(Clone, Copy)]
struct RecHeader {
    seq: u64,
    epoch: u64,
    payload_len: u32,
}

fn frame_encode(out: &mut [u8; 32], h: &RecHeader, payload: &[u8]) {
    put_u32(out, 0, REC_MAGIC);
    put_u64(out, 4, h.seq);
    put_u64(out, 12, h.epoch);
    put_u32(out, 20, h.payload_len);
    put_u32(out, 24, crc32c(payload));
    let hcrc = crc32c(&out[..28]);
    put_u32(out, 28, hcrc);
}

// Returns the header and the stored payload CRC, or None on corruption.
fn frame_decode(raw: &[u8; 32]) -> Option<(RecHeader, u32)> {
    if get_u32(raw, 0) != REC_MAGIC {
        return None;
    }
    if get_u32(raw, 28) != crc32c(&raw[..28]) {
        return None;
    }
    Some((
        RecHeader { seq: get_u64(raw, 4), epoch: get_u64(raw, 12), payload_len: get_u32(raw, 20) },
        get_u32(raw, 24),
    ))
}

#[derive(Clone, Copy, Default)]
struct EpochSlot {
    epoch: u64,
    valid_end: u64,
    last_seq: u64,
}

fn slot_encode(sector: &mut [u8; SLOT_BYTES], s: &EpochSlot) {
    sector.fill(0);
    put_u32(sector, 0, SLOT_MAGIC);
    put_u32(sector, 4, SLOT_VERSION);
    put_u64(sector, 8, s.epoch);
    put_u64(sector, 16, s.valid_end);
    put_u64(sector, 24, s.last_seq);
    let crc = crc32c(&sector[..32]);
    put_u32(sector, 32, crc);
}

fn slot_decode(sector: &[u8; SLOT_BYTES]) -> Option<EpochSlot> {
    if get_u32(sector, 0) != SLOT_MAGIC || get_u32(sector, 4) != SLOT_VERSION {
        return None;
    }
    if get_u32(sector, 32) != crc32c(&sector[..32]) {
        return None;
    }
    Some(EpochSlot {
        epoch: get_u64(sector, 8),
        valid_end: get_u64(sector, 16),
        last_seq: get_u64(sector, 24),
    })
}

/// Pure-Rust WAL handle. Owns one log file and one header file.
pub struct FallbackWal {
    log: File,
    header: File,
    epoch: u64,
    next_seq: u64,
    end: u64,
    next_slot: u32,
}

impl FallbackWal {
    pub fn open(log: &Path, header: &Path) -> Result<Self, WalError> {
        let log = OpenOptions::new().read(true).write(true).create(true).open(log)?;
        let header = OpenOptions::new().read(true).write(true).create(true).open(header)?;
        let mut wal = Self { log, header, epoch: 0, next_seq: 0, end: 0, next_slot: 0 };
        wal.recover()?;
        Ok(wal)
    }

    // Read both slots and pick the valid one with the highest epoch.
    fn pick_winning_slot(&mut self) -> std::io::Result<(i32, EpochSlot)> {
        let size = file_len(&self.header)?;
        let mut winner = -1i32;
        let mut best = EpochSlot::default();
        let mut sector = [0u8; SLOT_BYTES];
        for i in 0..2u64 {
            let off = i * SLOT_BYTES as u64;
            if size < off + SLOT_BYTES as u64 {
                break;
            }
            if read_at(&mut self.header, off, &mut sector).is_err() {
                continue;
            }
            if let Some(s) = slot_decode(&sector) {
                if winner < 0 || s.epoch > best.epoch {
                    best = s;
                    winner = i as i32;
                }
            }
        }
        Ok((winner, best))
    }

    // Validate one record at `offset` within `limit`. Returns its total
    // byte length, or None on corruption or a short tail.
    fn scan_record(&mut self, offset: u64, limit: u64, expected_seq: u64) -> Option<(u64, RecHeader)> {
        if offset + REC_HEADER_BYTES > limit {
            return None;
        }
        let mut raw = [0u8; 32];
        read_at(&mut self.log, offset, &mut raw).ok()?;
        let (h, payload_crc) = frame_decode(&raw)?;
        if h.seq != expected_seq || h.payload_len as usize > MAX_PAYLOAD {
            return None;
        }
        let total = REC_HEADER_BYTES + h.payload_len as u64;
        if offset + total > limit {
            return None;
        }
        let mut payload = vec![0u8; h.payload_len as usize];
        read_at(&mut self.log, offset + REC_HEADER_BYTES, &mut payload).ok()?;
        if crc32c(&payload) != payload_crc {
            return None;
        }
        Some((total, h))
    }
}

impl WalBackend for FallbackWal {
    fn name(&self) -> &'static str {
        "rust-fallback"
    }

    fn append(&mut self, epoch: u64, payload: &[u8]) -> Result<(), WalError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(WalError::InvalidArgument);
        }
        let h = RecHeader { seq: self.next_seq, epoch, payload_len: payload.len() as u32 };
        let mut raw = [0u8; 32];
        frame_encode(&mut raw, &h, payload);
        write_at(&mut self.log, self.end, &raw)?;
        if !payload.is_empty() {
            write_at(&mut self.log, self.end + REC_HEADER_BYTES, payload)?;
        }
        self.end += REC_HEADER_BYTES + payload.len() as u64;
        self.next_seq += 1;
        Ok(())
    }

    fn commit_epoch(&mut self, new_epoch: u64) -> Result<(), WalError> {
        if new_epoch <= self.epoch {
            return Err(WalError::InvalidArgument);
        }
        // Step 1: records durable before the slot names them.
        self.log.sync_all()?;
        // Step 2: write the inactive slot; the active slot stays intact.
        let slot = EpochSlot {
            epoch: new_epoch,
            valid_end: self.end,
            last_seq: if self.next_seq == 0 { 0 } else { self.next_seq - 1 },
        };
        let mut sector = [0u8; SLOT_BYTES];
        slot_encode(&mut sector, &slot);
        let off = self.next_slot as u64 * SLOT_BYTES as u64;
        write_at(&mut self.header, off, &sector)?;
        // Step 3: the barrier that makes the new epoch the winner.
        self.header.sync_all()?;
        self.epoch = new_epoch;
        self.next_slot ^= 1;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), WalError> {
        self.log.sync_all()?;
        Ok(())
    }

    fn recover(&mut self) -> Result<WalStats, WalError> {
        let mut stats = WalStats::default();
        let (winner, slot) = self.pick_winning_slot()?;
        let log_size = file_len(&self.log)?;

        if winner < 0 {
            if log_size > 0 {
                return Err(WalError::Corrupt);
            }
            self.epoch = 0;
            self.next_seq = 0;
            self.end = 0;
            self.next_slot = 0;
            return Ok(stats);
        }

        // Committed bytes must exist; the slot was written after the flush.
        if log_size < slot.valid_end {
            return Err(WalError::Corrupt);
        }
        let mut offset = 0u64;
        let mut count = 0u64;
        let mut last_seq = 0u64;
        while offset < slot.valid_end {
            let (total, h) = self.scan_record(offset, slot.valid_end, count).ok_or(WalError::Corrupt)?;
            last_seq = h.seq;
            offset += total;
            count += 1;
        }
        if count > 0 && last_seq != slot.last_seq {
            return Err(WalError::Corrupt);
        }
        if count == 0 && (slot.last_seq != 0 || slot.valid_end != 0) {
            return Err(WalError::Corrupt);
        }

        stats.truncated_bytes = log_size - slot.valid_end;
        if stats.truncated_bytes > 0 {
            self.log.set_len(slot.valid_end)?;
            self.log.sync_all()?;
        }

        self.epoch = slot.epoch;
        self.next_seq = count;
        self.end = slot.valid_end;
        self.next_slot = if winner == 0 { 1 } else { 0 };

        stats.epoch = slot.epoch;
        stats.last_seq = if count > 0 { last_seq } else { 0 };
        stats.valid_end = slot.valid_end;
        stats.records_recovered = count;
        Ok(stats)
    }

    fn epoch(&self) -> u64 {
        self.epoch
    }
}