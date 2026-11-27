import Mathlib.Analysis.SpecialFunctions.Pow.Real
import Mathlib.Tactic
import PFSH02.Params

/-!
# Lemma 1: Greedy Peeling Analysis `Peel(M)` & Exact 4-Line Telescoping

Formalizes Lemma 1 of `docs/security/peel-binius-shielded-pq-payments.md` (lines 1279–1341):
1. Exact per-step square-root decrease `√x_i - √x_{i+1} > √(D/2)` for every peeling iteration.
2. Exact telescoping bound `k * D ≤ √(2D) * (√n_0 - √n')` (strict for `k ≥ 1`) for ALL `n' ≥ 0`,
   yielding `k ≤ ⌊√(2 n_0 / D)⌋ ≤ 86 = L_max` at PFSH02 parameters.
3. Complete Case 1 (`0 ≤ n' ≤ 2D`, explicitly including `n' = 0`): `|A| < √(2 D n)`, hence
   `Pr[Bad_peel] = 0`.
4. Complete Case 2 (`2D < n' ≤ n`): verifies all verbatim hypotheses of BCIKS20 Theorem 6.2
   (`m' = 3`, `ρ' < 1/2`, `0 < δ' < 1 - (7/6) √ρ'`) and proves curve-bound monotonicity
   `(n')^2 / (ρ')^(3/2) ≤ n^2 / ρ^(3/2)`.
-/

namespace PFSH02.Peel

/-- A valid `k`-step peeling sequence of remaining domain sizes `x 0, x 1, ..., x k`
    with Reed–Solomon degree bound `D > 0`. -/
structure PeelSeq (D : ℝ) (k : ℕ) where
  x : Fin (k + 1) → ℝ
  hD : 0 < D
  hx_nonneg : ∀ i, 0 ≤ x i
  /-- Every iteration begins with `|L_i| > 2D`. -/
  hx_gt_2D : ∀ (i : Fin k), 2 * D < x i.castSucc
  /-- Every iteration peels a slice of size `x_i - x_{i+1} ≥ √(2 D x_i)`. -/
  hstep : ∀ (i : Fin k), Real.sqrt (2 * D * x i.castSucc) ≤ x i.castSucc - x i.succ

/-- Single-step square-root telescoping inequality (Spec Lemma 1(i)):
    `√x_i - √x_{i+1} > √(D / 2)`. -/
theorem step_sqrt_decrease {D a b : ℝ} (hD : 0 < D) (ha : 2 * D < a) (hb : 0 ≤ b)
    (hstep : Real.sqrt (2 * D * a) ≤ a - b) :
    Real.sqrt (D / 2) < Real.sqrt a - Real.sqrt b := by
  have ha_pos : 0 < a := by linarith
  have h2Da_pos : 0 < 2 * D * a := by positivity
  have hdiff_pos : 0 < a - b := lt_of_lt_of_le (Real.sqrt_pos.mpr h2Da_pos) hstep
  have hb_lt_a : b < a := by linarith
  have hsqa_pos : 0 < Real.sqrt a := Real.sqrt_pos.mpr ha_pos
  have hsqb_lt : Real.sqrt b < Real.sqrt a := Real.sqrt_lt_sqrt hb hb_lt_a
  have hsum_pos : 0 < Real.sqrt a + Real.sqrt b := by linarith [Real.sqrt_nonneg b]
  have hsum_lt : Real.sqrt a + Real.sqrt b < 2 * Real.sqrt a := by linarith
  have hprod : (Real.sqrt a - Real.sqrt b) * (Real.sqrt a + Real.sqrt b) = a - b := by
    nlinarith [Real.mul_self_sqrt ha_pos.le, Real.mul_self_sqrt hb]
  have hrhs_eq : Real.sqrt (D / 2) * (2 * Real.sqrt a) = Real.sqrt (2 * D * a) := by
    have h1 : Real.sqrt (D / 2) * 2 = Real.sqrt (2 * D) := by
      have : (2 : ℝ) = Real.sqrt 4 := by norm_num
      rw [this, ← Real.sqrt_mul (by positivity)]
      congr 1; ring
    calc
      Real.sqrt (D / 2) * (2 * Real.sqrt a)
          = (Real.sqrt (D / 2) * 2) * Real.sqrt a := by ring
      _ = Real.sqrt (2 * D) * Real.sqrt a := by rw [h1]
      _ = Real.sqrt (2 * D * a) := (Real.sqrt_mul (by positivity) a).symm
  have hlt1 : Real.sqrt (D / 2) * (Real.sqrt a + Real.sqrt b) < Real.sqrt (D / 2) * (2 * Real.sqrt a) :=
    mul_lt_mul_of_pos_left hsum_lt (Real.sqrt_pos.mpr (by positivity))
  have hlt2 : Real.sqrt (D / 2) * (Real.sqrt a + Real.sqrt b)
      < (Real.sqrt a - Real.sqrt b) * (Real.sqrt a + Real.sqrt b) := by
    linarith [hrhs_eq, hstep, hprod]
  exact lt_of_mul_lt_mul_right hlt2 hsum_pos.le

/-- Telescoping bound across all `k` peeling steps:
    `k * √(D / 2) ≤ √(x 0) - √(x k)`, with strict `<` whenever `k ≥ 1`. -/
theorem peel_sqrt_telescoping {D : ℝ} {k : ℕ} (P : PeelSeq D k) :
    (k : ℝ) * Real.sqrt (D / 2) ≤ Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩) := by
  induction k with
  | zero => simp
  | succ m ih =>
    let P' : PeelSeq D m :=
      { x := fun i => P.x i.castSucc
        hD := P.hD
        hx_nonneg := fun i => P.hx_nonneg i.castSucc
        hx_gt_2D := fun i => P.hx_gt_2D i.castSucc
        hstep := fun i => P.hstep i.castSucc }
    have hprev := ih P'
    dsimp [P'] at hprev
    have hlast := step_sqrt_decrease P.hD (P.hx_gt_2D ⟨m, Nat.lt_succ_self m⟩)
      (P.hx_nonneg ⟨m + 1, Nat.lt_succ_self (m + 1)⟩)
      (P.hstep ⟨m, Nat.lt_succ_self m⟩)
    simp only [Fin.castSucc_mk] at hprev hlast
    push_cast
    linarith

/-- Strict telescoping bound whenever at least one slice was peeled (`k ≥ 1`). -/
theorem peel_sqrt_telescoping_strict {D : ℝ} {k : ℕ} (hk : 1 ≤ k) (P : PeelSeq D k) :
    (k : ℝ) * Real.sqrt (D / 2) < Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩) := by
  cases k with
  | zero => omega
  | succ m =>
    let P' : PeelSeq D m :=
      { x := fun i => P.x i.castSucc
        hD := P.hD
        hx_nonneg := fun i => P.hx_nonneg i.castSucc
        hx_gt_2D := fun i => P.hx_gt_2D i.castSucc
        hstep := fun i => P.hstep i.castSucc }
    have hprev := peel_sqrt_telescoping P'
    dsimp [P'] at hprev
    have hlast := step_sqrt_decrease P.hD (P.hx_gt_2D ⟨m, Nat.lt_succ_self m⟩)
      (P.hx_nonneg ⟨m + 1, Nat.lt_succ_self (m + 1)⟩)
      (P.hstep ⟨m, Nat.lt_succ_self m⟩)
    simp only [Fin.castSucc_mk] at hprev hlast
    push_cast
    linarith

/-- Exact peeled-slice collision budget:
    `k * D ≤ √(2D) * (√(x 0) - √(x k))` for all `k ≥ 0`, and strict `<` for `k ≥ 1`. -/
theorem peel_kD_le {D : ℝ} {k : ℕ} (P : PeelSeq D k) :
    (k : ℝ) * D ≤ Real.sqrt (2 * D) * (Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩)) := by
  have hD := P.hD
  have htel := peel_sqrt_telescoping P
  have hmul := mul_le_mul_of_nonneg_left htel (Real.sqrt_nonneg (2 * D))
  have hprod : Real.sqrt (2 * D) * Real.sqrt (D / 2) = D := by
    rw [← Real.sqrt_mul (by positivity)]
    have : 2 * D * (D / 2) = D * D := by ring
    rw [this, Real.sqrt_mul_self hD.le]
  calc
    (k : ℝ) * D = (k : ℝ) * (Real.sqrt (2 * D) * Real.sqrt (D / 2)) := by rw [hprod]
    _ = Real.sqrt (2 * D) * ((k : ℝ) * Real.sqrt (D / 2)) := by ring
    _ ≤ Real.sqrt (2 * D) * (Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩)) := hmul

theorem peel_kD_lt {D : ℝ} {k : ℕ} (hk : 1 ≤ k) (P : PeelSeq D k) :
    (k : ℝ) * D < Real.sqrt (2 * D) * (Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩)) := by
  have hD := P.hD
  have htel := peel_sqrt_telescoping_strict hk P
  have hmul := mul_lt_mul_of_pos_left htel (Real.sqrt_pos.mpr (by positivity : (0 : ℝ) < 2 * D))
  have hprod : Real.sqrt (2 * D) * Real.sqrt (D / 2) = D := by
    rw [← Real.sqrt_mul (by positivity)]
    have : 2 * D * (D / 2) = D * D := by ring
    rw [this, Real.sqrt_mul_self hD.le]
  calc
    (k : ℝ) * D = (k : ℝ) * (Real.sqrt (2 * D) * Real.sqrt (D / 2)) := by rw [hprod]
    _ = Real.sqrt (2 * D) * ((k : ℝ) * Real.sqrt (D / 2)) := by ring
    _ < Real.sqrt (2 * D) * (Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩)) := hmul

/-- Lemma 1(i) general list-size bound: `k^2 * D ≤ 2 * x 0`. -/
theorem peel_list_size_sq_bound {D : ℝ} {k : ℕ} (P : PeelSeq D k) :
    (k : ℝ) ^ 2 * D ≤ 2 * P.x 0 := by
  have hD := P.hD
  have hle := peel_kD_le P
  have hsq_nonneg : 0 ≤ Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩) := Real.sqrt_nonneg _
  have hle2 : (k : ℝ) * D ≤ Real.sqrt (2 * D) * Real.sqrt (P.x 0) := by
    have : Real.sqrt (P.x 0) - Real.sqrt (P.x ⟨k, Nat.lt_succ_self k⟩) ≤ Real.sqrt (P.x 0) := by
      linarith
    exact le_trans hle (mul_le_mul_of_nonneg_left this (Real.sqrt_nonneg _))
  have hsq : ((k : ℝ) * D) ^ 2 ≤ (Real.sqrt (2 * D) * Real.sqrt (P.x 0)) ^ 2 :=
    pow_le_pow_left₀ (by positivity) hle2 2
  have hrhs : (Real.sqrt (2 * D) * Real.sqrt (P.x 0)) ^ 2 = 2 * D * P.x 0 := by
    rw [mul_pow, Real.sq_sqrt (by positivity), Real.sq_sqrt (P.hx_nonneg 0)]
  rw [hrhs] at hsq
  have hfact : (k : ℝ) ^ 2 * D * D ≤ 2 * P.x 0 * D := by linarith
  exact le_of_mul_le_mul_right hfact hD

/-- Lemma 1(i) instantiated at PFSH02 parameters (`D = 565`, `x 0 ≤ 2^21`): `k ≤ 86 = L_max`. -/
theorem pfsh02_peel_list_bound {k : ℕ} (P : PeelSeq (Params.D : ℝ) k)
    (hx0 : P.x 0 ≤ (Params.n : ℝ)) :
    k ≤ Params.L_max := by
  have hsq := peel_list_size_sq_bound P
  have h2 : (k : ℝ) ^ 2 * (Params.D : ℝ) ≤ 2 * (Params.n : ℝ) := by linarith
  have hnat : k ^ 2 * Params.D ≤ 2 * Params.n := by
    exact_mod_cast h2
  have hlt := Params.L_max_exact.2
  by_contra hgt
  push_neg at hgt
  have hge : Params.L_max + 1 ≤ k := hgt
  have hsq_ge : (Params.L_max + 1) ^ 2 * Params.D ≤ k ^ 2 * Params.D := by
    gcongr
  omega

/-- Lemma 1(ii) Case 1 (`0 ≤ n' ≤ 2D`, explicitly including `n' = 0`):
    Any candidate polynomial outside `P(M)` agrees on at most `k * (D - 1) + n'` points,
    which is strictly less than `√(2 D n)` whenever `2D < n` (in particular for PFSH02). -/
theorem peel_case1_agreement_strict {D n_tot A_peel A_rem : ℝ} {k : ℕ}
    (P : PeelSeq D k)
    (hx0 : P.x 0 ≤ n_tot)
    (hn_gt_2D : 2 * D < n_tot)
    (hrem_le_2D : P.x ⟨k, Nat.lt_succ_self k⟩ ≤ 2 * D)
    (hA_peel : A_peel ≤ (k : ℝ) * D)
    (hA_rem : A_rem ≤ P.x ⟨k, Nat.lt_succ_self k⟩) :
    A_peel + A_rem < Real.sqrt (2 * D * n_tot) := by
  set n' := P.x ⟨k, Nat.lt_succ_self k⟩
  have hD := P.hD
  have hn'_nonneg : 0 ≤ n' := P.hx_nonneg ⟨k, Nat.lt_succ_self k⟩
  have hn'_le_sqrt : n' ≤ Real.sqrt (2 * D * n') := by
    have hsq : n' ^ 2 ≤ 2 * D * n' := by nlinarith
    calc
      n' = Real.sqrt (n' ^ 2) := (Real.sqrt_sq hn'_nonneg).symm
      _ ≤ Real.sqrt (2 * D * n') := Real.sqrt_le_sqrt hsq
  have hsqrt_mono : Real.sqrt (2 * D * P.x 0) ≤ Real.sqrt (2 * D * n_tot) :=
    Real.sqrt_le_sqrt (by nlinarith)
  cases k with
  | zero =>
    have hA_peel0 : A_peel ≤ 0 := by simpa using hA_peel
    have h2D_lt : 2 * D < Real.sqrt (2 * D * n_tot) := by
      have hsq : (2 * D) ^ 2 < 2 * D * n_tot := by nlinarith
      calc
        2 * D = Real.sqrt ((2 * D) ^ 2) := (Real.sqrt_sq (by positivity)).symm
        _ < Real.sqrt (2 * D * n_tot) := Real.sqrt_lt_sqrt (by positivity) hsq
    linarith
  | succ m =>
    have hstrict := peel_kD_lt (Nat.succ_pos m) P
    have hsplit : Real.sqrt (2 * D * n') = Real.sqrt (2 * D) * Real.sqrt n' :=
      Real.sqrt_mul (by positivity) n'
    have hsplit0 : Real.sqrt (2 * D * P.x 0) = Real.sqrt (2 * D) * Real.sqrt (P.x 0) :=
      Real.sqrt_mul (by positivity) (P.x 0)
    linarith

/-- Lemma 1(ii) Case 2 (`2D < n' ≤ n`): verifies all verbatim BCIKS20 Theorem 6.2 hypotheses
    at `m' = 3` and proves that on `¬E_rem` (`A_rem < √(2 D n')`), total agreement satisfies
    `A_peel + A_rem < √(2 D n)`. -/
theorem peel_case2_bciks_hypotheses {D n' : ℝ} (hD : 0 < D) (hn' : 2 * D < n') :
    let rho' := D / n'
    let delta' := 1 - Real.sqrt (2 * rho')
    0 < rho' ∧ rho' < 1 / 2 ∧
    0 < delta' ∧ delta' < 1 - (1 + 1 / (2 * (3 : ℝ))) * Real.sqrt rho' := by
  have hn'_pos : 0 < n' := by linarith
  have hrho_pos : 0 < D / n' := div_pos hD hn'_pos
  have hrho_lt : D / n' < 1 / 2 := by
    rw [div_lt_div_iff₀ hn'_pos (by norm_num : (0 : ℝ) < 2)]
    linarith
  have h2rho_lt : 2 * (D / n') < 1 := by linarith
  have hsqrt_lt_1 : Real.sqrt (2 * (D / n')) < 1 := by
    rw [← Real.sqrt_one]
    exact Real.sqrt_lt_sqrt (by positivity) h2rho_lt
  have h76_lt_sqrt2 : (1 + 1 / (2 * (3 : ℝ))) < Real.sqrt 2 := by
    have hsq : ((1 + 1 / (2 * (3 : ℝ))) : ℝ) ^ 2 < 2 := by norm_num
    calc
      (1 + 1 / (2 * (3 : ℝ))) = Real.sqrt ((1 + 1 / (2 * (3 : ℝ))) ^ 2) :=
        (Real.sqrt_sq (by norm_num)).symm
      _ < Real.sqrt 2 := Real.sqrt_lt_sqrt (by positivity) hsq
  have hmul_lt : (1 + 1 / (2 * (3 : ℝ))) * Real.sqrt (D / n') < Real.sqrt (2 * (D / n')) := by
    rw [Real.sqrt_mul (by norm_num : (0 : ℝ) ≤ 2)]
    exact mul_lt_mul_of_pos_right h76_lt_sqrt2 (Real.sqrt_pos.mpr hrho_pos)
  refine ⟨hrho_pos, hrho_lt, by linarith, by linarith⟩

/-- Lemma 1(ii) Case 2 monotonicity of the BCIKS20 curve bound:
    `(n')^2 / (D / n')^(3/2) ≤ n^2 / (D / n)^(3/2)` whenever `0 < D` and `0 < n' ≤ n`. -/
theorem peel_case2_curve_monotone {D n' n_tot : ℝ} (hD : 0 < D) (hn'_pos : 0 < n')
    (hle : n' ≤ n_tot) :
    n' ^ 2 / (D / n') ^ ((3 : ℝ) / 2) ≤ n_tot ^ 2 / (D / n_tot) ^ ((3 : ℝ) / 2) := by
  have hn_pos : 0 < n_tot := lt_of_lt_of_le hn'_pos hle
  have hrew : ∀ x : ℝ, 0 < x →
      x ^ 2 / (D / x) ^ ((3 : ℝ) / 2) = x ^ ((7 : ℝ) / 2) / D ^ ((3 : ℝ) / 2) := by
    intro x hx
    rw [Real.div_rpow hD.le hx.le, div_div_eq_mul_div, ← Real.rpow_natCast x 2,
      ← Real.rpow_add hx]
    norm_num
  rw [hrew n' hn'_pos, hrew n_tot hn_pos]
  apply div_le_div_of_nonneg_right _ (Real.rpow_nonneg hD.le _)
  exact Real.rpow_le_rpow hn'_pos.le hle (by norm_num)

/-- Lemma 1(ii) Case 2 agreement combination on `¬E_rem`:
    if `A_peel ≤ k * D` and `A_rem < √(2 D n')`, then `A_peel + A_rem < √(2 D n)`. -/
theorem peel_case2_agreement_strict {D n_tot A_peel A_rem : ℝ} {k : ℕ}
    (P : PeelSeq D k)
    (hx0 : P.x 0 ≤ n_tot)
    (hA_peel : A_peel ≤ (k : ℝ) * D)
    (hA_rem_lt : A_rem < Real.sqrt (2 * D * P.x ⟨k, Nat.lt_succ_self k⟩)) :
    A_peel + A_rem < Real.sqrt (2 * D * n_tot) := by
  set n' := P.x ⟨k, Nat.lt_succ_self k⟩
  have hD := P.hD
  have hpeel := peel_kD_le P
  have hsplit : Real.sqrt (2 * D * n') = Real.sqrt (2 * D) * Real.sqrt n' :=
    Real.sqrt_mul (by positivity) n'
  have hsplit0 : Real.sqrt (2 * D * P.x 0) = Real.sqrt (2 * D) * Real.sqrt (P.x 0) :=
    Real.sqrt_mul (by positivity) (P.x 0)
  have hsqrt_mono : Real.sqrt (2 * D * P.x 0) ≤ Real.sqrt (2 * D * n_tot) :=
    Real.sqrt_le_sqrt (by nlinarith)
  linarith

end PFSH02.Peel
