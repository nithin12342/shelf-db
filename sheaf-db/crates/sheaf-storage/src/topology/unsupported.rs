// topology/unsupported.rs - the explicit fallback for other targets.
//
// The engine must not invent an L3 layout. Every call returns Unsupported with
// the target name, so a planner can report the missing capability instead of
// trusting a made-up domain map.

use super::{L3Domain, TopologyError};

pub(super) fn l3_domains() -> Result<Vec<L3Domain>, TopologyError> {
    Err(TopologyError::Unsupported(std::env::consts::OS.to_string()))
}
