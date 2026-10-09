# Next validator release plan (after signer-rotation-20261007)

Skeleton written 2026-10-08. The date suffix is a name, not a date. No date is promised: this
release starts after `signer-rotation-20261007` is deployed and its activation is done
([current plan](next-validator-release-plan-20261005.md)).

## Summary

1. `signer-rotation-20261007` is cut from main at `0fa55d0b` (`release/signer-rotation-20261007`, tip `b4d3ebde`); rollout 2026-10-09 at the earliest.
2. This release carries main past that cut plus the candidates below that are ready and reviewed.
3. Main past the cut changes no binary behaviour: one test-only Rust removal, the Python fallback and docs.
4. Rule kept from the current plan: changes that alter a consensus or state result get their own qualification; read-path and tooling changes ride along.
5. Qualification and rollout follow the current plan; nothing starts before the "Not before" rules hold.

## Already on main past the cut

`git log --oneline 0fa55d0b..origin/main`; `git diff --stat 0fa55d0b origin/main -- crates/` touches only test code.

- [x] `9d837bef` (PR #54, contains `c2a03a3b`, `b817b5a1`): removes the dead `#[cfg(test)]` swap-pricing wrapper (`crates/node/src/execution_actions.rs`, `crates/node/src/lib.rs`, `crates/node/src/tests/asset_orchard_issued_tests.rs`). Test-only.
- [x] `51144786`: Python fallback reads the newest blocks at or below the end height (`_account_tx_client_side_scan`, `python/postfiat_rpc/client.py`; `python/tests/test_account_tx_fallback.py`); archive-scan regression test `account_tx_scan_and_index_return_the_same_transaction_kinds` (`crates/node/src/tests/account_tx_truncation.rs`). Read-path tooling.
- [x] `bbe82032`, `54515003`: docs (Z3 plan status; 2026-10-07 handoff).
- [x] Also docs only: `e92b8157`, `0a136eaa`, `f3f2a1e1`, `774a6025`.

## Candidates

Included only when ready and reviewed by the cut; none is promised.

- [ ] Draft PR #47 (citadelculture): relative RPC child request spool path, issue #27 (`crates/node/src/rpc_cli.rs`). Blocker: draft; owner must finish it. Read-path (RPC serving).
- [ ] PR #52 (JJ2203-theRealOne): RPC worker requests with relative data directories (`crates/node/src/main_parts/cli_dispatch_parts/group_03.rs`). Blocker: waits for #47, the correct fix. Read-path (RPC serving).
- [ ] Draft PRs #43–#46 (citadelculture), if their owner finishes them. Tooling:
    - [ ] #43 rejects uncorrelated raw finality RPC responses (`scripts/native_rpc_finality_submit.py`).
    - [ ] #44 documented node wrapper invocations (`scripts/node-*`).
    - [ ] #45 Python RPC probe false negative across resolved addresses (`python/postfiat_rpc/rpc_probe.py`).
    - [ ] #46 transparent-only monitor smoke with the sole proof warning (`scripts/testnet-monitor-snapshot-smoke`).
- [ ] Python fallback gaps: no atomic-swap rows; escrow, NFT and offer row indexes shift in blocks with a swap (`_account_tx_client_side_scan`; [account history](../../rpc/account-history.md#result-window-and-truncated)). Owner: this lane. Read-path tooling.
- [ ] Archive scan `truncated` means "possibly incomplete" (`account_tx_scan`, `crates/node/src/block_finality.rs`). Changed only if an exact flag is wanted; it needs an unbounded scan of a public read. Owner: this lane, on request. Read-path.
- [ ] The other lane's delayed-precommit fix for its unreleased candidate. Blocker: pushed to main and reviewed. **Consensus change: its own qualification.**
- [x] Testnet provisioning default of the RPC accept budget. Done: [public RPC operator policy](../../runbooks/public-rpc-operator-policy.md) and `systemd/postfiat-rpc.service.example` carry `--max-requests 100000` and `RestartSec=1` like the release generator (`crates/node/src/batch_snapshot.rs:2403`); `example_rpc_unit_matches_generator_accept_budget` (`crates/node/src/tests/snapshot_deployment.rs`) fails if either drifts again. The two `scripts/a666-r4-*` rehearsal scripts still pass `10000`; they are frozen records pinned to candidate `39f7fae3` and were left as is. Tooling/docs.
- Not in this repository: FW-13 and FW-14 (P3, StakeHub), owned by the other lane. Tracked here only.

## Not before

- [ ] `signer-rotation-20261007` deployed on all six validators and its activation finished (both signer groups; [activation](next-validator-release-plan-20261005.md#activation-after-rollout)).
- [ ] CI green on main at the cut commit.
- [ ] Full workspace suite at the cut commit (the release gate for the full suite).

## Qualification and rollout

As in the current plan: [Qualification](next-validator-release-plan-20261005.md#qualification) (~90 min after the suite) and [Rollout](next-validator-release-plan-20261005.md#rollout) (~60 min, one validator at a time, `rollback-one.sh` back to `signer-rotation-20261007`). A consensus candidate adds its own qualification before the cut.
