# Cobalt governance evaluation

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-24 UTC

## BLUF

Cobalt is the required validator-trust governance authority for the controlled
testnet; it does not replace Consensus v2 block finality. The decisive corpus,
isolated-validator liveness simulation, and paired Consensus v2 integration now
pass. Foundation authority remains live because the owner paused work before
deployment. Finish the release-lineage candidate and disposable-clone rollback
rehearsal, then stop at the live cutover boundary.

## Current state

- Production Cobalt matches all 18 frozen decisive scenarios with no per-node
  mismatch or conflicting root, including compatible non-uniform trust views
  and the corrected 90%-overlap boundary.
- The RippleD 3.1.3 governance adapter admits conflicting registry roots in the
  divergent-local-quorums case; Cobalt rejects that trust graph before
  commitment. Native RippleD ledger consensus is a separate control.
- Cobalt-authorized registry admission requires a validator-key-bound
  RBC -> ABBA -> MVBA -> DABC decision certificate over the exact update,
  current chain domain, registry root, and trust graph.
- The isolated six-validator simulation passes five-of-six progress,
  four-of-six safe halt, signed catch-up, restart recovery, byte-identical
  durable history, fault schedules, and validator/trust transitions. It is
  protocol-capability evidence, not independent-operator evidence.
- The paired 50-round baseline and 50-round Cobalt integration run kept
  Consensus v2 finality within budget: p95 moved from 1617.88 ms to 1660.42 ms,
  a 2.63% increase, while Cobalt covered 99.9985% of the integration window.
- The release-lineage repair reproduces the deployed devnet history through
  height 915 on quarantine data. The final debug-free binary still needs the
  focused replay checks and exact 915-block replay.
- The live six-validator fleet remains unchanged. Foundation authority is
  active; Cobalt authority and Cobalt block control are inactive.

## Next action

1. Run only the focused replay-compatibility checks required by the changed
   historical state boundary.
2. Build the final debug-free release binary and verify all 915 archived blocks
   against the quarantine copy.
3. Commit the release-lineage repair and rerun the signed disposable-clone
   handoff, negative cases, and forward rollback.
4. Stop before live deployment or activation, as directed by the owner.

## References

- [Active Cobalt Activation Milestone](../plans/active/cobalt-activate-or-retire-milestone.md)
- [Activation Research Specification](../governance/cobalt-activate-or-retire-research-spec.md)
- `benchmarks/cobalt-activate-or-retire/section2-packet`
- `benchmarks/cobalt-activate-or-retire/section3-packet`
- `crates/consensus_cobalt/src/trust_graph_governance.rs`
- `crates/node/src/cobalt_authority_certificate.rs`
- `crates/node/src/cobalt_handoff.rs`
- `crates/node/src/block_replay_wallet.rs`
