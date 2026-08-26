# PostFiat L1 Status

## Current Phase: Controlled Testnet

PostFiat L1 is in the controlled-testnet phase. The protocol is functional end-to-end
with local and remote multi-validator testnets but has not reached public mainnet.
The numbers below are controlled-environment evidence, not public-mainnet service
level commitments.

## Operational State Boundary

The [Current State](docs/status/chain-state-current.md) page is authoritative for
questions about the devnet, active binaries, repository state, and adversarial
campaign. An authenticated read-only validator/RPC probe ran from
`2026-08-26T01:40:51Z` through `01:41:04Z` and observed:

- all six validator and all six RPC services active;
- all six validators equal at height 919 with empty mempools; and
- active consensus release `cobalt-verifier-92b63f5a`, binary SHA-256
  `c7cb0c25…9f6337` on every validator.

That probe did not re-run the authority auditor or inspect the shadow services.
The last full audit at `2026-08-25T15:37:40Z` records Cobalt validator-trust
authority and all six advisory shadows active.

The repository/campaign state is distinct: E1-E4 and design-only E6 are
complete; E5 and publication remain open, so the milestone-wide `KEEP_ACTIVE`
gate is not yet earned. Pushed E4 evidence commit `6c22f866` and every later
source or documentation commit are not deployed without a new fleet receipt.

## What Works

- **HotStuff-style ordering**: local 5-validator submit-to-finality at p50 1.56s, p95 1.71s.
- **Remote testnet**: 5-validator certified round at p50 1.03s over WAN.
- **Transparent transactions**: XRP-style transfers with account history and finality RPC.
- **Shielded settlement**: Orchard/Halo2 deposit, spend, and withdraw with nullifier set.
- **Cobalt governance**: the last full authority audit verified Cobalt active for validator-registry and trust-graph changes from height 916; the first Cobalt-authorized key rotation committed at height 917. The later 2026-08-26 validator/RPC probe preserved six-node convergence at height 919. Consensus v2 still finalizes blocks. See [Current State](docs/status/chain-state-current.md).
- **Post-quantum auth**: ML-DSA account and validator signatures from genesis.
- **NAVCoins**: OTC swap primitives and proof-of-reserve on controlled testnet.
- **RPC**: full read/write surface with account state, history, receipts, and finality queries.
- **Python client**: wallet functions, transfer, and shielded operations.
- **MkDocs site**: engineering documentation with architecture diagrams and evidence citations.

## Known Limitations

- No public mainnet; all testing is on controlled local and remote testnets.
- Orchard proof generation is CPU-intensive; not yet optimized for production latency.
- Governance agent gates are implemented but not all are exercised in live testnet.
- WAN devnet fleet operations require manual operator setup.

## Not Yet Implemented

- Public mainnet launch and token distribution.
- Production-grade peer discovery and DHT.
- Mobile wallet SDKs.
- Hardware security module (HSM) integration for validator keys.
- Cross-chain bridge to XRPL mainnet.

## Evidence

Performance and correctness evidence is curated in the
[Evidence Index](docs/evidence/index.md). The live authority result is in the
[controlled-testnet activation packet](benchmarks/cobalt-activation-live/packet/README.md);
the passing Byzantine-validator campaign is in the
[E2 packet](benchmarks/cobalt-adversarial-verification/e2/README.md).
Each claim cites code paths, scripts, tests, or redaction-safe reports.
