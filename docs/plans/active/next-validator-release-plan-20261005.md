# Next validator release plan (after the conference)

Written 2026-10-05. The conference is [TOKEN2049 Singapore](https://www.token2049.com/singapore),
Wednesday 7 and Thursday 8 October 2026; on those days nothing touches the six
validator hosts, the chain or StakeHub `master`. Nothing is deployed before 2026-10-09.
The release name `signer-rotation-20261007` keeps its date suffix; it is a name, not a date.

## Summary

1. The fleet (six validators of `postfiat-wan-devnet-2`) runs `combined-fastpay-20260928`: source `c93b2137`, executable `1f8b332d…`, deployed 2026-09-28 ([deployment](https://github.com/postfiatorg/postfiatl1v2/blob/main/deployments/combined-fastpay-20260928/README.md), [current state](../../status/chain-state-current.md)).
2. The next release carries the undeployed main changes plus an issuer-signed bridge policy update operation, which is required.
3. Optional if ready before the cut: an RPC accept loop without restart gaps, two `account_tx` truncation follow-ups and the other lane's delayed-precommit fix.
4. It is qualified and rolled out like 2026-09-28: ~90 min qualification after the full workspace suite, ~60 min rollout one validator at a time.
5. After rollout, both signer groups go back to six members, quorum 5 ([decision proposal](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/validator-5-signer-committees-decision-proposal-20261001.md), lean A). The FastPay half cannot take effect before height 10001.

## Scope

`git diff --stat c93b2137 origin/main -- crates/` lists exactly the commits marked done below.

Done on main, undeployed:

- [x] `1bb15a78`: `deployment_manifest_verified` is `true` when a record written by `deployment-manifest-verify` is present. The release generator writes the new variable `POSTFIAT_DEPLOYMENT_VERIFIED_RECORD` into the unit environment (`crates/node/src/lifecycle_queries.rs:880`, `crates/node/src/batch_snapshot.rs:2556-2563`).
- [x] `b1d1928c` (PR #55): `account_tx` sets `truncated` only when rows are omitted (`crates/node/src/block_finality.rs`, `crates/node/src/tests/account_tx_truncation.rs`).
- [x] `0609cb01` (PR #48, contains `b560997c` and `72177cf3`): one Apple-only line in `crates/node/src/storage_migration.rs` (no Linux change), plus smoke and monitor tooling.
- [x] `dc7d9bb6` (PR #51): scripts only (`scripts/node-*`, `scripts/node-helper-smoke`). Not part of the binary.

Being built on main (2026-10-05 and 2026-10-06):

- [x] (1) Issuer-signed operation that updates an asset's Ethereum bridge policy (new `authority_epoch` and `committee_root`). At `c93b2137` the policy is written only at route creation (`crates/execution/src/nav_vault_asset_execution.rs:4292`) and no operation changes it. Done: `pftl_uniswap_route_bridge_policy_update` (`crates/types/src/transactions_mempool_receipts.rs:3105`, `crates/execution/src/nav_vault_asset_execution.rs:5643`).
- [ ] (2) FastPay committee record (prepared by `fastpay-committee-prepare`, `a683475a`; install and payment dry-run 2026-10-05): six validators, quorum 5, under the recovery policy (`crates/types/src/fastpay_recovery_types.rs:130-140`, activation rule `:320`). This is a signed record, not new code; it is prepared with the release and submitted after it.
- [x] (3) RPC accept budget restart gap shortened. At `c93b2137` the release generator passes `--max-requests 10000` and systemd restarts the service after 5 s. Done (short-gap variant; socket handover does not fit the single-process unit model): `--max-requests 100000` and `RestartSec=1` (`crates/node/src/batch_snapshot.rs:2403`); the report is written before the listener closes (`rpc_serve_with_report`, `crates/node/src/rpc_serve_runtime.rs:14-30`). About 1 s of closed port remains per budget.
- [x] (3b) PR #55 follow-ups found in review on 2026-10-05:
    - [x] Python fallback scan marks a result truncated when the matching rows exactly fill the limit (`python/postfiat_rpc/client.py:1609`, `:1636-1638`). Done: truncated only when a further matching row or an unread block of the range exists; newest rows without a start height (`_account_tx_client_side_scan`, `python/tests/test_account_tx_fallback.py`).
    - [x] Without a start height the archive scan returned the oldest rows of the window; the disk index returns the newest. Done: newest rows (`account_tx_scan`, `crates/node/src/block_finality.rs`; test `account_tx_scan_and_index_return_the_newest_rows_without_a_start_height`).
    - [x] Not changed, documented: the archive scan reads at most `limit` blocks, so with more blocks in the range `truncated` means "possibly incomplete". An exact flag needs an unbounded scan of a public read that live validators serve whenever the index is stale ([account history](../../rpc/account-history.md#result-window-and-truncated)).
    - [x] After the cut, on main for the release after it: with an end height below the tip and no start height, the Python fallback read the chain's newest blocks before filtering. It now reads the newest blocks at or below the end height (`_account_tx_client_side_scan`; `test_end_height_below_tip_without_start_height_reads_that_range`). The archive scan already returned every kind the index returns (shared row builder; `account_tx_scan_and_index_return_the_same_transaction_kinds`).
    - [x] Python fallback atomic swap rows and row indexes. Done: `_account_tx_client_side_scan` emits one `atomic_swap` row per leg the account owns or receives (`tx_role` `leg_0`/`leg_1`) and counts the block's swaps before numbering escrow, NFT and offer rows, so `transaction_index` and `tx_id` match the index and archive scan (`python/postfiat_rpc/client.py`, `python/tests/test_account_tx_fallback.py`; [account history](../../rpc/account-history.md#result-window-and-truncated)).

Needed for the bridge activation, not started:

- [x] (5) Offline FastSwap WAL migration: command exists, host step on release day (`postfiat-node fastswap-store-migrate`, `crates/storage/src/fastswap_store/legacy_migration.rs`; [runbook](../../runbooks/fastpay-committee-recovery.md#converting-the-fastswap-store-before-the-rotation)). On validator-1, every `fastswap_*` request fails with `fastswap_unavailable`. The cause is the WAL's legacy unkeyed tags, which a normal store open rejects (`crates/storage/src/fastswap_store.rs:1707-1718`). No command calls `FastSwapStore::open_for_legacy_migration`. No unit argument enables the service; it opens on first request. The release needs this command, and the rollout runs it on each validator while its units are stopped. Without it, there are no final-checkpoint votes and no `ActivateCommittee` ([read](../../review/signer-committee-rotation-dry-run-20261005.md#live-fastswap-control-path-2026-10-05-read)).

Waiting on the other lane:

- [ ] (4) Fix for the delayed-precommit finding in the other lane's unreleased candidate. That candidate is not on main and must not be released until the fix lands. Included only when pushed and reviewed; not promised.

## Order and dependencies

Must land before the candidate is cut:

- [x] (1) on main with focused tests. It is the reason for this release; without it the bridge half of the signer-group change cannot happen.
- [x] (5) on main with focused tests, for the same reason. Legacy snapshot conversion and the forced restore after a failed verification are tested (`crates/storage/src/fastswap_store/legacy_migration/tests.rs`).
- [ ] CI green on main at the cut commit. At `0fa55d0b` on 2026-10-06: `docs-build` green, `rust-ci` and `product-security-ci` still running at the cut.

Included if ready and reviewed by the cut, otherwise a later release:

- [ ] (3) and (3b) (both on main). Neither blocks (1) or (2).
- [ ] (4), only after it is on main and reviewed.

Does not depend on the binary:

- [ ] (2) can be submitted on either release; its effect is limited by height (see Activation).

## Qualification

The 2026-09-28 pattern; about 90 min after the suite.

- [ ] Full workspace suite on the work server at the cut commit, started the day before (several hours). This is the release gate for the full suite. Ran at `672b707c` on 2026-10-05, not at the cut commit; the later changes were covered by focused suites ([packet](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/qualification/README.md)).
- [x] Cut `release/<new-release>` from that commit; create `deployments/<new-release>/` from `deployments/combined-fastpay-20260928/` (10 min). Done 2026-10-06: `release/signer-rotation-20261007` at `0fa55d0b` ([release inputs](https://github.com/postfiatorg/postfiatl1v2/blob/release/signer-rotation-20261007/deployments/signer-rotation-20261007/README.md)).
- [x] Two identical clean builds: same executable hash (30–40 min). `decaa411…`.
- [x] History checks (15–20 min). Signed 1050 canary, six 1020 originals, two saved V2 copies: roots match.
- [x] Rotation and rollback rehearsal, including `rollback-one.sh` back to `combined-fastpay-20260928` (15–20 min). Its checks ran on local copies; the script itself runs on a host.
- [x] Signed deployment manifest; `deployment-manifest-verify` passes (10 min). `fa4649aa…`, publisher `pfc531e0…`.
- [x] Evidence packet in the release directory (10–15 min). Not done on 2026-10-06: canary backup, fleet before-state, rollout, store conversion, activation.

## Rollout

About 60 min, after the objection window closes; 2026-10-09 at the earliest.

- [ ] Before: `observe-fleet.py` shows six validators on `1f8b332d…` with the same height, tip and root (5 min).
- [ ] `scripts/postfiat-safe-rollout apply-next`, one validator at a time (validator-1 canary, then 0, 2, 3, 4, 5). One devnet faucet grant per validator; each must certify with the same tip and root on all six before the next (6 × 6–8 min).
- [ ] On divergence or a missed certification: stop, `rollback-one.sh` on the affected validator. The `combined-fastpay-20260928` executable and release directory stay on every host.
- [ ] Live checks: all 12 validator and RPC processes on the new hash; `deployment_manifest_verified=true` on every host; `account_tx` truncation reads (10 min).
- [ ] `deployments/combined-fastpay-20260928/demo-preflight.py` adapted to the new release directory, run read-only and passing (10 min).
- [ ] Update `docs/status/chain-state-current.md` and the deployment README.

## Activation after rollout

Both changes depend on the decision on the [proposal](https://github.com/postfiatorg/postfiatl1v2/blob/main/docs/review/validator-5-signer-committees-decision-proposal-20261001.md). Validator-5 is a genesis member of both groups. Its key was rotated at heights 917 and 924, and neither group followed.

- [x] Dry run of both changes on six local validators first, with before and after read-backs and a rollback note (FastPay half 1–2 h). Done 2026-10-05 on six-validator fixtures, both halves pass ([dry run](../../review/signer-committee-rotation-dry-run-20261005.md)). The missing `ActivateCommittee`/`StopPrepare` signer is added: `fastswap-control-prepare`, `fastswap-control-vote-sign`, `fastswap-control-assemble` ([commands](../../navcoins/pftl-tools.md#fastswap-control-certificates)). The 2026-10-06 tests close the three rehearsal gaps: `StopPrepare` admission with a FastSwap policy, store conversion of a legacy snapshot with a forced restore, and the FastPay epoch-2 boundary ([dry run](../../review/signer-committee-rotation-dry-run-20261005.md#what-did-not-work-or-was-not-exercised)).
- [ ] Bridge group: the issuer signs the policy update from (1), with `authority_epoch 2` and a committee root that includes validator-5's checkpoint key. The epoch 2 committee is activated by a FastLane control certificate from validators 0–4 after the drained final epoch 1 checkpoint (`crates/execution/src/fastswap_control.rs:167-171`, `:386-410`). The dry run confirmed the order: final checkpoint and `ActivateCommittee` need all of validators 0–4, the route stays at epoch 1 until the issuer update, then 5 of 6 sign. No Ethereum transaction.
- [ ] FastPay group: all six validators sign the epoch + 1 record (six validators, quorum 5) with their current registry keys, validator-5 with its rotated key; there is no separate governance key (dry run: five authorizations or a stale key are refused). The committee cannot activate before the recovery policy (`fastpay_recovery_types.rs:320`). Its `valid_from_height` must equal the previous `new_orders_through_height` + 1, which is **10001** (`crates/execution/src/owned_transfer_recovery.rs:875-889`). Tested with epoch 1 ending at 10000 and epoch 2 installed at height 100: a payment is refused at 10000 and applied at 10001 (`fastpay_epoch_two_installed_far_below_deadline_activates_at_previous_end_plus_one`).
- **Limit:** the chain was at about height 1078 on 2026-10-01. Until height 10001, FastPay still needs validators 0–4 all online, and the current committee remains valid until then.

## Communication

- [ ] Inform the other lane; do not ask. Send the handoff and a Telegram message, both carrying this plan.
- [ ] The other lane may object until 2026-10-08 evening UTC (end of Thursday). Rollout starts only after that, on 2026-10-09 at the earliest.
- [ ] Ask for the activation signatures separately, under the decision proposal, after the dry run.

## Not in this release

- Relay units on the six hosts: host hygiene with its own window ([demo readiness](../../status/demo-readiness-20261001.md)).
- The SCT-06 traffic campaign.
- Anything that touches the conference setup (7–8 October).
- The other lane's unreleased candidate itself; at most the fix in (4).
