# Native PFT Supply and Custody Inventory

**Date:** 2026-07-16
**Scope:** canonical native PFT creation, custody, movement, and destruction
**Invariant owner:** `native_pft_live_total` and block replay
**Status:** fixed-candidate; exhaustive inventory, genesis-to-tip replay,
checkpoint, prune, snapshot/restore, adversarial fuzz and immutable-candidate
gates pass. Live-fleet scale remains a real-value launch gate.

## 1. Protocol invariant

The only native PFT creation event is genesis. `Genesis.native_supply_atoms`
commits exactly `GENESIS_NATIVE_SUPPLY_ATOMS` and the height-zero faucet replay
base must hold that exact amount. At every accepted block:

```text
live native custody before - sum(receipt.fee_burned) = live native custody after
```

History checkpoint v2 additionally requires:

```text
checkpoint live native custody + cumulative native fee burns = genesis native supply
```

All arithmetic is checked. Duplicate custody keys, impossible Orchard totals,
overflow, unreported destruction, and unsupported checkpoint schemas fail closed.

## 2. Live custody lanes

`crates/node/src/block_replay_wallet.rs::native_pft_live_total` contains
compile-time exhaustive destructures of both `LedgerState` and `ShieldedState`.
Adding a replicated field therefore fails compilation until the field is either
counted below or explicitly classified as non-native.

| Canonical state lane | Native amount counted | Identity/duplicate rule | Production transition boundary |
|---|---:|---|---|
| `LedgerState.accounts` | every `Account.balance` | unique account address | transfer, payment v2, asset/NFT/escrow/offer/atomic-swap fees, owned/FastLane deposit and withdrawal, Orchard turnstile |
| `LedgerState.escrows` | amount of every open escrow whose asset is exactly `PFT` | unique escrow ID | escrow create moves account value in; finish/cancel moves the same value out |
| `LedgerState.offers` | every offer reserve plus the remaining sell amount when an open offer sells exactly `PFT` | unique offer ID | offer create/match/cancel; reserves return or burn only through receipt-accounted fees |
| `LedgerState.owned_objects` | value of every object whose asset is exactly `PFT` | unique `(object ID, version)` | native wrap/deposit, owned transfer, native unwrap; issued labels cannot unwrap into native balances |
| `LedgerState.fast_lane_reserves` | `amount_atoms` for the canonical native FastAsset ID | unique asset ID | FastLane deposit/redeem and checkpoint-anchored fee burn |
| `ShieldedState.orchard` | `turnstile_deposit_total - fee_burn_total - withdraw_total` | single optional canonical Orchard pool; checked subtraction | transparent ingress, private spend/swap, public egress, and Orchard fee burn |

The following fields are explicitly non-native accounting lanes: issued-asset
definitions/trustlines, NFTs, NAV/market-ops/vault-bridge/PFTL-Uniswap records,
FastLane issued reserves, FastLane rule/permit/control metadata, FastSwap policy
metadata, legacy shielded note metadata, and bridge/governance records. PFTL and
pfUSDC are issued assets, not native PFT.

FastLane pending fee burns remain backed inside the primary reserve until a
checkpoint certificate is anchored. Anchoring removes the amount from the
reserve and emits the exact native burn in the canonical receipt. Counting the
reserve before anchoring and the receipt burn at anchoring prevents both early
burn and double burn.

## 3. Creation, movement, and destruction paths

| Class | Paths | Supply effect |
|---|---|---|
| Genesis creation | `Genesis::new`, node `init`, faucet replay-base validation | creates the fixed supply exactly once |
| Transparent movement | transfer and payment v2 | sender to recipient; fee is explicit burn |
| Asset/product fees | issued-asset, NFT, escrow, offer, atomic swap | native account fee debit; exact receipt burn; product value moves in its own lane |
| Native escrow | create, finish, cancel | account to/from open native escrow; no mint |
| Native offer | create, match, cancel | account to/from reserve and remaining native sell balance; no mint |
| Owned/FastPay payment | native wrap/deposit, owned transfer, native unwrap | account to/from exact-`PFT` owned objects; no label conversion and no mint |
| FastLane bridge | deposit, exit redemption, checkpoint anchoring | account to/from native primary reserve; deposit/checkpoint fees are receipt burns |
| Asset-Orchard | ingress, private operations, egress | account to/from the turnstile-derived live pool; Orchard fees are receipt burns |
| Governance/bridge metadata | all governance, NAV, external bridge, and PFTL-Uniswap records | no authority to create native PFT |

Rejected transactions must leave every custody lane and the cumulative burn
unchanged. There is no post-genesis mint, operator credit, bridge credit, or
governance amendment that changes native supply.

## 4. Replay, prune, and recovery boundaries

- Full replay starts from the genesis-bound faucet and checks the invariant at
  every block using replayed accepted receipts.
- Checkpoint v2 commits the complete economic state, the prune boundary, archive
  roots, and cumulative native burns.
- Checkpoint v1 is refused because it cannot prove cumulative burns.
- `history-checkpoint-rebuild-from-archive` is the offline recovery operation.
  It ignores all economic state in v1, verifies a contiguous imported archive
  prefix from height 1, replays it from genesis, verifies both prefix and retained
  suffix in isolated shadow stores, backs up v1, and only then atomically installs
  v2. A tampered archive fails without changing the checkpoint.
- Snapshot import runs full block verification before success; the regression
  also compares the exact all-lane native live total before export and after
  restore.

## 5. Local evidence

- `native_supply_oracle_counts_each_live_custody_lane_once_and_checks_overflow`:
  every custody lane, maximum arithmetic, duplicate account/escrow/offer/object/
  reserve keys, issued-lane exclusion, 256 deterministic custody splits, and
  unreported burn rejection.
- `history_prune_writes_checkpoint_and_allows_post_prune_block`: v2 cumulative
  burn equality, post-prune append, v1 refusal, tampered-archive no-mutation
  rejection, malicious v1 economic-state discard, archive rebuild, and retained
  suffix verification.
- `init_then_run_once`: snapshot round trip preserves exact native live custody
  and passes replay verification after restore.

This inventory closes the implementation boundary only. Final closure still
requires the immutable-candidate genesis-to-tip, pruned-history, snapshot, and
post-restore runs recorded by the master remediation plan.
