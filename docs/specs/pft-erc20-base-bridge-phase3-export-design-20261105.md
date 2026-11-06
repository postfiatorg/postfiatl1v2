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

## 9. Decisions

The product owner decides:

1. Whether Phase 3 ships at all: it is optional in the milestone. The PFTL half exists behind the genesis switch `exports_enabled`, off by default.
2. Whether exports have a minimum above one atom, for example the lock's `minimumLockAmount`. Enforcing it needs a new lane policy field and a genesis change.
3. Whether an unpaid export can be refunded on PFTL after a deadline. The refund needs the verifier to refuse the refunded packet. Without a refund, an export waits for the relay indefinitely.
4. The prover program and its audit budget, and who runs the relay.
5. How the relay is funded: self-relay only, a relay fee carried inside the export, or an operator-funded service.
6. The verifier's route profile and asset id for PFT on Base.
7. The response to a false release once exports exist (section 8). The options:
   - **An export halt.** The same governed zero-cap amendment that halts releases also halts exports: exports are refused while the lane's active cap is zero (Phase 2, section 5.2). The watcher's alarm then stops new exports from queuing behind a shortfall.
   - **An independently authenticated Base lock balance.** PFTL would bound exports by the lock's real `totalLocked − totalReleased`, which this design avoids reading.
   - **A refund path.** An export the lock never pays is refunded on PFTL, which needs the verifier to refuse the refunded packet (see decision 3).

   The lean is the export halt. It costs least: one more check against a value the lane already has, with no new trust. It keeps the Phase 2 model of a committee-trusted bridge bounded by the cap and answered by the watcher and governance. It does not repay exports already accepted before the halt; only the refund path does that.

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
