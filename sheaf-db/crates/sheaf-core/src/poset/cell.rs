// crates/sheaf-core/src/poset/cell.rs — define graded cell descriptors
// FILE-002. Must never store stalks or matrices.
use super::id::CellId;

/// Minimal Phase-1 cell header. Stalks/morphisms live in BCSF arena, not here.
#[derive(Clone, Copy, Debug)]
pub struct Cell {
    pub id: CellId,
    pub dim: u8,
}

impl Cell {
    pub fn new(id: CellId, dim: u8) -> Self {
        Self { id, dim }
    }
}
