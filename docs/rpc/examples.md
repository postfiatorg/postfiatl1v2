# RPC Examples

Use the Python client for most integration work. Use raw RPC only when testing
the server boundary.

## Status

```bash
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:8080 --method status
```

## Fee

```bash
PYTHONPATH=python python -m postfiat_rpc --endpoint 127.0.0.1:8080 --method fee
```

## Account History

```bash
PYTHONPATH=python python -m postfiat_rpc \
  --endpoint 127.0.0.1:8080 \
  --method account_tx \
  --account <address> \
  --limit 50
```

## Doctor

```bash
scripts/testnet-rpc-doctor
```

## Source

- `python/postfiat_rpc/__main__.py`
- `python/postfiat_rpc/client.py`
- `docs/runbooks/python-rpc-client.md`
