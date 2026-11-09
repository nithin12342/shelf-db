// crates/sheaf-storage/src/bcsf/reader.rs — serve zero-copy snapshot views
// FILE-015 (METHOD-001). Must never write or lock.
// V3: boundary() carries a debug_assert sorted check (zero-cost release).
use std::sync::Arc;

use super::arena::StalkArena;
use super::indices::IncidenceIndices;
use super::offsets::CellOffsets;

/// B5: a shadow-paging CHAIN. A merged segment is not always a fresh copy
/// of the whole dataset: when the merge drops nothing, the new segment is
/// the OLD base (untouched) plus a DELTA of the newly added cells. The
/// reader then resolves cells through this chain instead of copying the
/// base (merge cost O(delta) instead of O(n)).
///
/// Cell id space of a chained reader:
///   [0, base_cells)              -> the base's cells (via `remap`)
///   [base_cells, base_cells+delta) -> the delta's cells
/// The delta's boundary ids refer to BASE cells, which is exactly the id
/// space the base part keeps (the remap is the identity when nothing was
/// dropped), so the ids stay valid in the chained space.
#[derive(Debug)]
pub struct Chain {
    /// The pre-merge base, kept as-is.
    pub base: Arc<BcsfReader>,
    /// The cells the merge added.
    pub delta: Arc<BcsfReader>,
    /// base cell -> chained cell index (u32::MAX = dropped). Identity when
    /// the merge dropped nothing.
    pub remap: Vec<u32>,
    /// True when `remap` is the identity (the hot path stays
    /// indirection-free: no remap lookup at all).
    pub identity: bool,
    /// Number of chained cells that come from the base.
    pub base_cells: usize,
}

/// Immutable BCSF view: dims + offsets + indices + arena. Zero heap
/// indirection in the read path (slice indexing only).
#[derive(Debug)]
pub struct BcsfReader {
    pub dims: Vec<u8>,
    pub offsets: CellOffsets,
    pub indices: IncidenceIndices,
    pub arena: StalkArena,
    /// Null bitmap mirrored here for fast O(1) validity checks without
    /// going through the arena.
    pub validity: Vec<u64>,
    /// B5: optional shadow-paging chain. When set, this reader's OWN slabs
    /// are empty and its cells resolve through the chain.
    pub chain: Option<Arc<Chain>>,
}

impl BcsfReader {
    /// Build a shadow-paging chain over a base and a delta (B5).
    /// `remap` maps base cells to chained indices (u32::MAX = dropped).
    pub fn chained(base: Arc<BcsfReader>, delta: Arc<BcsfReader>, remap: Vec<u32>) -> BcsfReader {
        let base_cells = base.len();
        let identity = remap.iter().enumerate().all(|(i, &r)| r == i as u32);
        BcsfReader {
            dims: Vec::new(),
            offsets: CellOffsets { offsets: vec![0] },
            indices: IncidenceIndices { indices: Vec::new() },
            arena: StalkArena {
                stalk_offsets: vec![0],
                stalks: Vec::new(),
                morph_cells: Vec::new(),
                morph_offsets: vec![0],
                morphs: Vec::new(),
                discrete: Vec::new(),
                validity: Vec::new(),
            },
            validity: Vec::new(),
            chain: Some(Arc::new(Chain { base, delta, remap, identity, base_cells })),
        }
    }

    /// Resolve a cell to (FLAT source reader, source index) through the
    /// chain. The base may itself be a chain (the depth is bounded), so
    /// the resolution RECURSES until it reaches a flat reader.
    #[inline]
    fn resolve(&self, cell: usize) -> (&BcsfReader, usize) {
        match &self.chain {
            None => (self, cell),
            Some(c) => {
                if cell < c.base_cells {
                    // identity remap: the hot path stays indirection-free
                    let idx = if c.identity { cell } else { c.remap[cell] as usize };
                    c.base.resolve(idx)
                } else {
                    (&c.delta, cell - c.base_cells)
                }
            }
        }
    }

    /// Depth of the shadow-paging chain (0 = flat reader). The merge
    /// bounds it (an LSM-style level count) so the read path never chases
    /// an unbounded chain.
    pub fn chain_depth(&self) -> usize {
        match &self.chain {
            None => 0,
            Some(c) => 1 + c.base.chain_depth(),
        }
    }

    pub fn len(&self) -> usize {
        match &self.chain {
            None => self.dims.len(),
            Some(c) => c.base_cells + c.delta.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Dimension of a cell (0 = face, 2 = hypercell).
    #[inline]
    pub fn dim(&self, cell: usize) -> u8 {
        let (r, c) = self.resolve(cell);
        r.dims[c]
    }
    pub fn boundary(&self, cell: usize) -> &[u64] {
        let (r, c) = self.resolve(cell);
        let (s, e) = r.offsets.range(c);
        let sl = r.indices.slice(s, e);
        debug_assert!(
            sl.windows(2).all(|w| w[0] <= w[1]),
            "BCSF boundary slice unsorted"
        );
        sl
    }
    pub fn stalk(&self, cell: usize) -> &[f32] {
        let (r, c) = self.resolve(cell);
        r.arena.stalk(c)
    }
    pub fn morph_weights(&self, cell: usize) -> &[f32] {
        let (r, c) = self.resolve(cell);
        r.arena.morph(c)
    }
    /// V1: discrete exact payload for a cell.
    pub fn discrete(&self, cell: usize) -> &[u8] {
        let (r, c) = self.resolve(cell);
        r.arena.discrete(c)
    }
    /// O(1) null check: true if the cell is valid (not null).
    pub fn valid(&self, cell: usize) -> bool {
        let (r, c) = self.resolve(cell);
        let word = c / 64;
        let bit = c % 64;
        match r.validity.get(word) {
            Some(w) => (w >> bit) & 1 == 1,
            None => false,
        }
    }
    /// Raw payload bytes counted for the <1.5x gate (V1: discrete included).
    pub fn raw_payload_bytes(&self) -> usize {
        if let Some(c) = &self.chain {
            return c.base.raw_payload_bytes() + c.delta.raw_payload_bytes();
        }
        self.indices.indices.len() * 8
            + self.arena.stalks.len() * 4
            + self.arena.morphs.len() * 4
            + self.arena.discrete.len()
    }
    /// Total resident bytes of the hot-path arrays.
    pub fn resident_bytes(&self) -> usize {
        if let Some(c) = &self.chain {
            return c.base.resident_bytes() + c.delta.resident_bytes();
        }
        self.raw_payload_bytes()
            + self.offsets.offsets.len() * 4
            + self.dims.len()
            + self.arena.stalk_offsets.len() * 4
            + self.arena.morph_cells.len() * 4
            + self.arena.morph_offsets.len() * 4
    }
    /// Capacity-based resident bytes (true allocation incl. Vec slack).
    /// Equals resident_bytes() after the builder freeze step; a gap here
    /// means growth slack is hiding real memory from the gate.
    pub fn resident_capacity_bytes(&self) -> usize {
        if let Some(c) = &self.chain {
            return c.base.resident_capacity_bytes() + c.delta.resident_capacity_bytes();
        }
        self.indices.indices.capacity() * 8
            + self.arena.stalks.capacity() * 4
            + self.arena.morphs.capacity() * 4
            + self.arena.discrete.capacity()
            + self.offsets.offsets.capacity() * 4
            + self.dims.capacity()
            + self.arena.stalk_offsets.capacity() * 4
            + self.arena.morph_cells.capacity() * 4
            + self.arena.morph_offsets.capacity() * 4
    }
    /// Materialize a chained reader into a FLAT reader (B5 escape hatch:
    /// consumers that need the raw slabs - e.g. the segment writer - use
    /// this; the hot read path never does).
    pub fn flatten(&self) -> BcsfReader {
        if self.chain.is_none() {
            return BcsfReader {
                dims: self.dims.clone(),
                offsets: CellOffsets { offsets: self.offsets.offsets.clone() },
                indices: IncidenceIndices { indices: self.indices.indices.clone() },
                arena: StalkArena {
                    stalk_offsets: self.arena.stalk_offsets.clone(),
                    stalks: self.arena.stalks.clone(),
                    morph_cells: self.arena.morph_cells.clone(),
                    morph_offsets: self.arena.morph_offsets.clone(),
                    morphs: self.arena.morphs.clone(),
                    discrete: self.arena.discrete.clone(),
                    validity: self.arena.validity.clone(),
                },
                validity: self.validity.clone(),
                chain: None,
            };
        };
        let mut b = super::builder::BcsfBuilder::new();
        for cell in 0..self.len() {
            let dim = self.dim(cell);
            let boundary: Vec<u64> = self.boundary(cell).to_vec();
            let disc: [u8; 16] = {
                let d = self.discrete(cell);
                let mut a = [0u8; 16];
                a.copy_from_slice(&d[..16]);
                a
            };
            let stalk = self.stalk(cell).to_vec();
            let morph = self.morph_weights(cell).to_vec();
            b.push(dim, boundary, &disc, &stalk, &morph);
        }
        b.finish()
    }

    /// Structural validation: runs in release, called by the E2E bench.
    /// Closes dangling-boundary, misaligned-slab, and unsorted-morph classes.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(c) = &self.chain {
            c.base.validate()?;
            // B5: the delta is a FRAGMENT - its boundary ids point at BASE
            // cells, so it is validated against the base's id space, not
            // its own.
            c.delta.validate_fragment(c.base_cells)?;
            if c.base_cells != c.base.len() {
                return Err("chain base_cells != base length".to_string());
            }
            if c.remap.len() != c.base.len() {
                return Err("chain remap does not cover the base".to_string());
            }
            for &r in &c.remap {
                if r != u32::MAX && (r as usize) >= c.base_cells {
                    return Err("chain remap points outside the base".to_string());
                }
            }
            return Ok(());
        }
        self.validate_inner(None)
    }

    /// Validate a FRAGMENT (B5): structurally sound, but its boundary ids
    /// may point outside itself (they refer to the base it will be chained
    /// onto). `max_id` bounds the legal boundary ids.
    pub fn validate_fragment(&self, max_id: usize) -> Result<(), String> {
        if self.chain.is_some() {
            return self.validate();
        }
        self.validate_inner(Some(max_id))
    }

    fn validate_inner(&self, fragment_max_id: Option<usize>) -> Result<(), String> {
        let n = self.dims.len();
        if self.offsets.offsets.len() != n + 1 {
            return Err("offsets length != ncells+1".to_string());
        }
        if self.arena.stalk_offsets.len() != n + 1 {
            return Err("stalk_offsets length != ncells+1".to_string());
        }
        if self.arena.discrete.len() != n * 16 {
            return Err("discrete slab != ncells*16".to_string());
        }
        let validity_words_needed = (n + 63) / 64;
        if self.arena.validity.len() < validity_words_needed {
            return Err("validity bitmap too short for ncells".to_string());
        }
        if self.validity.len() < validity_words_needed {
            return Err("validity bitmap too short for ncells".to_string());
        }
        if self.arena.morph_offsets.len() != self.arena.morph_cells.len() + 1 {
            return Err("morph_offsets length != morph_cells+1".to_string());
        }
        if !self.arena.morph_cells.windows(2).all(|w| w[0] < w[1]) {
            return Err("morph_cells not strictly sorted".to_string());
        }
        if self.arena.morph_cells.iter().any(|&c| (c as usize) >= n) {
            return Err("morph cell out of range".to_string());
        }
        let mut prev = 0usize;
        for (i, w) in self.offsets.offsets.iter().enumerate() {
            let v = *w as usize;
            if v < prev || v > self.indices.indices.len() {
                return Err(format!("offsets not monotonic at cell {}", i));
            }
            prev = v;
        }
        let max_id = fragment_max_id.unwrap_or(n);
        for (i, &id) in self.indices.indices.iter().enumerate() {
            if (id as usize) >= max_id {
                return Err(format!("dangling boundary id at index {}", i));
            }
        }
        Ok(())
    }
}
