# Corrected G4 Campaign: Executable Plan

**Status:** Completed — failed; no retry authorized
**Date:** 2026-08-28
**Baseline:** `main` at `442c5a4d` (vote-lock fix `be4c7f44` + handoff)
**Predecessor plans:** [Vote-lock index fix](vote-lock-index-fix-plan.md) (implemented, verified),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)
**Motivating failures:** [G4 structural vote-path failure](../../handoffs/2026-08-28___postfiatchad__storage_g4_structural_vote_path_failure.md),
[Vote-lock fix handoff](../../handoffs/2026-08-28___postfiatchad__vote_lock_index_fix.md)

## BLUF for the executing agent

The vote-lock index fix is implemented and locally verified. The question this
plan answers is the one that matters: **does the chain now stay fast as it
grows?** The prior G4 campaign failed with height-5,000/height-50 p95 ratios of
2.808 (consensus round) and 2.762 (wallet-to-finality) against a required
≤1.10, with the entire material height relationship isolated to the two
vote-lock stages that the fix bounds. Your job: freeze one corrected candidate
binary (G1), refresh the binary-bound safety evidence (G2), extend the
measurement runner to gate on the new vote-lock work counters, and run **one**
new, unchanged 5+5+5 G4 measurement campaign under a fresh four-hour clock.

Pass → the storage milestone's local-qualification lane advances toward G5.
Fail → diagnose once from the stage timing data, write the handoff, and stop.
Either way, this plan ends with a definitive, evidence-bound answer. Do not
loop, enlarge the matrix, or "retry" a failed gate.

## Execution result — 2026-08-28

**Outcome: `FAILED`; the one-run allowance is consumed. Do not resume, retry,
relabel, or modify the frozen candidate under this plan.**

The corrected vote-lock lookup worked on the selected path, but the chain still
did not stay fast as history grew. All ten selected `redb` windows completed
with exact literal receipts, six-validator convergence, zero reported
full-history storage reads, and passing vote-lock work gates. Nevertheless, the
height-5,000/height-50 p95 ratios were 2.693 for consensus round time and 2.649
for wallet-to-finality, both far above the 1.10 ceiling.

The first legacy control then failed the locked migration-position rule. Five
validators each migrated once in finalized round 2 rather than round 1. That is
a real campaign failure even though it appears to be a mismatch between the
portable-snapshot control and the runner's first-round-only allowance. The
runner stopped at `2026-08-28T22:33:12Z` after 2,092.637 seconds. No legacy
window completed, no final campaign report exists, and the qualification packet
packager was not invoked.

### Frozen identities

| Item | Corrected-run identity |
| --- | --- |
| Candidate source | `442c5a4ddafed3aa0709f64e213fe0cedac5222d` |
| Candidate binary SHA-256 | `29423cba098ce793ccab4a234ab26a2d30c6b11ad9eacd339b11b89cd6187c48` |
| G1 candidate manifest SHA-256 | `b4a580f7f4c61db4992f83f823d6715cd712589eaaada9debde3f45622f1bf01` |
| G2 safety manifest SHA-256 | `132220922f0d5b6e3728861f227e6aff4f28ad87f36fce6d78119f9e75ef78c7` |
| Runner source | `693855e3492bc3d37801653e90bc308969fbad85` |
| Batch-builder binary SHA-256 | `754c9e8600f0a5c4f05e1fab62400ef222ae7ad154cecf533d8f5df4f69a1c0d` |
| Corrected prepared-input manifest SHA-256 | `9d48530539eaf05a18879dbafb3d7c62862617c28b843ae300dc1d87ed05cb88` |
| Campaign checkpoint SHA-256 | `847b60f924414825ac050fd901bc80b3dbb200d7db6d91c74f1357fc018cd6c1` |
| Legacy failure receipt SHA-256 | `ce8703dfc16c22c3930508b314231c6992c82fa20c54bc9d9fa2254da9c98c38` |
| Private diagnosis SHA-256 | `4c7bb67b8622de967b240a6583a34bd554e9a1c7f19672ef43a06f88ef7832f8` |

The private campaign directory is
`~/repos/postfiat-storage-g4-measurement-693855e3-442c5a4d-v1`. It contains
validator private material and must never be committed, published, or deleted.
The diagnosis is stored below it at
`diagnosis/corrected-g4-failure-diagnosis.json` and contains only redaction-safe
claims, but remains private under this plan.

### Gate table

| Required gate | Evidence | Result |
| --- | --- | --- |
| Consensus p95 ratio ≤ 1.10 | 691.143 ms at height 50; 1,861.319 ms at height 5,000; ratio 2.693 | **FAIL** |
| Wallet p95 ratio ≤ 1.10 | 706.998 ms at height 50; 1,872.667 ms at height 5,000; ratio 2.649 | **FAIL** |
| No material positive height relationship | The runner's listed-stage model says true, but omits the synchronous pre-setup outbox-resume phase carrying the observed cost | **FAIL — coverage gap** |
| Selected/legacy height-50 comparison | No legacy window completed | **NOT AVAILABLE / FAIL** |
| Selected window correctness and bounded redb work | All ten windows passed literal receipts, six-validator convergence, backend work, zero full-history reads, and resource gates | **PASS** |
| Selected vote-lock bounded work | All ten windows passed; all 50 allowed migrations occurred in round 1; 2,450 later vote operations examined at most 2 files and decoded at most 314 bytes | **PASS** |
| Legacy vote-lock bounded work | Five validators migrated once in finalized round 2; stable reason `VOTE_LOCK_MIGRATION_AFTER_FIRST_FINALIZED_ROUND` | **FAIL** |
| Four-hour measurement budget | Stopped after 2,092.637 of 14,400 seconds | **PASS on time; campaign incomplete** |
| Overall candidate | A candidate passes only if every gate passes | **FAIL** |

### Single diagnosis and next owning surface

The remaining height cost is not in `redb` append and is no longer in vote-lock
lookup. It occurs in the synchronous proposer phase before `setup_start`, which
the runner does not time separately:

1. `transport_peer_certified_batch_round` starts the round clock and calls
   `resume_durable_certified_send_outbox` before named setup timing
   (`crates/node/src/transport_runtime.rs:2745-2774`).
2. Resume calls `compact_completed_durable_certified_send_jobs`
   (`crates/node/src/transport_cli.rs:2466-2477`).
3. Compaction validates every retained completed certified-send tombstone and
   then pruning validates the same set again
   (`transport_cli.rs:1848-1917,1962-2037`).
4. Each validation rereads `job.json`, `batch.json`, and `certificate.json` and
   rehashes both payloads (`transport_cli.rs:2041-2126`).

The frozen validator-0 outbox contains 240 completed tombstones/720 payload files
at height 50 and the retention cap of 1,024 tombstones/3,072 payload files at
height 5,000. In every selected window the slow proposer is validator 0 at
rounds 4, 10, 16, 22, 28, 34, 40, and 46. Validator-0 consensus p95 rises from
745.581 ms to 1,922.521 ms, while the other non-migration proposers remain below
401 ms at height 5,000. This is a high-confidence source attribution, not a
direct phase timer: the missing timing is itself part of the defect.

The next owner is therefore durable certified-send tombstone retention/resume in
`crates/node/src/transport_cli.rs`, plus explicit phase coverage in
`crates/node/src/transport_runtime.rs`. The work is proportional to retained
history until the 1,024-tombstone cap and remains expensive at the cap. This plan
does not authorize that fix or another campaign.

## Prior identities (preserve as failed-campaign lineage; do not overwrite)

| Item | Value |
| --- | --- |
| Failed candidate source | `ae65844190f153cbdd49d1e5ac28ab96a19f7af4` |
| Failed candidate binary SHA-256 | `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4` |
| Runner source | `1f478e0c473de42ecf43b4dd0925893de8f181ed` (repo `~/repos/postfiat-storage-measurement-runner-1f478e0c`) |
| Prior prepared-input manifest SHA-256 | `9ac31841a41ba514855a82f52650e1951ed97c9f99d54a4048a07407d6734c61` |
| Prior campaign report SHA-256 | `88502bca7aaa4e576e5e9684b3d9b72d8c1b66e24b6c6c8e746f11807ac7eabb` |
| Prior private run directory | `~/repos/postfiat-storage-g4-measurement-1f478e0c-ae658441-v1` — contains validator private material; never commit, publish, or delete |

## Hard rules (from the active milestone — violating any invalidates the campaign)

1. **Build outside the clock.** The four-hour budget covers measurement only.
   Freeze the binary and fleets first. Hitting the budget is a recorded
   `TIME_BUDGET_EXCEEDED` result, not permission to continue.
2. **Unchanged matrix.** Five selected-redb windows at height 50, five at
   height 5,000, five legacy-JSONL control windows at height 50; 50 finalized
   rounds per window; six validators; exact signed inputs; literal receipts.
   Run selected-path windows before the expensive legacy controls.
3. **One clean candidate.** Any change to node source or binary after the G1
   freeze restarts G1–G4. Evidence-runner-only changes are permitted and are
   hash-bound separately.
4. **Diagnose once.** A failed or timed-out gate gets one diagnosis from its
   own artifacts, a handoff, and a stop. No automatic restart, no larger
   matrix, no relabeled partial run.
5. **Offline only.** No devnet contact, no deployment, no height-924 copy
   (G3 has its own authorization), no Task Node actions, no network mutation.
6. Success per round means a valid certificate **and** matching literal
   accepted/rejected receipts **and** six-validator convergence on height,
   block hash, and state root — never elapsed time alone.

## Step 0 — Host and repository preconditions

1. **Fix the build toolchain.** `~/.local/bin/cc` is a symlink directly to the
   `zig` binary, which rejects invocation as `cc` (`error: unknown command:
   -m64`). Fresh clean-checkout builds fail. Replace the `cc` symlink with a
   wrapper script that executes `zig cc "$@"` (and translate
   `--target=x86_64-unknown-linux-gnu` to `--target=x86_64-linux-gnu`, which
   this zig build requires), or install a real gcc. Add matching `c++` and
   `ar` handling. Verify with a clean `cargo build --release -p postfiat-node`
   from a scratch checkout. Record the exact compiler identity in the build
   manifest — it is part of binary reproducibility.
2. **Push `main`.** The G1 freeze must reference a commit that exists on
   `origin/main`. Pushing `442c5a4d` requires operator authorization; obtain
   it before starting, or stop here and report. Do not commit the two
   unrelated untracked `docs/security/` inventories.

## Step 1 — G1: corrected candidate freeze

From a **clean checkout** of the pushed corrected commit:

1. Build one release `postfiat-node` binary. Record: source commit, binary
   SHA-256, embedded `build_git_revision`, Rust toolchain, `Cargo.lock`
   SHA-256, build command, compiler/linker identity, host, and storage device
   — same schema as the prior G1 record in the milestone.
2. Rerun the G1 regression set against that source: focused storage/node
   suites, the vote-lock suite (`cargo test -p postfiat-node vote_lock`),
   formatting, and warnings-denied Clippy.
3. Update the milestone's G1 row: prior lineage preserved, corrected freeze
   recorded. The new binary must not change again until G4 completes.

## Step 2 — G2: binary-bound safety refresh

The prior tamper/rollback receipts were bound to binary `891b…bf4` and are
history, not evidence. Repeat every **binary-bound** G2 check against the
corrected binary: tamper matrix, rollback, crash/restart atomicity, and
`storage-rebuild-transactional --verify-only` fail-closed behavior. Reuse the
existing G2 scripts and record fresh receipts. Design-level items that are not
binary-bound do not need repetition; mark each reused item explicitly.

## Step 3 — Runner extension: vote-lock bounded-work gate

In the runner repo (`~/repos/postfiat-storage-measurement-runner-1f478e0c`,
worktree — do not touch candidate source):

1. Parse the three new fields the fix added to
   `postfiat.block_vote_creation_timing.v1` reports:
   `vote_lock_files_examined`, `vote_lock_bytes_decoded`,
   `vote_lock_migration_performed` (serde-defaulted; absent in legacy-control
   reports — treat absent as zero/false).
2. Add a per-campaign gate, reported per window and per validator:
   - `vote_lock_migration_performed == true` at most **once per validator per
     window restore**, and only in that window's first finalized round;
   - for every other measured vote: `vote_lock_files_examined <= 3` and
     `vote_lock_bytes_decoded <= 4096`;
   - gate failure fails the window with a stable reason code.
3. Add runner unit tests for the gate (bounded pass, migration-twice fail,
   late-migration fail, oversized-bytes fail, legacy report without fields).
4. Commit in the runner repo and record the new runner source hash. Runner
   changes do not restart G1.

**Why the migration allowance exists (do not "fix" it):** the prepared fleets
were written by the old binary. Their lock directories hold 49 (height 50) or
4,999 (height 5,000) lock files, all already at derived paths, and **no index
marker**. The corrected binary's first vote per validator therefore runs the
one-time verification migration — a single bounded-by-history scan — then
writes the marker, and every later vote is O(1). This is designed behavior.
One slow first round per window does not move a 50-round p95, and the counter
gate proves the migration happened exactly once and never again. Do not
regenerate or mutate the frozen fleets to pre-write markers; their content
hashes are part of the preserved lineage.

## Step 4 — New prepared-input manifest

Follow the merged prepared-input manifest workflow (`90d68784` lineage) to
produce one new manifest binding: corrected candidate source and binary
hashes, updated runner source hash, the **unchanged** prepared height-50 and
height-5,000 fleet digests and the height-50 legacy snapshot, the canonical
batch-builder identity, corpus and certificate digests, topology, keys,
timeouts, and instrumentation schema. Record the manifest SHA-256. This
completes the open G1 "freeze topology/inputs" checklist item.

## Step 5 — Run the campaign

1. Create a fresh private run directory
   (`~/repos/postfiat-storage-g4-measurement-<runner-rev>-<candidate-rev>-v1`
   naming convention). It will contain validator private material: never
   commit or publish it.
2. Start the four-hour measurement clock only when measurement starts.
3. Execute the unchanged matrix via the runner's campaign driver
   (`run_campaign.py` / `run_paired_campaign.py` per the runner README and the
   prior campaign's checkpoint workflow), selected windows first, with
   checkpoint/resume enabled and every window's fleet restored
   content-verified from the frozen digests.
4. Package the final report and checkpoint with `package_packet.py`; record
   report and checkpoint SHA-256 values.

Execution note: item 4 was not applicable after the binding failure. The runner
created no final report, and `package_packet.py` requires a complete,
evidence-eligible campaign. Invoking it would not turn a partial failed run into
a packet. The checkpoint, legacy failure receipt, and private diagnosis hashes
above are the preserved failure evidence.

## Step 6 — Pass/fail gates

The corrected candidate **passes** only if all of the following hold:

- [ ] height-5,000 / height-50 p95 ratio ≤ 1.10 for `consensus_round_ms`;
- [ ] height-5,000 / height-50 p95 ratio ≤ 1.10 for `wallet_to_finality_ms`;
- [ ] `no_positive_linear_height_relationship` true — no synchronous stage
      shows a material positive height relationship;
- [ ] selected/legacy height-50 comparison within the existing gate;
- [ ] every window: literal receipts, six-validator convergence, bounded redb
      work counters, zero full-history storage reads (existing gates);
- [ ] the new vote-lock bounded-work gate passes in every selected window at
      both heights;
- [ ] the campaign finished inside the budget, or the result is recorded as
      `TIME_BUDGET_EXCEEDED` with only independently verifiable completed
      units preserved.

"Campaign completed" is never reported as "candidate passed." Report the gate
booleans explicitly, as the prior handoff did.

## Step 7 — Outcomes

**On pass:**
- Update the milestone: corrected G4 result, all identities, gate table;
  local-qualification lane advances to assembling locally available G5
  material. G3 remains blocked on its own authorization; deployment remains a
  separate decision. State plainly that this is an offline local result, not
  devnet or production evidence.
- Write the handoff with all hashes, commands, gates, and omissions.

**On fail:**
- Diagnose **once**, from the campaign's own stage timing reports: recompute
  the per-stage height regression (the prior campaign's method) and identify
  which stage now carries the material height relationship and whether the
  vote-lock counters stayed bounded (separating "fix didn't work" from "a
  third unbounded path exists").
- Write the handoff naming the implicated stage and the owning source file(s),
  update the milestone, and stop. Do not modify candidate source, rerun, or
  enlarge the matrix under this plan.

## Explicit non-goals

- No devnet access, deployment, or fleet mutation of any live system.
- No height-924 validator-directory copy (G3 authorization is separate).
- No candidate source changes after the G1 freeze — including "harmless" ones.
- No reuse of prior binary-bound G2/G4 receipts as fresh evidence.
- No publication of anything from private run directories.

## Exit criteria

- [x] Toolchain repaired and recorded; clean-checkout release build works.
- [x] `main` pushed with operator authorization.
- [x] G1 corrected freeze recorded (source, binary, toolchain, manifest).
- [x] G2 binary-bound receipts refreshed against the corrected binary.
- [x] Runner gate for vote-lock counters implemented, tested, hash-bound.
- [x] One new prepared-input manifest recorded.
- [x] One unchanged 5+5+5 campaign executed under one fresh four-hour clock.
- [x] Gate table reported truthfully; milestone and handoff updated; on fail,
      a single evidence-bound diagnosis names the next owning surface.
