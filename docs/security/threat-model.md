# Threat Model

PostFiat's current controlled-testnet threat model focuses on protocol
correctness and operator reliability before public launch. The list below was
last changed on 17 July 2026. It does not yet cover the Solana USDC bridge on the
devnet (deposits, and withdrawals since 23 November 2026); the
[Bridge Audit Map](../status/bridge-audit-map.md) records what public source
reproduces for it and what remains unverified.

## Covered Adversaries

- malformed transactions and batches;
- stale replay;
- validator crash/restart;
- below-quorum outages;
- partial partitions;
- governance spam;
- trust graph poisoning;
- captured validator sets;
- Cobalt equivocation;
- oversized RPC requests;
- malformed Orchard proof payloads;
- accidental private-material disclosure in docs and evidence.

## Not A Public Launch Claim

Controlled-testnet evidence does not claim broad public decentralization. Public
launch requires independent placement and operator diversity evidence.

## Sources

- `docs/status/cobalt-adversarial-burndown.md`
- `docs/status/controlled-testnet-burndown.md`
- `docs/status/privacy-production-burndown.md`
- `docs/status/public-claims-checklist.md`
