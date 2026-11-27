import Mathlib.Tactic

/-!
# PFSH02 Pinned Protocol Parameters & Kernel-Checked Rational Bounds

Replaces the exact-rational sanity assertions of `tools/pft-binius-shielded-pq/check_math.py`
with Lean 4 kernel-verified theorems over `ℕ` and `ℚ`.
-/

namespace PFSH02.Params

/-- Reed–Solomon degree bound `D = 565`. -/
def D : ℕ := 565

/-- Evaluation domain size `n = |L| = 2^21 = 2,097,152`. -/
def n : ℕ := 2 ^ 21

/-- Greedy peeling list-size ceiling `L_max = ⌊√(2/ρ)⌋ = 86`. -/
def L_max : ℕ := 86

/-- Number of Round-34 opened columns `t = 52`. -/
def t_open : ℕ := 52

/-- Proof-of-work zero-bit difficulty `k_pow = 20`. -/
def k_pow : ℕ := 20

/-- Number of packed witness + mask polynomials batched in `F_lp`: `l = 164` (`165` polynomials). -/
def l_batch : ℕ := 164

/-- Total scalar circuit constraints `N_C = 2,192,844`. -/
def N_C : ℕ := 2192844

/-- Verifier oracle query cap `κ_v = 68 + 52 + 843 = 963`. -/
def kappa_v : ℕ := 963

/-- Extension field cardinality `|E| = 2^384`. -/
def cardE : ℕ := 2 ^ 384

/-- Compiler oracle output cardinality `|Y| = 2^1536`. -/
def cardY : ℕ := 2 ^ 1536

/-- Internal Fiat–Shamir state cardinality `2^512`. -/
def cardState : ℕ := 2 ^ 512

/-- Hard archival wire ceiling in bytes (`149,130 B`). -/
def wireCap : ℕ := 149130

/-- Code rate `ρ = D / n` over `ℚ`. -/
def rho : ℚ := (D : ℚ) / (n : ℚ)

/-- Exact verification that `L_max = 86 = ⌊√(2 n / D)⌋`. -/
theorem L_max_exact : L_max ^ 2 * D ≤ 2 * n ∧ 2 * n < (L_max + 1) ^ 2 * D := by
  unfold L_max D n
  norm_num

/-- Exact verification of the Johnson-bound margin for `m' = 3`:
    `1 + 1/(2m') = 7/6 < 1.4142 < √2`, so `δ' = 1 - √(2ρ') < 1 - (7/6)√ρ'` strictly. -/
theorem johnson_margin_m3 : ((7 : ℚ) / 6) ^ 2 < 2 := by
  norm_num

/-- In Case 1 of Lemma 1 (`n' ≤ 2D`), `2D = 1,130 < 48,680 ≤ √(2 D n)`. -/
theorem peel_case1_margin : (2 * D) ^ 2 < 2 * D * n ∧ 48680 ^ 2 ≤ 2 * D * n := by
  unfold D n
  norm_num

/-- Exact rational verification of the `3.652` constant in Theorem 2A / Lemma A.3.4:
    `((2/3) * √30)^2 = 40/3 < (3.652)^2`. -/
theorem coeff_3652_upper : ((2 : ℚ) / 3) ^ 2 * 30 < (3652 / 1000 : ℚ) ^ 2 := by
  norm_num

/-- Exact wire-size ceiling margins under the PFSH02 `149,130 B` archival cap. -/
theorem wire_margins_exact :
    140476 + 8654 = wireCap ∧
    145737 + 3393 = wireCap ∧
    146546 + 2584 = wireCap ∧
    146580 + 2550 = wireCap := by
  unfold wireCap
  norm_num

/-- PFSH03 archival cap (`160,000 B`, ~12.87 GiB/day at 1 tx/s) accommodating
    2 × 5,261 B = 10,522 B external ML-DSA-65 spend authorization per transaction. -/
def wireCapPFSH03 : ℕ := 160000

/-- Exact PFSH03 wire-size ceiling margins under the `160,000 B` archival cap:
    - Standalone non-mint worst-case: `140,476 + 10,522 = 150,998 B` (margin `9,002 B`)
    - Standalone signed deposit worst-case: `150,998 + 5,261 = 156,259 B` (margin `3,741 B`)
    - L1 archived redb deposit worst-case: `146,546 + 10,522 = 157,068 B` (margin `2,932 B`)
    - L1 archived JSONL deposit worst-case: `146,580 + 10,522 = 157,102 B` (margin `2,898 B`). -/
theorem pfsh03_wire_margins_exact :
    150998 + 9002 = wireCapPFSH03 ∧
    156259 + 3741 = wireCapPFSH03 ∧
    157068 + 2932 = wireCapPFSH03 ∧
    157102 + 2898 = wireCapPFSH03 := by
  unfold wireCapPFSH03
  norm_num

/-- Exact verification of the 66-bit unsigned conservation ceiling:
    `3 * (2^64 - 1) < 2^66`. -/
theorem conservation_66bit_no_overflow : 3 * (2 ^ 64 - 1) < 2 ^ 66 := by
  norm_num

/-- Round-34 compiled PoW + 52-column spot-check probability:
    `p_34 = 2^(-20) * (2ρ)^26 < 2^(-302)`. -/
def p34 : ℚ := (1 / (2 ^ k_pow : ℚ)) * (2 * rho) ^ (t_open / 2)

theorem p34_lt_inv_2_302 : p34 < 1 / (2 ^ 302 : ℚ) := by
  unfold p34 rho k_pow t_open D n
  norm_num

/-- Round-33a pointwise bad-event probability `ε_pt = L_max * n * l / |E| < 2^(-349)`. -/
def eps_pt : ℚ := (L_max * n * l_batch : ℚ) / (cardE : ℚ)

theorem eps_pt_lt_inv_2_349 : eps_pt < 1 / (2 ^ 349 : ℚ) := by
  unfold eps_pt L_max n l_batch cardE
  norm_num

/-- Exact rational verification that `√(ρ')`-scaled BCIKS20 curve bound squared satisfies
    `ε_curve^2 < (2^(-305))^2`, i.e., `ε_curve < 2^(-305)`.
    Here `ε_curve = 164 * ((7/2)^7 / (3 * ρ^(3/2))) * n^2 / |E|`, so
    `ε_curve^2 = 164^2 * (7/2)^14 / (9 * ρ^3) * n^4 / |E|^2`. -/
theorem eps_curve_sq_lt_inv_2_610 :
    ((164 : ℚ) ^ 2 * ((7 / 2 : ℚ) ^ 14) / (9 * rho ^ 3)) * ((n : ℚ) ^ 4 / (cardE : ℚ) ^ 2)
      < (1 / (2 ^ 305 : ℚ)) ^ 2 := by
  unfold rho D n cardE
  norm_num

/-- Verify that the Round-34 PoW-gated spot-check bound `p_34` dominates Round-33a
    (`ε_curve + ε_pt < 2^(-305) + 2^(-349) < 2^(-304) < p_34`). -/
theorem p34_gt_inv_2_303 : (1 / (2 ^ 303 : ℚ)) < p34 := by
  unfold p34 rho k_pow t_open D n
  norm_num

/-- Theorem 2A conditional QROM extraction bound check at `Q = 2^64, L = 1`:
    with `Q_v = 2^64 + 963`, `(3.652 * Q_v^(3/2) * 2^-192 + Q_v * √(10 * p_34))^2 + 2*963/2^384 < 2^(-170)`.
    We prove the stronger squared upper bound using `(a + b)^2 ≤ 2 a^2 + 2 b^2`? Even tighter:
    we bound `a < 2^-94` and `b < 2^-85.4` via rational squares so `(a + b)^2 < 2^-170`! -/
theorem theorem2A_Q64_lt_inv_2_170 :
    let Qv : ℚ := (2 ^ 64 + kappa_v : ℕ)
    -- `a^2 = (3652/1000)^2 * Qv^3 / 2^384 < (1 / 2^94)^2`
    ((3652 / 1000 : ℚ) ^ 2 * Qv ^ 3 / (2 ^ 384 : ℚ)) < (1 / (2 ^ 94 : ℚ)) ^ 2 ∧
    -- `b^2 = 10 * Qv^2 * p34 < (99 / 100 * 1 / 2^85)^2`
    (10 * Qv ^ 2 * p34) < ((99 / 100 : ℚ) / (2 ^ 85 : ℚ)) ^ 2 ∧
    -- Hence `(a + b)^2 + 2*kappa_v/2^384 < (1/2^94 + 99/100/2^85)^2 + 2*963/2^384 < 1/2^170`
    ((1 / (2 ^ 94 : ℚ) + (99 / 100 : ℚ) / (2 ^ 85 : ℚ)) ^ 2 + (2 * kappa_v : ℚ) / (2 ^ 384 : ℚ))
      < 1 / (2 ^ 170 : ℚ) := by
  dsimp only
  unfold p34 rho k_pow t_open D n kappa_v
  constructor
  · norm_num
  constructor
  · norm_num
  · norm_num

/-- Theorem 2A conditional QROM extraction bound check at `Q = 2^80, L = 1` (`< 2^(-137)`). -/
theorem theorem2A_Q80_lt_inv_2_137 :
    let Qv : ℚ := (2 ^ 80 + kappa_v : ℕ)
    -- `a^2 < (29/100 / 2^70)^2`? Check: `3.652 * 2^120 / 2^192 = 3.652 / 2^72 < 1 / 2^70`
    ((3652 / 1000 : ℚ) ^ 2 * Qv ^ 3 / (2 ^ 384 : ℚ)) < ((23 / 100 : ℚ) / (2 ^ 68 : ℚ)) ^ 2 ∧
    -- `b^2 = 10 * Qv^2 * p34 < (37/100 / 2^68)^2`
    (10 * Qv ^ 2 * p34) < ((37 / 100 : ℚ) / (2 ^ 68 : ℚ)) ^ 2 ∧
    (((23 / 100 : ℚ) / (2 ^ 68 : ℚ) + (37 / 100 : ℚ) / (2 ^ 68 : ℚ)) ^ 2
      + (2 * kappa_v : ℚ) / (2 ^ 384 : ℚ))
      < 1 / (2 ^ 137 : ℚ) := by
  dsimp only
  unfold p34 rho k_pow t_open D n kappa_v
  constructor
  · norm_num
  constructor
  · norm_num
  · norm_num

/-- Theorem 2A conditional QROM extraction bound check at `Q = 2^96, L = 1` (`< 2^(-92)`). -/
theorem theorem2A_Q96_lt_inv_2_92 :
    let Qv : ℚ := (2 ^ 96 + kappa_v : ℕ)
    -- `3.652 * 2^144 / 2^192 = 3.652 / 2^48 < 92/100 / 2^46`
    ((3652 / 1000 : ℚ) ^ 2 * Qv ^ 3 / (2 ^ 384 : ℚ)) < ((92 / 100 : ℚ) / (2 ^ 46 : ℚ)) ^ 2 ∧
    -- `b = Qv * sqrt(10*p34) ~ 2^96 * 2^-149.5 = 2^-53.5 < 1/100 / 2^46`
    (10 * Qv ^ 2 * p34) < ((1 / 100 : ℚ) / (2 ^ 46 : ℚ)) ^ 2 ∧
    (((92 / 100 : ℚ) / (2 ^ 46 : ℚ) + (1 / 100 : ℚ) / (2 ^ 46 : ℚ)) ^ 2
      + (2 * kappa_v : ℚ) / (2 ^ 384 : ℚ))
      < 1 / (2 ^ 92 : ℚ) := by
  dsimp only
  unfold p34 rho k_pow t_open D n kappa_v
  constructor
  · norm_num
  constructor
  · norm_num
  · norm_num

end PFSH02.Params
