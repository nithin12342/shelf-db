//! wal_recovery - durability and crash-recovery proofs for the WAL port.
//!
//! Every semantic test runs against each linked backend. Cross-backend
//! tests write with one adapter and recover with the other, proving the
//! record framing and A/B epoch slots are byte-identical.

use sheaf_native::wal::{self, WalBackend, WalError, MAX_PAYLOAD};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempWal {
    dir: PathBuf,
}

impl TempWal {
    fn new(tag: &str) -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("sheaf_wal_{}_{}_{}", tag, std::process::id(), n));
        fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }
    fn log(&self) -> PathBuf {
        self.dir.join("log.wal")
    }
    fn header(&self) -> PathBuf {
        self.dir.join("epoch.hdr")
    }
}

impl Drop for TempWal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

// Backend kinds: 0 = Rust fallback, 1 = C11 native (when linked).
fn kinds() -> Vec<usize> {
    let mut v = vec![0usize];
    #[cfg(sheaf_native_wal)]
    v.push(1);
    v
}

fn open_with(kind: usize, log: &Path, header: &Path) -> Result<Box<dyn WalBackend>, WalError> {
    match kind {
        0 => Ok(Box::new(wal::open_fallback(log, header)?)),
        #[cfg(sheaf_native_wal)]
        1 => Ok(Box::new(wal::open_native(log, header)?)),
        _ => unreachable!("unknown backend kind"),
    }
}

#[test]
fn append_commit_recover_roundtrip() {
    for kind in kinds() {
        let t = TempWal::new("roundtrip");
        {
            let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
            assert_eq!(w.epoch(), 0, "kind {kind} fresh epoch");
            w.append(1, b"alpha").unwrap();
            w.append(1, b"beta").unwrap();
            w.commit_epoch(1).unwrap();
            assert_eq!(w.epoch(), 1, "kind {kind} committed");
        }
        let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
        let s = w.recover().unwrap();
        assert_eq!(s.epoch, 1, "kind {kind} epoch");
        assert_eq!(s.records_recovered, 2, "kind {kind} records");
        assert_eq!(s.last_seq, 1, "kind {kind} last seq");
        assert_eq!(s.truncated_bytes, 0, "kind {kind} clean tail");
    }
}

#[test]
fn torn_uncommitted_tail_is_truncated() {
    for kind in kinds() {
        let t = TempWal::new("torn");
        {
            let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
            w.append(1, b"kept").unwrap();
            w.commit_epoch(1).unwrap();
            // This record never commits. Its 32-byte frame + 10-byte payload
            // form the torn tail that recovery must remove.
            w.append(2, b"0123456789").unwrap();
        }
        // open() runs recovery, which truncates the torn tail. The log file
        // must now hold exactly the committed prefix: 32-byte frame + "kept".
        let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
        let s = w.recover().unwrap();
        assert_eq!(s.epoch, 1, "kind {kind} epoch holds");
        assert_eq!(s.records_recovered, 1, "kind {kind} only committed");
        assert_eq!(fs::metadata(t.log()).unwrap().len(), 36, "kind {kind} tail removed");
        // A second recover is clean because the tail is already gone.
        let again = w.recover().unwrap();
        assert_eq!(again.truncated_bytes, 0, "kind {kind} idempotent");
    }
}

#[test]
fn torn_higher_slot_falls_back_to_previous_epoch() {
    for kind in kinds() {
        let t = TempWal::new("slotfall");
        {
            let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
            w.append(1, b"one").unwrap();
            w.commit_epoch(1).unwrap(); // writes slot A
            w.append(2, b"two").unwrap();
            w.commit_epoch(2).unwrap(); // writes slot B
        }
        // Corrupt slot B (the higher epoch) inside its CRC-covered region.
        let hp = t.header();
        let mut bytes = fs::read(&hp).unwrap();
        bytes[4096 + 8] ^= 0xFF;
        fs::write(&hp, &bytes).unwrap();

        // open() recovers to slot A and truncates the epoch-2 record. The
        // log must hold only the epoch-1 prefix: 32-byte frame + "one".
        let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
        let s = w.recover().unwrap();
        assert_eq!(s.epoch, 1, "kind {kind} fell back to A");
        assert_eq!(s.records_recovered, 1, "kind {kind} only epoch-1 record");
        assert_eq!(fs::metadata(t.log()).unwrap().len(), 35, "kind {kind} epoch-2 tail removed");
    }
}

#[test]
fn epoch_must_increase() {
    for kind in kinds() {
        let t = TempWal::new("monotonic");
        let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
        w.append(5, b"x").unwrap();
        w.commit_epoch(5).unwrap();
        assert_eq!(w.commit_epoch(3), Err(WalError::InvalidArgument), "kind {kind} lower");
        assert_eq!(w.commit_epoch(5), Err(WalError::InvalidArgument), "kind {kind} equal");
        w.commit_epoch(6).unwrap();
        assert_eq!(w.epoch(), 6, "kind {kind} advanced");
    }
}

#[test]
fn both_slots_invalid_on_nonempty_log_fails() {
    for kind in kinds() {
        let t = TempWal::new("bothbad");
        {
            let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
            w.append(1, b"data").unwrap();
            w.commit_epoch(1).unwrap();
        }
        let hp = t.header();
        let mut bytes = fs::read(&hp).unwrap();
        bytes[8] ^= 0xFF; // slot A epoch field
        if bytes.len() >= 4096 + 16 {
            bytes[4096 + 8] ^= 0xFF; // slot B epoch field
        }
        fs::write(&hp, &bytes).unwrap();
        // Open runs recovery, which must refuse a nonempty log with no
        // valid slot. The exact error code differs by tier, so assert failure.
        assert!(open_with(kind, &t.log(), &t.header()).is_err(), "kind {kind} must fail");
    }
}

#[test]
fn committed_payload_bit_rot_is_detected() {
    for kind in kinds() {
        let t = TempWal::new("bitrot");
        {
            let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
            w.append(1, b"important payload").unwrap();
            w.commit_epoch(1).unwrap();
        }
        // Flip a payload byte inside the committed region (after the 32-byte
        // frame). The payload CRC must catch it.
        let lp = t.log();
        let mut bytes = fs::read(&lp).unwrap();
        bytes[32] ^= 0xFF;
        fs::write(&lp, &bytes).unwrap();
        assert!(open_with(kind, &t.log(), &t.header()).is_err(), "kind {kind} must detect rot");
    }
}

#[test]
fn oversized_payload_is_rejected() {
    for kind in kinds() {
        let t = TempWal::new("oversize");
        let mut w = open_with(kind, &t.log(), &t.header()).unwrap();
        let big = vec![0u8; MAX_PAYLOAD + 1];
        assert_eq!(w.append(1, &big), Err(WalError::InvalidArgument), "kind {kind} oversize");
    }
}

#[test]
fn cross_backend_recovery() {
    // Write with each backend, recover with every other backend.
    for writer in kinds() {
        let t = TempWal::new("cross");
        {
            let mut w = open_with(writer, &t.log(), &t.header()).unwrap();
            w.append(1, b"shared format").unwrap();
            w.append(1, b"second record").unwrap();
            w.commit_epoch(7).unwrap();
        }
        for reader in kinds() {
            let mut w = open_with(reader, &t.log(), &t.header()).unwrap();
            let s = w.recover().unwrap();
            assert_eq!(s.epoch, 7, "writer {writer} reader {reader} epoch");
            assert_eq!(s.records_recovered, 2, "writer {writer} reader {reader} records");
            assert_eq!(s.truncated_bytes, 0, "writer {writer} reader {reader} clean");
        }
    }
}