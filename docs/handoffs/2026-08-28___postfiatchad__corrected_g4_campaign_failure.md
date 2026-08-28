# Corrected G4 campaign failure

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

The exactly one authorized corrected G4 campaign is finished and **failed**.
The vote-lock index fix worked: all ten selected `redb` windows passed the
new bounded-work gate, exact-receipt checks, six-validator convergence, and
zero-full-history-read checks. The chain still became about 2.7 times slower at
height 5,000 because a different synchronous path validates the proposer's
retained certified-send tombstones twice before every proposal. The first
legacy control also hit a binding round-2 migration-position failure. The run
stopped without retry, candidate change, devnet access, final report, or packet.
The authoritative records are the
[corrected campaign plan](../plans/active/corrected-g4-campaign-plan.md) and
[storage milestone](../plans/active/storage-scaling-milestone.md).

## Current state

### Repository, candidate, and authority boundary

- The runtime candidate is source
  `442c5a4ddafed3aa0709f64e213fe0cedac5222d` and release binary SHA-256
  `29423cba098ce793ccab4a234ab26a2d30c6b11ad9eacd339b11b89cd6187c48`.
  It remained byte-identical from G1 through the failed campaign.
- At documentation close, the repository branch is `main`; its pre-handoff
  source/docs baseline was `442c5a4d`, synchronized with `origin/main`.
  This handoff and the two updated plans form a docs-only successor commit; no
  runtime source changes are included.
- Runner changes are committed in the separate local runner worktree on branch
  `postfiatchad/corrected-g4-vote-lock-gate` at
  `693855e3492bc3d37801653e90bc308969fbad85`. They are not merged into the L1
  repository. Their source hash is bound by the prepared manifest and
  checkpoint.
- No Task Node, parallel-agent workflow, controlled-devnet query, height-924
  copy, service action, deployment, network mutation, or live fleet probe was
  performed. This handoff makes no claim about what is running now; see
  [Current State](../status/chain-state-current.md) for dated network evidence.
- The two unrelated untracked auditor inventories under `docs/security/` were
  preserved and excluded from the handoff commit.

### Completed prerequisites

| Boundary | Result and identity |
| --- | --- |
| Toolchain | Local `cc`, `c++`, and `ar` wrappers invoke Zig correctly; a clean-checkout release build passed |
| G1 corrected freeze | PASS; manifest SHA-256 `b4a580f7f4c61db4992f83f823d6715cd712589eaaada9debde3f45622f1bf01` |
| G2 corrected safety refresh | PASS locally; manifest SHA-256 `132220922f0d5b6e3728861f227e6aff4f28ad87f36fce6d78119f9e75ef78c7` |
| G2 rollback | PASS; report SHA-256 `1a05fe1132c4993b52171288848f03b524230c085934cfe0b4b68fa1cc359970` |
| G2 tamper/crash | PASS, 69 cases; report SHA-256 `95c723e2f054f3feafd020cfa4e8116388b4105d89ca5b7bc1b6488912541f07` |
| Runner vote-lock gate | PASS, 84 focused tests; runner `693855e3` |
| Batch-builder helper | SHA-256 `754c9e8600f0a5c4f05e1fab62400ef222ae7ad154cecf533d8f5df4f69a1c0d` |
| Corrected prepared manifest | PASS after independent reopen/rehash; SHA-256 `9d48530539eaf05a18879dbafb3d7c62862617c28b843ae300dc1d87ed05cb88` |

The corrected prepared manifest deliberately distinguishes two identities. The
frozen height-50 and height-5,000 fleets were built earlier by candidate
`ae658441`; the corrected candidate `442c5a4d` measured those unchanged
fleets. The manifest's `prepared_by` section binds the old build provenance,
while its top-level candidate, runner, helper, and G1 manifest fields bind the
corrected measurement. Rewriting the fleet provenance as though the corrected
binary had built it would have been false.

### Prepared-manifest recovery incident

The first derivation implementation used candidate `status` to inspect the
private frozen seed. A second whole-bundle digest caught that this was not
read-only: it changed exactly `postfiat-state-v1.redb`.

- The changed file was preserved privately under
  `~/repos/postfiat-storage-g4-status-probe-recovery-2c562eeb`, SHA-256
  `b19d36f15fce49eceb57f571a7e2e23f79c18c13967e410103e1fe887924fac6`.
- The frozen source file was restored byte-for-byte from a separately imported,
  already verified copy.
- The complete private source bundle returned to its expected SHA-256
  `fbb74d2352ac7a60058ac845dd1d4968ef07aa32ec9bf27278295996d3013a54`.
- Runner commit `693855e3` now binds the already-frozen G1 manifest and does
  not open candidate state during derivation.
- The campaign used the restored, digest-verified fleet. The mutated recovery
  file, private bundle, validator data, and keys were not committed, published,
  or deleted.

### The one corrected campaign

The private run is
`~/repos/postfiat-storage-g4-measurement-693855e3-442c5a4d-v1`. It contains
validator private material and must never be committed, published, or deleted.

| Item | Recorded value |
| --- | --- |
| Start | `2026-08-28T21:58:19Z` |
| Stop/failure | `2026-08-28T22:33:12Z` |
| Aggregate measurement time | 2,092.637 seconds of the 14,400-second budget |
| Completed units | 10: five selected height-50 and five selected height-5,000 windows |
| Completed selected rounds | 500 |
| Failed unit | `legacy-jsonl/height-50-window-1` |
| Checkpoint SHA-256 | `847b60f924414825ac050fd901bc80b3dbb200d7db6d91c74f1357fc018cd6c1` |
| Failure reason | `VOTE_LOCK_MIGRATION_AFTER_FIRST_FINALIZED_ROUND` |
| Legacy failure receipt SHA-256 | `ce8703dfc16c22c3930508b314231c6992c82fa20c54bc9d9fa2254da9c98c38` |
| Private diagnosis SHA-256 | `4c7bb67b8622de967b240a6583a34bd554e9a1c7f19672ef43a06f88ef7832f8` |
| Final report | None |
| Qualification packet | None; `package_packet.py` was not invoked |
| Surviving campaign processes | Zero |

The packager was intentionally not invoked. It requires a complete
evidence-eligible report, while this runner failed before any legacy window or
final report completed. The checkpoint, failure receipt, and diagnosis are the
preserved evidence; no partial run was relabeled into a packet.

### Gate results

| Gate | Height 50 | Height 5,000 | Ratio/state | Result |
| --- | ---: | ---: | ---: | --- |
| Consensus-round p95 | 691.143 ms | 1,861.319 ms | 2.693, required ≤1.10 | **FAIL** |
| Wallet-to-finality p95 | 706.998 ms | 1,872.667 ms | 2.649, required ≤1.10 | **FAIL** |
| Selected exact receipts and convergence | 5/5 windows | 5/5 windows | six validators | **PASS** |
| Selected redb bounded work | 5/5 windows | 5/5 windows | zero full-history reads | **PASS** |
| Selected vote-lock work | 5/5 windows | 5/5 windows | after migration: at most 2 files/314 bytes | **PASS** |
| Legacy comparison | — | — | no completed legacy window | **NOT AVAILABLE / FAIL** |
| Budget | — | — | 2,092.637 ≤14,400 seconds | **PASS on time; incomplete** |
| Overall candidate | — | — | every gate required | **FAIL** |

Every selected window restored the frozen fleet and permitted one first-round
migration per participating validator. Across the ten windows there were 50
allowed migrations. The other 2,450 observed vote operations examined no more
than two paths and decoded no more than 314 bytes. This proves the vote-lock
index fix worked in the selected measurement path.

The legacy control observed 50 finalized rounds, but validators 0, 1, 2, 3,
and 5 each performed their one migration in round 2. Validator 4 performed no
migration, and no validator migrated twice. This likely exposes a mismatch
between portable-snapshot control setup and the runner's first-round-only
migration model, but it remains a binding failure for this campaign. It may not
be waived or retried retroactively.

The runner's existing listed-stage model would say
`no_positive_linear_height_relationship: true`, because all stages it models
remain roughly flat. That boolean is insufficient: the full synchronous round
still fails both ratio gates, and the actual cost occurs in a pre-setup phase
the model does not time. There is no final report carrying a pass boolean.

### Single failure diagnosis

The recurring slow rounds are 4, 10, 16, 22, 28, 34, 40, and 46 in every
selected window. They are exactly the rounds whose `source_node` is
`validator-0`.

| Proposer group | Height-50 consensus p95 | Height-5,000 consensus p95 |
| --- | ---: | ---: |
| Validator 0 | 745.581 ms | 1,922.521 ms |
| Other non-migration proposers | below 387 ms | below 401 ms |

The named stages do not explain the increase: proposal p95 changes
27.712→29.627 ms, vote requests 68.891→67.464 ms, local vote
40.187→41.534 ms, certificate 165.038→165.114 ms, local apply
33.668→33.188 ms, and certified sends 79.761→81.990 ms.

The missing interval has a concrete source owner:

1. The round clock starts and
   `resume_durable_certified_send_outbox` runs before `setup_start`
   ([transport_runtime.rs at candidate 442c5a4d](https://github.com/postfiatorg/postfiatl1v2/blob/442c5a4ddafed3aa0709f64e213fe0cedac5222d/crates/node/src/transport_runtime.rs#L2742-L2774)).
2. Resume calls completed-job compaction
   ([transport_cli.rs](https://github.com/postfiatorg/postfiatl1v2/blob/442c5a4ddafed3aa0709f64e213fe0cedac5222d/crates/node/src/transport_cli.rs#L2466-L2477)).
3. Compaction validates the full completed set, then pruning validates the same
   set again
   ([validation and pruning](https://github.com/postfiatorg/postfiatl1v2/blob/442c5a4ddafed3aa0709f64e213fe0cedac5222d/crates/node/src/transport_cli.rs#L1848-L2037)).
4. Each validation reads `job.json`, `batch.json`, and
   `certificate.json` and hashes both payloads
   ([payload validation](https://github.com/postfiatorg/postfiatl1v2/blob/442c5a4ddafed3aa0709f64e213fe0cedac5222d/crates/node/src/transport_cli.rs#L2041-L2126)).

The frozen validator-0 outbox has:

| Prepared height | Completed tombstones | Payload files under `completed/` | All outbox files |
| --- | ---: | ---: | ---: |
| 50 | 240 | 720 | 735 |
| 5,000 | 1,024 | 3,072 | 3,087 |

The tombstone count is capped at 1,024. The path is therefore proportional to
retained history until that cap, not unbounded forever, but it remains
synchronously expensive at the cap. The timing/source/filesystem match is
high-confidence attribution, not a direct timer measurement. Adding that
missing phase timer is part of the next fix's evidence requirement.

This diagnosis was performed once from the failed campaign. No candidate source
was changed and no second run was started.

### Verification performed

| Command or check | Result |
| --- | --- |
| `cargo build --release -p postfiat-node --locked` from clean corrected checkout | PASS |
| `cargo test -p postfiat-storage --locked` | PASS: 83, 2 intentionally ignored |
| `cargo test -p postfiat-node vote_lock --locked` | PASS: 15, 1 manual release test ignored |
| 5,000-lock release spot check | PASS: 2 files, 312 bytes, no migration |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | PASS |
| Corrected G2 tamper/crash, rollback, and verify-only checks | PASS |
| Runner focused unit suites | PASS: 84 |
| Prepared manifest independent reopen/hash verification | PASS |
| Campaign checkpoint and diagnosis internal hash verification | PASS |
| Post-failure process check | PASS: no campaign validator process remained |

## Next decision or action

Do not resume or retry this campaign, do not modify the frozen candidate under
the corrected G4 plan, and do not deploy it.

If storage work continues, the next bounded action is to write and review a
**new** plan for the certified-send tombstone resume/retention owner. That plan
must require:

1. preserved durable-delivery, acknowledgement, quarantine, restart, crash,
   tamper, and fail-closed semantics;
2. no full retained-set validation on every proposal—startup/recovery validation
   and bounded per-job checks must have explicit authority and invariants;
3. a direct phase timer and work counters for outbox resume, compaction,
   validation, and pruning;
4. executable gates with 0, 240, and 1,024 completed tombstones and source-node
   rotation;
5. a separately resolved contract for the legacy portable-snapshot round-2
   migration rather than a retroactive waiver;
6. a new candidate source/binary freeze and refreshed binary-sensitive G1/G2/G3
   evidence after any source change; and
7. separate authorization before any later performance campaign.

G3 remains independently open. A named custodian and explicit authorization are
still required before any read-only height-924 validator-directory copy. Do not
wait on that input, contact the devnet, collect six fleet directories, build G5,
or begin deployment work from this failed result.

## References

- [Corrected G4 campaign plan](../plans/active/corrected-g4-campaign-plan.md)
- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Vote-lock index implementation plan](../plans/active/vote-lock-index-fix-plan.md)
- [Vote-lock fix handoff](2026-08-28___postfiatchad__vote_lock_index_fix.md)
- [Pre-fix G4 structural failure](2026-08-28___postfiatchad__storage_g4_structural_vote_path_failure.md)
- [Storage scaling implementation contract](../architecture/storage-scaling-fix-spec.md)
- [Current recorded network state](../status/chain-state-current.md)
