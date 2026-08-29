# Remediated G4 Qualification Campaign: Executable Plan

**Status:** Closed — exactly one execution failed; no retry, final report, packet, or qualification
**Date:** 2026-08-29
**Planning baseline:** `main` at `24edd8fa`; frozen candidate source `a92bb085`
**Predecessors:** [Final G4 qualification plan](final-g4-qualification-campaign-plan.md) (closed, failed — mechanics of this plan follow it),
[Certified-send eager-index remediation spec](certified-send-eager-index-remediation-spec.md) (implemented, verified),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)
**Key handoffs:** [Eager-index remediation complete](../../handoffs/2026-08-29___postfiatchad__certified_send_eager_index_remediation_complete.md),
[Final G4 qualification failure](../../handoffs/2026-08-29___postfiatchad__final_g4_qualification_failure.md)

## BLUF

The plan's single authorized campaign executed once and **failed**. All ten
selected-redb windows completed 500 valid rounds with literal receipts,
six-validator convergence, bounded transactional work, zero full-history
reads, and passing vote-lock, certified-send, and timing-coverage gates.
However, their checkpoint-bound reports independently recompute to
height-5,000/height-50 p95 ratios of **1.403** for consensus and **1.402** for
wallet-to-finality, both above the 1.10 limit.

The first legacy height-50 control then completed all 50 raw rounds but failed
`VOTE_LOCK_MIGRATION_AFTER_FIRST_VALIDATOR_RESERVATION`. Four validators
migrated their vote-lock index on their second observed reservation. The node's
empty-directory path returns without writing the index marker, so the first
reservation creates a lock and the next reservation performs migration. The
preflight telemetry fixture did not exercise that actual node sequence.

There is no final campaign report or packet. The packager was not invoked.
Checkpoint SHA-256 is
`e33dfdb628563f38d486ace5a3ebc13be280ecea5cb862a8da51627b1c6028a3`;
the one-diagnosis SHA-256 is
`e2134a4ea8988ced89e95f601b0cdc0aeaeffe9acd46676976f54adadb60c164`.
This run cannot be resumed, retried, or relabeled. Storage remains selected but
not offline qualified.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `a92bb085ceb6a9f405e916608e6b7bb6010fcc9b` |
| Candidate binary | SHA-256 `902773e00e5226dab9e027ebce2b932b2cf26509dba08424f6ebe46db985e182`; 51,977,656 bytes; embedded revision `a92bb085`; profile `release` |
| G1 candidate manifest | SHA-256 `ed66a6375234f64d5aab863bccb6415b07c77fc5a3a028c5a6c2f01f41af0190` |
| G2 safety manifest | SHA-256 `dd300bcb8130f91ab54e26f969fe7dca37335d99cc5bf4ca78a939a79584d170` |
| Rollback report | SHA-256 `9c32319693df1f55a6c1ecd75449fe8341d180317e11547c604f135741c3e8a5` |
| Tamper/crash report | SHA-256 `6b63fe1070a2981e5d2720bf25b3cf3b8ad95beece364d9fd76579f027b146e0` |
| Runner/verifier | Branch `postfiatchad/corrected-g4-vote-lock-gate` at `a3c7bea9285ab02871fd2111038764c6174b905b`; gate-logic parent `15d059d1`; test-only successor |
| Batch-builder helper | Release binary SHA-256 `ad70ca685cfaf1d0a67eb80f4805438c0e4363c8957598d1d884abd03690014a`; identity smoke `a3c7bea9` / `release` |
| Frozen fleets | Unchanged 5,000-block input prepared by `ae658441`; the manifest preserves this `prepared_by` provenance distinction |
| Prepared-input manifest | SHA-256 `c9fb32e7c3cebcf2ef16a90843c63dd96b7ed0ebc3c20ce94d2fd21707e7da42` |
| Independent input-verification receipt | SHA-256 `6848d49d2488cd0730efd14863c5fe446a1f31827cec98346583beee8b9cbb58`; all 18 references rehashed |
| Failed-campaign lineage | Both prior private run directories and all closed-failed records preserved untouched; never resumed, retried, or relabeled |

The candidate binary must be byte-identical from its G1 record through the end
of the campaign. Any node-source change aborts this plan and returns to a new
freeze. Runner-only changes remain separately hash-bound; the runner used to
measure must be exactly `a3c7bea9`.

## Hard rules (inherited from both predecessor campaign plans — all still bind)

1. Build and fleet verification outside the four-hour measurement clock.
2. Unchanged matrix: five selected-redb windows at height 50, five at height
   5,000, five legacy-JSONL controls at height 50; 50 finalized rounds per
   window; six validators; selected windows before legacy controls.
3. Diagnose once on failure; no retry, no matrix enlargement, no relabeled
   partial runs; `TIME_BUDGET_EXCEEDED` is a recorded result.
4. Offline only: no devnet, deployment, height-924 copy, or live-fleet claims.
5. Round success = valid certificate + literal matching receipts +
   six-validator convergence. Never elapsed time alone.
6. Private run directories contain validator material: never commit, publish,
   or delete.
7. Prepared-input derivation and verification are read-only; derivation must
   not open or mutate candidate state.

## Known residual risks (state honestly before the clock starts)

- **Height-5,000 with the certified-send fix is unmeasured in-campaign.** The
  spot check (2.064 ms at the cap, height-flat) predicts passing ratios, but
  the prior corrected campaign is the only one to complete height-5,000
  windows and it predates this fix. A genuine ratio failure here is a real
  scaling result, not a contract bug, and closes this plan as FAIL.
- **The legacy lane's re-keyed vote-lock migration contract is unproven
  in-campaign.** The corrected campaign's first legacy window failed on the
  old round-keyed contract; the re-keyed first-use contract passed preflight
  suites but no legacy window has run since. The legacy windows run last, so
  a legacy-lane contract failure would strand ten completed selected windows;
  the checkpoint preserves them as verifiable units either way.
- **Both migration allowances fire inside measurement windows.** Vote-lock at
  each validator's first reservation; certified-send at each validator's first
  resume — now including empty-index creation for outbox-less validators with
  all work counters zero. Both are modeled by fixtures on runner `a3c7bea9`.

## Step 0 — Authorization gate

**Authorization recorded:** on 2026-08-29, the operator directly instructed the
executing IC to "do this" plan and named the three governing context documents.
That instruction authorizes exactly one run of this plan. Per the operator's
standing instruction and the milestone's operations boundary, no Task Node or
agent workflow is used. Review and authorization are now complete; measurement
may start only after Steps 1 and 2 pass.

## Step 1 — Re-verify bound inputs (outside the clock)

The prepared-input manifest `c9fb32e7…da42` and independent receipt
`6848d49d…bb58` already exist and passed. Before the clock starts:

- [x] Rehash the candidate binary against `902773e0…e182` and its embedded
      revision against `a92bb085`.
- [x] Confirm the runner worktree is clean at exactly `a3c7bea9` and the
      helper binary rehashes to `ad70ca68…014a`.
- [x] Independently reopen the input manifest and rehash all 18 references;
      any mismatch aborts before measurement with no clock consumed.

**Completed:** all source, binary, manifest, runner, helper, and receipt identities
match the frozen table. The candidate and helper binaries contain their expected
embedded revisions. A fresh independent read-only pass rehashed all 18 input
references successfully. The intended output path remained absent.

## Step 2 — Preflight the migration allowances (outside the clock)

- [x] Run the full runner/packager/verifier suite at `a3c7bea9` (96 tests)
      and confirm it includes: vote-lock first-use, certified-send first-use
      with a populated outbox, certified-send first-use with **no outbox**
      (empty-index migration, zero counters), the
      no-outbox→deliveries→second-resume sequence, repeated-migration
      rejection, and late-migration rejection.
- [x] Run the focused node suites (`completed_index_tests`,
      `certified_send`) once against the frozen source to confirm the working
      tree still matches the freeze.
- [ ] **Retrospective FAIL:** exercise the exact portable-snapshot legacy
      node sequence. The preflight fixture modeled accepted per-validator
      telemetry, but did not execute empty vote-lock directory → first durable
      lock → second reservation against the candidate.

**Recorded preflight result:** the runner, packager, and verifier suite passed
96 tests in 22.556 seconds; the frozen candidate passed 15 completed-index tests
and 35 certified-send tests, with only the intentional manual release spot
check ignored in each node suite. Both source and runner worktrees remained
clean. The campaign proved this coverage was insufficient: the claimed
portable-restore contract was not exercised end to end.

## Step 3 — Campaign result

Exactly one campaign ran in the fresh private directory
`~/repos/postfiat-storage-g4-measurement-a3c7bea9-a92bb085-v1`. The four-hour
measurement clock started at `2026-08-29T14:44:45Z`; the campaign failed at
`2026-08-29T15:18:44Z` after 2,038.594669 seconds, well inside the budget.

Ten selected units completed: five height-50 windows and five height-5,000
windows, 50 finalized rounds each. The first legacy height-50 unit completed
its 50 raw rounds, then failed its vote-lock work gate. The checkpoint status
is `FAILED`, `final_report_sha256` is null, and no campaign process survives.
The packager was not invoked.

| Failure artifact | SHA-256 |
| --- | --- |
| Campaign checkpoint | `e33dfdb628563f38d486ace5a3ebc13be280ecea5cb862a8da51627b1c6028a3` |
| Failed legacy raw report | `379a7b2630925b529ef55f727f92f38d32cfa49f3466f286e9fef12ab4815790` |
| Failed legacy vote-lock receipt | `4f3ad65296946d28bebd9a1ae88eb472ba92141deda5bb1b1bcacddb18cb4327` |
| One diagnosis | `e2134a4ea8988ced89e95f601b0cdc0aeaeffe9acd46676976f54adadb60c164` |

## Step 4 — Pass/fail gates

The candidate **qualifies** only if every row holds. Unavailable rows are
never passes.

| Gate | Threshold | Result |
| --- | --- | --- |
| Height-5,000/height-50 `consensus_round_ms` p95 ratio | ≤ 1.10 | **FAIL** — 405.759 ms → 569.129 ms; ratio `1.402629`, recomputed from the ten completed selected windows |
| Height-5,000/height-50 `wallet_to_finality_ms` p95 ratio | ≤ 1.10 | **FAIL** — 418.215 ms → 586.163 ms; ratio `1.401582`, recomputed from the ten completed selected windows |
| No material positive height relationship in synchronous stages | Required | **PASS for the ten completed selected windows** — every named-stage model reports false; no final campaign report exists |
| Round-coverage residual gate | Every measured round; residual < 100 ms | **Unavailable overall** — all 500 selected rounds pass, max 79.619 ms; the failed legacy report independently recomputes to 50/50 pass, max 66.968 ms, but its receipt was not emitted before the vote-lock stop |
| Selected/legacy height-50 comparison | All five legacy windows complete and compare | **FAIL / unavailable** — only the first legacy window ran, and it failed before becoming a completed unit |
| Literal receipts, six-validator convergence, bounded `redb` work, zero full-history reads | Every round | **Unavailable overall** — all ten selected windows pass; the raw legacy benchmark passes 50 rounds, but the campaign stopped before all 15 units |
| Vote-lock bounded-work and migration-position gate | First-reservation only; ≤2 files / ≤314 bytes ordinary votes | **FAIL** — validators 0, 1, 2, and 5 migrated in round 2 on reservation observation 2; four files / 866 bytes each |
| Certified-send bounded-work and migration-position gate | First-resume only, including zero-work empty-index migrations; `validated == compacted + pruned` on ordinary resumes | **Unavailable overall** — all ten selected windows pass; the failed legacy report independently recomputes to pass, but no receipt was emitted before the vote-lock stop |
| Four-hour measurement budget | Not exceeded | **PASS** — 2,038.595 / 14,400 seconds |

The ratio and named-stage entries are exact artifact-based recomputations using
the frozen runner's own aggregation functions. They are not a substitute for
the absent final campaign report and are not packet evidence.

## Step 5 — Recorded outcome

**FAIL; no retry authorized.** The process stopped at the first legacy
vote-lock work gate. `crates/node/src/vote_locks.rs:193-260` owns the failed
node behavior: when the vote-lock directory is empty, migration returns without
writing the marker. The first reservation then writes a lock; the next
reservation migrates it. The locked runner correctly rejects migration after
the validator's first observed reservation.

The preflight gap is equally specific: its telemetry fixture represented
first-use and late-use results, but did not execute the candidate's exact
empty-directory → first lock → second reservation sequence on the portable
legacy restore. A future remediation needs an owner-level node fixture and an
end-to-end portable-restore fixture for that sequence.

Separately, the completed selected data misses both scaling-ratio limits. A
future proposal must address both the vote-lock marker contract and the
selected-window latency tail. No source change, new freeze, or new campaign is
authorized by this failure closure. G3 remains open; G5, deployment, and public
testnet remain blocked.

## Explicit non-goals

- No devnet contact, deployment, or live-fleet claims.
- No height-924 copy (G3 authorization is separate; do not wait idle for it).
- No candidate source changes after the freeze.
- No reuse of either prior failed campaign's receipts as fresh evidence.
- No second campaign under this plan, pass or fail.
- No G5 packet assembly from private raw output; packaging authority stays
  with `package_packet.py` on a passing report only.
- The two untracked `docs/security/` auditor inventories remain untouched.

## Exit criteria

- [x] This plan reviewed; one run explicitly authorized and recorded by the operator's direct 2026-08-29 instruction.
- [x] Step-1 identity re-verification passed outside the clock.
- [ ] Step-2's focused suites passed outside the clock, but the claimed exact
      legacy-lane vote-lock sequence was not actually covered; the campaign
      exposed this retrospective preflight failure.
- [x] Exactly one campaign executed under one fresh four-hour clock.
- [x] Full gate table reported truthfully with all identities.
- [x] Milestone and handoff updated; exactly one named-stage diagnosis recorded,
      then stopped without candidate change, packaging, or retry.
