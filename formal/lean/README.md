# PFSH02 Lean 4 Formal Proofs

Machine-checked Lean 4 (`v4.20.0`) + Mathlib proofs covering the mathematical core of
**Peel-Binius-Shielded (PFSH02)** (`docs/security/peel-binius-shielded-pq-payments.md`).

Every theorem compiles with **zero `sorry`s** and **zero custom axioms**—depending solely on Lean's
standard foundational axioms (`propext`, `Classical.choice`, `Quot.sound`).

## Module Map

| Module | Spec Target | Key Machine-Checked Theorems |
| --- | --- | --- |
| `PFSH02/Params.lean` | `check_math.py`, Tables 1–2 | `L_max_exact` (`⌊√(2/ρ)⌋ = 86`), `johnson_margin_m3` (`7/6 < √2`), `coeff_3652_upper`, `wire_margins_exact`, `pfsh03_wire_margins_exact`, `conservation_66bit_no_overflow`, `p34_lt_inv_2_302`, `eps_curve_sq_lt_inv_2_610`, `theorem2A_Q64_lt_inv_2_170`, `theorem2A_Q80_lt_inv_2_137`, `theorem2A_Q96_lt_inv_2_92` |
| `PFSH02/Peel.lean` | Appendix A.1, Lemma 1 (`Peel(M)`) | `step_sqrt_decrease`, `peel_sqrt_telescoping`, `peel_sqrt_telescoping_strict`, `peel_kD_le`, `peel_kD_lt`, `pfsh02_peel_list_bound` (`k ≤ 86`), `peel_case1_agreement_strict`, `peel_case2_bciks_hypotheses`, `peel_case2_curve_monotone`, `peel_case2_agreement_strict` |
| `PFSH02/DualBinding.lean` | Appendix A.1, Lemmas 2, 3 & 4 | `eq_of_agree_on_finset`, `card_vanishing_le_deg`, `lemma2_bad_pt_union_bound`, `lemma2_dual_binding`, `lemma3_subfield_inheritance` (Lagrange `K_32` subfield inheritance), `lemma4_gamma_soundness`, `lemma4_rho_rs_soundness` |
| `PFSH02/Rank3Op.lean` | Appendix A.3, Lemma A.3.0 & A.3.4(b) | `opO_rank3_identity`, `opO_symmetric`, `entry_norm_bound` (`≤ √(8p)`), `exit_norm_bound` (`≤ √(8p)`), `readout_error_exact` (`2/N - 1/N² < 2/N`) |
| `PFSH02/CompressedOracle.lean` | Appendix A.3, Lemmas A.3.1–A.3.4 & Theorem 2A | `blockDiag_norm_le` (superposition reduction), `pointwise_both_directions_bound`, `preimage_unique_of_no_collision`, `lemmaA31_tree_invariance`, `sum_sqrt_le_two_thirds` (`∑√q ≤ (2/3)(Q+1)^(3/2)`), `amplitude_telescoping`, `theorem2A_telescoped_bound` |
| `PFSH02/ZeroKnowledge.lean` | Section 3.4 & Appendix B (Lemmas 1.1–1.2, Theorem 1B) | `lemma11_unopened_leaf_bijection` (exact `5,216`-bit marginal min-entropy), `one_sub_pow_le_exp`, `lemma12_pow_tv_bound` (`TV < 2^-182`), `sim_iop_bijections`, `theorem1B_Q96_bound` |
| `PFSH02/ValueConservation.lean` | Section 4.1, Theorem 3 (3a & 3b) | `genesis_invariant`, `step_preserves_invariant`, `theorem3a_shielded_conservation`, `theorem3b_joint_conservation_burned`, `theorem3b_joint_conservation_credited` |
| `PFSH02/ConjectureA4.lean` | Appendix A.4, Theorem 2B (SE wrapper `B^H`) | `monotone_exit_norm_zero`, `theoremA4_E3_bound` (`≤ 8 Q_eff² p_E3 ≤ 10 Q_eff² p_E3`), `escape_permanence`, `appendixA4_caseA`, `appendixA4_caseB`, `appendixA4_caseC`, `appendixA4_exhaustive_trichotomy` |
| `PFSH02/AuthAndPrivacy.lean` | Sections 4.2–4.3, Theorems 4 & 5 (+ PFSH03) | `mode3_key_pinned_eliminates_160bit`, `mode3_forge_dichotomy`, `theorem4_hybrid_bound`, `symmetric_five_game_hybrid`, `theorem5_global_privacy` (incl. GMP22 sub-hop `(v)`), **`pfsh03_spend_auth_dichotomy`**, **`pfsh03_theorem4_zero_appA4`** (ZERO Appendix A.4 / ZERO QROM-ZK / ZERO `JointPRF`!), **`pfsh03_theorem5_multitx_hybrid`** (`q_c`-tx adaptive hybrid) |

## Reproduce Locally

```bash
cd formal/lean
lake exe cache get   # fetches prebuilt Mathlib v4.20.0 oleans
lake build PFSH02
```

## Honest Scope Boundary

This formalization verifies the algebraic, combinatorial, Hilbert-space operator-norm,
compressed-oracle telescoping, transcript-overlay case split (Cases A/B/C), and ledger-accounting
steps of the specification with Lean's kernel. External cryptographic review is still required for:
1. **Model-to-code fidelity** (confirming Lean definitions match the Rust/Go implementations and full
   68-call transcript/Merkle wiring);
2. **External literature interfaces** (BCIKS20 Theorem 6.2, GHHM21 Theorem 1, AHU19 Theorem 3, and
   connecting real `Sim_BCS` unopened leaves to wrapper `B^H`);
3. **Non-standard primitive assumptions** (`JointPRF`, `PRG`, `Hide`, FIPS-203 `IK-CCA`/`IR-PRF`) and
   extending Theorem 5 beyond its single-transaction challenge model.
