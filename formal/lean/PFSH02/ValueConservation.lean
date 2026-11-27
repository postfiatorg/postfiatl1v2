import Mathlib.Data.Finset.Basic
import Mathlib.Algebra.BigOperators.Group.Finset.Basic
import Mathlib.Tactic

/-!
# Theorem 3: Shielded-Pool & Joint-Ledger Supply Conservation (Unmodified-Oracle Extraction)

Formalizes Section 4.1 (`Theorem 3`, lines 717–804) of
`docs/security/peel-binius-shielded-pq-payments.md`:

Conditioned on valid extracted witnesses for all accepted transactions (from Theorem 2A)
and absence of SHAKE256 `H_Node`/`H_Coin`/`H_A` collisions or `H_Coin` zero-preimages:
- **Theorem 3a (Shielded-Pool Conservation):** Every set of distinct unspent coins with valid
  openings satisfies `∑ v_j + unshielded_acc + fees_acc ≤ supply_minted`.
- **Theorem 3b (Joint Transparent + Shielded Conservation):** Both under burned fees
  (`public_supply + ∑ v_j + fees_acc ≤ total_supply_0`) and under validator/collector-credited
  fees (`public_supply + ∑ v_j ≤ total_supply_0`), total supply never inflates.
-/

namespace PFSH02.ValueConservation

/-- Abstract cryptographic hash interfaces for PFSH02's in-circuit SHAKE256 domain-separated maps. -/
structure HashSpec (Secret Addr Coin SN Blinder : Type*) where
  H_A : Secret → Addr
  H_Coin : Addr → ℕ → Blinder → Coin
  H_SN : Secret → Coin → SN
  emptyCoin : Coin

/-- A canonical opening `(addr, val, blinder)` recorded for a coin commitment. -/
structure CoinOpening (Addr Blinder : Type*) where
  addr : Addr
  val : ℕ
  blinder : Blinder
  deriving DecidableEq

/-- Extracted witness + public fields of an accepted PFSH02 transaction (2 inputs, 2 outputs). -/
structure AcceptedTx (Secret Addr Coin SN Blinder : Type*) where
  -- Input slot 0
  dummy0 : Bool
  inS0 : Secret
  inCoin0 : Coin
  inVal0 : ℕ
  inBlind0 : Blinder
  sn0 : SN
  -- Input slot 1
  dummy1 : Bool
  inS1 : Secret
  inCoin1 : Coin
  inVal1 : ℕ
  inBlind1 : Blinder
  sn1 : SN
  -- Output slots 0, 1
  outCoin0 : Coin
  outOpen0 : CoinOpening Addr Blinder
  outCoin1 : Coin
  outOpen1 : CoinOpening Addr Blinder
  -- Public value flows
  v_mint : ℕ
  v_unshield : ℕ
  fee : ℕ

/-- Consensus & circuit validity predicates guaranteed by Rules R1–R6 + extracted witness `w ∈ R_pay(x)`
    under no `H_Node` collision and no `H_Coin` zero-preimage. -/
structure ValidStep {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (coins : Finset Coin)
    (spentSN : Finset SN)
    (tx : AcceptedTx Secret Addr Coin SN Blinder) : Prop where
  /-- Rule R2: the two transaction nullifiers are distinct. -/
  sn_distinct : tx.sn0 ≠ tx.sn1
  /-- Rule R3: neither nullifier was previously spent. -/
  sn0_fresh : tx.sn0 ∉ spentSN
  sn1_fresh : tx.sn1 ∉ spentSN
  /-- Rule R4: output coins are distinct, non-zero, and fresh. -/
  out_distinct : tx.outCoin0 ≠ tx.outCoin1
  out0_fresh : tx.outCoin0 ∉ coins
  out1_fresh : tx.outCoin1 ∉ coins
  out0_commit : tx.outCoin0 = H.H_Coin tx.outOpen0.addr tx.outOpen0.val tx.outOpen0.blinder
  out1_commit : tx.outCoin1 = H.H_Coin tx.outOpen1.addr tx.outOpen1.val tx.outOpen1.blinder
  /-- C3 dummy gating: dummy inputs carry value 0. -/
  dummy0_zero : tx.dummy0 = true → tx.inVal0 = 0
  dummy1_zero : tx.dummy1 = true → tx.inVal1 = 0
  /-- Real inputs open to existing coins in `coins` (from C4 Merkle membership + no `H_Node` coll / no zero-preimage). -/
  real0_mem : tx.dummy0 = false → tx.inCoin0 ∈ coins
  real1_mem : tx.dummy1 = false → tx.inCoin1 ∈ coins
  real0_commit : tx.dummy0 = false → tx.inCoin0 = H.H_Coin (H.H_A tx.inS0) tx.inVal0 tx.inBlind0
  real1_commit : tx.dummy1 = false → tx.inCoin1 = H.H_Coin (H.H_A tx.inS1) tx.inVal1 tx.inBlind1
  real0_sn : tx.dummy0 = false → tx.sn0 = H.H_SN tx.inS0 tx.inCoin0
  real1_sn : tx.dummy1 = false → tx.sn1 = H.H_SN tx.inS1 tx.inCoin1
  /-- Z3-verified 66-bit exact conservation in `ℕ` / `ℤ`. -/
  conservation : tx.inVal0 + tx.inVal1 + tx.v_mint =
    tx.outOpen0.val + tx.outOpen1.val + tx.fee + tx.v_unshield

/-- Abstract shielded-pool ledger state tracked along the induction. -/
structure PoolState (Secret Addr Coin SN Blinder : Type*) [DecidableEq Coin] [DecidableEq SN] where
  coins : Finset Coin
  live : Finset Coin
  spentSN : Finset SN
  openMap : Coin → Option (CoinOpening Addr Blinder)
  /-- Records the spending secret of every coin already spent (for the `H_A` collision check). -/
  spentBy : Coin → Option Secret
  supplyMinted : ℕ
  unshieldedAcc : ℕ
  feesAcc : ℕ

/-- Initial empty genesis state (Rule R0). -/
def genesisState {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN] :
    PoolState Secret Addr Coin SN Blinder :=
  { coins := ∅
    live := ∅
    spentSN := ∅
    openMap := fun _ => none
    spentBy := fun _ => none
    supplyMinted := 0
    unshieldedAcc := 0
    feesAcc := 0 }

/-- Canonical value of a coin from `openMap`. -/
def coinVal {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN]
    (st : PoolState Secret Addr Coin SN Blinder) (c : Coin) : ℕ :=
  match st.openMap c with
  | some o => o.val
  | none => 0

/-- Inductive invariant maintained at every block / transaction step `t = 0..q_tx`. -/
structure PoolInvariant {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (st : PoolState Secret Addr Coin SN Blinder) : Prop where
  live_sub_coins : st.live ⊆ st.coins
  open_valid : ∀ c ∈ st.coins, ∃ o, st.openMap c = some o ∧ c = H.H_Coin o.addr o.val o.blinder
  spent_witness : ∀ c ∈ st.coins, c ∉ st.live →
    ∃ s o, st.spentBy c = some s ∧ st.openMap c = some o ∧
      H.H_A s = o.addr ∧ H.H_SN s c ∈ st.spentSN
  balance_eq : (∑ c ∈ st.live, coinVal st c) + st.unshieldedAcc + st.feesAcc = st.supplyMinted

/-- Collision-freeness hypothesis on `H_Coin` and `H_A` (exact cryptographic bad events of Theorem 3). -/
structure CollisionFree {Secret Addr Coin SN Blinder : Type*}
    (H : HashSpec Secret Addr Coin SN Blinder) : Prop where
  H_Coin_inj : ∀ a1 v1 r1 a2 v2 r2,
    H.H_Coin a1 v1 r1 = H.H_Coin a2 v2 r2 → a1 = a2 ∧ v1 = v2 ∧ r1 = r2
  H_A_inj : ∀ s1 s2, H.H_A s1 = H.H_A s2 → s1 = s2

/-- Genesis satisfies the pool invariant trivially. -/
theorem genesis_invariant {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder) :
    PoolInvariant H (genesisState : PoolState Secret Addr Coin SN Blinder) := by
  refine ⟨Finset.Subset.refl _, ?_, ?_, ?_⟩
  · intro c hc; simp [genesisState] at hc
  · intro c hc; simp [genesisState] at hc
  · simp [genesisState]

/-- Apply one accepted transaction to the pool state. -/
def stepState {Secret Addr Coin SN Blinder : Type*} [DecidableEq Coin] [DecidableEq SN]
    (st : PoolState Secret Addr Coin SN Blinder)
    (tx : AcceptedTx Secret Addr Coin SN Blinder) :
    PoolState Secret Addr Coin SN Blinder :=
  let live1 := if tx.dummy0 = false then st.live.erase tx.inCoin0 else st.live
  let live2 := if tx.dummy1 = false then live1.erase tx.inCoin1 else live1
  let live3 := insert tx.outCoin1 (insert tx.outCoin0 live2)
  { coins := insert tx.outCoin1 (insert tx.outCoin0 st.coins)
    live := live3
    spentSN := insert tx.sn1 (insert tx.sn0 st.spentSN)
    openMap := fun c =>
      if c = tx.outCoin1 then some tx.outOpen1
      else if c = tx.outCoin0 then some tx.outOpen0
      else st.openMap c
    spentBy := fun c =>
      if tx.dummy1 = false ∧ c = tx.inCoin1 then some tx.inS1
      else if tx.dummy0 = false ∧ c = tx.inCoin0 then some tx.inS0
      else st.spentBy c
    supplyMinted := st.supplyMinted + tx.v_mint
    unshieldedAcc := st.unshieldedAcc + tx.v_unshield
    feesAcc := st.feesAcc + tx.fee }

/-- Key inductive step of Theorem 3: under `CollisionFree H`, every valid accepted transaction
    preserves `PoolInvariant H` exactly. -/
theorem step_preserves_invariant {Secret Addr Coin SN Blinder : Type*}
    [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (hCF : CollisionFree H)
    (st : PoolState Secret Addr Coin SN Blinder)
    (hInv : PoolInvariant H st)
    (tx : AcceptedTx Secret Addr Coin SN Blinder)
    (hStep : ValidStep H st.coins st.spentSN tx) :
    PoolInvariant H (stepState st tx) := by
  -- Helper: any real input must currently be in `st.live` and carry its canonical value.
  have hreal0 : tx.dummy0 = false →
      tx.inCoin0 ∈ st.live ∧ coinVal st tx.inCoin0 = tx.inVal0 := by
    intro hd0
    have hmem := hStep.real0_mem hd0
    obtain ⟨o, hopen, hcom⟩ := hInv.open_valid tx.inCoin0 hmem
    have hcom' := hStep.real0_commit hd0
    have ⟨ha, hv, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans hcom')
    have hlive : tx.inCoin0 ∈ st.live := by
      by_contra hnot
      obtain ⟨s', o', _, hopen', ha', hsn'⟩ := hInv.spent_witness tx.inCoin0 hmem hnot
      rw [hopen] at hopen'
      cases hopen'
      have hs_eq : s' = tx.inS0 := hCF.H_A_inj s' tx.inS0 (ha'.trans ha)
      subst hs_eq
      rw [← hStep.real0_sn hd0] at hsn'
      exact hStep.sn0_fresh hsn'
    refine ⟨hlive, ?_⟩
    simp [coinVal, hopen, hv]
  have hreal1 : tx.dummy1 = false →
      tx.inCoin1 ∈ st.live ∧ coinVal st tx.inCoin1 = tx.inVal1 := by
    intro hd1
    have hmem := hStep.real1_mem hd1
    obtain ⟨o, hopen, hcom⟩ := hInv.open_valid tx.inCoin1 hmem
    have hcom' := hStep.real1_commit hd1
    have ⟨ha, hv, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans hcom')
    have hlive : tx.inCoin1 ∈ st.live := by
      by_contra hnot
      obtain ⟨s', o', _, hopen', ha', hsn'⟩ := hInv.spent_witness tx.inCoin1 hmem hnot
      rw [hopen] at hopen'
      cases hopen'
      have hs_eq : s' = tx.inS1 := hCF.H_A_inj s' tx.inS1 (ha'.trans ha)
      subst hs_eq
      rw [← hStep.real1_sn hd1] at hsn'
      exact hStep.sn1_fresh hsn'
    refine ⟨hlive, ?_⟩
    simp [coinVal, hopen, hv]
  -- If both inputs are real, they must be distinct coins (otherwise same secret -> sn0 = sn1).
  have hreal_distinct : tx.dummy0 = false → tx.dummy1 = false → tx.inCoin0 ≠ tx.inCoin1 := by
    intro hd0 hd1 heq
    have h0 := hStep.real0_commit hd0
    have h1 := hStep.real1_commit hd1
    rw [heq] at h0
    have ⟨ha, _, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (h0.symm.trans h1)
    have hs := hCF.H_A_inj tx.inS0 tx.inS1 ha
    have hsn : tx.sn0 = tx.sn1 := by
      rw [hStep.real0_sn hd0, hStep.real1_sn hd1, hs, heq]
    exact hStep.sn_distinct hsn
  -- Establish the 4 invariant fields for `stepState st tx`.
  set st' := stepState st tx
  set live1 := if tx.dummy0 = false then st.live.erase tx.inCoin0 else st.live
  set live2 := if tx.dummy1 = false then live1.erase tx.inCoin1 else live1
  have hlive1_sub : live1 ⊆ st.live := by
    dsimp [live1]; split_ifs <;> [exact Finset.erase_subset _ _; exact Finset.Subset.refl _]
  have hlive2_sub : live2 ⊆ st.live := by
    dsimp [live2]; split_ifs
    · exact (Finset.erase_subset _ _).trans hlive1_sub
    · exact hlive1_sub
  have hout0_not_live2 : tx.outCoin0 ∉ live2 :=
    fun h => hStep.out0_fresh (hInv.live_sub_coins (hlive2_sub h))
  have hout1_not_live2 : tx.outCoin1 ∉ live2 :=
    fun h => hStep.out1_fresh (hInv.live_sub_coins (hlive2_sub h))
  have hopen_eq_live2 : ∀ c ∈ live2, coinVal st' c = coinVal st c := by
    intro c hc
    have hmem_coins := hInv.live_sub_coins (hlive2_sub hc)
    have hne0 : c ≠ tx.outCoin0 := fun h => hStep.out0_fresh (h ▸ hmem_coins)
    have hne1 : c ≠ tx.outCoin1 := fun h => hStep.out1_fresh (h ▸ hmem_coins)
    simp [coinVal, st', stepState, hne0, hne1]
  refine ⟨?_, ?_, ?_, ?_⟩
  · -- live' ⊆ coins'
    intro c hc
    simp only [st', stepState, Finset.mem_insert] at hc ⊢
    rcases hc with rfl | rfl | hc2
    · exact Or.inl rfl
    · exact Or.inr (Or.inl rfl)
    · exact Or.inr (Or.inr (hInv.live_sub_coins (hlive2_sub hc2)))
  · -- open_valid
    intro c hc
    simp only [st', stepState, Finset.mem_insert] at hc
    rcases hc with rfl | rfl | hcold
    · exact ⟨tx.outOpen1, by simp [st', stepState], hStep.out1_commit⟩
    · exact ⟨tx.outOpen0, by simp [st', stepState, hStep.out_distinct], hStep.out0_commit⟩
    · have hne0 : c ≠ tx.outCoin0 := fun h => hStep.out0_fresh (h ▸ hcold)
      have hne1 : c ≠ tx.outCoin1 := fun h => hStep.out1_fresh (h ▸ hcold)
      obtain ⟨o, ho, hcom⟩ := hInv.open_valid c hcold
      exact ⟨o, by simp [st', stepState, hne0, hne1, ho], hcom⟩
  · -- spent_witness
    intro c hc hnotlive
    simp only [st', stepState, Finset.mem_insert] at hc hnotlive
    have hne1 : c ≠ tx.outCoin1 := fun h => hnotlive (Or.inl h)
    have hne0 : c ≠ tx.outCoin0 := fun h => hnotlive (Or.inr (Or.inl h))
    have hcold : c ∈ st.coins := by
      rcases hc with h | h | h
      · exact absurd h hne1
      · exact absurd h hne0
      · exact h
    have hnotlive2 : c ∉ live2 := fun h => hnotlive (Or.inr (Or.inr h))
    by_cases h1 : tx.dummy1 = false ∧ c = tx.inCoin1
    · rcases h1 with ⟨hd1, rfl⟩
      obtain ⟨o, ho, hcom⟩ := hInv.open_valid tx.inCoin1 hcold
      have ⟨ha, _, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans (hStep.real1_commit hd1))
      refine ⟨tx.inS1, o, ?_, ?_, ha.symm, ?_⟩
      · simp [st', stepState, hd1]
      · simp [st', stepState, hne0, hne1, ho]
      · rw [← hStep.real1_sn hd1]; simp [st', stepState]
    · by_cases h0 : tx.dummy0 = false ∧ c = tx.inCoin0
      · rcases h0 with ⟨hd0, rfl⟩
        obtain ⟨o, ho, hcom⟩ := hInv.open_valid tx.inCoin0 hcold
        have ⟨ha, _, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans (hStep.real0_commit hd0))
        refine ⟨tx.inS0, o, ?_, ?_, ha.symm, ?_⟩
        · simp [st', stepState, h1, hd0]
        · simp [st', stepState, hne0, hne1, ho]
        · rw [← hStep.real0_sn hd0]; simp [st', stepState]
      · -- Must have been spent previously
        have hnotlive_old : c ∉ st.live := by
          intro hclive
          apply hnotlive2
          dsimp [live2, live1]
          split_ifs with hd1 hd0 hd0
          · rw [Finset.mem_erase, Finset.mem_erase]
            exact ⟨fun hc1 => h1 ⟨hd1, hc1⟩, fun hc0 => h0 ⟨hd0, hc0⟩, hclive⟩
          · rw [Finset.mem_erase]
            exact ⟨fun hc1 => h1 ⟨hd1, hc1⟩, hclive⟩
          · rw [Finset.mem_erase]
            exact ⟨fun hc0 => h0 ⟨hd0, hc0⟩, hclive⟩
          · exact hclive
        obtain ⟨s, o, hs, ho, ha, hsn⟩ := hInv.spent_witness c hcold hnotlive_old
        refine ⟨s, o, ?_, ?_, ha, ?_⟩
        · simp [st', stepState, h1, h0, hs]
        · simp [st', stepState, hne0, hne1, ho]
        · simp [st', stepState, hsn]
  · -- balance_eq: exact accounting step!
    have hsum1 : (∑ c ∈ live1, coinVal st c) + tx.inVal0 = ∑ c ∈ st.live, coinVal st c := by
      dsimp [live1]
      split_ifs with hd0
      · obtain ⟨hmem0, hval0⟩ := hreal0 hd0
        rw [← hval0, Finset.sum_erase_add _ _ hmem0]
      · have hd0t : tx.dummy0 = true := Bool.eq_true_of_not_eq_false hd0
        rw [hStep.dummy0_zero hd0t, add_zero]
    have hsum2 : (∑ c ∈ live2, coinVal st c) + tx.inVal1 = ∑ c ∈ live1, coinVal st c := by
      dsimp [live2]
      split_ifs with hd1
      · obtain ⟨hmem1, hval1⟩ := hreal1 hd1
        have hmem1' : tx.inCoin1 ∈ live1 := by
          dsimp [live1]
          split_ifs with hd0
          · rw [Finset.mem_erase]
            exact ⟨(hreal_distinct hd0 hd1).symm, hmem1⟩
          · exact hmem1
        rw [← hval1, Finset.sum_erase_add _ _ hmem1']
      · have hd1t : tx.dummy1 = true := Bool.eq_true_of_not_eq_false hd1
        rw [hStep.dummy1_zero hd1t, add_zero]
    have hval_out0 : coinVal st' tx.outCoin0 = tx.outOpen0.val := by
      simp [coinVal, st', stepState, hStep.out_distinct]
    have hval_out1 : coinVal st' tx.outCoin1 = tx.outOpen1.val := by
      simp [coinVal, st', stepState]
    have hsum_new :
        ∑ c ∈ st'.live, coinVal st' c =
          tx.outOpen1.val + tx.outOpen0.val + ∑ c ∈ live2, coinVal st c := by
      change ∑ c ∈ insert tx.outCoin1 (insert tx.outCoin0 live2), coinVal st' c = _
      have hout1_not_ins : tx.outCoin1 ∉ insert tx.outCoin0 live2 := by
        rw [Finset.mem_insert]
        exact not_or.mpr ⟨hStep.out_distinct.symm, hout1_not_live2⟩
      rw [Finset.sum_insert hout1_not_ins, Finset.sum_insert hout0_not_live2,
        hval_out1, hval_out0, Finset.sum_congr rfl hopen_eq_live2]
      ring
    have hbal_old := hInv.balance_eq
    have hcons := hStep.conservation
    dsimp only [st', stepState] at hsum_new ⊢
    linarith

/-- **Theorem 3a (Shielded-Pool Value Conservation):**
    Under `PoolInvariant H st` and `CollisionFree H`, ANY finite set `U ⊆ st.coins` of distinct
    unspent coins with valid adversary-supplied openings `(S_j, v_j, R_j)` satisfies
    `(∑ c ∈ U, v_j) + st.unshieldedAcc + st.feesAcc ≤ st.supplyMinted`. -/
theorem theorem3a_shielded_conservation {Secret Addr Coin SN Blinder : Type*}
    [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (hCF : CollisionFree H)
    (st : PoolState Secret Addr Coin SN Blinder)
    (hInv : PoolInvariant H st)
    (U : Finset Coin)
    (hU_sub : U ⊆ st.coins)
    (openS : Coin → Secret)
    (openV : Coin → ℕ)
    (openR : Coin → Blinder)
    (hU_commit : ∀ c ∈ U, c = H.H_Coin (H.H_A (openS c)) (openV c) (openR c))
    (hU_unspent : ∀ c ∈ U, H.H_SN (openS c) c ∉ st.spentSN) :
    (∑ c ∈ U, openV c) + st.unshieldedAcc + st.feesAcc ≤ st.supplyMinted := by
  -- Every coin in `U` must belong to `st.live` and have `openV c = coinVal st c`.
  have hU_live : U ⊆ st.live := by
    intro c hcU
    by_contra hnotlive
    obtain ⟨s', o', _, ho', ha', hsn'⟩ := hInv.spent_witness c (hU_sub hcU) hnotlive
    obtain ⟨o, ho, hcom⟩ := hInv.open_valid c (hU_sub hcU)
    rw [ho] at ho'; cases ho'
    have ⟨ha, _, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans (hU_commit c hcU))
    have hs_eq : s' = openS c := hCF.H_A_inj s' (openS c) (ha'.trans ha)
    subst hs_eq
    exact hU_unspent c hcU hsn'
  have hU_val : ∀ c ∈ U, openV c = coinVal st c := by
    intro c hcU
    obtain ⟨o, ho, hcom⟩ := hInv.open_valid c (hU_sub hcU)
    have ⟨_, hv, _⟩ := hCF.H_Coin_inj _ _ _ _ _ _ (hcom.symm.trans (hU_commit c hcU))
    simp [coinVal, ho, hv]
  have hsum_le : ∑ c ∈ U, openV c ≤ ∑ c ∈ st.live, coinVal st c := by
    rw [Finset.sum_congr rfl hU_val]
    exact Finset.sum_le_sum_of_subset hU_live
  linarith [hInv.balance_eq]

/-- **Theorem 3b (Joint Transparent + Shielded Conservation):**
    - Burned-fee mode (`publicBal + supplyMinted = totalSupply0 + unshieldedAcc`):
      `publicBal + (∑ c ∈ U, openV c) + feesAcc ≤ totalSupply0`.
    - Collector/validator-credited fee mode (`publicBal + supplyMinted = totalSupply0 + unshieldedAcc + feesAcc`):
      `publicBal + (∑ c ∈ U, openV c) ≤ totalSupply0`. -/
theorem theorem3b_joint_conservation_burned {Secret Addr Coin SN Blinder : Type*}
    [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (hCF : CollisionFree H)
    (st : PoolState Secret Addr Coin SN Blinder)
    (hInv : PoolInvariant H st)
    (publicBal totalSupply0 : ℕ)
    (hL1 : publicBal + st.supplyMinted = totalSupply0 + st.unshieldedAcc)
    (U : Finset Coin) (hU_sub : U ⊆ st.coins)
    (openS : Coin → Secret) (openV : Coin → ℕ) (openR : Coin → Blinder)
    (hU_commit : ∀ c ∈ U, c = H.H_Coin (H.H_A (openS c)) (openV c) (openR c))
    (hU_unspent : ∀ c ∈ U, H.H_SN (openS c) c ∉ st.spentSN) :
    publicBal + (∑ c ∈ U, openV c) + st.feesAcc ≤ totalSupply0 := by
  have h3a := theorem3a_shielded_conservation H hCF st hInv U hU_sub openS openV openR hU_commit hU_unspent
  omega

theorem theorem3b_joint_conservation_credited {Secret Addr Coin SN Blinder : Type*}
    [DecidableEq Coin] [DecidableEq SN]
    (H : HashSpec Secret Addr Coin SN Blinder)
    (hCF : CollisionFree H)
    (st : PoolState Secret Addr Coin SN Blinder)
    (hInv : PoolInvariant H st)
    (publicBal totalSupply0 : ℕ)
    (hL1 : publicBal + st.supplyMinted = totalSupply0 + st.unshieldedAcc + st.feesAcc)
    (U : Finset Coin) (hU_sub : U ⊆ st.coins)
    (openS : Coin → Secret) (openV : Coin → ℕ) (openR : Coin → Blinder)
    (hU_commit : ∀ c ∈ U, c = H.H_Coin (H.H_A (openS c)) (openV c) (openR c))
    (hU_unspent : ∀ c ∈ U, H.H_SN (openS c) c ∉ st.spentSN) :
    publicBal + (∑ c ∈ U, openV c) ≤ totalSupply0 := by
  have h3a := theorem3a_shielded_conservation H hCF st hInv U hU_sub openS openV openR hU_commit hU_unspent
  omega

end PFSH02.ValueConservation
