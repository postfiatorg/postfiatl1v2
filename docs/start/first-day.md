# First Day

This page is for a new engineer joining the project. It tells you where the
system lives, what to run first, and which documents are current.

## Repository Shape

| Path | Purpose |
| --- | --- |
| `crates/types` | Chain data types and serialized protocol structures. |
| `crates/crypto_provider` | ML-DSA-style signing and verification boundary. |
| `crates/execution` | Deterministic state transition logic. |
| `crates/ordering_fast` | Fast transaction ordering path. |
| `crates/consensus_cobalt` | Cobalt governance mechanics and adversarial examples. |
| `crates/privacy` | Shielded-state semantics and privacy-facing types. |
| `crates/privacy_orchard` | Orchard/Halo2 adapter and verifier. |
| `crates/node` | Node execution, CLI, RPC, storage integration, wallet flows. |
| `crates/rpc_sdk` | Rust RPC SDK and TCP wallet example. |
| `python/postfiat_rpc` | Python RPC client. |
| `scripts/` | Devnet, testnet, launch, evidence, doctor, and smoke scripts. |
| `ops/` | Operator scripts and service units for the live bridges, the payout service and the NAV reading jobs. |
| `solana/` | The Solana vault, recorder and egress-verifier programs. |
| `docs/` | Canonical docs tree; MkDocs publishes the curated subset configured in `mkdocs.yml`. |
| `reports/` | Generated evidence; only its README is in Git, and earlier reports were archived outside the repository (`reports/README.md`). |

## Build

```bash
cargo fmt --check
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
```

For docs:

```bash
python -m pip install -r requirements-docs.txt
scripts/docs-site-build
scripts/docs-site-serve --host 127.0.0.1 --port 8088
```

## Local Devnet

Start with the existing scripts instead of inventing a new harness:

```bash
scripts/devnet-up
scripts/devnet-submit-transfer
scripts/devnet-status
```

`scripts/devnet-sdk-rpc-smoke` was archived outside the repository during the
open-source cleanup; `scripts/README.md` lists the retained scripts.

If you are touching consensus, storage, RPC, privacy, validator transport, or
cryptography, run the focused smoke that matches the change and update the
relevant evidence or status page.

## Current Docs To Trust

| Topic | Current Source |
| --- | --- |
| Unified thesis | [Whitepaper](../whitepaper.md) (`docs/whitepaper.md`) |
| Running network | [Network Status](../status/network-status.md), [Master State](../status/master-state.md) (what runs and which branch), then the full log in [Current State](../status/chain-state-current.md) |
| Path to public testnet | [Testnet Path](../status/testnet-path.md) and [Release Gate Inventory](../status/release-gate-inventory.md) |
| Security position | [Security](../security/index.md) |
| Integration | [Exchange Integration Guide](../integrate/exchange-guide.md) and [Addresses, Keys And Signatures](../integrate/addresses-keys-signatures.md) |
| RPC inventory | [RPC Method Inventory](../runbooks/rpc-method-inventory.md) |
| Python client | [Python RPC Client](../runbooks/python-rpc-client.md) |

Older execution burndowns (`docs/status/controlled-testnet-burndown.md`,
`docs/status/full-cobalt-burndown.md`, `docs/status/cobalt-adversarial-burndown.md`,
`docs/status/privacy-production-burndown.md`) and the operator launch runbook
(`docs/runbooks/controlled-testnet-operator-launch.md`) were last updated in July
and August 2026. Read them as history.

Archived drafts and old research prompts are useful context, not current
operating truth.

## Private Material Rule

Never publish signing secrets, seed phrases, validator launch secrets, live
credentials, raw SSH inventories, or private Orchard witness material. Evidence
pages must point only to redaction-safe reports.
