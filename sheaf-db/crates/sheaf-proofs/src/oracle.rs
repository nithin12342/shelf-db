// oracle.rs - the proof-backed rule oracle and its declared fallback (NF-01).
//
// A rewrite is applied only when the oracle marks its rule verified. The
// generated adapter reads the header emitted by Lean. The handwritten adapter
// is a declared fallback: it reports its reason and never claims to be the
// generated table.

use crate::generated::{self, EmittedTable};
use crate::rules;

/// A rule identifier from the verified table.
pub type RuleId = u16;

/// The port that answers whether a rule is proof-backed.
pub trait RuleOracle {
    /// True when the rule is in the emitted, verified table.
    fn is_verified(&self, rule: RuleId) -> bool;
    /// The checksum of the table the oracle used.
    fn digest(&self) -> u64;
    /// The source of the decision, for an explicit audit line.
    fn source(&self) -> &'static str;
    /// Why a fallback is in use, or None for the generated adapter.
    fn fallback_reason(&self) -> Option<&'static str>;
}

/// Why the generated table could not be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackReason {
    /// The Lean emitter has not produced a header for this build.
    LeanNotBuilt,
}

impl FallbackReason {
    pub fn text(self) -> &'static str {
        match self {
            FallbackReason::LeanNotBuilt => "the Lean rule table was not emitted for this build",
        }
    }
}

/// The adapter backed by the emitted C header.
#[derive(Clone, Debug)]
pub struct GeneratedRules {
    table: EmittedTable,
}

impl GeneratedRules {
    /// Parse the committed header. An invalid header is an error, not a
    /// fallback: drift must fail the caller explicitly.
    pub fn load() -> Result<GeneratedRules, String> {
        let table = generated::parse_header(generated::HEADER)?;
        if generated::recompute_digest(&table.rules) != table.digest {
            return Err("the emitted rule digest does not match the rule rows".into());
        }
        Ok(GeneratedRules { table })
    }

    pub fn table(&self) -> &EmittedTable {
        &self.table
    }
}

impl RuleOracle for GeneratedRules {
    fn is_verified(&self, rule: RuleId) -> bool {
        self.table.rules.iter().any(|entry| entry.id == rule)
    }
    fn digest(&self) -> u64 {
        self.table.digest
    }
    fn source(&self) -> &'static str {
        "lean-emitted-header"
    }
    fn fallback_reason(&self) -> Option<&'static str> {
        None
    }
}

/// The declared fallback. It uses the in-Rust mirror and reports that it is
/// not the generated table.
#[derive(Clone, Copy, Debug, Default)]
pub struct HandwrittenRules;

impl RuleOracle for HandwrittenRules {
    fn is_verified(&self, rule: RuleId) -> bool {
        rules::is_verified(rule)
    }
    fn digest(&self) -> u64 {
        0
    }
    fn source(&self) -> &'static str {
        "handwritten-mirror"
    }
    fn fallback_reason(&self) -> Option<&'static str> {
        Some(FallbackReason::LeanNotBuilt.text())
    }
}
