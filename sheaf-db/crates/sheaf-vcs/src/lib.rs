//! sheaf-vcs - the sheaf-based version control engine (spec Pillars 8 and 10).
//!
//! A repository is a cellular sheaf over a graph. Commits are zero-coboundary
//! global sections. Merge conflicts are cohomological obstructions. Versions
//! form a BLAKE3 content-addressed DAG. Concurrent writes use LCA-CW.
//!
//! Module map:
//!
//! ```text
//! complex   topology, restriction maps, coboundary, star neighborhoods
//! section   0-cochain algebra and the global-section commit rule
//! lca       star-neighborhood collision inspector and parallel waves
//! chart     append-only chronological time charts with delta folding
//! merkle    BLAKE3 content-addressed commit DAG with O(1) branch/tag
//! diff      algebraic time-travel diff and cohomological merge
//! repo      the composition root that wires the modules together
//! ```

pub mod chart;
pub mod complex;
pub mod diff;
pub mod lca;
pub mod merkle;
pub mod repo;
pub mod section;

pub use complex::{Complex, Restriction};
pub use merkle::{Hash, MerkleDag};
pub use repo::{RepoError, Repository};
pub use section::Section;