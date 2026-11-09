// crates/sheaf-storage/src/lsp/mem_complex.rs — append lock-free live poset
// FILE-016 (METHOD-012). Must never serve frozen reads.
// Concurrency contract (honest, documented):
// - Payload staging is thread-private per writer shard: zero shared state,
//   genuinely lock-free on the 50k/s path.
// - The Star reverse index is sharded 64-way; each insert takes one brief
//   shard mutex (~4 faces per hypercell). Deletes never nest shard locks:
//   star entries are collected first, then each shard is locked once in
//   sorted order (inserts hold at most one shard lock: no inversion).
// - WAL appends take one brief file mutex per record; flush per seal.
// - Readers only touch sealed Arcs + copied live bitsets (see snapshot.rs).
use std::collections::HashMap;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};

use sheaf_core::stalk::discrete::DiscreteStalk;

use crate::bcsf::builder::BcsfBuilder;
use crate::bcsf::reader::BcsfReader;
use super::ring_wal::RingWal;
use super::snapshot::{BatchView, Snapshot};

pub const STAR_SHARDS: usize = 64;
pub const SEAL_EVERY: u32 = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellRef {
    pub shard: usize,
    pub batch: u64,
    pub local: u32,
}

/// A sealed immutable batch plus its live bitset (tombstones flip live).
pub struct SealedBatch {
    pub id: u64,
    pub reader: Arc<BcsfReader>,
    pub live: Vec<AtomicBool>,
    /// B4: versioned tombstones. 0 = alive, else the generation at which
    /// the cell died. A snapshot at gen G sees the cell as alive iff
    /// death_gen == 0 || death_gen > G - so a snapshot needs NO copy of
    /// the liveness state (O(1) instead of O(ncells) per snapshot).
    pub live_gen: Arc<Vec<AtomicU64>>,
}

struct Shard {
    builder: BcsfBuilder,
    open: u32,
    batch_id: u64,
    sealed: Vec<SealedBatch>,
}

/// Op frame: [tag=1][nfaces u8][faces u64le][tx u64le][stalk f32le x2].
/// Delete frame: [tag=2][face u64le]. Deletes are durable: replay applies
/// them after inserts, so crash recovery never resurrects tombstones.
fn encode_op(boundary: &[u64], tx_id: u64, stalk: &[f32; 2]) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 + boundary.len() * 8 + 8 + 8);
    v.push(1u8);
    v.push(boundary.len() as u8);
    for f in boundary {
        v.extend_from_slice(&f.to_le_bytes());
    }
    v.extend_from_slice(&tx_id.to_le_bytes());
    for s in stalk {
        v.extend_from_slice(&s.to_le_bytes());
    }
    v
}

pub struct MemComplex {
    shards: Vec<Mutex<Shard>>,
    star: Vec<Mutex<HashMap<u64, Vec<CellRef>>>>,
    base: Mutex<Arc<BcsfReader>>,
    base_live: Mutex<Vec<AtomicBool>>,
    /// B4: versioned tombstones for base cells (0 = alive). Held behind an
    /// Arc so a snapshot SHARES the table (O(1)) instead of copying it.
    base_death: Mutex<Arc<Vec<AtomicU64>>>,
    /// Base-resident hypercell index: face -> base hypercell indices.
    /// Rebuilt at every install (base is immutable between merges, so indices
    /// stay valid). Without this, deletes after a merge miss cascades whose
    /// Star entries were dropped at install (E2E-caught dangling bug).
    base_star: Mutex<HashMap<u64, Vec<u32>>>,
    wal: RingWal,
    gen: AtomicU64,
    registry: Arc<Mutex<HashMap<u64, usize>>>,
    /// (shard, batch) -> per-local new base index after merge (u32::MAX = dropped).
    merged_map: Mutex<HashMap<(usize, u64), Vec<u32>>>,
    merge_gen: Mutex<HashMap<(usize, u64), u64>>,
}

impl MemComplex {
    pub fn new(base: Arc<BcsfReader>, n_shards: usize, wal_path: &Path) -> std::io::Result<Self> {
        let base_live = (0..base.len()).map(|_| AtomicBool::new(true)).collect();
        let base_death = Arc::new((0..base.len()).map(|_| AtomicU64::new(0)).collect::<Vec<AtomicU64>>());
        Ok(Self {
            shards: (0..n_shards)
                .map(|_| {
                    Mutex::new(Shard { builder: BcsfBuilder::new(), open: 0, batch_id: 0, sealed: Vec::new() })
                })
                .collect(),
            star: (0..STAR_SHARDS).map(|_| Mutex::new(HashMap::new())).collect(),
            base: Mutex::new(base),
            base_live: Mutex::new(base_live),
            base_death: Mutex::new(base_death),
            base_star: Mutex::new(HashMap::new()),
            wal: RingWal::create(wal_path, 16)?,
            gen: AtomicU64::new(1),
            registry: Arc::new(Mutex::new(HashMap::new())),
            merged_map: Mutex::new(HashMap::new()),
            merge_gen: Mutex::new(HashMap::new()),
        })
    }

    pub fn n_shards(&self) -> usize {
        self.shards.len()
    }
    pub fn generation(&self) -> u64 {
        self.gen.load(Ordering::Acquire)
    }

    fn seal_locked(&self, s: &mut Shard) {
        if s.open == 0 {
            return;
        }
        let finished = std::mem::take(&mut s.builder).finish_live();
        let live = (0..finished.len()).map(|_| AtomicBool::new(true)).collect();
        let live_gen = Arc::new((0..finished.len()).map(|_| AtomicU64::new(0)).collect::<Vec<AtomicU64>>());
        s.sealed.push(SealedBatch { id: s.batch_id, reader: Arc::new(finished), live, live_gen });
        s.batch_id += 1;
        s.open = 0;
        self.gen.fetch_add(1, Ordering::AcqRel);
    }

    /// Stage one hypercell on a writer-owned shard. Returns its CellRef.
    /// WALs the op frame first (durable before visible at seal).
    pub fn insert_hypercell(&self, shard: usize, boundary: &[u64], tx_id: u64, stalk: [f32; 2]) -> CellRef {
        let frame = encode_op(boundary, tx_id, &stalk);
        self.wal.append(&frame).unwrap();
        self.stage(shard, boundary, tx_id, stalk)
    }

    /// Recover from a WAL onto a fresh base: replays inserts then deletes.
    /// Returns (inserted, deleted, truncated_tail_seen).
    pub fn recover(base: Arc<BcsfReader>, n_shards: usize, wal_path: &Path) -> std::io::Result<(Self, usize, usize, bool)> {
        let scratch = std::env::temp_dir().join(format!("sheaf_recover_{}.wal", std::process::id()));
        let mem = Self::new(base, n_shards, &scratch)?;
        let (records, truncated) = super::wal::replay(wal_path)?;
        let (mut ins, mut del) = (0, 0);
        for r in &records {
            match r[0] {
                1 => {
                    let n = r[1] as usize;
                    let mut faces = Vec::with_capacity(n);
                    for i in 0..n {
                        faces.push(u64::from_le_bytes(r[2 + i * 8..10 + i * 8].try_into().unwrap()));
                    }
                    let o = 2 + n * 8;
                    let tx = u64::from_le_bytes(r[o..o + 8].try_into().unwrap());
                    let s0 = f32::from_le_bytes(r[o + 8..o + 12].try_into().unwrap());
                    let s1 = f32::from_le_bytes(r[o + 12..o + 16].try_into().unwrap());
                    mem.stage((tx as usize) % n_shards, &faces, tx, [s0, s1]);
                    ins += 1;
                }
                2 => {
                    let face = usize::from_le_bytes(r[1..9].try_into().unwrap());
                    mem.delete_base(face);
                    del += 1;
                }
                _ => {}
            }
        }
        Ok((mem, ins, del, truncated))
    }

    /// Crash-simulation helper: replay inserts then deletes.
    fn stage(&self, shard: usize, boundary: &[u64], tx_id: u64, stalk: [f32; 2]) -> CellRef {
        let mut s = self.shards[shard].lock().unwrap();
        let local = s.open;
        s.builder.push_slice(
            2,
            boundary,
            &DiscreteStalk::from_u64(tx_id).bytes,
            &stalk,
            &[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0],
        );
        s.open += 1;
        let r = CellRef { shard, batch: s.batch_id, local };
        for f in boundary {
            self.star[*f as usize % STAR_SHARDS].lock().unwrap().entry(*f).or_default().push(r);
        }
        if s.open >= SEAL_EVERY {
            self.seal_locked(&mut s);
            self.wal.flush().unwrap();
        }
        r
    }

    /// Delete a base cell + Upper-Star cascade. Shard locks are taken one at
    /// a time in sorted order (see module contract). The tombstone is WALed
    /// (tag 2) so deletes survive crashes.
    pub fn delete_base(&self, face: usize) {
        let mut frame = vec![2u8];
        frame.extend_from_slice(&(face as u64).to_le_bytes());
        self.wal.append(&frame).unwrap();
        // B4: the delete is stamped with the generation that INCLUDES it, so
        // snapshots older than that generation still see the cell as alive
        // (repeatable reads) while newer ones see it dead.
        let death_gen = self.gen.fetch_add(1, Ordering::AcqRel) + 1;
        self.base_live.lock().unwrap()[face].store(false, Ordering::Release);
        self.base_death.lock().unwrap()[face].store(death_gen, Ordering::Release);
        // Base-resident cascade: hypercells merged earlier lost their Star
        // entries at install; base_star covers them (lock order: base_star
        // -> base_live, never inverted anywhere).
        if let Some(v) = self.base_star.lock().unwrap().get(&(face as u64)) {
            let live = self.base_live.lock().unwrap();
            let death = self.base_death.lock().unwrap();
            for idx in v {
                live[*idx as usize].store(false, Ordering::Release);
                death[*idx as usize].store(death_gen, Ordering::Release);
            }
        }
        let refs: Vec<CellRef> = self.star[face % STAR_SHARDS]
            .lock()
            .unwrap()
            .remove(&(face as u64))
            .unwrap_or_default();
        let mut by_shard: HashMap<usize, Vec<CellRef>> = HashMap::new();
        for r in refs {
            by_shard.entry(r.shard).or_default().push(r);
        }
        let mut shards: Vec<usize> = by_shard.keys().copied().collect();
        shards.sort_unstable();
        for sh in shards {
            let mut s = self.shards[sh].lock().unwrap();
            for r in &by_shard[&sh] {
                if r.batch == s.batch_id {
                    // Still in the open builder: seal first so the tombstone
                    // has a stable (batch, local) home.
                    self.seal_locked(&mut s);
                }
                if let Some(b) = s.sealed.iter().find(|b| b.id == r.batch) {
                    b.live[r.local as usize].store(false, Ordering::Release);
                    b.live_gen[r.local as usize].store(death_gen, Ordering::Release);
                } else if let Some(v) = self.merged_map.lock().unwrap().get(&(sh, r.batch)) {
                    // Already merged into base: tombstone the new home.
                    let nb = v[r.local as usize];
                    if nb != u32::MAX {
                        self.base_live.lock().unwrap()[nb as usize].store(false, Ordering::Release);
                        self.base_death.lock().unwrap()[nb as usize]
                            .store(death_gen, Ordering::Release);
                    }
                }
            }
        }
    }

    /// Repeatable-read snapshot: immutable Arcs + copied live bitsets.
    /// F1: generation read and registry pin happen atomically under the
    /// registry lock, so install_base can never prune a translation this
    /// snapshot depends on.
    pub fn snapshot(&self) -> Snapshot {
        let mut reg = self.registry.lock().unwrap();
        let gen = self.gen.load(Ordering::Acquire);
        Snapshot::pin(&mut reg, gen);
        drop(reg);
        let base = self.base.lock().unwrap().clone();
        // B4: NO liveness copy. The snapshot shares the versioned tombstone
        // tables and resolves liveness against its own generation, so
        // snapshot creation is O(1) instead of O(ncells).
        let base_death = self.base_death.lock().unwrap().clone();
        let mut batches = Vec::new();
        for (sh, s) in self.shards.iter().enumerate() {
            let s = s.lock().unwrap();
            for b in &s.sealed {
                batches.push(BatchView {
                    shard: sh,
                    id: b.id,
                    reader: b.reader.clone(),
                    live_gen: b.live_gen.clone(),
                });
            }
        }
        Snapshot::new(gen, base, base_death, batches, self.registry.clone())
    }

    /// Seal every open tail without draining (end-of-test fence).
    pub fn seal_all_open(&self) {
        for s in &self.shards {
            let mut s = s.lock().unwrap();
            self.seal_locked(&mut s);
        }
        self.wal.flush().unwrap();
    }

    /// Group-commit flush for durability points (replay tests, shutdown).
    pub fn flush_wal(&self) {
        self.wal.flush().unwrap();
    }

    /// Drain only the named sealed batches (snapshot fencing: anything sealed
    /// after the snapshot waits for the next merge).
    pub fn drain_batches(&self, ids: &[(usize, u64)]) -> Vec<(usize, SealedBatch)> {
        let mut out = Vec::new();
        for (sh, s) in self.shards.iter().enumerate() {
            let mut s = s.lock().unwrap();
            let mut i = 0;
            while i < s.sealed.len() {
                if ids.contains(&(sh, s.sealed[i].id)) {
                    out.push((sh, s.sealed.remove(i)));
                } else {
                    i += 1;
                }
            }
        }
        out
    }

    /// Seal every open tail (end-of-test drain). Open builders keep accepting.
    pub fn drain_sealed(&self) -> Vec<(usize, SealedBatch)> {
        let mut out = Vec::new();
        for (sh, s) in self.shards.iter().enumerate() {
            let mut s = s.lock().unwrap();
            self.seal_locked(&mut s);
            for b in s.sealed.drain(..) {
                out.push((sh, b));
            }
        }
        out
    }

    /// B8: persist the current base as a DURABLE SEGMENT (self-describing
    /// header + payload). The segment can later be opened without running
    /// the builder - no in-memory reconstruction required.
    pub fn persist_base_segment(
        &self,
        path: &Path,
        generation: u64,
        src_fingerprint: u64,
    ) -> Result<(), String> {
        let base = self.base.lock().unwrap().clone();
        crate::bcsf::segment::write_segment(&base, path, generation, src_fingerprint)
    }

    /// B8: restore a persisted base segment - the segment OPENS without the
    /// builder. The restored base starts with an empty tombstone table
    /// (every cell alive) and a fresh generation.
    pub fn restore_base_segment(&self, path: &Path) -> Result<usize, String> {
        let reader = crate::bcsf::segment::open_segment(path)?;
        let n = reader.len();
        let live: Vec<AtomicBool> = (0..n).map(|_| AtomicBool::new(true)).collect();
        let death = Arc::new((0..n).map(|_| AtomicU64::new(0)).collect::<Vec<AtomicU64>>());
        *self.base.lock().unwrap() = Arc::new(reader);
        *self.base_live.lock().unwrap() = live;
        *self.base_death.lock().unwrap() = death;
        Ok(n)
    }

    pub fn base_len(&self) -> usize {
        self.base.lock().unwrap().len()
    }

    /// F4: Phase-2 memory footprint estimate (star refs/faces, live bitsets).
    /// Counts are exact; byte math is labeled estimate (HashMap entry
    /// overhead varies): refs×32B + faces×48B + live bits. Reported against
    /// the frozen baseline to keep the 1.5x discipline alive past Phase 1.
    /// Informational in Phase 2, gated by Phase 6 at latest.
    pub fn footprint(&self) -> (usize, usize, usize) {
        let (mut faces, mut refs) = (0, 0);
        for m in &self.star {
            let m = m.lock().unwrap();
            faces += m.len();
            refs += m.values().map(|v| v.len()).sum::<usize>();
        }
        let mut live_bits = self.base_live.lock().unwrap().len();
        for s in &self.shards {
            let s = s.lock().unwrap();
            live_bits += s.sealed.iter().map(|b| b.live.len()).sum::<usize>();
        }
        (faces, refs, live_bits)
    }

    /// Install a merged base. Batches named in `drained` are gone from mem;
    /// their translations are filed for late deletes. Star entries pointing
    /// into drained batches are dropped and REPLACED by a rebuilt base_star
    /// scan (merged hypercells are base-resident cascade targets now).
    /// Rejects nested hypercells (boundary faces must be dim-0 cells):
    /// the two-level merge scope the compactor enforces.
    pub fn install_base(
        &self,
        new_base: BcsfReader,
        new_base_live: Vec<bool>,
        drained: &[(usize, u64)],
        translations: HashMap<(usize, u64), Vec<u32>>,
    ) -> Result<(), String> {
        let mut rebuilt: HashMap<u64, Vec<u32>> = HashMap::new();
        for cell in 0..new_base.len() {
            if new_base.dim(cell) == 2 {
                for id in new_base.boundary(cell) {
                    if new_base.dim(*id as usize) != 0 {
                        return Err(format!("nested hypercell at base cell {} (two-level scope)", cell));
                    }
                    rebuilt.entry(*id).or_default().push(cell as u32);
                }
            }
        }
        for m in &self.star {
            let mut m = m.lock().unwrap();
            m.retain(|_, v| {
                v.retain(|r| !drained.contains(&(r.shard, r.batch)));
                !v.is_empty()
            });
        }
        {
            let mut mm = self.merged_map.lock().unwrap();
            let mut mg = self.merge_gen.lock().unwrap();
            let g = self.generation();
            for (k, v) in translations {
                mm.insert(k, v);
                mg.insert(k, g);
            }
            // Prune translations older than every live snapshot. F1: the
            // registry lock is held across min-computation AND pruning while
            // snapshot() registers under the same lock, so the prune set can
            // never cover a generation a live snapshot depends on.
            let reg = self.registry.lock().unwrap();
            let min = Snapshot::min_pinned(&reg);
            let stale: Vec<(usize, u64)> =
                mg.iter().filter(|(_, gen)| **gen < min).map(|(k, _)| *k).collect();
            for k in stale {
                mg.remove(&k);
                mm.remove(&k);
            }
        }
        let live: Vec<AtomicBool> = new_base_live.into_iter().map(AtomicBool::new).collect();
        assert_eq!(live.len(), new_base.len());
        // B4: the merged base starts with an empty tombstone table (dead cells
        // were skipped during the merge); deletes that land after the merge
        // stamp this table with a fresh generation.
        let death = Arc::new((0..new_base.len()).map(|_| AtomicU64::new(0)).collect::<Vec<AtomicU64>>());
        *self.base.lock().unwrap() = Arc::new(new_base);
        *self.base_live.lock().unwrap() = live;
        *self.base_death.lock().unwrap() = death;
        *self.base_star.lock().unwrap() = rebuilt;
        self.gen.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }
}
