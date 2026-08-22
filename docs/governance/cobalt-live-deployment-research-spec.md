# Live Cobalt Deployment and XRPL Liveness Research Specification

Status: locked — text-improvement average 91.40/100 (GPT 94.00, Fable 89.20, GLM 91.00)
Scored content SHA-256: `b490936e96679fffe108dacfee80691b7395b473ed7772767118dca25cce9f33`
Date: 2026-08-22
Task Node: `task_50b08c9b22e2348237b65436d4be4fed`
PostFiat baseline: `cde96f434ffa3d48afa0049cf55e1ad2e8779d9d`
XRPL baseline: `rippled 3.1.3`, commit `46b241ace8b30d9c9775d60ffba7d24b21903896`
Decision scope: live Cobalt governance on the controlled testnet and a reproducible Cobalt-versus-XRPL quorum/liveness comparison

## Plain-English decision

PostFiat should deploy Cobalt now as an always-on **shadow governance service on the real devnet validators**. It should not transfer governance authority yet.

Today the repository has substantial Cobalt agreement, trust-graph, replay, signer, and handoff code. It does not have a live distributed Cobalt service. The current shadow binary initializes local state, prints status, or runs a local adversarial drill. The examples use hard-coded seven-validator fixtures and loopback TCP. None of those facts show that real validator machines can exchange Cobalt messages, survive faults, converge on the same ratification, and recover from restart.

The next milestone should close that exact gap. It should put a networked, observable, non-authoritative Cobalt sidecar beside every live controlled-testnet validator; feed the sidecars real, inert validator-governance proposals; run them long enough to include planned faults and restarts; publish a compact verification packet; and compare their safety/liveness envelope with pinned RippleD consensus simulations. Only a subsequent, separately approved cutover may submit the already-implemented Foundation-to-Cobalt handoff.

“Live Cobalt” therefore has two distinct states:

1. **Live shadow:** real processes, real validators, real authenticated network traffic, real persistence and recovery, but no ability to mutate governance. This specification recommends proceeding.
2. **Live authority:** a consensus-ordered handoff makes Cobalt ratifications authoritative for validator-trust updates. This specification makes that conditional on the evidence gates below.

Cobalt remains governance-only. Consensus v2 remains the sole block-ordering and transaction-finality protocol. A Cobalt halt must pause validator-set evolution, not halt payments or create a second finality rule.

## What the code actually provides today

| Surface | Current code truth | What is still missing for a credible live deployment |
| --- | --- | --- |
| Authority boundary | `crates/node/src/cobalt_handoff.rs:1-5` restricts Cobalt to validator-registry and trust-graph evolution. `verify_governance_authority_batch` at lines 125-190 separates Foundation, transition, and Cobalt-validator-update batches. | No cutover is needed for shadow operation. Authority transfer must remain a later, explicit governance action. |
| Trust topology | `build_trust_graph`, `build_canonical_unl_trust_graph`, `verify_cobalt_safety_witness`, and `analyze_trust_graph` in `crates/consensus_cobalt/src/trust_graph_governance.rs:66-150,314-407,920-1030` bind roots, local views, linkage, fault assumptions, and old/new intersections. | Generate the graph from a fresh live validator registry and a reviewed placement/control manifest, instead of a seven-node fixture. |
| Agreement | Signed RBC support, ABBA support, MVBA input selection, and DABC ratification/replay live in `rbc_abba_mvba.rs` and `dabc_registry.rs`. Production signed evaluators are separate from simulation-only schema evaluators. | An event loop must drive the protocol over authenticated WAN transport and persist per-stage progress; a library call is not a running protocol. |
| Durable shadow state | `crates/node/src/cobalt_shadow.rs:256-347` creates a mode-0600 ML-DSA signer and reloads validated state. Lines 646-750 bound, authenticate, de-duplicate, reject replays, queue, and process messages. Atomic file replacement and directory sync are implemented at lines 1254-1278. | Bind each shadow key to a real validator identity and current registry root; add a long-running network command and production lifecycle. |
| Current shadow authority | `crates/node/src/cobalt_shadow.rs:1-4,283-302` explicitly sets `live_authority=false` and `controls_block_consensus=false`. | Preserve those invariants throughout shadow deployment and make the monitor fail if either changes. |
| Executable surface | `postfiat-cobalt-shadow` exposes only `init`, `status`, and `drill` in `crates/node/src/bin/postfiat_cobalt_shadow.rs:17-52`. | Add operator-grade `run`, `probe`, `snapshot`, and `replay` surfaces plus a systemd unit and rollout support. |
| Simulation surface | `current_trust_graph_root.rs`, `cobalt_partition_simulation.rs`, and `rbc_nonuniform_tcp_drill.rs` use seven logical validators; the TCP drill binds `127.0.0.1` and runs worker threads in one process. | Replace hard-coded topology with a canonical scenario manifest and support both current-fleet and scaled simulation sizes. |
| Validator transport | `systemd/postfiat-validator-transport.service.example` already runs authenticated private-topology transport with bounded connections, timeouts, validator keys, and an event log. `scripts/testnet-monitor-snapshot` already collects validator health and consensus metrics. | Reuse this authenticated topology or its network library; do not create an unrelated unauthenticated Cobalt TCP overlay. Add Cobalt-specific health and metrics. |
| Human interface | `python/postfiat_rpc/cobalt.py` and `cobalt_ui.py` inspect library examples, persisted shadow state, and the handoff gate. | Point them at the distributed fleet snapshot and comparison packets. The UI must distinguish “shadow healthy” from “authority active.” |

The most recent committed fleet rollout receipt is historical, not a live receipt. It records six validators converged on six machines across three regions in July 2026. The older public Cobalt evidence bundle is also historical and pinned to earlier revisions. Its strict launch packet correctly failed because the proposed seven logical validators did not have enough independently placed machines and required topology metadata. These artifacts are useful regression inputs, not proof of the fleet on 2026-08-22. The first milestone gate must rediscover the active fleet and must not assume that the current count is five, six, or seven.

## Scope

This work includes:

- the real controlled-testnet validator fleet and its current validator registry;
- Cobalt governance proposal transport, agreement, persistence, replay, observability, and operator recovery;
- validator-trust graph validation and transition safety;
- quorum, overlap, fork-safety, halt/liveness, and recovery comparisons with RippleD;
- a shadow deployment followed by a separate authority-handoff rehearsal and go/no-go decision;
- a concise Python CLI, browser view, runbook, and verifier-backed evidence packet.

This work excludes:

- replacing consensus v2 or changing transaction finality;
- claiming that Cobalt makes transactions faster;
- RippleD transaction-apply, MPT, trustline, AMM, NFT, invariant, or accounting remediation;
- enabling Cobalt authority before the shadow and rehearsal gates pass;
- mainnet, real-value, HSM, operator-decentralization, or public-network claims;
- counting multiple machines owned and funded by one operator as independent operators.

## The deployment path

### Gate 0 — freeze a fresh, redacted live baseline

Before writing deployment configuration, collect one signed or hash-bound receipt from every currently active validator using the existing read-only fleet and monitor tooling. The receipt must contain:

- repository commit, binary hash, chain ID, genesis hash, protocol version, height, tip, and state root;
- validator ID, active-registry root, quorum, consensus participation, transport health, and service status;
- machine, provider, region, jurisdiction, operator, funding, and key-custody group labels, redacted to stable identifiers;
- clock skew, disk headroom, CPU and memory headroom, and current restart count;
- collection time and maximum age.

The gate fails on an unreachable validator, divergent chain identity or state, unknown registry membership, reused validator key, missing topology label, or an inventory older than the chosen deployment window. The output becomes `fleet-receipt.json`; no IP address, credential, instance ID, or private topology is published.

The initial Cobalt graph must be generated from this receipt and the committed registry. Quorum and blocking thresholds are computed from the discovered graph; they are never copied from the old seven-validator fixtures.

### Gate 1 — reproduce the current Cobalt substrate at the pinned commit

Run the signed Cobalt unit tests and current examples from a clean build, then regenerate their compact reports. The present workstation’s `cc` path invokes a raw Zig binary incorrectly, so the milestone must first restore a working, pinned linker invocation; old report files do not excuse an unreproducible current checkout.

Required baseline commands include:

```bash
cargo test -p postfiat-consensus-cobalt --locked
cargo test -p postfiat-consensus-cobalt --features cobalt-unsafe-simulation --locked
cargo test -p postfiat-node --lib cobalt_handoff --locked
cargo run -p postfiat-consensus-cobalt --example current_trust_graph_root
cargo run -p postfiat-consensus-cobalt --example cobalt_partition_simulation   --features cobalt-unsafe-simulation
cargo run -p postfiat-consensus-cobalt --example rbc_nonuniform_tcp_drill   --features cobalt-unsafe-simulation
```

This gate records what the code proves and also records the fixture limitations. It must not relabel loopback output as remote-validator evidence.

### Gate 2 — build the networked shadow runtime

Extend the existing service rather than replacing its safety boundary.

The runtime should:

1. load chain identity, active registry root, trust graph, bounded limits, and peer endpoints from a versioned manifest;
2. bind each Cobalt shadow public key to one active validator identity through a registry-root-bound statement signed by that validator’s existing key;
3. exchange canonical, domain-separated Cobalt messages through the authenticated private validator topology;
4. drive signed RBC, ABBA, MVBA, and DABC state transitions, not merely increment the generic shadow message digest;
5. persist high-water marks and protocol locks before returning or transmitting the associated signature;
6. expose structured readiness, stage timestamps, message/byte counters, queue depth, peer status, graph root, ratification lock, and replay status;
7. consume only inert shadow proposals and emit a shadow ratification receipt that no block-execution path accepts;
8. restart and catch up from peers without generating a second vote for the same domain.

The process should run as an unprivileged systemd service with bounded memory, file descriptors, message size, candidates, queues, timeouts, and restart rate. It may share transport code and topology with the validator service, but its port, storage, logs, and failure domain should remain isolated. A Cobalt crash must not restart the block validator.

The operator CLI should make these actions obvious:

```text
postfiat-cobalt-shadow run
postfiat-cobalt-shadow probe
postfiat-cobalt-shadow snapshot
postfiat-cobalt-shadow replay
postfiat-cobalt-shadow status
```

The Python CLI and browser view consume those exact machine-readable outputs. They do not infer health from files that the Rust service has not authenticated.

### Gate 3 — canary, fleet rollout, and sustained real-validator shadow operation

Use the repository’s safe-rollout pattern:

1. deploy one canary sidecar;
2. verify that block height, block-finality metrics, CPU, memory, disk, and peer health remain within the pre-deployment envelope;
3. deploy one validator at a time;
4. stop automatically on a chain-health regression or shadow identity mismatch;
5. confirm every sidecar reports the same chain, registry, graph root, protocol version, and `live_authority=false`;
6. submit a fixed sequence of inert proposals: no-op control, validator add, validator remove, trust-view change, key rotation, invalid parent, and rollback;
7. compare every validator’s accepted/rejected result and ordered ratification digest.

“Running for a week” is not sufficient by itself. Shadow completion is event-based: the fleet must complete the full proposal corpus, a planned restart of every sidecar, a full-fleet replay from genesis, a one-validator outage, one region isolation, delay/loss/reorder injection, equivocation, stale replay, and partition healing. The active block chain must continue finalizing throughout every Cobalt-only fault.

### Gate 4 — build one matched quorum/liveness benchmark

The benchmark must answer a narrow question: **under the same declared validator views and faults, where do Cobalt and RippleD make one decision, halt safely, recover, or produce conflicting decisions?**

It must not compare Cobalt governance-round latency with XRPL payment latency. It must not compare a private PostFiat devnet with public XRPL mainnet. The primary comparison is simulator-to-simulator; a sequential same-host WAN control is secondary.

#### RippleD control

Pin upstream `XRPLF/rippled` 3.1.3 at `46b241ace8b30d9c9775d60ffba7d24b21903896`.

- Use upstream’s own `src/test/csf` framework and `Consensus_test::testFork` as the native baseline.
- Record `ValidatorList::calculateQuorum` from `src/xrpld/app/misc/detail/ValidatorList.cpp`: absent a special publisher-unavailable case, local quorum is the greater of 80% of effective UNL and 60% of local UNL; unsafe command-line overrides warn but are accepted.
- Keep AGTI’s extended UNL-overlap-implosion test as a separately pinned patch and artifact. Do not describe it as an upstream test.
- Do not infer global UNL intersection from a node’s local quorum. XRPL’s official documentation says each server chooses its own UNL and cites high, potentially 90% worst-case, overlap as a fork-safety requirement.

#### Cobalt control

Drive `analyze_trust_graph`, the signed RBC/ABBA evaluators, MVBA selection, DABC ratification, and replay from the same canonical scenario manifest. Use the actual discovered fleet size as one matrix and include 7-, 10-, and 20-validator scale controls so the result is not an accident of one six-validator topology.

#### Shared scenario manifest

Each deterministic scenario contains:

- validator identities and local UNL/trust views;
- essential subsets, quorum, and declared Byzantine budget;
- provider, region, operator, and key-custody correlation groups;
- offline, Byzantine, censored, and equivocal validators;
- partitions and heal time;
- latency distribution, packet loss, duplication, and reordering;
- list/graph drift, validator add/remove, and key rotation;
- seed, repetition count, timeout, binary hash, and expected safety outcome.

Sweep pairwise overlap from 100% to 0%, but report the actual set intersections and local quorum values rather than only a percentage. Include no-fault, one-fault, within-declared-budget, beyond-budget, correlated-region loss, asymmetric views, publisher/list drift, and healed-partition cases.

A secondary real-machine lane may run pinned private RippleD validators sequentially on the same machines, isolated ports, and disposable data directories. It measures validator operations and recovery only. It must not compete with the live PostFiat processes for load, mutate the PostFiat chain, reuse keys, or become a claim about XRPL mainnet.

## KPI contract

| KPI | Exact measurement | Decision use |
| --- | --- | --- |
| Conflicting-decision count | Distinct accepted decision/ledger hashes for the same domain and slot across correct validators. | Absolute safety gate: must be zero in every shadow and within-model scenario. A beyond-model conflict is reported, never averaged away. |
| Safe-halt accuracy | Scenarios expected to lack safe quorum that halt without a conflicting acceptance, divided by all such scenarios. | Must be 100% for the fixed adversarial corpus before authority rehearsal. |
| Liveness completion | Correct validators reaching the common accepted result before the scenario timeout, by fault class. | Must be 100% for no-fault and every scenario inside the declared fault/linkage model. Outside-model results are characterized, not passed. |
| Decision latency | Proposal receipt to RBC accept, ABBA finish, MVBA selection, and DABC ratification; p50/p95/p99 and maximum. | Establishes the governance activation lead time. It is not a transaction-finality comparison. |
| Recovery time | Fault heal or process restart to common graph root, queue drain, and next successful ratification. | Must complete without manual state edits and without a conflicting signature. |
| Quorum/topology margin | Smallest validator or correlation-group removal that blocks progress; smallest set that can support conflicting decisions under the model; pairwise set intersections. | Makes machine, region, operator, and key-custody concentration visible. |
| Trust-view safety | Linked, fully linked, and unsafe validator pairs plus rejected old/new safety-witness intersections. | Cobalt activation requires no unexplained unsafe pair in the live graph. |
| Replay determinism | Fresh replay bundle digest and final lock versus every validator’s live digest and lock. | Must match bit-for-bit on all validators. |
| Communication cost | Signed messages and wire bytes per validator per successful proposal and per failed scenario. | Quantifies Cobalt’s expected governance overhead versus the XRPL control. |
| Resource headroom | Sidecar CPU time, peak RSS, disk growth, queue high-water mark, open descriptors, and validator-service delta. | A canary or rollout stops on exhaustion, unbounded growth, or material validator degradation. Final budgets are frozen from the canary envelope before fleet faults begin. |
| Operational availability | Successful probes divided by scheduled probes; stale graph/peer/round age; restart count. | Shows that the service is actually running rather than periodically demonstrated. |
| Evidence completeness | Required manifest, receipts, markers, raw reports, verifier result, and checksums present. | Missing evidence is a failed gate, not a narrative exception. |

Latency and resource thresholds must be frozen after a no-fault canary calibration and before adversarial runs. This avoids choosing thresholds after seeing failures. The absolute gates—zero conflicting decisions, deterministic replay, no authority mutation in shadow, and continued block finality—are not tunable.

The comparison should publish both systems’ failure regions. A safe halt is not scored as a fork; a slow decision is not scored as a safety failure; and a Cobalt governance halt is not called a chain halt when consensus v2 continues to finalize blocks.

## Evidence packet

Every baseline, simulation sweep, WAN run, and authority rehearsal produces an AGTI-style packet:

```text
packet/
  README.md
  manifest.json
  scenario-manifest.json
  fleet-receipt.public.json
  cobalt/
  rippled/
  markers.json
  verify_packet.py
  SHA256SUMS.txt
```

The manifest pins source commits, binary hashes, feature flags, scenario seeds, topology digest, run timestamps, and expected markers. Per-scenario wrappers assert their marker and outcome. `verify_packet.py` checks schema, hashes, required scenarios, result cardinality, safety invariants, replay equality, and zero-failure footers without rerunning the expensive experiments. `SHA256SUMS.txt` provides one canonical packet root.

Public packets exclude credentials, private keys, IP addresses, instance IDs, and private topology. They preserve stable validator and correlation-group labels needed to interpret the result. The report always separates:

1. current live state observed in the fresh fleet receipt;
2. behavior reproduced in a simulator or controlled fault;
3. the operator’s activation decision.

Historical May/June Cobalt bundles and July fleet receipts remain named inputs with their original hashes. They are never silently refreshed or presented as current.

## Authority rehearsal and go/no-go

After the shadow packet passes, rehearse the exact handoff on a disposable clone of the current chain state.

The rehearsal must:

- use the current active registry’s distinct ML-DSA approvals;
- bind the exact old registry root, Cobalt registry root, trust graph root, Cobalt lock, amendment sequence, protocol version, scope, and future activation height;
- show that early, stale, self-authorized, mixed-authority, replayed, and wrong-root transitions fail;
- prove that a pre-activation abort leaves Foundation authority unchanged;
- prove that a post-activation rollback is a new forward transition;
- execute a validator-trust update under Cobalt authority and reject unrelated governance kinds;
- replay the entire cloned chain state to the same authority state.

The recommendation becomes “activate Cobalt authority on the controlled testnet” only when:

- the fresh live inventory and graph pass;
- current-commit tests and packet verification pass;
- all real validators have completed the event-based shadow corpus;
- safety, liveness, replay, recovery, resource, and continued-finality gates pass;
- the matched benchmark reports both systems without an unresolved methodology exception;
- the disposable handoff and rollback rehearsal pass;
- the Python CLI, browser view, operator runbook, monitoring, and alerting reflect the live distributed service;
- a separately reviewed cutover packet fixes the activation height and rollback action.

If any condition fails, keep the sidecars live in shadow, preserve Foundation authority, and repair the failed gate. Do not deactivate useful observation merely because authority is held.

## Advice on the XRPL finding

The AGTI audit’s liveness conclusion is the relevant inheritance lesson: RippleD enforces a node’s local quorum but cannot prove that independently chosen UNLs retain the global overlap needed for fork freedom. Cobalt is worth activating only if PostFiat uses its stronger trust-graph machinery operationally—authenticated local views, explicit correlation groups, linkage analysis, transition witnesses, and fail-closed admission. Copying one canonical validator list into Cobalt would reproduce much of the same trust-management weakness behind a different protocol.

The credible benefit is therefore not “Cobalt never halts” or “Cobalt is faster than XRP.” The hypothesis to test is:

> Cobalt can make unsafe validator-trust topology visible and reject it before activation, while keeping a governance failure outside the block-finality path.

The benchmark and live shadow deployment can falsify that hypothesis. Until they do, Cobalt is promising implemented machinery, not a live consensus benefit.

## Primary references

- `docs/governance/cobalt-research-spec.md`
- `docs/governance/cobalt-implementation.md`
- `docs/plans/completed/cobalt-governance-milestone.md`
- `crates/consensus_cobalt/src/trust_graph_governance.rs`
- `crates/consensus_cobalt/src/rbc_abba_mvba.rs`
- `crates/consensus_cobalt/src/dabc_registry.rs`
- `crates/node/src/cobalt_shadow.rs`
- `crates/node/src/cobalt_handoff.rs`
- `crates/node/src/bin/postfiat_cobalt_shadow.rs`
- `systemd/postfiat-validator-transport.service.example`
- `scripts/testnet-monitor-snapshot`
- `python/postfiat_ops/safe_rollout.py`
- [Historical Cobalt devnet evidence manifest](https://postfiat.org/benchmarks/cobalt-devnet-evidence-20260609/manifest.json)
- [XRPL UNL documentation](https://xrpl.org/docs/concepts/consensus-protocol/unl)
- [RippleD 3.1.3 `ValidatorList::calculateQuorum`](https://github.com/XRPLF/rippled/blob/46b241ace8b30d9c9775d60ffba7d24b21903896/src/xrpld/app/misc/detail/ValidatorList.cpp)
- [RippleD 3.1.3 CSF `Consensus_test::testFork`](https://github.com/XRPLF/rippled/blob/46b241ace8b30d9c9775d60ffba7d24b21903896/src/test/consensus/Consensus_test.cpp)
- [AGTI RippleD fork-inheritance audit](https://agtico.github.io/intelligence-reports/2026/05/26/xrpl-rippled-open-p0-freeze-audit/)
