//! sheaf-proofs - the formal verification bridge (spec Pillar 6, Phase 4).
//!
//! The Lean 4 tier (proofs/) proves the sheaf axioms and the TIR rewrite
//! rules. The Lean build emits a static rule validity table as a C header
//! (native/proofs-generated/sheaf_verified_rules.h). This crate mirrors that
//! table in Rust and applies rewrites gated by proof, not runtime checks.
//!
//! Module map:
//!
//! ```text
//! rules    the verified rule table (mirror of the Lean-emitted header)
//! planner  proof-gated rewrites on a linear plan model
//! ```

pub mod generated;
pub mod oracle;
pub mod planner;
pub mod rules;

pub use oracle::{
    FallbackReason, GeneratedRules, HandwrittenRules, RuleId, RuleOracle,
};
pub use rules::{is_verified, lookup, VerifiedRule, VERIFIED_RULES};