# Next validator release plan (after vote-records-20261030)

Skeleton written 2026-10-30; pointed at the separately qualified next release on 2026-11-03; its base moved to `76bc12fea` on 2026-11-04. The date suffix is a name, not a date, and no date is promised
([previous plan](next-validator-release-plan-20261108.md)).

## Summary

1. The next validator release is the one prepared on a private branch, with its own qualification. On 2026-11-04 it was rebuilt on a branch that contains main up to `76bc12fea` (2026-11-03, the end-of-session note), so it carries everything on main up to that commit, including the genesis external lane and the inbound release path ([list](#in-the-next-release)). Its qualification and rollout are its own, not this plan's.
2. `vote-records-20261030` (main `ae70d66d`, executable `87cfe797…`) has run on all six validators since 2026-10-30 11:14Z. It was applied at 1162–1167, and the after state is 1168 / `a20e406e…` / `39771b74…`. It carries the VR-01 fix to the vote outcome records, live from 1168. Riding along were the wallet app's private path, the kept snapshot points, the public finalized private-pool reads, the Asset-Orchard secret wiping, the removed `k256` node dependency and the transport ack timeout fix. The private path is present but not usable: admission is off, the pricing rules and egress registry are inert, and `private_readiness` reports not ready.
3. The commits on main after `76bc12fea` are not in the candidate ([list](#already-on-main-past-the-cut)). One changes the node binary and a state result: the pruning of the external lane release records (`f53e7457c`), which waits for the release after with its own qualification. The others change no node binary (docs, CI, Solidity, Python, tests) and ride along.
4. The other candidates move to the release after, unchanged. The rule is kept: a change that alters a consensus or state result gets its own qualification. Read-path and tooling changes ride along.

## Already on main past the cut

`git log --oneline --first-parent 76bc12fea..origin/main` on 2026-11-04, checked with `git show --stat`. None is in the candidate.

- [x] `2574c1cbe`: the external lane release read in the Python CLI's method choices, and the contracts CI job's lint setting. Python, `crates/pft-token-contracts/foundry.toml` and docs; no node change, rides along.
- [ ] `f53e7457c`: the pruning of the external lane release records (branch commits `f301e0225` to `f4839b1ff`, and `3875bdc92`). It changes the lane state rules and their committed fields in `crates/types` and the paged lane read in `crates/node`, so the node binary and a state result: it waits for the release after, with its own qualification.
- [x] `8351ac551`: the Base lock contract's minimum lock amount (branch commits `e5fa5dfa1` to `ad8f3db32`, and `8cb904585`). Solidity, the deploy script, docs and a `#[cfg(test)]` module in `crates/execution` (`lock_minimum_binding_tests`); no node change, rides along.
- [x] `76d0935e5` and `d3611b9aa`: the ERC-20 milestone ticks and the record that the Base lane needs no proof program of its own. Docs only.
- [x] `61027a7fa`: the CI runner disk step and the smaller debug test build. CI only.

## Candidates

### In the next release

- [x] Everything on main up to `7a91bb30`: the genesis external lane (`bfe448416`, 2026-10-30), and after the `vote-records-20261030` cut the docs commits `76dd4f86`, `85051b2b`, `7dcc23eb` and `059776e3` and the `private_readiness` fixture record `92425854`.
- [x] `git log --oneline --first-parent 7a91bb30..76bc12fea`. Node changes:
  - `8f29aaccb`: the inbound release path for the ERC-20 slice (`external_lane_release_v1`), with its milestone tick `9ae46a531` and qualification run record `4f5aeb763`;
  - `e80ee8f9f`: initialising a chain from a genesis that carries external lanes;
  - `b26de17a9`: governed external lane amendments refused until they are built;
  - `094f180df` and `5eb305627`: the failing test and the fix for the round service's rejection of a batch that a block commit is including;
  - `f03dd41a8`: the readiness admission-switch test's verifying-key build, test code in `crates/node/src/private_action_rpc.rs` only.
- [x] The same range, no node change: `1e2cc55f8` and `8ec313e19` (the lock deployment's anchor and binding checks, contract tooling, and their milestone record), `85340e84a` (the wallet proxy's certification through the validator-owned admission: proxy, compose file and docs), and the docs commits `ba0c58f32`, `49aeca2be`, `fc6f9a061`, `ccf86448a`, `a6fa02399` and `76bc12fea`.

### For the release after

Included only when ready and reviewed by its cut; none is promised.

- [ ] The pruning of the external lane release records (`f53e7457c`, on main). It changes a state result and **needs its own qualification**.
- [ ] The W-1 governance activations on the devnet: the pricing enforcement height, one governed A666/pfUSDC route, and the egress policy records and their switch. Each needs its own qualification. On 2026-10-30, `private_readiness` showed atomic batches active from 483, pricing enforcement absent, and two A666/pfUSDC routes where the read needs one ([milestone](user-proven-shielded-actions-milestone.md#s7-readiness-and-secret-inspection-12-days-left)).
- [ ] The admission switch (`--allow-private-action-admission` on `rpc-serve`). This is an operator step on the units after the activations, not a release.
- [ ] Remove the wallet proxy's legacy shielded certification (W-1 S3.3, [milestone](user-proven-shielded-actions-milestone.md#s3-the-validator-owned-admission-and-round-service-24-days-left)). Its switch defaults to `off` since S3.3; removal waits for the app on `/api/private/v1/` (S2.4) and the public deployment set to `off`. Proxy only, no node change.
- [ ] Carried over from the previous plan, the crypto-bigint rows ([previous plan](next-validator-release-plan-20261108.md#candidates)):
  - the k256 0.14 upgrade waits for upstream releases;
  - the Solana stake reader's lock update needs an identity re-pin, which is its own qualification.
- [ ] The other lane's delayed-precommit fix. It is a **consensus change and needs its own qualification**.
- [ ] Draft PRs: #39, #40 and #128 are open. #43–#46 from the earlier carry-over are closed.
- [ ] The previous plan's other open rows carry over: the storage packet check, Z2 step 1b, the SCT-08 campaign, and the vote records' signed-snapshot export and retention.

## Not before

The next release's qualification is the private branch's: CI green on its tip, the replay on copies of the validators' data, and the rehearsals. Its rollout follows its own runbook, a coordinated halt of all six validators, not this plan's rolling [Rollout](#qualification-and-rollout).

- [x] `vote-records-20261030` deployed on all six validators. Done 2026-10-30 11:14Z.
- [ ] The next release qualified on its private branch: CI green on its tip, the data-copy replay, the rehearsals.
- [ ] The next release deployed on all six validators under its own runbook.

For the release after:

- [ ] The commits on main after `76bc12fea` taken into this cut, the pruning merge with its own qualification.
- [ ] CI green on main at the cut commit (`docs-build`, `rust-ci`, `product-security-ci`).
- [ ] The full workspace suite at the cut commit (`rust-ci` `test` runs `cargo test --workspace --locked`).
- [ ] Any consensus or state candidate, such as a W-1 activation or the delayed-precommit fix, has its own qualification before the cut.

## Qualification and rollout

For the release after; once the next release is deployed, it replaces `vote-records-20261030` as the rollback target.

As in the current plan: [Qualification](next-validator-release-plan-20261005.md#qualification) (~90 min after the suite) and [Rollout](next-validator-release-plan-20261005.md#rollout) (~60 min, one validator at a time, `rollback-one.sh` back to `vote-records-20261030`, `87cfe797…`). A consensus candidate adds its own qualification before the cut.

If new SSH connections to a host hang, use a connection-reuse wrapper. On 2026-10-30, SSH to validator-2 hung with exit 255 before any change. The rollout continued with `ssh` and `scp` wrappers using `ControlMaster=auto`, a per-host `ControlPath`, `ControlPersist=120`, `ServerAliveInterval=10` and `ServerAliveCountMax=3`. `scp` ran as `-S` with the `ssh` wrapper. The wrapper changes only the transport; the commands stay the same ([rollout record](https://github.com/postfiatorg/postfiatl1v2/blob/release/vote-records-20261030/deployments/vote-records-20261030/observed/rollout-record.json)).

- [ ] Cut: release name, main commit, branch, two identical clean builds.
- [ ] History: signed canaries and their exact replays, checkpoint and full replay, recorded roots.
- [ ] Six new-build services on disposable copies; the rollback build reads them.
- [ ] Signed manifest, units, `systemd-analyze verify`, focused suites of this release's changes.
- [ ] Canary backup, fleet before-state, rollout (release day, operator's go).
