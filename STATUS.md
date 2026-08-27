# PostFiat L1 Status

## Current Phase: Controlled Testnet

PostFiat L1 is in the controlled-testnet phase. The protocol is functional end-to-end
with local and remote multi-validator testnets but has not reached public mainnet.
The numbers below are controlled-environment evidence, not public-mainnet service
level commitments.

## Operational State Boundary

The [Current State](docs/status/chain-state-current.md) page is authoritative for
questions about the devnet, active binaries, repository state, and adversarial
campaign. The final authenticated E5 observation ran from
`2026-08-26T06:34:55Z` through `06:35:50Z` and observed:

- all six validator, RPC, and advisory shadow services active;
- all six validators converged at height 924 with empty mempools;
- Cobalt active for validator-trust governance after the signed rollback at 922
  and return at 923;
- the legitimate validator-5 rotation committed at 924; and
- identical node binary SHA-256 `d5e5ef63…c2696caf` on every validator.

The repository/campaign state is distinct. E1-E6, the interfaces, evidence, and
publication are complete and the milestone-wide result is `KEEP_ACTIVE`.
Repository commits after the embedded deployed revision `8cc7d15e` are not
themselves deployed without a new fleet receipt.

## What Works

- **HotStuff-style ordering**: local 5-validator submit-to-finality at p50 1.56s, p95 1.71s.
- **Remote testnet**: 5-validator certified round at p50 1.03s over WAN.
- **Transparent transactions**: XRP-style transfers with account history and finality RPC.
- **Shielded settlement**: Orchard/Halo2 deposit, spend, and withdraw with nullifier set.
- **Cobalt governance**: Cobalt is active for validator-registry and trust-graph changes. The signed final-gate rollback/return committed at heights 922/923, all nine live negative cases rejected without durable mutation, and the legitimate validator-5 rotation committed at 924. All six nodes converged; Consensus v2 still finalizes blocks. See [Current State](docs/status/chain-state-current.md) and the [adversarial results](docs/governance/cobalt-adversarial-verification-results.md).
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
- E4 exposed near-linear finality growth with chain height in the deployed JSON/JSONL lineage. Current source has an undeployed transactional `redb` candidate; snapshot/rebuild equality, exact height-915 replay, the 69-case tamper/crash matrix, and compatible two-binary rollback pass. Exact height 924, paired six-validator performance, six-clone migration, and the final packet remain open. Public testnet remains blocked.

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
the complete campaign is in the
[adversarial results](docs/governance/cobalt-adversarial-verification-results.md),
with the live drills in the
[E5 packet](benchmarks/cobalt-adversarial-verification/e5/README.md).
Each claim cites code paths, scripts, tests, or redaction-safe reports.
