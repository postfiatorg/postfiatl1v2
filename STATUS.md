# PostFiat L1 Status

## Current Phase: Controlled Testnet

PostFiat L1 is in the controlled-testnet phase. The protocol is functional end-to-end
with local and remote multi-validator testnets but has not reached public mainnet.
The numbers below are controlled-environment evidence, not public-mainnet service
level commitments.

## What Works

- **HotStuff-style ordering**: local 5-validator submit-to-finality at p50 1.56s, p95 1.71s.
- **Remote testnet**: 5-validator certified round at p50 1.03s over WAN.
- **Transparent transactions**: XRP-style transfers with account history and finality RPC.
- **Shielded settlement**: Orchard/Halo2 deposit, spend, and withdraw with nullifier set.
- **Cobalt governance**: validator registry transitions with safety witness verification.
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
[Evidence Index](docs/evidence/index.md). Each claim cites code paths, scripts,
tests, or redaction-safe reports.
