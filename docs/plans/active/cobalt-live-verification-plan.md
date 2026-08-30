# Cobalt Live Verification

**Goal:** Cobalt on, current configs, proven working on the running devnet. Nothing else.

- [ ] 1. Read-only fleet probe: all six validators up, heights converged, governance
      `authority_mode`, registry root, and trust root recorded (compare against
      `docs/status/chain-state-current.md` h924 values)
- [ ] 2. Confirm Cobalt is the active registry authority (`authority_mode = 1`,
      Cobalt-ratified) with the currently deployed binary and configs — no
      upgrades, no config changes
- [ ] 3. Execute one real signed registry transition through the full path
      (proposal → old-rule ML-DSA quorum → Cobalt transition check → commit),
      e.g. a trust-graph re-ratification; all six validators accept one root
- [ ] 4. Negative check: submit one invalid transition (stale or wrong-root);
      every validator rejects it with a named reason and no durable mutation
- [ ] 5. Verify receipts: ratification anchor advances, governance-verifier
      receipts collected from all six hosts, one converged observation recorded
- [ ] 6. Write the result into `docs/status/chain-state-current.md` and a short
      handoff; Gate Zero Z1 window starts counting from here

Operator authorization: this plan touches the controlled devnet (steps 3–5 are
mutations). Proceeding only on explicit go.

References: E5 drill machinery (`crates/node/src/cobalt_e5_live_drill.rs`,
`cobalt_handoff.rs`), fleet observation runbook, prior drill history heights
920–924.
