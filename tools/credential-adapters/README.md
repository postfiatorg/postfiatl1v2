# Pillar 8 Standalone Zero-PII Issuer Adapters (`tools/credential-adapters/`)

Reference off-chain credential adapters for Pillar 8 (*On-chain Identity: DID, KYC & Optional Trust Lines*) matching **Contract C4** (`docs/specs/l1-upgrade-train-b-contract.md` §4.2–§4.7).

## Components

1. **W3C Verifiable Credential (VC) Issuer Adapter (`w3c_vc_adapter.py`):**
   - Verifies W3C VC Data Model v1.1 / v2.0 JSON payloads signed with Ed25519 (`Ed25519Signature2020` or `DataIntegrityProof` with `eddsa-jcs-2022`) against registered vendor DIDs.
   - Supports optional HMAC-SHA256 webhook authentication (`X-PostFiat-Webhook-Signature`).
   - Enforces `did:pftl:<chain_id>:<account>` subject binding, validity windows, and Pillar 8 `<domain>/<type>` credential-type namespacing.
   - Emits Contract C4 `credential_create_and_accept` payloads (enabling 0-PFT delegated user onboarding) and `credential_revoke` payloads on vendor revocation events.
2. **ZK-Passport Sandbox Verifier Adapter (`zk_passport_adapter.py`):**
   - Off-chain Self / OpenPassport compatible sandbox verifier (zero SNARKs in L1 consensus).
   - Verifies proof digest, sandbox verifier attestation (or pluggable external Groth16 verifier callback), trusted ICAO/OFAC roots, minimum age, sanctions checks, scope, expiry, and anti-Sybil nullifier uniqueness.
   - Emits Contract C4 `credential_create_and_accept` and `credential_revoke` payloads.
3. **Zero-PII Firewall & Audit Store (`zero_pii_store.py`):**
   - Raw PII is read strictly in memory to verify the external proof and derive a salted 96-hex SHA3-384 `commitment_hash` (ephemeral CSPRNG salt is never stored).
   - Recursive PII key denylist (`assert_zero_pii_structure`) blocks any accidental persistence or emission of raw personal data (`name`, `dateOfBirth`, `passportNumber`, `email`, `address`, `mrz`, `credentialSubject`, etc.).
4. **HTTP Webhook & Sandbox Server (`server.py`) & CLI (`cli.py`):**
   - Bounded 64 KiB JSON endpoints: `GET /health`, `POST /v1/w3c-vc/issue`, `POST /v1/w3c-vc/revoke`, `POST /v1/zk-passport/verify-and-issue`, `POST /v1/zk-passport/revoke`, `GET /v1/credentials/<credential_id>`.

## Tests

```bash
PYTHONPATH=tools/credential-adapters pytest tools/credential-adapters/tests/
```
