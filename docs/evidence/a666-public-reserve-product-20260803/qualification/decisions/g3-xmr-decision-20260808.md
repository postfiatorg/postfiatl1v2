# Monero reserves — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** XMR is record 6/6 for B3. No wallet,
view key, spend key, or live Monero state was accessed or changed for this
decision.

## Evidence against the source criteria

- Required checks are governed network/address; fresh domain-separated
  NAV/profile/manifest/policy/epoch challenge; ReserveProof; proven/spent
  quantity semantics; substitution/replay/stale-proof rejection; bounded
  parsing; and XMR/USD valuation
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:816-827`).
- The public path uses bounded canonical `ReserveProofV2`, transaction and
  header/inclusion reconstruction, key-image-status verification, checkpoint
  certification, and separate valuation; it excludes wallet seed, spend key,
  and view key from the proof kit
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:838-857`).
- The record names public quantity and valuation verifiers, cryptographic trust
  classes, freshness, Monero/checkpoint fuzz targets, and spent-key-image
  rejection
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:188-202`).
- Distinct nonzero inputs are
  `d31f7f651a4cb7dc8a7912b3565f6e90a7b2db8e691a5180ec95cc4d8863f802` (7)
  and
  `c467ebb78cb1a2bc308a45621c1b4d8ae422d87de09278331fc4280b56307600` (8)
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:203-207`).
- Retained fuzz: 326,405,841 executions; zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Fresh proof closure

Epoch 7/8 witness hashes are
`8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
`4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89`;
fresh proof hashes are
`229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee` /
`bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
Both were independently bounded-verified; manifests hash to
`e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
`c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
(`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).

## Boundary

The G3 condition at
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1249`
is met. Controlled lifecycle, final-state reconciliation, and clean-checkout
lifecycle reproduction remain subsequent gates.
