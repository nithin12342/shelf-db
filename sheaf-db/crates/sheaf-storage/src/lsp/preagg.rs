// crates/sheaf-storage/src/lsp/preagg.rs — incremental pre-aggregation
// MAX-PERF P3 (METHOD-025, FILE-061). Depends on nothing (pure data).
// Maintains per-(group, measure) accumulators updated O(1) at ingest, so a
// GROUP BY query reads O(#groups) rows instead of scanning. Groups are u32
// ids (segments, customers, any discrete key); measures are f64 sums
// (+ optional counts for AVG). Updates are plain adds — callers serialize
// via the graded log order (same thread that seals also pre-aggregates, so
// no extra locking; concurrent readers see atomically-swapped snapshots).
use std::collections::HashMap;

/// O(1)-update group accumulator store: (group -> (sum, count)).
pub struct PreAgg {
    sums: HashMap<u32, (f64, u64)>,
}

impl PreAgg {
    pub fn new() -> Self {
        Self { sums: HashMap::new() }
    }

    /// Ingest one measured event into its group. O(1) amortized.
    pub fn ingest(&mut self, group: u32, value: f64) {
        let e = self.sums.entry(group).or_insert((0.0, 0));
        e.0 += value;
        e.1 += 1;
    }

    /// Whole-store GROUP BY readout: sorted (group, sum, count) vec.
    /// Cost O(#groups) — independent of ingested row count.
    pub fn readout(&self) -> Vec<(u32, f64, u64)> {
        let mut out: Vec<(u32, f64, u64)> =
            self.sums.iter().map(|(&g, &(s, c))| (g, s, c)).collect();
        out.sort_unstable_by_key(|&(g, _, _)| g);
        out
    }

    pub fn groups(&self) -> usize {
        self.sums.len()
    }
}

impl Default for PreAgg {
    fn default() -> Self {
        Self::new()
    }
}
