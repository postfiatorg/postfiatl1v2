# Storage G4 timeout and persistent remediation

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-28 UTC

## BLUF

The [active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
still blocks public testnet. The selected transactional `redb` candidate did not
show the original full-history rescan in the four-hour G4 attempt, but that run
failed its time budget because the evidence harness launched and restarted node
processes once per setup block. Commit `438fb29c` replaces only that setup path
with one persistent six-validator batch loop; focused tests, a real 12-round
smoke, stop/resume, and helper-tamper rejection pass. G4 itself has **not**
passed: the next bounded action is one new, clean, checksum-bound v3
qualification run from height 1.

## Current state

- Repository branch is `main`. The implementation boundary is `438fb29c`
  (`test(storage): make setup advances persistent`). This handoff and the
  adjacent milestone correction are the documentation checkpoint; the exact
  clean revision used for the new run must be captured by its checkpoint rather
  than guessed from the implementation commit.
- The candidate remains source
  `ae65844190f153cbdd49d1e5ac28ab96a19f7af4` and release node-binary
  SHA-256
  `891bfb42ea16af844fd72351ee38a90eaeb8f4302492a8fd64ce0f3db5dcbbf4`.
  The node binary did not change during the harness remediation.
- The exhausted v2 output is
  `/home/postfiatchad/repos/postfiat-storage-g4-8768866a-ae658441-v1`.
  Its checkpoint SHA-256 is
  `5c48b9b8ad7d97ac9797a5b7c2f9388328cc82beebe2f7e1f31b334caa36ef85`.
  It ran from `2026-08-27T23:54:36Z` through
  `2026-08-28T04:10:24Z`, consumed 14,398.975 of 14,400 seconds, is
  `INTERRUPTED`, and has no final campaign report.
- The v2 run durably reached height 3,050 with prepared-fleet SHA-256
  `68d9e538d2f3727ab6a60db00238c9abaebfaad024c5eaf0f6aecde4a81dd8df`.
  Completed 1,500-round chunks reported p95 consensus times of 1,877.727 and
  1,928.697 ms, six-validator convergence, literal accepted receipts, and zero
  full-history reads. The partial next unit reached height 4,542 with diagnostic
  p95 1,928.775 ms and zero full-history reads. That partial unit is not release
  evidence.
- The timeout was a harness problem: setup launched one foreground node and
  restarted resident validator services for each block. It was not evidence
  that the transactional append path resumed full-chain rescans.
- The remediation adds the release helper
  `postfiat-storage-corpus-batches` and keeps validator processes resident for
  setup-only advances. Canonical batches, certificates, every processed round,
  helper/build identity, runner identity, resources, receipts, and before/after
  fleet digests are bound in v3 checkpoints and packets. The three measured G4
  rows still use the previous measurement path.
- Two Rust helper tests and 60 focused Python tests passed, along with
  warnings-denied release clippy and formatting checks.
- A real six-validator persistent smoke advanced height 1 to 13 in one
  foreground process in 5,655.333 ms. It recorded 72 committed writes, 4,896
  page reads, 576 page writes, zero full-history records or bytes, and
  six-validator convergence. Raw report SHA-256 is
  `4a976603449edc16b1d48418d09f56865eedaefb08c1d0530eb9f1732a3535a4`.
- The v3 development campaign report SHA-256 is
  `e1e87b854dc4fbae88f00e3ebfe9c4c848706090cd52d6c394c5cd2b8ea8518f`.
  The controlled stop/resume report SHA-256 is
  `f8501bf2fa15d1fc5a5714f3f5a476b28c6419f250fed9176ebb3f4e0f1abc87`.
  Resume also refused an intentionally changed batch-builder binary before
  executing another unit. These are development proofs, not G4 evidence.
- The helper currently in the primary `target/release` directory embeds the
  pre-commit development revision and must **not** be used for the release run.
  Rebuild it from the clean documentation checkpoint.
- The old v2 run cannot be resumed under v3 and must not receive another budget.
  Start one unique v3 output from height 1.
- No campaign or validator process was alive at handoff inspection. No Task
  Node, subagent, controlled-devnet query, data copy, service action,
  deployment, or mutation was used.
- The last authenticated controlled-devnet observation remains the point-in-time
  height-924 capture from `2026-08-26T06:34:55Z`–`06:35:50Z`; see
  [Current State](../status/chain-state-current.md). This session performed no
  live probe, and the storage candidate is not deployed.

## Next decision or action

Commit and push this documentation checkpoint. From that clean revision, rebuild
the setup helper, verify both binary identities, and pin a detached runner
worktree:

```bash
cd /home/postfiatchad/repos/postfiatl1v2
export CARGO_FEATURE_PURE=1
export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/home/postfiatchad/.local/bin/zig-cc
cargo build --release --locked -p postfiat-bench \
  --bin postfiat-storage-corpus-batches
sha256sum target/release/postfiat-node \
  target/release/postfiat-storage-corpus-batches
git status --porcelain
STORAGE_RUNNER_REV="$(git rev-parse HEAD)"
git worktree add --detach \
  "/home/postfiatchad/repos/postfiat-storage-runner-${STORAGE_RUNNER_REV:0:8}" \
  "$STORAGE_RUNNER_REV"
```

The candidate node hash must remain `891b…bf4`, the helper must report the
same embedded revision as the clean runner, the worktree must be clean, and the
new output path must not exist. Then start exactly one v3 output, with the first
unattended segment capped at two hours:

```bash
cd "/home/postfiatchad/repos/postfiat-storage-runner-${STORAGE_RUNNER_REV:0:8}"
timeout --signal=INT --kill-after=120s 7200s \
  python3 -u benchmarks/storage-scaling/run_paired_campaign.py \
  --node-bin /home/postfiatchad/repos/postfiatl1v2/target/release/postfiat-node \
  --batch-builder-bin \
    /home/postfiatchad/repos/postfiatl1v2/target/release/postfiat-storage-corpus-batches \
  --expected-source-revision \
    ae65844190f153cbdd49d1e5ac28ab96a19f7af4 \
  --output-dir \
    "/home/postfiatchad/repos/postfiat-storage-g4-${STORAGE_RUNNER_REV:0:8}-ae658441-v3"
```

If that segment stops cleanly, independently verify the checkpoint, confirm no
child process survived, and resume the same output for at most the remaining
two-hour segment. Do not create a second output. If the campaign passes, verify
the final report requirement by requirement before closing G4 and packaging the
locally available G2/G4 material. Exact height-924 replay still requires a
separately authorized read-only validator-directory copy; G6 still requires
separate authorization for six distinct stopped copies.

## References

- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage-scaling evidence workflow](https://github.com/postfiatorg/postfiatl1v2/tree/438fb29c/benchmarks/storage-scaling)
- [Storage scaling fix specification](../architecture/storage-scaling-fix-spec.md)
- [Locked storage scaling research specification](../architecture/storage-scaling-research-spec.md)
- [State and storage architecture](../architecture/state-and-storage.md)
- [Independent storage candidate review](2026-08-27___dravlic__storage_candidate_review.md)
- [Prior G4 checkpoint, now historical](2026-08-28___postfiatchad__storage_g4_qualification_checkpoint.md)
- [Current State](../status/chain-state-current.md)
