// replication/mod.rs - the section replication domain (NF-05, NF-06).
//
// A section is a cell's payload. A delta is the XOR patch that changes a
// section. XOR is commutative and associative, so applying every patch once,
// in any delivery order, gives the same final state. The adapter only moves
// frames; the state merger owns the order-tolerance rule.

pub mod frame;
pub mod port;
pub mod rdma_port;
pub mod rdma_unsupported;
pub mod rdma_native;
pub mod tcp_adapter;

use std::collections::{BTreeMap, HashSet};

pub use port::{
    Capability, RdmaTransport, ReplicationError, ReplicationTransport, Result,
};
pub use rdma_unsupported::RdmaUnsupported;
pub use tcp_adapter::{TcpReceiver, TcpSender};

/// One ordered change to a cell's section.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SectionDelta {
    /// A unique sequence number. The merger applies each sequence once.
    pub sequence: u64,
    /// The cell the patch belongs to.
    pub cell_id: u64,
    /// `new_payload = old_payload XOR xor_patch`, zero-extended as needed.
    pub xor_patch: Vec<u8>,
}

impl SectionDelta {
    pub fn new(sequence: u64, cell_id: u64, xor_patch: Vec<u8>) -> Self {
        Self { sequence, cell_id, xor_patch }
    }
}

/// The merged state of all sections, keyed by cell.
#[derive(Clone, Debug, Default)]
pub struct SectionState {
    sections: BTreeMap<u64, Vec<u8>>,
    applied: HashSet<u64>,
}

impl SectionState {
    /// Apply one delta. A repeated sequence is ignored, so delivery retries
    /// cannot change the state.
    pub fn apply(&mut self, delta: &SectionDelta) {
        if !self.applied.insert(delta.sequence) {
            return;
        }
        let payload = self.sections.entry(delta.cell_id).or_default();
        if payload.len() < delta.xor_patch.len() {
            payload.resize(delta.xor_patch.len(), 0);
        }
        for (byte, patch) in payload.iter_mut().zip(&delta.xor_patch) {
            *byte ^= patch;
        }
    }

    /// Apply a whole batch.
    pub fn apply_all(&mut self, deltas: &[SectionDelta]) {
        for delta in deltas {
            self.apply(delta);
        }
    }

    /// The number of distinct sections held.
    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// The number of distinct sequences applied.
    pub fn applied_count(&self) -> usize {
        self.applied.len()
    }

    /// A deterministic BLAKE3 fingerprint over the ordered cell/payload pairs.
    pub fn fingerprint(&self) -> blake3::Hash {
        let mut hasher = blake3::Hasher::new();
        for (cell, payload) in &self.sections {
            hasher.update(&cell.to_le_bytes());
            hasher.update(&(payload.len() as u64).to_le_bytes());
            hasher.update(payload);
        }
        hasher.finalize()
    }
}
