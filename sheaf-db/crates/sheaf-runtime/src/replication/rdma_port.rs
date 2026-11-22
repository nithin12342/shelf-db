// Timestamp: 2026-11-22_11-53-08 +05:30
// rdma_port.rs - the remote direct memory access port (NF-06).
//
// This file holds only the port and the capability vocabulary. No adapter here
// claims support that the host has not proved. The Windows adapter lives in
// rdma_unsupported.rs.

pub use super::port::{Capability, RdmaTransport};
