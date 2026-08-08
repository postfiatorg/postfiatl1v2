# Hyperliquid — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** Hyperliquid is record 3/6 for B3. The
certified source-checkpoint boundary is explicit: it is not an
aggregate-operator attestation or a claim of direct HyperEVM account-MPT proof.

## Evidence against the source criteria

- Required checks are pinned header and receipts root, reader/event and salted
  snapshot commitments, reserve identity, complete spot/perpetual/liability
  arithmetic, bounded rows/proofs, duplicate/invalid-value rejection, and
  replay/freshness controls
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:610-625`).
- Public code requires the complete policy-pinned position sets and account-wide
  notional, reconstructs receipt-trie evidence, and commits the unavoidable
  zero-`stateRoot` limitation to validator-reproduced quorum certification;
  receipt contents remain cryptographically checked
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:637-664`).
- The record pins reader
  `0xddb4ed1edf1f0d81f7531cddb27810080601a2cb`, runtime Keccak
  `c252f32acd9fdcfe2b4f9b1d70c3de17acf83649a6313fc3ab9155bca1010db3`,
  cryptographic trust classes, freshness, negative-equity policy, and relevant
  fuzz targets
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:121-135`).
- Distinct input hashes are
  `bed99bd5673469b746c42472e8fd083e3bc94c6a338a70da0e676cb296d1b92d` (7)
  and
  `59fb601c80917c5b29db6856f77ae8f0aa662864b27bee97dcad26b96a3f83d6` (8)
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:136-140`).
- Retained fuzz: 326,405,841 executions; zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Fresh proof closure

Epoch 7/8 witness hashes are
`8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
`4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89`;
fresh proof hashes are
`229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee` /
`bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
Both were independently bounded-verified under the pinned successor identity;
the reproduction manifests hash to
`e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
`c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
(`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).

## Boundary

The G3 condition at
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1249`
is met; G5 lifecycle and all live-migration controls remain open.
