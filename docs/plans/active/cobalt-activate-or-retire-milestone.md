# Cobalt Activation Milestone

## Plain-English outcome

This milestone fixes Cobalt, proves its liveness capability against the failure demonstrated in the original Cobalt evaluation, and activates it as the live authority for validator-registry and trust-graph changes on the controlled testnet. Benchmark failures are P0 implementation work, not grounds to abandon the feature. Shadow-only operation and open-ended evaluation are not completion states.

Cobalt does not replace Consensus v2. It controls validator-trust governance only.

This is a protocol-capability milestone, not an operator-recruitment milestone. "Independent validators" in this plan means isolated simulated validator domains with distinct identities, keys, trust views, message schedules, failures, durable state, and recovery paths. It does not mean independent human operators and does not require third parties, separate provider accounts, or external key custodians. The original Cobalt article did not establish this liveness capability; this simulation must close that evaluation gap by proving five-of-six progress, deterministic recovery, and consistent durable history under isolated-validator faults. Simulation results must be labeled as simulation evidence and must not be presented as proof that the controlled testnet is operationally decentralized.

- **Status:** Active
- **Specification:** [Cobalt Activation Research Specification](../../governance/cobalt-activate-or-retire-research-spec.md)
- **Specification lock task:** `task_a8ffc2885a6fbab8aa06c4b20e92f6b8` — rewarded 2026-08-23
- **Milestone-document task:** `task_18b8d92d981221b88d0a38159ea1fd26` — accepted
- **Required result:** `[ ] ACTIVATE`

## Current position

The decisive implementation run now passes. The independent frozen oracle has no production Cobalt dependency, every one of its 18 per-node cases has a terminal expectation, production Cobalt matches all 18 with zero conflicting roots, and every applicable decision-critical replay is identical. All three 20-validator 90%-overlap cases now resolve at the specified support boundary.

The matched RippleD 3.1.3 validator-governance adapter admits two conflicting registry roots in `six-divergent-local-quorums`; Cobalt rejects that unsafe trust graph before commitment. Native RippleD CSF ledger consensus remains separately labeled and synchronized in the control.

The live admission gap is also fixed: a five-of-six signature set no longer qualifies by itself as a Cobalt decision. Cobalt authority now requires a validator-key-bound RBC -> ABBA -> MVBA -> DABC certificate over the exact registry update, current chain domain, registry root, and trust graph. The authority certificate stores shared DABC checks once, deterministically retains the smallest signer set that has strong support in every trust view, requires that signer set at every protocol stage, uses bounded canonical compression, and is capped at the 1 MiB consensus-batch limit. A 20-validator regression produces a 938,032-byte certificate with the required 14 signers. The exact live release produces a 329,883-byte canonical certified update in the six-validator disposable-clone rehearsal and passes all 15 verifier gates without changing live state.

The operator-admission boundary and published onboarding packet remain usable for a later real decentralization program, but they are not activation gates for this controlled-testnet milestone. The full locked workspace test run passed after the receipt-boundary change, including 292 node-library tests, 115 node-binary tests, the long AssetOrchard proof cases, all remaining crates, and doc tests, with zero failures.

The isolated-validator capability simulation now passes against the production Cobalt decision and recovery code: six isolated validator domains, five-of-six progress, four-of-six safe halt, proof-carrying catch-up, byte-identical recovered history, all required fault schedules, and all four validator/trust transitions. The packet explicitly makes no independent-operator or operational-decentralization claim.

The remaining work is operational:

- measure Consensus v2 finality in a paired same-fleet baseline/integration run;
- complete the release-lineage verification; and
- keep Foundation authority active until the governed future-height Cobalt cutover.

## Completed foundation

- [x] Run authenticated Cobalt shadow services on the six-validator WAN fleet. Evidence: completed [live-deployment milestone](../completed/cobalt-live-deployment-and-liveness-milestone.md).
- [x] Prove five-of-six progress, four-of-six rejection, canonical decision identity, signed catch-up, restart recovery, and no manual history repair. Code: [`cobalt_shadow.rs`](../../../crates/node/src/cobalt_shadow.rs), [`cobalt_shadow_runtime.rs`](../../../crates/node/src/cobalt_shadow_runtime.rs).
- [x] Keep Consensus v2 finality live during Cobalt outages and recovery. Existing live evidence finalized heights 913→914 and 914→915.
- [x] Build the initial 80-case Cobalt/RippleD packet. Packet: [`benchmarks/cobalt-rippled-liveness/packet`](../../../benchmarks/cobalt-rippled-liveness/packet); `SHA256SUMS` root `7968a085033419255b52b844edd586346a1e85561394e52c69e6683b2561c50b`.
- [x] Rehearse future-height activation, scoped validator-key rotation, negative cases, and forward rollback on a disposable clone. Code: [`cobalt_handoff.rs`](../../../crates/node/src/cobalt_handoff.rs), [`cobalt_handoff_rehearsal.rs`](../../../crates/node/src/cobalt_handoff_rehearsal.rs); initial packet root `b678b3f45eb2a14299b941101bd556d61795a1033f1f6e53557442b7e315807e`.
- [x] Require the actual signed RBC -> ABBA -> MVBA -> DABC decision at Cobalt-authorized registry admission; reject quorum-only, tampered, replayed, wrong-root, and cross-chain certificates. Code: [`cobalt_authority_certificate.rs`](../../../crates/node/src/cobalt_authority_certificate.rs), commit `34540c540545d582c663d59aa05de452a9485a04`.
- [x] Re-run the handoff with disposable copies of all six live validator signers and bind the exact update payload to six signed contributions and six full-knowledge checkpoints. Initial authoritative packet: [`packet-authoritative-v2`](../../../benchmarks/cobalt-handoff-rehearsal/packet-authoritative-v2), `SHA256SUMS` root `f056c891e034238c5523bfcaf6ab2e022884ea35d6569c9b501dca4fae388968`.
- [x] Replace quadratic DABC proof duplication with one shared support certificate, canonical quorum-minimal signers across every protocol stage, bounded deterministic compression, and a 1 MiB complete-certificate cap; verify through the real live signed-governance-batch path. The 20-validator ML-DSA regression reduces a 3,967,265-byte expanded transcript to a 938,032-byte certificate with the required 14 signers. The six-live-validator certificate is 319,060 bytes. Code commits: `cf5a89334aee8f53a74eab4201b924b9d24bc674`, `2a9d449bbde3b09c62603c800732ba1f89ba4cde`. Current packet: [`packet-authoritative-v4`](../../../benchmarks/cobalt-handoff-rehearsal/packet-authoritative-v4), `SHA256SUMS` root `a17b2a2206e30845b2554f24c89ee65d9ca87e62aad92586e5a6419939cfc9a8`. The v3 packet is superseded.
- [x] Use the same canonical compact transcript for sidecar commit RPC instead of sending the expanded transcript. The 20-validator request is 590,087 bytes, below the 2 MiB RPC frame, and the socket drill commits and rejects a tampered signature through the compressed route. Code: [`cobalt_shadow_runtime.rs`](../../../crates/node/src/cobalt_shadow_runtime.rs), [`postfiat_cobalt_shadow.rs`](../../../crates/node/src/bin/postfiat_cobalt_shadow.rs).
- [x] Roll source `bafc23fc424fdd78364928999038193b22d180db` to all six live advisory sidecars as binary `bd679b676c6be496635a5ffd02158cd772632a38da55f172734ebef4e768c6b4`, one node at a time. Every Consensus v2 PID, restart count, and binary hash remained unchanged; Cobalt authority and block control remained false.
- [x] Re-run the full six-live-validator signed handoff/update/negative/forward-rollback rehearsal against that exact sidecar release. All 15 verifier checks pass; the canonical certified update is 329,883 bytes; the live fleet is identical before and after. Current packet: [`packet-authoritative-v5`](../../../benchmarks/cobalt-handoff-rehearsal/packet-authoritative-v5), `SHA256SUMS` root `2858940ef188df9770584aaa5e8942f8a728da4b6c1a80775a9a0a1d6acec9df`. The v4 packet remains the evidence for source `2a9d449b`; v5 is authoritative for the deployed `bafc23fc` release.
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

Task Node: `[x] task_690f0c63d1c0d175a4e47d947825402b — rewarded 2026-08-24 (2.5 PFT)`

- [x] Run the frozen corpus from clean state through both adapters.
- [x] Report false accepts, false halts, conflicts, per-node results, decision latency, and byte-identical replay; no post-hoc reclassification.
- [x] Trace the 90%-overlap result through essential subsets, linkage inequalities, cover extraction, certificate construction, and local validation.
- [x] Require at least one compatible non-uniform Cobalt decision, one incompatible Cobalt halt, and one fair material safety distinction from RippleD local-UNL admission.
- [x] Fix the owning non-uniform support boundary, add regression coverage, and rerun the unchanged oracle and corpus. Code commit: `01822ecc53ad1cdab50e6c55536fcc7b81aba02a`.
- [x] Complete the corpus with zero per-node mismatches, zero Cobalt conflicts, resolved 90% support boundaries, deterministic replay, and the RippleD distinction. Evidence: [`section2-packet`](../../../benchmarks/cobalt-activate-or-retire/section2-packet), `SHA256SUMS.txt` root `40bc86c9416a1b468f5625a2ff83724c9268f9d49c41007e9b0c4bc70c43c1e1`.

### 3. Independent-validator liveness simulation

- Superseded operator-establishment task: `[x] task_46d1707cb9e11f04648ea54a7163fbee — cancelled because real external operators are outside this milestone`
- Simulation task: `[x] task_043e009b196aea0b685b3f09a6ebb45d — accepted; execution packet passes, Task Node verification pending`

Prior operator-onboarding artifacts remain available for a future decentralization program, but recruitment, external custody, separate provider accounts, and real third-party operation are not milestone work or activation gates here.

- [x] Instantiate at least six isolated simulated validator domains. Each has a distinct validator identity, Cobalt and validator ML-DSA key material, durable state directory, local trust view, loopback transport endpoint, message schedule, and fault-control channel. The harness keeps each validator's identity, state, and controls isolated; it does not require separate human operators.
- [x] Generate non-identical but formally compatible trust views and pass the same production topology and certificate validation paths used by Cobalt. No simulation-only acceptance rule is used.
- [x] Execute one validator addition, one removal, one ML-DSA key rotation, one compatible trust-view transition, every one-validator outage, lagging-validator proof-carrying catch-up, and an incompatible four-of-six safe halt.
- [x] Reproduce the original Cobalt liveness-failure contract and demonstrate five-of-six progress plus gap refusal and exact durable-history recovery under the repaired implementation without conflicting roots.
- [x] Run deterministic delay, loss, reorder, duplicate, stale-replay, equivocation, crash/restart, and partition/healing schedules against the isolated simulated validator domains.
- [x] Compare the same proposed validator-governance decisions with the pinned RippleD 3.1.3 local-UNL adapter, keeping native RippleD ledger consensus separately labeled.
- [ ] Prove Consensus v2 continues finalizing in the controlled integration run, p95 finality stays within 5% of the frozen same-fleet baseline, and no Cobalt recovery mutates durable history by hand.
- [x] Produce signed-protocol, redaction-safe simulation receipts and a verifier-backed packet that identifies every result as simulated-validator evidence. Evidence: [`section3-packet`](../../../benchmarks/cobalt-activate-or-retire/section3-packet), `SHA256SUMS.txt` root `54bc5ebb445994dbc1c6c8c04ea8a9ce054af8438f04fd93554e776ac21a5c4c`. The packet makes no real provider, administrator, custody, geographic-independence, or decentralization claim.

### 4. Live activation

Task Node: `[ ] request and accept after the activation gates pass`

- [x] Make a real Cobalt protocol decision certificate mandatory at the consensus admission boundary; validator signatures alone are insufficient.
- [x] Verify the authoritative handoff/update/rollback flow with all six current validator identities on disposable signer-state clones while proving the live fleet unchanged.
- [ ] Pass the independent-validator liveness simulation and bind its manifest, production source hash, scenario results, and verifier output into the activation packet.
- [ ] Verify a signed forward rollback on a disposable clone immediately before cutover.
- [ ] Schedule and execute the future-height live controlled-testnet authority transition through [`cobalt_handoff.rs`](../../../crates/node/src/cobalt_handoff.rs).
- [ ] Execute one real validator-registry change under active Cobalt authority.
- [ ] Reject early, stale, replayed, wrong-root, mixed-authority, and self-authorized updates without mutation.
- [ ] Keep Cobalt active and Foundation validator-trust authority inactive after all gates pass.


### 5. Human interfaces, packet, and documentation

Governed inside the terminal-operation task; do not request microtasks.

- [ ] Make the Python CLI display the actual terminal decision, authority, registry root, trust-graph root, transition history, and verifier result.
- [ ] Make the browser interface consume the same authenticated output and display the activation state without readiness language.
- [ ] Produce one compact verifier-backed packet containing `activation-status.json`, frozen oracle and manifest, source pins, per-node results, KPI summaries, 90% trace, signed simulation receipts, controlled-testnet cutover receipts, finality/resource metrics, authority/update/rollback records, CLI/UI output, and `SHA256SUMS.txt`.
- [ ] Publish a concise first-page explanation of what Cobalt controls, the unique benefit tested, compatible/incompatible results, RippleD comparison, simulated-independent-validator result, and the controlled-testnet activation result. State plainly that the result proves protocol capability, not real operator decentralization.
- [ ] Refresh [`README.md`](../../../README.md), [`STATUS.md`](../../../STATUS.md), architecture/governance docs, CLI help, and operator instructions to match the live result.

## Activation completion gate

### Activate only when every item passes

- [x] Zero conflicting Cobalt decisions.
- [x] Zero per-node outcome/root mismatches against the frozen corpus.
- [x] At least one compatible non-uniform decision and one incompatible safe halt.
- [x] Byte-identical decision-critical replay.
- [x] A fair, reproducible material safety distinction from RippleD local-UNL admission.
- [x] Cobalt-authorized registry admission requires and verifies the actual signed protocol decision over the exact update.
- [x] The isolated independent-validator simulation completes every liveness, fault, transition, recovery, determinism, and RippleD-comparison exercise without conflicting Cobalt roots.
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
