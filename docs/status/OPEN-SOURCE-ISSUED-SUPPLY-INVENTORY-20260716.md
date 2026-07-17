# Open-source issued-supply inventory — 2026-07-16

**Invariant:** for each supported issued asset, every live atom is counted once
across custody lanes; only an issuer-authorized or externally verified mint can
increase live supply; burns/redemptions decrease it by the signed amount; pure
custody moves preserve it; aggregate supply never exceeds `max_supply` or an
active finalized NAV circulating-supply ceiling.

## Live custody lanes

| Lane | Authoritative state | Counted by candidate | Movement proof |
|---|---|---|---|
| Holder balance | `LedgerState.trustlines[].balance` | `issued_asset_supply` | issued payment/property tests |
| Open escrow | `LedgerState.escrows[]` where state is open | `issued_asset_supply` | lock/finish/cancel conservation tests |
| Open offer sell side | `LedgerState.offers[].taker_gets_amount_remaining` where open | `issued_asset_supply` | partial fill/cancel/reject conservation tests |
| FastLane custody | `LedgerState.fast_lane_reserves[]` matching the 48-byte issued asset ID | `issued_asset_supply`, checked `u128` to `u64` | issued deposit/redeem and cap-bypass regression |
| AssetOrchard custody | `ShieldedState.orchard.asset_orchard_balances[].live_total` | combined replicated-state/mempool invariant | real ingress/private accounting/egress round trip and cap-bypass regression |
| Legacy cleartext privacy | historical replay only | excluded from supported live issuance | `P0-PRIVACY-01` live-path rejection |
| Owned-object lane | native PFT only | issued labels rejected | `P0-ASSET-01` regressions |
| PFTL↔Ethereum representation | `LedgerState.pftl_uniswap_routes[]`: outstanding claims, pending returns, Ethereum spendable, and other registered venues | `issued_asset_supply`, grouped by native issued asset and route ID | route transition, global-cap, proof/replay, and end-to-end bridge tests |

Compile-exhaustive `LedgerState` and `ShieldedState` destructures make a new
state field fail compilation until it is classified. The combined validator
rejects duplicate definitions and duplicate trustline, escrow, offer,
FastLane, route, owned-object, or AssetOrchard custody keys. Unknown issued
asset references and issued assets in the native-only owned-object lane fail
closed. Per-lane and aggregate overflow reject instead of wrapping.

## Supply-changing operations

| Operation | Required authority/evidence | Exact effect and evidence |
|---|---|---|
| `issued_payment` issuer → holder | issuer account signature and sequence | `+amount`; max/global cap tests |
| `issued_payment` holder → issuer | holder signature and authorized/unfrozen line | `-amount`; lifecycle/property tests |
| `asset_burn` | holder signature | `-amount`; property and replay tests |
| `asset_clawback` | issuer signature plus asset `clawback_enabled` | `-amount`; authorization/no-mutation tests |
| `nav_mint_at_nav` | live finalized reserve packet and issuer | `+amount`, bounded by finalized circulating supply and static cap |
| `nav_redeem_at_nav` | holder signature and live packet | `-amount`, creates exact redemption liability; settlement cannot mint |
| vault bridge claim/mint | finalized counted receipt/allocation | `+amount`, receipt/allocation one-use and cap checked |
| vault bridge burn-to-redeem | holder signature and live packet/bucket | `-amount`, exact queue/allocation accounting; settlement drains liability |

AssetOrchard ingress/egress, FastLane deposit/redeem, escrow, offer matching,
and atomic swap are supply-neutral custody moves. The execution/property suites
assert their exact before/after totals and no mutation on rejection.

## Global enforcement points

1. Execution mint calculations count transparent, escrow, offer, FastLane, and
   registered external-route custody.
2. Asset-transaction admission and whole-mempool replay combine that amount
   with current AssetOrchard live balances.
3. Replicated-state root construction applies the same aggregate check, so an
   invalid state cannot be proposed, committed, or accepted by canonical
   replay.
4. A finalized `NavTrackedAsset.circulating_supply` is an additional aggregate
   ceiling. Private ingress therefore cannot create hidden NAV mint capacity.
5. State roots commit every counted lane, including FastLane state after its
   explicit v2 activation and AssetOrchard balance rows.
6. Vault-bridge status and the v2 reserve replay bundle use the same global
   oracle. The bundle carries FastLane, external-route, and AssetOrchard rows;
   tampering either FastLane or Orchard custody fails replay.
7. The ordered-commit lock now spans recovery, state read, execution, and
   persistence for every batch kind. Concurrent same-nullifier egress therefore
   admits one batch and rejects all losing calls before a second state transition.

## Evidence

- issued lifecycle property test spans create, trust, issue, transfer, burn,
  over-cap rejection, insufficient balance, and trustline limit rejection;
- escrow and offer suites preserve exact issued totals across open/finish/
  cancel/fill/reject states;
- atomic swap conserves both issued legs and rejects partial mutation;
- FastLane issued deposit/redeem preserves reserve solvency, while a new
  regression rejects reminting against reserve custody;
- real AssetOrchard issued ingress/egress round trip passes and asserts global
  supply 40 before ingress, after ingress, and after egress; an eight-worker
  same-note race admits exactly one spend and seven idempotent rejections;
- the same private round trip exports and restores a signed snapshot, compares
  the exact issued total, and replays all blocks; the state-root counterexample
  rejects public 10 + private 1 under cap 10 and accepts the exact-cap inverse;
- NAV and vault bridge lifecycle tests cover finalized supply ceiling, mint,
  redeem, receipt consumption, queue settlement, replay, and cap rejection;
- `issued-supply-invariants` PASS: 256 iterations, 4,352 cases, zero invariant
  failures across all custody lanes, duplicates, unknown assets, unsupported
  owned custody, and maximum/aggregate overflow;
- targeted mint, burn, clawback, escrow, offer, private ingress/egress,
  snapshot/replay, and reserve-bundle tamper tests PASS; affected workspace
  checks, formatting, and strict Clippy PASS.

This closes the local implementation and adversarial-inventory portion of the
issued-asset supply/cap gate. The final immutable-candidate customer flow and
global release battery remain open and are not implied by these local results.
