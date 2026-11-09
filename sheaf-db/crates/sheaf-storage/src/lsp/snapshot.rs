// crates/sheaf-storage/src/lsp/snapshot.rs — isolate MVCC read snapshots
// FILE-018 (METHOD-018). Must never compact segments.
// A snapshot pins immutable Arcs plus COPIED live bitsets, so later
// tombstones never leak into repeatable reads. Dropping a snapshot
// releases its generation pin for merged-map pruning.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::bcsf::reader::BcsfReader;

pub struct BatchView {
    pub shard: usize,
    pub id: u64,
    pub reader: Arc<BcsfReader>,
    /// B4: versioned tombstones (0 = alive, else death generation).
    pub live_gen: Arc<Vec<std::sync::atomic::AtomicU64>>,
}

pub struct Snapshot {
    pub gen: u64,
    pub base: Arc<BcsfReader>,
    /// B4: versioned tombstones for base cells (0 = alive).
    pub base_death: Arc<Vec<std::sync::atomic::AtomicU64>>,
    pub batches: Vec<BatchView>,
    registry: Arc<Mutex<HashMap<u64, usize>>>,
}

impl Snapshot {
    /// Construct from a pre-registered generation. Registration MUST happen
    /// under the registry lock together with the generation read (done by
    /// MemComplex::snapshot); otherwise install_base can prune a translation
    /// between the read and the registration and a late delete silently
    /// misses (F1 race). This constructor never touches the registry.
    pub fn new(
        gen: u64,
        base: Arc<BcsfReader>,
        base_death: Arc<Vec<std::sync::atomic::AtomicU64>>,
        batches: Vec<BatchView>,
        registry: Arc<Mutex<HashMap<u64, usize>>>,
    ) -> Self {
        Self { gen, base, base_death, batches, registry }
    }

    /// N5: the epoch this snapshot is frozen at (its generation plus the
    /// fingerprint of the base it sees). A fiber built from this snapshot
    /// carries this epoch.
    pub fn epoch(&self, src_fingerprint: u64) -> super::fiber_epoch::Epoch {
        super::fiber_epoch::Epoch::new(self.gen, src_fingerprint)
    }

    /// B4: liveness of a base cell AS OF this snapshot's generation.
    /// A cell deleted after the snapshot was taken stays alive for it
    /// (repeatable reads), without any per-snapshot copy of the liveness
    /// state. O(1) per cell.
    pub fn base_alive(&self, cell: usize) -> bool {
        match self.base_death.get(cell) {
            Some(g) => {
                let d = g.load(std::sync::atomic::Ordering::Acquire);
                d == 0 || d > self.gen
            }
            None => false,
        }
    }

    /// B4: liveness of a batch cell AS OF this snapshot's generation.
    pub fn batch_alive(&self, view: &BatchView, cell: usize) -> bool {
        match view.live_gen.get(cell) {
            Some(g) => {
                let d = g.load(std::sync::atomic::Ordering::Acquire);
                d == 0 || d > self.gen
            }
            None => false,
        }
    }

    /// Register a generation pin. The caller MUST hold the registry lock
    /// across its generation read and this call (pass the guarded map).
    pub fn pin(map: &mut HashMap<u64, usize>, gen: u64) {
        *map.entry(gen).or_insert(0) += 1;
    }

    /// Oldest pinned generation over an already-locked registry view.
    pub fn min_pinned(map: &HashMap<u64, usize>) -> u64 {
        map.keys().copied().min().unwrap_or(u64::MAX)
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        let mut r = self.registry.lock().unwrap();
        if let Some(c) = r.get_mut(&self.gen) {
            *c -= 1;
            if *c == 0 {
                r.remove(&self.gen);
            }
        }
    }
}
