import Mathlib.Algebra.Polynomial.Roots
import Mathlib.LinearAlgebra.Lagrange
import Mathlib.Tactic

/-!
# Lemmas 2, 3, and 4: Dual-Polynomial Binding, Subfield `K_32` Inheritance, and Ring-Switching

Formalizes Appendix A.1 Lemmas 2, 3, and 4 of `docs/security/peel-binius-shielded-pq-payments.md`
(lines 1343–1396) over an arbitrary finite field `E` (instantiated to `GF(2^384)`):

- **Lemma 2(i):** For any mismatched `(u, x)` (`M(x) ≠ u(x)`), the batched difference polynomial
  `F_Z(M(x)) - F_Z(u(x))` is nonzero of degree `≤ l` (`164`), hence vanishes for at most `l`
  values of `λ_p ∈ E` (`Pr[Bad_pt] ≤ L_max * n * l / |E|`).
- **Lemma 2(ii):** Under `¬Bad_peel` and `¬Bad_pt`, if `|A_both| ≥ D`, then `v_prox = F_lp(u)` for
  a **unique** `u ∈ P(M)`, `A_both ⊆ A_prox ⊆ D_u`, and `v_open` equals the honest batched
  polynomial `b_pcs(u) + λ_o * ∑ c_r u_r` identically in `E[X]_{<D}`.
- **Lemma 3 (Subfield `K_32` Inheritance):** Any polynomial `p ∈ E[X]` of degree `< D` whose values
  on `D` distinct points of a subfield `K ⊆ E` lie in `K` actually lies in `K[X]_{<D}`
  (via Lagrange interpolation over `K`).
- **Lemma 4 (Ring-Switching Soundness):** Any nonzero vector difference in `E^5` produces a nonzero
  univariate polynomial of degree `≤ 4` in `γ` (`≤ 4` roots in `E`), and any nonzero linear-map
  or scalar discrepancy across the `387` powers `ρ_rs^0 .. ρ_rs^386` produces a nonzero univariate
  polynomial of degree `≤ 386` in `ρ_rs` (`≤ 386` roots in `E`).
-/

namespace PFSH02.DualBinding

open Polynomial Finset

/-- Two polynomials of degree `< D` over a field `E` that agree on at least `D` distinct points
    are identical. -/
theorem eq_of_agree_on_finset {E : Type*} [Field E] [DecidableEq E]
    {D : ℕ} {p q : E[X]}
    (hp : p.natDegree < D ∨ p = 0)
    (hq : q.natDegree < D ∨ q = 0)
    (S : Finset E) (hS : D ≤ S.card)
    (hagree : ∀ x ∈ S, p.eval x = q.eval x) :
    p = q := by
  by_contra hne
  have hdiff_ne : p - q ≠ 0 := sub_ne_zero.mpr hne
  have hdeg : (p - q).natDegree < D := by
    have hle := Polynomial.natDegree_sub_le p q
    rcases hp with hp' | rfl <;> rcases hq with hq' | rfl
    · exact lt_of_le_of_lt hle (max_lt hp' hq')
    · simpa using hp'
    · simpa using hq'
    · exact absurd rfl hne
  have hroots : ∀ x ∈ S, x ∈ (p - q).roots := by
    intro x hx
    rw [Polynomial.mem_roots hdiff_ne, Polynomial.IsRoot.def, Polynomial.eval_sub, hagree x hx,
      sub_self]
  have hcard_le : S.card ≤ (p - q).roots.card := by
    have hnodup : S.val.Nodup := S.nodup
    have hsub : S.val ⊆ (p - q).roots := fun x hx => hroots x hx
    exact Multiset.card_le_card (Multiset.le_iff_subset hnodup |>.mpr hsub)
  have hcard_roots : (p - q).roots.card ≤ (p - q).natDegree := Polynomial.card_roots' (p - q)
  omega

/-- A nonzero univariate polynomial of degree `≤ d` over a finite field `E` vanishes on at most `d`
    field elements (Schwartz–Zippel for univariate polynomials). -/
theorem card_vanishing_le_deg {E : Type*} [Field E] [Fintype E] [DecidableEq E]
    {d : ℕ} (f : E[X]) (hf : f ≠ 0) (hdeg : f.natDegree ≤ d) :
    (Finset.univ.filter fun z : E => f.eval z = 0).card ≤ d := by
  set Z := Finset.univ.filter fun z : E => f.eval z = 0
  have hsub : Z.val ⊆ f.roots := by
    intro z hz
    have hz' : z ∈ Z := hz
    rw [Finset.mem_filter] at hz'
    exact (Polynomial.mem_roots hf).mpr hz'.2
  have hle1 : Z.card ≤ f.roots.card :=
    Multiset.card_le_card ((Multiset.le_iff_subset Z.nodup).mpr hsub)
  exact le_trans hle1 (le_trans (Polynomial.card_roots' f) hdeg)

/-- **Lemma 2(i) (Single-Pair Root Bound for `Bad_pt`):**
    If two tuples `a, b : Fin (l + 1) → E` differ at some coordinate `i`, then the univariate
    batching polynomial `∑_{i=0}^l (a i - b i) Z^i` is nonzero of degree `≤ l`, hence vanishes
    for at most `l` choices of `λ_p ∈ E`. -/
theorem lemma2_single_pair_roots {E : Type*} [Field E] [Fintype E] [DecidableEq E]
    {l : ℕ} (a b : Fin (l + 1) → E) (hdiff : ∃ i, a i ≠ b i) :
    (Finset.univ.filter fun lp : E =>
      (∑ i : Fin (l + 1), (a i - b i) * lp ^ (i : ℕ)) = 0).card ≤ l := by
  let f : E[X] := ∑ i : Fin (l + 1), Polynomial.C (a i - b i) * Polynomial.X ^ (i : ℕ)
  have hcoeff : ∀ j : Fin (l + 1), f.coeff (j : ℕ) = a j - b j := by
    intro j
    simp only [f, Polynomial.finset_sum_coeff, Polynomial.coeff_C_mul_X_pow]
    rw [Finset.sum_eq_single j]
    · simp
    · intro i _ hij
      have hne : (j : ℕ) ≠ (i : ℕ) := fun h => hij (Fin.ext h).symm
      simp [hne]
    · intro hj; exact absurd (Finset.mem_univ j) hj
  have hf_ne : f ≠ 0 := by
    rcases hdiff with ⟨i, hi⟩
    intro h0
    have := hcoeff i
    rw [h0, Polynomial.coeff_zero] at this
    exact hi (sub_eq_zero.mp this.symm)
  have hf_deg : f.natDegree ≤ l := by
    apply Polynomial.natDegree_sum_le_of_forall_le
    intro i _
    exact le_trans (Polynomial.natDegree_C_mul_X_pow_le (a i - b i) (i : ℕ)) (Nat.lt_succ_iff.mp i.isLt)
  have heval : ∀ lp : E, f.eval lp = ∑ i : Fin (l + 1), (a i - b i) * lp ^ (i : ℕ) := by
    intro lp
    simp [f, Polynomial.eval_finset_sum, Polynomial.eval_mul, Polynomial.eval_C, Polynomial.eval_pow]
  have hfilt : (Finset.univ.filter fun lp : E =>
      (∑ i : Fin (l + 1), (a i - b i) * lp ^ (i : ℕ)) = 0) =
      (Finset.univ.filter fun lp : E => f.eval lp = 0) := by
    ext lp; simp [heval]
  rw [hfilt]
  exact card_vanishing_le_deg f hf_ne hf_deg

/-- **Lemma 2(i) (Union Bound over `P(M) × supp(M)`):**
    The total number of `λ_p ∈ E` triggering `Bad_pt` across at most `L_max` peeled candidates
    and `n` domain points is at most `L_max * n * l`. -/
theorem lemma2_bad_pt_union_bound {E U X : Type*} [Field E] [Fintype E] [DecidableEq E]
    {l : ℕ} (P : Finset U) (L0 : Finset X)
    (Mval : X → Fin (l + 1) → E)
    (Uval : U → X → Fin (l + 1) → E) :
    (Finset.univ.filter fun lp : E =>
      ∃ u ∈ P, ∃ x ∈ L0, Mval x ≠ Uval u x ∧
        (∑ i : Fin (l + 1), (Mval x i - Uval u x i) * lp ^ (i : ℕ)) = 0).card
      ≤ P.card * L0.card * l := by
  let badPair (u : U) (x : X) : Finset E :=
    if Mval x ≠ Uval u x then
      Finset.univ.filter fun lp : E =>
        (∑ i : Fin (l + 1), (Mval x i - Uval u x i) * lp ^ (i : ℕ)) = 0
    else ∅
  have hsub : (Finset.univ.filter fun lp : E =>
      ∃ u ∈ P, ∃ x ∈ L0, Mval x ≠ Uval u x ∧
        (∑ i : Fin (l + 1), (Mval x i - Uval u x i) * lp ^ (i : ℕ)) = 0)
      ⊆ (P ×ˢ L0).biUnion (fun p => badPair p.1 p.2) := by
    intro lp hlp
    rw [Finset.mem_filter] at hlp
    rcases hlp.2 with ⟨u, hu, x, hx, hne, hzero⟩
    rw [Finset.mem_biUnion]
    refine ⟨(u, x), Finset.mem_product.mpr ⟨hu, hx⟩, ?_⟩
    simp [badPair, hne, hzero]
  have hpair_le : ∀ p ∈ P ×ˢ L0, (badPair p.1 p.2).card ≤ l := by
    intro p _
    dsimp [badPair]
    by_cases hne : Mval p.2 ≠ Uval p.1 p.2
    · rw [if_pos hne]
      have hdiff : ∃ i, Mval p.2 i ≠ Uval p.1 p.2 i := by
        by_contra hall; push_neg at hall; exact hne (funext hall)
      exact lemma2_single_pair_roots (Mval p.2) (Uval p.1 p.2) hdiff
    · rw [if_neg hne]; simp
  calc
    _ ≤ ((P ×ˢ L0).biUnion (fun p => badPair p.1 p.2)).card := Finset.card_le_card hsub
    _ ≤ ∑ p ∈ P ×ˢ L0, (badPair p.1 p.2).card := Finset.card_biUnion_le
    _ ≤ ∑ _p ∈ P ×ˢ L0, l := Finset.sum_le_sum hpair_le
    _ = (P ×ˢ L0).card * l := by simp
    _ = P.card * L0.card * l := by rw [Finset.card_product]

/-- **Lemma 2(ii) (Pointwise Inclusion `A_prox ⊆ D_u`, Uniqueness of `u`, and Dual-Polynomial Binding):**
    Whenever `¬Bad_peel(lp)`, `¬Bad_pt(lp)`, and `|A_both| ≥ D`:
    1. `v_prox` equals `F_lp(u)` for a unique `u ∈ P(M)` (given distinct codewords in `P(M)`
       agree on `< D` points);
    2. `A_both ⊆ A_prox ⊆ D_u`;
    3. `v_open` equals `openPoly(u)` identically in `E[X]_{<D}`. -/
theorem lemma2_dual_binding {E U : Type*} [Field E] [DecidableEq E]
    {D : ℕ}
    (P : Finset U) (L0 : Finset E)
    (Mval : E → E) -- `F_lp(M(x))`
    (Mopen : E → E) -- `B_pcs(x) + lo * ∑ c_r M_r(x)`
    (Fpoly : U → E[X]) -- `F_lp(u)` of degree `< D`
    (Opoly : U → E[X]) -- `b_pcs(u) + lo * ∑ c_r u_r` of degree `< D`
    (agreeU : U → E → Prop) -- `M(x) = u(x)`
    (hagree_open : ∀ u x, agreeU u x → Mopen x = (Opoly u).eval x)
    (hOdeg : ∀ u ∈ P, (Opoly u).natDegree < D ∨ Opoly u = 0)
    (hP_inj : ∀ u₁ ∈ P, ∀ u₂ ∈ P, Fpoly u₁ = Fpoly u₂ → u₁ = u₂)
    (v_prox v_open : E[X])
    (hv_open_deg : v_open.natDegree < D ∨ v_open = 0)
    (A_both : Finset E)
    (hAboth_sub : A_both ⊆ L0)
    (hAboth_prox : ∀ x ∈ A_both, v_prox.eval x = Mval x)
    (hAboth_open : ∀ x ∈ A_both, v_open.eval x = Mopen x)
    (hAboth_card : D ≤ A_both.card)
    -- `¬Bad_peel`: since `|A_prox| ≥ |A_both| ≥ D`, `v_prox` matches some `u ∈ P`.
    (hnot_bad_peel : ∃ u ∈ P, v_prox = Fpoly u)
    -- `¬Bad_pt`: whenever `F_lp(M(x)) = F_lp(u)(x)` for `u ∈ P, x ∈ L0`, full tuple agrees (`x ∈ D_u`).
    (hnot_bad_pt : ∀ u ∈ P, ∀ x ∈ L0, Mval x = (Fpoly u).eval x → agreeU u x) :
    ∃! u, u ∈ P ∧ v_prox = Fpoly u ∧
      (∀ x ∈ A_both, agreeU u x) ∧ v_open = Opoly u := by
  rcases hnot_bad_peel with ⟨u, huP, hvprox_eq⟩
  have hAboth_Du : ∀ x ∈ A_both, agreeU u x := by
    intro x hx
    apply hnot_bad_pt u huP x (hAboth_sub hx)
    rw [← hAboth_prox x hx, hvprox_eq]
  have hvopen_eq : v_open = Opoly u := by
    apply eq_of_agree_on_finset hv_open_deg (hOdeg u huP) A_both hAboth_card
    intro x hx
    rw [hAboth_open x hx, hagree_open u x (hAboth_Du x hx)]
  refine ⟨u, ⟨huP, hvprox_eq, hAboth_Du, hvopen_eq⟩, ?_⟩
  intro u' ⟨hu'P, hvprox_eq', _, _⟩
  exact hP_inj u' hu'P u huP (hvprox_eq'.symm.trans hvprox_eq)

/-- **Lemma 3 (Subfield `K_32` Inheritance via Lagrange Interpolation):**
    Let `ι : K →+* E` be any field embedding (e.g. `K_32 ↪ E = GF(2^384)`).
    If a polynomial `p ∈ E[X]` of degree `< D` takes values inside `ι(K)` on `D` distinct
    points `ι(s_0), ..., ι(s_{D-1})` from `K`, then `p = Polynomial.map ι q` for a unique
    polynomial `q ∈ K[X]` of degree `< D`.
    Consequently, `p(H) ⊆ ι(K)` for every `H ⊆ K` (enforcing structural booleanity of witness bits). -/
theorem lemma3_subfield_inheritance {K E : Type*} [Field K] [Field E] [DecidableEq K] [DecidableEq E]
    (ι : K →+* E) {D : ℕ} (p : E[X]) (hp : p.natDegree < D ∨ p = 0)
    (S : Finset K) (hS : S.card = D)
    (r : K → K)
    (hagree : ∀ x ∈ S, p.eval (ι x) = ι (r x)) :
    ∃ q : K[X], (q.natDegree < D ∨ q = 0) ∧ p = q.map ι := by
  let q : K[X] := Lagrange.interpolate S id r
  have hq_deg : q.natDegree < D ∨ q = 0 := by
    by_cases hq0 : q = 0
    · exact Or.inr hq0
    · left
      have hinj : Set.InjOn (id : K → K) (S : Set K) := Set.injOn_id _
      have hne : S.Nonempty := by
        by_contra hempty
        rw [Finset.not_nonempty_iff_eq_empty] at hempty
        have : q = 0 := by simp [q, hempty, Lagrange.interpolate]
        exact hq0 this
      have hlt := Lagrange.degree_interpolate_lt r hinj
      rw [hS] at hlt
      exact (Polynomial.natDegree_lt_iff_degree_lt hq0).mpr hlt
  have hq_eval : ∀ x ∈ S, q.eval x = r x := by
    intro x hx
    exact Lagrange.eval_interpolate_at_node r (Set.injOn_id _) hx
  have hmap_deg : (q.map ι).natDegree < D ∨ q.map ι = 0 := by
    rcases hq_deg with hlt | h0
    · left; exact lt_of_le_of_lt Polynomial.natDegree_map_le hlt
    · right; simp [h0]
  have hSim_card : (S.image ι).card = D := by
    rw [Finset.card_image_of_injective S ι.injective, hS]
  have hp_eq : p = q.map ι := by
    apply eq_of_agree_on_finset hp hmap_deg (S.image ι) (le_of_eq hSim_card.symm)
    intro y hy
    rcases Finset.mem_image.mp hy with ⟨x, hxS, rfl⟩
    rw [hagree x hxS, Polynomial.eval_map, Polynomial.eval₂_at_apply, hq_eval x hxS]
  exact ⟨q, hq_deg, hp_eq⟩

/-- **Lemma 4(ii) (Single-Challenge Ring-Switching Soundness Bounds):**
    - Batching 5 evaluation discrepancies `Δ : Fin 5 → E` with powers `γ^0..γ^4` vanishes for at
      most `4` values of `γ ∈ E` whenever some `Δ x ≠ 0`.
    - Batching `387` ring-switch + scalar discrepancies `Δ : Fin 387 → E` with powers
      `ρ_rs^0..ρ_rs^386` vanishes for at most `386` values of `ρ_rs ∈ E` whenever some `Δ j ≠ 0`. -/
theorem lemma4_gamma_soundness {E : Type*} [Field E] [Fintype E] [DecidableEq E]
    (Δ : Fin 5 → E) (hnonzero : ∃ x, Δ x ≠ 0) :
    (Finset.univ.filter fun γ : E =>
      (∑ x : Fin 5, Δ x * γ ^ (x : ℕ)) = 0).card ≤ 4 := by
  have h := lemma2_single_pair_roots Δ (fun _ => 0) (by simpa using hnonzero)
  simpa using h

theorem lemma4_rho_rs_soundness {E : Type*} [Field E] [Fintype E] [DecidableEq E]
    (Δ : Fin 387 → E) (hnonzero : ∃ j, Δ j ≠ 0) :
    (Finset.univ.filter fun ρ : E =>
      (∑ j : Fin 387, Δ j * ρ ^ (j : ℕ)) = 0).card ≤ 386 := by
  have h := lemma2_single_pair_roots Δ (fun _ => 0) (by simpa using hnonzero)
  simpa using h

end PFSH02.DualBinding
