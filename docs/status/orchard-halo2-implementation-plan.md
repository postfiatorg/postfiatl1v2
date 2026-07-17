# Orchard/Halo2 Shielded Pool Implementation Plan

Status: implementation plan  
Date: 2026-05-15  
Scope: replace PostFiat debug shielded semantics with a real Orchard/Halo2-backed
shielded pool as fast as possible.

## Objective

Ship a PostFiat shielded pool by reusing the Zcash Orchard Rust stack instead
of writing a new proving system.

The fastest credible target is:

- one asset;
- transparent-to-shielded mint;
- shielded-to-shielded transfer;
- shielded-to-transparent withdraw;
- one or more Orchard actions per shielded transaction, bounded by policy;
- real Halo2 proof verification in node execution;
- encrypted output notes that wallets can scan;
- nullifier double-spend rejection in consensus;
- anchors/root history in consensus;
- fee/resource pricing for proof bytes, ciphertext bytes, and verifier work;
- no claim that Halo2 private transactions are end-to-end post-quantum.

## Crates To Use

Primary dependency:

- `orchard = "0.13.1"`
  - Crates.io: `https://crates.io/crates/orchard/0.13.1`
  - Repo: `https://github.com/zcash/orchard`
  - License: `MIT OR Apache-2.0`
  - Rust: `1.85.1+`
  - Relevant features: default includes `circuit`, `multicore`, `std`.

Supporting candidate:

- `zcash_primitives = "0.27.1"`
  - Crates.io: `https://crates.io/crates/zcash_primitives/0.27.1`
  - Repo: `https://github.com/zcash/librustzcash`
  - License: `MIT OR Apache-2.0`
  - Useful if we need transaction-builder, consensus-encoding, or note-scanning
    glue around Orchard.

Do not start by copy-pasting source into this repo. Start with crates.io
dependencies so we keep upstream security fixes and a clean license boundary.
Fork/vendor only if the public API blocks the PostFiat transaction shape.

## Current PostFiat Integration Points

Current debug privacy state:

- `crates/types/src/lib.rs`
  - `ShieldedNote`
  - `ShieldedState`
  - `ShieldedAction`
  - `ShieldedActionBatch`
- `crates/privacy/src/lib.rs`
  - `mint_debug_note`
  - `spend_debug_note`
  - `migrate_debug_note`
  - `note_tree_root`
  - `scan_owner`
  - `disclose_note`
- `crates/proofs/src/lib.rs`
  - `ProofSystem`
  - `ProofStatement`
  - `ProofArtifact`
  - `DebugProofSystem`
- `crates/node/src/lib.rs`
  - `execute_shielded_batch`
  - `apply_shielded_batch`
  - `shield_scan`
  - `shield_disclose`
  - `shield_turnstile`
  - `verify_shielded`

The replacement should preserve the node ordering path and batch/certificate
path. It should replace the internals of shielded state and action validation.

## Architecture Decision

Use Orchard/Halo2 as the production-candidate privacy v1 backend.

Do not attempt to make Orchard look like ML-DSA or the transparent account
model. Shielded ownership is note/key based. The transparent account layer
remains ML-DSA/PQ. The shielded pool uses Orchard keys/proofs/ciphertexts.

Do not import Zcash consensus wholesale. PostFiat consensus should verify and
apply an Orchard-backed shielded bundle inside PostFiat ordered batches.

The chain stores public shielded consensus data only:

- pool id;
- proof-system id;
- verifier/circuit identity;
- anchor/root;
- nullifiers;
- output commitments;
- encrypted note payloads;
- value balance or turnstile amount fields required for mint/withdraw policy;
- proof bytes;
- finality/receipt ids.

The chain must not store:

- note plaintext;
- spend keys;
- viewing keys;
- witnesses/Merkle paths;
- wallet scan secrets;
- decrypted memo material.

## New Code Shape

Add a narrow adapter crate:

- `crates/privacy_orchard`

This crate owns Orchard-specific code so `crates/node` and `crates/types` do
not become tightly coupled to Zcash internals.

Recommended module layout:

```text
crates/privacy_orchard/
  Cargo.toml
  src/lib.rs
  src/error.rs
  src/types.rs
  src/keys.rs
  src/bundle.rs
  src/tree.rs
  src/verify.rs
  src/wallet.rs
  src/vectors.rs
```

Responsibilities:

- Convert PostFiat shielded actions into Orchard/Halo2 bundle verification.
- Expose stable PostFiat-owned serializable types.
- Hide Orchard crate types behind an adapter boundary.
- Enforce size limits before deserialization/proof verification.
- Produce local test vectors for commitments/nullifiers/ciphertexts/proofs.

## New Protocol Types

Add versioned production shielded types in `crates/types/src/lib.rs`.

Recommended first types:

```rust
pub struct OrchardShieldedAction {
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub anchor: String,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
    pub value_balance: i64,
    pub fee: u64,
    pub proof: BinaryBlob,
    pub binding_signature: BinaryBlob,
}

pub struct EncryptedShieldedOutput {
    pub cmx: String,
    pub epk: String,
    pub enc_ciphertext: BinaryBlob,
    pub out_ciphertext: BinaryBlob,
    pub compact_ciphertext: Option<BinaryBlob>,
}

pub struct OrchardShieldedPoolState {
    pub pool_id: String,
    pub note_commitment_tree_root: String,
    pub root_history: Vec<ShieldedRootRecord>,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    pub encrypted_outputs: Vec<EncryptedShieldedOutput>,
}
```

Exact field names can change during implementation, but the concepts should
not. The current `ShieldedNote` debug structure should not be used for
production Orchard state because it stores owner/value/memo in clear text.

## Consensus Apply Rule

Add a production path next to the debug path:

```text
execute_shielded_batch
  if production privacy disabled:
    reject orchard actions or keep debug-only test behavior
  if production privacy enabled:
    reject debug spend/mint actions
    verify Orchard proof/binding signature
    verify anchor exists in root history
    reject duplicate nullifiers
    apply nullifiers
    append output commitments/encrypted outputs
    update note commitment tree root
    write receipts
```

Consensus must fail closed:

- malformed proof: reject;
- unknown proof-system id: reject;
- wrong circuit/verifier id: reject;
- stale anchor outside retention window: reject;
- duplicate nullifier: reject;
- too many actions: reject;
- proof or ciphertext too large: reject;
- proof verification timeout/concurrency exceeded: reject or defer before
  consensus apply, depending on mempool policy.

## Implementation Slices

### Current Implemented Status

As of 2026-05-15, Slices 0-3 are materially implemented for the bounded
Orchard/Halo2 path, and Slice 4 has local v0 coverage for deposit-side migrated
value, one-note shielded transfer, and one-note shielded-to-transparent
withdraw:

- `crates/privacy_orchard` builds upstream Orchard/Halo2 actions, verifies
  serialized PostFiat-owned action JSON, and binds authorization to the
  PostFiat chain domain plus optional external envelope hash.
- `crates/types/src/lib.rs` has `ShieldedAction::OrchardV1` and
  `ShieldedAction::OrchardWithdrawV1`; the withdraw envelope carries
  recipient, amount, fee, policy id, and disclosure hash outside the proof
  while the proof signature binds their hash.
- `crates/node/src/lib.rs` now applies withdraws atomically: proof verification,
  duplicate-nullifier/root checks, Orchard accounting, fee burn, withdraw-total
  accounting, and transparent ledger credit happen inside one ordered shielded
  batch commit.
- The focused node test
  `orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers`
  covers real output/spend proofs, underpriced fee rejection, external withdraw
  envelope mismatch rejection, valid ordered withdraw ledger crediting, and
  post-withdraw pool accounting.
- `scripts/testnet-orchard-wallet-finality-smoke` now proves the local ordered
  wallet path through withdraw and audited `tx` finality at height 5. Latest
  evidence:
  `reports/testnet-orchard-wallet-finality-smoke/withdraw-v0-20260515T075515Z/testnet-orchard-wallet-finality-smoke.json`.
- `scripts/testnet-orchard-peer-certified-smoke` now proves the same
  output/spend/withdraw path across a local four-validator peer-certified set
  with convergence at height 5. Latest evidence:
  `reports/testnet-orchard-peer-certified-smoke/withdraw-v0-20260515T080523Z/testnet-orchard-peer-certified-smoke.json`.

### Slice 0: Dependency Proof Of Life

Goal: prove the Zcash Orchard crate builds inside the PostFiat workspace.

Tasks:

1. Add `orchard = "0.13.1"` behind a new `privacy-orchard` crate.
2. Add a minimal test that imports:
   - `orchard::bundle::Bundle`;
   - `orchard::bundle::Authorized`;
   - `orchard::note::Nullifier`;
   - `orchard::circuit::VerifyingKey`.
3. Run `cargo test -p postfiat-privacy-orchard`.
4. Record compile time, dependency tree size, and Rust version requirement.

Exit:

- Workspace builds with Orchard dependency isolated.
- No node/consensus behavior changes.

### Slice 1: Orchard Type Adapter

Goal: define PostFiat-owned serializable wrappers around Orchard public data.

Tasks:

1. Add `OrchardProofSystemId`, `OrchardCircuitId`, `OrchardAnchor`,
   `OrchardNullifier`, `OrchardOutputCommitment`, and `OrchardProofBytes`.
2. Add parse/serialize functions that reject:
   - wrong byte length;
   - non-canonical encodings;
   - empty vectors where not allowed;
   - oversized proofs/ciphertexts.
3. Add deterministic JSON fixtures.
4. Add fuzz targets for the parsers.

Exit:

- Node can parse production Orchard action payloads without linking Orchard
  internals through the entire codebase.

### Slice 2: Verification Adapter

Goal: verify an already-created Orchard authorized bundle/proof from bytes.

Tasks:

1. Implement `verify_orchard_action(action, context) -> Result<VerifiedAction>`.
2. Bind verification to:
   - chain id;
   - genesis hash;
   - protocol version;
   - pool id;
   - proof-system id;
   - circuit/verifier id;
   - anchor;
   - nullifiers;
   - output commitments;
   - fee;
   - value balance;
   - ciphertext hashes.
3. Call Orchard `Bundle<Authorized, V>::verify_proof(&VerifyingKey)` through
   the adapter.
4. Verify Orchard binding/spend authorization signatures if not already covered
   by the selected bundle verification path.
5. Add tamper tests:
   - proof byte flip rejected;
   - anchor flip rejected;
   - nullifier flip rejected;
   - output commitment flip rejected;
   - wrong circuit id rejected;
   - oversized proof rejected before expensive work.

Exit:

- A fixture Orchard action verifies.
- Every public-field tamper test fails closed.

### Slice 3: Pool State And Root History

Goal: replace debug note storage with production pool state.

Tasks:

1. Add `OrchardShieldedPoolState`.
2. Use Orchard-compatible incremental tree data structures where possible.
3. Store only commitments, encrypted outputs, nullifiers, and root history.
4. Define root retention:
   - controlled testnet default: keep recent N roots;
   - archive/indexer mode: keep full output history;
   - validator partial-history mode: keep enough roots for spend windows.
5. Add snapshot/export/import support.
6. Add state verifier:
   - root recomputes;
   - nullifiers unique;
   - outputs unique;
   - root history monotonic;
   - no plaintext note fields.

Exit:

- Production pool state survives restart, snapshot, export/import, and
  verification.

### Slice 4: Mint, Spend, Withdraw Actions

Goal: get value into and out of the shielded pool under PostFiat accounting.

Tasks:

1. Transparent-to-shielded mint:
   - debit transparent account or lock transparent value;
   - create Orchard output;
   - append output commitment/encrypted output;
   - record turnstile event.
2. Shielded-to-shielded spend:
   - verify Orchard proof;
   - reject duplicate nullifiers;
   - append outputs;
   - update root.
3. Shielded-to-transparent withdraw:
   - verify proof and value balance;
   - credit transparent account only after proof verifies;
   - record turnstile event.
4. Add receipt fields:
   - accepted/rejected;
   - nullifiers;
   - output commitments;
   - new root;
   - fee charged;
   - proof-system id.

Exit:

- Local node executes mint -> spend -> withdraw with real Orchard/Halo2 proof
  verification and no debug proof dependency.

### Slice 5: Wallet And Scanning

Goal: users can receive, scan, spend, and disclose notes.

Tasks:

1. Add shielded key generation using Orchard keys.
2. Add receive address/export format.
3. Add wallet scan over encrypted outputs.
4. Add witness tracking for spendable notes.
5. Add spend construction using Orchard builder or a PostFiat adapter around it.
6. Add disclosure packet:
   - decrypted note;
   - tx/receipt/finality reference;
   - nullifier if spent;
   - auditor instructions;
   - redaction rules.

Exit:

- CLI or SDK can:
  - create shielded wallet;
  - generate receive key/address;
  - mint to shielded;
  - scan and find note;
  - spend note;
  - withdraw;
  - create disclosure artifact.

### Slice 6: RPC And Indexer

Goal: expose the minimum data needed by wallets without leaking private state.

Tasks:

1. Add read RPC:
   - current shielded root;
   - root history window;
   - encrypted outputs by height/range;
   - nullifier status;
   - shielded transaction/receipt finality.
2. Add bounded submit path for shielded actions.
3. Add SDK validators for every response.
4. Add pagination and byte limits.
5. Add archive/indexer role for full encrypted-output history.

Exit:

- Wallet can sync from RPC and submit shielded transactions through the same
  controlled write path as transparent transactions.

### Slice 7: Fees, DoS Bounds, And Latency

Goal: make shielded transactions safe for validators.

Tasks:

1. Define max actions per shielded transaction.
2. Define max proof bytes and ciphertext bytes.
3. Define verifier concurrency limit.
4. Move expensive proof verification off reactor threads.
5. Add fee components:
   - base tx fee;
   - proof verification fee;
   - proof byte fee;
   - ciphertext/output storage fee;
   - new-account/new-output state expansion fee if needed.
6. Add benchmarks:
   - prove time;
   - verify time;
   - memory;
   - proof size;
   - tx size;
   - block apply time;
   - RPC sync size.

Exit:

- Valid shielded txs clear under bounded latency.
- Invalid/oversized shielded txs cannot starve transparent finality.

### Slice 8: Evidence Gate

Goal: prove this is real enough for semi-production privacy alpha.

Required gate:

1. local 5-validator mint -> scan -> spend -> withdraw;
2. duplicate nullifier rejected;
3. wrong anchor rejected;
4. proof tamper rejected;
5. ciphertext tamper detected by wallet scan/decrypt;
6. restart preserves pool state;
7. snapshot export/import preserves pool state;
8. below-quorum outage does not advance state;
9. post-recovery shielded spend finalizes;
10. RPC scan works from clean wallet;
11. disclosure packet verifies against receipt/finality evidence;
12. redaction scan proves no note plaintext or private keys in public reports.

Exit:

- A single report under `reports/privacy-orchard-alpha-gate/...` captures the
  above.

## First 48 Hours

### Hour 0-4

- Add `crates/privacy_orchard`.
- Add `orchard = "0.13.1"`.
- Prove the workspace builds with Orchard isolated.
- Add dependency/license/Rust-version note.

### Hour 4-12

- Define PostFiat Orchard wrapper types.
- Add parser bounds and deterministic fixtures.
- Add tests for malformed byte lengths and oversized payloads.

### Hour 12-24

- Build an out-of-consensus proof-of-life:
  - generate Orchard keys;
  - build one output;
  - build one spend if Orchard builder path is straightforward;
  - verify proof through `privacy_orchard`.
- If builder API is too Zcash-transaction-shaped, isolate the gap and decide
  whether to use `zcash_primitives` or fork/adapt Orchard builder code.

### Hour 24-36

- Wire production shielded action parsing into `crates/types`.
- Add `ShieldedAction::Orchard` or a versioned `ShieldedActionV1`.
- Keep old debug actions explicitly debug-only.

### Hour 36-48

- Wire node verification path:
  - parse action;
  - verify proof;
  - reject duplicate nullifier;
  - append output commitment;
  - update root;
  - write receipt.
- Add local tests for accept/reject paths.

## Main Technical Risks

1. Orchard builder may assume Zcash transaction structures more strongly than
   we want. If so, use `zcash_primitives` glue or fork the builder while keeping
   the core proof/circuit types upstream.
2. Serialization must be PostFiat-owned. Consensus-critical bytes cannot depend
   on accidental Rust debug or serde defaults from upstream crates.
3. Proof verification must be bounded. Halo2 proof parsing/verifying cannot be
   reachable through unbounded RPC payloads.
4. Wallet scanning is not optional. A verified shielded pool without a wallet
   scan path is not a usable privacy product.
5. Halo2/Orchard is not post-quantum. It is the fast real-privacy path; the
   PQ-private migration remains a separate versioned backend.

## Definition Of Done For Alpha

The alpha is done when a fresh user can run:

```text
shielded keygen
shielded receive-address
transparent mint-to-shielded
shielded scan
shielded spend
shielded withdraw
shielded disclose
tx finality
```

against a local 5-validator network, with real Orchard/Halo2 proof
verification, encrypted output scanning, duplicate-nullifier rejection, bounded
RPC payloads, and a machine-readable evidence report.

## Immediate Next Code Task

Start with Slice 0 and Slice 1:

1. create `crates/privacy_orchard`;
2. add `orchard = "0.13.1"`;
3. add a compile-only proof-of-life test;
4. add PostFiat wrapper types for proof-system id, circuit id, anchor,
   nullifier, output commitment, encrypted output, and proof bytes;
5. add parser bounds and fixture tests.

That gives the codebase a real Orchard integration point without touching
consensus yet. After that, wire verification into `execute_shielded_batch`.

## Progress Log

### 2026-05-15: Slice 0 Complete, Slice 1 Started

Implemented:

- Added isolated `crates/privacy_orchard`.
- Added upstream `orchard = "0.13.1"` as a workspace dependency.
- Added compile-only proof-of-life imports for:
  - `orchard::bundle::Bundle`;
  - `orchard::bundle::Authorized`;
  - `orchard::note::Nullifier`;
  - `orchard::circuit::VerifyingKey`.
- Added PostFiat-owned wrapper/parser types for:
  - proof-system id;
  - circuit id;
  - anchor;
  - nullifier;
  - output commitment;
  - proof bytes;
  - bounded ciphertext/signature blobs;
  - encrypted shielded output;
  - Orchard shielded action shape.
- Added fail-closed parser/shape tests for fixed byte lengths, lowercase
  canonical hex, proof byte bounds, output-count mismatch, and valid bounded
  action shape.
- Added upstream Orchard byte conversions for:
  - `orchard::Anchor`;
  - `orchard::note::Nullifier`;
  - `orchard::note::ExtractedNoteCommitment`.
- Added rejection tests for non-canonical Orchard field encodings.
- Added `postfiat-fuzz orchard-parser` coverage for Orchard action JSON parser
  mutation, fail-closed deserialization, wrapper validation, and upstream
  Orchard conversion invariants.
- Changed Orchard wrapper deserialization to use `try_from` parsers so invalid
  JSON cannot bypass canonical hex, byte-size, or Orchard field checks.
- Enforced exact Orchard ciphertext component sizes at the adapter boundary:
  - `epk`: 32 bytes;
  - `enc_ciphertext`: 580 bytes;
  - `out_ciphertext`: 80 bytes;
  - `compact_ciphertext`: 52 bytes when present.
- Added `EncryptedShieldedOutput::from_bytes` so tests and fuzz seeds build
  valid output payloads through the same typed constructor expected by
  production callers.
- Added rejection coverage for wrong ciphertext component lengths.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 10 tests.
- `cargo check --workspace` passed after adding the new crate.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `git diff --check` passed.

Next:

- Start Slice 2 by verifying an already-created Orchard authorized bundle/proof
  through the adapter and binding the verification context to PostFiat chain
  metadata.

### 2026-05-15: Slice 2 Started

Implemented:

- Added `verify_authorized_bundle` in `crates/privacy_orchard` for upstream
  `orchard::Bundle<Authorized, V>` values.
- Added `OrchardVerificationContext` carrying:
  - proof-system id;
  - circuit id;
  - max action bound;
  - authorizing sighash supplied by the PostFiat transaction layer.
- The adapter now verifies:
  - Orchard/Halo2 proof via `Bundle::verify_proof`;
  - binding signature against the authorizing sighash;
  - every spend authorization signature against the same authorizing sighash;
  - max action count before proof/signature verification;
  - extracted anchor, nullifiers, output commitments, encrypted outputs, and
    value balance after verification.
- Added a real proof test that builds one output-only Orchard bundle, creates a
  Halo2 proof, applies signatures, verifies through the adapter, and rejects a
  wrong authorizing sighash using the same generated proof.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 12 tests in 39.98s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Add PostFiat canonical transaction-domain hashing for Orchard authorizing
  sighash instead of using caller-provided test bytes.
- Add serialized fixture support so node and RPC paths can carry a
  PostFiat-owned Orchard action payload without leaking upstream Orchard
  consensus encoding into the rest of the codebase.

### 2026-05-15: Canonical Orchard Authorizing Sighash Added

Implemented:

- Added `OrchardAuthorizingDomain` for the PostFiat chain-domain material that
  signs Orchard bundles:
  - chain id;
  - genesis hash;
  - protocol version;
  - pool id.
- Added `orchard_authorizing_sighash` using length-prefixed canonical binary
  fields and `postfiat.privacy.orchard-authorizing-sighash.v1` domain
  separation.
- The sighash covers:
  - proof-system id;
  - circuit id;
  - Orchard flags;
  - anchor;
  - value balance;
  - action count;
  - per-action nullifier, randomized verification key, output commitment,
    value commitment, ephemeral key, note ciphertext, and outgoing ciphertext.
- Added `OrchardVerificationContext::for_bundle` so verification derives the
  same sighash from the PostFiat domain and Orchard bundle public data.
- Updated the real proof test so signatures are created and verified against
  the canonical PostFiat Orchard sighash. A wrong chain-domain value now fails
  binding-signature verification.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 13 tests in 39.57s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Add serialized fixture support for a PostFiat-owned Orchard action payload.
- Start node-side production privacy wiring behind an explicit feature/config
  gate: parse action, derive sighash, verify bundle, reject duplicate
  nullifiers, and expose verified public data to pool-state apply.

### 2026-05-15: Serialized Orchard Action Schema Corrected

Implemented:

- Expanded `OrchardShieldedAction` so a serialized PostFiat action carries the
  public inputs required to reconstruct and verify an authorized Orchard bundle:
  - flags;
  - nullifiers;
  - randomized verification keys;
  - value commitments;
  - output commitments;
  - encrypted outputs;
  - spend authorization signatures;
  - binding signature;
  - proof bytes.
- Added typed wrappers/parsers for:
  - Orchard flags;
  - randomized verification keys;
  - value commitments;
  - spend authorization signatures;
  - binding signatures.
- Validation now requires every per-action vector to match the bundle action
  count, and rejects reserved flag bits and non-canonical Orchard public
  inputs.
- Added `orchard_action_from_authorized_bundle` to produce a PostFiat-owned
  `OrchardShieldedAction` from an upstream `orchard::Bundle<Authorized, V>`.
- The real proof test now serializes the authorized bundle to JSON, parses it
  back through PostFiat wrappers, validates it, and checks parsed public data
  against the verified bundle output.
- Updated `postfiat-fuzz orchard-parser` to seed from an Orchard-built bundle
  so parser fuzzing exercises valid randomized verification keys and value
  commitments instead of fake public inputs.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 13 tests in 39.58s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Define the node-side production privacy gate and state apply surface around
  the corrected `OrchardShieldedAction` schema.
- Add a reconstruction/verification path from serialized PostFiat action bytes
  to Orchard verifier inputs; if upstream Orchard does not expose enough public
  constructors, isolate the exact fork/glue requirement before touching node
  consensus.

### 2026-05-15: Serialized Action Reconstruction Works

Implemented:

- Added `orchard_bundle_from_action` to reconstruct an upstream
  `orchard::Bundle<Authorized, i64>` from PostFiat-owned
  `OrchardShieldedAction` fields.
- Added `verify_serialized_orchard_action` to:
  - check action pool id against the PostFiat authorizing domain;
  - reconstruct the Orchard bundle;
  - derive the canonical PostFiat Orchard sighash;
  - verify the real Halo2 proof;
  - verify binding and spend authorization signatures.
- Added exact-byte extraction helpers for proof/ciphertext blobs.
- Added a direct `nonempty = "0.11.0"` dependency because Orchard bundle
  reconstruction requires `NonEmpty<Action<_>>`, and this is the same crate
  already used by upstream Orchard.
- The real proof test now proves the full submitted-action path:
  upstream bundle -> PostFiat JSON action -> parse -> reconstruct Orchard
  bundle -> verify proof/signatures.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 13 tests in 40.09s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Wire node-side production privacy behind an explicit gate using
  `verify_serialized_orchard_action`.
- Add pool-state apply scaffolding for verified nullifier/output insertion
  before enabling any RPC submit path.

### 2026-05-15: Node Orchard Gate And Pool-State Apply Landed

Implemented:

- Added production Orchard pool state alongside the existing debug shielded
  state, without treating debug notes as production-private notes.
  - `crates/types/src/lib.rs:406` defines `OrchardPoolState`.
  - `crates/types/src/lib.rs:431` defines persisted encrypted output records.
- Added a node-side action gate:
  - `crates/node/src/lib.rs:6376` loads a serialized
    `OrchardShieldedAction`;
  - derives the PostFiat chain-bound Orchard authorizing domain;
  - verifies the reconstructed Orchard/Halo2 proof and signatures through
    `verify_serialized_orchard_action_with_built_key`;
  - optionally applies verified public pool state.
- Added pool-state apply checks:
  - duplicate nullifiers inside the action are rejected;
  - duplicate output commitments inside the action are rejected;
  - replayed nullifiers against persisted pool state are rejected;
  - replayed output commitments against persisted pool state are rejected;
  - nonzero Orchard value balances are rejected until transparent/shielded
    turnstile accounting is implemented;
  - accepted anchors are retained as a unique set.
- Added persisted Orchard pool-state verification in
  `crates/node/src/lib.rs:7131`.
- Added a local CLI gate:
  - `postfiat-node orchard-action --action-file PATH` verifies without state
    mutation;
  - `POSTFIAT_ALLOW_DIRECT_STATE=1 postfiat-node orchard-action --action-file
    PATH --apply` verifies and applies to local shielded state.
- Added a node integration test that creates a real Orchard/Halo2 proof,
  serializes it as a PostFiat action, verifies it through the node gate, applies
  it, proves nonzero value-balance rejection at the apply boundary, then proves
  replay rejection by duplicate nullifier.

Checks:

- `cargo check -p postfiat-node` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 75.88s.
- `cargo test -p postfiat-privacy-orchard` passed with 13 tests in 39.72s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Move Orchard actions from the direct local gate into an ordered/certified
  shielded batch path.
- Add root-history policy and real Orchard note tree anchors instead of the
  current bootstrap zero-anchor test action.
- Add wallet-side encrypted note scanning around Orchard transmitted notes.
- Add pricing/limits for proof bytes, ciphertext bytes, verifier time, and
  persisted output growth before any public RPC submit path.

### 2026-05-15: Orchard Actions Enter Ordered Shielded Batch Path

Implemented:

- Added `OrchardActionPayload` to `ShieldedActionBatch` as
  `kind = "orchard_action_v1"` so Orchard actions can use the existing
  shielded batch, block proposal, certificate, archive, replay, and apply
  machinery.
  - `crates/types/src/lib.rs:492`
  - `crates/types/src/lib.rs:498`
- Added `create_orchard_action_batch` to verify an Orchard action, canonicalize
  it to compact JSON, and wrap it in a chain-bound shielded batch.
  - `crates/node/src/lib.rs:6698`
- Added `execute_orchard_shielded_action` so `apply-shield-batch` verifies and
  applies Orchard actions through the ordered shielded commit path.
  - `crates/node/src/lib.rs:11719`
- Extended `verify-shielded` / RPC `verify_shielded` output with Orchard pool
  id, nullifier count, output count, and anchor count.
  - `crates/node/src/lib.rs:1123`
  - `crates/node/src/lib.rs:6830`
- Added CLI:
  - `postfiat-node shield-batch-orchard --action-file PATH --batch-file PATH`
  - then `postfiat-node apply-shield-batch --batch-file PATH`
- Extended the real-proof node test to prove both paths with one generated
  Orchard/Halo2 action:
  - direct local verify/apply gate;
  - ordered shielded batch create/apply path;
  - nonzero value-balance fail-closed behavior;
  - duplicate-nullifier replay rejection.

Checks:

- `cargo check -p postfiat-node` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 96.65s.
- `cargo test -p postfiat-privacy-orchard` passed with 13 tests in 38.74s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Add root-history policy and real Orchard note tree anchors instead of the
  current bootstrap zero-anchor test action.
- Add wallet-side encrypted note scanning around Orchard transmitted notes.
- Add fee/resource pricing and verifier concurrency limits before any public
  RPC submit path.

### 2026-05-15: Orchard Root History And Retained-Anchor Gate

Implemented:

- Added upstream Orchard note-commitment root computation from persisted output
  commitments using `incrementalmerkletree` plus Orchard's
  `MerkleHashOrchard`.
- Added canonical empty Orchard root helper and root recomputation helper to the
  Orchard adapter.
- Added `root_history` to `OrchardPoolState` with `(root, output_count)`
  records.
- Updated local and ordered Orchard apply paths so:
  - empty pools only accept actions anchored to the canonical empty Orchard
    root;
  - existing pools only accept actions anchored to a retained root;
  - accepted actions append the new recomputed Orchard root;
  - duplicate nullifiers, duplicate commitments, and unretained anchors reject
    before state mutation.
- Extended `verify-shielded` / RPC `verify_shielded` with retained-root count
  and latest retained root.
- Extended state verification to recompute every retained root from the stored
  output commitment prefix and reject mismatches, duplicate roots,
  non-monotonic output counts, and accepted anchors outside retained history.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 14 tests in 39.29s.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 109.98s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.

Next:

- Build wallet-side Orchard address/key, encrypted-output scan/decrypt, and
  spend construction around retained roots.
- Add transparent/shielded turnstile accounting for nonzero Orchard value
  balances.
- Add proof/ciphertext fee pricing, verifier concurrency limits, and
  operator-configured root retention before public shielded RPC submission.

### 2026-05-15: Orchard Wallet Scan V0

Implemented:

- Added upstream Orchard note-decryption integration through
  `zcash_note_encryption`.
- Added adapter helpers to:
  - derive the default raw Orchard address from 32-byte Orchard spending-key
    material;
  - trial-decrypt persisted encrypted outputs using the aligned action
    nullifier list to recover the Orchard `rho` domain;
  - return decrypted output index, commitment, derived note nullifier, rho,
    value, raw address, and memo bytes.
- Added node `orchard-scan --spending-key-hex HEX` for local scan reports over
  persisted Orchard pool outputs.
- The real-proof node test now proves the correct spending key recovers the
  owned output and a wrong spending key recovers none.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed with 14 tests in 39.85s.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 109.63s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed with
  0 invariant failures.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Replace raw CLI spending-key input with a shielded wallet/receive-key file
  format.
- Build wallet-created Orchard actions against retained roots.
- Add nonzero value turnstile accounting, fee policy, and finality-aware
  shielded SDK flow.

### 2026-05-15: Orchard Wallet Key File V0

Implemented:

- Added deterministic `orchard-keygen` from 32-byte master seed plus account
  index to canonical Orchard spending-key material.
- Added private `postfiat-orchard-wallet-v1` key files with schema, KDF,
  derivation domain, account index, spending key, and default raw Orchard
  address.
- Updated `orchard-scan` to accept either `--key-file` or
  `--spending-key-hex`, with fail-closed validation if both or neither are
  supplied.
- Extended the proof-heavy node test to generate an Orchard key file and scan
  persisted Orchard outputs from that file.

Checks:

- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 107.63s.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Add nonzero value turnstile accounting, fee policy, and finality-aware
  shielded SDK flow.

### 2026-05-15: Orchard View Key Export V0

Implemented:

- Added Orchard full-viewing-key derivation from spending-key bytes in the
  adapter, plus default-address derivation from full viewing keys.
- Added full-viewing-key scanning over persisted encrypted Orchard outputs so a
  local receiver can scan without exposing spend authority.
- Added node `orchard-view-key-export --key-file PATH --view-key-file PATH`
  with private `postfiat-orchard-view-key-v1` file validation.
- Updated `orchard-scan` to accept exactly one of `--view-key-file`,
  `--key-file`, or `--spending-key-hex`.
- Extended the proof-heavy node test so unrelated view keys decrypt zero
  outputs, the correct view key decrypts the accepted output, and exported view
  key files do not contain `spending_key_hex`.

Checks:

- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed.
- `cargo check --workspace` passed.
- `git diff --check` passed.

Next:

- Add nonzero value turnstile accounting, fee policy, and finality-aware
  shielded SDK flow.

### 2026-05-15: Wallet-Created Orchard Output Actions V0

Implemented:

- Added adapter-side `orchard_build_output_action` to build a real
  Orchard/Halo2 output bundle against a retained anchor and raw Orchard
  recipient address.
- Added node `orchard-output-create` to write a verified zero-value Orchard
  action file using exactly one recipient source: raw address, wallet key file,
  or receive-only view-key file.
- The action creator anchors to the latest retained Orchard root, so a second
  wallet-created output action can be accepted after the pool already has
  history.
- Extended the proof-heavy node integration test to create, apply, and scan a
  wallet-created zero-value output action after the duplicate-replay check.

Checks:

- `cargo check --workspace` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 155.57s.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed.
- `git diff --check` passed.

Next:

- Build wallet-created spends against scanned notes plus retained inclusion
  paths.
- Add finality-aware shielded SDK flow.

### 2026-05-15: Orchard Deposit Turnstile Accounting V0

Implemented:

- Added Orchard pool accounting fields for cumulative value balance and
  consumed turnstile deposit budget.
- Updated `orchard-output-create --value N` so wallet-created output actions can
  carry nonzero values.
- Apply now accepts negative Orchard value balances only when prior
  `pool_migration` events into `orchard-v1` cover the deposit amount.
- Migration now nullifies the source debug note, so migrated budget cannot also
  be spent in the debug pool.
- Positive Orchard value balances still fail closed because withdraw accounting
  is not implemented.
- `verify-shielded` now reports Orchard value-balance and turnstile deposit
  totals, rejects migration events whose source note is not nullified, and
  rejects inconsistent Orchard turnstile accounting.
- Extended the proof-heavy node integration test to mint a debug shielded note,
  migrate it into `orchard-v1`, create a nonzero Orchard output action, apply
  it, scan the resulting note, and verify the accounting totals.

Checks:

- `cargo check --workspace` passed.
- `cargo test -p postfiat-privacy` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 211.71s.
- `cargo test -p postfiat-node external_proposal_certificates_apply_non_transparent_batches -- --nocapture`
  passed.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed.
- `git diff --check` passed.

Next:

- Build wallet-created spends against scanned notes plus retained inclusion
  paths.
- Add withdraw accounting and fee policy.
- Add finality-aware shielded SDK flow.

### 2026-05-15: Orchard Scan Witness Material V0

Implemented:

- Added adapter-side Orchard Merkle witness construction from retained output
  commitments. It emits the note position, latest anchor, output count, and
  32-node auth path while checking the reconstructed root matches the upstream
  Orchard frontier root.
- Extended `orchard-scan` reports with `latest_retained_root`,
  `latest_retained_output_count`, and per-note `merkle_position`,
  `witness_anchor`, `witness_output_count`, and `witness_auth_path`.
- The node scan path now verifies Orchard pool state before returning witness
  material, so corrupted root history or output/commitment shape fails closed.
- Extended the proof-heavy node integration test to assert witness material for
  spending-key scans, view-key scans, wallet-created outputs, and migrated-value
  outputs.

Checks:

- `cargo fmt --all` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 212.06s.
- `cargo check --workspace` passed.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed.

Next:

- Build wallet-created spends from scanned note data plus the retained witness
  path.
- Add withdraw accounting and fee policy.
- Add finality-aware shielded SDK flow.

### 2026-05-15: Orchard Wallet Spend Actions V0

Implemented:

- Added adapter-side `orchard_build_spend_action` for a real Orchard/Halo2
  spend plus output bundle. It reconstructs the decrypted note, validates the
  note commitment, validates the Merkle path against the requested retained
  anchor, signs with the Orchard spend authorizing key, and rejects nonzero
  value-balance spend actions that do not match the signed fee.
- Added node `orchard-spend-create` for one-note private transfers: rescan
  with a spending key or wallet key file, select `--input-output-index`, build
  a spend action to one recipient address/key/view-key, verify it, report the
  minimum fee when a fee is burned, and write the action file.
- Extended local scan output with `rseed`, which is private wallet material
  needed to reconstruct the note for spending. Treat Orchard scan reports as
  sensitive wallet artifacts.
- Extended the proof-heavy node integration test to create, apply, and rescan a
  real Orchard spend of a migrated-value note. The rescan proves the original
  note is spent and the replacement note is unspent.

Checks:

- `cargo fmt --all` passed.
- `cargo check -p postfiat-privacy-orchard -p postfiat-node` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 267.86s.
- `cargo check --workspace` passed.
- `cargo run -p postfiat-fuzz -- orchard-parser --iterations 64` passed.

Current limits:

- This was the initial V0 spend step: one decrypted note into one same-value
  Orchard output before fee burn and change output support landed.

Next:

- Add fee burn, change handling, and withdraw accounting.
- Add finality-aware shielded SDK flow around keygen, scan, spend-create,
  action apply, and finality lookup.

### 2026-05-15: Orchard Fee-Bound Authorization Domain

Implemented:

- Added PostFiat `fee` to the Orchard authorizing sighash. This binds the
  action fee to the same signature domain as chain id, genesis hash, protocol
  version, pool id, proof system, circuit id, anchor, value balance, and action
  fields.
- Updated serialized-action verification to derive the verification context
  from `action.fee`, so mutating fee after signing now invalidates the binding
  signature.
- Updated Orchard output/spend builders and test action construction to sign
  with the fee-bound domain.
- Added adapter test coverage that the sighash changes when fee changes and a
  serialized action with mutated fee is rejected.

Checks:

- `cargo fmt --all` passed.
- `cargo check -p postfiat-privacy-orchard -p postfiat-node` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 270.23s.

Next:

- Add fee burn, change handling, and withdraw accounting now that fee cannot be
  changed without invalidating authorization.

### 2026-05-15: Orchard Spend Fee Burn V0

Implemented:

- Updated `orchard_build_spend_action` so a one-note spend can burn a signed fee
  by creating recipient outputs whose total value is `input_value - fee`.
- Node apply now accepts positive Orchard value balance only when it exactly
  equals the signed `action.fee`; otherwise positive value balance still fails
  closed as unsupported withdraw or fee mismatch.
- Orchard pool state now tracks cumulative `fee_burn_total`, updates
  `value_balance_total` for fee burns, and verifies
  `turnstile_deposit_total == issued_value + fee_burn_total`.
- Accepted Orchard fee-burn receipts populate `fee_charged` and `fee_burned`.
- Extended the proof-heavy node integration test to spend a migrated 7-unit
  note with a 2-unit fee, apply the action, rescan the 5-unit replacement note,
  and verify pool/receipt fee accounting.

Checks:

- `cargo fmt --all` passed.
- `cargo check -p postfiat-privacy-orchard -p postfiat-node` passed.
- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 267.23s.

Current limits:

- Fee support exists for one-note private spend actions only.
- Direct positive-balance withdraw remains closed unless the positive balance
  exactly equals the signed fee.
- No configurable change address selection, multi-input transfer, direct
  transparent withdraw, or deposit/outer-envelope fee policy yet.

Next:

- Add SDK/finality wrapper for the local privacy flow.
- Add withdraw accounting as a separate explicit turnstile path.

### 2026-05-15: Orchard Fee-Burn Minimum Policy V0

Implemented:

- Added deterministic minimum-fee enforcement for positive Orchard
  value-balance actions that burn fees from shielded value.
- Minimum fee is currently `max(2, ceil(action_weight_bytes / 1 MiB))`, where
  `action_weight_bytes` is computed from typed Orchard public fields, proof
  bytes, ciphertext bytes, and signatures rather than ad hoc JSON formatting.
- Underpriced positive value-balance actions reject with
  `orchard_fee_too_low` before pool mutation.
- Accepted fee-burn receipts now include `minimum_fee` in addition to
  `fee_charged` and `fee_burned`.
- `orchard-spend-create` reports the computed `minimum_fee` for wallet-created
  one-note spends.

Current limits:

- This policy covers the current fee source: positive value balance burned from
  shielded value.
- Zero-balance and deposit-side Orchard actions still need a transparent outer
  transaction envelope or separate deposit fee policy before they are
  anti-spam complete.
- The fee constants are v0 fixed constants, not yet governance-configurable.

Next:

- Add a finality-aware wallet/SDK wrapper around keygen, scan, spend, apply,
  and tx-finality evidence.

### 2026-05-15: Orchard Wallet Finality Smoke V0

Implemented:

- Added `scripts/testnet-orchard-wallet-finality-smoke`.
- The script builds `postfiat-node`, initializes a local one-validator chain,
  creates Orchard wallet/view-key files plus a distinct change view key, runs
  ordered shielded mint, migration into `orchard-v1`, Orchard output creation,
  wallet scan, one-note `orchard-spend-create --amount N --fee N` with
  `--change-recipient-view-key-file`, ordered Orchard spend apply, post-spend
  recipient/change scans, `verify-shielded`, and
  `rpc --method tx --audit-block-log` for mint/migrate/output/spend receipts.
- Script reports redact private key material and ignore generated local devnet
  artifacts under `devnet/orchard-wallet-finality-smoke/`.

Evidence:

- `scripts/testnet-orchard-wallet-finality-smoke` passed.
- Latest report:
  `reports/testnet-orchard-wallet-finality-smoke/20260515T062948Z/testnet-orchard-wallet-finality-smoke.json`.

Current limits:

- This is local one-validator ordered finality evidence, not a 4/5 or live
  validator privacy-alpha gate.
- The script does not yet cover withdraw, duplicate-nullifier replay, malformed
  action tamper, restart recovery, or snapshot/export/import.

Next:

- Extend the local privacy gate with duplicate-nullifier/tamper checks and a
  multi-validator mode.
- Add SDK-library packaging for the same flow.
- Add disclosure metadata.

### 2026-05-15: Orchard Spend Change Output V0

Implemented:

- Extended `orchard_build_spend_action` so a one-note spend can choose a
  recipient value and return remaining value after fee to a change output.
- Added `orchard-spend-create --amount N`; when omitted, the legacy behavior
  sends `input_value - fee` to the recipient. When provided, the wallet sends
  `--amount` to the recipient and returns `input_value - fee - amount` to the
  spender default Orchard address.
- Spend reports now expose `output_value`, `recipient_value`, `change_value`,
  and `change_address_raw_hex`.
- Extended the proof-heavy node integration test to spend a 7-unit note with a
  2-unit fee and 3-unit recipient amount, then rescan the 3-unit recipient note
  and 2-unit change note.

Current limits:

- Change can return to the spender default Orchard address or an explicit raw
  address/key/view-key file.
- This remains one-input transfer construction. Multi-input and arbitrary
  multi-output wallet planning remain future work.

Next:

- Add SDK-library packaging around keygen, scan, spend, apply, and tx-finality
  evidence.

### 2026-05-15: Orchard Configurable Change Recipient V0

Implemented:

- Added `--change-recipient-view-key-file`,
  `--change-recipient-key-file`, and
  `--change-recipient-address-raw-hex` to `orchard-spend-create`.
- The spend builder still defaults change to the spender address when no change
  recipient is provided.
- If a change recipient is provided but `--amount` does not leave change, the
  command fails closed instead of silently ignoring the option.
- The proof-heavy node integration test now sends change to a distinct Orchard
  view key and verifies the spender view key does not see the change note while
  the change view key does.
- `scripts/testnet-orchard-wallet-finality-smoke` now uses a distinct change
  view key and records separate recipient/change scan evidence.

Evidence:

- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 261.59s.
- `scripts/testnet-orchard-wallet-finality-smoke` passed with latest report:
  `reports/testnet-orchard-wallet-finality-smoke/20260515T062948Z/testnet-orchard-wallet-finality-smoke.json`.

### 2026-05-15: Orchard Peer-Certified Privacy Smoke V0

Implemented:

- Added `scripts/testnet-orchard-peer-certified-smoke`.
- The script builds `postfiat-node`, starts a local 4-validator harness, splits
  validator keys, starts signed-proposal validator vote services, and runs four
  shielded peer-certified rounds: debug shielded mint, migration into
  `orchard-v1`, Orchard/Halo2 output action, and Orchard/Halo2 spend action.
- The smoke verifies each peer-certified round has signed proposal evidence,
  4-of-4 votes, verified vote requests, verified certified sends, local apply,
  and the expected block height.
- After the rounds it verifies all validators converged at height 4 with one
  state root and one block tip, runs `verify-state` and `verify-shielded` on
  every validator, scans recipient and change notes, and records `tx` finality
  for mint/migrate/output/spend receipts.
- Report output redacts raw Orchard action JSON to byte length plus SHA3-384
  hash and fails if private-key or private witness markers appear in the
  report/log evidence.

Evidence:

- `scripts/testnet-orchard-peer-certified-smoke` passed.
- Latest report:
  `reports/testnet-orchard-peer-certified-smoke/20260515T065718Z/testnet-orchard-peer-certified-smoke.json`.

Current limits:

- This is local 4-validator peer-certified privacy-alpha evidence, not yet a
  live privacy-alpha network gate.
- Withdraw accounting, deposit/outer-envelope fee policy, broader malformed
  envelope coverage, restart recovery, and operator verifier limits remain.

### 2026-05-15: Orchard External Envelope Binding V0

Implemented:

- Added optional `external_binding_hash` to `OrchardShieldedAction`.
- The field is a 48-byte lowercase hex hash and is skipped in JSON when absent,
  so existing Orchard action JSON remains backward compatible.
- The Orchard authorization sighash now includes the external binding hash
  value, or an explicit empty value when absent.
- Added public helpers for creating/verifying actions with an external binding:
  `orchard_authorizing_sighash_with_external_binding` and
  `orchard_action_from_authorized_bundle_with_external_binding`.
- Node fee-weight accounting includes the binding hash bytes.

Evidence:

- `cargo test -p postfiat-privacy-orchard` passed.
- `cargo check --workspace` passed.
- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 263.14s.

Security purpose:

- This is the enabling primitive for withdrawals and disclosure envelopes. A
  future withdraw action must bind transparent recipient, amount, fee, policy
  id, and disclosure hash into this field before state transition credits any
  transparent account.
- Mutation tests prove changing or removing the binding hash after signing
  fails with `binding_signature_invalid`.

### 2026-05-15: Orchard Withdraw RPC SDK Path V0

Implemented:

- Added `shield_batch_orchard_withdraw` as a first-class RPC SDK method.
- `postfiat-rpc-sdk` now builds and validates request-file envelopes carrying
  `action_file`, transparent recipient, amount, fee, optional policy id,
  optional disclosure hash, and `batch_file`.
- SDK response validation now accepts and validates `orchard_withdraw_v1`
  shielded batch payloads, including bounded Orchard action JSON, transparent
  recipient, nonzero amount, fee, canonical policy id, and optional 48-byte
  disclosure hash.
- `postfiat-node rpc --request-file` now routes
  `shield_batch_orchard_withdraw` to the existing
  `create_orchard_withdraw_action_batch` path, so wallet/RPC flows no longer
  need the direct `shield-batch-orchard-withdraw` CLI command for valid
  withdraw batch creation.
- `scripts/testnet-orchard-wallet-finality-smoke` now creates output, spend,
  and withdraw batches through the SDK request-file path; it still keeps the
  direct CLI mismatch probe to prove external-binding tamper rejection.

Evidence:

- `cargo test -p postfiat-rpc-sdk` passed.
- `cargo check --workspace` passed.
- `python3 -m py_compile scripts/testnet-orchard-wallet-finality-smoke` passed.
- `scripts/testnet-orchard-wallet-finality-smoke` passed with SDK withdraw
  evidence:
  `reports/testnet-orchard-wallet-finality-smoke/sdk-withdraw-v0-20260515T082626Z/testnet-orchard-wallet-finality-smoke.json`.

Current limits:

- This is still a local request-file RPC path, not a public live privacy-alpha
  write endpoint.
- Scan/spend helpers are still CLI-oriented; broader SDK-library ergonomics,
  disclosure packets, verifier/operator limits, and privacy-alpha live evidence
  remain.

### 2026-05-15: Orchard Disclosure Packet V0

Implemented:

- Added `postfiat-node orchard-disclose` as a local wallet/auditor artifact
  path.
- Added `postfiat-orchard-disclosure-packet-v1`, generated from a locally
  decrypted Orchard output.
- The packet includes chain id, genesis hash, protocol version, pool id,
  address, output index, Merkle position, note commitment, nullifier, value,
  spent flag, memo, retained-root metadata, auditor instructions, and a
  domain-separated `disclosure_hash`.
- For outputs created by ordered shielded batches, the packet includes batch
  kind/id, batch payload hash, block height/hash, state root, certificate id,
  and receipt ids by matching the output commitment against the archived
  shielded batch payload.
- The packet intentionally omits spend keys, viewing keys, note `rseed`, and
  Merkle auth paths; the focused test asserts those JSON field names are absent
  from the artifact.
- Added `postfiat-node orchard-disclosure-verify` to validate packet
  schema/hash, local chain/genesis context, archive commitment inclusion, and
  ordered-batch block/finality fields when present.

Evidence:

- `cargo test -p postfiat-node orchard_action_gate_verifies_applies_and_rejects_duplicate_nullifiers -- --nocapture`
  passed in 335.19s.
- `cargo check --workspace` passed.
- `scripts/testnet-orchard-wallet-finality-smoke` passed with change-note
  disclosure packet generation, local verification, and snapshot-import
  verification:
  `reports/testnet-orchard-wallet-finality-smoke/perf-malformed-v0-20260515T103617Z/testnet-orchard-wallet-finality-smoke.json`.
  The report also records the first local performance baseline for the real
  Orchard output/spend/withdraw path: 7,264-byte proof blobs, roughly 19 KB
  action files, about 39-40s action construction per shielded action on this
  host, about 11.5-12.0s ordered apply/verify per shielded batch, sub-20ms
  disclosure verification, fail-closed rejection of a 1,048,577-byte oversized
  proof with `oversized_hex` in about 63ms, fail-closed rejection of a
  4,097-byte encrypted-output ciphertext with `oversized_hex` in about 3ms,
  exact-size malformed proof rejection with `proof_verification_failed` in
  about 11.7s, and exact-size malformed ciphertext rejection with
  `binding_signature_invalid` in about 11.4s.

### 2026-05-15: Orchard Operator Policy Report V0

- Added `postfiat-node orchard-operator-policy`.
- The command emits `postfiat-orchard-operator-policy-v1` with:
  - chain id, genesis hash, and protocol version;
  - explicit privacy-enabled posture;
  - configured verifier concurrency and timeout;
  - configured root-retention window and indexing role;
  - protocol caps for Orchard action JSON, max actions per bundle, proof bytes,
    ciphertext blob bytes, and Orchard ciphertext component sizes;
  - enforcement flags for protocol size bounds, action-count bounds, in-process
    verifier status, timeout/concurrency enforcement, public write admission,
    and required worker isolation.
- Latest local evidence:
  `reports/testnet-orchard-operator-policy/operator-policy-v1-20260515T110835Z/orchard-operator-policy.json`.
- Current limitation is explicit in the report: the verifier still runs
  in-process; timeout and concurrency are not enforced in-process; public
  privacy write edges are disallowed until verifier worker/process isolation is
  implemented.

### 2026-05-15: Child-Isolated Remote Orchard Batch Creation V0

- Added SDK request builders and validators for `shield_batch_orchard` and
  `shield_batch_orchard_withdraw` using bounded `action_json` instead of
  server-local file paths.
- Added `rpc-serve --allow-orchard-batch-create` with separate per-peer and
  total rate limits:
  `--max-orchard-batch-create-per-peer` and
  `--max-orchard-batch-create-total`.
- Remote Orchard batch creation now:
  - requires `action_json`;
  - rejects remote `action_file` and `batch_file` paths as not public-safe;
  - writes action/batch files only under server-controlled spool directories;
  - runs through the existing `rpc-serve` child-process timeout boundary;
  - does not expose `apply_shield_batch`.
- Latest TCP RPC evidence:
  `reports/testnet-orchard-rpc-batch-create/orchard-rpc-batch-create-v0-20260515T111516Z/testnet-orchard-rpc-batch-create.json`
  from `scripts/testnet-orchard-rpc-batch-create-smoke`.
- Latest operator policy evidence:
  `reports/testnet-orchard-operator-policy/operator-policy-v1-20260515T110835Z/orchard-operator-policy.json`.

### 2026-05-15: Remote Orchard Malformed Edge-Load V0

- Added `scripts/testnet-orchard-rpc-malformed-edge-load-smoke`.
- The smoke drives concurrent exact-size malformed Orchard proof actions
  through `rpc-serve --allow-orchard-batch-create`, records per-request
  timings, server counters, event logs, and sampled parent+child RSS, then
  sends a normal `status` request after the malformed load.
- Latest evidence:
  `reports/testnet-orchard-rpc-malformed-edge-load/orchard-rpc-malformed-edge-load-v0-20260515T113620Z/testnet-orchard-rpc-malformed-edge-load.json`.
- Current local result: three concurrent malformed proof requests failed closed
  with `proof_verification_failed` surfaced as `rpc_error`, no child timeout
  fired, the post-load `status` request succeeded, malformed request latency was
  about 13.5-13.8s, and sampled parent+child RSS peaked at about 78.5 MB.

### 2026-05-15: Remote Orchard Rate-Limit Evidence V0

- Added `scripts/testnet-orchard-rpc-rate-limit-smoke`.
- The smoke starts `rpc-serve --allow-orchard-batch-create` twice and proves:
  - per-peer cap rejection with `rpc_orchard_batch_create_rate_limited`;
  - global cap rejection with `rpc_orchard_batch_create_global_rate_limited`;
  - no child timeout in either phase.
- Latest evidence:
  `reports/testnet-orchard-rpc-rate-limit/orchard-rpc-rate-limit-v0-20260515T114343Z/testnet-orchard-rpc-rate-limit.json`.

### 2026-05-15: Remote Orchard RPC Threshold Gate V0

- Added `scripts/testnet-orchard-rpc-threshold-gate`.
- The gate consumes the latest malformed-edge and rate-limit reports and fails
  unless:
  - at least three malformed proof requests were exercised;
  - max malformed request latency is `30000ms` or lower;
  - sampled parent+child RSS is `524288KB` or lower;
  - child timeout count is zero;
  - post-load `status` succeeds;
  - per-peer and global rate-limit errors are observed.
- Latest evidence:
  `reports/testnet-orchard-rpc-threshold-gate/orchard-rpc-threshold-gate-v0-20260515T115102Z/testnet-orchard-rpc-threshold-gate.json`.

Current limits:

- Direct local apply can produce a disclosure packet, but it has no finality
  object because there is no ordered block record.
- There is not yet an approved auditor registry, regulated disclosure policy,
  custodian workflow, or SDK disclosure helper.
