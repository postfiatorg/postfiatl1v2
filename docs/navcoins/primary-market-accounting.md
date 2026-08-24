# Canonical NAVCoin Primary-Market Accounting

**Adopted:** 2026-07-27  
**Scope:** NAVCoin economics, a666 implementation, documentation, wallet copy,
and deployment gates  
**Precedence:** This document controls when an older NAVCoin document describes
dealer inventory, a separate redemption bucket, or market operations as a
substitute for primary redemption.

> **Architecture boundary (2026-07-30):** This document remains canonical for
> the deployed A666 single-settlement-asset primary market. The proposed
> [Deferred NAVCoin Reserve Redemption System](../deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md)
> generalizes that accounting into separately escrowed, two-sided facilities
> for exact approved assets in the `pfUSD`, `pfXRP`, `pfETH`, `pfStakedETH`,
> and `pfBTC` families. NRRS is not yet deployed and does not silently change
> A666 v2 state or economics.

## One-sentence model

Primary subscription moves the subscriber's counted settlement value into the
NAVCoin reserve and creates new NAVCoin; primary redemption retires NAVCoin and
releases the corresponding value from that same reserve.

```text
counted reserves in  <->  valid NAVCoin supply out
```

No second pool of redemption principal is required.

The a666 launch has one explicit successor-conversion exception to the normal
subscription flow. Fresh finalized reserve-proof value can create the opening a666
supply, but every resulting wA666 atom is locked in the ownerless migration
contract. It becomes spendable only when the contract burns the corresponding
legacy a651 amount. Thus the opening issue changes denomination; it does not
create a second spendable claim on the same portfolio.

## System boundaries

| Layer | Responsibility |
|---|---|
| pfUSDC | Proof-backed, source-labeled settlement asset representing USDC locked on the source chain. |
| PFTL | Canonical reserve, NAV, primary issue/redeem, supply, nonce, policy, and bridge-accounting ledger. |
| a666 | PFTL-native pro-rata claim on one verified reserve portfolio. |
| wA666 | Ethereum representation of canonical a666 moved through a proof-backed supply bridge. |
| Uniswap | Optional secondary venue. It is not NAV, backing, primary issuance, or redemption principal. |

## Canonical state

Let:

```text
V = verified net assets attributable to NAVCoin holders
S = valid global NAVCoin supply across all representations and in-flight claims
N = finalized NAV per displayed unit
```

Then:

```text
N = V / S
```

Consensus uses integer atoms, checked wide intermediates, canonical encodings,
and explicit floor/ceiling rules. It never uses floating point.

## Primary issue

For `x` new units:

```text
base_value = ceil(x * finalized_pre_inflow_NAV)
user_due   = ceil(base_value * issue_multiplier)
issue_fee  = user_due - base_value
```

Atomic transition:

```text
subscriber pfUSDC        -= user_due
NAV reserve principal    += base_value
non-NAV fee custody      += issue_fee
valid global supply      += x
subscriber a666          += x
```

The a666 launch multiplier is:

```text
issue_multiplier = 10050 / 10000
```

The fee is separate from holder backing under the current product decision.
If a future policy gives the fee to holders, that policy must state the
resulting NAV accretion explicitly.

## a651 successor conversion

At the 2026-07-27 historical internal-operator snapshot:

```text
verified net assets       = $31,386.19745591
opening a666 supply       = 31,386.197455
opening a666 NAV          = $1.000000
legacy a651 supply        = 4,000.000000000000000000
fixed conversion ratio    = 31,386.197455 a666 / 4,000 a651
rounding overcollateral   = $0.00000091
```

PFTL mints the opening supply to one route-inventory holder only after the
fresh registered reserve proof is finalized. Route initialization must prove that the
holder balance, issued supply, and NAV circulating supply are exactly equal.
The full opening supply is then exported to
`A651ToA666MigrationV1`, not to the operator.

The migration contract:

- has no owner and no mutable ratio;
- cannot mint either token;
- burns a651 through its existing primary controller before releasing wA666;
- reverts atomically if successor inventory is insufficient; and
- leaves the PoolManager and external-holder allocations locked until their
  corresponding legacy a651 can actually be burned.

The old a651 pool has zero active position liquidity and is deprecated. Its
token remains a legacy claim only for purposes of this burn-for-successor
conversion; it is not valid pool seed, primary inventory, or independent
backing for a666.

## Primary redemption

For `x` returned units:

```text
base_value = floor(x * finalized_pre_outflow_NAV)
user_out   = floor(base_value * redeem_multiplier)
redeem_fee = base_value - user_out
```

Atomic transition:

```text
holder a666              -= x
valid global supply      -= x
NAV reserve principal    -= base_value
holder pfUSDC            += user_out
non-NAV fee custody      += redeem_fee
```

The a666 launch multiplier is:

```text
redeem_multiplier = 9995 / 10000
```

The principal released is the subscription-funded NAV reserve principal. A
separate `redemption_fund`, `redemption_bucket`, or launch inventory deposit is
not part of this model.

## Capacity

The `2,000,000 a666` issue and redeem values are policy ceilings:

- issue/redeem units per epoch;
- order-size and rate limits;
- risk and operational throughput limits;
- quote validity and freshness bounds.

They are not:

- lifetime supply maxima;
- balances that must be prefunded twice;
- promises to redeem more supply than exists;
- permission to ignore actual reserve liquidity.

Available issue capacity can be large before the reserve holds the user's
principal because the user brings that principal in the same atomic
subscription.

Available redemption is:

```text
min(
  remaining policy redemption capacity,
  valid redeemable supply,
  unencumbered NAV reserve settlement principal,
  reserve liquidity available under policy,
  per-order limit
)
```

The quote API must distinguish the policy ceiling from the currently available
amount.

## Million-dollar example

Initial state:

```text
V = 100,000 pfUSDC
S = 100,000 a666
N = 1.000000 pfUSDC/a666
```

Ignoring spread for the simplest accounting example, Bob contributes
`1,000,000 pfUSDC`:

```text
V = 1,100,000 pfUSDC
S = 1,100,000 a666
N = 1.000000 pfUSDC/a666
```

With the `1.005` quote, an order for `1,000,000 a666` instead requires:

```text
base NAV reserve principal = 1,000,000 pfUSDC
non-NAV issue fee           =     5,000 pfUSDC
total user input            = 1,005,000 pfUSDC
```

The Uniswap pool is not touched, so a `$30,000` pool does not constrain the
fill.

If those `1,000,000 a666` are later redeemed at `0.9995` while NAV is still
`1.00`:

```text
NAV reserve principal removed = 1,000,000 pfUSDC
user receives                 =   999,500 pfUSDC
non-NAV redemption fee        =       500 pfUSDC
supply retired                = 1,000,000 a666
```

No separate million-dollar redemption inventory was needed.

## Primary, secondary, and bridge transactions

| Operation | Reserves | Global supply | Existing seller/LP inventory |
|---|---:|---:|---:|
| Primary subscription | Increase | Increase | Not required |
| Primary redemption | Decrease | Decrease | Not required |
| Secondary OTC | Unchanged | Unchanged | Required |
| Uniswap trade | Unchanged | Unchanged | Required |
| PFTL/Ethereum bridge | Unchanged | Unchanged | Operator inventory not required |

The product must not label a reserve-forming primary subscription as OTC.

## Bridge invariant

Bridge-out changes representation:

```text
native a666 debited or locked
  -> finalized PFTL receipt
  -> Ethereum verifies proof
  -> equal wA666 amount minted or released
```

Bridge-in reverses that movement.

At all times:

```text
PFTL spendable a666
+ Ethereum spendable wA666
+ outstanding bridge claims
= canonical authorized supply
```

Subject to temporary state terms explicitly defined by the bridge lifecycle,
no bridge operation changes NAV reserve assets or creates global economic
supply.

## Optional alignment reserve

An alignment reserve may fund bounded buys or sells in secondary venues. It is
never:

- NAV backing;
- subscription inventory;
- primary redemption principal;
- a condition for recognizing subscription-funded reserve value.

Primary mint/redeem and optional market support use separate policies and
separate accounting.

## Failure behavior

Issue/redeem fails closed on:

- stale or mismatched NAV/reserve packet;
- arithmetic overflow or invalid rounding domain;
- insufficient user balance;
- insufficient valid supply for redemption;
- insufficient unencumbered NAV reserve principal or permitted liquidity;
- exhausted order/epoch capacity;
- paused route;
- replayed nonce;
- inconsistent global supply or reserve state.

Failure never authorizes an unbacked mint, synthetic pfUSDC credit, manual
balance repair, or hidden operator discretion.

## Documentation and implementation rule

Any field or operation named like:

```text
required_redemption_liquidity
redemption_fund
redemption_bucket
prefunded_redemption
```

must either be removed from a666 or explicitly documented as a legacy/optional
secondary-market mechanism. It must not gate primary redemption independently
of the subscription-funded NAV reserve principal.
