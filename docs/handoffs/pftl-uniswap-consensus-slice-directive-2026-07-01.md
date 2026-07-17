# CTO Directive — PFTL-Uniswap consensus wiring slice (2026-07-01)

Scope: independent review of commit `9abba229` ("wire pftl uniswap bridge into
asset consensus") on `main`. This directive records what the review confirmed,
the one consensus-level gap the commit's own slice note does not list, and the
ordered work that follows. Directives are binding and ordered.

## Verification result

Re-ran every claim in the handoff. All reproduce:

- `9abba229` is on `main` and pushed to `origin/main`. The only dirty file is
  `wallet-proxy/server.js`, correctly left uncommitted per the 2026-07-01
  directive at `docs/plans/pftl-uniswap-bridge-redeployment-spec.md:1111`.
- `cargo test -p postfiat-execution pftl_uniswap_consensus_subscribe_export_and_refund_moves_real_balances --lib` passes.
  Full `postfiat-types --lib` (64), `postfiat-execution --lib` (77), and
  `postfiat-node navcoin_bridge_status_reads_persisted_pftl_uniswap_ledgers`
  all pass. `cargo fmt --check` and `git diff --check` are clean.
- The test is real: distinct ML-DSA keys for issuer/operator/subscriber, real
  trustline balances asserted before and after, early refund rejected with
  `pftl_uniswap_refund_before_window`, then accepted after the refund window.
- The spec slice note does not falsely close the milestone. It leaves the
  checkbox open and lists destination-consume, return-import proofs, and the
  vacuous `latest_finalized_nav_epoch` as remaining.

Confirmed clean under review: hash preimages are injection-safe
(`validate_text_field` bans control characters; keys are fixed-order); all
arithmetic is checked; the supply-conservation invariant
(`pftl_spendable + outstanding_bridge_claims + pending_return_import +
ethereum_spendable + other == authorized_valid_supply`) is re-validated after
every transition; state-root commitment is sorted, canonical, and gated behind
`commit_complete_nav_state`; nonce/packet/burn replay protection is present; and
empty ledgers serialize identically to before, so existing state roots are
unperturbed.

## Directive 1 (BLOCKING) — Add an actor gate to all five operations

The slice's remaining-items list omits this and it must be added before any
further work builds on the slice. None of `route_init`, `primary_subscribe`,
`export_debit`, `refund_source`, or `return_import` check *who* may invoke them.
The only actor check is `is_authorized_source`, which merely requires the signer
to equal the operation's own self-declared `operator`/`subscriber`/`owner`
field. The vault-bridge operations in the same file
(`crates/execution/src/lib_parts/nft_escrow_asset_state_parts/part_02.rs`) all
call `ensure_vault_bridge_asset_policy` (part_02.rs:3662), requiring the operator
to be the native asset's issuer or reserve operator. The new appliers do not
even take `genesis`.

Consequence (unauthorized mint): anyone can `route_init` a route binding any
existing NAV asset to any settlement asset they control, then `primary_subscribe`
to mint real native NAV via `credit_issued_asset_balance` at a fully
caller-supplied price. `nav_price_settlement_atoms_per_nav_atom` is never checked
against reserve state, `pricing_reserve_packet_hash` is shape-checked but never
cross-checked, and the epoch equality check is vacuous. Any wallet that can hold
the NAV asset can mint it against junk collateral, bounded only by the asset's
`max_supply`. The passing test itself shows route init succeeding for an operator
who is not the issuer.

Required:

- Gate `route_init`, `primary_subscribe`, `refund_source`, and `return_import`
  on the native NAV asset's issuer or reserve operator, mirroring
  `ensure_vault_bridge_asset_policy`. Thread `genesis` through the appliers as
  the vault-bridge path does.
- Add this as an explicit unchecked item in
  `docs/plans/pftl-uniswap-bridge-redeployment-spec.md` remaining list so the
  checkbox cannot close without it. The existing freshness/proof items do not
  cover this; authorization and pricing are separate controls.
- Decide and record whether `route_init` is issuer-scoped or
  governance-scoped, and cap route creation per issuer to close the
  64-slot exhaustion DoS (`MAX_PFTL_UNISWAP_ROUTES`, first-come route IDs, no
  removal path).

## Directive 2 — Bind subscription price to finalized NAV state

`primary_subscribe` trusts the caller for both price and pricing epoch. Verify
`pricing_reserve_packet_hash` against the actual finalized reserve packet and
derive the minted amount from that reserve state, rather than accepting
`nav_price_settlement_atoms_per_nav_atom` from the caller. This is the sharp end
of the already-listed freshness item (`latest_finalized_nav_epoch` is set once at
route init and never updated). Do not close the freshness checkbox until price is
derived from state, not asserted by the caller.

## Directive 3 — Complete the listed transition semantics

Then the items the slice note already records: consensus
destination-consume / Ethereum-spendable transition (without it,
`return_import` is unreachable — nothing credits `ethereum_spendable_supply_atoms`
today); full return-import proof semantics (return import currently trusts
operator-supplied burn events); refund `current_height` and return-burn finality
heights sourced from chain state, not caller-supplied request files; and
`non_consumption_proof_hash` verified against the selected Gate 5 proof format,
not shape-checked as 96-hex. Note when destination-consume lands, an
unauthenticated `refund_source` racing an Ethereum-side consumption is a
cross-chain double-mint — the actor gate in Directive 1 and the proof semantics
here must both be in place first. `refund_source` also skips the `paused` check;
confirm that is intended and record it as a decision.

## Directive 4 — Wallet-proxy digest authority

Land the withheld `wallet-proxy/server.js` `route_family` edit only together with
the digest-authority fix, per the standing directive at spec line 1111. The proxy
must consume node-produced route/launch config digests over RPC, or byte-match
the node canonical form (omitting default-valued optional fields from the digest
preimage), with a test pinning proxy digest equality against a node-generated
vector. As written, the proxy hashes its own `JSON.stringify`, so adding
`route_family` flips the proxy digest from `23c4522e…` to `e3a33db4…` and breaks
the MVP4 `expected_gate3_route_config_digest` binding.

## Directive 5 — Two accounting decisions before public use

- Dust: `primary_subscribe` debits the full `settlement_value_atoms` but mints
  `floor(value / price)`; the remainder silently folds into
  `settlement_reserve_atoms`. The spec's own test matrix requires dust be
  refunded or explicitly recorded as a fee. Pick one and implement it.
- Receipts growth: every operation appends to `pftl_uniswap_receipts` forever,
  and `MAX_PFTL_UNISWAP_RECEIPTS` (131,072) is enforced in ledger validation, so
  reaching it bricks the route. Define a pruning or checkpoint story before
  public routing. Also note `native_spendable_balances_atoms` can diverge from
  real trustline balances after normal NAV transfers (solvency is still
  protected by the real trustline debit at export); record this as a known
  route-accounting limitation.

## Process note

The slice followed the standing directives: honest spec update, forbidden edit
withheld, `main` pushed. The one omission is that the authorization gap belongs
on the remaining-items list the slice itself authored. Directive 1 corrects that.
