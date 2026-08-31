# Devnet Transactional Storage: Single-Writer Deployment Plan

**Status:** **LOCKED** — Text Improvement Harness full gate passed on
2026-08-31, average 88.60/100 (GPT 89.60, Fable 87.80, GLM 88.40; five runs
per lane; project `storage-single-writer-deployment-plan`); scored content
SHA-256
`6aa3ba182bb4f2c539b374005d9185d7037b396d178c34a0c9bc5676c592f535`.
Supersedes the stopped
[devnet storage rollout plan](devnet-storage-rollout-plan.md).
**Date:** 2026-08-31
**Owner:** PostFiatChad
**Target:** transactional `redb` storage live on all six `postfiat-wan-devnet-2`
validators, ending the linear consensus-latency growth measured by E4
(1.66 s at round 50 to 14.9 s at round 500).

## Why this is not deployed yet, in one paragraph

The storage engine itself is done and heavily tested: bounded ordered-history
accumulator, atomic per-height commit, 69-case tamper/crash matrix, exact
height-915 replay (`benchmarks/storage-scaling/`). It failed to deploy twice
for reasons outside the engine. First, the height-925 continuation failure was
the validator-registry reapplication bug, which lived in the *deployed* binary
and is now fixed and live (`registry-fix-291d1eb1`, blocks 925-926 committed
2026-08-31). Second, the qualification rehearsal started one process while
production starts two: `transport-validator-serve` and `rpc-serve` both open
the same data directory, and `redb` grants one process an exclusive write
lock, so the second service could never become ready. That is the only
engineering problem left: **two processes, one writable database.**

## The fix: bounded writer lease, read-only everywhere else

`redb 4.2` already supports one writer process plus concurrent cross-process
readers, and the storage layer already ships both halves of the solution:

- `TransactionalStore::open_read_only_with_integrity_key`
  (`crates/storage/src/transactional.rs:529`) opens a `ReadOnlyDatabase`
  that coexists with a live writer.
- The node store caches the transactional handle lazily
  (`transactional_store: Arc<Mutex<Option<Arc<TransactionalStore>>>>`,
  `crates/storage/src/lib.rs:80`) and
  `release_inactive_shared_transactional_stores` (`lib.rs:158`) already
  exists to drop idle handles.

Design:

1. **All queries use read-only opens.** `rpc-serve` request handling never
   takes the write lock.
2. **Writes acquire the writer on demand and release it when the commit
   ends.** The devnet is operator-driven: blocks exist only during explicit
   certified rounds, and rounds are serialized by proposer election, so
   writer contention windows are short and bounded. Acquisition uses retry
   with backoff and a hard deadline; failure to acquire fails the round
   closed with a distinct `storage_writer_busy` error, mutating nothing.
3. **Transport holds no idle writer.** Both services follow the same
   acquire-commit-release discipline, so start order no longer matters.

Alternative considered and rejected for now: delegating all RPC writes to
transport over a local socket. It is the better long-term shape for a
high-throughput chain, but it adds an IPC protocol, a failure surface, and a
week of work to solve a problem the lease solves in a day on an
operator-driven devnet. Revisit at public-testnet planning.

## What we will not block on

Fail-closed applies to consensus state, not to the calendar. Explicitly:

- **No new snapshot dependency.** The fleet-wide finalized-checkpoint export
  defect (block 924 certificate replay, see the
  [2026-08-31 postmortem](../../postmortems/devnet-registry-continuation-wedge-2026-08-31.md))
  gets fixed in parallel. Canary backups use the raw-copy method that
  actually restored validator-1 twice this weekend: full data-directory copy
  (1.7 GB, seconds), all four JSONL heads, binary, and unit files.
- **No full workspace or Orchard suite before the canary.** The change
  surface is storage plus two service entry points. Focused tests plus the
  deployment-exact gate are the evidence. The long suite runs once, at
  milestone completion, as the mandate already requires.
- **No re-scoring of locked research.** The research spec passed its gate on
  2026-08-26 (88.67/100) and stays locked.

## Checklist

Base revision: current fleet lineage (`registry-fix-291d1eb1` =
`8cc7d15e` + `2c7aa36f` backport) plus the storage commits
(`dfd0b9f1..10dd9f20`) rebased onto it.

- [x] 1. **Writer lease (done 2026-08-31, `f0013c29`).** Implement acquire-commit-release in
      `TransactionalStore` and route `rpc-serve` reads through read-only
      opens. Focused tests: concurrent two-process open in both orders,
      writer-busy fail-closed round, reader sees committed height during an
      active writer, lease released after crash mid-round (lock dies with
      the process).
- [x] 2. **Rebase and unit parity (done 2026-08-31; `main` is the deployed lineage plus storage commits; units byte-identical mod release paths).** Rebase storage commits onto
      the deployed lineage; regenerate units with
      `deployment-validator-units-stage`; diff against live units — only
      release paths and storage flags may differ.
- [x] 3. **Deployment-exact gate — PASSED 2026-08-31 on the real hosts** (receipt: `benchmarks/storage-scaling/deployment-exact-gate/gate-926-receipt.json`). Six clones from
      exact height-926 copies of the live data directories, running the
      signed systemd units under the real sandbox (`ProtectSystem=strict`,
      `ReadWritePaths`), with transport and RPC co-started in both orders.
      Gate passes only if: both services reach ready on every clone; one
      RPC-driven certified transfer finalizes at 927 with six-way root
      convergence; a second round proves repeatability; then the exact
      rollback restores the deployed binary, all four JSONL heads, and unit
      files, and the rolled-back clone serves status at the pre-round
      height. Any `PASS` emitted by a runner that skipped a production
      service is a gate bug, not evidence.
- [x] 4. **Canary — DONE 2026-08-31.** Raw-copy backup of validator-1, deploy signed
      release to validator-1 only, verify both services ready and status
      convergence at the live height. Stop rule: any service failure or
      divergence → restore the backup, one diagnosis, stop.
- [x] 5. **Fleet rollout + liveness proof — DONE 2026-08-31** (blocks 927-931; receipt `deployments/storage-lease-20260831/deploy-receipt.json`). Remaining five
      validators one at a time with per-host status checks; then two
      value-carrying certified rounds through RPC finality submit; all six
      converged on the new storage with `transactional_generation` present
      and zero full-history reads in telemetry.
- [x] 6. **Receipts recorded; Z1 clock started 2026-08-31T04:29:41Z.** Deploy receipt under
      `deployments/`, update
      [Current State](../../status/chain-state-current.md), start the Z1
      observation clock. Milestone completion gate (full suite) runs at the
      end of Z1, once.

Parallel, non-blocking:

- [x] P1. Fix finalized-checkpoint certificate replay at drill heights
      922-924 — done 2026-08-31: `activate_validator_registry_updates_for_height`
      in `crates/node/src/block_replay_wallet.rs` now skips recorded updates
      whose effect is already reflected and anchors the applied history on
      each block's quorum-signed certificate registry root, so superseded
      drill history is no longer reapplied on the replay/export path.
      Regression tests reconstruct both failing shapes from local fixtures in
      `crates/node/src/tests/validator_registry_continuation_tests.rs`
      (`superseded_unapplied_rotation_history_replays_for_checkpoint_export`,
      `recorded_offchain_rollback_history_replays_for_checkpoint_export`);
      restores signed snapshot backups.
- [ ] P2. RPC serve-loop read timeout and an RPC-round-trip health probe
      (the validator-0 wedge class).
- [x] P3. Finality-submit idempotent response (no error after successful
      commit) — done 2026-08-31: `committed_signed_transfer_finality_replay`
      in `crates/node/src/rpc_cli.rs` replays the committed finality result
      when the exact signed transfer (same tx_id) is already final;
      conflicting duplicates still fail. Regression tests in
      `crates/node/src/main_parts/tests/rpc_serve_request_tests.rs`.

## Schedule

| Date | Milestone |
| --- | --- |
| Sep 1 | Writer lease implemented, focused tests green |
| Sep 2 | Rebase done; deployment-exact gate passing on six clones |
| Sep 3 | Canary, fleet rollout, liveness proof, Z1 start |
| Sep 5 | Buffer exhausted — if not live, escalate with the specific blocker |

Slipping past Sep 5 requires naming the exact failing gate and its error,
not a process reason.

## Rollback

Per validator, restorable in under a minute, rehearsed this weekend on
validator-1: stop services, restore raw-copy data directory (four JSONL
heads included), restore unit files from
`/root/postfiat-deploy-backups/<release>/`, `daemon-reload`, start, verify
status height and roots against the fleet. The old release directory is
never modified in place.

## Evidence trail

- Engine qualification: `benchmarks/storage-scaling/` (G1-G4, replay,
  tamper/crash matrix)
- Both failure receipts: `benchmarks/storage-scaling/devnet-rollout/`
- Registry fix now live: `deployments/registry-fix-20260831/deploy-receipt.json`
- Latency motivation: E4 results in
  [Cobalt adversarial verification results](../../governance/cobalt-adversarial-verification-results.md)
