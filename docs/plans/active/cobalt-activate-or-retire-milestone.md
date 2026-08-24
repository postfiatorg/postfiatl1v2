# Cobalt Activation Milestone

## Plain-English outcome

This milestone fixes Cobalt and activates it as the live authority for validator-registry and trust-graph changes on the controlled testnet. Benchmark failures are P0 implementation work, not grounds to abandon the feature. Shadow-only operation and open-ended evaluation are not completion states.

Cobalt does not replace Consensus v2. It controls validator-trust governance only.

- **Status:** Active
- **Specification:** [Cobalt Activation Research Specification](../../governance/cobalt-activate-or-retire-research-spec.md)
- **Specification lock task:** `task_a8ffc2885a6fbab8aa06c4b20e92f6b8` — rewarded 2026-08-23
- **Milestone-document task:** `task_18b8d92d981221b88d0a38159ea1fd26` — accepted
- **Required result:** `[ ] ACTIVATE`

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

Task Node: `[x] task_fd8342b57a6364f93953934c776080fc — rewarded 2026-08-24 (2.7 PFT)`

- [x] Write a versioned mathematical decision contract that defines compatible and incompatible trust configurations without importing or calling production Cobalt code. Evidence: [`oracle-contract.md`](../../../benchmarks/cobalt-activate-or-retire/oracle-contract.md).
- [x] Implement the oracle in a separate benchmark crate with hand-worked boundary fixtures and no production protocol dependency. Code: [`cobalt_decision_oracle`](../../../crates/cobalt_decision_oracle).
- [x] Replace permissive decisive expectations with a new frozen manifest containing a per-node prediction for every correct validator. The historical 80-case packet remains unchanged; the decisive generator is [`generate_inputs.py`](../../../benchmarks/cobalt-activate-or-retire/generate_inputs.py).
- [x] Cover positive non-identical essential subsets, linkage and support boundaries, 90% overlap, divergent roots, outages, equivocation, message schedules, membership changes, rotation, and missed-history recovery.
- [x] Add a production Cobalt adapter that reports every correct node’s decision and root plus certificate, timing, replay, authority, and rejection evidence. Code: [`postfiat_cobalt_decisive_benchmark.rs`](../../../crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs).
- [x] Add a pinned RippleD 3.1.3 adapter for the same proposed registry decision and each node’s local-UNL result, with native CSF ledger consensus separately labeled. Code: [`DecisiveGovernanceBenchmark_test.cpp`](../../../benchmarks/cobalt-activate-or-retire/rippled/DecisiveGovernanceBenchmark_test.cpp).
- [x] Freeze the oracle, 18 scenarios, source pins, both adapter hashes, and expected per-node results before execution. Manifest: [`scenario-manifest.json`](../../../benchmarks/cobalt-activate-or-retire/scenario-manifest.json), canonical ID `78fc3f92d460f45a4941d40ef705af6c761e3782155a5b599dbd78c90396bde3`.

### 2. Decisive run and implementation remediation

Task Node: `[ ] request and accept a remediation-until-pass task`

- [ ] Run the frozen corpus from clean state through both adapters.
- [ ] Report false accepts, false halts, conflicts, per-node results, decision latency, and byte-identical replay; no post-hoc reclassification.
- [ ] Trace the 90%-overlap result through essential subsets, linkage inequalities, cover extraction, certificate construction, and local validation.
- [ ] Require at least one compatible non-uniform Cobalt decision, one incompatible Cobalt halt, and one fair material safety distinction from RippleD local-UNL admission.
- [ ] Treat every failed activation gate as P0 implementation work: fix the owning code, add regression coverage, and rerun the unchanged oracle and corpus.
- [ ] Continue remediation until the corpus has zero false accepts, zero false halts, zero conflicts, a resolved 90% result, deterministic replay, and the fair RippleD distinction.

### 3. Independent live-validator proof

Task Node: `[ ] request and accept only if Section 2 passes ACTIVATE gates`

- [ ] Establish at least three operationally independent operator groups with separate host administration and signing-key custody, across at least three infrastructure domains.
- [ ] Ensure no operator can reach quorum alone or halt governance by withdrawing all validators it controls.
- [ ] Deploy non-identical but formally compatible trust views.
- [ ] Execute one validator addition, one removal, one ML-DSA key rotation, one compatible trust-view transition, one-validator outage, lagging-node catch-up, and one incompatible safe halt.
- [ ] Prove Consensus v2 continues finalizing, p95 finality stays within 5% of the frozen same-fleet baseline, and no recovery mutates durable history by hand.
- [ ] Collect signed, redaction-safe operator receipts. Geographic distribution under one operator is not independent operation.

### 4. Live activation

Task Node: `[ ] request and accept after the activation gates pass`

- [ ] Verify a signed forward rollback on a disposable clone immediately before cutover.
- [ ] Schedule and execute the future-height live controlled-testnet authority transition through [`cobalt_handoff.rs`](../../../crates/node/src/cobalt_handoff.rs).
- [ ] Execute one real validator-registry change under active Cobalt authority.
- [ ] Reject early, stale, replayed, wrong-root, mixed-authority, and self-authorized updates without mutation.
- [ ] Keep Cobalt active and Foundation validator-trust authority inactive after all gates pass.


### 5. Human interfaces, packet, and documentation

Governed inside the terminal-operation task; do not request microtasks.

- [ ] Make the Python CLI display the actual terminal decision, authority, registry root, trust-graph root, transition history, and verifier result.
- [ ] Make the browser interface consume the same authenticated output and display the activation state without readiness language.
- [ ] Produce one compact verifier-backed packet containing `activation-status.json`, frozen oracle and manifest, source pins, per-node results, KPI summaries, 90% trace, signed live receipts, finality/resource metrics, authority/update/rollback records, CLI/UI output, and `SHA256SUMS.txt`.
- [ ] Publish a concise first-page explanation of what Cobalt controls, the unique benefit tested, compatible/incompatible results, RippleD comparison, independent-validator result, and the live activation result.
- [ ] Refresh [`README.md`](../../../README.md), [`STATUS.md`](../../../STATUS.md), architecture/governance docs, CLI help, and operator instructions to match the live result.

## Activation completion gate

### Activate only when every item passes

- [ ] Zero conflicting Cobalt decisions.
- [ ] Zero false halts in the frozen compatible corpus.
- [ ] Zero false accepts in the frozen incompatible corpus.
- [ ] Byte-identical decision-critical replay.
- [ ] A fair, reproducible material safety distinction from RippleD local-UNL admission.
- [ ] Independent operators complete every live exercise.
- [ ] Consensus v2 stays live and within the 5% p95 finality budget.
- [ ] Live future-height handoff, real Cobalt-authorized registry change, CLI/UI readback, and forward rollback readiness all pass.

Any unchecked item remains active P0 remediation work. It does not authorize retiring or deleting Cobalt.

## Completion

- [ ] Write `activation-status.json` with `ACTIVATED` and bind it to the live authority state.
- [ ] Run the packet verifier, relevant Rust tests, Python CLI/UI tests, formatting, strict Clippy, workspace checks/tests, documentation build, and redaction checks.
- [ ] Submit honest Task Node evidence and final verification for every accepted task.
- [ ] Confirm every Task Node task reaches rewarded state.
- [ ] Move this journal to `docs/plans/completed/` only after the CLI and browser interface work against the terminal live state and every selected path requirement is proven.
- [ ] Remove obsolete active Cobalt plans. This must remain the only active Cobalt plan until completion.
