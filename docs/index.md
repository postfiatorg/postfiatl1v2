# PostFiat L1 Engineering Docs

PostFiat is an XRP-style authority-validator Layer 1 rebuilt around Rust,
post-quantum authorization, versioned validator governance, Orchard/Halo2
privacy, fixed supply, fee burn, and no native validator reward schedule. An
authenticated read-only `postfiat-wan-devnet-2` probe completed at
`2026-08-25T15:37:40Z` with all six validators active and equal at height 919.
Cobalt was active for the bounded validator-trust lane from height 916;
Foundation governance retained unrelated scope and Consensus v2 remained the
only block-finality protocol.

Start with [Current State](status/chain-state-current.md). It separates the active
consensus runtime, governance auditor, Cobalt shadow, current Git HEAD,
adversarial campaign, and evidence freshness. The repository branch must not be
used as proof of what is running.

This site is the engineering front door. It is not a dump of internal notes. It
points to the current code, scripts, reports, and operating runbooks that define
what has been built.

For the A666 implementation and operating boundary, start with the
[July 30 deployed-state baseline](status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md)
and the
[deferred production-hardening specification](deferred-plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md).
Completed campaign plans and dated execution handoffs are deliberately absent
from the active documentation tree; Git history retains them when historical
investigation is necessary. The proposed next-generation
[NAVCoin Reserve Redemption System](deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md)
defines separately escrowed two-sided primary facilities for approved
`pfUSD`, `pfXRP`, `pfETH`, `pfStakedETH`, and `pfBTC` assets; it is a
specification, not a claim about currently deployed A666 functionality.

The sidebar is curated for reading. Generated governance packets and per-gate
receipts stay searchable in the site and are collected in the
Generated Governance Archive, but
they are not listed one-by-one in the primary navigation.

## What Exists Now

| Area | Current State | Where To Read |
| --- | --- | --- |
| A666 primary market | Mainnet components and full economic loop are functionally proven; resident private swaps remain limited availability and production GA is closed. Multi-asset reserve facilities are proposed, not deployed. | [A666 current state](status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md), [deferred production hardening](deferred-plans/A666-PRIVATE-SWAP-PRODUCTION-HARDENING-SPEC-20260730.md), [deferred reserve redemption system](deferred-plans/NAVCOIN-RESERVE-REDEMPTION-SYSTEM-SPEC-20260730.md) |
| Core chain | Rust L1 with accounts, signed transfers, fees, blocks, receipts, deterministic replay, and state roots. | [Architecture](architecture/overview.md) |
| Finality | Versioned certified ordering: legacy single-view mode and activated consensus v2 with durable prepare/precommit, timeout certificates, and proposer rotation. | [Finality](architecture/finality.md) |
| Settlement lanes | Consensus transactions, W6 dual-authorized atomic swap, FastPay payments, FastSwap DvP, and Asset-Orchard private settlement have distinct finality and recovery boundaries. | [Settlement Lanes](architecture/settlement-lanes.md) |
| Governance | The fresh devnet probe verified Cobalt ratifying validator-registry and trust-graph changes; Foundation governance retains unrelated scopes and currently administers proposals and validators. | [Current State](status/chain-state-current.md), [Validator Registry](governance/validator-registry.md) |
| Cobalt governance | Active at the `2026-08-25T15:37:40Z` observation for validator-trust evolution only; it does not choose trust, originate proposals, or control block finality. | [Cobalt Governance](governance/cobalt.md), [browser observatory](governance/cobalt.md#read-only-browser-interface) |
| Verifiable Constitution | Canonical readable constitution for typed, replayable, challengeable model-assisted governance with no-live-effect authority boundaries. | [Constitution](governance/verifiable-constitution.md), Proof Summary |
| Privacy | Orchard/Halo2 deposit, spend, withdraw, scan, disclose, pool report, and live validator evidence. | [Privacy](privacy/overview.md) |
| Quantum auth | ML-DSA-style account and validator authorization with larger certificate economics accepted as a design cost. | [Quantum Authorization](quantum/authorization.md) |
| RPC | Read RPC, transaction finality, account history, pool reports, controlled write policy, doctor tooling. | [RPC](rpc/overview.md) |
| Python | Python client for status, ledger, fee, finality, account history, and CSV-oriented reads. | [Python Client](python/quickstart.md) |
| Validators | Launch packets, service layout, history retention, doctor, monitor, restart/outage drills, emergency key rotation. | [Validators](validators/overview.md) |
| Wallets | Transparent/PFT, issued-asset, FastPay, swap, memo, and shielded tooling with explicit proxy/custody boundaries. | [Web Wallet](wallets/web-wallet.md), [Shielded Wallet](wallets/shielded-wallet.md) |
| Evidence | Redaction-safe reports tie claims to scripts and code. | [Evidence](evidence/index.md) |

## Fast Reading Path

1. Read the [Whitepaper](whitepaper.md) for the thesis.
2. Read [First Day](start/first-day.md) for build and local workflow.
3. Read [Settlement Lanes](architecture/settlement-lanes.md), the
   [Constitution](governance/verifiable-constitution.md), [Cobalt](governance/cobalt.md),
   and [Privacy](privacy/overview.md) for the protocol boundaries.
4. Read [RPC](rpc/overview.md), [Python](python/quickstart.md), and
   [Validators](validators/overview.md) for integration and operation.
5. Use [Evidence](evidence/index.md) when you need proof, not prose.

## Core Claim

The implementation is a controlled-testnet L1. It is built to prove protocol
correctness, replayability, validator behavior, wallet/RPC behavior, privacy
flows, and operator runbooks before public launch.

Public launch adds independent placement evidence, longer mixed soaks, external
privacy review, production public write-edge policy, and custodian workflows.
Those are launch tasks. They do not erase the controlled-testnet code and
evidence that already exist.

## Self-Hosted URL

The docs are meant to run on a project-controlled machine:

```bash
scripts/docs-site-build
scripts/docs-site-serve --host 127.0.0.1 --port 8088
```

Then visit:

```text
http://127.0.0.1:8088/
```

Remote access should be placed behind SSH forwarding or an authenticated reverse
proxy. Opening the firewall is an operator decision:

```bash
sudo ufw allow 8088/tcp
```

The docs server does not modify UFW.
