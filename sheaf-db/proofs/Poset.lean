import Mathlib

/-!
# Poset cells and the face order (spec Pillar 6)

A cellular sheaf lives over a graded poset complex. Each cell carries a
dimension tag and an index. The face order `U ≤ V` indexes the restriction
morphisms. This file defines the order and proves it is a preorder, which the
functoriality theorem in `Restriction.lean` consumes.
-/

namespace Sheaf

/-- A cell in a graded poset complex, tagged by dimension and sequence index. -/
structure PosetCell where
  dim : Nat
  index : Nat
  deriving DecidableEq, Repr

/-- The face order: `U ≤ V` when the dimension of `U` is at most that of `V`.
    Restriction morphisms run along this order. -/
instance : LE PosetCell where
  le U V := U.dim ≤ V.dim

/-- Reflexivity of the face order. -/
theorem le_refl (U : PosetCell) : U ≤ U :=
  Nat.le_refl U.dim

/-- Transitivity of the face order. The composite restriction along
    `U ≤ V ≤ W` is indexed by this proof. -/
theorem le_trans {U V W : PosetCell} (h1 : U ≤ V) (h2 : V ≤ W) : U ≤ W :=
  Nat.le_trans h1 h2

/-- Antisymmetry on the dimension tag. Two cells with the same dimension and
    index are equal. -/
theorem ext {U V : PosetCell} (hd : U.dim = V.dim) (hi : U.index = V.index) :
    U = V := by
  cases U; cases V; simp_all

end Sheaf