# Staked NEAR — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** Staked NEAR is record 4/6 for B3. It
does not close G5, permit a live profile/route change, or deprecate StakeHub.

## Evidence against the source criteria

- Required checks are mainnet/head identity, outcome/block Merkle proofs, reader
  and staking-pool identity, ownership, snapshot schema, staked/unstaked
  quantities, substitution rejection, freshness, conversion, and valuation
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:681-693`).
- The public stateless reader and unsigned invocation verify policy-pinned code
  hashes at the checkpointed head plus outcome/block Merkle proof before an
  observation; no NEAR private key enters the proof kit
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:704-721`).
- The immutable record pins reader account
  `eed15bedebb4ac46d1528187a8c2f00aa59b441398d3e346c44eb2dcb2fc1d9a`,
  code hash `5swZhNNqpD6HsqFXhNjRUiSYoXtnkWPipiW8hRbrkbN`, cryptographic
  quantity/valuation, freshness, and NEAR/checkpoint fuzz targets
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:143-157`).
- Inputs are distinct:
  `b408c4bc90a3f07bc6ed47904308fb5e753f3e49ba1c192a84eb039b4be9cfa6` (7)
  and
  `6e4d776cf52c8e9ffcc8ff7429fc1a341242ac4e0555dfc73fea70dfc9de062d` (8)
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:158-162`).
- Retained fuzz: 326,405,841 executions; zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Fresh proof closure

Epoch 7/8 witness hashes are
`8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
`4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89`;
fresh proof hashes are
`229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee` /
`bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
Prover-B recorded independent bounded valid verification for both; manifests
hash to
`e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
`c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
(`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).

## Boundary

The G3 condition at
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1249`
is met. The checkpoint is disclosed, not mislabeled as direct NEAR consensus
finality. Controlled lifecycle and final-state reconciliation remain later work.
