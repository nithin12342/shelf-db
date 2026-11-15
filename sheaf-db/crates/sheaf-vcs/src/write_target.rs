// write_target.rs - the cell write port for the parallel wave executor (MF-03).
//
// The wave executor decides when a cell may run. This port decides how a cell
// is stored. Callers depend on the trait, so a durable store, an overlay, and
// the recording target below are interchangeable adapters.
//
// A write failure carries the cell number, so the executor can report the
// exact cell that stopped the wave.

use std::fmt;
use std::sync::Mutex;

/// One cell write that failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteError {
    /// The cell that the target rejected.
    pub cell: u64,
    /// Why the target rejected the cell.
    pub detail: String,
}

impl WriteError {
    pub fn new(cell: u64, detail: impl Into<String>) -> Self {
        Self { cell, detail: detail.into() }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cell {}: {}", self.cell, self.detail)
    }
}

impl std::error::Error for WriteError {}

/// The write port. An implementation must accept a cell write from any worker
/// thread, so the trait requires `Sync`.
pub trait CellWriteTarget {
    /// Store one cell. The call returns Err when the cell is rejected.
    fn write_cell(&self, cell: u64) -> Result<(), WriteError>;
}

/// A target that records every accepted cell in memory.
///
/// The recorder is a real adapter, not a test stub: it implements the port and
/// it reports what it accepted. A dry run and a verification pass both use it.
#[derive(Debug, Default)]
pub struct RecordingTarget {
    accepted: Mutex<Vec<u64>>,
    fail_on: Option<u64>,
}

impl RecordingTarget {
    /// A target that accepts every cell.
    pub fn new() -> Self {
        Self { accepted: Mutex::new(Vec::new()), fail_on: None }
    }

    /// A target that rejects one cell and accepts the others.
    pub fn failing_on(cell: u64) -> Self {
        Self { accepted: Mutex::new(Vec::new()), fail_on: Some(cell) }
    }

    /// The accepted cells in acceptance order.
    pub fn accepted(&self) -> Vec<u64> {
        self.accepted.lock().expect("recording target poisoned").clone()
    }

    /// The accepted cells in ascending order.
    pub fn accepted_sorted(&self) -> Vec<u64> {
        let mut out = self.accepted();
        out.sort_unstable();
        out
    }

    /// The number of accepted cells.
    pub fn count(&self) -> usize {
        self.accepted.lock().expect("recording target poisoned").len()
    }
}

impl CellWriteTarget for RecordingTarget {
    fn write_cell(&self, cell: u64) -> Result<(), WriteError> {
        if self.fail_on == Some(cell) {
            return Err(WriteError::new(cell, "injected write failure"));
        }
        self.accepted.lock().expect("recording target poisoned").push(cell);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recorder_accepts_and_reports_every_cell() {
        let target = RecordingTarget::new();
        for cell in [7u64, 2, 9] {
            target.write_cell(cell).expect("accept");
        }
        assert_eq!(target.count(), 3);
        assert_eq!(target.accepted(), vec![7, 2, 9]);
        assert_eq!(target.accepted_sorted(), vec![2, 7, 9]);
    }

    #[test]
    fn recorder_rejects_the_named_cell_only() {
        let target = RecordingTarget::failing_on(5);
        let err = target.write_cell(5).expect_err("must reject");
        assert_eq!(err.cell, 5);
        target.write_cell(6).expect("accept the other cell");
        assert_eq!(target.accepted(), vec![6]);
    }

    #[test]
    fn write_error_names_the_cell() {
        let err = WriteError::new(41, "disk full");
        assert_eq!(err.to_string(), "cell 41: disk full");
    }
}
