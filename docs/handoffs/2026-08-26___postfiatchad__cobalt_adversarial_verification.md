# Cobalt adversarial verification continuation

- **Operator:** Post Fiat Chad (`postfiatchad`)
- **Date:** 2026-08-26 UTC

## BLUF

Resume from the
[active Cobalt adversarial-verification milestone](../plans/active/cobalt-adversarial-verification-milestone.md).
E1-E4 and the design-only E6 decision are complete. E4 passed its unchanged
500+500-round local finality-isolation rerun and its checksum-bound packet is
pushed on `main` at `6c22f866e9ba56ec18f3a62fbf2b00ec9aa17103`.
E5 has not been executed live, publication is not complete, and the overall
`KEEP_ACTIVE` gate is still open.

The controlled devnet was not mutated by E4. Current Git is not the deployed
runtime. Cobalt ratifies validator-registry and trust-graph changes; a separate
layer decides who deserves trust, current proposals originate from
Foundation-administered validators, and Consensus v2 remains block finality.
The result proves protocol capability, not operator decentralization.

Continue this work directly. The user explicitly instructed this session not to
use Task Node and not to spawn parallel agents.

## Current state

### Repository

- Branch: `main`
- Latest pushed evidence commit: `6c22f866e9ba56ec18f3a62fbf2b00ec9aa17103`
- E4 packet root:
  `93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`
- `origin/main` matched `6c22f866` immediately after the E4 push.
- The worktree also contains intentional, uncommitted E5 protocol/runtime,
  rehearsal, CLI, and browser changes. They are not qualified or deployed yet.
  Preserve them; do not reset, check out, or stage everything blindly.

[Current State](../status/chain-state-current.md) is the canonical operational
reference. The active milestone is the authoritative execution plan. This
handoff records the continuation boundary and must not replace either one.

### Last observed controlled devnet

An authenticated read-only validator/RPC probe ran across all six hosts from
`2026-08-26T01:40:51Z` through `2026-08-26T01:41:04Z`. It changed no fleet
files or services and called no mutation RPC.

- Chain: `postfiat-wan-devnet-2`
- All six validator and all six RPC services: active
- All validators: `running`, height 919, zero pending transactions
- Tip:
  `3a8a117af9ed40728717005d03edf032719a3ca3d696365415a2d5b0d9aeef1c509d06d54029e6c34660e29aab43d0fb`
- State root:
  `ffa16323555800df7a4ff7cd336b9b151b0edfcf60954c207b704749133ff4b31ebd24444696d67e652f6e94510f7e60`
- Deployed validator release: `cobalt-verifier-92b63f5a`
- Deployed validator binary SHA-256:
  `c7cb0c25001a0bfe22eba32ce870f3739f9710471906e27c32797670ea9f6337`

That later probe did not re-run the governance auditor or inspect shadows. The
last full authenticated audit at `2026-08-25T15:37:40Z` records Cobalt
validator-trust authority from height 916, the first Cobalt-authorized key
rotation at height 917, registry root
`945768d593497541f59961d1ba3920560cfde7bf5037e40eb89dd5466637f221709bff05b69d2d40a36d5cff8505c37e`,
and trust root
`9221316ae7f0f0e7e58d734700167f73f29aa1240377a8d61c637e7f36c5deb728203fcbb283c9f8f3398fc41d6b8b13`.
The same older audit found all six advisory shadow services active.

### E4 result

The final unchanged campaign ran 500 baseline and 500 attack rounds on the same
local six-validator topology, signed initial state, binaries, full-vote policy,
and CPU allocation.

- Both lanes converged independently at height 501.
- Consensus v2 never stopped or forked.
- Baseline wallet-to-finality: p50 `7,471.082586 ms`, p95
  `14,133.573682 ms`.
- Attack wallet-to-finality: p50 `7,500.377266 ms`, p95
  `14,197.471440 ms`.
- Attack p95 delta: `+0.4520990899943289%`, inside the 5% budget.
- Governance stress: 47 runs, 940 proposals, 329 safe halts, 329 view changes.
- Rejections: 987 boundary, 846 named-limit, 752 flood; durable state unchanged.
- Validator 5: 12 automated restarts; zero manual operator actions.
- Source: `add07a7cce416daeaa61073085734937477f2b71`.
- Packet verifier prints `e4-packet-ok` and the packet root above.

The packet preserves two harness remediations: a retry window shorter than the
deliberate restart outage, and an invalid exact-hash comparator across
independent randomized executions. Neither observed a fork or durable
divergence. The corpus, topology, binaries, quota, crash cadence, and
adversarial inputs remained unchanged. E4 was isolated local evidence; it did
not query or mutate devnet.

### Uncommitted E5 and interface work

The intentional dirty tree covers:

- cross-root DABC ratification lineage and persisted shadow anchors;
- full decision-certificate validation during registry reset;
- forward rollback and separately authorized return preparation;
- explicit activation-height handling for DABC updates;
- a read-only nine-negative-case plus stolen-key E5 drill;
- rehearsal helpers for live update/certificate assembly;
- a fail-closed adversarial packet CLI and read-only browser panel; and
- focused Rust and Python regression tests.

The major production issue under remediation is that a post-rotation DABC
ratification must validate against the prior committed ratification/lineage
anchor, not assume the current graph roots are also the previous decision roots.
The changes have passed formatting and the Python unit subset previously
recorded, but have not yet passed their focused Rust test suite, release build,
rolling deployment, or live E5 drill. Do not describe them as validated.

The E3 packet verifier correction is already pushed at `ec3c3833`; its current
packet root is
`9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600`.

## Next decision or action

1. Review and qualify the dirty E5 protocol/runtime diff off-chain. Run
   formatting, focused Cobalt/governance tests, persisted-height-917 migration
   coverage, cross-root lineage regressions, and a release build.
2. Commit and push coherent E5 source changes before building deployment
   evidence. Do not derive deployment identity from an uncommitted tree.
3. Re-probe all six validators, RPCs, authority state, and shadows. Re-derive
   every scheduled height from that fresh state; the prior 920/921/922 sketch is
   provisional.
4. Use the signed safe-rollout workflow one validator/RPC pair at a time, proving
   convergence after each. Migrate shadow ratification anchors separately and
   stop on any root, signer, lineage, service, or finality mismatch.
5. Execute E5: signed forward rollback to Foundation, separately authorized
   return to Cobalt, every named negative case without mutation, certified
   stolen-key rejection, and the legitimate validator-5 rotation. Preserve one
   accepted authority history and uninterrupted Consensus v2 finality.
6. Build and verify the E5 and consolidated evidence packets, CLI/browser
   snapshots, publication, canonical state, strict docs, redaction checks, and
   the explicit final release gate.
7. Move the milestone to `docs/plans/completed/` and record `KEEP_ACTIVE`,
   `REMEDIATION_REQUIRED`, or `ROLLED_BACK` only when every completion gate
   is supported by direct evidence. Then replace this checkpoint with the final
   clean handoff and push it.

## References

- [Current State](../status/chain-state-current.md)
- [Active adversarial-verification milestone](../plans/active/cobalt-adversarial-verification-milestone.md)
- [Locked adversarial-verification specification](../governance/cobalt-adversarial-verification-research-spec.md)
- E4 packet: `benchmarks/cobalt-adversarial-verification/e4/`
- E4 verifier:
  `python3 benchmarks/cobalt-adversarial-verification/e4/verify_packet.py`
- E3 packet: `benchmarks/cobalt-adversarial-verification/e3/`
- E6 packet: `benchmarks/cobalt-adversarial-verification/e6/`
- Historical activation packet: `benchmarks/cobalt-activation-live/packet/`
- [Prior post-activation handoff](2026-08-25___dravlic__cobalt_post_activation_review.md)
