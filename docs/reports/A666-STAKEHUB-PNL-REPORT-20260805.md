# A666 StakeHub PnL Report v1 — 2026-08-05

## Scope and read boundary

This is a read-only mainnet and venue snapshot captured at **2026-08-05T14:45:37Z**. It covers every component the current StakeHub accounting code tracks, plus native XMR and the pfUSDC/A666 bridge inventory class that could not be read because the product-profile guard rejected the active services.

No transaction, service, ledger, fund, configuration, or key mutation occurred. Account identifiers are represented only by SHA-256 hashes in the companion JSON.

- Interpreter: `/usr/bin/python3`
- Interpreter SHA-256: `1643dacd9feaedc58f3cc581e4d22577dfe25c09b10282936186ccf0f2e61118`
- CLI repository: `/home/postfiat/repos/StakeHub-repeat-demo`
- CLI invocation discipline: `/usr/bin/python3 -m stakehub.cli …`

## Profile diagnosis C

The five active StakeHub units were read from systemd and their process environments without emitting unrelated environment values:

| Unit role | PID | observed profile hash | source |
|---|---:|---|---|
| dashboard | 3002164 | `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0` | dashboard status observed by CLI preflight |
| private-swap asset-orchard | 1895104 | `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0` | `STAKEHUB_PRODUCT_PROFILE_SHA256` |
| private-swap wallet-agent | 1895107 | `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0` | `STAKEHUB_PRODUCT_PROFILE_SHA256` |
| pfUSDC wallet-agent | 1895112 | `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0` | `STAKEHUB_PRODUCT_PROFILE_SHA256` |
| dashboard-bfinal transient | 1895114 | `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0` | `STAKEHUB_PRODUCT_PROFILE_SHA256` |

**Five-of-five active service agreement:** `91aa6c9d…`.

The machine-readable dashboard identity read by `wallet demo preflight` rejected the invoked guard profile: expected `bfb3d0439adabd4ee4995fcbdba488fa68ba9859037181be83a48d7885a12a12`, observed `91aa6c9d94f95a343346a961122529b99f26c2bb403800a351dc80dc9d058fd0`. The preflight consequently stopped before its fleet-status phase, so no profile-bound fleet height was emitted. **Verdict: the live guard expects the pin `bfb3d043…`, not the active bfinal profile `91aa6c9d…`.** This blocks profile-bound pfUSDC/A666 inventory reads; it does not justify assuming that inventory is zero.

## Current book

All dollar rows below are current marked values. Rounded component rows are explanatory; the stated gross-assets and equity totals are the captured full-precision accounting totals.

| Component | Asset or liability | Current value USD | Price or valuation source | Included in net equity |
|---|---|---:|---|---|
| Hyperliquid perpetuals, five positions | asset/equity | 9,239.94 | Hyperliquid current marks | yes |
| Hyperliquid spot: USDC, HYPE, XMR1 | asset | 23,526.31 | Hyperliquid current marks | yes |
| SOL liquid plus delegated/deactivating stake | asset | 1,082.00 | Hyperliquid SOL mark, current stake accounts | yes |
| NEAR liquid plus staked balance | asset | 8,290.00 | Hyperliquid NEAR mark, current NEAR stake account | yes |
| Aave v3 collateral | asset | 562.65 | Aave on-chain account/oracle read | yes |
| Aave v3 debt | liability | (201.01) | Aave on-chain account/oracle read | yes |
| Mainnet, Arbitrum, and Base ETH/USDC cash | asset | 637.73 | chain balances, ETH mark, USDC at 1.00 | yes |
| Native XMR | asset, excluded reference | 54.71 | 0.15419024 XMR × Hyperliquid XMR mark 354.83 | no |
| pfUSDC/A666 bridge inventory | unknown while profile guard is mismatched | not read | profile-bound certified wallet status | no |

| Metric | USD |
|---|---:|
| Gross assets | **43,338.56** |
| Gross liabilities | **201.01** |
| Net equity | **43,137.55** |

No bridge inventory was added to gross assets or net equity. Native XMR is shown as an excluded reference row because `stakehub blotter` explicitly excludes it and the snapshot has no independent XMR price or cost-basis source.

## PnL disclosure

| Measure | USD | Status |
|---|---:|---|
| Hyperliquid unrealized PnL | **+2,877.40** | current mark |
| Journaled realized funding | **+485.78** | cumulative journal capture |
| Trading realized PnL | not available | no complete trading realization ledger |
| Fees | not available | no complete fee ledger |
| Aave principal versus accrued interest | not available | only total debt was returned |
| Total PnL | **not computed** | historical cost basis and the missing ledgers prevent a non-invented total |

Current net equity is a valuation, not total PnL. This report deliberately does not derive a total PnL by treating marked equity as cost-basis PnL.

## Component-registry coverage

| Tracked component | Code adapter | Read command | Valuation/freshness | Sign | Report row |
|---|---|---|---|---|---|
| Hyperliquid perps | `stakehub/blotter.py`, Hyperliquid adapter | `blotter` | current venue marks, 2026-08-05T14:45:37Z | asset/equity | Hyperliquid perpetuals |
| Hyperliquid spot | `stakehub/blotter.py`, Hyperliquid adapter | `blotter` | current venue marks, same snapshot | asset | Hyperliquid spot |
| SOL liquid/stake | `stakehub/blotter.py`, `stakehub/solana.py` | `blotter` | current stake accounts and SOL mark | asset | SOL |
| NEAR liquid/staked/unstaked | `stakehub/blotter.py`, `stakehub/near.py` | `blotter`, `near-balance` | current NEAR account/stake and NEAR mark | asset | NEAR |
| Aave collateral | `stakehub/evm.py` | `aave-status` | Aave account/oracle, captured snapshot | asset | Aave collateral |
| Aave debt | `stakehub/evm.py` | `aave-status` | Aave account/oracle, captured snapshot | liability | Aave debt |
| EVM ETH/USDC cash | `stakehub/evm.py` | `evm-balances` | chain balances, ETH mark and USDC 1.00 | asset | EVM cash |
| Native XMR | `stakehub por`; excluded by `stakehub/blotter.py` | `xmr-balance` | node scan height 3,733,551; reference mark | excluded asset | Native XMR |
| pfUSDC/A666 certified inventory | `stakehub/generalized_wallet.py` | profile-bound `wallet status` | unavailable after guard mismatch | unknown | bridge inventory |
| Funding journal | `stakehub/hl.py` | `blotter` | cumulative captured journal | PnL memo | realized funding |

**Component count: 10. Uncovered components: 0.** The bridge row is covered as unavailable, rather than omitted.

## Read commands and timing

All commands were read-only and executed from `/home/postfiat/repos/StakeHub-repeat-demo`.

| Command shape | Exit | Elapsed |
|---|---:|---:|
| `/usr/bin/python3 -m stakehub.cli blotter` | 0 | 24,013 ms |
| `/usr/bin/python3 -m stakehub.cli aave-status` | 0 | 5,406 ms |
| `/usr/bin/python3 -m stakehub.cli status` | 0 | 6,507 ms |
| `/usr/bin/python3 -m stakehub.cli near-balance` | 0 | 3,303 ms |
| `/usr/bin/python3 -m stakehub.cli evm-balances` | 0 | 3,971 ms |
| `/usr/bin/python3 -m stakehub.cli xmr-balance` | 0 | 84,480 ms |
| duplicate read-only `evm-balances` | 0 | 4,526 ms |
| `/usr/bin/python3 -m stakehub.cli wallet demo preflight --profile [process-derived path]` | nonzero | profile identity refusal before fleet status |

The duplicate EVM command was read-only and produced no mutation.

## Spend headroom

The read-only campaign ledger reports **501.024845 USDC** spent across **70** ceremonies. The control-state cap is **530 USDC**, leaving **28.975155 USDC** headroom. The ledger was not modified.

## Required next reads before a complete PnL

| Missing class | Required source/read |
|---|---|
| Historical cost basis | immutable trade, funding, transfer, and custody acquisition ledger keyed to each component |
| Trading realized PnL | complete venue fills/closed-PnL ledger |
| Fees | complete Hyperliquid, chain gas, bridge, and Aave fee ledger |
| Aave principal/interest split | Aave reserve/account debt-index read with principal history |
| pfUSDC/A666 bridge inventory | profile guard must be reconciled, then run the certified profile-bound wallet status against the matching six-validator fleet |

## Boundaries

No live-chain mutation, transaction, service mutation, key access, fund movement, configuration change, or journey execution occurred. The profile mismatch is a fail-closed data-availability limitation, not a valuation assumption.
