# NAVCOIN Proof of Reserves

Status: Phase 0 + Phase 1 live on devnet
Plan: `NAVCOIN_POR_V2_PLAN.md` (repo root)
Public proposal: [The NAVCoin Proposal](https://postfiat.org/blog/navcoin-proposal/)
Evidence bundle: [postfiat.org/benchmarks/navcoin-por-devnet-20260610/](https://postfiat.org/benchmarks/navcoin-por-devnet-20260610/README.md)

The NAV rail described in [NAVCOIN Current Infrastructure](navcoin-current-infra.md)
proves ledger behavior once a reserve packet exists. This page documents the
verification layer above it: how the chain decides a reserve packet may
finalize at all.

## Design principle

Every deterministic check runs inside transaction validity — no oracle
committee, no attestation theater on facts any validator can recompute.
Voting survives only where physics requires it: observation of external
state (consensus nodes cannot perform live I/O) and bonded-challenge
adjudication. The full reasoning trail is in the plan document and the
public proposal.

## Proof profiles

`nav_profile_register` creates a content-addressed, immutable
`NavProofProfile` ledger object. The profile id is a domain-tagged
SHA3-384 over the profile parameters, so identical parameters always
resolve to the same id and "what counts as proof" is itself a registered
fact. Fields:

| Field | Meaning |
|---|---|
| `verifier_kind` | `ledger-transparent`, `placeholder`, or `multi-fetch-quorum` |
| `source_class` | What observers consult: `ledger`, `hyperliquid`, a portfolio label, etc. |
| `max_snapshot_age_blocks` | Packet staleness bound for finalization (0 disables) |
| `challenge_window_blocks` | Minimum blocks between submit and finalize |
| `max_epoch_gap_blocks` | Deadman switch: mint/redeem fail closed past this age |
| `settle_deadline_blocks` | Unsettled redemptions past this block minting |
| `min_challenge_bond` | Bond a challenger must escrow |
| `min_attestations` | Pass attestations required for multi-fetch finalize |
| `tolerance_bp` | Relative band for observer pass verdicts |
| `valuation_policy_hash` | Hash of the leg/mark/invariant policy observers enforce |

NAV assets registered with a profile-id-shaped `proof_profile` must
reference an existing profile. Legacy free-text labels keep the
pre-profile semantics, so existing tests and flows are unchanged.

## Verifier kinds

**`ledger-transparent`** — reserves are native accounts on this ledger.
`nav_reserve_submit` is consensus-INVALID unless the declared
`reserve_accounts` balances sum exactly to `verified_net_assets`.
Verified reserves with no trusted party at all; these packets are immune
to challenges because consensus itself verified them.

**`multi-fetch-quorum`** — reserves live at an external public,
address-indexed source. Registered observers independently fetch the
source, apply the profile's tolerance and valuation policy, and submit
`nav_reserve_attest` verdicts carrying an `observation_root` (a
domain-tagged hash of their normalized observation — evidence, never a
matching criterion). Finalization requires `min_attestations` passing
verdicts and ZERO failing ones: a single fail forces packet supersession
rather than debate.

**`placeholder`** — testing only; no reserve verification.

## The attestor registry

`nav_attestor_register` creates an identity-bearing observer record
(address, domain, optional escrowed bond, registration height).
Multi-fetch attestations are only accepted from registered attestors —
this is what makes `min_attestations` Sybil-resistant. Registration is
open and is NOT gated to validators: observation is paid service work
under the no-validator-rent policy, and validators may compete for it
without holding a monopoly (plan §10a).

## Lifecycle hardening

- **Bonded challenges**: `nav_reserve_challenge` escrows a bond
  (`min_challenge_bond` floor). Deterministic resolution at finalize:
  a same-epoch replacement packet refunds the challenger; an abandoned
  epoch forfeits the bond to the issuer.
- **Freshness**: packets stamp `submitted_at_height`; finalize enforces
  the challenge window and staleness bound; the asset stamps
  `finalized_at_height` and the deadman switch fails mint/redeem closed
  once the active packet exceeds `max_epoch_gap_blocks`.
- **Redemption settlement**: `nav_redeem_settle` records an off-chain
  settlement receipt hash; redemptions pending past
  `settle_deadline_blocks` block further minting.

## Empirical basis for tolerance

The drift study (`scripts/hyperliquid-drift-study`) observed a live
~$275M Hyperliquid vault: equity span drift of 0.0005 bp across ten
samples, with ZERO bit-identical observation snapshots. Conclusion baked
into the design: exact-match attestation is physically impossible on
active accounts, and even a 1 bp tolerance band carries ~2000x margin
over measured drift while catching any real misstatement.

## Source adapters and valuation policies

Python modules under `python/postfiat_rpc/`:

- `hyperliquid.py` — public info-endpoint observation (perp state with
  signed positions, spot balances, venue mids), deterministic
  normalization, observation roots.
- `solana.py` — public RPC observation (native balances, parsed stake
  accounts), same discipline.
- `basis_policy.py` — the nSOL/nETH valuation policy: long staked X +
  short X perp + USDC margin, every leg marked at the venue mid where
  the hedge settles (one mark source makes the hedge net exactly;
  divergent oracles inject phantom basis). Strategy invariants — hedge
  gap and margin ratio — are part of the verdict: an unhedged book
  FAILS attestation even when its value is accurate. The policy
  descriptor hashes to the profile's `valuation_policy_hash`.
- `navcoin.py` — operation builders for every NAV transaction kind,
  including the cross-language profile-id mirror (consensus vectors
  asserted in both the Rust and Python test suites).

## Smokes and evidence

| Script | Proves |
|---|---|
| `scripts/navcoin-current-infra-smoke` | Legacy lifecycle end-to-end (see the Current Infrastructure page) |
| `scripts/navcoin-multifetch-smoke` | Full multi-fetch lane against LIVE Hyperliquid data: bonded attestor registration, tolerance profile, packet from a live observation, three independent observer attestations with distinct roots, quorum finalize, validator convergence |
| `scripts/hyperliquid-drift-study` | Snapshot-skew measurement for tolerance policy |

Reports land under `reports/`; public copies with checksums are mirrored
to the website evidence bundle.

## Not yet built

TEE attestation for authenticated venues (IBKR, Binance — blocked on
AWS Nitro infrastructure), host-chain supply observation for external
assets, attestor scoring integration, and governance-gated profile
registration. The plan document tracks open decisions (fee schedule,
post-finalize challenges, emergency revocation).
