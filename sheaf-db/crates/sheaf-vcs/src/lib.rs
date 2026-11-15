//! sheaf-vcs - the sheaf-based version control engine (spec Pillars 8 and 10).
//!
//! A repository is a cellular sheaf over a graph. Commits are zero-coboundary
//! global sections. Merge conflicts are cohomological obstructions. Versions
//! form a BLAKE3 content-addressed DAG. Concurrent writes use LCA-CW.

pub mod complex;