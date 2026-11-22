// consensus/mod.rs - deterministic Raft consensus (NF-07).
//
// The engine has no wall clock and no real sockets. A Cluster drives elections
// and replication by delivering messages in a fixed order. The safety rules
// are explicit: one leader per term, a write commits only on a majority, and
// a minority cannot commit. These are the rules the tests check.

pub mod cluster;
pub mod log;
pub mod state;

pub use cluster::{Cluster, ConsensusError};
pub use log::{Entry, Log};
pub use state::{Message, Node, Persisted, Role};
