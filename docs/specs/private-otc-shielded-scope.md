# Private NAV OTC Swaps — Shielded Phase Scope (Tier 1, non-Railgun)

> Next phase after the transparent MVP. Current state: Asset-Orchard private swaps are implemented for controlled devnet use; true private egress is still future work.

## Goal

**Private NAVCoin selection + rotation.** Once a user holds **shielded pfUSDC**, they can swap in/out of NAVCoins (a651, a652, …) through **shielded swaps** — counterparty, amounts, price, and *which NAVCoin they chose* all private. NAVCoin **reserves stay auditable** (transparent backing — the pfUSDC principle: privacy at the transfer layer, not the backing layer). Arbitrum bridge edges are **visible** this tier (Railgun edge privacy is a future option).

The product target is a **private cash hub**. Pay the bridge edge once to enter shielded pfUSDC; rotate across NAVCoins privately and cheaply inside PFTL; pay the edge again to exit. In the current tier, ingress and egress edges are visible/disclosed rather than private.

## What already exists — DO NOT rebuild

The shielded layer is implemented across `privacy_orchard`, `privacy`, `proofs`, and `node/src/privacy.rs`. Framework: `ShieldedNote`, `ShieldedState`, `ShieldedAction`, real Orchard actions, and Asset-Orchard asset-typed actions.

Current action classes include debug shield mint/spend/migrate, value-only Orchard action/deposit/withdraw, Asset-Orchard ingress, Asset-Orchard swap, and disclosed Asset-Orchard egress.

## What exists now — the Asset-Orchard swap

`ShieldedAction::ShieldedSwapV1` exists. For the current NAV OTC path it carries an `AssetOrchardSwapAction`: a fixed two-input/two-output asset-typed Halo2 action that consumes two private Asset-Orchard notes, proves authorization and asset/value conservation, and produces two replacement notes without revealing participants, raw asset ids, amounts, or price.

The current public boundary is:

- `AssetOrchardIngressV1`: public asset/value enters the asset-typed pool.
- `ShieldedSwapV1`: internal movement is private inside `asset-orchard-v1`.
- `AssetOrchardEgressV1`: current exit is disclosed, not private.

## Current build phases

1. **Map the shielded layer** → complete enough for current work in `docs/status/shielded-layer-map.md`.
2. **Build Asset-Orchard swap** → implemented and tested with good path, forged non-conservation rejection, spendability, key metadata pinning, and live WAN-devnet evidence.
3. **Integrate pfUSDC + a651** → implemented for public ingress and private internal swap. Reserves stay auditable at bridge/NAV boundaries.
4. **OTC matching** → still product/workflow work. The primitive is bilateral and fixed-shape; a user-friendly matching/quote flow is not the same as the circuit.
5. **Private egress** → not implemented. Current disclosed egress lets value leave functionally but reveals the exited note facts.

## Honest scope limits (disclose in any writeup)

- **Arbitrum edges visible** (no Railgun this tier). The bridge-in/out gross flows are public on Arbitrum.
- **Aggregate NAVCoin reserves are auditable** → a large flow into a *small* NAVCoin is visible in aggregate (WHO is hidden; the flow magnitude into that NAVCoin is not). Privacy is strongest for flows small relative to the target NAVCoin, or spread across several.
- This is the **shielded middle only**. Current disclosed egress reveals the asset/value at exit. Edge privacy, private egress, batching/mixing/delay, or Railgun-style source-chain privacy are future add-ons.

## Locked design decisions

- **Asset-typed notes + conservation proof (gate, decided 2026-06-19): design (A) with proof approach (a) — a full zk-SNARK.** Single shielded pool + **private asset commitments** + a **real zk-SNARK swap circuit** (Halo2 / Orchard-native, reusing the orchard crate's proving/verifying machinery) that proves value+asset conservation with asset/value/parties hidden. **Rejected (b) algebraic/Pedersen commitments** because (b) is the Monero/RingCT lineage (delisted/blacklisted, no clean compliance path) and non-native to the Orchard layer, whereas (a) is the Zcash/Orchard lineage (Coinbase-listed, supports viewing-key/selective-disclosure compliance) and extends the SNARK already in the codebase. Rejected separate-pools-per-asset because `pool_id` leaks the NAVCoin choice. Keep `ShieldDisclose` viewing-key/disclosed-compliance intact.
- **Matching:** bilateral atomic `ShieldedSwap` (OTC), not AMM.
- **NAV-band in-circuit:** deferred — ship the conservation swap first.
- **Privacy scope:** Tier 1 (middle private, edges visible).

## Hard constraints

- **Real a651 + pfUSDC on the WAN devnet** (already there) — do not rebuild or re-bootstrap.
- **Reserves stay auditable** — shielded transfers, never shielded backing.
- **Correctness over speed** — prove each phase with live assertions before the next; never paper over a failure.
- **Hygiene:** SSH key not sshpass; no keys/passwords in tree or fixtures; pre-push secret scan; never mask a failing test.
