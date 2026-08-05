# A666 Principal Decision Sheet — Fire-Blocking Items

- **Date:** 2026-08-05
- **Scope:** only remaining fire-blocking confirmations and external operational
  blockers for the literal-A666 live loop.
- **Route:** `pftl-a666-ethereum-wA666-usdc-v1`
- **Fire discipline:** every leg remains HELD until its own packet SHA-256 is
  confirmed at fire time.

## 1. Per-leg packet-hash confirmations — OPEN AT FIRE TIME

The Principal must confirm the SHA-256 of each reviewed HELD packet immediately
before its corresponding live leg:

1. Leg 1 — `leg1-a666-bridge-in-held.json`
2. Leg 2 — `leg2-a666-governed-subscription-held.json`
3. Leg 3 — `leg3-a666-mainnet-export-held.json`
4. Leg 4 — `leg4-a666-return-burn-import-held.json`
5. Leg 5 — `leg5-a666-redeem-conservation-held.json`

Each confirmation is leg-specific. It neither authorizes a later leg nor
permits a retry after any deviation.

## 2. External operational blockers — OPEN

These require deployment or contract work outside this repository before a
full loop can fire:

1. **Leg 1B: Arbitrum-to-pfUSDC ingress.** No audited
   Arbitrum-native-USDC to pfUSDC ingress surface exists:
   `literal_a666_bridge_in_surface`.
2. **Leg 5B: literal-route controller binding.** The typed redeem
   `quoteRedeem` / `redeem` specification remains unbound to a
   literal-A666-route controller address:
   `literal_route_controller_binding`.
3. **Leg 5C: pfUSDC-to-external-USDC exit.** No audited invoker exists:
   `a666_redeem_external_usdc_surface`.

## Already decided; do not re-litigate

- Implementation authorization for the five adapter surfaces is already
  recorded; Leg 2 through Leg 4 are now WIRED and Leg 1 and Leg 5 retain only
  their documented partial or absent substeps.
- The emergency live-loop sequencing override is superseded by
  qualify-now-fire-later; that decision is recorded and creates no fire
  authorization.
- The route literal is `pftl-a666-ethereum-wA666-usdc-v1`; it is never
  relabeled as `a651`.
- The objective is a real full E2E loop with LIVE StakeHub funds, never Anvil
  or synthetic assets.
- The principal is 10,000,000 atoms / 10.000000 USDC. The 530.000000 USDC cap
  stays unchanged; the legacy 30-USDC projection is forbidden.
- StakeHub remains a product and its five services and funds are preserved.
