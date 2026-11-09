// crates/sheaf-storage/src/lsp/epoch_wal.rs — epoch-validated append-only WAL
// MAX-PERF P4 (METHOD-026, FILE-062). Depends on wal (frames/CRC) + ring (HW CRC).
// Eliminates the per-op header rewrite (seek-to-0 + write + seek-back) that
// dominates durable single-op latency: validity derives from a monotonic
// epoch counter per frame ([epoch u64LE][payload]) instead of a rewritten
// valid_len header. Recovery scans frames in order and accepts the longest
// prefix with contiguous epochs + valid CRCs; torn tails fail the epoch
// chain loudly (returned, never silently dropped).
use super::ring_wal::crc_hw;

pub struct EpochWriter {
    buf: std::io::BufWriter<std::fs::File>,
    epoch: u64,
}

impl EpochWriter {
    pub fn create(path: &std::path::Path) -> std::io::Result<Self> {
        use std::io::Write;
        let file = std::fs::File::create(path)?;
        file.set_len(8 << 20)?;
        let mut buf = std::io::BufWriter::with_capacity(64 << 10, file);
        buf.write_all(&0u64.to_le_bytes())?; // epoch base marker
        Ok(Self { buf, epoch: 1 })
    }

    /// Append one frame: [epoch u64LE][len u32LE][crc u32LE][payload].
    /// Pure forward append — no seeks, no header rewrites.
    pub fn append(&mut self, payload: &[u8]) -> std::io::Result<()> {
        use std::io::Write;
        self.buf.write_all(&self.epoch.to_le_bytes())?;
        self.buf.write_all(&(payload.len() as u32).to_le_bytes())?;
        self.buf.write_all(&crc_hw(payload).to_le_bytes())?;
        self.buf.write_all(payload)?;
        self.epoch += 1;
        Ok(())
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        use std::io::Write;
        self.buf.flush()
    }
}

/// Replay: longest prefix of contiguous epochs starting at 1 with valid
/// CRCs. Returns (payloads, epochs_recovered, torn). A gap or CRC failure
/// ends the prefix loudly via `torn=true`.
pub fn replay_epochs(path: &std::path::Path) -> std::io::Result<(Vec<Vec<u8>>, u64, bool)> {
    use std::io::Read;
    let mut buf = Vec::new();
    std::fs::File::open(path)?.read_to_end(&mut buf)?;
    let mut out = Vec::new();
    let mut pos = 8usize; // skip base marker
    let mut expect = 1u64;
    let mut torn = false;
    while pos < buf.len() {
        if buf.len() - pos < 16 {
            torn = true;
            break;
        }
        let epoch = u64::from_le_bytes(buf[pos..pos + 8].try_into().unwrap());
        let len = u32::from_le_bytes(buf[pos + 8..pos + 12].try_into().unwrap()) as usize;
        // All-zero frame = unwritten prealloc padding: clean end, not torn.
        // Sound: epochs start at 1, so epoch 0 with len 0 can only be padding
        // (a crash before any byte lands commits nothing there either).
        if epoch == 0 && len == 0 {
            break;
        }
        let crc = u32::from_le_bytes(buf[pos + 12..pos + 16].try_into().unwrap());
        if epoch != expect || buf.len() - pos - 16 < len {
            torn = true;
            break;
        }
        let payload = &buf[pos + 16..pos + 16 + len];
        if crc_hw(payload) != crc {
            torn = true;
            break;
        }
        out.push(payload.to_vec());
        expect += 1;
        pos += 16 + len;
    }
    Ok((out, expect - 1, torn))
}
