# NAVCoins

> **Current product status (2026-07-30):** A666 v2, Ethereum-mainnet pfUSDC,
> proof-gated wA666, private PFTL issue/redeem, and the wA666/USDC Uniswap v4
> venue are deployed and functionally proven. The service remains limited
> availability rather than production GA. Start with
> [A666 Current State](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).

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
| NAVCoin | The asset class and protocol pattern: proof profile, reserve packet, supply discipline, mint/redeem controls, and market-operation limits. | Implemented on PFTL; A666 v2 is the deployed production lineage, with release hardening still open. |
| a651 | The first named NAVCoin instance for the proven six-leg reserve portfolio. | Legacy/deprecated product lineage. Its historical Ethereum pool had zero pool-specific liquidity at the last a651 inspection; a651 remains migration and research history. |
| a652 | A second NAVCoin instance used to prove cross-NAVCoin swap mechanics. | WAN devnet evidence only, not a production public asset. |
| a666 | The deployed NAVCoin lineage for large, low-slippage primary subscriptions, symmetric primary redemption, and proof-backed Ethereum representation. | PFTL A666 v2, wA666, and the wA666/USDC pool are deployed; transparent/private flows work; public GA remains closed. |
| pfUSDC | A PFTL-side, source-labeled vault-bridge receipt used as countable cash for NAVCoin settlement. | Direct Ethereum-mainnet ingress/egress is deployed and proved a complete `20m12s` latency run; the older Arbitrum route is deprecated. |

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
5. [Assets And Venues](assets-and-venues.md) documents a651, a652, a666,
   pfUSDC, the
   Ethereum venues, and the bridge/market-operation contracts.
6. [a651 Uniswap Pool](uniswap-pool.md) retains the legacy a651 venue, pool,
   launch, and migration history.
7. [PFTL Tools](pftl-tools.md) maps the scripts, CLI commands, Python modules,
   and Solidity contracts to the evidence they produce.
8. [Reference Posts](references.md) links the public NAVCoin series and the
   deeper local implementation documents.
