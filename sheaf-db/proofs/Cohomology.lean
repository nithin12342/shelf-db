import Mathlib.Basic.Real.Basic
import Mathlib.Tactic.Ring

/-!
# Cohomological merge obstruction (spec Pillars 6 and 8)

On a 1-dimensional cell complex there are no 2-cells, so `δ¹ = 0` and
`ker δ¹ = C¹`. The first cohomology is therefore `H¹ = C¹ / im δ⁰`. A merge
conflict is a 1-cochain that lies outside `im δ⁰`: a nonzero coboundary that
no global section can realize. This file defines the coboundary on an edge and
proves the conflict characterization the merge engine uses.

The scalar model below mirrors `crates/sheaf-vcs/src/diff.rs`. A 0-cochain is a
real value per vertex. The coboundary on edge `(u, v)` is `x_v - x_u`.
-/

namespace Sheaf.Cohomology

/-- A 0-cochain assigns a real value to each vertex. -/
def Cochain0 (V : Type) := V → ℝ

/-- A 1-cell (edge) with two endpoints. -/
structure Edge (V : Type) where
  u : V
  v : V

/-- The coboundary of a 0-cochain on one edge: `(δx)_e = x_v - x_u`. -/
def coboundaryEdge {V : Type} (x : Cochain0 V) (e : Edge V) : ℝ :=
  x e.v - x e.u

/-- A merge conflict on an edge is a nonzero coboundary. -/
def HasConflict {V : Type} (x : Cochain0 V) (e : Edge V) : Prop :=
  coboundaryEdge x e ≠ 0

/-- The conflict characterization. On a graph the cohomological obstruction
    `H¹ ≠ 0` reduces to a nonzero coboundary, because `ker δ¹ = C¹`. -/
theorem merge_conflict_iff_coboundary_nonzero {V : Type} (x : Cochain0 V) (e : Edge V) :
    HasConflict x e ↔ coboundaryEdge x e ≠ 0 :=
  Iff.rfl

/-- A global section (zero coboundary on every edge) has no conflict. This is
    the `δs = 0` consensus rule: compatible branches compose without conflict. -/
theorem global_section_no_conflict {V : Type} (x : Cochain0 V)
    (h : ∀ e : Edge V, coboundaryEdge x e = 0) : ∀ e, ¬ HasConflict x e := by
  intro e he
  exact he (h e)

/-- The algebraic time-travel diff `δ(s₂ − s₁)` on one edge. -/
def coboundaryDiff {V : Type} (s₁ s₂ : Cochain0 V) (e : Edge V) : ℝ :=
  coboundaryEdge (fun v => s₂ v - s₁ v) e

/-- The diff is linear: `δ(s₂ − s₁) = δs₂ − δs₁`. This is the identity the
    Rust diff module relies on to isolate changed cells. -/
theorem coboundaryDiff_linear {V : Type} (s₁ s₂ : Cochain0 V) (e : Edge V) :
    coboundaryDiff s₁ s₂ e = coboundaryEdge s₂ e - coboundaryEdge s₁ e := by
  simp [coboundaryDiff, coboundaryEdge]
  ring

end Sheaf.Cohomology