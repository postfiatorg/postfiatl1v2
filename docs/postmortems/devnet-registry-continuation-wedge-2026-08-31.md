# Devnet registry-continuation wedge and recovery

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-31 UTC
- **Chain:** `postfiat-wan-devnet-2`

## BLUF

The live devnet could not produce any block after height 924 on the deployed
`8cc7d15e` binary. The wedge was the superseded validator-registry
reapplication defect already fixed on `main` by `2c7aa36f`, previously
misattributed to the storage candidate. A minimal release
(`registry-fix-291d1eb1` = `8cc7d15e` + `2c7aa36f` only, no storage changes)
was built, signed, and rolled to all six validators. Two value-carrying
certified rounds then committed blocks 925 and 926 with full six-validator
convergence. The chain is live again.

## Faults found and fixed

1. **Validator-0 RPC accept-queue wedge.** A client half-closed a socket
   mid-request around Aug 26; the single-connection serve loop blocked forever
   in `recvmsg` with no read timeout, filling the accept backlog (129/128)
   while systemd reported `active`. Fixed by service restart. Follow-up: the
   RPC serve loop needs a per-connection read timeout and health checks must
   probe an actual RPC round-trip, not `systemctl is-active`.
2. **Incomplete canary rollback on validator-1.** The 2026-08-30 storage
   canary rebuild upgraded four authenticated JSONL heads; the rollback
   restored only three. The leftover v2 `ordered_batches.append.jsonl.head`
   failed v1 integrity verification, so every proposal from validator-1
   failed. Repaired with the deployed binary's own offline
   `storage-integrity-migrate-legacy` path after moving the bad head aside
   (preserved in `/root/postfiat-deploy-backups/storage-10dd9f20-failed-20260830/validator-1/`).
3. **Chain wedge: superseded registry-history reapplication on the deployed
   lineage.** The Aug-30 drill (rotation, signed rollback, later legitimate
   rotation of the same record) left history that
   `live_validator_registry_after_due_updates` re-tested in isolation, so the
   first new certified height failed
   `live validator registry activation previous validator registry root
   mismatch`. This is the exact defect `2c7aa36f` fixes; it was in the
   deployed code all along, not only in the storage candidate. G6's failure at
   height 925 was a true signal about the deployed lineage, mislabeled as a
   candidate defect.
4. **Root-owned stale certified-send outbox jobs** on validators 2-5 (from
   root-run manual rounds on Aug 26) made the postfiat-user round machinery
   fail with `Permission denied` when completing jobs. Fixed by `chown -R
   postfiat:postfiat` on every `certified-send-outbox`; all six verified
   clean.

## Deployment

- Release: `registry-fix-291d1eb1`, git `291d1eb156894dd502b6f56b783e93fba7433b6e`
  (worktree commit: `8cc7d15e` + code files of `2c7aa36f`), binary SHA-256
  `6b07a8c31ee5f306995e12df23d644348c3ab074beb68800f7251f3f38ef7de6`.
- Staged with `deployment-validator-units-stage`; units and env files verified
  byte-identical to the live deployment modulo release paths; manifest signed
  by the existing trusted publisher key and verified on every host with the
  exact `ExecStartPre` command before restart.
- Focused regression tests passed (4/4 in
  `validator_registry_continuation_tests`, including
  `superseded_registry_rotation_history_continues_to_next_certified_height`,
  the exact devnet shape).
- Rolling deploy validator-1 (canary) first, then 0, 2, 3, 4, 5; convergence
  at 924 re-verified after the canary and after full rollout. Old release
  binaries, units, and per-host unit backups
  (`/root/postfiat-deploy-backups/registry-fix-291d1eb1/`) retained for exact
  rollback.

## Liveness proof

- Block 925: faucet transfer 1,000,000 atoms + 32 fee, batch
  `3fee4666…`, certificate under post-rotation registry root `08a451e0…`.
  The submitting client received a spurious `batch already applied` error from
  an internal retry after the commit had already succeeded; balances confirm
  exactly-once application.
- Block 926: faucet transfer 500,000 atoms + 22 fee, fully clean response
  (`confirmed: true`, `accepted: true`), all six validators converged at tip
  `9e738e879d4a…`.
- Faucet: 13,904,463 → 12,404,409 atoms, sequence 92 → 94; recipient
  `pfde0ba09f…`: 0 → 1,500,000 atoms.

## Known open defects (not blocking liveness)

1. **Snapshot finalized-checkpoint export fails fleet-wide** with
   `block 924 certificate registry root mismatch` on both the old and the
   fixed binary. The certificate-replay path
   (`activate_validator_registry_updates_for_height`) reconstructs a registry
   at 924 that does not match the recorded pre-rotation certificate root
   `945768d5…`. Signed snapshot backups are unavailable until fixed; the
   safe-rollout backup stage was bypassed with per-host raw copies of every
   file the deploy touches.
2. **Finality-submit response path can report an error after a successful
   commit** (the `already applied` retry artifact above). Clients must treat
   an error response as unknown-outcome and re-check chain state.
3. **RPC serve loop lacks a read timeout** (fault 1 above).

## Rollback surface

Restore per-validator unit files from
`/root/postfiat-deploy-backups/registry-fix-291d1eb1/validator-N/`,
`systemctl daemon-reload`, restart both services. The old
`cobalt-adversarial-8cc7d15e` release directories are untouched on every
host. No data-directory format changed.
