# Remediated G4 Qualification Campaign: Executable Plan

**Status:** Reviewed and authorized for exactly one execution — Steps 1–2 passed; measurement not started
**Date:** 2026-08-29
**Planning baseline:** `main` at `24edd8fa`; frozen candidate source `a92bb085`
**Predecessors:** [Final G4 qualification plan](final-g4-qualification-campaign-plan.md) (closed, failed — mechanics of this plan follow it),
[Certified-send eager-index remediation spec](certified-send-eager-index-remediation-spec.md) (implemented, verified),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)
**Key handoffs:** [Eager-index remediation complete](../../handoffs/2026-08-29___postfiatchad__certified_send_eager_index_remediation_complete.md),
[Final G4 qualification failure](../../handoffs/2026-08-29___postfiatchad__final_g4_qualification_failure.md)

## BLUF for the executing agent

Every defect any G4 campaign has ever surfaced is now fixed and locally
verified at the frozen identities below:

1. **`redb` bounded work** — proven across two campaigns: bounded transactional
   commits, zero full-history reads.
2. **Vote-lock index scan** — fixed at `442c5a4d`, proven in-campaign: 2,450
   votes at ≤2 files / ≤314 bytes.
3. **Certified-send tombstone resume** — fixed at `e52e0502`, re-verified at
   `a92bb085`: 2.064 ms resume at the 1,024-tombstone cap, zero retained
   payload reads.
4. **Certified-send migration position** — fixed node-side at `a92bb085`: a
   validator with no outbox now writes and binds an empty completed-set index
   on its first successful resume, so migration always lands on observation 1.
   The runner gate logic is byte-identical to `15d059d1`; runner `a3c7bea9`
   adds only the previously missing no-outbox→deliveries→resume fixture.

The round-coverage residual gate makes unattributed time itself a failure, so
no fifth defect can hide untimed. This plan requests **exactly one** unchanged
5+5+5 measurement campaign. It may pass or fail on real scaling ratios; either
result is final under this plan. No retry, no matrix change, no relabeling.

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
- [x] Confirm the portable-snapshot legacy sequence passes the re-keyed
      vote-lock contract in the preflight suite; the legacy lane must not
      discover a contract failure inside the clock again.

**Completed:** the runner, packager, and verifier suite passed 96 tests in
22.556 seconds. It includes per-validator first use after restore, eager empty
migration followed by compaction, and repeated/late-migration rejection. The
frozen candidate passed 15 completed-index tests and 35 certified-send tests;
each suite ignored only the intentional manual release spot check. Both source
and runner worktrees remained clean.

## Step 3 — Run the campaign

1. Fresh private run directory
   `~/repos/postfiat-storage-g4-measurement-a3c7bea9-a92bb085-v1`.
2. Fresh four-hour measurement clock starting at first measurement; expected
   duration ~55–60 minutes based on the corrected campaign's 3,311-second
   full matrix.
3. Execute the unchanged matrix with checkpoint/resume enabled; restore every
   window's fleet content-verified against the frozen digests.
4. On completion, package the report and checkpoint with `package_packet.py`;
   record all SHA-256 identities. On failure, do not invoke the packager;
   partial raw output is not a packet and is not release evidence.

## Step 4 — Pass/fail gates

The candidate **qualifies** only if every row holds. Unavailable rows are
never passes.

| Gate | Threshold | Result |
| --- | --- | --- |
| Height-5,000/height-50 `consensus_round_ms` p95 ratio | ≤ 1.10 | — |
| Height-5,000/height-50 `wallet_to_finality_ms` p95 ratio | ≤ 1.10 | — |
| No material positive height relationship in synchronous stages | Required | — |
| Round-coverage residual gate | Every measured round; residual < 100 ms | — |
| Selected/legacy height-50 comparison | All five legacy windows complete and compare | — |
| Literal receipts, six-validator convergence, bounded `redb` work, zero full-history reads | Every round | — |
| Vote-lock bounded-work and migration-position gate | First-reservation only; ≤2 files / ≤314 bytes ordinary votes | — |
| Certified-send bounded-work and migration-position gate | First-resume only, including zero-work empty-index migrations; `validated == compacted + pruned` on ordinary resumes | — |
| Four-hour measurement budget | Not exceeded | — |

## Step 5 — Outcomes

**On pass:**

- Update the milestone: G4 **PASSED** with the complete identity and gate
  table. The local-qualification lane advances to assembling locally
  available G5 packet material. State plainly: this is offline local
  qualification evidence — not devnet evidence, not deployment authorization;
  G3's remediated height-915 replay (binary `902773e0…e182`) and the
  separately authorized height-924 replay remain open and still block
  `OFFLINE QUALIFIED`.
- Write the handoff; include the full gate table and every hash.

**On fail:**

- Diagnose once from the campaign's own artifacts. The coverage gate
  guarantees the failure is attributable to a named stage; name the stage and
  its owning source, write the handoff, update the milestone, stop. No
  candidate changes, no second run under this plan.

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
- [x] Step-2 preflight suites passed outside the clock, including both
      no-outbox fixtures and the legacy-lane vote-lock contract.
- [ ] Exactly one campaign executed under one fresh four-hour clock.
- [ ] Full gate table reported truthfully with all identities.
- [ ] Milestone and handoff updated; on fail, exactly one named-stage
      diagnosis recorded, then stopped without candidate change or retry.
