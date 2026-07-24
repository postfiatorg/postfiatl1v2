# Synthetic Lightning regtest environment

This environment runs one Bitcoin Core **regtest** node and three LND nodes:

```text
lnd-user <-> lnd-router <-> lnd-coordinator
                     |
          Bitcoin Core regtest
```

It is test infrastructure for the PFTL Lightning/NAVcoin coordinator. All BTC,
transactions, channels, and invoices are synthetic. Runtime services attach
only to a Docker `internal: true` network. Bitcoin P2P is disabled
(`listen=0`, `connect=0`, `dnsseed=0`) and LND automatic bootstrap is disabled.
No container port is published. On Linux, local tooling reaches the pinned
internal bridge subnet directly; that subnet has no default route or external
egress.

## Pinned supply chain

- LND `v0.20.1-beta`, pinned to the official Lightning Labs linux/amd64 image
  manifest `sha256:fb30ea34...3b7f54`. `images` runs the image's
  `/verify-install.sh`, requiring the release manifest and at least five valid
  developer signatures.
- Bitcoin Core `31.1`, built into a minimal local image from the reproducible
  x86_64 release archive at `bitcoincore.org`. The Docker build verifies
  archive SHA-256
  `b80d9c3e04da78fb6f0569685673418cf686fadba9042d926d13fb87ff503f9e`
  before installing `bitcoind` and `bitcoin-cli`. The Debian base image is also
  pinned by platform digest.

Image fetching/signature verification needs internet access before startup.
The running demo topology has no external connectivity.

## Operator commands

From the repository root:

```bash
scripts/lightning-navcoin-regtest-env lint
scripts/lightning-navcoin-regtest-env init
scripts/lightning-navcoin-regtest-env fund
scripts/lightning-navcoin-regtest-env channels
scripts/lightning-navcoin-regtest-env status
scripts/lightning-navcoin-regtest-env credentials
scripts/lightning-navcoin-regtest-env grpc-smoke
scripts/lightning-navcoin-regtest-env evidence
scripts/lightning-navcoin-regtest-env stop
```

`init`, `fund`, and `channels` are idempotent. `stop` retains all synthetic
chain, wallet, and channel state. No reset/delete command is provided.

For diagnostics:

```bash
scripts/lightning-navcoin-regtest-env bitcoin-cli getblockchaininfo
scripts/lightning-navcoin-regtest-env lncli user getinfo
scripts/lightning-navcoin-regtest-env host-lncli user getinfo
scripts/lightning-navcoin-regtest-env lncli coordinator listinvoices
scripts/lightning-navcoin-regtest-env mine 6
```

The host-local direct gRPC endpoints on the isolated bridge are:

| Node | Endpoint |
|---|---|
| user | `172.30.24.11:10009` |
| coordinator | `172.30.24.12:10009` |
| router | `172.30.24.13:10009` |

Run `credentials` for the TLS certificate and regtest admin-macaroon paths.
Each node's TLS SAN contains its exact bridge IP. `status` invokes the verified
`lncli` binary from the host against all three endpoints and fails if any
network identity is not Bitcoin regtest.
Do not copy those files into evidence or source control. The standard evidence
command explicitly excludes credentials, wallet seeds, preimages, and logs.

## Pinned Python gRPC adapter

`pftl/lnd-python-grpc:v0.20.1-beta` supplies generated modules pinned to the
exact LND tag used by the nodes:

```python
from pftl_lnd_grpc import connect_lnd

channel, lightning, router, lightning_pb2, router_pb2 = connect_lnd(
    "coordinator", state_dir="/state"
)
```

The top-level generated imports are `lightning_pb2`, `lightning_pb2_grpc`,
`router_pb2`, and `router_pb2_grpc`. `connect_lnd` admits only the three fixed
synthetic endpoints and combines the node TLS certificate with macaroon
metadata credentials. Its source archive, Python base, and every installed
wheel are hash-pinned.

Run coordinator or harness Python inside the same no-egress network:

```bash
scripts/lightning-navcoin-regtest-env grpc-python \
  /workspace/path/to/demo_runner.py
```

The repository is mounted read-only at `/workspace`, node state read-only at
`/state`, and `PYTHONPATH` includes both `/workspace` and the pinned generated
modules.

## Environment overrides

- `PFTL_LN_STATE_DIR`: persistent LND/setup/evidence directory (default
  `.state` beside this file).
- `PFTL_LIGHTNING_PROJECT`: isolated Compose project name.

The demo intentionally uses LND's `noseedbackup` testing mode. It must never be
used for signet, testnet, mainnet, or value-bearing wallets.
