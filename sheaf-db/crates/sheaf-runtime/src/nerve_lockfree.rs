// nerve_lockfree.rs - the lock-free nerve router (MF-01).
//
// One bounded ring per core. A send maps its partition to a core through the
// pinning policy and pushes into that ring with no mutex. Two producers on
// different cores therefore never touch the same lock and never bounce a
// cache line outside their own ring.
//
// The router is the adapter for the NerveTransport port. The policy is a type
// parameter, so a new routing rule is a new adapter, not a change here.

use crate::affinity::PinningPolicy;
use crate::nerve_port::{NerveError, NerveMessage, NerveTransport};
use crate::nerve_queue::{BoundedRing, RingError};

/// The default ring capacity per core. It rounds to a power of two.
pub const DEFAULT_RING_CAPACITY: usize = 1024;

/// A router with one lock-free ring per core.
pub struct LockFreeNerveRouter<P: PinningPolicy> {
    policy: P,
    rings: Vec<BoundedRing<NerveMessage>>,
}

impl<P: PinningPolicy> LockFreeNerveRouter<P> {
    /// Build a router with one ring of `capacity` messages per core.
    pub fn new(policy: P, capacity: usize) -> Self {
        let rings = (0..policy.cores()).map(|_| BoundedRing::with_capacity(capacity)).collect();
        Self { policy, rings }
    }

    /// Build a router with the default ring capacity.
    pub fn with_default_capacity(policy: P) -> Self {
        Self::new(policy, DEFAULT_RING_CAPACITY)
    }

    /// The rounded ring capacity that the router holds per core.
    pub fn ring_capacity(&self) -> usize {
        self.rings.first().map(|r| r.capacity()).unwrap_or(0)
    }

    /// The routing policy.
    pub fn policy(&self) -> &P {
        &self.policy
    }

    fn ring(&self, core: usize) -> Result<&BoundedRing<NerveMessage>, NerveError> {
        self.rings.get(core).ok_or(NerveError::NoSuchCore(core))
    }
}

impl<P: PinningPolicy> NerveTransport for LockFreeNerveRouter<P> {
    fn cores(&self) -> usize {
        self.rings.len()
    }

    fn core_for(&self, partition: u64) -> usize {
        self.policy.core_for(partition)
    }

    fn send(&self, msg: NerveMessage) -> Result<(), NerveError> {
        let core = self.policy.core_for(msg.partition);
        self.ring(core)?.push(msg).map_err(|e| match e {
            RingError::QueueFull => NerveError::QueueFull,
            RingError::Empty => NerveError::Empty,
        })
    }

    fn recv(&self, core: usize) -> Result<NerveMessage, NerveError> {
        self.ring(core)?.pop().map_err(|e| match e {
            RingError::QueueFull => NerveError::QueueFull,
            RingError::Empty => NerveError::Empty,
        })
    }

    fn pending(&self, core: usize) -> Result<usize, NerveError> {
        Ok(self.ring(core)?.len())
    }

    fn drain(&self, core: usize) -> Result<Vec<NerveMessage>, NerveError> {
        let ring = self.ring(core)?;
        let mut out = Vec::with_capacity(ring.len());
        ring.pop_batch(usize::MAX, &mut out);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affinity::CoreTopology;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    fn router(cores: usize, capacity: usize) -> LockFreeNerveRouter<CoreTopology> {
        LockFreeNerveRouter::new(CoreTopology::single_domain(cores), capacity)
    }

    #[test]
    fn messages_land_on_the_owning_core() {
        let r = router(4, 16);
        r.send(NerveMessage::new(0, 100)).expect("send");
        r.send(NerveMessage::new(5, 101)).expect("send");
        r.send(NerveMessage::new(2, 102)).expect("send");
        assert_eq!(r.cores(), 4);
        assert_eq!(r.pending(0), Ok(1));
        assert_eq!(r.pending(1), Ok(1));
        assert_eq!(r.pending(2), Ok(1));
        assert_eq!(r.pending(3), Ok(0));
        assert_eq!(r.recv(1).expect("recv").cell_id, 101);
        assert_eq!(r.core_for(5), 1);
    }

    #[test]
    fn ring_capacity_is_rounded_and_reported() {
        let r = router(1, 1000);
        assert_eq!(r.ring_capacity(), 1024);
    }

    #[test]
    fn queue_preserves_arrival_order() {
        let r = router(1, 8);
        for i in 0..8u64 {
            r.send(NerveMessage::new(0, i)).expect("send");
        }
        for i in 0..8u64 {
            assert_eq!(r.recv(0).expect("recv").cell_id, i);
        }
        assert_eq!(r.recv(0), Err(NerveError::Empty));
    }

    #[test]
    fn full_ring_reports_queue_full_and_recovers() {
        let r = router(1, 2);
        r.send(NerveMessage::new(0, 1)).expect("send");
        r.send(NerveMessage::new(0, 2)).expect("send");
        assert_eq!(r.send(NerveMessage::new(0, 3)), Err(NerveError::QueueFull));
        assert_eq!(r.pending(0), Ok(2));
        assert_eq!(r.recv(0).expect("recv").cell_id, 1);
        r.send(NerveMessage::new(0, 3)).expect("send after recv");
        assert_eq!(r.pending(0), Ok(2));
    }

    #[test]
    fn drain_takes_every_message_once() {
        let r = router(2, 64);
        for i in 0..10u64 {
            r.send(NerveMessage::new(i * 2, i)).expect("send");
        }
        let taken = r.drain(0).expect("drain");
        assert_eq!(taken.len(), 10);
        let ids: Vec<u64> = taken.iter().map(|m| m.cell_id).collect();
        assert_eq!(ids, (0..10u64).collect::<Vec<_>>());
        assert_eq!(r.pending(0), Ok(0));
        assert!(r.drain(0).expect("drain empty").is_empty());
    }

    #[test]
    fn out_of_range_core_is_reported() {
        let r = router(2, 8);
        assert_eq!(r.recv(99), Err(NerveError::NoSuchCore(99)));
        assert_eq!(r.pending(99), Err(NerveError::NoSuchCore(99)));
        assert_eq!(r.drain(99), Err(NerveError::NoSuchCore(99)));
    }

    #[test]
    fn eight_producers_and_eight_consumers_through_the_router() {
        const PRODUCERS: usize = 8;
        const PER_PRODUCER: usize = 50_000;
        const CONSUMERS: usize = 8;
        let r = router(1, 1024);
        let total = PRODUCERS * PER_PRODUCER;
        let sent = AtomicUsize::new(0);
        let mut received: Vec<u64> = Vec::with_capacity(total);

        thread::scope(|scope| {
            let router = &r;
            let sent_ref = &sent;
            for p in 0..PRODUCERS {
                let base = (p * PER_PRODUCER) as u64;
                scope.spawn(move || {
                    for i in 0..PER_PRODUCER as u64 {
                        let msg = NerveMessage::new(0, base + i);
                        while let Err(NerveError::QueueFull) = router.send(msg.clone()) {
                            std::hint::spin_loop();
                        }
                        sent_ref.fetch_add(1, Ordering::Release);
                    }
                });
            }
            let handles: Vec<_> = (0..CONSUMERS)
                .map(|_| {
                    scope.spawn(|| {
                        let mut local = Vec::new();
                        loop {
                            match router.recv(0) {
                                Ok(msg) => local.push(msg.cell_id),
                                Err(NerveError::Empty) => {
                                    let done = sent_ref.load(Ordering::Acquire) == total
                                        && router.pending(0) == Ok(0);
                                    if done {
                                        break;
                                    }
                                    std::hint::spin_loop();
                                }
                                Err(other) => panic!("unexpected transport error: {other}"),
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
        assert_eq!(received, (0..total as u64).collect::<Vec<_>>());
    }
}
