// crates/sheaf-storage/src/lsp/delta_wal.rs — local cochain differential WAL
// Node N1 of IMPLEMENTATION_PLAN_OPTIMIZE_EXPAND_2026-10-27.md.
// Records only the stalk delta and the cell id, not the whole cell.
// Restriction maps are time-invariant, so the differential alone is enough.
// Depends on ring_wal::crc_hw for the hardware CRC.
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use super::ring_wal::crc_hw;

const BASE_MARKER_LEN: usize = 8;
// Actual frame layout: [epoch u64][cell_id u64][len u32][crc u32][payload]
// Header before payload is 8+8+4+4 = 24 bytes.

pub struct DeltaWal {
    buf: std::io::BufWriter<File>,
    epoch: u64,
}

impl DeltaWal {
    pub fn create(path: &Path) -> std::io::Result<Self> {
        let file = File::create(path)?;
        file.set_len(8 << 20)?;
        let mut buf = std::io::BufWriter::with_capacity(64 << 10, file);
        buf.write_all(&0u64.to_le_bytes())?; // epoch base marker
        Ok(Self { buf, epoch: 1 })
    }

    pub fn append_delta(&mut self, cell_id: u64, delta: &[u8]) -> std::io::Result<()> {
        let len = u32::try_from(delta.len()).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "delta too large")
        })?;
        self.buf.write_all(&self.epoch.to_le_bytes())?;
        self.buf.write_all(&cell_id.to_le_bytes())?;
        self.buf.write_all(&len.to_le_bytes())?;
        self.buf.write_all(&crc_hw(delta).to_le_bytes())?;
        self.buf.write_all(delta)?;
        self.epoch += 1;
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.buf.flush()
    }
}

pub fn replay_delta(path: &Path) -> std::io::Result<(Vec<(u64, Vec<u8>)>, u64, bool)> {
    let mut buf = Vec::new();
    File::open(path)?.read_to_end(&mut buf)?;
    let mut out = Vec::new();
    let mut pos = BASE_MARKER_LEN;
    let mut expect = 1u64;
    let mut torn = false;
    while pos < buf.len() {
        if buf.len() - pos < 24 {
            torn = true;
            break;
        }
        let epoch = u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        let cell_id = u64::from_le_bytes(buf[pos + 8..pos + 16].try_into().unwrap());
        let len = u32::from_le_bytes(buf[pos + 16..pos + 20].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(buf[pos + 20..pos + 24].try_into().unwrap());
        if epoch == 0 && len == 0 {
            break; // padding
        }
        if epoch != expect || buf.len() - pos - 24 < len {
            torn = true;
            break;
        }
        let payload = &buf[pos + 24..pos + 24 + len];
        if crc_hw(payload) != crc {
            torn = true;
            break;
        }
        out.push((cell_id, payload.to_vec()));
        expect += 1;
        pos += 24 + len;
    }
    Ok((out, expect - 1, torn))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn round_trip() {
        let dir = std::env::temp_dir().join("delta_wal_test");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("t.wal");
        let _ = fs::remove_file(&path);
        let mut w = DeltaWal::create(&path).unwrap();
        for i in 0..1000u64 {
            w.append_delta(i, &[i as u8, (i >> 8) as u8]).unwrap();
        }
        w.flush().unwrap();
        drop(w);
        let (recs, n, torn) = replay_delta(&path).unwrap();
        assert_eq!(recs.len(), 1000);
        assert_eq!(n, 1000);
        assert!(!torn);
        for (i, r) in recs.iter().enumerate() {
            assert_eq!(r.0, i as u64);
            assert_eq!(r.1, vec![i as u8, (i >> 8) as u8]);
        }
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn torn_tail() {
        let dir = std::env::temp_dir().join("delta_wal_test2");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("t2.wal");
        let _ = fs::remove_file(&path);
        let mut w = DeltaWal::create(&path).unwrap();
        for i in 0..10u64 {
            w.append_delta(i, &[1, 2, 3]).unwrap();
        }
        w.flush().unwrap();
        drop(w);
        // truncate into the last frame's payload (data ends well before the 8 MB prealloc)
        let data_end = 8usize + 10 * (24 + 3);
        let f = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        f.set_len((data_end - 5) as u64).unwrap();
        drop(f);
        let (recs, _, torn) = replay_delta(&path).unwrap();
        assert!(recs.len() < 10);
        assert!(torn);
        let _ = std::fs::remove_file(&path);
    }
}
