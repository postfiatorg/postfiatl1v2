# Independent Cobalt Operator Onboarding

This packet migrates the six controlled-testnet validator slots to six genuinely separate operators. The chain uses six validators and quorum five, so each validator must have a different operator: any operator controlling two validators could halt quorum by withdrawing both.

The code verifies the evidence files themselves. Each operator signs three structured control attestations with the same ML-DSA master key that signs its manifest. The terminal topology check verifies every signature and binds the provider account, host administration, key custody, onboarding challenge, operator identity, source revision, release binary, Cobalt trust graph, and hot validator key.

Private keys never leave the operator's machine. Submit only the public keygen report, signed attestations, and signed manifest.

## Release binding

- Source commit: `3b01c2ad57fb0ce1c29e12edc88aece5b22548ae`
- Release binary SHA-256: `e036033d437d85c4f60fc8e6689a771fdda01dd2ce88456571e6c9092faf4caf`
- Build command: `cargo build --release -p postfiat-node --bin postfiat-node --locked`

Verify the provided binary before using it:

```sh
sha256sum postfiat-node
```

## Operator procedure

Choose the assigned `validator_id`, `onboarding_challenge_id`, and `trust_view_id` from `onboarding-contract.json`.

Generate independent ML-DSA master and validator keys locally:

```sh
umask 077
mkdir -p private public
./postfiat-node operator-onboarding-keygen \
  --validator-id VALIDATOR_ID \
  --master-key-file ./private/manifest-master-key.json \
  --validator-key-file ./private/validator-keys.json \
  > ./public/keygen-report.json
```

The two files under `private/` remain with the operator. The public report contains the hot public key needed for the governed registry rotation.

Create stable SHA-256 fingerprints for the provider account, selected host administrator key, host, and custody boundary. Fingerprints identify control boundaries without disclosing secrets. They must not be random labels.

Create the provider attestation. `PROVIDER_NAME` and `REGION` must exactly match the manifest's `--provider-group` and `--region-group` values:

```sh
./postfiat-node operator-attestation-create \
  --master-key-file ./private/manifest-master-key.json \
  --validator-id VALIDATOR_ID \
  --onboarding-challenge-id ASSIGNED_CHALLENGE_ID \
  --operator OPERATOR_NAME \
  --observed-at UTC_TIMESTAMP \
  --kind provider \
  --provider-name PROVIDER_NAME \
  --provider-account-fingerprint PROVIDER_ACCOUNT_FINGERPRINT \
  --instance-id INSTANCE_ID \
  --region REGION \
  --output ./public/VALIDATOR_ID.provider-attestation.json
```

Create the host-control attestation:

```sh
./postfiat-node operator-attestation-create \
  --master-key-file ./private/manifest-master-key.json \
  --validator-id VALIDATOR_ID \
  --onboarding-challenge-id ASSIGNED_CHALLENGE_ID \
  --operator OPERATOR_NAME \
  --observed-at UTC_TIMESTAMP \
  --kind host \
  --host-fingerprint HOST_FINGERPRINT \
  --host-admin-fingerprint HOST_ADMIN_FINGERPRINT \
  --output ./public/VALIDATOR_ID.host-control-attestation.json
```

Create the custody attestation:

```sh
./postfiat-node operator-attestation-create \
  --master-key-file ./private/manifest-master-key.json \
  --validator-id VALIDATOR_ID \
  --onboarding-challenge-id ASSIGNED_CHALLENGE_ID \
  --operator OPERATOR_NAME \
  --observed-at UTC_TIMESTAMP \
  --kind custody \
  --key-custody-fingerprint KEY_CUSTODY_FINGERPRINT \
  --storage-boundary STORAGE_BOUNDARY \
  --backup-boundary BACKUP_BOUNDARY \
  --output ./public/VALIDATOR_ID.custody-attestation.json
```

`UTC_TIMESTAMP` uses the form `2026-08-24T06:57:20Z`. Do not include API tokens, private keys, SSH private keys, recovery codes, invoices, addresses, or billing details. The CLI rejects common private-material markers.

Verify all three attestations locally and copy each returned `attestation_hash` into the manifest command:

```sh
for kind_file in \
  ./public/VALIDATOR_ID.provider-attestation.json \
  ./public/VALIDATOR_ID.host-control-attestation.json \
  ./public/VALIDATOR_ID.custody-attestation.json
do
  ./postfiat-node operator-attestation-verify --attestation-file "$kind_file"
done
```

Create the custody-bound signed manifest:

```sh
./postfiat-node operator-manifest-create \
  --master-key-file ./private/manifest-master-key.json \
  --chain-id postfiat-wan-devnet-2 \
  --network controlled-testnet \
  --validator-id VALIDATOR_ID \
  --hot-public-key-hex HOT_PUBLIC_KEY_FROM_KEYGEN_REPORT \
  --operator OPERATOR_NAME \
  --contact OPERATOR_CONTACT \
  --provider-group PROVIDER_NAME \
  --region-group REGION \
  --jurisdiction-group JURISDICTION \
  --legal-domain-group LEGAL_CONTROL_DOMAIN \
  --funding-domain-group FUNDING_CONTROL_DOMAIN \
  --trust-graph-root c872bf8a9628cb3b27f2c0826084beb540c645d0c9d06107643358a4df078fa919e88ba2aa6b376a904eb79d28d69e77 \
  --trust-graph-version 1 \
  --trust-view-id ASSIGNED_TRUST_VIEW_ID \
  --trust-view-version 1 \
  --section2-packet-root 40bc86c9416a1b468f5625a2ff83724c9268f9d49c41007e9b0c4bc70c43c1e1 \
  --source-commit 3b01c2ad57fb0ce1c29e12edc88aece5b22548ae \
  --release-binary-sha256 e036033d437d85c4f60fc8e6689a771fdda01dd2ce88456571e6c9092faf4caf \
  --onboarding-challenge-id ASSIGNED_CHALLENGE_ID \
  --provider-account-fingerprint PROVIDER_ACCOUNT_FINGERPRINT \
  --host-admin-fingerprint HOST_ADMIN_FINGERPRINT \
  --key-custody-fingerprint KEY_CUSTODY_FINGERPRINT \
  --provider-attestation-hash PROVIDER_ATTESTATION_HASH \
  --host-control-attestation-hash HOST_ATTESTATION_HASH \
  --custody-attestation-hash CUSTODY_ATTESTATION_HASH \
  --output ./public/VALIDATOR_ID.operator-manifest.json
```

Verify locally:

```sh
./postfiat-node operator-manifest-verify \
  --manifest-file ./public/VALIDATOR_ID.operator-manifest.json
```

Submit exactly these five public files:

- `keygen-report.json`
- `VALIDATOR_ID.provider-attestation.json`
- `VALIDATOR_ID.host-control-attestation.json`
- `VALIDATOR_ID.custody-attestation.json`
- `VALIDATOR_ID.operator-manifest.json`

## Coordinator gate

Place manifests and attestations in their respective directories using the exact filenames above. Rehearse the governed registry/key migration on a disposable state clone, then run:

```sh
postfiat-node operator-independence-verify \
  --data-dir POST_MIGRATION_CLONE_DATA_DIR \
  --manifest-dir RECEIVED_MANIFEST_DIR \
  --attestation-dir RECEIVED_ATTESTATION_DIR \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --quorum 5 \
  --network controlled-testnet \
  --section2-packet-root 40bc86c9416a1b468f5625a2ff83724c9268f9d49c41007e9b0c4bc70c43c1e1 \
  --source-commit 3b01c2ad57fb0ce1c29e12edc88aece5b22548ae \
  --release-binary-sha256 e036033d437d85c4f60fc8e6689a771fdda01dd2ce88456571e6c9092faf4caf \
  --min-operator-groups 6 \
  --min-infrastructure-domains 3
```

A declaration packet is not completion. Completion requires all six independently controlled validators running the pinned release, the governed registry matching their hot keys, and the live fault and transition exercises in the active Cobalt milestone passing.
