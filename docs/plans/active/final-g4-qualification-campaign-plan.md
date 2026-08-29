# Final G4 Qualification Campaign: Executable Plan

**Status:** Ready for execution — awaiting explicit operator authorization for exactly one campaign
**Date:** 2026-08-29
**Baseline:** `main` at `32d7542c`; frozen candidate source `e52e0502`
**Predecessors:** [Corrected G4 campaign plan](corrected-g4-campaign-plan.md) (closed, failed — mechanics of this plan follow it),
[Certified-send tombstone bounding plan](certified-send-tombstone-bounding-plan.md) (implemented, verified),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)
**Key handoffs:** [Certified-send bounding complete](../../handoffs/2026-08-29___postfiatchad__certified_send_tombstone_bounding_complete.md),
[Corrected G4 failure](../../handoffs/2026-08-28___postfiatchad__corrected_g4_campaign_failure.md)

## BLUF for the executing agent

All three known height-scaling defects are fixed and locally verified:
storage (redb, proven in two campaigns), vote locks (proven in-campaign:
2,450 votes at ≤2 files / ≤314 bytes), and certified-send tombstone resume
(66.893 ms → 2.098 ms at 1,024 tombstones, zero retained payload reads,
proposer-rotation delta 2.054 ms). The round-coverage residual gate now makes
unattributed time itself a failure, so no fourth defect can hide untimed.

This plan runs **exactly one** unchanged 5+5+5 measurement campaign against
the frozen candidate to answer the qualification question. Expected outcome
given the evidence: all rounds land near the ~400 ms band that non-proposer
rounds already achieved at height 5,000, passing the ≤1.10 ratio gates. But
expectation is not evidence; run the campaign, report the gate table
truthfully, and stop.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `e52e050269a2f9fdd28c5083c3888debf3a85063` |
| G1 candidate manifest | SHA-256 `895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe` (campaign inputs not yet bound — Step 1 completes this) |
| G2 safety manifest | SHA-256 `dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7` |
| Runner/verifier | branch `postfiatchad/corrected-g4-vote-lock-gate` at `15d059d1` in `~/repos/postfiat-storage-measurement-runner-1f478e0c` |
| Frozen fleets | unchanged from prior campaigns; built by `ae658441` — preserve the `prepared_by` provenance distinction exactly as the corrected campaign's manifest did |
| Failed-campaign lineage | preserve both prior campaign records and private run directories untouched |

The candidate binary must be byte-identical from its G1 record through the
end of the campaign. Any node-source change aborts this plan and returns to a
new freeze. Runner-only changes remain separately hash-bound.

## Hard rules (inherited from the corrected campaign plan — all still bind)

1. Build and fleet verification outside the four-hour measurement clock.
2. Unchanged matrix: five selected-redb windows at height 50, five at height
   5,000, five legacy-JSONL controls at height 50; 50 finalized rounds per
   window; six validators; selected windows before legacy controls.
3. Diagnose once on failure; no retry, no matrix enlargement, no relabeled
   partial runs; `TIME_BUDGET_EXCEEDED` is a recorded result.
4. Offline only: no devnet, deployment, height-924 copy, or Task Node.
5. Round success = valid certificate + literal matching receipts +
   six-validator convergence. Never elapsed time alone.
6. Private run directories contain validator material: never commit,
   publish, or delete.

## Step 1 — Bind campaign inputs

Complete the open G1 item: produce one new prepared-input manifest binding
the frozen candidate identities above, runner `15d059d1`, the batch-builder
helper, the unchanged fleet digests, topology, keys, corpus, certificates,
timeouts, and instrumentation schema — following the corrected campaign's
manifest workflow, including its read-only derivation rule (the
status-probe mutation incident from that campaign must not recur; derivation
must not open candidate state). Independently reopen and rehash before use.
Record the manifest SHA-256.

## Step 2 — Preflight the two migration allowances

Both one-time migrations will fire inside measurement windows, and both are
now correctly keyed to per-validator first-use, not round index:

- vote-lock index migration: allowed once per validator per window restore,
  at that validator's **first vote-lock reservation**
  (`VOTE_LOCK_MIGRATION_AFTER_FIRST_VALIDATOR_RESERVATION` on violation);
- certified-send index migration: allowed once per validator per window
  restore, at that validator's **first outbox resume**
  (`CERTIFIED_SEND_INDEX_MIGRATION_AFTER_FIRST_VALIDATOR_RESUME` on
  violation).

Run the runner's focused gate suites (the 15d059d1 test set) once before the
campaign and confirm the portable-snapshot legacy sequence passes the
re-keyed contract — the defect that killed `legacy-jsonl/height-50-window-1`
must be provably resolved **before** the clock starts, not discovered again
inside it.

## Step 3 — Run the campaign

1. Fresh private run directory per naming convention
   (`postfiat-storage-g4-measurement-<runner-rev>-<candidate-rev>-v1`).
2. Fresh four-hour measurement clock starting at first measurement.
3. Execute the unchanged matrix with checkpoint/resume enabled; restore every
   window's fleet content-verified against the frozen digests.
4. On completion, package the report and checkpoint with `package_packet.py`;
   record all SHA-256 identities.

## Step 4 — Pass/fail gates

The candidate **qualifies** only if every row holds:

- [ ] height-5,000/height-50 p95 ratio ≤ 1.10 — `consensus_round_ms`;
- [ ] height-5,000/height-50 p95 ratio ≤ 1.10 — `wallet_to_finality_ms`;
- [ ] no material positive height relationship in any synchronous stage;
- [ ] **round-coverage residual gate** passes every measured round (named
      stages account for wall time within the fixed tolerance) — this is the
      gate that certifies no untimed work remains;
- [ ] selected/legacy height-50 comparison within the existing gate, with
      **all five legacy control windows completed** this time;
- [ ] literal receipts, six-validator convergence, redb bounded work, zero
      full-history reads — every window;
- [ ] vote-lock bounded work: migrations only at first reservation, once per
      validator per restore; all other votes ≤2 files / ≤ existing byte bound;
- [ ] certified-send bounded work: migrations only at first resume, once per
      validator per restore; all other resumes with zero retained-payload
      reads and counters inside the gate bounds;
- [ ] campaign inside budget or truthfully recorded otherwise.

## Step 5 — Outcomes

**On pass:**
- Update the milestone: corrected G4 **PASSED**, with the complete identity
  and gate table. The local-qualification lane advances to assembling
  locally available G5 packet material. State plainly: this is offline local
  qualification evidence — not devnet evidence, not deployment authorization,
  and G3's height-924 exact replay remains separately open.
- Write the handoff; include the full gate table and every hash.

**On fail:**
- Diagnose once from the campaign's own artifacts. The coverage gate
  guarantees the failure is attributable to a *named* stage this time; name
  the stage and its owning source, write the handoff, update the milestone,
  stop. No candidate changes, no second run under this plan.

## Explicit non-goals

- No devnet contact, deployment, or live-fleet claims.
- No height-924 copy (G3 authorization is separate).
- No candidate source changes after the freeze — none.
- No reuse of prior failed-campaign receipts as fresh evidence.
- No second campaign under this plan, pass or fail.
- The two untracked `docs/security/` auditor inventories remain untouched.

## Exit criteria

- [ ] Campaign-input manifest bound, independently rehash-verified, recorded.
- [ ] Both migration-allowance contracts verified against the
      portable-snapshot sequence before the clock started.
- [ ] Exactly one campaign executed under one fresh four-hour clock.
- [ ] Full gate table reported truthfully with all identities.
- [ ] Milestone and handoff updated; on pass, G5 assembly unblocked; on
      fail, one named-stage diagnosis and stop.
