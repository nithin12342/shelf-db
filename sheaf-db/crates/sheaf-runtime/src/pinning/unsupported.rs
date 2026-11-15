// pinning/unsupported.rs - the explicit fallback for other targets.
//
// The engine must not pretend that a thread is pinned. Every call returns an
// Unsupported error with the target name, so the caller can report the
// capability instead of relying on a silent no-op.

use super::PinError;

pub(super) fn allowed_cores() -> Result<Vec<usize>, PinError> {
    Err(PinError::unsupported(std::env::consts::OS))
}

pub(super) fn current_cores() -> Result<Vec<usize>, PinError> {
    Err(PinError::unsupported(std::env::consts::OS))
}

pub(super) fn set_cores(_cores: &[usize]) -> Result<(), PinError> {
    Err(PinError::unsupported(std::env::consts::OS))
}
