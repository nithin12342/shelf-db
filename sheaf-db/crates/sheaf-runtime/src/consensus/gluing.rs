// Timestamp: 2026-11-22_11-53-08 +05:30
// gluing.rs - sheaf gluing consensus (C-11).
//
// Spec Pillar 7 states that network agreement does not use Raft voting loops.
// Nodes agree when the boundary sections satisfy delta s = 0. This module
// runs a deterministic Jacobi relaxation on the boundary cochain until the
// coboundary residual is below tolerance. No election and no vote occur.

use std::collections::BTreeMap;
use std::fmt;

/// Why a gluing run could not finish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GluingError {
    /// A boundary names an unknown node.
    UnknownNode,
    /// The tolerance is not positive and finite.
    BadTolerance,
    /// The iteration limit is zero.
    BadIterationLimit,
}

impl fmt::Display for GluingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode => f.write_str("a boundary names an unknown node"),
            Self::BadTolerance => f.write_str("the tolerance must be positive and finite"),
            Self::BadIterationLimit => f.write_str("the iteration limit must be positive"),
        }
    }
}

impl std::error::Error for GluingError {}

/// One node's local section at a cell.
#[derive(Clone, Debug, PartialEq)]
pub struct LocalSection {
    pub cell: String,
    pub value: f64,
}

/// Two nodes joined at a shared boundary cell.
#[derive(Clone, Debug)]
pub struct Boundary {
    pub left: u64,
    pub right: u64,
    pub cell: String,
}

/// Deterministic gluing cluster. There is no leader and no vote.
#[derive(Clone, Debug)]
pub struct GluingCluster {
    nodes: Vec<u64>,
    sections: BTreeMap<(u64, String), f64>,
    boundaries: Vec<Boundary>,
}

impl GluingCluster {
    pub fn new(nodes: &[u64]) -> Self {
        Self {
            nodes: nodes.to_vec(),
            sections: BTreeMap::new(),
            boundaries: Vec::new(),
        }
    }

    pub fn add_boundary(&mut self, boundary: Boundary) -> Result<(), GluingError> {
        if !self.nodes.contains(&boundary.left) || !self.nodes.contains(&boundary.right) {
            return Err(GluingError::UnknownNode);
        }
        self.boundaries.push(boundary);
        Ok(())
    }

    pub fn set(&mut self, node: u64, cell: &str, value: f64) -> Result<(), GluingError> {
        if !self.nodes.contains(&node) {
            return Err(GluingError::UnknownNode);
        }
        self.sections.insert((node, cell.to_owned()), value);
        Ok(())
    }

    /// Relax boundary values until every edge has zero coboundary.
    pub fn agree(&mut self, tolerance: f64, max_iterations: usize) -> Result<usize, GluingError> {
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(GluingError::BadTolerance);
        }
        if max_iterations == 0 {
            return Err(GluingError::BadIterationLimit);
        }
        for iteration in 0..max_iterations {
            if self.coboundary_norm() < tolerance {
                return Ok(iteration);
            }
            let mut updates: BTreeMap<(u64, String), f64> = BTreeMap::new();
            for boundary in &self.boundaries {
                let left = self.value(boundary.left, &boundary.cell);
                let right = self.value(boundary.right, &boundary.cell);
                let mean = (left + right) * 0.5;
                updates.insert((boundary.left, boundary.cell.clone()), mean);
                updates.insert((boundary.right, boundary.cell.clone()), mean);
            }
            for (key, value) in updates {
                self.sections.insert(key, value);
            }
        }
        Ok(max_iterations)
    }

    fn value(&self, node: u64, cell: &str) -> f64 {
        self.sections.get(&(node, cell.to_owned())).copied().unwrap_or(0.0)
    }

    /// The Euclidean norm of the boundary coboundary.
    pub fn coboundary_norm(&self) -> f64 {
        let sum: f64 = self
            .boundaries
            .iter()
            .map(|boundary| {
                let difference = self.value(boundary.left, &boundary.cell)
                    - self.value(boundary.right, &boundary.cell);
                difference * difference
            })
            .sum();
        sum.sqrt()
    }

    /// True when both nodes hold the same value at every boundary cell.
    pub fn agrees(&self) -> bool {
        self.boundaries.iter().all(|boundary| {
            (self.value(boundary.left, &boundary.cell)
                - self.value(boundary.right, &boundary.cell))
                .abs()
                < 1e-9
        })
    }
}
