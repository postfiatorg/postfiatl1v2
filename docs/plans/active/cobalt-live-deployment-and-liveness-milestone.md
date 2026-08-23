# Cobalt Live Deployment and Liveness Milestone

## What this plan does

This plan puts Cobalt on the actual controlled-testnet validator machines and determines, from measured behavior, whether PostFiat should activate it for validator-governance decisions.

The first live state is an authenticated, always-on **shadow service** beside each validator. It exchanges real Cobalt protocol messages, persists and replays them, survives planned faults, and produces the same ratification result across the fleet—but it cannot change governance. Consensus v2 continues to order blocks and finalize transactions. A Cobalt failure may pause validator-set evolution; it must not pause payments or create another block-finality rule.

After the real-validator shadow corpus passes, the same declared trust views and faults are run through Cobalt and pinned RippleD simulations. The comparison measures conflicting decisions, safe halts, liveness, recovery, quorum/topology margin, message cost, and resource use. Only then is the existing Foundation-to-Cobalt handoff rehearsed on a disposable clone. This milestone does **not** authorize a live handoff.

- **Status:** Active — authenticated local shadow runtime verified; current live-fleet Gate 0 access pending
- **Locked specification:** [Live Cobalt Deployment and XRPL Liveness Research Specification](../../governance/cobalt-live-deployment-research-spec.md)
- **Research task:** `task_50b08c9b22e2348237b65436d4be4fed` — rewarded
- **Milestone-document task:** `task_4f13e8a9969df968d5a25e5613c6bdd6` — rewarded
- **Decision boundary:** Cobalt governs validator-trust evolution only. Consensus v2 remains the only block-ordering and transaction-finality protocol.

## Task Node ledger

- [x] `task_50b08c9b22e2348237b65436d4be4fed` — write and lock the code-grounded research specification. Rewarded: 2.4 PFT.
- [x] `task_4f13e8a9969df968d5a25e5613c6bdd6` — create and verify this active milestone journal. Rewarded: 2.5 PFT.
- [x] `task_af9dbfab039b00a0b97ee061d3c96a71` — establish the validator-fleet baseline posture and reproduce the Cobalt substrate. Rewarded: 2.4 PFT. The submitted receipt failed closed on current fleet access; the first three section-1 checks remain open.
- [x] `task_5923e7dd509a438806e86f936495709b` — ship the authenticated shadow runtime, Python operator CLI, isolated sidecar, and local verifier packet. Rewarded: 2 PFT.
- [ ] `task_1e38f226f10748cea1367ae883eb6193` — deploy Cobalt shadow to the six live WAN validators and run the full evidence corpus. Accepted: 5 PFT.

The remaining substantial task boundaries will receive Task Node IDs only when the prior gate is complete:

- [x] **Live baseline and reproducible substrate Task Node work item:** `task_af9dbfab039b00a0b97ee061d3c96a71` rewarded 2.4 PFT. The reproducible substrate is green; the current live-fleet receipt remains a Gate 0 prerequisite below.
- [x] **Networked shadow runtime and operator CLI:** `task_5923e7dd509a438806e86f936495709b` rewarded 2 PFT. Local authenticated socket execution and CLI verification are green; live-validator deployment remains gated.
- [ ] **Real-validator rollout and evidence corpus:** `task_1e38f226f10748cea1367ae883eb6193` accepted. Canary, roll out one validator at a time, run the full fault/restart/replay corpus, and publish verifier-backed evidence.
- [ ] **Matched Cobalt/XRPL liveness benchmark:** run the common scenario manifest through both systems and publish the KPI comparison.
- [ ] **Handoff rehearsal and user-facing interface:** rehearse activation and rollback on a disposable clone, expose the verified fleet packet in the browser UI, and prepare—but do not execute—the cutover decision.

## Current code boundary

| Area | Current code | Milestone gap |
| --- | --- | --- |
| Governance authority | `crates/node/src/cobalt_handoff.rs` verifies Foundation, transition, and Cobalt validator-update batches. | Keep authority under Foundation during this milestone; rehearse the handoff only on a disposable clone. |
| Local shadow state | `crates/node/src/cobalt_shadow.rs` authenticates, bounds, de-duplicates, persists, and replays shadow messages with `live_authority=false` and `controls_block_consensus=false`. | Drive signed RBC, ABBA, MVBA, and DABC over the authenticated WAN fleet rather than only local generic shadow input. |
| Agreement and trust | `crates/consensus_cobalt/src/rbc_abba_mvba.rs`, `dabc_registry.rs`, and `trust_graph_governance.rs` provide agreement, replay, graph analysis, and transition witnesses. | Generate topology from the fresh validator registry and run it continuously on the real machines. |
| Executable surface | `crates/node/src/bin/postfiat_cobalt_shadow.rs` exposes `init`, `status`, and `drill`. | Add operator-grade run/probe/snapshot/replay surfaces and a Python CLI that reads authenticated machine output. |
| Existing fleet operations | `systemd/postfiat-validator-transport.service.example`, `scripts/testnet-monitor-snapshot`, and `python/postfiat_ops/safe_rollout.py`. | Reuse their authenticated topology, monitoring, and canary pattern; isolate Cobalt storage, service, port, logs, and restart domain. |
| Existing user interface | `python/postfiat_rpc/cobalt.py` and `python/postfiat_rpc/cobalt_ui.py`. | Show the distributed fleet receipt and benchmark packet, with “shadow healthy” visibly distinct from “authority active.” |

## Milestones

### 1. Freeze the real validator baseline and reproduce current Cobalt

- [ ] Collect a fresh, redacted receipt from every active validator: chain/genesis identity, commit and binary hash, height/tip/state root, registry root, quorum, transport and service health, resource headroom, stable placement/control labels, timestamp, and maximum age.
- [ ] Fail closed on unreachable validators, chain divergence, stale inventory, unknown registry membership, reused keys, or missing topology labels.
- [ ] Generate the Cobalt trust graph and thresholds from the discovered registry; do not reuse the old seven-validator fixtures.
- [x] Restore a working pinned C/C++ linker invocation and run the current locked Cobalt tests and examples from a clean build. (`scripts/zig-cc`, `scripts/zig-ar`, `scripts/verify-cobalt-substrate`)
- [x] Label loopback and hard-coded-fixture results as local baseline evidence, never as live-validator proof. (`scripts/verify-cobalt-substrate` emits `live_validator_evidence=false`.)

Evidence: `fleet-receipt.public.json`, private bound receipt, graph root, build manifest, test reports, hashes, and verifier result.

Implementation journal, 2026-08-22:

- Vultr provider authentication was restored on 2026-08-23. Fresh inventory proves all six prior WAN validator instance identities remain active within a 30-instance account inventory.
- Operating-system access remains the live Gate 0 boundary: this workstation has no matching WAN SSH identity, direct RPC remains closed, and no validator registry, topology, or chain-health receipt has yet been collected. Deployment therefore remains stopped before the canary.
- The pinned Zig wrappers now translate Rust's vendor-qualified Linux target and provide both compiler and archiver entrypoints. They fail closed when `POSTFIAT_ZIG` does not resolve to an executable.
- Current substrate verification passes 70 Cobalt tests, 70 unsafe-simulation tests, five node handoff tests, the current trust-root example, all partition scenarios, and the seven-worker TCP loopback drill. The two simulation examples were repaired to use an explicitly nonempty schema-only simulation signature under `cobalt-unsafe-simulation`; this is not message authentication.
- Clean-build verifier manifest SHA-256: `8e9aac2f3ebfa84595bbcebb2f71ed3309a799078c962b2a2e418b198e413715`. Bound source SHA-256: `159f5cf0bd7d61a1cc1eefaf19f31e682a6c6decb44d9623975c50d7dcb121ff`. The generated packet is intentionally uncommitted under `.tih/`.
- Task Node accepted the mixed evidence after requesting and receiving the raw clean-build excerpt. Final state: rewarded 2.4 PFT. This reward closes the work item, not the explicitly red live-fleet gate.

### 2. Run Cobalt as authenticated, non-authoritative WAN infrastructure

- [x] Add long-running run/probe/snapshot/replay service surfaces around the durable shadow state in `crates/node/src/cobalt_shadow.rs` and `cobalt_shadow_runtime.rs`.
- [ ] Bind each Cobalt signer to one live registry validator and the current registry root using the existing validator identity.
- [x] Carry canonical, domain-separated protocol messages through a bounded authenticated socket topology; live WAN evidence remains pending.
- [x] Drive and persist the real signed RBC, ABBA, MVBA, and DABC stages, including locks and high-water marks, before related signatures leave the process.
- [x] Expose structured peer, queue, stage-latency, graph-root, ratification-lock, replay, message/byte, and resource metrics.
- [x] Provide an unprivileged, bounded sidecar whose lifecycle and writable storage are isolated from the block validator.
- [x] Prove the local three-socket nodes report `live_authority=false` and `controls_block_consensus=false`.

Evidence: service configuration, signer-binding receipts, authenticated peer snapshots, restart/replay tests, bounded-resource tests, and machine-readable probe output.

Implementation journal, 2026-08-22:

- `cobalt_shadow.rs` now binds validator keys to registry and trust-graph roots, persists outbound and ratification locks, validates real ML-DSA-signed RBC/ABBA/MVBA/DABC transcripts, and records replay-safe decisions and per-stage timing.
- `cobalt_shadow_runtime.rs` and `postfiat-cobalt-shadow` expose bounded long-running `run`, `probe`, `snapshot`, `replay`, `commit`, binding, and reservation surfaces. Mutating message paths fail closed on membership, domain, root, signature, replay, and frame bounds.
- `python/postfiat_rpc/cobalt.py` exposes human-readable `fleet`, `graph`, `shadow-status`, `probe`, `snapshot`, and `replay` commands against the same structured runtime output. The sidecar unit has no validator lifecycle dependency and cannot write validator state.
- Local verifier packet `.tih/cobalt-shadow-runtime-20260822-v2` passes 8 focused Rust tests, 11 Python tests, strict Clippy, three socket nodes, 25 signed stage messages, restart-equivalent replay, tamper and oversized-frame rejection, and real Python-to-Rust probe/snapshot/replay calls. Manifest SHA-256: `f2bc94bcc839943d7b70ee3f96c11808fb5e995b4295fda19908f5df986ec274`.
- This packet is explicitly local loopback evidence (`live_validator_evidence=false`). The live-registry binding check remains open until current fleet access is restored; no deployment or authority transfer occurred.
- Task Node accepted the initial and follow-up verifier evidence and rewarded the work item 2 PFT. Transaction: `4A8293A5A9DDCA433A9250DF82F386DFE84C12A1BF13C230A18C2EE996F5CB63`.

### 3. Canary and complete the real-validator shadow corpus

- [ ] Deploy one canary and freeze resource/latency alert thresholds before adversarial tests.
- [ ] Roll out one validator at a time, stopping on chain-health regression, identity mismatch, graph disagreement, or resource exhaustion.
- [ ] Submit the fixed inert proposal corpus: no-op, validator add/remove, trust-view change, key rotation, invalid parent, and rollback.
- [ ] Complete planned restart of every sidecar, replay from genesis, one-validator outage, one-region isolation, delay/loss/reorder injection, equivocation, stale replay, and partition healing.
- [ ] Require identical accepted/rejected outcomes and ordered ratification digests across correct validators.
- [ ] Confirm consensus v2 continues finalizing blocks throughout every Cobalt-only fault.

Evidence: per-validator receipts, proposal and fault markers, ratification digests, block-finality continuity, recovery timing, raw reports, canonical checksums, and static verifier.

### 4. Compare Cobalt with pinned RippleD under one scenario contract

- [ ] Pin RippleD 3.1.3 at `46b241ace8b30d9c9775d60ffba7d24b21903896`; use upstream `src/test/csf` and `Consensus_test::testFork` as the native control.
- [ ] Record local quorum from `ValidatorList::calculateQuorum`; do not treat local quorum as proof of global UNL overlap.
- [ ] Keep AGTI’s UNL-overlap extension separately pinned and identified as a downstream test.
- [ ] Run Cobalt trust analysis and signed agreement/replay from the same canonical scenario manifest.
- [ ] Cover the actual fleet and 7-, 10-, and 20-validator controls across overlap, asymmetric views, declared Byzantine budgets, correlated loss, partitions/healing, delay/loss/reorder, list/graph drift, add/remove, and key rotation.
- [ ] Publish failures and safe halts without averaging them away or comparing Cobalt governance latency to XRPL payment latency.

Required KPI report:

| KPI | Required interpretation |
| --- | --- |
| Conflicting decisions | Zero for the same domain/slot among correct validators inside each declared model. |
| Safe halt and liveness | Separate correct halts from forks; require completion for no-fault and within-model scenarios. |
| Stage latency and recovery | Report p50/p95/p99/max by stage and fault class; measure healed/restarted convergence without state edits. |
| Quorum/topology margin | Report exact set intersections and the smallest validator or correlation-group loss that blocks progress or permits conflict. |
| Trust safety and replay | Report unsafe validator pairs and require bit-identical live/replay roots and locks. |
| Communication and resources | Signed messages, bytes, CPU, RSS, disk, queues, descriptors, and validator-service delta. |
| Operational and evidence health | Probe availability, stale ages, restarts, required artifacts, hashes, markers, and verifier outcome. |

### 5. Rehearse authority transfer; deliver human interfaces; decide

- [ ] Rehearse the exact `cobalt_handoff.rs` transition on a disposable clone using current-registry ML-DSA approvals and a future activation height.
- [ ] Prove early, stale, replayed, wrong-root, mixed-authority, and self-authorized handoffs fail.
- [ ] Prove pre-activation abort keeps Foundation authority and post-activation rollback is a new forward transition.
- [ ] Execute one validator-trust update under rehearsed Cobalt authority while unrelated governance kinds remain rejected.
- [ ] Deliver a Python CLI that a human can run to inspect fleet, graph, shadow, scenario, replay, and readiness state.
- [ ] Update the read-only browser interface to consume the same authenticated output and clearly distinguish shadow health, rehearsal readiness, and actual authority.
- [ ] Refresh the concise operator/runbook documentation after the CLI and interface work.
- [ ] Produce a separate go/no-go packet. A live controlled-testnet cutover requires explicit later authorization and its own Task Node-governed work.

## Activation decision

Recommend a later controlled-testnet authority cutover only if all of these are true:

- [ ] Every current validator is represented by a fresh, consistent fleet receipt and safe trust graph.
- [ ] Current-commit tests, live shadow operation, replay, restart, and the full fault corpus pass.
- [ ] Conflicting-decision count is zero and safe-halt/liveness behavior matches the declared model.
- [ ] Consensus v2 finality stays healthy through every Cobalt fault.
- [ ] The matched XRPL/Cobalt packet has no unresolved methodology exception.
- [ ] The disposable handoff, abort, forward rollback, and scoped-authority checks pass.
- [ ] The Python CLI, browser UI, monitoring, alerts, verifier, and operator runbook reflect the live service.

If any check fails, Cobalt stays live in shadow and Foundation authority remains active. The failed gate is repaired and rerun; observation does not need to stop.

## Completion rule

This milestone is complete only after all implementation tasks reach Task Node’s final rewarded state, the Python CLI and browser interface work against authenticated live-fleet evidence, and the cutover recommendation has a verifier-backed packet. At that point this journal moves to `docs/plans/completed/`.

A recommendation to activate is not activation. Any authority cutover remains a separate, explicitly approved operation.
