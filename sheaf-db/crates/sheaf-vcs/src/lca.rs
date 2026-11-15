// lca.rs - logical collision avoidance and chronological writing (Pillar 10).
//
// Every cell v has a star neighborhood Star(v). Two writes whose stars do not
// overlap are mathematically orthogonal:
//     Star(v_A) intersect Star(v_B) = empty
// The engine routes them to parallel core-pinned workers with zero locking.
// Writes whose stars overlap are routed to an entity-local chronological time
// chart (see chart.rs) instead of an exclusive mutex.
//
// This module is the topological overlap inspector. It decides the route for
// a pair of cells and partitions a batch into parallel waves. Each wave is an
// independent set of mutually orthogonal cells.

use std::collections::BTreeSet;

use crate::complex::Complex;

/// The route chosen for a pair of concurrent writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// Disjoint stars. The writes run in parallel with no coordination.
    Parallel,
    /// Overlapping stars. The writes serialize on a chronological chart.
    Chronological,
}

/// Inspects star-neighborhood overlap on a fixed complex.
pub struct CollisionInspector<'a> {
    complex: &'a Complex,
}

impl<'a> CollisionInspector<'a> {
    /// Bind the inspector to a complex topology.
    pub fn new(complex: &'a Complex) -> Self {
        Self { complex }
    }

    /// The cells shared by the two star neighborhoods. An empty set means the
    /// writes are orthogonal.
    pub fn shared_cells(&self, a: u64, b: u64) -> BTreeSet<u64> {
        let sa = self.complex.star(a);
        let sb = self.complex.star(b);
        sa.intersection(&sb).copied().collect()
    }

    /// Decide the route for two cells. Disjoint stars route to parallel
    /// execution. Any shared cell routes to the chronological chart.
    pub fn route(&self, a: u64, b: u64) -> Route {
        if self.shared_cells(a, b).is_empty() {
            Route::Parallel
        } else {
            Route::Chronological
        }
    }

    /// Partition a batch of target cells into parallel waves. Each wave holds
    /// cells that are pairwise orthogonal, so the whole wave runs at once.
    /// Conflicting cells fall into later waves, which the chart orders in time.
    /// The greedy rule keeps the first wave as large as possible.
    pub fn parallel_waves(&self, cells: &[u64]) -> Vec<Vec<u64>> {
        let mut waves: Vec<Vec<u64>> = Vec::new();
        for &cell in cells {
            let mut placed = false;
            for wave in waves.iter_mut() {
                if wave.iter().all(|&other| self.route(cell, other) == Route::Parallel) {
                    wave.push(cell);
                    placed = true;
                    break;
                }
            }
            if !placed {
                waves.push(vec![cell]);
            }
        }
        waves
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Restriction;

    fn two_disjoint_edges() -> Complex {
        let mut c = Complex::new(vec![1, 1, 1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(2, 3, Restriction::identity(1), Restriction::identity(1));
        c
    }

    fn triangle() -> Complex {
        let mut c = Complex::new(vec![1, 1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(1, 2, Restriction::identity(1), Restriction::identity(1));
        c.add_edge(2, 0, Restriction::identity(1), Restriction::identity(1));
        c
    }

    #[test]
    fn disjoint_stars_route_parallel() {
        let c = two_disjoint_edges();
        let ins = CollisionInspector::new(&c);
        assert_eq!(ins.route(0, 2), Route::Parallel);
        assert!(ins.shared_cells(0, 2).is_empty());
    }

    #[test]
    fn adjacent_stars_route_chronological() {
        let c = two_disjoint_edges();
        let ins = CollisionInspector::new(&c);
        // Vertices 0 and 1 share edge id 4.
        assert_eq!(ins.route(0, 1), Route::Chronological);
        assert!(ins.shared_cells(0, 1).contains(&4));
    }

    #[test]
    fn same_cell_conflicts_with_itself() {
        let c = triangle();
        let ins = CollisionInspector::new(&c);
        assert_eq!(ins.route(1, 1), Route::Chronological);
    }

    #[test]
    fn orthogonal_batch_fits_one_wave() {
        let c = two_disjoint_edges();
        let ins = CollisionInspector::new(&c);
        let waves = ins.parallel_waves(&[0, 2]);
        assert_eq!(waves, vec![vec![0u64, 2]]);
    }

    #[test]
    fn triangle_vertices_need_three_waves() {
        let c = triangle();
        let ins = CollisionInspector::new(&c);
        let waves = ins.parallel_waves(&[0, 1, 2]);
        // Every pair of triangle vertices shares an edge, so none can parallel.
        assert_eq!(waves.len(), 3);
        assert!(waves.iter().all(|w| w.len() == 1));
    }
}