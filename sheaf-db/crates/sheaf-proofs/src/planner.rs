// planner.rs - the proof-gated TIR rewrite planner (spec Pillar 6).
//
// The planner applies a rewrite only when the verified rule table proves it
// semantically invariant. There is no runtime semantic validation check: the
// proof in the Lean tier is the validation. This module demonstrates the gate
// on a small linear plan model, where a restriction (scalar multiply)
// distributes over a colimit (a linear sum).
//
// The model is intentionally tiny. The real sheaf-query optimizer consumes the
// same is_verified gate before it applies a rewrite to a full TIR plan.

use crate::rules::{is_verified, RULE_IDENTITY_RESTRICTION, RULE_RESTRICTION_PUSHDOWN};

/// One stage of a linear plan over a float cochain.
#[derive(Clone, Debug, PartialEq)]
pub enum Stage {
    /// Load the source cochain.
    Scan,
    /// A restriction morphism: scale every lane by a weight.
    Restrict(f32),
    /// A colimit: glue the lanes by a linear sum.
    Colimit,
    /// Emit the result.
    Yield,
}

/// A plan is an ordered list of stages.
pub type Plan = Vec<Stage>;

/// Evaluate a plan on an input cochain and return the scalar result. Every
/// stage is linear, so restriction and colimit commute (distributivity).
pub fn eval(plan: &[Stage], input: &[f32]) -> f32 {
    let mut lanes: Vec<f32> = input.to_vec();
    for stage in plan {
        match stage {
            Stage::Scan => lanes = input.to_vec(),
            Stage::Restrict(w) => {
                for l in lanes.iter_mut() {
                    *l *= w;
                }
            }
            Stage::Colimit => {
                let s: f32 = lanes.iter().sum();
                lanes = vec![s];
            }
            Stage::Yield => {}
        }
    }
    lanes.iter().sum()
}

/// Move every restriction below a preceding colimit. This is the restriction
/// pushdown rewrite. It runs only because the rule is proof-backed.
pub fn pushdown_restrictions(plan: &[Stage]) -> Plan {
    let mut out = plan.to_vec();
    let mut changed = true;
    while changed {
        changed = false;
        for i in 1..out.len() {
            if matches!(out[i], Stage::Restrict(_)) && matches!(out[i - 1], Stage::Colimit) {
                out.swap(i - 1, i);
                changed = true;
            }
        }
    }
    out
}

/// Remove identity restrictions (weight 1.0). The identity morphism acts as
/// the identity on the stalk, so dropping it preserves semantics.
pub fn eliminate_identity_restrictions(plan: &[Stage]) -> Plan {
    plan.iter()
        .filter(|s| !matches!(s, Stage::Restrict(w) if *w == 1.0))
        .cloned()
        .collect()
}

/// Apply a rewrite by rule id, gated by the verified table. An unknown or
/// unverified id returns the plan unchanged. This is the single entry point
/// the query optimizer calls.
pub fn apply_rule(plan: &[Stage], rule_id: u16) -> Plan {
    if !is_verified(rule_id) {
        return plan.to_vec();
    }
    match rule_id {
        RULE_RESTRICTION_PUSHDOWN => pushdown_restrictions(plan),
        RULE_IDENTITY_RESTRICTION => eliminate_identity_restrictions(plan),
        _ => plan.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::RULE_COLIMIT_ASSOC;

    fn sample_plan() -> Plan {
        vec![Stage::Scan, Stage::Colimit, Stage::Restrict(2.0), Stage::Yield]
    }

    #[test]
    fn pushdown_preserves_semantics() {
        let plan = sample_plan();
        let rewritten = apply_rule(&plan, RULE_RESTRICTION_PUSHDOWN);
        for input in [vec![1.0, 2.0, 3.0], vec![0.0, 0.0], vec![-4.0, 5.0]] {
            assert_eq!(eval(&plan, &input), eval(&rewritten, &input));
        }
        // Both evaluate to 2 * (1 + 2 + 3) = 12.
        assert_eq!(eval(&plan, &[1.0, 2.0, 3.0]), 12.0);
    }

    #[test]
    fn pushdown_moves_restrict_before_colimit() {
        let plan = sample_plan();
        let rewritten = apply_rule(&plan, RULE_RESTRICTION_PUSHDOWN);
        assert_eq!(
            rewritten,
            vec![Stage::Scan, Stage::Restrict(2.0), Stage::Colimit, Stage::Yield]
        );
    }

    #[test]
    fn identity_restriction_is_eliminated() {
        let plan = vec![Stage::Scan, Stage::Restrict(1.0), Stage::Restrict(3.0), Stage::Yield];
        let rewritten = apply_rule(&plan, RULE_IDENTITY_RESTRICTION);
        assert_eq!(rewritten, vec![Stage::Scan, Stage::Restrict(3.0), Stage::Yield]);
        assert_eq!(eval(&plan, &[2.0, 2.0]), eval(&rewritten, &[2.0, 2.0]));
    }

    #[test]
    fn unverified_rule_leaves_plan_unchanged() {
        let plan = sample_plan();
        assert_eq!(apply_rule(&plan, 9999), plan);
    }

    #[test]
    fn verified_but_unimplemented_rule_is_a_noop() {
        // colimit_assoc is verified but has no rewrite body here yet.
        let plan = sample_plan();
        assert_eq!(apply_rule(&plan, RULE_COLIMIT_ASSOC), plan);
    }

    #[test]
    fn pushdown_is_idempotent() {
        let plan = sample_plan();
        let once = apply_rule(&plan, RULE_RESTRICTION_PUSHDOWN);
        let twice = apply_rule(&once, RULE_RESTRICTION_PUSHDOWN);
        assert_eq!(once, twice);
    }
}