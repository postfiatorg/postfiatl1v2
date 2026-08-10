# A666 Egress Lane Redeploy Plan — 2026-08-09

> **CLOSED — PASS — 2026-08-10 03:35:45Z.** The old bucket was truthfully impaired at
> h792, the governed epoch-6 successor route finalized at h795, and a fresh 10,000,000-atom
> USDC -> pfUSDC -> USDC round trip settled at h800. Wallet USDC returned exactly to
> 74,161,443 atoms, the successor vault and obligations returned to zero, all six validators
> converged, and the protected 103,000,000 wA666 baseline did not move. The closeout and
> evidence index are in `docs/reports/A666-EGRESS-LANE-REDEPLOY-CLOSEOUT-20260810.md`.

Owner-authored after completing the settlement-binding code verification the 2200Z handoff
required before any deployment. Supersedes the "fund new vault with 9,932,863 atoms and settle
the existing redemption" sketch in both 20260809 handoffs: that path is IMPOSSIBLE under
deployed consensus rules. Evidence below, all file:line in worktree
/home/postfiat/repos/a666-eth-fast-lane-combined-20260724 at `9d14fdc`.

## 1. Code verdict: the pending redemption can never settle through a replacement vault

- The redemption's withdrawal packet embeds the OLD vault address and both binding hashes
  commit to it: `crates/types/src/account_owned_asset_types.rs:2936-2953`
  (`VaultBridgeRedemption::new` builds `VaultBridgeWithdrawalPacket { vault_address, ... }`
  from `source_domain`, then `withdrawal_packet_hash` + `withdrawal_packet_evm_digest`).
- Settlement requires attestor observations that EXACTLY match the packet's vault address:
  `crates/execution/src/nav_vault_asset_execution.rs:1442-1455`
  (`observation.vault_address == redemption.withdrawal_packet.vault_address` or
  `vault_bridge_withdrawal_observation_mismatch`). Quorum path:
  `nav_vault_asset_execution.rs:1511` -> `6816-6823` inside
  `apply_vault_bridge_redeem_settle_with_compatibility`.
- There is NO cancel, rebind, or migrate operation for redemptions. Full vault-bridge op set:
  `crates/types/src/core_chain.rs:42-57` (receipt_submit, deposit_*, mint_from_receipts,
  burn_to_redeem, redeem_settle, bucket_impair, nav_subscription_allocate). `redeem_settle`
  is the only exit from `pending`, and it is bound to the old vault as above.
- Dishonest settlement (signing observations that claim the old vault paid) is refused
  outright: it falsifies replicated records and breaks the product's core claim.
- The OLD vault cannot be revived either: `finalityVerifier` is set only in the constructor
  with no setter (`crates/ethereum-contracts/src/ERC20BridgeVaultL1.sol:75,110`; setter
  inventory confirms: docs/evidence/pfusdc-eth-campaign-20260725/reviews/
  route-preregistration/vault-setter-inventory.txt), and the deployed verifier's `programVKey`
  is immutable (`PFTLFinalityVerifierV1.sol:114`) at the dead vkey `0x0026a156…`.
- Re-minting pfUSDC against the stranded reserve (a hypothetical "cancel + remint" upgrade)
  is REJECTED: the backing USDC is unreachable on Ethereum, so any re-mint is unbacked.

Consequence: the 9,932,863-atom redemption `b3651dd4…3a931d5b` is a permanent tombstone.
Its economic loss is covered by the bucket write-down (Section 3). The round trip completes
by proving the FIXED lane end to end with fresh money, not by settling the dead claim.

## 2. Designed recovery machinery (already in deployed code, no protocol change needed)

- Governed route profiles: `VaultBridgeRouteProfileV1`
  (`account_owned_asset_types.rs:115-121`) carry `vault_address`,
  `vault_runtime_code_hash`, `route_epoch`; retirement sentinel
  `RETIRED_VAULT_BRIDGE_ADDRESS` (`:109`). Route ops exist
  (`core_chain.rs:58-70`: route_init_v2, route_epoch_advance, route_pause).
- Bucket write-down: `vault_bridge_bucket_impair`
  (`nav_vault_asset_execution.rs:6893+`) reduces `counted_value_atoms` and sets the
  impairment factor; issuer/operator-signed; exact factor arithmetic enforced.
- New verifier seeds from a CURRENT checkpoint: `PFTLFinalityVerifierV1.Config`
  takes `initialCheckpointCommitment` + `initialFinalizedHeight`
  (`PFTLFinalityVerifierV1.sol:30-32`), so NO historical re-proving of h544+ is needed.
  Only fresh checkpoints past the new burn height get proven (GPU, defect-ledger recipe).

## 3. Ordered execution plan

Gate order is strict; each step produces dated evidence under /tmp/a666-owner-20260809/
(archived to the evidence tree at close).

1. [x] [PFTL, accounting] Impair the old bucket to its truthful counted value (write off the
   195,031,396-atom stranded pool, which includes the 9,932,863-atom pending claim) via
   `vault_bridge_bucket_impair` with exact factor arithmetic. Old vault stays PAUSED forever
   as a tombstone; document the pending redemption as permanently pending and covered by
   this impairment.
2. [x] [PFTL, governance] Register/advance the governed route to a new epoch binding the NEW
   lane: new verifier (fresh guest vkey
   `0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87`, ELF sha256
   `4d5f8449…761c67e0`), new vault address, runtime code hashes. Address precommit order:
   predict vault address, finalize profile, deploy verifier binding profile hash, deploy
   vault binding verifier (mirrors the July deployment order).
3. [x] [Ethereum, deploy] Deploy new PFTLFinalityVerifierV1 (seeded at current finalized
   height/commitment) + new ERC20BridgeVaultL1. Gas is a few dollars, pre-authorized.
4. [x] [Custody] Authorize the two new contract addresses through the already-unlocked
   agent's bounded launch-session capability. No passphrase, global-policy rewrite, service
   restart, or relock was required. The source fix now retains the live unlock capability for
   later policy persistence, eliminating continuous reauthentication on an unlocked agent.
5. [x] [Fresh round trip, ~$10] deposit 10,000,000 atoms USDC into the new vault ->
   mint pfUSDC -> burn_to_redeem against the NEW bucket -> GPU-prove fresh checkpoint +
   receipt under the new vkey -> single `withdrawWithProof` -> `redeem_settle` with
   attestor observations matching the NEW vault -> six-validator reconciliation.
   One attempt per mutation, STOP-no-retry, exact atoms end to end.
6. [x] [Close] A6 conservation table restated with the write-down; tracker closeout; archive
   /tmp evidence; Task Node evidence for task_0f8d57dcc1dab7228ce8ff8792b50fe3.
7. [x] [Prevention, fix round] (a) automated checkpoint cadence with alarms; (b) day-zero
   campaign survey: every money-path contract vs current chain software; (c) rule: any
   change to block-ID derivation ships with a bridge handover plan (new route epoch +
   seeded verifier) BEFORE money flows.

The prevention gate is now explicit and fail-closed: `scripts/pfusdc-mainnet-latency-gate.py`
rejects a money-path preflight when verifier lag exceeds one PFTL block; campaign start requires
the money-path contract/software survey recorded in the closeout; and any block-ID derivation
change requires a route-epoch/verifier handover before deposits are permitted. The existing gate
tests pass as part of closeout verification.

## 4. Money impact

- Write-off: 195,031,396 atoms (~$195) stranded in the old vault; includes the $9.93 claim.
- New spend: ~$10 working capital through the new lane (returns to wallet at leg end),
  a few dollars of gas, GPU rental inside the $150 envelope (~$146 remains).
- Untouched: 103,000,000 wA666 baseline; wallet USDC balance beyond the $10 leg.
- Nothing reaches the principal's $1,000 line.
