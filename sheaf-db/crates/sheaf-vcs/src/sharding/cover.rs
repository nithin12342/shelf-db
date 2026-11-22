// Timestamp: 2026-11-22_11-53-08 +05:30
// cover.rs - the shard cover and its invariants (NF-08).

use super::ghost::GhostCell;
use super::glue::{glue, GlueConflict};
use std::collections::BTreeMap;
use std::fmt;

/// One cell of the complex. A value is the stalk coordinate used for gluing.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub id: u64,
    pub dim: u8,
    pub value: f32,
}

/// One shard of the cover.
#[derive(Clone, Debug, Default)]
pub struct Shard {
    pub id: usize,
    pub cells: BTreeMap<u64, Cell>,
    pub ghosts: BTreeMap<u64, GhostCell>,
}

/// The checked cover.
#[derive(Clone, Debug)]
pub struct Cover {
    pub shards: Vec<Shard>,
    pub conflicts: Vec<GlueConflict>,
}

/// Why the cover could not be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShardError {
    /// The shard count is zero.
    NoShards,
    /// An edge endpoint is missing from the cell set.
    MissingEndpoint,
    /// A vertex and an edge share an identifier.
    DuplicateCell,
}

impl fmt::Display for ShardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShardError::NoShards => f.write_str("a cover needs at least one shard"),
            ShardError::MissingEndpoint => f.write_str("an edge names a missing endpoint"),
            ShardError::DuplicateCell => f.write_str("a cell identifier is used twice"),
        }
    }
}

impl std::error::Error for ShardError {}

impl Cover {
    /// True when every cell is present in at least one shard.
    pub fn covers(&self, cells: &[Cell]) -> bool {
        cells.iter().all(|cell| {
            self.shards.iter().any(|shard| {
                shard.cells.contains_key(&cell.id) || shard.ghosts.contains_key(&cell.id)
            })
        })
    }

    /// The number of ghost copies across the cover.
    pub fn ghost_count(&self) -> usize {
        self.shards.iter().map(|shard| shard.ghosts.len()).sum()
    }

    /// The number of cells held as owners across the cover.
    pub fn owner_count(&self) -> usize {
        self.shards.iter().map(|shard| shard.cells.len()).sum()
    }
}

/// Build a cover from vertices and edges.
pub fn build(cells: &[Cell], edges: &[(u64, u64)], shard_count: usize) -> Result<Cover, ShardError> {
    if shard_count == 0 {
        return Err(ShardError::NoShards);
    }
    let mut vertices = BTreeMap::new();
    let mut edge_cells = BTreeMap::new();
    for cell in cells {
        if cell.dim == 0 {
            if vertices.insert(cell.id, cell.clone()).is_some() {
                return Err(ShardError::DuplicateCell);
            }
        } else if edge_cells.insert(cell.id, cell.clone()).is_some() {
            return Err(ShardError::DuplicateCell);
        }
    }
    // The canonical cover places cells with the seeded cut-weight objective
    // from spec Pillar 7. Modulo placement stays only as a comparison helper.
    let vertex_ids: Vec<u64> = vertices.keys().copied().collect();
    let placement = super::objective::minimize_cut(&vertex_ids, edges, shard_count);

    let mut shards: Vec<Shard> = (0..shard_count).map(|id| Shard { id, ..Shard::default() }).collect();
    for vertex in vertices.values() {
        let shard = placement[&vertex.id];
        shards[shard].cells.insert(vertex.id, vertex.clone());
    }
    for (u, v) in edges {
        let left = vertices.get(u).ok_or(ShardError::MissingEndpoint)?;
        let right = vertices.get(v).ok_or(ShardError::MissingEndpoint)?;
        let edge = edge_cells.get(&edge_id(*u, *v)).cloned().ok_or(ShardError::MissingEndpoint)?;
        let left_shard = placement[u];
        let right_shard = placement[v];
        if left_shard == right_shard {
            shards[left_shard].cells.insert(edge.id, edge.clone());
        } else {
            // The edge is a boundary cell. Both shards hold a ghost copy of
            // the same owner value; the smaller shard id is the owner.
            let owner = left_shard.min(right_shard);
            shards[owner].cells.insert(edge.id, edge.clone());
            for shard_id in [left_shard, right_shard] {
                if shard_id != owner {
                    shards[shard_id].ghosts.insert(
                        edge.id,
                        GhostCell::new(edge.clone(), owner, left.clone(), right.clone()),
                    );
                }
            }
        }
    }
    let conflicts = glue(&shards);
    Ok(Cover { shards, conflicts })
}

/// A stable identifier for an edge.
pub fn edge_id(u: u64, v: u64) -> u64 {
    let (low, high) = (u.min(v), u.max(v));
    low.wrapping_mul(0x9e37_79b9).wrapping_add(high)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_shard_holds_every_cell() {
        let cells = vec![
            Cell { id: 1, dim: 0, value: 0.0 },
            Cell { id: 2, dim: 0, value: 1.0 },
            Cell { id: edge_id(1, 2), dim: 1, value: 2.0 },
        ];
        let cover = build(&cells, &[(1, 2)], 1).expect("cover");
        assert!(cover.covers(&cells));
        assert_eq!(cover.ghost_count(), 0);
        assert_eq!(cover.owner_count(), 3);
        assert!(cover.conflicts.is_empty());
    }
}
