// nerve_port.rs - the nerve transport port and its message type (MF-01).
//
// Query stages hand a topological reference to the stage that owns the
// partition. The handoff stays in process, so no socket and no copy across the
// kernel are needed. Callers depend on this trait. The lock-free router in
// nerve_lockfree.rs is one adapter, and the mutex router in nerve.rs is the
// legacy adapter.
//
// The port reports QueueFull and Empty as explicit errors. It never blocks and
// it never drops a message on the caller's behalf.

use std::fmt;

/// A topological reference passed between query stages. `partition` selects
/// the destination core. `cell_id` names the target cell. `payload` carries
/// the serialized section or delta bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
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

    /// Build a reference with a payload.
    pub fn with_payload(partition: u64, cell_id: u64, payload: Vec<u8>) -> Self {
        Self { partition, cell_id, payload }
    }
}

/// Why a transport operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NerveError {
    /// The destination ring is full. The message was not accepted.
    QueueFull,
    /// The source ring holds no message.
    Empty,
    /// The core number is outside the router topology.
    NoSuchCore(usize),
}

impl fmt::Display for NerveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NerveError::QueueFull => write!(f, "nerve queue is full"),
            NerveError::Empty => write!(f, "nerve queue is empty"),
            NerveError::NoSuchCore(core) => write!(f, "core {core} is outside the router"),
        }
    }
}

impl std::error::Error for NerveError {}

/// The in-process transport port. A caller names a destination by partition on
/// the send side and by core on the receive side, which matches the
/// thread-per-core worker model.
pub trait NerveTransport {
    /// Number of core-local queues.
    fn cores(&self) -> usize;

    /// The core that owns `partition`. The mapping is stable for one router.
    fn core_for(&self, partition: u64) -> usize;

    /// Hand a message to the core that owns its partition. Returns QueueFull
    /// when that ring is full. The message is not queued in that case.
    fn send(&self, msg: NerveMessage) -> Result<(), NerveError>;

    /// Take the next message of `core` in arrival order. Returns Empty when
    /// the ring holds no message and NoSuchCore when `core` is out of range.
    fn recv(&self, core: usize) -> Result<NerveMessage, NerveError>;

    /// Number of messages waiting on `core`.
    fn pending(&self, core: usize) -> Result<usize, NerveError>;

    /// Take every message waiting on `core`, in arrival order.
    fn drain(&self, core: usize) -> Result<Vec<NerveMessage>, NerveError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_keeps_partition_cell_and_payload() {
        let msg = NerveMessage::with_payload(3, 77, vec![9, 8, 7]);
        assert_eq!(msg.partition, 3);
        assert_eq!(msg.cell_id, 77);
        assert_eq!(msg.payload, vec![9, 8, 7]);
        assert_eq!(NerveMessage::new(1, 2).payload.len(), 0);
    }

    #[test]
    fn errors_carry_a_readable_detail() {
        assert_eq!(NerveError::QueueFull.to_string(), "nerve queue is full");
        assert_eq!(NerveError::Empty.to_string(), "nerve queue is empty");
        assert_eq!(NerveError::NoSuchCore(5).to_string(), "core 5 is outside the router");
    }
}
