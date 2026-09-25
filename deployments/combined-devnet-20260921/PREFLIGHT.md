# Preflight boundary and preparation results

**PASS for offline release inputs; full fleet preflight NOT RUN.**
The exact output is [local-preflight.json](local-preflight.json).
No rollout state, signed manifest, signed backup, or passing fleet receipt was
created.

## What the supported command checks

The implementation at
[python/postfiat_ops/safe_rollout.py](../../python/postfiat_ops/safe_rollout.py)
executes preflight in this order:

1. Reject unsafe CLI destinations/deletion flags and an existing state file.
2. Parse exactly six ordered, unique inventory hosts and Vultr instance IDs.
3. Reconcile ID/IP/region/active/running against the owning Vultr account.
4. Read all six RPC statuses: identical height/tip/root, empty mempools.
5. Validate each host's isolated signer against one complete, identical active
   six-validator public registry.
6. For each validator, resolve the allowlisted staged files and run
   `deployment-manifest-verify` against the trusted publisher, runtime binding,
   executable, topology, and both circuit metadata files.
7. Read hashes of the allowlisted remote targets and record only
   create/update/unchanged actions.
8. Freeze the stage report and inventory hashes, record the verified preflight,
   and set the canary order to **1, 0, 2, 3, 4, 5**.

There is no `--offline` or `--unsigned` switch. Without a manifest, the full
command could perform steps 1–5 before failing at step 6; it cannot create a
verified rollout state. Those cloud/fleet checks exceed this task's narrow
validator-0 read allowance, so the full command was not invoked. The local leaf
checks below have no network access and do not impersonate full preflight.

## Executed locally

The qualified executable generated the stage successfully with:

```bash
CACHE="$HOME/.cache/deploy-prep-20260921"
NODE="$HOME/.cache/qualify-fix-20260918/binaries/candidate-1"
"$NODE" deployment-validator-units-stage \
  --release-id combined-devnet-20260921 \
  --topology-file "$CACHE/observed/topology.json" \
  --binary-file "$NODE" \
  --swap-circuit-metadata-file "$CACHE/observed/swap.metadata.json" \
  --private-egress-circuit-metadata-file "$CACHE/observed/private-egress.metadata.json" \
  --output-dir "$CACHE/validator-stage"
PYTHONDONTWRITEBYTECODE=1 python3 \
  deployments/combined-devnet-20260921/local-preflight.py \
  --stage "$CACHE/validator-stage"
```

| Local check | Result |
| --- | --- |
| Canonical generator, complete stage schema/order | PASS |
| Staged executable equals the supplied qualified SHA-256 | PASS |
| All 33 generated configuration files match committed inputs | PASS |
| Six RPC/transport pairs and their unit/environment hashes | PASS |
| Six runtime bindings and complete ordered signing bindings | PASS |
| Chain/genesis/protocol and six private-overlay peers | PASS |
| Four embedded circuit artifact hashes and byte lengths | PASS |
| Inventory syntax, unique identities, topology ports | PASS; historical cloud facts only |
| Existing deployment trust-file hash | PASS; no public key bytes committed |
| Production allowlist for ten present targets per validator | PASS |
| Production `verify_local_stage` for validators 0–5 | EXPECTED BLOCK: signed manifest missing |

Representative final output:

```text
result: PASS_LOCAL_INPUTS_ONLY
network_calls: 0
rollout_state_created: false
canonical_generated_files: 33
circuit_artifacts_matched: 4
each validator: inputs=PASS, allowlisted_present_files=10,
               manifest_gate=BLOCKED_MISSING_SIGNED_MANIFEST
```

The missing eleventh file is
`rootfs/etc/postfiat/releases/combined-devnet-20260921/deployment-manifest.json`.
No dummy file was created to bypass this check.

## Blocked and deferred

- **Signature:** trusted publisher authorization, activation/expiry, and all six
  signed runtime checks require the real manifest from the existing key holder.
- **Fresh deploy-day reads:** Vultr reconciliation, six-host signer/registry
  verification, live unit/process identity, health, convergence, and exact remote
  diffs remain unperformed. Only validator-0's allowed release files and four
  units were read; this is not a fresh fleet-status claim.
- **Backup signature:** the mandatory canary export/sign/import and verified
  backup state are deployment-day actions requiring the snapshot publisher key.
  No validator backup was exported during preparation.
- **Recovery prerequisites:** exact old executable plus compatible pre-upgrade
  per-validator data and signer-safety continuity must be available before apply.
  The canary's signed backup alone is not evidence that all six per-host rollback
  anchors exist. Allowed reads cannot establish that; see the conditional,
  fail-closed recovery commands in the deploy sheet.
- **Application:** every copy/restart/migration/health gate remains for deployment
  day. No governance or replicated-state activation is included.

The safe-rollout runbook describes backup `verify-state`; this release's actual
implementation runs `verify-finalized-checkpoint` after signed re-import.
Its apply also performs offline `storage-integrity-migrate-legacy` and a
checkpoint check before restarting the two validator services. This packet
records that narrower implemented backup verification honestly; existing full
history qualification is not a newly run backup replay.

The [September 18 rollback evidence](../release-repair-20260918/README.md)
covers pre-activation recovery only. It does not authorize rollback after a new
activation or establish fresh backups for today's fleet.

## Publication gates

See [publication-gates.json](publication-gates.json) for actual command exits and
output. Required commands are:

```bash
.venv-docs/bin/mkdocs build --strict
scripts/public-doc-links
scripts/public-secret-scan
```

No workspace, Orchard/Halo2, build reproduction, CI requalification, or live
deployment test is needed for this input/documentation packet.
