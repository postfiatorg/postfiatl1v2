# Batched-index G4 campaign failed closed on one round-coverage residual

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

The single campaign authorized by the
[batched-index G4 plan](../plans/active/batched-index-g4-campaign-plan.md) ran
once and **failed closed** with `ROUND_COVERAGE_RESIDUAL_EXCEEDED` in
`selected-indexed/height-5000-window-5`: finalized round 43 carried 108.4 ms
of unattributed time against the 100 ms cap. That is 1 round of 500 measured,
an 8.4 ms overshoot, with no stage anomaly and no work-gate violation
anywhere in the campaign. Exact diagnostic recomputation from the nine
completed windows shows both height ratios **passing** for the first time:
consensus 1.0926 and wallet-to-finality 1.0869 against the 1.10 limit. These
are diagnostics from an incomplete matrix, not a final report; formally G4
remains FAIL and no packet exists.

## Identities (unchanged from the plan)

Candidate `86929450992a64d1be1fb98cfde6aa46143c4568`, binary
`d27fc062e1248275e39b922a0da175f0c8e4cb7218023a2e9885ffcb9911112c`, runner
`a3c7bea9`, helper `cd43dd98…e047a`, prepared input `78206fea…5cad`. Private
output: `~/repos/postfiat-storage-g4-measurement-a3c7bea9-86929450-v1`
(preserve; never commit or publish).

## What the run proved before failing

- 9 of 10 selected windows completed and passed **every** gate: literal
  receipts, six-validator convergence, bounded `redb` work, zero full-history
  reads, round coverage, vote-lock, and certified-send work.
- Both remediations behaved exactly as designed in campaign conditions:
  - every validator's certified-send migration landed on resume observation 1,
    including the five outbox-less validators (migration rounds 1–6, zero
    violations);
  - vote-lock eager markers migrated at reservation observation 1;
  - validator-0's at-cap steady-state resumes ran 50–63 ms with 5 compacted +
    5 pruned per round (down from ~205 ms), within the ≤5/≤5 per-resume gate
    limits.
- Diagnostic p95s: height 50 consensus 363.3 ms (n=250) versus height 5,000
  396.9 ms (n=200) → ratio `1.0926`; wallet-to-finality 379.3 → 412.2 ms →
  ratio `1.0869`. The batching also sped height 50 itself, so the passing
  margin (~1%) is thinner than the naive projection.

## One diagnosis

Round 43 of the fifth height-5,000 window, proposed by validator-3, totaled
468.6 ms with 360.2 ms attributed to named stages and 108.4 ms residual. All
individual stages were normal (certificate 154.5 ms, certified sends 73.3 ms,
precommit votes 72.9 ms). The residual is scheduling scatter between named
stages, 8.4 ms over a cap that 449 other rounds satisfied (next-worst 71.5
ms). Two environmental factors were present and are operator-owned:

1. the disk was at 91% occupancy during measurement (an earlier initialization
   attempt aborted on `ENOSPC` before any measurement; rebuildable `target/`
   caches were cleaned to proceed); and
2. the operator ran checkpoint-inspection commands on the same host during
   measured windows.

This failure class is measurement hygiene, not candidate behavior: no
candidate-owned stage grew, and the same stage set covered every other round.

## Environment actions taken (recorded)

- Deleted before the run, to recover disk space: the aborted no-evidence
  partial initialization directory and the rebuildable `target/` caches of
  the main repository and the three superseded candidate checkouts
  (`e52e0502`, `a92bb085`, `442c5a4d`). All frozen binaries remain preserved
  in their G1 evidence directories; no campaign, fleet, or evidence directory
  was touched.

## Next action under the standing operator directive

The operator's standing directive for this session is to unblock the testnet
path. A successor plan (same frozen identities, one run) adds the two hygiene
controls this diagnosis names: minimum free-disk headroom before start, and
zero operator host activity during measurement. It does not relabel or reuse
this failed run. See the
[hygiene-corrected G4 plan](../plans/active/hygiene-corrected-g4-campaign-plan.md).
