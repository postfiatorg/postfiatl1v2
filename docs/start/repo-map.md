# Repo Map

PostFiat is organized around protocol crates, operator scripts, status docs, and
evidence reports.

## Crates

```text
crates/
  types/              protocol data types
  crypto_provider/    account and validator signing boundary
  execution/          deterministic state transition
  ordering_fast/      fast certified ordering path
  consensus_cobalt/   Cobalt governance and adversarial examples
  privacy/            shielded state semantics
  privacy_orchard/    Orchard/Halo2 adapter
  storage/            persistence and snapshots
  network/            validator transport
  node/               node, CLI, RPC, wallet, operations
  rpc_sdk/            Rust RPC SDK
```

## Scripts

Script names are intentionally descriptive:

- `testnet-cobalt-*` for Cobalt gates and adversarial packets;
- `testnet-orchard-*` for privacy and Orchard evidence;
- `testnet-live-*` for live controlled network evidence;
- `testnet-rpc-*` and `postfiat-rpc-*` for RPC tooling;
- `testnet-validator-doctor*` and `testnet-monitor-snapshot*` for operator
  checks;
- `devnet-*` for local development.

## Docs

The canonical docs live under `docs/`. MkDocs publishes the curated subset
configured in `mkdocs.yml`; source-only archives, runbooks, specs, and status
logs remain in the same tree but are excluded from the hosted site when needed.

- `docs/whitepaper.md`
- `docs/status/full-cobalt-burndown.md`
- `docs/status/privacy-production-burndown.md`
- `docs/status/controlled-testnet-burndown.md`
- `docs/runbooks/`
- `docs/specs/`

## Reports

Reports are evidence. Do not treat every report as public. The hosted site uses
a curated index of redaction-safe reports.
