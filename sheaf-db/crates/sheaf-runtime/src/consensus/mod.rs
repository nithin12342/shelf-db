// Timestamp: 2026-11-22_11-53-08 +05:30
// consensus/mod.rs - sheaf gluing consensus (C-11).
//
// Spec Pillar 7 states that network agreement does not use Raft voting loops.
// Nodes agree when every boundary section satisfies delta s = 0. The gluing
// module owns that rule. No leader, term, or vote exists in this tier.

pub mod gluing;

pub use gluing::{Boundary, GluingCluster, GluingError};
