# NAVcoin Archive Summary

This page summarizes the archived NAVcoin-related projects that were pulled into the `postfiat-archive` monorepo. It is a high-level inventory for engineering context, not a full smart-contract audit.

Archive location:

```text
$POSTFIAT_ARCHIVE_ROOT/navcoin_related/
```

Archive repo state checked locally:

```text
postfiat-archive tip: 77a9fc7a2
remote: https://github.com/postfiatorg/postfiat-archive.git
```

## Subprojects

| Subproject | Role |
| --- | --- |
| `nav-coin` | Main NAV/VAN Web3 application. Contains the React site, history and leaderboard services, Ethereum mainnet contracts, Sepolia contracts, and deployment scripts. |
| `nav-token` | Newer Hardhat 3 token/staking implementation using UUPS upgradeable contracts. |
| `pftl-nav-token` | Python-oriented token tooling and scripts. |
| `jupiter-swap-widget` | Solana/Jupiter swap UI. |
| `uniswap-widget` | Uniswap-style frontend widget. |
| `uniswap-aws-api` | API/frontend fork around Uniswap-style functionality. |
| `v3-uniswap-subgraph` | Uniswap v3 subgraph copy for pool, swap, liquidity, and TVL indexing patterns. |

## Live Mainnet Contracts

The `nav-coin` archive references these Ethereum mainnet contracts:

| Contract | Address | Notes |
| --- | --- | --- |
| VAN token | `0xE46e345092e47b373B6C8AA97e91f237fcE09d8A` | ERC20-style token with owner mint/burn, pause, blacklist, transfer fees, and legacy deprecation hooks. |
| VAN staking | `0x5A8D4DF93eC942BF529AcF3d02775235E06E5235` | Time-lock staking contract for VAN positions and point accrual. |

Live JSON-RPC snapshot taken from Ethereum mainnet at block `25203426`:

| Field | Value |
| --- | --- |
| Token name / symbol | `VANcoin` / `VAN` |
| Token decimals | `18` |
| Total supply | `910,000 VAN` |
| Staking contract balance | `47 VAN` |
| Staking `totalStaked` | `47 VAN` |
| Staking positions | `8` |
| Token owner | `0x7a64988D8914c8FE5509b3d0Fa0A286467E66515` |
| Staking owner | `0x7a64988D8914c8FE5509b3d0Fa0A286467E66515` |
| Token paused | `false` |
| Staking paused | `false` |

The token deployed bytecode exactly matched the archived `nav-coin/contracts/eth/token-contract` artifact. The staking deployed bytecode did not exactly match the checked-in `VANcoinStaking.sol` artifact, even after stripping Solidity metadata. Treat the archived staking source as close operational context, but verify the deployed staking source directly before relying on line-level behavior.

## Token Functionality

The main archived token implementation is a Tether-style ERC20 variant:

- Owner can issue tokens to the owner address.
- Owner can redeem tokens from the owner balance.
- Owner can pause and unpause transfers.
- Owner can blacklist addresses.
- Owner can burn the full balance of a blacklisted address.
- Owner can set a transfer fee, capped in source at less than 20 basis points and less than 50 tokens maximum fee.
- Owner can deprecate the token to an upgraded contract address, causing `transfer`, `transferFrom`, `approve`, `balanceOf`, `allowance`, and `totalSupply` to route through the upgraded contract interface.
- Owner can recover accidentally sent ERC20 tokens and ETH.

The practical consequence is simple: this is not a trust-minimized token. It is an admin-controlled token with mint, pause, blacklist, burn, fee, and migration powers.

## Staking Functionality

The archived mainnet staking implementation supports:

- Staking VAN for a fixed period.
- Lock periods from 1 to 36 months in the older mainnet package.
- Multiple independent positions per user.
- Daily point accrual using a duration-based multiplier.
- No early withdrawal.
- Withdrawal after maturity.
- Owner pause and unpause.
- Owner recovery of non-staking tokens accidentally sent to the staking contract.

The staking contract appears to be a points and leaderboard mechanism, not an on-chain yield distributor. The archived code tracks points and positions; it does not pay a separate reward token during withdrawal.

## Newer `nav-token` Draft

The `nav-token` subproject is a newer Hardhat 3 implementation:

- `NavToken.sol` is UUPS upgradeable.
- It adds ERC20Permit support.
- It keeps the same broad admin model: pause, blacklist, burn blacklisted funds, owner issuance, owner redemption, fee setting, token recovery, and UUPS upgrades.
- `NavStaking.sol` is also UUPS upgradeable.
- Staking requires a PFTL address to be registered before withdrawal.
- Owner can change duration and point multiplier parameters for future stakes.
- The staking code explicitly assumes the staking token has no transfer fee; if fees are enabled on `NavToken`, the staking accounting can become insolvent unless upgraded to use balance-delta accounting.

This newer package did not compile under the local Node 18 runtime because Hardhat 3 requires newer Node APIs. It should be tested under Node 20 or later before reuse.

## Site And Services

The `nav-coin` application includes:

- A React frontend with Ethereum mainnet and Sepolia contract addresses.
- Wallet/network switching helpers for Ethereum mainnet and Sepolia.
- A staking service wrapper that reads balances, allowances, positions, points, and total staked values.
- A history service that monitors token transactions through Alchemy WebSocket/HTTP endpoints and stores derived records in Supabase.
- A leaderboard service that parses staking activity and writes position/ranking tables.

The archive also includes Docker deployment notes that set:

```text
TARGET_CONTRACT_ADDRESS=0xE46e345092e47b373B6C8AA97e91f237fcE09d8A
STAKING_CONTRACT_ADDRESS=0x5A8D4DF93eC942BF529AcF3d02775235E06E5235
```

## Local Test Results

Checked locally after cloning `postfiat-archive`:

| Package | Command | Result |
| --- | --- | --- |
| `nav-coin/contracts/eth/staking-contract` | `npm ci && npm test` | Passed: 36 tests. |
| `nav-coin/contracts/eth/token-contract` | `npm ci && npm test` | Compiled, but tests failed because the checked-in tests call stale `ethers.utils.parseEther` style helpers under the installed toolchain. |
| `nav-token` | `npm ci && npx hardhat compile` | Blocked by Node 18; Hardhat 3 expects Node 20+ APIs. |

## Revival Checklist

Before reusing any NAVcoin component with meaningful TVL:

1. Verify the deployed staking contract source and ABI directly; the checked-in staking artifact does not byte-match mainnet.
2. Move owner powers to a multisig with a timelock, especially mint, pause, blacklist burn, deprecation, and UUPS upgrade authority.
3. Decide whether blacklist, confiscation, pause, and deprecation powers are product requirements or unacceptable custody risk.
4. Add invariant tests for `totalStaked <= token.balanceOf(stakingContract)` and for exact withdrawability after maturity.
5. Use balance-delta accounting in staking if the token can ever charge transfer fees.
6. Add tests for owner-key compromise scenarios and migration/deprecation behavior.
7. Re-run the newer `nav-token` package under Node 20+ and add deployment/proxy upgrade tests before considering it production-ready.
