# Devnet registry-continuation fix deployed; chain live at 926

- **Operator:** PostFiatChad (`postfiatchad`)
- **Date:** 2026-08-31 UTC

## BLUF

The devnet's real blocker was the superseded validator-registry
reapplication defect in the deployed `8cc7d15e` binary itself; no binary on
the fleet could commit height 925. Minimal signed release
`registry-fix-291d1eb1` (`8cc7d15e` + backported `2c7aa36f` only) was rolled
to all six validators, canary-first. Blocks 925 and 926 then committed via
real faucet transfers with full convergence at tip `9e738e87…c2098b`. See the
[postmortem](../postmortems/devnet-registry-continuation-wedge-2026-08-31.md).

## What changed on the fleet

- Binary: `d5e5ef63…` → `6b07a8c3…` on all six (`/opt/postfiat/releases/registry-fix-291d1eb1/`).
- Unit files updated to the new release paths; content otherwise identical.
- Repairs: validator-0 RPC restart (accept-queue wedge), validator-1
  `ordered_batches.append.jsonl.head` regenerated via
  `storage-integrity-migrate-legacy`, certified-send outboxes chowned to
  `postfiat` on validators 2-5.
- Rollback surface: `/root/postfiat-deploy-backups/registry-fix-291d1eb1/`
  per host; old release dirs untouched.

## Open defects

1. Snapshot finalized-checkpoint export fails fleet-wide at block 924
   certificate replay (both binaries); signed snapshot backups unavailable.
2. `mempool_submit_signed_transfer_finality` can return an error after a
   successful commit (internal retry hits its own duplicate check).
3. RPC serve loop has no per-connection read timeout; a stalled client can
   wedge the whole RPC edge again.

## Storage rollout status

Unchanged: `10dd9f20` and `d0ae79f3` remain **DO NOT DEPLOY**. The
registry fix removes the G6 height-925 continuation confound, so the storage
candidate must be requalified against a deployment-exact gate on top of the
now-live `registry-fix-291d1eb1` lineage.

## Next decision or action

Fix the certificate-replay snapshot defect (open defect 1) so signed
backups work again, then add the RPC read timeout and the finality-response
idempotency fix, each with focused tests, before any further storage work.
