# NAVCOIN On Current Infrastructure

Status: native NAV smoke  
Script: `scripts/navcoin-current-infra-smoke`

> This page documents the settlement rail. The verification layer that
> now governs whether reserve packets may finalize — proof profiles,
> consensus-verified transparent reserves, the attestor registry,
> multi-fetch quorums, and valuation policies — is documented in
> [NAVCOIN Proof of Reserves](navcoin-proof-of-reserves.md).

The current NAVCOIN prototype now uses native NAV lifecycle operations on top
of the existing issued-asset and offer-book stack:

- issued assets for the NAV unit;
- trustlines for holders and authorized participants;
- native NAV asset registration;
- native reserve-packet submission and epoch finalization;
- native mint-at-NAV and redeem-at-NAV operations;
- the offer book for NAV/PFT secondary liquidity;
- evidence packets and reports for reserve/NAV provenance.

This proves the operating shape without adding a general smart-contract VM.

## What The Smoke Does

`scripts/navcoin-current-infra-smoke` starts a controlled validator devnet and
runs the NAVCOIN path end to end:

1. Create a `NAV` issued asset.
2. Write a reserve packet that binds supply, NAV per unit, verified net assets,
   source class, attestor group, and proof profile.
3. Register the issued asset as a native NAV-tracked asset.
4. Open trustlines for an authorized participant and a liquidity provider.
5. Submit and finalize the reserve packet as native chain state.
6. Mint NAV to the authorized participant at the finalized NAV epoch.
7. Put PFT liquidity on the offer book.
8. Swap NAV into PFT through the current offer book.
9. Cancel residual liquidity.
10. Redeem part of the AP's NAV at the finalized NAV epoch.
11. Verify every validator has the same state root, NAV state, redemption
    record, balances, offer state, and indexed asset view.

Run:

```bash
scripts/navcoin-current-infra-smoke
```

The report is written under:

```text
reports/navcoin-current-infra/<run-id>/navcoin-current-infra-report.json
```

## What This Proves

The current chain can already express the operational skeleton:

| NAVCOIN requirement | Current primitive |
| --- | --- |
| NAV token unit | Issued asset definition. |
| AP custody of units | Trustline plus issued balance. |
| Controlled mint | Native `nav_mint_at_nav`. |
| Redemption-style contraction | Native `nav_redeem_at_nav`. |
| Reserve lifecycle | Native reserve submit and epoch finalization. |
| NAV/PFT liquidity | Native offer book. |
| Public evidence | Reserve packet hash plus source and attestor roots. |
| Validator agreement | Controlled validator state-root convergence. |

## What Is Now Enforced In Consensus

The chain now carries first-class NAV state for the parts that matter to the
instrument:

- a registered NAV asset linked to an issued asset;
- submitted reserve packets with source and attestor roots;
- finalized NAV epochs;
- minting capped by finalized reserve supply;
- redemption records with deterministic claims;
- halt behavior for challenged or unsafe assets.

The remaining production work is attestor hardening: replacing the local
placeholder source with a real Nitro or equivalent proof profile, adding the
full challenge-window policy, and wiring off-chain redemption settlement
receipts to the pending redemption records.

## DEX Requirement

NAVCOIN does not require a DEX for correctness. Correctness comes from
finalized NAV, proof freshness, AP mint/redeem, and halt behavior.

The offer book is still valuable. It lets users trade NAV against PFT without
waiting for an AP redemption window, and it gives the market a secondary price
that APs can arbitrage back toward finalized NAV.

So the design split is:

| Layer | Role |
| --- | --- |
| AP mint/redeem | Primary correctness and NAV convergence. |
| Offer book | Secondary liquidity and user convenience. |
| Native NAV module | Consensus-enforced reserve/NAV lifecycle. |

PostFiat already has the offer-book path needed for the first NAV/PFT smoke.
The next protocol step is reserve-attestation hardening, not arbitrary smart
contracts.
