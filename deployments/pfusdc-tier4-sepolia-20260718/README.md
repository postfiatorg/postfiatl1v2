# pfUSDC Tier-4 Sepolia deployment freeze

This directory freezes the deterministic deployment and PFTL bootstrap inputs
derived from the fresh consensus-v2 chain's finalized height-1 checkpoint.
Nothing in this directory is evidence that a live deployment or SP1 proof has
already occurred.

## Frozen identifiers

- PFTL checkpoint block ID: `b9c3e38c523cc258dfbe106b45e000155dd8f0c193770d4d905f8b0777f91612519fc964ac890483b844c2ef7b6fdce8`
- PFTL committee root: `a84d4b4cadc9c068d5c668e040efe9ba303c59560bfb4c315c5b23aa235b8a6a279f3886d1352810e0b83822a90fc5d0`
- pfUSDC asset ID: `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`
- NAV profile ID: `f61b0bc9f5b51964605cb0df8304b4e3a36de350c1734a40f836aa73d2bd7b104caf7ac7e4a2daafc780928a30ef7659`
- Ingress policy hash: `a19667214636171d344e9fdbed490cf849359ddb762b587879e6456a70000f7b`
- Route profile hash: `d89f1b9cc9842112748090c7c655d9b8208a5bbd591085347b4750c4076ab005c38575625754cf4274d6a380c04cec48`
- Route binding: `d072739d73648a6b3bf853ab284da9072584ad83605a16a66de4748b110b795c`
- Ingress ELF SHA-256: `9e9278fc725541815fb36a5e6049301a4183e3a950778cb091be2a4bf719c373`
- Ingress program vkey: `0x00cf5150195737400718baa10a8cc8bfe419857a2507d5916bb95e024fa52726`

## Predicted contracts

- Ethereum-Sepolia ingress anchor, deployer nonce 0: `0x89ec019b4aa5423b8d96152a502a0db52cf48164`
- Arbitrum-Sepolia finality verifier, deployer nonce 0: `0x89ec019b4aa5423b8d96152a502a0db52cf48164`
- Arbitrum-Sepolia vault, deployer nonce 1: `0xa796dc3c9308f9c855a0659153b7afc2006cf27b`

Exact constructor-specific runtime Keccak commitments are:

- finality verifier: `0x8dd7e23c7d42a104fc91893cfc93184c0d2d2a7e2b2115574c9a048e82fdb781`
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
input.json     7a507e956198c3f35f4ea1e22e68629ced5118866237e51fa9fd0ca57ddd5bc9
manifest.json  efc94f6f426a89f6e8581af95e6f95e0138a312bf3b06ac7113134ffd0af3ada
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

## Governed finality bootstrap

After all three contracts pass live readback, capture the one governed
Ethereum/Arbitrum starting checkpoint. The command verifies the Helios update,
the finalized RollupCore account/storage proof, the canonical Nitro assertion,
and the vault/token/anchor account-code proofs before writing a new file:

```sh
cargo run --manifest-path tools/pfusdc-tier4-prover/Cargo.toml --locked -- \
  finality-bootstrap \
  --manifest deployments/pfusdc-tier4-sepolia-20260718/manifest.json \
  --ethereum-rpc "$ETHEREUM_SEPOLIA_RPC_URL" \
  --ethereum-consensus-rpc "$ETHEREUM_SEPOLIA_CONSENSUS_RPC_URL" \
  --arbitrum-rpc "$ARBITRUM_SEPOLIA_RPC_URL" \
  --output docs/evidence/pfusdc-tier4-finality-live/bootstrap.json
```

Route activation must carry that exact file. The later `ingress-capture`
command requires it through `--prior-finality-state` and refuses to write a
witness unless the proof starts from that retained root/slot and can advance
the governed state.

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
