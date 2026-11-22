// objective.rs - deterministic incidence cut-weight objective for the open cover (C-08, spec Pillar 7).
// Timestamp: 2026-11-22_12-45-18 +05:30.
//
// The spec Pillar 7 requires a canonical open poset cover whose slices minimize
// incidence cut weight. This module replaces modulo placement with a seeded,
// deterministic local search over a graph of vertices and boundary edges.
// The cut weight is the count of edges whose endpoints land in different shards.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Fixed seed for deterministic multi-start search. Never read from the clock.
pub const FIXED_SEED: u64 = 0x9E37_79B9_7F4A_7C15;

/// Number of extra seeded starts. Bounded so the test stays fast and repeatable.
const EXTRA_STARTS: usize = 4;

/// Count edges whose endpoints are assigned to different shards.
/// An edge with a missing endpoint contributes nothing. `build` rejects such edges.
pub fn cut_weight(assignment: &BTreeMap<u64, usize>, edges: &[(u64, u64)]) -> usize {
    edges
        .iter()
        .filter(|(u, v)| match (assignment.get(u), assignment.get(v)) {
            (Some(left), Some(right)) => left != right,
            _ => false,
        })
        .count()
}

/// The modulo baseline placement, kept only as a comparison reference.
pub fn modulo_placement(vertices: &[u64], shard_count: usize) -> BTreeMap<u64, usize> {
    vertices
        .iter()
        .map(|&vertex| (vertex, super::assign::assign_vertex(vertex, shard_count)))
        .collect()
}

/// Assign vertices to shards so that the incidence cut weight is small.
/// The result is a deterministic function of the inputs and `FIXED_SEED`.
pub fn minimize_cut(
    vertices: &[u64],
    edges: &[(u64, u64)],
    shard_count: usize,
) -> BTreeMap<u64, usize> {
    if shard_count == 0 || vertices.is_empty() {
        return BTreeMap::new();
    }

    // Keep only edges whose endpoints exist in the vertex set. This keeps the
    // search total even when `build` later reports a missing endpoint error.
    let present: BTreeSet<u64> = vertices.iter().copied().collect();
    let active_edges: Vec<(u64, u64)> = edges
        .iter()
        .copied()
        .filter(|(u, v)| u != v && present.contains(u) && present.contains(v))
        .collect();

    let sorted_vertices: Vec<u64> = present.iter().copied().collect();
    let adjacency = build_adjacency(&sorted_vertices, &active_edges);

    // Candidate zero: a breadth-first traversal cut into contiguous blocks.
    // On a path graph this gives a near-optimal cut with few boundary edges.
    let mut best = bfs_block_assignment(&sorted_vertices, &adjacency, shard_count);
    local_search(&mut best, &adjacency, shard_count);
    let mut best_cost = cut_weight(&best, &active_edges);

    // Extra starts come from a seeded generator. Better candidates replace the
    // incumbent only on a strict improvement, so ties keep candidate zero.
    let mut rng = SplitMix64::new(FIXED_SEED);
    for _ in 0..EXTRA_STARTS {
        let mut candidate = seeded_assignment(&sorted_vertices, shard_count, &mut rng);
        local_search(&mut candidate, &adjacency, shard_count);
        let cost = cut_weight(&candidate, &active_edges);
        if cost < best_cost {
            best = candidate;
            best_cost = cost;
        }
    }
    best
}

/// Build a sorted, de-duplicated adjacency map for the active graph.
fn build_adjacency(
    vertices: &[u64],
    edges: &[(u64, u64)],
) -> BTreeMap<u64, Vec<u64>> {
    let mut adjacency: BTreeMap<u64, Vec<u64>> =
        vertices.iter().map(|&v| (v, Vec::new())).collect();
    for (u, v) in edges {
        adjacency.get_mut(u).expect("endpoint is present").push(*v);
        adjacency.get_mut(v).expect("endpoint is present").push(*u);
    }
    for neighbors in adjacency.values_mut() {
        neighbors.sort_unstable();
        neighbors.dedup();
    }
    adjacency
}

/// Assign contiguous breadth-first blocks to shards in ascending shard order.
fn bfs_block_assignment(
    vertices: &[u64],
    adjacency: &BTreeMap<u64, Vec<u64>>,
    shard_count: usize,
) -> BTreeMap<u64, usize> {
    let mut seen = BTreeSet::new();
    let mut order = Vec::with_capacity(vertices.len());
    let mut queue = VecDeque::new();
    for &start in vertices {
        if seen.contains(&start) {
            continue;
        }
        seen.insert(start);
        queue.push_back(start);
        while let Some(vertex) = queue.pop_front() {
            order.push(vertex);
            if let Some(neighbors) = adjacency.get(&vertex) {
                for &neighbor in neighbors {
                    if !seen.contains(&neighbor) {
                        seen.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }
    let total = order.len();
    order
        .into_iter()
        .enumerate()
        .map(|(index, vertex)| (vertex, (index * shard_count) / total))
        .collect()
}

/// Produce a deterministic random placement from the seeded generator.
fn seeded_assignment(
    vertices: &[u64],
    shard_count: usize,
    rng: &mut SplitMix64,
) -> BTreeMap<u64, usize> {
    vertices
        .iter()
        .map(|&vertex| (vertex, (rng.next_u64() % shard_count as u64) as usize))
        .collect()
}

/// Move vertices one at a time while each move strictly lowers the cut weight.
/// Moves stop at a local minimum. Order and tie-breaks are fixed for determinism.
fn local_search(
    assignment: &mut BTreeMap<u64, usize>,
    adjacency: &BTreeMap<u64, Vec<u64>>,
    shard_count: usize,
) {
    if shard_count < 2 {
        return;
    }
    let vertices: Vec<u64> = assignment.keys().copied().collect();
    loop {
        let mut improved = false;
        for &vertex in &vertices {
            let current = assignment[&vertex];
            let mut best_target = current;
            let mut best_delta = 0_i64;
            for target in 0..shard_count {
                let delta = if target == current {
                    0
                } else {
                    move_delta(vertex, target, current, adjacency, assignment)
                };
                if delta < best_delta || (delta == best_delta && delta < 0 && target < best_target) {
                    best_delta = delta;
                    best_target = target;
                }
            }
            // A zero-gain move is never applied, so the loop terminates.
            if best_delta < 0 && best_target != current {
                assignment.insert(vertex, best_target);
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }
}

/// Change in cut weight when `vertex` moves from `current` to `target`.
fn move_delta(
    vertex: u64,
    target: usize,
    current: usize,
    adjacency: &BTreeMap<u64, Vec<u64>>,
    assignment: &BTreeMap<u64, usize>,
) -> i64 {
    let mut delta = 0_i64;
    if let Some(neighbors) = adjacency.get(&vertex) {
        for &neighbor in neighbors {
            let neighbor_shard = assignment[&neighbor];
            let before = i64::from(current != neighbor_shard);
            let after = i64::from(target != neighbor_shard);
            delta += after - before;
        }
    }
    delta
}

/// SplitMix64 generator. The seed is fixed; no system time enters the result.
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path_fixture(count: u64) -> (Vec<u64>, Vec<(u64, u64)>) {
        let vertices: Vec<u64> = (1..=count).collect();
        let edges: Vec<(u64, u64)> = (1..count).map(|id| (id, id + 1)).collect();
        (vertices, edges)
    }

    #[test]
    fn cut_weight_counts_only_cross_shard_edges() {
        let assignment = BTreeMap::from([(1, 0), (2, 0), (3, 1)]);
        assert_eq!(cut_weight(&assignment, &[(1, 2), (2, 3)]), 1);
    }

    #[test]
    fn placement_is_deterministic() {
        let (vertices, edges) = path_fixture(64);
        let first = minimize_cut(&vertices, &edges, 4);
        let second = minimize_cut(&vertices, &edges, 4);
        assert_eq!(first, second);
    }

    #[test]
    fn objective_beats_modulo_on_1000_vertex_path() {
        let (vertices, edges) = path_fixture(1000);
        let objective = minimize_cut(&vertices, &edges, 4);
        let baseline = modulo_placement(&vertices, 4);
        let objective_cut = cut_weight(&objective, &edges);
        let baseline_cut = cut_weight(&baseline, &edges);
        eprintln!("C08 CUT WEIGHTS objective={objective_cut} modulo={baseline_cut}");
        assert!(
            objective_cut < baseline_cut,
            "objective cut {objective_cut} must be lower than modulo cut {baseline_cut}"
        );
    }
}
