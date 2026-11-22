// ebpf/mod.rs - the eBPF VFS interception boundary (NF-11).
//
// The policy engine is pure Rust and runs on every host. The loader is a
// platform boundary: this Windows host reports Unsupported with a named cause
// and never claims that a BPF program is attached.

pub mod loader_unsupported;
pub mod policy;

pub use loader_unsupported::{Capability, LoaderError, UnsupportedLoader, REASON};
pub use policy::{Decision, Op, RuleSet, VfsEvent, VfsPolicy};
