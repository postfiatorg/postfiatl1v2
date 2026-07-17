# Cobalt Adversarial Burndown

Status: active adversarial-hardening burndown
Date: 2026-05-19
Scope: controlled pre-testnet Cobalt governance hardening
Primary implementation reference: [cobalt-implementation.md](../governance/cobalt-implementation.md)
Execution burndown: [full-cobalt-burndown.md](full-cobalt-burndown.md)

## Goal

Prove the Cobalt governance path does not fall apart under the bad conditions
that are likely to appear even in a controlled testnet:

- validators equivocate;
- validators withhold votes or messages;
- validators replay stale governance evidence;
- validators collude inside an essential subset;
- operators are bribed or captured as a correlated group;
- trust graph updates are malicious or unsafe;
- network messages are delayed, dropped, duplicated, or reordered;
- validators crash or restart in the middle of governance;
- governance messages are malformed, oversized, or expensive to verify.

This burndown does not block on outside validators. Project-controlled machines,
VMs, and reused infrastructure are acceptable for these drills. The objective is
code correctness, deterministic replay, and fail-closed evidence for controlled
testnet launch.

## What Would Blow Up Cobalt

Cobalt is not dangerous because validators run on one machine or seven machines.
It is dangerous if the code accepts a bad trust graph, accepts stale governance
state, lets linked validators finalize contradictory amendments, or cannot make
progress after realistic Byzantine behavior.

The failures that matter are:

- **Safety split:** two linked local views can finalize contradictory governance
  amendments.
- **Unsafe graph activation:** a malicious trust graph becomes active even
  though linkage assumptions are false.
- **Stale replay:** an old registry root, trust graph root, certificate, or DABC
  replay bundle is accepted after a newer activation.
- **Transition race:** old and new validator sets are both accepted for the same
  post-activation governance or transaction-network decision.
- **Liveness grief:** a small colluding group can stop governance progress below
  the configured fault threshold.
- **Resource attack:** malformed or oversized Cobalt messages force excessive
  ML-DSA verification, memory use, or unbounded queues.
- **Crash inconsistency:** a validator restarts with enough lost local state to
  double-sign, replay an old vote, or forget an activated graph.

## Why This Is Hard

The likely reason a mature network would avoid turning on full Cobalt is that it
is not a switch. A single global validator list has one quorum story. Full
Cobalt has many local trust views, essential-subset thresholds, linkage
conditions, graph transitions, and replay obligations. Every governance change
must prove not only "enough validators signed" but "the right local trust view
accepted this under the active linked graph, and activation cannot split later."

That is solvable, but it needs an adversarial harness and launch gates. The
burndown below is the work to make that true for PostFiat.

## Existing Coverage

| Area | Current Status | Evidence |
| --- | --- | --- |
| Essential-subset parameter validation | Done | Invalid `t_S` / `q_S` combinations are rejected in `postfiat-consensus-cobalt`. |
| Linkedness and unsafe graph counterexamples | Done | `reports/testnet-cobalt-linkedness-checker/trust-graph-types-v0-20260518T155052Z/testnet-cobalt-linkedness-checker.json` |
| Linked contradictory support property check | Done | Small-graph support enumeration exists in the Cobalt crate tests. |
| Non-uniform certificate rejects wrong local view | Done | `reports/testnet-cobalt-nonuniform-certificate/nonuniform-certificate-v0-20260518T160659Z/testnet-cobalt-nonuniform-certificate.json` |
| Canonical evidence rejected in non-uniform mode | Done | `reports/testnet-cobalt-nonuniform-certificate/nonuniform-mode-gate-v0-20260518T160934Z/testnet-cobalt-nonuniform-mode-gate.json` |
| RBC conflicting accept evidence | Done | `reports/testnet-cobalt-rbc-nonuniform/rbc-conflict-evidence-v0-20260518T161814Z/testnet-cobalt-rbc-conflict-evidence.json` |
| ABBA same-sender equivocation evidence | Done | `reports/testnet-cobalt-abba-nonuniform/abba-equivocation-v0-20260518T205140Z/testnet-cobalt-abba-equivocation.json` |
| DABC replay bundle verification | Done | `reports/testnet-cobalt-dabc-nonuniform/dabc-replay-bundle-v0-20260518T164830Z/testnet-cobalt-dabc-replay-bundle.json` |
| Old/new transaction-network transition validation | Done | `reports/testnet-cobalt-trust-graph-transition/transaction-network-transition-v0-20260518T170758Z/testnet-cobalt-transaction-network-transition.json` |
| Post-suspend finality and restart evidence | Done | `reports/testnet-cobalt-full-nonuniform-remote-drill/full-cobalt-remote-v0-20260518T223730Z/testnet-cobalt-full-nonuniform-remote-drill.json` |
| Deterministic seven-validator adversarial harness | Done | `reports/testnet-cobalt-adversarial/adversarial-harness-v0-20260519T0228Z/testnet-cobalt-adversarial-harness.json` |
| Strict release/replay adversarial packet requirement | Done | `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-adversarial-v0-20260519T0236Z/testnet-cobalt-controlled-launch-gate.json` |
| Collusion threshold matrix | Done | `reports/testnet-cobalt-adversarial/collusion-threshold-v0-20260519T0308Z/testnet-cobalt-collusion-threshold.json` |
| Bribery/correlated capture model | Done | `reports/testnet-cobalt-adversarial/capture-model-v0-20260519T0320Z/testnet-cobalt-capture-model.json` |
| Trust graph poisoning | Done | `reports/testnet-cobalt-adversarial/trust-graph-poison-v0-20260519T0330Z/testnet-cobalt-trust-graph-poison.json` |
| Stale governance replay | Done | `reports/testnet-cobalt-adversarial/stale-replay-v0-20260519T0345Z/testnet-cobalt-stale-replay.json` |
| RBC Byzantine proposer/voters | Done | `reports/testnet-cobalt-adversarial/rbc-byzantine-v0-20260519T0358Z/testnet-cobalt-rbc-byzantine.json` |
| ABBA Byzantine senders | Done | `reports/testnet-cobalt-adversarial/abba-byzantine-v0-20260519T0410Z/testnet-cobalt-abba-byzantine.json` |
| MVBA/DABC invalid candidates | Done | `reports/testnet-cobalt-adversarial/dabc-invalid-candidates-v0-20260519T0425Z/testnet-cobalt-dabc-invalid-candidates.json` |
| Membership transition races | Done | `reports/testnet-cobalt-adversarial/membership-race-v0-20260519T0445Z/testnet-cobalt-membership-race.json` |
| Partitions and message disorder | Done | `reports/testnet-cobalt-adversarial/partition-simulation-v0-20260519T0505Z/testnet-cobalt-partition-simulation.json` |
| Crash/restart persistence replay | Done | `reports/testnet-cobalt-adversarial/crash-restart-v0-20260519T0525Z/testnet-cobalt-crash-restart.json` |
| Live process-kill and respawn | Done | `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json` |
| Resource and verification DoS | Done | `reports/testnet-cobalt-adversarial/resource-dos-v0-20260519T0545Z/testnet-cobalt-resource-dos.json` |
| Governance spam and amendment flood | Done | `reports/testnet-cobalt-adversarial/governance-spam-v0-20260519T0615Z/testnet-cobalt-governance-spam.json` |
| Parser and canonical payload fuzzing | Done | `reports/testnet-cobalt-adversarial/parser-payload-fuzz-v0-20260519T0645Z/testnet-cobalt-parser-payload-fuzz.json` |
| Long adversarial soak | Done | `reports/testnet-cobalt-adversarial/soak-v0-20260519T0715Z/testnet-cobalt-adversarial-soak.json` |

## Burndown

| ID | Priority | Status | Adversarial Condition | Required Work | Exit Artifact |
| --- | --- | --- | --- | --- | --- |
| COBALT-ADV-000 | P0 | Done | Inventory existing adversarial coverage. | Identify which bad-actor paths are already covered by linkedness, non-uniform certs, RBC, ABBA, DABC, transition, and remote outage evidence. | This document. |
| COBALT-ADV-010 | P0 | Done | Deterministic adversarial Cobalt harness. | Add a local harness that runs 7 validators with behavior scripts: honest, equivocate, withhold, delay, drop, duplicate, invalid signature, stale root, malformed payload, crash/restart. | `crates/consensus_cobalt/examples/cobalt_adversarial_harness.rs`, `scripts/testnet-cobalt-adversarial-harness`, and evidence `reports/testnet-cobalt-adversarial/adversarial-harness-v0-20260519T0228Z/testnet-cobalt-adversarial-harness.json`. |
| COBALT-ADV-011 | P0 | Done | Collusion inside essential subsets. | Enumerate faulty/captured validator sets up to threshold and just over threshold for G1. Prove under-threshold safety and produce explicit over-threshold break/capture evidence. | `crates/consensus_cobalt/examples/cobalt_collusion_threshold.rs`, `scripts/testnet-cobalt-collusion-threshold`, and evidence `reports/testnet-cobalt-adversarial/collusion-threshold-v0-20260519T0308Z/testnet-cobalt-collusion-threshold.json`. |
| COBALT-ADV-012 | P0 | Done | Bribery/correlation model. | Treat bribery as correlated validator capture by host/operator/funding label or arbitrary injected capture set. Compute whether captured validators can satisfy strong support or block liveness under each local view. | `crates/consensus_cobalt/examples/cobalt_capture_model.rs`, `scripts/testnet-cobalt-capture-model`, and evidence `reports/testnet-cobalt-adversarial/capture-model-v0-20260519T0320Z/testnet-cobalt-capture-model.json`. |
| COBALT-ADV-013 | P0 | Done | Malicious trust graph activation. | Try graph updates with unsafe linkage, invalid subset parameters, duplicate validators, missing validators, stale owner signatures, and hostile essential subsets. Must fail before activation with counterexample. | `crates/consensus_cobalt/examples/cobalt_trust_graph_poison.rs`, `scripts/testnet-cobalt-trust-graph-poison`, and evidence `reports/testnet-cobalt-adversarial/trust-graph-poison-v0-20260519T0330Z/testnet-cobalt-trust-graph-poison.json`. |
| COBALT-ADV-014 | P0 | Done | Stale governance replay. | Replay old G0/G1 roots, old registry roots, old non-uniform certificates, and old DABC replay bundles after activation. Verifier must reject every stale path. | `crates/consensus_cobalt/examples/cobalt_stale_replay.rs`, `scripts/testnet-cobalt-stale-replay`, and evidence `reports/testnet-cobalt-adversarial/stale-replay-v0-20260519T0345Z/testnet-cobalt-stale-replay.json`. |
| COBALT-ADV-015 | P0 | Done | RBC Byzantine proposer and voters. | Exercise double propose, conflicting echo, conflicting ready, conflicting accept, ready without valid trigger, accept without valid ready, duplicate messages, invalid signatures, and withheld ready messages. | `crates/consensus_cobalt/examples/cobalt_rbc_byzantine.rs`, `scripts/testnet-cobalt-rbc-byzantine`, and evidence `reports/testnet-cobalt-adversarial/rbc-byzantine-v0-20260519T0358Z/testnet-cobalt-rbc-byzantine.json`. |
| COBALT-ADV-016 | P0 | Done | ABBA Byzantine sender behavior. | Exercise init/aux/conf/finish equivocation, withheld messages, invalid signatures, bad rounds, conflicting finish values, deterministic coin misuse, and nonterminating sender paths. | `crates/consensus_cobalt/examples/cobalt_abba_byzantine.rs`, `scripts/testnet-cobalt-abba-byzantine`, and evidence `reports/testnet-cobalt-adversarial/abba-byzantine-v0-20260519T0410Z/testnet-cobalt-abba-byzantine.json`. |
| COBALT-ADV-017 | P0 | Done | MVBA/DABC invalid candidate paths. | Feed invalid RBC accepts, conflicting candidate ids, duplicate candidates, conflicting parent hashes, skipped slots, wrong activation heights, and mismatched payload hashes. | `crates/consensus_cobalt/examples/cobalt_dabc_invalid_candidates.rs`, `scripts/testnet-cobalt-dabc-invalid-candidates`, and evidence `reports/testnet-cobalt-adversarial/dabc-invalid-candidates-v0-20260519T0425Z/testnet-cobalt-dabc-invalid-candidates.json`. |
| COBALT-ADV-018 | P0 | Done | Membership transition race. | Attempt old-set signatures after activation, new-set signatures before activation, mixed old/new certificates, stale transaction-network ids, and block membership bound to the wrong graph root. | `crates/consensus_cobalt/examples/cobalt_membership_race.rs`, `scripts/testnet-cobalt-membership-race`, and evidence `reports/testnet-cobalt-adversarial/membership-race-v0-20260519T0445Z/testnet-cobalt-membership-race.json`. |
| COBALT-ADV-019 | P0 | Done | Network partitions and message disorder. | Deterministically simulate 3/4, 2/2/3, and single-validator-isolated partitions with delay, reorder, duplicate, and healed-partition replay. Safety must hold; liveness expectations must be explicit. | `crates/consensus_cobalt/examples/cobalt_partition_simulation.rs`, `scripts/testnet-cobalt-partition-simulation`, and evidence `reports/testnet-cobalt-adversarial/partition-simulation-v0-20260519T0505Z/testnet-cobalt-partition-simulation.json`. |
| COBALT-ADV-020 | P0 | Done | Crash/restart during governance. | Kill and restart validators during RBC, ABBA, MVBA, DABC, graph activation, validator suspension, and rollback. Restart must not double-sign, forget activation, or accept stale evidence. | Deterministic persistence/replay packet: `crates/consensus_cobalt/examples/cobalt_crash_restart.rs`, `scripts/testnet-cobalt-crash-restart`, and evidence `reports/testnet-cobalt-adversarial/crash-restart-v0-20260519T0525Z/testnet-cobalt-crash-restart.json`. Live process-kill coverage is now tracked separately under `COBALT-ADV-034`. |
| COBALT-ADV-021 | P0 | Done | Resource and verification DoS. | Bound Cobalt message sizes, signature counts, duplicate message retention, malformed payload parsing, ML-DSA verification fanout, and per-round memory growth. | `MAX_COBALT_SIGNATURE_HEX_LEN` bounds signature size before verification. `crates/consensus_cobalt/examples/cobalt_resource_dos.rs`, `scripts/testnet-cobalt-resource-dos`, and evidence `reports/testnet-cobalt-adversarial/resource-dos-v0-20260519T0545Z/testnet-cobalt-resource-dos.json` prove oversized signatures, malformed payloads, DABC pending-pair floods, DABC checkpoint floods, RBC duplicate floods, and ABBA duplicate equivocations fail closed or dedupe deterministically. |
| COBALT-ADV-022 | P0 | Done | Governance spam and amendment flood. | Submit many governance amendments, duplicate slots, future slots, and invalid parent chains. Verify rate/resource policy and deterministic rejection. | `MAX_MVBA_CANDIDATES_PER_SET` bounds MVBA valid-input candidate sets at 1024. `crates/consensus_cobalt/examples/cobalt_governance_spam.rs`, `scripts/testnet-cobalt-governance-spam`, and evidence `reports/testnet-cobalt-adversarial/governance-spam-v0-20260519T0615Z/testnet-cobalt-governance-spam.json` prove many under-bound amendments select deterministically while candidate floods, raw replay floods, duplicate amendment slots, future pending slots, and invalid parent chains fail closed. |
| COBALT-ADV-023 | P0 | Done | Release gate requires adversarial packet. | Add a controlled-testnet Cobalt adversarial evidence requirement to release/replay once the harness exists. This is a code-quality gate, not an external-operator gate. | Release/replay support `REQUIRE_COBALT_ADVERSARIAL_HARNESS=1`; strict controlled launch now generates and requires a fresh adversarial packet. Evidence: `reports/testnet-cobalt-controlled-launch-gate/controlled-launch-adversarial-v0-20260519T0236Z/testnet-cobalt-controlled-launch-gate.json`. |
| COBALT-ADV-030 | P1 | Done | Parser and canonical payload fuzzing. | Fuzz RBC/ABBA/MVBA/DABC message decoders, canonical signing payloads, trust graph JSON, replay bundles, and registry transition payloads. | `crates/consensus_cobalt/examples/cobalt_parser_payload_fuzz.rs`, `scripts/testnet-cobalt-parser-payload-fuzz`, and evidence `reports/testnet-cobalt-adversarial/parser-payload-fuzz-v0-20260519T0645Z/testnet-cobalt-parser-payload-fuzz.json` prove valid corpus roundtrips preserve canonical signing payloads, truncated JSON fails parsing, protocol-version type mutations fail parsing, tampered ids/bindings fail validation, and replay/transition ids recompute from parsed payloads. |
| COBALT-ADV-031 | P1 | Done | Long adversarial soak. | Run repeated adversarial governance rounds with random scheduled crashes, delayed messages, stale replays, and equivocation below threshold. | `ratify_dabc_amendment` now extends DABC chains beyond the second amendment by validating the previous ratification core without falsely requiring genesis sequence. `crates/consensus_cobalt/examples/cobalt_adversarial_soak.rs`, `scripts/testnet-cobalt-adversarial-soak`, and evidence `reports/testnet-cobalt-adversarial/soak-v0-20260519T0715Z/testnet-cobalt-adversarial-soak.json` prove 32 governance rounds with scheduled offline validators, delay/reorder/duplicate delivery, deterministic restart replay, stale replay rejection, below-threshold ABBA equivocation handling, and full DABC replay verification. |
| COBALT-ADV-032 | P1 | Done | Compromised-key recovery. | Drill key compromise, validator suspension, key rotation, reactivation, and stale compromised-key vote rejection through DABC. | `rotate_key` now supports inactive-to-inactive key replacement so a compromised validator can be suspended, rotated while out of the active set, then reactivated with replacement key material. `crates/consensus_cobalt/examples/cobalt_key_compromise_recovery.rs`, `scripts/testnet-cobalt-key-compromise-recovery`, and evidence `reports/testnet-cobalt-adversarial/key-compromise-recovery-v0-20260519T0730Z/testnet-cobalt-key-compromise-recovery.json` prove DABC-bound suspension, inactive key rotation, reactivation, stale compromised support rejection, tampered compromised vote rejection, old-key reactivation rejection, and post-reactivation proposer acceptance. |
| COBALT-ADV-033 | P1 | Done | Rollback under Byzantine activation attempt. | Attempt unsafe graph activation, prove rejection, then ratify rollback/recovery path and replay it offline. | `crates/consensus_cobalt/examples/cobalt_rollback_recovery.rs`, `scripts/testnet-cobalt-rollback-recovery`, and evidence `reports/testnet-cobalt-adversarial/rollback-recovery-v0-20260519T0750Z/testnet-cobalt-rollback-recovery.json` prove unsafe trust-view updates fail before activation, a Byzantine-forced unsafe graph has explicit unsafe linkage, rollback restores authority trust views, rollback is DABC-ratified, replay verifies after JSON roundtrip, and tampered rollback/wrong DABC payloads fail closed. |
| COBALT-ADV-034 | P1 | Done | Live process kill and respawn. | Start seven actual local validator child processes for one Cobalt RBC plus ABBA plus MVBA/DABC request, kill the delayed validator before waiting for the round, prove the remaining six child processes still accept under non-identical trust views, then respawn the killed validator and prove it accepts after restart. | `crates/consensus_cobalt/examples/cobalt_live_process_kill.rs`, `scripts/testnet-cobalt-live-process-kill`, and evidence `reports/testnet-cobalt-adversarial/live-process-kill-v7-20260519T1240Z/testnet-cobalt-live-process-kill.json` prove all seven child processes start before kill-wait, process death is non-successful, six online validators satisfy RBC support, finish the same ABBA value, select the same MVBA candidate, ratify the same DABC amendment, verify the same DABC replay bundle, produce no same-payload RBC or same-value ABBA conflict evidence, and the respawned validator repeats the DABC-aware path after restart. |
| COBALT-ADV-035 | P1 | Done | Live process-kill gate contract drift. | Self-test the live process-kill script predicate so stale or incomplete evidence cannot silently satisfy the standard check path. | `scripts/testnet-cobalt-live-process-kill --self-test` accepts a valid DABC-aware packet shape and rejects missing concurrency, MVBA, DABC ratification, and restart replay evidence. `scripts/check` now runs it. Evidence: `reports/testnet-cobalt-adversarial/live-process-kill-self-test-v2-20260519T1240Z/testnet-cobalt-live-process-kill-self-test.json`. |
| COBALT-ADV-036 | P1 | Done | Standard check exercises live process-kill. | Run the actual seven-child-process kill/respawn drill in the standard check path so local green checks prove live behavior, not only predicate shape. | `scripts/check` now runs `scripts/testnet-cobalt-live-process-kill` after the self-test. Evidence: `reports/testnet-cobalt-adversarial/live-process-kill-standard-check-v0-20260519T1305Z/testnet-cobalt-live-process-kill.json` and `reports/testnet-cobalt-adversarial/live-process-kill-self-test-standard-v0-20260519T1305Z/testnet-cobalt-live-process-kill-self-test.json`. |

## Controlled Launch Gate

For controlled testnet, the launch gate should not require outside operators.
It should require:

- non-identical trust views;
- linkedness and unsafe-graph rejection;
- non-uniform certificate checks;
- RBC/ABBA/MVBA/DABC evidence;
- stale replay rejection;
- membership transition race rejection;
- crash/restart evidence;
- live process-kill and respawn evidence;
- resource bounds for malformed or oversized Cobalt messages;
- a current adversarial harness packet in strict controlled launch.

Independent public operator diversity is a separate claims topic and is not a
condition for this adversarial burndown.

## Next Implementation Slice

All listed adversarial Cobalt packets are implemented through live process-kill
and respawn. Strict Cobalt release/replay/controlled-launch/readiness gates now
support requiring the full adversarial packet set with
`REQUIRE_COBALT_ADVERSARIAL_PACKET_SET=1`. Continue with the next open item in
`docs/status/full-cobalt-burndown.md`.
