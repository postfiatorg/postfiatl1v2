import Mathlib.Tactic

/-!
# Theorems 4 & 5: Deposit Key-Pinning, Spend Authorization, and 5-Game Privacy Hybrid

Formalizes Sections 4.2 & 4.3 of `docs/security/peel-binius-shielded-pq-payments.md`
(lines 805–960):

1. **Theorem 4, Mode 3 (On-Chain ML-DSA-65 Public-Key Pinning):**
   Models `prepare_pq_account_debit` (`crates/execution/src/entrypoints.rs:2353-2364`):
   - When `account.public_key_hex = some pk*` is already pinned on-chain, any accepted deposit
     debiting that account MUST present `pk = pk*` (ZERO 160-bit address-preimage exposure;
     reduces strictly to SHA-384 CR + ML-DSA-65 EUF-CMA).
   - Only when `account.public_key_hex = none` (unpinned brand-new account) can an attacker
     substitute `pk ≠ pk*` with `addr160(pk) = addr160(pk*)` (160-bit second-preimage exposure).
2. **Theorem 4, Modes 1–2 (2-Hop Spend-Authorization Reduction Accounting):**
   Proves the exact triangle-inequality hybrid composition (`H_0 → H_1 → H_2` + Mode-1/Mode-2
   extraction split) ensuring zero missing terms.
3. **Theorem 5 (5-Game Single-Transaction Privacy Hybrid incl. GMP22 Sub-Hop `(v)`):**
   Proves the exact symmetric hybrid bound `|p_0(0) - p_0(1)| ≤ 2 * (h1 + h2 + h3 + h4 + h5)`
   when the terminal game satisfies `p_5(0) = p_5(1)`, with `h2` explicitly decomposed into all
   five sub-hops `(i)–(v)` (including sub-hop `(v)` closing the GMP22 implicit-rejection
   non-recipient scanning asymmetry in `H_4`), matching Theorem 5 verbatim!
-/

namespace PFSH02.AuthAndPrivacy

/-! ## Part 1: Theorem 4, Mode 3 — On-Chain ML-DSA-65 Public-Key Pinning -/

/-- Minimal faithful model of an L1 account's public-key pinning state in
    `prepare_pq_account_debit` (`crates/execution/src/entrypoints.rs`). -/
structure AccountPinState (Addr PubKey : Type*) where
  addr : Addr
  pinnedKey : Option PubKey

/-- Acceptance predicate enforced by `prepare_pq_account_debit` before ML-DSA-65 verification. -/
def debitKeyAccepted {Addr PubKey : Type*}
    (addr160 : PubKey → Addr)
    (acct : AccountPinState Addr PubKey)
    (submittedPK : PubKey) : Prop :=
  addr160 submittedPK = acct.addr ∧
  ∀ pkPinned, acct.pinnedKey = some pkPinned → submittedPK = pkPinned

/-- **Theorem 4, Mode 3 (Zero 160-Bit Exposure for Key-Pinned Accounts):**
    If an account has pinned `pk*` (`acct.pinnedKey = some pk*`) and a deposit debiting `acct`
    is accepted with `submittedPK`, then `submittedPK = pk*` deterministically — NO distinct key
    `pk' ≠ pk*` can ever pass even if `addr160 pk' = addr160 pk*`! -/
theorem mode3_key_pinned_eliminates_160bit {Addr PubKey : Type*}
    (addr160 : PubKey → Addr)
    (acct : AccountPinState Addr PubKey) (pkStar submittedPK : PubKey)
    (hPinned : acct.pinnedKey = some pkStar)
    (hAccepted : debitKeyAccepted addr160 acct submittedPK) :
    submittedPK = pkStar :=
  hAccepted.2 pkStar hPinned

/-- **Theorem 4, Mode 3 (Exact Dichotomy — Pinned vs. Unpinned):**
    For any accepted forged deposit debiting `acct` (with honest owner key `pk*` where
    `addr160 pk* = acct.addr`), either:
    - `submittedPK = pk*` (direct ML-DSA-65 EUF-CMA or SHA-384 digest forgery), OR
    - `acct.pinnedKey = none ∧ submittedPK ≠ pk* ∧ addr160 submittedPK = addr160 pk*`
      (targeted 160-bit address second-preimage on an unpinned brand-new account). -/
theorem mode3_forge_dichotomy {Addr PubKey : Type*}
    (addr160 : PubKey → Addr)
    (acct : AccountPinState Addr PubKey) (pkStar submittedPK : PubKey)
    (hOwner : addr160 pkStar = acct.addr)
    (hAccepted : debitKeyAccepted addr160 acct submittedPK)
    (hPinValid : ∀ pkP, acct.pinnedKey = some pkP → pkP = pkStar) :
    submittedPK = pkStar ∨
      (acct.pinnedKey = none ∧ submittedPK ≠ pkStar ∧ addr160 submittedPK = addr160 pkStar) := by
  by_cases hEq : submittedPK = pkStar
  · exact Or.inl hEq
  · right
    have hNone : acct.pinnedKey = none := by
      cases hP : acct.pinnedKey with
      | none => rfl
      | some pkP =>
        have h1 := hAccepted.2 pkP hP
        have h2 := hPinValid pkP hP
        exact absurd (h1.trans h2) hEq
    exact ⟨hNone, hEq, hAccepted.1.trans hOwner.symm⟩

/-! ## Part 2: Theorem 4, Modes 1 & 2 — Two-Hop Hybrid Reduction Accounting -/

/-- Exact 2-hop hybrid bound for Theorem 4 (Section 4.2, lines 828–876):
    `H_0` (real game) → `H_1` (simulated `O_Hon` proofs via QROM ZK) → `H_2` (uniform nullifiers
    via `JointPRF` + `d_hon * PRG`), followed by Mode-1/2 extraction (`ε_SE`) + Mode-3 forgery. -/
theorem theorem4_hybrid_bound
    (p0 p1 p2
     advZK advJointPRF advPRG epsSE advCR384 advMTTPR advOW advCRCoin advCRNode advZeroPre
     adv2ndPre advEUF : ℝ)
    (d_hon : ℕ)
    (h01 : p0 - p1 ≤ advZK)
    (h12 : p1 - p2 ≤ advJointPRF + (d_hon : ℝ) * advPRG)
    (hH2 : p2 ≤ epsSE + advCR384 + advMTTPR + advOW + advCRCoin + advCRNode + advZeroPre
                + adv2ndPre + advEUF) :
    p0 ≤ advZK + advJointPRF + (d_hon : ℝ) * advPRG
         + epsSE + advCR384 + advMTTPR + advOW + advCRCoin + advCRNode + advZeroPre
         + adv2ndPre + advEUF := by
  linarith

/-! ## Part 3: Theorem 5 — 5-Game Privacy Hybrid Including GMP22 Sub-Hop `(v)` -/

/-- Exact symmetric 5-game hybrid lemma (`H_0` through `H_5` in both worlds `b ∈ {0, 1}`):
    If `|p_k(b) - p_{k+1}(b)| ≤ hop_k` in both worlds `b ∈ {0, 1}`, and the terminal game `H_5`
    is bit-independent (`p_5(0) = p_5(1)`), then
    `|p_0(0) - p_0(1)| ≤ 2 * (hop_1 + hop_2 + hop_3 + hop_4 + hop_5)`. -/
theorem symmetric_five_game_hybrid
    (p0 p1 p2 p3 p4 p5 : Bool → ℝ)
    (hop1 hop2 hop3 hop4 hop5 : ℝ)
    (h1 : ∀ b, |p0 b - p1 b| ≤ hop1)
    (h2 : ∀ b, |p1 b - p2 b| ≤ hop2)
    (h3 : ∀ b, |p2 b - p3 b| ≤ hop3)
    (h4 : ∀ b, |p3 b - p4 b| ≤ hop4)
    (h5 : ∀ b, |p4 b - p5 b| ≤ hop5)
    (hEnd : p5 false = p5 true) :
    |p0 false - p0 true| ≤ 2 * (hop1 + hop2 + hop3 + hop4 + hop5) := by
  have hchain : ∀ b, |p0 b - p5 b| ≤ hop1 + hop2 + hop3 + hop4 + hop5 := by
    intro b
    have t1 := abs_add_le (p0 b - p1 b) (p1 b - p5 b)
    have t2 := abs_add_le (p1 b - p2 b) (p2 b - p5 b)
    have t3 := abs_add_le (p2 b - p3 b) (p3 b - p5 b)
    have t4 := abs_add_le (p3 b - p4 b) (p4 b - p5 b)
    have e1 : (p0 b - p1 b) + (p1 b - p5 b) = p0 b - p5 b := by ring
    have e2 : (p1 b - p2 b) + (p2 b - p5 b) = p1 b - p5 b := by ring
    have e3 : (p2 b - p3 b) + (p3 b - p5 b) = p2 b - p5 b := by ring
    have e4 : (p3 b - p4 b) + (p4 b - p5 b) = p3 b - p5 b := by ring
    rw [e1] at t1; rw [e2] at t2; rw [e3] at t3; rw [e4] at t4
    linarith [h1 b, h2 b, h3 b, h4 b, h5 b]
  have htot := abs_add_le (p0 false - p5 false) (p5 true - p0 true)
  have heq : (p0 false - p5 false) + (p5 true - p0 true) = p0 false - p0 true := by
    rw [hEnd]; ring
  rw [heq, abs_sub_comm (p5 true) (p0 true)] at htot
  linarith [hchain false, hchain true]

/-- **Theorem 5 (Complete Concrete Privacy Bound With GMP22 Sub-Hop `(v)`):**
    Instantiates `symmetric_five_game_hybrid` with PFSH02's exact 5 games:
    - `H_0 → H_1`: QROM ZK (`advZK`)
    - `H_1 → H_2` (sub-hops `(i)`–`(v)` for the 2 challenge envelopes, including sub-hop `(v)`
      closing the GMP22 implicit-rejection scanning asymmetry):
      `2 * U * (advINDCCA + advIKCCA) + 2 * (advIRPRF + 2 * advPRF + 2 * advINTCTXT + advINDCPA)`
    - `H_2 → H_3`: challenge nullifiers (`r * U * advJointPRF + d * advPRG`)
    - `H_3 → H_4`: challenge commitments (`2 * advHide`)
    - `H_4 → H_5`: negligible R3/R4 abort collision (`abortBound`)
    giving Theorem 5 verbatim (`lines 945–955` of the spec)! -/
theorem theorem5_global_privacy
    (p0 p1 p2 p3 p4 p5 : Bool → ℝ)
    (U r d : ℕ)
    (advZK advINDCCA advIKCCA advIRPRF advPRF advINTCTXT advINDCPA
     advJointPRF advPRG advHide abortBound : ℝ)
    (h1 : ∀ b, |p0 b - p1 b| ≤ advZK)
    (h2 : ∀ b, |p1 b - p2 b| ≤
      2 * (U : ℝ) * (advINDCCA + advIKCCA)
      + 2 * (advIRPRF + 2 * advPRF + 2 * advINTCTXT + advINDCPA))
    (h3 : ∀ b, |p2 b - p3 b| ≤ (r : ℝ) * (U : ℝ) * advJointPRF + (d : ℝ) * advPRG)
    (h4 : ∀ b, |p3 b - p4 b| ≤ 2 * advHide)
    (h5 : ∀ b, |p4 b - p5 b| ≤ abortBound)
    (hEnd : p5 false = p5 true) :
    |p0 false - p0 true| ≤
      2 * (advZK
        + 2 * (U : ℝ) * (advINDCCA + advIKCCA)
        + 2 * (advIRPRF + 2 * advPRF + 2 * advINTCTXT + advINDCPA)
        + (r : ℝ) * (U : ℝ) * advJointPRF
        + (d : ℝ) * advPRG
        + 2 * advHide
        + abortBound) := by
  have h := symmetric_five_game_hybrid p0 p1 p2 p3 p4 p5 _ _ _ _ _ h1 h2 h3 h4 h5 hEnd
  linarith

end PFSH02.AuthAndPrivacy
