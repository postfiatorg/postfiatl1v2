# Aave on Arbitrum — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** Aave is record 1/6 for B3. This
qualification does not authorize a live activation, a G5 lifecycle claim, or
StakeHub deprecation.

## Evidence against the source criteria

- The required Aave checks are chain/deployment and owner identity, accepted-root
  account/storage evidence, all collateral and debt, valuation, duplicate and
  freshness rejection, checked net arithmetic, and canonical commitments
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:561-573`).
- The immutable record identifies the public verifier and policy, classifies
  quantity and valuation as cryptographic, binds 20-block / 8-block freshness,
  lists Aave EVM/checkpoint adversarial targets, and derives net value as
  collateral minus debt
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:44-57`).
- Distinct epoch inputs are
  `0c3af45c15c6b54639883ba0419c461c3d3fc5aabfcb5f1ec48e1f1dd219c27a`
  (7) and
  `943af79761b91fd03479795acabd6898dc6a2e2d2837370957add90b25bb53e1`
  (8), with distinct source commitments
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:58-77`).
- The retained 10-target fuzz campaign completed 326,405,841 executions with
  zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Fresh proof closure

Epoch 7/8 source-witness hashes are
`8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
`4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89`;
fresh proof hashes are
`229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee` /
`bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
Both have independently bounded `valid: true` verification under pinned ELF,
vkey, and successor profile; committed reproduction manifests hash to
`e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
`c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
(`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).

## Boundary

This meets the G3 public-adapter, bounded-parser/adversarial/fuzz, and
no-aggregate-attestation condition
(`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1249`).
Controlled lifecycle, reconciliation, and clean-checkout lifecycle reproduction
remain later gates.
