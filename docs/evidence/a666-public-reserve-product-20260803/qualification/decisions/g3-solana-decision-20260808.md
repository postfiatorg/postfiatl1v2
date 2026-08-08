# Staked Solana — G3 qualification decision

## Verdict

**PASS — G3 source-adapter qualified.** Staked Solana is record 5/6 for B3.
The historical signed-RPC adapter is excluded; this covers only the public
reserve-reader/BFT-checkpoint successor.

## Evidence against the source criteria

- Required evidence is cluster/finalized-slot identity, owner/stake authority,
  stake/vote account state, lamports/conversion, stake lifecycle,
  duplicate/bounded-account rejection, freshness, and SOL/USD valuation
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:743-754`).
- The selected public mechanism rejects malformed/duplicate/writable/wrong-owner
  accounts and verifies finalized transaction/block, immutable reader ProgramData,
  full authority bindings, return data, and checkpoint certificate; it does not
  claim direct Solana consensus verification
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:773-792`).
- The record pins program
  `Gp2oTn6VjFF22n98H6YSH4uVvQxWFHNCL7pp1tcAPF36`, raw ELF SHA-256
  `af70e82df3f1d519da5c5c7ddb62ab594d7babdd95dbc67c1692c2d6cea96716`,
  no upgrade authority, cryptographic trust classes, and Solana/checkpoint fuzz
  targets
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:165-180`).
- Inputs are distinct:
  `27d063606a753833f4c078d8c580ae39dfa84cdfe8b580399868fac9429c39d8` (7)
  and
  `078fb3dd57875cbd3d5155615132ddc0fb867675d4da5644da2fe93348b799ab` (8)
  (`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json:181-185`).
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
is met. This does not assert G5 lifecycle, live migration, or StakeHub
deprecation.
