# pfUSDC Ethereum Mainnet Cutover Criteria

> **Status amendment (2026-07-30):** The direct Ethereum-mainnet pfUSDC lane
> subsequently passed a complete `1 USDC` round trip in `20m12s`, with exact
> conservation and replay rejection, and became the canonical replacement for
> the commercially unusable Arbitrum route. The rail was later used by the
> A666 transparent and private mainnet flows. See
> [the campaign handoff](PFUSDC-MAINNET-CAMPAIGN-HANDOFF-20260726.md) and
> [A666 current state](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).

**Date:** 2026-07-25
**Author:** Angmar (Nazgul command)
**Status:** DRAFT — awaiting Sauron sign-off on Section 3
**Parent spec:** `A666-MAINNET-TRUSTLESS-MINT-SPEC-20260725.md` (WS4)
**Scope:** pfUSDC rails only. No navswaps, no a666 bands, no Arbitrum — ever.

## 1. Gate: Sepolia rehearsal must be green first

All four legs in one scripted, re-runnable E2E, with evidence:

| Leg | Acceptance | Evidence required |
|---|---|---|
| Ingress | MetaMask-initiated USDC deposit -> SP1 ingress proof (Helios + storage slots) -> pfUSDC credited directly spendable at finalized claim, visible in wallet UX | proof artifacts, credit receipt, balance delta, direct-spendable + replay regression |
| Transparent send | pfUSDC A->B, exact balance deltas both sides | tx receipt, before/after balances |
| Orchard send | pfUSDC A->C shielded, nullifier recorded, C receives exact amount | nullifier evidence, before/after balances |
| Egress | burn -> exit leaf in finalized block -> SP1 finality proof -> batch exit root -> Merkle claim -> USDC to a different recipient | proof artifacts, claim tx, USDC balance delta |

Ingress notes (verified in `crates/execution/src/nav_vault_asset_execution.rs`):

- The Ethereum route is **self-checkpointing**: cap growth
  (`circulating_supply`, `finalized_epoch`, reserve packet hash) is applied
  atomically with the proof-backed claim
  (`SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1`). Manual headroom
  checkpoints are for non-proof-backed exceptions only.
- Ethereum-finality claims credit **directly spendable** atoms
  (`lifecycle_release_required=false`): claim recipient/amount are bound to
  finalized vault evidence, the deposit nullifier prevents replay, and no
  issuer/operator release step exists. The ingress KPI ("minutes not days")
  is measured to spendable at finalized credit. At least one transfer leg
  must spend the newly credited ingress atoms, not pre-existing inventory.
- The escrowed-provisional lifecycle (guard
  `pfusdc_fast_ingress_escrow_not_releasable`) is scoped exclusively to
  `NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1` claims (lines 1911-1953).
  That branch is unreachable on this chain: no Arbitrum route profile is
  ever registered, per standing directive. Recorded here only as contrast.
- Trust-model finding (resolved, positive): ingress on the Ethereum route is
  operator-independent end to end — consistent with the unilateral-exit
  invariant. No open release-authorization question remains.

Cross-cutting: conservation identity `V = S + D + B - R` verified at start and
end of the run; per-stage wall-clock table; ingress and egress proof cost
recorded (guest cycles, proving wall-clock, peak RAM). Egress proving must be
production-viable: bounded segment via fresh verifier checkpoints, measured
under 30 min wall-clock on available hardware, or a recorded checkpoint
cadence plan that achieves it.

## 2. Gate: mainnet deployment discipline

1. ELF/vkey hashes for ingress and egress programs frozen and recorded in the
   deployment manifest; mainnet contracts deploy against those exact hashes.
2. `PFTLFinalityVerifierV1` wired to the canonical Succinct SP1 verifier
   gateway on mainnet; batch-exits root consumption; replay protection.
3. Ethereum mainnet route profile registered on PFTL; pfUSDC bound BEFORE
   route activation height. No observer/mock fallback; no-downgrade.
4. Vault asset is canonical mainnet USDC (Circle). Issuer-freeze risk noted
   per WS10: no proof system mitigates an administrative freeze; concentration
   limits and `nav_halt` deadman path documented before activation.
5. Controlled-size first run: bounded deposit, full four-leg round trip,
   different-recipient withdrawal, before any size increase.

## 3. Requires explicit Sauron sign-off (blocking mainnet, not Sepolia)

| # | Item | Needed value |
|---|---|---|
| 1 | Deployment gas funding | ETH amount + funding address |
| 2 | SP1 prover operations | who runs proving, hardware budget, cadence |
| 3 | Audit scope | which mainnet contracts, internal vs external, timing |
| 4 | Controlled-run size | USDC amount for the first live round trip |

pfUSDC egress releases the USDC locked by pfUSDC ingress; it does not require
a second redemption buffer.

## 4. Explicit non-goals of this cutover

- No a666 / wA666 deployment (WS5 comes after this rail is proven).
- No NAV band mint/redeem activation (WS6).
- No HTLC lane changes (WS7 parallel track).
- No Arbitrum route profile registration under any circumstances.


## Activity-driven activation protocol citations (2026-07-25, Lane C dispatch #232)

- Registration rejects before activation: `crates/node/src/execution_actions.rs:940-944`
  ("vault bridge route profile cannot be committed before its activation height").
- Authorization requires registration height >= activation:
  `crates/types/src/shielded_bridge_governance.rs:779-783`
  (`VaultBridgeRouteProfileRecordV1::new` rejects `authorized_height < profile.activation_height`).
- Consequence: activation-at-registration (M_safety = 0) is identical on
  mainnet — both predicates are pure functions of candidate block height and
  the committed amendment log with no network-specific branch. Mainnet
  cutover therefore cannot schedule an activation *window*: the activation
  height IS the registration block, and any binding-before-activation
  ordering must be expressed in block order (bind block strictly before the
  registration block), not as a height margin.
