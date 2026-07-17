# XRPL Feature Parity Burndown

Status: P0 feature-parity implementation complete through `DEX-008`; P1
issued-asset controls and metrics complete through `ASSET-011`; NFT P1
policy work complete through `NFT-008`; XRPL-style Python wallet helper UX
complete
Date: 2026-05-21
Scope: payment rails, issued assets, trustlines, escrow, atomic settlement, NFTs,
and the later DEX decision for PostFiat L1 controlled testnet.

This is the working scope for adding the XRPL-like financial primitives that
matter before a public DEX. The execution call is simple: build the rails first,
then decide how much exchange functionality belongs in protocol.

## Executive Call

PostFiat should not lead with a DEX. A native order book is only useful after
the ledger has assets, trust relationships, account history, reserves, fee
policy, and wallet/RPC support. Shipping the DEX first would increase consensus
surface, ordering-fairness pressure, and user confusion before the base ledger
objects exist.

The right order is:

1. Payment rails and memos. Status: complete; `PAY-001` through `PAY-007`
   implementation and controlled multi-validator evidence gates are complete.
2. Issued assets and trustlines. Status: P0 complete; `ASSET-001` through
   `ASSET-008` deterministic protocol/state/execution/mempool/fee/read-RPC,
   Python wallet, replay, and invariant gates are complete. `ASSET-009` issuer
   freeze/unfreeze and authorization control surfaces are complete. `ASSET-010`
   issuer-declared clawback policy is complete. `ASSET-011` asset-level metrics
   and monitor output are complete.
3. Escrow and atomic settlement. Status: PFT escrow P0 complete,
   issued-asset escrow P1 complete, and atomic settlement templates complete;
   `ESCROW-001` through `ESCROW-009`
   deterministic state, signed transaction-envelope, native PFT escrow
   execution, derived escrow index, read RPC, account-history, Python
   wallet-helper, restart/replay invariant, and issued-asset escrow gates are
   complete.
4. NFTs as ledger objects. Status: `NFT-001` deterministic NFT id, bounded
   metadata, and ownership-state definitions are complete; `NFT-002` signed
   mint/transfer/burn transaction envelopes, batch commitments, mempool
   serialization, and deterministic pre-execution rejected receipts are
   complete; `NFT-003` deterministic mint/transfer/burn execution,
   authorization, fee policy, mempool admission, and Python client quote/submit
   paths are complete; `NFT-004` NFT read RPC is complete; `NFT-005`
   account_tx rows with `nft_id` are complete; `NFT-006` Python wallet
   lifecycle helpers for mint, transfer, and burn are complete; `NFT-007`
   deterministic issuer transfer fee is complete; `NFT-008` collection-level
   policy flags are complete.
5. DEX/order book research and implementation. Status: `DEX-001` design gate,
   `DEX-002` matching model, `DEX-003` ordering/MEV policy, and `DEX-004`
   protocol/state offer types are complete after primitives above became live;
   `DEX-005` deterministic offer create/cancel execution, reserves, mempool
   dry-runs, replay/account_tx, quote/submit paths, and `DEX-006` bounded
   matching/fill execution, plus `DEX-007` DEX read RPC and wallet helper
   ergonomics, are complete. `DEX-008` conservation/property/replay and
   controlled-validator smoke evidence is complete.

## Current Baseline

The current transparent chain has native PFT accounts and transfers:

- `crates/types/src/lib.rs` defines `Account` with `address`, `balance`,
  `sequence`, and optional `public_key_hex`.
- `crates/types/src/lib.rs` defines `LedgerState` as account state with
  optional ledger-native issued asset definitions and trustlines.
- `crates/types/src/lib.rs` defines `UnsignedTransfer` and `SignedTransfer` for
  native transparent PFT transfers.
- `crates/node/src/lib.rs` implements transfer fee quotes, mempool admission,
  transfer batch creation, and signed transfer submission.
- `crates/node/src/block_finality.rs` implements `account_tx` indexing for
  transparent transfers.
- `python/postfiat_rpc/client.py` and `python/postfiat_rpc/wallet.py` provide
  Python RPC and wallet helpers for PFT send/account history.

Implemented ledger-native transparent transaction functionality now includes
issued asset creation, trustline creation/mutation, issued payments, burns,
native PFT escrow create/finish/cancel, read RPC, account history, fee quote,
mempool admission, replay checks, Python wallet helpers, and two-sided atomic
settlement template construction for PFT/issued-asset swaps. Deterministic NFT
ids, bounded metadata fields, ownership state, burned-state tracking, derived
owner/issuer/collection indexes, signed NFT transaction envelopes, NFT-aware
batch/mempool serialization, NFT read RPC, NFT account history, and Python NFT
mint/transfer/burn wallet helpers are defined.

The Python package now also exposes XRPL-style convenience helper names over
the canonical wallet helpers: `send_payment`, `mint_token`, `set_trustline`,
`send_token`, issuer trustline controls, `create_escrow`, `finish_escrow`,
`cancel_escrow`, NFT lifecycle aliases, `build_atomic_swap_template`, and
`place_offer`. These wrappers do not bypass protocol quote/sign/submit paths;
they give Python callers a familiar wallet UX without requiring manual JSON
assembly.

Remaining outside this feature-parity burndown:

- richer atomic-settlement condition languages beyond the deterministic template
  rails already added;
- public DEX/write-edge claims, advanced order types, and production
  decentralization claims.

The shielded and bridge subsystems already carry `asset_id` and memo-like
fields in their own domains, but that is not the same thing as a general
transparent asset layer.

## Design Rules

These features affect consensus state. They must be implemented as deterministic
state transitions with canonical serialization and bounded storage costs.

- Do not mutate the existing `UnsignedTransfer::signing_bytes()` format without
  a versioned transaction upgrade.
- Add a versioned transaction envelope or explicit `PaymentV2` path for new
  payment fields.
- Every state-expanding object needs reserves or fees: trustlines, escrows,
  NFT records, and asset definitions.
- Every new transaction kind must be visible through `tx`, `account_tx`, block
  archive, receipts, Python RPC, and operator diagnostics.
- Every object id must be deterministic, domain-separated, and replayable from
  genesis.
- No unbounded memo strings, metadata blobs, asset codes, issuer fields, or
  trustline fanout.
- Public write RPC remains gated. Read RPC should expose these objects cleanly.

## Acceptance Model

Each feature phase is complete only when all five gates pass:

1. Protocol gate: canonical transaction/state types exist, signing bytes are
   deterministic, validation rejects malformed input before mempool admission,
   and replay from archived blocks reconstructs the same state.
2. Ledger gate: balances, object ownership, reserves, fees, sequence handling,
   and object indexes are correct across success, rejection, restart, and
   re-application paths.
3. RPC gate: read RPC exposes the object, write/build RPC is gated where
   appropriate, response validation exists, and `tx` plus `account_tx` show the
   feature without requiring raw archive spelunking.
4. Wallet/SDK gate: Python helpers can run the normal user flow without manual
   JSON assembly.
5. Evidence gate: focused tests pass, one deterministic vector is recorded for
   the feature, and a controlled local or live multi-validator smoke report is
   written under `reports/xrpl-feature-parity-*`.

## Python Wallet UX Layer

Status: complete. Canonical Python helpers remain the stable protocol-shaped
surface, and the new XRPL-style aliases are thin wrappers for integration UX.
They cover native PFT payments with memos, issued-token definition/trustline
flows, issuer controls and clawback, native or issued-token escrow,
NFToken-style mint/transfer/burn, atomic swap template construction, and DEX
offer placement.

Evidence:

- `reports/xrpl-feature-parity-python-wallet-helpers/xrpl-py-style-helpers-20260521T130542Z/python-xrpl-style-helpers.json`.
  Checks: `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `python3 -m compileall -q python/postfiat_rpc`, `git diff --check`, and
  `PYTHONPATH=python python3 -m unittest discover -s python/tests -q`; export
  check imported the XRPL-style helpers from `postfiat_rpc`.

## Phase 1: Payment Rails And Memos

Goal: make normal payments usable for institutions and wallet tooling.

Add a versioned payment transaction that preserves the current PFT send path and
adds bounded payment metadata.

P0 tasks:

- `PAY-001`: complete. Defined `PaymentV2` protocol types with native
  PFT payment fields: `from`, `to`, `amount`, `fee`, `sequence`, and optional
  memo fields.
- `PAY-002`: complete. Defined bounded `memo_type`, `memo_format`, and
  `memo_data` fields, each byte-limited and total byte-capped.
- `PAY-003`: complete. Account history indexes memo payments with
  `memo_hash`, `memo_count`, and `memo_bytes` instead of storing unbounded memo
  payloads in account rows.
- `PAY-004`: complete. Fee quote accepts memo fields, selects `PaymentV2`
  weight when memos are present, and reports bounded memo counts/bytes.
- `PAY-005`: complete. `PaymentV2` mempool validation, replay-aware dry runs,
  mixed transparent batch construction, and deterministic batch references are
  wired.
- `PAY-006`: complete. `account_tx`, `tx`, receipts, block archive replay, RPC
  SDK validation, and Python client/wallet helpers cover memo payments.
- `PAY-007`: complete. Protocol vectors for signed payments with and without
  memos exist, and a controlled four-validator smoke finalizes a memo payment
  through mempool admission, batch sealing, replay-verified `tx`, receipts, and
  `account_tx` on every validator.

Acceptance:

- `cargo test -p postfiat-types` covers `PaymentV2` canonical signing bytes,
  memo byte caps, and transaction id stability;
- `cargo test -p postfiat-node` covers execution, mempool admission, fee quote,
  archive replay, `tx`, and `account_tx` for memo payments;
- invalid memo lengths, malformed memo encodings, zero amounts, bad sequence,
  insufficient fee, and insufficient balance are rejected before commit;
- legacy PFT transfer still works and keeps its existing signed format;
- `account_tx` returns payment rows with memo hash/reference and bounded memo
  summary fields;
- Python can send PFT with an optional memo and retrieve the finalized payment
  by account history;
- controlled multi-validator smoke finalizes at least one memo payment and
  writes `reports/xrpl-feature-parity-payment-*/`.

Evidence:

- `PAY-001`/`PAY-002` protocol vectors and validation:
  `reports/xrpl-feature-parity-payment-v2-protocol/pay-001-002-v0-20260520T133406Z/payment-v2-protocol.json`.
  Checks: `cargo test -p postfiat-types`,
  `cargo test -p postfiat-execution`, and `cargo test -p postfiat-node`.
- `PAY-004`/`PAY-005` execution primitives:
  `reports/xrpl-feature-parity-payment-v2-execution/pay-004-005-execution-v0-20260520T133812Z/payment-v2-execution.json`.
  Checks: `cargo test -p postfiat-execution` and
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow`.
- `PAY-003`/`PAY-006` mempool, RPC, account history, and Python wallet/client
  wiring:
  `reports/xrpl-feature-parity-payment-v2-mempool/pay-003-006-mempool-rpc-account-history-v0-20260520T141734Z/payment-v2-mempool-rpc-account-history.json`.
  Checks: `cargo test -p postfiat-types`,
  `cargo test -p postfiat-mempool-dag`, `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-node --lib payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow`,
  `cargo test -p postfiat-node --lib`, `cargo test -p postfiat-node --bin postfiat-node`,
  and `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`.
- `PAY-007` controlled multi-validator memo-payment smoke:
  `reports/xrpl-feature-parity-payment-v2-multivalidator/pay-007-payment-v2-multivalidator-20260520T143256Z/payment-v2-multivalidator-smoke.json`.
  Checks: `bash -n scripts/xrpl-feature-parity-payment-v2-multivalidator-smoke`,
  `scripts/xrpl-feature-parity-payment-v2-multivalidator-smoke`, report
  validation with `jq`, and public artifact scan for private key material.

Next work:

- Payment/memos P0 is complete; no open payment feature-parity task remains in
  this burndown.

## Phase 2: Issued Assets And Trustlines

Goal: support XRP-style asset issuance without pretending every token is native
PFT.

Add ledger-native issued assets with explicit trustlines. Native PFT remains the
reserve/fee asset. Issued assets are balances between an issuer and accounts
that opted in through trustlines.

Core state:

- `AssetDefinition`: issuer address, asset code, asset id, precision/display
  metadata, supply policy, issuer flags.
- `TrustLine`: account, issuer, asset id, limit, balance, authorization state,
  freeze state, reserve paid.
- `IssuerState`: issuer controls, outstanding supply, optional authorization
  policy, optional freeze/clawback policy.

P0 tasks:

- `ASSET-001`: complete. Added deterministic `IssuedAssetId` using the
  `postfiat.issued_asset_id.v1` domain, chain id, issuer, asset code, and asset
  version.
- `ASSET-002`: complete. Added ledger state for asset definitions and
  trustlines with deterministic trustline ids, validation, duplicate rejection,
  missing-asset rejection, and legacy empty-ledger serialization preservation.
- `ASSET-003`: complete. Added canonical signed asset transaction envelopes for
  `asset_create`, `trust_set`, `issued_payment`, and `asset_burn`, including
  source/kind validation, deterministic signing bytes, signed preimage bytes,
  and legacy-safe `TransactionBatch.asset_transactions` serialization.
- `ASSET-004`: complete. Added deterministic execution for `asset_create`,
  `trust_set`, `issued_payment`, and `asset_burn`; enforced trustline limits,
  issuer authorization, frozen-line rejection, balance/supply overflow checks,
  state-expansion reserve fees, receipt ids, replay reconstruction, batch
  references, and `account_tx` rows for asset transactions.
- `ASSET-005`: complete. Added fee quote support for state-expanding
  `asset_create` and `trust_set` operations; persisted signed asset
  transactions in mempool state; added replay-aware mempool dry-runs, sequence
  accounting, mixed-batch sealing, RPC/CLI/SDK request validation, and Python
  client quote/submit helpers for signed asset transaction JSON.
- `ASSET-006`: complete. Added deterministic read RPC/CLI/SDK/Python surfaces
  for `asset_info`, `account_lines`, `account_assets`, and `issuer_assets`;
  issued-asset `account_tx` rows now carry `asset_id` and `issuer`.
- `ASSET-007`: complete. Added Rust SDK-backed Python wallet helpers for
  asset creation, trustline creation, issued payments, signed asset transaction
  submission, optional local finalization, and deterministic asset id return
  values.
- `ASSET-008`: complete. Added execution-level deterministic property-style
  conservation/trustline invariant tests and node-level replay/RPC/account
  history tests proving issued-asset supply equals trustline balances after
  issue, payment, burn, rejection, and archived block replay paths.

P1 tasks:

- `ASSET-009`: complete. Exposed issuer freeze/unfreeze and authorization
  controls through issuer-signed `trust_set`, account_tx
  `trustline_authorized`/`trustline_frozen` fields, RPC SDK validation, and
  Python wallet helpers that preserve existing holder limit/reserve terms.
- `ASSET-010`: complete. Added optional issuer-declared `asset_clawback`
  policy for issued assets only; native PFT clawback is rejected by issued
  asset id validation, execution requires `clawback_enabled`, account history
  indexes the owner-to-issuer debit, RPC SDK validation accepts the new kind,
  and Python wallet helpers can quote/sign/submit issuer clawback.
- `ASSET-011`: complete. Added deterministic issued-asset metrics under
  `metrics.assets`, including asset/trustline/holder counts, total outstanding
  issued supply, open issued escrow/offer locked totals, issuer policy counts,
  and authorization/freeze counters; fixed `asset_info`/`issuer_assets` supply
  accounting for open issued sell offers; exposed the counters in RPC SDK
  validation, testnet RPC doctor summaries, monitor snapshot output, and Python
  RPC smoke checks.

Acceptance:

- `cargo test -p postfiat-types` covers deterministic asset ids, trustline ids,
  and canonical signing bytes for asset operations;
- `cargo test -p postfiat-node` covers create asset, trust set, issued payment,
  burn/redeem, fee quote, archive replay, and indexed account history;
- issuer can create an asset with bounded code/metadata and declared policy;
- account can create a trustline with a limit and reserve;
- issuer/account can send issued assets within trustline limits;
- impossible states are rejected: supply overflow, balance overflow, missing
  trustline, missing issuer authorization, frozen line movement, reserve
  shortfall, and native-PFT clawback;
- `asset_info`, `account_lines`, `account_assets`, and `issuer_assets` return
  deterministic results;
- Python can run create asset -> trustline -> issued payment -> account history;
- controlled multi-validator smoke writes `reports/xrpl-feature-parity-assets-*/`.

Evidence:

- `ASSET-001`/`ASSET-002` deterministic state vectors:
  `reports/xrpl-feature-parity-assets/asset-001-002-state-v0-20260520T143600Z/asset-state-vectors.json`.
  Checks: `cargo test -p postfiat-types`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-storage`, and
  `cargo test -p postfiat-node --lib payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx`.
- `ASSET-003` protocol transaction vectors:
  `reports/xrpl-feature-parity-assets/asset-003-protocol-v0-20260520T144533Z/asset-003-protocol.json`.
  Checks: `cargo test -p postfiat-types`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-storage`, and
  `cargo test -p postfiat-node --lib payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx`.
- `ASSET-004` execution/replay/account-history vectors:
  `reports/xrpl-feature-parity-assets/asset-004-execution-v0-20260520T150109Z/asset-004-execution.json`.
  Checks: `cargo test -p postfiat-types`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-mempool-dag`,
  `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-node --lib payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx`,
  and `git diff --check`.
- `ASSET-005` asset fee quote, mempool admission, RPC/SDK/Python request
  surfaces, and mixed-batch sealing:
  `reports/xrpl-feature-parity-assets/asset-005-mempool-fee-v0-20260520T153054Z/asset-005-mempool-fee.json`.
  Checks: `cargo test -p postfiat-node --lib asset_fee_quote_mempool_batch_and_replay_flow`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-mempool-dag`,
  `cargo test -p postfiat-node --lib payment_v2_memo_flows_through_mempool_batch_finality_and_account_tx`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow`,
  `cargo test -p postfiat-node --bin postfiat-node`, and
  `cargo test -p postfiat-node --lib`, `git diff --check`, and report
  validation with `jq`.
- `ASSET-006` deterministic read RPC/CLI/SDK/Python surfaces and
  issued-asset `account_tx` rows:
  `reports/xrpl-feature-parity-assets/asset-006-read-rpc-v0-20260520T160457Z/asset-006-read-rpc.json`.
  Checks: `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-types -p postfiat-storage -p postfiat-execution -p postfiat-mempool-dag -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `git diff --check`, and report validation with `jq`.
- `ASSET-007` Rust SDK-backed Python wallet helpers for issued asset create,
  trustline creation, issued payments, submit, and optional local finalization:
  `reports/xrpl-feature-parity-assets/asset-007-python-wallet-v0-20260520T162136Z/asset-007-python-wallet.json`.
  Checks: `cargo test -p postfiat-rpc-sdk wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib asset_fee_quote_mempool_batch_and_replay_flow`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `git diff --check`, and report validation with `jq`.
- `ASSET-008` deterministic replay/property-style conservation and trustline
  invariant coverage:
  `reports/xrpl-feature-parity-assets/asset-008-invariants-v0-20260520T163734Z/asset-008-invariants.json`.
  Checks: `cargo test -p postfiat-execution asset_transaction_property_conserves_supply_and_trustline_limits`,
  `cargo test -p postfiat-node --lib asset_replay_preserves_conservation_and_trustline_invariants`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-node --lib asset_fee_quote_mempool_batch_and_replay_flow`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-types`,
  `cargo test -p postfiat-node --lib`, `git diff --check`, and report
  validation with `jq`.
- `ASSET-009` issuer freeze/unfreeze and authorization control surfaces:
  `reports/xrpl-feature-parity-assets/asset-009-issuer-controls-v0-20260521T021951Z/asset-009-issuer-controls.json`.
  Checks: `cargo test -p postfiat-execution asset_transactions_create_trust_pay_burn_and_reject_frozen_lines -- --nocapture`,
  `cargo test -p postfiat-node --lib asset_fee_quote_mempool_batch_and_replay_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_asset_issuer_control_helpers_preserve_line_terms`,
  `cargo test -p postfiat-execution asset -- --nocapture`,
  `cargo test -p postfiat-node --lib asset -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk`, and
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-execution`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `ASSET-010` optional issuer-declared clawback policy, never native PFT:
  `reports/xrpl-feature-parity-assets/asset-010-clawback-v0-20260521T024617Z/asset-010-clawback.json`.
  Checks: `cargo test -p postfiat-types asset_transaction_validation_covers_all_asset_operations -- --nocapture`,
  `cargo test -p postfiat-execution asset_clawback_requires_issuer_policy_and_rejects_native_pft -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_asset_wallet_helpers_quote_sign_and_submit_operations python.tests.test_wallet.WalletHelperTests.test_asset_wallet_helpers_validate_bounds`,
  `cargo test -p postfiat-node --lib asset_fee_quote_mempool_batch_and_replay_flow -- --nocapture`,
  `cargo test -p postfiat-execution asset -- --nocapture`,
  `cargo test -p postfiat-node --lib asset -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.
- `ASSET-011` deterministic issued-asset metrics and monitor output:
  `reports/xrpl-feature-parity-assets/asset-011-metrics-v0-20260521T030849Z/asset-011-metrics.json`.
  Checks: `cargo test -p postfiat-node asset_fee_quote_mempool_batch_and_replay_flow -- --nocapture`,
  `cargo test -p postfiat-node issued_asset_escrow_fee_mempool_account_tx_and_replay_flow -- --nocapture`,
  `cargo test -p postfiat-node offer_create_matching_replay_and_maker_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-node init_then_run_once -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk health_response_validation_accepts_supported_results -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk health_response_validation_rejects_bad_schema_and_key_leaks -- --nocapture`,
  `python3 -m py_compile scripts/testnet-monitor-snapshot scripts/testnet-rpc-doctor`,
  `bash -n scripts/testnet-monitor-snapshot-smoke scripts/testnet-python-rpc-client-smoke`,
  `cargo test -p postfiat-node --lib`, `cargo test -p postfiat-rpc-sdk --lib`,
  `cargo test -p postfiat-node --bins --no-run`,
  `PYTHONPATH=python python3 -m unittest discover -s python/tests -q`,
  `python3 -m compileall -q python/postfiat_rpc`,
  `scripts/testnet-monitor-snapshot-smoke`,
  `scripts/testnet-python-rpc-client-smoke`, and report validation with `jq`.

Next work:

- Issued-assets/trustlines P0 and P1 items in this burndown are complete
  through `ASSET-011`. No open issued-assets/trustlines feature-parity task
  remains here.

## Phase 3: Escrow And Atomic Settlement

Goal: support conditional settlement and basic atomic flows before any DEX.

Start with native PFT escrow, then extend to issued assets once Phase 2 is
stable.

Core state:

- `Escrow`: escrow id, owner, recipient, asset, amount, fee, condition,
  cancel_after, finish_after, state, created height.
- Condition types: time lock and hash lock first.

P0 tasks:

- `ESCROW-001`: complete. Defined deterministic escrow ids and canonical
  ledger escrow state with bounded condition metadata, timing/state validation,
  duplicate rejection, lookup helpers, and legacy empty-ledger serialization
  preservation.
- `ESCROW-002`: complete. Added canonical signed transaction envelopes for
  `escrow_create`, `escrow_finish`, and `escrow_cancel`, including operation
  validation, source authorization, signed preimage bytes, legacy-safe batch
  and mempool serialization, and escrow-aware deterministic batch references.
- `ESCROW-003`: complete. Added deterministic native PFT escrow execution for
  create, finish, and cancel; enforced locked-balance accounting, fee and
  state-expansion fee policy, finish/cancel height gates, fulfillment matching,
  failed-attempt immutability, and height-aware transparent batch replay.
- `ESCROW-004`: complete. Added deterministic derived escrow indexes by owner,
  recipient, condition hash, and cancel-after expiry height without duplicating
  consensus state.
- `ESCROW-005`: complete. Exposed deterministic read RPC/CLI/SDK/Python client
  surfaces for `escrow_info` and `account_escrows`, and added escrow rows with
  deterministic `escrow_id` and `condition_hash` fields to `account_tx`.
- `ESCROW-006`: complete. Added Python helpers for native PFT escrow create,
  finish, and cancel, backed by Rust SDK signing, escrow fee quote, signed
  escrow mempool admission, and optional local finalization.
- `ESCROW-007`: complete. Added restart/replay tests proving native PFT escrow
  locked funds cannot be double-spent or stranded by rejected replay order,
  finish, cancel, and restart boundaries.

P1 tasks:

- `ESCROW-008`: complete. Extended escrow to issued assets with deterministic
  trustline locking, finish/cancel release, fee quote and mempool admission,
  replay/account-history/read-RPC coverage, max-supply accounting for open
  issued escrows, and Python wallet create helper support.
- `ESCROW-009`: complete. Added deterministic two-sided atomic settlement
  templates for PFT/issued-asset swaps, including reciprocal leg validation,
  exactly-one-PFT asset-pair validation, shared-condition validation, symmetric
  settlement ids, derived escrow-create operations, RPC/CLI/SDK/Python builder
  surfaces, account-history checks, and replay coverage.

Acceptance:

- `cargo test -p postfiat-types` covers escrow ids and canonical signing bytes;
- `cargo test -p postfiat-node` covers create, finish, cancel, expiry, replay,
  restart, fee quote, and indexed account history;
- escrow creation locks funds and prevents double-spend;
- finish releases only when the condition is satisfied;
- cancel releases only after cancellation conditions are met;
- failed finish/cancel attempts do not mutate state;
- `escrow_info` and `account_escrows` return deterministic results;
- Python can create, finish, and cancel PFT escrows, create issued-asset
  escrows, and build two-sided PFT/issued-asset settlement templates;
- controlled multi-validator smoke writes `reports/xrpl-feature-parity-escrow-*/`.

Evidence:

- `ESCROW-001` deterministic escrow id and canonical ledger state:
  `reports/xrpl-feature-parity-escrow/escrow-001-state-v0-20260520T164355Z/escrow-001-state.json`.
  Checks: `cargo test -p postfiat-types escrow`,
  `cargo test -p postfiat-types ledger_state_preserves_legacy_empty_serialization`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-node --lib asset_replay_preserves_conservation_and_trustline_invariants`,
  `cargo test -p postfiat-node --bin postfiat-node`, `git diff --check`,
  and report validation with `jq`.
- `ESCROW-002` signed escrow transaction envelopes and deterministic batch
  references:
  `reports/xrpl-feature-parity-escrow/escrow-002-protocol-v0-20260520T165255Z/escrow-002-protocol.json`.
  Checks: `cargo test -p postfiat-types escrow_transaction`,
  `cargo test -p postfiat-mempool-dag escrow_transactions_are_committed_to_batch_reference`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-mempool-dag`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-rpc-sdk`,
  `git diff --check`, and report validation with `jq`.
- `ESCROW-003` deterministic native PFT escrow execution, locked-balance
  accounting, fee/reserve policy, and height-aware transparent replay:
  `reports/xrpl-feature-parity-escrow/escrow-003-execution-v0-20260520T171229Z/escrow-003-execution.json`.
  Checks: `cargo test -p postfiat-execution escrow_transactions_lock_finish_cancel_and_reject_replay`,
  `cargo test -p postfiat-node --lib escrow_create_applies_from_batch_and_replays`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `cargo test -p postfiat-types escrow`, `git diff --check`, and report
  validation with `jq`.
- `ESCROW-004` deterministic derived owner, recipient, condition-hash, and
  expiry indexes:
  `reports/xrpl-feature-parity-escrow/escrow-004-indexes-v0-20260520T171734Z/escrow-004-indexes.json`.
  Checks: `cargo test -p postfiat-types escrow_indexes_group_deterministically_by_owner_recipient_condition_and_expiry`,
  `cargo test -p postfiat-node --lib escrow_create_applies_from_batch_and_replays`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-node --bin postfiat-node`, `git diff --check`,
  and report validation with `jq`.
- `ESCROW-005` deterministic read RPC/CLI/SDK/Python client surfaces and
  escrow `account_tx` rows:
  `reports/xrpl-feature-parity-escrow/escrow-005-rpc-account-history-v0-20260520T174255Z/escrow-005-rpc-account-history.json`.
  Checks: `cargo test -p postfiat-node --lib escrow_create_applies_from_batch_and_replays`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-node --lib asset_transactions_apply_from_batch_replay_and_account_tx`,
  `cargo test -p postfiat-node --bin postfiat-node`, and
  `cargo test -p postfiat-node --lib`.
- `ESCROW-006` Rust SDK-backed Python wallet helpers for native PFT escrow
  create, finish, cancel, submit, and optional local finalization:
  `reports/xrpl-feature-parity-escrow/escrow-006-python-wallet-v0-20260520T181419Z/escrow-006-python-wallet.json`.
  Checks: `cargo fmt`, `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib escrow_`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `cargo test -p postfiat-node --lib`, `git diff --check`, and report
  validation with `jq`.
- `ESCROW-007` restart/replay locked-funds invariant coverage:
  `reports/xrpl-feature-parity-escrow/escrow-007-restart-replay-v0-20260520T183233Z/escrow-007-restart-replay.json`.
  Checks: `cargo fmt`,
  `cargo test -p postfiat-node --lib escrow_restart_replay_preserves_locked_funds_and_release_edges`,
  `cargo test -p postfiat-node --lib escrow_`,
  `cargo test -p postfiat-execution escrow_transactions_lock_finish_cancel_and_reject_replay`,
  `cargo test -p postfiat-node --lib`, `git diff --check`, and report
  validation with `jq`.
- `ESCROW-008` issued-asset escrow execution, supply accounting, mempool,
  account history, read RPC, SDK validation, and Python wallet helper coverage:
  `reports/xrpl-feature-parity-escrow/escrow-008-issued-assets-v0-20260520T190401Z/escrow-008-issued-assets.json`.
  Checks:
  `cargo test -p postfiat-execution issued_asset_escrow_locks_finishes_cancels_and_counts_locked_supply`,
  `cargo test -p postfiat-types escrow_transaction_validation_covers_all_operations`,
  `cargo test -p postfiat-node --lib issued_asset_escrow_fee_mempool_account_tx_and_replay_flow`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_escrow_wallet_helpers_quote_sign_and_submit_operations python.tests.test_wallet.WalletHelperTests.test_asset_wallet_helpers_validate_bounds`,
  `cargo test -p postfiat-node --lib escrow`,
  `cargo test -p postfiat-node --lib asset_replay_preserves_conservation_and_trustline_invariants`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-types`,
  `cargo test -p postfiat-rpc-sdk`, `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`, and
  `git diff --check`.
- `ESCROW-009` deterministic PFT/issued-asset atomic settlement templates,
  generated escrow-create operations, RPC/CLI/SDK/Python builder surfaces,
  account-history checks, and replay coverage:
  `reports/xrpl-feature-parity-escrow/escrow-009-atomic-template-v0-20260520T193720Z/escrow-009-atomic-template.json`.
  Checks: `cargo test -p postfiat-types atomic_settlement`,
  `cargo test -p postfiat-types atomic_settlement_template_id_is_symmetric`,
  `cargo test -p postfiat-types`,
  `cargo test -p postfiat-rpc-sdk typed_request_builders_emit_object_params`,
  `cargo test -p postfiat-rpc-sdk request_validation_accepts_supported_kinds`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results`,
  `cargo test -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-node --lib atomic_settlement_template_builds_pft_issued_swap_through_escrow_rails`,
  `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `cargo test -p postfiat-execution`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_atomic_settlement_template_sends_swap_params python.tests.test_wallet.WalletHelperTests.test_atomic_settlement_wallet_helper_builds_template`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.

Next work:

- Escrow and atomic-settlement P0/P1 scope in this burndown is complete; no
  open escrow feature-parity task remains here.

## Phase 4: NFTs

Goal: add ledger-native ownership records, not a marketplace.

NFTs should be simple objects with deterministic ids, bounded metadata, and
account history/indexing. Marketplace, royalties, and auctions are not P0.

Core state:

- `NftDefinition`: collection id, issuer, serial, owner, metadata hash,
  metadata URI, flags, collection policy flags, optional issuer transfer fee,
  burned state.
- `NftId = H(chain domain, issuer, collection, serial)`.

P0 tasks:

- `NFT-001`: complete. Defined deterministic NFT ids using the
  `postfiat.nft_id.v1` domain, bounded collection/metadata fields, supported
  flag mask validation, burned-state ownership records, ledger NFT storage,
  duplicate/malformed state rejection, legacy empty-ledger serialization
  preservation, and derived owner/issuer/collection indexes.
- `NFT-002`: complete. Added signed `nft_mint`, `nft_transfer`, and
  `nft_burn` transaction envelopes with canonical signing bytes, deterministic
  transaction ids, legacy-safe batch/mempool serialization, NFT-aware batch
  references, mempool accounting hooks, storage append support, and
  deterministic rejected receipts for manually supplied NFT batches before
  `NFT-003` execution rules landed.
- `NFT-003`: complete. Enforced issuer/source rules, owner authorization,
  metadata bounds during deterministic mint execution, duplicate mint
  rejection, burned/non-transferable rejection, reserve preservation, NFT
  state-expansion fee policy, mempool admission, fee quote, RPC/CLI/SDK signed
  NFT transaction submission, Python client quote/submit paths, and replay
  reconstruction.
- `NFT-004`: complete. Added deterministic read RPC/CLI/SDK/Python surfaces for
  `nft_info`, `account_nfts`, and `issuer_nfts`, including bounded limits,
  burned-state filtering, issuer collection filters, and response validation.
- `NFT-005`: complete. Added NFT rows to `account_tx` and account_tx index
  replay for `nft_mint`, `nft_transfer`, and `nft_burn`; rows include
  deterministic `nft_id`, issuer on mint rows, receipt status, fees, and stable
  mixed-batch transaction indexes. Python client server parsing and archive
  fallback decode NFT account history rows.
- `NFT-006`: complete. Added Rust SDK-backed Python wallet helpers for NFT
  mint, transfer, and burn; helpers quote fees, sign bounded quote responses,
  submit signed NFT transactions, optionally finalize local batches, validate
  bounded NFT fields, and return deterministic NFT ids for mint/lifecycle
  tracking.

P1 tasks:

- `NFT-007`: complete. Added optional fixed native-PFT issuer transfer fee for
  NFTs, stored immutably at mint, normalized into transfer fee quotes, committed
  by signed transfer operations, enforced during deterministic execution, and
  surfaced through NFT read RPC, receipts, account_tx rows, RPC SDK validation,
  Python client parsing, and Python wallet mint helpers without adding
  order-book or sale-price semantics.
- `NFT-008`: complete. Added collection-level policy flags declared at mint,
  stored on NFT ledger objects, validated consistently across issuer
  collections during replay, enforced for collection transfer and burn locks,
  surfaced through NFT read RPC, mint receipts, account_tx rows, RPC SDK
  validation, Python account-history parsing, and Python wallet mint helpers.

Acceptance:

- `cargo test -p postfiat-types` covers NFT ids, metadata bounds, and canonical
  signing bytes;
- `cargo test -p postfiat-node` covers mint, transfer, burn, fee quote, replay,
  restart, and indexed account history;
- NFT mint, transfer, and burn replay deterministically;
- account, issuer, and collection indexes are correct after restart;
- metadata cannot become an unbounded storage vector;
- unauthorized transfer and duplicate mint are rejected;
- Python can run the full NFT lifecycle;
- controlled multi-validator smoke writes `reports/xrpl-feature-parity-nft-*/`.

Evidence:

- `NFT-001` deterministic NFT id, bounded metadata, ownership state, duplicate
  rejection, legacy serialization, and derived owner/issuer/collection indexes:
  `reports/xrpl-feature-parity-nft/nft-001-state-v0-20260520T195049Z/nft-001-state.json`.
  Checks: `cargo test -p postfiat-types nft`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-node --bin postfiat-node`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `NFT-002` signed mint/transfer/burn envelopes, canonical preimage vectors,
  NFT-aware batch/mempool serialization, deterministic batch commitments, and
  pre-execution rejected receipt coverage:
  `reports/xrpl-feature-parity-nft/nft-002-protocol-v0-20260520T201238Z/nft-002-protocol.json`.
  Checks: `cargo fmt`,
  `cargo test -p postfiat-types nft_transaction`,
  `cargo test -p postfiat-mempool-dag nft_transactions_are_committed_to_batch_reference`,
  `cargo test -p postfiat-execution nft_transaction`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-mempool-dag`,
  `cargo test -p postfiat-storage`, `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow`,
  `cargo test -p postfiat-node --bin postfiat-node`,
  `cargo test -p postfiat-node --lib nft`,
  `cargo test -p postfiat-node --lib`, `cargo test -p postfiat-rpc-sdk`,
  and `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`.
- `NFT-003` deterministic NFT mint/transfer/burn execution, authorization,
  reserve/fee policy, mempool admission, NFT-only batch replay, account_tx
  `nft_id` rows, RPC SDK validation, and Python client quote/submit paths:
  `reports/xrpl-feature-parity-nft/nft-003-execution-v0-20260520T205010Z/nft-003-execution.json`.
  Checks: `cargo test -p postfiat-types nft -- --nocapture`,
  `cargo test -p postfiat-execution nft_transaction -- --nocapture`,
  `cargo test -p postfiat-node --lib nft_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_nft_fee_quote_and_submit_send_json_params`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-node --lib`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `NFT-004` deterministic NFT read RPC, served/local CLI routing, SDK request
  and response validation, Python client/CLI methods, and mint/transfer/burn
  read-index assertions:
  `reports/xrpl-feature-parity-nft/nft-004-read-rpc-v0-20260520T211513Z/nft-004-read-rpc.json`.
  Checks:
  `cargo test -p postfiat-node --lib nft_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk typed_request_builders_emit_object_params -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk request_validation_accepts_supported_kinds -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_asset_read_methods_send_bounded_params`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `NFT-006` Python NFT lifecycle wallet helpers and Rust SDK NFT quote signer:
  `reports/xrpl-feature-parity-nft/nft-006-python-wallet-v0-20260520T212747Z/nft-006-python-wallet.json`.
  Checks:
  `cargo test -p postfiat-rpc-sdk wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_nft_wallet_helpers_quote_sign_and_submit_operations`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.
- `NFT-007` deterministic NFT issuer transfer fee:
  `reports/xrpl-feature-parity-nft/nft-007-issuer-transfer-fee-v0-20260521T034422Z/nft-007-issuer-transfer-fee.json`.
  Checks: `cargo test -p postfiat-types nft -- --nocapture`,
  `cargo test -p postfiat-execution nft_transaction -- --nocapture`,
  `cargo test -p postfiat-node --lib nft_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_nft_wallet_helpers_quote_sign_and_submit_operations`,
  `cargo test -p postfiat-types -p postfiat-execution -p postfiat-mempool-dag -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib`, `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.
- `NFT-008` deterministic NFT collection-level policy flags:
  `reports/xrpl-feature-parity-nft/nft-008-collection-policy-flags-v0-20260521T042047Z/nft-008-collection-policy-flags.json`.
  Checks: `cargo test -p postfiat-types nft -- --nocapture`,
  `cargo test -p postfiat-execution nft_transaction -- --nocapture`,
  `cargo test -p postfiat-node --lib nft_collection_policy_flags_flow_through_rpc_account_tx_and_mempool -- --nocapture`,
  `cargo test -p postfiat-node --lib nft_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_nft_wallet_helpers_quote_sign_and_submit_operations`,
  `cargo test -p postfiat-types -p postfiat-execution -p postfiat-mempool-dag -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.

Next work:

- NFT P0 is complete and NFT P1 work is complete through `NFT-008`; no open NFT
  feature-parity task remains in this burndown.

## Phase 5: DEX Decision

Goal: make the DEX call after the ledger primitives are real.

A native DEX is not rejected. It is deferred until issued assets, trustlines,
escrow, account history, and wallet/RPC support exist.

DEX questions to answer before implementation:

- What ordering fairness is acceptable for offers and cross-asset settlement?
- Does the controlled testnet need an order book, or only atomic settlement?
- Are offers ledger objects with reserves?
- How are partial fills represented in receipts and account history?
- How do fees work for offer creation, cancellation, and execution?
- Does privacy interact with issued assets or offers in v1?

P0 design gates before DEX implementation:

- `DEX-001`: complete. Wrote the DEX v1 design gate selecting a transparent
  PFT/issued-asset limit-order book, keeping atomic settlement as a separate
  bilateral toolkit, and specifying offer state, reserves/locked funds,
  matching order, partial fills, fees, receipts, account history, RPC/SDK/Python
  surfaces, mempool/replay invariants, and controlled-testnet MEV posture.
- `DEX-002`: complete. Added a deterministic local model for integer
  limit-order crossing under the proposed max-cross cap. The model covers book
  sizes up to 4096 offers, crossing caps of 16/64/128, exact integer partial
  fills, canonical price/height/sequence/id ordering, and recommends an initial
  `MAX_DEX_CROSSES_PER_TRANSACTION = 64` cap before a Rust execution prototype.
- `DEX-003`: complete. Defined the controlled-testnet DEX ordering and MEV
  policy: finalized batch order controls cross-transaction priority,
  deterministic book order controls fills inside one transaction, fees cannot
  buy priority, DEX writes stay behind controlled write edges, and no public
  fairness/MEV-resistance claims are allowed for this v1 posture.

P0 implementation tasks:

- `DEX-004`: complete. Added protocol/state types for deterministic offer ids,
  offer ledger objects, offer indexes, signed `offer_create`/`offer_cancel`
  transaction envelopes, canonical signing bytes, transaction ids,
  legacy-safe batch/mempool serialization, and offer-aware mempool DAG batch
  commitments.
- `DEX-005`: complete. Added deterministic offer create/cancel execution without
  crossing, including offer reserves, locked sell-side balances, issued-asset
  buy-side capacity checks, fee quotes, mempool dry-runs, replay
  reconstruction, account history rows, RPC/CLI routing, Rust SDK validation,
  and Python client quote/submit helpers.
- `DEX-006`: complete. Added bounded matching/fill execution for
  `offer_create`, including deterministic best-price maker selection,
  max-cross caps, integer partial fills, residual taker offers, stable fill
  receipts, per-cross match fees, maker/taker account history rows, RPC SDK
  receipt/fee validation, Python account history parsing, and deterministic
  maker/taker replay.
- `DEX-007`: complete. Added DEX read RPC and remaining Python wallet helpers
  for offer inspection and cancel ergonomics after matching went live,
  including `offer_info`, `account_offers`, and `book_offers` node/RPC
  surfaces; SDK request/response validation; CLI/rpc-serve routing; Rust SDK
  offer signing from fee quotes; and Python client/wallet create/cancel helpers.
- `DEX-008`: complete. Added explicit native-PFT and issued-asset conservation
  assertions for DEX offer create, match, partial fill, cancel, and rejection
  paths; extended node replay tests with offer conservation checks; and added a
  four-validator controlled smoke covering asset setup, offer create, full
  match, partial fill with residual offer, cancel, rejected filled-offer cancel,
  account history, state verification, and validator convergence.

Acceptance before any DEX build starts:

- issued assets/trustlines are implemented and have controlled-validator
  evidence;
- escrow and atomic settlement templates are implemented and have evidence;
- NFTs are implemented, indexed, and wallet/RPC accessible with evidence;
- the DEX design specifies offer object state, reserves, matching order,
  partial-fill receipts, fee accounting, and account history rows;
- the DEX design includes a concrete ordering-fairness/MEV position;
- the implementation plan explicitly states whether v1 is an order book, atomic
  swap toolkit, or both.

Evidence:

- `DEX-001` DEX v1 design gate:
  `reports/xrpl-feature-parity-dex/dex-001-design-v0-20260520T213148Z/dex-001-design.json`.
  Design:
  `docs/specs/xrpl-dex-v1-design.md`.
  Checks: `test -s docs/specs/xrpl-dex-v1-design.md`,
  `rg -n "V1 Decision|Offer Object|Reserves And Locked Funds|Matching Order|Partial Fills|Fee Accounting|Account History|Ordering Fairness" docs/specs/xrpl-dex-v1-design.md`,
  `git diff --check`, and report validation with `jq`.
- `DEX-002` deterministic offer matching model and max-cross recommendation:
  `reports/xrpl-feature-parity-dex/dex-002-matching-model-v0-20260520T213410Z/dex-002-matching-model.json`.
  Script:
  `scripts/xrpl-dex-offer-matching-model`.
  Checks: `scripts/xrpl-dex-offer-matching-model --repetitions 200 --output reports/xrpl-feature-parity-dex/dex-002-matching-model-v0-20260520T213410Z/dex-002-matching-model.json`,
  report validation with `jq`, `python3 -m py_compile scripts/xrpl-dex-offer-matching-model`,
  and `git diff --check`.
- `DEX-003` controlled-testnet ordering and MEV policy:
  `reports/xrpl-feature-parity-dex/dex-003-order-fairness-v0-20260520T213637Z/dex-003-order-fairness.json`.
  Policy:
  `docs/specs/xrpl-dex-order-fairness-policy.md`.
  Checks: `test -s docs/specs/xrpl-dex-order-fairness-policy.md`,
  `rg -n "Policy Decision|Threat Model|Consensus Rule|Controlled Write-Edge Rule|Fee Policy|Receipt And Audit Requirements|Public Claims Boundary|Implementation Consequences" docs/specs/xrpl-dex-order-fairness-policy.md`,
  `git diff --check`, and report validation with `jq`.
- `DEX-004` offer protocol/state types, signed transaction envelopes,
  legacy-safe batch/mempool serialization, execution tx-id helper, and
  offer-aware mempool DAG commitments:
  `reports/xrpl-feature-parity-dex/dex-004-protocol-state-v0-20260520T215028Z/dex-004-protocol-state.json`.
  Checks: `cargo test -p postfiat-types offer -- --nocapture`,
  `cargo test -p postfiat-mempool-dag offer_transactions_are_committed_to_batch_reference -- --nocapture`,
  `cargo test -p postfiat-execution offer_transaction_tx_id_is_domain_separated -- --nocapture`,
  `cargo test -p postfiat-types`, `cargo test -p postfiat-mempool-dag`,
  `cargo test -p postfiat-execution`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow -- --nocapture`,
  `cargo test -p postfiat-storage`, `cargo test -p postfiat-rpc-sdk`,
  `cargo fmt --check`, `git diff --check`, and report validation with `jq`.
- `DEX-005` deterministic offer create/cancel execution, offer reserves,
  locked balances, fee quotes, mempool dry-runs, replay reconstruction,
  account_tx rows, RPC/CLI routing, SDK validation, and Python client helpers:
  `reports/xrpl-feature-parity-dex/dex-005-execution-v0-20260520T223130Z/dex-005-execution.json`.
  Checks: `cargo test -p postfiat-rpc-sdk typed_request_builders_emit_object_params -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk request_validation_accepts_supported_kinds -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `cargo test -p postfiat-execution offer_create_rejection_does_not_lock_partial_reserve -- --nocapture`,
  `cargo test -p postfiat-execution offer -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet.WalletHelperTests.test_offer_fee_quote_and_submit_send_json_params`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-types offer -- --nocapture`,
  `cargo test -p postfiat-storage`,
  `cargo test -p postfiat-node --lib mempool_limits_reject_global_and_sender_overflow -- --nocapture`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-execution`, `cargo test -p postfiat-node --lib`,
  `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `DEX-006` bounded deterministic offer matching/fill execution, partial-fill
  receipts, max-cross capped maker selection, per-cross match fees, maker/taker
  account_tx rows, RPC SDK validation, Python account history parsing, and
  replay verification:
  `reports/xrpl-feature-parity-dex/dex-006-matching-v0-20260520T230227Z/dex-006-matching.json`.
  Checks: `cargo test -p postfiat-execution offer -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_create_matching_replay_and_maker_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-types offer -- --nocapture`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-node --lib`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `DEX-007` DEX read RPC and remaining Python wallet/helper ergonomics:
  `reports/xrpl-feature-parity-dex/dex-007-read-rpc-wallet-v0-20260520T233612Z/dex-007-read-rpc-wallet.json`.
  Checks: `cargo test -p postfiat-rpc-sdk typed_request_builders_emit_object_params -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk request_validation_accepts_supported_kinds -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk read_response_validation_accepts_supported_results -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_create_matching_replay_and_maker_account_tx_flow -- --nocapture`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-rpc-sdk`,
  `cargo test -p postfiat-node --bin postfiat-node --no-run`,
  `cargo test -p postfiat-node --lib`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.
- `DEX-008` conservation/property/replay tests and controlled-validator smoke:
  `reports/xrpl-feature-parity-dex/dex-008-conservation-replay-20260521T015618Z/dex-008-conservation-replay.json`.
  Smoke script:
  `scripts/xrpl-feature-parity-dex-008-controlled-validator-smoke`.
  Checks: `cargo test -p postfiat-execution offer_transactions_conserve_native_and_issued_assets_through_partial_fill_cancel_and_reject -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_create_matching_replay_and_maker_account_tx_flow -- --nocapture`,
  `cargo test -p postfiat-node --lib offer_fee_quote_mempool_batch_replay_and_account_tx_flow -- --nocapture`,
  `scripts/xrpl-feature-parity-dex-008-controlled-validator-smoke`,
  `cargo test -p postfiat-execution offer -- --nocapture`,
  `cargo test -p postfiat-node --lib offer -- --nocapture`,
  `cargo test -p postfiat-rpc-sdk`,
  `PYTHONPATH=python python3 -m unittest python.tests.test_wallet`,
  `cargo test -p postfiat-node --lib`, `cargo fmt --check`,
  `git diff --check`, and report validation with `jq`.

Next work:

- DEX P0 is complete through `DEX-008`; no open DEX feature-parity task remains
  in this burndown.

## Cross-Cutting Work

Every phase must update:

- protocol types;
- state transition execution;
- mempool admission;
- fee quote;
- block archive and deterministic replay;
- account history index;
- RPC request/response validation;
- Python client and wallet helpers;
- operator diagnostics and monitor snapshot;
- focused local tests;
- controlled validator evidence.

Evidence naming:

- WHIP prompt/automation guardrails:
  `reports/xrpl-feature-parity-whip-*`;
- payment/memos: `reports/xrpl-feature-parity-payment-*`;
- issued assets/trustlines: `reports/xrpl-feature-parity-assets-*`;
- escrow/atomic settlement: `reports/xrpl-feature-parity-escrow-*`;
- NFTs: `reports/xrpl-feature-parity-nft-*`;
- DEX design: `reports/xrpl-feature-parity-dex-*`.

Automation status:

- `WHIP-001`: active `l1` WHIP cron has been re-enabled with a one-line inline
  `message`; do not point it at a long `message_file`. Evidence:
  `reports/xrpl-feature-parity-whip-prompt-fix/one-line-prompt-v0-20260520T130744Z/whip-prompt-fix.json`.

## Immediate Implementation Order

The PAY, ASSET, ESCROW, NFT, and DEX P0 feature-parity slices tracked in this
burndown are complete through `DEX-008`, and the ordered asset P1 control
slices plus asset metrics are complete through `ASSET-011`. NFT P1 policy work
is complete through `NFT-008`; no open PAY/ASSET/ESCROW/NFT/DEX
feature-parity task remains in this burndown. Do not start unrelated privacy,
Cobalt, latency, storage-cleanup, public-claims, or general-docs work from this
burndown without a new feature-parity blocker or a new scoped mandate.

## Main Risks

- State growth from trustlines, escrows, NFTs, and metadata. Mitigation:
  reserves, bounded fields, and explicit state-expansion fees.
- Consensus divergence from ad hoc serialization. Mitigation: versioned
  transaction envelopes and deterministic signing bytes.
- Asset issuance bugs. Mitigation: conservation/property tests and replay tests.
- Memo and metadata abuse. Mitigation: strict byte caps, fee weighting, and hash
  indexing.
- Escrow stuck funds. Mitigation: explicit finish/cancel conditions and replay
  tests for every state edge.
- DEX MEV and fairness. Mitigation: do not implement DEX until the base rails
  and ordering policy are ready.
