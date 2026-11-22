// rdma_unsupported.rs - the explicit RDMA boundary (NF-06).
//
// This host has no remote direct memory access device or verbs library. The
// adapter reports that fact under the named cause. It never returns a fake
// success and never silently falls back to TCP inside the RDMA port.

use super::port::{Ack, Capability, RdmaTransport, ReplicationError, Result};
use super::SectionDelta;

/// The reason this host cannot use remote direct memory access.
pub const REASON: &str = "no RDMA verbs device or library is available on this host";

/// An adapter that reports the missing capability.
#[derive(Clone, Copy, Debug, Default)]
pub struct RdmaUnsupported;

impl RdmaTransport for RdmaUnsupported {
    fn capability(&self) -> Capability {
        Capability::Unsupported { reason: REASON.to_owned() }
    }

    fn post_write(&self, _delta: &SectionDelta) -> Result<Ack> {
        Err(ReplicationError::Unsupported(REASON.to_owned()))
    }
}
