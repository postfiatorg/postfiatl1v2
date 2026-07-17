# Operator Day Two Runbook

Status: controlled-testnet operator checklist  
Date: 2026-05-16

This runbook is for an operator who already has a validator data directory and
systemd services installed. It separates validator service health, read-only
public RPC, controlled write RPC, and local evidence collection.

## Do Not Publish

Never paste or upload:

- `validator_keys.json`
- `faucet_key.json`
- wallet backups, master seeds, spending keys, view keys, note randomness, or
  private SSH material
- raw service logs before running a redaction-safe doctor if there is any
  chance they include request payloads

Public evidence should come from the scripts below, not from ad hoc file dumps.

## Validator Health

Run the local validator doctor from the repo:

```bash
scripts/testnet-validator-doctor \
  --data-dir /var/lib/postfiat/validator-0 \
  --validator-service postfiat-validator-0.service \
  --rpc-service postfiat-rpc-0.service
```

For a machine with multiple validator data dirs:

```bash
scripts/testnet-validator-doctor \
  --data-root /var/lib/postfiat \
  --validator-count 5 \
  --validator-service-template 'postfiat-{node_id}.service' \
  --rpc-service-template 'postfiat-rpc-{index}.service'
```

Green means the local node is running, state verification passes, the active
registry root is available, partial-history retention is ready, key-file
permissions are tight, retained account-history index status is visible, and
observed validators are height/root consistent.

For the controlled live fleet:

```bash
RUN_ID=live-validator-doctor-$(date -u +%Y%m%dT%H%M%SZ) \
  SSH_CRED_FILE=/path/to/machine-credentials.txt \
  scripts/testnet-live-validator-doctor
```

## Read-Only RPC Health

Public RPC should be read-only by default:

```bash
scripts/testnet-rpc-doctor \
  --endpoint validator-0=127.0.0.1:27650 \
  --account-address "$FAUCET_OR_CANARY_ADDRESS"
```

Multi-endpoint:

```bash
scripts/testnet-rpc-doctor \
  --endpoint validator-0=127.0.0.1:27650 \
  --endpoint validator-1=127.0.0.1:27651
```

The doctor checks read methods, latency, height lag, registry-root
consistency, response size, schema shape, and write posture. When
`--account-address` is supplied it also checks transparent `account` and
server-side bounded `account_tx`.

## Monitor Snapshot

For cron-style collection:

```bash
scripts/testnet-monitor-snapshot \
  --endpoint validator-0=127.0.0.1:27650 \
  --endpoint validator-1=127.0.0.1:27651 \
  --account-address "$FAUCET_OR_CANARY_ADDRESS"
```

For the controlled live topology:

```bash
RUN_ID=live-monitor-$(date -u +%Y%m%dT%H%M%SZ)
scripts/testnet-monitor-snapshot \
  --endpoint-file reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json \
  --account-address "$PUBLIC_CANARY_ADDRESS" \
  --include-account-tx-history \
  --account-tx-history-require-row \
  --timeout-seconds 12 \
  --no-build \
  --alert-spool-dir /var/lib/postfiat/monitor-alerts \
  --log-dir "reports/testnet-monitor-snapshot/logs-$RUN_ID" \
  --output "reports/testnet-monitor-snapshot/testnet-monitor-snapshot-$RUN_ID.json"
```

This wraps RPC doctor output into a compact report with threshold status,
mempool counters, total/recent certificate vote counts, per-node recent
certificate participation, validator clock-skew measurement, correctly sourced
ordering/execution/storage counters, a
bounded recent-receipt sample split into accepted/rejected/unknown semantics,
Orchard public pool counters, and optional transparent account / `account_tx`
canary status. Unknown receipt semantics are critical. Height lag, RPC p95 and
mempool depth have ordered warning/critical thresholds; any recent rejected
receipt warns by default. Certificate participation below 800,000 ppm warns;
clock skew warns above 1,000 ms and is critical above 5,000 ms. RPC active
connection utilization warns at 750,000 ppm and is critical at 950,000 ppm;
missing saturation telemetry is critical. Use the explicit
`--warn-*` and `--critical-*`
arguments only with a reviewed operating policy rather than hiding an alert.

When `--alert-spool-dir` is configured, each warning or critical snapshot also
emits an idempotent `postfiat.monitor-alert.v1` JSON envelope. The directory is
required to be owned by the running user and not a symlink; it is mode 0700 and
events are atomically persisted mode 0600 before the command returns. Point a
separately authenticated delivery agent at this spool. The monitor deliberately
does not execute shell hooks or embed pager credentials. An undelivered spool
is not proof that an operator was paged; alert-delivery health must be monitored
and drilled separately.

Add `--include-account-tx-history` when the monitor should also embed a
bounded multi-window account-history canary. It runs the same helper used by
`scripts/postfiat-rpc-account-tx`, records per-endpoint row/window counts,
whether all reads used the retained account-history index, and whether any
archive lookups or retained-history scans were needed. Use
`--account-tx-history-require-row` for public wallet canaries that should
already have visible history.

## Log Rotation

Install `systemd/postfiat-logrotate.example` as
`/etc/logrotate.d/postfiat`. It retains 14 daily rotations, rotates early at
100 MiB, compresses old logs, and covers both flat and per-validator log
directories. Validate the exact installed policy before enabling it:

```bash
scripts/test-postfiat-logrotate
sudo logrotate --debug /etc/logrotate.d/postfiat
```

`copytruncate` is required by the current append-file services because they do
not yet expose a coordinated reopen signal. Operators must account for its
small copy/truncate race in incident evidence; a structured logging sink with
atomic rotation remains preferable for real-value production.

## Live Evidence Refresh

For a one-command read-only live evidence sweep:

```bash
scripts/testnet-live-evidence-refresh \
  --account-address "$PUBLIC_CANARY_ADDRESS"
```

To skip SSH checks and only exercise public/read-only RPC surfaces:

```bash
scripts/testnet-live-evidence-refresh \
  --account-address "$PUBLIC_CANARY_ADDRESS" \
  --skip-ssh-checks
```

For the full controlled operator sweep, including SSH validator checks and the
bounded live wallet / Orchard write gates:

```bash
scripts/testnet-live-evidence-refresh \
  --account-address "$PUBLIC_CANARY_ADDRESS" \
  --include-write-gates
```

The full mode advances live chain height by design. The script writes an
aggregate report under `reports/testnet-live-evidence-refresh/` and keeps
individual tool reports in their existing report directories. Remote
observability uses light public RPC reads by default; local state verification
is covered by validator doctor. Add `--include-remote-verify-state` only when
you explicitly want expensive `verify_state` RPC reads in the monitor path.

## Python RPC Client Smoke

Use the Python client for integration/user-facing checks:

```bash
PYTHONPATH=python python3 - <<'PY'
from postfiat_rpc import PostFiatRpcClient

client = PostFiatRpcClient("127.0.0.1:27650")
print(client.server_info())
print(client.ledger(limit=5))
PY
```

`account_tx` now uses the server-side bounded transparent history read when the
endpoint supports it, with a client-side bounded scan fallback for older
endpoints. Current endpoints use an auto-refreshed retained-history index when
it is fresh, append missing blocks during normal commit refresh when the
previous cache tip is a known ancestor, prefer disk-backed per-account shards
for reads, and fall back to aggregate index / bounded retained-history scan
when the disk index is absent or stale.

For a live read-only fleet smoke with public account-history canary:

```bash
scripts/testnet-live-python-rpc-client-smoke \
  --endpoint-file reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json \
  --account-address "$PUBLIC_CANARY_ADDRESS" \
  --require-account-tx-row
```

For a bounded historical account-transaction pull similar to an integration
client workflow:

```bash
scripts/postfiat-rpc-account-tx \
  --endpoint-file reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json \
  --address "$PUBLIC_CANARY_ADDRESS" \
  --from-height 0 \
  --window-size 25 \
  --limit-per-window 64 \
  --require-row \
  --require-convergence
```

The report redacts endpoint hosts, writes only public account-history data,
and fails closed on truncated windows unless `--allow-truncated` is explicit.

For one-off Python-client-backed reads, use:

```bash
scripts/postfiat-rpc-query \
  --endpoint validator-0=127.0.0.1:27650 \
  --method status
```

For a single-endpoint bounded history canary:

```bash
scripts/postfiat-rpc-query \
  --endpoint validator-0=127.0.0.1:27650 \
  --method account_tx_history \
  --address "$PUBLIC_CANARY_ADDRESS" \
  --from-height 0 \
  --window-size 25 \
  --limit-per-window 64 \
  --require-row
```

The CLI writes `postfiat-rpc-query-v1` JSON reports under
`reports/postfiat-rpc-query/` by default and redacts non-local endpoint hosts.

## Account History Index

Ordered block commit refreshes the index on a best-effort basis. The hot path
appends a cache whose tip is still a known ancestor of the local tip and falls
back to full retained-history rebuild when the cache cannot be safely
extended. Reads prefer the disk-backed shard index when
`account_tx_index_meta.json` is current. Account-history readiness is now
aggregate-or-disk: the aggregate `account_tx_index.json` can be absent without
an operator warning when `account_tx_index_meta.json` and per-account disk
shards are usable. Validator doctor and monitor snapshot both use that
effective readiness rule. Before using `account_tx` canaries, check freshness:

```bash
target/debug/postfiat-node account-tx-index-status \
  --data-dir /var/lib/postfiat/validator-0
```

If the status is absent or stale, rebuild manually:

```bash
target/debug/postfiat-node account-tx-index-build \
  --data-dir /var/lib/postfiat/validator-0
```

For a controlled live fleet refresh with SSH credentials available, run:

```bash
RUN_ID=live-account-tx-index-$(date -u +%Y%m%dT%H%M%SZ) \
  SSH_CRED_FILE=/path/to/machine-credentials.txt \
  scripts/testnet-live-account-tx-index-refresh
```

The live refresh builds and verifies `account_tx_index.json` on each validator,
confirms the index tip matches the chain tip, proves chain status is unchanged
by the cache operation, and writes a redaction-safe report under
`reports/testnet-live-account-tx-index-refresh/`.

The public read RPC method `account_tx_index_status` reports whether the index
is current without exposing the validator filesystem path. See
`docs/runbooks/account-tx-index.md`.
Validator doctor also reports per-validator `account_tx_index` readiness and
aggregate ready/present/usable counts.

## RPC Surface Inventory

Before exposing a method externally, check:

```bash
scripts/testnet-rpc-method-inventory \
  --markdown docs/runbooks/rpc-method-inventory.md
```

The inventory marks methods as read-only public, controlled-write gated,
privacy-alpha gated, or operator/local-only.

## Evidence Refresh

For a local current-head operator-tooling evidence packet:

```bash
RUN_ID=current-head-tooling-$(date -u +%Y%m%dT%H%M%SZ)
scripts/testnet-overnight-evidence-refresh \
  --run-id "$RUN_ID" \
  --output "reports/testnet-overnight-evidence-refresh/$RUN_ID/testnet-overnight-evidence-refresh.json" \
  --skip-privacy \
  --skip-finality
```

Only skip privacy/finality when the code delta is limited to operator tooling
or docs and a prior full privacy/finality packet remains current.

## Controlled Write Edge

Do not expose a persistent public write edge without launch approval and the
controlled-write policy. The only remote write method intended for transparent
wallet flow is `mempool_submit_signed_transfer`, and it must be enabled
explicitly with RPC service rate limits.

Orchard batch creation is privacy-alpha gated. It must stay disabled unless
the operator intentionally starts `rpc-serve --allow-orchard-batch-create`
with bounded request, per-peer, total, concurrency, and child-timeout limits.

Direct `apply_*` methods are operator/state-apply paths. They are not public
RPC.

## Disk And History

History retention is checked by validator doctor. For deeper retention work:

```bash
target/debug/postfiat-node history-status --data-dir /var/lib/postfiat/validator-0
```

Partial-history validators should keep a recent replay window and rely on
archive-window handoff for older ranges. Archive operators must preserve and
verify archive bundles before pruning.

## Independent Operator Onboarding

Before an independent operator is counted toward controlled-testnet diversity:

- Confirm the operator has its own machine, legal entity, funding path, SSH
  principal, and infrastructure provider. Do not count multiple validator slots
  on one host as independent diversity.
- Stage validator material through the approved operator packet or key-rotation
  process. Do not transmit raw `validator_keys.json`, `faucet_key.json`, wallet
  backups, mnemonics, seeds, or SSH private material in chat, tickets, email, or
  public evidence packets.
- Start the validator and read-only RPC services with public write methods
  disabled by default.
- Run `scripts/testnet-live-validator-doctor` or the local validator doctor and
  keep only the redaction-safe report path as evidence.
- Run `scripts/testnet-rpc-doctor` against the read-only RPC endpoint before
  publishing or sharing the endpoint.
- Run `scripts/testnet-monitor-snapshot` once and then from cron or equivalent
  monitoring so height lag, read posture, account-history index freshness, and
  Orchard public counters are visible.
- Escalate before launch admission if validator doctor reports state/root
  divergence, unsafe key-file permissions, missing public state files, stale
  history/index posture, or inactive services.

## Restart And Logs

```bash
sudo systemctl start postfiat-validator-0.service
sudo systemctl start postfiat-validator-0-rpc.service
sudo systemctl restart postfiat-validator-0.service
sudo systemctl restart postfiat-validator-0-rpc.service
sudo systemctl stop postfiat-validator-0-rpc.service
sudo systemctl is-active postfiat-validator-0.service postfiat-validator-0-rpc.service
```

After restart, run validator doctor and RPC doctor. If the validator doctor
reports height lag or root divergence, stop and escalate with the doctor JSON
report path plus redacted log tail. Do not prune or copy state manually while
divergence is unresolved.

## Key Rotation

Use the dedicated key-rotation runbook:

- `docs/runbooks/validator-emergency-key-rotation.md`

Do not replace validator keys by editing JSON files directly.
