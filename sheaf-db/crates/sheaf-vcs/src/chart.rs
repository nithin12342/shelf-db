// chart.rs - append-only chronological time charts (spec Pillar 10).
//
// When concurrent writes target the same cell or overlapping edges, they are
// never blocked by an exclusive mutex. Instead they append to an entity-local
// chronological time chart:
//     C(v) = [ delta_t0, delta_t1, delta_t2, ... ]
// Cells are never overwritten in place. A reader pulls a snapshot at logical
// time T by folding the delta chain up to T. Conflicting writes are ordered in
// time without aborting or rolling back, which removes write-amplification stalls.
//
// Deltas are additive f32 vectors. The fold is a sum, so it is deterministic
// and commutative for a fixed timestamp order.

use crate::complex::Complex;
use crate::section::Section;

/// An error produced by an invalid chart append.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChartError {
    /// The timestamp does not strictly exceed the last appended timestamp.
    NonMonotonicTimestamp { last: u64, got: u64 },
    /// The delta length does not match the cell stalk dimension.
    DimensionMismatch { expected: usize, got: usize },
}

/// One additive delta stamped with a logical timestamp.
#[derive(Clone, Debug, PartialEq)]
pub struct Delta {
    pub timestamp: u64,
    pub values: Vec<f32>,
}

/// The append-only delta chain for a single cell.
#[derive(Clone, Debug)]
pub struct TimeChart {
    base: Vec<f32>,
    chain: Vec<Delta>,
}

impl TimeChart {
    /// Start a chart with a base stalk value and an empty chain.
    pub fn new(base: Vec<f32>) -> Self {
        Self { base, chain: Vec::new() }
    }

    /// The stalk dimension of this cell.
    pub fn dim(&self) -> usize {
        self.base.len()
    }

    /// Number of deltas appended so far.
    pub fn len(&self) -> usize {
        self.chain.len()
    }

    /// True when no delta has been appended.
    pub fn is_empty(&self) -> bool {
        self.chain.is_empty()
    }

    /// Append a delta. The timestamp must strictly increase, and the delta
    /// length must match the stalk dimension. The base is never modified.
    pub fn append(&mut self, timestamp: u64, values: Vec<f32>) -> Result<(), ChartError> {
        if let Some(last) = self.chain.last() {
            if timestamp <= last.timestamp {
                return Err(ChartError::NonMonotonicTimestamp { last: last.timestamp, got: timestamp });
            }
        }
        if values.len() != self.base.len() {
            return Err(ChartError::DimensionMismatch { expected: self.base.len(), got: values.len() });
        }
        self.chain.push(Delta { timestamp, values });
        Ok(())
    }

    /// Fold the chain up to logical time `at`. Returns base plus every delta
    /// whose timestamp is <= at. This is the non-destructive snapshot read.
    pub fn snapshot(&self, at: u64) -> Vec<f32> {
        let mut acc = self.base.clone();
        for delta in &self.chain {
            if delta.timestamp > at {
                break;
            }
            for (a, d) in acc.iter_mut().zip(&delta.values) {
                *a += d;
            }
        }
        acc
    }

    /// The latest state, folding the entire chain.
    pub fn latest(&self) -> Vec<f32> {
        self.snapshot(u64::MAX)
    }
}

/// A chart per vertex of a complex. It folds every cell into a full section.
pub struct ChartRegistry {
    charts: Vec<TimeChart>,
}

impl ChartRegistry {
    /// Initialize a zero-base chart for every vertex, sized to its stalk.
    pub fn new(c: &Complex) -> Self {
        let charts = (0..c.vertex_count()).map(|i| TimeChart::new(vec![0.0; c.vertex_dim(i)])).collect();
        Self { charts }
    }

    /// Append a delta to the chart of vertex `cell`.
    pub fn append(&mut self, cell: usize, timestamp: u64, values: Vec<f32>) -> Result<(), ChartError> {
        self.charts[cell].append(timestamp, values)
    }

    /// Fold every vertex chart at time `at` into a 0-cochain section. The
    /// concatenation order matches the complex's 0-cochain layout.
    pub fn snapshot_section(&self, c: &Complex, at: u64) -> Section {
        let mut values = Vec::with_capacity(c.c0_len());
        for chart in &self.charts {
            values.extend_from_slice(&chart.snapshot(at));
        }
        Section { values }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::Restriction;

    #[test]
    fn snapshot_folds_only_earlier_deltas() {
        let mut chart = TimeChart::new(vec![0.0, 0.0]);
        chart.append(1, vec![1.0, 2.0]).unwrap();
        chart.append(3, vec![10.0, 20.0]).unwrap();
        // At time 2 only the first delta has landed.
        assert_eq!(chart.snapshot(2), vec![1.0, 2.0]);
        // At time 3 both deltas fold.
        assert_eq!(chart.snapshot(3), vec![11.0, 22.0]);
        assert_eq!(chart.latest(), vec![11.0, 22.0]);
    }

    #[test]
    fn base_is_preserved() {
        let mut chart = TimeChart::new(vec![5.0]);
        chart.append(1, vec![1.0]).unwrap();
        assert_eq!(chart.snapshot(0), vec![5.0]);
        assert_eq!(chart.latest(), vec![6.0]);
    }

    #[test]
    fn non_monotonic_timestamp_is_rejected() {
        let mut chart = TimeChart::new(vec![0.0]);
        chart.append(5, vec![1.0]).unwrap();
        let err = chart.append(5, vec![2.0]).unwrap_err();
        assert_eq!(err, ChartError::NonMonotonicTimestamp { last: 5, got: 5 });
        assert!(chart.append(4, vec![2.0]).is_err());
        assert_eq!(chart.len(), 1);
    }

    #[test]
    fn dimension_mismatch_is_rejected() {
        let mut chart = TimeChart::new(vec![0.0, 0.0]);
        let err = chart.append(1, vec![1.0]).unwrap_err();
        assert_eq!(err, ChartError::DimensionMismatch { expected: 2, got: 1 });
    }

    #[test]
    fn registry_folds_a_section() {
        let mut c = Complex::new(vec![1, 1]);
        c.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        let mut reg = ChartRegistry::new(&c);
        reg.append(0, 1, vec![3.0]).unwrap();
        reg.append(1, 1, vec![3.0]).unwrap();
        let section = reg.snapshot_section(&c, 1);
        assert_eq!(section.values, vec![3.0, 3.0]);
        // Equal values on an identity edge form a global section.
        assert!(section.is_global(&c, 1e-6));
    }
}