# Bug fixes and pending decisions

- **Operator:** Domagoj Ravlić (`dravlic`)
- **Date:** 2026-08-31 UTC

## BLUF

Congratulations on the storage deployment — transactional storage live on all
six validators at height 931 is the milestone this month was building toward.
This session pushed the operator's 8 stranded commits (the deployment record
was not on origin), fixed all three open defects from his
[registry-fix handoff](2026-08-31___postfiatchad__devnet_registry_fix_deploy.md)
(P1/P2/P3, each with red-then-green regression tests), repaired the strict
docs build he left failing (72 warnings), and consolidated every decision
only he can make into one
[sheet](../governance/pending-operator-decisions.md).

## Current state

- Pushed his `52d9ee4a..b5dcef3f` to origin/main, including the deployment
  record ([deploy-receipt.json](https://github.com/postfiatorg/postfiatl1v2/blob/main/deployments/storage-lease-20260831/deploy-receipt.json),
  [Current State](../status/chain-state-current.md)); killed the finished
  docsview session per his Telegram permission.
- `cc74e76c` — the [plans index](../plans/README.md) now lists the
  testnet-path milestone and both deployment plans.
- `83c51b77` — strict docs build restored
  ([LOC-audit inventory](../security/core-feature-loc-audit-inventory.md)
  link style).
- `6c970c80` — P2: per-connection read budget in the RPC serve loop
  (`RpcServeDeadlineStream`, 30 s cap, `crates/node/src/rpc_cli.rs`). Root
  cause: `SO_RCVTIMEO` bounds one recv, so a byte-trickling client resets it
  forever; the regression test in
  `crates/node/src/main_parts/tests/rpc_serve_request_tests.rs` proves other
  connections keep serving. The health-probe half of P2 is fleet wiring and
  stays open.
- `a4234716` — P3: finality submit is idempotent after its own successful
  commit (`idempotent_replay: true` replay path in
  `crates/node/src/rpc_cli.rs`; a conflicting duplicate still fails).
- `353156c3` — P1: checkpoint export replays superseded registry history
  (`crates/node/src/block_replay_wallet.rs` anchors the applied run on the
  quorum-signed `certificate.registry_root`; both devnet error shapes were
  reproduced from local fixtures pre-fix in
  `crates/node/src/tests/validator_registry_continuation_tests.rs`).
  Restores signed snapshot backups. Full lib suite: 335 pass; the two
  remaining failures —
  `tests::consensus_history::cross_view_vote_and_legacy_lock_migration_fail_closed`
  and
  `ordered_history_v2_active_commit_uses_one_database_transaction_without_jsonl`
  — are pre-existing on the baseline and unrelated.
- `82fe4e9b` —
  [pending operator decisions](../governance/pending-operator-decisions.md):
  8 one-line decisions with recommendations pre-filled.
- Boundaries: no Task Node action, no SSH to validator hosts, the devnet was
  neither queried nor mutated (no live probe this session), and Z1
  observation is untouched. All work was done from the `postfiatl1v2-dravlic`
  worktree and pushed with `git push origin HEAD:main`.

## Next decision or action

1. Answer the 8 rows in the
   [pending-decisions sheet](../governance/pending-operator-decisions.md) —
   one line each.
2. The fixes are on main, but the deployed release `storage-lease-af9b83c3`
   predates them — fold them into the next release he cuts (his call,
   alongside the Z1-end suite).
3. The P2 fleet health probe and his owed cleanups (gate ufw rule, ~8 GB gate
   data per host) remain his operator actions.

## References

- [Devnet storage canary rollback](2026-08-30___postfiatchad__devnet_storage_canary_rollback.md)
- [Devnet registry-continuation fix deployed](2026-08-31___postfiatchad__devnet_registry_fix_deploy.md)
- [Storage single-writer gate passed](2026-08-31___postfiatchad__storage_gate_passed_rollout_pending.md)
- [Single-writer deployment plan](../plans/active/devnet-storage-single-writer-deployment-plan.md)
- [Pending operator decisions](../governance/pending-operator-decisions.md)
- [Storage G4 time-budget decision](2026-08-28___dravlic__storage_g4_time_budget_decision.md)
