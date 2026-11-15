// epoch.rs - lock-free epoch-based reclamation (EBR), spec Pillar 4.
//
// Transaction versions carry a monotonically increasing epoch number. A
// reader pins the current epoch for the lifetime of its view. A writer
// retires an old version at some epoch E. That version is safe to unmap
// only when every reader that could still see epoch E has retired its lease.
//
// The participant table is a fixed array of atomic slots. Each slot holds
// either INACTIVE (no reader) or the epoch a reader pinned. A pin claims a
// free slot with one compare-and-swap. A drop releases the slot with one
// store. There is no mutex on the read path, so concurrent readers never
// block each other or the writer (the lock-free rule from the spec).

use std::sync::atomic::{AtomicU64, Ordering};

/// Sentinel for a slot that no reader currently holds.
const INACTIVE: u64 = u64::MAX;

/// Default number of concurrent reader leases the table can hold.
pub const DEFAULT_PARTICIPANTS: usize = 64;

/// The shared epoch registry. Writers advance the global epoch. Readers pin
/// it. The reclamation test compares a retired epoch against the smallest
/// epoch still held by an active reader.
pub struct EpochRegistry {
    global: AtomicU64,
    slots: Box<[AtomicU64]>,
}

impl EpochRegistry {
    /// Build a registry with `participants` reader slots. The global epoch
    /// starts at zero. Every slot starts inactive.
    pub fn new(participants: usize) -> Self {
        let count = participants.max(1);
        let slots = (0..count).map(|_| AtomicU64::new(INACTIVE)).collect();
        Self { global: AtomicU64::new(0), slots }
    }

    /// The current global epoch.
    pub fn current(&self) -> u64 {
        self.global.load(Ordering::Acquire)
    }

    /// Advance the global epoch by one and return the new value. A writer
    /// calls this after it publishes a new version, so later readers pin a
    /// newer epoch and older versions become candidates for reclamation.
    pub fn advance(&self) -> u64 {
        self.global.fetch_add(1, Ordering::AcqRel) + 1
    }

    /// Pin the current epoch. Returns a guard that releases the slot on drop.
    /// Returns None when every participant slot is already held, so the
    /// caller can back off instead of blocking.
    pub fn pin(&self) -> Option<EpochGuard<'_>> {
        let epoch = self.global.load(Ordering::Acquire);
        for (index, slot) in self.slots.iter().enumerate() {
            // Claim the slot only if it is still free. The epoch we store is
            // the one we read, which is <= the true current epoch. That is
            // safe: pinning an older epoch only delays reclamation.
            if slot
                .compare_exchange(INACTIVE, epoch, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(EpochGuard { registry: self, index, epoch });
            }
        }
        None
    }

    /// The smallest epoch still held by an active reader. When no reader is
    /// active this returns the current global epoch, because nothing older
    /// can still be observed.
    pub fn min_active_epoch(&self) -> u64 {
        let mut min = self.global.load(Ordering::Acquire);
        for slot in self.slots.iter() {
            let value = slot.load(Ordering::Acquire);
            if value != INACTIVE && value < min {
                min = value;
            }
        }
        min
    }

    /// True when a version retired at `retired_epoch` can be reclaimed. The
    /// strict comparison keeps any epoch a live reader still holds.
    pub fn reclaimable(&self, retired_epoch: u64) -> bool {
        retired_epoch < self.min_active_epoch()
    }

    /// Number of reader leases currently held. Used by tests and metrics.
    pub fn active_readers(&self) -> usize {
        self.slots.iter().filter(|s| s.load(Ordering::Acquire) != INACTIVE).count()
    }
}

impl Default for EpochRegistry {
    fn default() -> Self {
        Self::new(DEFAULT_PARTICIPANTS)
    }
}

/// A reader lease on one epoch. While this guard lives, the registry will
/// not report its epoch as reclaimable. Dropping it releases the slot.
pub struct EpochGuard<'a> {
    registry: &'a EpochRegistry,
    index: usize,
    epoch: u64,
}

impl EpochGuard<'_> {
    /// The epoch this lease pins.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

impl Drop for EpochGuard<'_> {
    fn drop(&mut self) {
        // Release the slot so a later pin can reuse it. Release ordering
        // pairs with the Acquire loads in min_active_epoch.
        self.registry.slots[self.index].store(INACTIVE, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_starts_at_zero_and_advances() {
        let reg = EpochRegistry::new(4);
        assert_eq!(reg.current(), 0);
        assert_eq!(reg.advance(), 1);
        assert_eq!(reg.advance(), 2);
        assert_eq!(reg.current(), 2);
    }

    #[test]
    fn guard_blocks_reclamation_until_dropped() {
        let reg = EpochRegistry::new(4);
        let guard = reg.pin().expect("a free slot");
        assert_eq!(guard.epoch(), 0);
        assert_eq!(reg.active_readers(), 1);
        // A version retired at epoch 0 is still visible to the live reader.
        assert!(!reg.reclaimable(0));
        drop(guard);
        assert_eq!(reg.active_readers(), 0);
        // With no reader, epoch 0 is now safe to reclaim.
        assert!(reg.reclaimable(0));
    }

    #[test]
    fn advancing_does_not_reclaim_a_pinned_epoch() {
        let reg = EpochRegistry::new(4);
        let guard = reg.pin().unwrap();
        reg.advance();
        reg.advance();
        // The reader still pins epoch 0 even though the global epoch moved.
        assert!(!reg.reclaimable(0));
        assert_eq!(reg.min_active_epoch(), 0);
        drop(guard);
        assert!(reg.reclaimable(0));
        assert!(reg.reclaimable(1));
    }

    #[test]
    fn table_full_returns_none() {
        let reg = EpochRegistry::new(2);
        let _a = reg.pin().unwrap();
        let _b = reg.pin().unwrap();
        assert!(reg.pin().is_none());
        assert_eq!(reg.active_readers(), 2);
    }

    #[test]
    fn slots_are_reused_after_drop() {
        let reg = EpochRegistry::new(1);
        {
            let _g = reg.pin().unwrap();
            assert!(reg.pin().is_none());
        }
        // The single slot is free again.
        let _g2 = reg.pin().unwrap();
        assert_eq!(reg.active_readers(), 1);
    }
}