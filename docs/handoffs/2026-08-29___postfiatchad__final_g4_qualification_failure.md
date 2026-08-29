# Final G4 qualification campaign failed closed

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

The one and only campaign authorized by the
[final G4 qualification plan](../plans/active/final-g4-qualification-campaign-plan.md)
ran once, failed closed in its first selected height-50 window, and was not
retried. The failure was
`CERTIFIED_SEND_INDEX_MIGRATION_AFTER_FIRST_VALIDATOR_RESUME`: five validators
had no outbox on their first resume, so their first possible completed-index
migration occurred on their second observed resume, while the runner permits a
migration only on observation 1. This is a candidate/runner contract mismatch,
not evidence that `redb` passed or failed the height-scaling ratios. There is no
final campaign report or packet; storage remains **SELECTED, NOT OFFLINE
QUALIFIED**, and public testnet remains blocked. The governing state is in the
[storage scaling milestone](../plans/active/storage-scaling-milestone.md).

## Current state

### Exact frozen lineage and inputs

| Boundary | Identity and result |
| --- | --- |
| Repository before this docs-only closeout | `main` at plan commit `d3da5169`, synchronized with `origin/main`; runtime candidate is an older frozen commit |
| Candidate source | `e52e050269a2f9fdd28c5083c3888debf3a85063` |
| Candidate binary | SHA-256 `6b130a1f9c81bd64bc9dc42043595f5a27e84185cf3f40b13b5f37a40d72a82e` |
| G1 candidate manifest | SHA-256 `895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe` |
| G2 safety manifest | SHA-256 `dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7` |
| Runner/verifier | Branch `postfiatchad/corrected-g4-vote-lock-gate` at `15d059d14a7bc0be046f6109be3ceb1c29f35a37`; clean and synchronized with its remote |
| Batch-builder helper | Release binary SHA-256 `e8c48700308a3a37dc81547d701c00a6bdac7dc809291e8d206dd9e3a968ec71` |
| Prepared-input manifest | SHA-256 `b5be151aca829ab275c99a890e4eae77c28ffa1a97b657a489354e6164144ce8` |
| Independent input-verification receipt | SHA-256 `1e9c753bd41f93589c6fb0b8eb7592f0679175892e6e1c3d93f0bcc23eae1aa8`; all 18 referenced files/directories rehashed exactly |

The input manifest preserved the distinction between the candidate used for
measurement and source `ae658441`, which built the unchanged prepared fleets.
The verification was standalone and did not open or mutate the frozen source
fleet.

### Preflight

The helper was rebuilt at runner `15d059d1`; 95 focused
runner/packager/verifier tests passed. Explicit tests covered vote-lock and
certified-send first validator use plus repeated/late migration rejection. Those
tests did not cover the sequence that failed in the campaign: first resume with
no outbox, deliveries creating the outbox, then first possible migration on the
second resume.

### The one run

| Fact | Recorded value |
| --- | --- |
| Private output | `~/repos/postfiat-storage-g4-measurement-15d059d1-e52e0502-v1` |
| Measurement start | `2026-08-29T03:53:59Z` |
| Failure time | `2026-08-29T03:55:05Z` |
| Measurement elapsed | 65.322754 seconds |
| Checkpoint status | `FAILED` |
| Completed campaign units | 0 |
| Failed unit | `selected-indexed/height-50-window-1` |
| Raw rounds in failed unit | 50 complete measured rounds |
| Retry count | 0; no retry is authorized |
| Surviving campaign processes | 0 |

The private output contains validator material. Preserve it. Do not commit,
publish, delete, merge, or relabel it.

### Full gate result

| Gate | Result |
| --- | --- |
| Height-5,000/height-50 consensus p95 ratio ≤1.10 | **NOT AVAILABLE**; no height-5,000 window ran |
| Height-5,000/height-50 wallet-to-finality p95 ratio ≤1.10 | **NOT AVAILABLE**; no height-5,000 window ran |
| No positive height relationship | **NOT AVAILABLE**; only one height-50 unit has raw rounds |
| Round-coverage residual | **PASS for failed unit only**; 50/50 rounds, max 68.988102 ms against 100 ms |
| Selected/legacy height-50 comparison | **NOT AVAILABLE**; no legacy window ran |
| Receipts and six-validator convergence | **PASS for failed unit only** |
| `redb` bounded work / zero full-history reads | **PASS for failed unit only**; 300 transactional commits, zero full-history records/bytes |
| Vote-lock bounded work | **PASS for failed unit only**; five allowed migrations, 245 ordinary votes at ≤2 files / ≤314 bytes |
| Certified-send bounded work and migration position | **FAIL**; `CERTIFIED_SEND_INDEX_MIGRATION_AFTER_FIRST_VALIDATOR_RESUME` |
| Four-hour budget | **PASS**; failed closed after 65.322754 seconds |
| Overall G4 | **FAIL**; an unavailable gate is never a pass |

The raw first-window p95 values were 409.031363 ms for consensus round and
422.585025 ms for wallet-to-finality. They are not scaling ratios.

### Preserved artifact identities

| Artifact | SHA-256 |
| --- | --- |
| Failed checkpoint | `f62e1bc11793795c0420a782eac0399fc99acc5dcd91fa6183e99e6a7050ac1` |
| Raw 50-round report | `793f9ae02fd9c3994217d585389a93a41a52d940f090a32ebfcdc82ccd0da3aa` |
| Certified-send gate receipt | `bb0f04bcb861be9b5fdca3e02b14185d43a43851a5e49b0ba75e90b9f0fbd969` |
| Vote-lock gate receipt | `d3c565f04c4d263bc94e1587c6730c05274c061ba6dadb634f7afce48d0d534a` |
| One diagnosis | `10606b318f77fc52ad4c8313b3d243866ee1129a394b26eeb69f34254bd01739` |

No final campaign report exists, so the packet packager was intentionally not
invoked. Partial raw output is not a packet and is not release evidence.

### One diagnosis

The validators proposed in the initial order v3, v4, v5, v0, v1, v2.
Validator 0 already had 240 tombstones, so its first resume performed the
one-time index migration and passed. Validators 1 through 5 had no outbox on
their first resumes and reported no migration or work. After each received five
certified deliveries, the outbox existed. On each validator's second observed
resume, its first possible migration ran and compacted five jobs. The runner
rejected those migrations solely because their resume-observation number was 2.

The candidate cause is the no-outbox return in
`crates/node/src/certified_send_completed_index.rs`: it accepts an absent
outbox without creating an empty index, and only calls `ensure_index` when the
outbox exists. The runner cause is
`benchmarks/storage-scaling/run_campaign.py`: it increments every resume
observation and allows migration only when that count equals 1.

### Boundaries

- Offline only. No devnet, deployment, service, fleet, validator-copy, or
  height-924 action occurred.
- No live probe was performed; this handoff makes no claim about current devnet
  state.
- No Task Node or subagents were used.
- No candidate source was changed.
- No second campaign, packet, G5 claim, or deployment action was attempted.
- Existing campaign/fleet directories were preserved.
- The main repository's rebuildable `target/` cache was cleaned to recover
  disk space before execution; source, documentation, and evidence were not
  removed.
- The unrelated untracked auditor inventories
  `docs/security/core-feature-loc-audit-inventory.md` and
  `docs/security/nav-infrastructure-repository-map.md` remain untouched and
  must not be included in this docs closeout.

## Next decision or action

Do **not** rerun the campaign. First write and review a remediation plan that
chooses one contract:

1. Candidate-owned: create and bind an empty completed-set index even when the
   outbox is absent, so migration is genuinely complete on first resume.
2. Runner-owned: allow the one migration on the first
   migration-eligible/work-bearing resume, rather than the first observed
   resume.

The review must decide which behavior is the intended durable invariant, add the
missing no-outbox/deliveries/second-resume fixture, and state which identities
must be refreshed. A candidate change requires a new source/binary freeze and
binary-sensitive evidence; a runner change requires a new runner and campaign
binding. Either path needs a new reviewed campaign plan and separate operator
authorization. This handoff authorizes neither.

G3 remains separate: remediated height-915 replay is open, and height 924 still
requires a named custodian plus separate read-only-copy authorization. Do not
wait idle for height-924 access, and do not claim `OFFLINE QUALIFIED` without a
future G4 pass and the remaining G3/G5 gates.

## References

- [Final G4 qualification plan](../plans/active/final-g4-qualification-campaign-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Certified-send remediation handoff](2026-08-29___postfiatchad__certified_send_tombstone_bounding_complete.md)
- [Corrected G4 failure handoff](2026-08-28___postfiatchad__corrected_g4_campaign_failure.md)
- [Candidate no-outbox path](https://github.com/postfiatorg/postfiatl1v2/blob/e52e050269a2f9fdd28c5083c3888debf3a85063/crates/node/src/certified_send_completed_index.rs#L1173-L1195)
- Runner gate: branch `postfiatchad/corrected-g4-vote-lock-gate` at
  `15d059d1`, `benchmarks/storage-scaling/run_campaign.py` lines 1162–1250
