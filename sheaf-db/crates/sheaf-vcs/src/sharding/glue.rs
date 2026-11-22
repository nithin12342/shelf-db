// glue.rs - checked gluing of ghost copies (NF-08).

use super::cover::Shard;

/// A disagreement between an owner and a ghost, or between two ghosts.
#[derive(Clone, Debug, PartialEq)]
pub struct GlueConflict {
    pub cell_id: u64,
    pub shard: usize,
    pub owner: usize,
    pub owner_value: f32,
    pub ghost_value: f32,
}

/// Compare every ghost with its owner. A conflict is reported, not hidden.
pub fn glue(shards: &[Shard]) -> Vec<GlueConflict> {
    let mut conflicts = Vec::new();
    for shard in shards {
        for (cell_id, ghost) in &shard.ghosts {
            let Some(owner_shard) = shards.iter().find(|candidate| candidate.id == ghost.owner) else {
                continue;
            };
            let Some(owner_cell) = owner_shard.cells.get(cell_id) else {
                continue;
            };
            if !ghost.matches(owner_cell) {
                conflicts.push(GlueConflict {
                    cell_id: *cell_id,
                    shard: shard.id,
                    owner: ghost.owner,
                    owner_value: owner_cell.value,
                    ghost_value: ghost.cell.value,
                });
            }
        }
    }
    conflicts
}
