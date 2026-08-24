# Independent Cobalt Operator Onboarding

This packet migrates the six controlled-testnet validator slots to six genuinely separate operators. With six validators and quorum five, one operator per validator is mandatory: an operator controlling two validators could halt quorum by withdrawing both.

Private keys never leave the operator's machine. Post Fiat receives the public onboarding report, signed operator manifest, and redaction-safe provider/host/custody receipts only.

## Operator procedure

Use the release declared in `onboarding-contract.json`. Verify its SHA-256 before doing anything else:

```sh
sha256sum postfiat-node
```

Choose the assigned `validator_id`, `onboarding_challenge_id`, and `trust_view_id` from the contract. Generate independent ML-DSA master and validator keys locally:

```sh
umask 077
./postfiat-node operator-onboarding-keygen \
  --validator-id VALIDATOR_ID \
  --master-key-file ./private/manifest-master-key.json \
  --validator-key-file ./private/validator-keys.json \
  > ./public/keygen-report.json
```

The two files under `private/` must remain with the operator. The report is public-only and contains the hot public key needed for the governed registry rotation.

Create three short redaction-safe receipts:

- provider receipt: provider name, stable account fingerprint, instance ID, region, timestamp, and a statement that this operator controls the account;
- host-control receipt: host fingerprint, selected administrator public-key fingerprint, timestamp, and a statement that this operator controls root administration;
- custody receipt: master public-key fingerprint, storage boundary, backup boundary, and a statement that no other validator operator holds the key.

Do not include API tokens, private keys, SSH private keys, recovery codes, invoices, addresses, or billing details. Hash the receipts with SHA-256. The provider-account, host-admin, and key-custody fingerprints must describe stable control identities, not random labels.

Create the signed manifest using the exact contract values:

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
  --source-commit 2fb5fa08e9769ef928cd1149bea2c589d0228c22 \
  --release-binary-sha256 a101e0bb407f891587517c91f3c2e15e97b4708509e75e1d0dc3184b0440da20 \
  --onboarding-challenge-id ASSIGNED_CHALLENGE_ID \
  --provider-account-fingerprint PROVIDER_ACCOUNT_FINGERPRINT \
  --host-admin-fingerprint HOST_ADMIN_FINGERPRINT \
  --key-custody-fingerprint KEY_CUSTODY_FINGERPRINT \
  --provider-attestation-hash PROVIDER_RECEIPT_SHA256 \
  --host-control-attestation-hash HOST_CONTROL_RECEIPT_SHA256 \
  --output ./public/VALIDATOR_ID.operator-manifest.json
```

Verify locally:

```sh
./postfiat-node operator-manifest-verify \
  --manifest-file ./public/VALIDATOR_ID.operator-manifest.json
```

Submit only the keygen report, signed manifest, and redaction-safe receipts.

## Coordinator gate

First verify each manifest signature. Then construct and rehearse the governed registry/key migration on a disposable clone. The terminal topology check runs against the post-migration registry:

```sh
postfiat-node operator-independence-verify \
  --data-dir POST_MIGRATION_CLONE_DATA_DIR \
  --manifest-dir RECEIVED_MANIFEST_DIR \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --quorum 5 \
  --network controlled-testnet \
  --section2-packet-root 40bc86c9416a1b468f5625a2ff83724c9268f9d49c41007e9b0c4bc70c43c1e1 \
  --source-commit 2fb5fa08e9769ef928cd1149bea2c589d0228c22 \
  --release-binary-sha256 a101e0bb407f891587517c91f3c2e15e97b4708509e75e1d0dc3184b0440da20 \
  --min-operator-groups 6 \
  --min-infrastructure-domains 3
```

A passing declaration packet is not enough. Completion requires all six independently controlled validators to be running, the post-migration registry to match their hot keys, and the live fault/transition exercises in the active milestone to pass.
