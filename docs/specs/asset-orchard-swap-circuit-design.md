# AssetOrchardSwapCircuit Design

Date: 2026-06-19

Status: superseded historical draft.

This file is retained only to preserve the original design-document path. Do
not use it as current L1 state.

Current sources of truth:

- `docs/specs/asset-orchard-swap-circuit-design-v2.md`
- `docs/specs/private-otc-shielded-scope.md`
- `docs/status/shielded-layer-map.md`
- `docs/runbooks/private-nav-otc-shielded-swap-wan-devnet.md`

Current state summary:

- Asset-Orchard private swaps are implemented as the current v1
  production-candidate primitive for private a651/pfUSDC movement inside PFTL.
- The consensus-facing `ShieldedSwapV1` action can carry an
  `AssetOrchardSwapAction`.
- Internal swaps hide raw asset ids, values, owners, recipients, and price from
  public chain state.
- Boundary ingress is public.
- Current Asset-Orchard egress is disclosed egress, not private egress.
- Public-network/privacy claims remain gated on external cryptographic review,
  release-candidate tests, and broader wallet/service hardening.
