// rewrite_equivalence.rs - proof-gated rewrite equivalence (NF-03).
//
// The test generates 200 expressions, applies every emitted rule where the
// shape matches, and checks that the scalar value is unchanged on five inputs.
// It also checks that an unverified rule is rejected before any shape test.

use sheaf_proofs::oracle::{GeneratedRules, RuleOracle};
use sheaf_query::ir::rewrite::{rewrite, rule_id, Expr, RewriteError};

struct RejectFirst;
impl RuleOracle for RejectFirst {
    fn is_verified(&self, rule: u16) -> bool {
        rule != rule_id::RESTRICTION_PUSHDOWN
    }
    fn digest(&self) -> u64 {
        0
    }
    fn source(&self) -> &'static str {
        "test-reject-first"
    }
    fn fallback_reason(&self) -> Option<&'static str> {
        Some("the test oracle rejects rule 1")
    }
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn weight(&mut self) -> f64 {
        ((self.next() % 9) as f64 + 1.0) / 4.0
    }
}

fn generate(rng: &mut Rng, depth: usize) -> Expr {
    if depth == 0 {
        return if rng.next().is_multiple_of(2) {
            Expr::Input
        } else {
            Expr::Value(rng.weight())
        };
    }
    match rng.next() % 4 {
        0 => Expr::Restrict(Box::new(generate(rng, depth - 1)), rng.weight()),
        1 => Expr::Colimit(
            Box::new(generate(rng, depth - 1)),
            Box::new(generate(rng, depth - 1)),
        ),
        2 => Expr::Filter(Box::new(generate(rng, depth - 1)), rng.weight()),
        _ => generate(rng, depth - 1),
    }
}

#[test]
fn two_hundred_generated_plans_keep_their_value_under_every_rule() {
    let oracle = GeneratedRules::load().expect("oracle");
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let rules = [
        rule_id::RESTRICTION_PUSHDOWN,
        rule_id::COLIMIT_ASSOC,
        rule_id::RESTRICTION_COMP,
        rule_id::FILTER_RESTRICT_COMMUTE,
        rule_id::IDENTITY_RESTRICTION,
    ];
    let inputs = [-3.0, 0.0, 0.5, 2.0, 11.25];
    let mut applied = 0usize;
    for _ in 0..400 {
        let before = generate(&mut rng, 4);
        for rule in rules {
            if let Ok(after) = rewrite(&before, rule, &oracle) {
                applied += 1;
                for x in inputs {
                    let left = before.eval(x);
                    let right = after.eval(x);
                    assert!(
                        (left - right).abs() <= 1e-9,
                        "rule {rule} changed {left} to {right} on input {x}"
                    );
                }
            }
        }
    }
    assert!(applied > 100, "the corpus must actually exercise the rules: {applied}");
}

#[test]
fn an_unverified_rule_is_rejected_before_a_shape_test() {
    let oracle = RejectFirst;
    let expr = Expr::Restrict(
        Box::new(Expr::Colimit(Box::new(Expr::Input), Box::new(Expr::Value(1.0)))),
        2.0,
    );
    assert_eq!(
        rewrite(&expr, rule_id::RESTRICTION_PUSHDOWN, &oracle),
        Err(RewriteError::Unverified(rule_id::RESTRICTION_PUSHDOWN))
    );
    assert_eq!(
        rewrite(&expr, 9999, &GeneratedRules::load().expect("oracle")),
        Err(RewriteError::Unverified(9999))
    );
}
