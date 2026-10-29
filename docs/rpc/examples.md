# RPC Examples

Use the Python client for most integration work. Use raw RPC only when testing
the server boundary.

## Status

```bash
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:27650 --method status
```

## Fee

```bash
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:27650 --method fee
```

## Account History

```bash
PYTHONPATH=python python -m postfiat_rpc \
  --endpoint 127.0.0.1:27650 \
  --method account_tx \
  --address <address> \
  --limit 50
```

## Doctor

```bash
scripts/testnet-rpc-doctor
```

## ERC-20 PFT On Base

`postfiat_rpc.pft_token` reads the ERC-20 PFT token over Base JSON-RPC
(`eth_chainId`, `eth_blockNumber`, `eth_getCode`, `eth_call`), pinned to one
block: name, symbol, decimals, total supply, the treasury balance and whether
the treasury holds the whole supply. It prints JSON, or aligned lines with
`--table`, and exits 2 on a chain-id mismatch, a token without code or a
malformed answer. A record supplies the token, treasury and chain id:

```bash
PYTHONPATH=python python -m postfiat_rpc.pft_token \
  --record crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json \
  --table
```

Without a record, give the token and treasury; the endpoint must be Base
(8453) or Base Sepolia (84532). `--rpc-url` defaults to
`https://sepolia.base.org`:

```bash
PYTHONPATH=python python -m postfiat_rpc.pft_token \
  --token 0x18fE4ca51A0cE042688D5A54917319d03B527D92 \
  --treasury 0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB
```

`postfiat_rpc.pft_token_ui` serves the same state as a read-only page on
`127.0.0.1` (default port 8766): the token and treasury with Blockscout
links, the supply in PFT and in units, a green or red line for whether the
treasury holds the whole supply, the block, the read time and a Refresh
button. It takes the same `--rpc-url`, `--record` or `--token`/`--treasury`
options, shows the RPC URL only up to its host, and answers only GET:

```bash
PYTHONPATH=python python -m postfiat_rpc.pft_token_ui \
  --record crates/pft-token-contracts/deployments/pft-token-base-84532/deployment.json
```

## Private Path State

`postfiat_rpc.private_path_ui` serves a read-only page on `127.0.0.1`
(default port 8767) for one validator's user-proven shielded action path:

- every `private_readiness` field, with the reasons when it is not ready;
- the `private_pool_snapshot` token, counts and completeness;
- the `asset_orchard_pricing_claim` for the governed A666/pfUSDC pair at the
  next height, with its status or refusal;
- the `asset_orchard_private_egress_binding`, which is `ungoverned` or shows
  the registered policy;
- a `private_action_receipt` for an intent id typed into the page.

It calls only those read methods, shows a node refusal as a refusal, and
answers only GET:

```bash
PYTHONPATH=python python -m postfiat_rpc.private_path_ui \
  --endpoint validator-0=127.0.0.1:27650
```

`--base-asset-id` and `--quote-asset-id` override the pair taken from the
governed route.

## Source

- `python/postfiat_rpc/__main__.py`
- `python/postfiat_rpc/client.py`
- `python/postfiat_rpc/pft_token.py`
- `python/postfiat_rpc/pft_token_ui.py`
- `python/postfiat_rpc/private_path_ui.py`
- `docs/runbooks/python-rpc-client.md`
