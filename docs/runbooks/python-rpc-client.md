# Python RPC Client V0

Status: v1 read client plus canonical and XRPL-style wallet helpers
Date: 2026-05-21

PostFiat now has a small stdlib-only Python RPC client under
`python/postfiat_rpc/`. It is intended for integration users who want Python
tooling similar in shape to XRP client libraries. Transport stays in Python;
ML-DSA signing and Orchard/Halo2 action construction call the Rust node/SDK
binaries.

## Use

```bash
PYTHONPATH=$POSTFIAT_REPO/python \
python3 - <<'PY'
from postfiat_rpc import PostFiatRpcClient

client = PostFiatRpcClient("127.0.0.1:27650")
print(client.server_info())
print(client.fee())
print(client.account_tx("postfiat-address", from_height=0, limit=25).to_dict())
print(
    client.account_tx_history(
        "postfiat-address",
        from_height=0,
        window_size=25,
        limit_per_window=64,
    ).to_dict()
)
PY
```

For one-off operator reads without writing Python, use the stdlib CLI wrapper:

```bash
scripts/postfiat-rpc-query \
  --endpoint validator-0=127.0.0.1:27650 \
  --method account_tx_history \
  --address "$PUBLIC_CANARY_ADDRESS" \
  --from-height 0 \
  --window-size 25 \
  --limit-per-window 64 \
  --require-row
```

The CLI writes a JSON report under `reports/postfiat-rpc-query/` by default.
It supports the v0 read methods below, redacts non-local endpoint hosts in the
report, and fails closed when method-specific checks fail.

## Supported RPC Methods

- `status()`
- `server_info()`
- `ledger(limit=None)`
- `fee()`
- `transfer_fee_quote(from_address, to_address, amount, sequence=None)`
- `transfer_fee_quote_response(from_address, to_address, amount, sequence=None)`
- `mempool_submit_signed_transfer(signed_transfer)`
- `mempool_submit_signed_payment_v2(signed_payment_v2)`
- `asset_fee_quote(source, operation, sequence=None)`
- `asset_fee_quote_response(source, operation, sequence=None)`
- `escrow_fee_quote(source, operation, sequence=None)`
- `escrow_fee_quote_response(source, operation, sequence=None)`
- `nft_fee_quote(source, operation, sequence=None)`
- `nft_fee_quote_response(source, operation, sequence=None)`
- `offer_fee_quote(source, operation, sequence=None)`
- `offer_fee_quote_response(source, operation, sequence=None)`
- `mempool_submit_signed_asset_transaction(signed_asset_transaction)`
- `mempool_submit_signed_escrow_transaction(signed_escrow_transaction)`
- `mempool_submit_signed_nft_transaction(signed_nft_transaction)`
- `mempool_submit_signed_offer_transaction(signed_offer_transaction)`
- `asset_info(asset_id)`
- `account_lines(account, issuer=None, asset_id=None, limit=None)`
- `account_assets(account, asset_id=None, limit=None)`
- `issuer_assets(issuer, limit=None)`
- `escrow_info(escrow_id)`
- `account_escrows(account, role=None, state=None, limit=None)`
- `atomic_settlement_template(...)`
- `nft_info(nft_id)`
- `account_nfts(account, include_burned=False, limit=None)`
- `issuer_nfts(issuer, collection_id=None, include_burned=False, limit=None)`
- `offer_info(offer_id)`
- `account_offers(account, state=None, limit=None)`
- `book_offers(taker_gets_asset_id, taker_pays_asset_id, limit=None)`
- `shield_batch_orchard_deposit(deposit)`
- `validators()`
- `manifests()`
- `metrics()`
- `blocks(from_height=None, limit=None)`
- `receipts(tx_id=None, limit=None)`
- `tx(tx_id, audit_block_log=False)`
- `account(address)`
- `mempool_status()`
- `bridge_status()`
- `navcoin_bridge_routes()`
- `navcoin_bridge_packet(route_id, packet_hash)`
- `navcoin_bridge_claims(route_id, limit=None, include_terminal=False)`
- `navcoin_bridge_supply_status(route_id)`
- `navcoin_bridge_receipt_replay(route_id)`
- `shield_turnstile()`
- `orchard_pool_report()`
- `account_tx_index_status()`
- `batch_archive(batch_kind=None, batch_id=None, limit=None)`
- `account_tx(address, from_height=None, to_height=None, limit=None)`
- `account_tx_history(address, from_height=0, to_height=None,
  window_size=100, limit_per_window=512, max_windows=1000)`
- `owned_objects(owner_public_key_hex, asset=None, limit=None)`
- `wrap_owned(from_address, owner_public_key_hex, amount, asset="PFT")`
- `owned_sign(signed_order_json, validator_id)` where the JSON contains the
  order, owner public key, and owner signature; bare orders fail closed.
- `owned_apply(cert_json)`
- `owned_unwrap_sign(order_json, validator_id)`
- `owned_unwrap_apply(cert_json)`

## Wallet Helpers

Canonical helper functions:

- `create_wallet(chain_id, wallet_dir, account_index=0)`
- `request_faucet_pft(data_dir, to_address, amount, validator_data_dirs=None)`
- `send_pft(client, wallet, to_address, amount, memo_type=None,
  memo_format=None, memo_data=None)`
- `wrap_fastpay(client, wallet, amount, asset="PFT")`
- `send_fastpay(client, wallet, recipient_public_key_hex, amount, fee=1)`
- `unwrap_fastpay(client, wallet, amount=None, fee=0, object_id=None)`
- `create_issued_asset(client, wallet, code, precision, ...)`
- `create_asset_trustline(client, wallet, issuer, asset_id, limit, ...)`
- `send_issued_asset(client, wallet, to_address, issuer, asset_id, amount)`
- `authorize_asset_trustline(...)`, `freeze_asset_trustline(...)`,
  `unfreeze_asset_trustline(...)`, and
  `revoke_asset_trustline_authorization(...)`
- `clawback_issued_asset(client, wallet, owner, asset_id, amount)`
- `create_pft_escrow(...)`, `create_issued_asset_escrow(...)`,
  `finish_pft_escrow(...)`, `cancel_pft_escrow(...)`
- `build_atomic_settlement_template(...)`
- `mint_nft(...)`, `transfer_nft(...)`, `burn_nft(...)`
- `create_offer(...)`, `cancel_offer(...)`
- `create_orchard_wallet(wallet_dir, account_index=0)`
- `send_shielded_pft(data_dir, from_wallet, recipient, amount)`
- `scan_orchard_wallet(data_dir, wallet)`

`unwrap_fastpay` is the standard signed unwrap path. It does not call the
disabled unsigned `unwrap_owned` compatibility method. It selects FastPay owned
inputs up to the 2048 input cap, signs an `OwnedUnwrapOrder`, collects
validator votes through `owned_unwrap_sign`, and applies the certificate with
`owned_unwrap_apply`.

Example:

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    create_orchard_wallet,
    create_wallet,
    request_faucet_pft,
    send_pft,
    send_shielded_pft,
)

wallet = create_wallet(chain_id="postfiat-controlled-testnet", wallet_dir="wallets")
request_faucet_pft(data_dir="devnet/local/validator-0", to_address=wallet.address, amount=1000)

client = PostFiatRpcClient("127.0.0.1:27650")
sent = send_pft(client, wallet=wallet, to_address="pfrecipient...", amount=100)

orchard_wallet = create_orchard_wallet(wallet_dir="wallets")
shielded = send_shielded_pft(
    data_dir="devnet/local/validator-0",
    from_wallet=wallet,
    recipient=orchard_wallet,
    amount=25,
)
```

## XRPL-Style Helper UX

The package also exposes convenience wrappers that use names closer to
`xrpl-py` transaction flows while still calling the canonical quote, signing,
submit, receipt, and optional local-finalization helpers above.

- `send_payment(...)`: native PFT `Payment` with optional bounded memo fields.
- `mint_token(...)`: issuer-signed issued-asset definition creation. Holder
  balances still require `set_trustline(...)` and `send_token(...)`.
- `set_trustline(...)`: holder-signed TrustSet-style trustline creation.
- `authorize_trustline(...)`, `freeze_trustline(...)`,
  `unfreeze_trustline(...)`, `revoke_trustline_authorization(...)`: issuer
  controls for assets that require authorization or freeze policy.
- `send_token(...)` and `clawback_token(...)`: issued-token payment and issuer
  clawback flows.
- `create_escrow(...)`, `finish_escrow(...)`, `cancel_escrow(...)`: generic
  native PFT or issued-token escrow helpers.
- `mint_non_fungible_token(...)`, `transfer_non_fungible_token(...)`,
  `burn_non_fungible_token(...)`: NFToken-style NFT lifecycle helpers.
- `build_atomic_swap_template(...)`: reciprocal escrow-leg template for PFT and
  issued-token atomic settlement.
- `place_offer(...)`: OfferCreate-style DEX offer helper.

Example token and trustline flow:

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    create_wallet,
    mint_token,
    send_payment,
    send_token,
    set_trustline,
)

client = PostFiatRpcClient("127.0.0.1:27650")
issuer = create_wallet(
    chain_id="postfiat-controlled-testnet",
    wallet_dir="wallets/issuer",
)
holder = create_wallet(
    chain_id="postfiat-controlled-testnet",
    wallet_dir="wallets/holder",
)

token = mint_token(
    client,
    issuer_wallet=issuer,
    currency="USD",
    precision=2,
    display_name="US Dollar",
    max_supply=1_000_000_00,
    requires_authorization=True,
    freeze_enabled=True,
    clawback_enabled=True,
)

set_trustline(
    client,
    holder_wallet=holder,
    issuer=issuer.address,
    asset_id=token.asset_id,
    limit=10_000_00,
)

send_token(
    client,
    sender_wallet=issuer,
    destination=holder.address,
    issuer=issuer.address,
    asset_id=token.asset_id,
    value=250_00,
)

send_payment(
    client,
    wallet=holder,
    destination=issuer.address,
    amount=5,
    memo_type="invoice",
    memo_format="text/plain",
    memo_data="INV-1001",
)
```

Amounts are integer ledger units. `precision` is display metadata for the asset
code; callers should convert UI decimal amounts before signing.

## Account History

`account_tx` first uses the server-side bounded `account_tx` read when the
endpoint supports it. That endpoint uses the node's auto-refreshed
retained-history index when the index is current, including incremental
ancestor-tip catch-up and disk-backed per-account shards, and falls back to
the aggregate index / bounded retained-history scan when the disk index is
absent or stale. The client falls back to the older bounded client-side scan
against pre-upgrade endpoints that reject the method with `rpc_method_not_allowed`.

For operator/integration pulls across a height range, use the stdlib CLI:

```bash
scripts/postfiat-rpc-account-tx \
  --endpoint-file reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json \
  --address "$PUBLIC_CANARY_ADDRESS" \
  --from-height 0 \
  --window-size 25 \
  --limit-per-window 64 \
  --require-row \
  --require-convergence \
  --csv-output reports/postfiat-rpc-account-tx/account-history.csv
```

The tool reads only public RPC, splits the range into bounded height windows,
deduplicates rows, records endpoint hashes instead of endpoint hosts, and
fails when a window truncates unless `--allow-truncated` is explicitly set.
The CLI now uses the same client-level `account_tx_history()` helper exposed to
Python integrations. The helper resolves the current height when `to_height` is
omitted, walks deterministic height windows, deduplicates rows by
height/batch/transaction id, and reports whether the pull completed without
truncation or max-window exhaustion.

`tx(tx_id)` uses the selected-block hot path by default. The client only sends
`audit_block_log` when the caller explicitly requests `audit_block_log=True`,
matching the SDK request builder and avoiding accidental full block-log replay
on public read-only endpoints.

When `--csv-output` is provided, the tool also writes a flat CSV export from
the first successful endpoint's rows. The JSON report remains canonical and
records `csv_output`, `csv_written`, `csv_row_count`, and the endpoint label
used for the CSV. For multi-endpoint pulls, use `--require-convergence` when
the CSV is intended as the operator-facing account-history export.

For a local end-to-end wallet receipt packet, use:

```bash
scripts/testnet-wallet-receipt-packet-smoke
```

The packet starts a local four-validator harness, generates fresh
sender/recipient wallets under `/tmp`, funds the sender, signs one transparent
transfer, seals it, queries read-only RPC `tx` finality, pulls sender and
recipient `account_tx_history`, and writes matching CSV exports. The final
JSON report is redaction-safe and requires indexed account-history reads with
zero archive lookups and zero retained-history scans.

For a live read-only receipt pull using an existing public canary transaction:

```bash
ENDPOINT_FILE=reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json
ADDRESS=pflivewalletrecipient0000000000000001
TX_ID=98bccf352db9ae8a2a62938477b14c5ccd675c2423ee931ac19cd110e7dece7c05c5796b036790815041653440b3de5e

scripts/testnet-live-readonly-receipt-pull \
  --endpoint-file "$ENDPOINT_FILE" \
  --address "$ADDRESS" \
  --tx-id "$TX_ID"
```

The wrapper runs public read-only `tx`, pulls multi-endpoint
`account_tx_history`, requires endpoint convergence, requires the transaction
to appear in indexed account history, writes CSV, and emits a combined JSON
packet under `reports/testnet-live-readonly-receipt-pull/$RUN_ID/`.

Current limitations:

- bounded by the caller-provided scan limit, capped at 512;
- aggregate JSON index compaction is still a scale follow-up;
- public validator RPC remains read-only unless the operator explicitly enables
  gated write methods.

## Smoke

Run the local smoke:

```bash
scripts/testnet-python-rpc-client-smoke
```

Run a live read-only endpoint smoke:

```bash
scripts/testnet-live-python-rpc-client-smoke \
  --endpoint-file reports/testnet-remote-config-bundles-ssh-smoke/testnet-config-bundle-20260514T143535Z/remote-topology.json \
  --account-address "$PUBLIC_CANARY_ADDRESS" \
  --require-account-tx-row
```

Latest smoke evidence:

```text
reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-20260516T054717Z.json
reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-live-python-rpc-20260516T071044Z.json
reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-canary-20260516T093108Z.json
reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-20260516T112406Z.json
reports/testnet-live-python-rpc-client-smoke/testnet-live-python-rpc-client-smoke-python-history-helper-live-20260516T112425Z.json
reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-python-history-helper-account-tx-20260516T112425Z.json
reports/testnet-python-rpc-client-smoke/testnet-python-rpc-client-smoke-python-rpc-query-cli-clean-20260516T124021Z.json
reports/postfiat-rpc-query/postfiat-rpc-query-live-python-rpc-query-clean-20260516T124035Z.json
reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-account-tx-csv-20260517T030514Z.json
reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-packet-20260517T034103Z.json
reports/testnet-wallet-receipt-packet-smoke/testnet-wallet-receipt-packet-smoke-wallet-receipt-latest-20260517T063800Z.json
reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-20260517T035053Z/live-readonly-receipt-pull.json
reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-current-20260517T043055Z/live-readonly-receipt-pull.json
reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T044721Z/live-readonly-receipt-pull.json
reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T052626Z/live-readonly-receipt-pull.json
reports/testnet-live-readonly-receipt-pull/live-readonly-receipt-write-gates-20260517T060121Z/live-readonly-receipt-pull.json
reports/testnet-python-wallet-functions-smoke/
reports/xrpl-feature-parity-python-wallet-helpers/xrpl-py-style-helpers-20260521T130542Z/python-xrpl-style-helpers.json
```

The report confirms the client can read status, server info, ledger, fee,
validators, manifests, metrics, blocks, receipts, account, mempool, bridge,
Orchard pool telemetry, `account_tx_index_status`, and server-side bounded
indexed `account_tx` through a local read-only RPC server. The smoke now
creates one local transparent transfer, builds and verifies the
retained-history index, applies the transfer, and proves `account_tx` returns
the finalized funding row with `index_used=true` through the RPC edge. It keeps
local validator/faucet key material in `/tmp` by default and removes it on
exit.

The live smoke confirms the same client surface across all five controlled
read-only endpoints. It redacts endpoint hosts in the report and records only
public account canary data.

The latest account-history pull report queried all five live read-only
endpoints for the public wallet canary through height `51`, required row
fingerprint convergence, returned seven rows per endpoint, and recorded
`all_index_used=true`, zero archive lookups, and zero retained-history scans.
The latest history-helper live reports queried all five read-only endpoints at
height `63`, returned 11 converged rows per endpoint across three bounded
windows, and recorded complete indexed history with zero archive lookups and
zero retained-history scans.

The latest Python RPC query CLI smoke ran `scripts/postfiat-rpc-query
--method account_tx_history` against a local read-only RPC server and proved
one indexed row with zero archive lookups/scans under an exact 17-request
server budget. The latest live CLI query hit one controlled read-only endpoint
at height `69`, returned 13 rows across three complete indexed windows, and
redacted the endpoint host in the report.

The latest live CSV pull queried all five read-only endpoints at height `71`,
required row-fingerprint convergence, returned 14 indexed rows from the first
endpoint, wrote a 15-line CSV export, and recorded zero archive lookups and
zero retained-history scans:
`reports/postfiat-rpc-account-tx/postfiat-rpc-account-tx-live-account-tx-csv-20260517T030514Z.csv`.

The latest local wallet receipt packet passed at height `2`, confirmed the
signed spend
`4d6040819629f75539b93fe88419a4b0c7e10873eb3f40a3eae780b3507c93642fa8595e606a4e55f5c70d97f40fb2ef`
through `tx`, proved the sender history has funding and spend
rows, proved the recipient history has the inbound spend row, wrote matching
sender/recipient CSV exports, and recorded indexed reads only with zero
archive lookups and zero retained-history scans.

The latest live read-only receipt pull confirmed transaction
`98bccf352db9ae8a2a62938477b14c5ccd675c2423ee931ac19cd110e7dece7c05c5796b036790815041653440b3de5e`
through `tx` on `validator-0` at block height `85`, pulled converged account
history across all five read-only endpoints at height `86`, exported `19`
rows to CSV, and recorded zero archive lookups and zero retained-history
scans.
