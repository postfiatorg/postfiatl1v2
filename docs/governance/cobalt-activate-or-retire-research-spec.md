# Cobalt Activate-or-Retire Research Specification

**Status:** Draft; pending Text Improvement Harness score and Task Node lock  
**Date:** 2026-08-23  
**Decision owner:** Post Fiat  
**Prior work:** [Cobalt live-deployment research specification](cobalt-live-deployment-research-spec.md), [completed live-deployment milestone](../plans/completed/cobalt-live-deployment-and-liveness-milestone.md)

## Plain-English directive

Cobalt has one remaining job: prove that it should control validator-trust governance on the live Post Fiat controlled testnet.

This campaign has exactly two permitted outcomes:

- **ACTIVATE:** hand validator-registry and trust-graph authority to Cobalt on the controlled testnet because it demonstrates a material safety and decentralization advantage without damaging liveness.
- **RETIRE:** remove Cobalt from the production path because it cannot demonstrate that advantage reliably enough to justify its code, operational burden, and attack surface.

“Keep it in shadow mode,” “GO for a later cutover,” and “continue evaluating” are not terminal outcomes. The existing implementation has already earned a live decision, not another rehearsal.

## What Cobalt must earn

Post Fiat already has Consensus v2 for block finality. Cobalt is not competing to replace transaction consensus in this decision. It is competing to become the authority for validator-registry and trust-graph changes.

Cobalt deserves to exist only if it gives the network all four of these properties:

1. **Selective safety:** incompatible trust configurations cannot authorize conflicting validator-registry states.
2. **Useful liveness:** compatible but non-identical validator trust views can still authorize a change. Rejecting every heterogeneous view is not a safety achievement.
3. **Independent operation:** the protocol works across independently controlled validators rather than depending on one publisher, one operator, or one shared trust-list file.
4. **Operational isolation:** a Cobalt governance failure cannot stop Consensus v2 block production, and Cobalt recovery does not require rewriting durable history.

These properties matter to a high-value network because validator admission and removal must be credible to operators, markets, and integrators. If Cobalt cannot make validator governance materially safer and less publisher-dependent than the inherited RippleD model, then it is complexity without network value and must be retired.

## Existing evidence and unresolved defect

The completed milestone established that the current implementation can:

- run six live sidecars across EWR, AMS, and SGP;
- decide with five of six validators and reject four of six;
- produce the same decision identifier across valid signer subsets;
- recover signed history after a missed round;
- leave Consensus v2 finality operating through a sidecar outage;
- rehearse a future-height authority handoff, validator-key rotation, and forward rollback; and
- replay matched Cobalt and RippleD benchmark packets deterministically.

That work is necessary but insufficient for activation.

The strongest unresolved result is that the current Cobalt adapter halted in the 20-validator case with 90% trust-list overlap. A system that avoids conflict by rejecting compatible heterogeneity has not demonstrated the benefit claimed for Cobalt. The follow-on must determine whether that halt is:

- a benchmark-modeling error;
- an incorrect trust-graph or essential-subset configuration;
- an implementation error in strong-support or certificate validation; or
- the correct consequence of the formal model.

The answer must be proven against the code and a pre-registered oracle. It cannot be chosen after observing the run.

## Decision question

> Under one shared validator-governance threat model, does Post Fiat’s Cobalt implementation accept every tested compatible non-uniform trust configuration, reject every tested incompatible configuration, prevent conflicting validator-registry decisions where a RippleD-style local-UNL model does not, and operate safely across live independent validators?

If yes, activate it on the controlled testnet. If no after the bounded remediation allowed below, retire it.

## Scope

### In scope

- Cobalt authority over validator-registry and trust-graph transitions.
- The live controlled-testnet validator fleet.
- Compatible and incompatible non-uniform trust views.
- Divergent proposals, message delay, validator outage, recovery, replay, and key rotation.
- A matched comparison with RippleD’s local-UNL acceptance model.
- The existing handoff and forward-rollback mechanism.
- CLI and user-facing reporting of the final authority state.

### Out of scope

- Replacing Consensus v2 block finality.
- Repairing the RippleD defects catalogued in the earlier fork-inheritance audit.
- Mainnet activation.
- Claims that deterministic replay alone proves consensus safety.
- Indefinite shadow operation after this decision.

## Shared property under test

For fixed validator identities, per-validator trust views, candidate registry roots, Byzantine behavior, and message schedule, each adapter must report the result for every correct node:

1. **Agreement:** no two correct nodes accept different registry roots for the same transition.
2. **Validity:** a correct node accepts only a proposal authorized by its formal trust conditions.
3. **Liveness:** every correct node decides when the shared oracle classifies the configuration as compatible and the stated synchrony assumptions hold.
4. **Safe halt:** no correct node decides when the shared oracle classifies the configuration as incompatible.
5. **Recovery:** restarted or lagging nodes recover the same signed history without operator-written state repair.

The benchmark must report node-level votes, decisions, registry roots, timing, and rejection reasons. A single aggregate branch count is not enough.

## Oracle and frozen corpus

The existing benchmark uses `model_scope: characterize` for the asymmetric-view, list-drift, and overlap cases. In [`postfiat_cobalt_benchmark.rs`](../../crates/node/src/bin/postfiat_cobalt_benchmark.rs), that path does not require a substantive predicted decision. It therefore measures determinism and regression behavior, not independent correctness.

Before any decisive execution:

1. Replace every decisive `characterize` case with a pre-registered expected node-level result.
2. Derive the Cobalt expectation from the formal essential-subset, strong-support, and linkage rules implemented in:
   - [`trust_graph_governance.rs`](../../crates/consensus_cobalt/src/trust_graph_governance.rs), especially `has_strong_support`, `analyze_trust_graph`, and non-uniform certificate validation;
   - [`cobalt_cover_extractor.rs`](../../crates/consensus_cobalt/src/cobalt_cover_extractor.rs); and
   - [`rbc_abba_mvba.rs`](../../crates/consensus_cobalt/src/rbc_abba_mvba.rs).
3. Derive the RippleD expectation from each node’s local UNL and quorum rule, not from the expected Cobalt result.
4. Freeze the scenario manifest, oracle output, source revisions, and SHA-256 root before running either adapter.
5. Make the packet verifier fail if execution inputs, expected results, or adapter revisions change after the freeze.

The oracle must classify configurations, not implementations. If an adapter disagrees with the frozen oracle, the adapter failed.

## Experiment 1 — prove Cobalt is selective

Build a boundary corpus around the actual trust graph, not only three parameterized scenario families.

The corpus must include:

- identical trust views as controls;
- non-identical views with formal linkage that must decide;
- non-identical views immediately inside and outside each linkage boundary;
- the existing 90% overlap case;
- trust-list drift during a future-height transition;
- one and two unavailable validators;
- one equivocating validator;
- delayed, duplicated, reordered, stale, and replayed messages;
- two simultaneously proposed registry roots;
- validator addition, removal, and ML-DSA key rotation; and
- recovery by a node that missed the decision.

At least one compatible case must use genuinely different essential subsets rather than merely different validator ordering. At least one incompatible case must be locally quorate from the perspective of two disjoint or insufficiently linked groups.

### Required result

- Every compatible case decides one registry root at every correct node.
- Every incompatible case safely halts.
- No case produces conflicting accepted roots.
- The 90% overlap case must decide if the frozen formal oracle classifies it as compatible. If the oracle classifies it as incompatible, the packet must identify the exact failed linkage inequality and explain why the earlier “90% overlap” label was misleading.

A universal halt, a universal accept, or a post-hoc reclassification fails this experiment.

## Experiment 2 — demonstrate a real delta from RippleD

The comparison must test the same governance decision object and fault schedule. It must not imply that a Cobalt registry certificate and RippleD ledger consensus are identical protocols.

Extend the existing generator and adapters:

- [`generate_scenarios.py`](../../benchmarks/cobalt-rippled-liveness/generate_scenarios.py) must emit frozen per-node trust views, candidate registry roots, event schedules, and expected per-node results.
- [`postfiat_cobalt_benchmark.rs`](../../crates/node/src/bin/postfiat_cobalt_benchmark.rs) must expose every correct node’s Cobalt decision and remove the permissive `characterize` pass path from the decisive corpus.
- [`MatchedLivenessBenchmark_test.cpp`](../../benchmarks/cobalt-rippled-liveness/rippled/MatchedLivenessBenchmark_test.cpp) must expose every simulated RippleD node’s local-quorum decision for the same inputs.
- [`aggregate_packet.py`](../../benchmarks/cobalt-rippled-liveness/aggregate_packet.py) must calculate false accepts, false halts, conflicts, and decision latency for each model.

The comparison succeeds only if it contains:

1. at least one pre-registered compatible non-uniform case where Cobalt decides;
2. at least one pre-registered incompatible case where Cobalt halts;
3. at least one case where the RippleD-style local-UNL model admits incompatible concurrent decisions or cannot detect the unsafe trust transition while Cobalt rejects it; and
4. no case where Cobalt accepts conflicting registry roots.

If the experiment cannot demonstrate item 3 under a fair shared threat model, the claimed differentiation has not been established. Deterministic replays and different policy labels do not substitute for it.

## Experiment 3 — fix or vindicate the 90% halt

Trace the 90% case through:

- essential-subset validation;
- `has_strong_support`;
- `analyze_trust_graph`;
- cover extraction;
- non-uniform certificate construction; and
- local certificate validation at every validator.

Produce a compact trace containing the trust view, satisfied subsets, failed inequalities, support set, linkage report, certificate, and per-validator verdict.

If the oracle says the case is compatible and the implementation halts, fix the implementation. If the oracle says it is incompatible, add a nearby non-identical configuration that is formally compatible and must decide. Cobalt cannot pass without a positive heterogeneous case.

## Experiment 4 — prove independent live operation

The present six-node deployment demonstrates geographic distribution, not independent governance. Activation requires separately controlled validator operators and separately held authorization keys.

Use the live controlled testnet with:

- at least three operationally independent operator groups;
- no shared host administration or validator signing key between groups;
- no operator group able by itself to reach the decision quorum;
- no operator group controlling enough validators to halt governance by withdrawing all of its validators;
- at least three providers or independently administered infrastructure domains; and
- non-identical but formally compatible trust views.

If the current five-of-six policy makes the no-single-group blocking condition impossible, change the operator allocation or formally justified essential-subset configuration before activation. Do not call Foundation-owned machines “independent validators.”

Run, on the live fleet:

1. one Cobalt-authorized validator addition;
2. one validator removal;
3. one ML-DSA validator-key rotation;
4. one compatible trust-view transition;
5. one validator outage during a decision;
6. one lagging-node signed-history recovery; and
7. one deliberately incompatible transition that must halt without affecting block finality.

Consensus v2 must continue finalizing throughout. No result may require editing durable history by hand.

## Experiment 5 — perform the cutover, not another rehearsal

After Experiments 1–4 pass, use the existing authority transition code in [`cobalt_handoff.rs`](../../crates/node/src/cobalt_handoff.rs) and the operational flow exercised by [`cobalt_handoff_rehearsal.rs`](../../crates/node/src/cobalt_handoff_rehearsal.rs) to schedule a future-height controlled-testnet transition.

The live cutover must:

1. bind the current governance state, registry root, trust-graph root, activation height, sequence, and protocol version;
2. collect valid approvals without placing production private keys in the evidence packet;
3. activate Cobalt authority at the scheduled height;
4. execute one real validator-registry change under Cobalt authority;
5. show the active Cobalt authority and transition history in the Python CLI and user-facing interface;
6. demonstrate that an early, stale, replayed, wrong-root, mixed-authority, or self-authorized update is rejected; and
7. keep a signed forward rollback to Foundation authority ready and verify it on a disposable clone before the live cutover.

This is the activation event. A readiness packet that says another activation could happen later does not pass.

## Performance and operations

Cobalt is outside the block-finality hot path, but it still consumes validator resources and creates an operational dependency.

The decisive packet must report:

- governance decision latency without faults and with one unavailable validator;
- recovery time for a validator that missed a round;
- CPU, memory, network, and disk overhead per validator;
- Consensus v2 p50 and p95 finality before, during, and after governance operations;
- sidecar restarts, malformed-message rejections, and history mismatches; and
- operator actions required for recovery.

Activation fails if:

- any Cobalt event stops or rewrites Consensus v2 finality;
- p95 Consensus v2 finality regresses by more than 5% against the frozen same-fleet baseline;
- a normal one-validator outage prevents a compatible governance decision;
- recovery requires manual mutation of durable history; or
- any correct validator reaches a different authority, registry root, or decision history.

## Binary decision gates

### ACTIVATE

The final decision is **ACTIVATE** only when all of the following are true:

- the frozen corpus reports zero conflicting decisions;
- compatible cases have zero false halts;
- incompatible cases have zero false accepts;
- every replay is byte-identical for decision-critical artifacts;
- Cobalt demonstrates at least one material safety distinction from the RippleD local-UNL model under the shared threat model;
- independent live operators complete the addition, removal, rotation, trust transition, outage, recovery, and safe-halt exercises;
- Consensus v2 stays live and within the performance budget;
- the live future-height handoff succeeds;
- a real validator-registry change is authorized under active Cobalt authority;
- the CLI and user-facing interface report the same active authority and signed history; and
- the forward rollback path remains valid.

When these gates pass, leave Cobalt active on the controlled testnet and publish the evidence and operator instructions.

### RETIRE

The final decision is **RETIRE** if any of the following remains true after the bounded remediation allowance:

- a correct node accepts an unsafe or conflicting registry root;
- a formally compatible non-uniform case halts;
- the implementation cannot explain and resolve the 90% overlap result;
- no material safety distinction from the RippleD local-UNL model can be reproduced fairly;
- live operation depends on one publisher, one operator, or shared validator keys;
- Cobalt disrupts Consensus v2 or exceeds the performance budget;
- live handoff, real Cobalt-authorized update, recovery, or forward rollback fails; or
- the claimed result depends on post-hoc expected outcomes.

On RETIRE:

1. keep Foundation governance authoritative;
2. disable and remove Cobalt sidecar services from the validator fleet;
3. remove Cobalt activation controls and readiness claims from the CLI, UI, operator documentation, and public positioning;
4. remove the Cobalt authority path from production builds, or isolate code needed for historical replay behind a non-production research feature after a compatibility review;
5. preserve only the minimum data types and verification logic required to read existing history safely;
6. archive one compact evidence packet and a plain-English retirement record; and
7. delete obsolete active Cobalt plans and shadow-operation instructions.

Cobalt must not survive as a permanently running experiment.

## Bounded remediation

The team gets one implementation-remediation cycle after the first frozen-corpus execution.

The oracle and decisive scenarios remain frozen. A remediation may change implementation code, configuration generation, or an adapter defect, but it must record the exact cause and diff. The entire corpus and live pre-cutover exercise are then rerun from clean state.

If a second execution still triggers a RETIRE condition, the decision is RETIRE. A new research program would require a new user mandate; this specification cannot extend itself.

## Required evidence packet

The final packet must contain:

- `decision.json` with exactly `ACTIVATE` or `RETIRE`;
- the frozen scenario manifest and oracle;
- source commit identifiers for Post Fiat and RippleD;
- per-node Cobalt and RippleD results;
- false-accept, false-halt, conflict, and latency summaries;
- the 90% overlap trace;
- signed live operator receipts with secrets excluded;
- Consensus v2 continuity and resource metrics;
- authority handoff, validator update, recovery, and rollback receipts;
- CLI and UI output;
- `SHA256SUMS.txt`; and
- a verifier that fails on missing, mutated, or internally inconsistent evidence.

Self-declared expected outcomes, screenshots without machine-readable receipts, and operator assertions without signatures are not evidence.

## Required publication

The research output must be understandable without reading source code. Its first page must say:

- what Cobalt controls;
- what unique benefit it was required to prove;
- what happened in the compatible and incompatible trust cases;
- what happened against the RippleD local-UNL model;
- whether real independent validators completed the live exercise; and
- the final word: **ACTIVATE** or **RETIRE**.

The conclusion may describe limitations, but it may not weaken the binary decision with “perhaps,” “later,” “candidate,” or “continued shadow evaluation.”

## Work sequence

1. Lock this research specification through the Task Node.
2. Freeze the oracle, corpus, baselines, and source revisions.
3. Run the first decisive corpus.
4. Use the single remediation cycle if required.
5. Run the decisive corpus again if remediated.
6. Complete the independent live pre-cutover exercise.
7. Apply the binary gates.
8. If ACTIVATE, perform the live controlled-testnet cutover and real registry change.
9. If RETIRE, execute the removal list.
10. Publish the compact decision packet and update the active/completed plan state.

No implementation milestone may redefine the decision gates without a newly locked research specification.
