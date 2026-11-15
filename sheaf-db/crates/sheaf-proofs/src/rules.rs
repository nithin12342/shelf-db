// rules.rs - the verified TIR rewrite-rule table (spec Pillar 6).
//
// The Lean 4 tier proves each rewrite rule semantically invariant. The Lean
// build emits this table as a static C header (native/proofs-generated/
// sheaf_verified_rules.h). This module is the Rust mirror of that header.
//
// Because every entry is proof-backed, the planner applies a rule without a
// runtime validation check. The rule id, the human name, and the Lean theorem
// name are kept in lockstep with the header. A build-time test guards that
// the table is well-formed.

/// A verified rewrite rule. `invariant` records that the Lean theorem proves
/// the rewrite preserves plan semantics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedRule {
    pub id: u16,
    pub name: &'static str,
    pub lean_theorem: &'static str,
    pub invariant: bool,
}

// Rule identifiers. These match the SHEAF_RULE_* macros in the C header.
/// Push a restriction below a colimit evaluation (distributivity).
pub const RULE_RESTRICTION_PUSHDOWN: u16 = 1;
/// Reassociate a nested colimit (associativity of the gluing sum).
pub const RULE_COLIMIT_ASSOC: u16 = 2;
/// Compose two adjacent restrictions into one (functoriality).
pub const RULE_RESTRICTION_COMP: u16 = 3;
/// Commute a discrete filter past a restriction (linearity).
pub const RULE_FILTER_RESTRICT_COMMUTE: u16 = 4;
/// Eliminate a redundant restriction that is the identity morphism.
pub const RULE_IDENTITY_RESTRICTION: u16 = 5;
/// Fold a zero-coboundary section to its global representative.
pub const RULE_GLUING_UNIQUE: u16 = 6;

/// The static verified rule table. The Lean build regenerates the matching C
/// header from the same theorem list. Order is stable for the header mirror.
pub static VERIFIED_RULES: &[VerifiedRule] = &[
    VerifiedRule {
        id: RULE_RESTRICTION_PUSHDOWN,
        name: "restriction_pushdown",
        lean_theorem: "restriction_pushdown_invariant",
        invariant: true,
    },
    VerifiedRule {
        id: RULE_COLIMIT_ASSOC,
        name: "colimit_assoc",
        lean_theorem: "colimit_assoc",
        invariant: true,
    },
    VerifiedRule {
        id: RULE_RESTRICTION_COMP,
        name: "restriction_comp",
        lean_theorem: "restriction_comp",
        invariant: true,
    },
    VerifiedRule {
        id: RULE_FILTER_RESTRICT_COMMUTE,
        name: "filter_restrict_commute",
        lean_theorem: "filter_restrict_commute",
        invariant: true,
    },
    VerifiedRule {
        id: RULE_IDENTITY_RESTRICTION,
        name: "identity_restriction",
        lean_theorem: "identity_restriction_elim",
        invariant: true,
    },
    VerifiedRule {
        id: RULE_GLUING_UNIQUE,
        name: "gluing_unique",
        lean_theorem: "sheaf_gluing_unique",
        invariant: true,
    },
];

/// Look up a rule by id. Returns None for an unknown id.
pub fn lookup(id: u16) -> Option<&'static VerifiedRule> {
    VERIFIED_RULES.iter().find(|r| r.id == id)
}

/// True when `id` names a proof-backed, semantically invariant rule. The
/// planner calls this instead of a runtime semantic check.
pub fn is_verified(id: u16) -> bool {
    lookup(id).map(|r| r.invariant).unwrap_or(false)
}

/// Number of rules in the verified table.
pub fn rule_count() -> usize {
    VERIFIED_RULES.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn table_is_well_formed() {
        assert!(rule_count() >= 6);
        let mut ids = HashSet::new();
        for r in VERIFIED_RULES {
            assert!(r.invariant, "every emitted rule must be proven invariant");
            assert!(!r.name.is_empty());
            assert!(!r.lean_theorem.is_empty());
            assert!(ids.insert(r.id), "rule ids must be unique");
        }
    }

    #[test]
    fn known_rules_are_verified() {
        assert!(is_verified(RULE_RESTRICTION_PUSHDOWN));
        assert!(is_verified(RULE_GLUING_UNIQUE));
        assert_eq!(lookup(RULE_RESTRICTION_COMP).unwrap().lean_theorem, "restriction_comp");
    }

    #[test]
    fn unknown_rule_is_not_verified() {
        assert!(!is_verified(9999));
        assert!(lookup(9999).is_none());
    }
}