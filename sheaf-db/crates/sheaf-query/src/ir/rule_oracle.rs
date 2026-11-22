// Timestamp: 2026-11-22_11-53-08 +05:30
// rule_oracle.rs - the proof-gated rule adapter for sheaf-query (NF-03).
//
// sheaf-proofs owns the port and the generated adapter. This module supplies
// one construction point for callers, so a caller cannot accidentally use the
// handwritten mirror when a proof-backed table is available.

use sheaf_proofs::oracle::{GeneratedRules, HandwrittenRules, RuleOracle};

/// The oracle used by the production rewrite path.
pub fn production_oracle() -> Result<GeneratedRules, String> {
    GeneratedRules::load()
}

/// The declared fallback. It reports its reason through the port.
pub fn fallback_oracle() -> HandwrittenRules {
    HandwrittenRules
}

/// Whether an oracle is proof-backed. A caller must report this in its plan.
pub fn is_proof_backed(oracle: &impl RuleOracle) -> bool {
    oracle.fallback_reason().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_production_oracle_is_proof_backed() {
        let oracle = production_oracle().expect("load");
        assert!(is_proof_backed(&oracle));
        assert_eq!(oracle.source(), "lean-emitted-header");
    }

    #[test]
    fn the_fallback_reports_its_reason() {
        let oracle = fallback_oracle();
        assert!(!is_proof_backed(&oracle));
        assert!(oracle.fallback_reason().is_some_and(|reason| !reason.is_empty()));
    }
}
