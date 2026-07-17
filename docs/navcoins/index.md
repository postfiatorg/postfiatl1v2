# NAVCoins

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
| NAVCoin | The asset class and protocol pattern: proof profile, reserve packet, supply discipline, mint/redeem controls, and market-operation limits. | Implemented on PFTL devnet paths, with proof-profile and bridge work in progress. |
| a651 | The first named NAVCoin instance for the proven six-leg reserve portfolio. | Registered and exercised on the PFTL WAN devnet; also launched on Ethereum as a live a651/USDC venue/representation with the caveats in [Assets And Venues](assets-and-venues.md). |
| a652 | A second NAVCoin instance used to prove cross-NAVCoin swap mechanics. | WAN devnet evidence only, not a production public asset. |
| pfUSDC | A PFTL-side, source-labeled vault-bridge receipt used as countable cash for NAVCoin settlement. | Implemented through the generic vault-bridge/NAV profile path; not a separate hardcoded stablecoin subsystem. |

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
balances, Ethereum a651/USDC venues, source-chain vaults, or shielded
Asset-Orchard notes. Local liquidity is market depth, not a separate backing
pool.

Privacy lives at the transfer and swap layer. Reserve packets, counted cash
receipts, supply changes, and NAV epochs remain auditable. In the current
Asset-Orchard path, internal shielded swaps hide raw asset ids, values, owners,
recipients, and price. Boundary actions still disclose the public asset/value
entering or leaving the shielded pool.

## Reading order

1. [Proof-of-Reserve Primitives](reserve-primitives.md) explains proof profiles,
   reserve packets, attestors, challenges, and the native NAV transaction path.
2. [a651 Uniswap Pool](uniswap-pool.md) documents the live Ethereum a651/USDC
   Uniswap v4 venue, pool id, addresses, launch configuration, and caveats.
3. [Assets And Venues](assets-and-venues.md) documents a651, a652, pfUSDC, the
   Ethereum/Uniswap a651 venue, and the bridge/market-operation contracts.
4. [PFTL Tools](pftl-tools.md) maps the scripts, CLI commands, Python modules,
   and Solidity contracts to the evidence they produce.
5. [Reference Posts](references.md) links the public NAVCoin series and the
   deeper local implementation documents.
