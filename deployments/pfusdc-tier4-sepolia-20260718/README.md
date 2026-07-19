# pfUSDC Tier-4 Sepolia deployment freeze

This directory freezes the deterministic deployment and PFTL bootstrap inputs
derived from the fresh consensus-v2 chain's finalized height-1 checkpoint.
Live deployment evidence is recorded separately under
`docs/evidence/pfusdc-tier4-deployment-live-corrected/`. Nothing in this
directory is itself SP1 proof evidence.

## Frozen identifiers

- PFTL checkpoint block ID: `b9c3e38c523cc258dfbe106b45e000155dd8f0c193770d4d905f8b0777f91612519fc964ac890483b844c2ef7b6fdce8`
- PFTL committee root: `a84d4b4cadc9c068d5c668e040efe9ba303c59560bfb4c315c5b23aa235b8a6a279f3886d1352810e0b83822a90fc5d0`
- pfUSDC asset ID: `02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b`
- NAV profile ID: `6dbe399361478e97b8fb0dc73193d39b577e840b212a0b4f3fb81cd52e6e76f64f0d8067e3128fa4361f1985bdbd5bbd`
- Ingress policy hash: `eae79eb50386c5e7b0b97f51735fd074615e55beecc051e369ac103d896fd73c`
- Route profile hash: `7b93053c2a1a26b918c3bd2cd4737d1e00f3f5cf0f8cb8fba9aff1a1126eac10516881b4a2bb153200f9c08fe8c1b5ef`
- Route binding: `0cdf6748abdf669143acea7a4e657066b7e4c049594966b8f5cfde31f7c6d2c5`
- Ingress ELF SHA-256: `7c581aa42a196bd5df5a1efc2c4569663744d9b597cc1cd2253e839f9ba2f921`
- Ingress program vkey: `0x0033bd140207b97fb2442eb279cc2ce55714be6fbcd66beb325fe7c3786d4dfc`

## Predicted contracts

- Ethereum-Sepolia ingress anchor, deployer nonce 4: `0xabe2a1a76fb5c89f00780bb46f9870b7768f523a`
- Arbitrum-Sepolia finality verifier, deployer nonce 5: `0xa17a876deea3a711591248f726d9fac420809cfe`
- Arbitrum-Sepolia vault, deployer nonce 6: `0x2983579e8c60b1e1ff06b3bdc59805ffb0d4f915`

Exact constructor-specific runtime Keccak commitments are:

- finality verifier: `0x3c79de8193ae2ca24c817d690fc06c2fbac59ff89937b718f95499af6bd472f6`
- vault: `0x9552ab7b9b2c515ac6b38060fe08197d807eac56c21ea79db7e260637d3c7387`
- ingress anchor: `0x243d5a90f24189d818a8d8ff8bf5b1a712c1801793d0c423c4f4b116f7cc698e`

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
input.json     2ebf71c6cf156b71317147ff5e7579a231c36ce0a9c4fcdc0d2624c8dc8678e4
manifest.json  5871fa73bcf5472198c6946095a388bdf7d32bd535429b53c3c45ce8ea408ad4
route-profile.json 1c5ad84fe8b6fd66e8d7f09617dffb4d45b10f9de98f45a3abd09b7ba33519b0
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
  --output docs/evidence/pfusdc-tier4-finality-live-corrected/bootstrap.json
```

Route activation must carry that exact file. The later `ingress-capture`
command requires it through `--prior-finality-state` and refuses to write a
witness unless the proof starts from that retained root/slot and can advance
the governed state.

Create the activation amendment and governance batch from the exact standalone
profile (which is byte-for-byte equivalent to `manifest.json`'s profile) and
the captured finality file:

```sh
target/debug/postfiat-node vault-bridge-route-profile-governance \
  --data-dir "$PFTL_DATA_DIR" \
  --validators validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --support validator-0,validator-1,validator-2,validator-3,validator-4,validator-5 \
  --profile-file deployments/pfusdc-tier4-sepolia-20260718/route-profile.json \
  --tier4-finality-bootstrap-file docs/evidence/pfusdc-tier4-finality-live-corrected/bootstrap.json \
  --amendment-file docs/evidence/pfusdc-tier4-finality-live-corrected/route-amendment.json \
  --batch-file docs/evidence/pfusdc-tier4-finality-live-corrected/route-governance-batch.json
```

Submit that batch through the normal certified six-validator path; do not apply
it directly or omit the finality bootstrap.

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
