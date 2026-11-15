import Sheaf.Poset

/-!
# Restriction morphisms and functoriality (spec Pillar 6)

A cellular sheaf assigns a stalk to each cell and a restriction map to each
order proof. The functoriality axiom states that restricting along a composite
order equals composing the two restrictions. We package the axiom as a
structure field and derive the function-level theorem `restriction_comp`.

Composition order: for `h1 : U ≤ V` and `h2 : V ≤ W`, the map `U → W` is
`restriction h2 ∘ restriction h1`. This is the standard covariant direction.
-/

namespace Sheaf

/-- A cellular sheaf over the poset cell complex. `Stalk` assigns a type to
    each cell. `restriction` maps a stalk along an order proof. `functorial`
    is the sheaf axiom that restriction respects composition. -/
structure CellularSheaf where
  Stalk : PosetCell → Type
  restriction : {U V : PosetCell} → (U ≤ V) → Stalk U → Stalk V
  functorial : {U V W : PosetCell} → (h1 : U ≤ V) → (h2 : V ≤ W) →
    (x : Stalk U) → restriction h2 (restriction h1 x) = restriction (le_trans h1 h2) x

/-- Functoriality of restriction morphisms, at the level of functions.
    The composite restriction equals the restriction along the transitive
    order proof. This is the rule the optimizer uses to fuse two adjacent
    restrictions into one (`RULE_RESTRICTION_COMP`). -/
theorem restriction_comp (F : CellularSheaf) {U V W : PosetCell}
    (h1 : U ≤ V) (h2 : V ≤ W) :
    (F.restriction h2) ∘ (F.restriction h1) = F.restriction (le_trans h1 h2) := by
  funext x
  exact F.functorial h1 h2 x

/-- Restriction along a reflexive order is the identity on the stalk.
    This follows from functoriality together with the identity axiom below. -/
structure IdentitySheaf extends CellularSheaf where
  identity : (U : PosetCell) → (x : Stalk U) → restriction (le_refl U) x = x

/-- The identity restriction acts as the identity function. The optimizer
    eliminates it (`RULE_IDENTITY_RESTRICTION`). -/
theorem identity_restriction_elim (F : IdentitySheaf) (U : PosetCell) :
    F.restriction (le_refl U) = id := by
  funext x
  exact F.identity U x

end Sheaf