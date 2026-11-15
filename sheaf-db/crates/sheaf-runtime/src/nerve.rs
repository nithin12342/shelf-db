// nerve.rs - in-process nerve routing, spec Pillar 4.
//
// Intermediate query stages transfer topological references directly
// in-process. They never cross a loopback TCP or IPC socket. The router owns
// one queue per core. A message is routed to the core that owns its partition
// by the pinning policy. Each queue has its own lock, so workers on different
// cores never contend on a single global mutex (the cache-line bouncing rule).

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::affinity::PinningPolicy;

/// A topological reference passed between query stages. `partition` selects
/// the destination core. `cell_id` names the target cell. `payload` carries
/// the serialized section or delta bytes.
#[derive(Clone, Debug, PartialEq)]
pub struct NerveMessage {
    pub partition: u64,
    pub cell_id: u64,
    pub payload: Vec<u8>,
}

impl NerveMessage {
    /// Build a reference with an empty payload.
    pub fn new(partition: u64, cell_id: u64) -> Self {
        Self { partition, cell_id, payload: Vec::new() }
    }
}

/// The in-process router. It holds one core-local queue per worker.
pub struct NerveRouter<P: PinningPolicy> {
    policy: P,
    queues: Vec<Mutex<VecDeque<NerveMessage>>>,
}

impl<P: PinningPolicy> NerveRouter<P> {
    /// Build a router with one queue per core in the policy.
    pub fn new(policy: P) -> Self {
        let queues = (0..policy.cores()).map(|_| Mutex::new(VecDeque::new())).collect();
        Self { policy, queues }
    }

    /// Number of core-local queues.
    pub fn cores(&self) -> usize {
        self.queues.len()
    }

    /// Route a message to the core that owns its partition. This is a direct
    /// in-process handoff, not a socket write.
    pub fn send(&self, msg: NerveMessage) {
        let core = self.policy.core_for(msg.partition);
        self.queues[core].lock().expect("nerve queue poisoned").push_back(msg);
    }

    /// Pop the next message for `core`, in arrival order. Returns None when
    /// the queue is empty.
    pub fn recv(&self, core: usize) -> Option<NerveMessage> {
        self.queues.get(core)?.lock().expect("nerve queue poisoned").pop_front()
    }

    /// Number of messages waiting on `core`.
    pub fn pending(&self, core: usize) -> usize {
        self.queues.get(core).map(|q| q.lock().expect("nerve queue poisoned").len()).unwrap_or(0)
    }

    /// Take every message waiting on `core`, in arrival order.
    pub fn drain(&self, core: usize) -> Vec<NerveMessage> {
        match self.queues.get(core) {
            Some(q) => q.lock().expect("nerve queue poisoned").drain(..).collect(),
            None => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::affinity::CoreTopology;

    fn router(cores: usize) -> NerveRouter<CoreTopology> {
        NerveRouter::new(CoreTopology::single_domain(cores))
    }

    #[test]
    fn messages_land_on_the_owning_core() {
        let r = router(4);
        // Partition p maps to core p % 4.
        r.send(NerveMessage::new(0, 100));
        r.send(NerveMessage::new(5, 101));
        r.send(NerveMessage::new(2, 102));
        assert_eq!(r.pending(0), 1);
        assert_eq!(r.pending(1), 1);
        assert_eq!(r.pending(2), 1);
        assert_eq!(r.pending(3), 0);
        assert_eq!(r.recv(1).unwrap().cell_id, 101);
    }

    #[test]
    fn queue_preserves_arrival_order() {
        let r = router(1);
        for i in 0..5 {
            r.send(NerveMessage::new(0, i));
        }
        for i in 0..5 {
            assert_eq!(r.recv(0).unwrap().cell_id, i);
        }
        assert!(r.recv(0).is_none());
    }

    #[test]
    fn drain_takes_all_and_empties() {
        let r = router(2);
        r.send(NerveMessage::new(0, 1));
        r.send(NerveMessage::new(2, 2));
        r.send(NerveMessage::new(4, 3));
        let taken = r.drain(0);
        assert_eq!(taken.len(), 3);
        assert_eq!(r.pending(0), 0);
        assert!(r.drain(0).is_empty());
    }

    #[test]
    fn payload_round_trips() {
        let r = router(1);
        let mut msg = NerveMessage::new(7, 42);
        msg.payload = vec![1, 2, 3, 4];
        r.send(msg.clone());
        assert_eq!(r.recv(0).unwrap(), msg);
    }

    #[test]
    fn out_of_range_core_is_empty() {
        let r = router(2);
        assert!(r.recv(99).is_none());
        assert_eq!(r.pending(99), 0);
        assert!(r.drain(99).is_empty());
    }
}