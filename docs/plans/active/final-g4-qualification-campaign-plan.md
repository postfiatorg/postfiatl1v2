# Final G4 Qualification Campaign: Executable Plan

**Status:** Executed once — **FAIL**; no retry authorized
**Date:** 2026-08-29
**Baseline:** plan commit `d3da5169`; frozen candidate source `e52e0502`
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

The operator authorized **exactly one** unchanged 5+5+5 measurement campaign
through the goal for this plan. It ran once and stopped on the first window's
certified-send migration-position gate. There was no retry. The failed unit
completed all 50 measured rounds, but five validators could not create their
completed-set index until their second observed resume because they had no
outbox on their first resume. This is a candidate/runner contract mismatch,
not a `redb` scaling result. The candidate does not qualify.

## Frozen identities

| Item | Value |
| --- | --- |
| Candidate source | `e52e050269a2f9fdd28c5083c3888debf3a85063` |
| G1 candidate manifest | SHA-256 `895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe` |
| G2 safety manifest | SHA-256 `dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7` |
| Runner/verifier | branch `postfiatchad/corrected-g4-vote-lock-gate` at `15d059d14a7bc0be046f6109be3ceb1c29f35a37` in `~/repos/postfiat-storage-measurement-runner-1f478e0c` |
| Batch-builder helper | release binary SHA-256 `e8c48700308a3a37dc81547d701c00a6bdac7dc809291e8d206dd9e3a968ec71` |
| Frozen fleets | unchanged from prior campaigns; built by `ae658441` — the manifest preserves this `prepared_by` provenance distinction |
| Prepared-input manifest | SHA-256 `b5be151aca829ab275c99a890e4eae77c28ffa1a97b657a489354e6164144ce8` |
| Independent input-verification receipt | SHA-256 `1e9c753bd41f93589c6fb0b8eb7592f0679175892e6e1c3d93f0bcc23eae1aa8` |
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

**Completed:** the prepared-input manifest is `b5be151a…4ce8`; an independent
standalone pass rehashed all 18 referenced files/directories and recorded
receipt `1e9c753b…1aa8`. The frozen source fleet was not opened or mutated.

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

**Completed with a fixture gap:** 95 focused runner/packager/verifier tests
passed, including the named first-use and repeated/late-migration cases for both
indexes. The campaign exposed an unmodeled sequence: no outbox exists on first
resume, then deliveries create it before the second resume.

## Step 3 — Run the campaign

1. Fresh private run directory per naming convention
   (`postfiat-storage-g4-measurement-<runner-rev>-<candidate-rev>-v1`).
2. Fresh four-hour measurement clock starting at first measurement.
3. Execute the unchanged matrix with checkpoint/resume enabled; restore every
   window's fleet content-verified against the frozen digests.
4. On completion, package the report and checkpoint with `package_packet.py`;
   record all SHA-256 identities. This run failed before a final campaign report
   existed, so packet packaging was intentionally not invoked.

## Step 4 — Pass/fail gates

The candidate **qualifies** only if every row holds. The one authorized run
produced this final gate table:

| Gate | Result | Evidence |
| --- | --- | --- |
| Height-5,000/height-50 `consensus_round_ms` p95 ≤ 1.10 | **NOT AVAILABLE** | The first height-50 window failed before any height-5,000 window. First-window p95 was 409.031363 ms. |
| Height-5,000/height-50 `wallet_to_finality_ms` p95 ≤ 1.10 | **NOT AVAILABLE** | The first height-50 window failed before any height-5,000 window. First-window p95 was 422.585025 ms. |
| No material positive height relationship in synchronous stages | **NOT AVAILABLE** | Only one height-50 window completed its raw rounds. |
| Round-coverage residual gate passes every measured round | **PASS FOR FAILED UNIT ONLY / CAMPAIGN INCOMPLETE** | Independently recomputed 50/50 rounds passed; maximum residual was 68.988102 ms against the 100 ms limit. |
| Selected/legacy height-50 comparison; all five legacy windows | **NOT AVAILABLE** | No legacy window ran. |
| Literal receipts, six-validator convergence, bounded `redb` work, zero full-history reads | **PASS FOR FAILED UNIT ONLY / CAMPAIGN INCOMPLETE** | All 50 rounds passed correctness and convergence; zero full-history records/bytes; 300 transactional commits. |
| Vote-lock bounded-work and migration-position gate | **PASS FOR FAILED UNIT ONLY / CAMPAIGN INCOMPLETE** | Five allowed first-reservation migrations; 245 ordinary votes were ≤2 files and ≤314 bytes. |
| Certified-send bounded-work and migration-position gate | **FAIL** | `CERTIFIED_SEND_INDEX_MIGRATION_AFTER_FIRST_VALIDATOR_RESUME` for five validators. |
| Four-hour measurement budget | **PASS** | Failed closed after 65.322754 measurement seconds; the budget was not exceeded. |

Because one binding row failed and the matrix did not complete, overall G4 is
**FAIL**. Unavailable rows are not treated as passes.

## Execution result and one diagnosis

Measurement started at `2026-08-29T03:53:59Z` in the fresh private directory
`~/repos/postfiat-storage-g4-measurement-15d059d1-e52e0502-v1`. It failed
closed at `2026-08-29T03:55:05Z` after 65.322754 measurement seconds. The
checkpoint records status `FAILED`, zero completed units, and failed unit
`selected-indexed/height-50-window-1`. All 50 raw measured rounds in that unit
completed before the runner applied the binding work gate. No campaign process
survived.

| Artifact | SHA-256 |
| --- | --- |
| Failed checkpoint | `f62e1bc11793795c0420a782eac0399fc99acc5dcd91fa6183e99e6a7050ac1` |
| Raw 50-round report | `793f9ae02fd9c3994217d585389a93a41a52d940f090a32ebfcdc82ccd0da3aa` |
| Certified-send gate receipt | `bb0f04bcb861be9b5fdca3e02b14185d43a43851a5e49b0ba75e90b9f0fbd969` |
| Vote-lock gate receipt | `d3c565f04c4d263bc94e1587c6730c05274c061ba6dadb634f7afce48d0d534a` |
| One diagnosis | `10606b318f77fc52ad4c8313b3d243866ee1129a394b26eeb69f34254bd01739` |

The diagnosis is a contract mismatch. Validator 0 already had 240 completed
tombstones, so its first resume could create/migrate the completed-set index and
passed. Validators 1 through 5 had no outbox on their first observed resume.
The candidate's no-outbox path in
`crates/node/src/certified_send_completed_index.rs` returns without creating an
empty index. After each validator received certified deliveries, its outbox
existed and its second observed resume performed the first possible migration.
The runner in `benchmarks/storage-scaling/run_campaign.py` permits migration
only on observation 1, so it rejected all five otherwise legitimate first
migrations.

A later reviewed remediation plan must choose and test one contract: either
create the empty index eagerly on a no-outbox resume, or key the runner allowance
to the first migration-eligible/work-bearing resume. This plan authorizes
neither change and no second campaign.

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

- [x] Campaign-input manifest bound, independently rehash-verified, recorded.
- [x] The required migration preflight suites ran and passed before the clock;
      the campaign then proved their certified-send fixture did not model the
      no-outbox-first-resume sequence.
- [x] Exactly one campaign executed under one fresh four-hour clock.
- [x] Full gate table reported truthfully with all identities.
- [x] Milestone and handoff updated; one certified-send migration-position
      diagnosis recorded, then stopped without a candidate change or retry.
