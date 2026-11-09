// crates/sheaf-storage/src/lsp/graded_log.rs — dimension-partitioned WAL
// SYNTHESIS Stage 2 (Choice 2 pipeline). Depends on FILE-017 (wal frames).
// One append log per cell dimension; a monotonic watermark guarantees poset
// down-closure (0 → 1 → K): dimension d seals only when every dimension
// below d is already sealed. A k-cell can never become durable before its
// faces — concurrent writers may ARRIVE in any order, but durability
// commits in graded order. Readers below the watermark see a closed complex.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::wal::{replay, WalWriter};

pub struct GradedWal {
    dir: PathBuf,
    logs: BTreeMap<u8, WalWriter>,
    sealed: BTreeMap<u8, bool>,
    watermark: u8,
}

impl GradedWal {
    pub fn create(dir: &Path) -> std::io::Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self { dir: dir.to_path_buf(), logs: BTreeMap::new(), sealed: BTreeMap::new(), watermark: 0 })
    }

    fn log(&mut self, dim: u8) -> std::io::Result<&mut WalWriter> {
        if !self.logs.contains_key(&dim) {
            let path = self.dir.join(format!("dim{}.wal", dim));
            let _ = std::fs::remove_file(&path);
            self.logs.insert(dim, WalWriter::create(&path)?);
            self.sealed.insert(dim, false);
        }
        Ok(self.logs.get_mut(&dim).unwrap())
    }

    /// Append a cell payload at dimension `dim`. Always accepted (arrival
    /// order is unconstrained); durability is gated by `seal`.
    pub fn append(&mut self, dim: u8, payload: &[u8]) -> std::io::Result<()> {
        self.log(dim)?.append(payload)
    }

    /// Seal dimension `dim`. Succeeds only if every PRESENT dimension below
    /// `dim` is already sealed (graded order); absent dims are vacuously
    /// sealed (no cells → nothing to order). Otherwise returns `Err` and the
    /// watermark does not advance. Sealing re-seals idempotently.
    pub fn seal(&mut self, dim: u8) -> std::io::Result<()> {
        for d in 0..dim {
            if self.logs.contains_key(&d) && !self.sealed.get(&d).copied().unwrap_or(false) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("cannot seal dim {} before dim {} (down-closure)", dim, d),
                ));
            }
        }
        self.log(dim)?.flush()?;
        self.sealed.insert(dim, true);
        // Advance past sealed dims, skipping absent ones (vacuous); bounded
        // by the max present dim so an empty tail cannot run away.
        let max = self.logs.keys().max().copied().unwrap_or(0);
        while self.watermark <= max
            && (!self.logs.contains_key(&self.watermark)
                || self.sealed.get(&self.watermark).copied().unwrap_or(false))
        {
            self.watermark += 1;
        }
        Ok(())
    }

    /// Highest sealed dimension + 1 (all dims below are durable + closed).
    pub fn watermark(&self) -> u8 {
        self.watermark
    }

    /// Filesystem path of a dimension's log (for colimit projection).
    pub fn dir_for(&self, dim: u8) -> PathBuf {
        self.dir.join(format!("dim{}.wal", dim))
    }

    /// Replay every sealed dimension in graded order. Returns per-dim counts.
    pub fn replay_sealed(&self) -> std::io::Result<Vec<(u8, usize)>> {
        let mut out = Vec::new();
        for &dim in self.logs.keys() {
            if self.sealed.get(&dim).copied().unwrap_or(false) {
                let path = self.dir.join(format!("dim{}.wal", dim));
                let (recs, truncated) = replay(&path)?;
                assert!(!truncated, "sealed log must replay clean");
                out.push((dim, recs.len()));
            }
        }
        Ok(out)
    }
}
