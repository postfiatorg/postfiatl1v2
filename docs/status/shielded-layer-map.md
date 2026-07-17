# Shielded Layer Map for Private NAV OTC

Date: 2026-06-22

Scope: Phase 1 of `docs/specs/private-otc-shielded-scope.md`.

Status: current-state map. The Asset-Orchard private swap path has been implemented and exercised on WAN devnet. True private egress is not implemented; the current egress path is disclosed egress.

## Executive Summary

The shielded layer is split into two paths:

- The legacy/debug shielded-note path lives in `crates/privacy/src/lib.rs` and uses `ShieldedNote` records with explicit `asset_id`, `owner`, `value`, memo, note commitment, and a debug proof adapter.
- The production Orchard path lives in `crates/privacy_orchard/src` plus the node apply code in `crates/node/src/privacy.rs` and `crates/node/src/lib_parts/part_02.rs`. It uses real Orchard/Halo2 actions, nullifiers, note commitments, encrypted outputs, retained anchors, spend/binding signatures, and turnstile accounting.

The important current-state finding for private NAV OTC is this: the original Orchard wallet path remains value-only, but the Asset-Orchard path now adds asset-typed notes and a fixed-shape two-input/two-output Halo2 swap circuit in the versioned `asset-orchard-v1` pool. Internal Asset-Orchard swaps do not expose raw asset ids, values, owners, recipients, or price in the public action. Boundary actions are public: ingress reveals the asset/value being shielded, and the current egress path reveals the note opening and the asset/value being credited back to the public issued-asset ledger.

## Core Data Model

The shared types are in `crates/types/src/lib_parts/shielded_bridge_governance.rs`.

`ShieldedState` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:37`) contains:

- `next_note_position`
- debug-path `notes: Vec<ShieldedNote>`
- debug-path `nullifiers: Vec<String>`
- `turnstile_events: Vec<TurnstileEvent>`
- optional real `orchard: Option<OrchardPoolState>`

`ShieldedNote` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:24`) is the debug-note representation:

- `note_id`
- `commitment`
- `position`
- `owner`
- `asset_id`
- `value`
- `rho`
- `memo`
- `created_by`

`OrchardPoolState` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:69`) is the persisted public state for the real Orchard pool:

- `pool_id`
- `nullifiers`
- `output_commitments`
- `encrypted_outputs`
- `asset_commitment_records`
- `asset_orchard_outputs`
- retained `root_history`
- `accepted_anchors`
- aggregate `value_balance_total`
- `turnstile_deposit_total`
- `fee_burn_total`
- `withdraw_total`

`TurnstileEvent` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:125`) is the bridge between transparent accounting and shielded pool accounting. It records event kind, owner, asset id, amount, note/transfer id, source pool, and target pool.

## ShieldedAction Framework

The consensus-facing action enum is `ShieldedAction` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:202`). It is serde-tagged by `kind` and currently supports:

- `shield_mint` -> `ShieldMintAction`
- `shield_spend` -> `ShieldSpendAction`
- `shield_migrate` -> `ShieldMigrateAction`
- `orchard_action_v1` -> `OrchardActionPayload`
- `orchard_withdraw_v1` -> `OrchardWithdrawActionPayload`
- `orchard_deposit_v1` -> `OrchardDepositActionPayload`
- `shielded_swap_v1` -> `ShieldedSwapActionPayload`
- `asset_orchard_ingress_v1` -> `AssetOrchardIngressActionPayload`
- `asset_orchard_egress_v1` -> `AssetOrchardEgressActionPayload`

`ShieldedActionBatch` wraps an ordered vector of actions plus a `batch_id` (`crates/types/src/lib_parts/shielded_bridge_governance.rs:220`). The node creates and verifies the batch id through `build_shielded_action_batch` / `verify_shielded_action_batch_id` (`crates/node/src/lib_parts/part_03.rs:3770`). The batch id is chain-bound through `chain_bound_action_batch_id` with domain `postfiat.shielded_action_batch.v1` and batch kind `shielded`.

The ordered apply path is:

1. CLI/RPC creates a `ShieldedActionBatch`.
2. `apply_shielded_batch` reads the batch, verifies `batch_id`, loads ledger/shielded/governance state, and prepares the next ordered commit (`crates/node/src/privacy.rs:3518`).
3. `execute_shielded_batch` dispatches each `ShieldedAction` match arm (`crates/node/src/lib_parts/part_02.rs:9419`).
4. The resulting ledger/shielded state is committed through the normal ordered-commit path with receipts.

## Debug Mint, Spend, Migrate

The debug path is useful for controlled local testing and legacy tooling, but it is not the real Orchard privacy path.

Mint:

- `create_shielded_mint_batch` wraps `ShieldedAction::Mint` (`crates/node/src/privacy.rs:3340`).
- `execute_shielded_batch` dispatches to `mint_debug_note_with_creator` with an ordered creator string (`crates/node/src/lib_parts/part_02.rs:9428`).
- `mint_debug_note_with_creator` validates owner/asset/value, derives a deterministic mint id, verifies a debug proof statement, builds the note, advances the note position, appends the note, and records a bootstrap turnstile event (`crates/privacy/src/lib.rs:83`).
- The debug mint proof public inputs are `owner`, `asset_id`, `value`, `position`, `mint_id` (`crates/proofs/src/lib.rs:10`).

Spend / transfer:

- `create_shielded_spend_batch` wraps `ShieldedAction::Spend` (`crates/node/src/privacy.rs:3356`).
- `execute_shielded_batch` dispatches to `spend_debug_note` (`crates/node/src/lib_parts/part_02.rs:9443`).
- `spend_debug_note` loads the source note, derives the nullifier, rejects duplicate nullifiers and overspend, verifies the debug spend statement, creates a recipient note and optional change note with the same `asset_id`, then appends the nullifier and outputs (`crates/privacy/src/lib.rs:140`).
- The debug spend proof public inputs are `note_id`, `nullifier`, `to`, `amount`, `spend_id` (`crates/proofs/src/lib.rs:12`).

Migrate:

- `create_shielded_migrate_batch` wraps `ShieldedAction::Migrate` (`crates/node/src/privacy.rs:3372`).
- `execute_shielded_batch` dispatches to `migrate_debug_note` (`crates/node/src/lib_parts/part_02.rs:9457`).
- Migration nullifies a debug note and records a pool-migration turnstile event. Orchard turnstile budget includes migration totals into a target pool through `orchard_turnstile_migration_total` / `orchard_turnstile_budget_total` (`crates/node/src/privacy.rs:2746`).

Debug note commitments and nullifiers:

- `debug_nullifier(note_id)` hashes the note id with domain `postfiat.shielded.nullifier.debug.v1` (`crates/privacy/src/lib.rs:342`).
- `debug_note_commitment` includes owner, `asset_id`, value, memo, position, and creator (`crates/privacy/src/lib.rs:346`).
- `note_tree_root` hashes the vector of debug notes (`crates/privacy/src/lib.rs:409`).
- The node exposes a chain-bound tree root that additionally binds chain id, genesis hash, protocol version, and notes (`crates/node/src/privacy.rs:3705`).

## Orchard Action Model

The real shielded proof boundary is `OrchardShieldedAction` in `crates/privacy_orchard/src/types.rs:663`. It stores:

- `pool_id`
- proof/circuit identifiers
- flags and anchor
- nullifiers
- randomized verification keys
- value commitments
- output commitments
- encrypted outputs
- aggregate `value_balance`
- optional `external_binding_hash`
- fee
- proof bytes
- spend authorization signatures
- binding signature

Validation in `OrchardShieldedAction::validate` checks canonical identifiers, at least one action/nullifier, optional external binding hash length, vector length consistency, and that each encrypted output commitment matches the corresponding output commitment (`crates/privacy_orchard/src/types.rs:684`).

The verifier reconstructs the raw Orchard authorized bundle from the serialized wrapper (`crates/privacy_orchard/src/verify.rs:434`), computes a chain-bound authorizing sighash, verifies the Halo2 proof, verifies binding and spend signatures, and extracts the public bundle fields (`crates/privacy_orchard/src/verify.rs:333`).

The authorizing sighash binds:

- chain id
- genesis hash
- protocol version
- pool id
- proof system id
- circuit id
- flags
- anchor
- value balance
- fee
- optional external binding hash
- action count
- each action nullifier, randomized key, output commitment, value commitment, epk, encrypted ciphertext, and out ciphertext

This is implemented in `orchard_authorizing_sighash_with_external_binding` (`crates/privacy_orchard/src/verify.rs:167`).

## Orchard Note Tree and Nullifiers

The real Orchard note tree is the Orchard incremental note commitment tree over output commitments, not the debug `ShieldedNote` vector.

Tree/witness functions:

- `orchard_anchor_from_commitments` builds an Orchard frontier root from the ordered output commitments (`crates/privacy_orchard/src/verify.rs:944`).
- `orchard_merkle_witness_from_commitments` builds a witness path for a selected output index and verifies the computed root matches the frontier root (`crates/privacy_orchard/src/verify.rs:961`).
- `OrchardPoolState.root_history` retains roots with output counts; `verify_orchard_root_history` recomputes each retained root from commitment prefixes and enforces monotonic output counts (`crates/node/src/privacy.rs:4240`).

Nullifier checks:

- The apply path rejects duplicate nullifiers inside a single action (`crates/node/src/privacy.rs:1883`).
- It rejects nullifiers already present in the Orchard pool (`crates/node/src/privacy.rs:1920`).
- It rejects duplicate output commitments inside a single action and output commitments already present in pool state (`crates/node/src/privacy.rs:1895`, `crates/node/src/privacy.rs:1932`).
- It requires the action anchor to be retained, or the empty root for a fresh pool (`crates/node/src/privacy.rs:1954`).

When an Orchard action is accepted, apply appends verified nullifiers, output commitments, encrypted outputs, accepted anchor, updated aggregate counters, and the new current root (`crates/node/src/privacy.rs:2016`).

## Orchard Mint / Transfer / Spend / Withdraw Wiring

Output-only Orchard action:

- CLI command: `orchard-output-create` (`crates/node/src/main_parts/cli_dispatch.rs:4608`).
- Node helper: `create_orchard_output_action` (`crates/node/src/privacy.rs:475`).
- Proof builder: `orchard_build_output_action_with_external_binding` (`crates/privacy_orchard/src/verify.rs:539`).
- The builder adds an Orchard output to a recipient address with `NoteValue::from_raw(value)` and memo, builds the bundle, creates the proof, signs it, and serializes it into `OrchardShieldedAction` (`crates/privacy_orchard/src/verify.rs:629`).

Orchard spend / private transfer:

- CLI command: `orchard-spend-create` (`crates/node/src/main_parts/cli_dispatch.rs:4685`).
- Node helper: `create_orchard_spend_action` (`crates/node/src/privacy.rs:820`).
- The helper scans local encrypted outputs, selects an input output index, rejects already-nullified notes, builds a Merkle witness, validates amount/fee/change, then calls `orchard_build_spend_action`.
- Proof builder: `orchard_build_spend_action` (`crates/privacy_orchard/src/verify.rs:667`).
- The builder reconstructs the Orchard note from decrypted note parts, verifies the witness anchor, adds one spend, adds recipient output and optional change output, requires `value_balance == fee`, creates the proof, applies the spend authorization signature, and returns an `OrchardShieldedAction`.
- The batch wrapper is `shield-batch-orchard`, which wraps `OrchardV1` (`crates/node/src/privacy.rs:3387`, `crates/node/src/main_parts/cli_dispatch.rs:5002`).
- Apply parses `action_json`, verifies the serialized action, and calls `apply_verified_orchard_action_to_shielded_state` (`crates/node/src/lib_parts/part_02.rs:9497`).

Orchard deposit / shield mint from transparent balance:

- CLI command: `orchard-deposit-create` (`crates/node/src/main_parts/cli_dispatch.rs:4643`).
- Node helper: `create_orchard_deposit_action` (`crates/node/src/privacy.rs:663`).
- The helper builds a transparent funding transfer to the burn sink for `amount + fee`, computes `orchard_deposit_external_binding_hash`, builds an output action with an external binding hash, verifies it, and writes an `OrchardDepositActionFile`.
- Batch wrapper: `shield-batch-orchard-deposit`, which wraps `OrchardDepositV1` (`crates/node/src/privacy.rs:3404`, `crates/node/src/main_parts/cli_dispatch.rs:5017`).
- Apply validates payload, verifies funding transfer amount/target, checks the action external binding, executes the funding transfer, burns the sink amount, applies the Orchard action with deposit budget credit, records an `orchard_deposit` turnstile event, and updates fee policy (`crates/node/src/privacy.rs:2227`, `crates/node/src/privacy.rs:2548`).

Orchard withdraw / unshield to transparent:

- CLI command: `orchard-withdraw-create` (`crates/node/src/main_parts/cli_dispatch.rs:4741`).
- Node helper: `create_orchard_withdraw_action` (`crates/node/src/privacy.rs:990`).
- Proof builder: `orchard_build_withdraw_action` (`crates/privacy_orchard/src/verify.rs:797`).
- The builder spends one note, optionally creates change, requires positive `value_balance == withdraw_amount + fee`, binds the transparent recipient/amount/fee/policy/disclosure through `external_binding_hash`, creates proof and spend signature, and returns an `OrchardShieldedAction`.
- Batch wrapper: `shield-batch-orchard-withdraw`, which wraps `OrchardWithdrawV1` (`crates/node/src/privacy.rs:3471`, `crates/node/src/main_parts/cli_dispatch.rs:5034`).
- Apply validates payload, verifies external binding, enforces fee/resource policy and pool issued value, applies the Orchard nullifier/output state transition, and credits the transparent recipient if accepted (`crates/node/src/privacy.rs:2353`, `crates/node/src/privacy.rs:2520`).

Disclosure:

- CLI commands: `orchard-disclose` and `orchard-disclosure-verify` (`crates/node/src/main_parts/cli_dispatch.rs:4849`, `crates/node/src/main_parts/cli_dispatch.rs:4875`).
- Disclosure packets are built from a scanned output and local finality evidence; verification checks local context, packet hash, and that the commitment is in an archived shielded batch/block (`crates/node/src/privacy.rs:1242`, `crates/node/src/privacy.rs:1306`).
- Archive commitment lookup now recognizes ordinary Orchard actions, `ShieldedSwapV1` output commitments, and `AssetOrchardIngressV1` output commitments. `AssetOrchardEgressV1` has no new private output commitment because it nullifies an existing typed note and credits a public issued-asset balance.

## RPC and CLI Surfaces

CLI commands are declared in usage text at `crates/node/src/main_parts/runtime_helpers.rs:618` and dispatched in `crates/node/src/main_parts/cli_dispatch.rs:4523` onward.

Current shielded commands include:

- `verify-shielded`
- `orchard-action`
- `orchard-output-create`
- `orchard-deposit-create`
- `orchard-spend-create`
- `orchard-withdraw-create`
- `orchard-keygen`
- `orchard-view-key-export`
- `orchard-scan`
- `orchard-disclose`
- `orchard-disclosure-verify`
- `shield-batch-mint`
- `shield-batch-spend`
- `shield-batch-migrate`
- `shield-batch-orchard`
- `shield-batch-orchard-deposit`
- `shield-batch-orchard-withdraw`
- `asset-orchard-ingress-create`
- `shield-batch-asset-orchard-ingress`
- `asset-orchard-swap-create`
- `shield-batch-swap`
- `asset-orchard-egress-create`
- `shield-batch-asset-orchard-egress`
- `apply-shield-batch`

RPC SDK validation recognizes shielded action kinds in archive payloads and batch responses. Asset-Orchard command-line creation and batch wrapping exist; public RPC request builders for the full user flow remain CLI/request-file oriented rather than wallet-service oriented.

## Current Asset-Orchard Swap

The implemented Asset-Orchard path uses the third design from the original gate: one versioned asset-typed pool with private asset commitments and a Halo2 proof enforcing the exact two-note asset/value permutation.

Implemented pieces:

- `AssetOrchardIngressV1` burns a transparent issued-asset balance and inserts an asset-typed note commitment into `asset-orchard-v1`.
- `asset-orchard-swap-create` consumes two local Asset-Orchard wallet notes, creates two replacement notes, and writes a public `AssetOrchardSwapAction`.
- `shield-batch-swap` wraps that action as `ShieldedAction::ShieldedSwapV1`.
- Consensus parses the swap JSON, verifies the chain/genesis/protocol/pool-bound Asset-Orchard proof and RedPallas spend authorization signatures, rejects duplicate or replayed nullifiers, rejects duplicate or existing output commitments, requires a retained anchor, appends nullifiers/output commitments/encrypted outputs, and recomputes retained roots.
- The public action exposes pool id, proof/circuit identifiers, anchor, nullifiers, randomized verification keys, output commitments, encrypted outputs, swap binding hash, fee, proof bytes, and spend authorization signatures.
- The public action does not expose raw asset ids, values, owners, recipients, or price.

The live WAN-devnet evidence is recorded in `docs/runbooks/private-nav-otc-shielded-swap-wan-devnet.md`. The prover optimization and K=15 cached-key measurements are recorded in `docs/status/zk-prover-optimization-results.md`.

## Current Asset-Orchard Egress

The current Asset-Orchard egress is disclosed egress, not private egress.

Implemented pieces:

- `asset-orchard-egress-create` reads a local Asset-Orchard wallet note and writes an egress payload.
- `shield-batch-asset-orchard-egress` wraps the payload as `ShieldedAction::AssetOrchardEgressV1`.
- Consensus validates the public note opening against the disclosed `asset_id` and `amount`, recomputes the commitment and nullifier, checks the disclosed nullifier has not already been used, verifies the spend authority and RedPallas spend authorization signature, and credits the public issued-asset balance.
- The receipt message is explicit: `AssetOrchard disclosed egress nullified typed note and credited public issued asset balance`.

Privacy boundary:

- Egress reveals the note opening, `asset_id`, amount, destination account, nullifier, and spend/view material needed for validation.
- It is useful for functional bridge-out and recovery, but it is not a private cash-out primitive.
- A true private egress circuit is still future work. It must prove ownership of a valid unspent typed note and public output correctness without revealing the note opening or linking the private commitment to the public exit more than the chosen policy requires.

## Current Gaps

1. The ordinary Orchard wallet path is still value-only; asset privacy for NAV OTC lives in the Asset-Orchard path.
2. Asset-Orchard v1 supports fixed two-input/two-output internal swaps. It does not provide a general arbitrary-recipient private asset transfer UX yet.
3. Private egress is not implemented. Current egress is disclosed and reveals the exited asset/value.
4. Issuer/NAV policy authorization for private primary mint/redeem is not yet a standalone private policy circuit; current reserve/NAV accounting remains transparent at the boundaries.
5. The optimized hot prover exists, but one-shot CLI workflows can still pay cold proving-key setup unless a long-lived runner/prover is used.

## Gate Findings

1. Existing debug transfer/mint/spend plumbing remains useful for local testing but is not the production NAV privacy path.
2. Existing Orchard deposit/withdraw paths are externally bound to transparent funding/withdrawal envelopes and update turnstile/pool accounting.
3. Asset-Orchard ingress/swap/egress action kinds are present in the consensus-facing `ShieldedAction` enum.
4. The Asset-Orchard swap circuit is the current production-candidate primitive for private a651/pfUSDC movement inside PFTL.
5. The current disclosed egress path must not be described as private egress or private cash-out.
