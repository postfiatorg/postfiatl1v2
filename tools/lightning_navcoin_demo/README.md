# Lightning → NAVcoin synthetic submarine-swap demo

This harness proves a fixed-hash, fixed-amount submarine swap between a local
Lightning regtest and a six-validator local PFTL devnet:

```text
user LND ── router LND ── coordinator LND
                              │
                   durable SQLite coordinator
                              │
           six local PFTL validators + LNNAVTEST escrow
```

Everything is synthetic. The safety envelope rejects non-regtest Bitcoin,
public endpoints, `ce22`, the wrong asset symbol, fewer or more than three LND
nodes, and fewer or more than six PFTL validators. The PFTL adapter consumes an
externally built hardened node binary; this branch does not modify consensus.

## Run

Prerequisites are Docker, Compose, Python 3.12, and the pinned Python package in
`coordinator/requirements.txt`. Supply the committed binary and adjacent
`postfiat-rpc-sdk` from the hardened escrow branch:

```bash
python3 -m pip install -r \
  tools/lightning_navcoin_demo/coordinator/requirements.txt

scripts/lightning-navcoin-regtest-env channels

POSTFIAT_NODE_BIN=/absolute/path/to/postfiat-node \
POSTFIAT_NODE_GIT_REV=<committed-revision> \
scripts/lightning-navcoin-demo all \
  --pftl-root /absolute/local/path/pftl-lightning-six \
  --evidence-dir /absolute/empty/path/lightning-navcoin-evidence
```

`all` gates the binary by reported revision, SHA-256, adjacent SDK SHA-256, and
a fresh semantic probe before initializing the six-validator devnet. It then
runs:

- Lightning → PFTL and PFTL → Lightning happy paths;
- an expired-invoice refund path;
- canonical vector and AMP rejection checks;
- wrong hashlock, malformed claim, boundary, replay, and route-failure checks;
- abrupt coordinator exits after every durable state edge, including a live
  Lightning payment and live PFTL create/finish reconciliation;
- one-validator-down certification/catch-up and all-RPC hard restart.

The command leaves both local stacks running so another operator can inspect
the exact terminal state. Stop the Lightning lab without deleting state with:

```bash
scripts/lightning-navcoin-regtest-env stop
```

Verify an evidence bundle independently:

```bash
scripts/lightning-navcoin-demo verify-evidence \
  /absolute/path/lightning-navcoin-evidence
```

Run the offline suite:

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/lightning_navcoin_demo -p 'test*.py' -v
```

## Claim boundary

The result is non-custodial, conditionally atomic settlement under the stated
synthetic-chain, timelock, liveness, participant-availability, and non-freezable
asset assumptions. It is not coordinator-free, always available, private,
unconditionally trustless, or production-ready.

The wallet enforces a conservative PFTL-height margin over the payer's maximum
Lightning CLTV, but PFTL does not authenticate Bitcoin height or wall-clock
time. The regtest harness therefore proves ordering under controlled mining,
not a consensus-comparable cross-chain clock. That remains a release gate for
any public trustless claim.

General evidence redacts claim material. Synthetic preimages appear only in
the dedicated test-vector artifact. The bundle has an exact-file manifest,
SHA-256 hashes, and an append-only event hash chain; it is a self-consistency
proof, not an externally timestamped or independently signed archive.
