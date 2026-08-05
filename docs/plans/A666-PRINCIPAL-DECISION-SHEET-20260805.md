# A666 Principal Decision Sheet — Fire-Blocking Items

- **Date:** 2026-08-05
- **Scope:** only genuine unresolved fire-blocking decisions for the literal-A666
  live loop.
- **Route:** `pftl-a666-ethereum-wA666-usdc-v1`
- **Fire discipline:** each live leg remains HELD until its own packet hash is
  confirmed.

## 1. Per-leg packet-hash confirmations — OPEN AT FIRE TIME

The Principal must confirm the SHA-256 of each reviewed HELD packet immediately
before its corresponding live leg:

1. Leg 1 — `leg1-a666-bridge-in-held.json`
2. Leg 2 — `leg2-a666-governed-subscription-held.json`
3. Leg 3 — `leg3-a666-mainnet-export-held.json`
4. Leg 4 — `leg4-a666-return-burn-import-held.json`
5. Leg 5 — `leg5-a666-redeem-conservation-held.json`

Each confirmation is leg-specific. It neither confirms a later leg nor permits
a retry after any deviation.

## 2. Live-loop sequencing override — CONFIRM-ONLY

The emergency live-loop sequencing override is superseded by
**qualify-now-fire-later**, as decided this campaign. This entry needs only
confirmation of that already-decided control truth; it is not a per-leg fire
command.

## 3. Implementation authorization for five UNWIRED surfaces — OPEN

Principal approval is required to build, test, and regress the following
surfaces. This is implementation authorization only; it does not authorize a
live fire:

1. `literal_a666_bridge_in_surface`
2. `governed_a666_subscription_surface`
3. `a666_mainnet_export_surface`
4. `pftl_uniswap_return_import_invoker`
5. `a666_redeem_external_usdc_surface`

## 4. Mainnet endpoint binding — OPEN

The profile currently exposes `local_l4` endpoints and therefore fails
preflight fail-closed. Approval is required to bind the profile to Ethereum
mainnet chain_id 1 and Arbitrum chain_id 42161 before any live leg may be
considered.

## Already decided; do not re-litigate

- The route literal is `pftl-a666-ethereum-wA666-usdc-v1`; it is never
  relabeled as `a651`.
- The objective is a real full E2E loop with LIVE StakeHub funds, never
  Anvil or synthetic assets.
- The principal is 10,000,000 atoms / 10.000000 USDC. The 530.000000 USDC cap
  stays unchanged; the legacy 30-USDC projection is forbidden.
- StakeHub remains a product and its five services and funds are preserved.
