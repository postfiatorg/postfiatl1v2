# Python Wallet Functions

For full end-to-end examples that use these helpers, see
[XRP-Style Python Transactions](xrp-style-transactions.md).

`postfiat_rpc.wallet` is the canonical Python wallet surface for controlled
testnet integration code.

The Python code does orchestration and RPC. It does not reimplement ML-DSA
signing or Orchard/Halo2 action construction. Those operations call the same
Rust `postfiat-node` and `postfiat-rpc-sdk` binaries used by the node.

## Functions

| Function | What it does |
| --- | --- |
| `create_wallet()` | Creates a transparent ML-DSA wallet key file and backup file. |
| `request_faucet_pft()` | Creates and applies a local faucet funding batch. |
| `send_pft()` | Gets a fee quote, signs it, submits the signed transfer, and can finalize it locally. |
| `send_payment()` | XRP-style alias for native PFT `Payment`, including bounded memo fields. |
| `wrap_fastpay()` | Locally signs an account-to-FastPay PFT deposit, submits it through consensus, verifies the accepted receipt, and resolves the created owned object. |
| `send_fastpay()` | Signs a recovery-safe FastPay v3 order, collects a distinct-validator certificate, and requires an authenticated durable-apply quorum. |
| `unwrap_fastpay()` | Performs the recovery-safe v3 owned-to-account flow with automatic object selection, change, and authenticated durable-apply quorum. |
| `create_issued_asset()` | Creates a ledger-native issued-asset definition. |
| `mint_token()` | XRP-style alias for issuer-side token definition creation. |
| `create_asset_trustline()` | Creates or updates a holder trustline. |
| `set_trustline()` | XRP-style `TrustSet` helper for trustline creation or update. |
| `authorize_trustline()` | Issuer-side trustline authorization helper. |
| `freeze_trustline()` | Issuer-side trustline freeze helper. |
| `unfreeze_trustline()` | Issuer-side trustline unfreeze helper. |
| `revoke_trustline_authorization()` | Issuer-side authorization revoke helper. |
| `send_issued_asset()` | Sends ledger-native issued assets. |
| `send_token()` | XRP-style issued-token payment helper. |
| `clawback_token()` | Issuer clawback helper for clawback-enabled assets. |
| `create_escrow()` | XRP-style helper for native PFT or issued-asset escrow create. |
| `finish_escrow()` | Finishes an escrow from the recipient wallet. |
| `cancel_escrow()` | Cancels an escrow from the owner wallet. |
| `build_atomic_swap_template()` | Builds reciprocal escrow legs for PFT/issued-asset swaps. |
| `mint_non_fungible_token()` | NFToken-style NFT mint helper. |
| `transfer_non_fungible_token()` | NFToken-style NFT transfer helper. |
| `burn_non_fungible_token()` | NFToken-style NFT burn helper. |
| `place_offer()` | OfferCreate-style DEX helper. |
| `cancel_offer()` | OfferCancel helper. |
| `create_orchard_wallet()` | Creates an Orchard spending key and exported full viewing key. |
| `send_shielded_pft()` | Creates a transparent-to-Orchard deposit and applies it locally, or sends it to a gated Orchard batch-create RPC edge. |
| `scan_orchard_wallet()` | Scans local Orchard pool state for notes visible to the wallet. |

The XRP-style names are wallet UX wrappers. They still quote, sign, submit, and
optionally finalize PostFiat-native transaction envelopes.

## FastPay Helpers

FastPay helpers operate through a wallet-facing endpoint, normally the wallet
proxy. Signed owned-object methods are enabled under normal RPC startup. An
operator may explicitly disable them during an incident with
`--disable-owned-lane`; `server_info.rpc.owned_lane_enabled` lets clients fail
closed instead of presenting unavailable payment controls. The v3 recovery
protocol provides governed committee rotation, bounded lock recovery and
fail-closed confirm-or-cancel decisions without giving the proxy custody.

- `wrap_fastpay` constructs and locally signs `OwnedDepositV1`; funding enters
  through that consensus-committed FastLane deposit and must finish with the
  exact accepted receipt code `owned_deposit_applied`. The legacy unsigned
  `wrap_owned`/`unwrap_owned` RPC methods are not exposed by the Python client.
- `send_fastpay(client, wallet, recipient_public_key_hex, amount, fee=1)`
  binds the exact governed recovery capability and validity window, signs an
  `OwnedTransferOrderV3`, collects distinct votes with `owned_sign_v3`, applies
  with `owned_apply_v3`, and verifies at least the governed quorum of signed
  durable acknowledgements locally.
- `unwrap_fastpay(client, wallet, amount, fee=0)` selects one or more owned
  objects up to the 2048 input cap and performs the equivalent
  `OwnedUnwrapOrderV3` flow through `owned_unwrap_sign_v3` and
  `owned_unwrap_apply_v3`. An absent or inactive recovery capability fails
  closed before signing.

The default owned-object lookup limit is 2048 for `wrap_fastpay`,
`send_fastpay`, `unwrap_fastpay`, and the `pftl_transfer.py unwrap-fastpay`
CLI. This matches the protocol input cap used by standard unwrap and avoids
hiding fragmented FastPay value from Python tooling.

## Transparent Flow

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    create_wallet,
    request_faucet_pft,
    send_pft,
)

wallet = create_wallet(
    chain_id="postfiat-controlled-testnet",
    wallet_dir="wallets",
)

request_faucet_pft(
    data_dir="devnet/local/validator-0",
    to_address=wallet.address,
    amount=1_000,
)

client = PostFiatRpcClient("127.0.0.1:27650")
sent = send_pft(
    client,
    wallet=wallet,
    to_address="pfrecipient...",
    amount=100,
)

print(sent.tx_id)
```

## Issued Token And TrustSet Flow

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

set_trustline(
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

sent = send_token(
    client,
    sender_wallet=issuer,
    destination=holder.address,
    issuer=issuer.address,
    asset_id=usd.asset_id,
    value=250_00,
)

print(usd.asset_id)
print(sent.tx_id)
```

## Issuer Controls

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

## Escrow And Atomic Swap

```python
from postfiat_rpc import build_atomic_swap_template, create_escrow, finish_escrow

escrow = create_escrow(
    client,
    owner_wallet=issuer,
    destination=holder.address,
    asset_id=usd.asset_id,
    amount=50_00,
    condition="shared-hashlock",
    cancel_after=200,
)

finish_escrow(
    client,
    recipient_wallet=holder,
    escrow_id=escrow.escrow_id,
    owner=issuer.address,
    fulfillment="shared-hashlock",
)

swap = build_atomic_swap_template(
    client,
    left_wallet=issuer,
    right_wallet=holder,
    left_asset_id=usd.asset_id,
    left_amount=25_00,
    right_asset_id="PFT",
    right_amount=100,
    condition="shared-secret",
    cancel_after=300,
)

print(swap.settlement_id)
```

Escrow helpers quote through RPC, sign locally, then submit the signed escrow
transaction. When the wallet's `.key.json` file is present, Python uses
`postfiat-node wallet-sign-escrow-transaction --key-file ... --quote-file ...`
so live custody key files can sign escrow create, finish, and cancel
transactions directly. The older SDK backup signer remains a fallback for
callers that only have backup files.

## NFT And Offer Helpers

```python
from postfiat_rpc import (
    cancel_offer,
    mint_non_fungible_token,
    place_offer,
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
    destination=issuer.address,
)

offer = place_offer(
    client,
    wallet=holder,
    taker_gets_asset_id="PFT",
    taker_gets_value=100,
    taker_pays_asset_id=usd.asset_id,
    taker_pays_value=25_00,
)

cancel_offer(client, wallet=holder, offer_id=offer.offer_id)
```

## Shielded Flow

```python
from postfiat_rpc import create_orchard_wallet, scan_orchard_wallet, send_shielded_pft

orchard_wallet = create_orchard_wallet(wallet_dir="wallets")

shielded = send_shielded_pft(
    data_dir="devnet/local/validator-0",
    from_wallet=wallet,
    recipient=orchard_wallet,
    amount=25,
)

notes = scan_orchard_wallet(
    data_dir="devnet/local/validator-0",
    wallet=orchard_wallet,
)

print(shielded.tx_id)
print(notes)
```

## Local Finality

For local harnesses, pass validator data dirs so helpers apply batches to every
validator:

```python
validators = [
    "devnet/local/validator-0",
    "devnet/local/validator-1",
    "devnet/local/validator-2",
    "devnet/local/validator-3",
]

request_faucet_pft(
    data_dir=validators[0],
    to_address=wallet.address,
    amount=1_000,
    validator_data_dirs=validators,
)

sent = send_pft(
    client,
    wallet=wallet,
    to_address="pfrecipient...",
    amount=100,
    finalize_data_dir=validators[0],
    validator_data_dirs=validators,
)
```

## Smoke

```bash
PYTHONPATH=python python -m unittest discover -s python/tests
scripts/testnet-python-wallet-functions-smoke
```
