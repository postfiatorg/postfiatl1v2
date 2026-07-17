# Open-Source Operations Readiness Inventory

**Date:** 2026-07-16
**Scope:** source-publication candidate and controlled pre-testnet operation
**Production claim:** not approved for real-value operation

## Result

The repository has a coherent controlled-pretestnet deployment and recovery
path, but it does not yet have the custody, alerting, fault-drill, and
independent-operator evidence required for a decentralized production claim.
The source candidate says that plainly and does not ship an enabled production
signer/runtime profile.

The review also found and fixed two concrete operational regressions:

1. the safe-rollout preflight verified signed binaries, units, topology and
   fleet convergence but not each node's mutable committee-key roster. A node
   with a one-member `validator_keys.json` could therefore restart and later
   fail quorum-certificate verification. Preflight and every `apply-next` now
   run the active binary's `validate-local-keys --validators 6` on all six
   nodes and require exact node identity, key count, required count, validity,
   and private-file permissions before any mutation;
2. the public-tree cleanup removed the operator doctor/monitor programs while
   retaining a runbook that promised them. The bounded local validator doctor,
   RPC doctor, monitor snapshot, RPC method inventory, and account-history
   query tools and their local smoke harnesses are restored. The v2 method
   inventory is exhaustive across 135 methods: 63 read-only public, 12
   default-public cryptographically authorized protocol mutations, 14
   controlled-write gated, four privacy-alpha gated, four owned-lane gated,
   and 38 operator/local. New or unclassified methods fail CI.

## Service and privilege boundary

| Control | Validator transport | RPC | Docs | Evidence |
| --- | --- | --- | --- | --- |
| Dedicated unprivileged user | yes | yes | yes | systemd examples and generated units |
| Safe default bind | private overlay only | loopback only | loopback only | source bind guards and unit tests |
| Filesystem sandbox | strict, explicit write paths | strict, explicit write paths | strict read-only system | unit definitions |
| Privilege escalation blocked | yes | yes | yes | `NoNewPrivileges`, empty capabilities |
| Kernel/control-plane hardening | yes | yes | yes | ProtectKernel/ControlGroups directives |
| File and core policy | `UMask=0077`, core disabled | same | same | unit definitions |
| Resource bounds | 65,536 FDs, 1,024 tasks | same | 4,096 FDs, 256 tasks | unit definitions |
| Memory ceiling | not set | not set | not set | residual: measure before selecting a safe limit |
| Production signer | no | no | n/a | file signer requires explicit `--unsafe-devnet-file-signer` |

The validator and RPC units deliberately remain devnet units because the
current signer boundary is a plaintext software key file. This is not an HSM,
remote-signer, encrypted-keystore, rotation, backup, or disaster-recovery
design and must not be relabeled as one.

## Release, rollout, and rollback

- `deployment-validator-units-stage` emits immutable release paths, exact
  topology and circuit-metadata bindings, per-validator systemd units, and a
  signed ML-DSA-65 deployment manifest.
- Unit `ExecStartPre` verifies the running binary, topology, circuit metadata,
  environment and service-unit hashes against the signed manifest.
- `scripts/postfiat-safe-rollout` accepts no arbitrary destination or delete
  option, reconciles provider inventory, requires exact six-node ledger
  convergence and exact six-node committee rosters, freezes input hashes,
  takes and re-imports a signed verified backup, and advances a durable
  canary-first state machine exactly one validator at a time.
- Every node restart is followed by local status and exact fleet convergence;
  any failure leaves the rollout state short of the next transition.
- Mixed-version operation is allowed only inside that one-node-at-a-time
  compatibility window. A release with state-schema or replay incompatibility
  requires a migration/rollback drill before staging; there is no operator
  flag that waives convergence.

## Backup and recovery

The source contains signed snapshot export/import, manifest verification,
state-root verification, history retention, validator doctor, emergency key
rotation and snapshot recovery runbooks. The safe rollout proves a backup can
be exported, signed away from the validator, imported into a fresh directory,
and pass `verify-state` before deployment begins.

Still required before a production claim: measured archive restore, disk-loss
node replacement, corrupt/torn-write recovery, full key-disaster recovery, and
multi-region drills executed by operators who do not share credentials or a
control plane.

## Observability and incident response

The restored tools expose height/tip/root agreement, mempool state, registry
identity, state verification, retained-history/index readiness, read/write RPC
posture, bounded response latency and public Orchard counters. Runbooks define
stop-on-divergence and evidence-redaction rules.

The node metrics and monitor now expose total/recent certificate votes, each
node's recent certificate participation in integer parts-per-million, and a
node-observed Unix-millisecond clock sample. The monitor has ordered
warning/critical thresholds for height lag, cross-validator clock skew, RPC p95
and mempool depth; warns below the reviewed certificate-participation floor;
treats unknown receipt semantics as critical; warns on any recent rejected
receipt by default; and exposes correctly sourced ordering/execution/storage
counters. Storage metrics use checked `statvfs` arithmetic to report total,
available, and available-parts-per-million filesystem capacity. The monitor is
critical when capacity is absent or at/below 5% and warns at/below 15%. A real
four-node RPC smoke and synthetic threshold regressions pass.
Supported AssetOrchard private-egress and private-swap consensus verification
now records the exact Halo2 `verify_proof` duration, excluding key setup and
state execution, in private local operator metadata. The metrics RPC exposes
the latest integer microsecond sample and observation time. Telemetry-write
failure is non-gating for consensus and emits a local warning; corrupted
telemetry fails closed at the metrics boundary. The monitor warns above 5 s,
is critical above 15 s, and treats a previously observed sample older than
5 minutes as critical. A release-profile real-proof regression verifies both
accepted and rejected timing records without double-counting cloned collectors.
Warning/critical monitor results can also be atomically emitted as private,
content-identified alert envelopes using `--alert-spool-dir`; symlink spools
are rejected and repeated emission is idempotent.
The RPC listener now overlays current, configured-limit, peak, and cumulative
accepted connection counts plus exact integer utilization onto the canonical
`metrics` response. This measures saturation at the listener instead of using
latency as a proxy. The monitor warns at 75%, is critical at 95%, and treats a
missing value as critical.
`systemd/postfiat-logrotate.example` retains 14
daily compressed rotations with a 100-MiB early-rotation bound; its parser and
policy regression passes.

The following production controls remain absent and block a real-value
production launch, while the corresponding runtime is explicitly
feature-contained for source publication: maintained alert
delivery/export/dashboard integration,
structured log reopening without
the `copytruncate` race, public incident communication, multi-region fault
drills, and evidence that no single founder account or host is required. Until
those are implemented and drilled, the repository supports controlled
pre-testnet operation only.

`docs/runbooks/incident-response.md` now defines measurable controlled-pretestnet
SLOs, SEV-1/SEV-2 acknowledgement and incident-command deadlines, escalation,
public-update deadlines, evidence handling, and closure criteria. Those fields
are embedded in every durable alert envelope and covered by regression tests.
This is a defined response contract, not evidence of external delivery or an
independent operator drill.

## Verification commands

```text
PYTHONPATH=python python3 -m unittest python.tests.test_safe_rollout -v
scripts/test-public-runtime-default-scan
scripts/public-runtime-default-scan
scripts/testnet-rpc-method-inventory --output <tmp>/inventory.json --markdown <tmp>/inventory.md
python3 -m py_compile scripts/testnet-validator-doctor scripts/testnet-rpc-doctor scripts/testnet-monitor-snapshot scripts/testnet-rpc-method-inventory scripts/postfiat-rpc-account-tx
python3 -m unittest python.tests.test_monitor_snapshot -v
scripts/test-postfiat-logrotate
cargo test -p postfiat-node deployment_validator_unit_stage_is_canonical_and_non_overwriting
cargo test --release -p postfiat-privacy-orchard swap_consensus_verifier_accepts_real_proof_and_rejects_forged_nonconservation -- --ignored --nocapture
```

The Python roster regression passes 13/13, including a one-member
`validator-0` rejection and six complete-roster acceptance. The runtime-default
scanner passes after removing all live validator IP defaults and global TLS
verification disables from product/runtime surfaces. Rust unit-stage evidence
is recorded in the audit lab book after the current integrated suite releases
the Cargo test lock.
