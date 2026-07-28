# a666 End-to-End Mainnet Primary Issuance and Redemption Spec

**Date:** 2026-07-27  
**Priority:** P0  
**Status:** canonical specification; opening inventory proof-exported to
Ethereum and the Uniswap venue seeded/trading; new-subscription and redemption
acceptance campaigns remain
**Owner:** Post Fiat protocol and product release owners  
**Change control:** any normative change requires a dated amendment, a new
document hash, and a fresh acceptance run  
**Product:** Ethereum USDC -> pfUSDC -> newly issued a666 -> Ethereum wA666,
with the inverse redemption path  
**Latency SLO:** no more than 25 minutes from included user deposit
transaction to spendable wA666 under the supported operating envelope

This document is the binding implementation, deployment, and acceptance
specification for the a666 launch. It replaces the program outline in
`A666-MAINNET-TRUSTLESS-MINT-SPEC-20260725.md` where that outline conflicts
with this document. It also supersedes any a651-era assumption that a large
buyer must acquire existing inventory, traverse the Uniswap curve, or depend
on an owner-authorized Ethereum mint.

The deployment is now partially live. On 2026-07-27, production a666 v2 was
created on the six-validator PFTL WAN fleet with no permanent maximum supply.
The fleet finalized the StakeHub reserve proof, epoch-one `$1.00` NAV mark, and
an opening supply of `31,386.197455 a666` against `$31,386.19745591` of
verified net assets. On Ethereum mainnet, the wA666 token, SP1 receipt
verifier, proof-gated primary controller, and ownerless a651-to-a666 migration
contract were deployed and their immutable/controller bindings verified. On
2026-07-28, the full opening inventory was source-debited on PFTL at height
`348`, proven with a genuine SP1 Groth16 proof, and minted exactly once as
`31,386.197455 wA666` to the ownerless migration contract. Burning
`382.333668078301459218 a651` released exactly `3,000 wA666`; that amount and
`3,000 USDC` seeded the official Uniswap v4 PositionManager. Third-party swaps
began immediately and all temporary token/Permit2 allowances were revoked.

This does **not** yet mean the complete public primary-issuance product is
generally available. The opening migration and secondary venue are live, but
a fresh buyer-funded subscription, its 25-minute end-to-end measurement, and
the inverse primary-redemption campaign remain incomplete. See
`../status/A666-MAINNET-DEPLOYMENT-20260727.md` for deployed identifiers,
transactions, evidence, and the exact remaining gates.

### Normative language

`MUST`, `MUST NOT`, `REQUIRED`, and declarative “must” statements are
normative. `SHOULD` is a default that needs a recorded exception.
`MAY` is optional. Examples, research history, current-state descriptions,
and rationale are non-normative unless a requirement explicitly incorporates
them. If repeated values disagree, the canonical parameter table below wins.

### Glossary

| Term | Meaning |
|---|---|
| PFTL | The Post Fiat L1 and canonical a666 NAV/supply ledger. |
| NAV | Finalized verified net asset value per valid a666 unit. |
| pfUSDC | PFTL-issued representation of source-labeled Ethereum-mainnet USDC. |
| a666 | The PFTL-native NAVCoin liability and canonical supply unit. |
| wA666 | The Ethereum ERC-20 representation of exported a666. |
| SP1 | The succinct proof system used to verify Ethereum ingress or PFTL finality without an owner assertion. |
| `TRUSTLESS_FINALITY` | PFTL-to-Ethereum verification by a valid SP1 proof under immutable bindings. |
| `BFT_CHECKPOINT` | Ethereum-to-PFTL verification using a PFTL-validator checkpoint certificate plus receipt-trie inclusion. |
| Primary issue | New a666 created against newly counted reserve value. |
| Primary redemption | Existing a666 retired while the corresponding reserve value is released. |
| NAV reserve principal | Counted settlement value contributed through primary subscriptions and attributable to valid a666 holders. It is the source of primary redemption principal. |
| Secondary trade | Existing a666/wA666 changes owners; supply and backing do not grow. |

### Canonical launch parameters

| Requirement ID | Parameter | Binding value |
|---|---|---:|
| ECON-001 | Production asset version | `2` |
| ECON-002 | a666/wA666 precision | `6` |
| ECON-003 | Permanent asset maximum | none (`max_supply=None`) |
| ECON-004 | Issue multiplier | `10050 / 10000` |
| ECON-005 | Redemption multiplier | `9995 / 10000` |
| CAP-001 | Issue capacity per policy epoch | `2,000,000 a666` |
| CAP-002 | Redemption capacity per policy epoch | `2,000,000 a666` |
| CAP-003 | Maximum user order | `1,000,000 a666` |
| CAP-004 | Export packet cap | `250,000 a666` |
| CAP-005 | Net wrapped exposure cap | `2,000,000 a666` |
| VENUE-001 | Uniswap pair | wA666 / canonical mainnet USDC |
| VENUE-002 | Uniswap v4 fee | `500` (`0.05%`) |
| TRUST-001 | PFTL -> Ethereum | `TRUSTLESS_FINALITY` |
| TRUST-002 | Ethereum -> PFTL | `BFT_CHECKPOINT` |
| SLO-001 | Deposit inclusion -> spendable wA666 | no more than 25 minutes |

Operational values such as block-denominated proof freshness, reservation
expiry, and gas ceilings MUST be frozen in the G0 deployment parameter sheet
because they depend on the measured final fleet cadence. They are not
alternative architectural designs.

## 1. Binding outcome

Bob has canonical Ethereum-mainnet USDC. Bob does not know Alex and does not
negotiate with Alex. Bob chooses the primary a666 route and submits a
user-bound order.

At a finalized pre-inflow NAV of `$1.00` and the posted issue multiplier of
`1.005`:

```text
Bob pays:                       100,500 USDC
New a666 valid supply:          100,000 a666
Bob receives on mainnet:        100,000 wA666
AMM price impact on the fill:   $0
Uniswap liquidity consumed:     0
```

Of the 100,500 USDC, 100,000 becomes counted NAV reserve value and the 500
spread is posted to separately disclosed non-NAV fee custody that is
excluded from NAV assets. The protocol creates 100,000 new a666 under the
finalized NAV and subscription policy. This preserves the stated `$1.00` NAV
instead of silently accreting the spread into it. PFTL then removes those units
from native spendability and authorizes exactly 100,000 wA666 on Ethereum. Bob
receives wA666 directly; operating a PFTL wallet is an internal implementation
detail, not a user requirement.

The inverse route is also required. While a posted redemption envelope is
active, Bob can burn wA666, return the native claim to PFTL, retire a666
supply, receive pfUSDC at the posted `NAV × 0.9995` price, and withdraw
mainnet USDC through the existing proof-native pfUSDC rail.

Launch capacity is:

```text
posted primary issuance capacity:   2,000,000 a666
posted primary redemption capacity: 2,000,000 a666
required supported single order:    1,000,000 a666
required supported single export:     100,000 a666
```

These are active policy capacities. They are not lifetime supply ceilings.

## 2. Explicit product decisions

### 2.1 No permanent a666 maximum supply

The production a666 asset definition must set:

```text
max_supply = None
precision  = 6
```

Supply expands when verified net assets and a valid primary subscription
expand together. Supply contracts when a valid redemption retires units and
releases reserve value. The economic constraints are fresh reserve proof,
post-transition backing, active issuance/redemption capacity, per-order
limits, and cross-venue exposure—not an arbitrary lifetime token maximum.

The existing controlled a666 definition has a 1,000,000-a666 static maximum.
It is a test artifact and cannot serve as the production lineage because the
current transaction set has no asset-definition update that safely removes
that maximum. Production therefore uses a fresh asset version and asset ID.

### 2.2 This is primary issuance, not OTC

Bob's pfUSDC is not exchanged for Alex's existing a666 inventory. The
subscription increases both counted reserve value and authorized valid a666
supply in one atomic PFTL transition.

Secondary OTC trading remains a separate, inventory-bounded product. It must
not be silently selected as a fallback when the primary route is paused or
full.

### 2.3 Uniswap is a venue, not the issuance mechanism

The new wA666/USDC pool provides price discovery, transfers, LP access, and a
secondary exit. It does not determine NAV and does not determine primary
capacity.

A `$30,000` pool can coexist with a `$100,000` or `$1,000,000` primary
subscription because the primary fill does not trade through the pool.

### 2.4 Redemption is symmetric, bounded, and executable

The launch includes a standing *posted facility* of up to 2,000,000 a666 at
`NAV × 0.9995`, subject to its active epoch, remaining capacity, freshness,
and sufficient unencumbered settlement value in the a666 NAV reserve.

The principal is not funded twice. Primary subscriptions place base settlement
value into the NAV reserve as they create supply. Primary redemption removes
that reserve value as it retires supply. The 2,000,000 figure is an
order/epoch policy limit, not a requirement for a separate 2,000,000-pfUSDC
inventory deposit.

This is stronger than discretionary market support: while the policy is
active and the reserve/freshness/capacity checks pass, a conforming user order
cannot be faded by an issuer. Optional alignment-reserve market operations are
separate from this primary facility.

Here, “trustless redemption” means Alex or another issuer operator cannot
choose whether to honor a conforming, funded order. It does not mean the user
stops relying on PFTL consensus. The current Ethereum-return verifier relies
on a disclosed PFTL-validator `BFT_CHECKPOINT` for the Ethereum header plus a
cryptographic receipt proof. The wallet must show that boundary.

### 2.5 Privacy is not a launch dependency

Orchard can later shield pfUSDC or a666 during the PFTL portion of the route.
The first production route is transparent because the critical product is
large, low-slippage primary acquisition with proof-checked supply. Privacy
must not delay or weaken that path.

## 3. Research review and resulting decisions

The following research in the sibling
`postfiatorg.github.io/content/` repository was reviewed before writing this
spec.

| Research | Decision carried into this spec |
|---|---|
| `research/canonical-navcoin-transaction.md` | PFTL is the canonical supply/NAV ledger. Native supply is authorized first; Ethereum receives only a verified representation. Uniswap is not the NAV oracle. Return requires an Ethereum burn proof. |
| `research/trustless-pftl-uniswap-bridges.md` | Every packet is domain-separated and replay-protected; consume and refund are mutually exclusive; public trustless labeling requires direct or succinct finality, not an owner toggle. |
| `research/trustless-wrapped-stablecoins.md` | The pfUSDC conservation identity is `V = S + D + B - R`; in-flight deposit and redemption terms must remain explicit. Its old Arbitrum/Tier-1 current-state description is superseded by the proven Ethereum-mainnet SP1 route. |
| `research/private-nav-otc-swaps.md` | Primary subscription adds user cash to reserves and creates new NAVCoin supply. Secondary OTC merely transfers existing inventory and does not form TVL. This product uses the former. |
| `research/private-otc-swaps.md` | The earlier transparent and Orchard flows were small, self-published devnet evidence with bridge-out deferred in the transparent run. They are useful controls, not mainnet or large-capacity proof. |
| `research/private-nav-swap-explainer.md` | A `$1M` subscription can grow a `$100k` reserve/supply base to `$1.1M` without changing a `$1.00` NAV when assets and liabilities grow together. Pool depth is irrelevant to the primary fill. |
| `research/proven-private-swap.md` | Privacy is useful but does not replace reserve provenance, supply accounting, replay protection, or public boundary proofs. |
| `blog/navcoin-proposal.md` | Freshness, proof-profile identity, reserve/supply arithmetic, and redemption deadlines belong in validity rules. Proof verifies the declared sources and computation; it cannot prove that a source is honest or that no liability was omitted. |
| `blog/navcoin-ethereum.md` | There is one global reserve portfolio and one valid global supply across local access venues. Pool-level liquidity is not local backing. |
| `blog/navcoin-collateralization.md` | Primary subscription and redemption are symmetric reserve/supply transitions. Subscription-funded reserve principal funds redemption; policy capacity and optional market-support budgets are separate quantities. |
| `blog/pfusdc-trustless-bridge.md` | Source-labeled pfUSDC is the settlement primitive. Its Arbitrum testnet architecture is historical; Ethereum-mainnet evidence is now canonical. |

Two older research assumptions are intentionally not carried forward:

1. a651 is not the production Ethereum representation.
2. discretionary market operations without a posted redemption facility are
   insufficient for the a666 launch requirement.

## 4. Current system truth

| Component | Current status | Launch consequence |
|---|---|---|
| Arbitrum pfUSDC design | The proof-correct Nitro assertion path required approximately 6.4 days before finalized acceptance. | Rejected for this product. Do not register or fall back to an Arbitrum a666 route. |
| Ethereum-mainnet USDC -> pfUSDC | Real round trip passed with SP1 proofs, exact conservation, and replay rejection. Replacement latency run passed in 20m12s. | Reuse the deployed mainnet pfUSDC rail and frozen proof lineage. |
| pfUSDC ingress gas | Approval plus deposit used about 271k gas: 0.000015740899 ETH in the 25-USDC run and 0.000081604399 ETH in the 1-USDC latency run, approximately `$0.03` and `$0.15` at the campaign-pinned `$1,874.50/ETH`. | Quote live gas; do not scale gas estimates with principal size. Proof compute and later transactions are separate. |
| Former controlled a666 | Six decimals; test asset maximum 1,000,000 a666; route cap 10 a666; packet cap 1 a666; controlled/test supply only. | Superseded by production a666 v2. Do not mutate or relabel the controlled lineage. |
| PFTL primary subscription | Atomic settlement debit and a666 credit exist, with replayed nonce and supply conservation. | Extend it with rational spread pricing, policy envelopes, user limits, and redemption symmetry. |
| PFTL export/refund/return | Export, destination consume, cancellation-based refund, and Ethereum return-import state machines exist. BFT-checkpoint routes verify Ethereum receipt-trie logs. | Reuse and harden; do not replace with manual operator assertions. |
| Ethereum wA666 stack | Production wA666, receipt verifier, proof-gated controller, and ownerless a651 migration are deployed with immutable bindings. The finalized opening export proof was accepted and wrapped supply is `31,386.197455`. | Run fresh primary-subscription and redemption acceptance campaigns; no owner mint is available. |
| Ethereum PFTL finality verifier | `PFTLFinalityVerifierV1` works for pfUSDC withdrawals but is hard-bound to the pfUSDC route, vault caller, token, and withdrawal public-value schema. | Build a receipt verifier for a666 export receipts. The pfUSDC verifier cannot be reused as configuration. |
| Mainnet wA666/USDC pool | Hookless v4 pool `0xc5f1…6e98` was initialized at Q96/`$1.00`, fee `500`, tick spacing `10`, then seeded with `3,000 USDC` and `3,000` proof-exported/migrated wA666. Third-party swaps are finalized. | Treat it only as a secondary venue. Live pool price never controls NAV or primary issue/redemption arithmetic. |
| a651 mainnet pool | Historical standalone token/controller/pool. Current position liquidity is zero; 4,000 a651 remains distributed between the operator, PoolManager, and external holders. | Deprecate the pool. Use a651 only as the burn input to the fixed-ratio successor contract; never use it as pool seed or an independently backed live product. |
| Current redemption | `nav_redeem_at_nav` creates a claim, but settlement requires issuer/redemption-account action. It does not encode the 0.9995 band or guarantee permissionless execution from NAV reserve custody. | Add a policy-bound atomic primary redemption transition that releases subscription-funded reserve principal. |

Canonical pfUSDC mainnet contracts:

```text
vault:    0x8583409ddbac984ec195dfa06a21103d92403c1e
verifier: 0xa77d5af456ef212303e31727b6ca4888cd771e2c
```

These addresses are inputs to a read-back, not authorization to assume that
their state or code is unchanged. Deployment preflight must re-read chain ID,
runtime bytecode, code hashes, immutable bindings, and live route state.

### Threat model

| Adversary or failure | Required defense |
|---|---|
| User, relayer, or operator mutates an order or packet | User signature, full field/domain binding, canonical encoding, and exact receipt inclusion. |
| Packet, receipt, proof, nonce, burn, or refund is replayed | Persistent consume-once registries on both chains and replay tests across restart/snapshot recovery. |
| Issuer attempts an unbacked mint or fades a conforming redemption | Consensus-derived pricing/backing checks and an atomic redemption transition from NAV reserve custody with no issuer completion signature. |
| Ethereum or PFTL evidence is stale, forked, or forged | Freshness bounds, recognized checkpoint lineage, SP1 verification outbound, and BFT checkpoint plus receipt-trie verification on return. |
| Contract owner or relayer attempts an independent wA666 mint | Immutable locked token controller; relayers can only submit already-bound packets. |
| Thin-pool manipulation changes issuance price or capacity | PFTL NAV/policy is authoritative; Uniswap price and liquidity never enter primary issue/redeem arithmetic. |
| Process, prover, RPC, or validator crashes mid-workflow | Durable state machine, idempotency keys, bounded retries, explicit recovery states, and no duplicate proof job after timeout. |
| Reserve venue lies or liabilities are omitted | Not solved cryptographically; disclose source/proof profile and apply governance, legal, haircut, and status controls. |

The launch does not claim protection from compromised Ethereum or PFTL
consensus, a broken SP1/Solidity cryptographic stack, USDC issuer seizure, or
false source data that satisfies the declared reserve profile. Those are
explicit residual trust and catastrophic-risk boundaries.

## 5. End-to-end architecture

```text
Bob's Ethereum account
  |
  | 1. approve + deposit canonical mainnet USDC
  v
ERC20BridgeVaultL1
  |
  | 2. finalized Ethereum evidence + SP1 ingress proof
  v
PFTL credits spendable pfUSDC to Bob's route account
  |
  | 3. atomic primary subscription at pre-inflow NAV × 1.005
  |    debit pfUSDC reserve / create native a666
  v
PFTL export debit
  |
  | 4. native a666 becomes non-spendable; export receipt finalizes
  | 5. permissionless SP1 proof verifies PFTL finality + receipt inclusion
  v
PFTLReceiptFinalityVerifierV1 on Ethereum
  |
  | 6. PFTLUniswapHandoffController consumes once
  v
wA666 minted directly to Bob
  |
  +--> hold / transfer / provide liquidity / trade on wA666-USDC v4 pool
```

Redemption is the inverse:

```text
Bob burns wA666 for PFTL return
  -> finalized Ethereum receipt + governed PFTL checkpoint proof
  -> native a666 credited on PFTL
  -> atomic primary redemption at pre-outflow NAV × 0.9995
  -> a666 retired and subscription-funded NAV reserve pfUSDC released
  -> pfUSDC burn/exit proof
  -> mainnet vault releases USDC to Bob
```

The wallet may orchestrate these legs as one user intent, but no component may
pretend that the cross-chain route is one atomic transaction. Each leg must
have an explicit state, deadline, recovery action, and durable idempotency
key.

## 6. Canonical economic and supply invariants

### 6.1 pfUSDC conservation

The existing invariant remains:

```text
V = S + D + B - R
```

where `V` is source-vault balance, `S` is spendable issued pfUSDC, `D` is
accepted deposits not yet issued, `B` is burned pfUSDC not yet released, and
`R` is released but not yet settled redemptions.

### 6.2 a666 backing

For `UNIT_SCALE = 10^6`:

```text
verified_net_assets_usd_e8 * UNIT_SCALE
  >= valid_global_supply_atoms * nav_floor_usd_e8
```

For a primary subscription, settlement is counted and supply is created in
the same consensus transition:

```text
base_subscription_value = minted_a666 * pre_inflow_nav
net_assets_after         = net_assets_before + base_subscription_value
supply_after             = supply_before + minted_a666
```

For primary redemption, reserve principal and supply contract in the same
consensus transition:

```text
base_redemption_value = redeemed_a666 * pre_outflow_nav
net_assets_after      = net_assets_before - base_redemption_value
supply_after          = supply_before - redeemed_a666
```

The reserve principal released here is the same principal contributed through
primary subscriptions. No separate redemption asset or duplicate backing
allocation exists.

Pricing uses the finalized pre-inflow NAV. At launch, the issue spread is
credited to disclosed non-NAV fee custody so that equal base assets
and liabilities are added and NAV remains unchanged. A future policy may
count the spread into NAV only if it explicitly discloses the resulting NAV
accretion and passes the post-transition arithmetic.

### 6.3 Cross-venue supply

For every route state:

```text
pftl_spendable
  + ethereum_spendable
  + other_registered_venue_spendable
  + outstanding_export_claims
  + pending_return_claims
  = authorized_valid_supply
```

And globally:

```text
issued_asset_supply_all_custody_lanes
  = authorized_valid_supply_all_active_route_versions
```

An export changes location, not authorized valid supply. A return changes
location, not authorized valid supply. Only primary issue/redeem changes
authorized valid supply.

### 6.4 No independent Ethereum issuance

Every wA666 mint must consume one accepted PFTL export receipt. The wrapped
token controller is locked once, cannot be repointed, and cannot expose an
owner mint, emergency mint, upgrade mint, or inherited role that bypasses the
handoff controller.

### 6.5 No consume-and-refund

For one export packet, exactly one terminal branch is possible:

```text
CONSUMED_ON_ETHEREUM
or
CANCELLED_ON_ETHEREUM_AND_REFUNDED_ON_PFTL
```

Elapsed source-chain time is not a non-consumption proof. Refund requires
proof of the destination cancellation event, and the Ethereum replay registry
must make cancellation and consumption mutually exclusive.

### 6.6 Reserve-proof boundary

The production a666 NAV profile must be content-addressed, consensus-enforced,
fresh, and bound to the exact reserve source classes, valuation program,
haircuts, liabilities, proof encoding, verifier key, byte bounds, and expiry
rules. A placeholder or audit-only verifier cannot power issue or redemption.

“Verified reserves” means that the configured proof system verified the
declared source data and deterministic valuation policy. It does not prove
that a venue told the truth or that no undisclosed liability exists. Wallet
and status surfaces must publish the proof profile and source composition so
the product does not turn a narrow cryptographic claim into a blanket solvency
claim.

## 7. Production asset reset

### 7.1 Create a666 version 2

Create a new issued asset with:

```text
code:                   A666
version:                2
precision:              6
max_supply:             None
requires_authorization: true
freeze_enabled:         true
clawback_enabled:       false
```

The resulting 48-byte asset ID becomes the only production `native_nav_asset_id`.
All manifests, route digests, public values, wallet allowlists, controller
immutables, dashboards, and pool metadata bind this ID.

### 7.2 Retire the controlled lineage

Before public activation:

1. halt or leave disabled every controlled a666 v1 route;
2. account for its test supply and move it to an explicitly terminal,
   non-production state;
3. prove that no controlled export packet can be accepted by the production
   verifier or controller;
4. publish both asset IDs with `TEST/RETIRED` and `PRODUCTION` labels; and
5. reject the v1 ID in the wallet and deployment preflights.

No silent balance migration is allowed. Any economically real v1 holder must
receive an explicit, audited migration transaction against the same reserve
accounting.

### 7.3 Proof-backed successor bootstrap

The production opening state is determined by the fresh StakeHub proof, not a
round-number website narrative. The 2026-07-27 proof establishes:

```text
verified net assets:       $31,386.19745591
opening supply:             31,386.197455 a666
opening NAV:                $1.000000
rounding overcollateral:    $0.00000091
```

This is a successor denomination of the 4,000-unit a651 claim. It is not a
founder mint and it is not an ordinary new-cash subscription. Consensus must
finalize the StakeHub proof, mint the exact opening supply to one PFTL
inventory holder, and initialize the route only when that holder balance,
global issued supply, and NAV circulating supply are identical.

The entire opening supply is exported to an ownerless fixed-ratio migration
contract. That contract releases wA666 only after burning a651 through the
legacy primary controller. The fixed ratio is:

```text
31,386.197455 a666 / 4,000.000000000000000000 a651
```

Until the corresponding a651 is burned, successor inventory remains locked
and nonspendable. This prevents the fresh portfolio proof from backing two
spendable liabilities at once. PoolManager-held and external-holder successor
allocations remain reserved, not reassigned to the operator.

## 8. Primary issuance and redemption policy

### 8.1 New policy object

Add a governance-versioned `NavPrimaryMarketPolicy` bound to:

```text
asset_id
policy_epoch
settlement_asset_id
issue_multiplier_bps       = 10050
redeem_multiplier_bps      = 9995
issue_capacity_atoms       = 2_000_000 * 10^6
redeem_capacity_atoms      = 2_000_000 * 10^6
max_order_atoms            = 1_000_000 * 10^6
minimum_order_atoms        = 1 * 10^6
valid_from_height
expires_at_height
max_nav_age_blocks
reserve_packet_hash
nav_epoch
policy_hash
```

Capacity consumption and restoration use this launch rule per policy epoch:

```text
issue_remaining  = issue_capacity  - issued_in_epoch
redeem_remaining = redeem_capacity - redeemed_in_epoch
```

A new policy epoch can replenish or change the posted facility after a fresh
reserve/capacity proof. It does not change lifetime supply history.

### 8.2 Integer pricing

Do not represent the spread as “settlement atoms per NAV atom.” At six
decimals that field is `1` at par and cannot encode `1.005`.

Use rational integer arithmetic:

```text
base_value_atoms =
  ceil(mint_amount_atoms * nav_price_numerator / nav_price_denominator)

issue_due_atoms =
  ceil(base_value_atoms * 10050 / 10000)

redeem_out_atoms =
  floor(base_value_atoms * 9995 / 10000)
```

The NAV numerator/denominator must account for both asset precisions and the
NAV valuation scale without intermediate truncation. All multiplications use
checked `u128` or wider arithmetic and downcast only after bounds checks.

The issue spread is:

```text
issue_due - base_value
```

For the launch policy it is assigned to a disclosed non-NAV fee account
excluded from verified NAV assets. It must never disappear into an off-ledger
operator balance.

For redemption, the full base NAV value leaves the counted reserve
composition:

```text
base_value = user_pfUSDC_out + redemption_spread
```

The user receives `redeem_out`; the difference is posted to the same disclosed
non-NAV fee accounting. This keeps the post-redemption NAV unchanged
at the stated example price.

### 8.3 User-bound issue order

Replace or version `PftlUniswapPrimarySubscribeOperation` so the signed order
binds:

```text
subscriber
beneficial PFTL recipient
Ethereum recipient
route_id and route_epoch
asset_id and settlement_asset_id
requested mint amount
maximum settlement input
issue multiplier
pricing NAV epoch and reserve packet hash
primary-market policy hash
subscription nonce
expiry height/time
export requested flag
```

Consensus derives the exact settlement debit. The user does not assert the
price as an authority. Any worse price, stale epoch, expired policy, changed
recipient, changed route, insufficient capacity, insufficient backing, or
partial fill rejects before mutation.

For the direct-to-Ethereum product, subscription and export may be separate
consensus transitions under one durable workflow, but the wallet must reserve
the full order and export capacity before taking the user's deposit. Add a
bounded `PrimaryOrderReservation` state keyed by the signed order hash. It
locks the policy/NAV epoch, issue capacity, export exposure, amount, Ethereum
recipient, and an expiry long enough to cover the 25-minute SLO plus recovery
margin. The deposit event references the same order hash.

Reservations need deterministic anti-grief rules: a per-wallet limit, a
global reserved-capacity limit, a short governed maximum lifetime, and either
a bond or an immediately submitted matching deposit. Expiry releases capacity
without creating supply. A partial state exposes a user-visible recovery
action.

### 8.4 Permissionless primary redemption

Add a versioned primary redemption operation that:

1. is signed by the a666 owner;
2. binds the active policy, finalized NAV, minimum pfUSDC output, amount,
   destination, nonce, and expiry;
3. proves sufficient unencumbered pfUSDC settlement principal in the a666 NAV
   reserve custody;
4. debits/retires a666 and credits pfUSDC atomically;
5. reduces both authorized valid supply and counted reserve value by the
   policy-derived amounts;
6. consumes redemption capacity and nonce; and
7. requires no later issuer or redemption-account signature.

The current `nav_redeem_at_nav -> nav_redeem_settle` flow is not sufficient
for this promise because its completion is operator-signed. It may remain for
other NAV assets, but a666 wallet copy must not say “issuer-independent
redemption” until the new atomic reserve-release transition is active, and must never use
an unqualified “trustless” label that hides the return `BFT_CHECKPOINT`.

### 8.5 Available capacity

The quote service and consensus must compute:

```text
available_issue =
  min(
    policy_issue_remaining,
    proven_unallocated_settlement_capacity,
    post_mint_backing_headroom,
    route_authorized_supply_headroom,
    active_export_exposure_headroom,
    per_order_limit
  )

available_redeem =
  min(
    policy_redeem_remaining,
    unencumbered_nav_reserve_settlement_principal,
    reserve_liquidity_available_under_policy,
    per_order_limit
  )
```

The quote endpoint must return every limiting term. Subscription-funded
principal can satisfy the reserve term; it must not be excluded merely because
it was not deposited through a separate funding operation. “Up to 2,000,000”
may be displayed only when the computed remaining capacity actually supports
it.

## 9. Route versioning and limits

The current PFTL route-init fields and Ethereum controller limits are
immutable, and PFTL has no general route-update transition. That is safe for
controlled evidence but unsuitable for an indefinitely growing product.

The current route also has one `route_trust_class` even though verification is
directional. That field is insufficient:

```text
PFTL -> Ethereum: TRUSTLESS_FINALITY (SP1 PFTL-finality receipt proof)
Ethereum -> PFTL: BFT_CHECKPOINT (PFTL quorum header + receipt-trie proof)
```

Version the route schema to bind and expose both
`outbound_verification_class` and `return_verification_class`. The outbound
class is committed into the Ethereum packet/verifier/controller. The return
class selects the PFTL Ethereum-event verification policy. Both enter the
route config digest, state commitment, RPC response, wallet allowlist, and
pre-sign display.

Update `verify_live_route_initialization` accordingly. It currently admits
only `CONTROLLED` rehearsals or a single `BFT_CHECKPOINT` strict route, which
would reject or mislabel the required outbound SP1 route. Legacy route states
retain their old schema and labels; no migration may reinterpret them.

Implement a governed `pftl_uniswap_route_epoch_advance` transition that
creates a new immutable epoch under the same route family while preserving all
old packet terminal states. Contract bytecode or immutable-address changes
still require a new controller and route ID; ordinary NAV, policy, and
capacity advancement does not.

The epoch transition MUST enforce:

- old in-flight packets finish against their pinned epoch;
- new orders use only the newest active epoch;
- reducing capacity cannot strand valid return burns;
- pausing inbound issue/export does not block return or redemption;
- route exposure is net outstanding wA666, not lifetime minted volume; and
- a capacity increase requires fresh backing/capacity evidence and a
  content-addressed config digest.

Launch parameters:

```text
net wrapped exposure cap:          2,000,000 a666
single packet cap:                  250,000 a666
maximum reserved user order:      1,000,000 a666
```

A 1,000,000-a666 order uses four 250,000-a666 packets. The batch reserves all
capacity first and either completes all packets or exposes deterministic
completion and refund state. It must not become a best-effort partial market
order.

## 10. PFTL export receipt and Ethereum proof verifier

### 10.1 Required new verifier

Implement `PFTLReceiptFinalityVerifierV1` as a
permissionless SP1-backed implementation of `IPFTLReceiptVerifier`.

It must not inherit the pfUSDC vault caller requirement. Its job is to prove
PFTL consensus finality and exact inclusion of an accepted a666 export receipt,
then store the receipt commitment that the handoff controller queries.

Required interface:

```solidity
function routeTrustClass() external pure returns (bytes32);

function verifyAndAccept(
    bytes calldata publicValues,
    bytes calldata proofBytes
) external returns (bytes32 receiptCommitment);

function isReceiptAccepted(
    bytes calldata sourceReceiptRoot,
    bytes calldata sourceReceiptHash,
    bytes calldata routeConfigDigest,
    bytes32 routeTrustClass,
    bytes32 packetDigest
) external view returns (bool);

function advanceCheckpoint(
    bytes calldata publicValues,
    bytes calldata proofBytes
) external;
```

`routeTrustClass()` returns `keccak256("TRUSTLESS_FINALITY")`.

There is no owner acceptance method, signer threshold, optimistic resolver, or
admin bypass in this verifier. Anyone may submit a valid proof. Invalid proofs
cannot mutate checkpoints or acceptance state.

### 10.2 SP1 public values

The a666 export public values must use a new magic and schema, not the pfUSDC
withdrawal schema, and bind at minimum:

```text
proof program version
PFTL chain ID hash
PFTL genesis commitment
PFTL protocol version
prior and resulting checkpoint commitments
committee root and transition commitment
finalized PFTL block height and block/state commitment
source receipt root
source receipt hash
accepted receipt code
route ID and route config digest
route epoch and TRUSTLESS_FINALITY class
native a666 asset ID
settlement pfUSDC asset ID
pricing NAV epoch and reserve packet hash
primary-market policy hash
source wallet hash
Ethereum chain ID = 1
handoff controller
wA666 token
Ethereum recipient
mint amount
settlement amount
packet nonce, deadline, and packet digest
proof nullifier
```

The guest must prove:

1. the PFTL checkpoint advances from a recognized checkpoint under the
   consensus-v2 committee rules, including valid committee transitions;
2. the block and receipt root are committed by that checkpoint;
3. the exact receipt is included and accepted;
4. the receipt represents a source debit/export of the same minted amount;
5. every public field matches the accepted PFTL operation and route state; and
6. the receipt and packet have not taken a conflicting terminal branch in the
   proved state.

Proof and public-value byte limits are immutable and exercised at their
boundary values.

### 10.3 Checkpoint liveness

Checkpoint advancement is permissionless and independent of user withdrawals.
At least two operationally independent provers/relayers must be able to
produce and submit proofs. The product may operate with one, but launch
readiness requires a documented second path and a stale-checkpoint alert.

The verifier accepts historical branches only under explicitly proved
committee lineage. A proof cannot replace or skip an unrecognized prior
checkpoint.

## 11. Ethereum contract stack

Deploy fresh immutable instances on Ethereum mainnet:

1. `WrappedVenueNAVCoin` for wA666;
2. `PacketReplayRegistry`;
3. `PFTLReceiptFinalityVerifierV1`;
4. `PFTLUniswapHandoffController`;
5. a pool-bound Uniswap v4 exact-input router/adapter.

Pool initialization and seed-position creation use a deterministic deployment
script whose calldata and simulation output are part of the reviewed package;
no additional privileged launch-helper contract is deployed.

Contract requirements:

- Solidity and SP1 artifacts are built reproducibly from a frozen commit and
  lockfiles.
- Constructor arguments, creation bytecode hash, runtime code hash, immutables,
  deployer, nonce, transaction hash, block, and receipt are recorded.
- The wrapped token's controller is locked irreversibly after the controller
  address is known and verified.
- The replay registry authorizes only the final controller.
- The controller accepts mint-only delivery for Bob. `consumeMintAndSwap`
  MUST be absent or fail closed on the launch route.
- Controller pause blocks new mint/consume paths but never blocks
  `burnForPftlReturn`.
- Caps apply to `total_minted - total_return_burned`.
- Approved executors are relayers, not mint authorities; they cannot alter a
  packet.
- Pause/executor ownership MUST move to the production governance address
  frozen at G0 after deployment. It cannot alter verifier truth or token
  supply.
- No upgrade proxy is used for the launch stack. A new audited route version
  replaces faulty contracts.

The controller's existing exact receipt, packet, route, recipient, amount,
deadline, source packet, source receipt, and replay bindings are retained.
Its `route_trust_class` describes the PFTL-to-Ethereum consume direction only;
it must not be presented as the trust class of the inverse return path.

## 12. Ethereum return verification

The current PFTL `BFT_CHECKPOINT` path already verifies:

- a governed Ethereum checkpoint certificate;
- the Ethereum receipts root;
- the receipt-trie inclusion proof;
- the exact `ReturnBurned` log and controller/token/asset/user/amount/nonce
  bindings; and
- the configured confirmation depth.

Production must use that strict path, never `CONTROLLED`.

Before launch:

1. bind a fresh Ethereum verification policy to the production controller and
   wA666 runtime code hashes;
2. register the correct authority epoch and committee root;
3. prove checkpoint vote isolation by route/domain;
4. exercise committee rotation;
5. demonstrate forged header, receipt, log index, code hash, controller,
   amount, recipient, and nonce rejection; and
6. prove that return remains live while inbound issue/export is paused.

This side is PFTL-consensus trust: its checkpoint certificate is produced by
the governed PFTL validator committee, while receipt inclusion is
cryptographic. Public UX must call it `BFT_CHECKPOINT`, not silently relabel it
as Ethereum light-client verification.

## 13. New Uniswap v4 pool

Create a new, hookless launch pool:

```text
pair:          wA666 / canonical Ethereum-mainnet USDC
USDC:          0xA0b86991c6218b36c1d19d4a2e9eb0ce3606eb48
fee tier:      500 (0.05%)
tick spacing:  10
initial price: fresh finalized a666 NAV translated into token ordering/Q96
```

A hook is not required to launch. NAV display and primary policy live outside
the AMM. Adding a custom v4 hook expands audit scope and is deferred unless a
specific launch requirement cannot be met without it.

Pool creation order:

1. derive and publish the PoolKey and PoolId;
2. verify official mainnet PoolManager, PositionManager, Universal Router,
   Permit2, and StateView addresses and runtime code;
3. create verified opening a666 successor supply;
4. export it to the fixed-ratio a651-burn migration contract;
5. migrate only burnable operator a651 into wA666;
6. initialize the pool at fresh NAV;
7. add migrated wA666 and USDC through PositionManager; and
8. record the NFT position ID, tick range, amounts, owner, and receipts.

The seed must not use an owner mint or unburned a651. Its wA666 side must be
traceable to operator a651 burned by the migration contract. A reasonable
`$30,000` two-sided seed at the issue price is approximately:

```text
15,000 wA666 released against burned a651 successor claims
15,000 USDC supplied as the other LP side
```

Exact token amounts depend on tick rounding and range. Seed capital,
portfolio backing, and deployer gas are separate accounting lines.

## 14. Wallet and orchestration

The user-facing action is:

```text
Acquire a666 on Ethereum
```

Before Bob signs, the wallet displays:

```text
USDC input and maximum input
wA666 output and minimum output
finalized NAV and NAV epoch
issue multiplier and dollar price
remaining issue and export capacity
reserve proof age/expiry
route trust classes in both directions
estimated Ethereum gas
estimated completion time
all recovery deadlines
production asset/token/controller/pool identifiers
```

The orchestration service:

- derives a durable workflow ID before the first transaction;
- uses one idempotency key per state transition;
- persists every submitted tx/proposal/proof job before polling;
- resumes after process failure without resubmission ambiguity;
- never holds Bob's private key;
- may pay relayer/proof gas without gaining authority over value;
- refuses legacy a651, controlled, optimistic, wrong-chain, wrong-code-hash,
  stale-NAV, over-cap, or disabled routes; and
- does not quote “completed” until wA666 is spendable at Bob's address.

Required workflow states:

```text
QUOTED
USDC_DEPOSIT_SUBMITTED
USDC_DEPOSIT_FINALIZED
PFUSDC_PROOF_RUNNING
PFUSDC_CREDITED
PRIMARY_SUBSCRIPTION_FINALIZED
A666_EXPORT_FINALIZED
PFTL_FINALITY_PROOF_RUNNING
WA666_CLAIM_SUBMITTED
COMPLETED
RECOVERY_REQUIRED
REFUNDED
FAILED_TERMINAL
```

Every nonterminal state has a documented next action and retry policy.

## 15. Latency and cost budgets

### 15.1 Latency

The public SLO is:

```text
USDC deposit transaction included
  -> wA666 spendable at Bob's Ethereum address
  <= 25 minutes
```

The 20m12s pfUSDC round trip proves that the base proof rail can meet a
25-minute whole-roundtrip target under one observed run. It does not prove
that adding primary issuance and wA666 export will automatically remain under
25 minutes.

Before launch, measure every stage and set fail-closed time budgets. G7
requires at least 20 production-equivalent end-to-end runs across three
separate run windows, including at least three real-value mainnet runs. The
measured p95 MUST be at most 20 minutes and no conforming run may exceed 25
minutes. A proof job older than its bound becomes `RECOVERY_REQUIRED`; the
service does not silently start a duplicate.

### 15.2 Cost

Known ingress user cost is the approximately 271k-gas approval/deposit class
recorded in Section 4. All other costs must be fork-measured and then captured
live:

```text
SP1 proof compute
PFTL transaction fees
Ethereum proof verification / receipt acceptance
controller consume + wA666 mint
return burn
pool initialization
LP position creation
contract deployment
```

The deployment manifest reports gas units and ETH at the transaction's actual
effective gas price. It must not mix principal, LP capital, portfolio backing,
and gas into one “launch cost.”

## 16. Implementation work packages

### WP0 — Freeze identifiers and remove false capacity claims

- Select a666 v2 code/version and generate its asset ID.
- Mark a666 v1 and a651 as non-production in wallet/config/docs.
- Replace all permanent-max language with `max_supply=None`.
- Define issue/redeem policy epoch semantics.

Exit: a signed parameter sheet and no conflicting canonical document.

### WP1 — Primary-market consensus

- Add `NavPrimaryMarketPolicy`.
- Version primary subscription with rational spread math and user limits.
- Add permissionless atomic primary redemption from NAV reserve custody.
- Split outbound and return verification classes; update state commitments,
  route validation, RPC, wallet policy, and tests without reinterpreting
  legacy routes.
- Add route epoch/limit advancement or formalize immutable route replacement.
- Add quote/status RPC fields and strict response validation.

Exit: deterministic state-root tests for issue, redeem, capacity, stale proof,
rounding, replay, and restart.

### WP2 — a666 PFTL-finality proof

- Define the export receipt public-value schema.
- Implement and freeze the SP1 guest/ELF/vkey.
- Implement `PFTLReceiptFinalityVerifierV1`.
- Produce Rust/Solidity/JSON conformance vectors.
- Add permissionless checkpoint advancement and receipt acceptance.

Exit: genuine Groth16 proof consumed on an Ethereum mainnet fork; mock verifier
cannot satisfy the production deployment preflight.

### WP3 — Mainnet contract and pool package

- Bind final asset, policy, route, pool, chain, token, controller, and code
  hashes.
- Rehearse deterministic deployment on a pinned mainnet fork.
- Execute 100,000-a666 mint-only, return, cancellation/refund, replay, buy,
  sell, and LP tests.
- Generate the no-broadcast deployment package.

Exit: independent review of bytecode, constructor tuple, balances, nonces,
PoolId, price encoding, gas ceiling, and rollback boundary.

### WP4 — Fleet and route activation

- Roll the reviewed PFTL binary through the signed-snapshot discipline.
- Create/register a666 v2 and its NAV/profile/policy.
- Deploy mainnet contracts.
- Initialize the PFTL route disabled.
- Read back every binding on all six validators and Ethereum.
- Enable live value only after proof and wallet gates pass.

Exit: identical route/state digest on the fleet and exact equality with
Ethereum immutables.

### WP5 — Canonical bootstrap and pool seed

- Finalize the opening reserve/NAV packet.
- Perform canonical primary issue for seed wA666.
- Export and proof-mint seed wA666.
- Initialize and seed the new pool.
- Publish pool and supply composition status.

Exit: seed provenance traces to a PFTL issue and export; no manual mint.

### WP6 — Live user and redemption tests

- Run the smallest meaningful real-value USDC -> wA666 flow.
- Run wA666 -> USDC to a different recipient.
- Pass the 25-minute latency gate.
- Re-run at 100,000 a666 on a mainnet fork using production code/state.
- Run a real 100,000-a666 order only under separate principal authorization.

Exit: evidence bundle passes every gate in Sections 17 and 18.

## 17. Required test matrix

### Primary issue/redeem

- exact `$1.00`, 1.005, and 0.9995 arithmetic;
- fractional NAV and mixed precision;
- every rounding boundary;
- one atom below/at/above minimum and maximum;
- one atom below/at/above issue and redemption capacity;
- stale/expired/future NAV and policy epochs;
- changed policy hash, reserve packet, recipient, amount, or limit;
- insufficient balance/backing/NAV reserve principal or liquidity/route headroom;
- duplicate nonce across restart and snapshot replay;
- issue and redeem competing in the same block;
- no mutation on every rejection;
- no operator signature required for a reserve-backed conforming redemption.

### PFTL export proof

- valid checkpoint advance and receipt inclusion;
- invalid SP1 proof and wrong vkey;
- unknown prior checkpoint and bad committee transition;
- wrong PFTL chain/genesis/protocol;
- wrong route/epoch/config/trust class;
- wrong asset, settlement asset, NAV epoch, policy, controller, token, pool,
  recipient, amount, deadline, or packet digest;
- receipt not accepted, wrong receipt index/root/code;
- proof nullifier, receipt, source packet, and packet replay;
- proof/public-value maximum-size boundaries;
- verifier call reentrancy and state-before-proof mutation checks.

### Export/refund/return

- consume before deadline;
- cancellation only after deadline;
- consume after cancel rejected;
- cancel after consume rejected;
- PFTL refund without cancellation proof rejected;
- return burn with genuine Ethereum checkpoint/receipt proof;
- forged header/root/trie node/log index/event rejected;
- duplicate return nonce/event rejected;
- inbound pause preserves burn and return import;
- route epoch rotation preserves old packet completion;
- cross-route and cross-chain replay rejected.

### Ethereum token/controller/pool

- no direct/owner wA666 mint;
- controller lock cannot change;
- cap uses net outstanding exposure;
- packet cap and route cap exact boundaries;
- zero-value ERC-20 behavior and nonzero mint/burn requirements;
- fee-on-transfer or malicious router cannot overstate settlement;
- official v4 runtime code and pool-bound router checks;
- PoolId, currency ordering, decimals, Q96 price, ticks, fee, and position
  amounts independently recomputed;
- external buy/sell do not change wrapped supply;
- pool removal does not affect primary issue/redemption availability.

### Operations

- prover crash/restart and duplicate job submission;
- relayer crash after broadcast but before persistence;
- RPC disagreement, Ethereum reorg before finality, and stale endpoint;
- PFTL validator loss while maintaining safety;
- deployment partial failure at every transaction boundary;
- monitoring alert delivery and runbook execution;
- recovery from every workflow state without private-key export.

## 18. Deployment gates

| Gate | Pass condition |
|---|---|
| G0 — economic freeze | a666 v2 has no static maximum; issue/redeem bands, capacity epochs, spread allocation, opening reserves, and seed budget are signed off. |
| G1 — consensus | Primary issue and permissionless reserve-backed redemption pass deterministic replay, state-root, fuzz, and full release tests on the final commit. |
| G2 — proof | Genuine SP1 export-receipt proof is verified by the final Solidity verifier; all binding/replay negatives pass. |
| G3 — mainnet fork | Final bytecode and constructor tuple execute the complete 100,000-a666 issue/export/return/refund/pool path on a pinned fork. |
| G4 — production deploy | Mainnet deployment receipts, code hashes, ownership, controller lock, replay registry authority, and official v4 bindings match the reviewed manifest. Route remains disabled. |
| G5 — canonical seed | Opening reserves/supply reconcile; seed wA666 traces to primary issue plus verified export; new pool is initialized and seeded. |
| G6 — real-value round trip | Independent-recipient USDC -> wA666 -> USDC passes conservation, replay, failure recovery, and no-controlled-fallback checks. |
| G7 — latency/capacity | The Section 15.1 run set passes p95 <=20m and worst-case <=25m; quote exposes the 2M policy ceilings and the actual reserve-backed available amounts separately; one 1M-a666/four-packet issue order succeeds in production-equivalent testing and its resulting units are redeemable from the subscription-funded reserve. |
| G8 — launch | Monitoring, halt/return behavior, evidence index, public identifiers, wallet copy, and release-owner approval are complete. |

No gate may be waived by changing the visible trust label. `CONTROLLED` and
`OPTIMISTIC` evidence remains useful test evidence but cannot satisfy G2, G4,
G6, or G8 for the public production route. The wallet must show
`TRUSTLESS_FINALITY` outbound and `BFT_CHECKPOINT` return as separate fields.

## 19. Monitoring and halt behavior

Alert on:

```text
stale reserve/NAV proof
checkpoint or prover SLO breach
route or verifier binding mismatch
issue/redeem/export capacity below threshold
pfUSDC or a666 conservation mismatch
PFTL versus Ethereum wrapped-supply mismatch
unexpected wA666 mint/burn/controller event
replay rejection rate
refund or return queue age
Uniswap liquidity drop and NAV/market deviation
Ethereum/PFTL RPC disagreement
contract ownership or code change
```

Automatic safe state:

```text
new primary issue: paused
new export:        paused
mint-and-swap:     paused
return burn:       enabled
return import:     enabled
primary redemption: enabled when its proofs and NAV reserve custody remain valid
```

An accounting mismatch halts issuance first. It does not erase packet history,
disable user returns, or authorize an owner to repair balances manually.

## 20. Evidence deliverables

Create a dated evidence root containing:

```text
source commit and dirty-worktree declaration
Cargo/npm/Foundry/SP1 lockfile hashes
asset/profile/policy/route payloads and IDs
ELF hashes, vkeys, verifier gateway binding
Solidity creation/runtime bytecode hashes
constructor tuple and deterministic address derivation
deployment transactions and receipts
Ethereum and six-validator read-backs
PoolKey, PoolId, price/tick derivation, LP position
issue/export/proof/consume/return/redeem/withdraw artifacts
before/after balances and every conservation term
negative and replay test reports
stage-level latency and actual cost report
monitoring alert delivery evidence
independent review and release approval
```

The summary must distinguish:

```text
implemented
tested locally
tested on mainnet fork
deployed disabled
live tested
publicly enabled
```

“Mainnet ready” is forbidden shorthand.

## 21. Funding and authority boundaries

This specification authorizes engineering, testnet/fork work, and generation
of a no-broadcast mainnet package. It does not itself authorize spending:

- 100,000 or 1,000,000 USDC of Bob/customer principal;
- the opening reserve contribution;
- the pool seed and LP range;
- production deployment gas.

The launch MUST NOT require or advertise a duplicate 2,000,000-pfUSDC
redemption inventory deposit. The policy ceiling may be 2,000,000 a666, but
the available redemption amount is bounded by valid supply and the
unencumbered settlement principal already held in the a666 NAV reserve.
Primary subscriptions add that principal atomically as they create supply.

At a `$1.00` NAV, a completed primary issue of 1,000,000 a666 places
1,000,000 pfUSDC of base value into the NAV reserve. Those units can therefore
be redeemed from that reserve, subject to the published multiplier, remaining
epoch capacity, proof freshness, and any disclosed reserve-liquidity policy.
The redemption spread is posted to the disclosed non-NAV spread account; it
is not a second source of principal.

## 22. Definition of done

The program is done only when a user who starts with canonical mainnet USDC
can, without knowing Alex and without trading through Uniswap:

1. receive a quote at finalized pre-inflow NAV × 1.005;
2. create new, fully accounted a666 supply;
3. receive the exact wA666 output on Ethereum mainnet within 25 minutes;
4. verify that every wrapped unit traces to a finalized PFTL debit;
5. use the new wA666/USDC pool without that pool limiting primary capacity;
6. execute the posted, reserve-backed NAV × 0.9995 redemption without issuer
   discretion; and
7. arrive back at mainnet USDC through proof-verified pfUSDC egress.

At that point, a666 is a low-slippage, proof-bound primary acquisition product
with an Ethereum access venue. Before that point, it is an implementation or
deployment candidate and must be labeled accordingly.

## 23. Implementation traceability (2026-07-27)

The current implementation candidate maps to this specification as follows:

| Area | Implementation |
|---|---|
| a666 v2 economics | Version-2 asset creation requires code `A666`, precision 6, and `max_supply=None`. The v2 policy commits 1.005 issue, 0.9995 redeem, epoch capacities, order bounds, NAV freshness, and reserve packet. Available redemption is derived from valid supply and unencumbered settlement principal in NAV reserve custody; there is no separate redemption-liquidity funding gate. |
| Primary market | Signed reservation, release, atomic primary subscription, permissionless reserve-backed redemption, emergency pause/resume, and epoch-advance operations execute through normal PFTL transactions. Reservations escrow maximum settlement value and become bounded export entitlements after issuance. Disabled or paused routes reject new reserve/issue/export work; conforming redemption, return import, and refund remain available according to their own safety checks. |
| Large-order export | Each v2 export requires the user/recipient-bound entitlement. Packet size remains capped at 250,000 a666; partial exports decrement the entitlement, allowing one 1,000,000-a666 order to produce four packets without consuming Uniswap liquidity. |
| Supply/capacity | The v2 route cap applies to net wrapped exposure, not lifetime a666 supply. Route validation includes Ethereum supply, outstanding claims, active reservations, and remaining export entitlements. Status RPC exposes policy bounds, directional trust, reservations, entitlements, wrapped exposure, and currently available issue/export/redeem amounts with strict client validation. |
| Finalized receipts | Block headers, proposals, votes, commits, storage, replay, and state commitments bind the PFTL-Uniswap receipt root. Export receipts use bounded Merkle inclusion proofs. |
| SP1 proof | `crates/pftl_uniswap_proofs` verifies committee finality, checkpoint lineage, receipt inclusion, route state, policy, reservation-bound packet digest, destination, amount, settlement value, nonce, and deadline. Its proof-only checkpoint witness has no receipt, route, or mint-packet dependency, so liveness does not require user traffic. `programs/pftl-uniswap-receipt` commits either the exact 1,120-byte receipt tuple or 256-byte checkpoint tuple consumed by Solidity under one pinned program vkey; `tools/pftl-uniswap-prover` natively cross-checks and executes or Groth16-proves either mode. |
| Ethereum | `PFTLReceiptFinalityVerifierV1.sol` pins the PFTL chain, a666 route identity, native/settlement assets, proof program, controller, and token; it advances proof-bound checkpoints and consumes proof/nullifier/receipt/packet exactly once. Policy and route epochs may advance only when the same pinned SP1 program proves the canonical PFTL transition. `PFTLUniswapPrimaryMarketV2.sol` is the sole locked wA666 controller, enforces token/pool/caps and proof-bound packets, supports mint pause without disabling burns, and emits proof-bound return burns. |
| Deployment/pool | `DeployA666PrimaryMarket.s.sol` predicts and verifies the controller address/code hash before locking wA666. `InitializeA666UniswapV4.s.sol` checks mainnet USDC and official component code hashes, initializes fee 500/tick-spacing 10, and seeds directly from the operator EOA. |
| Wallet/orchestration | `wallet-web/src/lib/a666-primary-route.js` fail-closes unless the v2 economics, directional trust, live state, invariant, capacity, proof/program, route, controller, token, pool, NAV, and recovery-deadline pins match. `wallet-proxy/a666-primary-workflow.js` provides an atomically persisted, restart-safe state journal with deterministic workflow/transition idempotency keys, required submission IDs before polling, explicit recovery states, and secret-field rejection. |
| Operations | `docs/runbooks/a666-mainnet-primary-market-deployment.md` defines the no-broadcast, fork, genuine-proof, read-back, funding, pool, production-order, and rollback procedure. |

Local unit/build success is not G2, G3, or deployment evidence. The remaining
external gates are a frozen production parameter sheet; genuine SP1
Groth16 proof against the recorded vkey; production adapters that drive the
persisted USDC-to-pfUSDC, reservation, issuance, export, proof, and
Ethereum-consume workflow; pinned mainnet-fork end-to-end campaign; governance,
reserve/redemption/LP funding approvals; production deployment; real-value
round trip; and the 20-run latency/capacity acceptance set.
