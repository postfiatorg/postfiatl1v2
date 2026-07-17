# Signed controlled-testnet deployment manifest

The deployment manifest prevents a scored private-swap run from combining a
wallet binary, validator fleet, topology, service unit, environment, RPC
schema, or circuit release that were not reviewed together.  Version 2 keeps
one fleet-wide signed manifest hash while binding the different RPC and
transport unit/environment pair used by every validator.

## Threat model

The gate catches stale worktree binaries, accidental local defaults, partial
fleet rollouts, edited environment/unit files, wrong topology, stale circuit
metadata, expired deployment approvals, a swapped validator binding, and a
validator launched against a different manifest. The signing key is a
release/deployment key and must never be a validator signing key.

The gate does not make a compromised root host trustworthy. Host integrity,
isolated signers, package provenance, and operator access controls remain
separate controls.

## Create and export the dedicated publisher key

Create the key once in a pre-existing, access-controlled directory on the
release host. `deployment-publisher-key-create` refuses to overwrite an
existing file and writes a private key with the dedicated
`postfiat.deployment_publisher_private_key.v1` schema and
`deployment-manifest-publisher` purpose. Do not substitute a faucet,
development, wallet, or validator signing key.

```text
install -d -m 0700 /secure
postfiat-node deployment-publisher-key-create \
  --publisher-key-file /secure/deployment.private.json
```

Export the corresponding public trust file. The private key stays on the
release host; only the public file is installed with the validators.

```text
postfiat-node deployment-publisher-key-export \
  --publisher-key-file /secure/deployment.private.json \
  --public-key-file /etc/postfiat/deployment.public.json
```

## Create a manifest

Stage the validator half of the release from the exact release binary,
six-validator topology, and circuit metadata that will be signed. The command
creates a new rootfs-style directory and refuses to overwrite an existing
stage:

```text
postfiat-node deployment-validator-units-stage \
  --release-id controlled-testnet-YYYYMMDD \
  --topology-file /release/topology.json \
  --binary-file /release/postfiat-node \
  --swap-circuit-metadata-file /release/swap.metadata.json \
  --private-egress-circuit-metadata-file /release/private-egress.metadata.json \
  --output-dir /release/validator-stage
```

The staged tree contains one consolidated RPC unit and one consolidated
transport unit per validator, typed environment files, per-validator runtime
bindings, and the complete signing binding file. It deliberately emits no
systemd drop-ins. The RPC unit always carries the durable spool and readiness
arguments; the transport unit always prewarms both shielded verifiers before
publishing readiness. Both units verify the signed manifest and the actual
runtime binary, topology, and circuit metadata in `ExecStartPre`.

Treat the generated units as the canonical release artifacts. Do not layer
historical host-specific drop-ins over them: a needed setting belongs in this
generator and its regression test, followed by a newly signed release.

First create a canonical binding file on the release host. It names the exact
unit/environment files for each validator; the signed manifest contains only
their hashes, not the file paths. The entries and services must be strictly
sorted. Every validator has exactly `rpc` and `transport` entries.

```json
{
  "schema": "postfiat.deployment_validator_bindings.v1",
  "validators": [
    {
      "validator_id": "validator-0",
      "services": [
        {
          "service_id": "rpc",
          "service_unit_file": "/release/validator-0/postfiat-rpc.service",
          "environment_file": "/release/validator-0/rpc.env"
        },
        {
          "service_id": "transport",
          "service_unit_file": "/release/validator-0/postfiat-transport.service",
          "environment_file": "/release/validator-0/transport.env"
        }
      ]
    }
  ]
}
```

The production file includes all topology validators, in strict validator-ID
order. The
`--service-unit-file` and `--environment-file` below remain the local scored
wallet/operator component that invokes preflight; they are not a substitute for
the validator bindings.

```text
postfiat-node deployment-manifest-create \
  --deployment-id controlled-testnet-YYYYMMDD \
  --valid-from-unix START \
  --valid-until-unix END \
  --chain-id CHAIN \
  --genesis-hash HASH \
  --git-revision REVISION \
  --binary-file /usr/local/bin/postfiat-node \
  --build-profile release \
  --build-features privacy,transport,rpc \
  --protocol-version 1 \
  --rpc-schema postfiat-local-rpc-v1 \
  --service-unit-file /etc/systemd/system/postfiat-validator.service \
  --environment-file /etc/postfiat/validator.env \
  --validator-bindings-file /release/validator-bindings.json \
  --topology-file /etc/postfiat/topology.json \
  --swap-circuit-metadata-file /etc/postfiat/swap.metadata.json \
  --private-egress-circuit-metadata-file /etc/postfiat/private-egress.metadata.json \
  --publisher-key-file /secure/deployment.private.json \
  --manifest-file /etc/postfiat/deployment-manifest.json
```

The command hashes every named file, verifies that the binding IDs exactly
match the topology, canonicalizes build features, binds the activation/expiry
window, and signs the complete record with ML-DSA-65.

## Launch contract

Every validator RPC and transport unit sets:

```text
POSTFIAT_DEPLOYMENT_MANIFEST=/etc/postfiat/deployment-manifest.json
POSTFIAT_DEPLOYMENT_VALIDATOR_ID=validator-N
POSTFIAT_DEPLOYMENT_VALIDATOR_BINDINGS_FILE=/etc/postfiat/validator-N-bindings.json
POSTFIAT_DEPLOYMENT_BINARY=/usr/local/bin/postfiat-node
POSTFIAT_DEPLOYMENT_TOPOLOGY=/etc/postfiat/topology.json
POSTFIAT_DEPLOYMENT_SWAP_CIRCUIT_METADATA=/etc/postfiat/swap.metadata.json
POSTFIAT_DEPLOYMENT_PRIVATE_EGRESS_CIRCUIT_METADATA=/etc/postfiat/private-egress.metadata.json
```

Before either service starts, its unit runs:

```text
postfiat-node deployment-manifest-verify \
  --manifest-file /etc/postfiat/deployment-manifest.json \
  --trusted-publisher-key-file /etc/postfiat/deployment.public.json \
  --validator-id validator-N \
  --validator-bindings-file /etc/postfiat/validator-N-bindings.json \
  --runtime-binary-file /usr/local/bin/postfiat-node \
  --runtime-topology-file /etc/postfiat/topology.json \
  --runtime-swap-circuit-metadata-file /etc/postfiat/swap.metadata.json \
  --runtime-private-egress-circuit-metadata-file /etc/postfiat/private-egress.metadata.json
```

`status` reports the SHA-256 of the exact global manifest bytes, its validator
ID, the live RPC/transport unit/environment hashes, and the hashes observed for
the runtime binary, topology, and both circuit-metadata files. If a configured
file cannot be read, either artifact group is partial, or the configured
binding names another node, status fails instead of silently omitting
deployment identity.

StakeHub's scored profile sets `require_signed_deployment_manifest=true` and
names the trusted publisher key plus the local unit, environment, topology,
and circuit metadata files. Preflight performs all of these checks before any
execution:

1. the L1 CLI verifies signature, schema, canonical features, activation, and
   expiry;
2. StakeHub re-hashes every local component and compares it to the signed
   record;
3. the local binary/profile Git revision and RPC schema match the record; and
4. all six validator status responses report the exact signed-manifest hash,
   protocol, build revision/profile, RPC schema, correct validator ID, and
   the exact signed RPC/transport artifact pair for that validator.

A missing, unsigned, expired, tampered, or mixed deployment is a hard preflight
failure. Do not replace it with a warning or quorum rule.

## Rollout and rollback

Use `scripts/postfiat-safe-rollout` as documented in
`docs/runbooks/safe-validator-rollout.md`. Manual fleet copies and rsync-based
deployment are unsupported. The tool enforces a read-only six-host preflight,
Vultr inventory reconciliation, a verified signed backup, canary-first order,
allowlisted atomic file promotion, manifest verification, and six-node
convergence before its durable state can advance.

On mismatch, the tool exits without advancing the rollout state. Stop the
affected validator if its service is unhealthy, preserve the incoming files
and logs, and diagnose from the last verified signed backup. Do not advance
the remaining fleet.
