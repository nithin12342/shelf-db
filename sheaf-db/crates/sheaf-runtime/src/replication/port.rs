// port.rs - the replication and RDMA ports (NF-05, NF-06).
//
// A caller depends on these traits only. The TCP adapter and the unsupported
// RDMA adapter are interchangeable implementations behind the ports.

use super::SectionDelta;

pub type Result<T> = std::result::Result<T, ReplicationError>;

/// Why a replication operation failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplicationError {
    /// Socket or thread input and output failed.
    Io(String),
    /// A frame was truncated, oversized, or failed its checksum.
    Framing(String),
    /// A transport the host cannot provide returned this explicit result.
    Unsupported(String),
}

impl std::fmt::Display for ReplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplicationError::Io(message) => write!(f, "replication I/O: {message}"),
            ReplicationError::Framing(message) => write!(f, "replication frame: {message}"),
            ReplicationError::Unsupported(message) => write!(f, "unsupported: {message}"),
        }
    }
}

impl std::error::Error for ReplicationError {}

/// The acknowledgement for one delivered delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ack {
    pub sequence: u64,
}

/// The port that moves section deltas between nodes.
pub trait ReplicationTransport {
    fn send(&self, delta: &SectionDelta) -> Result<Ack>;
    fn receive(&self) -> Result<SectionDelta>;
}

/// What a host can do with a transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    /// The transport is present.
    Available,
    /// The transport is absent. The reason names the missing device or library.
    Unsupported { reason: String },
}

/// The remote direct memory access port. It is separate from the TCP port so
/// a host without the device can report an explicit capability result.
pub trait RdmaTransport {
    fn capability(&self) -> Capability;
    fn post_write(&self, delta: &SectionDelta) -> Result<Ack>;
}
