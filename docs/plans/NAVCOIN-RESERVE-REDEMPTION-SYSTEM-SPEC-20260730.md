# NAVCoin Reserve Redemption System Specification

**Date:** 2026-07-30

**Priority:** P0 product architecture; P1 protocol implementation

**Status:** the generic multi-asset NRRS described in sections 8–16 is a
proposed successor and is not authorized for live value. The binding Monday
profile in section 2.2 uses already-deployed A666/pfUSDC operations and is the
only live execution scope of this document.

**Working name:** NAVCoin Reserve Redemption System (`NRRS`)

**Immediate execution profile:** Monday, 2026-08-03, uses the deployed
A666/pfUSDC primary route only; see section 2.2

**Monday scope precedence:** if any general requirement elsewhere in this
document appears to require new consensus code, a new facility, a new bridge,
a new price-packet format, another settlement asset, or a new Uniswap
deployment before the Monday demonstration, section 2.2 controls and that work
is deferred.

**Not a pool-creation plan:** the mainnet wA666/USDC Uniswap v4 pool already
exists. Monday proves low-slippage primary creation and reserve-backed
redemption without using that AMM curve.

**Phase-one activated settlement asset:** the existing Ethereum-mainnet
pfUSDC asset and route only

**Later expansion families:** `pfXRP`, `pfETH`, `pfStakedETH`, and `pfBTC`

**Current implementation baseline:**
`A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md`

**Current production state:**
`../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md`

**Production release gate for the existing lane:**
`A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md`

**Primary implementation anchors (deployed code this spec must extend, not
duplicate):**

- `crates/types/src/market_nav_asset_types.rs` —
  `PftlUniswapPrimaryMarketPolicyV2`, `PftlUniswapRouteV2State`,
  `PftlUniswapOrderReservationV2`, export entitlements/packets, trust-class
  constants, bounded-collection validation.
- `crates/execution/src/nav_vault_asset_execution.rs` — deterministic
  pricing helpers, primary issue/redeem execution, vault-bridge settlement
  top-up, `pftl_uniswap_*` deterministic error codes.
- `crates/node/src/shielded_batch_actions.rs`, `crates/privacy_orchard/` —
  Asset-Orchard typed-note execution.
- `crates/node/src/market_bridge.rs`, `crates/node/src/state_commitment.rs`
  — route state-root commitment coverage.

`MUST`, `MUST NOT`, `REQUIRED`, and declarative “must” statements are
normative. `SHOULD` is the default unless a dated exception records its owner,
risk, and expiry. Examples are non-normative.

## 1. Objective

Create a registry-driven primary market in which a NAVCoin can post reserves
of arbitrary approved PFTL settlement assets and publish a two-sided,
capacity-bound quote:

```text
pfAsset -> newly issued NAVCoin
NAVCoin -> pfAsset from posted reserve
```

The first activated facility MUST be the existing Ethereum-mainnet pfUSDC
asset used by A666. The state, registry, pricing, and transaction schemas MUST
remain generic, but no non-pfUSDC facility is part of the phase-one release.

The intended later settlement families are:

- `pfXRP`;
- `pfETH`;
- `pfStakedETH`; and
- `pfBTC`.

These are product families, not consensus identifiers. The phase-one pfUSDC
facility and every later facility bind an exact PFTL asset ID, source domain,
source token or native asset, proof profile, valuation policy, precision,
bridge route, and risk policy.

The NAVCoin operator posts reserve inventory and terms once. A user with a
valid signed order may execute against that committed reserve without:

- contacting the NAVCoin operator;
- disclosing a bilateral identity to the NAVCoin operator;
- receiving an operator signature on the individual trade;
- trusting the operator to honor the quote after reservation; or
- buying or selling through an AMM curve.

This system is intended to keep fund capital productive. A NAVCoin need not
hold its entire redemption capacity as idle USDC. It may post yield-bearing or
portfolio-native assets for in-kind redemption, while retaining pfUSD as an
optional convenience facility.

## 2. Product model

For each approved `NAVCoin / pfAsset` pair, the system maintains a standing
facility:

```text
                       posted two-sided facility

        pfAsset --------------------------------> new NAVCoin
                 primary issue / facility ask

        NAVCoin --------------------------------> posted pfAsset
                 primary redeem / facility bid
```

The facility is not an order book and is not an AMM. Its executable price is a
deterministic function of:

- the NAVCoin's finalized pre-trade NAV;
- the settlement asset's finalized price and proof state;
- the facility's issue and redemption multipliers;
- asset-specific haircuts or delivery markups;
- deterministic rounding;
- remaining policy capacity; and
- actual unreserved reserve inventory.

A quote is executable only up to the amount the committed state can honor.
The protocol must distinguish:

```text
headline policy capacity
actual executable issue capacity
actual executable redemption capacity
temporarily reserved capacity
```

### 2.1 Phase-one product slice

Phase one activates exactly:

```text
Ethereum-mainnet pfUSDC -> newly issued A666
A666 -> posted Ethereum-mainnet pfUSDC
```

The concrete compatibility route is
`pftl-a666-ethereum-wA666-usdc-v1`. Phase one MUST use the generic facility
schema; it MUST NOT implement a special pfUSDC-only state object or hardcode:

- settlement price permanently equal to one dollar;
- six-decimal precision as a protocol-wide rule;
- one facility per NAVCoin;
- Ethereum ERC-20 custody as the only source model; or
- pfUSDC-specific field meanings into generic hashes and receipts.

The pfUSDC facility MUST use a fresh settlement-price packet even when its
verified price is `$1.00`. This proves the price, freshness, haircut, and
depeg path needed by every later asset.

No `pfXRP`, `pfETH`, `pfStakedETH`, or `pfBTC` facility may be activated
until the pfUSDC facility passes the phase-one release gates in section 25.

### 2.2 Binding Monday demonstration profile

The Monday, 2026-08-03 demonstration has one business claim:

> Any normally authorized and funded user—not a privileged operator—can
> contribute Ethereum-mainnet USDC through pfUSDC, receive newly issued A666,
> cause that pfUSDC reserve to be counted in A666 NAV, and redeem A666 back
> into pfUSDC under the standing primary-market policy.

This is an execution and evidence demonstration of the already deployed
A666/pfUSDC route. It is not the activation of the generic multi-facility
NRRS state proposed by sections 8–16.

For avoidance of doubt, completing section 2.2 is the Monday deliverable.
Implementing the remainder of this specification is not a prerequisite for
that demonstration.

The demo does **not** create a Uniswap pool. The wA666/USDC Uniswap v4 pool
already exists. It also does not create a new pfUSDC facility. It exercises
the deployed primary route:

```text
pftl-a666-ethereum-wA666-usdc-v1
```

#### 2.2.1 Required flow

The required flow is:

```text
ordinary funded user
  -> deposits canonical Ethereum-mainnet USDC
  -> receives source-labeled pfUSDC on PFTL
  -> executes pftl_uniswap_primary_subscribe_v2
  -> receives newly issued A666
  -> incoming base pfUSDC increases settlement_reserve_atoms
  -> releases the unused export entitlement with pftl_uniswap_order_release
  -> fresh StakeHub-backed NAV mark counts the updated settlement reserve
  -> governed route epoch advance pins that fresh NAV for pricing
  -> user executes pftl_uniswap_primary_redeem
  -> redeemed A666 is retired
  -> user receives pfUSDC from settlement reserve
```

The pfUSDC remains on PFTL between issue and redemption. The core demo does
not bridge the facility reserve back to Ethereum. A final pfUSDC-to-USDC
withdrawal may be shown only after the required PFTL accounting proof has
passed; it is not allowed to obscure or replace that proof.

The issuing and redeeming wallet may be the same wallet. It requires no
special issuer or operator role beyond the ordinary authorizations already
required by the deployed deposit, subscription, and redemption operations.
The fresh NAV mark and route epoch advance remain normal market-maintenance
operations under their existing governance authority; they are not
per-customer trade approvals.

The deployed transparent subscription creates an export entitlement even when
the user intends to retain A666 on PFTL. The user MUST release that entitlement
before the NAV mark and route epoch advance. Directly redeeming while leaving
the entitlement active is not an acceptable terminal state.

#### 2.2.2 Capital requirement

No separate operator seed is required for this demonstration because issue
occurs before redemption. The user's own issue payment funds the incremental
settlement reserve.

The demo redemption amount MUST be no greater than the redemption output
supported by:

```text
the newly credited base settlement reserve
+ any separately proven unreserved legacy settlement reserve
```

The demo SHOULD rely only on the incremental reserve created in the same run.
That makes the proof independent of undocumented prior liquidity.

At A666 NAV `$1.00`, issue multiplier `1.005`, and redemption multiplier
`0.9995`, an example round trip is:

```text
user pays                 100.500000 pfUSDC
new A666 issued           100.000000 A666
base reserve increase     100.000000 pfUSDC
issue spread                0.500000 pfUSDC

user redeems              100.000000 A666
pfUSDC returned            99.950000 pfUSDC
A666 retired              100.000000 A666
```

Exact live amounts MUST be calculated from the fresh governed NAV and policy;
the example does not authorize a hardcoded `$1.00` NAV.

#### 2.2.3 Required machine evidence

The demo packet MUST contain:

1. fresh preflight output proving all six validators agree on finalized
   height and relevant state roots;
2. the Ethereum USDC deposit transaction, finality evidence, vault event, and
   pfUSDC claim receipt;
3. pre-issue A666 supply, user balances, route policy, capacity usage, and
   `settlement_reserve_atoms`;
4. the finalized `pftl_uniswap_primary_subscribe_v2` transaction and receipt;
5. post-issue proof that:

   ```text
   authorized_valid_supply_after
     = authorized_valid_supply_before + issued_A666

   user_A666_after
     = user_A666_before + issued_A666

   settlement_reserve_after
     = settlement_reserve_before + base_pfUSDC
   ```

6. the finalized user-authorized `pftl_uniswap_order_release` transaction
   removing the unused export entitlement without changing A666 supply or
   settlement reserve;
7. a fresh finalized StakeHub-backed NAV mark whose primary-market reserve
   overlay contains the updated `settlement_reserve_atoms`;
8. the finalized governed route epoch advance pinning that NAV epoch and
   reserve packet for primary pricing;
9. the finalized `pftl_uniswap_primary_redeem` transaction and receipt;
10. post-redemption proof that:

   ```text
   authorized_valid_supply_final
     = authorized_valid_supply_after - redeemed_A666

   settlement_reserve_final
     = settlement_reserve_after - redemption_pfUSDC

   user_pfUSDC_final
     = user_pfUSDC_after_issue + redemption_pfUSDC
   ```

11. zero active reservation and export-entitlement state for the demo order;
12. six-validator convergence on the final route and NAV state; and
13. a single machine-readable summary with transaction IDs, heights, hashes,
    amounts, elapsed times, invariant verdicts, and artifact paths.

The existing execution anchors are:

- `scripts/a666-mainnet-primary-issue-ops.py`, using its reserve and subscribe
  operations but deliberately not submitting its generated export operation;
- `scripts/a666-build-live-nav-mark-ops.py`;
- `scripts/a666-build-route-epoch-advance.py`;
- `scripts/a666-build-transparent-redeem-op.py`; and
- the deployed consensus operations and evidence patterns referenced in the
  current-state and acceptance documents.

`scripts/a666-mainnet-transparent-issue-after-deposit.sh` is reference
evidence, not the Monday driver, because it continues directly into Ethereum
export. The Monday driver MUST stop after subscription, release the export
entitlement, and preserve the A666 on PFTL for the NAV mark and redemption.

#### 2.2.4 Pass criteria

The Monday demo passes only if:

- the user is not exercising a privileged operator-only transaction path;
- canonical Ethereum USDC produces the exact source-labeled pfUSDC;
- issue creates new A666 rather than transferring existing inventory;
- incoming base pfUSDC increases the route settlement reserve exactly once;
- the unused export entitlement is released and cannot later export redeemed
  A666;
- the fresh NAV mark includes that updated reserve exactly once;
- the governed route advances to that fresh pricing epoch;
- redemption retires the exact A666 amount and releases the exact governed
  pfUSDC amount;
- no Uniswap swap is used to obtain or redeem the demonstrated A666;
- no manual ledger edit, validator-local mutation, replay, or hidden
  inventory transfer occurs;
- all six validators converge; and
- every claim is backed by the machine evidence in section 2.2.3.

#### 2.2.5 Stop conditions

The live demo MUST NOT begin if:

- StakeHub NAV or pfUSDC route state is stale;
- validator heights, state roots, policies, or route epochs disagree;
- the user lacks confirmed USDC, ETH gas, or required PFTL authority;
- an active reservation or in-flight operation would make the before-state
  ambiguous;
- issue or redemption capacity is insufficient;
- the exact issue and redemption amounts have not passed a dry-run
  calculation; or
- the demo would require an emergency rolling upgrade, manual state edit, or
  unrehearsed live-value code change.

#### 2.2.6 Explicitly deferred from Monday

The following are outside the Monday critical path:

- implementing `NavReserveRedemptionFacilityV1`;
- `nav_reserve_facility_post_v1` or facility-withdraw operations;
- the new settlement-asset registry;
- the generic settlement-price packet replacing deployed pfUSDC par pricing;
- pfXRP, pfETH, pfStakedETH, or pfBTC;
- new Asset-Orchard circuit work or a private demo;
- new bridge contracts or trust classes;
- creating, redeploying, or reseeding the Uniswap pool;
- large-capacity or multi-user claims; and
- public production-GA claims.

These remain required for the longer-term system but are not allowed to delay
the narrow proof that buyer-funded pfUSDC becomes A666 reserve and supports
primary redemption.

#### 2.2.7 Required preparation checklist

- [ ] Implement or assemble a narrow orchestration wrapper for exactly the
  section 2.2.1 sequence; it may compose existing operations but MUST NOT add a
  consensus transaction kind.
- [ ] Make every generated artifact fail on overwrite and bind it to the fresh
  route epoch, policy hash, NAV packet, account, and amount.
- [ ] Rehearse the exact reserve -> subscribe -> entitlement-release sequence
  without submitting an export operation.
- [ ] Rehearse the fresh NAV mark and governed route epoch advance with zero
  active reservations and zero export entitlements.
- [ ] Calculate the maximum redeemable A666 from the incremental base reserve
  after the fresh NAV is known; do not assume the full issued amount remains
  redeemable if NAV moves.
- [ ] Rehearse the exact redemption and final six-validator reconciliation.
- [ ] Produce one operator-facing command sheet with explicit stop points
  before Ethereum deposit, PFTL issue, NAV finalization, route advance, and
  redemption.
- [ ] Complete at least one clean dress rehearsal using the same code,
  topology, authorizations, and evidence schema intended for Monday.

## 3. Economic principles

### 3.1 Unit of account is separate from settlement asset

The NAVCoin may be valued in `USD_1E8` while settling in XRP, ETH, staked ETH,
BTC, or a USD asset. Settlement asset selection does not change the NAVCoin's
unit of account.

### 3.2 Primary issue and redemption change supply

This system is primary issuance and primary redemption:

- issue adds verified reserve value and creates NAVCoin supply;
- redemption removes reserve value and retires NAVCoin supply.

It is not an OTC transfer of existing operator inventory.

### 3.3 Posted reserve, not an operator promise

A redemption facility is live only when the exact settlement inventory is:

- represented by a valid PFTL pfAsset;
- held in consensus-committed facility escrow;
- unencumbered by another allocation;
- included exactly once in NAV accounting when fund-owned;
- subject to a fresh valuation policy; and
- withdrawable only through the facility state machine.

An API response, database row, wallet balance screenshot, external custody
statement, or operator signature is not posted reserve.

### 3.4 Productive reserve is allowed

Facility inventory may be a productive asset. For example, a
`pfStakedETH` reserve may continue to accrue staking value while escrowed.
Yield and index changes must be captured by the registered valuation policy
and the next finalized NAV packet.

Consensus must account in immutable share units. It must not mutate balances
using an off-chain rebase, floating-point index, or wall-clock calculation.

### 3.5 In-kind redemption is preferred

When a posted asset is already part of the NAVCoin portfolio, in-kind
redemption is the most capital-efficient path:

```text
fund reserve asset leaves
NAVCoin liability is retired
```

No intermediate USDC sale is required.

### 3.6 Reserve composition may change

A user may issue through one facility and later redeem through another if both
facilities independently have capacity. This changes portfolio composition
but must not create or destroy value outside the disclosed spreads and price
movements.

Asset-specific caps, haircuts, and pause controls protect the fund from
unbounded composition drift and adverse selection.

### 3.7 Phase-one pfUSDC seed capital

Primary issue and primary redemption have different inventory requirements:

- issue does not require pre-posted pfUSDC because the buyer supplies the
  pfUSDC that backs the newly created A666; and
- redemption requires escrowed pfUSDC before the facility may advertise a
  nonzero executable redemption size.

A phase-one facility with zero posted pfUSDC may expose issue capacity, but it
MUST expose:

```text
actual executable redemption capacity = 0
```

It MUST NOT be described as a functional two-sided facility.

Before activating a two-sided pfUSDC quote, the operator MUST post enough
pfUSDC to cover the intended concurrently executable redemption size:

```text
required_initial_pfUSDC_atoms
  >= settlement_out_atoms(target_initial_redeem_nav_atoms)
```

The calculation uses the exact section 11 redemption formula and the current
NAV, pfUSDC price packet, redemption multiplier, haircut, and rounding rules.
Any governed minimum reserve or safety buffer is withheld from executable
capacity rather than counted twice.

For example, at A666 NAV `$1.00`, pfUSDC price `$1.00`, redemption multiplier
`0.9995`, and no outgoing haircut, a displayed executable redemption size of
`100,000 A666` requires at least:

```text
99,950.000000 pfUSDC
```

The initial seed may come from:

1. existing A666 `settlement_reserve_atoms`, but only through a
   reconciliation-gated transition that removes the same atoms from the
   legacy route before or atomically with crediting the new facility;
2. a new fund-owned pfUSDC contribution posted through
   `nav_reserve_facility_post_v1` and included under the section 10 NAV rules;
   or
3. completed subscriptions after activation, whose base settlement principal
   automatically increases facility reserve under section 10.2.

An external wallet balance, an unfunded policy capacity, the general value of
non-pfUSDC portfolio assets, or the existing reserve counted simultaneously
in two state objects is not phase-one pfUSDC liquidity.

Posting seed capital does not create A666. Reclassifying already counted
pfUSDC leaves NAV and supply unchanged. Newly contributed pfUSDC requires the
specified reserve-packet treatment before it becomes counted holder backing.
There is no protocol requirement to pre-fund the entire `2,000,000 A666`
policy capacity: the facility publishes only the smaller redemption size
that its currently unreserved escrow can actually honor.

## 4. Non-goals

Version 1 does not:

- accept an unregistered token because its ticker resembles an approved asset;
- treat a mutable display symbol as asset identity;
- use Uniswap or another single venue spot price as the sole valuation source;
- guarantee that an external issuer, custodian, bridge, or source chain cannot
  freeze or fail;
- make a transfer-restricted security permissionless;
- allow a quote to exceed escrowed reserve;
- allow an operator to revoke an already committed user reservation;
- provide an unfunded redemption promise;
- make Ethereum or another public-chain withdrawal private;
- allow hidden manual balance repair;
- pool third-party liquidity-provider claims with holder backing without
  recording the corresponding liability; or
- mutate A666 v2 state in place without a versioned migration.

## 5. Terminology

| Term | Meaning |
|---|---|
| NAVCoin | A PFTL-native liability whose valid supply and NAV are bound to a finalized reserve policy. |
| pfAsset | A source-labeled PFTL representation of an external or native settlement asset. |
| Facility | One versioned `NAVCoin / pfAsset` two-sided primary market. |
| Reserve inventory | pfAsset atoms escrowed to honor facility redemption. |
| Reserve reservation | A temporary consensus lock of exact output atoms for one user order. |
| Ask | Settlement pfAsset required to create NAVCoin. |
| Bid | Settlement pfAsset delivered when NAVCoin is retired. |
| Base value | Pre-spread value of the NAVCoin quantity at finalized NAV. |
| Settlement price | Finalized unit-of-account price of one displayed settlement asset unit. |
| Facility epoch | Monotonic policy version for one facility. |
| In-kind reserve | A fund-owned portfolio asset posted directly for redemption. |
| Source-labeled | Bound to exact chain, vault, token/native identity, verifier, and proof profile. |

## 6. Trust and execution claim

### 6.1 Supported claim

The system may be described as trustless execution only when:

1. reserve inventory is escrowed in consensus state;
2. the NAV and settlement price proofs are fresh;
3. the order fits actual available capacity;
4. the user signs the exact order and quote commitment;
5. PFTL atomically retires or creates NAVCoin and transfers the corresponding
   PFTL pfAsset;
6. no operator signature is required on the individual execution;
7. replays and competing consumes cannot both succeed; and
8. any external-chain exit is accepted only through the registered proof and
   replay boundary.

### 6.2 What remains trusted or externally risky

Trustless execution does not remove:

- PFTL consensus liveness assumptions;
- source-chain finality and liveness assumptions;
- disclosed bridge or checkpoint assumptions;
- custodian, wrapper, staking, slashing, depeg, and issuer-freeze risk;
- completeness risk in the NAVCoin reserve perimeter;
- legal transfer restrictions; or
- the operator's discretion about whether to post or replenish future reserve.

The operator may observe that a redemption occurred. A private PFTL execution
can hide the holder's note lineage, but a public source-chain payout reveals
destination, asset, amount, and timing.

## 7. Roles and authorization

| Role | Authority |
|---|---|
| NAVCoin issuer | Creates a versioned facility subject to NAV policy and governance rules. |
| Reserve operator | Posts or reclassifies fund-owned reserve, advances facility epochs, and pauses new reservations. |
| Governance | Registers asset profiles, bounds policy parameters, and authorizes upgrades. |
| User | Reserves and executes issue or redemption using a locally signed order. |
| Relayer | Transports signed payloads and proofs; cannot change asset, amount, recipient, price, or route. |
| Prover | Produces proofs for committed public values; receives no unrestricted spending authority. |
| Validator | Deterministically verifies and applies state transitions. |

The issuer or registered reserve operator may manage facility policy. Neither
may approve, reject, or redirect an individual conforming execution.

Version 1 supports only fund-owned reserve. Third-party liquidity is a later
profile requiring an explicit provider liability and settlement rule. Without
that liability, third-party reserve would be either an undocumented donation
or incorrectly counted holder backing.

## 8. Settlement asset registry

### 8.1 Registry entry

Every settlement asset must have a content-addressed registry entry:

```text
asset_id
display_family
issued_asset_precision
source_domain
source_chain_id_or_genesis
source_asset_kind
source_asset_identifier
source_asset_precision
source_vault_or_custody_identity
deposit_verification_class
withdrawal_verification_class
proof_profile_id
valuation_policy_hash
valuation_unit
price_precision
maximum_price_age_blocks
maximum_reserve_proof_age_blocks
haircut_policy_bounds
transfer_restriction_class
freeze_and_clawback_disclosure
bridge_route_id
registry_version
registry_hash
```

The registry hash is included in every facility policy, quote, reservation,
receipt, bridge packet, and state commitment.

Existing anchors the registry MUST build on rather than replace:

- the `nav_asset_register` and `nav_profile_register` transaction kinds and
  their content-addressed profile records;
- the deployed vault-bridge source-domain convention
  (`erc20_bridge_vault:<chain_id>:<vault_address>:<token_address>`, with
  lowercased addresses) already used for bucket selection and egress policy
  matching; and
- the deployed `route_config_digest` content addressing on
  `PftlUniswapConsensusRouteState`, plus the distinct `route_profile_hash`
  binding used by vault-bridge finality profiles and pfAsset egress scripts.

A registry entry for a new pfAsset is therefore an extension of the existing
profile/source-labeling machinery with the additional valuation, precision,
haircut-bounds, and restriction-disclosure fields listed above — not a new
parallel identity system.

### 8.2 Product families

| Family | Required profile behavior |
|---|---|
| `pfUSD` | Bind the exact underlying USD asset. USDC, USDT, tokenized deposits, and Treasury tokens must not share an ambiguous asset ID. |
| `pfXRP` | Bind the exact XRP source domain and custody/bridge route; account for drops and XRP reserve requirements where applicable. |
| `pfETH` | Bind native ETH or a specific wrapped-ETH contract; never treat them as interchangeable without a registered conversion rule. |
| `pfStakedETH` | Use non-rebasing share atoms or a proof-bound share index; bind validator/slashing, withdrawal, and transfer policy. |
| `pfBTC` | Bind the exact BTC custody or wrapper model, chain proof, confirmation policy, and redemption authority. `WBTC`, native-BTC custody receipts, and other wrappers are distinct assets. |

“Arbitrary” means registry-extensible. It does not mean permissionless
admission of unreviewed assets into a NAVCoin's backing.

## 9. Facility state

The implementation must introduce a versioned state object, conceptually:

```text
NavReserveRedemptionFacilityV1 {
  facility_id
  facility_epoch
  nav_asset_id
  settlement_asset_id
  settlement_registry_hash

  nav_issue_multiplier_bps
  nav_redeem_multiplier_bps
  incoming_asset_value_bps
  outgoing_asset_value_bps

  issue_capacity_nav_atoms
  redeem_capacity_nav_atoms
  minimum_order_nav_atoms
  maximum_order_nav_atoms

  valid_from_height
  expires_at_height
  maximum_nav_age_blocks
  maximum_settlement_price_age_blocks

  pricing_nav_epoch
  pricing_nav_packet_hash
  settlement_price_epoch
  settlement_price_packet_hash

  reserve_inventory_atoms
  reserve_reserved_atoms
  reserve_pending_egress_atoms
  issue_used_nav_atoms
  redeem_used_nav_atoms
  spread_inventory_atoms

  live_value_enabled
  issue_paused
  new_redemption_reservations_paused

  active_reservations
  terminal_reservation_ids
  consumed_issue_nonces
  consumed_redemption_nonces

  spread_destination
  policy_hash
}
```

Consensus collections must use canonical sorted encodings and deterministic
maps. Facility, reservation, and nonce counts must be bounded.

### 9.1 Policy bounds

- multipliers and haircuts must be governance-bounded;
- issue multiplier must be at or above the governed minimum;
- redemption multiplier must be positive and at or below the governed maximum;
- incoming asset value must be at most par with its verified price;
- outgoing delivery value must be at least par with its verified price;
- minimum order must not exceed maximum order;
- maximum order must fit both issue and redemption capacity;
- validity and freshness windows must be nonzero and ordered; and
- every hash must equal its canonical computed value.

### 9.2 Spread destination

Every facility selects one immutable-per-epoch spread treatment:

```text
NAV_RESERVE
NON_NAV_FEE_CUSTODY
```

The default for a new product should be `NAV_RESERVE` so facility economics
accrue to NAVCoin holders. A666 compatibility may retain
`NON_NAV_FEE_CUSTODY`. Wallets must disclose the selected treatment.

The deployed implementation already accounts spread separately as
`non_nav_spread_atoms` on the route state; that is the existing concrete form
of `NON_NAV_FEE_CUSTODY`. `NAV_RESERVE` is new behavior and requires the
corresponding NAV-packet inclusion rule from section 10.

### 9.3 Relationship to deployed A666 v2 types

`NavReserveRedemptionFacilityV1` generalizes two deployed structures and MUST
reuse their validation discipline rather than restate it:

| Facility field group | Deployed precedent or nearest analogue |
|---|---|
| Multipliers, capacities, order bounds, validity heights, `maximum_nav_age_blocks`, `pricing_nav_epoch`, `pricing_nav_packet_hash`, `policy_hash` | `PftlUniswapPrimaryMarketPolicyV2` contains the semantic counterparts; NRRS normalizes several field names and adds settlement-price and haircut terms |
| `active_reservations`, `terminal_reservation_ids`, reserved-sum accounting, pinning of every reservation to route epoch + policy epoch + policy hash | `PftlUniswapRouteV2State.active_reservations` / `terminal_reservations` validation |
| Bounded consensus collections | `MAX_PFTL_UNISWAP_ROUTE_ENTRIES`-style constants; NRRS MUST define equivalent bounds per facility |
| `reserve_pending_egress_atoms` | Export entitlement / export packet accounting is the nearest exact-once lifecycle precedent, but it currently moves NAVCoin rather than settlement reserve; the new settlement-asset conservation semantics require separate vectors |
| Pause semantics | deployed `pftl_uniswap_route_pause` and the `paused` / `live_value_enabled` flags |

New fields with no deployed precedent (and therefore the actual
implementation surface): `settlement_asset_id` as a per-facility variable,
`settlement_registry_hash`, `incoming_asset_value_bps` /
`outgoing_asset_value_bps` haircuts, `settlement_price_epoch` /
`settlement_price_packet_hash`, `reserve_inventory_atoms` as operator-posted
escrow, `spread_destination` selection, and separate issue/redeem pause
flags.

## 10. Reserve posting and double-count prevention

### 10.1 Posting fund-owned reserve

`facility_reserve_post` transfers pfAsset atoms from a NAVCoin-controlled,
verified reserve allocation into facility escrow.

If the asset was already counted in NAV:

```text
unencumbered portfolio allocation -= posted value
facility reserve allocation       += posted value
total verified net assets          unchanged
NAVCoin supply                     unchanged
```

If the asset was not previously counted, posting is a new capital
contribution. It must not become holder backing until a new reserve packet and
NAV epoch explicitly include it. It does not create NAVCoin supply by itself.

### 10.2 Subscription-funded reserve

On primary issue, the incoming base settlement value becomes facility reserve:

```text
user settlement pfAsset -= total issue due
facility reserve         += base settlement atoms
spread destination       += spread settlement atoms
NAVCoin supply           += issued NAVCoin atoms
user NAVCoin             += issued NAVCoin atoms
```

### 10.3 Reserve withdrawal

An operator may withdraw only reserve that is:

- above all active and pending reservations;
- above the facility's governed minimum reserve;
- not required by an unexpired quote reservation;
- reconciled against source-vault backing;
- reflected by a new reserve allocation and, when material, a fresh NAV packet;
  and
- outside any safety hold.

Pausing new reservations does not cancel existing reservations.

### 10.4 Conservation identities

For each facility:

```text
escrowed pfAsset
  = available reserve
  + reserved reserve
  + pending external egress
  + spread inventory held by the facility
```

Across the NAVCoin:

```text
valid NAVCoin supply
  = transparent PFTL NAVCoin
  + private PFTL NAVCoin custody
  + external wrapped NAVCoin
  + valid in-flight bridge claims
```

Across reserve allocations:

```text
verified net assets
  = unencumbered counted portfolio value
  + facility reserve value
  + other counted allocations
  - recorded liabilities
```

The same asset atoms must never appear in more than one term.

## 11. Deterministic two-sided pricing

### 11.1 Inputs

Let:

```text
Q = NAVCoin order quantity in NAVCoin atoms
N = finalized NAVCoin price in USD_1E8 per displayed NAVCoin
P = finalized settlement asset price in USD_1E8 per displayed pfAsset
Dnav = 10 ^ NAVCoin precision
Dset = 10 ^ settlement asset precision
Ai = NAV issue multiplier in basis points
Br = NAV redemption multiplier in basis points
Hi = incoming settlement-asset value factor in basis points
Ho = outgoing settlement-asset delivery factor in basis points
BPS = 10,000
```

Required relationships:

```text
Ai >= BPS
0 < Br <= BPS
0 < Hi <= BPS
Ho >= BPS
```

`Hi < 10,000` means incoming settlement assets receive a conservative
haircut. `Ho > 10,000` means outgoing settlement assets use a conservative
delivery price.

### 11.2 Issue quote

```text
nav_value_usd_e8 =
  ceil(Q * N / Dnav)

issue_value_usd_e8 =
  ceil(nav_value_usd_e8 * Ai / BPS)

incoming_effective_price_usd_e8 =
  floor(P * Hi / BPS)

settlement_in_atoms =
  ceil(issue_value_usd_e8 * Dset / incoming_effective_price_usd_e8)
```

### 11.3 Redemption quote

```text
nav_value_usd_e8 =
  floor(Q * N / Dnav)

redeem_value_usd_e8 =
  floor(nav_value_usd_e8 * Br / BPS)

outgoing_delivery_price_usd_e8 =
  ceil(P * Ho / BPS)

settlement_out_atoms =
  floor(redeem_value_usd_e8 * Dset / outgoing_delivery_price_usd_e8)
```

### 11.4 Arithmetic rules

- floating point is forbidden;
- all operations use checked wide integers;
- the implementation must prove bounds for `u128` or use an already-reviewed
  wider integer type;
- multiplication and division order is normative;
- divide-by-zero, overflow, underflow, or zero-output quotes reject;
- asset precision and price scale are registry-bound;
- validators must produce identical quote bytes; and
- wallets must independently recompute every quote.

The implementation MUST reuse the deployed checked-integer pricing helpers in
`nav_vault_asset_execution.rs` (`checked_mul_div_ceil` /
`checked_mul_div_floor`, `u128` intermediates, `u64` results, deterministic
`pftl_uniswap_pricing_overflow`-class error codes) as the arithmetic
foundation, extended with the settlement-price and haircut terms. The
existing helpers encode the normative rounding directions for the multiplier
step; the new price-division step MUST add reference vectors of its own
(section 23.1) because it introduces a second rounding boundary the deployed
code does not have.

### 11.5 Example: pfBTC facility

Assume:

```text
NAVCoin NAV       = $1.00
pfBTC price       = $100,000
issue multiplier  = 1.005
redeem multiplier = 0.9995
asset factors     = 1.0000 / 1.0000
posted reserve    = 2.00000000 pfBTC
```

Then:

```text
issue 100,000 NAVCoin:
  user pays 1.00500000 pfBTC
  100,000 NAVCoin are created

redeem 100,000 NAVCoin:
  user receives 0.99950000 pfBTC
  100,000 NAVCoin are retired
```

The posted two-BTC reserve supports at most the NAV-equivalent quantity
computed from actual unreserved inventory and all other policy bounds. It is
not an unlimited promise.

## 12. Executable capacity

### 12.1 Issue

```text
available_issue_nav_atoms =
  min(
    remaining issue capacity,
    maximum order,
    NAVCoin route/supply headroom,
    incoming settlement allocation headroom,
    destination export headroom
  )
```

The user brings the settlement asset, so existing facility inventory is not an
issue prerequisite.

### 12.2 Redemption

```text
available_redeem_nav_atoms =
  min(
    remaining redemption capacity,
    maximum order,
    valid redeemable NAVCoin supply,
    NAVCoin return/import capacity,
    NAV-equivalent of unreserved facility inventory,
    settlement bridge exit capacity
  )
```

The NAV-equivalent inventory calculation must use the exact redemption formula
and may be implemented as a bounded deterministic binary search.

### 12.3 Portfolio and concentration limits

Each facility may additionally enforce:

- maximum portfolio weight after issue;
- minimum portfolio weight after redemption;
- maximum issue/redeem flow per block or epoch;
- maximum outstanding reservations;
- maximum external bridge exposure;
- maximum source-custodian concentration; and
- maximum permitted price deviation between independent sources.

These bounds must be consensus-visible. An off-chain risk engine may recommend
limits but cannot silently override consensus.

## 13. Quote and reservation protocol

### 13.1 Read-only indicative quote

The quote RPC returns a deterministic preview. It is not a guaranteed fill
until capacity is reserved in consensus.

### 13.2 Consensus reservation

The user signs:

```text
reservation_id
direction
facility_id
facility_epoch
policy_hash
nav_asset_id
settlement_asset_id
settlement_registry_hash
nav_quantity_atoms
maximum_settlement_in_atoms OR minimum_settlement_out_atoms
NAV epoch and packet hash
settlement price epoch and packet hash
PFTL recipient
external destination and route, if requested
privacy mode
created_at_height
expires_at_height
user nonce
```

For redemption, reservation atomically moves the exact pfAsset output from
`available reserve` to `reserved reserve`. Competing users cannot consume it.

For issue, reservation locks issue, route, and allocation capacity and escrows
the user's maximum input where supported.

### 13.3 Execution

Execution must:

- match the reservation byte-for-byte;
- recompute pricing from the pinned packets;
- reject after expiry;
- consume the reservation and nonce exactly once;
- apply all balance, supply, reserve, capacity, and spread changes atomically;
- emit a canonical receipt; and
- move the reservation ID into a bounded terminal replay registry.

### 13.4 Expiry and release

Expiry releases capacity to the original state owner. Anyone may relay an
expiry transition after the deadline, but cannot redirect value.

Operator pause affects only new reservations. Existing reservations may
execute or expire under their committed rules.

## 14. Consensus operations

Version 1 should define:

```text
nav_reserve_facility_register_v1
nav_reserve_facility_post_v1
nav_reserve_facility_withdraw_v1
nav_reserve_facility_epoch_advance_v1
nav_reserve_facility_pause_v1
nav_reserve_quote_reserve_v1
nav_reserve_quote_release_v1
nav_reserve_primary_issue_v1
nav_reserve_primary_redeem_v1
nav_reserve_external_egress_commit_v1
nav_reserve_external_egress_consume_v1
nav_reserve_external_egress_cancel_v1
```

These operations follow the existing transaction-kind registry convention
(snake-case kind strings such as `nav_reserve_submit`,
`pftl_uniswap_order_reserve`) and, where a deployed A666 v2 operation is
being generalized, MUST preserve its semantics for the compatibility facility
(section 22, Phase C):

| NRRS operation | Deployed precedent generalized |
|---|---|
| `nav_reserve_facility_register_v1` | `pftl_uniswap_route_init` + `market_ops_policy_register` |
| `nav_reserve_facility_epoch_advance_v1` | policy-epoch advance on the deployed route |
| `nav_reserve_facility_pause_v1` | `pftl_uniswap_route_pause` (split into per-direction flags) |
| `nav_reserve_quote_reserve_v1` | `pftl_uniswap_order_reserve` |
| `nav_reserve_quote_release_v1` | `pftl_uniswap_order_release` |
| `nav_reserve_primary_issue_v1` | `pftl_uniswap_primary_subscribe` |
| `nav_reserve_primary_redeem_v1` | `pftl_uniswap_primary_redeem` + `NavRedeemSettleOperation` settlement |
| `nav_reserve_external_egress_*_v1` | `pftl_uniswap_export_debit` / export packet consume / `pftl_uniswap_refund_source` provide the exact-once packet precedent; settlement-asset direction and conservation are new |
| `nav_reserve_facility_post_v1`, `nav_reserve_facility_withdraw_v1` | **no deployed precedent** — new operator escrow lifecycle |

Deterministic error codes MUST follow the deployed style: stable snake-case
code strings (`nav_reserve_*`, mirroring the existing `pftl_uniswap_*`
catalog such as `pftl_uniswap_reservation_policy_mismatch`,
`pftl_uniswap_issue_capacity_exceeded`,
`pftl_uniswap_opening_inventory_double_count`).

Every operation must have:

- a versioned canonical schema;
- bounded field and collection sizes;
- domain-separated signing bytes;
- source-account authorization;
- canonical transaction and receipt identifiers;
- deterministic error codes;
- exact replay keys;
- state-root commitment coverage; and
- positive and adversarial test vectors.

## 15. Transparent and private execution

### 15.1 Transparent

Transparent execution debits and credits ordinary PFTL asset balances. The
facility, asset, amount, recipient, and timing are public.

### 15.2 Private

Private execution may consume and create Asset-Orchard typed notes. The proof
must bind:

- facility and policy identity;
- hidden input and output asset types;
- hidden values;
- quote commitment;
- NAV and settlement price packet commitments;
- exact public capacity deltas;
- nullifiers and output commitments;
- spend and binding authorization; and
- fee/spread treatment.

Consensus may publish aggregate facility inventory and capacity without
publishing the user's note opening or spending authority.

Deployed precedent: Asset-Orchard typed-note private primary issue and
redemption have run on the mainnet pfUSDC facility under controlled,
limited-availability operation
(`shielded_batch_actions`, `pftl_uniswap_private_primary_*` validation,
private-primary reservation replay rejection), fronted by the resident
private swap service described in the current-state document. NRRS private
execution extends the existing action circuits' public-input bindings with
the facility, registry, and settlement-price commitments listed above; it
MUST NOT introduce a second shielded execution path.

The prover must not receive unrestricted wallet keys. Non-custodial release
requires user-held spending authority and recoverable private wallet state.

### 15.3 Privacy boundary

Private PFTL execution does not hide:

- public reserve inventory and aggregate capacity;
- external source-chain transfers;
- a public withdrawal address;
- external token transfer restrictions; or
- timing correlation without batching or delay.

## 16. External-chain delivery

### 16.1 PFTL completion

The simplest completion boundary is receipt of the selected pfAsset on PFTL.
That transition is atomic with NAVCoin issue or retirement.

### 16.2 Direct external delivery

Direct delivery creates a destination-bound egress packet after the PFTL
transition. The source vault or adapter releases the underlying asset only
after verifying the registered PFTL finality proof.

Consumption and cancellation must be mutually exclusive. A timeout alone is
not evidence that an external transfer did not occur.

Required packet bindings include:

```text
facility_id and epoch
reservation_id
settlement_asset_id and registry hash
source domain and vault
destination chain and recipient
amount
PFTL finalized block and receipt
program/verifier identity
route epoch
expiry/cancellation policy
```

### 16.3 Directional trust disclosure

Every facility must publish the actual directional trust class, for example:

```text
TRUSTLESS_FINALITY
BFT_CHECKPOINT
OPTIMISTIC
TRANSFER_RESTRICTED_ISSUER
```

The first two are already deployed constants
(`PFTL_UNISWAP_TRUST_CLASS_TRUSTLESS_FINALITY`,
`PFTL_UNISWAP_TRUST_CLASS_BFT_CHECKPOINT`), and the deployed route publishes
`route_trust_class`, `outbound_verification_class`, and
`return_verification_class` in its supply status. The deployed code also has
a `CONTROLLED` trust class that forbids `live_value_enabled`; NRRS MUST
extend the deployed string-label and validator set in place (reusing
`OPTIMISTIC` and adding `TRANSFER_RESTRICTED_ISSUER`) rather than defining a
parallel enum, and MUST keep the `CONTROLLED`-implies-no-live-value rule.

Product copy must not collapse these into a generic “trustless bridge” claim.

## 17. Asset-specific requirements

### 17.1 pfUSD

- bind the exact source asset and issuer;
- disclose freeze, blacklist, and redemption rights;
- price depegs rather than hardcoding `$1`;
- prevent two stablecoins from sharing one reserve identity;
- support tokenized Treasury shares as a distinct registry asset; and
- disclose transfer/eligibility restrictions.

### 17.2 pfXRP

- use integer drops and exact source-ledger identity;
- bind destination tags or equivalent routing where required;
- prove source finality under the registered XRP route;
- account for custody and reserve requirements; and
- reject issuer or trustline substitution.

### 17.3 pfETH

- distinguish native ETH from WETH and other wrappers;
- bind chain ID and contract code hash for wrapped variants;
- use a finalized ETH/USD valuation policy; and
- account for source-chain gas separately from user principal.

### 17.4 pfStakedETH

- represent non-rebasing shares in consensus;
- prove the share-to-underlying index;
- combine index and underlying price with checked integer arithmetic;
- include slashing, validator, withdrawal-delay, and liquidity haircuts;
- define who receives accrued yield while reserved; and
- reject stale or decreasing indexes that violate the registered policy.

### 17.5 pfBTC

- distinguish native-custody BTC, WBTC, tBTC, and other wrappers;
- bind the exact custody and mint/burn authority;
- enforce source confirmation/finality policy;
- disclose seizure, pause, and custodian risk;
- use integer satoshis; and
- require a separate facility for each wrapper or source domain.

## 18. Operator controls and user protections

The operator may:

- post additional reserve;
- advance a facility epoch;
- reduce future capacity;
- pause new issue reservations;
- pause new redemption reservations on a documented risk event;
- withdraw unreserved surplus; and
- retire a facility after all obligations are terminal.

The operator may not:

- revoke an active reservation;
- change a reserved price or asset;
- redirect settlement;
- reuse a nonce;
- withdraw reserved or pending-egress inventory;
- count the same reserve elsewhere;
- disable return/import solely to avoid redemption;
- create synthetic pfAsset inventory; or
- repair balances outside normal consensus transitions.

Emergency controls must prefer safety while preserving deterministic user
recovery.

## 19. RPC and product surfaces

### 19.1 Facility listing

`nav_reserve_facilities` returns bounded rows containing:

- exact asset and registry identities;
- bid and ask;
- finalized NAV and settlement price;
- price and proof ages;
- actual issue and redemption availability;
- reserved and pending amounts;
- min/max order;
- epoch and expiry;
- spread destination;
- privacy support;
- external delivery routes;
- transfer restrictions; and
- trust classes.

### 19.2 Quote

`nav_reserve_quote` returns:

- canonical quote input echo;
- deterministic output;
- every limiting capacity term;
- exact rounding;
- policy and packet hashes;
- quote expiry;
- reservation requirement;
- estimated proof and bridge latency; and
- a machine-readable failure reason.

The wallet recomputes the quote and rejects unknown fields, missing fields,
asset substitution, stale proofs, inconsistent precision, and unsupported
trust classes.

### 19.3 User language

Allowed:

> Redeem up to 50,000 NAVCoin into pfBTC against posted reserve at the displayed
> finalized price and policy.

Not allowed:

> Two million redemption capacity.

unless two million is the current executable amount after inventory, price,
route, supply, reservation, and policy limits.

## 20. Failure and recovery

The system must fail closed on:

- stale NAV or settlement price;
- missing or invalid reserve proof;
- insufficient unreserved inventory;
- expired quote or reservation;
- duplicate nonce or reservation;
- wrong asset, route, recipient, or precision;
- overflow, underflow, or zero-output rounding;
- facility epoch mismatch;
- exceeded issue, redemption, concentration, or bridge capacity;
- source asset freeze or impairment policy trigger;
- conflicting external consume/cancel evidence;
- corrupt journal or state snapshot; and
- six-validator state disagreement.

Recovery must be deterministic:

- no publication means no value transition;
- published-but-uncommitted requests are recovered by exact request identity;
- committed requests are returned from durable receipt state;
- expired reservations release to the original source;
- external delivery resumes from the same packet;
- replay attempts return the original terminal result or reject; and
- manual ledger edits are forbidden.

## 21. State, storage, and upgrade requirements

- introduce new versioned state rather than reinterpreting A666 v2 fields;
- commit facility policy, inventory, reservations, nonces, pending egress, and
  receipts into the replicated state root;
- use canonical sorted encodings;
- bound facilities per NAVCoin, reservations per facility, and terminal replay
  history;
- define pruning only after a signed content-addressed checkpoint preserves
  replay safety;
- support deterministic export/import and clean recovery;
- gate activation by protocol version and deployment manifest;
- provide a downgrade/rollback plan before activation; and
- migrate no live value until old and new conservation reports agree exactly.

## 22. Current A666 delta

What the deployed A666 v2 implementation **already has** (NRRS reuses, not
rebuilds):

- one route with one `settlement_asset_id` and one
  `settlement_reserve_atoms` balance
  (`PftlUniswapConsensusRouteState`; its `v2` member carries the policy and
  reservation state);
- one primary-market policy object with multipliers, capacities, order
  bounds, validity heights, NAV-age freshness, and pinned pricing packet
  (`PftlUniswapPrimaryMarketPolicyV2`);
- a working consensus reservation lifecycle: `pftl_uniswap_order_reserve` /
  `pftl_uniswap_order_release`, bounded active/terminal reservation maps,
  reservations pinned to route epoch + policy epoch + policy hash, expiry,
  per-wallet reservation limits, and private-primary reservation replay
  rejection;
- deterministic checked-integer pricing with normative rounding directions
  and a stable deterministic error-code catalog;
- opening-inventory double-count rejection
  (`pftl_uniswap_opening_inventory_double_count` and related codes);
- directional trust classes committed in state and published in supply
  status;
- spread accounting as `non_nav_spread_atoms` (the `NON_NAV_FEE_CUSTODY`
  treatment only);
- redemption settlement top-up drawn from verified vault-bridge allocations
  at settlement time (`NavRedeemSettleOperation`, bucket-matched by source
  domain and policy hash); and
- export entitlements/packets, return import, refund, and pause operations.

What it **does not have** (the actual NRRS implementation surface):

- more than one facility or settlement asset per NAVCoin;
- a settlement-price packet: pfUSDC is priced at par USD, so there is no
  finalized `P`, no price-freshness window, and no depeg handling;
- incoming/outgoing haircut factors (`Hi` / `Ho`);
- a standing operator-posted facility escrow with post/withdraw lifecycle
  (the vault-bridge top-up is settlement-time sourcing, not posted
  reserve inventory);
- the `NAV_RESERVE` spread destination;
- the settlement asset registry of section 8 as a first-class object;
- in-kind (non-USD, share/index-valued) reserve facilities; and
- per-direction reservation pause flags.

NRRS should be implemented as a versioned successor or parallel facility
layer. It must reuse:

- generic issued assets and trustlines;
- NAV proof profiles and reserve packets;
- vault-bridge source labeling;
- deterministic issue/redeem arithmetic;
- Asset-Orchard typed notes;
- bridge receipt proofs;
- exact-once journals; and
- six-validator state-root convergence checks.

It must not silently mutate the deployed A666 policy or reinterpret existing
pfUSDC reserve atoms.

## 23. Adversarial and invariant test matrix

### 23.1 Arithmetic

- one atom below/at/above every rounding boundary;
- every supported precision pair;
- maximum quantity and price;
- checked overflow and division by zero;
- depegged pfUSD;
- large BTC/ETH price movement;
- staked-asset index increase and policy-valid decrease;
- haircut and multiplier boundaries; and
- independent reference-vector comparison.

### 23.2 State transitions

- reserve post from counted and uncounted sources;
- exact double-count rejection;
- issue and redemption conservation;
- cross-facility issue/redeem;
- spread to each allowed destination;
- reserve withdrawal above/at/below surplus;
- pause with active reservations;
- epoch advance with active obligations;
- duplicate and reordered operations; and
- deterministic replay from genesis and checkpoint.

### 23.3 Concurrency and Byzantine behavior

- two users racing for the last reserve atom;
- reserve withdrawal racing a reservation;
- expiry racing execution;
- duplicate relayers;
- malformed proof, packet, and registry entry;
- stale but correctly signed quote;
- conflicting external consume/cancel evidence;
- validator restart after reservation and after publication;
- bounded reservation and facility exhaustion; and
- state-root equality across all validators.

### 23.4 Privacy

- wrong hidden asset type;
- wrong hidden value;
- quote/facility substitution;
- duplicate nullifier;
- stale anchor;
- changed public capacity delta;
- private evidence forbidden-field scan; and
- wallet recovery/rescan from checkpoint.

### 23.5 External routes

For every supported pfAsset:

- real deposit and proof;
- PFTL balance and facility posting;
- issue and redemption;
- real external withdrawal;
- replay rejection;
- wrong recipient;
- wrong source token or chain;
- source pause/freeze behavior;
- timeout, restart, and cancellation; and
- exact vault/issued-supply conservation.

## 24. Implementation phases

The section 2.2 Monday demonstration occurs before these implementation
phases. It reuses the deployed A666/pfUSDC route and MUST NOT be represented
as completion of the proposed generic NRRS successor.

### Phase A — Economic and schema freeze

- [ ] Freeze reserve ownership and spread treatment.
- [ ] Freeze the phase-one pfUSDC seed amount, source allocation, governed
  minimum reserve, and reconciliation procedure.
- [ ] Freeze the pfUSDC price-packet sources, aggregation/quorum rule,
  freshness window, depeg behavior, and recovery authority.
- [ ] Freeze deterministic price and rounding vectors.
- [ ] Freeze registry, facility, reservation, receipt, and egress schemas.
- [ ] Complete threat model and storage-growth bounds.

### Phase B — Transparent PFTL prototype

- [ ] Implement versioned types and validation.
- [ ] Implement state commitments.
- [ ] Implement reserve post/withdraw and epoch lifecycle.
- [ ] Implement quote, reservation, issue, redeem, and expiry.
- [ ] Add RPC and independent wallet verification.

### Phase C — pfUSDC-only transparent activation

- [ ] Register the current pfUSDC lane (deployed route
  `pftl-a666-ethereum-wA666-usdc-v1`, vault-bridge source-labeled pfUSDC)
  as one concrete `pfUSD` family asset, priced by a real settlement-price
  packet instead of hardcoded par.
- [ ] Capture the pre-activation legacy settlement reserve, allocation
  obligations, and unencumbered amount from fresh consensus state.
- [ ] Complete the old/new dry-run reconciliation before moving live value.
- [ ] Post the governed initial pfUSDC seed through a signed transition and
  prove it is not simultaneously available to the legacy route or another
  allocation.
- [ ] Publish actual executable redemption capacity derived from the posted,
  unreserved seed rather than from policy capacity.
- [ ] Prove A666-equivalent economics through the new facility.
- [ ] Complete transparent issue/redeem, last-liquidity race, expiry, restart,
  and reserve-withdrawal tests.

### Phase D — pfUSDC private and external completion

- [ ] Add typed-note pfUSDC facility issue and redemption by extending the
  existing Asset-Orchard path.
- [ ] Keep spending authority client-side.
- [ ] Prove recovery and forbidden-field behavior.
- [ ] Register and prove the canonical Ethereum-mainnet USDC route.
- [ ] Implement consume/cancel mutual exclusion.
- [ ] Guarantee that failed or cancelled external delivery returns the exact
  pending pfUSDC to the redeeming user's PFTL control.
- [ ] Publish directional trust and restriction disclosures.

### Phase E — pfUSDC controlled live qualification

- [ ] Run small and variable-size transparent pfUSDC issue/redeem.
- [ ] Run small and variable-size private pfUSDC issue/redeem.
- [ ] Run variable-size and concurrency ladders.
- [ ] Complete restart, expiry, stale-price, and last-liquidity races.
- [ ] Complete a real Ethereum USDC -> pfUSDC -> A666 -> pfUSDC -> Ethereum
  USDC round trip.
- [ ] Reconcile NAV, A666 supply, facility inventory, pfUSDC supply, and the
  Ethereum vault balance.

### Phase F — later asset expansion

Only after phase-one pfUSDC passes Gate R5:

- [ ] Add pfETH.
- [ ] Add pfBTC.
- [ ] Add pfXRP.
- [ ] Add pfStakedETH share/index valuation.
- [ ] Activate one new asset at a time behind its own live-value gate.

### Phase G — multi-asset qualification

- [ ] Run transparent issue/redeem for every added asset.
- [ ] Run private issue/redeem for every eligible added asset.
- [ ] Run same-asset and cross-facility amount ladders.
- [ ] Complete each asset's external round trip, recovery, and restriction
  tests.
- [ ] Reconcile NAV, supply, facility inventory, pfAsset supply, and external
  vault balances across all active facilities.

## 25. Release gates

### Gate R0 — specification

- schema, math, trust model, and reserve ownership are frozen;
- the phase-one pfUSDC seed source and target executable redemption size are
  frozen;
- all open economic decisions have an owner; and
- no document calls policy capacity executable liquidity.

### Gate R1 — deterministic local

- all arithmetic vectors pass;
- state-root replay is deterministic;
- adversarial state tests pass; and
- malformed inputs cannot panic or cause unbounded work.

### Gate R2 — six-validator transparent

- all validators converge through every facility transition;
- two-user last-liquidity race commits at most one consume;
- crash recovery is exact-once; and
- NAV/supply/reserve invariants hold.

### Gate R3 — private

- private issue/redeem passes with client-held authority;
- note recovery/rescan passes;
- evidence is redaction-safe; and
- public capacity changes equal hidden economic changes.

### Gate R4 — external completion, pfUSDC first

- the phase-one pfUSDC asset completes a real Ethereum-mainnet round trip;
- every later facility that advertises external delivery completes its own
  real route before that facility can pass R6;
- bridge/vault replay and cancellation tests pass;
- failed or cancelled external delivery returns the exact pending pfUSDC to
  the redeeming user's PFTL control;
- transfer restrictions are accurately surfaced; and
- no route is described with a stronger trust class than deployed.

### Gate R5 — pfUSDC controlled availability

- sustained issue/redeem and concurrency campaigns pass;
- p95 and worst-case latency meet the published SLO, measured with the same
  machine-stamped qualification discipline as the existing private-swap
  gate (currently a `42-second` issue gate, most recently missed at p95
  `50.365s`; NRRS facilities MUST publish their own per-asset gates rather
  than inherit that number);
- monitoring and rollback are proven;
- actual available capacity is shown separately from policy capacity; and
- all live-value limits are bounded by the smallest proven run.

Passing Gate R5 authorizes only the registered pfUSDC facility. It does not
authorize another settlement asset.

### Gate R6 — later asset activation

For each later asset independently:

- its registry, proof, price, haircut, custody, and transfer rules pass R0-R1;
- its transparent facility passes R2;
- its private path, if advertised, passes R3;
- its external route, if advertised, passes R4;
- its own latency, recovery, concurrency, and live-value limits pass R5; and
- activating it does not modify the already registered pfUSDC facility.

## 26. Definition of done

### 26.1 Phase-one pfUSDC release

The first executable release is complete when A666 has one generic-schema
pfUSDC facility that:

1. uses a fresh registered pfUSDC price packet rather than hardcoded par;
2. accepts buyer pfUSDC and creates new A666;
3. holds posted pfUSDC reserve without double counting;
4. publishes redemption capacity limited by actual unreserved escrow;
5. retires A666 and pays pfUSDC without per-user operator approval;
6. supports the qualified transparent and private PFTL paths;
7. completes or safely refunds Ethereum-mainnet USDC egress;
8. survives duplicates, races, stale prices, restarts, and expiry; and
9. reconciles A666 supply, facility inventory, pfUSDC supply, source-vault
   backing, and NAV allocations after every transition.

No later asset is required to ship this phase-one release.

### 26.2 Multi-asset completion

The full multi-asset NRRS is complete when a NAVCoin operator can:

1. register an approved pfAsset facility;
2. post fund-owned reserve without double counting it;
3. publish deterministic two-sided terms;
4. expose the exact executable size;
5. leave the facility unattended for individual execution;
6. allow an unknown holder to reserve and redeem NAVCoin atomically for the
   posted pfAsset;
7. allow a user to issue new NAVCoin with the same pfAsset;
8. support transparent and private PFTL execution;
9. deliver the selected external asset through its registered proof route;
10. survive duplicates, races, stale prices, restarts, and expiry without
    value loss or creation; and
11. prove after every transition that NAVCoin supply, facility inventory,
    pfAsset supply, source-vault backing, and NAV allocations reconcile.

The resulting product is a trustlessly executable, reserve-backed primary
market. The operator chooses what reserve to post and on what future terms.
Once a user has a valid reservation, execution no longer depends on the
operator knowing the user or deciding whether to honor the trade.
