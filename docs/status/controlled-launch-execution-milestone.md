# Controlled Launch Execution Milestone

Status: active launch-execution checklist
Date: 2026-05-14
Scope: PostFiat L1 v2 controlled testnet

This document defines what remains between the previously cut controlled-testnet
candidate and a completed controlled launch. The release candidate was already
cut and recut against the optimized finality path. Launch execution now means
running the operator packet and recording post-launch evidence.

## Current Decision

Controlled launch execution has now passed on the live controlled operator
surface. Five validator slots were installed and started from an
operator-private, credential-bound package; all validator/RPC services came up;
the network converged; certified transparent rounds have continued advancing
the chain; and a real SDK wallet flow funded, quoted, signed, submitted through
a temporary SSH-local controlled write edge, ordered, and verified `tx`
finality through live read RPC. A refreshed live Orchard direct-deposit
privacy-alpha round has also finalized on the same controlled network with the
public write edge closed.

The running package was cut after the optimized `56db87a` candidate to add
launch tooling and gates. Its package revision is
`9e4fb20fb1dd1cde0b2b00d595351a0075120b7f`; the live-launch executor fixes
were committed through `fe00519`.

The optimized local 5-validator finality path now has 20-round evidence:
`reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`
reports `submit_to_finality` p50 `1.563s`, p95 `1.709s`, p99 `1.753s`.

The optimized remote 5-validator peer-certified path now has 20-round evidence:
`reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`
reports peer-certified round total p50 `1.032s`, p95 `1.116s`, p99 `1.139s`.
Fresh overnight live launch evidence on 2026-05-16 is green at height `51`:
SDK wallet finality advanced from height `48` to `50`, Orchard direct deposit
advanced from height `50` to `51`, account-history index refresh passed on all
five validators, and RPC doctor, Python RPC smoke, monitor snapshot, validator
doctor, and remote observability all passed with zero height lag:
`reports/testnet-live-wallet-finality/overnight-finality-refresh-20260516T090806Z/testnet-live-wallet-finality.json`,
`reports/testnet-live-orchard-direct-deposit/overnight-orchard-refresh-20260516T090925Z/testnet-live-orchard-direct-deposit.json`,
`reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T091319Z/testnet-live-account-tx-index-refresh.json`,
`reports/testnet-rpc-doctor/testnet-rpc-doctor-overnight-refresh-20260516T091420Z.json`,
`reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-overnight-refresh-20260516T091420Z.json`,
`reports/testnet-monitor-snapshot/testnet-monitor-snapshot-overnight-refresh-20260516T091420Z.json`,
`reports/testnet-live-validator-doctor/overnight-validator-doctor-20260516T091420Z/testnet-live-validator-doctor.json`,
and
`reports/testnet-remote-observability/testnet-remote-observability-20260516T091420Z.json`.
Follow-on live evidence on 2026-05-16 is green at height `54`: wallet finality
advanced from height `51` to `53`, Orchard direct deposit advanced from height
`53` to `54`, account-history refresh passed, and RPC doctor, Python RPC
smoke, monitor snapshot, Python account-history pull, validator doctor, and
remote observability passed with zero height lag:
`reports/testnet-live-wallet-finality/overnight-finality-refresh-20260516T094649Z/testnet-live-wallet-finality.json`,
`reports/testnet-live-orchard-direct-deposit/overnight-orchard-refresh-20260516T094813Z/testnet-live-orchard-direct-deposit.json`,
`reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T095215Z/testnet-live-account-tx-index-refresh.json`,
`reports/testnet-rpc-doctor/testnet-rpc-doctor-overnight-refresh-20260516T095305Z.json`,
`reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-overnight-refresh-20260516T095306Z.json`,
`reports/testnet-monitor-snapshot/testnet-monitor-snapshot-overnight-refresh-20260516T095306Z.json`,
`reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-overnight-refresh-20260516T095306Z.json`,
`reports/testnet-live-validator-doctor/overnight-validator-doctor-20260516T095306Z/testnet-live-validator-doctor.json`,
and
`reports/testnet-remote-observability/testnet-remote-observability-20260516T095306Z.json`.
The new `scripts/testnet-live-evidence-refresh` wrapper also passed a
read-only sweep at height `54`:
`reports/testnet-live-evidence-refresh/live-evidence-readonly-20260516T100102Z/testnet-live-evidence-refresh.json`.
It also passed the SSH-inclusive read-only operator sweep at height `54`:
`reports/testnet-live-evidence-refresh/live-evidence-ssh-readonly-20260516T101651Z/testnet-live-evidence-refresh.json`.
Full clean-tree wrapper evidence on pushed `7e7c910` passed with live write
gates and SSH checks included: wallet finality advanced from height `57` to
`59`, Orchard direct deposit advanced from height `59` to `60`, and all
read/monitor/validator/observability checks passed at height `60` with zero
lag:
`reports/testnet-live-evidence-refresh/live-evidence-full-clean-20260516T104752Z/testnet-live-evidence-refresh.json`.
Fresh current-head wrapper evidence on pushed `d0a9683` passed with live write
gates and SSH checks included: wallet finality advanced from height `60` to
`62`, Orchard direct deposit advanced from height `62` to `63`, and all
read/monitor/validator/observability checks passed at height `63` with zero
lag:
`reports/testnet-live-evidence-refresh/live-evidence-full-current-20260516T110640Z/testnet-live-evidence-refresh.json`.
Python RPC account-history tooling now has a reusable
`PostFiatRpcClient.account_tx_history()` helper, with live five-endpoint
evidence returning 11 converged rows per endpoint across three complete indexed
windows:
`reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-python-history-helper-live-20260516T112425Z.json`.
Post-commit read-only aggregate evidence on pushed `fbb7a0a` passed at height
`63`, including RPC doctor, Python RPC smoke with the history helper, monitor
snapshot, and standalone account-history pull:
`reports/testnet-live-evidence-refresh/python-history-helper-readonly-20260516T112740Z/testnet-live-evidence-refresh.json`.
Monitor snapshot account-history canary support landed in `3bd5e6a`.
`scripts/testnet-monitor-snapshot --include-account-tx-history` now embeds the
bounded multi-window account-history check directly in the monitor report,
including per-endpoint row/window counts, all-indexed status, archive lookup
counts, and retained-history scan counts. Clean-tree read-only aggregate
evidence on `3bd5e6a` passed at height `66` with `git.dirty=false`; RPC
doctor, Python RPC smoke, monitor snapshot with embedded account-history, and
standalone account-history pull all passed across five endpoints with zero
height lag:
`reports/testnet-live-evidence-refresh/monitor-history-clean-20260516T115737Z/testnet-live-evidence-refresh.json`.
The monitor report shows embedded history enabled and passing, with 12 rows on
the first endpoint across three complete indexed windows and zero archive
lookups/scans:
`reports/testnet-monitor-snapshot/testnet-monitor-snapshot-monitor-history-clean-20260516T115737Z.json`.
Follow-on SSH-inclusive read-only aggregate evidence on pushed `cb06510`
passed with `git.dirty=false`: account-history index refresh, RPC doctor,
Python RPC smoke, monitor snapshot with embedded account-history, standalone
account-history pull, validator doctor, and remote observability all passed at
height `66` with zero height lag:
`reports/testnet-live-evidence-refresh/monitor-history-ssh-clean-20260516T120349Z/testnet-live-evidence-refresh.json`.
Full current-head launch/privacy/finality evidence on pushed `cff47ba` passed
with `git.dirty=false`, live write gates included, and SSH checks included:
`reports/testnet-live-evidence-refresh/full-launch-current-cff47ba-20260516T121723Z/testnet-live-evidence-refresh.json`.
SDK wallet finality advanced from height `66` to `68`, Orchard direct deposit
advanced from height `68` to `69`, account-history index refresh passed, and
RPC doctor, Python RPC smoke, monitor snapshot with embedded account-history,
standalone account-history pull, validator doctor, and remote observability
all passed at height `69` with zero height lag. The monitor history canary
returned 13 rows on the first endpoint across three complete indexed windows
with zero archive lookups/scans:
`reports/testnet-monitor-snapshot/testnet-monitor-snapshot-full-launch-current-cff47ba-20260516T121723Z.json`.
The next milestone is post-launch hardening beyond the now-passed 100-round
live continuity window, below-quorum outage drill, host-group capture-threshold
outage, bounded mixed read/write load, and fresh live observability sweep:
broader partition/load drills on the live deployment, persistent write-edge
installation/exposure evidence, public endpoint load evidence, and continued
storage/runtime hardening. The controlled write-edge policy/audit is now
documented at `docs/runbooks/controlled-write-edge-policy.md` and
`reports/testnet-controlled-write-edge-policy/testnet-controlled-write-edge-policy-20260514T171402Z.json`;
it proves validator RPC remains read-only by default and that the live wallet
write proof was bounded/SSH-local. It does not prove a persistent externally
exposed write edge.

Before mutating live machines, run the fail-closed prep checker:

```sh
PACKAGE_DIR=reports/testnet-release-packages/testnet-release-package-20260514T145919Z \
  SSH_CRED_FILE=/path/to/machine-credentials.txt \
  scripts/testnet-controlled-launch-prep-check
```

The checker verifies the package manifest, topology host binding, matching
operator-private validator material, fake-root install/key validation for every
validator slot, credential parsing, and redaction. The current optimized public
package is release evidence, but it is not by itself a live mutable launch
artifact because its matching private material was intentionally removed after
rehearsal and its package topology still uses placeholder
`validator-N.testnet.local` hosts. A live launch needs either a freshly cut
operator-private package with credential-bound topology and retained private
material, or an explicit host-alias plus matching private-material handoff.

This remains a controlled-testnet claim only. It is not a public mainnet,
production privacy, bridge-custody, broad decentralization, or TPS claim.

## Launch Inputs

- Optimized candidate revision:
  `56db87a1f6f5be0dfe936c4619931aaefbbeffb5`.
- Live package revision:
  `9e4fb20fb1dd1cde0b2b00d595351a0075120b7f`.
- Final candidate report:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/testnet-release-final-candidate-20260514T-current-head-56db87a-optimized-latency.json`.
- Release package:
  `reports/testnet-release-packages/testnet-release-package-20260514T145919Z`.
- Operator launch packet:
  `reports/testnet-operator-launch-packet/current-56db87a-optimized-latency/testnet-operator-launch-packet.json`.
- Operator launch packet markdown:
  `reports/testnet-operator-launch-packet/current-56db87a-optimized-latency/operator-launch-packet.md`.
- Release status:
  `reports/testnet-release-status-current-56db87a-optimized-latency/testnet-release-status-current-56db87a-optimized-latency.json`.
- Controlled-launch evidence pack:
  `reports/testnet-controlled-launch-evidence-pack/head-56db87a-optimized-latency/testnet-controlled-launch-evidence-pack.json`.
- Fresh SDK-signer remote P0 evidence:
  `reports/testnet-p0-network-gate-remote-head-c18d590-sdk-signer/testnet-p0-network-gate-20260514T015505Z.json`.
- Exact artifact remote-join rehearsal:
  `reports/testnet-release-final-candidate-current-head-56db87a-optimized-latency/candidate/release-gate/release-gate/logs/exact-remote-join-dry-run.json`.
- Live prep checker:
  `scripts/testnet-controlled-launch-prep-check`.
- Live launch executor:
  `scripts/testnet-release-live-launch`.
- Live wallet finality executor:
  `scripts/testnet-live-wallet-finality`.
- Current package prep-check report:
  `reports/testnet-controlled-launch-prep-check/current-package-prep-check-20260514.json`.
- Operator-private live prep report:
  `reports/testnet-live-operator-artifact-20260514T-prep/launch-prep-check.json`.
- Operator-private exact remote-join dry run:
  `reports/testnet-live-operator-artifact-20260514T-prep/remote-join-dry-run/testnet-release-remote-join-dry-run.json`.
- Live launch report:
  `reports/testnet-live-launch-20260514T151903Z-head-9e4fb20-rerun5/testnet-release-live-launch.json`.
- Live SDK wallet finality report:
  `reports/testnet-live-wallet-finality/overnight-20260516T073001Z/testnet-live-wallet-finality.json`.
- Post-account-index-catchup live SDK wallet finality report:
  `reports/testnet-live-wallet-finality/post-upgrade-wallet-finality-20260516T080751Z/testnet-live-wallet-finality.json`.
- Live Orchard direct-deposit privacy-alpha report:
  `reports/testnet-live-orchard-direct-deposit/overnight-20260516T073133Z/testnet-live-orchard-direct-deposit.json`.
- Post-account-index-catchup live Orchard direct-deposit privacy-alpha report:
  `reports/testnet-live-orchard-direct-deposit/post-upgrade-orchard-direct-deposit-20260516T080908Z/testnet-live-orchard-direct-deposit.json`.
- Post-privacy live observability report:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T063929Z.json`.
- Live binary/RPC index upgrade report:
  `reports/testnet-live-orchard-binary-upgrade/live-rpc-index-20260516T065327Z/testnet-live-orchard-binary-upgrade.json`.
- Live account-history index refresh report:
  `reports/testnet-live-account-tx-index-refresh/live-account-tx-index-20260516T073517Z/testnet-live-account-tx-index-refresh.json`.
- Post-account-index-catchup live binary upgrade report:
  `reports/testnet-live-orchard-binary-upgrade/live-account-tx-index-catchup-upgrade-20260516T080506Z/testnet-live-orchard-binary-upgrade.json`.
- Post-account-index-catchup live account-history index refresh report:
  `reports/testnet-live-account-tx-index-refresh/post-upgrade-account-tx-index-20260516T081246Z/testnet-live-account-tx-index-refresh.json`.
  It passed at height `45` with present/usable path-redacted indexes on all
  five validators, 27 indexed rows, and 21 accounts.
- Fresh overnight live account-history index refresh report:
  `reports/testnet-live-account-tx-index-refresh/overnight-account-tx-index-20260516T091319Z/testnet-live-account-tx-index-refresh.json`.
  It passed at height `51` with aggregate and disk indexes present/usable and
  path-redacted on all five validators.
- Disk-only account-history RPC/CLI smoke:
  `reports/testnet-account-tx-disk-index-smoke/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`.
  It removes the aggregate `account_tx_index.json` on a local read-only RPC
  validator and proves disk-backed `account_tx` still returns a finalized
  canary row with zero retained-history scan and zero archive lookup. The
  paired monitor snapshot
  `reports/testnet-monitor-snapshot/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`
  reports `status=ok` with aggregate absent and disk index usable, and the
  paired validator doctor
  `reports/testnet-validator-doctor/testnet-account-tx-disk-index-smoke-20260516T093544Z.json`
  reports `account_tx_index_all_ready=true` under the same condition.
- Effective-index live validator doctor:
  `reports/testnet-live-validator-doctor/effective-index-validator-doctor-20260516T093606Z/testnet-live-validator-doctor.json`.
  It passed on all five live validators at height `51`.
- Live Python account-history pull report:
  `reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-canary-20260516T093108Z.json`.
  It queried all five live read-only endpoints for the public wallet canary
  through height `51`, required row fingerprint convergence, and returned
  indexed history with zero archive lookups and zero retained-history scans.
- Full current-head live RPC doctor report:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T073552Z.json`
  passed against all 16 checked read-only methods, including `account`,
  indexed `account_tx`, and `account_tx_index_status`, with five endpoints at
  height `42`, zero height lag, and present/usable path-redacted
  account-history indexes on every validator.
- Post-account-index-catchup live RPC doctor report:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T081331Z.json`
  passed across the five upgraded read-only endpoints at height `45`, zero
  height lag, indexed `account_tx`, and present/usable account-history indexes.
- Live Python RPC client smoke report:
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-live-python-rpc-20260516T073702Z.json`
  passed against all five deployed read-only endpoints, checked 16 client
  methods per endpoint, and proved indexed `account_tx` rows for the public
  wallet canary.
- Post-account-index-catchup live Python RPC client smoke report:
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-post-upgrade-python-rpc-20260516T081412Z.json`.
- Live monitor snapshot report:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-live-monitor-20260516T073741Z.json`
  passed with five online/read-only endpoints, BFT quorum observable, zero
  height lag, no warnings/criticals, indexed account-history canary rows, and
  Orchard public pool counters.
- Post-account-index-catchup live monitor snapshot report:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-post-upgrade-monitor-20260516T081503Z.json`.
- Live validator doctor report:
  `reports/testnet-live-validator-doctor/live-validator-doctor-20260516T073826Z/testnet-live-validator-doctor.json`
  passed with all five validator/RPC service pairs active, local state
  verified, partial-history retention ready, current account-history indexes,
  local split validator keys valid with safe permissions, matching binary hash,
  and full convergence at height `42`.
- Post-account-index-catchup live validator doctor report:
  `reports/testnet-live-validator-doctor/post-upgrade-validator-doctor-20260516T081546Z/testnet-live-validator-doctor.json`
  passed with all five validator/RPC service pairs active, state verified,
  partial-history retention ready, matching binary hash, current
  account-history indexes, and full convergence at height `45`.
- Post-upgrade live observability report:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T074314Z.json`.
- Post-account-index-catchup live observability report:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T082041Z.json`.
- Disk-backed account-history read-index live binary upgrade report:
  `reports/testnet-live-orchard-binary-upgrade/live-account-tx-disk-index-upgrade-20260516T084228Z/testnet-live-orchard-binary-upgrade.json`.
- Disk-backed account-history live refresh report:
  `reports/testnet-live-account-tx-index-refresh/live-account-tx-disk-index-20260516T084521Z/testnet-live-account-tx-index-refresh.json`.
  It passed at height `45` with aggregate and disk indexes present/usable,
  path-redacted, and 21 disk account shards on all five validators.
- Post-disk-index live SDK wallet finality report:
  `reports/testnet-live-wallet-finality/post-disk-index-wallet-finality-20260516T084601Z/testnet-live-wallet-finality.json`.
  It passed from height `45` to `47`.
- Post-disk-index live Orchard direct-deposit privacy-alpha report:
  `reports/testnet-live-orchard-direct-deposit/post-disk-index-orchard-direct-deposit-20260516T084737Z/testnet-live-orchard-direct-deposit.json`.
  It passed from height `47` to `48`.
- Post-disk-index live RPC doctor report:
  `reports/testnet-rpc-doctor/testnet-rpc-doctor-20260516T085123Z.json`.
- Post-disk-index live Python RPC client smoke report:
  `reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-live-disk-index-python-rpc-20260516T085206Z.json`.
- Post-disk-index live monitor snapshot report:
  `reports/testnet-monitor-snapshot/testnet-monitor-snapshot-live-disk-index-monitor-20260516T085247Z.json`.
- Post-disk-index live validator doctor report:
  `reports/testnet-live-validator-doctor/live-disk-index-validator-doctor-20260516T085327Z/testnet-live-validator-doctor.json`.
  It passed at height `48` with all five validator/RPC service pairs active,
  matching binary hash, current aggregate and disk account-history indexes, 29
  indexed rows, 22 disk account shards, and full convergence.
- Post-disk-index live observability report:
  `reports/testnet-remote-observability/testnet-remote-observability-20260516T085805Z.json`.
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
- Placement manifest:
  `docs/status/controlled-testnet-placement-manifest.json`.

## Completion Definition

Controlled launch is complete when all of the following are true:

- Five controlled validator slots are installed from the current release
  package.
- Validator and RPC services are running under the launch package configuration.
- All validators report the same chain id, genesis hash, validator registry
  root, block tip, and state root.
- Read-only RPC checks pass against the live services.
- At least one transparent certified round is recorded after launch.
- A wallet/SDK quote -> sign -> submit -> `tx` finality flow is recorded
  against the live launch surface or a documented controlled write edge.
- Post-launch status, service logs, launch evidence, and redaction scans are
  saved under a stable `reports/` path.
- `docs/status/chain-state-current.md`,
  `docs/status/controlled-testnet-burndown.md`, and
  `docs/status/public-claims-checklist.md` point at the post-launch evidence.

## Milestone Checklist

| ID | Item | Status | Exit Condition |
| --- | --- | --- | --- |
| CL-001 | Freeze launch candidate | Done | Candidate revision, final candidate, release package, release status, operator launch packet, remote P0, and controlled-launch evidence pack all point at `56db87a`. |
| CL-002 | Keep automation quiet during manual launch | Conditional | WHIP may be enabled for latency implementation while launch is paused. Before any manual launch execution, disable the `l1` WHIP cron block so tmux automation cannot inject commands during launch. |
| CL-002A | Clear finality-latency gate | Done | Local 5-validator submit-to-finality passes p50 `<2s` and p95 `<4s` without material height growth: `reports/testnet-tx-finality-latency-benchmark/local-gate-20round-20260514T142731Z/testnet-tx-finality-latency-benchmark.json`. Remote 5-validator peer-certified latency passes p50 `2-5s`, p95 `5-10s`: `reports/testnet-remote-ssh-smoke/optimized-latency-20260514T143534Z/testnet-remote-ssh-smoke.json`. |
| CL-003 | Verify package locally | Ready | Run the package-local verifier from `reports/testnet-release-packages/testnet-release-package-20260514T145919Z/verify-package-manifest.sh`; it must pass immediately before launch. |
| CL-004 | Refresh host preflight if environment changed | Conditional | If machines, credentials, sudo/systemd, ports, or staging paths changed after the current preflight report, rerun host preflight and attach the new report before launch. |
| CL-005 | Rehearse exact remote join if environment changed | Conditional | If package, private material, hosts, or staging changed after the current exact join rehearsal, rerun exact fake-root remote join and attach the new report. |
| CL-005A | Run fail-closed live prep check | Done | Operator-private package prep passed with credential-derived topology and retained per-validator private material: `reports/testnet-live-operator-artifact-20260514T-prep/launch-prep-check.json`. The older public package prep failure remains useful evidence that placeholder topology and removed private material fail closed. |
| CL-006 | Stage validator private material | Done | `scripts/testnet-release-live-launch` uploaded validator-specific private material, installed from it, then removed staged private material. Live report check `all_private_stage_material_removed=true`. |
| CL-007 | Install validator slots | Done | Five validator slots were installed from the operator-private package by `scripts/testnet-release-live-launch`; all result rows passed install, checksum, manifest, binary, state, and service checks. |
| CL-008 | Start services | Done | Validator and read-only RPC systemd services are active for all five slots in `reports/testnet-live-launch-20260514T151903Z-head-9e4fb20-rerun5/testnet-release-live-launch.json`. |
| CL-009 | Verify convergence | Done | Initial convergence passed at height 0 and post-round convergence passed at height 1 in the live launch report; live wallet finality later converged all five validators at height 6. |
| CL-010 | Verify live RPC and finality | Done | Live launch recorded a certified transparent round; `scripts/testnet-live-wallet-finality` recorded SDK wallet quote/sign/submit/`tx` finality through live read RPC plus a temporary SSH-local controlled write edge. |
| CL-011 | Record post-launch evidence | Done | Live launch, live wallet finality, prep, exact-join, and redaction scan evidence are written under stable `reports/` paths listed above. |
| CL-012 | Publish post-launch status | Done | Current-state, burndown, public-claims checklist, and handoff docs point at live launch, live wallet finality, and first post-launch hardening evidence. |
| CL-013 | Start post-launch hardening | In progress | Live continuity passed first for five rounds from height 11 to 16, then for 20 rounds from height 18 to 38, and now for 100 rounds to height 141. Live all-validator restart passed at height 16. Post-restart certified ordering passed to height 18. Live single-validator partial outage passed at height 39 with a 4-of-5 quorum, first-attempt recovery replay, and post-outage certified ordering to height 41. Live below-quorum outage passed at height 142: stopping two validators left only three online against a four-vote quorum, the attempted round failed without state advance, restart caused no state advance, and recovery certified with all five votes; post-recovery ordering then advanced to height 144. Live host-group outage passed at height 165: a two-validator operator-host group that can block quorum but cannot form quorum was stopped, the below-quorum attempt did not advance state, and recovery converged. Live mixed read/write load passed with 600 SDK-validated reads across 12 methods while ten certified rounds advanced from height 154 to height 164, followed by post-mixed convergence. Fresh live observability passed at height 164 with zero height lag after that mixed run. Live RPC oversized edge-load passed on all five validators. Live RPC read-load passed with 300 SDK-validated read requests across six methods, then 1,200 SDK-validated requests across 12 methods with post-run convergence at height 164. Controlled write-edge policy audit passed, proving read-only validator RPC units and bounded SSH-local wallet submit evidence. `scripts/testnet-live-hardening-evidence-pack` now validates the live hardening window including below-quorum, host-group outage, mixed-load, observability, and 1,200-read broad RPC evidence. Remaining work: broader partition/load evidence, external-WAN RPC load testing, persistent write-edge installation only if external write access is needed, and independent-operator onboarding work. |
| CL-014 | Recut optimized release evidence | Done | Final candidate, release status, operator launch packet, controlled-launch evidence pack, and public claims checklist all point at the optimized finality path and pass with `git.dirty=false`. |

## Stop Conditions

Stop launch and preserve evidence if any of these occur:

- Package manifest verification fails.
- Host preflight fails on SSH, sudo, systemd, required commands, staging
  writes, or redaction.
- Exact remote-join rehearsal fails or leaves remote staging behind.
- Live prep check fails on manifest verification, topology host binding,
  private material, fake-root install, key validation, credential parsing, or
  redaction.
- Install scripts accept the wrong validator key or wrong validator slot.
- A validator reports a mismatched chain id, genesis hash, validator registry
  root, block tip, or state root.
- RPC read checks fail after service start.
- A certified transparent round cannot form a quorum certificate.
- Wallet/SDK finality cannot verify the submitted transaction.
- Any public report contains private key, seed, mnemonic, SSH credential,
  password, or machine credential material.

## Next Command Surface

The operator packet is the launch source of truth:

```sh
reports/testnet-release-packages/testnet-release-package-20260514T145919Z/verify-package-manifest.sh \
  reports/testnet-release-packages/testnet-release-package-20260514T145919Z
```

After that, follow:

- `reports/testnet-operator-launch-packet/current-56db87a-optimized-latency/operator-launch-packet.md`
- `docs/runbooks/controlled-testnet-operator-launch.md`
- `reports/testnet-release-packages/testnet-release-package-20260514T145919Z/RUNBOOK.md`

For the operator-private live artifact that has already passed prep and
exact-join rehearsal, the live executor shape is:

```sh
POSTFIAT_CONFIRM_LIVE_LAUNCH=1 \
  VALIDATORS=5 \
  SSH_CRED_FILE=/path/to/machine-credentials.txt \
  PACKAGE_DIR=/path/to/operator-private/package \
  PRIVATE_MATERIAL_DIR=/path/to/operator-private/private-material \
  scripts/testnet-release-live-launch
```

## Post-Launch Evidence Packet

The first post-launch packet should include:

- package manifest verification output;
- host preflight output if refreshed;
- exact remote-join output if refreshed;
- per-validator install and service status output;
- convergence/status output;
- live RPC read checks;
- one certified transparent round;
- SDK wallet quote/sign/submit/finality evidence;
- redaction scan result;
- updated release status or launch-status report;
- updated current-state and public-claims docs.
