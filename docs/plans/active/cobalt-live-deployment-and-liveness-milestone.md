# Cobalt Live Deployment and Liveness Milestone

## What this plan does

This plan puts Cobalt on the actual controlled-testnet validator machines and determines, from measured behavior, whether PostFiat should activate it for validator-governance decisions.

The first live state is an authenticated, always-on **shadow service** beside each validator. It exchanges real Cobalt protocol messages, persists and replays them, survives planned faults, and produces the same ratification result across the fleet—but it cannot change governance. Consensus v2 continues to order blocks and finalize transactions. A Cobalt failure may pause validator-set evolution; it must not pause payments or create another block-finality rule.

After the real-validator shadow corpus passes, the same declared trust views and faults are run through Cobalt and pinned RippleD simulations. The comparison measures conflicting decisions, safe halts, liveness, recovery, quorum/topology margin, message cost, and resource use. Only then is the existing Foundation-to-Cobalt handoff rehearsed on a disposable clone. This milestone does **not** authorize a live handoff.

- **Status:** Active — Sections 1–5 are complete; the Section 5 matched benchmark is verifier-backed and rewarded, and Section 6 disposable-clone handoff rehearsal is next.
- **Locked specification:** [Live Cobalt Deployment and XRPL Liveness Research Specification](../../governance/cobalt-live-deployment-research-spec.md)
- **Research task:** `task_50b08c9b22e2348237b65436d4be4fed` — rewarded
- **Milestone-document task:** `task_4f13e8a9969df968d5a25e5613c6bdd6` — rewarded
- **Decision boundary:** Cobalt governs validator-trust evolution only. Consensus v2 remains the only block-ordering and transaction-finality protocol.

## Task Node ledger

- [x] `task_50b08c9b22e2348237b65436d4be4fed` — write and lock the code-grounded research specification. Rewarded: 2.4 PFT.
- [x] `task_4f13e8a9969df968d5a25e5613c6bdd6` — create and verify this active milestone journal. Rewarded: 2.5 PFT.
- [x] `task_af9dbfab039b00a0b97ee061d3c96a71` — establish the validator-fleet baseline posture and reproduce the Cobalt substrate. Rewarded: 2.4 PFT. The submitted receipt failed closed on current fleet access; the first three section-1 checks remain open.
- [x] `task_5923e7dd509a438806e86f936495709b` — ship the authenticated shadow runtime, Python operator CLI, isolated sidecar, and local verifier packet. Rewarded: 2 PFT.
- [x] `task_1e38f226f10748cea1367ae883eb6193` — deploy Cobalt shadow to the six live WAN validators and run the full evidence corpus. Rewarded: 3.5 PFT.
- [x] `task_84c0561295204bcf5e7c389475e5fcdc` — repair five-of-six progress and authenticated history recovery as one coupled Section 4 task. Rewarded: 2.7 PFT.
- [x] `task_c6c02afcf8fd9bef26dae16bbc5b32ec` — execute the complete matched Cobalt/RippleD benchmark and comparison. Rewarded: 1.5 PFT after the complete verification response; the initial evidence write was an accidental placeholder and is disclosed in the Task Node event history.
- [ ] `task_d0b0b9553d6eb09aef54b8b0b1e3aada` — rehearse Foundation-to-Cobalt handoff, abort, and forward rollback on a disposable clone. Accepted: 4.5 PFT; gated on remediation and benchmark evidence.
- [ ] `task_a0ffacf2640f5f76ae72002b98d14978` — deliver the complete Python CLI, browser UI, concise operator docs, and final go/no-go packet. Accepted: 4.5 PFT; gated on the preceding three tasks.

Execute the unchecked tasks in the order listed. Acceptance records the work ledger; it does not bypass the preceding evidence gates or authorize a live handoff.

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

- [x] Collect a fresh, redacted receipt from every active validator: chain/genesis identity, commit and binary hash, height/tip/state root, registry root, quorum, transport and service health, resource headroom, stable placement/control labels, timestamp, and maximum age.
- [x] Fail closed on unreachable validators, chain divergence, stale inventory, unknown registry membership, reused keys, or missing topology labels.
- [x] Generate the Cobalt trust graph and thresholds from the discovered registry; do not reuse the old seven-validator fixtures.
- [x] Restore a working pinned C/C++ linker invocation and run the current locked Cobalt tests and examples from a clean build. (`scripts/zig-cc`, `scripts/zig-ar`, `scripts/verify-cobalt-substrate`)
- [x] Label loopback and hard-coded-fixture results as local baseline evidence, never as live-validator proof. (`scripts/verify-cobalt-substrate` emits `live_validator_evidence=false`.)

Evidence: `fleet-receipt.public.json`, private bound receipt, graph root, build manifest, test reports, hashes, and verifier result.

Implementation journal, 2026-08-22:

- Vultr provider authentication was restored on 2026-08-23. Fresh inventory proves all six prior WAN validator instance identities remain active within a 30-instance account inventory.
- OS access is restored across all six validators. Fresh checks bind every machine to `postfiat-wan-devnet-2`, protocol 1, the expected genesis, the six-validator registry, and one registry root. Validator services remained active with zero restarts. Validator 0's separately installed RPC companion was started, without restarting its validator, when validator 0 was elected proposer for height 912.
- The canary sidecar is live on the private WireGuard interface with a 128 MiB memory cap and no validator lifecycle relationship. It uses about 1.8 MiB at idle, survives a planned restart with durable state, reports `live_authority=false` and `controls_block_consensus=false`, and left the validator PID, start time, restart count, and binary hash unchanged. It remains deliberately unbound until all six live identity statements exist.
- Fleet binding now requires a domain-separated ML-DSA statement made by each validator's existing key over its sidecar key and exact live registry root. Registry-manifest construction recomputes the live registry root and rejects missing, duplicated, tampered, cross-domain, or unregistered bindings before installing the trust graph.
- The pinned Zig wrappers now translate Rust's vendor-qualified Linux target and provide both compiler and archiver entrypoints. They fail closed when `POSTFIAT_ZIG` does not resolve to an executable.
- Current substrate verification passes 70 Cobalt tests, 70 unsafe-simulation tests, five node handoff tests, the current trust-root example, all partition scenarios, and the seven-worker TCP loopback drill. The two simulation examples were repaired to use an explicitly nonempty schema-only simulation signature under `cobalt-unsafe-simulation`; this is not message authentication.
- The final live-rollout artifact was built from commit `1bce501bcef6` with SHA-256 `d311ad733bcecd7f87769264b745e60fd31c919f354af851fc56db85b5e99067`. It is live on all six validators with validator-signed binding receipts and one six-validator trust manifest (quorum 5; trust-graph root `c872bf8a9628cb3b27f2c0826084beb540c645d0c9d06107643358a4df078fa919e88ba2aa6b376a904eb79d28d69e77`). TCP/29651 is allowed only on `wg0` from `10.77.0.0/24`. Every sidecar reports `live_authority=false` and `controls_block_consensus=false`; validator PIDs and restart counts remained unchanged.
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

- `cobalt_shadow.rs` now binds validator keys to registry and trust-graph roots, persists outbound and ratification locks, validates real ML-DSA-signed RBC/ABBA/MVBA/DABC transcripts, and records replay-safe decisions and per-stage timing. Distributed `propose`, `contribute`, and `assemble` commands keep each Cobalt private key on its validator while producing the common transcript delivered over the private WAN.
- `cobalt_shadow_runtime.rs` and `postfiat-cobalt-shadow` expose bounded long-running `run`, `probe`, `snapshot`, `replay`, `commit`, binding, and reservation surfaces. Mutating message paths fail closed on membership, domain, root, signature, replay, and frame bounds.
- `python/postfiat_rpc/cobalt.py` exposes human-readable `fleet`, `graph`, `shadow-status`, `probe`, `snapshot`, and `replay` commands against the same structured runtime output. The sidecar unit has no validator lifecycle dependency and cannot write validator state.
- Local verifier packet `.tih/cobalt-shadow-runtime-20260822-v2` passes 8 focused Rust tests, 11 Python tests, strict Clippy, three socket nodes, 25 signed stage messages, restart-equivalent replay, tamper and oversized-frame rejection, and real Python-to-Rust probe/snapshot/replay calls. Manifest SHA-256: `f2bc94bcc839943d7b70ee3f96c11808fb5e995b4295fda19908f5df986ec274`.
- This packet is explicitly local loopback evidence (`live_validator_evidence=false`). The live-registry binding check remains open until current fleet access is restored; no deployment or authority transfer occurred.
- Task Node accepted the initial and follow-up verifier evidence and rewarded the work item 2 PFT. Transaction: `4A8293A5A9DDCA433A9250DF82F386DFE84C12A1BF13C230A18C2EE996F5CB63`.

### 3. Canary and complete the real-validator shadow corpus

- [x] Deploy one canary and freeze resource/latency alert thresholds before adversarial tests.
- [x] Roll out one validator at a time, stopping on chain-health regression, identity mismatch, graph disagreement, or resource exhaustion.
- [x] Submit the fixed inert proposal corpus: no-op, validator add/remove, trust-view change, key rotation, invalid parent, and rollback. These are agreement payload hashes only; they execute no governance effect.
- [x] Complete planned restart of every sidecar, replay from genesis, one-validator outage, one-region isolation, delay/loss/reorder injection, equivocation, stale replay, and partition healing.
- [ ] Require identical accepted/rejected outcomes and ordered ratification digests across correct validators. **Failed:** validator 5 missed round 1004, advanced to 1005, and then correctly rejected the unseen older transcript as stale, leaving non-identical durable decision history.
- [ ] Confirm consensus v2 continues finalizing blocks throughout every Cobalt-only fault. Consensus v2 advanced from height 910 to 913 with all six validator PIDs unchanged, including a block during the validator-5 Cobalt outage and another during the validators-3/4 Cobalt partition. The shorter equivocation, stale, duplicate, reorder, and replay calls are bracketed by this finality evidence but did not each contain a block.

Evidence: per-validator receipts, proposal and fault markers, ratification digests, block-finality continuity, recovery timing, raw reports, canonical checksums, and static verifier.

Live result, 2026-08-23:

- Clean WAN agreement, six-sidecar restart/replay, validator-5 outage/heal, validators-3/4 partition/heal, reordered and lost delivery, durable equivocation rejection, stale replay, duplicate idempotency, and rounds 1006–1011 of the inert corpus were executed on the real fleet. Each completed transcript carried 49 signed messages.
- With validator 5 absent, five contributions did not assemble even though the trust graph declares quorum 5: the full-knowledge stage requires every active validator. The system safely halted and completed only after validator 5 returned.
- A second failure is more serious for activation: validator 5 missed round 1004, accepted round 1005, and then could not ingest round 1004 because the high-water mark correctly rejected it as stale. Five nodes retain rounds 1001–1011; validator 5 retains the same history except round 1004. Manual state repair was deliberately not used.
- Consensus v2 independently advanced from height 910 to 913 through three real one-atom devnet transfers: one after the corpus, one during the validator-5 Cobalt outage, and one during the validators-3/4 Cobalt partition. All six validators converged on one tip/state root with zero validator restarts. Sidecars use about 1.9–2.0 MiB each at rest.
- Static packet: `.tih/cobalt-live-evidence-20260823`; verifier result `cobalt-live-packet-ok`; canonical packet SHA-256 `2e07ada7ba4f174e5c2ad24422ac503838544aa359b461ca6cb95f146815177a`.
- **Decision for the current binary:** keep the six sidecars observational and Foundation authority unchanged. This is a repair-and-rerun result, not a rejection of Cobalt: the failures are in the shadow integration described below.

### 4. Repair quorum progress and authenticated history catch-up

The live test found two concrete integration defects. The Cobalt trust graph already says that five of six validators provide strong support, but the node wrapper separately demands all six contributions. The recovery path then stores only decision summaries, permits a node to advance over a missing round, and has no signed transcript journal from which to repair the gap. The next implementation segment fixes those boundaries and repeats the same live test.

Pre-remediation code findings, now closed by Sections 4.1–4.4:

| Finding | Current code | Required ownership |
| --- | --- | --- |
| All-six assembly override | `assemble_protocol_transcript` compares contributor IDs with the entire active registry at `crates/node/src/cobalt_shadow.rs:1771`; transcript validation separately requires `full_knowledge_checks.len() == committee.len()` at line 1114. | Quorum and full-knowledge support rules belong in `consensus_cobalt`; `node` should compose and persist their certificates. |
| Unchained ratifications | Both assembly and validation call `ratify_dabc_amendment(..., None, ...)` at `crates/node/src/cobalt_shadow.rs:1830` and line 1099, so every live round is treated as a sequence-1 child of the genesis parent even though `dabc_registry.rs` already supports parent-linked chains. | Build and validate every new ratification against the durable previous `DabcRatifiedAmendment`. |
| No recoverable history | `CobaltShadowState` stores `CobaltShadowProtocolDecision` summaries but not the signed transcripts; `Replay` in `cobalt_shadow_runtime.rs:379` only returns those summaries. | Persist bounded signed proof material and expose authenticated range synchronization. |
| High-water mark conflates safety with completeness | `commit_protocol_transcript` accepts a newer round and later rejects an unseen older round at `crates/node/src/cobalt_shadow.rs:892-915`. | Keep signer anti-equivocation locks, but track contiguous committed history separately and require catch-up before advancing across a gap. |

#### 4.1 Make five-of-six a real protocol certificate

- [x] Replace the exact-active-set check in `assemble_protocol_transcript` with sorted, unique, registered contributors whose signed RBC, ABBA, and DABC support satisfies every relevant trust view through the existing `evaluate_*_support_signed` and `has_strong_support` rules. Do not add a second integer threshold in `node`.
- [x] Replace the all-committee full-knowledge count with committee-signed `DabcFullKnowledgeCheckpoint` validation using `build_dabc_full_knowledge_checkpoint_signed` and `validate_dabc_full_knowledge_checkpoint_signed`. For the current canonical graph, any valid five-of-six set must satisfy every validator view; four-of-six must fail.
- [x] Separate the canonical decision identity from its support-certificate hash. Two correct assemblers that see different valid five-signer subsets must derive the same decision ID, ratification ID, and governance digest even if their audit certificate bytes differ.
- [x] Preserve duplicate-signer elimination, equivocation exclusion, ML-DSA committee verification, registry/trust-root binding, transcript bounds, and durable outbound locks.

#### 4.2 Chain every ratification and refuse silent gaps

- [x] Extend the transcript/state model so each ratification carries and validates the previous durable `DabcRatifiedAmendment`; only the explicit history anchor may use `dabc_genesis_parent_id()`.
- [x] Replace the single maximum `protocol_high_watermark` interpretation with distinct signer-safety and contiguous-history state. A transcript above the next expected sequence returns a bounded `catch_up_required` result and is not committed.
- [x] Make exact replay idempotent, conflicting replay fail closed, and parent/sequence/slot mismatch reject before any lock, decision, digest, or journal mutation.
- [x] Introduce an explicit state-schema migration. Existing shadow-v2 rounds are retained as historical evidence and linked into a migration receipt, but they are not relabeled as a valid DABC chain or repaired by editing state files.

#### 4.3 Add proof-carrying catch-up

- [x] Persist each accepted signed transcript in a bounded append-only journal separate from `state.json`, with hashes indexed by round and crash recovery that reconciles journal and state without partial acceptance.
- [x] Add bounded `HistoryRange` and `CatchUp` RPC operations in `cobalt_shadow_runtime.rs`, plus `history export`, `history verify`, and `catch-up` commands in `postfiat-cobalt-shadow` and `python/postfiat_rpc/cobalt.py`.
- [x] A lagging validator must fetch the missing contiguous range from one or more peers, independently verify every domain, registry root, trust graph, ML-DSA signature, strong-support certificate, parent link, sequence, slot, and size bound, then atomically advance. Peer snapshots or claimed high-water marks are never trusted.
- [x] Reject truncated, oversized, reordered, conflicting-parent, wrong-root, stale-graph, duplicate, and partially valid catch-up batches without changing durable state.
- [x] Expose `history_head`, `contiguous_sequence`, `missing_ranges`, `catch_up_status`, and certificate signer count in probe, Python CLI, and the existing read-only Cobalt UI.

#### 4.4 Prove the repair locally and on the same six validators

- [x] Add focused owner tests in `consensus_cobalt` and `node`: omit each validator in turn and complete with five; reject every four-signer set; converge across different valid five-signer certificates; exclude duplicates/equivocators; and preserve locks across restart.
- [x] Add recovery tests where a validator misses N, receives N+1, refuses to advance, imports N through signed catch-up, then accepts N+1 with the same ordered ratification chain and governance digest as its peers.
- [x] Add crash tests before journal write, after journal write but before state update, and after state update; restart must either complete the same commit or expose a recoverable pending record, never a fabricated decision.
- [x] Run formatting, strict Clippy, focused package tests, workspace checks/tests, Python CLI tests, and a static verifier packet.
- [x] Roll the new binary canary-first to the existing six shadow sidecars. Repeat the exact one-validator outage and missed-round tests without manual state edits, while finalizing a Consensus v2 block inside each fault window.
- [x] Pass only when any five-of-six correct validators can ratify, four cannot, a returning validator automatically reaches an identical chained history, all correct validators expose the same decision/governance digest, and both authority flags remain false.

Task Node task: `task_84c0561295204bcf5e7c389475e5fcdc` governed Sections 4.1–4.4 as one coupled consensus-and-recovery remediation and was rewarded 2.7 PFT on 2026-08-23.

Section 4 result, 2026-08-23:

- `crates/node/src/cobalt_shadow.rs` now owns canonical five-of-six certificates, parent-linked ratifications, schema-v3 migration, append-only signed history, gap refusal, atomic catch-up, and the three restart boundaries. Runtime/CLI/UI surfaces are in `cobalt_shadow_runtime.rs`, `postfiat_cobalt_shadow.rs`, and `python/postfiat_rpc/cobalt*.py`.
- Formatting, strict Clippy, 13 focused node tests, 70 `consensus_cobalt` tests, the full Rust workspace, and 16 Python CLI/UI tests passed. Local verifier packet: `.tih/cobalt-shadow-runtime-20260823-section4-v3`; manifest SHA-256 `d96fc847518310b2fc310b6ff453845bfe07f17ddf3ebb2d80d0340cae491b0e`.
- Release `cobalt-shadow-section4-4cbde934dfcb` rolled canary-first to the same six sidecars. All v2 states produced migration receipts; every validator PID, restart count, and binary hash stayed unchanged.
- Live rounds 1201–1202 proved five-of-six progress, four-of-six rejection, no-mutation gap refusal, signed catch-up, and one common history head/governance digest. Consensus v2 finalized heights 913→914 inside the outage and 914→915 after recovery.
- Live verifier packet: `.tih/cobalt-section4-live-20260823-v1`; result SHA-256 `b29ba88207f45dfb4d6ae4f161b33b023cd90bea7e12555ce2ed72961ed30ee5`; `SHA256SUMS` SHA-256 `333c5abdc295ee785c719d58ea0835f5a502eb5759245d1ca6ee863e74239232`. Foundation authority and Consensus v2 block authority remain unchanged.

### 5. Compare Cobalt with pinned RippleD under one scenario contract

- [x] Pin RippleD 3.1.3 at `46b241ace8b30d9c9775d60ffba7d24b21903896`; use upstream `src/test/csf` and `Consensus_test::testFork` as the native control.
- [x] Record local quorum from `ValidatorList::calculateQuorum`; do not treat local quorum as proof of global UNL overlap.
- [x] Keep AGTI’s UNL-overlap extension separately pinned and identified as a downstream test.
- [x] Run Cobalt trust analysis and signed agreement/replay from the same canonical scenario manifest.
- [x] Cover the actual fleet and 7-, 10-, and 20-validator controls across overlap, asymmetric views, declared Byzantine budgets, correlated loss, partitions/healing, delay/loss/reorder, list/graph drift, add/remove, and key rotation.
- [x] Publish failures and safe halts without averaging them away or comparing Cobalt governance latency to XRPL payment latency.

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

Section 5 result, 2026-08-23:

- Commit `3f00cb32` adds the 80-case deterministic contract, signed Cobalt adapter, downstream RippleD CSF adapter, valid deterministic-network patch, reproducible packet builder, and full comparison packet. Canonical manifest SHA-256: `4c95d48eb8414cea304b82c6ce334481509d584b39a205b5d6de0cf3110a44d4`.
- Both adapters passed all 80 declared outcomes with zero conflicting decisions. Cobalt contribution/assembly/commit p95 was 528,217/315,489/7,367,063 µs; RippleD native CSF convergence p95 was 46,000 virtual ms. Transport and cryptographic models remain separately labeled and are not ranked against each other.
- The smallest observed blocking loss was budget-plus-one in every topology (two, two, three, and five validators). Every single-validator loss remained live; no tested loss or overlap case produced a conflict.
- Native `ripple.consensus.Consensus`, including `testFork`, passed 13 cases and 1,370 elementary tests. `ValidatorList::calculateQuorum` is recorded as local only; the overlap sweep separately cites AGTI report commit `81f6a7e8d6e0da8c2ab334209c133e85e617e6e2` as downstream provenance.
- Packet: `benchmarks/cobalt-rippled-liveness/packet`; verifier result `passed`; `SHA256SUMS` SHA-256 `7968a085033419255b52b844edd586346a1e85561394e52c69e6683b2561c50b`. All Cobalt authority flags remained false and neither adapter touched the live validator services.
- Task Node reached rewarded state on 2026-08-23. The accidental placeholder initial write was followed by the complete verification response; reward was 1.5 PFT.

### 6. Rehearse authority transfer; deliver human interfaces; decide

- [ ] Rehearse the exact `cobalt_handoff.rs` transition on a disposable clone using current-registry ML-DSA approvals and a future activation height.
- [ ] Prove early, stale, replayed, wrong-root, mixed-authority, and self-authorized handoffs fail.
- [ ] Prove pre-activation abort keeps Foundation authority and post-activation rollback is a new forward transition.
- [ ] Execute one validator-trust update under rehearsed Cobalt authority while unrelated governance kinds remain rejected.
- [ ] Deliver a Python CLI that a human can run to inspect fleet, graph, shadow, scenario, replay, and readiness state.
- [ ] Update the read-only browser interface to consume the same authenticated output and clearly distinguish shadow health, rehearsal readiness, and actual authority.
- [ ] Refresh the concise operator/runbook documentation after the CLI and interface work.
- [ ] Produce a separate go/no-go packet. A live controlled-testnet cutover requires explicit later authorization and its own Task Node-governed work.

Task Node tasks: `task_d0b0b9553d6eb09aef54b8b0b1e3aada` governs the disposable handoff rehearsal after the benchmark; `task_a0ffacf2640f5f76ae72002b98d14978` governs the complete CLI, browser UI, documentation, and decision packet after the rehearsal.

## Activation decision

Recommend a later controlled-testnet authority cutover only if all of these are true:

- [ ] Every current validator is represented by a fresh, consistent fleet receipt and safe trust graph.
- [ ] Current-commit tests, live shadow operation, replay, restart, and the full fault corpus pass.
- [ ] Any valid five-of-six signer set makes progress, every four-of-six set fails, and different valid support certificates resolve to one canonical decision identity.
- [ ] A validator missing one or more rounds refuses to advance across the gap, catches up from independently verified signed history, and converges without manual state repair.
- [ ] Conflicting-decision count is zero and safe-halt/liveness behavior matches the declared model.
- [ ] Consensus v2 finality stays healthy through every Cobalt fault.
- [ ] The matched XRPL/Cobalt packet has no unresolved methodology exception.
- [ ] The disposable handoff, abort, forward rollback, and scoped-authority checks pass.
- [ ] The Python CLI, browser UI, monitoring, alerts, verifier, and operator runbook reflect the live service.

If any check fails, Cobalt stays live in shadow and Foundation authority remains active. The failed gate is repaired and rerun; observation does not need to stop.

## Completion rule

This milestone is complete only after all implementation tasks reach Task Node’s final rewarded state, the Python CLI and browser interface work against authenticated live-fleet evidence, and the cutover recommendation has a verifier-backed packet. At that point this journal moves to `docs/plans/completed/`.

A recommendation to activate is not activation. Any authority cutover remains a separate, explicitly approved operation.

## Historical pre-Section-5 handoff — 2026-08-23

This was the handoff used to start Section 5 and is retained as build/failure provenance; Section 5 is now complete above. Continue with Section 6. Do not activate Cobalt or modify the live validator authority path. Consensus v2 remains the only block-finality mechanism and Foundation authority remains active. Section 6 is a disposable-clone rehearsal only unless the user later gives explicit cutover authorization.

Task Node sequence:

1. Complete and fully verify/reward `task_c6c02afcf8fd9bef26dae16bbc5b32ec` for the matched Cobalt/RippleD benchmark.
2. Then complete `task_d0b0b9553d6eb09aef54b8b0b1e3aada` for the disposable handoff rehearsal.
3. Then complete `task_a0ffacf2640f5f76ae72002b98d14978` for CLI, browser UI, concise documentation, and the go/no-go packet.
4. Move this journal to `docs/plans/completed/` only after all three tasks reach final rewarded state and every completion gate above is satisfied.

Section 5 worktree at handoff:

- `benchmarks/cobalt-rippled-liveness/generate_scenarios.py` and `scenario-manifest.json` define 80 deterministic cases over the live six-validator topology plus 7-, 10-, and 20-validator controls. The corpus covers no-fault, one fault, within/beyond budget, correlated loss, partition/heal, delay/loss/reorder, asymmetric views, graph/list drift, validator add/remove, key rotation, equivocation, and overlap sweeps.
- `crates/node/src/bin/postfiat_cobalt_benchmark.rs` is the native Cobalt adapter. `crates/node/Cargo.toml` registers it as `postfiat-cobalt-benchmark`. It uses the real signed Cobalt stages, trust analysis, durable locks, commit/reopen/replay, and resource/message accounting; it never enables authority.
- `benchmarks/cobalt-rippled-liveness/rippled/MatchedLivenessBenchmark_test.cpp` is the downstream native CSF adapter. `csf-deterministic-network-faults.patch` is intended to add deterministic loss, duplication, reordering, and counters to upstream `BasicNetwork`.
- RippleD is cloned at `.tih/rippled-3.1.3` and pinned to `46b241ace8b30d9c9775d60ffba7d24b21903896`. Conan state is under `.tih/conan-home`; the build directory is `.tih/rippled-build`.
- The pinned Zig wrappers are `scripts/zig-cc`, `zig-cxx`, `zig-ar`, `zig-ranlib`, and `zig-ld`. They exist because the host `cc`/archive tools are not usable directly. Always set `POSTFIAT_ZIG=/home/postfiatchad/.local/zig-0.17.0-dev.1857/zig`.

Section 5 blockers at handoff, all remediated:

1. Cargo reaches the benchmark binary but stops at the large `serde_json::json!` expression around line 580 with a macro recursion-limit error. Add `#![recursion_limit = "256"]` at the top or split that report object, then rerun:
   `CC=$PWD/scripts/zig-cc CXX=$PWD/scripts/zig-cxx AR=$PWD/scripts/zig-ar RANLIB=$PWD/scripts/zig-ranlib CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$PWD/scripts/zig-cc cargo check -p postfiat-node --bin postfiat-cobalt-benchmark`.
2. Conan now passes Boost feature detection, but Boost PCH linking fails with `pch.o: unknown file type`. Disable Boost PCH through the Conan Boost option or extra b2 flag and rerun the install. Do not install or replace the system compiler.
3. `csf-deterministic-network-faults.patch` is malformed at its second hunk header. Apply the intended edits to the temporary pinned checkout, regenerate a valid patch from `git -C .tih/rippled-3.1.3 diff`, then apply-check that stored patch before building. The C++ test file has already been copied into the temporary checkout but has not compiled.
4. After both adapters compile, run the same canonical manifest through each, aggregate the KPI table required above, and place the compact verifier-backed packet under `benchmarks/cobalt-rippled-liveness/packet/`, not in a new `docs/evidence` or handoff directory.
5. Keep the RippleD 3.1.3 native CSF result, `ValidatorList::calculateQuorum` reading, and the AGTI report-derived overlap extension separately labeled. Do not present a local RippleD quorum as proof of global UNL overlap, and do not compare Cobalt governance latency with XRPL payment latency.

Operational facts:

- The same-day live receipts are `.tih/cobalt-sibling-baseline-20260823.json`, `.tih/cobalt-live-evidence-20260823/fleet-result.json`, and the rewarded Section 4 packets named above. The actual fleet is six validators with canonical quorum five, split across EWR, AMS, and SGP; validator keys remain local to each validator.
- Vast API access was verified and showed no running validator machines. The latest Vultr refresh failed closed with HTTP 401 because this workstation's observed source address did not match the allowlist at that moment; do not invent current fleet state from that failure. Use the vault helper without printing secrets and record any refresh failure explicitly.
- The repository has a very large user-owned dirty/staged cleanup. Never reset, restore, or broadly stage it. Commit milestone files only with `git commit --only -- <exact paths>`. Do not use a normal repository-wide commit.
- Work locally in this thread. Do not spawn agents, extra terminals, tmux sessions, or external rewrite workers. Do not fabricate Task Node evidence; submit only committed code, actual command results, packet hashes, and verifier output.
