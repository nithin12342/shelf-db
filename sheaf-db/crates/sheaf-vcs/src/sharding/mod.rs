// sharding/mod.rs - open-cover sharding with ghost cells (NF-08).
//
// A cover assigns every cell to at least one shard. A boundary cell is
// replicated as a ghost into the shards that touch it. A ghost carries the
// exact value of its owner, so gluing is a checked equality. A disagreement
// is a reported conflict, never a silent overwrite.

pub mod assign;
pub mod cover;
pub mod ghost;
pub mod glue;

pub use assign::assign_vertex;
pub use cover::{Cover, Shard, ShardError};
pub use ghost::GhostCell;
pub use glue::{GlueConflict, glue};
