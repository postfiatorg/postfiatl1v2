# Cobalt Adversarial Verification Milestone

## Plain-English outcome

This milestone tests the Cobalt authority that is already live on the controlled testnet. Cobalt stays active only when adversarial validators, trust views, schedules, history, authority transitions, and resource pressure cannot produce conflicting registry roots, false decisions, unsafe recovery, or an unacceptable Consensus v2 finality regression.

A failed benchmark or simulation creates P0 remediation and a rerun of the unchanged adversarial corpus. Rollback to Foundation authority is reserved for the published live stop conditions. Cobalt ratifies validator-registry and trust-graph changes; it does not choose which validators deserve trust, decentralize the proposal source, order blocks, or replace Consensus v2.

The result proves protocol capability, not operator decentralization.

- **Status:** Active — E1-E4 and E6 complete; E5 not started
- **Specification:** [Cobalt Adversarial Verification Research Specification](../../governance/cobalt-adversarial-verification-research-spec.md)
- **Historical specification-lock record:** `task_158622307482e23fb4519889b53b475f` — rewarded 2026-08-25
- **Historical milestone-document record:** `[x] task_d28eb3465dcac9a32524c25bba996e1e — rewarded 2026-08-25`
- **Required result:** `[ ] KEEP ACTIVE`

## Current position

Deployment, repository, campaign, and freshness identities are separated in
[Current State](../../status/chain-state-current.md). An authenticated read-only
six-validator/RPC probe ran from `2026-08-26T01:40:51Z` through
`2026-08-26T01:41:04Z`; it did not re-run the older authority/shadow audit.
Current repository HEAD is still not the active validator release.

- [x] Cobalt became the recorded controlled-testnet validator-trust authority at height 916; its first authorized validator-key rotation committed at height 917.
- [x] The research specification passed the Text Improvement Harness at 89.27/100 and is locked.
- [x] The public article now records the live authority state instead of saying Cobalt remains off.
- [x] E1-E4 and the design-only E6 decision have passed.
- [ ] The live E5 drills remain open, so the full adversarial campaign is not complete.
- [x] The current Foundation-administered proposal and authorization boundary remains explicit; no operator-decentralization result is claimed.

## Experiments

Historical Task Node identifiers are retained only where they already existed.
No Task Node interaction was used for E2 or E3 execution, verification, or completion.
Each experiment closes from committed, checksum-bound repository evidence.

### E1. Independent oracle and generated corpus

Historical Task Node record: `[x] task_59460b82c134e725fd1c902e2c3417b8 — rewarded 2026-08-25`

- [x] Build a second oracle from the formal essential-subset, strong-support, and linkage rules without importing production Cobalt or the first oracle.
- [x] Generate at least 10,000 trust graphs covering 6-20 validators and every linkage-inequality boundary.
- [x] Compare both oracles with production `analyze_trust_graph`, `has_strong_support`, and non-uniform certificate validation.
- [x] Freeze every mismatch before fixes; review any oracle correction; add regression cases for production defects; rerun the unchanged corpus from clean state.
- [x] Pass with complete oracle agreement and production agreement on every generated graph.

Initial comparison: 8,534 disagreements frozen before remediation; review found one harness-domain fixture defect and no oracle or production defect. Reconciled and clean-state passes agree on all 10,240 cases with identical corpus and classification hashes.

Code references: `crates/cobalt_adversarial_oracle/src/lib.rs`, `crates/cobalt_e1_harness/src/main.rs`, `crates/cobalt_decision_oracle/src/lib.rs`, `crates/consensus_cobalt/src/trust_graph_governance.rs`, `crates/node/src/cobalt_authority_certificate.rs`.

### E2. Byzantine validator campaign

Historical Task Node record: `task_91aebe5c632d90e03e7e151a6ffeb736`
was accepted before this run; it was not used for execution or completion.

Frozen source revision: `15ef2307732cf46ff3b921bf02f3ad096dda15f3`.
The first frozen run passed without remediation. The checksum-bound packet is
`benchmarks/cobalt-adversarial-verification/e2/`; its
`SHA256SUMS.txt` hash is
`8742d9603621408339d99c3d9fcc1ba8cc43dafdc900acdfccbf86cc60d7cba3`.

- [x] Derive and freeze the live six-validator fault bound `f=1` before execution from the pinned `n=6`, `q=5`, `t=1` topology and height-917 membership receipt.
- [x] Exercise up to `f` Byzantine domains across separate and combined RBC, ABBA, MVBA, and DABC equivocation; selective withholding; lying or changing trust views; competing proposals; late votes; and re-proposals.
- [x] Search delay, drop, reorder, duplicate, and partition schedules for maximum disagreement or delay instead of replaying a fixed schedule list.
- [x] Prove zero conflicting roots, zero false accepts, compatible progress within the 40-step synchrony bound, and incompatible safe halt without registry mutation.
- [x] Bind every Byzantine attribution to signed ML-DSA simulation evidence and revalidate it against the production signature paths.

Result: all 108 validator/strategy cases and 442,368 event schedules passed;
120 signed evidence pairs verified; conflicting roots, false accepts, false halts,
synchrony violations, and rejected-state mutations were all zero. The initial
and clean runs share classification SHA-256
`60ab419fc6cb165088c31e221a4d1a3247ad7e8d9fff9d9877bdf807b6590e93`.
The campaign was isolated and did not mutate the devnet.

Code references: `crates/cobalt_e2_harness/src/main.rs`,
`crates/consensus_cobalt/src/rbc_abba_mvba.rs`, and
`crates/node/src/cobalt_shadow.rs`.

### E3. Adversarial recovery

Execution: complete

Frozen source revision: `5c9e543ea0f56e7e6dda85d3a27093e810fdc111`.
Evidence commit: `2e63d6112de5ee7ef4d5ffdf82c4965b4f0956a8`.
The checksum-bound packet is
`benchmarks/cobalt-adversarial-verification/e3/`; its
`SHA256SUMS.txt` hash is
`9302b3555ab9091b2cae9b2d372d0548fe9f2fb1e67be43dfb3f63d89140b600`.

- [x] Test each validator in turn on a disposable clone bound to the live registry root.
- [x] Restart from truncated, padded, reordered, and one-entry-modified durable histories.
- [x] Reject fabricated transitions, wrong-root certificates, and catch-up histories that omit the latest update, each with a named reason.
- [x] Interrupt catch-up mid-transfer, resume from another peer, and reject inconsistent peer material before rejoin.
- [x] Restore byte-identical accepted history from honest peers without manual repair.

Result: all 24 durable-history tamper cases and 18 forged catch-up cases
rejected with named reasons and zero durable mutation. All six interrupted
recoveries resumed from a second honest peer, reached sequence 4 without manual
repair, and produced byte-identical accepted history. The initial and clean runs
share classification SHA-256
`ab53b5ddd5134e8fbbbb359b65c249ccbb1eb85a7ad034e496efa10bd85b90d3`.
The campaign was isolated on disposable clones. It was bound to the exact live
registry root while separately pinning the recorded live trust-transition root;
because no post-rotation live TrustGraph object is committed, the harness used a
canonical derived clone graph and does not claim it is the live sidecar graph.

Code references: `crates/cobalt_e3_harness/src/main.rs`, `crates/node/src/cobalt_shadow.rs`, `crates/node/src/cobalt_shadow_runtime.rs`, `crates/node/src/bin/postfiat_cobalt_liveness_simulation.rs`.

### E4. Finality isolation under governance stress

Execution: complete — passing unchanged clean 500+500 rerun

Frozen campaign source revision:
`451c2ad0e924f8be72feeac69c1356b3828a4f58`.
Executed comparator source revision:
`add07a7cce416daeaa61073085734937477f2b71`.
Checksum-bound packet:
`benchmarks/cobalt-adversarial-verification/e4/`; packet root
`93ba3db0bcc145144713088b612606fbb3b92c0f542809f258da49a555c14508`.

The final clean run passed all 500 baseline and 500 attack rounds. Both
six-validator lanes converged independently at height 501; Consensus v2 never
stopped or forked. Baseline wallet-to-finality was p50
`7,471.082586 ms` and p95 `14,133.573682 ms`; attack was p50
`7,500.377266 ms` and p95 `14,197.471440 ms`. The p95 delta was
`+0.4520990899943289%`, inside the locked 5% budget.

The attack lane completed 47 governance-stress runs covering 940 proposals,
329 safe halts, and 329 view changes. It recorded 987 boundary rejections,
846 named limit rejections, and 752 flood rejections with durable state
unchanged. Validator 5 restarted automatically 12 times. CPU, memory, network,
disk, finality, governance, rejection, restart, and operator-action receipts are
included; no manual operator action was required.

Two redaction-safe remediation receipts remain in the packet. The first records
a retry window shorter than the deliberate restart outage. The second records
an invalid cross-lane tip/root comparator for independent runs with randomized
authentication. Neither failure observed a fork or durable divergence. The
corrected oracle requires convergence inside each lane and equal
signed-message-independent workload and round outcomes across lanes. The
500+500 corpus, full-vote policy, topology, CPU allocation, crash cadence,
binaries, and adversarial inputs remained unchanged.

Frozen manifest:
`benchmarks/cobalt-adversarial-verification/e4/campaign-manifest.json`;
SHA-256
`838a0bccda40f13c6f999fd119706739d9384509bc9495165e0cd6f04fc4c68d`.

- [x] Run at least 500 baseline and 500 attack-lane Consensus v2 rounds from the same signed state on the same fleet, binary, and CPU quota.
- [x] In the attack lane, combine governance storms, repeated halts and view changes, near-limit certificates and RPC frames, sidecar flooding, and one crash-looping validator.
- [x] Count and name every rejection at the 1 MiB certificate and 2 MiB RPC-frame boundaries.
- [x] Pass only if Consensus v2 never stops or forks and attack-lane p95 client-visible finality remains within 5% of baseline.
- [x] Record governance latency, p50/p95 finality, CPU, memory, network, disk, rejected inputs, restarts, and operator actions.

Code references: `benchmarks/cobalt-activate-or-retire/run_consensus_v2_cobalt_integration.py`, `crates/node/src/consensus_v2_finality.rs`, `crates/node/src/cobalt_authority_certificate.rs`, `crates/node/src/cobalt_shadow_runtime.rs`.

### E5. Live authority drills

Execution: not started

- [ ] At scheduled live heights, execute the signed forward rollback to Foundation authority and the separately authorized return to Cobalt authority.
- [ ] Run the early, stale, replayed, wrong-root, cross-chain, mixed-authority, self-authorized, and replayed-rollback negative cases live without mutation.
- [ ] Treat one validator key as stolen, reject its attempted Cobalt-authorized rotation, and commit the legitimate rotation path.
- [ ] Record the proposal and authorization identities for every drill.
- [ ] Prove one accepted history and uninterrupted Consensus v2 finality through both authority transitions.

Code references: `crates/node/src/cobalt_handoff.rs`, `crates/node/src/cobalt_handoff_rehearsal.rs`, `crates/node/src/bin/postfiat_cobalt_handoff_rehearsal.rs`, `benchmarks/cobalt-activation-live/packet`.

### E6. Proposal source and independence decision

Execution: complete — design and decision only; no operators recruited and no live change authorized

Locked design: `docs/governance/cobalt-independent-operator-proposal-path-research-spec.md`; SHA-256 `91ad402672653f3e76489f7e7de719d5597553111985d939a9e90b52a1edec89`.
Checksum-bound decision packet: `benchmarks/cobalt-adversarial-verification/e6`; packet root `52a4ef91a7ec1c8dad385344edf3b498a96d45f5a8568d8c1371e3f3d6d05c81`.
Decision: reinstate the independent-operator gate as its own mandatory follow-on milestone.

- [x] Document the current registry-proposal process, signing keys, validator authorizations, and custody boundaries.
- [x] Design a non-Foundation proposal path and trust graph in which no single administrator can reach quorum or block it alone.
- [x] Reuse the retained operator-admission boundary and onboarding packet; do not recruit operators inside this experiment.
- [x] Record a dated decision to reinstate the locked activation specification's independent-operator gate as its own milestone or formally defer it.
- [x] Lock that decision through a new research specification; do not redefine the prior gate inside this milestone.

Code references: `crates/consensus_cobalt/src/trust_graph_governance.rs`, `crates/consensus_cobalt/src/dabc_registry.rs`, `crates/node/src/cobalt_handoff.rs`, `crates/node/src/cobalt_authority_certificate.rs`, `benchmarks/cobalt-independent-operators/onboarding-contract.json`.

## Gates

### KEEP ACTIVE

- [x] E1: production matches the reconciled independent oracle on every generated graph.
- [x] E2: zero conflicting roots, zero false halts, and zero false accepts under the Byzantine campaign.
- [x] E3: every tampered state and forged catch-up is rejected; honest recovery is byte-identical.
- [x] E4: Consensus v2 never stops or forks; attack-lane p95 finality stays within 5% of baseline.
- [ ] E5: both live authority transitions commit; every live negative case and the stolen-key attempt reject.
- [x] E6: the proposal-path design and independent-operator decision are recorded and locked.
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

- [ ] Every E1-E6 experiment has a final checksum-bound repository evidence packet.
- [ ] The final gate is recorded as `KEEP_ACTIVE`, `REMEDIATION_REQUIRED`, or `ROLLED_BACK` and matches the live authority state.
- [ ] The packet verifier, focused Cobalt and governance tests, CLI and browser tests, strict documentation build, redaction checks, and one explicit final release gate pass.
- [ ] The CLI and read-only browser interface work against the authenticated packet.
- [ ] The adversarial results and precise public claims are published.
- [ ] Honest direct evidence and final verification are complete.
- [ ] Move this journal to `docs/plans/completed/` only after the interfaces, publication, and selected completion gates pass.
