// pin.rs - end-to-end affine buffer pinning, spec Pillar 4.
//
// A query or network stream reads stalk data that lives in a memory-mapped
// arena. Background compaction must not reclaim, truncate, or remap that
// arena while a reader still borrows it. Rust's borrow checker enforces the
// slice lifetime, and the epoch lease enforces the reclamation rule. Together
// they give memory safety without a mutex on the read path.
//
// While a PinnedStalkView is in scope, the registry reports its epoch as not
// reclaimable, so the compactor cannot unmap the underlying bytes.

use crate::epoch::{EpochGuard, EpochRegistry};

/// A borrowed stalk slice protected by an epoch lease. The slice and the
/// registry must both outlive the view. Dropping the view releases the lease.
pub struct PinnedStalkView<'arena> {
    stalk_slice: &'arena [f32],
    epoch_lease: EpochGuard<'arena>,
}

impl<'arena> PinnedStalkView<'arena> {
    /// Pin `stalk_slice` at the current epoch. Returns None when the reader
    /// table is full, so the caller can retry instead of blocking.
    pub fn new(registry: &'arena EpochRegistry, stalk_slice: &'arena [f32]) -> Option<Self> {
        let epoch_lease = registry.pin()?;
        Some(Self { stalk_slice, epoch_lease })
    }

    /// The protected stalk data.
    pub fn data(&self) -> &'arena [f32] {
        self.stalk_slice
    }

    /// Number of f32 lanes in the view.
    pub fn len(&self) -> usize {
        self.stalk_slice.len()
    }

    /// True when the view holds no lanes.
    pub fn is_empty(&self) -> bool {
        self.stalk_slice.is_empty()
    }

    /// The epoch this view pins. Compaction cannot reclaim this epoch while
    /// the view lives.
    pub fn epoch(&self) -> u64 {
        self.epoch_lease.epoch()
    }

    /// Sum the lanes. A small read helper used by tests and by reduction
    /// stages that consume a pinned view without copying it.
    pub fn sum(&self) -> f32 {
        self.stalk_slice.iter().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_pins_and_reads_data() {
        let reg = EpochRegistry::new(4);
        let data = [1.0f32, 2.0, 3.0, 4.0];
        let view = PinnedStalkView::new(&reg, &data).unwrap();
        assert_eq!(view.len(), 4);
        assert_eq!(view.data(), &data);
        assert_eq!(view.sum(), 10.0);
        assert_eq!(view.epoch(), 0);
    }

    #[test]
    fn live_view_blocks_reclamation() {
        let reg = EpochRegistry::new(4);
        let data = [0.5f32; 8];
        let view = PinnedStalkView::new(&reg, &data).unwrap();
        // The compactor asks whether epoch 0 can be reclaimed. It cannot,
        // because the view still borrows the arena at that epoch.
        assert!(!reg.reclaimable(view.epoch()));
        drop(view);
        // The lease is gone, but epoch 0 is still current. Advance past it
        // before the compactor can reclaim the arena.
        reg.advance();
        assert!(reg.reclaimable(0));
    }

    #[test]
    fn empty_view_is_valid() {
        let reg = EpochRegistry::new(2);
        let data: [f32; 0] = [];
        let view = PinnedStalkView::new(&reg, &data).unwrap();
        assert!(view.is_empty());
        assert_eq!(view.sum(), 0.0);
    }

    #[test]
    fn full_table_yields_no_view() {
        let reg = EpochRegistry::new(1);
        let data = [1.0f32];
        let _held = PinnedStalkView::new(&reg, &data).unwrap();
        assert!(PinnedStalkView::new(&reg, &data).is_none());
    }
}