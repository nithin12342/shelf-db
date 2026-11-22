-- Timestamp: 2026-11-22_11-53-08 +05:30
import Mathlib.Basic.Real.Basic
import Mathlib.Tactic.Linarith
import Mathlib.Tactic.Ring
import Mathlib.Tactic.FieldSimp

/-!
# Spectral interval bounds for Chebyshev iteration (NF-02)

The kernel estimates the shifted spectrum in the interval [Î¼, 1.05Â·L + Î¼].
This file proves the algebraic invariants of that interval: its center and
radius reconstruct the endpoints, the radius is positive, the shifted centre
ratio is above one, and the Chebyshev polynomials grow monotonically there.
The Rust test checks the same statements on 100 deterministic inputs.
-/

namespace Sheaf.Spectral

noncomputable section

/-- The lower endpoint: the regularization shift. -/
def lower (mu L : â„) : â„ := mu

/-- The upper endpoint: the power-iteration estimate with a 5% safety margin. -/
def upper (mu L : â„) : â„ := L * (105 / 100) + mu

/-- The midpoint of the interval. -/
def center (mu L : â„) : â„ := (upper mu L + lower mu L) / 2

/-- The half width of the interval. -/
def radius (mu L : â„) : â„ := (upper mu L - lower mu L) / 2

theorem center_minus_radius (mu L : â„) : center mu L - radius mu L = lower mu L := by
  unfold center radius upper lower
  ring

theorem center_plus_radius (mu L : â„) : center mu L + radius mu L = upper mu L := by
  unfold center radius upper lower
  ring

theorem radius_positive (mu L : â„) (hmu : 0 < mu) (hL : 0 < L) : 0 < radius mu L := by
  unfold radius upper lower
  linarith

theorem center_inside (mu L : â„) (hmu : 0 < mu) (hL : 0 < L) :
    lower mu L < center mu L âˆ§ center mu L < upper mu L := by
  constructor
  Â· have hr := radius_positive mu L hmu hL
    have hc := center_minus_radius mu L
    linarith
  Â· have hr := radius_positive mu L hmu hL
    have hc := center_plus_radius mu L
    linarith

theorem sigma_gt_one (mu L : â„) (hmu : 0 < mu) (hL : 0 < L) :
    1 < center mu L / radius mu L := by
  have hr : 0 < radius mu L := radius_positive mu L hmu hL
  have hc : center mu L = radius mu L + mu := by
    have h := center_minus_radius mu L
    unfold lower at h
    linarith
  rw [hc]
  have hdiv : 0 < mu / radius mu L := div_pos hmu hr
  have hsplit : (radius mu L + mu) / radius mu L = 1 + mu / radius mu L := by
    field_simp [ne_of_gt hr]
  rw [hsplit]
  linarith

/-- The Chebyshev polynomial of the first kind, by its three-term recurrence. -/
def chebT : Nat â†’ â„ â†’ â„
  | 0, _ => 1
  | 1, x => x
  | n + 2, x => 2 * x * chebT (n + 1) x - chebT n x

theorem chebT_pos_monotone (x : â„) (hx : 1 < x) :
    âˆ€ n : Nat, 0 < chebT n x âˆ§ chebT n x < chebT (n + 1) x := by
  intro n
  induction n with
  | zero =>
      constructor
      Â· simp [chebT]
      Â· simpa [chebT] using hx
  | succ n ih =>
      constructor
      Â· exact lt_trans ih.1 ih.2
      Â· have hy : 0 < chebT (n + 1) x := lt_trans ih.1 ih.2
        have hprod : 0 < 2 * x * chebT (n + 1) x - chebT n x - chebT (n + 1) x := by
          rw [show 2 * x * chebT (n + 1) x - chebT n x - chebT (n + 1) x =
              2 * chebT (n + 1) x * (x - 1) + (chebT (n + 1) x - chebT n x) by ring]
          exact add_pos
            (mul_pos (mul_pos (by norm_num) hy) (sub_pos.mpr hx))
            (sub_pos.mpr ih.2)
        simpa [chebT, Nat.succ_eq_add_one] using hprod

end

end Sheaf.Spectral
