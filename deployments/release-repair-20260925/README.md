# Merged combined and FastPay release tip qualification, September 25, 2026

**Final. Nothing was deployed. No source was repaired.**
Every check that ran locally passed. The new canary backup was SKIPPED (the existing height-1036 backup replayed in the addendum), `warm-latency` was deferred to CI, and there is no CI verdict yet.

Source tip: `f60e9639f83649769f29276a9de14f5f16271877` (`f60e9639`). It is the test-only pre-step commit on top of the
merge record `a2dfa94735a1182551931b0de92da701898312b4` (`a2dfa947`). That merge brings the deployed FastPay line r4
`c1c81119` into the qualified combined line `f59dc07a`; see
[the merge record](../../docs/status/combined-fastpay-merge-20260925.md).
The evidence commits that follow add only this packet.

Executable SHA-256, build 1: `d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6`.
Executable SHA-256, build 2: `d66cecc36426ce05ced8730b2439a27285c6b404688acd13dc23594b884eabd6`.
Reproducibility: **PASS**. `cmp` compared the two executables byte for byte.
Each build came from its own clean worktree and its own copy of the September 22 release cache.
These are not empty-cache builds. Both used the GCC linker (`/usr/bin/gcc`, GCC in `.comment`, no Zig),
the same remap arguments and `SOURCE_DATE_EPOCH=1789514690`. Neither has RPATH or RUNPATH.

| Check | Result | Evidence |
|---|---|---|
| Two matching node executables | PASS | [Build records](node-builds.json) |
| Six original full-history checks, height 1020 | PASS | [History run](history-run.json) |
| Six saved V2 full-history checks, height 1021 | PASS | [History run](history-run.json) |
| Six fresh V2 full-history checks, height 1021 | PASS | [Fresh V2 history](fresh-v2-history-run.json) |
| r4 full replay of one original, contrast probe | FAILS AT 1011 (expected) | [Probe stderr](history/r4-full-replay-probe-validator-0.stderr.log) |
| Current-chain replay from the existing r4 validator-1 canary backup, height 1036 (addendum) | PASS: checkpoint verified, full history replayed to 1036, root `ba7cc012…` | [Result](canary-backup-1036/result.json), [replay log](canary-backup-1036/logs/merged-verify-state-remote.stdout) |
| New validator-1 canary backup | SKIPPED | [Reason](canary-backup.json) |
| Local governed rotation: both startup orders, convergence, restart | PASS | [Gate log](logs/governance-gate.stdout), [receipt](v2-service-receipt.json) |
| Rollback: six r4 checkpoint verifications | PASS | [History run](history-run.json) |
| Rollback: r4 services start and restart on six copies | PASS_STARTUP_AND_RESTART_ONLY | [Log](logs/rollback-services.stdout), [receipt](old-binary-service-receipt.json) |
| Rollback: new build full replay of restored validator-0 | PASS | [Receipt](receipts/final-candidate-on-rollback-validator-0.json) |
| Timeout vote and view recovery, node bin tests | PASS: 4 passed | [Log](logs/view-recovery-bin.stdout) |
| Timeout votes form a timeout certificate, node lib test | PASS: 1 passed | [Log](logs/timeout-votes-lib.stdout) |
| FastPay committee, recovery and checkpoint restore, node lib tests | PASS: 21 passed | [Log](logs/fastpay-committee-lib.stdout) |
| Workspace check | PASS | [Log](logs/workspace-check.stderr) |
| Rust formatting | PASS | [Log](logs/format.stdout) |
| Proof public-input inventory | PASS | [Log](logs/proof-input-inventory.stdout) |
| Workspace Clippy, `-D warnings` | PASS | [Log](logs/workspace-clippy.stderr) |
| fastpay-types | PASS: 9 passed | [Log](logs/fastpay-types.stdout) |
| fastpay-execution | PASS: 11 passed | [Log](logs/fastpay-execution.stdout) |
| cobalt-handoff-tests | PASS: 13 passed | [Log](logs/cobalt-handoff-tests.stdout) |
| live-replay-supply | PASS: 1 passed | [Log](logs/live-replay-supply.stdout) |
| warm-latency | DEFERRED_TO_CI | [Log](logs/warm-latency.stdout) |
| node-fastpay (all node lib `fastpay` tests) | PASS: 23 passed | [Log](logs/node-fastpay.stdout) |
| Python wallet tests (pre-step) | PASS: 67 passed | [Log](logs/pre-step-wallet-tests.log) |
| Full workspace test suite | LEFT TO CI | Not run locally; no CI run exists for the branch |
| mkdocs build --strict | PASS | [Log](logs/strict-docs.stdout) |
| public-doc-links | PASS | [Log](logs/public-doc-links.stdout) |
| public-secret-scan | PASS | [Log](logs/public-secret-scan.stdout) |

| Validator copy | Original, 1020 | Saved V2, 1021 | Fresh V2, 1021 | r4 checkpoint |
|---|---|---|---|---|
| validator-0 | PASS | PASS | PASS | PASS |
| validator-1 | PASS | PASS | PASS | PASS |
| validator-2 | PASS | PASS | PASS | PASS |
| validator-3 | PASS | PASS | PASS | PASS |
| validator-4 | PASS | PASS | PASS | PASS |
| validator-5 | PASS | PASS | PASS | PASS |

Per-node verifier stdout and stderr are in `history/`. Commands, timings, memory limits and
exit codes are in `receipts/`. See also [qualification.json](qualification.json) and
[the exact commands](verification-commands.md).


## Block 1011

The merged build replays the full archived history through block 1011 and on to the
certified tips: 1020 for originals and 1021 for both V2 sets. Original validator-0 reports
`verified: true`, `block_count: 1020`, tip `9d02b8eecb78408e8f1de12ae1e2607ad4987c2c8d883f1718593df7c2d9ca529f0707bd581e3e414361f202b1768feb`
([report](history/original-validator-0.stdout.json)).
The deployed r4 executable was run on its own disposable copy of the same original. It stops at 1011:

```text
error: verify-state failed: block 1011 replay state validation failed: issued asset supply exceeds finalized NAV circulating supply for `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`: global supply 313700595 exceeds finalized supply 304700595
```

([r4 probe stderr](history/r4-full-replay-probe-validator-0.stderr.log)).
So the merge keeps the combined line's history repair. The r4 line alone fails at 1011; the merged line replays it.

## Current-chain replay

**Addendum: PASS through height 1036** (see below). No new validator-1 canary backup was taken.
The authorized exception covered only the safe-rollout `backup` step, and that step cannot run by itself here:

- scripts/postfiat-safe-rollout backup refuses the r4 rollout state: all six validators are applied (state.applied) and a verified backup is already recorded (state.backup.verified). A fresh preflight state would be needed.
- The backup destination comes from the stage release id. validator-1 already has /var/lib/postfiat/pre-rollout-snapshots/fastpay-committee-20260925-r4-validator-1-finalized-checkpoint (read-only probe: logs/canary-backup-readonly-probe.stdout), and the backup script requires that path to be absent. A new staged release id would be needed.
- A fresh preflight also needs a Vultr API key file, and validator-1 has 3.5 GB free on its root disk (96% used). An extra snapshot export would add to that.

The only fleet contact was three read-only SSH listings of validator-1's snapshot directory and its disk usage
([probe](logs/canary-backup-readonly-probe.stdout)). As instructed, the saved 1020 and 1021 copies are the
only history evidence at that point.

### Addendum: existing height-1036 canary backup

The r4 rollout's own validator-1 backup, taken at height 1036 before r4 was applied, was copied and replayed.
Nothing on any validator was written, deleted or restarted.

- Copy: `rsync -a` pull over SSH, no `--delete`, 219M (229,276,689 bytes, 21 files). All 21 files match
  the r4 rollout's local `backup-unsigned` by SHA-256 ([rsync log](canary-backup-1036/logs/rsync.stdout),
  [hashes](canary-backup-1036/logs/remote-unsigned.sha256)). The copy stays under `~/.cache`; git holds only receipts and logs.
- Signature: the rollout tool signs the backup locally, so its signed copy (`backup-signed`, the same 21 data files)
  was imported the way `_verify_and_record_backup` does it, trusting the snapshot publisher public key
  `pf4ebb80…`. The merged build accepted it. A copy with a changed signature was rejected, and so was
  the r4 release's `deployment.public.json`, which is a deployment key, not a snapshot key.
- Merged build `d66cecc3…` (hash checked): finalized-checkpoint verification PASS on both the signed import
  and the direct validator-1 copy. Full-history `verify-state` of the validator-1 copy: `verified: true`,
  `block_count: 1036`, tip `4d04d290…`, state root `ba7cc0125d5503bd…`, 195 s. These match the r4 rollout's
  recorded backup, so block 1034 (one transparent transfer) replayed. The signed import replayed to the same tip and root.
- r4 contrast `44b6794f…` (hash checked), checkpoint verification on its own import: PASS, same tip and root.
- Not covered: blocks after 1036, including 1042 and 1043. They are not in this backup.

[Result](canary-backup-1036/result.json), [receipts](canary-backup-1036/receipts/),
[driver](canary-backup-1036/run_1036.py).
Fleet disk use, read-only: [fleet-disk.json](fleet-disk.json). validator-1 is at 96% (3.5G free).
Its largest users are `/var/log/postfiat/validator-1` (13G), `/var/backups/postfiat` (10G),
`pre-rollout-snapshots` (9.0G: 33 checkpoint exports and 36 staged binaries), `gate931` (5.7G), `gate926` (4.3G)
and `/opt/postfiat/releases` (4.1G).

## Merge behaviours

(a) Timeout votes and view recovery (`verify_block_log: false` in
`crates/node/src/finality_view_recovery.rs`). The local governance gate always uses the view-0
proposer and cannot stall one, so it does not exercise a view change.
These focused tests cover the path instead:

- `activated_consensus_v2_transport_survives_failed_view_zero_proposer_n4` and `_n6`
  (`crates/node/src/main_parts/tests/transport_batch_payload_tests.rs`): a failed view-0 proposer is
  skipped by a timeout certificate. Result: PASS: 4 passed.
- `consensus_v2_timeout_vote_rpc_is_finality_gated_and_durably_signed`
  (`rpc_serve_request_tests.rs`) and the `finality_view_recovery` tests: timeout-vote RPC signing on
  the finalized checkpoint. These run in the same command.
- `timeout_votes_reconstruct_hotstuff_timeout_certificate` (`crates/node/src/tests/consensus_history.rs`):
  PASS: 1 passed.

(b) FastPay: `fastpay_payment_safety`, which covers signer retention after rotation, effect restore
after restart and the rotated sixth-validator anchor, plus `fastpay_recovery_node` and
`finalized_checkpoint_snapshot_restores_transactional_storage_without_legacy_replay`.
Result: PASS: 21 passed.

## Rotation and rollback

The governance gate ran the unchanged `deployments/release-repair-20260916/run_local_governance_gate.py`
on disposable copies. It used isolated local signers and a six-peer loopback topology, with both
startup orders, one certified rotation accepted on every node, convergence and a restart.
Start orders: validator-0 transport-first, validator-1 rpc-first, validator-2 transport-first, validator-3 rpc-first, validator-4 transport-first, validator-5 rpc-first;
accepted receipts: 6.
Fresh certified tip: `0fca5e0a38f53f8c9fa5da736f6d1c79b2c028d1c6d19ed0926e45fb4d2f33b95c15c1afa6b8b7aab8b58981036f8850`, root `549da9b1150dcc6e7a65905ad73594f1c9c9a1701adf82dfbd05b22ca26c3fd145db68e770c8058178541f8145959046`, height 1021.

The rollback executable is the deployed r4 binary, SHA-256
`44b6794f2f8eab577713dffb6e59880f8ee8c811863e99ed96ed1b0f4ec66bab`. It was copied from
`~/.local/lib/postfiat/releases/fastpay-committee-20260925-r4/`, and its hash was checked first.
On six restored originals it ran checkpoint verification and a service start and restart.
The new build then fully replayed restored validator-0. This is pre-activation rollback only.
The r4 full-replay failure at 1011 is its known historical defect and is not treated as repaired.

`warm-latency` hit its five-minute local budget while the history replays were running.
It is deferred to CI and makes no passing claim. On September 22 it passed in 299 seconds.

## CI

`gh run list --branch release/combined-fastpay-20260925` returns no runs
([observation](logs/ci-run-list.json)). `rust-ci`, `docs-build` and `product-security-ci` run only on
pushes to `main` and on pull requests. There is no CI verdict for this tip. The full workspace test
suite and the long Orchard suite were not run locally and still need CI. That requires a pull request
or a manual dispatch.

## Notes

- Task Node: task `task_9b22c73ab23432ca0e6d80deb2471180` (request `req_5885df8e…`) was generated by Task Node, inspected and accepted. Evidence is submitted after the final push, so it is not recorded here.
- The disk had 2.5 GB free at the start. `cargo clean --target-dir ~/.cache/signing-fix-qualification-20260909/test-target`
  removed that stale September 9 build cache (51 GiB, last used September 18) so the copies would fit.
  No database, evidence or protected checkout was removed.
- The first build and copy drivers were killed when their tool session ended. The build-1 Cargo process
  had already started; it kept running and finished. Build 1 was then rerun with the identical command in
  the same tree and target. Build 1's receipt is from that rerun. The rerun recompiled only `postfiat-node`,
  because `crates/node/build.rs` watches `.git/HEAD`, which a linked worktree does not have.
  The partial copy was set aside and all copies were redone
  ([interrupted logs](logs/interrupted-build-driver-first.log)).
- Limits: one Cargo process at a time, `CARGO_BUILD_JOBS=2`, two test threads, a 20 GiB address-space
  limit and a disk-backed `TMPDIR`. Disposable copies stay under `/home/postfiatchad/.cache/release-repair-20260925`.
  The protected release checkout was not accessed.
- Checkpoint pushes: `bf9d22a7` at 11:21Z and `99a4b2d5` at 11:32Z (75 minutes). The final commit supersedes both.
- `SHA256SUMS` covers every packet file except itself.
