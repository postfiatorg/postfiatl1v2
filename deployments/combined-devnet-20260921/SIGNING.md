# Key-holder handoff

Nothing in this file was executed during preparation. The only operation
requiring the other lane is publisher signing: the deployment manifest now,
and the mandatory backup on deployment day. Both private keys stay on that
lane's trusted workstation. No key-generation command is part of this release.

## Regenerate the exact stage on the signing workstation

Use a checkout containing this packet and the qualified executable with
SHA-256 `051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d`.
The `/secure/` paths below are operator-supplied existing files, not repository
files or instructions to generate keys.

```bash
set -euo pipefail
export PYTHONDONTWRITEBYTECODE=1
PACKET="$PWD/deployments/combined-devnet-20260921"
CACHE="$HOME/.cache/deploy-prep-20260921"
STAGE="$CACHE/validator-stage"
python3 "$PACKET/prepare-stage.py" \
  --binary /secure/combined-devnet-20260921/postfiat-node \
  --trusted-publisher-public-file /secure/deployment.public.json \
  --output-dir "$STAGE"
```

The stage directory must not already exist. On the preparation server it already
exists; use it directly, or choose a new cache directory for a regeneration.
Only the stage report/signing binding paths change with location; signed unit,
environment, binary, topology, and metadata bytes stay identical.

## Exact signing command

Set a current activation time and the reviewed expiry. This example mirrors the
existing release's one-year approval window. The publisher must review that
window; every future service start must remain inside it.

```bash
VALID_FROM_UNIX="$(date -u +%s)"
VALID_UNTIL_UNIX="$((VALID_FROM_UNIX + 31536000))"
python3 "$PACKET/sign-manifest.py" \
  --stage "$STAGE" \
  --publisher-key-file /secure/deployment.private.json \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX"
```

This runs the offline checks, calls the node's real signer once, and verifies
all six bindings against the existing public trust file. It does not create a
key, contact a validator, or launch fleet preflight. A wrong key, wrong window,
or changed input is a stop condition. The expected output is:

```text
$STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260921/deployment-manifest.json
```

For review, the exact expanded node invocation is:

```bash
CONFIG="$STAGE/rootfs/etc/postfiat/releases/combined-devnet-20260921"
NODE="$STAGE/rootfs/opt/postfiat/releases/combined-devnet-20260921/postfiat-node"
"$NODE" deployment-manifest-create \
  --deployment-id combined-devnet-20260921 \
  --valid-from-unix "$VALID_FROM_UNIX" \
  --valid-until-unix "$VALID_UNTIL_UNIX" \
  --chain-id postfiat-wan-devnet-2 \
  --genesis-hash ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 \
  --git-revision 03e422a7 \
  --binary-file "$NODE" \
  --build-profile release \
  --build-features privacy,rpc,transport \
  --protocol-version 1 \
  --rpc-schema postfiat-local-rpc-v1 \
  --service-unit-file "$STAGE/operator/postfiat-release-operator.service" \
  --environment-file "$STAGE/operator/operator.env" \
  --validator-bindings-file "$STAGE/validator-bindings.signing.json" \
  --topology-file "$CONFIG/topology.json" \
  --swap-circuit-metadata-file "$CONFIG/swap.metadata.json" \
  --private-egress-circuit-metadata-file "$CONFIG/private-egress.metadata.json" \
  --publisher-key-file /secure/deployment.private.json \
  --manifest-file "$CONFIG/deployment-manifest.json"
python3 "$PACKET/local-preflight.py" --stage "$STAGE" --require-signed
```

Run either the helper or the expanded invocation, not both. The node creates and
signs the manifest in one operation; there is no unsigned-manifest CLI mode.
`manifest-input.unsigned.json` is the reviewable input inventory, not a
fabricated manifest or a signature placeholder to put on a validator.

## Exact backup signing command — deployment day only

After the fresh successful preflight in [DEPLOY-SHEET.md](DEPLOY-SHEET.md), on
the workstation holding the existing snapshot publisher key:

```bash
scripts/postfiat-safe-rollout backup \
  --state-file "$EVIDENCE/rollout-state.json" \
  --evidence-dir "$EVIDENCE/pre-rollout-backup" \
  --snapshot-publisher-key-file /secure/snapshot-publisher.private.json \
  --snapshot-publisher-public-key-file /secure/snapshot-publisher.public.json
```

This is a mutating deployment-day operation, including the tool's canary export
and local signed re-import. It is not authorized or run in this preparation.
The tool invokes the following exact signing primitive internally:

```bash
"$NODE" snapshot-export-signed-finalized-checkpoint \
  --data-dir "$EVIDENCE/pre-rollout-backup/backup-migrated-source" \
  --snapshot-dir "$EVIDENCE/pre-rollout-backup/backup-signed" \
  --publisher-key-file /secure/snapshot-publisher.private.json
```

Do not invoke that primitive a second time after a successful `backup`.
Expected signed file:
`$EVIDENCE/pre-rollout-backup/backup-signed/snapshot.signed-manifest.json`.
The normal `backup` command verifies it and records `backup.verified=true`.
If export/signing completed but local verification was interrupted, the supported
recovery command is:

```bash
scripts/postfiat-safe-rollout resume-backup-verification \
  --state-file "$EVIDENCE/rollout-state.json" \
  --evidence-dir "$EVIDENCE/pre-rollout-backup" \
  --snapshot-publisher-public-key-file /secure/snapshot-publisher.public.json
```

Run the entire preflight/backup/apply session on this workstation so frozen
absolute stage/inventory/state paths remain valid. Do not transplant or edit
rollout state to move it between machines.
