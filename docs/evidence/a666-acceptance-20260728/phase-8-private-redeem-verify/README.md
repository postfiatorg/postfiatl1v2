# A666 Phase 8 Private-Redemption Functional Run

**Date:** 2026-07-28

**Functional verdict:** `PASS`

**Formal A8 gate verdict:** `FAIL`

**Reason A8 is not a pass:** the live-value path completed with exact
conservation, but it required operator repairs during the run and missed both
25-minute timing thresholds. The acceptance spec requires a fresh hands-off
run after fixes.

## Completed mainnet path

```text
1.005000 Ethereum USDC
  -> 1.005000 PFTL pfUSDC
  -> 1.000000 newly issued A666
  -> 1.000000 Ethereum wA666
  -> 1.000000 returned PFTL A666
  -> 1.000000 private A666 note
  -> 0.999500 private pfUSDC note
  -> 0.999500 transparent PFTL pfUSDC
  -> 0.999500 Ethereum USDC
```

No Uniswap trade, operator A666 inventory, owner mint, or prefunded redemption
bucket was used. Primary issue created new A666 against the deposited pfUSDC.
Primary redemption retired that A666 and released its reserve-funded pfUSDC.

## Main identifiers

- USDC deposit: block `25,632,817`, transaction
  `0xa59e2b078f28c3bbd6579a1b2f27e1c9d235ca0d3794e12876fb616b68945c17`
- wA666 mint:
  `0x78ed7bd72a573c8f1ba8fac7a74cced61ec5b4c09d0ad2b908f405c734054711`
- wA666 return burn:
  `0x743cea0b7c5dc66f41f5ba743da695054a7215dd56a068afc62f7609f3f0216e`
- pfUSDC vault release:
  `0xcd10520e5e21d6492205d2f100c063c09fbd9b1d3891859e3d6d579c741bb219`
- PFTL private redemption: height `407`
- PFTL private pfUSDC egress: height `408`
- PFTL pfUSDC burn / settlement: heights `409` / `410`
- Final fleet state root:
  `12ea93322034fe5fcf092401ba0e71e2f7bb4edea9869ccb4752a30ad8c1bf3664b217240804ff1c7960cc9d80e5d48b`

## Exact conservation

| State | Before | After | Delta |
|---|---:|---:|---:|
| Joe Ethereum USDC | `230.187289` | `230.181789` | `-0.005500` |
| Epoch-5 vault USDC | `0.005500` | `0.011000` | `+0.005500` |
| Joe wA666 | `103.000000` | `103.000000` | `0` |
| wA666 total supply | `31,489.197455` | `31,489.197455` | `0` |
| Joe PFTL pfUSDC | `0.800000` | `0.800000` | `0` |
| A666 authorized supply | `31,489.197455` | `31,489.197455` | `0` |
| A666 reserve principal | `103.000000` | `103.000000` | `0` |

The round-trip cost was exactly `0.005500 USDC`: `0.005000` issue spread plus
`0.000500` redemption spread. The vault gained the same amount. Final
outstanding PFTL bridge claims equal wA666 total supply, Ethereum spendable
supply is zero, and the A666 supply invariant holds.

All six validators ended at height `410`, release revision `90618294`, one
identical state root, and empty mempools.

## Privacy boundary

The PFTL middle used verified Asset-Orchard proofs. The `1.000000 A666` input
note was consumed into an encrypted `0.999500 pfUSDC` output note; that note
then exited through a verified private-egress proof. Note openings and wallet
keys were not copied into this evidence directory.

This is not end-to-end private. Route, assets, amounts, action timing,
nullifiers, and commitments are public on PFTL. Ethereum exposes the deposit,
wA666 mint/burn, USDC withdrawal, wallet, amounts, and timing. See
`privacy-report.json`.

## Replay and proof results

- The private-primary redemption proof was `7,616` bytes and verified.
- The private pfUSDC egress proof was `7,616` bytes and verified.
- The PFTL egress SP1 Groth16 proof was `356` bytes and verified.
- Exact private-redemption batch replay rejected as already applied without a
  height change.
- Exact return-import replay rejected as
  `duplicate_pftl_uniswap_return_import` without a height change.
- Ethereum withdrawal replay rejected; burn, withdrawal, and proof nullifiers
  are consumed.

## Defects and timing

The run exposed three orchestration defects:

1. the issue relay pinned an obsolete Epoch-5 policy hash;
2. the Ethereum preflight compared the proof controller against the PFTL tip
   instead of the controller's prior finalized checkpoint; and
3. the issue workflow minted wA666 without recording the proof-backed
   destination-consume transition on PFTL. This made the later return import
   fail closed until the missing accounting transition was submitted.

All rejected attempts occurred before mutation. The policy/checkpoint pins and
resumable ingress workflow were corrected. The destination-consume operation
is now part of `scripts/a666-mainnet-transparent-issue-after-deposit.sh`
through `scripts/a666-mainnet-record-destination-consume.sh`.

Deposit inclusion to wA666 mint was `1,848 seconds` (`30m48s`). wA666 burn
inclusion to USDC release was `2,688 seconds` (`44m48s`). The entire
deposit-to-withdrawal interval was `4,668 seconds` (`77m48s`). Functional
correctness passed; the latency objective did not.

The machine verdict is `acceptance-summary.json`. Detailed interventions are
in `defect-ledger.json`, and timestamp calculations are in
`timing-summary.json`.
