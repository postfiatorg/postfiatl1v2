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

The decisive implementation run now passes. The independent frozen oracle has no production Cobalt dependency, every one of its 18 per-node cases has a terminal expectation, production Cobalt matches all 18 with zero conflicting roots, and every applicable decision-critical replay is identical. All three 20-validator 90%-overlap cases now resolve at the specified support boundary.

The matched RippleD 3.1.3 validator-governance adapter admits two conflicting registry roots in `six-divergent-local-quorums`; Cobalt rejects that unsafe trust graph before commitment. Native RippleD CSF ledger consensus remains separately labeled and synchronized in the control.

The live admission gap is also fixed: a five-of-six signature set no longer qualifies by itself as a Cobalt decision. Cobalt authority now requires a validator-key-bound RBC -> ABBA -> MVBA -> DABC certificate over the exact registry update, current chain domain, registry root, and trust graph. The authority certificate stores shared DABC checks once, deterministically retains the smallest signer set that has strong support in every trust view, requires that signer set at every protocol stage, uses bounded canonical compression, and is capped at the 1 MiB consensus-batch limit. A 20-validator regression produces a 938,032-byte certificate with the required 14 signers. The exact live release produces a 329,883-byte canonical certified update in the six-validator disposable-clone rehearsal and passes all 15 verifier gates without changing live state.

The operator-admission boundary now verifies the signed evidence files, not only hashes named in manifests. Provider-account, host-control, and key-custody attestations use the same ML-DSA master signer and onboarding challenge as the custody-bound v2 manifest. Missing, tampered, cross-operator-shared, or mismatched evidence is rejected. The refreshed operator release is source `3b01c2ad57fb0ce1c29e12edc88aece5b22548ae`, binary SHA-256 `e036033d437d85c4f60fc8e6689a771fdda01dd2ce88456571e6c9092faf4caf`.

The pinned binary and onboarding packet are published in GitHub release `cobalt-operator-v2-20260824`; a clean download verifies against its checksum manifest. Operator recruitment is tracked in GitHub issue `#36`. The full locked workspace test run passed after the receipt-boundary change, including 292 node-library tests, 115 node-binary tests, the long AssetOrchard proof cases, all remaining crates, and doc tests, with zero failures.

The remaining work is operational:

- the six current live machines still share one Post Fiat operator and provider;
- independent live operators have not completed the required transition and fault exercises; and
- Foundation authority remains active until the governed future-height Cobalt cutover.

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

### 3. Independent live-validator proof

- Operator establishment task: `[ ] task_46d1707cb9e11f04648ea54a7163fbee — accepted`
- Live transition/fault task: `[ ] request only after the independent topology verifier passes`

- [x] Refresh the provider and live-service inventory: all six validators and sidecars are active with zero restarts and one common chain/registry/trust-graph state, but remain under the same Vultr account and Post Fiat administrator.
- [x] Give each operator a redaction-safe local key-generation command; private ML-DSA master and validator keys stay in separate mode-0600 files while stdout contains public material only. Code: [`operator-onboarding-keygen`](../../../crates/node/src/main_parts/cli_dispatch_parts/group_04.rs).
- [x] Freeze the six-slot self-custody onboarding contract, exact release hash, unique challenges, current trust views, signed-receipt contract, and static verifier. Packet: [`benchmarks/cobalt-independent-operators`](../../../benchmarks/cobalt-independent-operators), `SHA256SUMS.txt` root `c8e6e40d98d40ab72dfbc11c665f092bd0c34a0035d9cc0fdb1c5ab505860786`.
- [x] Publish the pinned binary, checksum manifest, and onboarding packet as GitHub release `cobalt-operator-v2-20260824`, verify clean downloads, and open operator recruitment issue `#36`.
- [x] Bind each operator's custody-bound v2 manifest to the Cobalt trust view, Section 2 packet root, source commit, release-binary SHA-256, onboarding challenge, infrastructure-account fingerprint, host administrator, and ML-DSA custody evidence. Code: [`consensus_artifacts.rs`](../../../crates/node/src/consensus_artifacts.rs), [`operator-manifest-create`](../../../crates/node/src/main_parts/cli_dispatch_parts/group_04.rs).
- [x] Add structured ML-DSA provider, host-control, and custody attestations; verify the actual files against their signed manifest bindings and reject missing, tampered, mismatched, or private-material-bearing receipts. Code: [`operator_attestations.rs`](../../../crates/node/src/operator_attestations.rs), commit `3b01c2ad57fb0ce1c29e12edc88aece5b22548ae`.
- [x] Add an aggregate verifier that rejects mixed trust graphs, shared cross-operator control fingerprints, insufficient infrastructure domains, any operator that can reach quorum alone, and any operator withdrawal that can halt quorum. Code: [`verify_operator_independence`](../../../crates/node/src/governance.rs).
- [ ] Establish at least three operationally independent operator groups with separate host administration and signing-key custody, across at least three infrastructure domains.
- [ ] Pass the aggregate topology verifier. With the current six-validator/quorum-five topology this requires six one-validator operator groups; otherwise expand the validator set while preserving the no-single-operator-halt rule.
- [ ] Deploy non-identical but formally compatible trust views.
- [ ] Execute one validator addition, one removal, one ML-DSA key rotation, one compatible trust-view transition, one-validator outage, lagging-node catch-up, and one incompatible safe halt.
- [ ] Prove Consensus v2 continues finalizing, p95 finality stays within 5% of the frozen same-fleet baseline, and no recovery mutates durable history by hand.
- [ ] Collect signed, redaction-safe operator receipts. Geographic distribution under one operator is not independent operation.

### 4. Live activation

Task Node: `[ ] request and accept after the activation gates pass`

- [x] Make a real Cobalt protocol decision certificate mandatory at the consensus admission boundary; validator signatures alone are insufficient.
- [x] Verify the authoritative handoff/update/rollback flow with all six current validator identities on disposable signer-state clones while proving the live fleet unchanged.
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

- [x] Zero conflicting Cobalt decisions.
- [x] Zero per-node outcome/root mismatches against the frozen corpus.
- [x] At least one compatible non-uniform decision and one incompatible safe halt.
- [x] Byte-identical decision-critical replay.
- [x] A fair, reproducible material safety distinction from RippleD local-UNL admission.
- [x] Cobalt-authorized registry admission requires and verifies the actual signed protocol decision over the exact update.
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
