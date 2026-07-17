# Public Claims Checklist

Status: controlled-testnet claims source of truth
Date: 2026-05-22
Scope: PostFiat L1 v2 controlled testnet

This checklist maps public language to committed code, docs, and evidence. Use
it before editing the whitepaper, website copy, investor materials, or agent
handoffs.

## Allowed Claims

### CLAIM-CT-READY

Allowed language:

PostFiat has executed a controlled launch for transparent post-quantum
settlement with a known validator cohort. The optimized local 5-validator
submit-to-finality path and optimized remote 5-validator peer-certified path
pass the controlled-launch latency target, five validator/RPC services have
been installed and converged on the live controlled operator surface, a
certified transparent round has been recorded, and an SDK wallet
quote/sign/submit/`tx` finality flow has passed against the running network via
a documented temporary controlled write edge.
Post-launch hardening evidence now includes 100-round live continuity,
all-validator restart, oversized RPC edge-load, SDK-validated 12-method live
RPC read-load, single-validator partial-outage recovery, below-quorum outage
safety/recovery, host-group capture-threshold outage/recovery, bounded mixed
read/write load, fresh live observability, and a clean-head live hardening
evidence pack under the same controlled operator boundary. The
controlled write-edge policy/audit is documented and verifies that validator
RPC units remain read-only by default; a persistent externally exposed write
edge is not yet installed.

Evidence:

- Candidate revision:
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`.
- Final candidate report:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/testnet-release-final-candidate-20260514T-current-head-56db87a-optimized-latency.json`.
- Release status:
  `reports/testnet-release-status-current-56db87a-optimized-latency/testnet-release-status-current-56db87a-optimized-latency.json`.
- Operator launch packet:
  `reports/testnet-operator-launch-packet/current-56db87a-optimized-latency/testnet-operator-launch-packet.json`.
- Controlled launch evidence pack generator:
  `scripts/testnet-controlled-launch-evidence-pack`.
- Current clean-head controlled launch evidence pack:
  `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`.
- Operator-private live prep:
  `reports/testnet-live-operator-artifact-20260514T-prep/launch-prep-check.json`.
- Live launch report:
  `reports/testnet-live-launch-20260514T151903Z-head-9e4fb20-rerun5/testnet-release-live-launch.json`.
- Live SDK wallet finality report:
  `reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json`.
- First live continuity soak report:
  `reports/testnet-live-continuity-soak/current-rerun-20260514T162207Z/testnet-live-continuity-soak.json`.
- Live restart drill report:
  `reports/testnet-live-restart-drill/current-20260514T162808Z/testnet-remote-restart-drill-20260514T162808Z.json`.
- Post-restart continuity report:
  `reports/testnet-live-continuity-soak/post-restart-20260514T162853Z/testnet-live-continuity-soak.json`.
- Live RPC edge-load report:
  `reports/testnet-live-rpc-edge-load/current-20260514T163012Z/testnet-remote-rpc-edge-load.json`.
- Live RPC read-load report:
  `reports/testnet-remote-rpc-read-load/current-20260514T170449Z/testnet-remote-rpc-read-load.json`.
- Broad live RPC read-load report:
  `reports/testnet-remote-rpc-read-load/broad-12method-1200-20260514T185852Z/testnet-remote-rpc-read-load.json`.
- Controlled write-edge policy audit:
  `reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`.
- Controlled write-edge policy runbook:
  `docs/runbooks/controlled-write-edge-policy.md`.
- Longer live continuity soak report:
  `reports/testnet-live-continuity-soak/longer-20round-20260514T163232Z/testnet-live-continuity-soak.json`.
- Extended live continuity soak report:
  `reports/testnet-live-continuity-soak/longer-100round-20260514T171905Z/testnet-live-continuity-soak.json`.
- Live single-validator partial-outage drill report:
  `reports/testnet-live-partial-outage-drill/current-20260514T164149Z/testnet-remote-partial-outage-drill-20260514T164149Z.json`.
- Post-partial-outage continuity report:
  `reports/testnet-live-continuity-soak/post-partial-outage-20260514T164318Z/testnet-live-continuity-soak.json`.
- Live below-quorum outage drill report:
  `reports/testnet-live-below-quorum-outage-drill/live-below-quorum-rerun-20260514T181916Z/testnet-live-below-quorum-outage-drill-20260514T181916Z.json`.
- Post-below-quorum continuity report:
  `reports/testnet-live-continuity-soak/post-below-quorum-continuity-20260514T182141Z/testnet-live-continuity-soak.json`.
- Live mixed read/write load report:
  `reports/testnet-live-mixed-read-write-load/live-mixed-read-write-10x-20260514T185000Z/testnet-live-mixed-read-write-load.json`.
- Fresh live observability report:
  `reports/testnet-remote-observability/live-observability-post-mixed-10x-20260514T185456Z/testnet-remote-observability.json`.
- Live host-group outage report:
  `reports/testnet-live-host-group-outage-drill/live-host-group-outage-20260514T191629Z/testnet-live-host-group-outage-drill.json`.
- Live hardening evidence pack:
  `reports/testnet-live-hardening-evidence-pack/current-20260514T-post-host-group-outage/testnet-live-hardening-evidence-pack.json`.
- Optimized local latency evidence:
  `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`.
- Optimized remote latency evidence:
  `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`.
- External reviewer packet:
  `docs/review/controlled-testnet-review-packet.md`.

Boundary:

This is not a public decentralized network, mainnet, production privacy, or
permissionless public RPC claim. Safe language is "controlled launch executed"
or "running controlled operator network." Do not claim broad decentralization,
production readiness, or unrestricted public access.

### CLAIM-PQ-AUTH

Allowed language:

The transparent account and validator authorization path uses ML-DSA-style
post-quantum signatures, and the current evidence includes signature-size and
certificate-size measurements.

Evidence:

- `crates/crypto_provider/src/lib.rs`
- `crates/types/src/lib.rs`
- `crates/bench_harness/src/main.rs`
- `scripts/testnet-benchmark-evidence-pack`
- `reports/testnet-ml-dsa-performance/ml-dsa-verify-20260513T193753Z/testnet-ml-dsa-performance-20260513T193753Z.json`

Boundary:

Do not claim a completed external cryptographic audit. Do not claim lattice
signature aggregation.

### CLAIM-XRP-LIKE

Allowed language:

PostFiat is XRP-like in operating shape: known validators, fast finality target,
low fees, deterministic settlement, public account/transaction reads, and a
federated validator governance model.

Evidence:

- `docs/specs/transparent-transaction-envelope.md`
- `docs/runbooks/public-rpc-operator-policy.md`
- `reports/testnet-wallet-sign-transfer-smoke/sdk-signer-rpc-flow/testnet-wallet-sign-transfer-smoke.json`
- `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`
- `reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json`
- `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`

Boundary:

Do not imply PostFiat is a fork of XRPL or mechanically identical to rippled.

### CLAIM-WALLET-CUSTODY-MODEL

Allowed language:

PostFiat has a controlled-testnet wallet SDK path for deterministic ML-DSA
wallet backup, public identity restore, quote-bound transfer signing, signed
submit request construction, and finality verification. It also has a v0
exchange/custody deposit model that documents unique deposit addresses,
watch-only limits, no BIP32/xpub-style public derivation, and recovery
boundaries.

Evidence:

- `crates/rpc_sdk/src/lib.rs`
- `crates/rpc_sdk/src/main.rs`
- `docs/runbooks/sdk-wallet-flow.md`
- `docs/specs/wallet-exchange-custody-model.md`
- `docs/specs/account-key-rotation-boundary.md`
- `scripts/testnet-sdk-wallet-cli-smoke`
- `scripts/testnet-live-wallet-finality`
- `reports/testnet-live-wallet-finality/current-rerun3-20260514T161147Z/testnet-live-wallet-finality.json`

Boundary:

Do not claim exchange-grade custody, hardware-wallet support, xpub-style public
child derivation, production account-history indexing, account key rotation, or
unrestricted public write RPC. The current live wallet evidence used a
temporary SSH-local controlled write edge; validator read RPC remains read-only
by default; the persistent externally exposed write edge is still not
installed.

### CLAIM-COBALT-CANONICAL

Allowed language:

PostFiat runs Cobalt-derived validator governance in canonical-UNL mode.
Validator-set evolution is explicit, signed, registry-root-bound, replayable,
and release-gated.

Evidence:

- `docs/governance/cobalt-canonical-mode.md`
- `docs/governance/cobalt-controlled-testnet-plan.md`
- `docs/governance/cobalt-amendment-lifecycle.md`
- `crates/consensus_cobalt/src/lib.rs`
- `scripts/testnet-cobalt-lifecycle-audit`
- `scripts/testnet-registry-root-binding-audit`
- `scripts/testnet-cobalt-amendment-lifecycle-smoke`
- `scripts/testnet-controlled-launch-evidence-pack`
- `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`
- `reports/testnet-governance-replay-package-smoke/governance-replay-genesis-link-20260513T185251Z/testnet-governance-replay-package-smoke.json`

Boundary:

Do not say "full Cobalt consensus" or imply open-ended peer-selected trust
views are live in controlled testnet. Do not imply the standalone amendment
replay bundle is the same as full non-uniform Cobalt trust-view replay.

### CLAIM-PRIVACY-ROADMAP

Allowed language:

Privacy is a first-class product pillar. Current code has shielded
note/nullifier/turnstile semantics, a proof adapter boundary, and a real
Orchard/Halo2 action verifier behind an explicit node gate and the ordered
shielded batch path for zero-balance Orchard actions. Production privacy
requires root history, wallet scan/decrypt/spend/disclosure flow, turnstile
accounting for nonzero value balances, pricing, benchmarks, operator limits,
and review evidence.

Evidence:

- `crates/privacy/src/lib.rs`
- `crates/proofs/src/lib.rs`
- `crates/privacy_orchard/src/verify.rs`
- `crates/node/src/lib.rs`
- `docs/status/research-response-synthesis.md`
- `docs/status/controlled-testnet-burndown.md`

Boundary:

Do not claim production private transactions, full Zcash-equivalent wallet/pool
behavior, end-to-end post-quantum private value, or regulated confidential
settlement from the debug adapter, local Orchard gate, or zero-balance ordered
Orchard action path alone. Debug proof paths are controlled-testnet-only and
must remain behind the explicit debug-proof gate; production privacy language
requires the Orchard path, no debug env override, and audited proof evidence.

### CLAIM-HISTORY-PARTIAL

Allowed language:

Validators are not required to be full-history archive servers. Controlled
testnet has a partial-history validator policy, archive-window export/import,
source-driven backfill, and archive/indexer role separation.

Evidence:

- `docs/runbooks/validator-history-retention.md`
- `docs/status/controlled-testnet-history-roles.json`
- `scripts/testnet-history-role-policy-smoke`
- `reports/testnet-history-retention-smoke/archive-backfill-sdk-20260513T203911Z/testnet-history-retention-smoke.json`

Boundary:

Do not imply independent public archive providers are already onboarded.

### CLAIM-REVIEW-REMEDIATION

Allowed language:

Active runtime and operator-default paths have been hardened for controlled
transparent testnet on localhost/private networks. The raw Ambient P0/P1
inventory is dispositioned row-by-row.

Evidence:

- `docs/status/ambient-finding-disposition-ledger.json` plus its shard
  directory
- `docs/status/ambient-p0-remediation-status-2026-05-22.md`
- `docs/status/ambient-p1-remediation-status-2026-05-22.md`
- `docs/status/operator-script-hardening-burndown-2026-05-22.md`
- `docs/status/generated-evidence-hygiene-burndown-2026-05-22.md`
- `docs/specs/plaintext-key-file-boundary.md`

Boundary:

Do not claim all raw Ambient rows are fixed. Rows marked `script_backlog`,
`generated_artifact`, `archive_only`, `downgraded`, or
`wont_fix_documented` remain explicit boundaries. Public RPC/transport still
requires an explicit bind override plus TLS/tunnel/authenticated edge policy,
and plaintext JSON key files remain controlled-testnet compatibility files
until an encrypted key-file envelope ships.

## Disallowed Or Not-Yet Claims

### CLAIM-NO-DECENTRALIZATION

Do not claim broad decentralization. Safe language is "controlled validator
cohort" or "federated model under controlled-testnet conditions."

The current topology is useful launch evidence, not proof that no single
operator, funder, cloud, jurisdiction, or legal domain can affect quorum. The
live host-group outage drill explicitly records that one two-validator
operator-host group can block quorum under the current controlled deployment,
although it cannot form quorum or advance state alone.

### CLAIM-NO-BRIDGE-CUSTODY

Do not claim production bridge custody. Current bridge code is state-machine and
witness simulation, not external asset custody.

### CLAIM-NO-TPS

Do not publish TPS or latency numbers unless the claim includes exact build
hash, validator count, hardware, region/topology, workload, signature/cert
payload size, RPC settings, and measurement window. The benchmark evidence pack
is allowed to publish ML-DSA byte constants/timings, certificate-size model
rows, wallet finality proof linkage, RPC write-edge pressure checks,
observability/disk measurements, and soak linkage, but it must not be reframed
as throughput evidence until WAN/public endpoint load and target hardware
profiling are captured.

### CLAIM-NO-PRODUCTION-PRIVACY

Do not claim production privacy until root history, wallet scan/disclosure
flow, nonzero value turnstile accounting, pricing, benchmarks, operator limits,
and audit package exist.

## Review Rule

Every public claim must point to one of:

- committed code;
- a committed spec/runbook;
- a generated evidence report with a stable path;
- an explicit roadmap item labeled as not yet production.

If a claim cannot be mapped this way, remove it or rewrite it as roadmap.
