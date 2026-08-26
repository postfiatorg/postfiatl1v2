# Storage scaling implementation handoff

- **Operator:** Post Fiat Chad (postfiatchad)
- **Date:** 2026-08-26 UTC

## BLUF

Repaired the Cobalt adversarial packet's missing E1 verifier and stale
publication bindings, then started the storage-scaling milestone that blocks a
public testnet. Source commit
dfd0b9f11108b0b773d1e02bebae71685864228e replaces synchronous full-prefix
JSONL append work, adds a versioned ordered-history accumulator/index and
activation boundary, removes full ordered-history reconstruction from the
post-activation proposal/commit path, and extends replay and crash recovery.
Synthetic counters are bounded through height 5,000, but this is an undeployed
development candidate, not a milestone PASS. Resume from the
[active milestone](../plans/active/storage-scaling-milestone.md) and use
[Current State](../status/chain-state-current.md) for the live/repository
boundary.

The operator explicitly directed this session not to use Task Node or
subagents. Neither was used. Dynamic UNL proposal sourcing was not started.

## Current state

### Repository and deployed-state separation

- Branch: main.
- Cobalt packet repair commit:
  4120206726d01f34278639a1d315883f58669dfa.
- Bounded-storage implementation commit:
  dfd0b9f11108b0b773d1e02bebae71685864228e.
- The commit containing the development packet, milestone, documentation, and
  this handoff is a later documentation/evidence descendant. Use
  git rev-parse HEAD after pulling for the exact pushed checkout.
- These repository commits are not deployed. The last fleet-bound node source
  remains 8cc7d15edc58b5f5a0b745143fef2d45203465ff.
- No fleet probe, deployment, service restart, validator write, snapshot
  import, or live migration occurred in this session.
- The last committed fleet observation remains
  2026-08-26T06:34:55Z–06:35:50Z: all six validators converged at height 924.
  That is a point-in-time observation, not a right-now claim.

### Adversarial evidence repair

The missing benchmarks/cobalt-adversarial-verification/e1/verify_packet.py is
now committed. E1 through E6, the consolidated verifier, and the Python Cobalt
CLI pass from the repaired packet.

Current roots:

- E1: 495a59a27d6deb9f9872ae583077ef31296a978a0841cd7345593f42e4dfcd90.
- E2: 8742d9603621408339d99c3d9fcc1ba8cc43dafdc900acdfccbf86cc60d7cba3.
- E3: 9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600.
- E4: 93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508.
- E5: 0695284a7b38ac0129c47e1242f4a2227ad25096147920e79569a924e5f3b3db.
- E6: ee6848f516347a5e6f4a76b6d7c3bfbcede370010e548b9af4fe009a3121be0b.
- Consolidated packet:
  78e375ccfa09914dd1ea15c429ca1aff70137173bd84021720613b63fe6e2d63.

The publication binding was also corrected in the public-site repository and
its Pages build passed. An offline packet verifier authenticates committed
evidence; it is not a fleet probe.

### Storage implementation candidate

Completed in the current source:

- authenticated JSONL head v2 binds storage format, chain, genesis, protocol,
  log kind, record count, byte offset, current/previous MAC, finalized height,
  block hash, and state root under the node-local key;
- normal append reads only the authenticated head and at most one crash suffix;
  legacy v1 heads perform one full authenticated scan and migrate to v2;
- partial suffix recovery, missing logs, cross-log substitution, stale heads,
  rollback, mutation locks, and chain-tip rebinding fail closed or recover
  through the ordered-commit journal;
- process-local counters expose checkpoint, suffix, legacy-prefix, bitmap, and
  slot work;
- an authenticated fixed-slot ordered-batch index and
  postfiat-ordered-history-v2 accumulator provide bounded duplicate lookup and
  state commitment;
- Genesis has an optional ordered_history_v2_activation_height. Its absence
  preserves historical serialization and state roots;
- the index tracks pre-activation batches, then proposal, validator rebuild,
  commit, current-state verification, full replay, and history-checkpoint replay
  select v2 at the explicit activation boundary;
- transparent, governance, shielded, and bridge proposal/commit paths do not
  materialize the full ordered-batch list after activation;
- the offline operator command is:
  postfiat-node ordered-history-index-rebuild --data-dir PATH --offline-confirmed.

The development evidence packet is in benchmarks/storage-scaling/. Its verifier
reports storage-scaling-development-evidence-ok with manifest root
6397af175a70aaee0d8943f65822320c34f96cc0341f675dad6d45e443d5287d.

Controlling-counter result:

- JSONL append at heights 50, 100, 500, 1,000, and 5,000 verified zero
  accepted-prefix records and read only a 595–599-byte checkpoint.
- Proposal lookup plus index append read about 2.10 MB of authenticated bitmap
  data, wrote about 1.05 MB, read zero historical-prefix bytes, and wrote one
  slot at every sampled height.
- The fixed work is height-independent, but the unoptimized single-node harness
  measured about 0.33 seconds per proposal lookup and 0.64 seconds per index
  append. The fixed-slot candidate therefore still needs optimization or
  comparison with the specification's embedded B-tree/key-value candidate.
- These measurements are not six-validator finality evidence and are not an
  SLA.

### Verification completed

Passed against the implementation commit:

- postfiat-storage: 51 passed, zero failed; two manual height-5,000 evidence
  tests are ignored in the normal suite and passed when run explicitly;
- replicated_state_activation: three passed, including every ordered-commit
  persistence prefix at the ordered-history v2 activation boundary;
- ordered-history rebuild CLI offline-confirmation guard: passed;
- postfiat-node all-target compilation: passed;
- E1–E6 and consolidated Cobalt packet verification: passed;
- Python Cobalt adversarial CLI: passed;
- development evidence verifier and JSON parse: passed;
- formatting and git diff checks: passed before the documentation handoff.

## Next decision or action

Do not deploy this candidate and do not clear the public-testnet block.

Resume the unchecked items in the active milestone in this order:

1. Implement or evaluate the required embedded ordered key-value/B-tree
   candidate with atomic write batches, and reduce the fixed bitmap's high
   constant cost before selecting a store.
2. Run the exact 915-block quarantine archive and authenticated height-924
   history through legacy and v2 replay; require byte-identical pre-activation
   artifacts and matching post-activation synthetic artifacts.
3. Extend the complete E3 tamper and recovery corpus to checkpoints, index
   files/pages, stale valid heads, journal disagreement, and every index crash
   cut.
4. Freeze and run the paired six-validator windows at heights 50, 100, 500,
   1,000, and 5,000. The controlling release gate is height-5,000 p95 finality
   no more than 110% of height-50 p95.
5. Resolve activation for the existing height-924 chain. The current
   genesis-bound field works for a new chain but cannot be added to the existing
   genesis without changing its hash.
6. Only after those gates, write and rehearse the six-clone migration and
   rollback runbook, then deliver the required Python packet CLI and read-only
   browser interface.

Any live probe, deployment, restart, validator mutation, or migration requires
separate authorization and fresh fleet-bound evidence.

## References

- [Current State](../status/chain-state-current.md)
- [Active storage-scaling milestone](../plans/active/storage-scaling-milestone.md)
- [Storage-scaling research specification](../architecture/storage-scaling-research-spec.md)
- [State and Storage](../architecture/state-and-storage.md)
- [Evidence index](../evidence/index.md)
- [Dom's evidence review and next specifications](2026-08-26___dravlic__evidence_review_and_next_specs.md)
- Development packet: benchmarks/storage-scaling/
- Bounded JSONL implementation: crates/storage/src/lib.rs
- Ordered-history implementation: crates/storage/src/ordered_history.rs
- State commitment: crates/node/src/state_commitment.rs
- Proposal/commit paths: crates/node/src/mempool_proposals.rs,
  crates/node/src/batch_snapshot.rs, and crates/node/src/storage_commit.rs
- Replay paths: crates/node/src/block_replay_wallet.rs and
  crates/node/src/history.rs
