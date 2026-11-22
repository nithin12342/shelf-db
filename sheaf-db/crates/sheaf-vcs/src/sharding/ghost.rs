// Timestamp: 2026-11-22_11-53-08 +05:30
// ghost.rs - replicated boundary cells (NF-08).

use super::cover::Cell;

/// A ghost copy of a cell owned by another shard.
#[derive(Clone, Debug, PartialEq)]
pub struct GhostCell {
    /// The cell value copied at build time.
    pub cell: Cell,
    /// The shard that owns the cell.
    pub owner: usize,
    /// The restricted source endpoint.
    pub source: Cell,
    /// The restricted target endpoint.
    pub target: Cell,
}

impl GhostCell {
    pub fn new(cell: Cell, owner: usize, source: Cell, target: Cell) -> Self {
        Self { cell, owner, source, target }
    }

    /// The ghost equals its owner's boundary restriction.
    pub fn matches(&self, owner_value: &Cell) -> bool {
        self.cell == *owner_value && self.source.dim == 0 && self.target.dim == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ghost_matches_its_owner() {
        let cell = Cell { id: 9, dim: 1, value: 3.0 };
        let a = Cell { id: 1, dim: 0, value: 0.0 };
        let b = Cell { id: 2, dim: 0, value: 1.0 };
        let ghost = GhostCell::new(cell.clone(), 1, a, b);
        assert!(ghost.matches(&cell));
        let different = Cell { id: 9, dim: 1, value: 4.0 };
        assert!(!ghost.matches(&different));
    }
}
