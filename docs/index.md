# Post Fiat L1 Documentation

Post Fiat L1 (PFTL) is a layer-1 settlement ledger written in Rust. It follows
the XRP Ledger model: known validators, deterministic finality from quorum
certificates, a fixed-supply native token (PFT) whose fees burn, and no
validator rewards. Accounts and validators sign with post-quantum ML-DSA-65
from genesis, Cobalt can ratify on-chain validator-trust changes, and
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

## What changed this week

25 November 2026. Sources: [Current State](status/chain-state-current.md) and the
[handoffs](handoffs/index.md).

- **21 November.** All six validators moved to `main-r64-20261121`; `main` became the only long-lived branch ([Master State](status/master-state.md)).
- **23 November.** The first Solana withdrawal release (rules from block 1841); first real withdrawals; the deposit route re-anchored at block 1847.
- **24 November.** The second Solana exit release (rules from block 1890): every Solana deposit batch can be withdrawn, a refund path for a burn that cannot be paid, each validator reviews a settlement itself. Real withdrawals of 0.30, 0.30 and 4.00 USDC were paid and settled.
- **25 November.** Read-only read: all six validators at height 1897, one state root, build `04600868` (the second Solana exit release). B24 epoch 10 subsequently finalized at 1902.
- **Limits.** A controlled devnet with small caps, not announced: one withdrawal spends one deposit batch, the minimum is 0.30 USDC, the vault accepts 25 USDC more, a withdrawal takes about four minutes ([Network Status](status/network-status.md#usdc-bridges)).

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

<!-- component-observations -->

| Component and evidence | Chain / height | Last observed | Result |
| --- | --- | --- | --- |
| [Validator release](https://github.com/postfiatorg/postfiatl1v2/blob/003af00011c6af086f14f4d4017e450a11eaf45f/ops/b24-live/known-releases.txt) | postfiat-wan-devnet-2 / 1903 | 2026-11-25 20:15 UTC | Six validators reported on sol-exit-r2 |
| [Validator-trust authority](https://github.com/postfiatorg/postfiatl1v2/blob/003af00011c6af086f14f4d4017e450a11eaf45f/docs/governance/cobalt.md#current-controlled-devnet-state) | postfiat-wan-devnet-2 / 924 | 2026-08-26 06:35:50 UTC | Cobalt last verified active for validator trust |
| [Lane-export proof identity](https://github.com/postfiatorg/postfiatl1v2/actions/runs/45684553972) | Source build / n/a | 2026-11-25T19:30:09Z | Unverified (hosted comparison: failure) |

<!-- /component-observations -->



Reviewed against main `ce3659b22` and a read-only fleet read on 2026-11-25. Implementation
and local qualification below do not imply deployment; fleet observations retain their own
dates on [Network Status](status/network-status.md).

| Area | State | Read |
| --- | --- | --- |
| Core chain | Accounts, signed transfers, fees, blocks, receipts, deterministic replay and state roots. | [Architecture Overview](architecture/overview.md) |
| Finality | Consensus v2 prepare/precommit with commit quorum certificates; 5 of 6 on the devnet. | [Finality](architecture/finality.md) |
| Validator-trust governance | The E5 campaign verified Cobalt at height 924 for validator-registry and trust-graph changes. Other governance scopes and proposals remain Foundation-administered. | [Cobalt](governance/cobalt.md), [Validator Registry](governance/validator-registry.md) |
| Settlement lanes | Consensus transactions, W6 atomic swaps, FastPay, FastSwap and Asset-Orchard, each with its own finality and recovery rules. | [Settlement Lanes](architecture/settlement-lanes.md) |
| Privacy | Orchard/Halo2 deposit, spend, withdraw, scan and selective disclosure. The shielded stack is not post-quantum. Public submission of shielded actions is off on every validator. | [Privacy](privacy/overview.md) |
| Post-quantum authorization | ML-DSA-65 for accounts and validators; larger certificates accepted as a design cost. | [Quantum Authorization](quantum/authorization.md) |
| Storage | Transactional `redb` storage active since height 930; G5 offline qualified on 2026-10-23 for the October 21 binary. G6 remains open; later binaries need their own qualification. | [Network Status](status/network-status.md), [State And Storage](architecture/state-and-storage.md) |
| NAVCoins | A666 v2 (public general availability closed) and BMNRC (created 2026-10-02) are on the devnet. B24, the Schwab-backed NAVCoin, has been created and cashed in with real money since mid-November; epoch 10 finalized at height 1902 on 25 November. | [NAVCoins](navcoins/index.md) |
| USDC bridges | pfUSDC is minted from USDC on Ethereum and on Solana. Solana deposits and withdrawals run on the devnet with small caps and limits; no real Ethereum deposit or withdrawal is recorded since 2026-11-21. | [Network Status](status/network-status.md#usdc-bridges), [Bridge Audit Map](status/bridge-audit-map.md) |
| YOLO target receipts | Implemented and disabled by default; activation needs a future governance amendment. Receipts confer no trading, minting or reserve authority. | [YOLO Target Receipt v1](yolo/target-receipt-v1.md) |
| RPC and clients | Newline-delimited JSON over TCP; read methods, controlled write methods, account history; Python client and Rust SDK. `blocks`, `receipts` and `tx` are answered by key since #227. | [RPC](rpc/overview.md), [Python](python/quickstart.md) |
| Wallets | Transparent, issued-asset, FastPay, swap, memo and shielded flows in source and tools. Public submission of shielded actions is off on every validator. | [Web Wallet](wallets/web-wallet.md), [Shielded Wallet](wallets/shielded-wallet.md), [Network Status](status/network-status.md) |
| Validators | A separate software signer, durable double-sign guard, deployment checker and two-user unit generation are implemented. Qualification of the selected release and deployment under two users remain open. | [Validators](validators/overview.md), [Signer Policy](runbooks/public-rpc-operator-policy.md#two-user-units) |
| PFT on Base | A real Base Sepolia Lock released native PFT on a local six-validator fleet on 2026-11-06; replay was refused and the watcher confirmed backing. The Phase 2 audit remains open. Native export accounting exists behind `exports_enabled`, default off; the export proof program and relay are on `main`, and the Base verifier deployment remains open. | [PFT Token On Base](architecture/pft-token-on-base.md), [Genesis External Lanes](architecture/genesis-external-lanes.md) |

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
