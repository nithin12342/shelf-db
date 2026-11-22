// ebpf/policy.rs - the VFS policy engine (NF-11).
//
// Every recorded VFS event gets one explicit decision. A path in a protected
// prefix is denied. A write is denied on a read-only policy. There is no
// implicit allow: the decision type has only Allow and Deny with a reason.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Read,
    Write,
    Execute,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VfsEvent {
    pub op: Op,
    pub path: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    Allow,
    DenyProtected,
    DenyReadOnly,
}

/// The policy port. An adapter decides one event.
pub trait VfsPolicy {
    fn decide(&self, event: &VfsEvent) -> Decision;
}

/// A rule set with protected path prefixes and an optional read-only mode.
#[derive(Clone, Debug, Default)]
pub struct RuleSet {
    pub protected_prefixes: Vec<String>,
    pub read_only: bool,
}

impl VfsPolicy for RuleSet {
    fn decide(&self, event: &VfsEvent) -> Decision {
        if self.protected_prefixes.iter().any(|prefix| event.path.starts_with(prefix)) {
            return Decision::DenyProtected;
        }
        if self.read_only && event.op == Op::Write {
            return Decision::DenyReadOnly;
        }
        Decision::Allow
    }
}
