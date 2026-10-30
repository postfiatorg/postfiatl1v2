# Next validator release plan (after vote-records-20261030)

Skeleton written 2026-10-30. The date suffix is a name, not a date, and no date is promised
([previous plan](next-validator-release-plan-20261108.md)).

## Summary

1. `vote-records-20261030` (main `ae70d66d`, executable `87cfe797…`) has run on all six validators since 2026-10-30 11:14Z. It was applied at 1162–1167, and the after state is 1168 / `a20e406e…` / `39771b74…`. It carries the VR-01 fix to the vote outcome records, live from 1168.
2. Riding along were the wallet app's private path, the kept snapshot points, the public finalized private-pool reads, the Asset-Orchard secret wiping, the removed `k256` node dependency and the transport ack timeout fix. The private path is present but not usable: admission is off, the pricing rules and egress registry are inert, and `private_readiness` reports not ready.
3. The rule is kept: a change that alters a consensus or state result gets its own qualification. Read-path and tooling changes ride along.
4. Qualification and rollout follow the current plan. Nothing starts before the "Not before" rules hold.

## Already on main past the cut

`git log --oneline --first-parent ae70d66d..origin/main` on 2026-10-30. None changes the node binary.

- [x] Docs only: `76dd4f86` and `85051b2b` (the cut and the deployment of `vote-records-20261030`), `7dcc23eb` (the live vote records as SCT-08 item 4 evidence) and `059776e3` (the Solana stake reader row).
- [x] `92425854`: the W-1 manifest and milestone record the deployed release's `private_readiness`. It changes a test fixture and a contract test only.

## Candidates

Included only when ready and reviewed by the cut; none is promised.

- [ ] The genesis external lane (branch `token/genesis-external-lane`, not on main on 2026-10-30), once merged. It is inert until a lane is configured, so it rides along. The qualification's history replays must show no changed result.
- [ ] The W-1 governance activations on the devnet: the pricing enforcement height, one governed A666/pfUSDC route, and the egress policy records and their switch. Each needs its own qualification. On 2026-10-30, `private_readiness` showed atomic batches active from 483, pricing enforcement absent, and two A666/pfUSDC routes where the read needs one ([milestone](user-proven-shielded-actions-milestone.md#s7-readiness-and-secret-inspection-12-days-left)).
- [ ] The admission switch (`--allow-private-action-admission` on `rpc-serve`). This is an operator step on the units after the activations, not a release.
- [ ] Carried over from the previous plan, the crypto-bigint rows ([previous plan](next-validator-release-plan-20261108.md#candidates)):
  - the k256 0.14 upgrade waits for upstream releases;
  - the Solana stake reader's lock update needs an identity re-pin, which is its own qualification.
- [ ] The other lane's delayed-precommit fix. It is a **consensus change and needs its own qualification**.
- [ ] Draft PRs: #39, #40 and #128 are open. #43–#46 from the earlier carry-over are closed.
- [ ] The previous plan's other open rows carry over: the storage packet check, Z2 step 1b, the SCT-08 campaign, and the vote records' signed-snapshot export and retention.

## Not before

- [x] `vote-records-20261030` deployed on all six validators. Done 2026-10-30 11:14Z.
- [ ] CI green on main at the cut commit (`docs-build`, `rust-ci`, `product-security-ci`).
- [ ] The full workspace suite at the cut commit (`rust-ci` `test` runs `cargo test --workspace --locked`).
- [ ] Any consensus or state candidate, such as a W-1 activation or the delayed-precommit fix, has its own qualification before the cut.

## Qualification and rollout

As in the current plan: [Qualification](next-validator-release-plan-20261005.md#qualification) (~90 min after the suite) and [Rollout](next-validator-release-plan-20261005.md#rollout) (~60 min, one validator at a time, `rollback-one.sh` back to `vote-records-20261030`, `87cfe797…`). A consensus candidate adds its own qualification before the cut.

If new SSH connections to a host hang, use a connection-reuse wrapper. On 2026-10-30, SSH to validator-2 hung with exit 255 before any change. The rollout continued with `ssh` and `scp` wrappers using `ControlMaster=auto`, a per-host `ControlPath`, `ControlPersist=120`, `ServerAliveInterval=10` and `ServerAliveCountMax=3`. `scp` ran as `-S` with the `ssh` wrapper. The wrapper changes only the transport; the commands stay the same ([rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/observed/rollout-record.json)).

- [ ] Cut: release name, main commit, branch, two identical clean builds.
- [ ] History: signed canaries and their exact replays, checkpoint and full replay, recorded roots.
- [ ] Six new-build services on disposable copies; the rollback build reads them.
- [ ] Signed manifest, units, `systemd-analyze verify`, focused suites of this release's changes.
- [ ] Canary backup, fleet before-state, rollout (release day, operator's go).
