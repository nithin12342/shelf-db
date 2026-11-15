use crate::model::{Result, Statement, MEMORY_LIMIT};
use serde::{Deserialize, Serialize};
use sheaf_native::wal::{self, WalBackend, WalStats, MAX_PAYLOAD};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

const CHUNK_BYTES: usize = MAX_PAYLOAD - 16;
const MAX_LOG_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct Record {
    version: u32,
    statement: Statement,
}

pub struct Journal {
    wal: Box<dyn WalBackend>,
    log_path: PathBuf,
    stats: WalStats,
    poisoned: bool,
    _lock: File,
}

impl Journal {
    pub fn open(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("database.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("database is already open or cannot be locked: {e}"))?;
        let log_path = directory.join("mutations.wal");
        let mut wal = wal::open(&log_path, &directory.join("epochs.bin"))
            .map_err(|e| format!("cannot open database journal: {e:?}"))?;
        // An empty durable epoch makes an interrupted first append recoverable.
        if wal.epoch() == 0 {
            wal.commit_epoch(1)
                .map_err(|e| format!("initial epoch: {e:?}"))?;
        }
        let stats = wal
            .recover()
            .map_err(|e| format!("journal recovery: {e:?}"))?;
        if stats.valid_end > MAX_LOG_BYTES {
            return Err("journal exceeds local application size limit".into());
        }
        Ok(Self {
            wal,
            log_path,
            stats,
            poisoned: false,
            _lock: lock,
        })
    }

    pub fn name(&self) -> &'static str {
        self.wal.name()
    }
    pub fn epoch(&self) -> u64 {
        self.wal.epoch()
    }

    pub fn replay(&self, mut apply: impl FnMut(Statement) -> Result<()>) -> Result<()> {
        let mut pending = Vec::new();
        let mut current_epoch = 0;
        let mut next_part = 0;
        let mut total_parts = 0;
        let mut total_len = 0;
        sheaf_native::committed_records::replay(&self.log_path, self.stats, |epoch, payload| {
            if payload.len() < 16 || &payload[..4] != b"APP1" {
                return Err("unknown application log format".into());
            }
            let part = u32::from_le_bytes(payload[4..8].try_into().unwrap()) as usize;
            let parts = u32::from_le_bytes(payload[8..12].try_into().unwrap()) as usize;
            let len = u32::from_le_bytes(payload[12..16].try_into().unwrap()) as usize;
            if part == 0 {
                if !pending.is_empty()
                    || epoch <= current_epoch
                    || len == 0
                    || len > MEMORY_LIMIT
                    || parts != len.div_ceil(CHUNK_BYTES)
                {
                    return Err("invalid application transaction header".into());
                }
                current_epoch = epoch;
                next_part = 0;
                total_parts = parts;
                total_len = len;
            }
            if epoch != current_epoch
                || part != next_part
                || parts != total_parts
                || len != total_len
                || part >= parts
                || payload.len() - 16 != CHUNK_BYTES.min(len - pending.len())
            {
                return Err("invalid application transaction chunk".into());
            }
            pending.extend_from_slice(&payload[16..]);
            next_part += 1;
            if next_part == parts {
                let record: Record = serde_json::from_slice(&pending).map_err(|e| e.to_string())?;
                if record.version != 1 {
                    return Err("unsupported application journal version".into());
                }
                apply(record.statement)?;
                pending.clear();
            }
            Ok(())
        })?;
        if !pending.is_empty() {
            return Err("incomplete committed application transaction".into());
        }
        Ok(())
    }

    pub fn commit(&mut self, statement: &Statement) -> Result<()> {
        if self.poisoned {
            return Err("journal needs restart after a write failure".into());
        }
        let bytes = serde_json::to_vec(&Record {
            version: 1,
            statement: statement.clone(),
        })
        .map_err(|e| e.to_string())?;
        if bytes.len() > MEMORY_LIMIT {
            return Err("transaction byte limit exceeded".into());
        }
        let parts = bytes.len().div_ceil(CHUNK_BYTES);
        let added = bytes.len() as u64 + (parts * 48) as u64;
        if self.stats.valid_end + added > MAX_LOG_BYTES {
            return Err("journal full: local limit is 256 MiB".into());
        }
        let epoch = self.wal.epoch().checked_add(1).ok_or("epoch overflow")?;
        let result = (|| {
            for (part, chunk) in bytes.chunks(CHUNK_BYTES).enumerate() {
                let mut payload = Vec::with_capacity(chunk.len() + 16);
                payload.extend_from_slice(b"APP1");
                payload.extend_from_slice(&(part as u32).to_le_bytes());
                payload.extend_from_slice(&(parts as u32).to_le_bytes());
                payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                payload.extend_from_slice(chunk);
                self.wal.append(epoch, &payload)?;
            }
            self.wal.commit_epoch(epoch)
        })();
        if let Err(error) = result {
            self.poisoned = true;
            return Err(format!(
                "journal commit failed ({error:?}). Restart to resolve durability"
            ));
        }
        self.stats.valid_end += added;
        self.stats.epoch = epoch;
        Ok(())
    }
}
