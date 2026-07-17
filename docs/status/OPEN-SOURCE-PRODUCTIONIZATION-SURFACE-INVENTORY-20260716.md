# PostFiat L1 Production Surface Inventory

**Date:** 2026-07-16
**Baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Purpose:** code-derived transaction, action, RPC, and authorization inventory supporting STEP 1 of `OPEN-SOURCE-PRODUCTIONIZATION-REVIEW-CHECKLIST-20260716.md`.

This is a security inventory, not a feature list. A surface is not considered safe merely because it appears here. The closure register in `OPEN-SOURCE-PRODUCTIONIZATION-AUDIT-20260716.md` controls severity and remediation.

## 1. Ordered state-transition families

`TransactionBatch` at `crates/types/src/transactions_mempool_receipts.rs:3922-3940` carries eight families in a fixed batch envelope. Shielded, bridge, and governance batches are separate ordered batch domains. FastPay/FastSwap also expose certificate lanes whose primary-ledger deposits, redeems, checkpoints, and control actions enter `TransactionBatch` through `fastlane_primary_transactions`.

| Family | Live variants | Authorization at the type/admission boundary | Ordering/commit path | Primary review risk |
|---|---:|---|---|---|
| Legacy transparent transfer | 1 | ML-DSA signed source; chain/genesis/protocol, fee, sequence and amount in signing bytes | mempool → deterministic transaction batch → certified block | historical compatibility and duplicate legacy submit helpers |
| Payment V2 | 1 | ML-DSA signed source; versioned payment domain and memo | same | ensure no weaker legacy fallback is live |
| Asset/NAV/bridge/Uniswap transaction | 36 | one signed `source`; operation-specific source-role match; common chain/genesis/protocol/fee/sequence | same | very large authority matrix and external-evidence trust boundary |
| W6 atomic swap | 1 | two distinct ML-DSA authorizations over one exact dual-leg intent | same | DvP, quote/parent binding, replay and receipt-code semantics |
| FastLane primary | 4 | signed deposit, certified redeem/checkpoint/control depending variant | same | FastPay/FastSwap ↔ primary-ledger conservation and reconfiguration |
| Escrow | 3 | signed operation source | same | finish/cancel exclusivity and time predicates |
| NFT | 3 | signed issuer/owner source | same | ownership, issuer policy and replay |
| Offer | 2 | signed maker source | same | matching determinism and locked-liquidity accounting |
| Shielded | 10 | legacy cleartext actions or Orchard proof/binding signatures, depending variant | separate certified shielded batch | legacy privacy P0; circuit/public-input and proof-key audit |
| Bridge | 3 | domain config, witness attestations, pause authority | separate certified bridge batch | external finality, replay, pause and supply conservation |
| Governance | 4 batch collections | currently structural support names rather than cryptographic votes | separate certified governance batch | P0-GOVERNANCE-01 |
| FastPay owned transfer/unwrap | 2 certificate operations | owner-signed order plus distinct validator certificate | owned-object certificate lane | lock durability, cancellation and primary-lane boundary |
| FastSwap | confirm/cancel across 5 phases | two owner signatures plus phase-specific validator certificates | FastSwap certificate/WAL lane | cross-phase safety, policy/committee fencing, settlement conservation |

### 1.1 Common transaction envelope checks

The normal signed transaction families bind chain ID, genesis hash, protocol version, address namespace, transaction kind, signature algorithm, source, fee, and sequence in their signing bytes. Admission and execution must both verify canonical shape, signature-to-address recovery, minimum fee, exact next sequence, state-dependent authorization, balance/capacity, and replay identity. The audit must reject any family that only performs these checks at a wallet or proposer boundary.

The batch contains these fields in fixed family order:

1. `transactions` (`SignedTransfer`);
2. `payments_v2` (`SignedPaymentV2`);
3. `asset_transactions` (`SignedAssetTransaction`);
4. `atomic_swap_transactions` (`SignedAtomicSwapTransaction`);
5. `fastlane_primary_transactions` (`FastLanePrimaryTransactionV1`);
6. `escrow_transactions` (`SignedEscrowTransaction`);
7. `nft_transactions` (`SignedNftTransaction`);
8. `offer_transactions` (`SignedOfferTransaction`).

Every family needs an explicit test proving the same invalid input is rejected at admission, proposal validation, execution, replay, state verification, and catch-up without partial mutation.

`P1-MEMPOOL-01` reproduced a concrete violation of that contract: admission
previously replayed all existing families before the candidate and omitted
atomic/FastLane state in several copies. The candidate now simulates each
transaction at the exact family boundary above, including active
atomic/FastLane prefixes. A paused atomic entry remains intentionally skippable
so governance deactivation cannot wedge unrelated mempool traffic; proposal
construction owns its eviction.

## 2. Asset/NAV/bridge operation authorization matrix

The 36 variants are defined at `transactions_mempool_receipts.rs:2393-2466`. `UnsignedAssetTransaction::validate` binds the common envelope and calls `source_matches` (`:2684-2744`). The source-role map below is therefore security-critical.

| Operation | Required transaction `source` role | Principal invariant to prove |
|---|---|---|
| `AssetCreate` | issuer | unique definition; supply starts at zero; policy immutable/versioned |
| `TrustSet` | account or issuer | account intent and issuer authorization are not conflated; reserve accounting exact |
| `IssuedPayment` | sender | sender balance/trust authorization; issuer policy; conservation |
| `AssetBurn` | owner | owner debit equals supply reduction |
| `AssetClawback` | issuer | clawback capability enabled; victim debit and supply/accounting exact |
| `NavAssetRegister` | issuer | NAV binding is unique and authorized |
| `NavReserveSubmit` | submitter | proof profile, epoch, attestors, source root and reserve totals verified |
| `NavReserveChallenge` | challenger | bond locked once; challenge replay and resolution deterministic |
| `NavEpochFinalize` | issuer | finalization window, challenge outcome and active epoch monotonic |
| `MarketOpsPolicyRegister` | issuer | policy authority and immutable version/hash binding |
| `MarketOpsFinalize` | issuer | envelope evidence, price, capacity and epoch binding |
| `NavMintAtNav` | issuer | exact NAV arithmetic, caps, backing and rounding |
| `NavRedeemAtNav` | owner | owner debit, redemption liability and rounding |
| `NavHalt` | issuer | halt authority; fail-closed affected operations |
| `NavProfileRegister` | registrant | governance/issuer authority over proof profile and limits |
| `NavRedeemSettle` | issuer | liability discharged exactly once against real settlement evidence |
| `NavReserveAttest` | attestor | registered attestor key, epoch/root binding, unique vote |
| `NavAttestorRegister` | attestor | self-key proof plus policy authorization; no self-escalation |
| `VaultBridgeDepositPropose` | proposer | unique external evidence and bounded challenge lifecycle |
| `VaultBridgeDepositChallenge` | challenger | bond/replay/finality rules |
| `VaultBridgeDepositAttest` | attestor | registered attestor signature and distinct counting |
| `VaultBridgeDepositFinalize` | finalizer | quorum/challenge window and evidence exactness |
| `VaultBridgeDepositClaim` | claimer | beneficiary authorization and single claim |
| `VaultBridgeReceiptSubmit` | operator | external receipt authenticity/finality and replay |
| `VaultBridgeReceiptCount` | operator | count is derived from verified receipts, not asserted authority |
| `VaultBridgeMintFromReceipts` | issuer | minted amount equals unconsumed verified receipts |
| `VaultBridgeBurnToRedeem` | owner | burn/liability conservation and owner authorization |
| `VaultBridgeRedeemSettle` | issuer or redemption account | settlement evidence, beneficiary, replay and liability discharge |
| `VaultBridgeBucketImpair` | operator | bounded impairment authority and supply invariant |
| `VaultBridgeNavSubscriptionAllocate` | consume-supply owner; operator only for versioned legacy case | allocation consumes the correct owner's capacity; legacy exception replay-only |
| `PftlUniswapRouteInit` | operator | route config/contract/chain/caps bound once |
| `PftlUniswapPrimarySubscribe` | subscriber | subscriber pays settlement asset; NAV amount/cap arithmetic exact |
| `PftlUniswapExportDebit` | owner | owner debit locked/burned once; packet and recipient exact |
| `PftlUniswapDestinationConsume` | operator | Ethereum proof/finality and packet replay verified |
| `PftlUniswapRefundSource` | operator | only expired unconsumed packet refunded once to original owner |
| `PftlUniswapReturnImport` | operator | canonical burn proof/finality; return amount and replay exact |

The audit has not accepted “operator signed it” as sufficient external evidence. Each operator path must bind the exact external chain, contract/code hash, event/log, finality policy, route configuration, amount, beneficiary, nonce, and replay key; otherwise it must be labeled a controlled bridge rather than trustless verification. The current destination-consume, return-import, and refund paths do not carry those proofs and are P0-BRIDGE-01.

Every bridge deposit destination is also a deployment authority, not a wallet
preference or a source default. The candidate browser and proxy relay have no
default vault, reject the known retired `0x1A15…DEa9` vault, and remove the
destination from user settings. The live UX transaction script likewise
requires an explicit non-retired destination. Browser approval/deposit, proxy
relay, and the live harness additionally require and verify the release-bound
deployed bytecode hash. Bridge deposits remain disabled until a reviewed
release binds an exact live contract and code hash; this is the local
containment for P0-WALLET-BRIDGE-DEST-01.

## 3. Other operation variants

### 3.1 Escrow, NFT, and offers

- Escrow: `EscrowCreate`, `EscrowFinish`, `EscrowCancel`.
- NFT: `NftMint`, `NftTransfer`, `NftBurn`.
- Offer: `OfferCreate`, `OfferCancel`; fills are execution results of deterministic matching, not separately signed operations.

Closure requires pairwise exclusivity tests (finish/cancel, transfer/burn, fill/cancel), replay at every boundary, locked-value conservation, and deterministic matching under reordered admission.

### 3.2 Shielded actions

`ShieldedAction` has eleven variants:

- legacy: `Mint`, `Spend`, `Migrate`;
- Orchard: `OrchardV1`, `OrchardWithdrawV1`, `OrchardDepositV1`;
- private swap: `ShieldedSwapV1`;
- issued-asset Orchard: historical-replay-only `AssetOrchardIngressV1`, live
  encrypted `AssetOrchardIngressV2`, `AssetOrchardEgressV1`, and
  `AssetOrchardPrivateEgressV1`.

The first three carry or manipulate the cleartext `ShieldedNote` type and are
the subject of P0-PRIVACY-01. AssetOrchard ingress v1 additionally exposed a
recipient note opening and is live-disabled by P0-PRIVACY-02; v2 carries only
the public burn boundary, commitment, and authenticated ciphertext. The eight
supported/non-legacy variants require a complete circuit/public-input,
verifying-key, nullifier, anchor, value-conservation, binding-signature, fee,
replay, and privacy-leakage inventory. A 6/6-converged rejected receipt is not
success; every product and test must check `accepted` and the canonical receipt
code.

### 3.3 Bridge actions

`BridgeAction` contains `Domain`, `Transfer`, and `Pause` (`Resume` is encoded by the pause action state). Required checks are old-domain authority for domain changes, cryptographically verified and distinct witness attestations, chain/contract/epoch/finality binding, replay protection, amount/recipient conservation, and pause behavior at admission and execution.

### 3.4 Governance actions

`GovernanceActionBatch` at `shielded_bridge_governance.rs:960-970` carries:

- amendments;
- validator registry updates;
- governance-agent dry-run records;
- FastSwap governance bootstraps.

At the baseline, amendments and registry transitions rely on unsigned validator-name support. This entire domain remains blocked by P0-GOVERNANCE-01 until old-registry signatures are verified at every boundary with versioned historical replay.

### 3.5 FastLane primary and FastSwap

FastLane primary variants are `Deposit`, `Redeem`, `AnchorCheckpoint`, and `Control`. Control actions are `RegisterAssetRule`, `RegisterHolderPermit`, `RegisterPolicy`, `StopPrepare`, `ActivateCommittee`, and `ActivateProtocol`. Issuer asset-control commands are `Freeze`, `Unfreeze`, and `Clawback`.

FastSwap uses a dual-owner `SignedFastSwapIntentV1`, then validator votes/certificates across `Precommit`, `Commit`, `Effects`, `NewRound`, and `CancelApply`. Its safety case depends on exact committee/policy fencing, durable vote/lock ordering, certificate distinctness and quorum, confirm-or-cancel exclusivity, late-message behavior, and primary-lane checkpoint/redeem conservation. These surfaces are experimental until the associated formal model, fault tests, and production deployment posture are explicitly included in the public maturity matrix.

## 4. Remote RPC exposure

The candidate Rust SDK declares 95 unique method constants. The node's no-flag
remote allowlist contains 75 method names. The old 54-method runbook and the
baseline counts are stale and must not be used as the publication authority.
The executable policy in `rpc_cli.rs` plus its allowlist regression is canonical.

### 4.1 Always-on public reads

The baseline allowlist includes account/asset/escrow/NFT/offer reads; fee quotes; ledger/block/receipt/transaction/archive reads; validator/manifests/status/metrics; NAV/vault/Uniswap status and replay reports; Orchard counters; legacy shield scan/disclosure; owned-object and FastSwap status/effects/vote/policy reads; and local verification commands (`verify_blocks`, `verify_state`, `verify_bridge`, `verify_mempool`, `verify_shielded`).

Read-only does not mean harmless. Each method needs bounded input/output/work, no private-key/note leakage, no local-path disclosure, stable pagination, rate/concurrency limits, and a proof that an unauthenticated caller cannot trigger an unbounded child process or full-chain replay. Expensive verification methods should move to an authenticated operator listener unless strict bounded cost is demonstrated.

### 4.2 Always-on protocol mutation endpoints

The same no-flag allowlist exposes:

- FastSwap: `fastswap_prepare`, `fastswap_commit`, `fastswap_apply`, `fastswap_catch_up`, `fastswap_new_round_vote`, `fastswap_propose_round`, `fastswap_precommit`, `fastswap_commit_round`, `fastswap_cancel_apply`, `fastlane_exit`;
- FastLane asset control: `fastlane_asset_control_prepare`, `fastlane_asset_control_apply`, `fastlane_asset_control_catch_up` (preview is read-only);

FastPay vote/apply (`owned_sign`, `owned_apply`, `owned_unwrap_sign`, and
`owned_unwrap_apply`) is disabled by default and appears only under the explicit
experimental `--allow-owned-lane` flag. Legacy `wrap_owned` and `unwrap_owned`
are never remote methods under any flag.

Certificate/vote endpoints may legitimately be public protocol relays only if
they verify complete signed/certified payloads before any durable lock or
mutation, are idempotent, are bounded, and cannot be used as signing or storage
oracles. The removed unsigned owned bridge violated that rule at baseline;
current dispatch and real-store regressions prove it unreachable with no
mutation.

### 4.3 Opt-in public writes

`--allow-mempool-submit` enables non-finality signed submissions. `--allow-mempool-submit-finality` enables finality-driving submissions. The combined recognized set is:

- `mempool_submit_signed_transfer` and `_finality`;
- `mempool_submit_signed_payment_v2` and `_finality`;
- `mempool_submit_signed_asset_transaction` and `_finality`;
- `mempool_submit_signed_atomic_swap_transaction` and `_finality`;
- `mempool_submit_fastlane_primary`;
- `mempool_submit_signed_escrow_transaction` and `_finality`;
- `mempool_submit_signed_nft_transaction`;
- `mempool_submit_signed_offer_transaction`;
- `shield_batch_finality`.

`--allow-orchard-batch-create` separately enables `shield_batch_orchard`, `shield_batch_orchard_deposit`, `shield_batch_orchard_withdraw`, and `shield_batch_swap`.

These flags control exposure, not authorization. Every enabled handler must still validate the complete signed/proven payload before mempool insertion, proposal work, prover work, or mutation. Production listener separation and authenticated/rate-limited ingress remain P1-NET-01.

### 4.4 Local/operator-only commands

Local CLI dispatch also includes faucet-key export, local key validation, unsigned/local-key transfer helpers, batch construction/application, legacy shield mint/spend/migrate, bridge domain/transfer/pause creation, governance creation/application, snapshot import/export, key generation, and deployment tooling. Public release requires an explicit build/runtime boundary: local administrative commands must not become remotely reachable through generic child dispatch, path-controlled request files, or feature/config drift.

## 5. Known inventory findings

1. **P0-RPC-01 (contained):** the baseline unsigned public `wrap_owned`
   theft/state-divergence path is removed from remote and local live mutation.
2. **P0-GOVERNANCE-01 (contained):** unsigned governance cannot enter live
   proposal/apply; it remains historical replay/test-fixture only.
3. **P0-PRIVACY-01 (contained):** legacy cleartext note mint/spend is historical
   replay only; Asset-Orchard is the supported private path.
4. **P0-CUSTODY-01 (fixed):** browser signing is local and the backup-bearing
   proxy contract is removed.
5. **P0-BRIDGE-01 (contained):** asserted external consume/refund/import is
   rejected live and retained only for historical replay.
6. **P0-SUPPLY-01 (deployment blocked):** mint release now requires verifier
   output bound to the complete escrow settlement, but no concrete production
   verifier/code-hash policy exists and deployment is forbidden until it does.
7. **P1-NET-01 (fixed locally):** node listeners bind loopback/private only;
   external public read access requires an authenticated TLS edge.
8. **P1-ORDERING-01 (claim-corrected):** production batch selection retains
   fixed family/insertion order; stronger fee/bucket/omission claims are removed.
9. **P2-API-01 (fixed locally):** the generated v2 inventory covers all 135
   observed SDK/remote/local/Python methods, distinguishes 63 reads from 12
   default-enabled cryptographically authorized protocol mutations and every
   flag-gated class, reports zero unknowns, and fails CI when a new method is
   not explicitly classified. See
   `OPEN-SOURCE-RPC-AUTHORIZATION-INVENTORY-20260716.md`.
10. **P1-RPC-ERROR-01 (fixed locally):** internal and path-bearing remote
    errors now use stable public messages; typed protocol errors retain safe
    detail.

## 6. STEP 2 acceptance requirements derived from this inventory

- no state-changing public method without a complete signed/proven/certified authorization contract;
- no method classified as read-only may write ledger, locks, receipts, WAL, mempool, governance, bridge, shielded, or checkpoint state;
- every mutation has wrong-key, wrong-domain, stale-sequence, replay, duplicate, malformed, oversized, crash-before/after-durable-write, and rejected-no-mutation tests;
- every monetary family has per-asset conservation, supply/cap, fee-burn, locked-liability, and rejected-receipt assertions;
- every remote method has explicit exposure class, authentication class, rate/concurrency/body/work bounds, and privacy classification;
- generated inventories for SDK, node dispatch, remote allowlist, proxy, Python, and docs are byte-compared in CI so newly exposed methods fail the build until classified;
- historical decoders are activation/version gated and cannot be selected for new live input;
- integrated replay reconstructs byte-identical blocks, receipts, and state roots from the sanitized public candidate.
