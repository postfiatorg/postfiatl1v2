# PostFiat L1

> **Maturity:** controlled pre-testnet research software. This repository is
> not a production/mainnet release. Validator operation currently requires an
> explicit `--unsafe-devnet-file-signer` acknowledgement because HSM/remote
> signing is not implemented, and long-running validator/RPC services require
> `--unsafe-devnet-json-storage` because the bounded JSON/JSONL store is not a
> transactional indexed production engine. Do not place real-value keys or
> value on this controlled-devnet configuration.

PostFiat is a Rust Layer 1 settlement system for post-quantum, privacy-aware institutional value transfer: transparent accounts use ML-DSA authorization from genesis, shielded settlement is built around Orchard/Halo2-style proofs, and quorum certificates provide deterministic finality. The current candidate admits live governance only through distinct ML-DSA-65 authorizations from the active old-rule registry; unsigned legacy governance artifacts are historical-replay-only. Cobalt RBC/ABBA remain separately signed research primitives and are not the node's authoritative governance admission path.

```mermaid
flowchart LR
  Wallet[Wallets and SDKs] --> RPC[RPC write/read surface]
  RPC --> Mempool[Mempool and batch builder]
  Mempool --> Ordering[Quorum-certified ordering]
  Ordering --> Execution[Deterministic execution]
  Execution --> Storage[State, blocks, receipts]
  Storage --> Reads[Read RPC and history]

  Governance[Cobalt governance] --> Registry[Validator registry]
  Registry --> Ordering

  Shielded[Shielded Orchard/Halo2 actions] --> Ordering
  Execution --> Pool[Shielded pool roots and nullifiers]
```

## Key Features

- Post-quantum from genesis: ML-DSA account and validator authorization.
- Shielded settlement: Orchard/Halo2 proof verification with public nullifier and root checks.
- Cobalt governance research and replay tooling: live registry/amendment mutation is fail-closed pending signed-vote implementation.
- Versioned quorum-certified finality: legacy genesis retains the single-view fail-closed rule; networks with an explicit consensus-v2 activation height use durable prepare/precommit locks, signed timeout certificates, and deterministic proposer rotation.
- Fixed supply plus fee burn: transparent fees burn during deterministic execution.

## Build From Source

Prerequisites:

- Rust toolchain, including `cargo`, `rustfmt`, and `clippy`
- `tmux` for local/devnet operations

```bash
scripts/check
scripts/node-init
scripts/node-run
```

Useful single-node commands:

```bash
scripts/node-status
scripts/node-faucet
scripts/node-transfer
scripts/node-account
```

## Run A Local Devnet

```bash
scripts/devnet-up
scripts/devnet-submit-transfer
scripts/devnet-status
scripts/devnet-down
```

## Documentation

- Whitepaper: [docs/whitepaper.md](docs/whitepaper.md)
- MkDocs site: [http://127.0.0.1:8088/](http://127.0.0.1:8088/) by default when served locally
- Engineering docs source: [docs/](docs/)
- MkDocs config: [mkdocs.yml](mkdocs.yml)

Run the docs site locally:

```bash
.venv-docs/bin/mkdocs serve
```

If you do not have the repo-local docs venv, install the docs requirements and run `mkdocs serve`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for build, test, evidence, and PR expectations.

## License

Licensed under either MIT or Apache-2.0, at your option.
