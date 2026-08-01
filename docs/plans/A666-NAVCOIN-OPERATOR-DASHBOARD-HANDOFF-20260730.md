# A666 NAVCoin operator dashboard handoff

**Date:** 2026-07-30

**Status:** next implementation step after the A666/pfUSDC Monday demo

**Dependency:** the controlled live primary issue/NAV/redeem run is PASS; see
[`../evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/README.md`](../evidence/a666-pfusdc-reserve-demo-20260730/live-run-01/README.md).

## Objective

Turn the proven command-line flow into a safe dashboard where:

- anyone can inspect A666 NAV, reserves, supply, route state, policy, capacity,
  proof freshness, and validator convergence;
- an ordinary wallet can request a quote and authorize pfUSDC -> A666 issue or
  A666 -> pfUSDC redemption;
- user spending keys remain client-side;
- market-maintenance authorities can refresh proof/NAV and advance a route
  epoch through separately identified operator actions; and
- every action produces the same fail-closed evidence used by the live run.

This is not a generic exchange UI and not a Uniswap replacement. The first
dashboard supports exactly the deployed
`pftl-a666-ethereum-wA666-usdc-v1` route.

## Read-only first screen

The first deliverable should require no wallet connection and show:

| Widget | Authoritative source |
|---|---|
| A666 NAV and epoch | active A666 NAV profile from validator status |
| Verified assets and proof | latest finalized NAV packet and proof metadata |
| A666 valid supply | `authorized_valid_supply_atoms` |
| PFTL and wrapped exposure | route supply-status fields |
| pfUSDC base reserve | `settlement_reserve_atoms` |
| Non-NAV spread | `non_nav_spread_atoms` |
| Issue/redeem multipliers | active route policy |
| Executable issue/redeem capacity | computed route availability fields |
| Freshness | current height vs NAV/policy expiry limits |
| Safety state | pause flag, invariant, active reservations, entitlements |
| Fleet convergence | six-validator height/root/tip/route comparison |

The UI must display atom precision and friendly units together. A green
“healthy” badge is allowed only when all six validators agree and every
freshness/invariant gate passes.

## Ordinary wallet actions

### Deposit USDC for pfUSDC

The wallet shows exact mainnet USDC, vault, route binding, amount, recipient,
estimated Ethereum gas, and two-session approval/deposit state. It must not
hide that Ethereum deposit finality and PFTL proof/claim are asynchronous.

### Issue A666

The wallet obtains current route/NAV state and displays:

- A666 to create;
- base pfUSDC entering NAV reserve;
- issue spread;
- total pfUSDC paid;
- NAV epoch/packet and policy hash;
- reservation expiry; and
- resulting A666/pfUSDC balances.

The client signs reserve, subscribe, and entitlement release. The default
PFTL-retained flow must never submit an Ethereum export operation.

### Redeem A666

The wallet displays:

- A666 retired;
- base reserve decrease;
- pfUSDC returned;
- redemption spread;
- same-run reserve limit when applicable; and
- resulting balances.

It signs the existing transparent primary-redeem operation. Private actions
remain disabled until the production-hardening gates authorize them.

## Operator maintenance actions

These controls are visibly separated from ordinary wallet actions:

1. Capture and prove the current reserve packet through the open,
   provider-neutral reserve-proof kit.
2. Build and submit `nav_reserve_submit`.
3. Build and submit `nav_epoch_finalize`.
4. Build and submit `pftl_uniswap_route_epoch_advance`.
5. Pause the route through the existing governed mechanism.

The dashboard may prepare packets and show diffs, but it must not silently
combine user spending authority with NAV/route governance. Each operation
requires an explicit role-appropriate signature and a final pre-submit review.

## Transaction state machine

Every action is represented as a resumable state machine:

```text
draft -> quoted -> user reviewed -> signed -> submitted
      -> finalized on six validators -> independently verified -> complete
```

Failures must retain immutable evidence and offer either a safe retry with a
new nonce/packet or an explicit release/cancel action. The UI must never mark a
trade complete based only on RPC submission.

## API and verification boundary

The dashboard backend may aggregate validator reads and prepare unsigned
operations. It is not authoritative. The wallet independently verifies:

- chain, asset, route, account, amount, nonce, and expiry;
- route epoch, policy hash, NAV epoch, and reserve packet;
- price/spread arithmetic;
- capacity and reserve bounds;
- expected balance/supply deltas; and
- six-validator finality.

Reuse the checks in `scripts/a666-pfusdc-reserve-demo.py` as the initial
reference verifier. Extract shared typed logic rather than maintaining a
second arithmetic implementation in the UI.

## P0 implementation sequence

- [ ] Build a read-only route/NAV/fleet API and dashboard.
- [ ] Add deterministic issue and redemption quote rendering with test
  vectors from the 2026-07-30 live run.
- [ ] Add wallet-side packet verification and signing for reserve, subscribe,
  release, and redeem.
- [ ] Add finality tracking and post-trade delta verification.
- [ ] Add the two-session USDC approval/deposit and pfUSDC claim workflow.
- [ ] Add separately permissioned proof/NAV/route maintenance panels.
- [ ] Add immutable evidence export matching
  `postfiat.a666.pfusdc_reserve_demo_live_run.v1`.
- [ ] Run a zero-value/local rehearsal, then a small controlled live-value
  acceptance run.

## Explicitly deferred

- private issue/redeem;
- generic NRRS multi-facility operations;
- pfETH, pfBTC, pfXRP, or pfStakedETH;
- bridge-out and Uniswap trading;
- automatic operator key custody;
- production-GA or large-capacity claims.

The dashboard should make the proven A666/pfUSDC primary path usable first.
Expanding scope before that path is reliable would reintroduce the exact
delivery risk the Monday profile was created to avoid.
