# Privacy Production Burndown

Status: canonical privacy execution list  
Date: 2026-05-15  
Scope: PostFiat L1 confidential settlement

This document defines what must exist before PostFiat can credibly describe
privacy as semi-production or production-grade. It is deliberately separate
from the controlled transparent testnet burndown because privacy is not required
for the current transparent PQ settlement testnet to be live-green.

## Current State

PostFiat already has useful privacy-shaped protocol scaffolding:

- Shielded note, nullifier, tree-root, scan, disclose, and turnstile semantics
  exist in `crates/privacy/src/lib.rs`.
- Shielded mint, spend, migrate, apply, verify, scan, disclose, and turnstile
  node flows exist in `crates/node/src/lib.rs`.
- Shielded state is included in storage, state verification, snapshots,
  observability, metrics, archive validation, and RPC/SDK response validation.
- Shielded batches can be ordered by the same certified batch path as
  transparent/governance/bridge batches; `scripts/testnet-orchard-peer-certified-smoke`
  now proves the Orchard/Halo2 path through a local 4-validator peer-certified
  shielded mint/migrate/direct-deposit/output/spend/withdraw flow.
- The code has an explicit proof adapter boundary in `crates/proofs/src/lib.rs`.
- A real Orchard/Halo2 adapter exists in `crates/privacy_orchard`; it owns the
  PostFiat serialized action shape, reconstructs upstream Orchard bundles,
  verifies real Halo2 proofs and signatures, and binds authorization to chain
  id, genesis hash, protocol version, pool id, fee, and an optional external
  envelope hash.
- Node can now verify a serialized Orchard action and, with explicit local
  apply, persist Orchard nullifiers, output commitments, encrypted outputs, and
  accepted anchors in a separate production pool state.
- Orchard pool state now retains verified roots, accounts deposit-side migrated
  value, and `orchard-scan` returns receive-only decrypted notes with retained
  anchor and Merkle auth-path material needed by the spend builder.
- `orchard-spend-create` can build a real Orchard spend of one decrypted note
  into either one full-note recipient output worth `input_value - fee` or a
  recipient output plus spender-default change output when `--amount N` is set;
  node apply/rescan proves the original note is nullified, replacement/change
  notes remain spendable, the signed fee is burned, and underpriced positive
  value-balance fee burns fail closed.
- `orchard-withdraw-create` can now build a one-note Orchard/Halo2 withdraw
  action whose proof authorization binds an external transparent envelope hash.
  `shield-batch-orchard-withdraw` wraps the signed action with recipient,
  amount, fee, policy id, and disclosure hash; ordered shielded batch apply
  verifies the binding, rejects tampered envelopes, updates Orchard nullifier /
  root / value accounting, burns the fee, and credits the transparent ledger in
  the same committed block.
- `orchard-deposit-create` now builds a real direct transparent-to-Orchard
  deposit envelope. The envelope includes a signed ML-DSA transparent funding
  transfer to the protocol burn sink, an Orchard/Halo2 output action whose
  authorization binds that funding transfer id, amount, fee, policy id, and
  disclosure hash, and a deterministic `orchard_deposit` turnstile event.
  `shield-batch-orchard-deposit` wraps it as `orchard_deposit_v1`; ordered
  shielded batch apply verifies the funding signature/sequence, burns the
  transparent principal plus deposit resource fee, mints the Orchard note,
  updates pool/root/value accounting, and leaves the recipient note spendable by
  `orchard-scan`.
- `orchard-disclose` can now produce a local redacted
  `postfiat-orchard-disclosure-packet-v1` for a decrypted Orchard output. The
  packet includes chain/genesis/protocol context, note commitment/nullifier,
  value, memo, retained-root metadata, auditor instructions, and block/batch
  finality evidence when the output came from an ordered shielded batch. It
  deliberately omits spending keys, viewing keys, note `rseed`, and Merkle auth
  paths. `orchard-disclosure-verify` validates packet schema/hash, local
  chain/genesis context, and ordered-batch archive/block finality when present;
  tampered packet content fails closed.
- RPC/SDK validation now treats Orchard raw private witness/key fields such as
  `spending_key_hex`, `full_viewing_key_hex`, `master_seed_hex`, and `rseed` as
  key material, so public-shaped request/response validation fails closed if
  those fields appear.
- `postfiat-node orchard-pool-report` and read-only RPC method
  `orchard_pool_report` now expose public Orchard pool telemetry for privacy
  alpha: output count, nullifier count, retained roots, latest root, turnstile
  deposit volumes, fee burns, withdraw volume, and conservative active-note
  bounds. The report deliberately does not expose key material, wallet witness
  material, encrypted note payload bodies, or exact active-user/account counts.
- `rpc-serve --allow-orchard-batch-create` now accepts bounded inline Orchard
  `action_json` and direct-deposit `deposit_json`, writes batches only to
  server-controlled spool paths, rejects client-selected file paths and direct
  shielded apply at the public edge, and fails malformed direct-deposit
  envelopes closed on the external binding check.
- The local 4-validator peer-certified Orchard gate now proves both one-node
  restart/outage recovery and below-quorum no-advance: a direct-deposit round
  with only two total votes against quorum three fails before any validator
  advances, then the same shielded flow completes after quorum is restored.
- The live five-validator Orchard/Halo2 gate now proves a direct
  transparent-to-Orchard deposit, Orchard spend, and Orchard withdraw across
  the controlled network while injecting a one-validator outage during the
  shielded deposit round. The latest run finalized deposit/spend/withdraw at
  heights 30/31/32, kept the offline validator from advancing while stopped,
  replayed the certified shielded batch to recover it, and reconverged all five
  validators:
  `reports/testnet-live-orchard-full-flow/live-orchard-full-flow-20260515T183724Z/testnet-live-orchard-full-flow.json`.
The latest audit packet includes this live outage evidence and still passes
redaction:
`reports/testnet-orchard-privacy-audit-packet/orchard-privacy-audit-packet-20260515T185212Z/orchard-privacy-audit-packet.json`.

Fresh 2026-05-16 live privacy-alpha evidence also exists:
`reports/testnet-live-orchard-direct-deposit/overnight-20260516T0644/testnet-live-orchard-direct-deposit.json`.
That run certified a direct transparent-to-Orchard deposit at height `39`,
confirmed `tx` finality, scanned one decrypted unspent Orchard output, kept the
public write edge closed, and records both remote and local private material
cleanup after the run.

The current privacy implementation is not production privacy:

- Legacy debug-pool proofs use `DebugProofSystem`; this is an adapter
  boundary, not a ZK proof. The Orchard path verifies real Halo2 proofs but is
  not yet a complete public wallet/custodian flow.
- The production Orchard/Halo2 path is present as a gated node path and has
  local ordered plus local 4-validator peer-certified evidence, but is not yet
  the live default public wallet/write path.
- Debug notes are not encrypted for recipients and remain a separate state
  model.
- Debug spend authorization is note-id based, not witness/proof based.
- The current state stores note metadata in a transparent/debug shape.
- The current Orchard proof path reuses Zcash Orchard/Halo2 and must not be
  described as end-to-end post-quantum private value.
- There is no proving/verifying key lifecycle, no externally operated live
  privacy-alpha deposit / withdraw path, and no full proof-cost fee model yet.
- Wallet scanning and local disclosure exist for the Orchard receive path, but
  there is no exchange/custodian flow, hardware signer story, multi-account
  SDK, or regulated disclosure policy yet.

## Target Definitions

### Semi-Production Privacy

Semi-production privacy means the controlled testnet can execute shielded value
with real cryptographic privacy under clearly stated limits:

- real zero-knowledge proof verification in node execution;
- encrypted note payloads for recipients;
- nullifier-based double-spend prevention;
- amount-conservation proof for a constrained transaction shape;
- transparent/shielded turnstile accounting;
- wallet generation, scanning, spending, and selective disclosure;
- benchmarks and operator limits for proving time, proof size, memory, and fees;
- redaction-safe evidence that proves the flow without leaking private material.

Acceptable constraints for this stage:

- one asset;
- one input and up to two outputs;
- no recursive proof aggregation;
- seconds-to-tens-of-seconds proving latency;
- large proof receipts if bounded and priced;
- controlled validator set;
- explicit "privacy alpha" claim boundary.

### Production-Grade Privacy

Production-grade privacy means the protocol can survive external cryptographic,
implementation, wallet, operational, and regulatory review:

- audited circuits/guest programs and host verifier;
- stable proof statement and note-envelope formats;
- safe upgrade/migration path;
- chain reorg/finality, replay, and duplicate-nullifier behavior tested;
- sustained live-network evidence under realistic proof sizes and wallet load;
- independent implementation review and at least one external audit;
- clear regulated disclosure, travel-rule, sanctions-screening, and abuse
  response posture;
- public claims bounded to what anonymity set, disclosure model, and threat
  model actually support.

## Critical Path

The fastest credible path is to ship Orchard/Halo2 privacy first, with explicit
claim boundaries, rather than design a new proving system first. The critical
path is:

1. freeze the Orchard-backed shielded transaction model;
2. keep verified Orchard actions in ordered certified shielded batches and
   extend them beyond zero-balance actions;
3. implement root-history, note-tree, and replay rules for the production
   Orchard pool;
4. implement wallet scanning/decryption around Orchard transmitted notes;
5. add transparent/shielded turnstile accounting and fees;
6. price and benchmark proof bytes, ciphertext bytes, verifier time, and
   storage growth;
7. package evidence, claim boundaries, and an external review packet.

## Burndown

| ID | Item | Stage | Status | Exit Criteria |
| --- | --- | --- | --- | --- |
| PRIV-000 | Freeze privacy v0 claim boundary | Semi-production | Open | Document says "shielded privacy alpha" only; excludes Zcash-equivalent, audited, production anonymity, and unrestricted regulated use. Public claims checklist references this boundary. |
| PRIV-001 | Freeze shielded transaction shape | Semi-production | Partial | `OrchardShieldedAction` fixes the serialized public action shape for bounded Orchard actions. `orchard_deposit_v1` now supplies the direct transparent-to-Orchard funding envelope, and `orchard_withdraw_v1` supplies the shielded-to-transparent exit envelope. Remaining: multi-input/multi-output transaction planning and final policy wrapper. |
| PRIV-002 | Canonical public journal structs | Semi-production | Partial | Orchard action fields and authorizing domain are versioned, chain-bound, genesis-bound, protocol-version-bound, fee-bound, deterministic, and round-trip tested; mutating `action.fee` after signing fails verification. `OrchardShieldedAction` now carries an optional 48-byte `external_binding_hash` inside the authorization signature domain, and mutation/removal fails verification. Direct deposit and withdraw envelope hashes bind policy/disclosure metadata into that signature domain. Remaining: final multi-action transaction wrapper and richer regulated-disclosure policy fields. |
| PRIV-003 | Canonical private witness structs | Semi-production | Partial | Orchard scan/spend structs now carry note position, retained anchor, output count, 32-node auth path, and note `rseed` for local wallet spending. Remaining private witness structs must formalize owner/spend key handling, multi-input/output values, fee/change, and randomness boundaries without treating private scan artifacts as public reports. |
| PRIV-004 | Proof trait upgrade | Semi-production | Partial | `ProofSystem` supports backend id, receipt/proof bytes, public journal bytes, verifier mode, size bounds, and deterministic errors. Debug backend remains test-only. |
| PRIV-005 | Orchard key/prover lifecycle | Semi-production | Open | Proving/verifying key build, cache, versioning, operator startup behavior, memory cost, and upgrade posture are specified and benchmarked. |
| PRIV-006 | Orchard spend construction v0 | Semi-production | Partial | Wallet/prover can now build zero-value and direct-funded nonzero Orchard output actions, `orchard-deposit-create` builds a transparent-to-Orchard mint envelope, `orchard-spend-create` builds a real one-note private transfer from one scanned note, and `orchard-withdraw-create` builds a real one-note shielded-to-transparent withdraw action bound to an external transparent envelope. Remaining: multi-input/multi-output wallet planning, scan/spend SDK helpers, and full custodian policy. |
| PRIV-007 | Host verifier integration | Semi-production | Partial | Node rejects serialized Orchard actions unless the reconstructed Orchard/Halo2 proof and signatures verify against the exact PostFiat authorizing domain. The same verifier now runs through the ordered shielded batch path. Remaining: operator limits and broader malformed-proof cost evidence. |
| PRIV-008 | Mint and withdraw proof policy | Semi-production | Partial | Direct transparent-to-Orchard mint now exists through `orchard_deposit_v1`: the signed funding transfer authorizes the transparent debit, the deposit envelope binds funding tx id/from/amount/fee/policy/disclosure into the Orchard authorization domain, and accepted apply burns principal plus deposit resource fee before recording `orchard_deposit` turnstile budget. Legacy migrated debug-budget deposits still work but are no longer the only nonzero funding path. One-note spends burn signed fees from shielded value subject to a deterministic minimum fee. One-note withdraws bind recipient/amount/fee/policy/disclosure metadata into the Orchard authorization domain and credit the transparent ledger only inside ordered shielded batch apply. Latest local ordered evidence includes direct deposit, spend, withdraw, disclosure, snapshot import, and tx-finality: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-20260515T132406Z/testnet-orchard-wallet-finality-smoke.json`. Latest 4-validator peer-certified evidence includes direct deposit at height 3 and withdraw at height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-20260515T134027Z/testnet-orchard-peer-certified-smoke.json`. Fresh live direct-deposit evidence passed at height `39`: `reports/testnet-live-orchard-direct-deposit/overnight-20260516T0644/testnet-live-orchard-direct-deposit.json`. Remaining: richer disclosure policy and custodian deposit policy. |
| PRIV-009 | Note commitment format | Semi-production | Partial | Replace debug commitment format with versioned production commitment inputs and domain-separated hash/commitment function. Existing debug commitments are not accepted under production privacy mode. |
| PRIV-010 | Nullifier format | Semi-production | Partial | Orchard nullifiers are parsed, verified through the proof adapter, persisted in production pool state, replay-rejected in a node integration test, and exercised through local ordered plus 4-validator peer-certified spend/withdraw evidence. Remaining: broader malformed-envelope/replay tests. |
| PRIV-011 | Merkle tree implementation | Semi-production | Partial | Orchard pool state recomputes Orchard note-commitment roots from persisted output commitments using the upstream Orchard tree hash, rejects corrupted retained roots during state verification, derives per-note inclusion auth paths for scan output, and the spend builder consumes those paths. Remaining: retention-window config, archive/export behavior, deterministic vectors, and stale-root policy tests. |
| PRIV-012 | Root history and finality window | Semi-production | Partial | Zero-balance Orchard actions must reference a retained root; empty-pool actions use the canonical empty Orchard root, accepted actions append a new root record, duplicate roots are suppressed, unretained anchors fail closed, and scan reports identify the latest retained root/output count used for witness material. Remaining: configurable retention/finality window, stale-root tests across multiple spends, and indexer/archive policy. |
| PRIV-013 | Encrypted note envelope spec | Semi-production | Open | Versioned envelope includes recipient KEM public key id, ML-KEM encapsulation, AEAD ciphertext, nonce, commitment binding, chain id, genesis hash, and protocol version. |
| PRIV-014 | ML-KEM KEM/DEM implementation | Semi-production | Open | Sender encapsulates to recipient viewing/encryption key, derives AEAD keys with domain-separated HKDF, encrypts note payload, and recipient decrypts in wallet tests. |
| PRIV-015 | Viewing/scanning key model | Semi-production | Partial | Orchard adapter derives default Orchard addresses from spending keys and full viewing keys, node `orchard-keygen` writes deterministic private Orchard key files, `orchard-view-key-export` writes private receive-only `postfiat-orchard-view-key-v1` files, and `orchard-scan` trial-decrypts persisted encrypted outputs using upstream Orchard note decryption without requiring spend authority. Scan output now includes retained-root witness material and private note seed material for local spends. Remaining: scan-secret handling, multiple accounts/scopes, SDK flow, and validator/RPC separation. |
| PRIV-016 | Selective disclosure packet | Semi-production | Partial | Local `orchard-disclose` writes a redacted `postfiat-orchard-disclosure-packet-v1` from a decrypted Orchard output. Ordered-batch outputs include block height/hash, state root, certificate id, receipt ids, batch id, and batch payload hash; direct local apply packets are allowed but have no finality object. Packets include note commitment/nullifier/value/memo and retained-root metadata while omitting spend/view keys, `rseed`, and auth paths. Local `orchard-disclosure-verify` validates packet schema/hash, local chain/genesis context, archive commitment inclusion, and block/finality fields; tampered packet content fails closed. Remaining: policy/auditor registry, spend/withdraw-level disclosure semantics, custodian workflow, and SDK helper. |
| PRIV-017 | Shielded wallet CLI/SDK flow | Semi-production | Partial | CLI has `orchard-keygen`, `orchard-view-key-export`, `orchard-scan`, `orchard-disclose`, `orchard-disclosure-verify`, `orchard-deposit-create --amount N`, `shield-batch-orchard-deposit`, `orchard-output-create --value N`, `orchard-spend-create --amount N --fee N`, `orchard-withdraw-create --to ADDRESS --amount N --fee N`, and `shield-batch-orchard-withdraw` for local ordered privacy flows. `postfiat-rpc-sdk` now builds/validates `shield_batch_orchard`, `shield_batch_orchard_deposit`, and `shield_batch_orchard_withdraw` request/response envelopes, including CLI request generation for local `--deposit-file` and bounded inline `--deposit-json` / `--deposit-json-file`. `postfiat-node rpc --request-file` can create these ordered batch types. Focused direct-deposit check passed: `cargo test -p postfiat-node orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note -- --nocapture`. Full local wallet-finality evidence now spends from a direct transparent-funded Orchard note: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-20260515T132406Z/testnet-orchard-wallet-finality-smoke.json`. Remaining: multiple-account SDK ergonomics, scan/spend/disclose SDK helpers, and broader SDK-library packaging. |
| PRIV-018 | RPC surface hardening | Semi-production | Partial | RPC exposes shielded root, encrypted outputs, nullifier status, turnstile summaries, finality proof lookup, local request-file Orchard batch creation, local request-file Orchard deposit batch creation, local request-file Orchard withdraw batch creation, and opt-in remote Orchard batch creation from bounded inline JSON only. Remote Orchard action/deposit batch creation runs through `rpc-serve` child-process timeout, per-peer/global rate limits, a dedicated `--max-orchard-batch-create-concurrent` verifier-worker cap, and server-controlled spool paths; remote file-path batch creation is rejected as not public-safe. Latest remote action/deposit batch-create evidence from `scripts/testnet-orchard-rpc-batch-create-smoke`: `reports/testnet-orchard-rpc-batch-create/orchard-rpc-deposit-batch-create-20260515T151620Z/testnet-orchard-rpc-batch-create.json`. That gate sends valid `action_json`, valid direct-deposit `deposit_json`, malformed direct-deposit `deposit_json`, rejected client file-path creation, and rejected direct `apply_shield_batch` through the public RPC edge; it passed with a real deposit batch id, malformed deposit rejection on `external binding`, `rpc_orchard_batch_create_not_public_safe`, `rpc_method_not_allowed`, `orchard_batch_create_count=4`, and zero child timeouts. New concurrency-limit evidence from `scripts/testnet-orchard-rpc-concurrency-limit-smoke`: `reports/testnet-orchard-rpc-concurrency-limit/orchard-rpc-concurrency-limit-v0-20260515T171128Z/testnet-orchard-rpc-concurrency-limit.json`; it proves one active malformed verifier occupies the single slot, a second simultaneous Orchard batch-create fails before verifier work with `rpc_orchard_batch_create_concurrency_limited`, and a `status` read still succeeds. RPC child isolation now clears inherited environment, closes child stdin, pipes stdout/stderr, runs one process per request, canonicalizes the child data-dir cwd, and keeps timeout enforcement visible in `rpc-serve` reports. Latest local isolation evidence: `reports/testnet-orchard-rpc-child-isolation/orchard-rpc-child-isolation-v0-20260515T174034Z/testnet-orchard-rpc-child-isolation.json`. Direct shielded apply is rejected at the remote RPC edge. Request/response size limits are enforced; Orchard action/deposit JSON is validated as bounded JSON and redacted to size/hash in evidence. SDK/node RPC validation rejects raw Orchard private key/witness fields (`master_seed_hex`, `spending_key_hex`, `full_viewing_key_hex`, `rseed`) in public-shaped surfaces while allowing the public `public_key_hex` required by signed transparent funding transfers in deposit envelopes. Local malformed direct-deposit amount evidence now fails closed on `external binding` before the valid deposit path continues: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-malformed-20260515T144715Z/testnet-orchard-wallet-finality-smoke.json`. Remaining: live privacy-alpha edge policy and larger repeated malformed-load evidence on target hardware. |
| PRIV-019 | Fee and resource pricing | Semi-production | Partial | One-note Orchard deposits, spends, and withdraws now burn deterministic fees and receipts report `fee_charged`/`fee_burned`/`minimum_fee`; deposits charge the transparent funding transfer fee plus an Orchard resource fee before minting shielded value, spends burn signed fees from shielded value, and withdraws include transparent account-creation state expansion fees when needed. `postfiat-node orchard-fee-resource-policy` now publishes `postfiat-orchard-fee-resource-policy-v1` with the transparent fee schedule, Orchard resource-fee formula, direct-deposit/spend/withdraw fee components, protocol resource bounds, and anti-spam policy. `scripts/testnet-orchard-fee-resource-policy` passed using deterministic vectors and repeated live malformed-load evidence: `reports/testnet-orchard-fee-resource-policy/orchard-fee-resource-policy-20260515T171204Z/testnet-orchard-fee-resource-policy.json` (`minimum_orchard_resource_fee=2`, byte quantum `1048576`, max proof bytes `1048576`, public write edge closed, remote batch-create rate-limited and concurrency-limited, live malformed `p99=1614ms` below verifier timeout `30000ms`). Remaining: longer target-hardware soaks, final public RPC worker-isolation policy, and any production repricing after larger proof/request distributions. |
| PRIV-020 | Consensus apply path | Semi-production | Partial | Shielded batch apply can now carry `orchard_action_v1`, `orchard_deposit_v1`, and `orchard_withdraw_v1` actions and verify/apply production Orchard pool public state through the ordered commit path, including retained-anchor checks, wallet-created outputs, direct transparent-funded deposits, wallet-created spends, ordered withdraw ledger crediting, root-history updates, signed fee burns with minimum-fee enforcement, and migrated-budget compatibility. `cargo test -p postfiat-node orchard_deposit_batch_locks_transparent_value_and_mints_spendable_note -- --nocapture` proves the direct deposit path locally. `scripts/testnet-orchard-wallet-finality-smoke` now proves the full local ordered deposit/spend/withdraw/disclosure/snapshot/finality loop with direct deposit finality at height 3 and withdraw finality at height 6. Latest local malformed direct-deposit run mutates deposit amount `11 -> 12`, rejects the mutated file on `external binding`, then completes the valid direct deposit/spend/withdraw/finality path: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-malformed-20260515T144715Z/testnet-orchard-wallet-finality-smoke.json`. `scripts/testnet-orchard-peer-certified-smoke` now proves mint, migration, direct transparent-funded deposit, migrated-budget Orchard output, one-note spend from the direct deposit, and one-note withdraw across four local validators with peer-certified shielded batches, converged state at height 6, `orchard_deposit_total=11`, `orchard_turnstile_deposit_total=18`, and `orchard_withdraw_total=1`. It also supports `RESTART_SERVICES_EACH_ROUND=1`; latest restart evidence passed with 18 restarted validator service processes, direct deposit finality at height 3, spend finality at height 5, withdraw finality at height 6, `service_restart_verified=true`, and converged final height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-restart-20260515T140330Z/testnet-orchard-peer-certified-smoke.json`. Direct-deposit partial-outage recovery evidence also passed: `validator-2` was offline for direct-deposit round 3, the remaining validators formed a 3/4 shielded certificate, certified replay recovered the offline validator at height 3, and spend/withdraw continued to final height 6 with `partial_outage_verified=true`: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-partial-outage-20260515T142639Z/testnet-orchard-peer-certified-smoke.json`. Below-quorum no-advance evidence now exists for the direct-deposit round: expected votes `2` were below quorum `3`, all validators stayed at height 2 after the failed attempt, then the same flow completed to height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-below-quorum-20260515T151800Z/testnet-orchard-peer-certified-smoke.json`. Fresh live direct-deposit evidence passed at height `39` with state verification, tx finality, one decrypted scan output, public write edge closed, and local/remote private material cleanup: `reports/testnet-live-orchard-direct-deposit/overnight-20260516T0644/testnet-live-orchard-direct-deposit.json`. Remaining: longer mixed live privacy soaks with outage/archive folded into the same run. |
| PRIV-021 | State migration from debug mode | Semi-production | Open | Production privacy mode either starts with an empty production pool or has an explicit one-way migration with visible claim boundaries and no debug-note equivalence claims. |
| PRIV-022 | Local deterministic test vectors | Semi-production | Partial | `postfiat-node orchard-test-vector` now emits a public deterministic Orchard/Halo2 fixture using fixed test RNG seeds while production action creation still uses `OsRng`. `scripts/testnet-orchard-deterministic-vectors-smoke` pins the generated action hash, proof size, action/nullifier/output counts, value balance, encrypted-output byte lengths, root-after-outputs, and proof/fee tamper failure codes. Latest evidence: `reports/testnet-orchard-deterministic-vectors/orchard-deterministic-vectors-20260515T165200Z/testnet-orchard-deterministic-vectors.json` with action hash `a6245440e8a3c85b33a161fe75d5cee095c6f8c8418884c5dc266d1d17569cab6356898979262af2d37fed09d94e9ca9`, `proof_bytes=7264`, `action_count=2`, `nullifier_count=2`, `output_count=2`, `value_balance=-7`, proof tamper `proof_verification_failed`, fee tamper `binding_signature_invalid`, and private material redacted. Remaining: receipt-id vectors, full direct-deposit/spend/withdraw deterministic vectors, stale-root vectors, and committed golden-vector artifact policy. |
| PRIV-023 | Property tests | Semi-production | Partial | Node integration covers real Orchard proof apply, duplicate-nullifier replay rejection, unretained-anchor rejection, root-history persistence, ordered shielded batch execution, scan/decrypt, scan witness material, redacted disclosure packet generation, ordered disclosure finality evidence, disclosure verifier success/failure, direct transparent-to-Orchard deposit with signed funding debit and spendable scan result, underpriced fee-burn rejection, a real spend/rescan of a migrated-value output with fee burn, and a real withdraw that rejects a tampered transparent envelope before crediting the ledger. Remaining: value conservation beyond one-note transfer, multi-root stale-root behavior, broader malformed envelope rejection, and deterministic serialization properties. |
| PRIV-024 | Fuzz targets | Semi-production | Partial | `postfiat-fuzz orchard-parser` covers Orchard action JSON parser and canonical field rejection. Remaining: encrypted note envelope parser, RPC shielded requests, archive payloads, and disclosure packet parser. |
| PRIV-025 | Local privacy readiness gate | Semi-production | Partial | `scripts/testnet-orchard-wallet-finality-smoke` runs local ordered shielded mint, migration into `orchard-v1`, direct transparent-to-Orchard deposit via `orchard-deposit-create`, SDK-built `shield_batch_orchard_deposit`, SDK-built Orchard output/spend/withdraw batch requests, wallet scan, one-note spend from the direct-deposit note with explicit change view key, change-note disclosure packet generation/verification, snapshot-import disclosure verification, post-spend scan for recipient and change notes, one-note withdraw to a transparent account, withdraw envelope mismatch rejection (`external binding`), malformed direct-deposit amount rejection (`external binding`), shielded verification, `tx` finality lookup for mint/migrate/direct-deposit/output/spend/withdraw receipts, semantic spend-batch replay rejection (`already applied`), semantic batch-id tamper rejection (`batch id mismatch`), oversized and exact-size malformed Orchard proof/ciphertext rejection, snapshot export/import into a fresh data dir, imported shielded-state verification, imported change-note scanning, and local prover/verifier timing plus action/proof byte metrics. Latest local direct-deposit malformed/readiness evidence: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-malformed-20260515T144715Z/testnet-orchard-wallet-finality-smoke.json` with mutated deposit amount `11 -> 12`, `matched_expected_stderr=true`, `orchard_deposit_total=11`, `orchard_turnstile_deposit_total=18`, direct deposit finality at height 3, spend finality at height 5, and withdraw finality at height 6. `scripts/testnet-orchard-peer-certified-smoke` now runs a 4-validator peer-certified shielded mint/migrate/direct-deposit/output/spend/withdraw flow, verifies every validator's state and shielded state, scans recipient/change notes, records tx-finality for heights 1-6, and records `orchard_deposit_total=11`, `orchard_turnstile_deposit_total=18`, and `orchard_withdraw_total=1`. Latest peer-certified evidence: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-20260515T134027Z/testnet-orchard-peer-certified-smoke.json`. Restart evidence now exists for the same flow with `RESTART_SERVICES_EACH_ROUND=1`, 18 restarted validator services, `service_restart_verified=true`, and final height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-restart-20260515T140330Z/testnet-orchard-peer-certified-smoke.json`. Partial-outage recovery evidence now exists for the direct-deposit round with `partial_outage_verified=true`, certified replay recovery, and final height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-partial-outage-20260515T142639Z/testnet-orchard-peer-certified-smoke.json`. Below-quorum no-advance evidence now exists for the same direct-deposit round with `below_quorum_verified=true` and final recovery to height 6: `reports/testnet-orchard-peer-certified-smoke/peer-direct-deposit-below-quorum-20260515T151800Z/testnet-orchard-peer-certified-smoke.json`. Remote direct-deposit RPC edge evidence now exists: `reports/testnet-orchard-rpc-batch-create/orchard-rpc-deposit-batch-create-20260515T151620Z/testnet-orchard-rpc-batch-create.json`. Remaining: live privacy-alpha network mode. |
| PRIV-026 | Live privacy alpha gate | Semi-production | Partial | Local peer-certified privacy alpha now covers shielded mint/migrate/direct-deposit/output/spend/withdraw with real proofs, wallet scan, tx finality, validator convergence, redaction checks, service restart evidence, one-validator partial-outage recovery over the direct-deposit round, and below-quorum no-advance for the direct-deposit round. Live binary readiness now exists: the first command preflight proved the controlled hosts were still on binaries with no Orchard command surface (`reports/testnet-live-orchard-command-preflight/live-orchard-command-preflight-20260515T154409Z/testnet-live-orchard-command-preflight.json`); `scripts/testnet-live-orchard-binary-upgrade` then installed the current privacy-capable release binary hash `2b65d2ec4345a7ee036cffe342284aecc6a935595eb351ef6752d2eab47a2973` in place across three machines/five validator slots without wiping data dirs, kept every validator/RPC service active, and verified state at height 17 (`reports/testnet-live-orchard-binary-upgrade/live-orchard-binary-upgrade-20260515T154921Z/testnet-live-orchard-binary-upgrade.json`). Post-upgrade preflight passed with all five slots exposing the required Orchard commands and the same binary hash: `reports/testnet-live-orchard-command-preflight/live-orchard-command-preflight-20260515T155005Z/testnet-live-orchard-command-preflight.json`. Live direct-deposit transaction evidence exists: `scripts/testnet-live-orchard-direct-deposit-smoke` passed at height 19 with proposer `validator-4`, a 5-vote shielded certificate, state convergence, state verification, scan `decrypted_count=1`, and tx finality confirmed: `reports/testnet-live-orchard-direct-deposit/live-orchard-direct-deposit-20260515T155908Z/testnet-live-orchard-direct-deposit.json`. Fresh live direct-deposit refresh evidence passed at height `39` with scan `decrypted_count=1`, tx finality confirmed, public write edge closed, and local/remote private material cleanup checks true: `reports/testnet-live-orchard-direct-deposit/overnight-20260516T0644/testnet-live-orchard-direct-deposit.json`. Live full-flow evidence now exists: `scripts/testnet-live-orchard-full-flow-smoke` passed deposit at height 20, spend at height 21, and withdraw at height 22 across the controlled five-validator network with final convergence, state verification, and tx finality for all three shielded blocks: `reports/testnet-live-orchard-full-flow/live-orchard-full-flow-20260515T160513Z/testnet-live-orchard-full-flow.json`. Live privacy observability now exists: `scripts/testnet-live-orchard-observability` passed at height 22 with all validator services active, state/shielded verification converged, `orchard_output_count=10`, `orchard_deposit_total=33`, `orchard_withdraw_total=1`, `orchard_fee_burn_total=4`, and public write edge closed: `reports/testnet-live-orchard-observability/live-orchard-observability-20260515T161414Z/testnet-live-orchard-observability.json`. Live target-hardware malformed-load evidence now exists: `scripts/testnet-live-orchard-malformed-edge-load` generated a live-context Orchard action on `validator-0`, mutated the proof without changing its size, sent three concurrent `shield_batch_orchard` requests through a temporary loopback-only RPC edge, verified all malformed proofs failed closed on `proof_verification_failed`, recorded zero child timeouts, confirmed post-load `status`, and peaked at `48072KB` sampled RSS with max malformed latency `1484ms`: `reports/testnet-live-orchard-malformed-edge-load/live-orchard-malformed-edge-load-20260515T162850Z/testnet-live-orchard-malformed-edge-load.json`. Repeated target-hardware malformed-load evidence now spans all five validator slots with five passed samples, zero child timeouts, post-load status success each time, malformed latency `p95=1614ms`, and RSS `p99=49216KB`: `reports/testnet-live-orchard-malformed-edge-load-series/live-orchard-malformed-edge-load-series-20260515T170500Z/testnet-live-orchard-malformed-edge-load-series.json`. Live privacy soak evidence now exists: `scripts/testnet-live-orchard-privacy-soak` ran two additional consecutive live full-flow samples from height 22 to 28, with every deposit/spend/withdraw step finalized, state verified, public write edge closed, and pool counters moving from `output_count=10` / `nullifier_count=10` / `direct_deposit_total=33` / `withdraw_total=1` / `fee_burn_total=4` to `output_count=22` / `nullifier_count=22` / `direct_deposit_total=55` / `withdraw_total=3` / `fee_burn_total=12`: `reports/testnet-live-orchard-privacy-soak/live-orchard-privacy-soak-20260515T175026Z/testnet-live-orchard-privacy-soak.json`. Live Orchard snapshot evidence now exists: `scripts/testnet-live-orchard-snapshot-drill` exported/imported a snapshot on every controlled validator at height 28 and proved restored state verification, no validator/faucet private files in the manifest, and identical Orchard pool counters/root/bounds after import: `reports/testnet-live-orchard-snapshot-drill/live-orchard-snapshot-20260515T180932Z/testnet-live-orchard-snapshot-drill.json`. Live Orchard restart evidence now exists: `scripts/testnet-live-orchard-restart-drill` restarted every validator/RPC service pair at height 28 and proved services active, state unchanged, pool counters/root/bounds unchanged, CLI/RPC pool parity, and converged `output_count=22` / `nullifier_count=22`: `reports/testnet-live-orchard-restart-drill/live-orchard-restart-20260515T182006Z/testnet-live-orchard-restart-drill.json`. Remaining controlled live gate: longer mixed privacy soaks with outage/archive folded into the same live run, plus keeping public write edges gated. |
| PRIV-027 | Prover performance report | Semi-production | Partial | `scripts/testnet-orchard-wallet-finality-smoke` now records host platform/CPU count, per-step timings, action file sizes, proof bytes, ciphertext bytes, output counts, and disclosure packet bytes for the real direct-deposit/output/spend/withdraw path. Latest local run: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-20260515T132406Z/testnet-orchard-wallet-finality-smoke.json` (`proof_bytes=7264`; direct deposit file about 30 KB because it includes the signed transparent funding transfer; output/spend/withdraw action files about 19 KB; direct deposit action construction about 78.5s; output/spend/withdraw construction about 39.6-41.1s each; ordered apply about 11.7-11.9s each; disclosure verify below 20ms on this host). `scripts/testnet-orchard-rpc-malformed-edge-load-smoke` records parent+child RSS under concurrent malformed RPC verification; latest local run peaked at about 78.5 MB with three malformed proof requests. `scripts/testnet-live-orchard-malformed-edge-load` records the same class of malformed proof load on target hardware; latest single live run peaked at `48072KB` RSS with max malformed latency `1484ms`. `scripts/testnet-live-orchard-malformed-edge-load-series` now provides repeated target-hardware percentiles across all five validator slots: `p50=1581ms`, `p95=1614ms`, `p99=1614ms`, and RSS `p99=49216KB` in `reports/testnet-live-orchard-malformed-edge-load-series/live-orchard-malformed-edge-load-series-20260515T170500Z/testnet-live-orchard-malformed-edge-load-series.json`. `scripts/testnet-live-orchard-privacy-soak` now provides a repeated live full-flow distribution artifact with two consecutive deposit/spend/withdraw flows and pool-accounting progression at height 28: `reports/testnet-live-orchard-privacy-soak/live-orchard-privacy-soak-20260515T175026Z/testnet-live-orchard-privacy-soak.json`. The benchmark evidence pack now includes the Orchard RPC threshold gate as a required pass/fail artifact. Remaining: maximum supported proof request size publication and longer soak distributions. |
| PRIV-028 | Verifier DoS bounds | Semi-production | Partial | Orchard wrapper types enforce exact/bounded proof and ciphertext sizes before verification. `scripts/testnet-orchard-wallet-finality-smoke` now mutates a valid Orchard action into a 1,048,577-byte proof payload, a 4,097-byte encrypted-output ciphertext, a 7,264-byte malformed proof, a 580-byte malformed encrypted-output ciphertext, and a direct-deposit envelope with a mutated amount. The oversized cases fail closed with `oversized_hex`; exact-size malformed proof/ciphertext fail closed with `proof_verification_failed` and `binding_signature_invalid`; the malformed direct-deposit amount fails closed with `external binding` in about 11.6s before the valid ordered flow continues. Latest malformed direct-deposit evidence: `reports/testnet-orchard-wallet-finality-smoke/privacy-direct-deposit-malformed-20260515T144715Z/testnet-orchard-wallet-finality-smoke.json`. `rpc-serve --allow-orchard-batch-create` now provides child-process timeout, per-peer/total rate limits, a concurrent verifier-worker cap, and child process isolation fields for remote Orchard batch creation from bounded `action_json` and direct-deposit `deposit_json`; file-path requests and direct apply are rejected. Latest RPC admission evidence from `scripts/testnet-orchard-rpc-batch-create-smoke`: `reports/testnet-orchard-rpc-batch-create/orchard-rpc-deposit-batch-create-20260515T151620Z/testnet-orchard-rpc-batch-create.json`, including malformed direct-deposit rejection on `external binding` and zero child timeouts. Latest RPC concurrency-limit evidence: `reports/testnet-orchard-rpc-concurrency-limit/orchard-rpc-concurrency-limit-v0-20260515T171128Z/testnet-orchard-rpc-concurrency-limit.json` proves the second simultaneous malformed proof request fails with `rpc_orchard_batch_create_concurrency_limited`, the server reports `max_orchard_batch_create_concurrent=1`, and a post-pressure `status` succeeds with zero child timeouts. Latest child-isolation evidence: `reports/testnet-orchard-rpc-child-isolation/orchard-rpc-child-isolation-v0-20260515T174034Z/testnet-orchard-rpc-child-isolation.json` proves `orchard_pool_report` succeeds through a read-only child boundary, no child timeout fires, stdin is null, inherited environment is cleared, cwd is the canonical data dir, and a canary environment value does not leak into responses/reports. Latest local concurrent malformed-edge evidence: `reports/testnet-orchard-rpc-malformed-edge-load/orchard-rpc-malformed-edge-load-v0-20260515T113620Z/testnet-orchard-rpc-malformed-edge-load.json` proves three concurrent malformed proof requests fail closed in about 13.5-13.8s, no child timeout fires, post-load `status` succeeds, and sampled parent+child RSS peaks at about 78.5 MB. Latest live target-hardware malformed-edge evidence: `reports/testnet-live-orchard-malformed-edge-load/live-orchard-malformed-edge-load-20260515T162850Z/testnet-live-orchard-malformed-edge-load.json` proves three live-context malformed proof requests fail closed on `proof_verification_failed`, no child timeout fires, post-load `status` succeeds, and sampled parent+child RSS peaks at `48072KB`. Latest repeated live edge-load evidence across all five validator slots: `reports/testnet-live-orchard-malformed-edge-load-series/live-orchard-malformed-edge-load-series-20260515T170500Z/testnet-live-orchard-malformed-edge-load-series.json` with five samples, zero child timeouts, status reads after every load, malformed latency `p95=1614ms`, and RSS `p99=49216KB`. Latest rate-limit evidence: `reports/testnet-orchard-rpc-rate-limit/orchard-rpc-rate-limit-v0-20260515T114343Z/testnet-orchard-rpc-rate-limit.json` proves per-peer and global caps reject excess requests without child timeout. Latest threshold gate: `reports/testnet-orchard-rpc-threshold-gate/orchard-rpc-threshold-gate-v0-20260515T115102Z/testnet-orchard-rpc-threshold-gate.json` requires max malformed latency `30000ms`, max sampled RSS `524288KB`, and zero child timeouts; benchmark evidence now fails if this gate is absent or failed. Remaining: longer live edge-load soaks and stronger host-level sandboxing such as service-level memory caps, seccomp, or cgroups before public write exposure. |
| PRIV-029 | Operator config | Semi-production | Partial | `postfiat-node orchard-operator-policy` emits `postfiat-orchard-operator-policy-v1` with chain/genesis context, explicit privacy-enabled posture, root-retention setting, indexing role, protocol proof/ciphertext/action caps, verifier concurrency/timeout settings, and remote batch-create enforcement flags. Current evidence: `reports/testnet-orchard-operator-policy/operator-policy-v1-20260515T110835Z/orchard-operator-policy.json`. `postfiat-node orchard-fee-resource-policy` now adds the fee/resource operating surface for the same controlled privacy posture, with current evidence at `reports/testnet-orchard-fee-resource-policy/orchard-fee-resource-policy-20260515T171204Z/testnet-orchard-fee-resource-policy.json`. The reports are intentionally scoped: protocol size/action bounds are enforced; remote batch creation can use RPC child timeout, `action_json`, server-controlled spooling, rate limits, and a concurrent verifier-worker cap; verifier work still runs inside a child process; direct public shielded apply remains disallowed. |
| PRIV-030 | Audit packet v0 | Semi-production | Partial | `scripts/testnet-orchard-privacy-audit-packet` now emits a redaction-safe `postfiat-orchard-privacy-audit-packet-v1` that aggregates code refs, claim boundaries, docs hashes, local wallet/direct-deposit/spend/withdraw evidence, peer-certified convergence, restart evidence, partial-outage recovery, below-quorum no-advance, remote deposit RPC evidence, RPC concurrency-limit evidence, RPC child-isolation evidence, live binary readiness, live command preflight, live direct-deposit evidence, live deposit/spend/withdraw full-flow evidence including a live one-validator outage/recovery during shielded deposit, live privacy observability, live public pool/anonymity-bound reporting, live repeated privacy-soak evidence with six shielded block-log finality records, live Orchard snapshot/import evidence, live Orchard restart evidence, live target-hardware malformed-load evidence, repeated target-hardware malformed-load percentile evidence, deterministic Orchard vector evidence, fee/resource policy evidence, and local public Orchard pool/anonymity-bound evidence. Latest packet passed with `credential_material_redacted=true`: `reports/testnet-orchard-privacy-audit-packet/orchard-privacy-audit-packet-20260515T185212Z/orchard-privacy-audit-packet.json`. Remaining: dependency/audit inventory and external-review signoff fields. |

## Production Hardening Burndown

| ID | Item | Status | Exit Criteria |
| --- | --- | --- | --- |
| PRIV-PROD-001 | External cryptographic review | Open | Independent reviewer signs off on statement, witness, commitment, nullifier, tree, and encryption design, with findings tracked. |
| PRIV-PROD-002 | Implementation audit | Open | External audit covers Rust host verifier, parser bounds, wallet key handling, proof integration, and consensus apply path. Critical/high findings closed or explicitly accepted. |
| PRIV-PROD-003 | Circuit/guest reproducibility | Open | Guest build is reproducible; verifier binds to exact image/program id; upgrade path is governance-controlled and replayable. |
| PRIV-PROD-004 | Trusted setup posture | Open | If backend requires setup, setup ceremony and toxic-waste assumptions are documented. Preferred v0 path avoids trusted setup. |
| PRIV-PROD-005 | Long-running privacy soak | Partial | First sustained live privacy soak exists: `scripts/testnet-live-orchard-privacy-soak` ran two consecutive live Orchard/Halo2 full flows from height 22 to 28, collected baseline/post-flow pool reports, required monotonic output/nullifier/deposit/withdraw/fee progression, kept the public write edge closed, and passed redaction checks: `reports/testnet-live-orchard-privacy-soak/live-orchard-privacy-soak-20260515T175026Z/testnet-live-orchard-privacy-soak.json`. The audit packet now follows the soak child flow artifacts and verifies six shielded block-log finality records for deposit/spend/withdraw across both samples. Live post-soak snapshot/import evidence also exists: `reports/testnet-live-orchard-snapshot-drill/live-orchard-snapshot-20260515T180932Z/testnet-live-orchard-snapshot-drill.json` proved restored Orchard counters/root/bounds match on all five validators at height 28. Live post-soak restart evidence exists: `reports/testnet-live-orchard-restart-drill/live-orchard-restart-20260515T182006Z/testnet-live-orchard-restart-drill.json` proved validator/RPC restart does not change state or Orchard pool telemetry. Remaining: longer mixed transparent/shielded soak with partial outage, below-quorum, archive, and RPC load evidence in one release packet. |
| PRIV-PROD-006 | Wallet security review | Open | Key storage, seed handling, viewing key export, disclosure generation, and signer boundaries are reviewed and redaction-tested. |
| PRIV-PROD-007 | Custodian/exchange flow | Open | Deposit/address management, view-only monitoring, withdrawal review, disclosure, and account reconciliation are documented and exercised. |
| PRIV-PROD-008 | Compliance policy | Open | Travel-rule, sanctions-screening, lawful disclosure, abuse response, and institutional audit workflows are specified without undermining user privacy claims. |
| PRIV-PROD-009 | Anonymity-set reporting | Partial | `postfiat-node orchard-pool-report` and read-only RPC `orchard_pool_report` now expose pool size, nullifier count, retained-root count, latest root, turnstile volumes, fee burns, withdraw volume, and conservative active-note bounds without exposing wallet/key/witness material. Local evidence: `reports/testnet-orchard-pool-report/orchard-pool-report-v0-20260515T172747Z/testnet-orchard-pool-report.json` with `output_count=8`, `nullifier_count=8`, `direct_deposit_total=11`, `accounted_pool_deposit_total=18`, `fee_burn_total=4`, `withdraw_total=1`, exact active-note count not claimed, and redaction checks passed. Live five-validator evidence: `reports/testnet-live-orchard-pool-report/live-orchard-pool-report-20260515T173435Z/testnet-live-orchard-pool-report.json` with converged height `22`, `output_count=10`, `nullifier_count=10`, `direct_deposit_total=33`, `accounted_pool_deposit_total=33`, `fee_burn_total=4`, `withdraw_total=1`, CLI/RPC parity on every validator, exact active-note count not claimed, and redaction checks passed. Latest soak final pool report in `reports/testnet-live-orchard-privacy-soak/live-orchard-privacy-soak-20260515T175026Z/testnet-live-orchard-privacy-soak.json` shows height `28`, `output_count=22`, `nullifier_count=22`, `direct_deposit_total=55`, `accounted_pool_deposit_total=55`, `fee_burn_total=12`, and `withdraw_total=3` with exact active-note count still not claimed. Remaining: public dashboard/analytics packaging and final public-claims wording. |
| PRIV-PROD-010 | Upgrade and emergency controls | Open | Privacy backend can be paused or upgraded through governance without corrupting existing notes; user funds have documented recovery/migration paths. |
| PRIV-PROD-011 | Multi-client verification | Open | At least one independent verifier or minimal verifier implementation validates proof journals/disclosure packets. |
| PRIV-PROD-012 | Release evidence pack | Open | A single script aggregates all privacy readiness, benchmark, audit, live-soak, wallet, RPC, and claim-boundary evidence into a redacted release packet. |

## Suggested Execution Order

### Slice 1: Proof And Statement Skeleton

Goal: make the debug proof backend replaceable without touching shielded RPC and
state surfaces again.

1. Add versioned public journal and witness structs.
2. Extend `ProofSystem` to handle proof bytes and backend ids.
3. Add a production-mode feature/config that refuses `DebugProofSystem`.
4. Add local tests proving debug mode and production mode cannot be confused.

### Slice 2: zkVM Prototype

Goal: prove one spend outside consensus, then verify it through the node.

1. Build one-input/two-output spend guest.
2. Bind journal to chain id, genesis hash, protocol version, root, nullifier,
   output commitments, and fee.
3. Verify the receipt in a node command.
4. Benchmark RISC Zero versus SP1 on the exact guest.

### Slice 3: Encrypted Notes And Wallet Scan

Goal: make outputs privately recoverable by recipients.

1. Define ML-KEM note envelope.
2. Implement KEM/DEM encryption/decryption.
3. Add wallet receive key generation.
4. Add SDK scan/decrypt and disclosure packet generation.

### Slice 4: Consensus Privacy Alpha

Goal: run real shielded transactions through certified ordered batches.

1. Replace debug shielded spend apply with proof-required apply in production
   mode.
2. Preserve duplicate-nullifier and root checks as consensus-critical.
3. Add fee/resource pricing evidence and rerun after repricing or larger proof/request distributions.
4. Run local and live privacy alpha gates.

### Slice 5: Semi-Production Gate

Goal: make a controlled institutional demo defensible.

1. Generate privacy evidence pack.
2. Publish claim boundaries.
3. Run external design review.
4. Run live mixed transparent/shielded soak.
5. Record disclosure/compliance workflow evidence.

### Slice 6: Production Gate

Goal: make public production privacy claims.

1. Complete external cryptography and implementation audits.
2. Close critical/high findings.
3. Run long live soak on release hardware.
4. Publish anonymity-set and disclosure model.
5. Freeze upgrade and emergency procedures.

## Current Recommended First Engineering Task

Implement `PRIV-001` through `PRIV-004` first:

- create `crates/privacy` production statement/witness types;
- extend `crates/proofs` beyond debug artifacts;
- add a production privacy mode that refuses debug proofs;
- add tests and docs proving the current debug path cannot be mistaken for
  production privacy.

This gives the zkVM work a stable target and prevents future docs or agents from
accidentally overclaiming the existing debug adapter.
