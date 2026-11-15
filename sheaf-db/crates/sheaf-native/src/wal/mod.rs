//! wal - the port between Rust orchestration and the C11 durable log.
//!
//! Dependency inversion mirrors the kernel and storage tiers:
//!
//! ```text
//! engine / commit code
//!        |  depends on the trait only
//!        v
//! wal::WalBackend  (port)
//!        |---------------------------|
//!        v                           v
//! wal::native::NativeWal         wal::fallback::FallbackWal
//! (C11 library via C ABI)        (pure Rust, always available)
//! ```
//!
//! Both adapters write the identical record framing and A/B epoch slots,
//! so a log written by one recovers through the other. The fallback is a
//! full implementation of the format, not a stub, which lets the crash
//! recovery tests run on any machine.

pub mod crc32c;
pub mod fallback;
#[cfg(sheaf_native_wal)]
pub mod ffi;
#[cfg(sheaf_native_wal)]
pub mod native;

pub use fallback::FallbackWal;
#[cfg(sheaf_native_wal)]
pub use native::NativeWal;

use std::path::Path;

/// Sector size of one epoch header slot. Two slots occupy one 8 KB file.
pub const SLOT_BYTES: usize = 4096;
/// Maximum payload bytes per record.
pub const MAX_PAYLOAD: usize = 65536;

/// Recovery report, mirroring SheafWalStats in sheaf_wal.h.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WalStats {
    pub epoch: u64,
    pub last_seq: u64,
    pub valid_end: u64,
    pub records_recovered: u64,
    pub truncated_bytes: u64,
}

/// Errors the WAL port can return.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WalError {
    /// A bad argument or a non-monotonic epoch.
    InvalidArgument,
    /// A platform read, write, or flush failure.
    Io,
    /// Both header slots are invalid on a nonempty log, or committed
    /// bytes failed their integrity checks.
    Corrupt,
}

impl From<std::io::Error> for WalError {
    fn from(_: std::io::Error) -> Self {
        WalError::Io
    }
}

/// The durable write-ahead log port. A handle owns one log + header pair.
pub trait WalBackend {
    fn name(&self) -> &'static str;

    /// Append one framed record. Durable only after the next commit.
    fn append(&mut self, epoch: u64, payload: &[u8]) -> Result<(), WalError>;

    /// Durability boundary: flush the log, then flip to the inactive slot.
    /// `new_epoch` must exceed the current epoch.
    fn commit_epoch(&mut self, new_epoch: u64) -> Result<(), WalError>;

    /// Force the platform barrier without flipping the epoch.
    fn flush(&mut self) -> Result<(), WalError>;

    /// Re-scan and repair. Safe to call repeatedly.
    fn recover(&mut self) -> Result<WalStats, WalError>;

    /// Current committed epoch.
    fn epoch(&self) -> u64;
}

/// Open a log through the pure-Rust fallback.
pub fn open_fallback(log: &Path, header: &Path) -> Result<FallbackWal, WalError> {
    FallbackWal::open(log, header)
}

/// Open a log through the C11 native tier.
#[cfg(sheaf_native_wal)]
pub fn open_native(log: &Path, header: &Path) -> Result<NativeWal, WalError> {
    NativeWal::open(log, header)
}

/// Open a log through the best available backend.
pub fn open(log: &Path, header: &Path) -> Result<Box<dyn WalBackend>, WalError> {
    #[cfg(sheaf_native_wal)]
    {
        Ok(Box::new(NativeWal::open(log, header)?))
    }
    #[cfg(not(sheaf_native_wal))]
    {
        Ok(Box::new(FallbackWal::open(log, header)?))
    }
}

/// True when this build links the compiled C11 WAL library.
pub const fn native_wal_linked() -> bool {
    cfg!(sheaf_native_wal)
}