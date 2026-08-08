# Complete EVM spot set — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** EVM spot is record 2/6 for B3. This
does not authorize G5 migration, a live route change, or StakeHub deprecation.

## Evidence against the source criteria

- The plan requires governed chain/token/owner/slot identity, accepted-root
  account and balance-slot inclusion, finality/freshness, normalized amounts,
  duplicate rejection, separate valuation, and checked aggregation of every
  configured position
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:588-597`).
- The immutable record names the public multichain quantity verifier and public
  Chainlink valuation verifier, cryptographic trust classes, 20-block / 8-block
  freshness, EVM/checkpoint fuzz targets, and zero liability for governed
  spot-only positions
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:83-97`).
- Complete-position inputs are distinct:
  `d258474900cefb8203756f213af8c59a368746f4edfac2899c5eee1cf9c4c422` (7)
  and
  `7a142c923a345b3c223296b2a47258ef7e3121b8f8f4235599f4481f04e7475c` (8),
  with separate quantity and valuation commitments
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:98-118`).
- Retained fuzz: 326,405,841 executions; zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Fresh proof closure

Epoch 7/8 witness hashes are
`8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
`4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89`;
fresh proof hashes are
`229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee` /
`bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
Both are independently bounded-verified as valid; the committed reproduction
manifests hash to
`e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
`c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
(`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).

## Boundary

The G3 condition at
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1249`
is met. Fleet lifecycle, finalized-state reconciliation, and clean-checkout
lifecycle reproduction remain open.
