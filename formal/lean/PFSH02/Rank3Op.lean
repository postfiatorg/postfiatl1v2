import Mathlib.Analysis.InnerProductSpace.PiL2
import Mathlib.Analysis.InnerProductSpace.Adjoint
import Mathlib.Tactic

/-!
# Lemma A.3.0 & Lemma A.3.4(b): Exact Rank-3 Self-Adjoint Operator & Compressed-Oracle Readout

Formalizes from first principles in Mathlib's finite-dimensional Hilbert space
`H := EuclideanSpace ℝ (Option Y)` (real inner-product space suffices because `|⊥⟩`, `|φ⟩`,
`|ψ_z⟩`, `Z_z`, `ST`, `O_{x,z}`, and all diagonal projectors `P_S` have purely real entries;
extending scalars to `ℂ` preserves the operator norm of every real symmetric matrix identically):

1. Orthonormal triple `{|⊥⟩, |φ⟩, |ψ_z⟩}` for any finite `Y` (`N = |Y| > 0`) and any balanced
   character `s : Y → ℝ` (`s u ∈ {-1, 1}` and `∑ u, s u = 0`, matching `(-1)^{⟨z, u⟩}` for `z ≠ 0`).
2. **Lemma A.3.0 (Exact Rank-3 Self-Adjoint Identity):**
   `O v = Z_z v + ⟨⊥, v⟩ • (ψ - ⊥) + ⟨φ, v⟩ • (φ - ψ) + ⟨ψ, v⟩ • (⊥ - φ)` and `O` is symmetric (`⟨u, O v⟩ = ⟨O u, v⟩`).
3. **Lemma A.3.0 (Both-Directions Norm Bound):**
   For EVERY subset `S : Finset Y` (not containing `⊥`), with `p = |S| / |Y|`:
   `‖P_S (O (P_notS v))‖ ≤ √(8 * p) * ‖v‖` AND `‖P_notS (O (P_S v))‖ ≤ √(8 * p) * ‖v‖`
   for ALL `v : H`, hence `‖P_S ∘L O ∘L P_notS‖ ≤ √(8 * p) ≤ √(10 * p)` and identically for the reverse direction!
4. **Lemma A.3.4(b) (Exact Compressed-Oracle Readout Error):**
   For every classical output `y : Y`, the squared norm of the off-`y` component of `ST |y⟩`
   is literally `2 / N - 1 / N^2 < 2 / N`.
-/

namespace PFSH02.Rank3Op

set_option linter.unusedSectionVars false

open scoped BigOperators RealInnerProductSpace

variable {Y : Type*} [Fintype Y] [DecidableEq Y]

abbrev Reg (Y : Type*) := EuclideanSpace ℝ (Option Y)

/-- Basis ket `|⊥⟩` (unpopulated register `none`). -/
noncomputable def ketBot : Reg Y :=
  EuclideanSpace.single (none : Option Y) (1 : ℝ)

/-- Basis ket `|u⟩` for a classical output `u : Y`. -/
noncomputable def ketVal (u : Y) : Reg Y :=
  EuclideanSpace.single (some u) (1 : ℝ)

/-- Uniform superposition `|φ⟩ = N^{-1/2} ∑_{u : Y} |u⟩`. -/
noncomputable def ketPhi : Reg Y :=
  WithLp.equiv 2 (Option Y → ℝ) |>.symm fun o =>
    match o with
    | none => 0
    | some _ => (Real.sqrt (Fintype.card Y : ℝ))⁻¹

/-- Phase-character ket `|ψ_s⟩ = N^{-1/2} ∑_{u : Y} s(u) |u⟩`. -/
noncomputable def ketPsi (s : Y → ℝ) : Reg Y :=
  WithLp.equiv 2 (Option Y → ℝ) |>.symm fun o =>
    match o with
    | none => 0
    | some u => s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹

/-- A balanced non-trivial binary phase character `s(u) = (-1)^{⟨z, u⟩}` for `z ≠ 0`. -/
structure BalancedChar (Y : Type*) [Fintype Y] (s : Y → ℝ) : Prop where
  sq_eq_one : ∀ u : Y, s u * s u = 1
  sum_zero : ∑ u : Y, s u = 0

/-- Diagonal phase operator `Z_s`: `|⊥⟩ ↦ |⊥⟩`, `|u⟩ ↦ s(u) |u⟩`. -/
noncomputable def opZ (s : Y → ℝ) (v : Reg Y) : Reg Y :=
  WithLp.equiv 2 (Option Y → ℝ) |>.symm fun o =>
    match o with
    | none => v none
    | some u => s u * v (some u)

/-- Standard-basis inner product expansion on `Reg Y`. -/
lemma inner_reg (v w : Reg Y) :
    ⟪v, w⟫ = v none * w none + ∑ u : Y, v (some u) * w (some u) := by
  rw [EuclideanSpace.inner_eq_star_dotProduct]
  simp [dotProduct, Fintype.sum_option, mul_comm]

lemma ketBot_none : (ketBot : Reg Y) none = 1 := by
  simp [ketBot, EuclideanSpace.single_apply]

lemma ketBot_some (u : Y) : (ketBot : Reg Y) (some u) = 0 := by
  simp [ketBot, EuclideanSpace.single_apply]

lemma ketPhi_none : (ketPhi : Reg Y) none = 0 := rfl

lemma ketPhi_some (u : Y) :
    (ketPhi : Reg Y) (some u) = (Real.sqrt (Fintype.card Y : ℝ))⁻¹ := rfl

lemma ketPsi_none (s : Y → ℝ) : (ketPsi s : Reg Y) none = 0 := rfl

lemma ketPsi_some (s : Y → ℝ) (u : Y) :
    (ketPsi s : Reg Y) (some u) = s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ := rfl

lemma opZ_none (s : Y → ℝ) (v : Reg Y) : opZ s v none = v none := rfl

lemma opZ_some (s : Y → ℝ) (v : Reg Y) (u : Y) : opZ s v (some u) = s u * v (some u) := rfl

/-- Orthonormality of `{|⊥⟩, |φ⟩, |ψ_s⟩}` for nonempty `Y` and balanced `s`. -/
lemma inner_bot_bot : ⟪(ketBot : Reg Y), ketBot⟫ = 1 := by
  rw [inner_reg]
  simp [ketBot_none, ketBot_some]

lemma inner_bot_phi : ⟪(ketBot : Reg Y), ketPhi⟫ = 0 := by
  rw [inner_reg]
  simp [ketBot_none, ketBot_some, ketPhi_none]

lemma inner_bot_psi (s : Y → ℝ) : ⟪(ketBot : Reg Y), ketPsi s⟫ = 0 := by
  rw [inner_reg]
  simp [ketBot_none, ketBot_some, ketPsi_none]

lemma inner_phi_phi [Nonempty Y] : ⟪(ketPhi : Reg Y), ketPhi⟫ = 1 := by
  have hN : (0 : ℝ) < (Fintype.card Y : ℝ) := Nat.cast_pos.mpr Fintype.card_pos
  rw [inner_reg]
  simp only [ketPhi_none, ketPhi_some, mul_zero, zero_add, Finset.sum_const, Finset.card_univ,
    nsmul_eq_mul]
  rw [← mul_inv, Real.mul_self_sqrt hN.le, mul_inv_cancel₀ hN.ne']

lemma inner_psi_psi [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s) :
    ⟪ketPsi s, ketPsi s⟫ = 1 := by
  have hN : (0 : ℝ) < (Fintype.card Y : ℝ) := Nat.cast_pos.mpr Fintype.card_pos
  rw [inner_reg]
  simp only [ketPsi_none, ketPsi_some, mul_zero, zero_add]
  have hterm : ∀ u : Y,
      s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹) =
      (Fintype.card Y : ℝ)⁻¹ := by
    intro u
    calc
      s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹)
          = (s u * s u) * ((Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (Real.sqrt (Fintype.card Y : ℝ))⁻¹) := by ring
      _ = 1 * ((Real.sqrt (Fintype.card Y : ℝ) * Real.sqrt (Fintype.card Y : ℝ))⁻¹) := by
          rw [hs.sq_eq_one u, mul_inv]
      _ = (Fintype.card Y : ℝ)⁻¹ := by rw [one_mul, Real.mul_self_sqrt hN.le]
  simp_rw [hterm]
  simp [mul_inv_cancel₀ hN.ne']

lemma inner_phi_psi {s : Y → ℝ} (hs : BalancedChar Y s) :
    ⟪(ketPhi : Reg Y), ketPsi s⟫ = 0 := by
  rw [inner_reg]
  simp only [ketPhi_none, ketPhi_some, ketPsi_none, ketPsi_some, mul_zero, zero_add]
  have hfact : ∀ u : Y,
      (Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹) =
      s u * ((Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (Real.sqrt (Fintype.card Y : ℝ))⁻¹) := by
    intro u; ring
  simp_rw [hfact, ← Finset.sum_mul, hs.sum_zero, zero_mul]

/-- Compressed-oracle swap operator `ST v = v - ⟨⊥ - φ, v⟩ • (⊥ - φ)`. -/
noncomputable def opST (v : Reg Y) : Reg Y :=
  v - ⟪(ketBot : Reg Y) - ketPhi, v⟫ • ((ketBot : Reg Y) - ketPhi)

/-- Compressed-oracle single-register query unitary `O_s := ST ∘ Z_s ∘ ST`. -/
noncomputable def opO (s : Y → ℝ) (v : Reg Y) : Reg Y :=
  opST (opZ s (opST v))

lemma opZ_bot (s : Y → ℝ) : opZ s ketBot = ketBot := by
  ext o; cases o <;> simp [opZ_none, opZ_some, ketBot_none, ketBot_some]

lemma opZ_phi (s : Y → ℝ) : opZ s ketPhi = ketPsi s := by
  ext o; cases o <;> simp [opZ_none, opZ_some, ketPhi_none, ketPhi_some, ketPsi_none, ketPsi_some]

lemma opZ_psi {s : Y → ℝ} (hs : BalancedChar Y s) : opZ s (ketPsi s) = ketPhi := by
  ext o
  cases o with
  | none => simp [opZ_none, ketPsi_none, ketPhi_none]
  | some u =>
    simp only [opZ_some, ketPsi_some, ketPhi_some]
    calc
      s u * (s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹)
          = (s u * s u) * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ := by ring
      _ = (Real.sqrt (Fintype.card Y : ℝ))⁻¹ := by rw [hs.sq_eq_one u, one_mul]

lemma opZ_sub (s : Y → ℝ) (v w : Reg Y) : opZ s (v - w) = opZ s v - opZ s w := by
  ext o; cases o <;> simp [opZ_none, opZ_some, mul_sub]

lemma opZ_smul (s : Y → ℝ) (c : ℝ) (v : Reg Y) : opZ s (c • v) = c • opZ s v := by
  ext o; cases o <;> simp [opZ_none, opZ_some]; ring

lemma inner_opZ_left (s : Y → ℝ) (v w : Reg Y) : ⟪opZ s v, w⟫ = ⟪v, opZ s w⟫ := by
  rw [inner_reg, inner_reg]
  simp only [opZ_none, opZ_some]
  congr 1
  apply Finset.sum_congr rfl
  intro u _; ring

/-- **Lemma A.3.0 (Part 1 — Exact Rank-3 Identity):**
    `O_s v = Z_s v + ⟨⊥, v⟩ • (ψ_s - ⊥) + ⟨φ, v⟩ • (φ - ψ_s) + ⟨ψ_s, v⟩ • (⊥ - φ)`. -/
theorem opO_rank3_identity [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s) (v : Reg Y) :
    opO s v = opZ s v
      + ⟪(ketBot : Reg Y), v⟫ • (ketPsi s - ketBot)
      + ⟪(ketPhi : Reg Y), v⟫ • (ketPhi - ketPsi s)
      + ⟪ketPsi s, v⟫ • (ketBot - ketPhi) := by
  have hbb := inner_bot_bot (Y := Y)
  have hbp := inner_bot_phi (Y := Y)
  have hpb : ⟪(ketPhi : Reg Y), ketBot⟫ = 0 := by rw [real_inner_comm, hbp]
  have hpp := inner_phi_phi (Y := Y)
  have hbs := inner_bot_psi s
  have hsb : ⟪ketPsi s, (ketBot : Reg Y)⟫ = 0 := by rw [real_inner_comm, hbs]
  have hps := inner_phi_psi hs
  have hsp : ⟪ketPsi s, (ketPhi : Reg Y)⟫ = 0 := by rw [real_inner_comm, hps]
  have hss := inner_psi_psi hs
  -- Step 1: expand `Z_s (ST v)`
  have h1 : opZ s (opST v) =
      opZ s v - (⟪(ketBot : Reg Y), v⟫ - ⟪(ketPhi : Reg Y), v⟫) • (ketBot - ketPsi s) := by
    simp only [opST, inner_sub_left, opZ_sub, opZ_smul, opZ_bot, opZ_phi]
  -- Step 2: compute `⟨⊥ - φ, Z_s (ST v)⟩`
  have hinner_Z_bot : ⟪(ketBot : Reg Y), opZ s v⟫ = ⟪(ketBot : Reg Y), v⟫ := by
    rw [← inner_opZ_left, opZ_bot]
  have hinner_Z_phi : ⟪(ketPhi : Reg Y), opZ s v⟫ = ⟪ketPsi s, v⟫ := by
    rw [← inner_opZ_left, opZ_phi]
  have h2 : ⟪(ketBot : Reg Y) - ketPhi, opZ s (opST v)⟫ =
      ⟪(ketPhi : Reg Y), v⟫ - ⟪ketPsi s, v⟫ := by
    rw [h1, inner_sub_right, inner_smul_right, inner_sub_left, inner_sub_left,
      hinner_Z_bot, hinner_Z_phi, inner_sub_right, inner_sub_right,
      hbb, hbs, hpb, hps]
    ring
  -- Step 3: substitute into `opST (opZ s (opST v))`
  unfold opO
  rw [opST, h2, h1]
  module

/-- **Lemma A.3.0 (Part 2 — Symmetry / Self-Adjointness):**
    `⟪u, O_s v⟫ = ⟪O_s u, v⟫` for all `u, v : Reg Y`. -/
theorem opO_symmetric [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s) (u v : Reg Y) :
    ⟪u, opO s v⟫ = ⟪opO s u, v⟫ := by
  rw [opO_rank3_identity hs v, opO_rank3_identity hs u]
  simp only [inner_add_right, inner_add_left, inner_smul_right, inner_smul_left,
    inner_sub_right, inner_sub_left, inner_opZ_left, conj_trivial]
  rw [real_inner_comm u ketBot, real_inner_comm u ketPhi, real_inner_comm u (ketPsi s)]
  ring

/-- Diagonal projector `P_S` onto a classical subset `S : Finset Y` (explicitly excluding `⊥`). -/
noncomputable def projS (S : Finset Y) (v : Reg Y) : Reg Y :=
  WithLp.equiv 2 (Option Y → ℝ) |>.symm fun o =>
    match o with
    | none => 0
    | some u => if u ∈ S then v (some u) else 0

/-- Complement projector `P_{¬S} = I - P_S` (acting on `(Y \ S) ∪ {⊥}`). -/
noncomputable def projNotS (S : Finset Y) (v : Reg Y) : Reg Y :=
  v - projS S v

lemma projS_none (S : Finset Y) (v : Reg Y) : projS S v none = 0 := rfl

lemma projS_some (S : Finset Y) (v : Reg Y) (u : Y) :
    projS S v (some u) = if u ∈ S then v (some u) else 0 := rfl

lemma projS_add (S : Finset Y) (v w : Reg Y) : projS S (v + w) = projS S v + projS S w := by
  ext o; cases o with
  | none => simp [projS_none]
  | some u => simp only [projS_some, PiLp.add_apply]; split_ifs <;> simp

lemma projS_sub (S : Finset Y) (v w : Reg Y) : projS S (v - w) = projS S v - projS S w := by
  ext o; cases o with
  | none => simp [projS_none]
  | some u => simp only [projS_some, PiLp.sub_apply]; split_ifs <;> simp

lemma projS_smul (S : Finset Y) (c : ℝ) (v : Reg Y) : projS S (c • v) = c • projS S v := by
  ext o; cases o with
  | none => simp [projS_none]
  | some u => simp only [projS_some, PiLp.smul_apply, smul_eq_mul]; split_ifs <;> simp

lemma projS_ketBot (S : Finset Y) : projS S (ketBot : Reg Y) = 0 := by
  ext o; cases o <;> simp [projS_none, projS_some, ketBot_some]

lemma projS_opZ_projNotS (S : Finset Y) (s : Y → ℝ) (v : Reg Y) :
    projS S (opZ s (projNotS S v)) = 0 := by
  ext o; cases o with
  | none => simp [projS_none]
  | some u =>
    simp only [projS_some, opZ_some, projNotS, PiLp.sub_apply, PiLp.zero_apply]
    split_ifs with h
    · simp [h]
    · rfl

lemma inner_projS_left (S : Finset Y) (v w : Reg Y) : ⟪projS S v, w⟫ = ⟪v, projS S w⟫ := by
  rw [inner_reg, inner_reg]
  simp only [projS_none, projS_some, zero_mul, mul_zero, zero_add]
  apply Finset.sum_congr rfl
  intro u _
  split_ifs <;> simp

lemma inner_projNotS_left (S : Finset Y) (v w : Reg Y) :
    ⟪projNotS S v, w⟫ = ⟪v, projNotS S w⟫ := by
  simp only [projNotS, inner_sub_left, inner_sub_right, inner_projS_left]

/-- `P_{¬S}` is a contraction: `‖P_{¬S} v‖ ≤ ‖v‖`. -/
lemma norm_projNotS_le (S : Finset Y) (v : Reg Y) : ‖projNotS S v‖ ≤ ‖v‖ := by
  have hsq : ‖projNotS S v‖ ^ 2 ≤ ‖v‖ ^ 2 := by
    rw [← real_inner_self_eq_norm_sq, ← real_inner_self_eq_norm_sq, inner_reg, inner_reg]
    simp only [projNotS, PiLp.sub_apply, projS_none, projS_some, sub_zero]
    refine add_le_add le_rfl (Finset.sum_le_sum fun u _ => ?_)
    split_ifs <;> nlinarith
  nlinarith [norm_nonneg (projNotS S v), norm_nonneg v]

/-- `‖P_S |φ⟩‖ = √(|S| / N)`. -/
lemma norm_projS_phi [Nonempty Y] (S : Finset Y) :
    ‖projS S (ketPhi : Reg Y)‖ = Real.sqrt ((S.card : ℝ) / (Fintype.card Y : ℝ)) := by
  have hN : (0 : ℝ) < (Fintype.card Y : ℝ) := Nat.cast_pos.mpr Fintype.card_pos
  have hsq : ‖projS S (ketPhi : Reg Y)‖ ^ 2 = (S.card : ℝ) / (Fintype.card Y : ℝ) := by
    rw [← real_inner_self_eq_norm_sq, inner_reg]
    simp only [projS_none, projS_some, ketPhi_some, mul_zero, zero_add]
    have hterm : ∀ u : Y,
        (if u ∈ S then (Real.sqrt (Fintype.card Y : ℝ))⁻¹ else 0) *
        (if u ∈ S then (Real.sqrt (Fintype.card Y : ℝ))⁻¹ else 0) =
        if u ∈ S then (Fintype.card Y : ℝ)⁻¹ else 0 := by
      intro u
      split_ifs
      · rw [← mul_inv, Real.mul_self_sqrt hN.le]
      · simp
    simp_rw [hterm, Finset.sum_ite_mem, Finset.univ_inter, Finset.sum_const, nsmul_eq_mul,
      div_eq_mul_inv]
  rw [← Real.sqrt_sq (norm_nonneg _), hsq]

/-- `‖P_S |ψ_s⟩‖ = √(|S| / N)`. -/
lemma norm_projS_psi [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s) (S : Finset Y) :
    ‖projS S (ketPsi s)‖ = Real.sqrt ((S.card : ℝ) / (Fintype.card Y : ℝ)) := by
  have hN : (0 : ℝ) < (Fintype.card Y : ℝ) := Nat.cast_pos.mpr Fintype.card_pos
  have hsq : ‖projS S (ketPsi s)‖ ^ 2 = (S.card : ℝ) / (Fintype.card Y : ℝ) := by
    rw [← real_inner_self_eq_norm_sq, inner_reg]
    simp only [projS_none, projS_some, ketPsi_some, mul_zero, zero_add]
    have hterm : ∀ u : Y,
        (if u ∈ S then s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ else 0) *
        (if u ∈ S then s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ else 0) =
        if u ∈ S then (Fintype.card Y : ℝ)⁻¹ else 0 := by
      intro u
      split_ifs
      · calc
          s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (s u * (Real.sqrt (Fintype.card Y : ℝ))⁻¹)
              = (s u * s u) * ((Real.sqrt (Fintype.card Y : ℝ) * Real.sqrt (Fintype.card Y : ℝ))⁻¹) := by
                rw [mul_inv]; ring
          _ = (Fintype.card Y : ℝ)⁻¹ := by rw [hs.sq_eq_one u, one_mul, Real.mul_self_sqrt hN.le]
      · simp
    simp_rw [hterm, Finset.sum_ite_mem, Finset.univ_inter, Finset.sum_const, nsmul_eq_mul,
      div_eq_mul_inv]
  rw [← Real.sqrt_sq (norm_nonneg _), hsq]

/-- `‖|⊥⟩ - |φ⟩‖ = √2` and `‖|φ⟩ - |ψ_s⟩‖ = √2`. -/
lemma norm_bot_sub_phi [Nonempty Y] : ‖(ketBot : Reg Y) - ketPhi‖ = Real.sqrt 2 := by
  have hpb : ⟪(ketPhi : Reg Y), ketBot⟫ = 0 := by rw [real_inner_comm, inner_bot_phi]
  have hsq : ‖(ketBot : Reg Y) - ketPhi‖ ^ 2 = 2 := by
    rw [← real_inner_self_eq_norm_sq, inner_sub_left, inner_sub_right, inner_sub_right,
      inner_bot_bot, inner_phi_phi, inner_bot_phi, hpb]
    ring
  rw [← Real.sqrt_sq (norm_nonneg _), hsq]

lemma norm_phi_sub_psi [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s) :
    ‖(ketPhi : Reg Y) - ketPsi s‖ = Real.sqrt 2 := by
  have hsp : ⟪ketPsi s, (ketPhi : Reg Y)⟫ = 0 := by rw [real_inner_comm, inner_phi_psi hs]
  have hsq : ‖(ketPhi : Reg Y) - ketPsi s‖ ^ 2 = 2 := by
    rw [← real_inner_self_eq_norm_sq, inner_sub_left, inner_sub_right, inner_sub_right,
      inner_phi_phi, inner_psi_psi hs, inner_phi_psi hs, hsp]
    ring
  rw [← Real.sqrt_sq (norm_nonneg _), hsq]

/-- **Lemma A.3.0 (Part 3 — Entry Operator Norm Bound):**
    For EVERY subset `S : Finset Y` (`p = |S| / N`) and EVERY state vector `v : Reg Y`:
    `‖P_S (O_s (P_{¬S} v))‖ ≤ √(8 * p) * ‖v‖`. -/
theorem entry_norm_bound [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s)
    (S : Finset Y) (v : Reg Y) :
    ‖projS S (opO s (projNotS S v))‖ ≤
      Real.sqrt (8 * ((S.card : ℝ) / (Fintype.card Y : ℝ))) * ‖v‖ := by
  set w := projNotS S v
  set p := (S.card : ℝ) / (Fintype.card Y : ℝ)
  have hp_nonneg : 0 ≤ p := by positivity
  -- Exact projected rank-2 identity:
  -- `P_S O_s w = ⟨⊥ - φ, w⟩ • P_S ψ_s + ⟨φ - ψ_s, w⟩ • P_S φ`
  have hZ0 : projS S (opZ s w) = 0 := projS_opZ_projNotS S s v
  have hproj : projS S (opO s w) =
      ⟪(ketBot : Reg Y) - ketPhi, w⟫ • projS S (ketPsi s) +
      ⟪(ketPhi : Reg Y) - ketPsi s, w⟫ • projS S (ketPhi : Reg Y) := by
    rw [opO_rank3_identity hs w]
    simp only [projS_add, projS_smul, projS_sub, hZ0, projS_ketBot,
      zero_add, sub_zero, zero_sub, inner_sub_left]
    module
  rw [hproj]
  have htri := norm_add_le
    (⟪(ketBot : Reg Y) - ketPhi, w⟫ • projS S (ketPsi s))
    (⟪(ketPhi : Reg Y) - ketPsi s, w⟫ • projS S (ketPhi : Reg Y))
  simp only [norm_smul, Real.norm_eq_abs, norm_projS_psi hs, norm_projS_phi] at htri
  have hcs1 : |⟪(ketBot : Reg Y) - ketPhi, w⟫| ≤ Real.sqrt 2 * ‖v‖ := by
    calc
      |⟪(ketBot : Reg Y) - ketPhi, w⟫|
          ≤ ‖(ketBot : Reg Y) - ketPhi‖ * ‖w‖ := abs_real_inner_le_norm _ _
      _ = Real.sqrt 2 * ‖w‖ := by rw [norm_bot_sub_phi]
      _ ≤ Real.sqrt 2 * ‖v‖ := mul_le_mul_of_nonneg_left (norm_projNotS_le S v) (Real.sqrt_nonneg _)
  have hcs2 : |⟪(ketPhi : Reg Y) - ketPsi s, w⟫| ≤ Real.sqrt 2 * ‖v‖ := by
    calc
      |⟪(ketPhi : Reg Y) - ketPsi s, w⟫|
          ≤ ‖(ketPhi : Reg Y) - ketPsi s‖ * ‖w‖ := abs_real_inner_le_norm _ _
      _ = Real.sqrt 2 * ‖w‖ := by rw [norm_phi_sub_psi hs]
      _ ≤ Real.sqrt 2 * ‖v‖ := mul_le_mul_of_nonneg_left (norm_projNotS_le S v) (Real.sqrt_nonneg _)
  have hsqrt8 : 2 * Real.sqrt 2 * Real.sqrt p = Real.sqrt (8 * p) := by
    have h1 : 2 * Real.sqrt 2 = Real.sqrt 8 := by
      have : (2 : ℝ) = Real.sqrt 4 := by norm_num
      rw [this, ← Real.sqrt_mul (by positivity)]
      norm_num
    rw [h1, ← Real.sqrt_mul (by positivity)]
  have hmul1 := mul_le_mul_of_nonneg_right hcs1 (Real.sqrt_nonneg p)
  have hmul2 := mul_le_mul_of_nonneg_right hcs2 (Real.sqrt_nonneg p)
  calc
    ‖⟪(ketBot : Reg Y) - ketPhi, w⟫ • projS S (ketPsi s) +
      ⟪(ketPhi : Reg Y) - ketPsi s, w⟫ • projS S (ketPhi : Reg Y)‖
        ≤ |⟪(ketBot : Reg Y) - ketPhi, w⟫| * Real.sqrt p
          + |⟪(ketPhi : Reg Y) - ketPsi s, w⟫| * Real.sqrt p := htri
    _ ≤ (Real.sqrt 2 * ‖v‖) * Real.sqrt p + (Real.sqrt 2 * ‖v‖) * Real.sqrt p :=
        add_le_add hmul1 hmul2
    _ = (2 * Real.sqrt 2 * Real.sqrt p) * ‖v‖ := by ring
    _ = Real.sqrt (8 * p) * ‖v‖ := by rw [hsqrt8]

/-- **Lemma A.3.0 (Part 4 — Exit / Uncompute Operator Norm Bound via Self-Adjointness):**
    For EVERY subset `S : Finset Y` (`p = |S| / N`) and EVERY state vector `v : Reg Y`:
    `‖P_{¬S} (O_s (P_S v))‖ ≤ √(8 * p) * ‖v‖`. -/
theorem exit_norm_bound [Nonempty Y] {s : Y → ℝ} (hs : BalancedChar Y s)
    (S : Finset Y) (v : Reg Y) :
    ‖projNotS S (opO s (projS S v))‖ ≤
      Real.sqrt (8 * ((S.card : ℝ) / (Fintype.card Y : ℝ))) * ‖v‖ := by
  set u := projNotS S (opO s (projS S v))
  set C := Real.sqrt (8 * ((S.card : ℝ) / (Fintype.card Y : ℝ)))
  by_cases hu : ‖u‖ = 0
  · rw [hu]; positivity
  · have hu_pos : 0 < ‖u‖ := lt_of_le_of_ne (norm_nonneg u) (Ne.symm hu)
    have hsq : ‖u‖ * ‖u‖ ≤ (C * ‖v‖) * ‖u‖ := by
      calc
        ‖u‖ * ‖u‖ = ‖u‖ ^ 2 := (sq ‖u‖).symm
        _ = ⟪u, u⟫ := (real_inner_self_eq_norm_sq u).symm
        _ = ⟪u, projNotS S (opO s (projS S v))⟫ := rfl
        _ = ⟪projNotS S u, opO s (projS S v)⟫ := by rw [← inner_projNotS_left]
        _ = ⟪opO s (projNotS S u), projS S v⟫ := by rw [opO_symmetric hs]
        _ = ⟪projS S (opO s (projNotS S u)), v⟫ := by rw [inner_projS_left]
        _ ≤ |⟪projS S (opO s (projNotS S u)), v⟫| := le_abs_self _
        _ ≤ ‖projS S (opO s (projNotS S u))‖ * ‖v‖ := abs_real_inner_le_norm _ _
        _ ≤ (C * ‖u‖) * ‖v‖ :=
            mul_le_mul_of_nonneg_right (entry_norm_bound hs S u) (norm_nonneg v)
        _ = (C * ‖v‖) * ‖u‖ := by ring
    exact le_of_mul_le_mul_right hsq hu_pos

/-- Conservative `√(10 * p)` bound (aligning with CFHL21 Thm 5.17 / `check_math.py`). -/
theorem sqrt8_le_sqrt10 (p : ℝ) (hp : 0 ≤ p) :
    Real.sqrt (8 * p) ≤ Real.sqrt (10 * p) :=
  Real.sqrt_le_sqrt (by linarith)

/-- **Lemma A.3.4(b) (Exact Compressed-Oracle Readout Error):**
    Recompressing a classically measured basis state `|y⟩` via `ST` gives
    `ST |y⟩ = (1 - 1/N)|y⟩ + N^{-1/2}|⊥⟩ - N^{-1} ∑_{u ≠ y} |u⟩`, so the probability
    that `D_end(x) ≠ y` is strictly `1 - (1 - 1/N)^2 = 2/N - 1/N^2 < 2/N`. -/
theorem readout_error_exact [Nonempty Y] (y : Y) :
    (opST (ketVal y)) (some y) = 1 - (Fintype.card Y : ℝ)⁻¹ ∧
    1 - ((opST (ketVal y)) (some y)) ^ 2 = 2 / (Fintype.card Y : ℝ) - 1 / (Fintype.card Y : ℝ) ^ 2 ∧
    1 - ((opST (ketVal y)) (some y)) ^ 2 < 2 / (Fintype.card Y : ℝ) := by
  have hN : (0 : ℝ) < (Fintype.card Y : ℝ) := Nat.cast_pos.mpr Fintype.card_pos
  have hinner : ⟪(ketBot : Reg Y) - ketPhi, ketVal y⟫ = -(Real.sqrt (Fintype.card Y : ℝ))⁻¹ := by
    rw [inner_sub_left, inner_reg, inner_reg]
    simp [ketBot_none, ketBot_some, ketPhi_none, ketPhi_some, ketVal, EuclideanSpace.single_apply]
  have hcoord : (opST (ketVal y)) (some y) = 1 - (Fintype.card Y : ℝ)⁻¹ := by
    rw [opST, hinner]
    simp only [PiLp.sub_apply, PiLp.smul_apply, smul_eq_mul, ketVal,
      EuclideanSpace.single_apply, ite_true, ketBot_some, ketPhi_some, zero_sub]
    have hmul : (Real.sqrt (Fintype.card Y : ℝ))⁻¹ * (Real.sqrt (Fintype.card Y : ℝ))⁻¹ =
        (Fintype.card Y : ℝ)⁻¹ := by
      rw [← mul_inv, Real.mul_self_sqrt hN.le]
    linarith [hmul]
  refine ⟨hcoord, ?_, ?_⟩
  · rw [hcoord]; field_simp; ring
  · rw [hcoord]
    have hpos : (0 : ℝ) < 1 / (Fintype.card Y : ℝ) ^ 2 := by positivity
    have heq : 1 - (1 - (Fintype.card Y : ℝ)⁻¹) ^ 2 =
        2 / (Fintype.card Y : ℝ) - 1 / (Fintype.card Y : ℝ) ^ 2 := by
      field_simp; ring
    linarith

end PFSH02.Rank3Op
