import Mathlib

/-!
# Verified TIR rewrite rules (spec Pillar 6)

Each theorem here proves a query-plan rewrite semantically invariant. The Lean
build emits the rule validity table (`native/proofs-generated/
sheaf_verified_rules.h`) from these theorem names. The Rust planner in
`crates/sheaf-proofs` applies a rewrite only when its theorem appears in that
table, so no runtime validation check is needed.

The scalar model matches `crates/sheaf-proofs/src/planner.rs`: a restriction
scales a stalk value, a colimit glues by addition, and a discrete filter acts
as a linear mask. All maps are linear, so the rewrites are algebraic identities.
-/

namespace Sheaf.Optimizer

/-- A restriction morphism scales a stalk value by a weight. -/
def restrict (w x : ℝ) : ℝ := w * x

/-- A colimit glues two stalk values by a linear sum. -/
def colimit (a b : ℝ) : ℝ := a + b

/-- A discrete filter acts as a linear mask on a stalk value. -/
def filterMask (m x : ℝ) : ℝ := m * x

/-- Restriction pushdown: a restriction distributes over a colimit.
    `w · (a + b) = w·a + w·b`. This lets the planner push a restriction below
    a colimit evaluation (`RULE_RESTRICTION_PUSHDOWN`). -/
theorem restriction_pushdown_invariant (w a b : ℝ) :
    restrict w (colimit a b) = colimit (restrict w a) (restrict w b) := by
  simp [restrict, colimit]
  ring

/-- Colimit associativity: the gluing sum reassociates.
    `(a + b) + c = a + (b + c)` (`RULE_COLIMIT_ASSOC`). -/
theorem colimit_assoc (a b c : ℝ) :
    colimit (colimit a b) c = colimit a (colimit b c) := by
  simp [colimit]
  ring

/-- Identity restriction elimination: restricting by weight one is the
    identity, so the planner drops it (`RULE_IDENTITY_RESTRICTION`). -/
theorem identity_restriction_elim (x : ℝ) : restrict 1 x = x := by
  simp [restrict]

/-- A linear filter mask commutes with a restriction.
    `m · (w · x) = w · (m · x)` (`RULE_FILTER_RESTRICT_COMMUTE`). A threshold
    filter needs a positivity side condition and is handled separately. -/
theorem filter_restrict_commute (w m x : ℝ) :
    filterMask m (restrict w x) = restrict w (filterMask m x) := by
  simp [filterMask, restrict]
  ring

end Sheaf.Optimizer