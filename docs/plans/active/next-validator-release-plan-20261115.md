# Next validator release plan (after vote-records-20261030)

Skeleton written 2026-10-30; pointed at the separately qualified next release on 2026-11-03; its base moved to `76bc12fea` on 2026-11-04. The date suffix is a name, not a date, and no date is promised
([previous plan](next-validator-release-plan-20261108.md)).

## Summary

1. The next validator release is the one prepared on a private branch, with its own qualification. On 2026-11-04 it was rebuilt on a branch that contains main up to `76bc12fea` (2026-11-03, the end-of-session note), so it carries everything on main up to that commit, including the genesis external lane and the inbound release path ([list](#in-the-next-release)). Its qualification and rollout are its own, not this plan's.
2. `vote-records-20261030` (main `ae70d66d`, executable `87cfe797…`) has run on all six validators since 2026-10-30 11:14Z. It was applied at 1162–1167, and the after state is 1168 / `a20e406e…` / `39771b74…`. It carries the VR-01 fix to the vote outcome records, live from 1168. Riding along were the wallet app's private path, the kept snapshot points, the public finalized private-pool reads, the Asset-Orchard secret wiping, the removed `k256` node dependency and the transport ack timeout fix. The private path is present but not usable: admission is off, the pricing rules and egress registry are inert, and `private_readiness` reports not ready.
3. The commits on main after `76bc12fea` are not in the candidate ([list](#already-on-main-past-the-cut)). Three change the node binary and a state result: the pruning of the external lane release records (`f53e7457c`), the governed release cap (`f2dedd257`) and the governed committee move (`be09a97f1`), which wait for the release after with their own qualification. The readiness custody facts (`59b1d8a18`), the Base checkpoint source (`08d6e03f6`) the validator signer process (`b610061fc`) and its double-sign guard (`fcfdf595e`, `f86d501bb`) change the node binary but no consensus or state result and ride along. The others change no node binary (docs, CI, Solidity, Python, tests) and ride along.
4. The other candidates move to the release after, unchanged. The rule is kept: a change that alters a consensus or state result gets its own qualification. Read-path and tooling changes ride along.

## Already on main past the cut

`git log --oneline --first-parent 76bc12fea..origin/main` on 2026-11-04 and again on 2026-11-05, checked with `git show --stat`. None is in the candidate.

- [x] `2574c1cbe`: the external lane release read in the Python CLI's method choices, and the contracts CI job's lint setting. Python, `crates/pft-token-contracts/foundry.toml` and docs; no node change, rides along.
- [ ] `f53e7457c`: the pruning of the external lane release records (branch commits `f301e0225` to `f4839b1ff`, and `3875bdc92`). It changes the lane state rules and their committed fields in `crates/types` and the paged lane read in `crates/node`, so the node binary and a state result: it waits for the release after, with its own qualification.
- [x] `8351ac551`: the Base lock contract's minimum lock amount (branch commits `e5fa5dfa1` to `ad8f3db32`, and `8cb904585`). Solidity, the deploy script, docs and a `#[cfg(test)]` module in `crates/execution` (`lock_minimum_binding_tests`); no node change, rides along.
- [x] `76d0935e5` and `d3611b9aa`: the ERC-20 milestone ticks and the record that the Base lane needs no proof program of its own. Docs only.
- [x] `61027a7fa`: the CI runner disk step and the smaller debug test build. CI only.
- [x] `08d6e03f6`: the Base checkpoint source of the external lane (branch commits `5410d3ec7` to `4da5f4d61`). New node CLI commands (`external-lane-checkpoint-observe`, `-sign`, `-assemble`, `external-lane-lock-proof`, `crates/node/src/external_lane_checkpoint_source.rs`), a refactor of the Ethereum receipt-proof builder that also encodes OP Stack deposit receipts, and the `Locked` event signature exported from `crates/bridge`. It changes the node binary but no consensus or state result: the verifier `verify_external_lane_lock` and execution are untouched, and the commands only write files an operator submits. Tooling, rides along.
- [ ] `f2dedd257`: the governed release cap amendment of the external lane. Execution, admission and the lane equality rule read the cap amendments (`crates/execution`, `crates/node`), so the node binary and a state result: it waits for the release after, with its own qualification.
- [ ] `be09a97f1`: the governed committee epoch move of the external lane. Execution and admission select the lane's committee from the committed moves, so the node binary and a state result: it waits for the release after, with its own qualification.
- [x] `59b1d8a18`: the private readiness read's custody facts. A read path in `crates/node`; no consensus or state result, rides along.
- [x] `1599fb4b1`: a comment in the readiness read. No behaviour change, rides along.
- [x] `9251541ef` and `b319b4d2c`: pull request #185 and its review fixes, a genesis validation rule in `crates/types` that refuses a lane whose window cap rounds down to zero. Only a genesis with lanes is affected and no deployed genesis has lanes, so no state result changes on the fleet: rides along.
- [x] `b610061fc`: the validator signer process for rpc-serve (branch commits `b9b76e848` to `d6fd52338`). It changes the node binary, rpc-serve's signing path and the readiness read, but no consensus or state result: the signatures are the same bytes under the same contexts. Rides along with the next release. A two-user deployment needs a signer unit and an edited rpc unit, which is an operator step.
- [x] `0efe752a5`: the reader's page for the PFT token on Base. Docs only.
- [x] `5fc1e2374`, `264d0f6e7`, `7527df06a`, `739d49527`, `db2de7c0b`, `c43eaa27a`, `d4a1f42b7`, `a7347213f`, `6d9aa57cd` and this plan's update: the release plan, milestone ticks, the amendment rehearsal record, a handoff and the qualification records. Docs only.
- [x] `84b4ea41c`: a pull merge commit with nothing of its own.
- [x] `7a968007e`: the Phase 3 export design and the refused export operation (branch commits `edc74bbc6` to `e5caa1a54`). The node now parses `external_lane_export_v1` and refuses it at admission, batch building and execution with a typed code; an older binary refuses it at decode as an unknown operation. Both refuse, no state result changes and no network configures a lane: rides along. The parse set grows by one refused kind.
- [x] `313cf3926`: the crypto call-site policy script records the validator signer's seeded signing sites. CI script only, rides along.
- [x] `41bde8b8a`: the external lane watcher (branch commits `cea372c1b` to `ddd1713f9`). Python (`python/postfiat_rpc/external_lane_watcher.py` and its tests) and docs only; no node change, rides along.
- [x] `fcfdf595e` and `f86d501bb`: pull request #192 (branch commits `8b763dca2` to `5fe55f6cf`) and its review fixes (`0aedc2f42`, `e6b04ce41`, `89f9abbb4`, `e1448c8fb`, `2815dc26a`, `338e64326`, `e541cb168`, `7a8218c5b`, `af3adae6e`): the validator signer's durable double-sign guard, its readiness reasons and the Python deployment checker. It changes the node binary but nothing signed: the guard only refuses. Rides along. The guard's initialization (with the `--min-height` floor for an existing key), the checker run and the signer and rpc unit files are operator steps.
- [ ] The two-user units in the release packet (branch `w1/signer-two-user-units`, under review): `deployment-validator-units-stage --signer-shape two-user` and the deploy sheet template. Generator and docs only; the default single-user units are unchanged byte for byte, so no runtime change for the current units. Rides along once merged.

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
- [ ] The governed release cap and committee move of the external lane (`f2dedd257`, `be09a97f1`, on main). They change state results and **need their own qualification**.
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
