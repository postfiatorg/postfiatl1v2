# PostFiat L1 Engineering Docs

PostFiat is an XRP-style authority-validator Layer 1 rebuilt around Rust,
post-quantum authorization, Cobalt validator governance, Orchard/Halo2 privacy,
fixed supply, fee burn, and no native validator reward schedule.

This site is the engineering front door. It is not a dump of internal notes. It
points to the current code, scripts, reports, and operating runbooks that define
what has been built.

The sidebar is curated for reading. Generated governance packets and per-gate
receipts stay searchable in the site and are collected in the
Generated Governance Archive, but
they are not listed one-by-one in the primary navigation.

## What Exists Now

| Area | Current State | Where To Read |
| --- | --- | --- |
| Core chain | Rust L1 with accounts, signed transfers, fees, blocks, receipts, deterministic replay, and state roots. | [Architecture](architecture/overview.md) |
| Finality | Certified BFT-style ordering with controlled 1-2 second finality evidence. | [Finality](architecture/finality.md) |
| Cobalt | Full-Cobalt controlled-testnet mechanics: non-identical trust views, essential subsets, linkedness checks, non-uniform certificates, RBC, ABBA, MVBA, DABC, replay gates, and adversarial packets. | [Cobalt Governance](governance/cobalt.md) |
| Verifiable Constitution | Canonical readable constitution for typed, replayable, challengeable model-assisted governance with no-live-effect authority boundaries. | [Constitution](governance/verifiable-constitution.md), Proof Summary |
| Deterministic governance agent | Reviewable plan and active burndown for turning model-assisted governance into typed, replayable, Cobalt-ratified ruleset and registry-delta objects. | [Plan](governance/deterministic-governance-agent-plan.md), Burndown |
| Privacy | Orchard/Halo2 deposit, spend, withdraw, scan, disclose, pool report, and live validator evidence. | [Privacy](privacy/overview.md) |
| Quantum auth | ML-DSA-style account and validator authorization with larger certificate economics accepted as a design cost. | [Quantum Authorization](quantum/authorization.md) |
| RPC | Read RPC, transaction finality, account history, pool reports, controlled write policy, doctor tooling. | [RPC](rpc/overview.md) |
| Python | Python client for status, ledger, fee, finality, account history, and CSV-oriented reads. | [Python Client](python/quickstart.md) |
| Validators | Launch packets, service layout, history retention, doctor, monitor, restart/outage drills, emergency key rotation. | [Validators](validators/overview.md) |
| Evidence | Redaction-safe reports tie claims to scripts and code. | [Evidence](evidence/index.md) |

## Fast Reading Path

1. Read the [Whitepaper](whitepaper.md) for the thesis.
2. Read [First Day](start/first-day.md) for build and local workflow.
3. Read the [Constitution](governance/verifiable-constitution.md),
   [Cobalt](governance/cobalt.md), and [Privacy](privacy/overview.md) for the
   governance and protocol pillars.
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
