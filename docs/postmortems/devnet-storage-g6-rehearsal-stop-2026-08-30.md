# Devnet Storage G6 Rehearsal Stop - 2026-08-30

- **Date:** 2026-08-30 UTC
- **Classification:** release-gate failure; no live-fleet incident
- **Status:** stopped safely; candidate `d0ae79f3` must not deploy

> **Later event:** successor `10dd9f20` repaired the continuation defect and
> passed the old G6 runner, but a live canary exposed a different deployment
> topology failure and was rolled back. See the
> [live-canary postmortem](devnet-storage-live-canary-rollback-2026-08-30.md).

## Executive summary

The planned transactional-storage rollout stopped at the mandatory G6
six-clone rehearsal. Candidate source `d0ae79f3` successfully rebuilt and
independently verified the storage of all six height-924 validators. The first
certified continuation round, intended to produce height 925, then failed
closed because the node tried to apply an older, superseded validator-registry
update to the registry produced by a later update.

The exact rejection was:

```text
live validator registry activation previous validator registry root mismatch
```

Validator-1 returned the same rejection for all three delivery attempts. None
of the six working clones advanced beyond height 924.

This failure happened before any live deployment. No live binary, service, data
directory, storage mode, validator registry, trust graph, or Cobalt authority
state changed. A second authenticated fleet probe confirmed that all six live
validators remained healthy and converged at height 924. Rollback was therefore
unnecessary, and the Z1 observation clock never started.

## What was being attempted

The rollout was intended to replace the controlled devnet's bounded JSON/JSONL
storage with the qualified transactional `redb` candidate. The execution plan
required the following sequence:

1. Probe the live fleet without mutation.
2. Copy six stopped validator directories and rehearse the exact migration.
3. Deploy one validator at a time, retaining the old binary for rollback.
4. Prove convergence, finality, transactional-backend use, and bounded reads.
5. Prove that valid and invalid Cobalt registry transitions behave correctly.
6. Publish receipts and begin the Z1 observation period.

Steps 2 and 3 were separate operator authorization points. Passing the offline
performance gate did not authorize deployment, and passing the migration
rebuild alone did not satisfy G6. G6 also required successful certified
continuation against the real fleet history.

## Pre-rehearsal fleet state

The read-only probe ran from `13:53:22Z` through `13:53:39Z` and found:

- six reachable validators on `postfiat-wan-devnet-2`;
- convergence at height 924 with one tip and one state root;
- empty mempools;
- identical deployed binary SHA-256 on all six hosts;
- active validator, RPC, and advisory shadow services;
- Cobalt authority mode 1 with identical registry and trust roots; and
- no transactional generation or backend-mode record, as expected before the
  rollout.

The deployed fleet remained on binary
`d5e5ef630155e61b001b84edb404a4def7d29a9205f23d33d2ad9c37c2696caf`.
The proposed candidate binary was
`9e82d9286d79307b6246a773882e744ade1abad6b10498c3ed2d9c9e6b78c80c`,
built from source
`d0ae79f3342fc78cbf907dbf231a60de8bc40606`.

## What passed in G6

Six distinct, stopped height-924 validator directories were authenticated
before migration. The existing
`benchmarks/storage-scaling/run_migration_rehearsal.py` workflow then:

- rebuilt six transactional storage generations;
- published each rebuilt generation;
- ran six independent, non-mutating verify-only checks;
- confirmed one shared migration packet root,
  `6a6b53eab81011ad469c23cb8088a4bc6d2b628d06a11c873f3adcb829c087e9389d6e32020361c965a0dd361e4807d5`;
- staged split validator keys for the isolated rehearsal; and
- preserved convergence across the six source clones.

These results establish that the candidate could reconstruct and verify the
height-924 state. They do **not** establish that it could safely continue the
chain from that state.

## Failure

The rehearsal began its first post-migration certified transparent round. The
target block height was 925. Before storage activation or any later rollout
phase, validator-1 rejected the proposed continuation three times with the same
registry-root error.

The runner stopped. The failure receipt records:

- operation: first post-migration certified transparent round at height 925;
- phase: legacy finality;
- reason code:
  `VALIDATOR_REGISTRY_HISTORY_REAPPLICATION_ROOT_MISMATCH`;
- rejecting node: validator-1;
- working-clone height after failure: 924 on all six; and
- candidate disposition: `DO_NOT_DEPLOY`.

This is a fail-closed compatibility failure. It prevented an invalid
continuation rather than accepting ambiguous registry history.

## Technical diagnosis

The accepted chain history contains multiple validator-registry updates through
height 924. The live registry at height 924 reflects the latest accepted
update. During continuation, the candidate scans historical governance updates
to determine which updates are due.

The bounded source diagnosis points to
`live_validator_registry_after_due_updates` in
`crates/node/src/block_replay_wallet.rs`. The function iterates the recorded
updates and treats an update as already reflected only when applying its
affected validator set to the current registry produces that update's
`new_registry_root`.

That test is insufficient for superseded history:

1. An earlier update is accepted and changes a validator key.
2. A later update changes that key again.
3. The persisted registry correctly reflects the later update.
4. Reconstructing the earlier update's affected set from the current registry
   no longer equals the earlier update's new root.
5. The earlier update is therefore treated as due again.
6. Applying it to the later registry fails the required previous-root check.

The root check in
`apply_verified_validator_registry_update_to_registry_inner` in
`crates/node/src/storage_commit.rs` correctly rejects the operation because
the current registry is not the earlier update's predecessor. The defect is
therefore not that the check is too strict. The defect is the continuation
logic attempting to reapply already-accepted, superseded history.

This is a bounded diagnosis, not a completed repair. The final cause should be
considered proven only after an owning-boundary fix passes an exact regression
and the affected qualification gates.

## Impact

### Live fleet

There was no live-fleet impact:

- no deployment was attempted;
- no service was restarted;
- no live storage migration or activation occurred;
- no governance action was submitted;
- no validator or trust key changed;
- no rollback was required; and
- no Z1 observation period started.

The post-failure probe ran from `14:48:27Z` through `14:48:45Z`. It confirmed
the original binary on all six validators, active services, empty mempools, no
transactional backend, and unchanged height, tip, state root, registry root,
and trust root.

### Release and schedule

Candidate `d0ae79f3` lost deployment eligibility. Its earlier G4 performance
pass remains historical evidence for that exact binary, but it cannot override
the G6 compatibility failure. Any repair creates a new source and binary
identity and invalidates the affected qualification evidence.

Public-testnet progress remains blocked on a successor storage candidate.

## Why the gate was effective

The rehearsal separated three claims that could otherwise be conflated:

- **Rebuild correctness:** the candidate reconstructed all six stored states.
- **Verification correctness:** independent read-only checks accepted those
  reconstructed states.
- **Continuation compatibility:** the candidate could not extend the exact
  accepted chain history.

Synthetic and lower-height testing had not reproduced the sequence of
superseding registry updates present in the real height-924 history. The
six-clone gate caught that gap without putting the live fleet at risk. The
correct operational result was to stop, not to weaken the root check or proceed
because migration itself had passed.

## Required remediation

A successor candidate must:

1. Repair the owning validator-registry continuation boundary so accepted,
   superseded updates are not treated as due again.
2. Preserve deterministic replay and the fail-closed previous-root and new-root
   checks.
3. Add an exact regression with at least two accepted updates to the same
   validator record, beginning from the height-924-equivalent final registry
   and successfully producing the next certified height.
4. Cover stale, reordered, duplicated, missing, and wrong-root update history,
   including rejection without durable mutation.
5. Freeze a new source revision and binary hash.
6. Repeat every qualification gate invalidated by the code change, including
   exact-history replay and the complete six-clone G6 workflow.
7. Obtain a new written deployment decision after G6 passes.

No existing receipt authorizes modifying or deploying `d0ae79f3`.

## Evidence

Redaction-safe, committed evidence:

- [Pre-rehearsal fleet probe](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/devnet-rollout/fleet-probe-20260830.json)
- [G6 failure receipt](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/devnet-rollout/g6-failure-20260830.json)
- [Post-failure unchanged-fleet receipt](https://github.com/postfiatorg/postfiatl1v2/blob/main/benchmarks/storage-scaling/devnet-rollout/stop-receipt-20260830.json)
- [Devnet storage rollout plan](../plans/active/devnet-storage-rollout-plan.md)
- [Current chain state](../status/chain-state-current.md)
- [Storage architecture](../architecture/state-and-storage.md)
- [Execution handoff](../handoffs/2026-08-30___postfiatchad__devnet_storage_rollout_stopped_at_g6.md)

Raw clones, signer material, logs, and the incomplete rehearsal are private
operator artifacts. They must not be committed or published.
