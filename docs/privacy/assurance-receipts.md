# Assurance Receipts

PostFiat keeps Orchard/Halo2 as the shielded settlement core and adds an
assurance layer around it. The point is not to make private settlement
transparent again. The point is to let a holder prove bounded policy facts to a
counterparty, custodian, auditor, or venue without giving that party permanent
view-key access to the wallet.

This is the Railgun-style lesson PostFiat adopts: compliance should be a
selective proof surface, not ambient public leakage. Private Proofs of Innocence
shows the useful pattern: bind a shielded asset to a policy/list-provider root
and prove non-inclusion or provenance facts without revealing balances,
addresses, or history. PostFiat applies that idea to its own Orchard note pool
instead of replacing the pool.

## Design Boundary

```text
Orchard note spend
  -> public {root, nullifier, outputCommitments, fee, policyHash, disclosureHash}
  -> optional AssuranceReceipt
  -> scoped auditor/custodian/counterparty verification
```

An assurance receipt is not a full viewing key. It must not disclose future
history, full wallet history, raw spending material, note randomness, witness
paths, or private keys. A receipt is valid only for its explicit subject,
policy root, list-provider root, recipient class, ledger window, and expiry.

## Packet

The current design fixture is
`postfiat-privacy-assurance-receipt-v1`.

It binds:

- `chain`: chain id, Orchard pool id, and active registry root;
- `subject`: blinded note/withdrawal/auditor tag and public chain fields;
- `policy`: Cobalt-governed policy root, list-provider root, standby window,
  and freshness window;
- `disclosure`: recipient class and scoped allowed reveals;
- `validity`: ledger range and expiry;
- `evidence_artifacts`: existing Orchard evidence files and SHA3-384 hashes;
- `attestation`: attestor key hash, statement hash, signature hash, and
  signature scheme.

The valid fixture is:

```text
docs/governance/agent/fixtures/privacy_assurance_receipt/valid_assurance_receipt.json
```

The verifier intentionally rejects packets that try to turn assurance into broad
surveillance. The checked invalid fixtures are:

```text
docs/governance/agent/fixtures/privacy_assurance_receipt/invalid_full_viewing_key_receipt.json
docs/governance/agent/fixtures/privacy_assurance_receipt/invalid_ungoverned_policy_receipt.json
```

## Verification

```bash
scripts/privacy-assurance-receipt-verify --fixtures
scripts/privacy-assurance-receipt-verify --write-report
scripts/privacy-assurance-receipt-verify --verify-report
```

The verifier checks:

- canonical statement hash binding;
- Orchard pool id binding;
- policy root and list-provider root shape;
- Cobalt-governed policy-root requirement;
- scoped disclosure only;
- no full viewing key, future history, full wallet history, or raw private
  values;
- receipt expiry and ledger-range sanity;
- evidence artifact existence and SHA3-384 hash matches;
- invalid fixture rejection.

## Current Evidence

The first assurance receipt fixture is bound to the existing Orchard evidence:

| Evidence | Claim |
| --- | --- |
| `reports/orchard-verification-budget-v1-report.json` | Cached Orchard verifier budget and proof size class. |
| `reports/testnet-orchard-privacy-audit-packet/.../orchard-privacy-audit-packet.json` | Redaction-safe privacy audit packet. |
| `reports/testnet-live-orchard-full-flow/.../testnet-live-orchard-full-flow.json` | Live deposit, spend, withdraw, and recovery path evidence. |
| `reports/testnet-live-orchard-direct-deposit/.../testnet-live-orchard-direct-deposit.json` | Direct transparent-to-Orchard deposit evidence. |

The receipt is currently a design and controlled-testnet artifact. It does not
grant live authority, mutate the registry, or claim production compliance.

## Roadmap

| Phase | Goal | Gate |
| --- | --- | --- |
| 0. Design fixture | Define packet, attestation hash, invalid cases, and verifier. | `privacy-assurance-receipt-verify --fixtures` passes. |
| 1. Envelope binding | Add `policyHash` and `disclosureHash` to the live Orchard envelope and verify they are signed by the outer authorization digest. | Tamper tests reject mismatched policy/disclosure roots. |
| 2. Wallet receipt | Let the wallet produce a scoped receipt for one note/action/window without exporting a full viewing key. | Disclosure verifier accepts scoped packet and rejects future-history access. |
| 3. Policy-root registry | Publish Cobalt-governed policy and list-provider roots with freshness windows and rollback. | Ungoverned or stale roots fail closed. |
| 4. Non-inclusion proof | Replace the design placeholder with a real ZK non-inclusion/provenance proof over the accepted policy root. | Proof verifier accepts clean fixture and rejects listed-source fixture. |
| 5. Institution path | Let custodians, exchanges, and buy-side workflows require receipt classes for deposits, withdrawals, or auditor packets. | Missing/stale receipts route to restricted policy class, not silent acceptance. |

The production target is not "privacy with a compliance disclaimer." It is
private settlement with policy-bound, expiring, scoped assurance.
