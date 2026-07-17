# Validator Doctor Runbook

Status: current operator tooling  
Date: 2026-05-16

`scripts/testnet-validator-doctor` emits a redaction-safe JSON report for one
validator data dir or an entire local validator set. It is intended for
controlled-testnet launch checks, cron collection, and fast operator triage.

## Local All-Validator Check

```bash
scripts/testnet-validator-doctor \
  --data-root reports/testnet-validator-doctor-smoke/nodes \
  --validator-count 4
```

The report is written under `reports/testnet-validator-doctor/` by default.
Use `--output` and `--log-dir` to pin paths for release evidence.

## Single Validator Check

```bash
scripts/testnet-validator-doctor \
  --data-dir /var/lib/postfiat/validator-0 \
  --validator-service postfiat-validator-0.service \
  --rpc-service postfiat-rpc-0.service
```

One-validator mode verifies the local validator and reports the expected BFT
quorum from the active registry, but it does not require that quorum to be
observable from one machine.

## Live Service Templates

If unit names follow a pattern, use templates instead of one flag per node:

```bash
scripts/testnet-validator-doctor \
  --data-root /var/lib/postfiat \
  --validator-count 5 \
  --validator-service-template 'postfiat-{node_id}.service' \
  --rpc-service-template 'postfiat-rpc-{index}.service'
```

The doctor checks:

- `postfiat-node` binary checksum and executable status.
- Data-dir and required public state-file presence.
- Private key file permissions without emitting key material.
- `status`, `verify-state`, `metrics`, `history-status`,
  `account-tx-index-status`, `validate-local-keys`, and
  `validator-registry-root`.
- Validator registry membership, registry root, active validator count, and
  expected BFT quorum.
- Height, block tip, latest committed block/certificate metadata when present.
- Partial-history retention posture, retained account-history index freshness,
  disk usage, and optional systemd/journal status.

## Controlled Live Fleet Check

With SSH credentials available on the operator machine:

```bash
RUN_ID=live-validator-doctor-$(date -u +%Y%m%dT%H%M%SZ) \
  SSH_CRED_FILE=/path/to/machine-credentials.txt \
  scripts/testnet-live-validator-doctor
```

The live wrapper does not copy logs or key material back. It runs bounded
read-only diagnostics on each validator, validates split local validator keys
with `validate-local-keys --local-only`, checks systemd active state, verifies
history/index/state posture, and writes a redaction-safe report under
`reports/testnet-live-validator-doctor/`.

The default per-command timeout for the live wrapper is `180` seconds. Override
with `COMMAND_TIMEOUT_SECONDS=N` only for focused diagnosis; post-write
`verify-state` and `history-status` are expected to be heavier than basic RPC
checks.

Account-history index absence on an empty chain is accepted. Once blocks exist,
an absent or stale `account_tx_index.json` or `account_tx_index_meta.json` is
reported with per-validator `account_tx_index` details and aggregate
ready/present/usable counts. The index is a rebuildable cache, not consensus
state.

Reports intentionally identify validators by `node_id` plus public-key
fingerprint. Raw public keys, private keys, spending keys, seeds, and note
randomness are not emitted.

## Smoke

```bash
scripts/testnet-validator-doctor-smoke
```

The smoke builds a four-validator local harness, commits one transparent
transfer on every validator, proves the auto-refreshed account-history index is
ready on all validators, runs the doctor in all-validator mode, checks
convergence/quorum/history readiness, and verifies the report/logs do not
contain key-shaped fields.
