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
  bridge/             external checkpoint and receipt-proof verification
  pft-token-contracts/ PFT ERC-20 token and Base lock contracts
  ethereum-contracts/ Ethereum bridge/controller contracts
```

## Clients and external-lane tooling

| Path | Role | Guide |
| --- | --- | --- |
| `python/postfiat_rpc/` | RPC client, PFT token reader/UI and external-lane release watcher | [Python](../python/quickstart.md), [PFT On Base](../architecture/pft-token-on-base.md) |
| `wallet-web/`, `wallet-extension/`, `wallet-proxy/` | Wallet clients and gateway | [Web Wallet](../wallets/web-wallet.md), [Shielded Wallet](../wallets/shielded-wallet.md) |
| `crates/node/src/external_lane_checkpoint_source.rs` | Observe, sign and assemble Base checkpoints; construct a Lock receipt proof | [Genesis External Lanes](../architecture/genesis-external-lanes.md) |
| `crates/node/src/validator_signer.rs` | Separate software signer process and socket client | [Signer Policy](../runbooks/public-rpc-operator-policy.md#validator-signer-process) |

These paths describe implemented source. See [Network Status](../status/network-status.md)
for dated deployment records and the existing milestones for qualification gaps.

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

- `docs/whitepaper.md`: the protocol design.
- `docs/status/network-status.md` and `docs/status/chain-state-current.md`: the
  running network (summary and full log).
- `docs/integrate/`: exchange and wallet integration.
- `docs/runbooks/`: operator procedures.
- `docs/specs/`: specifications; the site publishes the ones listed under
  Reference.
- `docs/handoffs/`, `docs/plans/`, `docs/review/`, `docs/postmortems/` and the
  dated files in `docs/status/`: internal engineering records.

## Reports

Reports are evidence. Do not treat every report as public. The hosted site uses
a curated index of redaction-safe reports.
