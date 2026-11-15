//! native - FFI adapter that forwards the WAL port to the C11 library.
//!
//! Compiles only when build.rs linked libsheaf_wal (cfg sheaf_native_wal).
//! The handle is a raw pointer owned by the C tier; Drop closes it. Every
//! call passes live buffers, and the C tier validates lengths itself.

use super::{WalBackend, WalError, WalStats, MAX_PAYLOAD};
use crate::wal::ffi;
use std::ffi::CString;
use std::path::Path;

/// FFI WAL handle. The C tier enforces the single-writer contract.
pub struct NativeWal {
    handle: *mut ffi::SheafWal,
}

fn map_err(code: i32) -> WalError {
    match code {
        -1 => WalError::InvalidArgument,
        -3 => WalError::Corrupt,
        _ => WalError::Io,
    }
}

fn check(rc: i32) -> Result<(), WalError> {
    if rc == 0 { Ok(()) } else { Err(map_err(rc)) }
}

fn c_path(p: &Path) -> Result<CString, WalError> {
    let s = p.to_str().ok_or(WalError::InvalidArgument)?;
    CString::new(s).map_err(|_| WalError::InvalidArgument)
}

impl NativeWal {
    pub fn open(log: &Path, header: &Path) -> Result<Self, WalError> {
        let l = c_path(log)?;
        let h = c_path(header)?;
        // SAFETY: both CStrings live for this call; the C tier copies the
        // paths and returns an owned handle or null on failure.
        let handle = unsafe { ffi::sheaf_wal_open(l.as_ptr(), h.as_ptr()) };
        if handle.is_null() {
            return Err(WalError::Io);
        }
        Ok(Self { handle })
    }
}

impl WalBackend for NativeWal {
    fn name(&self) -> &'static str {
        "c11-native"
    }

    fn append(&mut self, epoch: u64, payload: &[u8]) -> Result<(), WalError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(WalError::InvalidArgument);
        }
        // SAFETY: handle is live; payload holds len bytes for this call.
        check(unsafe {
            ffi::sheaf_wal_append(self.handle, epoch, payload.as_ptr(), payload.len() as u32)
        })
    }

    fn commit_epoch(&mut self, new_epoch: u64) -> Result<(), WalError> {
        // SAFETY: handle is live; the C tier enforces epoch monotonicity.
        check(unsafe { ffi::sheaf_wal_commit_epoch(self.handle, new_epoch) })
    }

    fn flush(&mut self) -> Result<(), WalError> {
        // SAFETY: handle is live.
        check(unsafe { ffi::sheaf_wal_flush(self.handle) })
    }

    fn recover(&mut self) -> Result<WalStats, WalError> {
        // SAFETY: out is a live zeroed local; the C tier fills every field.
        let mut raw = ffi::SheafWalStats {
            epoch: 0,
            last_seq: 0,
            valid_end: 0,
            records_recovered: 0,
            truncated_bytes: 0,
        };
        check(unsafe { ffi::sheaf_wal_recover(self.handle, &mut raw) })?;
        Ok(WalStats {
            epoch: raw.epoch,
            last_seq: raw.last_seq,
            valid_end: raw.valid_end,
            records_recovered: raw.records_recovered,
            truncated_bytes: raw.truncated_bytes,
        })
    }

    fn epoch(&self) -> u64 {
        // SAFETY: handle is live; the C tier only reads the epoch field.
        unsafe { ffi::sheaf_wal_epoch(self.handle) }
    }
}

impl Drop for NativeWal {
    fn drop(&mut self) {
        // SAFETY: the handle came from sheaf_wal_open and is closed once.
        unsafe { ffi::sheaf_wal_close(self.handle) };
    }
}