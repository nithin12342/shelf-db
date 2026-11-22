//! sheaf-runtime - the Rust orchestration and ingress tier (spec Pillar 4).
//!
//! Module map:
//!
//! ```text
//! epoch     lock-free epoch-based reclamation (EBR) registry + guards
//! pin       affine PinnedStalkView bound to an epoch lease
//! affinity  thread-per-core topology and the pinning policy port
//! nerve     in-process nerve router with core-local queues (no sockets)
//! ingress   TCP ingress with a RequestHandler port and framed transport
//! pinning   OS thread affinity: the physical side of the topology model
//! nerve_queue        bounded lock-free MPMC ring with sequence slots
//! nerve_port         NerveTransport port and the NerveMessage type
//! nerve_lockfree     lock-free router, one ring per core
//! nerve_partition_map ordered B+ tree index that maps a partition to a core
//! ```
//!
//! Every module is pure Rust with no external dependency. The ports
//! (PinningPolicy, RequestHandler, ThreadPinner, NerveTransport) let adapters
//! change without touching callers, which is the dependency-inversion rule from
//! the spec.

pub mod affinity;
pub mod epoch;
pub mod ingress;
pub mod nerve;
pub mod nerve_lockfree;
pub mod nerve_partition_map;
pub mod nerve_port;
pub mod nerve_queue;
pub mod pin;
pub mod pinning;
pub mod replication;

pub use epoch::{EpochGuard, EpochRegistry};
pub use nerve_lockfree::LockFreeNerveRouter;
pub use nerve_partition_map::{IndexError, PartitionIndex, PartitionRanges};
pub use nerve_port::{NerveError, NerveMessage, NerveTransport};
pub use nerve_queue::{BoundedRing, RingError};
pub use pin::PinnedStalkView;
pub use pinning::{OsThreadPinner, PinError, PinErrorKind, PinToken, ThreadPinner};