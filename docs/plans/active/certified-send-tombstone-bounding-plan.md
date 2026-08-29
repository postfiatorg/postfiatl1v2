# Certified-Send Tombstone Bounding: Executable Implementation Plan

**Status:** Complete — implementation, verification, G1 source/binary freeze, and G2 refresh passed; campaign authorization pending
**Date:** 2026-08-29
**Planning baseline:** `main` at `d769f6c6` (candidate source `442c5a4d` + campaign-close docs)
**Implemented source:** `e52e050269a2f9fdd28c5083c3888debf3a85063` (`origin/main` at freeze)
**Runner source:** `15d059d1` on `postfiatchad/corrected-g4-vote-lock-gate`
**Motivating failure:** [Corrected G4 campaign failure](../../handoffs/2026-08-28___postfiatchad__corrected_g4_campaign_failure.md)
**Predecessors:** [Vote-lock index fix plan](vote-lock-index-fix-plan.md) (implemented, proven in-campaign),
[Corrected G4 campaign plan](corrected-g4-campaign-plan.md) (closed, failed),
[Storage scaling milestone](storage-scaling-milestone.md) (governing gates)

## BLUF for the executing agent

The corrected G4 campaign proved the vote-lock fix (2,450 votes, ≤2 files /
≤314 bytes each) and proved rounds proposed by five of six validators are
already height-flat (<387 ms at height 50, <401 ms at height 5,000). The
entire remaining 2.7× failure is one path: before **every** proposal, the
proposer re-validates its complete retained certified-send tombstone set —
reading `job.json`, `batch.json`, `certificate.json` and hashing both payloads
per tombstone, capped at 1,024 tombstones, and it does the full pass **twice**
(compaction validates the set, then pruning validates it again). At the cap
that is ~3,072 file reads plus 2,048 payload hashes per proposal, all in a
phase the round timing model does not even time.

Your job, in one bounded change-set:

1. make per-proposal certified-send resume work bounded by *pending* jobs, not
   by retained history, behind a durable completed-set index with one-time
   migration — the same shape as the vote-lock fix;
2. give the formerly invisible phase a direct timer and work counters, and add
   a round-coverage residual gate so no phase can hide untimed again;
3. resolve the runner's legacy-control migration-round contract that failed
   `legacy-jsonl/height-50-window-1`;
4. re-freeze and refresh binary-bound evidence.

A new performance campaign is **not** part of this plan and requires separate
operator authorization.

## Code map (all at candidate `442c5a4d`; use `rg` to re-locate if drifted)

All certified-send outbox logic lives in `crates/node/src/transport_cli.rs`;
the per-round call site is in `crates/node/src/transport_runtime.rs`.

| Item | Location |
| --- | --- |
| Constants: `CERTIFIED_SEND_OUTBOX_DIR`, `CERTIFIED_SEND_OUTBOX_MAX_JOBS = 1_024`, `CERTIFIED_SEND_COMPLETED_TOMBSTONE_MAX_JOBS = 1_024`, job/quarantine schemas, staging prefix, retention dir | `transport_cli.rs:33–42` |
| `validate_certified_send_disposable_job_dir` / `remove_certified_send_disposable_job_dir` | ~1115 / ~1209 |
| `enqueue_durable_certified_send_job` | ~1694 |
| `cleanup_certified_send_completed_retention_dir` | ~1821 |
| `validated_completed_durable_certified_send_jobs` — **the full-set scan** (read_dir, canonical-name check, symlink refusal, `job.json` read, per-job payload validation, deterministic sort) | ~1848–1911 |
| `prune_completed_durable_certified_send_jobs` — calls the full-set scan **again**, prunes over-cap through the retention dir with fsync ordering | ~1912–1961 |
| `compact_completed_durable_certified_send_jobs` — orphan-staging cleanup → retention cleanup → **full-set validation #1** → move newly completed jobs (validate each, quarantine on failure, fsync dirs) → prune (**full-set validation #2**) | ~1962–2040 |
| `validate_completed_durable_certified_send_job` — reads `batch.json` + `certificate.json`, hashes both against the job record | ~2105–2126 |
| `resume_durable_certified_send_outbox` — calls compaction, then scans outbox for pending jobs | ~2466–2530 |
| Round call site: `transport_peer_certified_batch_round` calls resume **before `setup_start`** (hence untimed) and refuses a new proposal while pending > 0 | `transport_runtime.rs:~2742–2775` |

The failure fingerprint from the campaign: slow rounds are exactly the
validator-0-proposed rounds (4, 10, 16, 22, 28, 34, 40, 46 in every window);
validator-0's frozen outbox holds 240 tombstones / 720 payload files at height
50 and 1,024 / 3,072 at height 5,000.

## Hard invariants — violating any of these is a failed execution

1. **Durable-delivery semantics are unchanged.** A proposer with pending
   (incomplete) certified-send jobs must still refuse to propose. Completion,
   acknowledgement, quarantine, staging-orphan cleanup, retention-dir
   handling, canonical job-ID names, symlink refusal, size bounds
   (`CERTIFIED_SEND_JOB_MAX_BYTES`), fsync ordering on move/prune, and
   crash-restart recovery all keep their current behavior.
2. **Fail closed, at explicit authority points.** Malformed or tampered
   completed tombstones must still be detected and must still quarantine /
   error — but the *authority* for full-set validation moves from
   every-proposal to: (a) one-time index migration, (b) explicit
   startup/repair, (c) the specific entries touched by compaction or pruning.
   This authority relocation is the deliberate, documented change of this
   plan; nothing else about tamper handling may weaken. State it in the
   handoff in exactly these terms.
3. **No retained-set enumeration with per-entry decoding in the proposal
   path.** After the index exists, per-proposal resume work is bounded by
   pending jobs plus O(1) index reads — independent of tombstone count at 0,
   240, and 1,024.
4. **Replay protection is unchanged.** Job-ID uniqueness checks in
   `enqueue_durable_certified_send_job` (outbox, completed, quarantine
   collision behavior) must behave identically, including against pruned
   IDs to whatever extent they do today — verify and preserve, do not extend
   or reduce.
5. **No consensus-byte changes.** The outbox is node-local durable state.
   Certificates, receipts, batches, and signing bytes are untouched. This
   keeps the change outside protocol-version territory, like the vote-lock
   fix.
6. **Determinism discipline.** Deterministic ordering everywhere an order is
   observable (the existing height-then-job-id sort is the model). No time,
   randomness, or unordered iteration in any decision.

## Step 1 — Durable completed-set index

New module or clearly bounded section in `transport_cli.rs` (extract a
`certified_send_outbox.rs` module if the file is near the 5,000-line ceiling —
check first).

**Index file:** `<outbox>/completed/.completed_index_state.v1` — no `.json`
extension, so any existing name-canonicality scan ignores it (verify
`certified_send_job_id_is_canonical` rejects it and that
`validated_completed_durable_certified_send_jobs` skips or is taught to skip
non-directory entries — today it errors on non-canonical names; the index file
must not trip it during migration or mixed-binary operation. If an old binary
would fail on seeing this file, place the index at
`<outbox>/.completed_index_state.v1` — outside `completed/` — instead. Decide
by reading the code, and record the decision in the handoff).

**Index content (JSON, atomic-write):**
- schema `postfiat.certified_send_completed_index.v1`;
- binding: chain/topology identity to the same degree job records bind it;
- ordered entries: `(block_height, job_id, job_json_sha256, batch_sha256,
  certificate_sha256)` in the existing height-then-job-id order;
- entry count and a self-checksum over the canonical entry encoding.

**Mutation rules:**
- When compaction moves a newly completed job into `completed/`: validate
  **that job only** (existing per-job validation), append its entry, rewrite
  the index atomically, fsync per the existing ordering discipline.
- When pruning: use the index's order to select the over-cap oldest entries,
  validate **only those entries** before removal (preserving the current
  validate-before-remove property), remove them via the existing
  retention-dir flow, drop them from the index, rewrite atomically.
- Crash between file mutation and index rewrite must be recoverable: on next
  mutation (or repair), a bounded reconciliation detects index/directory
  divergence and fails closed into repair rather than guessing. Divergence
  detection may count directory entries (readdir without decoding — cheap at
  ≤1,024) but must not read payloads.

**One-time migration:** when no index exists, run exactly the current
full-set validation once (it is the existing
`validated_completed_durable_certified_send_jobs`), build the index from the
result, write it atomically. Resumable and idempotent: a crash before the
index write just re-runs migration. Migration failure (any invalid tombstone)
preserves today's quarantine/error behavior.

**Repair authority:** add an explicit operator subcommand (e.g.
`certified-send-outbox verify`) that performs the full-set validation and
index rebuild on demand. This — plus migration — is where full-history
validation now lives.

## Step 2 — Bound the per-proposal path

In `resume_durable_certified_send_outbox`:

1. Replace the unconditional `compact_completed_durable_certified_send_jobs`
   call with: ensure-index (migrate once if absent), then compaction that
   validates only newly completed jobs and prunes only via the index, per
   Step 1. The orphan-staging and retention-dir cleanups stay (they are
   bounded — verify and state their bounds in the handoff).
2. The pending-job scan (the actual purpose of resume) stays as-is; it is
   bounded by active jobs and by `CERTIFIED_SEND_OUTBOX_MAX_JOBS`.
3. Return a work report: `tombstones_validated`, `files_read`,
   `bytes_hashed`, `index_migration_performed`, plus elapsed ms for resume
   total, compaction, validation, and prune phases.

## Step 3 — Timers, counters, and the coverage gate

1. **Node side:** `transport_peer_certified_batch_round` currently starts
   `round_start`, runs resume untimed, then sets `setup_start`. Add an
   explicit `outbox_resume_ms` (and the Step-2 work counters) to the round
   report schema, serde-defaulted for backward compatibility, so the formerly
   invisible phase is a named stage.
2. **Runner side** (separate runner worktree, branch from `693855e3`):
   - parse the new fields (absent = zero);
   - add a bounded-work gate: `index_migration_performed` at most once per
     validator per window restore and only on that validator's **first
     resume**; all other resumes: `tombstones_validated == 0` on the
     completed set beyond compacted/pruned entries, `files_read` and
     `bytes_hashed` under fixed bounds sized from the 0/240/1,024 executable
     tests;
   - add the **round-coverage residual gate**: sum of named stage timings must
     account for the measured round wall time within a fixed tolerance (size
     the tolerance from height-50 data; the point is that a 1,200 ms
     unattributed residual — this campaign's smoking gun — fails loudly);
   - runner changes are hash-bound separately and do not restart candidate
     freezes.

## Step 4 — Runner legacy-control migration contract

The failed unit `legacy-jsonl/height-50-window-1` died on
`VOTE_LOCK_MIGRATION_AFTER_FIRST_FINALIZED_ROUND`: five validators migrated in
round 2 under portable-snapshot setup. Fix the contract, not the evidence:
re-key the vote-lock migration allowance (and the new index-migration
allowance) to **each validator's first vote-lock reservation / first resume
after restore**, not to the window's first finalized round index. Reproduce
the portable-snapshot setup sequence in a runner test to prove the corrected
contract passes it and still rejects a genuine second migration. Document in
the runner why round-index keying was wrong. No retroactive waiver of the
failed campaign.

## Step 5 — Round-path scan audit (close the known-unknowns)

Complete the sweep so the next campaign has no fourth surprise:

1. `list_private_egress_loop_files` and `list_certified_loop_batch_files`
   (`transport_runtime.rs:~4968/~4993`): determine whether either runs inside
   the synchronous round. If yes, bound it under this plan; if no, record the
   proof (call-graph note) in the handoff.
2. Re-run the `read_dir` sweep over `transport_runtime.rs`,
   `transport_cli.rs`, `block_finality.rs`, `storage_commit.rs`,
   `mempool_proposals.rs`, `batch_snapshot.rs` and classify every hit:
   bounded-by-cap, migration/repair-only, tooling, or round-path (which must
   be zero unbounded). Put the classification table in the handoff.

## Step 6 — Tests

Owner tests beside the outbox code plus integration coverage. Scenario names:

1. `resume_with_zero_tombstones_is_bounded` — index present, no completed
   set: counters near zero.
2. `resume_with_240_and_1024_tombstones_examines_no_retained_payloads` —
   seed real completed jobs, index present: `tombstones_validated == 0`,
   `files_read`/`bytes_hashed` flat between 240 and 1,024.
3. `index_migration_runs_once_and_validates_everything` — no index, seeded
   tombstones incl. every-file hash verification, marker written, second
   resume performs no migration.
4. `tampered_tombstone_fails_migration_and_quarantines` — corrupt
   `batch.json` before migration: current quarantine/error behavior.
5. `tampered_tombstone_after_index_is_caught_by_repair_and_by_prune` —
   corrupt an indexed entry: per-proposal resume does not read it (documented
   authority change), explicit `verify` fails closed, and pruning that
   touches it fails closed.
6. `index_directory_divergence_fails_closed` — delete or add a completed dir
   behind the index's back: next mutation/repair detects and refuses.
7. `crash_between_move_and_index_rewrite_recovers` — simulate by staging the
   intermediate state; reconciliation resolves without payload re-validation
   of the whole set.
8. `pending_jobs_still_block_proposals` — unchanged refusal behavior.
9. `prune_preserves_retention_and_fsync_flow` — over-cap prune via index
   matches current retention-dir semantics; validate-before-remove holds.
10. `enqueue_duplicate_job_id_behavior_unchanged` — pin current behavior with
    a test before refactoring, then keep it green.
11. Proposer-rotation integration test: six-validator round loop where the
    proposer with a 1,024-tombstone outbox is no slower than peers beyond a
    fixed small delta (release-mode, may be `#[ignore]`d manual like the
    vote-lock spot check — but wire the same assertion into the runner gate).

## Step 7 — Verification gates

```bash
cargo test -p postfiat-node certified_send --locked
cargo test -p postfiat-node vote_lock --locked          # must stay green
cargo test -p postfiat-storage --locked
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

Then the release-mode spot check: seeded 1,024-tombstone outbox, one
proposal round, report `outbox_resume_ms`, `files_read`, `bytes_hashed`
before/after the fix. Record numbers in the handoff.

Toolchain note: this host's `cc`/`c++`/`ar` are Zig wrappers (see the
corrected-campaign handoff). Verify a clean-checkout release build before
claiming G1-grade builds.

## Step 8 — Freeze and evidence refresh

After the fix passes Step 7: new G1 source/binary candidate freeze (clean
checkout, full identity record), refreshed binary-bound G2 receipts, milestone
update
recording this plan's completion and the new lineage. **Stop there.** The next
5+5+5 campaign requires separate operator authorization and will follow the
corrected-G4 plan pattern with new identities, the Step-3 gates, and the
Step-4 contract.

## Completion evidence

### Changed invariant and implementation

Per-proposal certified-send resume now reads one bounded index and processes
only active/pending work plus entries actually compacted or pruned. The full-set
validation authority moves from every proposal to: **(a) one-time index
migration, (b) explicit startup/repair, (c) the specific entries touched by
compaction or pruning.** Delivery blocking, acknowledgement, quarantine,
canonical names, replay rejection, size limits, retention moves, and fsync
ordering remain fail closed.

The index, mutation intent, and `flock` mutation lock are stored at the data-dir
root as `.certified-send-completed-index-state.v1`,
`.certified-send-completed-index-intent.v1`, and
`.certified-send-completed-index-mutation.lock`. They are outside both
`certified-send-outbox/` and `completed/` because the previous binary rejects
unknown/noncanonical entries in those directories. Root placement therefore
preserves compatible rollback and mixed-binary operation. The index uses schema
`postfiat.certified_send_completed_index.v1`, deterministic height/job ordering,
entry and directory-stamp checksums, topology/chain/genesis/protocol bindings,
and raw job/batch/certificate SHA-256 bindings. An atomic intent record and
serialized mutation lock provide crash reconciliation; divergence fails closed
until the explicit verify/rebuild command repairs it.

The node report now includes the resume-node identity, `outbox_resume_ms`,
compaction/validation/prune timings, retained-work counters, index-read counters,
compaction/prune counts, enumeration count, and one-time migration flag. Runner
`15d059d1` binds and independently verifies the certified-send work receipt and
round-coverage receipt, permits vote-lock/index migration only on each
validator's first observed reservation/resume after restore, rejects repeated
migration, and applies a 100 ms maximum / -1 ms minimum round residual.

### Frozen identities and local evidence

| Artifact | Result and identity |
| --- | --- |
| Candidate source | PASS; clean detached checkout `e52e050269a2f9fdd28c5083c3888debf3a85063`, equal to `origin/main` at freeze |
| Release binary | SHA-256 `6b130a1f9c81bd64bc9dc42043595f5a27e84185cf3f40b13b5f37a40d72a82e`; 51,978,232 bytes; embedded revision `e52e0502`, profile `release` |
| G1 manifest | SHA-256 `895ec768927a2d630c7d90fd73c5275efe1e54e857248a7cf12441fe97df9ffe` |
| Runner | Branch `postfiatchad/corrected-g4-vote-lock-gate`, pushed commit `15d059d1`; 95 focused runner/packager/verifier tests passed |
| G2 manifest | PASS; SHA-256 `dc01f9770fc2344cce0dcfcfd58dbe37b968f9ffc0276c329bb4f4fea47378e7` |
| Compatible rollback | PASS; six validators converged through current → older compatible binary → current recovery; report SHA-256 `af37f0e4de9d23689b131532d0fded2593905b5d88a5de1600431c511d6904be` |
| Tamper/crash refresh | PASS; 69 cases / 37 owner tests / zero uncovered requirements; report SHA-256 `df45e0bb478299e7778bc50537fd6bb059f04a19c52f01d0c5adb444331c2ceb` |
| `--verify-only` | PASS; two tests prove missing inputs are not created and stale generations are rejected without mutation |

G1/G2 reports remain private local evidence because their directories include
disposable keys and campaign material. They are not a redaction-safe G5 packet
and must not be committed or published.

### Measured result and verification

The pre-fix release binary at source `442c5a4d` resumed 1,024 retained
tombstones in 66.893 ms while reading 6,144 files / 3,952,142 bytes and hashing
4,096 payloads / 73,728 bytes. The frozen candidate resumed the same retained
count in 2.098 ms with zero retained payload reads or hashes, one 687,566-byte
index read, and a 2.054 ms maximum proposer/peer delta in the six-validator
release spot check.

All Step-7 commands passed against the implemented source: 30 certified-send
tests (one manual release check ignored), 15 vote-lock tests (one manual release
check ignored), 83 storage tests (two manual scaling tests ignored, process-crash
integration passed), formatting, workspace all-target check, warnings-denied
workspace Clippy, and the complete locked workspace test suite. The workspace
suite completed with node library 329 passed / 3 ignored and node binary 138
passed / 3 ignored. The ignored release proposer-rotation test was then run
explicitly and passed.

No performance campaign, devnet contact, deployment, height-924 copy, Task Node
action, agent delegation, frozen-fleet mutation, or auditor-inventory edit was
performed. The source/G1/G2 refresh closes this plan only; storage remains
unqualified until a separately authorized future G4 campaign passes and the
remaining milestone gates close.

## Explicit non-goals

- No performance campaign under this plan (separate authorization).
- No devnet contact, deployment, height-924 copy, or Task Node actions.
- No changes to consensus voting, certificates, receipts, or signed bytes.
- No weakening of quarantine, retention, fsync, or fail-closed behavior to
  hit a latency number.
- No touching frozen fleets, prior private run directories, or the two
  untracked `docs/security/` auditor inventories.

## Exit criteria

- [x] Per-proposal certified-send work bounded by pending jobs + O(1) index
      access; proven flat across 0/240/1,024 tombstones by executable tests.
- [x] One-time migration + explicit repair own full-set validation; authority
      relocation documented in exactly those terms.
- [x] `outbox_resume_ms` and work counters in the round report; runner
      bounded-work and round-coverage residual gates implemented and tested.
- [x] Runner migration-allowance contract re-keyed and proven against the
      portable-snapshot sequence.
- [x] Round-path `read_dir` classification table complete; zero unbounded
      round-path sites.
- [x] All Step-7 gates pass; vote-lock suite untouched and green.
- [x] New G1 source/binary freeze + G2 refresh recorded; milestone updated;
      handoff written naming the changed invariant, surfaces, commands,
      omissions, and the pending campaign authorization boundary.
