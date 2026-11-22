// Timestamp: 2026-11-22_11-53-08 +05:30
// sharding.rs - integration test for open-cover sharding (NF-08).
//
// The test builds a cover of 1000 cells over four shards. It checks the cover
// invariant, that boundary edges have ghost copies, that the ghost value
// equals the owner value, and that a forced disagreement is reported.

use sheaf_vcs::sharding::cover::{build, edge_id, Cell, ShardError};
use sheaf_vcs::sharding::objective::{cut_weight, minimize_cut, modulo_placement};
use sheaf_vcs::sharding::{assign_vertex, glue, glue::GlueConflict};

fn fixture() -> (Vec<Cell>, Vec<(u64, u64)>) {
    let mut cells = Vec::new();
    let mut edges = Vec::new();
    for id in 1..=100u64 {
        cells.push(Cell { id, dim: 0, value: id as f32 });
    }
    for id in 1..100u64 {
        let edge = Cell { id: edge_id(id, id + 1), dim: 1, value: id as f32 * 0.5 };
        cells.push(edge);
        edges.push((id, id + 1));
    }
    (cells, edges)
}

fn fixture_1000() -> (Vec<Cell>, Vec<(u64, u64)>) {
    let mut cells = Vec::new();
    let mut edges = Vec::new();
    for id in 1..=1000u64 {
        cells.push(Cell { id, dim: 0, value: id as f32 });
    }
    for id in 1..1000u64 {
        let edge = Cell { id: edge_id(id, id + 1), dim: 1, value: id as f32 * 0.5 };
        cells.push(edge);
        edges.push((id, id + 1));
    }
    (cells, edges)
}

#[test]
fn every_cell_belongs_to_at_least_one_shard() {
    let (cells, edges) = fixture();
    let cover = build(&cells, &edges, 4).expect("cover");
    assert_eq!(cover.shards.len(), 4);
    assert!(cover.covers(&cells));
    assert!(cover.conflicts.is_empty());
    assert_eq!(cover.owner_count(), cells.len());
}

#[test]
fn boundary_edges_have_ghost_copies() {
    let (cells, edges) = fixture();
    let cover = build(&cells, &edges, 4).expect("cover");
    assert!(cover.ghost_count() > 0, "cross-shard edges must be ghosted");
    for shard in &cover.shards {
        for (id, ghost) in &shard.ghosts {
            let owner = cover.shards.iter().find(|s| s.id == ghost.owner).expect("owner shard");
            let cell = owner.cells.get(id).expect("owner cell");
            assert!(ghost.matches(cell), "ghost {id} must equal its owner");
            assert_ne!(shard.id, ghost.owner);
        }
    }
}

#[test]
fn a_forced_glue_conflict_is_reported() {
    let (cells, edges) = fixture();
    let mut cover = build(&cells, &edges, 4).expect("cover");
    let target_index = cover
        .shards
        .iter()
        .position(|shard| !shard.ghosts.is_empty())
        .expect("a shard with ghosts");
    let (cell_id, expected_owner, expected_owner_value) = {
        let shard = &mut cover.shards[target_index];
        let (cell_id, ghost) = shard.ghosts.iter_mut().next().expect("one ghost");
        let owner = ghost.owner;
        let value = ghost.cell.value;
        ghost.cell.value += 100.0;
        (*cell_id, owner, value)
    };
    let shard = cover.shards[target_index].id;
    let conflicts = glue(&cover.shards);
    assert!(!conflicts.is_empty());
    assert!(conflicts.contains(&GlueConflict {
        cell_id,
        shard,
        owner: expected_owner,
        owner_value: expected_owner_value,
        ghost_value: expected_owner_value + 100.0,
    }));
}

#[test]
fn zero_shards_and_missing_endpoints_are_rejected() {
    let (cells, edges) = fixture();
    assert_eq!(build(&cells, &edges, 0).err(), Some(ShardError::NoShards));
    assert_eq!(build(&cells, &[(1, 9999)], 4).err(), Some(ShardError::MissingEndpoint));
}

#[test]
fn placement_is_stable_across_builds() {
    let (cells, edges) = fixture();
    let first = build(&cells, &edges, 4).expect("cover");
    let second = build(&cells, &edges, 4).expect("cover");
    for (left, right) in first.shards.iter().zip(&second.shards) {
        assert_eq!(left.cells.keys().collect::<Vec<_>>(), right.cells.keys().collect::<Vec<_>>());
        assert_eq!(left.ghosts.keys().collect::<Vec<_>>(), right.ghosts.keys().collect::<Vec<_>>());
    }
    assert_eq!(assign_vertex(1, 4), assign_vertex(1, 4));
}

#[test]
fn cut_weight_objective_beats_modulo_baseline() {
    let (cells, edges) = fixture_1000();
    let vertices: Vec<u64> = cells.iter().filter(|cell| cell.dim == 0).map(|cell| cell.id).collect();

    let objective = minimize_cut(&vertices, &edges, 4);
    let baseline = modulo_placement(&vertices, 4);
    let objective_cut = cut_weight(&objective, &edges);
    let baseline_cut = cut_weight(&baseline, &edges);

    eprintln!("C08 CUT WEIGHTS objective={objective_cut} modulo={baseline_cut}");
    assert!(
        objective_cut < baseline_cut,
        "objective cut {objective_cut} must be lower than modulo cut {baseline_cut}"
    );

    // The cover must use the objective placement: one ghost per cut edge.
    let cover = build(&cells, &edges, 4).expect("cover");
    assert_eq!(cover.ghost_count(), objective_cut);
    assert!(cover.ghost_count() < baseline_cut);
}
