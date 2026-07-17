# Python Client Quickstart

The Python client is under `python/postfiat_rpc`.

## Use From The Repo

```bash
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:8080 --method status
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:8080 --method ledger
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:8080 --method fee
```

## Account History

```bash
PYTHONPATH=python python -m postfiat_rpc \
  --endpoint 127.0.0.1:8080 \
  --method account_tx_history \
  --account <address> \
  --to-height 100 \
  --window-size 100
```

## Wallet Functions

The canonical Python wallet helpers are exported from `postfiat_rpc`.
For TrustSet, issued-token, escrow, NFT, offer, and atomic-swap examples, see
[XRP-Style Python Transactions](xrp-style-transactions.md). For smaller
snippets, see [Python Examples](examples.md).

```python
from postfiat_rpc import (
    PostFiatRpcClient,
    create_wallet,
    request_faucet_pft,
    send_pft,
)

client = PostFiatRpcClient("127.0.0.1:27650")
wallet = create_wallet(chain_id="postfiat-controlled-testnet", wallet_dir="wallets")

request_faucet_pft(
    data_dir="devnet/local/validator-0",
    to_address=wallet.address,
    amount=1_000,
)

result = send_pft(
    client,
    wallet=wallet,
    to_address="pfrecipient...",
    amount=100,
)
print(result.tx_id)
```

## Smoke Evidence

- `reports/testnet-live-python-rpc-client-smoke/`
- `reports/testnet-python-rpc-client-smoke/`
- `reports/testnet-six-wallet-account-tx-smoke/`
- `reports/testnet-python-wallet-functions-smoke/`
