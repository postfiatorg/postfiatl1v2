# a666 Mainnet Trustless Mint/Redeem Spec

**Date:** 2026-07-25
**Priority:** P0
**Status:** approved direction; implementation not started as a unified program
**Supersedes in part:** the Arbitrum vault domain in
`PFUSDC-TIER4-IMPLEMENTATION-PLAN-20260717.md` (see Section 2)
**Companion:** `PFUSDC-TIER4-CLOCK-CRITICAL-HANDOFF-20260718.md` (gate model retained)

## 1. Mission

A user ("Bob") who holds USDC and does not know or trust the issuer ("Alex")
can:

1. acquire a666 at a flat, size-independent price band around a
   cryptographically verified NAV — mint at NAV x 1.005, redeem at
   NAV x 0.9995, up to posted caps — with no Uniswap slippage;
2. do so trustlessly: no observer, no signing committee, no counterparty
   relationship, no ability for any party to steal or fade the quote;
3. receive the asset as a mainnet Ethereum ERC-20 (`wA666`) where the
   Uniswap v4 pool lives, without needing to operate PFTL tooling;
4. complete ingress in minutes (Ethereum finality), not days;
5. always retain a unilateral exit: redeem at the floor into pfUSDC and
   withdraw to mainnet USDC through a proof-verified rail even if the
   issuer disappears and no market maker quotes.

Holders of BTC, XRP, Lightning, and (optionally) Tron-native assets can enter
through atomic HTLC swaps against issuer inventory with trustless execution.

## 2. Recorded user directives (2026-07-25)

These are explicit user-authorized parameter changes under the immutable
operator directive of the clock-critical handoff:

1. **Arbitrum is deprecated as the pfUSDC Tier-4 vault domain.** Rationale:
   (a) trustless ingress on Arbitrum requires the ~6.4-day Nitro assertion
   confirmation under finalized Ethereum state, which is commercially
   unacceptable; (b) the vault domain would mismatch the Ethereum-mainnet
   venue where a666/wA666 and the Uniswap v4 pool settle. Deprecation means:
   never register or activate an Arbitrum route profile on the fresh chain.
   Nothing needs teardown; the Arbitrum route was never deployed or activated.
2. **Ethereum mainnet is the settlement domain.** The Ethereum L1 fast lane
   (`pfusdc-eth-ingress`: Helios beacon finality + vault storage-slot proofs)
   replaces the Arbitrum route at the same no-trust standard with ~15-minute
   ingress finality.
3. **No Tier-4 wrapped asset is attempted for BTC or XRPL.** Their script
   systems cannot verify PFTL finality proofs on egress; any wrapped exit
   would require a signer committee, which Tier 4 bans. BTC/XRP enter via
   HTLC atomic swaps out of inventory only.
4. **The Uniswap pool is a price anchor, not the execution venue for size.**
   Primary flow goes through NAV-band mint/redeem; arbitrage against the band
   pins the pool price.

## 3. Target architecture

```text
                 PRIMARY RAIL (self-backing, unbounded by inventory)
  USDC (mainnet) --> L1 vault --> SP1 ingress proof (Helios + storage slots)
       --> PFTL credits pfUSDC --> primary fill vs posted band (mint @ 1.005)
       --> exit leaf in finalized PFTL block --> SP1 finality proof
       --> PFTLFinalityVerifierV1 on mainnet --> batch exits Merkle root
       --> per-user Merkle claim --> wA666 minted to user on mainnet
       --> user holds/LPs/trades on Uniswap v4 at will

                 SECONDARY RAILS (inventory-bounded per epoch)
  BTC / XRP / LN / (Tron) --HTLC atomic swap--> a666/wA666 from issuer inventory
       --> issuer sweeps proceeds to attested reserve addresses
       --> next reserve packet proves larger portfolio (with haircuts)
       --> NAV epoch finalizes --> cap grows --> inventory re-minted at NAV
```

Trust classification:

| Asset | Role | Mechanism | Access model |
|---|---|---|---|
| USDC (mainnet) | reserve numeraire + redemption floor | Tier-4 pfUSDC, L1 fast lane | unilateral, proof-gated |
| USDT (mainnet) | secondary numeraire (optional) | same fast-lane stack, new route profile | unilateral, proof-gated |
| BTC, XRP, LN | payment-in currencies | HTLC vs issuer market | bilateral, atomic execution |
| Tron-native USDT | deferred | HTLC first; light client only if demand proves | see WS9 |

## 4. Invariants (all fail-closed)

1. Conservation identity `V = S + D + B - R` holds across every round trip.
2. No observer, threshold-signer, or mock-verifier fallback on any active
   Tier-4 route. Route activation is no-downgrade.
3. `nav_mint_at_nav` / `nav_redeem_at_nav` refuse when the active NAV epoch
   exceeds `max_epoch_gap_blocks` or unsettled redemptions exceed
   `settle_deadline_blocks`. "Mint at NAV" must never degrade into "mint at
   asserted NAV".
4. Supply growth only against proven backing (cap-growth SP1 discipline).
5. Never verify a SNARK on-chain for a single user action when a batch root
   can carry it. Per-user cost is a Merkle path, not a Groth16 verify.
6. Proof code contains no user, wallet, or mutable deployment address.
   Route profiles pin chain IDs, genesis hashes, epochs, code hashes, vkeys,
   bounds, and activation heights.
7. Fleet changes only through the signed-snapshot rollout discipline. No
   weakening of archived-batch validation to force a rollout through.

## 5. Workstreams

Ordered by dependency; WS1-WS3 gate everything else.

### WS1 — Release the a666 fleet rollout hold (P0, blocking all supply growth)

Current state: a666-capable candidate `7b16ed48...c7c` staged on all six
validators, preflight green, but the signed snapshot transition is fail-closed
at `backup.verified: false`; the audit gate rejects a historical governance
batch ID after the constructor-compatible verifier fix.

Work:
- Root-cause the historical governance batch ID rejection. Determine whether
  the batch record is malformed-but-original (verifier must accept it under a
  narrow, documented compatibility rule, mirroring the extension-kind fix) or
  genuinely inconsistent (repair the snapshot lineage, never the gate).
- Re-run the signed snapshot transition to `backup.verified: true` on all six
  hosts; then and only then `apply-next`.
- Evidence: per-host verification transcripts, batch-ID audit trail, zero
  deletion actions, before/after fleet binary hashes.

Acceptance: six validators running the a666 candidate; a666
subscribe/export/refund conservation regression green on the live fleet.

### WS2 — pfUSDC backing headroom (P0)

Current state: funding fails closed with
`global supply 20 exceeds finalized NAV circulating supply 10`; identified
small Orchard notes spent.

Work:
- Author and authorize a backing checkpoint that raises the finalized NAV
  circulating supply for pfUSDC against proven backing, or identify a
  transferable balance. No supply may be created outside the cap discipline.
- Document the checkpoint procedure as the standing runbook for future
  headroom events (a $100M ticket must not require ad hoc surgery).

Acceptance: a test-scale pfUSDC issuance completes without cap violation;
conservation identity re-verified.

### WS3 — Bind the NAV proof lane in consensus (P0)

Current state: epoch 1 verifies GREEN externally (standalone verifier,
`proof_valid: true`, `onchain_binding_valid: true`) but finalized under
`multi-fetch-quorum`; SP1 verification is audit-only, not consensus-enforced.

Work:
- Register the SP1-verified NAV proof profile (program vkey
  `0x00efa2460d8a2460afcf8c452a93a6cffeda11a629dc0b488f5d0711ba9e8d4b` family,
  re-frozen as needed) via `nav_profile_register` and bind a666 to it, using
  the same bootstrap pattern the Tier-4 fresh chain used in blocks 2-4.
- Set and record the policy knobs: epoch cadence, `max_snapshot_age_blocks`,
  `challenge_window_blocks`, `max_epoch_gap_blocks`, `settle_deadline_blocks`,
  challenge bond floor, tolerance band. These knobs trade proof freshness
  against SP1 proving cost; they are user-approved parameters (Section 8).
- Keep the external standalone verifier and explorer publication
  (`nav-proof-explorer`, `postfiat-nav-proof-verify`) as the public audit lane.

Acceptance: an epoch finalizes under the SP1 profile on the live fleet, and a
mint attempt against a stale epoch fails closed.

### WS4 — Ethereum L1 fast lane to Tier-4 activation (P0)

Builds on `eth-l1-fast-lane-p0-20260723` /
`a666-eth-fast-lane-combined-20260724` (P1 ingress program, durable jobs,
resumable driver, gate-4 bounded contract checks, fail-closed core acceptance,
exact egress witness audit).

Work:
- Freeze the `pfusdc-eth-ingress` ELF/vkey and the egress program/vkey;
  record hashes in the deployment manifest.
- Deploy on Ethereum mainnet: ERC-20 vault (USDC), `PFTLFinalityVerifierV1`
  wired to the canonical Succinct SP1 verifier gateway, batch-exits root
  consumption, replay protection.
- Register and activate the Ethereum L1 route profile on PFTL (no Arbitrum
  profile ever registered, per Section 2). Bind pfUSDC to it before route
  activation height, per the plan-correction lesson from 2026-07-18.
- Egress batching: exits committed under one Merkle root per finality proof;
  per-user claims are Merkle paths.
- Retain the four-gate acceptance model from the clock-critical handoff,
  retargeted to the L1 route: both proof directions live on an activated
  no-fallback route.

Acceptance: one real USDC round trip (deposit -> pfUSDC -> burn -> USDC to a
different recipient) authorized end-to-end by proofs only, with ingress
wall-clock bounded by Ethereum finality plus proving time, measured and
recorded.

### WS5 — wA666 mainnet settlement stack (P1, after WS4 route freeze)

Current state: full controlled stack live on Sepolia (wA666, controlled
verifier, replay registry, bridge controller, settlement adapter, v4 router,
initialized pool, zero supply); one quarantined invalid deployment.

Work:
- Port the Sepolia stack to mainnet against real USDC and official Uniswap v4
  contracts, with the verifier consuming WS4 finality proofs (replace the
  controlled verifier with the production `PFTLFinalityVerifierV1` lineage).
- Direct-settlement claim path: a primary fill on PFTL settles to the user as
  a wA666 mint on mainnet via the batch exit root, so the user never operates
  PFTL tooling.
- Pool bootstrap: seed liquidity sized as a price anchor only (band
  arbitrage does the pinning); document that pool depth is not backing.

Acceptance: Bob-profile dry run on mainnet fork, then controlled-size live
run: USDC in, wA666 out, round trip cost = band + gas only, no slippage
dependence on size.

### WS6 — Primary fill bands and caps (P1)

Current state: atomic a666 primary fills merged (`8ae940f`) with execution
tests.

Work:
- Post the launch band: mint <= 2,000,000 a666 at NAV x 1.005; redeem
  <= 2,000,000 at NAV x 0.9995 (user-stated launch parameters; adjustable by
  issuer within governance caps).
- Verify atomicity properties on the live fleet: no fade after commit, no
  partial fills outside posted terms, band quotes bound to the active NAV
  epoch ID.

### WS7 — HTLC secondary lanes to production (P1, parallel)

Current state: EVM USDC lane complete with threat model and evidence verifier
(`28c53a2`); XRPL lane (`ff53149`); Lightning lane hardened; Bitcoin proven on
regtest, signet lane built (`e017df2`, `7e64f46`).

Work:
- Promote lanes in order: EVM -> XRPL -> Bitcoin signet -> Bitcoin mainnet;
  Lightning as capacity allows.
- Productionize the known HTLC caveats: confirmation depth per chain, timeout
  ladder, refund-liveness tooling (user must be able to reclaim after
  timeout without operator help), free-option pricing guidance folded into
  spread, quote binding to NAV epoch + FX snapshot.
- Publish the standing threat model per lane.

Acceptance: one live counterparty-adversarial test per lane (attempted fade,
attempted partial claim, timeout refund) with evidence bundles.

### WS8 — Reserve recycling policy (P2)

Work:
- Extend the valuation policy to admit HTLC proceeds as reserves at attested
  addresses with per-asset haircuts (BTC address observation via
  `multi-fetch-quorum`; USDT/USDC sweeps near-par).
- Define the sweep runbook: HTLC proceeds -> attested reserve address ->
  next reserve packet -> cap growth -> inventory re-mint at NAV.
- Document market capacity math: BTC/XRP throughput = inventory x epoch
  cadence; inventory is working capital, not a ceiling.

### WS9 — pfUSDT and the Tron decision gate (P2/P3)

Work:
- pfUSDT on Ethereum mainnet: new route profile over the existing fast-lane
  stack (same Helios guest lineage, USDT vault address, USDT-specific
  haircut). Near-config-level; audit pass required.
- Tron: HTLC lane first (TVM hashlocks) out of inventory. A Tron Tier-4
  route (from-scratch DPoS light client in SP1: 19-of-27 SR signature
  verification + committee rotation; TVM-side Groth16 verification via its zk
  precompiles) is feasible but Helios-sized. Build only on demonstrated
  Tron-native primary demand; requires explicit user authorization.

### WS10 — Issuer-freeze and reserve-concentration policy (P2, doc + policy)

- Record that Circle/Tether can administratively freeze vault addresses on
  any chain; no proof system mitigates this. Set reserve-policy responses:
  per-issuer concentration limits, haircuts, and a halt/deadman path
  (`nav_halt`) if a reserve asset is frozen.

## 6. Non-goals

- No Arbitrum route registration or activation, ever, on the fresh chain.
- No wrapped BTC/XRP on PFTL (no committee-custodied assets under a Tier-4
  label).
- No per-user on-chain SNARK verifications.
- No pool-depth-as-backing claims in any public material.
- No weakening of fail-closed gates (rollout audit, cap growth, NAV deadman)
  to hit a schedule.

## 7. Dependency order

```text
WS1 (rollout hold) ──> WS3 (NAV lane bound) ──> WS6 (bands live)
        │                      │
        └──> WS2 (headroom) ───┤
                               v
        WS4 (L1 Tier-4) ──> WS5 (wA666 mainnet) ──> Bob's flow live
                               │
        WS7 (HTLC lanes) ──────┴──> WS8 (recycling) ──> WS9/WS10 (extensions)
```

## 8. Parameters requiring explicit user sign-off before activation

1. NAV epoch cadence and all WS3 freshness/deadman knobs.
2. Launch band and caps (current: 1.005 / 0.9995, 2M each side).
3. Mainnet funding authorization for deployment gas, seed liquidity, and
   redemption buffer (Sepolia launch precedent: pool seed + buffer as
   separate allocations).
4. Prover operations: who runs SP1 proving, budget, and cadence.
5. Audit scope for mainnet contracts before real-value activation.
6. Any Tron light-client commitment (WS9).

## 9. Definition of done (program level)

Bob's flow executes live on Ethereum mainnet: USDC in, proof-verified ingress
in minutes, primary fill at the posted band against the active SP1-verified
NAV epoch, wA666 delivered to his mainnet address, Uniswap price pinned inside
the band by arbitrage, and a unilateral proof-verified redemption path back to
USDC at the floor — with every invariant in Section 4 enforced fail-closed and
evidence bundles recorded for each workstream acceptance.
