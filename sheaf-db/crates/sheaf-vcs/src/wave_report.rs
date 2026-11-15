// wave_report.rs - the recorded outcome of a parallel wave run (MF-03).
//
// A report answers three questions about a run. Did every cell run? How many
// worker threads did the work? Did the workers reach the pinned cores?
//
// A pinning failure is recorded, not hidden. The report holds the reason, so a
// reader never mistakes an unpinned run for a pinned one.

/// The outcome of one wave.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaveStats {
    /// The wave number in the input order, starting at zero.
    pub wave: usize,
    /// The number of cells in the wave.
    pub cells: usize,
    /// The number of worker slices that ran.
    pub workers: usize,
    /// The thread identifiers of the workers, in slice order.
    pub threads: Vec<String>,
    /// The number of workers that pinned themselves to a core.
    pub pinned_workers: usize,
    /// The elapsed time in nanoseconds.
    pub elapsed_nanos: u64,
}

impl WaveStats {
    /// The number of distinct worker threads.
    pub fn distinct_threads(&self) -> usize {
        let mut seen: Vec<&str> = self.threads.iter().map(String::as_str).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    /// True when every worker pinned itself.
    pub fn fully_pinned(&self) -> bool {
        self.workers > 0 && self.pinned_workers == self.workers
    }
}

/// The outcome of a whole wave run.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaveReport {
    /// The waves in execution order.
    pub waves: Vec<WaveStats>,
    /// The number of cells that ran.
    pub executed_cells: usize,
    /// Why pinning was not available, or None when every worker pinned.
    pub pin_note: Option<String>,
}

impl WaveReport {
    /// An empty report.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append the statistics of one wave.
    pub fn push_wave(&mut self, stats: WaveStats) {
        self.executed_cells += stats.cells;
        self.waves.push(stats);
    }

    /// The number of waves.
    pub fn wave_count(&self) -> usize {
        self.waves.len()
    }

    /// The largest number of distinct threads in any wave.
    pub fn max_distinct_threads(&self) -> usize {
        self.waves.iter().map(WaveStats::distinct_threads).max().unwrap_or(0)
    }

    /// True when pinning succeeded for every worker of every wave.
    pub fn fully_pinned(&self) -> bool {
        !self.waves.is_empty() && self.pin_note.is_none() && self.waves.iter().all(WaveStats::fully_pinned)
    }

    /// The total elapsed time in nanoseconds.
    pub fn total_elapsed_nanos(&self) -> u64 {
        self.waves.iter().map(|w| w.elapsed_nanos).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_accumulates_waves_and_cells() {
        let mut report = WaveReport::new();
        report.push_wave(WaveStats { wave: 0, cells: 4, workers: 2, threads: vec!["a".into(), "b".into()], pinned_workers: 2, elapsed_nanos: 10 });
        report.push_wave(WaveStats { wave: 1, cells: 1, workers: 1, threads: vec!["c".into()], pinned_workers: 1, elapsed_nanos: 5 });
        assert_eq!(report.wave_count(), 2);
        assert_eq!(report.executed_cells, 5);
        assert_eq!(report.max_distinct_threads(), 2);
        assert_eq!(report.total_elapsed_nanos(), 15);
        assert!(report.fully_pinned());
    }

    #[test]
    fn a_pin_note_marks_the_run_unpinned() {
        let mut report = WaveReport::new();
        report.push_wave(WaveStats { wave: 0, cells: 1, workers: 1, threads: vec!["a".into()], pinned_workers: 0, elapsed_nanos: 1 });
        report.pin_note = Some("no thread affinity on this target".into());
        assert!(!report.fully_pinned());
    }
}
