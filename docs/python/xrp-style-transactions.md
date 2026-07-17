# XRP-Style Python Transactions

The Python wallet helpers include XRP-style names for the transaction flows
that are easiest to explain with XRPL vocabulary: `Payment`, `TrustSet`,
`EscrowCreate`, `EscrowFinish`, `EscrowCancel`, `NFTokenMint`, `OfferCreate`,
and token clawback. These helpers build PostFiat-native transaction envelopes.
They do not create XRPL binary transactions.

Use this page when you want complete examples. Use
[Wallet Functions](wallet-functions.md) for the compact signature reference and
[Client API](client-api.md) for raw RPC reads.

## Assumptions

- The endpoint must expose the controlled write methods. Public read-only RPC
  endpoints will reject submit methods.
- Wallet private keys stay local. The Python helpers write quote responses to a
  temporary work directory, call the local Rust SDK or node binary to sign, and
  submit signed JSON to RPC.
- Amounts are integer ledger units. `precision` is display metadata for issued
  assets, not floating-point arithmetic.
- Native PFT is identified as `PFT` in escrow, offer, and atomic-swap helpers.
- Issued assets use the 96-character `asset_id` returned by `mint_token()` or
  `create_issued_asset()`.
- Escrow `finish_after` and `cancel_after` are ledger heights. A cancellation is
  valid only after the chain reaches `cancel_after`.

## Shared Setup

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    authorize_trustline,
    build_atomic_swap_template,
    burn_non_fungible_token,
    cancel_escrow,
    cancel_offer,
    clawback_token,
    create_escrow,
    create_wallet,
    ensure_wallet_binaries,
    finish_escrow,
    freeze_trustline,
    mint_non_fungible_token,
    mint_token,
    place_offer,
    request_faucet_pft,
    revoke_trustline_authorization,
    send_payment,
    send_token,
    set_trustline,
    transfer_non_fungible_token,
    unfreeze_trustline,
)

CHAIN_ID = "postfiat-controlled-testnet"
RPC_ENDPOINT = "127.0.0.1:27650"

client = PostFiatRpcClient(RPC_ENDPOINT)
ensure_wallet_binaries()

issuer = create_wallet(chain_id=CHAIN_ID, wallet_dir="wallets/issuer")
alice = create_wallet(chain_id=CHAIN_ID, wallet_dir="wallets/alice")
bob = create_wallet(chain_id=CHAIN_ID, wallet_dir="wallets/bob")
```

Use fresh wallet directories for examples. For disposable local devnets you can
pass `overwrite=True` to regenerate files in an existing directory.

For a local controlled testnet, pass finalization directories to write helpers
when you want the submitted transaction batched and applied immediately:

```python
VALIDATOR_DATA_DIRS = [
    "devnet/local/validator-0",
    "devnet/local/validator-1",
    "devnet/local/validator-2",
]

local_finality = {
    "finalize_data_dir": VALIDATOR_DATA_DIRS[0],
    "validator_data_dirs": VALIDATOR_DATA_DIRS,
}
```

If you are pointed at a remote RPC endpoint and do not have validator data
directories on the same machine, omit `local_finality`.

For local tests, give every signing wallet enough PFT for fees before running
the write examples:

```python
for wallet in (issuer, alice, bob):
    request_faucet_pft(
        data_dir=VALIDATOR_DATA_DIRS[0],
        validator_data_dirs=VALIDATOR_DATA_DIRS,
        to_address=wallet.address,
        amount=100_000,
    )
```

## Native PFT Payment

`send_payment()` is the XRP-style alias for native transparent PFT transfers.
It maps `destination` and `amount` into the canonical `send_pft()` flow.

```python
payment = send_payment(
    client,
    wallet=alice,
    destination=bob.address,
    amount=250,
    memo_type="invoice",
    memo_format="text/plain",
    memo_data="INV-1001",
    **local_finality,
)

print(payment.tx_id)
print(payment.submit_result)
```

Useful readbacks:

```python
print(client.account_state(alice.address))
print(client.account_state(bob.address))

if payment.tx_id:
    print(client.tx(payment.tx_id))
```

## Issued Token And TrustSet Flow

This is the usual issuer and holder path:

1. Create the asset definition with `mint_token()`.
2. Holder creates or updates a trustline with `set_trustline()`.
3. Issuer authorizes the trustline if the asset requires authorization.
4. Sender transfers units with `send_token()`.
5. Read the trustline and balances back with client read methods.

```python
usd = mint_token(
    client,
    issuer_wallet=issuer,
    currency="USD",
    precision=2,
    display_name="Test USD",
    max_supply=1_000_000_00,
    requires_authorization=True,
    freeze_enabled=True,
    clawback_enabled=True,
    **local_finality,
)

asset_id = usd.asset_id
print(asset_id)

trust = set_trustline(
    client,
    holder_wallet=alice,
    issuer=issuer.address,
    asset_id=asset_id,
    limit=25_000_00,
    **local_finality,
)

authorized = authorize_trustline(
    client,
    issuer_wallet=issuer,
    account=alice.address,
    asset_id=asset_id,
    **local_finality,
)

issued_payment = send_token(
    client,
    sender_wallet=issuer,
    destination=alice.address,
    issuer=issuer.address,
    asset_id=asset_id,
    value=5_000_00,
    **local_finality,
)

print(trust.tx_id)
print(authorized.tx_id)
print(issued_payment.tx_id)
```

Readbacks for the same asset:

```python
print(client.asset_info(asset_id))
print(client.issuer_assets(issuer.address, limit=20))
print(
    client.account_lines(
        alice.address,
        issuer=issuer.address,
        asset_id=asset_id,
        limit=20,
    )
)
print(client.account_assets(alice.address, asset_id=asset_id, limit=20))
```

## Issuer Trustline Controls

Issuer controls operate on an existing holder trustline. The helper reads the
current trustline first so it can preserve the holder limit and reserve values
while changing authorization or freeze flags.

```python
frozen = freeze_trustline(
    client,
    issuer_wallet=issuer,
    account=alice.address,
    asset_id=asset_id,
    **local_finality,
)

unfrozen = unfreeze_trustline(
    client,
    issuer_wallet=issuer,
    account=alice.address,
    asset_id=asset_id,
    **local_finality,
)

revoked = revoke_trustline_authorization(
    client,
    issuer_wallet=issuer,
    account=alice.address,
    asset_id=asset_id,
    **local_finality,
)

reauthorized = authorize_trustline(
    client,
    issuer_wallet=issuer,
    account=alice.address,
    asset_id=asset_id,
    **local_finality,
)

print(frozen.tx_id, unfrozen.tx_id, revoked.tx_id, reauthorized.tx_id)
```

Clawback is a separate issuer action. It requires an asset with
`clawback_enabled=True`, and the `owner` must not be the issuer address.

```python
clawed_back = clawback_token(
    client,
    issuer_wallet=issuer,
    owner=alice.address,
    asset_id=asset_id,
    value=100_00,
    **local_finality,
)

print(clawed_back.tx_id)
```

## EscrowCreate, EscrowFinish, And EscrowCancel

For native PFT escrow, use `asset_id="PFT"` or omit `asset_id`.

```python
escrow = create_escrow(
    client,
    owner_wallet=alice,
    destination=bob.address,
    amount=500,
    condition="preimage:shipment-42",
    finish_after=0,
    cancel_after=250,
    **local_finality,
)

print(escrow.escrow_id)
print(client.escrow_info(escrow.escrow_id))

finished = finish_escrow(
    client,
    recipient_wallet=bob,
    escrow_id=escrow.escrow_id,
    owner=alice.address,
    fulfillment="preimage:shipment-42",
    **local_finality,
)

print(finished.tx_id)
```

For issued-token escrow, provide the issued asset id:

```python
token_escrow = create_escrow(
    client,
    owner_wallet=alice,
    destination=bob.address,
    asset_id=asset_id,
    amount=250_00,
    condition="preimage:token-delivery-1",
    finish_after=0,
    cancel_after=300,
    **local_finality,
)

print(token_escrow.escrow_id)
```

Cancellation is valid only after the chain height reaches `cancel_after`.

```python
# Run this only after the chain height reaches the configured cancel_after.
cancelled = cancel_escrow(
    client,
    owner_wallet=alice,
    escrow_id=token_escrow.escrow_id,
    **local_finality,
)

print(cancelled.tx_id)
```

Escrow readbacks:

```python
print(client.account_escrows(alice.address, role="owner", state="open", limit=20))
print(client.account_escrows(bob.address, role="recipient", limit=20))
```

## Atomic Swap Template

`build_atomic_swap_template()` creates reciprocal escrow-leg templates. It does
not sign or submit either leg. Use it when you need deterministic swap ids and
the two escrow operations before wallet approval or before a custom submission
flow.

Exactly one leg must be native `PFT` and exactly one leg must be an issued
asset.

```python
swap = build_atomic_swap_template(
    client,
    left_wallet=alice,
    right_wallet=bob,
    left_asset_id="PFT",
    left_amount=1_000,
    right_asset_id=asset_id,
    right_amount=900_00,
    condition="preimage:swap-900-usd",
    finish_after=0,
    cancel_after=500,
)

print(swap.settlement_id)
print(swap.left_escrow_id)
print(swap.right_escrow_id)
print(swap.left_operation)
print(swap.right_operation)
```

For a normal wallet-helper path, use `create_escrow()` twice with the same
condition and compatible `cancel_after` values, then finish both escrows with
the same fulfillment when both legs are accepted.

## NFT Mint, Transfer, And Burn

`mint_non_fungible_token()` is the `NFTokenMint`-style helper. `metadata_hash`
is an application-defined hash string; keep the actual metadata in your own
storage or in the URI you reference.

```python
nft = mint_non_fungible_token(
    client,
    issuer_wallet=issuer,
    collection_id="art-2026",
    serial=1,
    metadata_hash="8b1a9953c4611296a827abf8c47804d7",
    metadata_uri="ipfs://example/art-2026/1.json",
    owner=alice.address,
    **local_finality,
)

print(nft.nft_id)
print(client.nft_info(nft.nft_id))

transferred = transfer_non_fungible_token(
    client,
    owner_wallet=alice,
    nft_id=nft.nft_id,
    destination=bob.address,
    **local_finality,
)

print(transferred.tx_id)
print(client.account_nfts(bob.address, limit=20))

burned = burn_non_fungible_token(
    client,
    owner_wallet=bob,
    nft_id=nft.nft_id,
    **local_finality,
)

print(burned.tx_id)
print(client.account_nfts(bob.address, include_burned=True, limit=20))
```

Issuer-wide NFT readbacks:

```python
print(client.issuer_nfts(issuer.address, collection_id="art-2026", limit=20))
```

## OfferCreate And OfferCancel

DEX offers use `PFT` for the native side and an issued `asset_id` for the token
side. The asset ids in an offer must differ.

```python
offer = place_offer(
    client,
    wallet=alice,
    taker_gets_asset_id="PFT",
    taker_gets_value=1_000,
    taker_pays_asset_id=asset_id,
    taker_pays_value=950_00,
    expiration_height=1_000,
    **local_finality,
)

print(offer.offer_id)
print(client.offer_info(offer.offer_id))
print(client.account_offers(alice.address, state="open", limit=20))
print(client.book_offers("PFT", asset_id, limit=20))

cancelled_offer = cancel_offer(
    client,
    wallet=alice,
    offer_id=offer.offer_id,
    **local_finality,
)

print(cancelled_offer.tx_id)
```

## Result Objects

Write helpers return typed result objects with the data needed to audit a
transaction locally.

| Helper family | Result type | Important fields |
| --- | --- | --- |
| `send_payment()`, `send_pft()` | `SendPftResult` | `tx_id`, `quote_response`, `signed_transfer`, `submit_result`, `finalized_batch_file`, `receipts_by_validator` |
| Asset and trustline helpers | `AssetTransactionResult` | `tx_id`, `operation`, `asset_id`, `signed_asset_transaction`, `submit_result`, `receipts_by_validator` |
| Escrow helpers | `EscrowTransactionResult` | `tx_id`, `operation`, `escrow_id`, `signed_escrow_transaction`, `submit_result`, `receipts_by_validator` |
| NFT helpers | `NftTransactionResult` | `tx_id`, `operation`, `nft_id`, `signed_nft_transaction`, `submit_result`, `receipts_by_validator` |
| Offer helpers | `OfferTransactionResult` | `tx_id`, `operation`, `offer_id`, `signed_offer_transaction`, `submit_result`, `receipts_by_validator` |
| Atomic swap template | `AtomicSettlementTemplateResult` | `settlement_id`, `left_operation`, `right_operation`, `left_escrow_id`, `right_escrow_id`, `template` |

## Transaction And Readback Map

| XRP-style flow | Python helper | Primary readbacks |
| --- | --- | --- |
| `Payment` for native PFT | `send_payment()` | `account_state()`, `tx()`, `account_tx_history()` |
| Issued asset definition | `mint_token()` | `asset_info()`, `issuer_assets()` |
| `TrustSet` | `set_trustline()` | `account_lines()`, `account_assets()` |
| Trustline authorization | `authorize_trustline()` | `account_lines()` |
| Trustline freeze | `freeze_trustline()`, `unfreeze_trustline()` | `account_lines()` |
| Issued-token payment | `send_token()` | `account_assets()`, `account_tx_history()` |
| Issuer clawback | `clawback_token()` | `account_assets()`, `account_lines()` |
| `EscrowCreate` | `create_escrow()` | `escrow_info()`, `account_escrows()` |
| `EscrowFinish` | `finish_escrow()` | `escrow_info()`, `account_escrows()` |
| `EscrowCancel` | `cancel_escrow()` | `escrow_info()`, `account_escrows()` |
| Atomic swap planning | `build_atomic_swap_template()` | Template result, then escrow readbacks after submission |
| `NFTokenMint` | `mint_non_fungible_token()` | `nft_info()`, `account_nfts()`, `issuer_nfts()` |
| NFT transfer | `transfer_non_fungible_token()` | `nft_info()`, `account_nfts()` |
| NFT burn | `burn_non_fungible_token()` | `nft_info()`, `account_nfts(include_burned=True)` |
| `OfferCreate` | `place_offer()` | `offer_info()`, `account_offers()`, `book_offers()` |
| `OfferCancel` | `cancel_offer()` | `offer_info()`, `account_offers()` |
