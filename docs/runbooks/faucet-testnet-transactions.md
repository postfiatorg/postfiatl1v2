# Faucet Testnet Transactions

Status: controlled-testnet operator checklist
Date: 2026-06-27

This runbook explains how to send PFT from the local operator faucet to a
target address on the controlled testnet or local devnet. The faucet is the
operator-funded account created during `postfiat-node init`. Its key material
lives in `faucet_key.json` inside each validator data directory.

## Do Not Publish

Never paste or upload:

- `faucet_key.json`
- `faucet_account.json`
- raw validator key material

## Prerequisites

- A built `postfiat-node` binary (`target/debug/postfiat-node` or
  `target/release/postfiat-node`).
- An initialized data directory containing `faucet_key.json` and
  `faucet_account.json` (created by `postfiat-node init` or
  `scripts/devnet-up`).
- The recipient's PostFiat address (a `pf`-prefixed hex string).

## Scope Warning: Local Harness Only

The `batch-transfer` + `apply-batch` flow described below is a **local
harness / operator tool**. It requires local filesystem access to
validator data directories and faucet key material.

**Do not use `apply-batch` for WAN/testnet wallet sends.** WAN/testnet
wallets must use the RPC submit path:
`mempool_submit_signed_transfer` via a write-enabled RPC edge, followed
by polling `tx` / `receipts` for finality. The Python helpers
`send_pft()` (submit-only mode) and `send_pft_and_poll_finality()`
implement this path and never call `apply-batch`.

`apply-batch` is quarantined to local harness flows where the operator
has direct filesystem access to all validator data directories. See
`docs/specs/wallet-wan-devnet-finality-fix.md` for the full policy.

## CLI Flow: batch-transfer + apply-batch

The canonical **local harness** faucet flow is a two-step process:
create a signed transfer batch from the faucet account, then apply that
batch to each validator's data directory. This requires local filesystem
access to all validator data dirs.

### Step 1 — Create The Batch

```bash
postfiat-node batch-transfer \
  --data-dir devnet/local/node0 \
  --to pfde0ba09f38b1748f8d77709715e1095a0ff74d0f \
  --amount 20000000 \
  --batch-file /tmp/faucet-send/faucet-20pft.batch.json
```

This produces a JSON batch file containing the signed transparent transfer
from the faucet account. The batch is signed with the faucet key at the
current faucet sequence number.

| Flag | Description |
| --- | --- |
| `--data-dir` | Validator data directory containing `faucet_key.json`. |
| `--to` | Recipient PostFiat address (`pf`-prefixed hex). |
| `--amount` | Raw PFT atoms as an integer. `1 PFT = 1,000,000` atoms. |
| `--batch-file` | Output path for the signed batch JSON. |

### Step 2 — Apply The Batch To Validators

```bash
postfiat-node apply-batch \
  --data-dir devnet/local/node0 \
  --batch-file /tmp/faucet-send/faucet-20pft.batch.json
```

For a multi-validator devnet, apply to each validator:

```bash
for i in 0 1 2 3; do
  postfiat-node apply-batch \
    --data-dir devnet/local/node$i \
    --batch-file /tmp/faucet-send/faucet-20pft.batch.json
done
```

Each `apply-batch` returns a receipt JSON array. A successful application
returns `"accepted": true` with `"code": "accepted"`.

### Sequence Note

The batch is signed at the faucet account's current sequence number on the
node that created it. All validators must be at the same chain height and
faucet sequence for the batch to be accepted by every node. If nodes have
diverged (e.g. from a prior single-node apply), re-initialize the devnet
with `scripts/devnet-up` before sending a new faucet batch. In a live
network using certified batch roundtrips, sequence propagation is handled
automatically.

### Step 3 — Verify The Recipient Balance

```bash
postfiat-node account \
  --data-dir devnet/local/node0 \
  --address pfde0ba09f38b1748f8d77709715e1095a0ff74d0f
```

Example output:

```json
{"address":"pfde0ba09f38b1748f8d77709715e1095a0ff74d0f","balance":20000000,"sequence":0,"public_key_hex":null}
```

## Wrapper Script

The repo includes `scripts/node-faucet` which runs the `faucet` subcommand
to print the faucet key report:

```bash
scripts/node-faucet
```

This prints the faucet account address and public key for inspection. It
does not send a transaction.

## Python Helper

For integration code, use the Python helper `request_faucet_pft()`:

```python
from postfiat_rpc import request_faucet_pft

funding = request_faucet_pft(
    data_dir="devnet/local/node0",
    to_address="pfde0ba09f38b1748f8d77709715e1095a0ff74d0f",
    amount=20_000_000,
)

print(funding.tx_id)
```

For multi-validator local finality:

```python
from postfiat_rpc import request_faucet_pft

validators = [
    "devnet/local/node0",
    "devnet/local/node1",
    "devnet/local/node2",
    "devnet/local/node3",
]

funding = request_faucet_pft(
    data_dir=validators[0],
    to_address="pfde0ba09f38b1748f8d77709715e1095a0ff74d0f",
    amount=20_000_000,
    validator_data_dirs=validators,
)

print(funding.tx_id)
```

For a one-line human amount wrapper, use:

```bash
scripts/pftl-transfer.py faucet \
  --to pfde0ba09f38b1748f8d77709715e1095a0ff74d0f \
  --amount 20
```

### WAN/Testnet Wallet Send (No apply-batch)

For wallet sends on the WAN devnet or any live testnet, use the RPC
submit path instead of `apply-batch`:

```python
from postfiat_rpc import PostFiatRpcClient
from postfiat_rpc.wallet import send_pft, send_pft_and_poll_finality, load_wallet

client = PostFiatRpcClient("192.0.2.10:27650")
wallet = load_wallet(wallet_dir="/path/to/wallet", chain_id="postfiat-wan-devnet")

# Submit only — returns tx_id, does not wait for finality
result = send_pft(client, wallet=wallet, to_address="pf...", amount=1_000_000)
print(result.tx_id, result.submit_mode)  # submit_mode = "submit_only"

# Submit and poll — waits for finality receipt
result = send_pft_and_poll_finality(
    client, wallet=wallet, to_address="pf...", amount=1_000_000,
    poll_timeout_seconds=120.0,
)
print(result.tx_id, result.submit_mode, result.finalized)  # submit_and_poll, True
```

Neither of these functions calls `apply-batch`. The
`--finalize-data-dir` flag on `pftl-transfer.py` is labeled "(local
harness only)" and is the only path that invokes `apply-batch`.

See [Python RPC Client](python-rpc-client.md) and
[Python Wallet Functions](../python/wallet-functions.md) for the full
helper surface.

## StakeHub Dashboard

The StakeHub dashboard server (`stakehub/dashboard_server.py`) wraps the
same `batch-transfer` + `apply-batch` flow behind the `pftl_faucet_fund`
action. It uses the PostFiat repo's `postfiat-node` binary and the PFTL
wallet data directory. See the StakeHub repo for details.

## Fee

The faucet transfer includes a minimum fee (32 base units at the current
protocol version) which is burned on acceptance. The fee is automatically
calculated and included in the signed batch.

## Related

- [Operator Day Two](operator-day-two.md)
- [Python RPC Client](python-rpc-client.md)
- [Python Wallet Functions](../python/wallet-functions.md)
- [Python Examples](../python/examples.md)
- [RPC Method Inventory](rpc-method-inventory.md)
