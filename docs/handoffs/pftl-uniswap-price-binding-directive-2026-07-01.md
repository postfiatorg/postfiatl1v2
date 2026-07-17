# CTO Directive — PFTL-Uniswap NAV price binding slice (2026-07-01)

Scope: independent review of commit `8a66e24d` ("gate pftl uniswap route
authority") on `main`, and the next ordered slice from
`docs/handoffs/pftl-uniswap-consensus-slice-directive-2026-07-01.md`. That
document's Directive 1 is now closed; its Directive 2 is promoted to BLOCKING
here, with one item pulled forward from its Directive 5. Directives are binding
and ordered.

## Verification result (8a66e24d)

Re-ran every claim in the handoff. All reproduce:

- `8a66e24d` is on `main` and pushed to `origin/main`. The only dirty file is
  `wallet-proxy/server.js` (`route_family` edit), correctly withheld per the
  standing directive at `docs/plans/pftl-uniswap-bridge-redeployment-spec.md:1111`.
- `cargo test -p postfiat-execution --lib` (77), `cargo test -p postfiat-types
  --lib` (64), and `cargo test -p postfiat-node
  navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers --lib` all pass,
  re-run independently.
- The gate is real: `ensure_pftl_uniswap_native_asset_policy` requires the
  native asset to be NAV-registered, the operator to be the NAV issuer or
  reserve operator, and the issued-asset issuer to match the NAV registration;
  it is wired into `route_init`, `refund_source`, and `return_import`.
  `ensure_pftl_uniswap_route_capacity` caps routes per native NAV issuer under
  the global cap, and `LedgerState::validate_asset_state` rejects routes whose
  native asset is not NAV-registered. The consensus test proves unauthorized
  route init rejects and the per-issuer ninth route rejects.
- **Accepted deviation, recorded here:** `primary_subscribe` and `export_debit`
  remain user-signed rather than issuer-gated. This is correct — the subscriber
  must authorize their own settlement debit and the owner exports their own
  balance. The unauthorized-mint path is closed at route creation; the
  remaining mint exposure is pricing, which this directive closes.
- Minor, acceptable: `genesis` is threaded through the appliers but unused
  (`_genesis`); the reserve operator is read from the ledger NAV registry
  instead. Ledger state is the right authority for this. Keep the signature.

## Directive 1 (BLOCKING) — Bind primary-subscription price to finalized NAV state

Consequence today (mispriced mint): on a *legitimate* route,
`nav_price_settlement_atoms_per_nav_atom` is accepted from the caller, and
`pricing_nav_epoch` is checked only against `route.latest_finalized_nav_epoch`
— which is itself caller-supplied at route init
(`part_02.rs:2332`) and never updated. Any wallet may subscribe at an asserted
price of 1 settlement atom per NAV atom and mint `settlement_value_atoms`
native NAV against a dust debit, bounded only by the route supply cap. The
authority gate from `8a66e24d` does not mitigate this: the subscriber is not,
and must not be, issuer-gated.

The ledger already holds the pricing authority. `NavTrackedAsset`
(`crates/types/src/lib_parts/ledger_assets_parts/part_01.rs:310`) carries
`finalized_epoch`, `nav_per_unit`, `circulating_supply`,
`finalized_reserve_packet_hash`, `halted`, and `finalized_at_height`,
maintained by the `NavReserveSubmit` → challenge-window → finalization path.
Bind to it. Required, in `apply_pftl_uniswap_primary_subscribe`:

1. Load the route's `NavTrackedAsset`. Reject when `halted`
   (`pftl_uniswap_nav_asset_halted`).
2. Reject when `finalized_epoch == 0`: no finalized reserve packet means no
   primary issuance (`pftl_uniswap_nav_not_finalized`).
3. `operation.pricing_nav_epoch` must equal `nav_asset.finalized_epoch`.
   Delete the check against `route.latest_finalized_nav_epoch`; the route
   field becomes informational only (see item 7).
4. `operation.pricing_reserve_packet_hash` must equal
   `nav_asset.finalized_reserve_packet_hash` — the shape check alone no longer
   counts (`pftl_uniswap_pricing_packet_mismatch`).
5. Derive the settlement price from `nav_asset.nav_per_unit` and require
   `operation.nav_price_settlement_atoms_per_nav_atom` to equal the derived
   value (`pftl_uniswap_price_mismatch`). Keeping the wire field as an
   equality-checked commitment preserves the packet schema — no
   `protocol_version` bump. Document the exact unit conversion between
   `nav_per_unit` (denominated in the NAV asset's `valuation_unit`) and
   settlement-asset atoms, with a worked test vector; if a scaling constant is
   needed, it is a consensus constant, not an operation field.
6. Freshness (this closes the open "operator-attested freshness" checklist
   item for the subscription path): reject when
   `block_height - nav_asset.finalized_at_height` exceeds a new consensus
   constant `MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS`, and reject when
   `finalized_at_height == 0` (legacy packets carry no height and are not
   acceptable pricing input). Choose the bound from the NAV epoch cadence,
   record the chosen value and rationale in the spec.
7. In `apply_pftl_uniswap_route_init`, stop trusting
   `operation.latest_finalized_nav_epoch`: seed the route field from
   `nav_asset.finalized_epoch` and reject when the operation value disagrees
   (`pftl_uniswap_route_epoch_mismatch`). Zero-epoch NAV assets may still
   route-init (the asset may finalize later); they simply cannot subscribe
   until item 2 passes.
8. Dust (pulled forward from the prior directive's item 5 because it is the
   same arithmetic): `minted_nav_atoms = floor(value / price)` currently
   debits the full `settlement_value_atoms` and folds the remainder into
   `settlement_reserve_atoms` silently. Decision: debit exactly
   `minted_nav_atoms * price`; the remainder never leaves the subscriber.
   Update the receipt and the supply-conservation invariant accordingly and
   record the decision in the spec's dust item. If you instead take the
   explicit-fee option, the fee must appear as its own receipt field — pick
   one, do not leave the fold.

Tests, extending the existing consensus e2e in
`crates/execution/src/lib_parts/tests_parts/part_02.rs`:

- Wrong asserted price rejects; wrong `pricing_reserve_packet_hash` rejects;
  wrong `pricing_nav_epoch` rejects; halted NAV asset rejects; stale
  `finalized_at_height` rejects; `finalized_epoch == 0` rejects.
- A correct subscription mints exactly `floor(value / derived_price)` and
  debits exactly `minted * derived_price` — assert the subscriber's
  settlement trustline balance, not just route counters.
- The supply-conservation invariant re-validates after the mint.
- Route init with a mismatched `latest_finalized_nav_epoch` rejects.

## Directive 2 — Transition semantics (next slice, unchanged in substance)

The prior document's Directive 3, restated so ordering survives the handoff:
consensus destination-consume / Ethereum-spendable transition (until it lands,
nothing credits `ethereum_spendable_supply_atoms` and `return_import` is
unreachable); full return-import proof semantics; refund and return-burn
heights sourced from chain state rather than request files;
`non_consumption_proof_hash` verified against the selected Gate 5 proof
format. One addition from this review: `refund_source` does not call
`ensure_pftl_uniswap_route_live`, so refunds proceed on a paused route —
decide whether pause blocks refunds (recommended: pause blocks new
subscriptions and exports but not refunds; refunds shrink exposure) and record
the decision either way.

## Directive 3 — Wallet-proxy digest authority (unchanged)

The withheld `wallet-proxy/server.js` `route_family` edit still lands only
together with the digest-authority fix (consume node-produced digests over
RPC, or byte-match the node canonical form), with a test pinning proxy digest
equality against a node-generated vector.

## Directive 4 — Receipts growth (remaining half of the prior item 5)

`pftl_uniswap_receipts` still grows without bound toward
`MAX_PFTL_UNISWAP_RECEIPTS` (131,072), which bricks the route at the cap.
Define the pruning or checkpoint story before public routing. Not blocking
this slice.

## Process note

`8a66e24d` followed the standing directives: honest spec update, forbidden
edit withheld, `main` pushed, evidence claims that reproduce. Continue the
pattern: update the spec checklist with `Partial` vs complete honestly, commit
this directive doc with the slice, and push `main` when the slice lands.
