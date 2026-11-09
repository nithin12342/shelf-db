// crates/sheaf-engine/src/transaction/manager.rs — manage txn begin commit
// FILE-044 (METHOD-007). Must never evaluate energy.
// Snapshot-isolated transactions over MemComplex: a txn pins a Snapshot at
// begin; buffered writes commit only if no face in the write set was
// committed by another txn after the snapshot generation (face-clock
// conflict check under sorted face locks). Abort touches nothing.
use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

use sheaf_storage::lsp::mem_complex::MemComplex;
use sheaf_storage::lsp::snapshot::Snapshot;
use super::lock::FaceLocks;

pub struct PendingOp {
    pub boundary: Vec<u64>,
    pub tx_id: u64,
    pub stalk: [f32; 2],
}

pub struct Txn {
    snap_seq: u64,
    _snap: Snapshot,
    writes: Vec<PendingOp>,
}

#[derive(Debug, PartialEq)]
pub enum CommitOutcome {
    Committed { cells: usize },
    AbortedConflict,
}

pub struct TxnManager {
    mem: Arc<MemComplex>,
    /// Face -> last committing sequence. Monotonic commit sequence (not cell
    /// generation: generations only move on seal/delete/merge, so two commits
    /// between seals would share one generation and miss the conflict).
    face_clock: Mutex<HashMap<u64, u64>>,
    commit_seq: AtomicU64,
    locks: FaceLocks,
}

impl TxnManager {
    pub fn new(mem: Arc<MemComplex>, lock_stripes: usize) -> Self {
        Self { mem, face_clock: Mutex::new(HashMap::new()), commit_seq: AtomicU64::new(1), locks: FaceLocks::new(lock_stripes) }
    }

    pub fn begin(&self) -> Txn {
        let snap = self.mem.snapshot();
        let seq = self.commit_seq.load(Ordering::Acquire);
        let _ = snap.gen;
        Txn { snap_seq: seq, _snap: snap, writes: Vec::new() }
    }

    pub fn insert(&self, txn: &mut Txn, boundary: Vec<u64>, tx_id: u64, stalk: [f32; 2]) {
        txn.writes.push(PendingOp { boundary, tx_id, stalk });
    }

    /// Commit with face-clock validation over the monotonic commit sequence:
    /// any face committed after our snapshot aborts us — including fully
    /// concurrent overlapping commits (first committer wins via fetch_add).
    /// Shard assignment round-robins across writer shards by tx_id
    /// (deterministic, no shared counter).
    pub fn commit(&self, txn: Txn) -> CommitOutcome {
        let mut faces: Vec<u64> = txn.writes.iter().flat_map(|w| w.boundary.iter().copied()).collect();
        let _guards = self.locks.lock_sorted(&mut faces);
        {
            let clock = self.face_clock.lock().unwrap();
            for f in &faces {
                if clock.get(f).copied().unwrap_or(0) > txn.snap_seq {
                    return CommitOutcome::AbortedConflict;
                }
            }
        }
        let seq = self.commit_seq.fetch_add(1, Ordering::AcqRel) + 1;
        let n_shards = self.mem.n_shards();
        for w in &txn.writes {
            self.mem.insert_hypercell(w.tx_id as usize % n_shards, &w.boundary, w.tx_id, w.stalk);
        }
        {
            let mut clock = self.face_clock.lock().unwrap();
            for f in &faces {
                clock.insert(*f, seq);
            }
        }
        CommitOutcome::Committed { cells: txn.writes.len() }
    }
}
