# Cobalt Adversarial Verification Milestone

## Plain-English outcome

This milestone tests the Cobalt authority that is already live on the controlled testnet. Cobalt stays active only when adversarial validators, trust views, schedules, history, authority transitions, and resource pressure cannot produce conflicting registry roots, false decisions, unsafe recovery, or an unacceptable Consensus v2 finality regression.

A failed benchmark or simulation creates P0 remediation and a rerun of the unchanged adversarial corpus. Rollback to Foundation authority is reserved for the published live stop conditions. Cobalt ratifies validator-registry and trust-graph changes; it does not choose which validators deserve trust, decentralize the proposal source, order blocks, or replace Consensus v2.

The result proves protocol capability, not operator decentralization.

- **Status:** Active — E1 in progress; E2-E6 not started
- **Specification:** [Cobalt Adversarial Verification Research Specification](../../governance/cobalt-adversarial-verification-research-spec.md)
- **Specification lock task:** `task_158622307482e23fb4519889b53b475f` — rewarded 2026-08-25
- **Milestone-document task:** `[ ] task_d28eb3465dcac9a32524c25bba996e1e — accepted 2026-08-25`
- **Required result:** `[ ] KEEP ACTIVE`

## Current position

- [x] Cobalt became the live controlled-testnet validator-trust authority at height 916; its first authorized validator-key rotation committed at height 917.
- [x] The research specification passed the Text Improvement Harness at 89.27/100 and is locked.
- [x] The public article now records the live authority state instead of saying Cobalt remains off.
- [ ] The cooperative activation evidence has not yet passed the adversarial E1-E6 campaign.
- [ ] The current Foundation-administered proposal and authorization boundary remains explicit; no operator-decentralization result is claimed.

## Major Task Node work

Each experiment is one substantial Task Node task, requested only after the previous experiment reaches its final rewarded outcome. Checklist items inside an experiment are not separate task requests. CLI and browser work stays governed inside these experiment tasks rather than becoming microtasks.

### E1. Independent oracle and generated corpus

Task Node: `[ ] task_59460b82c134e725fd1c902e2c3417b8 — accepted 2026-08-25`

- [x] Build a second oracle from the formal essential-subset, strong-support, and linkage rules without importing production Cobalt or the first oracle.
- [x] Generate at least 10,000 trust graphs covering 6-20 validators and every linkage-inequality boundary.
- [x] Compare both oracles with production `analyze_trust_graph`, `has_strong_support`, and non-uniform certificate validation.
- [ ] Freeze every mismatch before fixes; review any oracle correction; add regression cases for production defects; rerun the unchanged corpus from clean state.
- [ ] Pass with complete oracle agreement and production agreement on every generated graph.

Initial comparison: 8,534 disagreements frozen before remediation; review found one harness-domain fixture defect and no oracle or production defect. The reconciled full pass has zero disagreements across 10,240 cases; clean rerun pending.

Code references: `crates/cobalt_adversarial_oracle/src/lib.rs`, `crates/cobalt_e1_harness/src/main.rs`, `crates/cobalt_decision_oracle/src/lib.rs`, `crates/consensus_cobalt/src/trust_graph_governance.rs`, `crates/node/src/cobalt_authority_certificate.rs`.

### E2. Byzantine validator campaign

Task Node: `[ ] not requested`

- [ ] Derive and freeze the live six-validator fault bound `f` before execution.
- [ ] Exercise up to `f` Byzantine domains across separate and combined RBC, ABBA, MVBA, and DABC equivocation; selective withholding; lying or changing trust views; competing proposals; late votes; and re-proposals.
- [ ] Search delay, drop, reorder, duplicate, and partition schedules for maximum disagreement or delay instead of replaying a fixed schedule list.
- [ ] Prove zero conflicting roots, zero false accepts, compatible progress within the synchrony bound, and incompatible safe halt without registry mutation.
- [ ] Bind every Byzantine attribution to signed misbehavior evidence.

Code references: `crates/consensus_cobalt/src/rbc_abba_mvba.rs`, `crates/consensus_cobalt/examples/cobalt_adversarial_harness.rs`, `crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs`, `crates/node/src/cobalt_shadow.rs`.

### E3. Adversarial recovery

Task Node: `[ ] not requested`

- [ ] Test each validator in turn on a disposable clone bound to the live registry root.
- [ ] Restart from truncated, padded, reordered, and one-entry-modified durable histories.
- [ ] Reject fabricated transitions, wrong-root certificates, and catch-up histories that omit the latest update, each with a named reason.
- [ ] Interrupt catch-up mid-transfer, resume from another peer, and reject inconsistent peer material before rejoin.
- [ ] Restore byte-identical accepted history from honest peers without manual repair.

Code references: `crates/node/src/cobalt_shadow.rs`, `crates/node/src/cobalt_shadow_runtime.rs`, `crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs`.

### E4. Finality isolation under governance stress

Task Node: `[ ] not requested`

- [ ] Run at least 500 baseline and 500 attack-lane Consensus v2 rounds from the same signed state on the same fleet, binary, and CPU quota.
- [ ] In the attack lane, combine governance storms, repeated halts and view changes, near-limit certificates and RPC frames, sidecar flooding, and one crash-looping validator.
- [ ] Count and name every rejection at the 1 MiB certificate and 2 MiB RPC-frame boundaries.
- [ ] Pass only if Consensus v2 never stops or forks and attack-lane p95 client-visible finality remains within 5% of baseline.
- [ ] Record governance latency, p50/p95 finality, CPU, memory, network, disk, rejected inputs, restarts, and operator actions.

Code references: `benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py`, `crates/node/src/consensus_v2_finality.rs`, `crates/node/src/cobalt_authority_certificate.rs`, `crates/node/src/cobalt_shadow_runtime.rs`.

### E5. Live authority drills

Task Node: `[ ] not requested`

- [ ] At scheduled live heights, execute the signed forward rollback to Foundation authority and the separately authorized return to Cobalt authority.
- [ ] Run the early, stale, replayed, wrong-root, cross-chain, mixed-authority, self-authorized, and replayed-rollback negative cases live without mutation.
- [ ] Treat one validator key as stolen, reject its attempted Cobalt-authorized rotation, and commit the legitimate rotation path.
- [ ] Record the proposal and authorization identities for every drill.
- [ ] Prove one accepted history and uninterrupted Consensus v2 finality through both authority transitions.

Code references: `crates/node/src/cobalt_handoff.rs`, `crates/node/src/cobalt_handoff_rehearsal.rs`, `crates/node/src/bin/postfiat_cobalt_handoff_rehearsal.rs`, `benchmarks/cobalt-activation-live/packet`.

### E6. Proposal source and independence decision

Task Node: `[ ] not requested`

- [ ] Document the current registry-proposal process, signing keys, validator authorizations, and custody boundaries.
- [ ] Design a non-Foundation proposal path and trust graph in which no single administrator can reach quorum or block it alone.
- [ ] Reuse the retained operator-admission boundary and onboarding packet; do not recruit operators inside this experiment.
- [ ] Record a dated decision to reinstate the locked activation specification's independent-operator gate as its own milestone or formally defer it.
- [ ] Lock that decision through a new research specification; do not redefine the prior gate inside this milestone.

Code references: `crates/node/src/cobalt_shadow.rs`, `crates/node/src/cobalt_authority_certificate.rs`, `benchmarks/cobalt-independent-operators/onboarding-contract.json`.

## Gates

### KEEP ACTIVE

- [ ] E1: production matches the reconciled independent oracle on every generated graph.
- [ ] E2: zero conflicting roots, zero false halts, and zero false accepts under the Byzantine campaign.
- [ ] E3: every tampered state and forged catch-up is rejected; honest recovery is byte-identical.
- [ ] E4: Consensus v2 never stops or forks; attack-lane p95 finality stays within 5% of baseline.
- [ ] E5: both live authority transitions commit; every live negative case and the stolen-key attempt reject.
- [ ] E6: the proposal-path design and independent-operator decision are recorded and locked.
- [ ] The publication requirements are live.

### ROLL BACK

- [ ] Use the rehearsed signed transition only after a live conflicting root, failed five-of-six progress under an honest majority, divergent catch-up history, unexpected block authority, or sustained Consensus v2 finality regression.
- [ ] Record `ROLLED_BACK` in the evidence packet and bind it to the resulting live authority state.

A failed benchmark or simulation alone does not trigger rollback.

### REMEDIATION

- [ ] Treat any failed experiment gate as P0 work at the owning code boundary.
- [ ] Add regression coverage and rerun the unchanged adversarial corpus from clean state.
- [ ] Keep the corpus and oracles frozen unless an independently demonstrated oracle defect receives separate review.
- [ ] Record `REMEDIATION_REQUIRED` until every affected gate passes.

## Human interfaces

- [ ] Deliver `python -m postfiat_rpc.cobalt adversarial` to verify the packet, report the gate state, and list every rejected adversarial case with its reason. Code: `python/postfiat_rpc/cobalt.py`.
- [ ] After the CLI works, add a read-only browser panel beside the Cobalt observatory for the gate state, live authority transitions, and proposal and authorization identities. Code: `python/postfiat_rpc/cobalt_ui.py`.
- [ ] Make both interfaces consume the same authenticated packet and fail closed on missing, mutated, or inconsistent evidence.
- [ ] Test the CLI and browser behavior inside the governing experiment tasks.

## Evidence and publication

- [ ] Produce `adversarial-status.json` with `KEEP_ACTIVE`, `REMEDIATION_REQUIRED`, or `ROLLED_BACK`, bound to the live authority state.
- [ ] Include the frozen threat model and `f`, both pinned oracles, corpus manifest and classifications, per-validator E2/E3 results, signed misbehavior evidence, finality and resource receipts, live drill receipts, and the E6 decision.
- [ ] Include CLI and browser output, `SHA256SUMS.txt`, and a verifier that fails on missing, mutated, or inconsistent evidence.
- [ ] Use “validator-registry ratification” or “validator-trust governance,” never bare “validator governance.”
- [ ] State on the first page that Cobalt ratifies registry changes, a separate layer decides who deserves trust, and current proposals originate from Foundation-administered validators.
- [ ] Publish what was attacked, what held, what was fixed, and what remains open, while stating that the result proves protocol capability rather than operator decentralization.

## Completion

- [ ] Every E1-E6 Task Node task has reached its final rewarded outcome.
- [ ] The final gate is recorded as `KEEP_ACTIVE`, `REMEDIATION_REQUIRED`, or `ROLLED_BACK` and matches the live authority state.
- [ ] The packet verifier, focused Cobalt and governance tests, CLI and browser tests, strict documentation build, redaction checks, and one explicit final release gate pass.
- [ ] The CLI and read-only browser interface work against the authenticated packet.
- [ ] The adversarial results and precise public claims are published.
- [ ] Honest Task Node evidence and final verification are complete.
- [ ] Move this journal to `docs/plans/completed/` only after the interfaces, publication, and selected completion gates pass.
