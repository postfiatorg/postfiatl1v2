# Python Examples

These examples use the stdlib-only `postfiat_rpc` package. The Python layer
does transport and orchestration; signing still happens through the local Rust
node/SDK binaries.

The XRP-style helpers mirror familiar transaction names such as `Payment`,
`TrustSet`, `EscrowCreate`, `NFTokenMint`, and `OfferCreate`, but they submit
PostFiat-native transaction envelopes. They are not XRPL binary transactions.
For complete trustline, issued-token, escrow, atomic swap, NFT, and offer flows,
see [XRP-Style Python Transactions](xrp-style-transactions.md).

For NAVCOIN reserve arithmetic and native NAV operation JSON, see
[NAVCOIN Python Example](../business/navcoin-python-example.md).

## Pull Status

```python
from postfiat_rpc import PostFiatRpcClient

client = PostFiatRpcClient("127.0.0.1:8080")
print(client.status())
```

## Pull Account History

```python
from postfiat_rpc import PostFiatRpcClient

client = PostFiatRpcClient("127.0.0.1:8080")
history = client.account_tx_history("<address>", to_height=100)
for row in history.rows:
    print(row)
```

## Create A Transparent Wallet

```python
from postfiat_rpc import create_wallet

wallet = create_wallet(
    chain_id="postfiat-controlled-testnet",
    wallet_dir="wallets",
)

print(wallet.address)
```

## Request Faucet PFT

```python
from postfiat_rpc import request_faucet_pft

funding = request_faucet_pft(
    data_dir="devnet/local/validator-0",
    to_address=wallet.address,
    amount=1_000,
)

print(funding.tx_id)
```

## Send Transparent PFT

```python
from postfiat_rpc import PostFiatRpcClient, send_pft

client = PostFiatRpcClient("127.0.0.1:27650")

sent = send_pft(
    client,
    wallet=wallet,
    to_address="pfrecipient...",
    amount=100,
)

print(sent.tx_id)
```

`send_payment()` is the XRP-style alias for the same native PFT flow:

```python
from postfiat_rpc import send_payment

sent = send_payment(
    client,
    wallet=wallet,
    destination="pfrecipient...",
    amount=100,
    memo_type="invoice",
    memo_format="text/plain",
    memo_data="INV-1001",
)

print(sent.tx_id)
```

## Create An Issued Token And TrustSet

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    authorize_trustline,
    create_wallet,
    mint_token,
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

usd = mint_token(
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

trustline = set_trustline(
    client,
    holder_wallet=holder,
    issuer=issuer.address,
    asset_id=usd.asset_id,
    limit=10_000_00,
)

authorize_trustline(
    client,
    issuer_wallet=issuer,
    account=holder.address,
    asset_id=usd.asset_id,
)

payment = send_token(
    client,
    sender_wallet=issuer,
    destination=holder.address,
    issuer=issuer.address,
    asset_id=usd.asset_id,
    value=250_00,
)

print(usd.asset_id)
print(trustline.tx_id)
print(payment.tx_id)
```

## Issuer Trustline Controls

```python
from postfiat_rpc import (
    clawback_token,
    freeze_trustline,
    revoke_trustline_authorization,
    unfreeze_trustline,
)

freeze_trustline(
    client,
    issuer_wallet=issuer,
    account=holder.address,
    asset_id=usd.asset_id,
)

unfreeze_trustline(
    client,
    issuer_wallet=issuer,
    account=holder.address,
    asset_id=usd.asset_id,
)

clawback_token(
    client,
    issuer_wallet=issuer,
    owner=holder.address,
    asset_id=usd.asset_id,
    value=25_00,
)

revoke_trustline_authorization(
    client,
    issuer_wallet=issuer,
    account=holder.address,
    asset_id=usd.asset_id,
)
```

Use `account_lines()`, `account_assets()`, and `issuer_assets()` for readback:

```python
print(client.account_lines(holder.address, issuer=issuer.address, asset_id=usd.asset_id))
print(client.account_assets(holder.address, asset_id=usd.asset_id))
print(client.issuer_assets(issuer.address))
```

## EscrowCreate, EscrowFinish, And EscrowCancel

```python
from postfiat_rpc import cancel_escrow, create_escrow, finish_escrow

pft_escrow = create_escrow(
    client,
    owner_wallet=wallet,
    destination=holder.address,
    asset_id="PFT",
    amount=100,
    condition="shared-hashlock",
    cancel_after=200,
)

finish_escrow(
    client,
    recipient_wallet=holder,
    escrow_id=pft_escrow.escrow_id,
    owner=wallet.address,
    fulfillment="shared-hashlock",
)

issued_escrow = create_escrow(
    client,
    owner_wallet=issuer,
    destination=holder.address,
    asset_id=usd.asset_id,
    amount=50_00,
    condition="issued-hashlock",
    cancel_after=300,
)

# Run this only after the chain height reaches cancel_after.
cancel_escrow(
    client,
    owner_wallet=issuer,
    escrow_id=issued_escrow.escrow_id,
)
```

## Atomic Swap Template

```python
from postfiat_rpc import build_atomic_swap_template

swap = build_atomic_swap_template(
    client,
    left_wallet=wallet,
    right_wallet=holder,
    left_asset_id="PFT",
    left_amount=100,
    right_asset_id=usd.asset_id,
    right_amount=25_00,
    condition="shared-secret",
    finish_after=10,
    cancel_after=200,
)

print(swap.settlement_id)
print(swap.left_escrow_id)
print(swap.right_escrow_id)
```

## NFToken-Style Lifecycle

```python
from postfiat_rpc import (
    burn_non_fungible_token,
    mint_non_fungible_token,
    transfer_non_fungible_token,
)

nft = mint_non_fungible_token(
    client,
    issuer_wallet=issuer,
    collection_id="collection-1",
    serial=1,
    metadata_hash="ab" * 32,
    owner=holder.address,
    metadata_uri="ipfs://postfiat-nft",
    issuer_transfer_fee=7,
)

transfer_non_fungible_token(
    client,
    owner_wallet=holder,
    nft_id=nft.nft_id,
    destination=wallet.address,
)

burn_non_fungible_token(
    client,
    owner_wallet=wallet,
    nft_id=nft.nft_id,
)
```

Read NFTs with:

```python
print(client.nft_info(nft.nft_id))
print(client.account_nfts(holder.address, include_burned=True))
print(client.issuer_nfts(issuer.address, collection_id="collection-1"))
```

## OfferCreate And OfferCancel

```python
from postfiat_rpc import cancel_offer, place_offer

offer = place_offer(
    client,
    wallet=holder,
    taker_gets_asset_id="PFT",
    taker_gets_value=100,
    taker_pays_asset_id=usd.asset_id,
    taker_pays_value=25_00,
    expiration_height=500,
)

print(client.offer_info(offer.offer_id))
print(client.account_offers(holder.address, state="open"))
print(client.book_offers("PFT", usd.asset_id))

cancel_offer(
    client,
    wallet=holder,
    offer_id=offer.offer_id,
)
```

## Send Shielded PFT

```python
from postfiat_rpc import create_orchard_wallet, send_shielded_pft

orchard_wallet = create_orchard_wallet(wallet_dir="wallets")

shielded = send_shielded_pft(
    data_dir="devnet/local/validator-0",
    from_wallet=wallet,
    recipient=orchard_wallet,
    amount=25,
)

print(shielded.tx_id)
```

## Operational Script

```bash
scripts/postfiat-rpc-account-tx --endpoint 127.0.0.1:8080 --account <address>
```

The exact CLI flags are defined by `python/postfiat_rpc/__main__.py` and the
runbook in `docs/runbooks/python-rpc-client.md`.
