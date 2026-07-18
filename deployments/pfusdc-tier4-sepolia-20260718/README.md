# pfUSDC Tier-4 Sepolia deployment freeze

This directory freezes the deterministic deployment and PFTL bootstrap inputs
derived from the fresh consensus-v2 chain's finalized height-1 checkpoint.
Nothing in this directory is evidence that a live deployment or SP1 proof has
already occurred.

## Frozen identifiers

- PFTL checkpoint block ID: `b9c3e38c523cc258dfbe106b45e000155dd8f0c193770d4d905f8b0777f91612519fc964ac890483b844c2ef7b6fdce8`
- PFTL committee root: `a84d4b4cadc9c068d5c668e040efe9ba303c59560bfb4c315c5b23aa235b8a6a279f3886d1352810e0b83822a90fc5d0`
- pfUSDC asset ID: `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`
- NAV profile ID: `3b876874ee28167ee4b751544e17ca4a50983d9eb48337f13b53d4e9d1ea461775fbcc61fb3ea280006e98e437b1dd8e`
- Ingress policy hash: `a19667214636171d344e9fdbed490cf849359ddb762b587879e6456a70000f7b`
- Route profile hash: `e7a4ed044f66dbff0d75df786bed6857fefffb8009b5e4959f4105bce4ae1483a418501d3df60b8848247ee41f003731`
- Route binding: `a1fdb5c4550bb3d54ea515490e415525de4b26201889c547e733f4b7ea773fd3`

## Predicted contracts

- Ethereum-Sepolia ingress anchor, deployer nonce 0: `0x89ec019b4aa5423b8d96152a502a0db52cf48164`
- Arbitrum-Sepolia finality verifier, deployer nonce 0: `0x89ec019b4aa5423b8d96152a502a0db52cf48164`
- Arbitrum-Sepolia vault, deployer nonce 1: `0xa796dc3c9308f9c855a0659153b7afc2006cf27b`

Exact constructor-specific runtime Keccak commitments are:

- finality verifier: `0x0feb99e603cdaafe111cb0fdac03e693e049d206b0ea3a43c83ec8613eedbd2b`
- vault: `0xc53ec5dad1757e65df90675446ee1f02bcadafbde12a4df4ccb396f7a98b9812`
- ingress anchor: `0x3a5e3f49d40d340dd996975d29bb4a17669ab3a8f32f1dc1d0c13e1889825fc0`

The three runtime hashes were measured from exact constructor deployments on
local forks of the target networks. The finality-verifier fork deployment also
read back every immutable and initial checkpoint storage field. No live funds
were used.

## Reproduction

Build the pinned Solidity artifacts with Foundry, then run:

```sh
cargo run --manifest-path tools/pfusdc-tier4-prover/Cargo.toml --locked -- \
  deployment-manifest \
  --input deployments/pfusdc-tier4-sepolia-20260718/input.json \
  --output deployments/pfusdc-tier4-sepolia-20260718/manifest.json
```

The frozen file hashes are:

```text
input.json     14ef6ca9de00e4e768fcf6f699eb8cde90628d8521cc64089f33ac8ccd6524ec
manifest.json  33dc0a56039a59d980c471a9687ec408c6e215c9d4096fc75f8a3888ee010513
funding-route.json c65edb1d09dc46e7e589888d57381633037b40a1e86e7dba916775bb8431bf3d
```

The generated bootstrap operations independently reproduce the manifest's
asset ID, NAV profile ID, ingress policy hash, route profile hash, SP1 vkey,
encoding, and proof bounds. They are unsigned and must enter PFTL only through
the normal certified transaction path.

## Live deployment driver

Use the StakeHub virtualenv so deployment signing remains inside the unlocked
agent. The driver accepts no private-key argument and never reads a raw key:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-deploy.py preflight
```

Preflight validates the frozen input, manifest, artifacts, compiler settings,
constructor encodings, predicted CREATE addresses, target chain IDs, system
contract runtime hashes, deployer nonces, target addresses, StakeHub agent
state, and testnet gas balances. It does not write evidence or send a
transaction.

After the wallet has the minimum gas on both chains, deploy Arbitrum first and
Ethereum second:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-deploy.py deploy-arbitrum
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-deploy.py deploy-ethereum
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-deploy.py readback
```

Every agent request is checkpointed under
`docs/evidence/pfusdc-tier4-deployment-live/state.json` before broadcast. A
restart first checks predicted-address code, runtime hash, deployer nonce, and
all constructor/storage getters, so an accepted deployment is never blindly
rebroadcast. A mismatched chain, nonce, artifact, system contract, runtime
hash, or getter fails closed.

## Live funding route

The exact provider, source contract, live-code hash, target chains, quote
snapshot, user cap, and canonical-USDC route are frozen in
`funding-route.json`. Refresh the read-only quote with:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-fund.py quote
```

At the 2026-07-18 freeze, the two native-gas orders plus conservative mainnet
gas totaled about `$2.13`, far below the aggregate `$500` authorization. The
provider route is an off-chain delivery service: its verified mainnet vault
emits the order and the provider relayer delivers the testnet ETH. The driver
therefore records both the source transaction and delivered target balance.

One passphrase-gated StakeHub policy step is required before that exact vault
can receive value:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/stakehub policy \
  --add-whitelist 0x33c1AD63CCbd322208A0Dd2C9f3C3FD21CCA3329
```

After the policy command, execute each bounded order independently:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-fund.py buy-ethereum-gas --confirm-live-funds
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-fund.py buy-arbitrum-gas --confirm-live-funds
```

Canonical Arbitrum-Sepolia USDC remains the official Circle token at
`0x75faf114eafb1BDbe2F0316DF893fd58CE46AA4d`. If `CIRCLE_API_KEY` is available,
the official API request is ready as:

```sh
/home/postfiat/repos/StakeHub/.venv/bin/python \
  scripts/pfusdc-tier4-fund.py circle-usdc --confirm-circle-request
```

Otherwise, one browser reCAPTCHA completion at `https://faucet.circle.com/` is
required. The route must not substitute a mock or noncanonical token.
