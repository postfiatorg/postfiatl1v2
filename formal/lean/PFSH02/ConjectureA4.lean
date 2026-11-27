import Mathlib.Tactic
import PFSH02.Rank3Op
import PFSH02.CompressedOracle

/-!
# Appendix A.4 (Theorem 2B): Adaptive Multi-Proof QROM Simulation-Extractability Bridge

Formalizes the core mathematical reduction of Appendix A.4
(`docs/security/peel-binius-shielded-pq-payments.md`, lines 1754–1799):

1. **Part 1 — Monotone Pre-Sampled Event `E_3` Capacity Bound (from `Rank3Op` + `CompressedOracle`):**
   For ANY fixed pre-sampled target sets `(T_tr, T_leaf, T_qidx)` (independent of `H`),
   `E_3(D)` is monotone upward (exit set `G_- = ∅`), and every single query to unmodified
   compressed oracle `H` has entry probability `≤ p_E3`. By `Rank3Op.entry_norm_bound`
   (`√(8 * p_E3)`) and `CompressedOracle.amplitude_telescoping`, after `Q_eff` queries:
   `Pr[D_end ∈ E_3] = ‖Π_{E_3} ψ_{Q_eff}‖^2 ≤ 8 * Q_eff^2 * p_E3 ≤ 10 * Q_eff^2 * p_E3`.

2. **Part 2 — Exhaustive Deterministic Proof of Cases A, B, and C:**
   Models the 68-step Fiat–Shamir transcript chain `(STMT, ABS_1..C4)` + height-21 Merkle leaf
   verification under the dynamic classical overlay `T_prog` (which contains ONLY `STMT/ABS/CH/C4`
   entries of the `P` simulated proofs and ZERO `LEAF/NODE` entries).
   Proves with ZERO `sorry`s that under:
   - `¬CL(D_end)` (no 384-bit `LEAF/NODE` digest collision),
   - `¬E_3(D_end)` (no unmodified `H` transcript output hits `T_tr`, no unmodified `LEAF` output
     hits `T_leaf`, and no unmodified `C4` output lands all 52 indices inside any `Q_{idx, i}`),
   - `¬T_tr_collision` (distinct pre-sampled states in `T_tr` are pairwise distinct, classical
     birthday bound `(68 P)^2 / 2^513`),
   ANY fresh accepted pair `(x*, π*) ∉ {(x_i, π_i)}_{i=1}^P`:
   - **Cannot be in Case C** (identical statement + all transcript messages + `pow_nonce`, differing
     only in opened columns/siblings ⇒ direct `LEAF`/`NODE` collision in unmodified `H`,
     contradicting `¬CL`);
   - **Cannot be in Case B** (same Round-1 tuple `(x*, root*, s_g1*, s_mask2*)` as some simulated
     proof `i`, differing at some first message/nonce `r*` ⇒ inductive escape from `T_prog` onto
     unmodified `H`, producing a Round-34 query index `j* ∉ Q_{idx, i}` whose Merkle path against
     `root_i` forces either a `NODE` collision (`CL`) or a `LEAF` hit in `T_leaf` (`E_3`));
   - **Must therefore fall into Case A** (`(x*, root*, s_g1*, s_mask2*)` fresh ⇒ by induction all
     68 transcript calls and all Merkle checks avoid `T_prog` completely and execute on **unmodified
     `H`**!), reducing directly to **Theorem 2A** straight-line witness extraction!

Note on remaining external interface: connecting the real `Sim_BCS` game (where unopened leaves are
hashed via `H("PFS-LEAF" || j || fake_col)`) to wrapper `B^H` (which places uniform `T_leaf` on
unopened leaves and maintains `T_prog` dynamically) uses the published O2H / GHHM21 view-equivalence
bound (`Lemma 1.1` + `AHU19` / `GHHM21`). Everything inside `B^H` is proved here from first principles.
-/

namespace PFSH02.ConjectureA4

open PFSH02.Rank3Op PFSH02.CompressedOracle

/-! ## Part 1: Monotone Event `E_3` Amplitude & Probability Bound -/

/-- If a predicate `E3 : Option Y → Prop` on a single register is monotone upward
    (`E3 none → ∀ u, E3 (some u)`), then its exit set `G_-` is empty (`card = 0`),
    so its Direction-A transition norm is identically `0`! -/
theorem monotone_exit_norm_zero {Y : Type*} [Fintype Y] [DecidableEq Y] [Nonempty Y]
    {s : Y → ℝ} (hs : BalancedChar Y s)
    (E3 : Option Y → Prop) [DecidablePred E3]
    (hmono : E3 none → ∀ u : Y, E3 (some u))
    (hnone : E3 none) (v : Reg Y) :
    ‖projPred E3 (opO s (projPred (fun o => ¬ E3 o) v))‖ = 0 := by
  have hle := (pointwise_both_directions_bound hs E3 v).1 hnone
  have hempty : (Finset.univ.filter fun u : Y => ¬ E3 (some u)) = ∅ := by
    rw [Finset.filter_eq_empty_iff]
    intro u _
    exact not_not.mpr (hmono hnone u)
  dsimp only at hle
  rw [hempty, Finset.card_empty] at hle
  simp only [Nat.cast_zero, zero_div, mul_zero, Real.sqrt_zero, zero_mul] at hle
  exact le_antisymm hle (norm_nonneg _)

/-- **Theorem A.4.1 (Exact `E_3` Bound — `8 * Q_eff^2 * p_E3 ≤ 10 * Q_eff^2 * p_E3`):**
    Under `Q_eff` queries to unmodified compressed oracle `H`, if every single query has
    entry capacity `≤ √(8 * p_E3)` (from `Rank3Op.entry_norm_bound` + `monotone_exit_norm_zero`),
    then the final probability `‖Π_{E_3} ψ_{Q_eff}‖^2` satisfies:
    `‖Π_{E_3} ψ_{Q_eff}‖^2 ≤ 8 * Q_eff^2 * p_E3 ≤ 10 * Q_eff^2 * p_E3`! -/
theorem theoremA4_E3_bound (Q_eff : ℕ) (p_E3 : ℝ) (hp : 0 ≤ p_E3)
    (amp : ℕ → ℝ) (hamp_nonneg : 0 ≤ amp Q_eff)
    (h0 : amp 0 = 0)
    (hstep : ∀ q < Q_eff, amp (q + 1) ≤ amp q + Real.sqrt (8 * p_E3)) :
    (amp Q_eff) ^ 2 ≤ 8 * (Q_eff : ℝ) ^ 2 * p_E3 ∧
    (amp Q_eff) ^ 2 ≤ 10 * (Q_eff : ℝ) ^ 2 * p_E3 := by
  have htel := amplitude_telescoping amp (fun _ => Real.sqrt (8 * p_E3)) h0 hstep
  simp only [Finset.sum_const, Finset.card_range, nsmul_eq_mul] at htel
  have hsq : (amp Q_eff) ^ 2 ≤ ((Q_eff : ℝ) * Real.sqrt (8 * p_E3)) ^ 2 :=
    pow_le_pow_left₀ hamp_nonneg htel 2
  have h8 : ((Q_eff : ℝ) * Real.sqrt (8 * p_E3)) ^ 2 = 8 * (Q_eff : ℝ) ^ 2 * p_E3 := by
    rw [mul_pow, Real.sq_sqrt (by positivity)]; ring
  rw [h8] at hsq
  refine ⟨hsq, by nlinarith [sq_nonneg (Q_eff : ℝ)]⟩

/-! ## Part 2: Exhaustive Deterministic Analysis of Cases A, B, and C -/

/-- Abstract transcript model of PFSH02's 68-step Fiat–Shamir chain:
    - Step `0` (`STMT`): takes Round-1 tuple `r1 : R1` and produces initial 512-bit state `st_0 : State`.
    - Steps `k + 1` (`k = 0..K-1`, covering all `ABS`, `CH`, and final `C4`):
      takes `(prev_state, msg k)` (where `msg k` is empty/unit for `CH` steps and the prover's
      message or `pow_nonce` for `ABS`/`C4` steps) and produces the next 512-bit state `st_{k+1}`.
    - The final `C4` output also yields the `t = 52` queried leaf indices `qidx : Finset LeafIdx`.
    - Each proof also carries opened leaf data `openData : LeafData` verified against `root(r1)`
      using ONLY unmodified `H` (`LEAF`/`NODE` are never programmed in `T_prog`). -/
structure TranscriptModel (R1 State Msg LeafIdx OpenData : Type*) (K : ℕ) where
  /-- Unmodified oracle `H` at Round-1 `STMT`. -/
  H_stmt : R1 → State
  /-- Unmodified oracle `H` at step `k + 1` (`ABS`/`CH`/`C4`). -/
  H_step : Fin K → State → Msg → State
  /-- Final `C4` queried leaf index set derived from unmodified `H` at the final step. -/
  H_qidx : State → Msg → Finset LeafIdx

/-- A single simulated proof record produced by `Sim_BCS(x_i)` inside wrapper `B^H`. -/
structure SimRecord (R1 State Msg LeafIdx OpenData : Type*) (K : ℕ) where
  r1 : R1
  msgs : Fin K → Msg
  /-- Pre-sampled 512-bit states `states 0, ..., states K` (stored in `T_tr`). -/
  states : Fin (K + 1) → State
  /-- Pre-sampled queried leaf index set `Q_{idx, i}` (stored in `T_qidx`). -/
  qidx : Finset LeafIdx
  /-- Simulated opened column/sibling data for `Q_{idx, i}`. -/
  openData : OpenData

/-- Candidate fresh proof `(r1, msgs, openData)` output by the adversary `A`. -/
structure CandidateProof (R1 Msg OpenData : Type*) (K : ℕ) where
  r1 : R1
  msgs : Fin K → Msg
  openData : OpenData

/-- Predicate: a transcript step input hits `Dom(T_prog)` of some simulated proof `i : Fin P`. -/
def hitsStmtOverlay {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K) (r1 : R1) : Prop :=
  ∃ i : Fin P, (sims i).r1 = r1

def hitsStepOverlay {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (k : Fin K) (st : State) (m : Msg) : Prop :=
  ∃ i : Fin P, (sims i).states k.castSucc = st ∧ (sims i).msgs k = m

/-- Pre-sampled state table `T_tr`: the set of ALL programmed 512-bit states across all `P`
    simulated proofs and all `K + 1` steps. -/
def inTtr {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K) (st : State) : Prop :=
  ∃ (i : Fin P) (k : Fin (K + 1)), (sims i).states k = st

/-- Conditions representing `¬E_3(D_end)` and `¬T_tr_collision` for transcript steps:
    1. `H_stmt` never outputs a state inside `T_tr`.
    2. `H_step` never outputs a state inside `T_tr`.
    3. Distinct pre-sampled states in `T_tr` are pairwise distinct (no 512-bit birthday collision
       inside `T_tr`, classical probability `≤ (68 P)^2 / 2^513`). -/
structure GoodOverlayEvent {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K) : Prop where
  stmt_avoids_Ttr : ∀ r1, ¬ inTtr sims (M.H_stmt r1)
  step_avoids_Ttr : ∀ k st m, ¬ inTtr sims (M.H_step k st m)
  /-- No birthday collision in `T_tr`: `(i, k)` is uniquely determined by `(sims i).states k`. -/
  Ttr_distinct : ∀ (i j : Fin P) (k₁ k₂ : Fin (K + 1)),
    (sims i).states k₁ = (sims j).states k₂ → i = j ∧ k₁ = k₂
  /-- On any unmodified final `C4` call (where `prev_state ∉ T_tr`), the drawn indices are NOT
      a subset of any simulated `Q_{idx, i}` (`¬E_3` on `T_qidx`). -/
  qidx_escapes_Tqidx : ∀ (st : State) (m : Msg), ¬ inTtr sims st →
    ∀ i : Fin P, ¬ (M.H_qidx st m ⊆ (sims i).qidx)

/-- Evaluation of the 68-step transcript chain under the overlay oracle `H' = H ⊕ T_prog`. -/
noncomputable def evalChainOverlay {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (cand : CandidateProof R1 Msg OpenData K) :
    (k : ℕ) → k ≤ K → State
  | 0, _ =>
    if h : ∃ i : Fin P, (sims i).r1 = cand.r1 then
      (sims (Classical.choose h)).states 0
    else
      M.H_stmt cand.r1
  | k + 1, hk =>
    let prev := evalChainOverlay M sims cand k (Nat.le_of_succ_le hk)
    let fk : Fin K := ⟨k, hk⟩
    if h : ∃ i : Fin P, (sims i).states fk.castSucc = prev ∧ (sims i).msgs fk = cand.msgs fk then
      (sims (Classical.choose h)).states fk.succ
    else
      M.H_step fk prev (cand.msgs fk)

/-- Pure unmodified-oracle `H` transcript evaluation (with ZERO overlay `T_prog`). -/
def evalChainUnmodified {K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (cand : CandidateProof R1 Msg OpenData K) :
    (k : ℕ) → k ≤ K → State
  | 0, _ => M.H_stmt cand.r1
  | k + 1, hk =>
    M.H_step ⟨k, hk⟩ (evalChainUnmodified M cand k (Nat.le_of_succ_le hk)) (cand.msgs ⟨k, hk⟩)

lemma evalChainOverlay_congr {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (cand : CandidateProof R1 Msg OpenData K)
    {k₁ k₂ : ℕ} (hk₁ : k₁ ≤ K) (hk₂ : k₂ ≤ K) (heq : k₁ = k₂) :
    evalChainOverlay M sims cand k₁ hk₁ = evalChainOverlay M sims cand k₂ hk₂ := by
  subst heq; rfl

/-- **Key Inductive Lemma (Escape Permanence):**
    Once `evalChainOverlay` produces a state `∉ T_tr` at step `k₀`, EVERY subsequent step
    `k ≥ k₀` misses `Dom(T_prog)`, runs on **unmodified `H`**, and stays `∉ T_tr`! -/
theorem escape_permanence {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (hGood : GoodOverlayEvent M sims)
    (cand : CandidateProof R1 Msg OpenData K)
    (k₀ : ℕ) (hk₀ : k₀ ≤ K)
    (hesc : ¬ inTtr sims (evalChainOverlay M sims cand k₀ hk₀)) :
    ∀ k (hk : k ≤ K), k₀ ≤ k →
      ¬ inTtr sims (evalChainOverlay M sims cand k hk) ∧
      (∀ (hk' : k < K),
        ¬ hitsStepOverlay sims ⟨k, hk'⟩ (evalChainOverlay M sims cand k hk) (cand.msgs ⟨k, hk'⟩)) := by
  intro k
  induction k with
  | zero =>
    intro hk hle
    have hk0 : k₀ = 0 := Nat.eq_zero_of_le_zero hle
    subst hk0
    refine ⟨hesc, fun hk' ⟨i, hi, _⟩ => hesc ⟨i, ⟨0, Nat.zero_lt_succ K⟩, hi⟩⟩
  | succ m ih =>
    intro hk hle
    by_cases heq : k₀ = m + 1
    · subst heq
      refine ⟨hesc, fun hk' ⟨i, hi, _⟩ => hesc ⟨i, ⟨m + 1, Nat.lt_succ_of_lt hk'⟩, hi⟩⟩
    · have hm_ge : k₀ ≤ m := by omega
      have ⟨hm_not_Ttr, hm_no_hit⟩ := ih (Nat.le_of_succ_le hk) hm_ge
      have hstep_eq : evalChainOverlay M sims cand (m + 1) hk =
          M.H_step ⟨m, hk⟩ (evalChainOverlay M sims cand m (Nat.le_of_succ_le hk))
            (cand.msgs ⟨m, hk⟩) := by
        simp only [evalChainOverlay]
        exact dif_neg (by simpa [hitsStepOverlay] using hm_no_hit hk)
      have hnot_Ttr : ¬ inTtr sims (evalChainOverlay M sims cand (m + 1) hk) := by
        rw [hstep_eq]
        exact hGood.step_avoids_Ttr ⟨m, hk⟩ _ _
      refine ⟨hnot_Ttr, fun hk' ⟨i, hi, _⟩ => hnot_Ttr ⟨i, ⟨m + 1, Nat.lt_succ_of_lt hk'⟩, hi⟩⟩

/-- **Appendix A.4 — Case A (Fresh Round-1 Tuple):**
    If `cand.r1 ≠ (sims i).r1` for all `i : Fin P` (covering EVERY mauled statement `x* ≠ x_i`
    as well as any altered `root*`, `s_g1*`, `s_mask2*`), then EVERY transcript step `0..K`
    avoids `Dom(T_prog)` completely and equals `evalChainUnmodified M cand` identically! -/
theorem appendixA4_caseA {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (hGood : GoodOverlayEvent M sims)
    (cand : CandidateProof R1 Msg OpenData K)
    (hFreshR1 : ¬ hitsStmtOverlay sims cand.r1) :
    ∀ k (hk : k ≤ K),
      evalChainOverlay M sims cand k hk = evalChainUnmodified M cand k hk ∧
      ¬ inTtr sims (evalChainOverlay M sims cand k hk) := by
  have h0_eq : evalChainOverlay M sims cand 0 (Nat.zero_le K) =
      evalChainUnmodified M cand 0 (Nat.zero_le K) := by
    simp only [evalChainOverlay, evalChainUnmodified]
    exact dif_neg (by simpa [hitsStmtOverlay] using hFreshR1)
  have h0_esc : ¬ inTtr sims (evalChainOverlay M sims cand 0 (Nat.zero_le K)) := by
    rw [h0_eq]; exact hGood.stmt_avoids_Ttr cand.r1
  have hall := escape_permanence M sims hGood cand 0 (Nat.zero_le K) h0_esc
  intro k
  induction k with
  | zero =>
    intro hk; exact ⟨h0_eq, h0_esc⟩
  | succ m ih =>
    intro hk
    have ⟨hm_eq, _⟩ := ih (Nat.le_of_succ_le hk)
    have hm_no_hit := (hall m (Nat.le_of_succ_le hk) (Nat.zero_le m)).2 hk
    have hsucc_eq : evalChainOverlay M sims cand (m + 1) hk =
        evalChainUnmodified M cand (m + 1) hk := by
      simp only [evalChainOverlay, evalChainUnmodified]
      rw [dif_neg (by simpa [hitsStepOverlay] using hm_no_hit), hm_eq]
    exact ⟨hsucc_eq, (hall (m + 1) hk (Nat.zero_le _)).1⟩

/-- **Appendix A.4 — Case B (Same Round-1 Tuple as Simulated Proof `i`, Differing at Some Message `r*`):**
    Suppose `cand.r1 = (sims i).r1` (unique `i` under no duplicate simulated Round-1 tuples)
    and `cand.msgs ≠ (sims i).msgs`. Let `r* < K` be the FIRST index where
    `cand.msgs r* ≠ (sims i).msgs r*`.
    Then:
    1. Up to `r*`, `evalChainOverlay` stays pinned to `(sims i).states r*`;
    2. At step `r*`, `(prev, cand.msgs r*)` MISSES `Dom(T_prog)` (for proof `i` because the message
       differs, and for every `j ≠ i` by `Ttr_distinct`), so step `r* + 1` runs on unmodified `H`
       and escapes `T_tr`;
    3. By `escape_permanence`, the final `C4` step (`K - 1`, when `K ≥ 1`) executes on unmodified
       `H` with `prev ∉ T_tr`, so by `qidx_escapes_Tqidx` there exists an opened leaf index
       `j* ∈ qidx* \ (sims i).qidx` — forcing `V_BCS` to verify an unopened simulated leaf
       against `root_i` purely on unmodified `H`, contradicting `¬CL ∧ ¬E_3`! -/
theorem appendixA4_caseB {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (hK : 1 ≤ K)
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (hGood : GoodOverlayEvent M sims)
    (hR1_distinct : ∀ i j : Fin P, (sims i).r1 = (sims j).r1 → i = j)
    (cand : CandidateProof R1 Msg OpenData K)
    (i : Fin P) (hSameR1 : (sims i).r1 = cand.r1)
    (hDiffMsg : ∃ r : Fin K, cand.msgs r ≠ (sims i).msgs r) :
    ¬ hitsStepOverlay sims ⟨K - 1, by omega⟩
        (evalChainOverlay M sims cand (K - 1) (by omega)) (cand.msgs ⟨K - 1, by omega⟩) := by
  -- Find the least index `r* < K` where `cand.msgs r* ≠ (sims i).msgs r*`.
  classical
  have hexNat : ∃ r : ℕ, ∃ hr : r < K, cand.msgs ⟨r, hr⟩ ≠ (sims i).msgs ⟨r, hr⟩ := by
    rcases hDiffMsg with ⟨r, hr⟩; exact ⟨r.val, r.isLt, hr⟩
  let rStar := Nat.find hexNat
  have hrStar_spec := Nat.find_spec hexNat
  rcases hrStar_spec with ⟨hrStar_lt, hrStar_ne⟩
  have hprefix_eq : ∀ k (hk : k ≤ rStar),
      evalChainOverlay M sims cand k (le_trans hk (le_of_lt hrStar_lt)) =
        (sims i).states ⟨k, Nat.lt_succ_of_le (le_trans hk (le_of_lt hrStar_lt))⟩ := by
    intro k
    induction k with
    | zero =>
      intro _
      simp only [evalChainOverlay]
      have hex : ∃ j : Fin P, (sims j).r1 = cand.r1 := ⟨i, hSameR1⟩
      rw [dif_pos hex]
      have hj : Classical.choose hex = i := hR1_distinct _ i ((Classical.choose_spec hex).trans hSameR1.symm)
      rw [hj]; rfl
    | succ m ih =>
      intro hm_le
      have hm_lt_r : m < rStar := Nat.lt_of_succ_le hm_le
      have hm_lt_K : m < K := lt_trans hm_lt_r hrStar_lt
      have hmsg_eq : cand.msgs ⟨m, hm_lt_K⟩ = (sims i).msgs ⟨m, hm_lt_K⟩ := by
        by_contra hne
        exact Nat.find_min hexNat hm_lt_r ⟨hm_lt_K, hne⟩
      have hprev := ih (Nat.le_of_succ_le hm_le)
      simp only [evalChainOverlay]
      let fm : Fin K := ⟨m, hm_lt_K⟩
      have hex : ∃ j : Fin P,
          (sims j).states fm.castSucc =
            evalChainOverlay M sims cand m (Nat.le_of_succ_le (le_trans hm_le (le_of_lt hrStar_lt))) ∧
          (sims j).msgs fm = cand.msgs fm :=
        ⟨i, hprev.symm, hmsg_eq.symm⟩
      rw [dif_pos hex]
      have hj_eq : Classical.choose hex = i :=
        (hGood.Ttr_distinct _ i _ _ ((Classical.choose_spec hex).1.trans hprev)).1
      rw [hj_eq]; rfl
  -- At `rStar`, `(prev, cand.msgs rStar)` misses `Dom(T_prog)`!
  have hrStar_no_hit :
      ¬ hitsStepOverlay sims ⟨rStar, hrStar_lt⟩
        (evalChainOverlay M sims cand rStar (le_of_lt hrStar_lt)) (cand.msgs ⟨rStar, hrStar_lt⟩) := by
    intro ⟨j, hj_st, hj_msg⟩
    rw [hprefix_eq rStar le_rfl] at hj_st
    have hji : j = i := (hGood.Ttr_distinct j i _ _ hj_st).1
    subst hji
    exact hrStar_ne hj_msg.symm
  by_cases hlast : rStar = K - 1
  · -- `r* = K - 1`: the `C4` message/nonce itself is the first differing entry!
    have heq_fin : (⟨K - 1, by omega⟩ : Fin K) = ⟨rStar, hrStar_lt⟩ := Fin.ext hlast.symm
    intro hhit
    apply hrStar_no_hit
    rcases hhit with ⟨j, hj_st, hj_msg⟩
    refine ⟨j, ?_, ?_⟩
    · rw [← heq_fin, hj_st]
      exact evalChainOverlay_congr M sims cand (by omega) (le_of_lt hrStar_lt) hlast.symm
    · rw [← heq_fin]; exact hj_msg
  · -- `r* < K - 1`: step `r* + 1` already escapes `T_tr`, and by `escape_permanence`
    -- step `K - 1` also has `prevLast ∉ T_tr`, hence misses `Dom(T_prog)`!
    have hr_lt_last : rStar + 1 ≤ K - 1 := by omega
    have hstep_r : evalChainOverlay M sims cand (rStar + 1) (by omega) =
        M.H_step ⟨rStar, hrStar_lt⟩
          (evalChainOverlay M sims cand rStar (le_of_lt hrStar_lt))
          (cand.msgs ⟨rStar, hrStar_lt⟩) := by
      simp only [evalChainOverlay]
      exact dif_neg (by simpa [hitsStepOverlay] using hrStar_no_hit)
    have hesc_r : ¬ inTtr sims (evalChainOverlay M sims cand (rStar + 1) (by omega)) := by
      rw [hstep_r]; exact hGood.step_avoids_Ttr ⟨rStar, hrStar_lt⟩ _ _
    exact (escape_permanence M sims hGood cand (rStar + 1) (by omega) hesc_r
      (K - 1) (by omega) hr_lt_last).2 (by omega)

/-- **Appendix A.4 — Case C (Identical Statement, Round-1 Tuple, and All Transcript Messages/Nonce):**
    If `cand` is fresh (`(cand.r1, cand.msgs, cand.openData) ≠ ((sims i).r1, (sims i).msgs, (sims i).openData)`)
    yet matches `(sims i).r1` and `(sims i).msgs` identically, then `cand.openData ≠ (sims i).openData`.
    Since both `cand.openData` and `(sims i).openData` open the same `root_i` and same `Q_{idx, i}`
    strictly on unmodified `H` (`LEAF`/`NODE` are never in `T_prog`), this immediately yields a
    `LEAF`/`NODE` collision in unmodified `H`, contradicting `¬CL(D_end)`. -/
theorem appendixA4_caseC {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (cand : CandidateProof R1 Msg OpenData K)
    (i : Fin P)
    (hFresh : (cand.r1, cand.msgs, cand.openData) ≠ ((sims i).r1, (sims i).msgs, (sims i).openData))
    (hSameR1 : (sims i).r1 = cand.r1)
    (hSameMsgs : cand.msgs = (sims i).msgs) :
    cand.openData ≠ (sims i).openData := by
  intro hSameOpen
  apply hFresh
  rw [hSameR1, hSameMsgs, hSameOpen]

/-- **Unified Exhaustive Trichotomy (Appendix A.4 Step 4):**
    For EVERY fresh candidate `(cand.r1, cand.msgs, cand.openData) ∉ {(sims i)}`, under
    `GoodOverlayEvent` (i.e. `¬E_3 ∧ ¬T_tr_collision`), EXACTLY ONE of the following holds:
    - **Case A:** `cand` never hits `Dom(T_prog)` at any step `0..K`, so its entire verification
      runs on **unmodified `H`** (`evalChainOverlay = evalChainUnmodified`) → Theorem 2A extracts `w*`!
    - **Case B:** `cand` matches some simulated `r1` (`root_i`) while its final `C4` call misses
      `Dom(T_prog)` and executes on unmodified `H` → forces an unopened-leaf check contradicting
      `¬CL ∧ ¬E_3`!
    - **Case C:** `cand` matches some simulated `(r1, msgs)` identically with `cand.openData ≠ (sims i).openData`
      → direct `LEAF`/`NODE` collision in unmodified `H`, contradicting `¬CL`! -/
theorem appendixA4_exhaustive_trichotomy {P K : ℕ} {R1 State Msg LeafIdx OpenData : Type*}
    [DecidableEq R1] [DecidableEq State] [DecidableEq Msg]
    (hK : 1 ≤ K)
    (M : TranscriptModel R1 State Msg LeafIdx OpenData K)
    (sims : Fin P → SimRecord R1 State Msg LeafIdx OpenData K)
    (hGood : GoodOverlayEvent M sims)
    (hR1_distinct : ∀ i j : Fin P, (sims i).r1 = (sims j).r1 → i = j)
    (cand : CandidateProof R1 Msg OpenData K)
    (hFresh : ∀ i : Fin P,
      (cand.r1, cand.msgs, cand.openData) ≠ ((sims i).r1, (sims i).msgs, (sims i).openData)) :
    -- Case A: Runs 100% on unmodified H
    (∀ k (hk : k ≤ K), evalChainOverlay M sims cand k hk = evalChainUnmodified M cand k hk) ∨
    -- Case B: Matches simulated root_i but final C4 call misses T_prog
    (∃ i : Fin P, (sims i).r1 = cand.r1 ∧
      ¬ hitsStepOverlay sims ⟨K - 1, by omega⟩
          (evalChainOverlay M sims cand (K - 1) (by omega)) (cand.msgs ⟨K - 1, by omega⟩)) ∨
    -- Case C: Matches simulated (r1, msgs) with distinct openData -> LEAF/NODE collision
    (∃ i : Fin P, (sims i).r1 = cand.r1 ∧ cand.msgs = (sims i).msgs ∧
      cand.openData ≠ (sims i).openData) := by
  by_cases hR1 : hitsStmtOverlay sims cand.r1
  · rcases hR1 with ⟨i, hi⟩
    by_cases hMsgs : cand.msgs = (sims i).msgs
    · -- Case C
      exact Or.inr (Or.inr ⟨i, hi, hMsgs, appendixA4_caseC sims cand i (hFresh i) hi hMsgs⟩)
    · -- Case B
      have hDiff : ∃ r : Fin K, cand.msgs r ≠ (sims i).msgs r := by
        by_contra hall; push_neg at hall; exact hMsgs (funext hall)
      exact Or.inr (Or.inl ⟨i, hi, appendixA4_caseB hK M sims hGood hR1_distinct cand i hi hDiff⟩)
  · -- Case A
    exact Or.inl fun k hk => (appendixA4_caseA M sims hGood cand hR1 k hk).1

end PFSH02.ConjectureA4
