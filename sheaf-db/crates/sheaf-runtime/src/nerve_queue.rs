// nerve_queue.rs - bounded lock-free multi-producer multi-consumer ring (MF-01).
//
// Each slot holds a sequence number. A producer claims a slot with a
// compare_exchange on the enqueue counter and then publishes the value with a
// release store to the slot sequence. A consumer claims a slot with a
// compare_exchange on the dequeue counter and then frees the slot with a
// release store. There is no mutex and no lock on the hot path.
//
// The ring never blocks and never drops a value. A push to a full ring returns
// QueueFull and a pop from an empty ring returns Empty. The caller decides
// whether to retry, which keeps the wait policy out of the data structure.

use std::cell::UnsafeCell;
use std::fmt;
use std::mem::MaybeUninit;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Why a ring operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    /// The ring holds `capacity` values and cannot accept another.
    QueueFull,
    /// The ring holds no value.
    Empty,
}

impl fmt::Display for RingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RingError::QueueFull => write!(f, "ring is full"),
            RingError::Empty => write!(f, "ring is empty"),
        }
    }
}

impl std::error::Error for RingError {}

/// One ring slot. `sequence` is the slot state, not the slot data.
struct Slot<T> {
    sequence: AtomicUsize,
    value: UnsafeCell<MaybeUninit<T>>,
}

/// A bounded ring with many producers and many consumers.
pub struct BoundedRing<T> {
    slots: Box<[Slot<T>]>,
    mask: usize,
    enqueue: AtomicUsize,
    dequeue: AtomicUsize,
}

// The slot data is written only after a producer claims the slot and read only
// after a consumer claims it, so the unsafe cell never aliases a live value.
unsafe impl<T: Send> Send for BoundedRing<T> {}
unsafe impl<T: Send> Sync for BoundedRing<T> {}

impl<T> BoundedRing<T> {
    /// Build a ring that holds at least `capacity` values. The capacity rounds
    /// up to a power of two and never falls below two.
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = capacity.max(2).next_power_of_two();
        let slots = (0..cap)
            .map(|i| Slot { sequence: AtomicUsize::new(i), value: UnsafeCell::new(MaybeUninit::uninit()) })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self { slots, mask: cap - 1, enqueue: AtomicUsize::new(0), dequeue: AtomicUsize::new(0) }
    }

    /// The rounded capacity. It is always a power of two.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// The number of values in the ring at this instant.
    pub fn len(&self) -> usize {
        let head = self.enqueue.load(Ordering::Acquire);
        let tail = self.dequeue.load(Ordering::Acquire);
        head.wrapping_sub(tail).min(self.slots.len())
    }

    /// True when the ring holds no value at this instant.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Add one value. Returns QueueFull without a side effect when the ring is
    /// full. This call never blocks.
    pub fn push(&self, value: T) -> Result<(), RingError> {
        let mut pos = self.enqueue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[pos & self.mask];
            let seq = slot.sequence.load(Ordering::Acquire);
            // The signed difference covers counter wrap.
            let dif = seq.wrapping_sub(pos) as isize;
            if dif == 0 {
                match self.enqueue.compare_exchange_weak(
                    pos,
                    pos.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // The slot is claimed, so this write has no other reader.
                        unsafe { (*slot.value.get()).write(value) };
                        slot.sequence.store(pos.wrapping_add(1), Ordering::Release);
                        return Ok(());
                    }
                    Err(current) => pos = current,
                }
            } else if dif < 0 {
                return Err(RingError::QueueFull);
            } else {
                pos = self.enqueue.load(Ordering::Relaxed);
            }
        }
    }

    /// Remove one value. Returns Empty without a side effect when the ring is
    /// empty. This call never blocks.
    pub fn pop(&self) -> Result<T, RingError> {
        let mut pos = self.dequeue.load(Ordering::Relaxed);
        loop {
            let slot = &self.slots[pos & self.mask];
            let seq = slot.sequence.load(Ordering::Acquire);
            let dif = seq.wrapping_sub(pos.wrapping_add(1)) as isize;
            if dif == 0 {
                match self.dequeue.compare_exchange_weak(
                    pos,
                    pos.wrapping_add(1),
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // The slot is claimed, so this read has no other writer.
                        let value = unsafe { (*slot.value.get()).assume_init_read() };
                        // Mark the slot free for the producer one lap ahead.
                        slot.sequence.store(pos.wrapping_add(self.mask).wrapping_add(1), Ordering::Release);
                        return Ok(value);
                    }
                    Err(current) => pos = current,
                }
            } else if dif < 0 {
                return Err(RingError::Empty);
            } else {
                pos = self.dequeue.load(Ordering::Relaxed);
            }
        }
    }

    /// Remove at most `limit` values into `out`. The call stops at the first
    /// Empty result.
    pub fn pop_batch(&self, limit: usize, out: &mut Vec<T>) -> usize {
        let mut taken = 0;
        while taken < limit {
            match self.pop() {
                Ok(value) => {
                    out.push(value);
                    taken += 1;
                }
                Err(_) => break,
            }
        }
        taken
    }
}

impl<T> Drop for BoundedRing<T> {
    fn drop(&mut self) {
        // Drop the values that no consumer took.
        while self.pop().is_ok() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn capacity_rounds_up_to_a_power_of_two() {
        assert_eq!(BoundedRing::<u8>::with_capacity(0).capacity(), 2);
        assert_eq!(BoundedRing::<u8>::with_capacity(3).capacity(), 4);
        assert_eq!(BoundedRing::<u8>::with_capacity(1024).capacity(), 1024);
        assert_eq!(BoundedRing::<u8>::with_capacity(1000).capacity(), 1024);
    }

    #[test]
    fn full_ring_reports_queue_full_and_then_drains() {
        let ring = BoundedRing::with_capacity(4);
        for i in 0..4u64 {
            ring.push(i).expect("push into free slot");
        }
        assert_eq!(ring.push(99), Err(RingError::QueueFull));
        assert_eq!(ring.len(), 4);
        assert_eq!(ring.pop(), Ok(0));
        // The freed slot is reusable after one pop.
        ring.push(4).expect("push after pop");
        let mut got = Vec::new();
        while let Ok(v) = ring.pop() {
            got.push(v);
        }
        assert_eq!(got, vec![1, 2, 3, 4]);
        assert_eq!(ring.pop(), Err(RingError::Empty));
    }

    #[test]
    fn single_producer_single_consumer_keeps_order() {
        let ring = BoundedRing::with_capacity(64);
        let count = 100_000u64;
        thread::scope(|scope| {
            scope.spawn(|| {
                for i in 0..count {
                    while ring.push(i).is_err() {
                        std::hint::spin_loop();
                    }
                }
            });
            let mut received = 0u64;
            while received < count {
                match ring.pop() {
                    Ok(v) => {
                        assert_eq!(v, received, "order must be exact");
                        received += 1;
                    }
                    Err(_) => std::hint::spin_loop(),
                }
            }
        });
        assert!(ring.is_empty());
    }

    #[test]
    fn eight_producers_and_eight_consumers_lose_nothing() {
        const PRODUCERS: usize = 8;
        const PER_PRODUCER: usize = 50_000;
        const CONSUMERS: usize = 8;
        let ring = BoundedRing::with_capacity(1024);
        let total = PRODUCERS * PER_PRODUCER;
        let pushed = AtomicUsize::new(0);
        let mut received: Vec<u64> = Vec::with_capacity(total);

        thread::scope(|scope| {
            let ring_ref = &ring;
            let pushed_ref = &pushed;
            for p in 0..PRODUCERS {
                let base = (p * PER_PRODUCER) as u64;
                scope.spawn(move || {
                    for i in 0..PER_PRODUCER as u64 {
                        let value = base + i;
                        while let Err(RingError::QueueFull) = ring_ref.push(value) {
                            std::hint::spin_loop();
                        }
                        pushed_ref.fetch_add(1, Ordering::Release);
                    }
                });
            }
            let handles: Vec<_> = (0..CONSUMERS)
                .map(|_| {
                    scope.spawn(|| {
                        let mut local = Vec::new();
                        loop {
                            match ring_ref.pop() {
                                Ok(v) => local.push(v),
                                Err(_) => {
                                    if pushed_ref.load(Ordering::Acquire) == total && ring_ref.is_empty() {
                                        break;
                                    }
                                    std::hint::spin_loop();
                                }
                            }
                        }
                        local
                    })
                })
                .collect();
            for handle in handles {
                received.extend(handle.join().expect("consumer thread"));
            }
        });

        assert_eq!(received.len(), total, "every message must arrive once");
        received.sort_unstable();
        let expected: Vec<u64> = (0..total as u64).collect();
        assert_eq!(received, expected, "the multiset must match the sent set");
    }

    #[test]
    fn pop_batch_stops_at_empty() {
        let ring = BoundedRing::with_capacity(8);
        ring.push(1u32).unwrap();
        ring.push(2u32).unwrap();
        let mut out = Vec::new();
        assert_eq!(ring.pop_batch(10, &mut out), 2);
        assert_eq!(out, vec![1, 2]);
        assert_eq!(ring.pop_batch(10, &mut out), 0);
    }
}
