# Private Egress Consensus Performance Plan

Date: 2026-06-23
Repo: `postfiatl1v2`
Scope: controlled-testnet performance work for `AssetOrchardPrivateEgressV1` inside the current peer-certified shielded batch path.

## Progress Tracker

Last updated: 2026-06-23 UTC

Current phase: **Complete - selected phases landed and gated**.

| Phase | Status | Evidence | Next Gate |
|---|---|---|---|
| Phase 1A - deterministic timing instrumentation | Complete | Commit `87eeba95` (`Instrument private egress certified round timings`); `cargo check` passed on 2026-06-23. | Run one instrumented private-egress certified round. |
| Phase 1B - instrumented evidence run | Complete | Controlled release run report at `reports/testnet-asset-orchard-private-egress-peer-certified/phase1b-release-20260623T035301Z/logs/round-reports/round-6.peer-certified-round.json`; `round_ok: true`; four validators converged at height `6` with state root `2e7f45cefcc45ce3cab834ac2c3b2449df2cd7de5eb3dbd432bc6c46794f53f4687426d26b48f5d39071d8469d754c7b`. | Record attribution and select evidence-supported phases. |
| Phase 1C - attribution and decision record | Complete | The report shows proposal and remote vote time are dominated by cold private-egress verifying-key build; `keygen_vk(...)` accounts for about `316-319s` per proposer/validator while Halo2 proof verification is only about `38-58ms`. | Execute selected phases without changing consensus safety. |
| Phase 2 - warm verifier material | Complete | Warm run at `reports/testnet-asset-orchard-private-egress-peer-certified/phase2-warm-20260623T043350Z/logs/round-reports/round-6.peer-certified-round.json`; proposal validation hit `cache_was_populated: true`, `build_triggered: false`; proposal `70.967ms`; remote vote requests `107.590ms`; manual `verify-state` and `verify-shielded` passed on all four validators. | Move remaining user-visible cold setup out of one-shot processes or remove it. |
| Phase 3 - resident validators/coordinator | Complete | Resident private-egress coordinator command added as `transport-peer-certified-private-egress-loop`; full release harness report at `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-resident-egress-20260623T053328Z/report.json` passed with `private_egress_resident_loop_ok: true`; loop readiness at `logs/round-reports/round-6.private-egress-loop-ready.json`; loop report at `logs/round-reports/round-6.private-egress-loop.json`; nested certified round at `logs/round-reports/round-6.peer-certified-round.json`. | Remove cold verifier-key generation cost instead of only moving it before queue processing. |
| Phase 4 - pinned verifier-key loading | Complete | Upstream `halo2_proofs 0.3.2` plus a narrow compatibility patch for pinned verifying-key assembly loading and an embedded artifact at `crates/privacy_orchard/artifacts/asset_orchard_private_egress_vk_pinned_assembly.v1.bin`; full release run `reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-private-only-20260623T071447Z/report.json`; ready prewarm `artifact_mode: embedded`, `keygen_vk_ms: 0.0`, private-egress prewarm `8005.144ms`, hot certified round `426.392ms`, and no hot-round VK builds. | Keep the pinned artifact covered by the Phase 6 regression gate. |
| Phase 5 - strict verified-proposal cache | Deferred | The evidence does not show duplicate identical proposal checks or retries; each validator performs one required local validation. | Reopen only if retries/duplicates appear in later reports. |
| Phase 6 - benchmark and regression gate | Complete | `scripts/private-egress-pinned-vk-regression-gate` passed on the two Phase 4 reports; p95 hot round `426.392ms`, proposal `74.367ms`, vote requests `115.502ms`, local apply `66.423ms`, proof verify `91.560ms`, private-egress prewarm `8005.144ms`; `keygen_vk_ms` remained `0.0` and all 21 hot-round `vk_builds` fields were empty in both samples. | Keep this gate in the controlled-testnet release checklist and tighten thresholds after more samples. |

Phase status vocabulary:

- `Complete`: implementation/evidence for that phase is done and referenced in this document.
- `In progress`: this is the active phase being worked.
- `Pending`: the phase is required but blocked by an earlier gate.
- `Selected`: Phase 1C evidence supports implementation.
- `Deferred`: do not implement unless later evidence reopens the phase.

## Executive Summary

The StakeHub shielded NAV swap demo now proves the end-to-end privacy path: public `pfUSDC` enters Asset-Orchard, swaps privately into `a651`, exits through direct private egress, burns through the vault bridge, withdraws on Arbitrum, and settles back on PFTL.

The private-egress step is functionally correct but too slow for a user-facing flow. The live run certified private egress in `12m49.141s`. The payload was small: one shielded action, about `15 KB` of batch JSON, and a `6,848`-byte private-egress proof.

The measured delay is concentrated in:

- proposal construction: `5m25.402s`;
- remote validator vote requests: `7m23.333s`.

Phase 1 instrumentation identified the bottleneck. Cold verifier setup dominates proposal and remote vote validation, and `keygen_vk(...)` dominates that setup. Once verifier material is warm, the controlled Phase 2 run certifies the same private-egress fixture with proposal construction in `70.967ms` and remote vote requests in `107.590ms`.

Phase 2 is complete for resident warm validators and the warm peer-certified validation path. Phase 3 is complete for resident coordinator/batch handling: the private-egress coordinator now warms, writes a readiness file, waits for queued egress actions, wraps them into shielded batches inside the same warm process, and certifies through the existing peer-certified round path.

The Phase 3 full release run proves the user-visible consensus segment is no longer the `12m49.141s` cold path once the coordinator and validators are warm. The resident loop paid `335.613s` before queue processing, then wrapped the queued private-egress file in `67.527ms` and certified the nested round in `367.519ms` with proposal construction at `74.945ms` and remote vote requests at `87.972ms`.

Phase 4 is complete. Warm-up still initializes Halo2 parameters, but the multi-minute `keygen_vk(...)` cost is gone from the private-egress verifier load path. The repo uses upstream `halo2_proofs 0.3.2` at a pinned commit with a narrow compatibility API for pinned verifying-key assembly loading; it does not replace the Halo2 proof system. The path embeds a release-pinned private-egress verifier artifact, validates artifact metadata fail-closed, and reports `keygen_vk_ms: 0.0` when loading the pinned artifact.

Phase 6 is complete for the current controlled-testnet gate. The regression gate reads existing peer-certified reports, checks that pinned verifier loading is used, checks that no hot certified round rebuilds verifier keys, and reports p50/p95/p99 over the supplied samples.

## Plain-English Consensus Model

This repo currently uses a controlled peer-certified batch round for live devnet finality. It is a Byzantine-fault-tolerant certification flow over the active validator set.

In plain English:

1. A client or operator creates a batch of actions. In this case the batch is a shielded batch containing `AssetOrchardPrivateEgressV1`.
2. A proposer builds a candidate block proposal from the current local chain state plus that batch. For shielded batches, this means executing the shielded action against a trial copy of the ledger and shielded state.
3. The proposer signs the proposal. The signed proposal contains the block height, parent hash, batch id, payload hash, receipt ids, state root, and other evidence fields.
4. Validators receive the proposal and batch. They do not trust the proposer blindly. Each validator checks that the proposal matches its own current state and the supplied batch. For shielded private egress, that includes validating the private-egress action and its zero-knowledge proof.
5. A validator signs a vote only if the proposal is valid for its local state and validator registry view.
6. The proposer or coordinator aggregates enough validator votes into a block certificate. In the six-validator live run, the certificate carried five validator votes.
7. Once the certificate exists, nodes can apply the certified block and advance height. The certificate is the finality evidence for that block.

The important safety property is that a proposer signature is not enough. The system relies on independent validator re-execution before votes are signed. This plan must preserve that rule. Optimizations may cache static verifier material, add measured fast paths, or reuse already-verified evidence under strict keys, but they must not turn "the proposer said it was valid" into consensus.

## Current Private-Egress Path

Private egress is represented by `AssetOrchardPrivateEgressV1`.

Relevant code paths:

- Action payload type: `crates/types/src/lib_parts/shielded_bridge_governance.rs`
- CLI creation and batch wrapping: `crates/node/src/main_parts/cli_dispatch_parts/group_05.rs`
- Local action creation: `crates/node/src/privacy_parts/part_01.rs`
- Consensus-facing private-egress apply: `crates/node/src/privacy_parts/part_02.rs`
- Shielded batch execution dispatch: `crates/node/src/lib_parts/part_02_parts/part_03.rs`
- Private-egress proof verification: `crates/privacy_orchard/src/verify.rs`
- Private-egress Halo2 verifying/proving key cache: `crates/privacy_orchard/src/asset_orchard_circuit.rs`
- Proposal construction: `crates/node/src/lib_parts/part_01.rs`
- Remote vote validation: `crates/node/src/block_finality.rs`
- Peer-certified transport round: `crates/node/src/transport_cli.rs`

The consensus-relevant verification call is `verify_serialized_asset_orchard_private_egress_action`. It validates the action, authorizing domain, asset tag, exit binding hash, spend authorization, public instance, and Halo2 proof before returning the verified nullifier and public exit facts used by state transition code.

## Live Performance Evidence

Live StakeHub private-egress run:

- Run directory: `$STAKEHUB_STATE/shielded-nav-swap/private-egress/stakehub-private-egress-20260623T001342Z-47b6b1a2`
- Receipt: `private-egress-receipt.json`
- Status: `private_egressed`
- Proof bytes: `6,848`
- Batch size: about `15 KB`
- Certified at PFTL height: `269`
- Public exit amount: `1`
- Payload privacy scan: `ok: true`

Measured timing from the certification report:

| Segment | Time |
|---|---:|
| Total certified private-egress round | `12m49.141s` |
| Proposal construction | `5m25.402s` |
| Remote validator vote requests | `7m23.333s` |
| Local vote | `0.017s` |
| Certificate assembly | `0.196s` |
| Local apply | `0.206s` |
| Deferred certified sends | `0.002s` |

Remote validator vote timings:

| Validator | Time |
|---|---:|
| `validator-4` | `6m33.599s` |
| `validator-1` | `6m50.656s` |
| `validator-3` | `6m50.807s` |
| `validator-5` | `7m23.332s` |

Interpretation:

- The dominant measured segments are proposal construction and remote vote validation.
- The batch and proof are small, so raw bandwidth volume is unlikely to explain the delay.
- There were no successful-path transport retries in the reported vote requests, so retry backoff is unlikely to explain the delay.
- EVM withdrawal happens later, so the slow certified-round segment is not Arbitrum settlement.
- Proposal construction and remote voting both invoke consensus/private-egress validation paths.
- Exact substep attribution was unknown from the live report alone; the Phase 1B report below attributes it to cold verifier setup and `keygen_vk(...)`.

## Phase 1B Instrumented Evidence

Controlled local release run:

- Harness: `scripts/testnet-asset-orchard-private-egress-peer-certified-smoke`
- Run directory: `reports/testnet-asset-orchard-private-egress-peer-certified/phase1b-release-20260623T035301Z`
- Round report: `logs/round-reports/round-6.peer-certified-round.json`
- Status: `round_ok: true`
- Validator set: four controlled local validators
- Certified height: `6`
- Final converged state root: `2e7f45cefcc45ce3cab834ac2c3b2449df2cd7de5eb3dbd432bc6c46794f53f4687426d26b48f5d39071d8469d754c7b`

The run was manually resumed after the action-create and batch-wrap stages exceeded earlier harness timeouts. That does not invalidate the certified-round timing report; the round itself completed with the instrumented peer-certified path and converged across all four validators.

Instrumented round timing:

| Segment | Time |
|---|---:|
| Total peer-certified round | `653.735s` |
| Proposal construction | `326.011s` |
| Remote validator vote requests | `327.487s` |
| Local vote | `0.022s` |
| Local apply | `0.051s` |

Proposal attribution:

| Proposal substep | Time |
|---|---:|
| State execution | `326.004s` |
| Private-egress verifier call | `325.994s` |
| Private-egress verifying-key build | `325.931s` |
| `keygen_vk(...)` | `318.519s` |
| `Params::new(...)` | `7.412s` |
| Halo2 `verify_proof` | `0.058s` |
| Shielded state verification after apply | `0.009s` |

Remote validator attribution:

| Validator | Vote request | Verifying-key build | `keygen_vk(...)` | Halo2 `verify_proof` | Process spawn |
|---|---:|---:|---:|---:|---:|
| `validator-0` | `326.424s` | `326.344s` | `316.624s` | `0.052s` | `0.000s` |
| `validator-1` | `327.486s` | `327.419s` | `316.999s` | `0.044s` | `0.000s` |
| `validator-3` | `327.320s` | `327.241s` | `317.072s` | `0.038s` | `0.000s` |

Interpretation:

- The certified round is slow because every proposer/validator process pays a cold private-egress verifying-key build before validating one action.
- The root cost inside that build is `keygen_vk(...)`, not proof verification, state cloning, ML-DSA signing, JSON serialization, disk I/O, or transport.
- The private-egress proof verification itself is already tens of milliseconds once verifier material exists.
- Resident validator services were used for remote votes, but they had not prewarmed private-egress verifier material, so each reported `cache_was_populated: false` and `build_triggered: true`.
- The proposer/coordinator path still behaves like a one-shot process for this round; unless that process is resident before the user-visible click, it also starts cold.

## Current Attribution Status

Known from the live and instrumented reports:

- The delay is concentrated in `proposal_ms` and remote vote request timings.
- Local vote, certificate assembly, local apply, and deferred certified sends are not material in this run.
- The instrumented evidence distinguishes the hot path: cold private-egress verifier setup dominates, and `keygen_vk(...)` dominates verifier setup.
- Transport read/write time in the requester is waiting for remote validator work, not network payload volume.
- Vote-lock reservation, ML-DSA signing, JSON serialization, vote file writes, retained-anchor checks, nullifier checks, state clone/apply, and shielded state verification are all small compared with cold verifier setup.

Candidate explanations now classified:

1. Cold private-egress verifier setup is on the hot path.
2. Verifier setup is repeated once per cold process because process-local cache starts empty.
3. Private-egress Halo2 proof verification itself is not the bottleneck in this report.
4. Trial-state cloning, shielded-state apply, retained-anchor checks, nullifier checks, and ledger validation do not dominate.
5. Vote-lock I/O, ML-DSA signing, JSON/file orchestration, process spawn, and transport serialization do not dominate this report.
6. One-shot or not-yet-warmed processes discard or lack process-local caches and therefore pay the cold build.
7. Duplicate vote requests or retries were not observed.

Phase 1 has classified these. Phase 2, Phase 3, Phase 4, and Phase 6 are selected. Phase 5 is deferred.

### Phase 1C Decision Record

Decision date: 2026-06-23.

Selected:

- Phase 2, because warm private-egress verifier material should remove the multi-minute cold build from resident validator vote handling.
- Phase 3, because the proposer/coordinator still needs a resident or pre-started process if the user-visible round must avoid first-use verifier setup.
- Phase 4, because the cold build cost is specifically `keygen_vk(...)`; warm-up moves the cost out of the click path but does not remove the operational cost.
- Phase 6, because cold/warm performance must become a regression gate after the selected fixes land.

Deferred:

- Phase 5, because this report does not show duplicate identical proposal validation, retries, or cacheable repeated checks.

Not selected as primary fixes:

- Proof-system optimization for `verify_proof`, because actual Halo2 proof verification is under `0.060s` in this report.
- State clone/apply optimization, because non-verifier state work is milliseconds to low tens of milliseconds.
- Transport optimization, because the requester is blocked waiting for validator verification rather than moving large payloads.

## Phase 2 Warm-Path Evidence

Controlled local release run:

- Harness: `scripts/testnet-asset-orchard-private-egress-peer-certified-smoke`
- Run directory: `reports/testnet-asset-orchard-private-egress-peer-certified/phase2-warm-20260623T043350Z`
- Round report: `logs/round-reports/round-6.peer-certified-round.json`
- Service reports: `logs/validator-*.validator-serve.json`
- Status: `round_ok: true`
- Certified height: `6`
- Final converged state root: `02365a8cc7f446c245f51f40fae92716d0ad79d7b88741135d6e4c39ec2ae1147b7b2118d2c6f73160612a84bdc44c94`

The first Phase 2 harness invocation reached the final private-egress round and produced the required round and service reports, then failed during post-run `verify-state` because the harness still used a `120s` timeout for a cold replay check. The harness timeout was raised to `900s`. Manual replay checks against the completed run passed:

- `verify-state`: `verified: true`, `block_log: true`, `shielded: true`, `mempool: true` on validators `0`, `1`, `2`, and `3`.
- `verify-shielded`: `verified: true` on validators `0`, `1`, `2`, and `3`.
- `status`: all four validators at height `6` with the same state root and block tip.

Warm round timing:

| Segment | Time |
|---|---:|
| Total peer-certified command | `336.304s` |
| Explicit verifier prewarm | `335.925s` |
| Proposal construction after prewarm | `0.071s` |
| Remote validator vote requests after prewarm | `0.108s` |
| Local vote | `0.017s` |
| Local apply | `0.052s` |

Prewarm attribution:

| Prewarm substep | Time |
|---|---:|
| Swap verifier warm-up | `326.913s` |
| Private-egress verifier warm-up after swap verifier | `9.011s` |

Warm-cache proof:

| Path | `cache_was_populated` | `build_triggered` | Cached lookup | Halo2 `verify_proof` |
|---|---:|---:|---:|---:|
| Proposer private-egress validation | `true` | `false` | `0.000071ms` | `52.784ms` |
| `validator-0` vote validation | `true` | `false` | `0.000331ms` | `74.008ms` |
| `validator-1` vote validation | `true` | `false` | `0.000351ms` | `81.109ms` |
| `validator-3` vote validation | `true` | `false` | `0.000300ms` | `84.251ms` |

Validator service prewarm:

| Validator | Prewarm status | Prewarm time | Accepted votes | Accepted certified batches | Rejections |
|---|---:|---:|---:|---:|---:|
| `validator-0` | `true` | `342.528s` | `5` | `5` | `0` |
| `validator-1` | `true` | `343.723s` | `4` | `4` | `0` |
| `validator-2` | `true` | `343.356s` | `4` | `4` | `0` |
| `validator-3` | `true` | `344.492s` | `5` | `5` | `0` |

Interpretation:

- Phase 2 succeeds for the hot consensus path: once the verifier cache is populated, proposal construction and all remote vote requests are sub-second for this private-egress fixture.
- Validator safety semantics are unchanged. Validators still rebuild/compare the proposal against local state and run local proof verification before voting.
- Phase 3 moved the coordinator/batch-wrapper cold setup before queue processing. The user-visible certified round is now sub-second once the resident coordinator and validators are warm.
- Phase 4 remains active because pinned verifier material is the direct way to remove the cold `keygen_vk(...)` cost instead of merely moving it earlier.

## Non-Negotiable Safety Constraints

These constraints are required because this code touches consensus and validator signatures:

- Do not skip validator re-execution of a proposed shielded batch.
- Do not accept a proposer-signed private-egress proposal without local proof and state validation.
- Do not cache a verification result across different chain id, genesis hash, protocol version, block height, parent hash, batch id, payload hash, proposal hash, verifier metadata, or current state root.
- Do not make state-affecting behavior depend on wall-clock timing, process randomness, thread scheduling, or nondeterministic map iteration.
- Do not turn performance instrumentation into consensus input. Timings are observability only.
- Do not weaken proof verification, spend authorization verification, nullifier checks, retained-anchor checks, or public-exit binding checks.

## Improvement Plan

### Phase 1: Add Deterministic Timing Evidence

Goal: prove exactly where the `12m49.141s` is spent before changing behavior.

Add timing spans or report fields around:

- `AssetOrchardPrivateEgressVerifyingKey::cached()`
- `AssetOrchardPrivateEgressVerifyingKey::build()`
- Halo2 `Params::new(...)`
- Halo2 `keygen_vk(...)`
- private-egress metadata pin validation
- private-egress public instance construction
- private-egress Halo2 `verify_proof`
- private-egress spend authorization verification
- retained-anchor and nullifier checks
- state clone/trial-state creation
- `apply_asset_orchard_private_egress_action_to_state`
- shielded batch proposal construction
- `block_vote_target` proposal rebuild and comparison
- vote-lock reservation
- ML-DSA vote signing
- JSON serialization/deserialization
- transport request read/write latency
- process startup/spawn time where one-shot tooling is still used

Output must appear in the existing JSON reports for peer-certified rounds, not only in logs. A reader should be able to inspect one report and see whether time was spent in verifier setup, proof verification, state execution, disk, signing, process startup, serialization, or network.

Acceptance criteria:

- A private-egress round report breaks down proposal and vote time into verifier setup, proof verify, state apply, vote lock, signature, serialization, process, and transport substeps.
- Reported substeps sum within a small tolerance of existing `proposal_ms` and per-validator vote request timings.
- The added instrumentation does not affect block hashes, proposal hashes, state roots, receipts, or certificates.
- The Phase 1 report identifies the top contributors and records which, if any, of Phases 2-6 are justified.
- No post-gate optimization is treated as committed until this evidence exists.

### Phase 1 Decision Gate

After Phase 1, write a short decision record from the instrumented report.

Gate rules:

- If verifier setup or first-use parameter initialization dominates, prioritize Phase 2 and consider Phase 4.
- If process churn or lost process-local caches dominate, prioritize Phase 3.
- If `keygen_vk(...)` or verifier-material rebuild dominates after warm-up, prioritize Phase 4.
- If duplicate identical proposal checks are observed, prioritize Phase 5.
- If proof verification itself dominates after setup is excluded, open a targeted verifier/circuit investigation; do not assume warm-up will fix it.
- If state clone/apply, disk I/O, vote-locking, signing, JSON serialization, or transport dominates, retarget the plan to that bottleneck before implementing verifier-specific work.
- If the evidence does not support a candidate phase, defer or drop that phase.

### Conditional Phase 2: Warm Private-Egress Verifier Material

Condition: selected by Phase 1C. First-use private-egress verifier setup contributes about `326s` per cold proposer/validator process, and `keygen_vk(...)` accounts for about `317s` of that.

Goal: remove confirmed first-use verifier setup from the user-visible path.

Add an explicit warm-up command or startup hook that calls:

- `AssetOrchardPrivateEgressVerifyingKey::cached()`
- required static Sinsemilla/Poseidon/Merkle parameter initialization
- optional proof verification against a tiny pinned fixture if one exists or is added for this purpose

This warm-up must run on every validator process that may sign private-egress votes, not only on the StakeHub web process. It must not replace local proof verification during voting.

Implementation status:

- Complete: the existing `POSTFIAT_PREWARM_SHIELDED_VERIFIER` transport startup hook also warms `AssetOrchardPrivateEgressVerifyingKey::cached()`.
- Complete: `shielded_verifier_prewarm` JSON report output is emitted by `transport-block-vote-listen`, `transport-validator-serve`, and `transport-peer-certified-batch-round`.
- Complete: the private-egress peer-certified smoke harness validates nested private-egress timing fields and warm-cache evidence.
- Complete: Phase 2 warm evidence shows `cache_was_populated: true` and `build_triggered: false` during proposer and remote validator private-egress validation.

Acceptance criteria:

- Validator startup or preflight can report `asset_orchard_private_egress_verifier_warm: true`.
- A second private-egress validation in the same process does not rebuild the Halo2 verifying key.
- Cold and warm timings are both recorded.
- Cold and warm paths produce identical proposal hashes, receipt ids, state roots, and certificates for the same fixture.

### Conditional Phase 3: Keep Validators Resident for Shielded Rounds

Condition: selected by Phase 1C for the proposer/coordinator path. Remote validator services were resident in the instrumented run, but all private-egress verifier caches were cold. Phase 3 is complete because the coordinator can now start, prewarm, write readiness evidence, wait for queued private-egress actions, wrap them into shielded batches, and certify them through the existing peer-certified round path without changing consensus semantics.

Goal: preserve valid process-local caches between consensus rounds.

Implementation direction:

- Keep the block-vote listener running across rounds.
- Keep private-egress verifier caches alive in that process.
- Avoid spawning one-shot binaries for proposal and vote paths in the user flow when a resident service can do the same deterministic work.
- Continue writing the same evidence files for auditability.

Implementation status:

- Complete: resident validator services can prewarm and report private-egress verifier material through `shielded_verifier_prewarm`.
- Complete: `transport-peer-certified-batch-loop` now calls the same shielded verifier prewarm hook at loop startup, before polling `--batch-dir`.
- Complete: the loop report now includes `shielded_verifier_prewarm`, so operator wrappers such as `scripts/node-run-peer-certified` can prove coordinator readiness before a user-visible batch arrives.
- Evidence: `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-loop-prewarm-20260623T051256Z/peer-certified-batch-loop-idle-prewarm.json` shows the resident coordinator paid `334.296s` of startup prewarm, warmed the swap verifier in `325.312s`, warmed the private-egress verifier in `8.984s`, and then shut down by `idle_timeout` with `processed_round_count: 0`.
- Complete: `transport-peer-certified-private-egress-loop` wraps queued private-egress JSON into a shielded batch inside the same warmed coordinator process and immediately delegates certification to `transport_peer_certified_batch_round`.
- Complete: the private-egress loop has an optional `--ready-file` so operators and harnesses can prove warm coordinator readiness before an egress file enters the queue.
- Complete: `scripts/testnet-asset-orchard-private-egress-peer-certified-smoke` now starts the resident private-egress loop for round 6, waits for the warm ready file, writes the private-egress action into the queue, and validates the nested certified round report.
- Evidence: full release harness `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-resident-egress-20260623T053328Z/report.json` passed with `asset_orchard_private_egress_peer_certified_ok: true`, `private_egress_resident_loop_ok: true`, `validator_services_verified: true`, `verified_all: true`, final height `6`, and final state root `e9ed8989a6a3f8f3e09fc7ca97cd5614d5ac06d9cebdb9db574728c75e336081b9889ef1affbb9f905fc93896f1287ad`.
- Evidence: resident loop readiness at `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-resident-egress-20260623T053328Z/logs/round-reports/round-6.private-egress-loop-ready.json` shows `ready: true`, total coordinator startup prewarm `335.613s`, swap verifier warm `true` in `326.317s`, and private-egress verifier warm `true` in `9.296s`.
- Evidence: resident loop report at `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-resident-egress-20260623T053328Z/logs/round-reports/round-6.private-egress-loop.json` shows one queued egress file processed, `batch_wrap_ms: 67.527`, archived egress and batch files, `loop_ok: true`, and nested `round_ok: true`.
- Evidence: nested private-egress certified round `reports/testnet-asset-orchard-private-egress-peer-certified/phase3-resident-egress-20260623T053328Z/logs/round-reports/round-6.peer-certified-round.json` shows `round_ok: true`, total `367.519ms`, proposal `74.945ms`, remote vote requests `87.972ms`, local apply `61.104ms`, prewarm requested `true`, private-egress verifier warm `true`, and no verifier-key builds in proposal/vote validation.

Acceptance criteria:

- Complete: private-egress vote requests handled by resident validators report warm verifier state and `build_triggered: false`.
- Complete: restarting a validator returns to cold status and warms again, shown by Phase 2/3 startup prewarm reports.
- Complete for resident path: the resident loop preserves the existing proposal/vote/apply functions and produced a converged four-validator state root with full `verify-state` and `verify-shielded` harness checks.
- Complete: validators still refuse to sign except after local proposal rebuild/compare, local proof verification, registry checks, vote-lock reservation, and ML-DSA signing; Phase 3 changes only queue orchestration and observability.

### Conditional Phase 4: Load Pinned Verifier Keys Instead of Rebuilding Them

Condition: selected by Phase 1C. The cold bottleneck is `keygen_vk(...)`, which took about `316-319s` in every cold proposer/validator process in the instrumented run.

Goal: avoid expensive `keygen_vk` work in production validator hot paths.

Implementation status:

- Complete: Phase 4A dependency audit found that current `halo2_proofs 0.3.2` exposes `Params::read` and `Params::write`, but `plonk::VerifyingKey` has private fields and no public deserialization or `from_parts` constructor.
- Complete: the workspace retains the pinned upstream `halo2_proofs 0.3.2` source in `third_party/halo2_proofs` and applies a narrow pinned verifying-key assembly compatibility API for this verifier path. This is not a Halo2 reimplementation. The patch serializes the assembly commitments and selector layout directly; it does not parse `Debug` output, use `unsafe`, depend on layout transmutation, or intentionally change the proof algorithm or verifier equations.
- Complete: `AssetOrchardPrivateEgressVerifyingKey::build` loads the embedded release artifact at `crates/privacy_orchard/artifacts/asset_orchard_private_egress_vk_pinned_assembly.v1.bin` by default, unless `POSTFIAT_ASSET_ORCHARD_PRIVATE_EGRESS_VK_REBUILD=1` explicitly requests development rebuild mode.
- Complete: artifact loading validates schema, Halo2 version, curve, proof system id, circuit id, `k`, public instance layout hash, parameter hash, verifier-key attestation hash, runtime pinned VK fingerprint, Poseidon parameter hash, note message layout hash, Merkle parameter hash, payload length, and payload hash before reconstructing the verifier.
- Complete: corrupted or mismatched external artifacts fail closed before a verifier is accepted.
- Complete: the private-egress smoke harness can prewarm only the private-egress verifier through `POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=0` and `POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1`, so private-egress startup measurements no longer include unrelated swap-verifier warm-up.
- Evidence: `cargo test -p postfiat-privacy-orchard private_egress_vk_artifact_tamper_fails_closed -- --nocapture` passed.
- Evidence: `cargo test -p postfiat-privacy-orchard --release private_egress_embedded_vk_artifact_loads_and_matches_release_pin -- --nocapture` passed.
- Evidence: `cargo check -p postfiat-node` passed after the pinned-artifact implementation.
- Evidence: full release harness `reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-private-only-20260623T071447Z/report.json` passed with `asset_orchard_private_egress_peer_certified_ok: true`, `private_egress_resident_loop_ok: true`, `validator_services_verified: true`, and final height `6`.
- Evidence: readiness report `reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-private-only-20260623T071447Z/logs/round-reports/round-6.private-egress-loop-ready.json` shows `artifact_mode: embedded`, `keygen_vk_ms: 0.0`, `full_shape_ms: 0.0`, `artifact_decode_ms: 6.935`, `artifact_vk_reconstruct_ms: 16.794`, `params_new_ms: 7978.686`, and private-egress prewarm `8005.144ms`.
- Evidence: nested certified round `reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-private-only-20260623T071447Z/logs/round-reports/round-6.peer-certified-round.json` shows `round_ok: true`, total `426.392ms`, proposal `73.770ms`, remote vote requests `115.502ms`, local apply `66.423ms`, and no verifier-key builds in proposal/vote validation.
- Observation: `Params::new` is still about `7.98s` in the private-egress prewarm path. That is now an operational warm-up cost, not a hot certified-round cost and not the old multi-minute `keygen_vk(...)` bottleneck.
- Constraint: do not implement a loader by parsing `Debug` output, using `unsafe` layout assumptions, or bypassing Halo2's transcript representation. That would make verifier behavior brittle and unauditable.
- Selected Phase 4 implementation path: retain the exact pinned upstream `halo2_proofs` source with a small verifying-key serialization/deserialization compatibility API for the structures needed by this circuit, generate a release artifact through that toolchain, and load it only after checking the pinned metadata listed below. The local patch is a cryptographic review boundary even though it does not introduce a new proof system.

The private-egress verifier now loads a pinned release artifact directly while still validating:

- circuit id
- proof system id
- `k`
- public instance layout hash
- parameter hash
- verifying-key attestation hash
- runtime pinned VK fingerprint
- Poseidon parameter hash
- note message layout hash
- Merkle parameter hash

This changes performance, not consensus semantics. Every validator must load the same pinned verifier material for the same protocol version.

Acceptance criteria:

- Complete: release builds load the private-egress verifying key without running `keygen_vk` on the hot path; the readiness report records `keygen_vk_ms: 0.0`.
- Complete: a corrupted verifier artifact fails closed before any vote is signed; the tamper test passed.
- Complete: tests prove loaded verifier metadata matches the release pins.
- Complete for the current development gate: the pinned artifact is generated from the rebuilt full-shape verifier path, and the embedded artifact path verifies the controlled private-egress fixture in the Phase 4 release harness.

### Conditional Phase 5: Add a Strict Verified-Proposal Cache

Condition: deferred by Phase 1C. The instrumented run did not show duplicate identical proposal validation, retries, or local rechecks that would justify this cache yet.

Goal: avoid repeating identical expensive proposal checks after the same validator has already fully verified the same proposal in the same state context.

The cache is not a substitute for first-time validator re-execution or local proof verification. A cache entry may be created only after full local validation succeeds.

Cache only the result of a fully verified proposal under a key that includes:

- chain id
- genesis hash
- protocol version
- validator id
- block height
- parent hash
- parent state root
- batch kind
- batch id
- payload hash
- proposal hash
- verifier metadata hash
- current validator registry root

The cache value can include receipt ids, resulting state root, verified action summary, and a short expiry. It must not contain private wallet material.

Acceptance criteria:

- A duplicate vote request for the same proposal can reuse the same validator's prior verified result.
- A changed parent state root, batch payload, proposal signature, verifier metadata, or validator registry root forces fresh verification.
- Tests cover cache hits, cache misses, stale-state rejection, tampered proposal rejection, and expiry.
- Cache hits do not change proposal hashes, state roots, receipts, vote signatures, or certificates.

### Conditional Phase 6: Benchmark and Gate the Hot Path

Condition: selected by Phase 1C. Execute after Phase 2 and the selected Phase 3/4 fixes land.

Goal: prevent the measured regression from returning.

Implementation status:

- Complete: `scripts/private-egress-pinned-vk-regression-gate` reads one or more private-egress peer-certified smoke reports and validates the pinned verifier-key evidence.
- Complete: the gate checks the top-level smoke report flags, ready-time private-egress verifier prewarm, artifact load mode, `keygen_vk_ms: 0.0`, `full_shape_ms: 0.0`, hot-round private-egress verifier warmth, empty hot-round `vk_builds`, `round_ok`, `all_vote_requests_verified`, `local_apply_verified`, and controlled-testnet timing thresholds.
- Complete: the gate reports p50, p95, p99, and max for private-egress prewarm, hot round total, proposal, remote vote requests, local apply, and proof verification over the supplied samples.
- Evidence: `python3 -m py_compile scripts/private-egress-pinned-vk-regression-gate` passed.
- Evidence: `scripts/private-egress-pinned-vk-regression-gate reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-20260623T065218Z/report.json reports/testnet-asset-orchard-private-egress-peer-certified/phase4-pinned-vk-private-only-20260623T071447Z/report.json` passed with `sample_count: 2`, no failures, and zero non-empty hot-round `vk_builds` fields.

Performance surfaces tracked by the gate:

- ready-time private-egress verifier setup in a cold process using the pinned artifact
- warm local private-egress proof verification inside proposer and validator validation
- shielded private-egress proposal construction in the warm certified round
- remote vote request handling in the warm certified round
- local apply in the warm certified round
- full peer-certified private-egress round over the controlled validator set

The current gate is intentionally modest and controlled-testnet scoped. It uses existing full-harness report artifacts rather than microbenchmark-only evidence because the original regression appeared in the integrated proposer/validator certification path.

Current controlled-testnet gate thresholds:

| Path | Gate |
|---|---:|
| Private-egress ready-time prewarm | `< 15s` |
| Single private-egress proof verify | `< 250ms` |
| Shielded private-egress proposal | `< 250ms` |
| Remote validator vote requests | `< 250ms` |
| Local apply | `< 250ms` |
| Full private-egress certified round | `< 1s` |
| Ready-time private-egress `keygen_vk_ms` | `0.0ms` |
| Hot-round verifier-key builds | `0` non-empty `vk_builds` fields |

Phase 6 measured gate result from the two Phase 4 samples:

| Metric | p50 | p95 | p99 | max |
|---|---:|---:|---:|---:|
| Private-egress prewarm | `7883.751ms` | `8005.144ms` | `8005.144ms` | `8005.144ms` |
| Hot certified round | `373.984ms` | `426.392ms` | `426.392ms` | `426.392ms` |
| Proposal | `73.770ms` | `74.367ms` | `74.367ms` | `74.367ms` |
| Remote vote requests | `100.607ms` | `115.502ms` | `115.502ms` | `115.502ms` |
| Local apply | `61.796ms` | `66.423ms` | `66.423ms` | `66.423ms` |
| Proof verify | `70.902ms` | `91.560ms` | `91.560ms` | `91.560ms` |

These gates should move into the controlled-testnet release checklist. Tighten them only after more samples are collected across the intended validator hardware and topology.

## Risk and Mitigation

### Verified-Proposal Cache

Risk: stale or poisoned cache entries could cause unsafe reuse.

Mitigations:

- Require the full strict cache key listed in Phase 5.
- Populate only after full local validator re-execution and proof verification.
- Use short expiry and fail closed on any key mismatch.
- Keep cache results out of consensus inputs; hashes and signatures must be derived from deterministic proposal/state data, not timing or cache state.
- Add tamper, stale-state, metadata-mismatch, and registry-mismatch tests.
- Feature-flag the cache and allow fallback to full verification.

### Resident Validators

Risk: long-lived processes may hide state drift, leak resources, or diverge from one-shot behavior.

Mitigations:

- Compare resident and one-shot outputs on fixed fixtures: proposal hash, receipt ids, state root, votes, and certificate must match.
- Treat restart as cold; require explicit rewarm and report cold/warm status.
- Keep verifier caches read-only after initialization.
- Continue writing audit evidence files.
- Add health checks and safe restart behavior.
- Never sign if local state, registry view, parent hash, or verifier metadata is inconsistent with the proposal.

## Expected Result

After Phase 1 identifies the bottleneck and the evidence-supported phases land, private egress should no longer spend minutes in avoidable proposal or vote work. The hot private-egress certified round should be bounded by the actual required work: local proof verification, deterministic state application, ML-DSA vote signing, and transport latency.

The work must preserve the security model:

- validators still independently verify the private-egress proof;
- validators still independently confirm the proposal matches local state;
- the certificate still represents quorum validator signatures;
- finality evidence remains replayable from batch, proposal, votes, and certificate artifacts.

## Implementation Order

1. Add timing fields around verifier setup, proof verification, state clone/apply, proposal rebuild, vote lock, signing, serialization, process startup, and transport.
2. Run another private-egress certification round and classify the bottleneck from the new report.
3. Hold the Phase 1 decision gate and record which candidate phases are justified.
4. If supported by evidence, add validator/private-egress verifier warm-up.
5. If supported by evidence, convert user-facing proposal/vote handling to resident validator processes where it still uses one-shot CLI work.
6. If supported by evidence, add pinned verifier loading for release builds.
7. If supported by evidence, add strict verified-proposal caching for duplicate requests and retries.
8. Add CI or release-gate benchmarks for the selected hot paths and fail on large warm-path regressions.

## Definition of Done

This plan is complete when:

- private-egress reports identify exact time spent by verifier setup, proof verification, state apply, vote signing, serialization, process startup, and transport;
- the Phase 1 decision gate is recorded with evidence;
- any implemented optimization is tied to a measured bottleneck;
- a warm private-egress certification round is demonstrated on the controlled validator set if warm/resident work is selected;
- the warm path avoids any measured multi-minute avoidable setup or orchestration delay;
- proposal/vote safety semantics are unchanged;
- loaded verifier metadata validation is tested if pinned loading is selected;
- verified-proposal cache invalidation is tested if caching is selected;
- docs record both cold and warm timings with paths to evidence artifacts.
