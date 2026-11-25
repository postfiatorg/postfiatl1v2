# NAVCoin PFTL Tools

> **Tooling lineage notice (2026-07-30):** Most commands below are generic or
> historical a651/devnet tools. The current A666 resident service, production
> deployment, and evidence entry points are documented in
> [A666 Current State](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md).
> Do not infer current production routing from an older script name.
> Added 2026-11-25: the B24 daily reading job and launch scripts are in
> `ops/b24-live/` and `ops/b24-replacement/`, and the read-only NAV proof
> watchdog is `tools/nav-proof-watch/`.

This page maps the main NAVCoin tools to the evidence they produce. It is a
starting point for operators and reviewers who need to find the actual scripts,
commands, Python builders, and contracts behind the docs.

## Smoke and evidence scripts

| Tool | Purpose | Availability |
|---|---|---|
| `scripts/navcoin-current-infra-smoke` | Runs the native NAV lifecycle: issued asset, NAV registration, reserve packet, epoch finalization, mint, offer-book swap, redeem, and validator convergence. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-multifetch-smoke` | Exercises `multi-fetch-quorum` against live Hyperliquid data with registered attestors, observation roots, tolerance policy, and quorum finalization. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/hyperliquid-drift-study` | Measures snapshot drift so tolerance bands are based on observed external-source behavior instead of exact-match assumptions. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-a651-wan-devnet-phase1` | Registers and finalizes interim a651 NAV state on the WAN devnet. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-sp1-test-wan-devnet.sh` | Tests the SP1-backed a651 proof path against WAN devnet state. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-sp1-test-wan-devnet-remote.sh` | Remote-run variant of the SP1 WAN devnet proof path. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-a651-sp1-wan-devnet-remote.sh` | Remote a651/SP1 WAN devnet execution helper. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/navcoin-market-ops-cold-start-check` | Checks market-operation cold-start assumptions and parameter readiness. | Archived outside this repository; see `scripts/README.md`. |
| `scripts/private-a651-pfusdc-optimized-swap-live` | Runs the legacy optimized private a651/pfUSDC shielded swap path for regression/reproduction. | Current in-repository command. |

Common local runs:

```bash
# HISTORICAL: the first two commands were archived outside the repository
# during the open-source cleanup; see scripts/README.md.
scripts/navcoin-current-infra-smoke
scripts/navcoin-multifetch-smoke
PYTHONPATH=python python3 docs/examples/navcoin_mint_and_nav.py --chain-id postfiat-navcoin-devnet
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

## Ethereum relays on the validator hosts

Each validator host runs three loopback Ethereum JSON-RPC relays:

| Port | Upstream | How it runs |
|---|---|---|
| `127.0.0.1:28701` | `https://ethereum-rpc.publicnode.com` | The enabled unit `navcoin-ethereum-rpc-28701.service`, on all six hosts |
| `127.0.0.1:28702` | `https://eth.drpc.org` | The enabled unit `navcoin-ethereum-rpc-28702.service`, on all six hosts |
| `127.0.0.1:28703` | `https://eth.drpc.org` | The enabled unit `navcoin-ethereum-rpc-28703.service`, on all six hosts |

All three run `/opt/postfiat/route-tools/navcoin-20260907/https-jsonrpc-loopback-proxy.py
--attempts 3` from enabled units in `/etc/systemd/system` that restart on boot.

Since 2026-10-01, 28701 refuses archive reads: `eth_getCode` at a historical block
returns HTTP 403, "Archive requests require a personal token". Plain block reads
still succeed. `ethereum-checkpoint-vote-sign` makes such archive reads, so
`scripts/a666-mainnet-return-import.sh` and
`scripts/a666-mainnet-record-destination-consume.sh` default to 28703. Set
`A666_VALIDATOR_ETHEREUM_RPC` to override the default. The observed relay state is
recorded in
[Current State](../status/chain-state-current.md#2026-10-16-archive-relay-runs-as-a-systemd-unit).

## FastSwap control certificates

`ActivateCommittee` (admit the next FastSwap and Ethereum bridge committee
epoch) and `StopPrepare` (fence one FastSwap policy epoch) need a control
certificate from the active FastSwap committee: a quorum of ML-DSA-65 votes
under `FASTLANE_CONTROL_CONTEXT_V1` over the committee domain (chain ID,
genesis hash, protocol version, epoch, root) and the action digest. The
`ActivateCommittee` digest binds the new committee's domain and root and the
final checkpoint ID; the `StopPrepare` digest binds the committee epoch,
policy epoch and finalized height. Each validator signs from its own key
file, as with `ethereum-checkpoint-vote-sign`:

```bash
# One node: build the unsigned message (refuses unless the epoch is active + 1,
# the root matches the active registry keys, and a drained final checkpoint is anchored).
postfiat-node fastswap-control-prepare --data-dir DIR --kind activate-committee \
  --epoch 2 --committee-root HEX --control-file activate-2.json
postfiat-node fastswap-control-prepare --data-dir DIR --kind stop-prepare \
  --policy-epoch N --control-file stop-prepare-N.json
# Each signer, on its own host: one vote.
postfiat-node fastswap-control-vote-sign --data-dir DIR --control-file activate-2.json \
  --validator validator-N --validator-key-file DIR/validator_keys.json --vote-file vote-N.json
# Anyone: check quorum and signatures, write the certificate.
postfiat-node fastswap-control-assemble --data-dir DIR --control-file activate-2.json \
  --vote-files vote-0.json,...,vote-4.json --certificate-file activate-2.certificate.json
```

`vote-sign` refuses a key that does not match the active committee and a
message the ledger would not admit. It keeps one record per (committee epoch,
kind, target epoch) under `fastswap-control-signing/`, next to
`ethereum-checkpoint-signing/`, and refuses a second different message;
re-signing the same message returns the recorded vote. `assemble` runs the
certificate through `execute_fastlane_control` on a copy of the ledger before
writing it. The certificate is the `Control` operation of
`mempool_submit_fastlane_primary_finality`. The final drained checkpoint is not
signed with `ethereum-checkpoint-vote-sign`, which signs Ethereum checkpoints;
its votes come from the FastSwap service request `fastswap_checkpoint_status`.

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
`docs/status/otc-swaps-mvp-proven-2026-06-19.md` and
`docs/runbooks/wan-devnet-full-live-end-to-end-run.md`.
