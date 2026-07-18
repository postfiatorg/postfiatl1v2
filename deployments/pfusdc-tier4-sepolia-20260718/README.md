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
```

The generated bootstrap operations independently reproduce the manifest's
asset ID, NAV profile ID, ingress policy hash, route profile hash, SP1 vkey,
encoding, and proof bounds. They are unsigned and must enter PFTL only through
the normal certified transaction path.
