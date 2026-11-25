# NAVCoins

> **Current product status (2026-11-25):** NAVCoins run on the controlled
> devnet `postfiat-wan-devnet-2`; none is generally available. B24, the
> Schwab-backed NAVCoin, has been created and cashed in with real money since
> mid-November; its newest reading on the chain is epoch 9. A666 v2 and BMNRC
> are on the chain with older readings. pfUSDC comes from Ethereum and, since
> November, Solana. Details and dates are in [Today](#today) below. The
> 2026-07-30 status of A666 is in
> [A666 Current State](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).
>
> **B24 (Schwab-backed), 2026-11-23:** epoch 9 finalized at NAV 1.006419. The TEE
> Schwab reader and the reading pipeline are in this repository under
> `tools/nav-reserve-proof/schwab-tee/` and `ops/b24-live/`; operating guide:
> [B24 Schwab NAV Reading](../runbooks/b24-schwab-nav-reading.md).

NAVCoins are floating-NAV issued assets whose supply, minting, redemption, and
halt behavior are tied to machine-checkable reserve packets. They are not
stablecoins and they do not promise a fixed dollar peg. If the reserve
portfolio loses value, the NAV falls. The protocol claim is narrower:

```text
current backing + liabilities + valuation policy + freshness + supply discipline
  -> reserve packet
  -> finalized NAV epoch
  -> bounded mint, redeem, swap, bridge, or market-operation path
```

The core accounting invariant is:

```text
verified_net_assets >= valid_global_supply * nav_per_unit_floor
```

Some examples use exact equality for controlled smokes. Production profiles can
use floors, haircuts, and policy-specific rounding, but the rule must be stated
in the proof profile and replayable from the packet.

## Current names

| Name | Meaning | Current status |
|---|---|---|
| NAVCoin | The asset class and protocol pattern: proof profile, reserve packet, supply discipline, mint/redeem controls, and market-operation limits. | Implemented on PFTL; A666 v2, BMNRC and B24 are on the controlled devnet, with release hardening still open. |
| B24 | The Schwab-backed NAVCoin: a proven reading of a Schwab account's equity. | On the devnet with real money since mid-November; newest reading epoch 9 (height 1842). See [Today](#today). |
| B23 | The earlier Schwab NAVCoin line that B24 replaced. | Replaced; its name remains in the proof profile and code (`b23-*`). |
| BMNRC | The BMNR carry NAVCoin (stock long, perpetual short). | Created on the devnet on 2026-10-02; one reading (epoch 1, height 1099). |
| a651 | The first named NAVCoin instance for the proven six-leg reserve portfolio. | Legacy/deprecated product lineage. Its historical Ethereum pool had zero pool-specific liquidity at the last a651 inspection; a651 remains migration and research history. |
| a652 | A second NAVCoin instance used to prove cross-NAVCoin swap mechanics. | WAN devnet evidence only, not a production public asset. |
| a666 | The deployed NAVCoin lineage for large, low-slippage primary subscriptions, symmetric primary redemption, and proof-backed Ethereum representation. | PFTL A666 v2, wA666, and the wA666/USDC pool are deployed; public GA remains closed. Newest reading epoch 9 (height 1108), older than its 128-block gap at height 1900. |
| pfUSDC | A PFTL-side, source-labeled vault-bridge receipt used as countable cash for NAVCoin settlement. | Ethereum-mainnet ingress/egress is deployed (a complete `20m12s` latency run was proved; no real Ethereum deposit or withdrawal is recorded since 2026-11-21). Solana deposits and, since 2026-11-23, withdrawals run with small caps. The older Arbitrum route is deprecated. |

## Today

Read-only `nav_reserve_proof_status` on validator 0, 2026-11-25 10:50 UTC,
height 1900:

| Asset | Newest finalized reading | Proof profile source class |
|---|---|---|
| B24 | epoch 9, `nav_per_unit` 1006419 (NAV 1.006419), submitted at height 1842 (23 November) | `b23-schwab-account-equity-nitro-sp1-v1` |
| A666 | epoch 9, `nav_per_unit` 116736912, submitted at height 1108 (October) | `stakehub-six-leg-reserves-v3` |
| BMNRC | epoch 1, `nav_per_unit` 100000000, submitted at height 1099 (2 October) | `manifest-driven` |

All three profiles have a 128-block `max_epoch_gap_blocks`. Create and cash-in
at NAV (`nav_mint_at_nav`, `nav_redeem_at_nav`) fail closed with
`nav_reserve_stale_deadman` once the chain is more than that many blocks past
the newest reading's finalize height (`ensure_nav_asset_live_for_epoch` in
`crates/execution/src/nav_vault_asset_execution.rs`). The chain counts blocks,
not time, so on an idle chain an old reading stays usable longer.

- **B24.** The B24 reading of the Schwab account's equity is taken in a Nitro
  enclave and proven with SP1. The daily job (`ops/b24-live/daily.py`, timer
  `ops/b24-live/b24-tee-nav-daily.timer`, 17:55 UTC) has a $9-per-run and
  $250 cap, does not retry, and counts a reading only once it is read back
  finalized. Readings landed at heights 1323 to 1842 (epochs 1 to 9); none
  landed on 24 or 25 November up to height 1900. The job pins a node binary
  (`DAILY_NODE_SHA`, `69de5ce0…`) that is not the fleet's (`6dd2ee56…` on
  25 November), so its runtime check stops it until the pin moves
  ([25 November handoff](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/status/handoff-2026-11-25.md)).
  The 24 November handoff puts the end of create and cash-in on the epoch-9
  reading at block 1943. B24 replaced the earlier B23 line
  (`ops/b24-replacement/`).
- **A666 and BMNRC.** Their readings are older than the 128-block gap at
  height 1900. A new reading is needed before NAV-priced create and cash-in can
  run on them.
- **pfUSDC.** Ethereum and Solana deposits mint separate pfUSDC series. Solana
  pfUSDC can be withdrawn to Solana since 23 November, within small caps; see
  [Network Status](../status/network-status.md#usdc-bridges).

## What the proof does and does not prove

NAVCoin proof-of-reserve primitives can prove:

- the admitted reserve packet matches a registered proof profile;
- packet arithmetic satisfies the NAV/supply invariant;
- the packet is fresh enough for the profile;
- deterministic source checks pass where the source is on-ledger or otherwise replayable;
- external-source observations have the required attestor quorum and no failing verdicts;
- mint, redeem, bridge, and market-operation actions respect finalized state.

They do not prove every possible off-ledger fact. A broker, exchange, bridge, or
custodian can still lie or fail outside the profile. A proof packet also proves
only the disclosed perimeter. Completeness, legal claim quality, and source
credit risk must be handled as explicit policy inputs, not hidden in the word
"proof".

## Watching that proofs keep arriving

A finalized NAV's age is counted in PFTL blocks, so on an idle chain an old NAV
can still look fresh, and nothing in the chain itself says when proofs stop.
The read-only [`tools/nav-proof-watch`](https://github.com/postfiatorg/postfiatl1v2/blob/main/tools/nav-proof-watch/README.md)
records a hash-chained log of the wallet API's routes and raises alarms when no
new proof epoch is first seen inside the observation window, when the gap
between new epochs exceeds the proof interval, when the chain stops advancing,
or when its own log stops. Its thresholds come from the product owner's
settlement terms; it has no defaults. It deliberately does not judge the
freshness of a NAV whose packet carries no observation time: it reports
"cannot be verified" instead of converting blocks into time. The same
directory has a read-only buyer exit capacity check.

## Architecture

The PFTL design separates backing, access, and privacy:

```text
reserve evidence
  -> proof profile
  -> reserve packet
  -> NAV epoch finalization
  -> primary mint/redeem, offer-book trading, vault bridge, shielded swap, or venue ops
```

Backing is global to the NAVCoin instance. Access can be local: PFTL native
balances, Ethereum wA666/USDC venues, source-chain vaults, or shielded
Asset-Orchard notes. Local liquidity is market depth, not a separate backing
pool.

Privacy lives at the transfer and swap layer. Reserve packets, counted cash
receipts, supply changes, and NAV epochs remain auditable. In the current
Asset-Orchard path, internal shielded swaps hide raw asset ids, values, owners,
recipients, and price. Boundary actions still disclose the public asset/value
entering or leaving the shielded pool.

## Reading order

0. [NAVCoin Create and Swap: Product and Build Specification](navcoin-create-and-swap-spec.md)
   specifies the next product: creating stock-plus-perp NAVCoins in the wallet,
   buying and selling them at NAV, bridging them to Ethereum, and swapping them
   privately. It starts with the BMNR carry NAVCoin (BMNRC), and it contains
   the build plan, launch gates, and capital record.
1. [A666 Current State](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md)
   records the deployed pfUSDC/A666/private-swap/bridge/Uniswap product and
   exact remaining release gates.
2. [Deferred NAVCoin Reserve Redemption System](../deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md)
   specifies the proposed registry-driven, trustlessly executable two-sided
   primary facilities for `pfUSD`, `pfXRP`, `pfETH`, `pfStakedETH`, and
   `pfBTC`. It is future architecture, not current deployment state.
3. [Canonical Primary-Market Accounting](primary-market-accounting.md)
   defines the adopted subscription, redemption, capacity, fee, bridge, and
   market-support invariants.
4. [Proof-of-Reserve Primitives](reserve-primitives.md) explains proof profiles,
   reserve packets, attestors, challenges, and the native NAV transaction path.
5. [YOLO Options Reserve Profile](yolo-options-reserve-profile.md) defines the
   pre-production brokerage commitment adapter and its deliberately attested
   trust classification. The consensus receipt that records a proved target on PFTL is
   specified in [YOLO Target Receipt v1](../yolo/target-receipt-v1.md).
6. [Assets And Venues](assets-and-venues.md) documents a651, a652, a666,
   pfUSDC, the
   Ethereum venues, and the bridge/market-operation contracts.
7. [a651 Uniswap Pool](uniswap-pool.md) retains the legacy a651 venue, pool,
   launch, and migration history.
8. [PFTL Tools](pftl-tools.md) maps the scripts, CLI commands, Python modules,
   and Solidity contracts to the evidence they produce.
9. [Reference Posts](references.md) links the public NAVCoin series and the
   deeper local implementation documents.
