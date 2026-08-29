# Vote-lock marker and batched completed-index fixes

- **Operator:** Post Fiat (`postfiatchad`)
- **Date:** 2026-08-29 UTC

## BLUF

Both defects behind the remediated G4 failure are diagnosed from the
campaign's own raw rounds and fixed on `main`: the vote-lock empty-directory
marker at `ff2b3532` and the certified-send per-entry index-rewrite
amplification behind the ~1.40 height ratios at `48a94425`. The release spot
check for the exact binding scenario dropped from ~205 ms to ≤6.4 ms per
round. The fixes are locally verified but **unfrozen**: no G1/G2 refresh,
prepared-input rebind, campaign, packet, or qualification claim exists. The
governing state is the
[storage scaling milestone](../plans/active/storage-scaling-milestone.md).

## Diagnosis (from preserved campaign artifacts, read-only)

Mining the ten checkpoint-bound selected windows of
`~/repos/postfiat-storage-g4-measurement-a3c7bea9-a92bb085-v1` localized the
entire 1.40 ratio to one surface:

| Fact | Height 50 | Height 5,000 |
| --- | --- | --- |
| Non-proposer-0 resume median | ~49 ms | ~49 ms |
| Validator-0 steady-state resume median | 66.4 ms | 204.4 ms |
| Validator-0 first-resume (one-time migration) | ~250–293 ms | ~1,865–1,955 ms |
| `outbox_jobs_pruned` per validator-0 round | 0 | 5 |
| `outbox_index_bytes_read` (validator-0) | ~183 KB | ~781 KB |

The round p95 at height 5,000 (569 ms ≈ 350 ms base + ~205 ms resume) is
exactly validator-0's steady-state rounds. At the 1,024-tombstone retention
cap, each validator-0 proposal compacted five new jobs **and pruned five old
ones**, and `certified_send_completed_index.rs` rewrote the full completed
index — with its own intent write and directory syncs — once per touched
entry: ~10 full ~780 KB rewrites and ~50 fsyncs per round. Per-round work was
bounded per touched job, so every campaign work gate passed honestly; the
defect was the O(index size × touched jobs) byte constant. The five ~1.9 s
first-resume migrations are the gate-allowed one-time cost and sit beyond p95.

## Fix 1 — vote-lock eager marker (`ff2b3532`)

`migrate_block_proposal_vote_locks` no longer defers the durable marker when
the lock directory is empty. A validator's first successful reservation now
performs the one-time migration eagerly, mirroring the locked certified-send
contract. A legacy JSON lock appearing after the marker exists is unsupported
out-of-band state requiring explicit operator repair; it never re-triggers
migration. New fixtures pin the exact failed campaign sequence
(empty directory → first reservation migrates → second reservation is
ordinary bounded work) and the stray-lock contract.

## Fix 2 — batched completed-index mutations (`48a94425`)

One resume's appends and prunes are now covered by a single durable batch
intent (`postfiat.certified_send_completed_index_intent.v2`, ≤32 operations
per chunk): one intent write, all validated moves, one directory-sync set,
**one index rewrite**, retention disposal, one intent clear. Crash recovery
replays the whole batch from the intent; v1 single-operation intents still
reconcile for upgrade safety. Unchanged: validation authority (one-time
migration, explicit repair, touched entries), quarantine and tamper behavior,
fsync ordering per move, deterministic entry ordering, the
`validated == compacted + pruned` gate arithmetic, node-local scope, and all
consensus bytes.

## Verification completed

| Check | Result |
| --- | --- |
| `cargo test -p postfiat-node certified_send --locked` | PASS: 37 passed (35 prior + 2 new batch fixtures) |
| `cargo test -p postfiat-node vote_lock --locked` | PASS: 17 passed (15 prior + 2 new marker fixtures) |
| `cargo test -p postfiat-node completed_index --locked` | PASS: 17 passed, including mid-batch crash recovery and at-cap batch behavior |
| `cargo test -p postfiat-node transactional_verify_only --locked` | PASS: 2 passed |
| Release spot check: at-cap steady-state rounds (new, mirrors the campaign shape) | PASS: 8 rounds at 6.1–6.4 ms each, 5 compacted + 5 pruned per round, `validated == compacted + pruned` |
| Release spot check: idle proposer rotation at 1,024 tombstones | PASS: 2.116 ms slowest, 2.071 ms delta, zero retained payload reads |
| `cargo fmt --all -- --check` | PASS |

The prior release spot check measured only idle resumes, which is why it
predicted ~2 ms while the campaign paid ~205 ms; the new at-cap steady-state
spot check closes that fixture gap. Projection from the campaign's own
arithmetic: removing ~200 ms from validator-0's height-5,000 rounds implies
ratios near 1.02–1.05 against the 1.10 limit. That is a projection, not a
measurement; only a future authorized campaign proves it.

## Boundaries preserved

- No new campaign was started; the three closed failed campaigns were not
  resumed, retried, or relabeled, and their private output is untouched.
- No release freeze, G1/G2 refresh, helper rebuild, or prepared-input rebind
  occurred; every campaign-facing identity in the milestone is stale for the
  new sources and must be refreshed before any campaign plan binds them.
- Runner gate logic is unchanged and no runner commit was made.
- No Task Node, agents, devnet, fleet, deployment, height-924, or live-probe
  action occurred.
- The two unrelated untracked `docs/security/` auditor inventories remain
  untouched.

## Next decision or action

1. Authorize the freeze/evidence cycle for the remediated source: release
   build, G1 candidate manifest, binary-sensitive G2 refresh, helper identity
   check, and prepared-input rebind.
2. Write and review a new campaign plan binding the refreshed identities,
   then separately authorize exactly one run.
3. G3 remains separate: remediated height-915 replay must target the future
   frozen binary; height 924 still needs a named custodian and separate
   read-only-copy authorization. Do not wait idle for it.

## References

- [Storage scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Remediated G4 qualification failure](2026-08-29___postfiatchad__remediated_g4_qualification_failure.md)
- [Vote-lock fix](https://github.com/postfiatorg/postfiatl1v2/commit/ff2b3532)
- [Batched index fix](https://github.com/postfiatorg/postfiatl1v2/commit/48a94425)
