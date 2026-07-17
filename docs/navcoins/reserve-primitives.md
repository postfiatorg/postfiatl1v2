# NAVCoin Proof-of-Reserve Primitives

NAVCoin reserves are not a marketing label. They are ledger objects and
transaction validity rules that decide whether a reserve packet can become the
active NAV epoch for an issued asset.

## Native lifecycle

The native settlement rail proves the NAVCoin operating shape without a general
smart-contract VM:

| Operation | Role |
|---|---|
| `nav_profile_register` | Registers the proof/freshness/challenge parameters that define what counts as proof. |
| `nav_asset_register` | Links an issued asset to a reserve operator, valuation unit, redemption account, and proof profile. |
| `nav_reserve_submit` | Publishes a reserve packet for a specific asset and epoch. |
| `nav_reserve_attest` | Records an observer verdict for multi-fetch profiles. |
| `nav_reserve_challenge` | Opens a bonded challenge against an unsafe or superseded packet. |
| `nav_epoch_finalize` | Promotes a packet to the active NAV epoch if profile rules pass. |
| `nav_mint_at_nav` | Issues units against the finalized NAV state and supply cap. |
| `nav_redeem_at_nav` | Burns or locks units into a deterministic redemption claim. |
| `nav_redeem_settle` | Records settlement evidence for a pending redemption. |
| `nav_halt` | Fails unsafe assets closed. |

The current native smoke covers registration, reserve submission, finalization,
minting, offer-book trading, redemption, and validator convergence. The detailed
walkthrough is [NAVCOIN Current Infrastructure](../business/navcoin-current-infra.md).

## Proof profiles

`NavProofProfile` is a content-addressed ledger object. Its profile id is a
domain-tagged SHA3-384 hash over the profile parameters, so two assets using the
same proof rules resolve to the same identity.

| Field | Meaning |
|---|---|
| `verifier_kind` | The verification lane: `ledger-transparent`, `multi-fetch-quorum`, or `placeholder`. |
| `source_class` | What observers check, such as a native ledger account set, Hyperliquid account, or vault-bridge source domain. |
| `max_snapshot_age_blocks` | How stale a packet may be before finalization fails. |
| `challenge_window_blocks` | Minimum delay before a challenged or challengeable packet can finalize. |
| `max_epoch_gap_blocks` | Deadman switch for mint/redeem once the active packet is too old. |
| `settle_deadline_blocks` | Deadline after which unsettled redemptions block new minting. |
| `min_challenge_bond` | Bond floor for challengers. |
| `min_attestations` | Required passing observer verdicts for multi-fetch profiles. |
| `tolerance_bp` | Relative tolerance band for live external observations. |
| `valuation_policy_hash` | Hash of the pricing, haircut, source, and invariant policy used by observers. |

The detailed proof-profile design is [NAVCOIN Proof of Reserves](../business/navcoin-proof-of-reserves.md).

## Verifier kinds

| Verifier | Use case | Finalization rule |
|---|---|---|
| `ledger-transparent` | Reserves are visible in native ledger accounts. | Consensus recomputes the reserve sum and rejects a mismatched packet. |
| `multi-fetch-quorum` | Reserves live at an external public source that validators cannot fetch during consensus. | Registered observers fetch, normalize, hash, and attest; finalization requires the configured passing quorum and zero failing verdicts. |
| `placeholder` | Tests and bootstraps only. | No reserve truth should be inferred from this profile. |

The `multi-fetch-quorum` lane exists because exact-match live observation is not
reliable for active external accounts. The Hyperliquid drift study found live
snapshot skew small enough for tight tolerance bands, but not bit-identical
across samples. The protocol therefore verifies bounded agreement, not byte
identity.

## Reserve packet

A reserve packet binds the NAVCoin instance, epoch, reserve value, supply,
policy, and evidence roots:

```text
asset_id
epoch
nav_per_unit
circulating_supply or valid_global_supply
verified_net_assets
proof_profile
source_root
attestor_root
reserve_packet_hash
```

For source-labeled cash such as pfUSDC, the packet must count only finalized,
allocated, replayable vault-bridge receipts. A ticker is not enough. The
reserve packet has to say which source domain produced the cash claim, what
finality proof was accepted, what haircut or policy applied, and whether the
value has already been consumed by a NAVCoin subscription.

## Attestors and challenges

`nav_attestor_register` creates an observer record with identity, domain, bond,
and registration height. For multi-fetch profiles, attestations from unregistered
observers do not count toward quorum.

Challenges are bonded and deterministic at finalization time:

- a same-epoch replacement packet can refund a challenger;
- an abandoned challenge can forfeit the bond to the issuer;
- a failing observer verdict blocks finalization rather than starting a debate;
- stale active packets fail mint/redeem closed through the profile deadman
  switch.

## SP1 and disclosed leverage

SP1 proof work is a reserve-evidence primitive, not a universal solvency proof.
It can verify a disclosed account set, valuation policy, arithmetic, and public
output buckets. It cannot prove that no other accounts or liabilities exist.
That limitation is why NAVCoin docs distinguish reserve evidence, source risk,
and completeness risk.

## Privacy boundary

Reserve packets are public and replayable. Shielded swaps can hide transfer
details after a user holds shielded `a651` or `pfUSDC`, but they do not hide the
aggregate reserve and supply changes needed to keep NAVCoin backing auditable.
