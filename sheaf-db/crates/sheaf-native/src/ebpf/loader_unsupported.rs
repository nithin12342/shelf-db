// ebpf/loader_unsupported.rs - the VFS loader capability (NF-11).
//
// This host has no Linux BPF facility and no clang BPF target. The loader
// returns an explicit Unsupported result. No call pretends that a program was
// attached.

use super::policy::{Decision, RuleSet, VfsPolicy};

/// Why the loader cannot attach a BPF program here.
pub const REASON: &str = "eBPF VFS interception requires Linux with CAP_BPF and a clang BPF target";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Capability {
    /// The host can load and attach a BPF program.
    Available,
    /// The host cannot. The reason names the missing facility.
    Unsupported { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoaderError {
    Unsupported { reason: String },
}

/// The loader on a host without eBPF.
#[derive(Clone, Copy, Debug, Default)]
pub struct UnsupportedLoader;

impl UnsupportedLoader {
    pub fn capability() -> Capability {
        Capability::Unsupported { reason: REASON.to_owned() }
    }

    /// Evaluate the policy locally for diagnostics. This is not an attach.
    pub fn dry_run(&self, policy: &RuleSet, event: &super::policy::VfsEvent) -> Decision {
        policy.decide(event)
    }

    pub fn load(&self) -> Result<(), LoaderError> {
        Err(LoaderError::Unsupported { reason: REASON.to_owned() })
    }
}
