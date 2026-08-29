# Certified-send bounding plan authored and ready for execution

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

The corrected G4 campaign failure is diagnosed, accepted, and answered with a
new executable plan:
[Certified-send tombstone bounding](../plans/active/certified-send-tombstone-bounding-plan.md).
This session was planning and verification only: the failure attribution was
independently re-confirmed against candidate source, the remaining round-path
scan surface was swept and classified, and no runtime source, frozen fleet,
private run directory, or devnet state was touched. The frozen candidate
`442c5a4d` remains unchanged. Plan execution and any later performance
campaign each remain separately authorized actions.

## Current state

### Where the storage-scaling effort stands

Three height-scaling defects have now been found in sequence, each hiding
behind the previous one:

1. **Storage append path** — fixed by the transactional `redb` candidate;
   proven bounded in both G4 campaigns (zero full-history reads, all
   selected-window gates passing).
2. **Vote-lock full-directory scan** — fixed at `be4c7f44`; proven in the
   corrected campaign: 2,450 measured votes examined ≤2 files / ≤314 bytes
   each.
3. **Certified-send tombstone re-validation** — the open defect. Before every
   proposal, the proposer validates its full retained completed-tombstone set
   twice (up to 1,024 tombstones, three files and two payload hashes each),
   inside a round phase the timing model does not time. Diagnosed once from
   the failed corrected campaign per its plan's diagnose-once rule.

The decisive supporting fact from the corrected campaign: rounds proposed by
the five validators without a large outbox were already height-flat
(<387 ms at height 50, <401 ms at height 5,000). The entire 2.693/2.649
ratio failure is attributable to the validator-0 proposal path.

### Verification performed this session

| Check | Result |
| --- | --- |
| Failure-handoff attribution re-read against source at `442c5a4d` | Confirmed: `resume_durable_certified_send_outbox` (`transport_cli.rs` ~2466) runs before `setup_start` in `transport_peer_certified_batch_round` (`transport_runtime.rs` ~2742–2775); compaction validates the full completed set (~1969) and pruning validates it again (~1990); per-tombstone validation reads `job.json`, `batch.json`, `certificate.json` and hashes both payloads (~2105) |
| `read_dir` sweep over round-adjacent node sources | 13 sites: certified-send outbox cluster (the diagnosed owner), one capped entry-count in `handle_transport_block_proposal_line` (bounded by `take(cap)`, no decoding), two loop-file listers pending classification (plan Step 5), benchmark tooling |
| Vote-lock fix status | Unchanged and proven; no regression claim made or needed this session |

### The authored plan

`docs/plans/active/certified-send-tombstone-bounding-plan.md` requires, in
one bounded change-set: a durable completed-set index with one-time migration
and an explicit repair command (full-set validation authority relocates from
every-proposal to migration/repair/touched-entries — a deliberate, documented
change); per-proposal work bounded by pending jobs; a direct
`outbox_resume_ms` phase timer plus a runner round-coverage residual gate so
no phase can sit untimed again; the runner migration-allowance contract
re-keyed to each validator's first reservation/resume after restore
(resolving the legacy-control round-2 failure without waiving it); a complete
round-path scan classification; eleven named test scenarios including
0/240/1,024-tombstone bounds and proposer rotation; then a new G1 freeze and
binary-bound G2 refresh. The plan stops at the freeze: the next 5+5+5
campaign is not authorized by it.

Working estimate communicated to the operator: roughly 5–8 agent-hours
(2–3 goal runs) to the corrected freeze; a subsequently authorized campaign
is approximately a further 2–2.5 hours with ~35 minutes of measurement.

### Untouched boundaries

- Frozen fleets, prior private run directories
  (`postfiat-storage-g4-measurement-*`), and validator material: untouched.
- The two unrelated untracked auditor inventories under `docs/security/`:
  untouched and uncommitted.
- No devnet access, deployment, Task Node action, height-924 copy, or
  network mutation.
- Candidate source `442c5a4d` and binary
  `29423cba098ce793ccab4a234ab26a2d30c6b11ad9eacd339b11b89cd6187c48`:
  unchanged.

## Next decision or action

1. **Operator:** authorize IC execution of the certified-send bounding plan
   (local code, tests, runner worktree, docs only — no campaign).
2. **After the fix freezes:** separately authorize exactly one new unchanged
   5+5+5 measurement campaign following the corrected-G4 plan pattern with
   new identities and the new coverage gates.
3. **Independent:** G3 still awaits a named custodian and explicit
   authorization for the read-only height-924 validator-directory copy. Do
   not block local work on it.
4. Push of this docs-only commit to `origin/main` requires the standing
   operator authorization for documentation pushes; nothing in this commit
   changes runtime behavior.

## References

- [Certified-send tombstone bounding plan](../plans/active/certified-send-tombstone-bounding-plan.md)
- [Corrected G4 campaign failure](2026-08-28___postfiatchad__corrected_g4_campaign_failure.md)
- [Corrected G4 campaign plan](../plans/active/corrected-g4-campaign-plan.md)
- [Vote-lock index fix handoff](2026-08-28___postfiatchad__vote_lock_index_fix.md)
- [Vote-lock index fix plan](../plans/active/vote-lock-index-fix-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Current recorded network state](../status/chain-state-current.md)
