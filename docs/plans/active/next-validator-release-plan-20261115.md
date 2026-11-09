# Next validator release plan (after vote-records-20261030)

Skeleton written 2026-10-30; pointed at the separately qualified next release on 2026-11-03; its base moved to `76bc12fea` on 2026-11-04. The date suffix is a name, not a date, and no date is promised
([previous plan](next-validator-release-plan-20261108.md)).

## Summary

Documentation review, 2026-11-06: public main `4b4da1b91` includes the merged
documentation refresh (PR190), local-EVM rehearsal (PR191), signer guard (PR192)
and their review fixes. Its [docs build](https://github.com/postfiatorg/postfiatl1v2/actions/runs/42223603082),
[Rust CI](https://github.com/postfiatorg/postfiatl1v2/actions/runs/42223602975) and
[product security CI](https://github.com/postfiatorg/postfiatl1v2/actions/runs/42223603181)
all completed successfully. These results supersede the handoff's pending-CI
observation for that public source. They do not qualify another release cut or
constitute a deployment receipt. The [latest reported fleet observation](../../status/chain-state-current.md)
still has `vote-records-20261030` on all six validators.

1. The next validator release is the one prepared on a private branch, with its own qualification. On 2026-11-04 it was rebuilt on a branch that contains main up to `76bc12fea` (2026-11-03, the end-of-session note), so it carries everything on main up to that commit, including the genesis external lane and the inbound release path ([list](#in-the-next-release)). Its qualification and rollout are its own, not this plan's.
2. `vote-records-20261030` (main `ae70d66d`, executable `87cfe797…`) has run on all six validators since 2026-10-30 11:14Z. It was applied at 1162–1167, and the after state is 1168 / `a20e406e…` / `39771b74…`. It carries the VR-01 fix to the vote outcome records, live from 1168. Riding along were the wallet app's private path, the kept snapshot points, the public finalized private-pool reads, the Asset-Orchard secret wiping, the removed `k256` node dependency and the transport ack timeout fix. The private path is present but not usable: admission is off, the pricing rules and egress registry are inert, and `private_readiness` reports not ready.
3. The commits on main after `76bc12fea` are not in the candidate ([list](#already-on-main-past-the-cut)). Eight changes (nine merges) wait for the release after with their own qualification. Five change the node binary and a state result: the pruning of the external lane release records (`f53e7457c`), the governed release cap (`f2dedd257`), the governed committee move (`be09a97f1`), the refused export operation (`7a968007e`) and the export accounting behind `exports_enabled` (`f751185c7`). Three change the process that holds the validator key or its stored state: the validator signer process (`b610061fc`), its double-sign guard (`fcfdf595e`, `f86d501bb`) and the two-user deployment shape (`7651635f6`). Nine change the node binary but no execution, state commitment, signed bytes or key-holding process, and ride along: the readiness custody facts (`59b1d8a18`), the zero-cap lane genesis rule (`9251541ef`, `b319b4d2c`), the Base checkpoint source (`08d6e03f6`), the transfer fee quote on the pending ledger (`3ac4adc65`), the escrow and offer quotes' native lock (`600cafb38`) the escrow finish history row with the `account_tx` index v4 (`66c110dfa`), whose index needs a one-time rebuild on release day, the in-process `server_info` (`9f76e2e5e`), the transfer quote priced on the entries that run before it (`6cf815b23`) and every fee quote and admission path started from admission's ledger (`693db9712`, `68331646d`). The others change no node binary (docs, CI, Solidity, Python, tests) and ride along.
4. The other candidates move to the release after, unchanged. The rule is kept: a change of execution, the state commitment, what is signed, or the process that holds a key or its stored state gets its own qualification. Records, docs, tooling, tests and read paths ride along.

## Already on main past the cut

`git log --oneline --first-parent 76bc12fea..origin/main` on 2026-11-04, 2026-11-05, 2026-11-06 and 2026-11-09, checked with `git show --stat`. None is in the candidate.

- [x] `2574c1cbe`: the external lane release read in the Python CLI's method choices, and the contracts CI job's lint setting. Python, `crates/pft-token-contracts/foundry.toml` and docs; no node change, rides along.
- [ ] `f53e7457c`: the pruning of the external lane release records (branch commits `f301e0225` to `f4839b1ff`, and `3875bdc92`). It changes the lane state rules and their committed fields in `crates/types` and the paged lane read in `crates/node`, so the node binary and a state result: it waits for the release after, with its own qualification ([row](#for-the-release-after)).
- [x] `8351ac551`: the Base lock contract's minimum lock amount (branch commits `e5fa5dfa1` to `ad8f3db32`, and `8cb904585`). Solidity, the deploy script, docs and a `#[cfg(test)]` module in `crates/execution` (`lock_minimum_binding_tests`); no node change, rides along.
- [x] `76d0935e5` and `d3611b9aa`: the ERC-20 milestone ticks and the record that the Base lane needs no proof program of its own. Docs only.
- [x] `61027a7fa`: the CI runner disk step and the smaller debug test build. CI only.
- [x] `08d6e03f6`: the Base checkpoint source of the external lane (branch commits `5410d3ec7` to `4da5f4d61`). New node CLI commands (`external-lane-checkpoint-observe`, `-sign`, `-assemble`, `external-lane-lock-proof`, `crates/node/src/external_lane_checkpoint_source.rs`), a refactor of the Ethereum receipt-proof builder that also encodes OP Stack deposit receipts, and the `Locked` event signature exported from `crates/bridge`. It changes the node binary but no consensus or state result: the verifier `verify_external_lane_lock` and execution are untouched, and the commands only write files an operator submits. Its `external-lane-checkpoint-sign` command signs a lane checkpoint vote with the validator's committee key as an operator action, outside the running node. Tooling, rides along.
- [ ] `f2dedd257`: the governed release cap amendment of the external lane. Execution, admission and the lane equality rule read the cap amendments (`crates/execution`, `crates/node`), so the node binary and a state result: it waits for the release after, with its own qualification ([row](#for-the-release-after)).
- [ ] `be09a97f1`: the governed committee epoch move of the external lane. Execution and admission select the lane's committee from the committed moves, so the node binary and a state result: it waits for the release after, with its own qualification ([row](#for-the-release-after)).
- [x] `59b1d8a18`: the private readiness read's custody facts. A read path in `crates/node`; no consensus or state result, rides along.
- [x] `1599fb4b1`: a comment in the readiness read. No behaviour change, rides along.
- [x] `9251541ef` and `b319b4d2c`: pull request #185 and its review fixes, a genesis validation rule in `crates/types` that refuses a lane whose window cap rounds down to zero. Only a genesis with lanes is affected and no deployed genesis has lanes, so no state result changes on the fleet: rides along.
- [ ] `b610061fc`: the validator signer process for rpc-serve (branch commits `b9b76e848` to `d6fd52338`). It changes the node binary, rpc-serve's signing path and the readiness read; the signatures are the same bytes under the same contexts, but with `--finality-signer-socket` the key moves into a separate signer process, so the process that holds the key changes: it waits for the release after, with its own qualification ([row](#for-the-release-after)). A two-user deployment needs a signer unit and an edited rpc unit, which is an operator step.
- [x] `0efe752a5`: the reader's page for the PFT token on Base. Docs only.
- [x] `5fc1e2374`, `264d0f6e7`, `7527df06a`, `739d49527`, `db2de7c0b`, `c43eaa27a`, `d4a1f42b7`, `a7347213f`, `6d9aa57cd`, `3a1978cdf`, `e7749f414`, `557b6bf52`, `1c5bf4c6f`, `5a69b481a`, `22db9507a`, `1d886cb6d`, `05fe9c37a`, `8daa7d4de`, `786d551d8`, `76716865e`, `65a889432`, `af592e98d`, `f05c2f7ea` and this plan's update: the release plan, milestone ticks, the amendment rehearsal record, handoffs, the qualification records, the contracts README's lock section and the Phase 2 design note's corrections. Docs only.
- [x] `84b4ea41c`: a pull merge commit with nothing of its own.
- [ ] `7a968007e`: the Phase 3 export design and the refused export operation (branch commits `edc74bbc6` to `e5caa1a54`). The node now parses `external_lane_export_v1` and refuses it at admission, batch building and execution with a typed code. An older binary refuses the kind at decode while this one burns the fee of a refused export in a certified block once the rejected-fee rule is active, so the two could disagree on such a block: it waits for the release after with `f751185c7`, with its own qualification ([row](#for-the-release-after)).
- [x] `313cf3926`: the crypto call-site policy script records the validator signer's seeded signing sites. CI script only, rides along.
- [x] `41bde8b8a`: the external lane watcher (branch commits `cea372c1b` to `ddd1713f9`). Python (`python/postfiat_rpc/external_lane_watcher.py` and its tests) and docs only; no node change, rides along.
- [x] `a98607527`: the audit package for the PFT token and lock contracts (branch commits `ded11d4fb`, `c2d8f08e5`; `docs/listings/coinbase/pft-token-audit-package-20261106.md`). Documentation only: rides along.
- [x] `6cfee928a`: the rehearsal token's and lock's explorer verification (Sourcify exact match, Blockscout fully verified) in their deployment records and on the PFT-on-Base page, the corrected lock deploy order in the contracts README, and the Python client's `export_limit` bound like `release_limit` (branch commits `2757ab334`, `abc4fb794`, `93d30711a`; the audit package's table refreshed in `6463b9aa4`). Records, a page sentence and a Python client bound: rides along.
- [ ] `f751185c7`: the external lane export accounting behind `exports_enabled`. It changes execution and the state commitment for a lane with exports on: it waits for the release after, with its own qualification ([row](#for-the-release-after)).
- [ ] `fcfdf595e` and `f86d501bb`: pull request #192 (branch commits `8b763dca2` to `5fe55f6cf`) and its review fixes (`0aedc2f42`, `e6b04ce41`, `89f9abbb4`, `e1448c8fb`, `2815dc26a`, `338e64326`, `e541cb168`, `7a8218c5b`, `af3adae6e`): the validator signer's durable double-sign guard, its readiness reasons and the Python deployment checker. Nothing signed changes (the guard only refuses), but the key-holding process's behaviour and its stored state (the durable guard records) do: it waits for the release after, with its own qualification ([row](#for-the-release-after)). The guard's initialization (with the `--min-height` floor for an existing key), the checker run and the signer and rpc unit files are operator steps.
- [x] `3c84580b7` and `d96dfe294`: pull request #191 (branch commit `02024aafd`) and its review fixes (`8e247d49f` to `643dc457f`), the local EVM Lock rehearsal of the external lane: `scripts/rehearse-base-lock.py`, an ignored composition test, a test fixture generalisation and its records. Test tooling and records only, no production code: rides along, no qualification needed.
- [x] `76c278f22`: the first real Base Sepolia Lock and its release on a local six-validator fleet (branch commits `11da444ae` to `14bf1f8d7`). Records, deployment files and a test path; nothing in production: rides along.
- [x] `620af6da6` and `7334b82ad`: pull request #186 (branch commit `0894869c6`) and its completion, the external lane release check table in the order the checks run. Docs, and comments only in `crates/types/src/external_lane_types.rs`; no behaviour change, rides along.
- [x] `3198674a8`: pull request #189 (branch commit `74402b7c7`), the Python client's bound on the external lane release page limit like the other paged reads. The Python client and its tests only; rides along.
- [x] `0d6b7a8d5` and `e5a508e73`: pull request #190 (branch commit `a006b86dc`) and its correction to today's main, the refreshed current guidance that distinguishes implemented, qualified and deployed state. Docs only, rides along.
- [ ] `7651635f6`: the two-user validator signer deployment shape (branch commits `c23175cdd` to `dae568ce2`). It changes the transport validator's runtime: it waits for the release after, with its own qualification ([row](#for-the-release-after)).
- [x] `f897d5526` and `a11e8d12c`: pull request #196 (branch commits `130989b7b` to `acf851839`) and its review fixes (`44088b95a` to `1ae408854`): the gateway's public private reads, its RPC connection lifecycle, typed transport errors and a 2-second shared cache for the four private reads. `wallet-proxy/` and docs only; the node binary is untouched, rides along. The content first landed as twelve replayed commits, `91d418187` to `ff0713595`; the two merges record the pull request's history.
- [x] `6f4a1c2df`, `3ac4adc65` and `b47ae08d6`: issue #193 with its reproduction pull request #194. `transfer_fee_quote` and `mempool-submit-transfer` price a transfer on the ledger after the executable mempool, as admission and the other quotes do (`crates/node/src/lifecycle_queries.rs`, `crates/node/src/mempool_proposals.rs`). The node binary's RPC answer and the CLI's declared fee change; execution, admission and the state do not: rides along. For a second transfer to a recipient that a pending transfer creates, the quoted and declared fee drops by the creation fee (10 atoms).
- [x] `7ecf15adf`: the SP1 installer fetched from its pinned release tag in two workflows. CI only.
- [x] `a20742ea1`, `600cafb38` and `31448c570`: issue #197 with its reproduction pull request #199. `escrow_fee_quote` and `offer_fee_quote` count the native PFT that an escrow create or a native-sell offer locks: two new response fields (`sender_balance_after_amount_and_fee`, `sender_balance_after_amount_fee_and_reserve`), and two flags stricter (`sender_meets_reserve_after_fee` for escrows, `sender_meets_reserve_after_fee_and_reserve` for offers). Read paths only, no consensus or state result: rides along.
- [x] `4a0bdd3ca`: the web wallet's offer send also blocks on `sender_meets_reserve_after_fee_and_reserve`. `wallet-web/` only, not the node: rides along.
- [x] `7866229b9` and `66c110dfa`: issue #198 with its reproduction pull request #200. The `account_tx` row of an `escrow_finish` names the recipient, who signed and paid it, in the node's row builder and the Python fallback, and the `account_tx` index schemas move from v3 to v4. A read path and the node's local history index; no consensus or state result: rides along. **Operator step on release day:** every node that serves `account_tx` runs `postfiat-node account-tx-index-build` once after deploying, because an old index is reported unusable and `account_tx` answers from the bounded archive scan until then ([runbook](../../runbooks/account-tx-index.md)).
- [x] `6cf815b23`, `2da6c6d74` and `d0e163d2f`: the review fix round of the three quote and history fixes.
  - `6cf815b23`: the transfer quote and `mempool-submit-transfer` price a plain transfer on the pending entries that run before it, through one helper with admission. A quote read path, like its parent `3ac4adc65`: rides along.
  - `2da6c6d74`: tests for issued-asset escrows and offers, and the web wallet's escrow message. Tests and `wallet-web/`: rides along.
  - `d0e163d2f`: the escrow history rows' amount statements. Docs, a code comment, a test assertion and a Python comment; no behaviour change: rides along.
- [x] `693db9712` and `68331646d`: every fee quote, including `atomic_swap_fee_quote`, and every admission path start from one helper, `admission_base_ledger`. It returns the committed ledger plus the confirmed FastPay effects not yet in a block, which is what admission already read. A test with a confirmed, uncommitted FastPay unwrap pins it. A quote read path; admission reads the same ledger as before: rides along.
- [x] `8a6216a84`: the Python client's 30 s timeout for `server_info`, and docs. No node change: rides along.
- [x] `9f76e2e5e`: rpc-serve answers `server_info` in-process, from its cached status, one governance read and the constant fees, instead of a child process that read the whole stored history (6.6–12 s on the devnet on 2026-11-09). The response is the same field for field (a comparison test, and old against new `rpc-serve` on one fixture). A node read path: no state result, nothing signed, and rpc-serve's key path is untouched: rides along. The Python client's 30 s allowance for `server_info` (`8a6216a84`) stays until a deployed release carries this.

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
- [ ] The validator signer process for rpc-serve (merge `b610061fc`). The signatures stay byte-identical, but with `--finality-signer-socket` the key moves from rpc-serve into a separate signer process, so it **needs its own qualification**.
- [ ] The validator signer's double-sign guard (merges `fcfdf595e`, `f86d501bb`). It signs nothing new, but it changes the key-holding process's behaviour and its stored state, so it **needs its own qualification**.
- [ ] The two-user validator signer deployment shape (merge `7651635f6`; branch commits `c23175cdd` to `dae568ce2`): `deployment-validator-units-stage --signer-shape two-user`, `transport-validator-serve --finality-signer-socket` and the deploy sheet template; the default single-user units are unchanged byte for byte. What is signed stays byte-identical, but the process that holds the key changes, so it **needs its own qualification** and does not ride along. The signer unit and its environment file are outside the signed bindings of `deployment-manifest-verify`; closing that is a change to `deployment-manifest-verify` for the release after (no code today).
- [ ] The refused external lane export operation (merge `7a968007e`). An older binary refuses the kind at decode while this one burns the fee of a refused export in a certified block once the rejected-fee rule is active, so the two could disagree on such a block; it **needs its own qualification**, with the export accounting.
- [ ] The external lane export accounting behind `exports_enabled` (merge `f751185c7`; branch commits `752d6aebf`, `244d06bb0`, `b60487eb0`, `78e08c23c`, `e79d6b332`, `e93da305e`, `aa26e44f7`, `0d9931e52`, `358d27625`, `57bc5a24d`, `7d9e3b8ac`, `a46a42828`, `214af76f2`). Execution and the state commitment change when a genesis enables exports, and the bare library entry `execute_asset_transaction` now refuses lane exports and verified releases without governed inputs (no node path affected). No deployed genesis has a lane, so the devnet is unaffected; it **needs its own qualification**.
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

- [ ] The commits on main after `76bc12fea` taken into this cut, each of these merges with its own qualification: the pruning (`f53e7457c`), the governed release cap (`f2dedd257`), the committee move (`be09a97f1`), the refused export operation (`7a968007e`), the export accounting (`f751185c7`), the validator signer process (`b610061fc`), its double-sign guard (`fcfdf595e`, `f86d501bb`) and the two-user deployment shape (`7651635f6`).
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
- [ ] Release day, after deploying (`66c110dfa`): every node that serves `account_tx` runs `postfiat-node account-tx-index-build` once, because an old index is reported unusable and `account_tx` answers from the bounded archive scan until then ([runbook](../../runbooks/account-tx-index.md)).
