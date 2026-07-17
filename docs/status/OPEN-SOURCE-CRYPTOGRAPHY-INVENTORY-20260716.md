# PostFiat L1 Cryptography, Proof, and Key-Purpose Inventory

**Audit date:** 2026-07-16
**Code baseline:** `4b5af7bc6bb6e793ed8a60219d13d6d35be03058`
**Status:** STEP 1 evidence; publication blocked pending the closures below

This inventory describes cryptography actually reachable in the repository. It
does not infer that a primitive is safe merely because a reputable crate is
used. Protocol binding, key custody, canonical encoding, parameter provenance,
replay behavior, and downgrade handling are part of the cryptographic boundary.

## 1. Primitive and purpose inventory

| Primitive/system | Implementation | Current purposes | Public-release finding |
|---|---|---|---|
| ML-DSA-65 | `fips204 = 0.4.6`; wrapper in `crates/crypto_provider/src/lib.rs` | Account transactions, block proposal/vote/timeout certificates, bridge witnesses, admission receipts, owned transfer/unwrap, FastLane/FastSwap, snapshots/deployment/operator manifests | Core implementation parses fixed-size keys/signatures fail-closed. The canonical transcript inventory and blocking call-site policy now cover enabled use: any new default-context or deterministic-seed call fails CI pending explicit cryptographic review. Plaintext validator/publisher custody remains feature-contained P1; unsigned governance is rejected live. |
| SHA3-384 | `sha3 = 0.11`; `crypto_provider::hash_bytes` | General domain-separated IDs/hashes, addresses (truncated), wallet derivation, signing transcripts and state artifacts | The helper uses `domain || 0x00 || bytes`, which is unambiguous for a fixed domain. Callers still need a complete canonical-encoding audit; many hand-built text/JSON transcripts coexist. |
| SHA3-256 | `sha3`; privacy and selected type helpers | Asset-Orchard encryption KDF and other 32-byte commitments | Domain and length encoding must be audited per caller; this is not interchangeable with SHA3-384. |
| Keccak-256 | `sha3::Keccak256` | Ethereum addresses, ABI/event/route commitments, EVM-facing SP1 and bridge values | Classical Ethereum compatibility primitive. It does not verify Ethereum consensus, headers, receipts, logs, or finality; asserted external transitions are therefore disabled live. |
| Halo2 over Pasta | upstream Zcash `halo2_proofs 0.3.2` at immutable commit `f6200ada...`, retained in-tree with a narrow verifying-key assembly compatibility patch; `pasta_curves 0.5`; `halo2_gadgets 0.5` | Asset-Orchard swap and private-egress proofs; Orchard action verification | This is not a Halo2 reimplementation. The 361-line normalized patch does not intentionally change the proving algorithm, verifier equations, transcript, fields, curves, or proof encoding. Active circuits use `k=15`; embedded parameters and VK artifacts have hash/fingerprint checks. Exact upstream licenses, commit, patch hash and a fail-closed source verifier are included; specialist review of PostFiat circuits, public inputs, and the local compatibility boundary remains required before real value. |
| Orchard protocol | `orchard 0.14.0`, `zcash_note_encryption`, `incrementalmerkletree`, `nonempty` | Spending keys, full/incoming viewing keys, addresses, note commitments, nullifiers, RedPallas authorization, bundles and note-tree paths | Supported Orchard/Asset-Orchard path meaningfully hides note data. Legacy cleartext Mint/Spend is historical replay only. |
| Pallas/Vesta | `pasta_curves 0.5` | Orchard keys/notes, DH-style note encryption, Halo2 commitment/proof arithmetic | Canonical point parsing and identity rejection are present on inspected note-encryption ingress. Full subgroup/canonical test coverage must be enumerated for every public parser. |
| RedPallas | via `orchard` | Orchard spend authorization and randomized verification keys | Classical discrete-log authorization inside the privacy system. It contradicts any blanket “all authorization is post-quantum” reading; the whitepaper must state the boundary. |
| Sinsemilla | `sinsemilla 0.1`, Orchard gadget constants | Asset-Orchard note commitments and Merkle hashing | Parameter/personalization bindings are implemented in `asset_orchard_sinsemilla.rs` and circuit code; hashes and conformance vectors must be published with the active circuit IDs. |
| Poseidon/Pow5 | `halo2_poseidon 0.1`, Halo2 gadgets | Asset-Orchard action binding inside circuits and matching host computation | Width 3/rate 2 and parameter hashes are pinned. Host/circuit differential tests exist and must be mandatory in CI rather than hidden by Orchard skips. |
| ChaCha20-Poly1305 | `chacha20poly1305 0.10` | Asset-Orchard wallet-note encryption | Uses ephemeral Pallas DH material, SHA3-256 KDF, random 96-bit nonce, chain/genesis/version/commitment-bound AAD, and zeroized key/plaintext buffers. Nonce and ephemeral scalar use `OsRng`. Full misuse, ciphertext malleability, scan-oracle, and metadata tests remain release gates. |
| ZIP-32 | `zip32 0.2.1`, Orchard APIs | Orchard spending-key derivation | Uses `SpendingKey::from_zip32_seed` with account indices below `2^31`; PostFiat coin type is hard-coded to `1`. The choice/collision implications and migration stability require an explicit protocol decision and test vectors. |
| secp256k1 ECDSA | `k256 0.13` | Ethereum-compatible signing/recovery in NAV round-trip tooling | Classical external-chain compatibility boundary. Keys must never be represented as ML-DSA/PQ security, and signing must be local or explicitly custodial. |
| SP1 Groth16 verifier | `sp1-verifier 6.3.1` | NAV reserve aggregate proof verification | Consensus verifies bounded proof/public-value bytes against the profile-bound SP1 program vkey and library Groth16 VK, then checks schema, policy hash, and `verified_net_assets`. Proof-parser adversarial tests, VK upgrade policy, and exact public-input specification remain required. |
| Debug proof system | `crates/proofs` | Debug shielded mint/spend fixtures and benchmark/fuzz plumbing | Explicitly identified as debug by IDs. It must be compile-time unreachable from public production mutation paths or removed from the public binary. |
| SLH-DSA | none found | Whitepaper claims recovery commitments/activation | Not implemented. Present-tense SLH-DSA recovery claims must be removed unless a complete FIPS 205 key commitment, verification, governance activation, migration, and recovery ceremony is implemented and tested. |

## 2. ML-DSA-65 key and domain map

The shared wrapper exposes fixed-size parse, sign, verify, and public-key
validation. Private-key byte arrays are wrapped in `Zeroizing` while parsed and
generated key pairs store private bytes in `Zeroizing<Vec<u8>>`.

Known context strings include:

| Purpose | Context source |
|---|---|
| Normal account transaction | `TX_SIGNATURE_CONTEXT = postfiat-l1-v2/tx/v1` |
| Block certificate vote | `BLOCK_CERTIFICATE_SIGNATURE_CONTEXT = postfiat-l1-v2/block-certificate/v1` |
| Block proposal | `BLOCK_PROPOSAL_SIGNATURE_CONTEXT = postfiat-l1-v2/block-proposal/v1` |
| Block timeout vote/certificate | `BLOCK_TIMEOUT_SIGNATURE_CONTEXT = postfiat-l1-v2/block-timeout/v1` |
| Bridge witness | `BRIDGE_WITNESS_SIGNATURE_CONTEXT = postfiat-l1-v2/bridge-witness/v1` |
| Admission receipt | `ADMISSION_RECEIPT_SIGNATURE_CONTEXT = postfiat-l1-v2/admission-receipt/v1` |
| Owned transfer / unwrap | `OWNED_TRANSFER_CONTEXT`, `OWNED_UNWRAP_CONTEXT` in `rpc_sdk` |
| FastSwap intent/vote | `FASTSWAP_INTENT_CONTEXT_V1`, `FASTSWAP_VOTE_CONTEXT_V1` |
| FastLane deposit/checkpoint/control/asset-control/exit/exit-vote | constants in `crates/types/src/fastswap_types.rs` |
| Cobalt RBC / ABBA messages | `RBC_MESSAGE_SIGNATURE_CONTEXT`, `ABBA_MESSAGE_SIGNATURE_CONTEXT` |
| Snapshot/deployment/operator manifests | constants in `crates/node/src/lifecycle_queries.rs` |
| Validator/dev/deployment key self-check | dedicated self-check contexts in `lifecycle_queries.rs` |

This list is paired with the code-derived canonical transcript table in
`OPEN-SOURCE-STORAGE-STATE-DETERMINISM-INVENTORY-20260716.md` and
`scripts/test-crypto-callsite-policy`. The gate freezes all 46 production uses
of the generic account context and deterministic-seed APIs. A new purpose must
use an explicit context and receive transcript review rather than silently
expanding the generic helper. Each enabled certificate family binds its required
chain/genesis/version and phase-specific state; empty legacy registry roots are
replay-only after `P1-CERT-DOMAIN-01`. A context string still cannot compensate
for an incomplete payload, so golden/mutation vectors remain mandatory for new
types.

### Key-purpose separation

The repository has distinct files/types for account keys, validator signing
keys, snapshot publishers, deployment publishers, operator manifests, bridge
witnesses, Ethereum keys, Orchard spending/viewing keys, and prover material.
The operational boundary is not production complete:

- validator and publisher private keys are plaintext software JSON files;
- the browser wallet now signs locally and the master-seed-bearing proxy contract
  has been removed (`P0-CUSTODY-01` fixed locally);
- unsigned governance support is rejected from live proposal/apply and cannot be
  treated as registered-validator authority (`P0-GOVERNANCE-01` contained);
- no HSM/remote-signer refusal/availability/rotation protocol is the supported
  production default (`P1-KEYS-01`);
- real credential material was captured in Git history (`P0-SECRET-01`).

## 3. Wallet derivation and secret handling

Transparent wallet backups contain a 32-byte master seed as lowercase hex.
`derive_wallet_seed` serializes the derivation domain, algorithm, chain ID,
account index, key role, and master seed; hashes with SHA3-384 using
`postfiat.wallet.seed.v1`; and truncates to 32 bytes before deterministic
ML-DSA-65 key generation. This provides stable domain separation but is a custom
KDF construction, not a password hardening function. It is suitable only when
the master seed is already uniformly random; the product must not accept a
human password or low-entropy phrase as equivalent input.

Required closure:

1. establish CSPRNG provenance for wallet creation on native and WASM targets;
2. add deterministic derivation vectors across Rust/WASM/JavaScript and versions;
3. prove account/chain/role separation and reject index overflow/unknown roles;
4. keep master seeds and transparent/Orchard spend keys client-side and zeroized;
5. define encrypted-at-rest backup format, authentication, recovery, export, and
   migration; do not treat raw JSON backup files as production custody;
6. add network/log/crash-dump/browser-storage tests proving secrets never cross
   the self-custody boundary.

## 4. Asset-Orchard proof and encryption boundary

### 4.1 Live and replay circuit identifiers

- proof system: `postfiat.privacy.asset-orchard-halo2.v1`;
- live swap circuit: `asset_orchard.swap.pricing_bound.v4`;
- replay swap circuit: `asset_orchard.swap.pricing_bound.v3`;
- live private egress: `asset_orchard.private_egress.v2`;
- replay private egress: `asset_orchard.private_egress.v1`;
- active circuit size: `k=15`;
- Merkle depth: `orchard::NOTE_COMMITMENT_TREE_DEPTH`;
- Poseidon width/rate: 3/2.

Live verification rejects replay-only circuit IDs, while archive replay has
explicit cached replay keys. The parameter artifact has exact byte length and
hash checks; VK metadata binds proof system, circuit ID, parameter hash,
Poseidon parameter hash, note layout hash, Merkle parameter hash, and runtime
fingerprint.

### 4.2 Circuit audit outputs

`OPEN-SOURCE-PROOF-PUBLIC-INPUT-INVENTORY-20260716.json` now enumerates every
public instance and witness for the live swap and egress circuits, including:

- chain/genesis/protocol and pool domain;
- action schema/version and circuit/proof-system IDs;
- anchors, nullifiers, input/output commitments and note-tree position/path;
- asset/value conservation and fee behavior;
- exact certified NAV/pricing ratio, rounding, age/expiry and policy binding;
- both DvP legs and both-or-neither atomicity;
- owner spend authorization and external/binding hash;
- egress destination, amount, fee, turnstile and public-credit binding;
- parameter/VK hashes and activation/replay policy.

The inventory is source-hash locked and CI verifies exact 0..27 and 0..12
coverage. It also records the SP1 host-decoded ABI subset, the missing SP1 guest
provenance, and the debug proof system's test/replay-only reachability.
Host-versus-circuit differential tests, malformed-proof fuzzing, unconstrained
witness review, counterfeit-proof negative controls, and independent test-vector
reproduction are mandatory. Current tests are substantial and candidate CI runs
the complete workspace without the former Orchard skip. Specialist circuit
review and clean hosted-run evidence remain real-value launch gates.

### 4.3 Note encryption

`asset_orchard_note_encryption.rs` rejects non-canonical/identity ephemeral
points, derives an ephemeral Pallas shared secret, derives a 32-byte key under
`postfiat.asset_orchard.note_encryption.kdf.v1`, and authenticates context under
`postfiat.asset_orchard.note_encryption.aad.v1`. AAD includes chain/genesis,
protocol version, output commitment, and ephemeral key. ChaCha20-Poly1305 uses
an independently random 96-bit nonce.

Required negative tests include nonce reuse simulation, invalid/non-canonical
points, identity/small-subgroup equivalents, wrong chain/genesis/version,
wrong commitment/recipient, truncated/extended ciphertexts, AEAD tag mutation,
duplicate ciphertext, scan timing/error uniformity, and ciphertext-size/traffic
metadata disclosure. The privacy claim must describe public commitments,
nullifiers, anchors, action counts, timing, pool size, fees, and egress leakage.

## 5. SP1/Groth16 NAV proof boundary

`crates/execution/src/nav_sp1_verifier.rs` is a consensus entry point for
profiles with verifier kind `sp1-groth16`. It:

1. rejects wrong verifier kind, missing proof/public values, and profile-bound
   size-limit violations;
2. calls `Groth16Verifier::verify` with proof bytes, public values, the
   governance/profile-bound SP1 program vkey, and the verifier crate's
   `GROTH16_VK_BYTES`;
3. decodes only the expected aggregate schema;
4. checks the decoded valuation-policy hash;
5. checks decoded `verified_net_assets` against the submitted reserve packet.

Release closure still requires exact provenance and checksums for the SP1
program and Groth16 verifier dependency/VK, public-value ABI documentation,
trailing/ambiguous encoding rejection, wrong-program/wrong-policy/wrong-schema
tests, proof parser fuzzing, maximum-cost benchmarks, governed VK/program
activation and rollback, and reproducibility from the actual guest source.

## 6. Confirmed cryptographic/authorization blockers

| ID | Failure | Required disposition |
|---|---|---|
| `P0-GOVERNANCE-01` | Unsigned legacy support is not a registered-validator certificate | Live path removed; implement signed, old-registry-authorized votes before re-enable |
| `P0-CUSTODY-01` | Baseline self-custody wallet could transmit master-seed-bearing backup | Fixed locally: remote seed contract removed and browser signs locally |
| `P0-SECRET-01` | Reachable history contains a captured access token | Revoke/decommission; sanitize public refs; blocking history scan |
| `P0-BRIDGE-01` | Baseline asserted external facts without Ethereum consensus/log proof | Fixed locally by live-path removal; explicit proof required before re-enable |
| `P0-PRIVACY-01` | Baseline legacy cleartext note path used shielded names | Fixed locally: historical replay only; Asset-Orchard is supported path |
| `P1-KEYS-01` | Validator/publisher keys remain plaintext software files | Production explicitly unsupported; exact unsafe-devnet acknowledgement required |
| `P1-LICENSE-01` | Baseline in-tree Halo2 snapshot lacked complete license/provenance | Fixed with exact licenses, immutable upstream commit, bounded-patch description, and normalized patch verifier |
| `P1-SUPPLYCHAIN-01` | Baseline crypto dependency policy/SBOM handling incomplete | Fixed locally: pinned toolchain, deny policy, audit, provenance and deterministic SBOM |
| `P1-DOCS-01` | Baseline whitepaper overstated PQ coverage and SLH-DSA recovery | Corrected: exact hybrid/classical boundary and no implemented recovery claim |

## 7. STEP 2 cryptography and real-value launch gates

- [x] Freeze the complete generic-context and deterministic-seed sign/verify
      call-site table; new use fails product-security CI.
- [x] Inventory canonical transcripts for every enabled artifact family and
      retain existing golden/domain/mutation vectors; new types require vectors.
- [x] Verify enabled public-key/point/signature/proof parsers fail closed on
      wrong-size/non-canonical/identity/tampered inputs through provider,
      Orchard, FastSwap and adversarial-harness suites.
- [x] Verify native/browser CSPRNG plumbing and deterministic uses: native
      `fips204` uses `rand_core/getrandom`, wallet WASM enables `getrandom/js`,
      Orchard encryption uses `OsRng`, and the three deterministic-key sites plus
      seven deterministic-signature sites are frozen by CI.
- [ ] Close the remaining secret-history P0; governance, custody, bridge and
      privacy live-path P0s are fixed or feature-contained.
- [x] Remove production-ready key-custody claims and require the exact unsafe
      file-signer acknowledgement; HSM/remote signer remains a real-value gate.
- [x] Freeze the machine-readable 28-field swap and 13-field private-egress
      public-instance/witness inventory against the exact proof source hashes.
- [x] Run mandatory Orchard host/circuit differential, proof, encryption,
      nullifier, replay, turnstile, conservation and privacy-leakage suites.
- [x] Run SP1/Groth16 valid, invalid, wrong-program, wrong-policy, malformed,
      maximum-size and resource-exhaustion suites.
- [x] Restore vendored license/provenance; pin toolchain/dependencies; produce an
      SBOM and reproducible checksums for parameters, VKs and binaries.
- [x] Remove present-tense SLH-DSA recovery claims unless the complete mechanism
      is implemented and receives its own migration/adversarial test program.
- [x] Publish an honest security table distinguishing PQ authorization from
      classical privacy, EVM, proof-system, encryption and custody assumptions.

Until every checked gate is backed by candidate-revision evidence, this
inventory is a STEP 1 remediation specification, not a cryptographic assurance
statement.
