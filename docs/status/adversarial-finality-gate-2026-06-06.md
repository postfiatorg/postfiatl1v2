# Adversarial Finality Gate

Date: 2026-06-06 UTC
Status: passed focused local gate and expanded process-level transport chaos gate
Scope: PostFiat L1 v2 fast finality, proposal votes, quorum certificates, local replay, and controlled local TCP validator services

## Bottom Line

The fast-finality path now has executable adversarial gates for the core safety
invariant:

```text
No two different blocks can both become final at the same height on one valid
local chain replay.
```

One real gap was found while building the local gate: proposal vote creation detected
double-vote equivocation after the fact, but did not persistently refuse a
second conflicting proposal vote for the same validator, height, and view. That
is now guarded by a proposal-vote lock.

## Code Change

`crates/node/src/block_finality.rs` now writes a per-validator proposal-vote
lock for proposal-backed votes before computing the vote signature:

```text
block_proposal_vote_locks/<height>.<view>.<hashed-height-view-validator>.json
```

Behavior:

- same validator, same height/view, same proposal: may re-sign the same
  deterministic vote if needed;
- same validator, same height/view, different proposal: rejects before computing
  or returning a second signature;
- committed-block audit votes are unchanged.

The lock filename is hash-derived and does not place the validator id directly
in the filesystem path. The reservation uses an atomic hard-link into the final
lock path, so concurrent conflicting proposals cannot both reserve the same
validator/height/view slot.

## Tests Added

Added node-level regressions in
`crates/node/src/lib_test_parts/consensus_block_history_snapshot_tests.rs`:

| Test | Safety case |
|---|---|
| `conflicting_certified_proposal_cannot_apply_after_height_committed` | a stale external certificate for height 1 cannot apply after height 1 is already committed |
| `stale_proposal_vote_cannot_be_reused_for_next_height_certificate` | a height-1 proposal vote cannot certify a height-2 proposal |
| `tampered_parent_or_state_root_proposal_rejects_votes` | validators reject proposals whose parent hash or state root differs from local deterministic evidence |
| `under_quorum_partition_votes_cannot_form_certificate` | a 4-of-6 partition cannot form a certificate when quorum is 5 |

Updated existing equivocation coverage:

| Test | Updated behavior |
|---|---|
| `block_vote_equivocation_evidence_detects_conflicting_signed_votes` | same-node conflicting proposal vote is now refused; evidence verification still accepts already-produced conflicting vote artifacts from an isolated signer state |

## Local Gate Command

```bash
set -euo pipefail
cargo test -p postfiat-ordering-fast -- --nocapture
for test_name in \
  split_block_votes_reconstruct_certificate \
  signed_block_proposals_verify_before_votes \
  timeout_votes_reconstruct_hotstuff_timeout_certificate \
  proposal_certificate_accepts_three_of_four_bft_quorum \
  block_vote_equivocation_evidence_detects_conflicting_signed_votes \
  block_proposal_equivocation_evidence_requires_signed_conflicts \
  conflicting_certified_proposal_cannot_apply_after_height_committed \
  stale_proposal_vote_cannot_be_reused_for_next_height_certificate \
  tampered_parent_or_state_root_proposal_rejects_votes \
  under_quorum_partition_votes_cannot_form_certificate \
  certify_batch_round_uses_split_keys_without_combined_file \
  verify_blocks_replays_historical_registry_after_live_key_rotation
do
  cargo test -p postfiat-node --lib "$test_name" -- --nocapture
done
```

## Result

Passed:

- `postfiat-ordering-fast`: 20/20 tests passed.
- `postfiat-node` focused finality/certificate gate: 12/12 exact tests passed.

The focused gate covers:

- duplicate vote rejection;
- same-validator conflicting proposal-vote refusal;
- equivocation evidence verification for already-produced conflicting artifacts;
- signed proposal verification;
- timeout certificate verification and tamper rejection;
- three-of-four quorum acceptance;
- four-of-six under-quorum rejection;
- stale vote rejection;
- stale external certificate rejection after local height advances;
- parent-hash and state-root proposal tamper rejection;
- split-key certificate construction;
- historical registry replay after key rotation.

## Process-Level Chaos Gate

Added five process-level gate scripts:

```text
scripts/testnet-proposal-vote-lock-restart-smoke
scripts/testnet-finality-partition-matrix-smoke
scripts/testnet-finality-delayed-vote-retry-smoke
scripts/testnet-byzantine-proposer-disjoint-smoke
scripts/testnet-finality-chaos-gate
```

`testnet-proposal-vote-lock-restart-smoke` exercises the proposal-vote lock
through live validator services:

1. start a validator service;
2. request a proposal-backed vote for proposal A;
3. stop and restart the service with the same node state;
4. request a conflicting proposal-backed vote for proposal B at the same
   height/view;
5. require rejection before a second signature is emitted;
6. restart again and prove the same proposal A can still be signed.

The aggregate chaos gate runs:

| Case | Process boundary |
|---|---|
| `focused_finality_tests` | local finality and certificate regression tests |
| `proposal_vote_lock_restart` | validator service restart preserves same-height/view proposal-vote lock |
| `peer_certified_partial_outage` | 6-validator peer-certified round with one validator offline; quorum still finalizes |
| `finality_partition_matrix` | explicit 3/3, 4/2, and 2/2/2 process partition reports; all remain below quorum and do not finalize |
| `finality_delayed_vote_retry` | delayed validator service starts after the first vote request attempt; retry reaches quorum and finalizes |
| `byzantine_proposer_disjoint` | one proposer signs two different valid proposals and sends them to disjoint live peer sets; neither branch reaches quorum |
| `node_run_peer_certified_restart` | 6-validator peer-certified loop across 3 rounds with services restarted between rounds |
| `transport_batch_tamper` | malformed transport batch frames rejected by live listener |
| `transport_certified_batch_tamper` | malformed certified-batch payload/certificate rejected; valid send still works afterward |

Gate command:

```bash
VALIDATORS=6 ROUNDS=3 TIMEOUT_SECONDS=50 TRANSPORT_TIMEOUT_MS=3000 \
  SEND_RETRIES=1 RETRY_BACKOFF_MS=75 \
  scripts/testnet-finality-chaos-gate
```

Result:

```text
reports/testnet-finality-chaos-gate/run-20260606T190718Z/testnet-finality-chaos-gate.json
sha256 07d97d1564d7f0463c1ea86a59ae6c2502be9d7415d193963a7f29b996d001dd
```

Summary:

- `chaos_gate_ok`: true
- validators: 6
- restart rounds: 3
- cases passed: 9/9
- `residual_work`: `[]`

## Full-Suite Caveat

A full `cargo test -p postfiat-node --lib -- --nocapture` was attempted but is
not a clean gate at the moment because unrelated golden-vector tests fail in
governance-agent/state-root/wallet fixtures. The finality tests listed above
passed independently and are the current gate for this slice.

## Completed Stronger Chaos Matrix

The prior remaining process-level matrix is now covered by the aggregate gate:

- explicit 3/3, 4/2, and 2/2/2 partition reports;
- delayed vote response and retry injection beyond one offline peer;
- Byzantine proposer sends two valid signed proposals to disjoint live peer
  sets in one scripted run.
