//! ffi - Rust mirror of native/include/sheaf_wal.h.
//!
//! Compiles only when build.rs linked the C WAL (cfg sheaf_native_wal).
//! SheafWal is opaque; the C tier owns its layout. SheafWalStats is a
//! plain field-for-field mirror of the recovery report struct.

use std::os::raw::c_char;

/// Opaque WAL handle owned by the C tier.
#[repr(C)]
pub struct SheafWal {
    _private: [u8; 0],
}

/// Recovery report, field order identical to SheafWalStats.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SheafWalStats {
    pub epoch: u64,
    pub last_seq: u64,
    pub valid_end: u64,
    pub records_recovered: u64,
    pub truncated_bytes: u64,
}

extern "C" {
    pub fn sheaf_wal_open(log_path: *const c_char, header_path: *const c_char) -> *mut SheafWal;
    pub fn sheaf_wal_append(
        wal: *mut SheafWal,
        epoch: u64,
        payload: *const u8,
        len: u32,
    ) -> i32;
    pub fn sheaf_wal_commit_epoch(wal: *mut SheafWal, new_epoch: u64) -> i32;
    pub fn sheaf_wal_flush(wal: *mut SheafWal) -> i32;
    pub fn sheaf_wal_recover(wal: *mut SheafWal, out: *mut SheafWalStats) -> i32;
    pub fn sheaf_wal_epoch(wal: *const SheafWal) -> u64;
    pub fn sheaf_wal_close(wal: *mut SheafWal);
}