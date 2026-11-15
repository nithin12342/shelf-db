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
//! ```
//!
//! Every module is pure Rust with no external dependency. The ports
//! (PinningPolicy, RequestHandler, ThreadPinner) let adapters change without
//! touching callers, which is the dependency-inversion rule from the spec.

pub mod affinity;
pub mod epoch;
pub mod ingress;
pub mod nerve;
pub mod pin;
pub mod pinning;

pub use epoch::{EpochGuard, EpochRegistry};
pub use pin::PinnedStalkView;
pub use pinning::{OsThreadPinner, PinError, PinErrorKind, PinToken, ThreadPinner};