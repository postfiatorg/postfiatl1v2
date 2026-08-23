# Cobalt Activate-or-Retire Milestone

## Plain-English outcome

This milestone ends the Cobalt experiment.

Cobalt will either become the live authority for validator-registry and trust-graph changes on the controlled testnet because it proves a material safety and decentralization advantage, or it will be removed from the production path. Shadow-only operation, another readiness recommendation, and indefinite evaluation are not completion states.

Cobalt does not replace Consensus v2. It controls validator-trust governance only.

- **Status:** Active
- **Locked specification:** [Cobalt Activate-or-Retire Research Specification](../../governance/cobalt-activate-or-retire-research-spec.md)
- **Specification lock task:** `task_a8ffc2885a6fbab8aa06c4b20e92f6b8` — rewarded 2026-08-23
- **Milestone-document task:** `task_18b8d92d981221b88d0a38159ea1fd26` — accepted
- **Terminal result:** `[ ] ACTIVATE` or `[ ] RETIRE`

## Current position

The prior milestone proved that six Post Fiat-operated sidecars can run Cobalt in shadow, decide with five-of-six, reject four-of-six, recover signed history, leave Consensus v2 finality running, replay deterministically, and rehearse handoff and forward rollback on a disposable clone.

It did **not** prove the remaining decision:

- the 20-validator 90%-overlap case halted;
- decisive asymmetric cases still use `model_scope: characterize`;
- the oracle is not independent of the production implementation;
- the RippleD adapter reports ledger convergence rather than the same validator-governance decision at every node;
- the six live machines share one Post Fiat operator and provider; and
- Foundation authority remains active.

## Completed foundation

- [x] Run authenticated Cobalt shadow services on the six-validator WAN fleet. Evidence: completed [live-deployment milestone](../completed/cobalt-live-deployment-and-liveness-milestone.md).
- [x] Prove five-of-six progress, four-of-six rejection, canonical decision identity, signed catch-up, restart recovery, and no manual history repair. Code: [`cobalt_shadow.rs`](../../../crates/node/src/cobalt_shadow.rs), [`cobalt_shadow_runtime.rs`](../../../crates/node/src/cobalt_shadow_runtime.rs).
- [x] Keep Consensus v2 finality live during Cobalt outages and recovery. Existing live evidence finalized heights 913→914 and 914→915.
- [x] Build the initial 80-case Cobalt/RippleD packet. Packet: [`benchmarks/cobalt-rippled-liveness/packet`](../../../benchmarks/cobalt-rippled-liveness/packet); `SHA256SUMS` root `7968a085033419255b52b844edd586346a1e85561394e52c69e6683b2561c50b`.
- [x] Rehearse future-height activation, scoped validator-key rotation, negative cases, and forward rollback on a disposable clone. Code: [`cobalt_handoff.rs`](../../../crates/node/src/cobalt_handoff.rs), [`cobalt_handoff_rehearsal.rs`](../../../crates/node/src/cobalt_handoff_rehearsal.rs); packet root `b678b3f45eb2a14299b941101bd556d61795a1033f1f6e53557442b7e315807e`.
- [x] Deliver the existing Python Cobalt CLI and read-only observatory. Code: [`python/postfiat_rpc/cobalt.py`](../../../python/postfiat_rpc/cobalt.py), [`cobalt_ui.py`](../../../python/postfiat_rpc/cobalt_ui.py).

## Major Task Node work

Each segment is one substantial personal task, normally requested after the previous segment reaches final verification. Checklist items inside a segment are not separate Task Node requests.

### 1. Independent oracle and decisive corpus

Task Node: `[ ] request and accept`

- [ ] Write a versioned mathematical decision contract that defines compatible and incompatible trust configurations without importing or calling production Cobalt code.
- [ ] Implement the oracle in a separate benchmark surface with hand-worked boundary fixtures and an explicit prohibition on shared decision logic.
- [ ] Replace every decisive `model_scope: characterize` case in [`generate_scenarios.py`](../../../benchmarks/cobalt-rippled-liveness/generate_scenarios.py) with frozen per-node predictions.
- [ ] Add positive non-identical essential-subset cases, immediate linkage-boundary cases, the 90%-overlap case, divergent registry roots, outage, equivocation, message faults, membership changes, rotation, and missed-history recovery.
- [ ] Make the Cobalt adapter in [`postfiat_cobalt_benchmark.rs`](../../../crates/node/src/bin/postfiat_cobalt_benchmark.rs) report every correct node’s decision, root, certificate, timing, and rejection reason.
- [ ] Make the RippleD adapter in [`MatchedLivenessBenchmark_test.cpp`](../../../benchmarks/cobalt-rippled-liveness/rippled/MatchedLivenessBenchmark_test.cpp) evaluate the same proposed registry decision and expose each node’s local-UNL admission result. Keep native ledger-consensus results separately labeled.
- [ ] Freeze the oracle, scenarios, source pins, adapter hashes, and expected per-node results before execution.

### 2. Decisive run and bounded remediation

Task Node: `[ ] request and accept after Section 1`

- [ ] Run the frozen corpus from clean state through both adapters.
- [ ] Report false accepts, false halts, conflicts, per-node results, decision latency, and byte-identical replay; no post-hoc reclassification.
- [ ] Trace the 90%-overlap result through essential subsets, linkage inequalities, cover extraction, certificate construction, and local validation.
- [ ] Require at least one compatible non-uniform Cobalt decision, one incompatible Cobalt halt, and one fair material safety distinction from RippleD local-UNL admission.
- [ ] If the first run fails, use the specification’s single remediation cycle, record the exact cause and diff, and rerun the unchanged oracle and corpus.
- [ ] If the second run still has a false accept, false halt, unresolved 90% result, conflict, or no fair RippleD distinction, set the terminal path to RETIRE.

### 3. Independent live-validator proof

Task Node: `[ ] request and accept only if Section 2 passes ACTIVATE gates`

- [ ] Establish at least three operationally independent operator groups with separate host administration and signing-key custody, across at least three infrastructure domains.
- [ ] Ensure no operator can reach quorum alone or halt governance by withdrawing all validators it controls.
- [ ] Deploy non-identical but formally compatible trust views.
- [ ] Execute one validator addition, one removal, one ML-DSA key rotation, one compatible trust-view transition, one-validator outage, lagging-node catch-up, and one incompatible safe halt.
- [ ] Prove Consensus v2 continues finalizing, p95 finality stays within 5% of the frozen same-fleet baseline, and no recovery mutates durable history by hand.
- [ ] Collect signed, redaction-safe operator receipts. Geographic distribution under one operator is not independent operation.

### 4. Terminal operation: activate or retire

Task Node: `[ ] request and accept after the binary gate is known`

#### ACTIVATE path

- [ ] Verify a signed forward rollback on a disposable clone immediately before cutover.
- [ ] Schedule and execute the future-height live controlled-testnet authority transition through [`cobalt_handoff.rs`](../../../crates/node/src/cobalt_handoff.rs).
- [ ] Execute one real validator-registry change under active Cobalt authority.
- [ ] Reject early, stale, replayed, wrong-root, mixed-authority, and self-authorized updates without mutation.
- [ ] Keep Cobalt active and Foundation validator-trust authority inactive after all gates pass.

#### RETIRE path

- [ ] Keep Foundation validator-trust authority active.
- [ ] Disable and remove live Cobalt sidecar services.
- [ ] Remove Cobalt activation/readiness controls and claims from the CLI, UI, operator docs, and public positioning.
- [ ] Remove the production Cobalt authority path or isolate only historical-replay compatibility behind a non-production research feature.
- [ ] Preserve the minimum safe historical decoder and verifier surface.

### 5. Human interfaces, packet, and documentation

Governed inside the terminal-operation task; do not request microtasks.

- [ ] Make the Python CLI display the actual terminal decision, authority, registry root, trust-graph root, transition history, and verifier result.
- [ ] Make the browser interface consume the same authenticated output and display ACTIVATE or RETIRE without readiness language.
- [ ] Produce one compact verifier-backed packet containing `decision.json`, frozen oracle and manifest, source pins, per-node results, KPI summaries, 90% trace, signed live receipts or retirement receipts, finality/resource metrics, authority/update/rollback records, CLI/UI output, and `SHA256SUMS.txt`.
- [ ] Publish a concise first-page explanation of what Cobalt controls, the unique benefit tested, compatible/incompatible results, RippleD comparison, independent-validator result, and the final word ACTIVATE or RETIRE.
- [ ] Refresh [`README.md`](../../../README.md), [`STATUS.md`](../../../STATUS.md), architecture/governance docs, CLI help, and operator instructions to match the live result.

## Binary completion gate

### ACTIVATE only if every item passes

- [ ] Zero conflicting Cobalt decisions.
- [ ] Zero false halts in the frozen compatible corpus.
- [ ] Zero false accepts in the frozen incompatible corpus.
- [ ] Byte-identical decision-critical replay.
- [ ] A fair, reproducible material safety distinction from RippleD local-UNL admission.
- [ ] Independent operators complete every live exercise.
- [ ] Consensus v2 stays live and within the 5% p95 finality budget.
- [ ] Live future-height handoff, real Cobalt-authorized registry change, CLI/UI readback, and forward rollback readiness all pass.

### RETIRE if any item remains after one remediation

- [ ] Any conflict or unsafe accepted registry root.
- [ ] Any formally compatible non-uniform false halt.
- [ ] Unresolved or incorrect 90%-overlap behavior.
- [ ] No fair material distinction from RippleD local-UNL admission.
- [ ] Dependence on one operator, publisher, or shared signing-key custody.
- [ ] Consensus v2 disruption or performance-budget failure.
- [ ] Failed live handoff, real registry update, recovery, or rollback.

## Completion and retirement

- [ ] Write `decision.json` with exactly `ACTIVATE` or `RETIRE`.
- [ ] Run the packet verifier, relevant Rust tests, Python CLI/UI tests, formatting, strict Clippy, workspace checks/tests, documentation build, and redaction checks.
- [ ] Submit honest Task Node evidence and final verification for every accepted task.
- [ ] Confirm every Task Node task reaches rewarded state.
- [ ] Move this journal to `docs/plans/completed/` only after the CLI and browser interface work against the terminal live state and every selected path requirement is proven.
- [ ] Remove obsolete active Cobalt plans. This must remain the only active Cobalt plan until completion.
