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
| `docs/` | Canonical docs tree; MkDocs publishes the curated subset configured in `mkdocs.yml`. |
| `reports/` | Redaction-safe evidence plus internal generated artifacts. |

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
scripts/devnet-sdk-rpc-smoke
scripts/devnet-submit-transfer
```

If you are touching consensus, storage, RPC, privacy, validator transport, or
cryptography, run the focused smoke that matches the change and update the
relevant evidence or status page.

## Current Docs To Trust

| Topic | Current Source |
| --- | --- |
| Unified thesis | `docs/whitepaper.md` |
| Controlled testnet | `docs/status/controlled-testnet-burndown.md` |
| Cobalt | `docs/status/full-cobalt-burndown.md` |
| Cobalt adversarial work | `docs/status/cobalt-adversarial-burndown.md` |
| Privacy | `docs/status/privacy-production-burndown.md` |
| Operator launch | `docs/runbooks/controlled-testnet-operator-launch.md` |
| RPC inventory | `docs/runbooks/rpc-method-inventory.md` |
| Python client | `docs/runbooks/python-rpc-client.md` |

Archived drafts and old research prompts are useful context, not current
operating truth.

## Private Material Rule

Never publish signing secrets, seed phrases, validator launch secrets, live
credentials, raw SSH inventories, or private Orchard witness material. Evidence
pages must point only to redaction-safe reports.
