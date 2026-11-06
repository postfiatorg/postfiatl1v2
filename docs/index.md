# Post Fiat L1 Documentation

Post Fiat L1 (PFTL) is a layer-1 settlement ledger written in Rust. It follows
the XRP Ledger model: known validators, deterministic finality from quorum
certificates, a fixed-supply native token (PFT) whose fees burn, and no
validator rewards. Accounts and validators sign with post-quantum ML-DSA-65
from genesis, validator-trust changes are ratified on chain by Cobalt, and
private settlement uses Asset-Orchard, a typed-asset extension of Zcash Orchard.
Its main application is NAVCoins, assets whose supply is tied to
machine-checkable reserve packets. The design is in the
[Whitepaper](whitepaper.md) (also published at
[postfiat.org/whitepaper](https://postfiat.org/whitepaper/)).

!!! warning "Devnet only, not audited"

    One network runs today: the controlled devnet `postfiat-wan-devnet-2`, run
    by six Foundation-administered validators. It is not a production mainnet,
    no public testnet is open, and the code has not been externally audited.
    Current facts are on [Network Status](status/network-status.md); the audit
    and release-gate position is on [Security](security/index.md).

## Start here

| If you are | Read, in order |
| --- | --- |
| Evaluating the project | [Whitepaper](whitepaper.md), [Network Status](status/network-status.md), [Security](security/index.md), [Architecture Overview](architecture/overview.md) |
| Integrating an exchange, custodian or wallet | [Exchange Integration Guide](integrate/exchange-guide.md), [Addresses, Keys And Signatures](integrate/addresses-keys-signatures.md), [RPC Overview](rpc/overview.md), [Python Client](python/quickstart.md) |
| A new engineer | [First Day](start/first-day.md), [Glossary](start/glossary.md), [Repo Map](start/repo-map.md), [Settlement Lanes](architecture/settlement-lanes.md) |
| A validator operator | [Validators](validators/overview.md), [Public Operator Runbook](runbooks/public-operator-runbook.md), [Public RPC Operator Policy](runbooks/public-rpc-operator-policy.md) |

Codenames such as A666, BMNRC, Z3 or G4 are defined in the
[Glossary](start/glossary.md).

## What exists now

Reviewed against main on 2026-11-05. Implementation and local qualification
below do not imply deployment; fleet observations retain their own dates on
[Network Status](status/network-status.md).

| Area | State | Read |
| --- | --- | --- |
| Core chain | Accounts, signed transfers, fees, blocks, receipts, deterministic replay and state roots. | [Architecture Overview](architecture/overview.md) |
| Finality | Consensus v2 prepare/precommit with commit quorum certificates; 5 of 6 on the devnet. | [Finality](architecture/finality.md) |
| Validator-trust governance | Cobalt ratifies validator-registry and trust-graph changes; active on the devnet since height 916. Other governance scopes and proposals remain Foundation-administered. | [Cobalt](governance/cobalt.md), [Validator Registry](governance/validator-registry.md) |
| Settlement lanes | Consensus transactions, W6 atomic swaps, FastPay, FastSwap and Asset-Orchard, each with its own finality and recovery rules. | [Settlement Lanes](architecture/settlement-lanes.md) |
| Privacy | Orchard/Halo2 deposit, spend, withdraw, scan and selective disclosure. The shielded stack is not post-quantum. | [Privacy](privacy/overview.md) |
| Post-quantum authorization | ML-DSA-65 for accounts and validators; larger certificates accepted as a design cost. | [Quantum Authorization](quantum/authorization.md) |
| Storage | Transactional `redb` storage active since height 930; G5 offline qualified on 2026-10-23 for the October 21 binary. G6 remains open; later binaries need their own qualification. | [Network Status](status/network-status.md), [State And Storage](architecture/state-and-storage.md) |
| NAVCoins | A666 v2 is the deployed NAVCoin lineage (public general availability closed); BMNRC was created on the devnet on 2026-10-02. | [NAVCoins](navcoins/index.md) |
| YOLO target receipts | Implemented and disabled by default; activation needs a future governance amendment. Receipts confer no trading, minting or reserve authority. | [YOLO Target Receipt v1](yolo/target-receipt-v1.md) |
| RPC and clients | Newline-delimited JSON over TCP; read methods, controlled write methods, account history; Python client and Rust SDK. | [RPC](rpc/overview.md), [Python](python/quickstart.md) |
| Wallets | Transparent, issued-asset, FastPay, swap, memo and shielded flows in source and tools. The private path was off and ungoverned at the last fleet observation. | [Web Wallet](wallets/web-wallet.md), [Shielded Wallet](wallets/shielded-wallet.md), [Network Status](status/network-status.md) |
| Validators | Launch packets, service layout, history retention, monitors, signed rollout, key rotation. A separate software signer process with a durable double-sign guard is implemented; its deployment under two users remains open. | [Validators](validators/overview.md), [Signer Policy](runbooks/public-rpc-operator-policy.md#validator-signer-process) |
| PFT on Base | Token/lock contracts, native-PFT release path, checkpoint CLI and watcher implemented. A real Base Sepolia Lock was released on a local six-validator fleet on 2026-11-06; the outside audit of the lock remains open; reverse export is refused until built. | [PFT Token On Base](architecture/pft-token-on-base.md), [Genesis External Lanes](architecture/genesis-external-lanes.md) |

What remains before public launch is listed in the
[Release Gate Inventory](status/release-gate-inventory.md), the
[Testnet Path](status/testnet-path.md) and the
[Public Launch Boundary](security/public-launch-boundary.md).

## How this site is organized

| Section | Contents |
| --- | --- |
| Start Here | Onboarding, glossary, repository map, release process |
| Architecture | Transaction lifecycle, settlement lanes, finality, storage, post-quantum authorization |
| Network Status | The current summary, the full state log, testnet path, release gates |
| Integrate | Exchange guide, addresses and signatures, RPC, Python client, wallets |
| Privacy, NAVCoins, Governance | Protocol areas in depth |
| Validators & Operations | Validator docs and operator runbooks |
| Security | Audit status, threat model, launch boundary, audit scopes |
| Reference | Specifications an integrator needs, and the evidence index |
| Engineering Records | Internal history: handoffs, plans, reviews, postmortems, status log |

## Engineering records

Dated handoffs, plans, reviews, postmortems and status entries are internal
engineering records. They are point-in-time notes written while the work
happened and are often superseded. Use them for history, not as current
operating truth:

- [Handoffs](handoffs/index.md): newest-first index, regenerated by
  `scripts/docs-handoffs-index`.
- [Plans](plans/README.md): active and completed execution plans.
- [Reviews](review/README.md): internal review packets and the defect inventory.
- [Postmortems](postmortems/README.md): incident write-ups.
- [Status log](status/README.md): the state log and dated status records.

## Build this site locally

```bash
python3 -m pip install --require-hashes -r requirements-docs.txt
scripts/docs-site-build
scripts/docs-site-serve --host 127.0.0.1 --port 8088
```

Then open `http://127.0.0.1:8088/`. For remote access, use SSH port forwarding
or an authenticated reverse proxy; `scripts/docs-site-serve` uses HTTP basic
authentication when `POSTFIAT_DOCS_PASSWORD` is set.
