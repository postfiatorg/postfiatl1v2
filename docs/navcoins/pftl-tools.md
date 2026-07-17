# NAVCoin PFTL Tools

This page maps the main NAVCoin tools to the evidence they produce. It is a
starting point for operators and reviewers who need to find the actual scripts,
commands, Python builders, and contracts behind the docs.

## Smoke and evidence scripts

| Tool | Purpose |
|---|---|
| `scripts/navcoin-current-infra-smoke` | Runs the native NAV lifecycle: issued asset, NAV registration, reserve packet, epoch finalization, mint, offer-book swap, redeem, and validator convergence. |
| `scripts/navcoin-multifetch-smoke` | Exercises `multi-fetch-quorum` against live Hyperliquid data with registered attestors, observation roots, tolerance policy, and quorum finalization. |
| `scripts/hyperliquid-drift-study` | Measures snapshot drift so tolerance bands are based on observed external-source behavior instead of exact-match assumptions. |
| `scripts/navcoin-a651-wan-devnet-phase1` | Registers and finalizes interim a651 NAV state on the WAN devnet. |
| `scripts/navcoin-sp1-test-wan-devnet.sh` | Tests the SP1-backed a651 proof path against WAN devnet state. |
| `scripts/navcoin-sp1-test-wan-devnet-remote.sh` | Remote-run variant of the SP1 WAN devnet proof path. |
| `scripts/navcoin-a651-sp1-wan-devnet-remote.sh` | Remote a651/SP1 WAN devnet execution helper. |
| `scripts/navcoin-market-ops-cold-start-check` | Checks market-operation cold-start assumptions and parameter readiness. |
| `scripts/private-a651-pfusdc-optimized-swap-live` | Runs the current live optimized private a651/pfUSDC shielded swap path. |

Common local runs:

```bash
scripts/navcoin-current-infra-smoke
scripts/navcoin-multifetch-smoke
PYTHONPATH=python python3 docs/examples/navcoin_mint_and_nav.py
```

## Vault-bridge and pfUSDC commands

The vault-bridge path is the PFTL primitive behind source-labeled pfUSDC:

| Command | Purpose |
|---|---|
| `postfiat-node vault-bridge-asset-id` | Derives the PFTL issued-asset identity for a configured source asset and issuer. |
| `postfiat-node vault-bridge-bootstrap-bundle` | Writes PFTL setup operations for proof profile, asset creation, NAV registration, and initial trustlines. |
| `postfiat-node vault-bridge-deposit-intent` | Commits the intended PFTL recipient and source deposit identity before a bridge deposit is counted. |
| `postfiat-node vault-bridge-deposit-plan` | Converts a source-chain event or receipt into PFTL deposit operations. |
| `postfiat-node vault-bridge-deposit-relay-bundle` | Builds the PFTL relay bundle from a source receipt file. |
| `postfiat-node vault-bridge-deposit-relay-rpc-bundle` | Fetches the source-chain receipt and produces the PFTL relay bundle. |
| `postfiat-node vault-bridge-status` | Displays bridge asset status, counted capacity, buckets, and queue state. |
| `postfiat-node vault-bridge-receipts` | Lists source receipts and their PFTL accounting state. |
| `postfiat-node vault-bridge-export-reserve-packet` | Exports a reserve packet for replay or audit. |
| `postfiat-node vault-bridge-replay-reserve-packet` | Replays an exported reserve packet bundle. |
| `postfiat-node vault-bridge-burn-to-redeem-bundle` | Builds the PFTL burn path for source-chain withdrawal. |
| `postfiat-node vault-bridge-withdrawal-plan` | Derives the exact source-chain withdrawal packet from PFTL state. |
| `postfiat-node vault-bridge-withdrawal-signature-bundle` | Builds the controlled-launch verifier submission bundle for a finalized PFTL withdrawal. |
| `postfiat-node vault-bridge-withdrawal-relay-bundle` | Relays the accepted withdrawal packet to the source-chain vault. |

The detailed bridge profile is tracked in the source tree at
`docs/specs/vault-bridge-navcoin-profile.md`.

## Python modules

Python source under `python/postfiat_rpc/` mirrors the NAV operation builders and
source adapters:

| Module | Role |
|---|---|
| `navcoin.py` | Builds NAV operation JSON, reserve packets, profile ids, attestor ops, reserve attestations, and redeem-settle operations. |
| `hyperliquid.py` | Fetches and normalizes public Hyperliquid account state for observation roots. |
| `solana.py` | Fetches and normalizes public Solana balances and stake-account state. |
| `basis_policy.py` | Applies the nSOL/nETH valuation policy, hedge-gap checks, margin checks, and policy hashing. |

The hosted example is [NAVCOIN Python Example](../business/navcoin-python-example.md).

## Solidity contracts

| File | Tooling role |
|---|---|
| `crates/ethereum-contracts/src/ERC20BridgeVault.sol` | Source-chain deposit custody and withdrawal payment. |
| `crates/ethereum-contracts/src/PFTLWithdrawalVerifier.sol` | Accepted PFTL withdrawal proof/challenge/finality record. |
| `crates/ethereum-contracts/src/PFTLBridgeAdapter.sol` | PFTL market-operation envelope admission. |
| `crates/ethereum-contracts/src/MarketOpsEnvelope.sol` | EVM representation of the PFTL envelope. |
| `crates/ethereum-contracts/src/MarketOpsVault.sol` | Alignment reserve execution. |
| `crates/ethereum-contracts/src/MintController.sol` | Escrowed above-NAV mint release. |
| `crates/ethereum-contracts/src/NAVGuardHook.sol` | Venue evidence adapter for Uniswap-shaped flows; controlled-launch only. |
| `crates/ethereum-contracts/src/PolicyRegistry.sol` | Accepted market-policy registry. |

## Evidence locations

Reports generated by smoke scripts normally land under `reports/`. The public
evidence index is [Evidence](../evidence/index.md). NAVCoin-specific detailed
pages are:

- [Detailed Proof Of Reserves](../business/navcoin-proof-of-reserves.md)
- [Current Infrastructure](../business/navcoin-current-infra.md)
- [Archive Summary](../evidence/navcoin-archive-summary.md)

Excluded internal handoffs and runbooks remain useful for operators, but they
are not part of the hosted MkDocs nav. Search them by path, for example
`docs/status/otc-swaps-mvp-proven-2026-06-19.md`,
`docs/status/pfusdc-bridge-handoff-2026-06-19.md`, and
`docs/runbooks/wan-devnet-full-live-end-to-end-run.md`.
