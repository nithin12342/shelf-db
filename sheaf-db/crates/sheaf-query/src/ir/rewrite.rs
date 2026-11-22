// rewrite.rs - proof-gated rewrites on a linear stalk expression (NF-03).
//
// The expression is the scalar model used by proofs/Optimizer.lean. A rewrite
// runs only when the rule oracle marks its id verified. An unverified rule is
// an explicit error; the optimizer never guesses that a rewrite is safe.

use sheaf_proofs::oracle::RuleOracle;

/// A scalar stalk expression with one input stalk value `x`.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    /// The input stalk value.
    Input,
    /// A constant stalk value.
    Value(f64),
    /// A restriction scales the stalk.
    Restrict(Box<Expr>, f64),
    /// A colimit glues two stalks by addition.
    Colimit(Box<Expr>, Box<Expr>),
    /// A discrete filter acts as a linear mask.
    Filter(Box<Expr>, f64),
}

impl Expr {
    /// Evaluate the expression on one input value.
    pub fn eval(&self, x: f64) -> f64 {
        match self {
            Expr::Input => x,
            Expr::Value(value) => *value,
            Expr::Restrict(inner, weight) => weight * inner.eval(x),
            Expr::Colimit(left, right) => left.eval(x) + right.eval(x),
            Expr::Filter(inner, mask) => mask * inner.eval(x),
        }
    }

    /// Count the nodes. A rewrite must not grow the plan without a reason.
    pub fn size(&self) -> usize {
        match self {
            Expr::Input | Expr::Value(_) => 1,
            Expr::Restrict(inner, _) | Expr::Filter(inner, _) => 1 + inner.size(),
            Expr::Colimit(left, right) => 1 + left.size() + right.size(),
        }
    }
}

/// Why a rewrite was not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewriteError {
    /// The rule id has no proof in the emitted table.
    Unverified(u16),
    /// The expression does not match the rule's left-hand side.
    NoMatch,
}

/// The ids from the emitted rule table.
pub mod rule_id {
    pub const RESTRICTION_PUSHDOWN: u16 = 1;
    pub const COLIMIT_ASSOC: u16 = 2;
    pub const RESTRICTION_COMP: u16 = 3;
    pub const FILTER_RESTRICT_COMMUTE: u16 = 4;
    pub const IDENTITY_RESTRICTION: u16 = 5;
}

/// Apply one proof-gated rewrite. The oracle is checked before the shape.
pub fn rewrite(expr: &Expr, rule: u16, oracle: &impl RuleOracle) -> Result<Expr, RewriteError> {
    if !oracle.is_verified(rule) {
        return Err(RewriteError::Unverified(rule));
    }
    match rule {
        rule_id::RESTRICTION_PUSHDOWN => match expr {
            Expr::Restrict(inner, weight) => match inner.as_ref() {
                Expr::Colimit(left, right) => Ok(Expr::Colimit(
                    Box::new(Expr::Restrict(left.clone(), *weight)),
                    Box::new(Expr::Restrict(right.clone(), *weight)),
                )),
                _ => Err(RewriteError::NoMatch),
            },
            _ => Err(RewriteError::NoMatch),
        },
        rule_id::COLIMIT_ASSOC => match expr {
            Expr::Colimit(outer, right) => match outer.as_ref() {
                Expr::Colimit(left, middle) => Ok(Expr::Colimit(
                    left.clone(),
                    Box::new(Expr::Colimit(middle.clone(), right.clone())),
                )),
                _ => Err(RewriteError::NoMatch),
            },
            _ => Err(RewriteError::NoMatch),
        },
        rule_id::RESTRICTION_COMP => match expr {
            Expr::Restrict(inner, outer_weight) => match inner.as_ref() {
                Expr::Restrict(deep, inner_weight) => Ok(Expr::Restrict(
                    deep.clone(),
                    outer_weight * inner_weight,
                )),
                _ => Err(RewriteError::NoMatch),
            },
            _ => Err(RewriteError::NoMatch),
        },
        rule_id::FILTER_RESTRICT_COMMUTE => match expr {
            Expr::Filter(inner, mask) => match inner.as_ref() {
                Expr::Restrict(deep, weight) => Ok(Expr::Restrict(
                    Box::new(Expr::Filter(deep.clone(), *mask)),
                    *weight,
                )),
                _ => Err(RewriteError::NoMatch),
            },
            _ => Err(RewriteError::NoMatch),
        },
        rule_id::IDENTITY_RESTRICTION => match expr {
            Expr::Restrict(inner, weight) if *weight == 1.0 => Ok(inner.as_ref().clone()),
            _ => Err(RewriteError::NoMatch),
        },
        _ => Err(RewriteError::Unverified(rule)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sheaf_proofs::oracle::GeneratedRules;

    #[test]
    fn a_rewrite_of_a_matching_shape_preserves_the_value() {
        let oracle = GeneratedRules::load().expect("oracle");
        let expr = Expr::Restrict(
            Box::new(Expr::Colimit(Box::new(Expr::Input), Box::new(Expr::Value(2.0)))),
            3.0,
        );
        let after = rewrite(&expr, rule_id::RESTRICTION_PUSHDOWN, &oracle).expect("rewrite");
        for x in [-2.5, 0.0, 1.0, 7.25] {
            assert_eq!(after.eval(x), expr.eval(x));
        }
    }

    #[test]
    fn a_nonmatching_shape_is_rejected() {
        let oracle = GeneratedRules::load().expect("oracle");
        let expr = Expr::Input;
        assert_eq!(
            rewrite(&expr, rule_id::RESTRICTION_PUSHDOWN, &oracle),
            Err(RewriteError::NoMatch)
        );
    }
}
