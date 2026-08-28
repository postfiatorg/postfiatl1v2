# Storage G4 time-budget decision

- **Operator:** Domagoj (`dravlic`)
- **Date:** 2026-08-28 UTC

## BLUF

The transactional `redb` candidate has still not failed on its own merits in any
of the three G4 release attempts. Run 1 hit the harness snapshot-size cap. Run 2
spent its four hours on the per-block setup lifecycle. Run 3 reached height 5,000
and completed the first 50-round high-height window, then failed resource-sampler
validation with about 413 seconds left. A clean rebuild from height 1 has a central
estimate of about 4 hours 32 minutes against a 4-hour budget, so the current rule
cannot pass reliably without favorable filesystem timing. The product owner
proposes the amendment below for the next run unless the operator objects in his
handoff.

## Current state

### Aggregate time accounting

| Run / runner | Status and stopping point | Checkpoint elapsed (s) | Measurement (s) | Setup and overhead (s) | Completed advance rate (s/finalized height) |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 / `4f976290` | `FAILED`, height 1,550; portable snapshot cap | 3,830.610 | 307.361 | 3,523.249 | 2.165 |
| 2 / `8768866a` | `INTERRUPTED`, durable height 3,050; partial height 4,542 | 14,398.975 | 308.025 | 14,090.950 | 2.482 |
| 3 / `2091d723` | `FAILED`, height 5,000; first high window raw-only | 13,987.118 | 386.158 | 13,600.960 | 1.979 |

The three local `campaign-checkpoint.json` files have SHA-256
`3c8cc66b…13d2f`, `5c48b9b8…ef85`, and `e1ba0e16…2459`. Elapsed values
come from `elapsed_wall_seconds`; measurement sums completed-window
`resources.duration_ms` and, for run 3, the 78.023-second raw window. All other
time is setup or harness overhead. Aggregate time is monotonic
([run_paired_campaign.py:356-378](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_paired_campaign.py#L356)),
but completed units lose `current_unit` timing
([run_paired_campaign.py:380-407](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_paired_campaign.py#L380)).

Run 3's 9,892.584 advance seconds finalized 4,999 heights; run 2's
7,567.785 seconds finalized 3,049. G4B therefore improved measured execution from
2.482 to 1.979 seconds per height, or 20.3%, but did not make the aggregate fit
([milestone:459-488](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L459)).

### Run 3 unit ledger

Approximate rows use adjacent checkpoint/artifact timestamps because v3 did not
persist unit times; exact spans use `resources.duration_ms` ending at file mtime.

| Class | Unit | Start UTC | End UTC | Heights | Wall (s) |
| --- | --- | --- | --- | ---: | ---: |
| Setup | advance 1→50 | 04:51:18.318 | 04:51:46.991 | 1→50 | 28.672 |
| Setup | height-50 material and first clone ≈ | 04:51:48.239 | 04:51:58.459 | 50 | 10.220 |
| Measurement | selected height-50 window 1 | 04:51:58.459 | 04:53:00.011 | 50→100 | 61.552 |
| Measurement | selected height-50 window 2 | 04:53:06.151 | 04:54:08.072 | 50→100 | 61.921 |
| Measurement | selected height-50 window 3 | 04:54:14.158 | 04:55:15.560 | 50→100 | 61.401 |
| Measurement | selected height-50 window 4 | 04:55:21.530 | 04:56:23.124 | 50→100 | 61.594 |
| Measurement | selected height-50 window 5 | 04:56:29.122 | 04:57:30.789 | 50→100 | 61.667 |
| Setup | advance 50→1,550 | 04:57:52.329 | 05:45:50.768 | 50→1,550 | 2,878.439 |
| Setup | advance 1,550→3,050 | 05:49:11.326 | 06:39:32.789 | 1,550→3,050 | 3,021.463 |
| Setup | discarded 3,050 retry | not persisted | quarantined 06:48:36 | ≥3,050 | not persisted |
| Setup | advance 3,050→4,550 retry | 06:56:04.229 | 07:46:48.043 | 3,050→4,550 | 3,043.814 |
| Setup | advance 4,550→5,000 | 08:06:30.158 | 08:21:50.353 | 4,550→5,000 | 920.195 |
| Setup | frozen copy and height-5,000 material ≈ | 08:24:13.418 | 08:40:17 | 5,000 | 963.582 |
| Setup | window-1 fleet preparation ≈ | 08:40:17 | 08:44:22.251 | 5,000 | 245.251 |
| Measurement, unreceipted | window-1 raw rounds/report ≈ | 08:44:22.251 | 08:45:40.274 | 5,000→5,050 | 78.023 |

The run-3 checkpoint started at `2026-08-28T04:51:12Z`; its
`current_unit.started_at` is `08:40:17Z`, and `last_stop` is
`{at: 2026-08-28T08:45:40Z, type: RuntimeError}`. The last stop was neither
the 7,200-second segment timeout nor the 14,400-second aggregate budget: it was
about 7,024 seconds after quarantine and left 412.882 aggregate seconds. A budget
failure would be typed `TimeBudgetExceeded` by
[run_paired_campaign.py:403-430](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_paired_campaign.py#L403).

The first height-5,000 raw report was written at `08:45:40.274Z`; SHA-256 is
`b608829ff1bc3d4fd9128d341d732457723f257cb11dbfbaedf1ab130ea7a472`.
It passed 50 rounds through height 5,050 with convergence, exact input binding,
and accepted final receipts. Its counters recompute to 300 commits, 300 fsyncs,
20,400 page reads, 2,400 page writes, and zero full-history work. Resource samples,
the normalized report, unit receipt, and campaign report are absent.

The failure was after raw aggregation and backend-work validation, at the
foreground resource-sampling checks in
[run_campaign.py:1460-1503](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_campaign.py#L1460).
The v3 checkpoint retained only the exception type and timestamp, not the message.
G4C subsequently confirmed the sampler performed its first full directory walk
while measurement started immediately, so one short-lived foreground process
missed its required observation
([milestone:506-515](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L506)).

### Remaining-cost estimate from v3

| Missing work after reaching height 5,000 | Evidence basis | Estimate (s) |
| --- | --- | ---: |
| Frozen copy plus height-5,000 material | 08:24:13.418→08:40:17 | 963.6 |
| One high-height input clone/preflight | 08:40:17→08:44:22.251 | 245.3 |
| One 50-round high-height measurement | observed raw window | 78.0 |
| One result-fleet digest | analogous post-loop digest | 143.1 |
| Five complete height-5,000 windows | 5 × 466.3 | 2,331.7 |
| Legacy height-50 staging/import and five windows | run-3 low windows plus staging | 350.0 |
| Work after final advance | sum | 3,645.0 |
| Whole clean v3-style campaign | observed build path plus remainder | **16,345 / about 4 h 32 m** |

The central estimate exceeds 14,400 seconds by about 1,945 seconds, or 32 minutes,
before retry and filesystem variance. A bare execution allowance is roughly
4¾ hours; a realistic checkpointed ceiling is 5½ hours, or 19,800 seconds. The
legacy row does not require a new 5,000-height advance: it reuses the retained
height-50 material and runs five bounded snapshot-import windows.

### What G4C already changed

Commit `03123ca0` addresses two findings from the review and should not be
repeated as new amendment work. The sampler now blocks until its first complete
sample and propagates startup failure
([shared runner:565-592](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py#L565)).
Prepared-fleet restoration now reuses the canonical workspace, copies only
changed entries, verifies the full destination digest, and falls back to a clean
copy on mismatch
([run_campaign.py:170-254](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_campaign.py#L170)).

G4C's preflight measured a 170.387-second clean copy and a 114.859-second reset
of all six 1,813,778,432-byte database files
([milestone:506-534](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L506)). Substituting
114.859 seconds for five old 245.251-second clones saves about 652 seconds but
still estimates 15,693 seconds, or 4 h 21 m. This inference is not release
evidence and does not create reliable four-hour margin.

G4C reduces copying but retains whole-fleet passes: `directory_digest` sorts and
hashes every file; restore inventories both trees and hashes the destination;
selected results hash the fleet; and resume rehashes retained material
([digest/restore:97-238](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_campaign.py#L97),
[result:1525-1544](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_campaign.py#L1525),
[resume:760-903](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_paired_campaign.py#L760)).
JSON corpus/batch/report generation is O(rounds), not O(height); no `rsync` is used.

### Frozen height-5,000 material

Run 3's stopped six-validator fleet is bound to prepared-fleet SHA-256
`8a4618e7ea81df7d26c4547868d9941f712552fc5e8982c74bc8763909bccfeb`.
Its checkpoint binds candidate/binary, runner/helper, runner/spec hashes, six
public validator identities, topology and height-1 snapshot hashes, fleet path
and digest, null high snapshot, the 50-transfer sequence-5,000–5,049 corpus, and
scratch before/after/mutation/discard/restored-digest fields.

The runner validates exactly `validator-0` through `validator-5`, rejects
symlinks and special files, and hashes sorted relative paths plus file digests
([run_campaign.py:97-131](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_campaign.py#L97)).
Resume verifies the public inputs and every retained fleet/material/receipt
([run_paired_campaign.py:760-903](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/run_paired_campaign.py#L760)).
The packet verifier pins candidate/helper identity and the four-hour v4 profile
([storage_scaling.py:1883-1954](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/python/postfiat_rpc/storage_scaling.py#L1883)),
then checks material/corpus/fleet digest relationships
([storage_scaling.py:1964-2046](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/python/postfiat_rpc/storage_scaling.py#L1964))
and five windows, literal receipts, convergence, counters, and zero full-history
work
([storage_scaling.py:1400-1649](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/python/postfiat_rpc/storage_scaling.py#L1400)).

The fleet is therefore strong bound input material, but the current milestone
still requires one campaign from height 1 inside one aggregate four-hour budget
([milestone:538-560](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L538)). Reuse as
separate G1 build input needs the narrow rule and verifier amendment below.

## Next decision or action

### Product owner's proposed milestone amendment

1. Split G4 into a **build phase** and a **measurement phase**. The build phase
   advances height 1→5,000 through the persistent setup path under its own
   checkpointed ceiling of 5.5 hours. Its build packet records every advance
   receipt and counter, contiguous heights, the final six-node tip and state root,
   and the prepared-fleet digest. The packet contains no private material. The
   measurement phase keeps the unchanged 5+5+5 matrix, and the four-hour budget
   applies to measurement only.

2. Permit a later measurement run to consume a verified build output through
   `--prepared-input-manifest PATH`. The runner copies the private bundle locally,
   requires source and destination fleet digests to be equal, and imports the
   height-50 and height-5,000 materials without inheriting the old campaign's
   status or elapsed time. The verifier requires contiguous build receipts, zero
   full-history reads, six-validator convergence, and build-final fleet identity
   equal to every height-5,000 window's initial fleet. Run 3's frozen fleet is the
   first eligible build output only if all of its build receipts verify under this
   rule; otherwise the next clean build produces the input.

3. Persist each unit's start, end, elapsed time, and full error message in the
   checkpoint so later accounting is exact, including discarded and failed units.

4. Keep unchanged the candidate identity, byte-verified fleets, literal receipts,
   zero full-history reads, six-validator convergence, raw samples, and existing
   performance ratios. This amendment authorizes no Task Node action and no
   controlled-devnet query, copy, service action, deployment, or mutation.

This amendment applies to the next G4 execution unless the operator records an
objection or replacement decision in his handoff.

### Decisions still waiting on the operator personally

1. Name the custodian of one complete, quiescent height-924 validator directory
   and authorize its read-only copy for exact replay. G3 remains open without that
   input
   ([milestone:122-125](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L122),
   [immediate order:641-644](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L641)).

2. Confirm or change the governance decisions requested in the
   [2026-08-27 operator handoff](2026-08-27___dravlic__storage_candidate_review.md#decisions-for-the-operator):
   Dynamic UNL inside the DGA/Cobalt envelope, and Option C. Another operator's
   goal run copied those proposals into `Decisions recorded` and G7
   ([milestone:126-137](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L126),
   [G7:608-620](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/docs/plans/active/storage-scaling-milestone.md#L608)); that goal run did not
   constitute the operator's own answer. They remain open until he confirms or changes
   them.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage-scaling evidence workflow](https://github.com/postfiatorg/postfiatl1v2/blob/03123ca060eaa25618b89b35fc0add02999915d5/benchmarks/storage-scaling/README.md)
- [Storage G4 qualification checkpoint](2026-08-28___postfiatchad__storage_g4_qualification_checkpoint.md)
- [Storage G4 timeout and persistent remediation](2026-08-28___postfiatchad__storage_g4_timeout_and_persistent_remediation.md)
- [Independent storage candidate review](2026-08-27___dravlic__storage_candidate_review.md)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Storage scaling research specification](../architecture/storage-scaling-research-spec.md)
- Local timing review: `/home/postfiatchad/wow-reports/2026-08-28-g4-time-review.md`

## End of session (dravlic, 2026-08-28)

### Delivered today

- [`9efa011c`](https://github.com/postfiatorg/postfiatl1v2/commit/9efa011c) —
  [this handoff](2026-08-28___dravlic__storage_g4_time_budget_decision.md):
  three-run G4 time accounting and the product owner's proposed build/measure
  amendment in the sections above.
- [`983ca6d7`](https://github.com/postfiatorg/postfiatl1v2/commit/983ca6d7) — made the
  [deferred Dynamic UNL proposal-source milestone](../deferred-plans/dynamic-unl-proposal-source-milestone.md)
  activation-ready: prerequisites without the Task Node lock, owner slots for
  the L1 observer service and independent-operator submitter, the inference-cost
  question, `Direction recorded` with the caveat that the operator's goal run
  copied it, and the authority boundary; also updated the
  [deferred-plan index](../deferred-plans/README.md).
- [`6ac53a26`](https://github.com/postfiatorg/postfiatl1v2/commit/6ac53a26) — added the
  [L1 observer research specification](../governance/l1-observer-research-spec.md),
  the first building block of Option C's L1-native path. Its one-loop Text
  Improvement Harness score was 89.40/100 (GPT 91.60, Fable 86.60, GLM 90.00;
  five runs per lane; run group `l1-observer-research-spec`). Task Node lock is
  pending the operator's decision.
- Branch `dravlic/prepared-input-manifest` is not merged. Its five commits are
  [`64a5eca0`](https://github.com/postfiatorg/postfiatl1v2/commit/64a5eca0),
  [`dcb5a26f`](https://github.com/postfiatorg/postfiatl1v2/commit/dcb5a26f),
  [`39ba1b37`](https://github.com/postfiatorg/postfiatl1v2/commit/39ba1b37),
  [`2e2135ab`](https://github.com/postfiatorg/postfiatl1v2/commit/2e2135ab), and
  [`e1925e09`](https://github.com/postfiatorg/postfiatl1v2/commit/e1925e09)
  (8 files, +3,051/−21). The three storage-scaling Python test modules pass,
  77 tests. The [paired runner](https://github.com/postfiatorg/postfiatl1v2/blob/e1925e09/benchmarks/storage-scaling/run_paired_campaign.py)
  persists per-unit `started_at`, `finished_at`, `elapsed_seconds`, and the stop
  message in the checkpoint. It can export
  `--export-prepared-input-manifest OUT.json` from any existing campaign output,
  including `FAILED` or `INTERRUPTED`, with contiguous 1→N advances, zero
  full-history counters, and no private material. A fresh run can import it with
  `--prepared-input-manifest PATH`: candidate, binary, and helper identities must
  match; the local copy must preserve source/destination digest equality; height-50
  and top-height materials are imported; `input_mode` is
  `prepared-input-manifest`; elapsed time starts at zero; the budget covers
  measurement only; and resume re-verifies the input. The
  [packet verifier](https://github.com/postfiatorg/postfiatl1v2/blob/e1925e09/python/postfiat_rpc/storage_scaling.py)
  requires the manifest, contiguous build receipts, zero full-history build
  counters, convergence, and a build-final fleet digest equal to every top-height
  window's initial fleet. The
  [runner README](https://github.com/postfiatorg/postfiatl1v2/blob/e1925e09/benchmarks/storage-scaling/README.md#build-once-measure-separately)
  documents “Build once, measure separately.” The default path without either
  flag is unchanged. Merge is the operator's call; no campaign was run with it.
- [`ca1d532b`](https://github.com/postfiatorg/postfiatl1v2/commit/ca1d532b) — added the
  [L1 anchor-profile research specification](../governance/l1-anchor-profile-research-spec.md),
  answering open question 3: whether the bounded `payment_v2` memo lane is enough
  for Dynamic UNL announcements and commit/reveal/convergence anchors, or a
  dedicated data-carrying transaction is needed. Its one-loop harness score was
  88.67/100 (GPT 90.40, Fable 86.60, GLM 89.00; five runs per lane; run group
  `l1-anchor-profile-research-spec`). It recommends Option B: retain the memo lane
  and add canonical validation, ML-DSA-65 role attribution, receipt metadata, and
  a bounded round index, all `SHADOW_ONLY`. Task Node lock is pending.

### Client's G4 v4 run at wrap-up

At 11:32 UTC its checkpoint reported status `RUNNING`, height 1,550, 3,412
seconds elapsed of the 14,400-second budget, seven completed units, and current
unit `advance-1550-to-3050`. This was a read-only observation of the client's
checkpoint; it was not our run and was not touched.

### Boundaries

No Task Node action. The controlled devnet was not queried, copied, or touched.
No Cargo builds, benchmarks, or node processes ran from this session. The
client's tmux sessions were not used. All our commits were pushed from the
`postfiatl1v2-dravlic` worktree with `git push origin HEAD:main`.

### Decisions for the operator

1. Accept or replace the build/measure amendment above before the next G4 run.
2. Name the height-924 directory custodian and authorize its read-only copy.
3. Confirm or change the two governance directions—Dynamic UNL inside the DGA
   envelope with formula fallback, and Option C. The milestone currently records
   them from his goal run, not from him.
4. Fill the two owner slots in the deferred milestone, or say they stay empty.
5. Review and merge, or reject, branch `dravlic/prepared-input-manifest`.
6. Say whether the L1 observer and anchor-profile specifications should be locked
   (Task Node exception or lock) once Task Node is allowed again.
