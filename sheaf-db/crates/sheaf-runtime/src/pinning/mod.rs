// pinning/mod.rs - OS thread affinity port and adapter (spec Pillar 4).
//
// The topology model in affinity.rs decides which core owns a partition. This
// module makes the decision physical: it pins the calling thread to that core
// with the operating system, and it reads the mask back to prove the change.
//
// The port keeps callers free of platform code. Three adapters implement it:
//   windows.rs      SetThreadAffinityMask and GetThreadGroupAffinity
//   linux.rs        sched_setaffinity and sched_getaffinity
//   unsupported.rs  an explicit Unsupported error, never a fake success

use std::fmt;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(windows, target_os = "linux")))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
use linux as platform;
#[cfg(not(any(windows, target_os = "linux")))]
use unsupported as platform;
#[cfg(windows)]
use windows as platform;

/// Why a pin operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PinErrorKind {
    /// The platform has no thread affinity API.
    Unsupported,
    /// The requested core is outside the allowed set.
    CoreUnavailable,
    /// The operating system call failed.
    OsError,
    /// The mask had no cores.
    EmptyMask,
}

/// A pin failure with a human-readable detail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinError {
    pub kind: PinErrorKind,
    pub detail: String,
}

impl PinError {
    pub fn new(kind: PinErrorKind, detail: impl Into<String>) -> Self {
        Self { kind, detail: detail.into() }
    }

    /// The platform cannot pin threads.
    pub fn unsupported(target: &str) -> Self {
        Self::new(PinErrorKind::Unsupported, format!("thread pinning is not supported on {target}"))
    }

    /// The requested core is outside the allowed set.
    pub fn core_unavailable(core: usize) -> Self {
        Self::new(PinErrorKind::CoreUnavailable, format!("core {core} is not allowed"))
    }

    /// An OS call failed with the given error code.
    pub fn os(code: i32, what: &str) -> Self {
        Self::new(PinErrorKind::OsError, format!("{what} failed with error {code}"))
    }
}

impl fmt::Display for PinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.detail)
    }
}

impl std::error::Error for PinError {}

/// The result of a successful pin. `previous` is the mask before the call, so
/// the caller can restore it exactly.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinToken {
    pub pinned: usize,
    pub previous: Vec<usize>,
}

/// The affinity port. Callers depend on this trait, not on a platform API.
pub trait ThreadPinner {
    /// The cores that the process may use, in ascending order.
    fn allowed_cores(&self) -> Result<Vec<usize>, PinError>;
    /// The cores that the calling thread may use now.
    fn current_cores(&self) -> Result<Vec<usize>, PinError>;
    /// Pin the calling thread to one allowed core.
    fn pin_current(&self, core: usize) -> Result<PinToken, PinError>;
    /// Set the calling thread mask to exactly `cores`.
    fn restore(&self, cores: &[usize]) -> Result<(), PinError>;
}

/// The operating-system adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct OsThreadPinner;

impl OsThreadPinner {
    pub fn new() -> Self {
        Self
    }
}

impl ThreadPinner for OsThreadPinner {
    fn allowed_cores(&self) -> Result<Vec<usize>, PinError> {
        platform::allowed_cores()
    }

    fn current_cores(&self) -> Result<Vec<usize>, PinError> {
        platform::current_cores()
    }

    fn pin_current(&self, core: usize) -> Result<PinToken, PinError> {
        if !self.allowed_cores()?.contains(&core) {
            return Err(PinError::core_unavailable(core));
        }
        let previous = self.current_cores()?;
        platform::set_cores(&[core])?;
        // Read the mask back. A silent failure here would break the caller's
        // assumption that the worker runs on the chosen core.
        let now = self.current_cores()?;
        if now != [core] {
            return Err(PinError::new(
                PinErrorKind::OsError,
                format!("mask readback mismatch: wanted [{core}], got {now:?}"),
            ));
        }
        Ok(PinToken { pinned: core, previous })
    }

    fn restore(&self, cores: &[usize]) -> Result<(), PinError> {
        if cores.is_empty() {
            return Err(PinError::new(PinErrorKind::EmptyMask, "cannot restore an empty mask"));
        }
        platform::set_cores(cores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_set_is_not_empty() {
        let pinner = OsThreadPinner::new();
        let cores = pinner.allowed_cores().expect("allowed cores");
        assert!(!cores.is_empty());
        let mut sorted = cores.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, cores, "allowed cores must be ascending");
    }

    #[test]
    fn pin_readback_and_restore_are_exact() {
        let pinner = OsThreadPinner::new();
        let allowed = pinner.allowed_cores().expect("allowed cores");
        let before = pinner.current_cores().expect("current mask");
        // Four cores is enough to prove the mapping and keeps the test short.
        for &core in allowed.iter().take(4) {
            let token = pinner.pin_current(core).expect("pin core");
            assert_eq!(token.pinned, core);
            assert_eq!(pinner.current_cores().expect("readback"), vec![core]);
            pinner.restore(&token.previous).expect("restore");
            assert_eq!(pinner.current_cores().expect("restored mask"), before);
        }
    }

    #[test]
    fn unavailable_core_is_rejected() {
        let pinner = OsThreadPinner::new();
        let allowed = pinner.allowed_cores().expect("allowed cores");
        let outside = allowed.iter().copied().max().unwrap_or(0) + 1;
        let err = pinner.pin_current(outside).expect_err("must reject");
        assert_eq!(err.kind, PinErrorKind::CoreUnavailable);
    }

    #[test]
    fn empty_restore_is_rejected() {
        let pinner = OsThreadPinner::new();
        let err = pinner.restore(&[]).expect_err("must reject");
        assert_eq!(err.kind, PinErrorKind::EmptyMask);
    }
}
