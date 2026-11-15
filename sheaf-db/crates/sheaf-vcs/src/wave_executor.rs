// wave_executor.rs - runs parallel waves of cell writes (MF-03, Pillar 10).
//
// The collision inspector splits a batch into waves. Every cell in a wave has a
// star neighborhood that does not overlap its neighbours, so the whole wave can
// run at once. This executor is the other half of that decision: it spreads one
// wave over worker threads, pins each worker to a core, and joins every worker
// before the next wave starts.
//
// Three rules hold at every step:
//   1. Waves run in order. A cell in wave n+1 never overlaps a cell in wave n.
//   2. A cell runs once. The input is checked for a repeat before any write.
//   3. A write error stops the wave, joins every worker, and returns that error.
//
// Pinning is a reported capability, not an assumption. When the platform has no
// thread affinity, the run continues unpinned and the report names the reason.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Instant;

use sheaf_runtime::pinning::{OsThreadPinner, ThreadPinner};

use crate::wave_report::{WaveReport, WaveStats};
use crate::write_target::{CellWriteTarget, WriteError};

/// Why a wave run stopped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WaveError {
    /// The input names the same cell twice.
    DuplicateCell(u64),
    /// A target rejected a cell.
    Write(WriteError),
}

impl fmt::Display for WaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WaveError::DuplicateCell(cell) => write!(f, "cell {cell} appears more than once"),
            WaveError::Write(err) => write!(f, "wave write failed: {err}"),
        }
    }
}

impl std::error::Error for WaveError {}

impl From<WriteError> for WaveError {
    fn from(err: WriteError) -> Self {
        WaveError::Write(err)
    }
}

/// Runs waves over a fixed number of worker threads.
#[derive(Clone, Copy, Debug)]
pub struct WaveExecutor {
    workers: usize,
}

impl WaveExecutor {
    /// Build an executor with `workers` threads. The count clamps to at least
    /// one, because a wave must run somewhere.
    pub fn new(workers: usize) -> Self {
        Self { workers: workers.max(1) }
    }

    /// The worker count.
    pub fn workers(&self) -> usize {
        self.workers
    }

    /// The cells of a wave split into at most `workers` contiguous slices.
    fn slice_size(&self, cells: usize) -> usize {
        if cells == 0 {
            1
        } else {
            cells.div_ceil(self.workers.min(cells))
        }
    }

    /// Reject a repeated cell before any write happens.
    fn check_unique(waves: &[Vec<u64>]) -> Result<usize, WaveError> {
        let mut seen: BTreeSet<u64> = BTreeSet::new();
        for cell in waves.iter().flatten() {
            if !seen.insert(*cell) {
                return Err(WaveError::DuplicateCell(*cell));
            }
        }
        Ok(seen.len())
    }

    /// Run every wave in order over `target`. Each wave is spread across the
    /// workers and joined before the next wave starts.
    pub fn execute<T>(&self, waves: &[Vec<u64>], target: &T) -> Result<WaveReport, WaveError>
    where
        T: CellWriteTarget + Sync,
    {
        let unique = Self::check_unique(waves)?;
        let pinner = OsThreadPinner::new();

        // Probe the affinity capability once. A failure is reported, never
        // hidden, so the run is never mistaken for a pinned run.
        let mut report = WaveReport::new();
        let allowed = match pinner.allowed_cores() {
            Ok(cores) => Some(cores),
            Err(err) => {
                report.pin_note = Some(err.to_string());
                None
            }
        };

        let executed = AtomicUsize::new(0);
        let failed = AtomicBool::new(false);
        let first_error: Mutex<Option<WriteError>> = Mutex::new(None);
        let pin_error: Mutex<Option<String>> = Mutex::new(None);

        for (index, wave) in waves.iter().enumerate() {
            let started = Instant::now();
            let slice = self.slice_size(wave.len());
            let outcomes: Mutex<Vec<(String, bool)>> = Mutex::new(Vec::new());

            thread::scope(|scope| {
                let failed = &failed;
                let executed = &executed;
                let first_error = &first_error;
                let pin_error = &pin_error;
                let outcomes = &outcomes;
                let allowed = allowed.as_deref();

                for (worker, chunk) in wave.chunks(slice).enumerate() {
                    scope.spawn(move || {
                        let pinner = OsThreadPinner::new();
                        let mut token = None;
                        if let Some(cores) = allowed {
                            let core = cores[worker % cores.len()];
                            match pinner.pin_current(core) {
                                Ok(held) => token = Some(held),
                                Err(err) => {
                                    let mut slot = pin_error.lock().expect("pin error lock");
                                    if slot.is_none() {
                                        *slot = Some(err.to_string());
                                    }
                                }
                            }
                        }

                        for &cell in chunk {
                            if failed.load(Ordering::Acquire) {
                                break;
                            }
                            if let Err(err) = target.write_cell(cell) {
                                failed.store(true, Ordering::Release);
                                let mut slot = first_error.lock().expect("first error lock");
                                if slot.is_none() {
                                    *slot = Some(err);
                                }
                                break;
                            }
                            executed.fetch_add(1, Ordering::Relaxed);
                        }

                        // Restore the mask that the thread had before the pin.
                        let pinned = token.is_some();
                        if let Some(held) = token {
                            let _ = pinner.restore(&held.previous);
                        }
                        let id = format!("{:?}", thread::current().id());
                        outcomes.lock().expect("outcome lock").push((id, pinned));
                    });
                }
            });

            let outcomes = outcomes.into_inner().expect("outcome lock");
            let stats = WaveStats {
                wave: index,
                cells: wave.len(),
                workers: outcomes.len(),
                threads: outcomes.iter().map(|(id, _)| id.clone()).collect(),
                pinned_workers: outcomes.iter().filter(|(_, pinned)| *pinned).count(),
                elapsed_nanos: started.elapsed().as_nanos() as u64,
            };
            report.push_wave(stats);

            let error = first_error.lock().expect("first error lock").take();
            if let Some(err) = error {
                return Err(WaveError::Write(err));
            }
        }

        if let Some(note) = pin_error.into_inner().expect("pin error lock") {
            if report.pin_note.is_none() {
                report.pin_note = Some(note);
            }
        }

        // The uniqueness gate makes this check unreachable. Assert it anyway,
        // because a repeated write would corrupt a versioned store.
        assert_eq!(
            executed.load(Ordering::Acquire),
            unique,
            "a cell must run exactly once"
        );
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::complex::{Complex, Restriction};
    use crate::lca::CollisionInspector;
    use crate::write_target::RecordingTarget;

    fn disjoint_cells(count: u64) -> Vec<Vec<u64>> {
        // One wave of independent cells. The complex is not needed here,
        // because the caller already decided that the cells are disjoint.
        vec![(0..count).collect()]
    }

    #[test]
    fn sixty_four_disjoint_cells_run_on_eight_workers() {
        let executor = WaveExecutor::new(8);
        let target = RecordingTarget::new();
        let waves = disjoint_cells(64);
        let report = executor.execute(&waves, &target).expect("run waves");

        assert_eq!(report.executed_cells, 64);
        assert_eq!(target.count(), 64);
        assert_eq!(target.accepted_sorted(), (0..64u64).collect::<Vec<_>>());
        assert_eq!(report.wave_count(), 1);
        assert_eq!(report.waves[0].workers, 8);
        assert!(
            report.max_distinct_threads() >= 2,
            "the wave must use more than one thread"
        );
    }

    #[test]
    fn triangle_cells_run_in_three_ordered_waves() {
        let mut complex = Complex::new(vec![1, 1, 1]);
        complex.add_edge(0, 1, Restriction::identity(1), Restriction::identity(1));
        complex.add_edge(1, 2, Restriction::identity(1), Restriction::identity(1));
        complex.add_edge(2, 0, Restriction::identity(1), Restriction::identity(1));
        let inspector = CollisionInspector::new(&complex);
        let waves = inspector.parallel_waves(&[0, 1, 2]);
        assert_eq!(waves.len(), 3);

        let executor = WaveExecutor::new(4);
        let target = RecordingTarget::new();
        let report = executor.execute(&waves, &target).expect("run waves");

        assert_eq!(report.wave_count(), 3);
        assert_eq!(report.executed_cells, 3);
        assert_eq!(target.accepted(), vec![0, 1, 2], "waves must run in order");
        assert!(report.waves.iter().all(|w| w.workers == 1));
    }

    #[test]
    fn a_write_error_stops_the_run_before_the_next_wave() {
        let executor = WaveExecutor::new(2);
        let target = RecordingTarget::failing_on(1);
        let waves = vec![vec![0u64], vec![1u64], vec![2u64]];
        let err = executor.execute(&waves, &target).expect_err("must fail");

        assert_eq!(err, WaveError::Write(WriteError::new(1, "injected write failure")));
        assert_eq!(target.accepted(), vec![0], "wave three must not run");
    }

    #[test]
    fn a_repeated_cell_is_rejected_before_any_write() {
        let executor = WaveExecutor::new(2);
        let target = RecordingTarget::new();
        let waves = vec![vec![3u64, 4], vec![4u64]];
        let err = executor.execute(&waves, &target).expect_err("must reject");
        assert_eq!(err, WaveError::DuplicateCell(4));
        assert_eq!(target.count(), 0);
    }

    #[test]
    fn empty_input_returns_an_empty_report() {
        let executor = WaveExecutor::new(4);
        let target = RecordingTarget::new();
        let report = executor.execute(&[], &target).expect("run");
        assert_eq!(report.wave_count(), 0);
        assert_eq!(report.executed_cells, 0);
        assert_eq!(target.count(), 0);
    }

    #[test]
    fn workers_pin_when_the_platform_supports_affinity() {
        let executor = WaveExecutor::new(4);
        let target = RecordingTarget::new();
        let report = executor.execute(&disjoint_cells(32), &target).expect("run waves");
        if cfg!(any(windows, target_os = "linux")) {
            assert!(report.fully_pinned(), "pin note: {:?}", report.pin_note);
            assert_eq!(report.waves[0].pinned_workers, 4);
        } else {
            assert!(report.pin_note.is_some(), "an unpinned run must report why");
        }
    }
}
