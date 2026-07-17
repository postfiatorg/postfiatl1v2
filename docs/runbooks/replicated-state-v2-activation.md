# Replicated State V2 Activation

This runbook covers the state-root transition that begins committing every
FastLane/FastSwap field in `LedgerState`. It is a consensus migration. It must
never be introduced by an uncoordinated rolling restart.

## Encoding contract

- A newly created genesis includes
  `replicated_state_v2_activation_height: 0`; complete FastLane state is
  committed from genesis.
- A legacy genesis omitting that field deserializes it as `null` and retains
  the historical state-root encoding.
- An existing chain may schedule the transition with a committed governance
  amendment whose kind is `replicated_state_v2_activation_height` and whose
  value is the future activation height.
- The effective height is the genesis value when present; otherwise it is the
  earliest committed amendment value. A later amendment cannot postpone or
  undo a scheduled transition.
- A migration amendment whose value is at or below its own block height is
  invalid. This prevents the amendment block from being interpreted under a
  different root schema by old and new validators.
- The state height used by the commitment is the complete ordered-batch count,
  including the checkpoint prefix after pruning.

No generic legacy-root fallback exists. Before activation, nonempty FastLane
state deliberately retains the old omitted encoding. At and after activation,
all ten FastLane/FastSwap fields are committed unconditionally when present.

## New networks

Use the genesis produced by the candidate binary and verify the field is
present with value `0` before signing the launch bundle. No migration amendment
is needed. Refuse a production genesis that omits the field.

## Existing controlled networks

1. Prove all validators have the same genesis hash, committee roster, height,
   block hash, state root, empty mempool, and release identity. Run
   `verify-blocks` and `verify-state` before scheduling anything.
2. Confirm the persisted genesis omits
   `replicated_state_v2_activation_height`. If it already contains a value,
   follow that immutable schedule instead of creating another.
3. Select a future height that leaves enough blocks to commit the marker,
   independently verify its accepted receipt, deploy every validator, and run
   pre-activation replay checks. The window must cover a full rollback to the
   old binary before activation.
4. Create and authorize the amendment through the chain's already-approved
   governance authority. Do not manufacture validator-name votes or re-enable
   the removed unsigned governance path. A legacy network without an approved
   authorization path cannot be migrated in place; launch a new genesis or
   obtain an explicit, separately reviewed migration mechanism.
5. Commit the amendment while every validator still uses the old root schema.
   Require an accepted receipt and identical block/root on the full committee.
6. Rolling-deploy the candidate before the activation height. After each node,
   require rejoin at the exact tip and run both block and state verification.
   Abort and restore the old release on any divergence.
7. Before activation, prove old and new binaries compute the same current root.
   Preserve this evidence with both binary hashes and the amendment artifact.
8. At activation, require a single identical root across the committee and an
   accepted receipt for the first block. Re-run replay from the last checkpoint
   through at least one post-activation block.
9. After activation, rollback to a binary that omits FastLane state is unsafe
   and prohibited. Recovery requires a forward-compatible binary that
   understands the v2 commitment.

## Compatibility and refusal matrix

`legacy` below means a binary that does not commit the ten FastLane/FastSwap
fields. `candidate` means a binary implementing the height-routed v2
commitment. The activation height is part of committed genesis or governance
state; it is not a local operator flag.

| Binary | Chain height | Required behavior |
| --- | --- | --- |
| legacy | below activation | Allowed only during the bounded rollback window; its root must byte-match the candidate's legacy-routed root. |
| candidate | below activation | Allowed; it must replay the historical v1 root and retain the committed future activation. |
| legacy | at or above activation | Prohibited. It proposes or verifies the omitted-field root and must be rejected by candidate validators as a state-root mismatch. |
| candidate | at or above activation | Required; it commits every inventoried FastLane/FastSwap field and rejects a legacy-root proposal or block. |

A mixed binary set is therefore permitted only below activation and only after
old/new root equivalence has been proven from the same snapshot. The deployment
gate must refuse to schedule or cross activation until every validator reports
the candidate release identity. At activation there is no compatibility
fallback: a legacy proposal, certificate, replay result, or catch-up result has
the wrong state root and fails closed.

## Snapshot migration and rollback procedure

The state-root upgrade changes commitment semantics, not the JSON layout of
the replicated state files. Snapshot v6 already carries the complete ledger,
governance, shielded, bridge, block, receipt, ordered-batch, validator-safety,
and QC artifacts. Import remains content-addressed and verifies the manifest,
file hashes, chain identity, tip, and state root before the node may join.

1. Before committing the amendment, export and sign a v6 snapshot at an exact
   full-committee tip. Record the legacy root, block hash, genesis hash,
   amendment absence, and old/candidate binary hashes.
2. Import that snapshot into a fresh candidate data directory. Verify the full
   block log and state, then replay to the same tip. The restored candidate root
   must equal the pre-activation legacy root byte-for-byte.
3. Commit the future-height amendment and export a second signed v6 snapshot.
   Its governance state must contain the immutable schedule while its tip still
   uses the legacy root encoding.
4. A rollback before activation restores the first pre-amendment snapshot and
   the old binary to every validator as one coordinated operation, or restores
   the second snapshot only with a binary that preserves the already-committed
   schedule. Never overlay a snapshot onto an existing data directory.
5. Once the activation block commits, do not restore an old binary or create a
   new block from a pre-activation snapshot. Forward recovery imports the most
   recent signed v6 snapshot into a fresh directory, runs the candidate binary,
   and replays through the activation boundary while comparing every block and
   root.
6. Snapshot v5 is not an activation vehicle. It may be imported only where the
   existing v5 compatibility checks permit it, then must be immediately
   re-exported as v6 and pass the same pre-activation equivalence gates. Any v5
   snapshot lacking required activated signer safety state is rejected.

The rollback drill is successful only when the isolated six-node clone returns
to one exact pre-activation tip/root on the old release and can subsequently
move forward again on the candidate, cross the scheduled height once, and
replay the identical post-activation root.

## Required release evidence

- legacy genesis JSON round-trip and genesis hash stability;
- new-genesis height-zero activation;
- rejection of same-block and backdated activation;
- pre-activation old/new root equality with nonempty FastLane state;
- exact transition-height root change and full-committee agreement;
- post-activation block/receipt replay from a real checkpoint;
- restart, snapshot/restore, and forward-recovery drills;
- binary, manifest, amendment, block, receipt, and root hashes.

Until that evidence exists for the exact candidate and target chain, the code
fix is local and the existing-chain rollout gate remains open.
