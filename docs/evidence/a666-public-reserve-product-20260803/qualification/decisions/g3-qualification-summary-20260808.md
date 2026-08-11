# B3 / G3 source-qualification summary — 2026-08-08

## Final B3 decision

**CLOSED — 6/6 G3 source-adapter decisions recorded.** The governing G3
condition requires public collector plus source-state/ownership/quantity/
liability and valuation verifiers, bounded parsers, adversarial tests, fuzz
qualification, and no aggregate-operator-attestation shortcut
(`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1244-1250`).

In the table, `Draft` is
`651bfa6282b730f0f7c09871f1c8e999ae1eda8b:docs/evidence/a666-public-reserve-product-20260803/qualification/source-qualification-draft.json`;
`plan` is
`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md`.

| Family | Decision | Evidence lines | Decision record |
|---|---|---|---|
| Aave on Arbitrum | PASS — G3 | Draft `:44-80`; plan `:561-573` | `g3-aave-decision-20260808.md` |
| Complete EVM spot set | PASS — G3 | Draft `:83-118`; plan `:588-597` | `g3-evm-spot-decision-20260808.md` |
| Hyperliquid | PASS — G3 | Draft `:121-140`; plan `:610-673` | `g3-hyperliquid-decision-20260808.md` |
| Staked NEAR | PASS — G3 | Draft `:143-162`; plan `:681-736` | `g3-near-decision-20260808.md` |
| Staked Solana | PASS — G3 | Draft `:165-185`; plan `:743-812` | `g3-solana-decision-20260808.md` |
| Monero reserves | PASS — G3 | Draft `:188-207`; plan `:816-874` | `g3-xmr-decision-20260808.md` |

## Shared proof and fuzz closure

- Epoch 7 witness/proof:
  `8f4538b111a6cebf97a71406895af40d4bc4c6ed369c3ecd540b6050765b6dd6` /
  `229c9e328050a82c628c370393e4d89120a93bee621fa0d5edad6e6ae8e975ee`.
- Epoch 8 witness/proof:
  `4bb36f68ebee3537afe62f6d9ed61d68513a96c747fffc3622024b0a2f9efe89` /
  `bf96ebd8894ed8a41329c9e61f70784dfacbf316f17f60c44ffd718dd60450d7`.
- Both were independently bounded-verified under ELF
  `2b41e4e8095b1dacdc519b2f0a2b4831ebc57cc8003a4d3686f6d9e4687e81df`,
  vkey `0x00f3857f96ef97e00bd15b4030acd8d6b0a72740b28c6160d154bc2c9bb141bf`,
  and profile
  `f8784629ff7338002d836c1988b8e2c0f19caf448429e0eb7fdc39fa2b08f7d9a44171fc1e7239bc25e06ad833c14e91`
  (`/tmp/a666-terra-20260808/prover-b-report.md:102-105`).
- Bundle commit `651bfa6282b730f0f7c09871f1c8e999ae1eda8b`; manifest hashes:
  `e2ea6ba4d69d461bed7451120f3ee22cb465abca0f85887abe4213a587fe7e20` /
  `c4ab1fda3306a6a44f4d87643d8f6c4d6fa5b5e8c10b8802bf0529ed49bac52b`
  (`/tmp/a666-terra-20260808/prover-b-report.md:105`).
- Retained fuzz: 326,405,841 executions; zero crash, timeout, or OOM
  (`docs/plans/STAKEHUB-DECOUPLING-AND-OPEN-RESERVE-PROOF-INFRASTRUCTURE-PLAN-20260801.md:1360-1383`).

## Non-closure boundary

This 6/6 result closes B3's G3 source-adapter records and source-equivalent
proof core only. G5 remains open for the exact six-validator lifecycle,
restart/replay/conservation/pause/rollback, and controlled-state
reconciliation. G6/G7 remain open. No live profile, route, packet, or balance
change is authorized, and `stakehub_deprecated=true` is not asserted.
