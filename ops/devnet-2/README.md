# devnet-2 operational repairs, 11 Nov 2026

Installed while proving the brokerage-backed NAVCoin primary market (deliverable
A1). Both items here are **repairs for node bugs, not fixes.** The bugs are
recorded as U4 and U5 in
`post-fiat-emergency-execution-plan/execution/DESIGN-brokerage-backed-navcoin.md`.

## U4 — a lost delivery ack wedges a proposer permanently

A certified-send delivery whose target already applied the batch returns
`storage_duplicate_record: ... already exists`. The retry loop treats that as a
failure, so the job never completes. A node refuses to propose while any
delivery job is pending, and the obligation is derived per peer from the
topology — so deleting the job directory does not clear it either; a missing job
for a peer is itself counted as pending.

On 11 Nov one lost acknowledgement (validator-2 -> validator-5, block 1178)
removed validator-2 from block production entirely. The ack schema already
carries `already_applied` for exactly this case; the transport never reaches it.

- `outbox-watchdog.py` + `postfiat-outbox-watchdog.service` — moves aside jobs
  whose every attempt was a duplicate-record failure, with a backup. Installed on
  all six validators. Note this alone is **insufficient**, because of the
  derived-obligation behaviour above.
- `rebuild_certified_send_tombstone.py` — reconstructs the lost
  `<target>.send.json` from the sibling delivery for the same block (same batch,
  same message id and payload hash; only the peer differs), with
  `already_applied: true`, which is what the target reported. No signature is
  involved; this is local delivery bookkeeping.
- `complete_certified_send_job.py` — marks such a job completed with an ack built
  from the job's own `block_hash` and `expected_state_root`.

Both scripts refuse to act unless `last_error` is a duplicate-record failure.

**Proper fix:** treat `storage_duplicate_record` as an idempotent success in the
transport retry path and write the ack with `already_applied = true`.

## U5 — validator-5 rpc-serve memory leak

`rpc-serve` on validator-5 grows roughly 800 MB/hour and then stops answering,
which breaks six-validator convergence checks. A watchdog was tried and **made
things worse**: its liveness probe was too aggressive, restarting the endpoint
every ~5 minutes at ~650 MB against a 2 GB threshold. Each restart killed
in-flight transport and manufactured the stuck deliveries in U4. It is disabled
(`postfiat-rpc5-watchdog.service`, disabled 11 Nov). The leak is unfixed; a
manual restart via `/usr/local/bin/rpc5-start.sh` is the current workaround.
