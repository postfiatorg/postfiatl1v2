# Contributing

PostFiat L1 is a Rust workspace for deterministic settlement, post-quantum authorization, shielded execution, validator governance, and controlled testnet operations. Contributions should keep consensus behavior explicit, reproducible, and evidence-backed.

## Build And Test

Install the exact Rust version pinned in `rust-toolchain.toml`, including
`rustfmt` and `clippy`. Install `tmux` only for local/devnet operations.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo check --workspace --all-targets --locked
```

The non-Rust release surfaces have independent gates. Install Python test
dependencies from the hash-locked file; do not rely on an ambient interpreter
environment.

```bash
python3 -m pip install --require-hashes -r requirements-test.txt
PYTHONPATH=python python3 -m pytest python/tests
(cd wallet-proxy && npm ci && npm test && npm audit --audit-level=moderate)
(cd wallet-web && npm ci && npm test && npm run build && npm audit --audit-level=moderate)
(cd crates/ethereum-contracts && forge test --no-match-path test/PFTLUniswapOfficialFork.t.sol)
scripts/docs-site-build
scripts/docs-site-redaction-check
scripts/test-public-doc-links
scripts/public-doc-links
```

The official-mainnet-fork contract test is a separate secret-backed CI job. It
must fail closed when `ETHEREUM_MAINNET_RPC_URL` is absent; an offline pass is
not valid fork evidence.

The repository wrapper runs the standard local gate:

```bash
scripts/check
```

## Controlled Testnet

Use the retained scripts for local controlled-testnet work:

```bash
scripts/devnet-up
scripts/devnet-submit-transfer
scripts/devnet-status
scripts/devnet-down
```

For broader pre-release checks, use `scripts/testnet-readiness-gate` and `scripts/testnet-p0-network-gate`. Do not commit generated run reports or evidence bundles; `reports/` keeps only `.gitkeep` and `README.md`.

## Evidence Model

Protocol, security, performance, and operational claims must cite evidence. A complete evidence trail can include code paths, scripts, tests, redaction-safe reports, or runbooks. The whitepaper and engineering docs should reference evidence by stable artifact names and hashes where possible, so another reviewer can reproduce or verify the claim.

When adding a new claim, update the nearest relevant documentation and identify the code, script, test, or report that supports it. Do not leave claims as uncited prose.

## Code Style

- Rust code uses edition 2021.
- Run `cargo fmt --all -- --check` before opening a PR.
- Run `cargo clippy --workspace --all-targets -- -D warnings` for changed Rust code.
- Keep consensus and state-transition code deterministic.
- Treat network, RPC, file, and wallet inputs as untrusted; validate bounds before use.
- Avoid panics in consensus or peer-facing paths unless the invariant is internal and already proven by construction.

## Pull Requests

PRs should describe the intent, list the commands run, and call out any consensus, state-root, receipt, storage, cryptography, RPC, or validator-operation impact. Include tests or a focused justification when tests are not practical.

Keep generated artifacts, local secrets, private keys, node data, proof outputs, and one-off reports out of git. If a PR changes user-facing docs, run the MkDocs strict build before requesting review.

Release candidates follow [docs/release-process.md](docs/release-process.md).
