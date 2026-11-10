# Phase 3 design: exporting native PFT as the same ERC-20 on Base

This is a draft design note, written on 2026-11-05 for milestone item P3.1 of `docs/plans/active/pft-erc20-base-listing-milestone.md`. It extends the Phase 2 design (`docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, section 6). It authorizes no deployment and no genesis change. The only code it describes as built is a refusing skeleton of the operation (section 7).

## 1. BLUF

1. Phase 3 lets a PFTL holder send native PFT back to Base as ERC-20 PFT. The new PFTL operation `external_lane_export_v1` debits the sender and credits the external lane. The ERC-20 PFT then comes out of the Phase 2 lock contract, through its dormant `release` and the PFTL finality verifier on Base (`crates/pft-token-contracts/src/PFTLockV1.sol:213-250`). No ERC-20 PFT is minted.
2. The lane grows back by exactly the exported atoms. Its remainder becomes reserved − (released − exported), computed in checked u64 arithmetic, so the per-block conservation check keeps its form (section 4).
3. PFTL does not read the lock's balance. It refuses an export that would make the lane's exported atoms exceed its released atoms. Every accepted export is payable on Base while two things hold: every PFTL release is backed by a real Lock, and every Base payout consumed one export packet. A false Phase 2 release or an unsound verifier payout breaks this. Sections 8 and 9 give the consequence and the response (section 4).
4. Base needs the PFTL finality verifier and a relay. The verifier must be deployed before the lock contract, because the lock fixes the verifier's address at construction (Phase 2, section 6). It can be deployed only after the first finalized PFTL checkpoint, because its constructor needs that checkpoint. Genesis therefore pins the lock's predicted address and its code hash, which is computed from the verifier's predicted address (section 5).
5. Today the operation is recognised and refused. Mempool admission, batch building and execution all refuse it with `external_lane_export_not_supported`. No lane or export state changes; in a certified block the ordinary rejected-transaction fee rule applies (section 7).

## 2. Terms

- **The lane**, **the lock contract**, **Base** and **PFTL** are as in the Phase 2 note, section 2.
- **An export** is one accepted `external_lane_export_v1`: native PFT leaves circulation into the lane and becomes claimable on Base (`docs/whitepaper.md:68-70`, the native → in-flight → external move).
- **A release on Base** is one successful `PFTLockV1.release(publicValues, proofBytes)`. It pays one export to its Base recipient and emits `Released(withdrawalIdCommitment, burnTxIdCommitment, packetDigest, recipient, amount)` (`PFTLockV1.sol:213-240`).
- **The PFTL finality verifier** is `PFTLFinalityVerifierV1`. It checks a proof that a PFTL withdrawal packet was finalized, and consumes its commitments once (`crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol:173-220`).
- **An export packet** is the PFTL record the verifier proves. It holds the export nonce, lane, sender, amount, Base recipient, digest and the two commitments the lock consumes. These are the same fields the NAV export debit carries (`crates/types/src/transactions_mempool_receipts.rs:3333-3350`).

## 3. The operation

`external_lane_export_v1` is an asset operation like `external_lane_release_v1` (`transactions_mempool_receipts.rs:3818-3819`):

| Field | Meaning |
|---|---|
| `sender` | PFTL address debited; the transaction's source |
| `lane_id` | the genesis external lane, for example `base-erc20` |
| `amount_atoms` | native atoms exported, one for one with ERC-20 base units (six decimals) |
| `base_recipient` | the Base address paid: `0x` and 40 lower-case hex digits |

The transaction pays its fee like any asset transaction.

**Built execution.** Execution will:

- debit `amount_atoms` plus the fee from the sender's native balance, keeping the account reserve;
- credit `amount_atoms` to the lane's `exported_atoms`;
- take the lane's next export nonce;
- record the export packet, with its digest and commitments.

The withdrawal-id commitment covers the lane, the export nonce and the packet. The burn-transaction commitment covers the accepted transaction id. The two contracts enforce different things:

- **The verifier** consumes the proof nullifier and the withdrawal-id commitment, and refuses either a second time (`PFTLFinalityVerifierV1.sol:194-204`).
- **The lock** consumes the withdrawal-id and the burn-transaction commitments, and refuses either a second time (`PFTLockV1.sol:228-235`).

So the withdrawal-id commitment is refused twice over, while the proof nullifier is checked only by the verifier and the burn commitment only by the lock.

**Shape refusals** (in `validate()`, before any state is read):

- a zero `amount_atoms`;
- a `base_recipient` that is not `0x` and 40 lower-case hex digits, or is the zero address;
- a malformed `lane_id`;
- an invalid `sender`.

**State refusals:**

- **Lane not configured.** The lane is not in genesis: `external_lane_not_configured`, the release's code (`crates/types/src/external_lane_types.rs:281`).
- **Recipient is the lock contract.** The `base_recipient` is the lane's `lock_contract_address`. This check runs after the lane check, so an unknown lane reports `external_lane_not_configured` first (`crates/execution/src/external_lane_release.rs:298-319`). A release to the lock itself can never pay out: the lock's balance does not change, so the delta check reverts (`PFTLockV1.sol:243-249`). The zero address is already refused by shape, and on Base by `PFTLockV1.sol:227` and `PFTLFinalityVerifierV1.sol:286`.
- **Above released.** `amount_atoms > released_atoms − exported_atoms` (`external_lane_export_above_released`, section 4).
- **Not enough balance.** The usual balance and reserve refusals.

**No minimum beyond one atom.** The lock's `release` has no minimum; it refuses only zero (`PFTLockV1.sol:226`). `minimumLockAmount` binds Locks, not releases (`:254-256`), and PFTL does not hold it: the lane policy has no such field (`external_lane_types.rs:305-320`). An exported amount below the lock minimum is still paid. The holder only cannot lock that exact amount back in one Lock. Section 9 lists whether to add a minimum.

**No per-window cap on exports.** The Phase 2 cap bounds what a colluding committee can release from the lane per window, because a release creates circulating native PFT from a claim PFTL cannot see (Phase 2, section 5.1). An export creates nothing on PFTL: it moves PFT the sender already holds out of circulation. The Base side is bounded by the verifier's proof and by the lock's balance (`PFTLockV1.sol:232-233`), not by PFTL. A cap on exports would delay honest holders and protect nothing on PFTL.

## 4. Accounting on PFTL

The lane state today holds `reserved_atoms`, `released_atoms`, the released Lock nonces and the window fields. Its remainder is reserved − released (`external_lane_types.rs:1048-1082`).

**The new fields.** Phase 3 adds `exported_atoms` and `next_export_nonce`, both u64. Phase 2 required released and exported atoms to be recorded separately so the lane can grow again (Phase 2, section 6). These fields arrive with the implementation, with the state-commitment change and their own qualification. The skeleton adds no ledger field.

**Arithmetic.** Every counter is u64 and every operation is checked; no expression is evaluated in a form that can wrap.

- **Net released.** `net_released = released_atoms.checked_sub(exported_atoms)`. The export rule keeps `exported_atoms ≤ released_atoms`, so a failure here is a corrupt state and is refused.
- **Lane invariant.** `net_released ≤ reserved_atoms`, and `remainder = reserved_atoms − net_released`. The form reserved − released + exported is never evaluated left to right: after a full cycle (reserved 100, released 100, exported 100), the next 1-atom release would make `100 − 101` underflow before `+ 100`.
- **A release.** `released_atoms.checked_add(amount)`, refused on overflow (`external_lane_counter_overflow`). The new net released must stay at or below `reserved_atoms`, refused otherwise as today (`external_lane_remainder_exceeded`).
- **An export.** It requires `amount_atoms ≤ released_atoms − exported_atoms` before any counter changes (`external_lane_export_above_released`). The export nonce is `next_export_nonce`, and `next_export_nonce.checked_add(1)` is refused when the counter is exhausted (`external_lane_export_nonce_exhausted`). Only then are `exported_atoms` and the nonce written. `exported_atoms + amount_atoms ≤ released_atoms` already holds, so that addition cannot overflow.

**Conservation.** The per-block check sums live native PFT, the lanes' remainders and the burns against the genesis supply (`crates/node/src/block_replay_wallet.rs:2864-2884`, `native_pft_live_total` `:2682`). An export lowers live by `amount_atoms` and raises the lane's remainder by the same amount; the fee is burned as today. The equation keeps its form: live + lanes' remainder + burns = genesis supply.

**What PFTL knows about the lock's balance.** PFTL does not read Base state, but its own record gives a lower bound:

- **Locked side.** Every released atom on PFTL is a `Locked` event proven against a committee checkpoint (`crates/execution/src/external_lane_release.rs:158-275`). The lock keeps every locked token until `release` (`PFTLockV1.sol:79-92`). So the lock's `totalLocked` is at least `released_atoms`.
- **Released side.** Every Base release consumes one export packet's commitments, once. So the lock's `totalReleased` is at most `exported_atoms`.
- **Rule, and its conditions.** Refusing any export that would make `exported_atoms > released_atoms` keeps `totalLocked − totalReleased ≥ released − exported ≥ 0`, but only while two conditions hold. Every PFTL release must be backed by a real Lock, and every Base payout must have consumed one export packet. Under them, every accepted export is payable whenever it is relayed.
- **When a condition fails.** Phase 2 does not assume the first condition: it treats a false release by a colluding committee quorum as possible, and answers it with the cap and the watcher (Phase 2, sections 5.1, 5.2 and 9). A false release raises `released_atoms` without raising the lock's `totalLocked`. An unsound verifier payout raises `totalReleased` without an export. Either way the bound can accept an export the lock cannot pay: `release` refuses with `ReleaseAboveLocked` when the balance is short (`PFTLockV1.sol:232-233`). Sections 8 and 9 give the consequence and the response.
- **Wait, not refund.** A release that has not been relayed yet waits: the packet stays valid and anyone can relay it. Nothing is refunded on PFTL.
- **Committee moves and unreleased Locks.** A Lock proven after a committee move still counts once, because the released Lock nonce set refuses a second release of the same nonce (`external_lane_types.rs:1068`). Locks made on Base but not yet released on PFTL raise `totalLocked` without raising `released_atoms`, so they only make the bound more conservative.

**The in-flight stage.** The whitepaper's in-flight stage (`docs/whitepaper.md:68-70`) has no ledger term of its own. An accepted export is external from the PFTL block that debits it, and the lane's remainder holds its packet until Base pays it.

**Nonces.** Lock nonces are the lock's `nextNonce` sequence. PFTL tracks them in `released_lock_nonces` (`external_lane_types.rs:1068`). Export nonces are a separate per-lane counter on PFTL. They appear on Base only inside the withdrawal-id commitment, never as a Lock nonce, so the two spaces cannot collide.

## 5. Proof on Base

**The verifier.** The lock calls `IPFTLFinalityVerifierV1.verifyAndConsume(publicValues, proofBytes)` (`PFTLockV1.sol:11-20`, `:219-225`), the interface the NAV vaults use (`ERC20BridgeVaultV2.sol:10-20`). `PFTLFinalityVerifierV1` checks the following:

- the route profile, epoch and checkpoint chain;
- the asset-id commitment (`PFTLFinalityVerifierV1.sol:274`);
- a chain id equal to `block.chainid`. The field is named `arbitrumChainId`; a Base deployment sets 8453 (`:275-277`).
- that the caller is the decoded vault, with the bound runtime code hash (`:278-281`);
- the token and its code hash (`:282-285`);
- a non-zero amount and recipient (`:286`).

For Phase 3 the "vault" is the lock contract and the token is the ERC-20 PFT.

**Deployment order.** The constructors fix the order:

- **The lock binds the verifier.** The lock fixes the verifier's address at construction, with no setter (Phase 2, section 6). Its constructor refuses a verifier address without code (`crates/pft-token-contracts/src/PFTLockV1.sol:151-160`). Solidity places immutables in runtime code, so the lock's runtime code hash depends on the verifier's address.
- **The verifier binds the lock.** The verifier fixes the lock's runtime code hash at construction (`PFTLFinalityVerifierV1.sol:122`, `:160`).
- **The verifier needs a finalized checkpoint.** Its constructor refuses a zero `initialCheckpointCommitment`, a zero `initialFinalizedHeight` and a zero `initialCommitteeRootCommitment`, as well as a zero PFTL genesis-hash commitment (`PFTLFinalityVerifierV1.sol:136-148`). Before PFTL genesis no finalized PFTL block exists, so the verifier cannot be deployed before the chain starts.
- **Genesis needs the lock.** Genesis must already name the lock's address and runtime code hash (`external_lane_types.rs:307-312`).

This supersedes the Phase 2 sentence that the lock contract is deployed before genesis. An operator follows these steps:

1. Pick two fresh deployer accounts on Base, one for the verifier and one for the lock. Predict both CREATE addresses from each deployer's next nonce; a CREATE address depends only on the deployer and its nonce.
2. Compute the lock's runtime code hash from the predicted verifier address and the lock's other constructor values: the token, its runtime code hash, `maxLockAmount` and `minimumLockAmount`.
3. Write the predicted lock address and that code hash into the lane policy of PFTL genesis.
4. Start the chain.
5. Take the first finalized PFTL checkpoint. It gives the verifier's `initialCheckpointCommitment`, `initialFinalizedHeight` and `initialCommitteeRootCommitment`.
6. Deploy the verifier from its deployer at the predicted nonce, with the lock's code hash as `vaultRuntimeCodeHash` and those initial values.
7. Deploy the lock from its deployer at its predicted nonce.
8. Check on chain that the lock's `finalityVerifier` and the verifier's `vaultRuntimeCodeHash`, `token` and `tokenRuntimeCodeHash` are the expected values, and that the lock's address and code hash equal the genesis pins.

**Deployer nonces.** Nothing else may be sent from either deployer before its deployment:

- A stray transaction from the verifier's deployer changes the verifier's address, and with it the lock's code hash.
- A stray transaction from the lock's deployer changes the lock's address.

Either way the genesis pins can never be met. No Lock is possible between genesis and step 7, because the lock does not exist yet.

**Dry run.** On Base Sepolia, the dry run deploys both contracts in this order. The first real proof must chain from the verifier's initial values, and one `release` must succeed.

The record follows the existing relay deployment record (`deployments/a666-export-relay-mainnet-20260731/README.md:7-12`). It holds the route, both deployers with their nonces, both addresses, the program verifying key, the code hashes and the initial checkpoint values.

**The relay.** The relay follows the A666 export relay (`deployments/a666-export-relay-mainnet-20260731/README.md`, user flow):

1. Wait for the export's PFTL finality.
2. Prove the export packet on the pinned prover.
3. Call `release` on Base.
4. Wait for Base finality.

Any party can relay, because `release` is permissionless and pays the packet's recipient, not the caller.

**What Base still needs.** A Base deployment of the verifier with a PFT route profile and asset id, a prover program that proves `external_lane_export_v1` packets, and the relay service. None of these exists today. The deployed verifier and relay serve the A666 route on Ethereum mainnet.

## 6. Operator and user flow

1. The holder submits `external_lane_export_v1` with the amount and its own Base address. The wallet signer (`wallet-sign-asset-transaction --operation-json`) runs the operation's `validate()` and so checks the address before signing (`crates/node/src/block_replay_wallet.rs:3300-3303`); a future dedicated CLI can do the same. No export CLI exists today.
2. PFTL accepts it, and the holder sees the export packet in the receipt.
3. The relay proves the packet and calls `release` on Base. The holder can also relay it.
4. The ERC-20 PFT arrives at the Base address, with a `Released` event naming the packet.
5. The lane info read shows `released_atoms` and `exported_atoms` side by side.

An exchange can export its customers' total in one operation and credit them on Base one for one, the reverse of the Phase 2 exchange flow (Phase 2, section 7).

## 7. Tests and qualification

**Built now: the skeleton.** It follows how the governed lane amendments were introduced before they were built (`b26de17a9`):

- The type `ExternalLaneExportOperationV1` with `validate()` (`crates/types/src/transactions_mempool_receipts.rs`).
- Its asset-operation variant `external_lane_export_v1`.
- An execution arm that refuses after the lane and lock-address checks, with `external_lane_export_not_supported` (`crates/execution/src/nft_escrow_asset_execution.rs`).
- Tests that the refusal holds at mempool admission, batch building and execution, with no lane or export state change. Further tests cover the shape rules (zero amount, bad recipient, unknown lane) and the lock-address refusal (`crates/node/src/tests/external_lane_export_tests.rs`, `crates/types/src/external_lane_type_tests.rs`).
- In a certified block the ordinary rejected-transaction fee rule applies (`crates/execution/src/rejected_fee_claim.rs`), as for a refused release. Before `rejected_tx_fee_activation_height` nothing changes. From it, the declared fee is burned, the sequence is consumed and the code becomes `tec_external_lane_export_not_supported`; nothing else changes (`external_lane_export_in_a_block_burns_the_fee_only_after_fee_activation`).

**With the implementation:**

Built 2026-11-06 behind `exports_enabled`: the PFTL accounting, the zero-cap halt, the export packet with its digest and commitments, and the paged read (`docs/architecture/genesis-external-lanes.md`, Exports); the Base fork test and the dry run wait for the Base side.

- the `exported_atoms` field and the export nonce, in the state commitment;
- the conservation check over a release followed by an export;
- the checked arithmetic of section 4:
  - an export with no releases yet (refused);
  - an export of exactly `released − exported` (accepted);
  - one atom above it (refused);
  - a second release after a full release-and-export cycle, with the right remainder;
  - an amount of `u64::MAX` (refused, no overflow);
  - an exhausted export nonce (refused);
- packet digest vectors shared with the prover program;
- a Base fork test of `release` against a verifier bound to the lock's code hash;
- the deployment-order dry run of section 5 on Base Sepolia.

## 8. Risks

- **Verifier soundness.** A false proof accepted on Base pays from the lock. The lock's balance bounds the loss, not PFTL: with no admin there is nothing to pause. The same holds for the NAV vaults today.
- **A false release meets exports.** A false Phase 2 release, or an unsound Base payout, leaves the lock holding less than `released − exported`. The exports accepted afterwards still pass the PFTL bound. Base pays them in relay order until the lock is short, and the last ones then fail with `ReleaseAboveLocked`. The loss lands on the honest exporters at the end of the queue, not on the attacker, who already received native PFT from the false release or ERC-20 PFT from the unsound payout. Section 9 gives the response.
- **Prover and relay liveness.** If no prover runs, exports stay unpaid on Base while the PFT has already left circulation on PFTL. Nothing is lost, but holders wait. Section 9 asks whether to add a refund after a deadline, as the NAV route's refund-source operation does (`transactions_mempool_receipts.rs:3493`).
- **Deployment order.** A wrong predicted address, a wrong code hash or a spent deployer nonce leaves the lane's lock or its release path dead forever, because the lock has no admin (section 5). The dry run on Base Sepolia must follow the steps of section 5 and complete one release before mainnet.
- **The chain-id field name.** `arbitrumChainId` is a generic chain-id check (`PFTLFinalityVerifierV1.sol:275-277`). A Base deployment must set 8453, not an Arbitrum id.
- **A fixed verifier.** The Base verifier fixes its program key, the PFTL protocol version (`PROTOCOL_VERSION`, `crates/types/src/core_chain.rs:9`) and the SP1 verifier address for ever, and the lock fixes the verifier for ever. Three events would end exports for a deployed lock: a PFTL upgrade that changes the protocol version or the finality format the program proves, a corrected program (a new key), or the retirement of that SP1 verifier. Nothing is lost on PFTL: exports are refused or stay unpaid, and the locked ERC-20 stays in the lock (decision 9).

## 9. Decisions

The product owner decides:

1. Whether Phase 3 ships at all: it is optional in the milestone. The PFTL half exists behind the genesis switch `exports_enabled`, off by default.
2. Whether exports have a minimum above one atom, for example the lock's `minimumLockAmount`. Enforcing it needs a new lane policy field and a genesis change.
3. Whether an unpaid export can be refunded on PFTL after a deadline. The refund needs the verifier to refuse the refunded packet. Without a refund, an export waits for the relay indefinitely.
4. The prover program and its audit budget, and who runs the relay.
5. How the relay is funded: self-relay only, a relay fee carried inside the export, or an operator-funded service.
6. The verifier's route profile and asset id for PFT on Base. Resolved by rule on 2026-11-10: both are derived from the lane's genesis policy ([Base half, step 1](#base-half-step-1-2026-11-10)).
7. The response to a false release once exports exist (section 8). The options:
   - **An export halt.** The same governed zero-cap amendment that halts releases also halts exports: exports are refused while the lane's active cap is zero (Phase 2, section 5.2). The watcher's alarm then stops new exports from queuing behind a shortfall.
   - **An independently authenticated Base lock balance.** PFTL would bound exports by the lock's real `totalLocked − totalReleased`, which this design avoids reading.
   - **A refund path.** An export the lock never pays is refunded on PFTL, which needs the verifier to refuse the refunded packet (see decision 3).

   The lean is the export halt. It costs least: one more check against a value the lane already has, with no new trust. It keeps the Phase 2 model of a committee-trusted bridge bounded by the cap and answered by the watcher and governance. It does not repay exports already accepted before the halt; only the refund path does that.
8. How a finalized PFTL block commits a lane export so that a bounded proof can show it. Taken on 2026-11-10: a block-header root ([Base half, step 2](#base-half-step-2-2026-11-10)).
9. How exports survive a change the fixed verifier cannot follow (section 8, "A fixed verifier"). The options:
   - **Accept it.** Add the rule that PFTL upgrades keep the old checkpoint format provable for the deployed program.
   - **A later verifier design.** A new program is admitted by a proof of the PFTL committee's approval. This is a new contract and a new audit scope.

## Base half, step 1 (2026-11-10)

**The rule (decision 6).** Neither value is chosen by hand. Both come from the lane's genesis entry and the PFTL chain id (`crates/types/src/external_lane_egress_types.rs`):

- **Asset id.** `GenesisExternalLane::pft_asset_id` follows `issued_asset_id` (`account_owned_asset_types.rs:658-675`): the PFTL chain id, then the lane's Base binding in the issuer's place (lane id, source chain id, lock, token), then code `PFT` and version 1. It is SHA3-384 under `postfiat.external_lane.asset_id.v1`, which gives 48 bytes.
- **Route profile.** `GenesisExternalLane::route_profile` takes the fields of `VaultBridgeRouteProfileV1` that genesis fixes for a lane, in its order and `key=value` preimage format (`account_owned_asset_types.rs:277-314`). Those fields are the lane id as `route_id`, the asset id, the source chain id, the lock and its code hash as the vault, the token and its code hash, and `route_epoch` 1. A lane's route has no governed successor, so it has a single epoch. The hash is SHA3-384 under `postfiat.external_lane.route_profile_hash.v1`, which gives 48 bytes. The verifier kind, program key and proof bounds are deployment values of the verifier, not route fields.
- **Verifier pins.** The verifier stores `keccak256` of each 48-byte value.
- **Production.** Mainnet uses the same functions at chain id 8453, with the production lock and token from the mainnet genesis.

For the Base Sepolia rehearsal lane of 2026-11-06 (`docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json`, PFTL chain id `postfiat-lane-sepolia-example`):

- asset id `1656c1b1…9521df`;
- route profile hash `9fd03c57…e3c48a17`.

The full values, their verifier commitments and an 8453 example with synthetic addresses are in `crates/types/testdata/external-lane-route-profile-vectors.json`. The values were computed by `scripts/external-lane-route-profile-vectors.py`.

**The public values.** `external_lane_export_egress_public_values` maps a packet, its lane and a finalized checkpoint (`ExternalLaneExportCheckpointV1`) to every field of the verifier's egress public values. `external_lane_export_egress_public_values_bytes` returns the bytes `verifyAndConsume` decodes. The verifier fixes the magic, the schema (`postfiat.pfusdc.egress_public_values.v1`) and the field layout (`PFTLFinalityVerifierV1.sol:354-446`). The lane reuses that layout unchanged.

| Tag | Field (decoded as) | Lane value |
| --- | --- | --- |
| 1, 2 | schema, `proofProgramVersion` | the egress schema, 1 |
| 3, 4, 5 | `pftlChainIdHash`, `pftlGenesisHashCommitment`, `pftlProtocolVersion` | checkpoint |
| 6, 7 | `routeProfileHashCommitment`, `routeEpoch` | derived profile hash, 1 |
| 8, 9, 11, 12, 13 | prior and resulting checkpoint, committee root and transition, `finalizedBlockHeight` | checkpoint |
| 10, 14-17 | committee epoch, view, block id, parent, state root (not decoded) | checkpoint |
| 18 | `bridgeExitRootCommitment` | the `external_lane_export_root` of the export's block header |
| 19, 20 | exit leaf index, leaf commitment (not decoded) | the leaf's index and the leaf in that root |
| 21, 22 | `acceptedReceiptCommitment`, receipt code | the export's transaction id, `accepted` |
| 23 | `assetIdCommitment` | derived asset id |
| 24, 25 | `burnTxIdCommitment`, `withdrawalIdCommitment` | the packet's burn-tx and withdrawal-id commitments |
| 26 | source bucket (not decoded) | the lane's `route_config_digest` |
| 27, 28 | `amount`, `recipient` | `amount_atoms`, `base_recipient` |
| 29, 30 | destination hash, evidence root (not decoded) | 48 zero bytes |
| 31 | `withdrawalFinalizedHeight` | the export block's height (`exported_at_height`) |
| 32 | `arbitrumChainId` | the policy's chain id (84532, 8453) |
| 33, 34 | `vault`, `vaultRuntimeCodeHash` | lock address and its code hash |
| 35, 36 | `token`, `tokenRuntimeCodeHash` | token address and its code hash |
| 37 | `packetDigest` | Keccak-256 under `postfiat.external_lane.export_evm_digest.v1` of the 48-byte packet digest |
| 38 | `withdrawalPacketHashCommitment` | the packet digest |
| 39 | `proofNullifier` | `pfusdc_egress_proof_nullifier_v1(1, burn tx, withdrawal id, finalized block)` |

**NAV-specific fields.** These fields have no natural source in a lane packet: route epoch, bridge-exit root, exit leaf index and commitment, accepted receipt, source bucket, destination hash, evidence root, the 32-byte `packetDigest` and the chain id's Arbitrum name. The table gives the lane's value for each.

**Limits.** None of the verifier's checks fails for a lane. The 48-byte commitments, the u64 amount and chain id, the lock and token code-hash bindings, and the replay keys (proof nullifier and withdrawal id on the verifier; withdrawal id and burn tx on the lock) all hold. Some differences remain:

- **Transaction id.** Tag 21 is 48 bytes, so the mapping refuses a transaction id that is not 96 lower-case hex digits. The vector file's `tx-0` placeholders are refused.
- **Released event.** `packetDigest` is 32 bytes, so the lock's `Released` event carries the Keccak digest, not the 48-byte PFTL packet digest. The 48-byte digest is in tag 38.
- **Statement.** The bytes are tested against the real decode (`crates/ethereum-contracts/test/ExternalLaneExportPublicValues.t.sol`, with a fixture the Rust test writes). The prover program that proves them is the next step.
- **A new guest.** The export program is a new SP1 guest with its own key. The pfUSDC guest requires `receipt.tx_id == burn_tx_id`, which a lane does not satisfy. Before it emits public values the program must authenticate the checkpoint's chain id and genesis, the PFTL finality of the export's block, that block's `external_lane_export_root`, the leaf's membership and index, the accepted transaction id, and the lane policy behind the route profile and asset id.
- **Replay.** The proof nullifier contains the finalized block id, so one export proven at two checkpoints has two nullifiers. Replay protection of one export rests on the withdrawal-id consumption in the verifier and in the lock.
- **Chain ids.** Every commitment binds the PFTL chain-id string. Two PFTL chains must never reuse a chain-id string.
- **The checkpoint input.** `ExternalLaneExportCheckpointV1` takes hashes without `0x`; its doc comment says so.
- **Deployment values.** `Genesis::external_lane_verifier_pins` returns the asset id, the route profile and its hash, their Keccak commitments, the chain-id hash and the genesis-hash commitment from one genesis. The lower-level functions take a chain id from the caller, so deployment code does not use them.

## Base half, step 2 (2026-11-10)

**Decision 8 is taken: a block-header root.** From an activation height, every block header carries `external_lane_export_root`. It is the root over the lane export packets the block accepted. A Merkleized state commitment would also give a bounded path, but it would change how every state field is committed, its replay and its archive. The header root is the shape the bridge-exit root and the PFTL-Uniswap receipt root already have.

**The leaf and the tree.**

- **Leaf.** SHA3-384 under `postfiat.external_lane.export_root_leaf.v1` over the PFTL chain id, the lane id, the export nonce (8 bytes big-endian) and the packet digest, each after a zero byte (`ExternalLaneExportPacket::export_root_leaf`).
- **Order.** Leaves follow the block's receipts, in execution order across all lanes (`external_lane_export_packets_for_block`, `crates/node/src/batch_snapshot.rs`).
- **Tree.** The bridge-exit tree's shape, in the lane's own small tree (`external_lane_export_root_v1`, `crates/types/src/external_lane_egress_types.rs`). It uses that tree's node hash and empty root unchanged, and does not edit `pfusdc_tier4_types.rs`, which the deployed pfUSDC egress guest links. A block without exports carries the empty value. A membership proof is `ExternalLaneExportRootProofV1`.
- **Bound.** At most `EXTERNAL_LANE_EXPORT_ROOT_MAX_LEAVES_V1` (4,096, the bridge-exit tree's per-block bound) exports per block. Batch building leaves exports past the bound in the mempool for the next block. A proposed or replayed block with more is invalid (`external_lane_export_root_full`). A batch holds at most 1,024 transactions, so the bound is also a defence in depth.
- **Vectors.** `crates/types/testdata/external-lane-export-root-vectors.json`, computed by `scripts/external-lane-export-root-vectors.py`.

**Activation.** The height comes from the genesis field `external_lane_export_root_activation_height`, so a new chain can have the root from its first block. A running chain activates it with the governed amendment of the same name, which must be scheduled after its own block. An operator proposes it with `postfiat-node ratify-external-lane-export-root-activation-height --validators CSV --height H --external-lane-export-root-activation-height H' --amendment-file PATH`. The genesis value wins; otherwise the earliest unpaused amendment applies. As for the bridge-exit root, consensus v2 must be active at that height.

- Before the height the header has no field, and the block hash, the proposal hash, the vote message and the consensus v2 block id are unchanged.
- From the height they take new versions that add the root: `postfiat.block.v5`, `postfiat.block_proposal.v5`, the `.v5` signature and vote messages, and `postfiat.consensus.block-id.external-lane-export-root.v1`.
- The proposer computes the root. A validator refuses a commit whose root differs from its own execution, and `verify-blocks` recomputes it from the replayed exports.

**Rollout.** Every node and every validator signer process must run a build that knows the root before the activation height is proposed. An older signer refuses to sign a block that carries the root: its guard does not know the `.v5` vote and proposal messages. The activation height must also be a height at which consensus v2 is active. `init` refuses a genesis that breaks this rule. The governed amendment is refused with `external_lane_export_root_activation_requires_consensus_v2` at admission, batch building, execution and replay when its height breaks it (`external_lane_export_root_consensus_v2_rejection`, `crates/node/src/execution_actions.rs`). The bridge-exit root's activation rule is unchanged. The operator steps are in [External Lane Export Root Activation](../runbooks/external-lane-export-root-activation.md).

**The empty value.** A block without exports carries the bridge-exit tree's empty value, the same bytes as an empty bridge-exit root. This is harmless: the leaf domain differs from the bridge-exit leaf domain, and the two roots sit in different header fields, each covered by the block id under its own name.

**NAV egress proofs.** `crates/pfusdc_proofs` is unchanged on purpose. A NAV egress proof does not compare the lane export root field by field; it relies on the recomputed block id, which binds the root. The lane's own proof must compare the root explicitly.

**The binding.** The inclusion names the export block's id and height. The proof program must show, in the same witness, that the root is that block's header root and that the block is the checkpoint's finalized block or an ancestor of it. The mapping cannot check this, and its doc comments say so. It refuses a block above the checkpoint, and a block id equal to the finalized block's at another height, or the reverse.

**Refusal before activation.** `external_lane_export_v1` is refused with `external_lane_export_root_not_active` while the root is not active at that height: at mempool admission, at batch building and at execution. In a certified block the ordinary rejected-transaction fee rule applies. An accepted export is therefore always in a header root. `exports_enabled` is unchanged.

**The binding follows the header.** The egress public values take tag 18 from the export block's root, tags 19 and 20 from the leaf, and tag 31 from that block (table above). The checkpoint input carries the export block's height, root and membership proof (`ExternalLaneExportInclusionV1`), not the lane's hash-chain root.

**Not done.** No proof was generated. The export proof program, the proof-input command, the relay and the deployment are still to do.

## Base half, step 3 (2026-11-10)

**The lane export proof program.** The lane needs its own program and key. The pfUSDC egress program requires `receipt.tx_id == burn_tx_id` and binds the bridge-exit root, which a lane export does not satisfy. Its source and key stay as on main.

**The statement.** `verify_external_lane_export_witness_v1` (`crates/pft_lane_proofs/src/lib.rs`) takes an `ExternalLaneExportProofWitnessV1` (`crates/types/src/external_lane_export_proof_types.rs`) and returns exactly `external_lane_export_egress_public_values` for it:

1. **Finality.** One block is final from the checkpoint the Base verifier already accepts, by `verify_pftl_finality_segment_v1` of `crates/pfusdc_proofs`, called unchanged. Nothing is copied from that crate.
2. **The export's block is the proven block.** The consensus v2 block id is recomputed from the witness block's fields with its `external_lane_export_root` (`postfiat.consensus.block-id.external-lane-export-root.v1`). It must equal the finalized id and the header's hash. The commit must bind the same root, and the block height must be the packet's `exported_at_height`.
3. **Membership.** The leaf is recomputed from the chain id, the lane id, the nonce and the packet digest. It must be in the header root (`verify_external_lane_export_root_proof_v1`). The index and leaf count are not unique under one root: the tree pairs an odd node with itself and the root does not commit the count, so a three-leaf proof for index 2 also verifies as index 3 of 4. The index in the public values is therefore advisory. The Base verifier does not use it, and replay protection rests on the withdrawal and burn commitments, which come from the packet alone.
4. **The lane policy.** The witness carries the chain's genesis. Its hash, chain id and protocol version must be the ones the finality domain signed. Its lane gives the route profile hash and the asset id (`Genesis::external_lane_verifier_pins`).
5. **The transaction.** The accepted transaction id in the public values is the packet's `tx_id`. The packet digest inside the leaf binds it, the header root binds the leaf, and the block id the committee signed binds the root. The block's `receipt_ids` are not checked: neither the block id nor the finality check covers them, so a witness could list any id there. A review found that the first version of this check proved nothing and removed it.

The result is then compared field by field with what the finality check returned: prior and resulting checkpoint, committee root and transition, heights, view, block ids, parent, state root, export root and leaf index. The Checkpoint variant calls `verify_checkpoint_witness_v1` unchanged.

**The witness.** It holds the chain id, the genesis hash, the protocol version and the prior checkpoint. It also holds the finality ancestry and committee (the pfUSDC bounds: at most 64 ancestry blocks and 64 validators), the export's block, the genesis, the packet and the membership proof (at most 12 siblings, so 4,096 leaves).

**The program.** `programs/pft-lane-export` has the shape of `programs/pfusdc-egress`. It reads the versioned CBOR `ExternalLaneExportProgramInputV1` (`withdrawal` or `checkpoint`) and commits `canonical_bytes_without_commitment()`. It has its own `Cargo.lock`.

**The input command.** `postfiat-node external-lane-export-proof-input --lane-id ID (--export-nonce N | --tx-id HEX) --prior-checkpoint HEX --output-dir PATH` writes `input.cbor` and `expected-public-values.json`. It works on a data directory only, not over RPC. The prior checkpoint is the Base verifier's latest accepted checkpoint. The command runs the statement before it writes anything. When the block's packets are pruned from the lane records, it rebuilds them by replaying the batch archive. Without the archive it refuses and names what the operator needs. It refuses an unknown nonce or transaction id, an export that is not final yet, a lane with exports off, and a chain without the export root.

**The cost of the genesis in the witness.** The witness carries the whole genesis, and the guest serialises and hashes it for every export. The cost was measured with `external_lane_export_witness_cost_inputs` (`crates/node/src/tests/external_lane_export_proof_tests.rs`): the same export, the same 4-validator chain and no ancestry, in SP1 execute mode with the ELF built at `bc36bd1df`.

| Genesis | Witness bytes (CBOR) | Instructions |
|---|---|---|
| The test genesis as it is (1 lane) | 153,345 | 33,577,987 |
| With 1,000 or 10,000 extra funded accounts | 153,345 | 33,577,987 |
| With the most lanes `validate` accepts (8, `EXTERNAL_LANE_MAX_LANES`) | 157,440 | 34,211,389 (+1.9 %) |

A `Genesis` has no account field (`crates/types/src/core_chain.rs:303`). Funded accounts start from the faucet after genesis and never enter the witness, so the account rows equal the base row by construction. The only part of a genesis that can grow is its lane list, which is capped at 8 lanes; at that size the cost rises by 1.9 %. That is within the 10 % rule, so the witness keeps the full genesis.

**Key and cycles.** No ELF and no key are committed. The CI proof-identity jobs reproduce programs with the Docker build (`cargo prove build --docker --tag v6.3.1`), and this host has no Docker. A local SP1 6.3.1 native build ran in execute mode only (`tools/pft-lane-export-prover execute`, `SP1_PROVER=cpu`). On the node test chain it committed exactly the expected public values for all three exports, 1,491 bytes each. It took 33,618,449 and 33,618,734 instructions for the two exports proven from their own block, and 43,034,593 for the export one block later, with one ancestry block. Next comes a Docker build on a host with Docker. Commit its ELF and `program-info` in the neighbours' format, then add the program to the proof public-input inventory and the proof-identity CI job.

**Tests.** The node test chain (`crates/node/src/tests/external_lane_export_proof_tests.rs`) has the lane fixture, consensus v2, `exports_enabled` and the export root. It runs a release, two exports in one block and one in the next. The statement accepts each proof input and returns the binding's public values, recomputed from the block's header and commit alone. It refuses a changed amount, recipient, nonce, transaction id, lane policy, leaf index, leaf count, header root, block id, parent or lane, a commit of another block, a prior checkpoint the witness does not chain from, and an export from an older block. Removing one check at a time (the genesis check, a receipt-list check since removed as vacuous, the commit's root check, the tree's sibling-depth check) makes the test fail. A second chain's lane names the lock and token that `crates/pft-token-contracts/test/ExternalLaneWitnessRelease.t.sol` deploys. Its proof input is recorded in `crates/pft_lane_proofs/testdata/witness-release-input.cbor`, because the chain's keys are fresh on every run. A test in `crates/pft_lane_proofs` runs the statement on that input and derives the Foundry fixture from its result (`ExternalLaneWitnessReleaseFixture.sol`). The fixture releases through the real `PFTLockV1` and `PFTLFinalityVerifierV1`. Executed in SP1, the recorded input takes 33,704,387 instructions and commits the same bytes.

**Limits.** The export's block must be the proven block. An export from an older block than the finalized one is refused in this version. No proof was generated. There is no relay and no deployment yet.

## Base half, step 4 (2026-11-10)

**The relay.** `scripts/pft-lane-export-relay.py` takes the A666 relay's four steps (wait for PFTL finality, prove, call `release`, wait for the receipt):

- `input` runs `postfiat-node external-lane-export-proof-input` for a lane and a nonce or transaction id. With `--wait-seconds` it retries while the export's block is not final, and it stops on any other refusal.
- `prove --mode execute-only` runs the guest in SP1's CPU executor (`tools/pft-lane-export-prover`). It checks the committed bytes against the expected public values and writes a PLACEHOLDER proof. The proof file says so in `mock_verifier_only` and `note`. Only a verifier built with a mock SP1 verifier accepts it.
- `prove --mode command --prover-command CMD` hands the input to an external prover. This mode has not been run.
- `release` reads the lock's `finalityVerifier`, `token`, `totalLocked` and `totalReleased`, and the verifier's pins and checkpoint records, before it sends anything. It refuses when the public values would revert: wrong vault, token, chain id, PFTL chain or genesis, route profile, asset id or code hashes; an unknown prior checkpoint; a consumed withdrawal id or proof nullifier; an amount above locked minus released. Then it sends `release(publicValues, proofBytes)`, decodes `Released` and checks that the recipient's balance moved by exactly the amount.
- `advance-checkpoint` runs the Checkpoint variant through the same steps (`pfusdc-checkpoint-witness`, execute, `advanceCheckpoint`). Use it when the verifier's checkpoint is too far behind for a bounded proof. It was not run in the rehearsal.
- `status` reports one packet: exported, final, proven, released, and the Base transaction.

**Cost and checkpoint cadence.** The finality ancestry of one proof is bounded at 64 blocks (`PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1`). So the Base verifier's checkpoint must advance at least every 64 PFTL blocks: through releases, or through `advance-checkpoint` before an export more than 64 blocks past it. Guest instruction counts measured in execute mode:

| Run | Validators | Ancestry blocks | Instructions |
|---|---|---|---|
| Step 3 node test, exports 0 and 1 | 4 | 0 | 33.6 M |
| Step 3 node test, export 2 | 4 | 1 | 43.0 M |
| Round trip, second export | 6 | 0 | 48.9 M |
| Round trip, first export | 6 | 2 | 76.7 M |

A private key is accepted only for a loopback Anvil; on any other chain the relay takes `--account` and `--password-file`. A placeholder proof goes only to a loopback Anvil, unless `--i-know-this-is-a-mock-verifier` names the lock's verifier.

**The prover-command contract.** The relay runs `CMD` through the shell with `PFT_LANE_EXPORT_INPUT` (the CBOR program input) and `PFT_LANE_EXPORT_PROOF_OUT` (a path) in its environment. The command writes the raw SP1 proof bytes to that path and the committed public values to the same path plus `.public-values`, then exits 0. The relay refuses public values that differ from the expected ones. The proof is then sent as `proofBytes` unchanged.

**The round trip.** `scripts/rehearse-lane-roundtrip.py` runs one round trip on local chains. It imports the deployment-order and Base-lock rehearsals and the relay; it does not copy them. The record is in `docs/review/lane-roundtrip-rehearsal-20261110/`.

1. It deploys the token on a fresh Anvil (chain id 84532), predicts the verifier's and the lock's addresses and the lock's code hash, and pins them in the lane genesis.
2. A generated six-validator PFTL chain starts from that genesis (`local_evm_lane_roundtrip`, an ignored node test that the script drives step by step). The chain has `exports_enabled`, and consensus v2 and the export root from block 2. Block 1 is the committee bootstrap.
3. It deploys the real `PFTLFinalityVerifierV1` with the genesis-owned pins and the first PFTL checkpoint, using the Foundry tests' `Tier4MockSP1Verifier` and a placeholder program key. Then it deploys the real `PFTLockV1`, and reads every constructor value back.
4. It locks 1,000,000 units, releases them on PFTL, exports 400,000 and relays them, refuses a second release of the same packet, exports and relays 600,000, and refuses a third export of one atom. It checks conservation on both chains at every step.

The exports are signed with `wallet-sign-asset-transaction --operation-json`.

**Limits.** No SP1 proof exists: a mock SP1 verifier accepted placeholder bytes, and the guest only ran in execute mode. Anvil stands in for Base. The ELF is a local build without a committed key. The PFTL chain is one generated data directory whose blocks carry consensus v2 commits from all six validators; it is not six networked nodes. The recipient got 10,000 native atoms from the faucet for the two export fees. The Phase 2 release watcher still reports the one release as backed. It has no export view, so it reports nothing about exports or their Base releases.

## 10. Sources

- `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md`, sections 5.1, 6 and 7.
- `docs/plans/active/pft-erc20-base-listing-milestone.md`, P3.1.
- `docs/whitepaper.md:68-70`.
- `crates/pft-token-contracts/src/PFTLockV1.sol:11-20`, `:79-92`, `:128`, `:151-160`, `:213-256`.
- `crates/ethereum-contracts/src/PFTLFinalityVerifierV1.sol:118-172`, `:173-220`, `:270-290`.
- `crates/ethereum-contracts/src/ERC20BridgeVaultV2.sol:10-20`.
- `crates/types/src/external_lane_types.rs:281`, `:305-320`, `:1048-1082`.
- `crates/types/src/transactions_mempool_receipts.rs:3333-3350`, `:3440`, `:3493`, `:3818-3819`.
- `crates/execution/src/external_lane_release.rs:158-275`.
- `crates/execution/src/nav_vault_asset_execution.rs:5744`, `:6470-6477`.
- `crates/node/src/block_replay_wallet.rs:2682`, `:2864-2884`, `:3300-3303`.
- `deployments/a666-export-relay-mainnet-20260731/README.md`.
