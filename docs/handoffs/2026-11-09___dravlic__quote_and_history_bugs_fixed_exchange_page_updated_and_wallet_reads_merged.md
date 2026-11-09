# Quote and history bugs fixed, exchange page updated, wallet reads merged

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-11-09 UTC

## BLUF

Today's work in brief:

- **Three outside-contributor bugs fixed.** bradsbots reported three correctness bugs ([#193], [#197], [#198]) with reproduction pull requests ([#194], [#199], [#200]). All three are fixed on main with the reproduction tests switched on, each after a review and a fix round.
- **Exchange page published.** The exchange guidance page's two-week update is out, dated 9 November with the next update on 24 November. It carries the real Base Sepolia Lock rehearsal, the verified lock source and corrected wording.
- **Client pull requests merged.** The client's weekend pull requests [#196] (the wallet gateway's private reads and the RPC connection lifecycle) and [#195] (the November 6 documentation refresh) are merged with review fixes.
- **CI repaired.** The SP1 installer step works again.
- **Phase 3 rehearsal.** A Phase 3 deployment-order rehearsal exists on a branch; it is not merged.
- **Devnet.** I made only read-only reads. All six validators are at height 1170 on `vote-records-20261030`.

## Current state

**Fee quotes and account history (all on main; the three issues are closed)**

- **[#193] `transfer_fee_quote`.**
  - `6f4a1c2df` merges [#194]. `3ac4adc65` prices the quote and `mempool-submit-transfer` on the ledger after the pending entries. `b47ae08d6` adds tests and docs.
  - A review found that this first version counted every pending kind, while admission runs only the pending plain transfers before a plain transfer.
  - `6cf815b23` adds one shared helper, `apply_pending_before_new_transfer`, used by admission, the submit and the quote. The test is `transfer_fee_quote_runs_only_the_entries_before_a_plain_transfer`.
  - [RPC methods](../rpc/methods.md) states the counted entries and two limits: a pending entry dropped before the block, and a recipient created by another pending kind.
- **[#197] `escrow_fee_quote` / `offer_fee_quote`.**
  - `a20742ea1` merges [#199]. `600cafb38` counts the locked native amount in the reserve flags and adds two fields: `sender_balance_after_amount_and_fee` (escrow) and `sender_balance_after_amount_fee_and_reserve` (offer). `31448c570` adds tests and docs.
  - `4a0bdd3ca` makes the web wallet's offer send block on the right flag; it adds the first offer test in `tx-builder.test.js`.
  - `2da6c6d74` adds issued-asset escrow and offer tests, and the wallet's escrow message now names the locked amount.
- **[#198] the `account_tx` row of an `escrow_finish`.**
  - `7866229b9` merges [#200]. `66c110dfa` sets `from` to the recipient who signed and paid, in Rust and in the Python fallback.
  - **The `account_tx` index schemas move from v3 to v4.** An old index is reported unusable, and `account_tx` answers from the bounded archive scan until the operator runs `postfiat-node account-tx-index-build` once after deploying ([runbook](../runbooks/account-tx-index.md)). The release plan carries this as a row and as a release-day checklist item.
  - `d0e163d2f` makes [Account history](../rpc/account-history.md) say where each escrow balance moves:
    - the create row is the owner's lock;
    - the recipient is credited at the finish, while the finish row shows 0, because no operation or receipt field carries the amount;
    - a cancel returns the amount.
  - The page also says to confirm a deposit at the finish row or by balance, and that owners watch `account_escrows` / `escrow_info`.
- **Every fee quote starts from the ledger admission uses.**
  - `693db9712` adds `admission_base_ledger`: the committed ledger plus the confirmed FastPay effects not yet in a block. The five quotes, the submit and the transfer and `payment_v2` admission paths start from it.
  - `68331646d` and `02a2ca229` route the atomic swap quote and the remaining admission paths through the same helper. A test pins it with a confirmed, uncommitted FastPay unwrap.

**`server_info`**

- **Measured.** 6.6–7.0 s on four validators, and 10.1–14.1 s on validator-2 and validator-5 (two read-only reads; the Python client's default timeout is 8 s).
- **Cause.** The method spawned a child process that recomputed the status and read the whole stored history to return three constant fees and one count. Likely but not proven: the child also contended with its own rpc-serve for the database lock.
- **Effect.** The wallet proxy sends one request at a time per validator, so a `server_info` delayed every other read to that validator.
- **Fixed on main.**
  - `9f76e2e5e` answers `server_info` inside rpc-serve, from the cached status and one governance read. A comparison test checks the old computation against the new answer on one fixture.
  - In a local run that is not recorded in the repository, old and new `rpc-serve` gave identical answers on a fixture: 6–26 ms against 63–95 ms.
  - `ff1f6a8b1` adds the release-plan row (rides along).
  - `f58515e52` reads the registry count with the cached status under one lease. It states the four small changes in the answer: `metrics.ok` covers only the governance read, `storage` describes rpc-serve's own handle, the answer can be up to 1 s old, and a failed status read returns `rpc_status_failed`.
- **Until the next release.** The deployed release still runs the old path until the next release carries this, so the Python client keeps its 30-second allowance for `server_info` (`8a6216a84`).

**Exchange guidance page (private review repository `postfiatorg/exchange-guidance-review`; the published page is the locked exchange-integration page)**

- **Draft.** The draft was re-pinned to `4b4da1b91` with the agreed wording fixes (`24d57f051`).
- **Second reading.** Three more fixes followed (`f366ac401`):
  - the rehearsal Lock used the disposable rehearsal token, not the PFT ERC-20;
  - the lock is linked to its verified source;
  - the Sources heading is scoped.
- **Scores.** 85.73 for the previously published page against 86.80 for the new one, both in a fresh harness database. The 28 October 87.40 is not comparable.
- **Published.**
  - Site source `4cec78e0c`, a fresh commit.
  - Main `ed35fd87c`: the generator re-pinned, `exchange-guidance.md` synced, the leak-check pin.
  - The handoff, `22c1e9149`.
  - The website repository `postfiatorg.github.io` at `851d4a441` (Pages run 42610191891, success).
- **Live check at 11:36Z.** Pin `4b4da1b91`, "Updated 9 November 2026", "next update 24 November 2026". Nothing else was published.

**Client pull requests**

- **[#196], the wallet gateway's private reads and the RPC connection lifecycle.**
  - Merged as `f897d5526`, with the review fixes merged as `a11e8d12c` (branch `w1/private-api-readiness-fixes` at `1ae408854`). The fixes:
    - typed transport errors from a fixed list, a connect timeout and a response-id check;
    - idle sockets retired at 25 s, and the 64-request ceiling as a named constant;
    - the auth test pins the public-read list;
    - a 2-second shared cache for the four private reads;
    - `external_lane_info` added to the gateway's public reads;
    - [Web wallet](../wallets/web-wallet.md) without private-repository links;
    - the W-1 milestone made consistent with its evidence, with S4.4 left open and its limits stated.
  - `4f87d6414` corrects the references.
  - The content first landed as twelve replayed commits ending at `ff0713595`, because a rebase flattened the merge. The two merge commits followed and record the pull request's history. Nothing was rewritten.
- **[#195], the November 6 documentation refresh.**
  - Merged as `e3aeefa52`, plus `32836c085` with corrections in the ERC-20 milestone:
    - the path sentence back in the BLUF;
    - the 6 November Lock named as the rehearsal lock with the disposable token;
    - the stand-in verifier named;
    - P1.4's audit estimate restored.
  - His GitHub links to pages excluded from the site stay.

**CI, the release plan and status**

- **CI.** `7ecf15adf` fetches the SP1 installer from its pinned release tag, because `sp1up.succinct.xyz` has answered 404 since the weekend.
  - `product-security-ci` failed at "Install pinned SP1 guest toolchain" on `b47ae08d6`, the push before the fix. It is green on `7ecf15adf`.
- **Release plan.** In [the release plan](../plans/active/next-validator-release-plan-20261115.md), `1c7f8c7c7` added the rows for today's merges; all ride along, with the `account_tx` index rebuild as a release-day operator step. `02a2ca229` and `f58515e52` added and corrected the later rows. The plan counts nine changes that touch the node binary but ride along.
- **Phase 3 ([ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md)).** The branch `token/phase3-deployment-order-rehearsal` is at `740ea2fb5`; it is **not merged**.
  - It adds [`scripts/rehearse-phase3-deployment-order.py`](https://github.com/postfiatorg/postfiatl1v2/blob/token/phase3-deployment-order-rehearsal/scripts/rehearse-phase3-deployment-order.py), which rehearses [design note section 5](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md) on a local Anvil. It:
    - predicts both CREATE addresses and both runtime code hashes before deployment (snapshot and revert);
    - checks the lane genesis that pins the prediction with `init --genesis-file`;
    - seeds the verifier from fixtures;
    - deploys the verifier and then the lock at pinned nonces, re-checked against latest and pending;
    - asserts the deployment on chain, and refuses a nonce off by one.
  - The record is [on the branch](https://github.com/postfiatorg/postfiatl1v2/tree/token/phase3-deployment-order-rehearsal/docs/review/phase3-deployment-order-rehearsal-20261109).
  - Limits: fixture inputs, a placeholder SP1 verifier, PFTL only as far as `init`, and one deployer.
  - The Base Sepolia dry run waits for the route profile and asset id. The branch gets its remaining review on Wednesday before any merge.
- **Status pages.** `29614f16d` records today's read-only fleet observation in [Current State](../status/chain-state-current.md) and [Network Status](../status/network-status.md).

**Deployment state**

- **Last observed fleet state** (read-only at 11:51–11:54Z and 12:23Z): all six at height 1170, tip `51c1cab0…`, state root `36233f64…`, mempool empty.
- **Deployed binary and lineage:** executable `87cfe797…`, build `ae70d66d` = `vote-records-20261030`.
- **Repository HEAD:** `main` at this handoff's commit.
- **Merged but undeployed:** everything above, on main since the cut.
- **The devnet was not touched:** no deployment, no signing, no transaction.

**Task Node**

Task Node works again; the pay-per-use credit was topped up. Rewarded tasks:

- `task_23c91cf2d0368550ec7404b9480c69b4` ([#193]): 3.5 PFT;
- `task_d9a6eac3efa388fa030d193e509329ee` ([#197]): 3.5;
- `task_34b74081245516db38916b00a357a29c` ([#198]): 2.45;
- `task_b44f2026919761725e8ec3d467271bca` ([#196]): 3;
- `task_ec804309432e53ed42184f7d7176cfa2` (the rehearsal): 3.5;
- `task_1d9065eb7e605efa20eeb4b3bfc6a528` (`server_info` in-process): 2.8.

**Public CI at the time of writing** (`gh run list --branch main`, 13:20Z)

- **`docs-build`:** green on every push.
- **`rust-ci`:** green through `29614f16d`; running on `d0e163d2f` and every later push.
- **`product-security-ci`:** green from `7ecf15adf` through `d0e163d2f`; running on `8a6216a84` and every later push.

## Next decision or action

Ours first:

1. Wednesday: the remaining review of the Phase 3 deployment-order rehearsal branch, then its merge. The Base Sepolia dry run follows once the verifier's route profile and asset id are decided.
2. Any notes left over from the last fix round's reviews. After that, the next ERC-20 milestone candidates that are ours. There are none until the client's decisions arrive: the audit vendor, the route profile and asset id, and the split at launch.
3. The two-user signer deployment on the devnet with the next release, after that release's own qualification. The `account_tx` index rebuild is now a release-day operator step.
4. The rollout day under the runbook, once the keyed rehearsal on the client's hosts is done.
5. Read CI on today's last pushes.

Then the client's, each with the date first asked:

- Pause minting on the Ethereum controller `0x9A02…` (2026-11-02). It was still on at 10:45Z today, the sixth working day.
- The rollout go, the keyed rehearsal on his hosts, the six migration signatures and the migration-trust descriptor (2026-11-02/03/04).
- The audit vendor and budget for the lock and token contracts, with the package for quotes (2026-10-26 / 2026-11-06).
- The wrapped-token path, A or B (2026-11-04).
- The Phase 3 decisions in the export design note's section 9, including the verifier's route profile and asset id (2026-11-05).
- The three red CI jobs on his next-release branch, and our fix branch `security/next-release-ci-fixes-20261106` (2026-11-06).
- The reachable host of the audit and operations machine (2026-11-04).
- The six cached checkpoint votes (2026-11-04).
- The old proving service and the faucet key (2026-11-02).
- The two issuer signatures (2026-09-25 / 2026-10-01).
- The reserve-proof successor registration (2026-09-24).
- The `ETHEREUM_MAINNET_RPC_URL` secret (2026-09-28).

From his 2026-11-07 wallet handoffs:

- **Now answered:** the proxy's `external_lane_info` read, which has been public through the gateway since `a11e8d12c`.
- **Still his:**
  - native-session admission;
  - the trusted FastPay committee and policy, and terminal proofs;
  - the NAV and private route health, and the BMNRC repair;
  - the audited and deployed Base lane identities.

## References

- **Issues and pull requests:** [#193], [#194], [#195], [#196], [#197], [#198], [#199], [#200].
- **RPC and history:** [RPC methods](../rpc/methods.md), [Account history](../rpc/account-history.md), [Account-history index runbook](../runbooks/account-tx-index.md).
- **Plans:** [Next validator release plan](../plans/active/next-validator-release-plan-20261115.md), [W-1 milestone](../plans/active/user-proven-shielded-actions-milestone.md), [ERC-20 milestone](../plans/active/pft-erc20-base-listing-milestone.md).
- **Phase 3:** [design note, section 5](../specs/pft-erc20-base-bridge-phase3-export-design-20261105.md).
- **Status:** [Current State](../status/chain-state-current.md).
- **The client's weekend handoffs** in the private wallet repository, at `07ecd36`: `docs/handoffs/2026-11-07_primary_postfiat_wallet.md` and `docs/handoffs/2026-11-07_normal_mac_private_wallet.md`.
- **Previous handoff:** [2026-11-06: first real Base Sepolia Lock released, bridge Phase 2 complete](2026-11-06___dravlic__first_real_base_sepolia_lock_released_and_bridge_phase_2_complete.md).

[#193]: https://github.com/postfiatorg/postfiatl1v2/issues/193
[#194]: https://github.com/postfiatorg/postfiatl1v2/pull/194
[#195]: https://github.com/postfiatorg/postfiatl1v2/pull/195
[#196]: https://github.com/postfiatorg/postfiatl1v2/pull/196
[#197]: https://github.com/postfiatorg/postfiatl1v2/issues/197
[#198]: https://github.com/postfiatorg/postfiatl1v2/issues/198
[#199]: https://github.com/postfiatorg/postfiatl1v2/pull/199
[#200]: https://github.com/postfiatorg/postfiatl1v2/pull/200
