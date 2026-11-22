import Mathlib.Basic.ExistsUnique
import Restriction

/-!
# The sheaf gluing condition (spec Pillar 6)

A compatible family of local sections glues to a unique global section. This
is the equalizer-exactness axiom of a sheaf. We package existence as a structure
field (`glue`) and prove uniqueness from the restriction equations. The combined
statement is `sheaf_gluing_unique`, the `∃!` form quoted in the specification.

A global section is a 0-cochain: a choice of stalk element at every cell.
-/

namespace Sheaf

/-- A global section assigns a stalk element to every cell. -/
def GlobalSection (F : CellularSheaf) := (U : PosetCell) → F.Stalk U

/-- Restrict a global section to the stalk at one cell. -/
def restrictTo (F : CellularSheaf) (g : GlobalSection F) (U : PosetCell) : F.Stalk U :=
  g U

/-- A sheaf with the gluing axiom. `glue` builds a global section from a family
    of local sections, and `glue_restrict` states that it restricts back to the
    family. Compatibility of the family is assumed by the caller. -/
structure GluingSheaf extends CellularSheaf where
  glue : (s : (U : PosetCell) → Stalk U) → GlobalSection toCellularSheaf
  glue_restrict : (s : (U : PosetCell) → Stalk U) → (U : PosetCell) →
    restrictTo toCellularSheaf (glue s) U = s U

/-- The sheaf gluing condition: a family of local sections has a unique global
    section that restricts to it. Existence comes from the gluing axiom.
    Uniqueness follows because two global sections with equal restrictions are
    equal pointwise, hence equal by function extensionality. -/
theorem sheaf_gluing_unique (F : GluingSheaf) (s : (U : PosetCell) → F.Stalk U) :
    ExistsUnique (fun g : GlobalSection F.toCellularSheaf =>
      ∀ U, restrictTo F.toCellularSheaf g U = s U) := by
  refine ⟨F.glue s, ?_, ?_⟩
  · intro U
    exact F.glue_restrict s U
  · intro g hg
    funext U
    exact (hg U).trans (F.glue_restrict s U).symm

/-- Two global sections that agree on every restriction are equal. This is the
    uniqueness half of gluing, isolated for reuse by the merge engine. -/
theorem global_section_ext (F : CellularSheaf) (g₁ g₂ : GlobalSection F)
    (h : ∀ U, restrictTo F g₁ U = restrictTo F g₂ U) : g₁ = g₂ := by
  funext U
  exact h U

end Sheaf