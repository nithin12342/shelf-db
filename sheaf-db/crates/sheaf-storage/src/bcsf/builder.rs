// crates/sheaf-storage/src/bcsf/builder.rs — build immutable BCSF segments
// Phase-1 helper (compactor precursor). Appends cells in index order.
// V4: every offset narrowing uses try_from().expect(), never silent `as`.
use super::arena::StalkArena;
use super::indices::IncidenceIndices;
use super::offsets::CellOffsets;
use super::reader::BcsfReader;

pub struct BcsfBuilder {
    dims: Vec<u8>,
    offsets: Vec<u32>,
    indices: Vec<u64>,
    stalk_offsets: Vec<u32>,
    stalks: Vec<f32>,
    morph_cells: Vec<u32>,
    morph_offsets: Vec<u32>,
    morphs: Vec<f32>,
    discrete: Vec<u8>,
    /// Null bitmap: 1 bit per cell, 1 = valid. Grows one bit per push.
    validity: Vec<u64>,
}

impl Default for BcsfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BcsfBuilder {
    pub fn new() -> Self {
        Self {
            dims: Vec::new(),
            offsets: vec![0],
            indices: Vec::new(),
            stalk_offsets: vec![0],
            stalks: Vec::new(),
            morph_cells: Vec::new(),
            morph_offsets: vec![0],
            morphs: Vec::new(),
            discrete: Vec::new(),
            validity: Vec::new(),
        }
    }
    /// boundary must be sorted; discrete is the 16-byte exact payload (V1);
    /// stalk = continuous vector; morph = flat W (empty for 0-cells, V6 sparse).
    pub fn push(&mut self, dim: u8, mut boundary: Vec<u64>, discrete: &[u8; 16], stalk: &[f32], morph: &[f32]) {
        boundary.sort_unstable();
        let cell = self.dims.len() as u32;
        self.dims.push(dim);
        self.indices.extend_from_slice(&boundary);
        self.offsets.push(u32::try_from(self.indices.len()).expect("BCSF indices overflow u32"));
        self.stalks.extend_from_slice(stalk);
        self.stalk_offsets.push(u32::try_from(self.stalks.len()).expect("BCSF stalks overflow u32"));
        // Mark the new cell as valid (set bit).
        let word = cell / 64;
        let bit = cell % 64;
        if word as usize >= self.validity.len() {
            self.validity.resize((word + 1) as usize, 0);
        }
        self.validity[word as usize] |= 1u64 << bit;
        if !morph.is_empty() {
            self.morph_cells.push(cell);
            self.morphs.extend_from_slice(morph);
            self.morph_offsets.push(u32::try_from(self.morphs.len()).expect("BCSF morphs overflow u32"));
        }
        self.discrete.extend_from_slice(discrete);
    }

    /// Push a cell using borrowed boundary slice without allocating a heap Vec.
    pub fn push_slice(&mut self, dim: u8, boundary: &[u64], discrete: &[u8; 16], stalk: &[f32], morph: &[f32]) {
        let cell = self.dims.len() as u32;
        self.dims.push(dim);
        let start_idx = self.indices.len();
        self.indices.extend_from_slice(boundary);
        self.indices[start_idx..].sort_unstable();
        self.offsets.push(u32::try_from(self.indices.len()).expect("BCSF indices overflow u32"));
        self.stalks.extend_from_slice(stalk);
        self.stalk_offsets.push(u32::try_from(self.stalks.len()).expect("BCSF stalks overflow u32"));
        // Mark the new cell as valid (set bit).
        let word = cell / 64;
        let bit = cell % 64;
        if word as usize >= self.validity.len() {
            self.validity.resize((word + 1) as usize, 0);
        }
        self.validity[word as usize] |= 1u64 << bit;
        if !morph.is_empty() {
            self.morph_cells.push(cell);
            self.morphs.extend_from_slice(morph);
            self.morph_offsets.push(u32::try_from(self.morphs.len()).expect("BCSF morphs overflow u32"));
        }
        self.discrete.extend_from_slice(discrete);
    }
    /// Bulk merge (METHOD-017): copy surviving cells from a frozen reader,
    /// rebasing offsets into this builder. `live` marks surviving cells
    /// (`None` copies all); boundary ids pass through `remap`, which maps
    /// the referenced id space (old global id -> new cell index) and must
    /// cover every referenced id — it may be larger than the source when
    /// batch cells reference base ids (Phase-2 two-level merge).
    /// Slice copies per cell (memcpy), never field-by-field; the compactor
    /// chunks calls over cell ranges for cache isolation.
    pub fn extend_from_reader(&mut self, reader: &BcsfReader, live: Option<&[bool]>, remap: &[u32]) {
        let n = reader.len();
        self.extend_range(reader, live, remap, 0, n);
    }

    /// Ranged bulk merge: one isolation quantum of the compactor. Small
    /// ranges let the merge thread yield between chunks instead of issuing
    /// one 50 MB memcpy that starves reader cores of bandwidth.
    pub fn extend_range(
        &mut self,
        reader: &BcsfReader,
        live: Option<&[bool]>,
        remap: &[u32],
        start: usize,
        end: usize,
    ) {
        let n = reader.len();
        assert!(start <= end && end <= n, "merge range out of bounds");
        if let Some(l) = live {
            assert_eq!(l.len(), n, "live mask must cover every source cell");
        }
        for cell in start..end {
            if live.map(|l| l[cell]).unwrap_or(true) {
                let new_idx = u32::try_from(self.dims.len()).expect("BCSF cell overflow u32");
                self.dims.push(reader.dim(cell));
                self.indices.extend(
                    reader.boundary(cell).iter().map(|id| {
                        *remap.get(*id as usize).expect("boundary id outside remap space") as u64
                    }),
                );
                self.offsets.push(u32::try_from(self.indices.len()).expect("BCSF indices overflow u32"));
                self.stalks.extend_from_slice(reader.stalk(cell));
                self.stalk_offsets.push(u32::try_from(self.stalks.len()).expect("BCSF stalks overflow u32"));
                let m = reader.morph_weights(cell);
                if !m.is_empty() {
                    self.morph_cells.push(new_idx);
                    self.morphs.extend_from_slice(m);
                    self.morph_offsets.push(u32::try_from(self.morphs.len()).expect("BCSF morphs overflow u32"));
                }
                self.discrete.extend_from_slice(reader.discrete(cell));
            }
        }
    }
    /// B4: merge directly from a REMAP table (u32::MAX = dropped cell).
    /// The remap already encodes liveness, so no boolean mask is
    /// materialized - the merge does O(cells) work with zero extra copies
    /// (the old path copied a bool mask per merge AND per snapshot).
    pub fn extend_range_remapped(
        &mut self,
        reader: &BcsfReader,
        remap: &[u32],
        start: usize,
        end: usize,
    ) {
        self.extend_range_remapped_with(reader, remap, remap, start, end)
    }

    /// B4: merge with TWO maps - `cell_map` maps the reader's own cells to
    /// their new indices (u32::MAX = dropped), `id_map` maps the BOUNDARY
    /// ids to their new indices. They differ for batch cells: a batch
    /// cell's boundary refers to BASE cells, so its id map is the base
    /// remap, not the batch translation.
    pub fn extend_range_remapped_with(
        &mut self,
        reader: &BcsfReader,
        cell_map: &[u32],
        id_map: &[u32],
        start: usize,
        end: usize,
    ) {
        let n = reader.len();
        assert!(start <= end && end <= n, "merge range out of bounds");
        assert_eq!(cell_map.len(), n, "cell map must cover every source cell");
        for (cell, &new_idx) in cell_map.iter().enumerate().take(end).skip(start) {
            if new_idx == u32::MAX {
                continue; // dropped (tombstoned) cell
            }
            self.dims.push(reader.dim(cell));
            self.indices.extend(
                reader.boundary(cell).iter().map(|id| {
                    *id_map.get(*id as usize).expect("boundary id outside remap space") as u64
                }),
            );
            self.offsets.push(u32::try_from(self.indices.len()).expect("BCSF indices overflow u32"));
            self.stalks.extend_from_slice(reader.stalk(cell));
            self.stalk_offsets.push(u32::try_from(self.stalks.len()).expect("BCSF stalks overflow u32"));
            let m = reader.morph_weights(cell);
            if !m.is_empty() {
                // the morph cell must point at a cell of THIS reader
                self.morph_cells.push((self.dims.len() - 1) as u32);
                self.morphs.extend_from_slice(m);
                self.morph_offsets.push(u32::try_from(self.morphs.len()).expect("BCSF morphs overflow u32"));
            }
            self.discrete.extend_from_slice(reader.discrete(cell));
        }
    }

    /// B4: bulk merge from a remap table (whole reader).
    pub fn extend_from_reader_remapped(&mut self, reader: &BcsfReader, remap: &[u32]) {
        self.extend_range_remapped(reader, remap, 0, reader.len());
    }

    /// B4: bulk merge with separate cell map and boundary-id map.
    pub fn extend_from_reader_remapped_with(
        &mut self,
        reader: &BcsfReader,
        cell_map: &[u32],
        id_map: &[u32],
    ) {
        self.extend_range_remapped_with(reader, cell_map, id_map, 0, reader.len());
    }

    pub fn len_before_finish(&self) -> usize {
        self.dims.len()
    }
    pub fn finish(self) -> BcsfReader {
        self.finish_inner(true)
    }

    pub fn finish_fragment(self) -> BcsfReader {
        self.finish_inner(true)
    }
    /// Seal path (METHOD-012): no shrink_to_fit — the 9-vec realloc memcpy
    /// is what stalled P99 writes (measured 1.5ms). Shrink happens once per
    /// frozen output in finish()/compactor, never per seal.
    pub fn finish_live(self) -> BcsfReader {
        self.finish_inner(false)
    }
    fn finish_inner(mut self, shrink: bool) -> BcsfReader {
        // Freeze step: release Vec growth slack so resident_bytes measures
        // the true allocation (capacity == len after this). Skipped on the
        // seal path (see finish_live).
        if shrink {
            self.dims.shrink_to_fit();
            self.offsets.shrink_to_fit();
            self.indices.shrink_to_fit();
            self.stalk_offsets.shrink_to_fit();
            self.stalks.shrink_to_fit();
            self.morph_cells.shrink_to_fit();
            self.morph_offsets.shrink_to_fit();
            self.morphs.shrink_to_fit();
            self.discrete.shrink_to_fit();
            self.validity.shrink_to_fit();
        }
        let r = BcsfReader {
            dims: self.dims,
            offsets: CellOffsets { offsets: self.offsets },
            indices: IncidenceIndices { indices: self.indices },
            arena: StalkArena {
                stalk_offsets: self.stalk_offsets,
                stalks: self.stalks,
                morph_cells: self.morph_cells,
                morph_offsets: self.morph_offsets,
                morphs: self.morphs,
                discrete: self.discrete,
                validity: self.validity.clone(),
            },
            validity: self.validity,
            chain: None,
        };
        debug_assert!(r.validate().is_ok(), "BCSF structural invariant broken: {:?}", r.validate().err());
        r
    }
}
