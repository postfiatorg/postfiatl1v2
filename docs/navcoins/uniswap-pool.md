# a651 Uniswap Pool

This page documents the live Ethereum a651/USDC Uniswap v4 venue. It is a venue
page, not the canonical NAV accounting page.

The most important boundary:

```text
pool liquidity != portfolio backing
```

The pool is secondary-market access. The NAVCoin backing model is still the
verified reserve portfolio, proof profile, reserve packet, valid supply, and
NAV policy described in [Proof-of-Reserve Primitives](reserve-primitives.md).

## Quick facts

| Field | Value |
|---|---|
| Chain | Ethereum mainnet (`chain_id 1`) |
| Pair | a651 / USDC |
| Venue | Uniswap v4 |
| Pool fee | 100 bps |
| Pool id | `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84` |
| a651 token | `0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e` |
| USDC token | `0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48` |
| Uniswap v4 PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| Uniswap v4 PositionManager | `0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e` |
| Universal Router | `0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af` |
| Launch date | 2026-06-15 UTC |

## Launch configuration

| Parameter | Value |
|---|---:|
| Genesis supply | 4,000.000 a651 |
| Proven portfolio NAV | $23,648.69 |
| NAV per unit | $5.912 |
| Initial pool liquidity | 676.57 a651 + $4,000.00 USDC |
| Redemption buffer | $990 USDC |
| Smoke swap | $1 USDC |
| Total USDC drawn for launch | $4,991 of $5,000 |

The launch used a 4,000 unit genesis supply so the per-unit NAV stayed near
$5.912. The pool USDC and redemption buffer are separate allocations. Pool depth
is not counted as the backing of only the pool tokens; every valid a651 unit
references the same verified reserve portfolio.

## Read-only inspection - 2026-06-29

Source: Ethereum mainnet read-only `cast call` via
`https://ethereum.publicnode.com`. The Uniswap v4 StateView address is the
Ethereum mainnet deployment listed in the official Uniswap v4 deployment docs.

Current token metadata:

| Field | Value |
|---|---:|
| a651 name | `a651` |
| a651 symbol | `a651` |
| a651 decimals | `18` |
| a651 total supply | `4000000000000000000000` raw (`4,000.000000000000000000 a651`) |

Current pool-specific StateView read:

| Field | Value |
|---|---:|
| StateView | `0x7ffe42c4a5deea5b0fec41c94c136cf115597227` |
| Pool id | `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84` |
| `getSlot0(...).sqrtPriceX96` | `264840598407943562605399` |
| `getSlot0(...).tick` | `-252187` |
| `getSlot0(...).protocolFee` | `0` |
| `getSlot0(...).lpFee` | `500` |
| `getLiquidity(...)` | `0` |

Current custody / contract balances:

| Holder | Asset | Raw balance | Human balance |
|---|---|---:|---:|
| StakeHub EOA `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | a651 | `3815876054831038397894` | `3815.876054831038397894 a651` |
| StakeHub EOA `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | USDC | `5805752289` | `5805.752289 USDC` |
| StakeHub EOA `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` | PositionManager NFT | `1` | `1 position` |
| PoolManager `0x000000000004444c5dc75cB358380D2e3dE08A90` | a651 | `93936271343217656` | `0.093936271343217656 a651` |

The PoolManager USDC ERC-20 balance is aggregate contract inventory across all
v4 pools, not a pool-specific reserve read, so it is not used as a651/USDC pool
depth evidence. The pool-specific StateView liquidity is `0`, which means this
legacy pool should currently be treated as inactive secondary liquidity. It is
still a historical Ethereum a651 venue, but it is not a usable trustless
PFTL-to-Uniswap handoff route and should not be targeted by wallet
`uniswap_atomic_handoff`.

## Related contracts

| Contract | Address | Role |
|---|---|---|
| Leverage verifier | `0xD665fAFe48137268D6Bb17E9609E9CBF21048068` | Verifies the disclosed leverage proof used by the launch. |
| NAV proof adapter | `0xD351fF828B83F9426D69E780b6B64E1550F137cD` | Exposes the latest NAV snapshot used by the Ethereum launch stack. |
| Mint/redeem controller | `0xd5c4200b74929952dCa4DB70FDc65317c2705207` | Controls Ethereum-side mint/redeem behavior for the launch stack. |
| Bridge controller | `0x74f4A27Acd503B3aABE955659BFEda33082e3340` | Bridge-controller contract in the launch stack. |
| Arbitrum venue token rep registry entry | `0xe66dA7fef6925d3FC8D66586A3a61fCD8e7d00A8` | Ethereum registry representative for a future Arbitrum venue token. |
| Base venue token rep registry entry | `0xF226c4f3d01ba63900032aD72203e09Ca6A62Cb3` | Ethereum registry representative for a future Base venue token. |

The Arbitrum and Base venue token representatives are deployed on Ethereum as
registry records. They are not live Arbitrum/Base a651 deployments.

## Where the code lives

The live Ethereum pool is a canonical Uniswap v4 pool. The pool itself is not
our contract. Our code lives in the a651 contracts, launch helper, proof adapter,
and launch orchestration around the pool.

| Code path | What it owns |
|---|---|
| `StakeHub/zk/contracts/src/navcoin/NavCoin.sol` | ERC-20-compatible a651 venue token with controller-gated mint/burn, pause, quarantine, venue id, and allocation-registry binding. |
| `StakeHub/zk/contracts/src/navcoin/NavProofAdapter.sol` | Reads `StakeHubLeverageVerifier.latest()`, checks schema/mode/program vkey/policy hash/freshness, computes `verifiedNetAssetsUsdE8`, global supply, and NAV per unit. |
| `StakeHub/zk/contracts/src/navcoin/NavAllocationRegistry.sol` | Defines venue ids, 80/10/10 Ethereum/Arbitrum/Base allocation caps, active venue tokens, current global supply, and allocation policy hash. |
| `StakeHub/zk/contracts/src/navcoin/NavMintRedeemController.sol` | Operator-gated primary mint/redeem against fresh NAV snapshots, fees, epoch limits, redemption buffer, and USDC settlement. |
| `StakeHub/zk/contracts/src/navcoin/NavBridgeController.sol` | Supply controller for primary mint/burn and specified owner-only burn-here/mint-there flows. It is not a live cross-chain a651 bridge. |
| `StakeHub/zk/contracts/src/navcoin/NavCoinV4LaunchHelper.sol` | Real Uniswap v4 launch helper: computes the `PoolKey`, initializes the pool at NAV, seeds liquidity through PositionManager, and runs smoke swaps through PoolManager unlock. |
| `StakeHub/zk/contracts/script/NavCoinUniswapV4DryRun.s.sol` | Fork/dry-run math, mock v4 pool manager, mock position manager, and helper functions for pool id, sqrt price, and implied NAV checks. |
| `StakeHub/stakehub/navcoin_launch.py` | Launch orchestration: proof submission, contract deploys, rehearsal gate, Uniswap v4 address table, helper deploy, pool initialization, liquidity seed, smoke swap, and publish list. |
| `StakeHub/zk/contracts/test/NavCoinPhase1.t.sol` | Tests proof adapter, allocation caps, stale-proof fail-closed behavior, NAV formula, mint/redeem, and unified supply invariants. |
| `StakeHub/zk/contracts/test/NavCoinUniswapV4Mock.t.sol` | Tests Uniswap v4 pool math and dry-run behavior against mocks/forks. |

The L1 repo also has a separate, more generic EVM contract suite for PFTL
bridge and market-operation work:

| Code path | What it owns |
|---|---|
| `postfiatl1v2/crates/ethereum-contracts/src/NAVGuardHook.sol` | Controlled-launch, Uniswap-v4-shaped venue-evidence adapter. It records pool observations against PFTL state hashes, but is not the live a651 pool hook. |
| `postfiatl1v2/crates/ethereum-contracts/src/MarketOpsEnvelope.sol` | Solidity representation of a PFTL-finalized market-operation envelope. |
| `postfiatl1v2/crates/ethereum-contracts/src/PFTLBridgeAdapter.sol` | Optimistic adapter for admitting PFTL-finalized market-operation envelopes. |
| `postfiatl1v2/crates/ethereum-contracts/src/MarketOpsVault.sol` | Alignment reserve custody and bounded below-NAV buy execution. |
| `postfiatl1v2/crates/ethereum-contracts/src/MintController.sol` | Escrowed above-NAV mint release against accepted envelope/settlement proof. |

These L1 market-operation contracts document the intended PFTL-controlled venue
architecture. They should not be confused with the already-launched Ethereum
a651/USDC pool, which was launched hookless with `hooks = address(0)`.

## How the pool is linked to NAV

There are three different links, and only one is the live pool itself:

| Link | Mechanism | Live a651 status |
|---|---|---|
| Pool initialization at NAV | `NavCoinV4LaunchHelper.initialSqrtPriceX96()` computes the Uniswap v4 initial price from the fresh NAV snapshot, then `initializeAndSeed()` initializes/seeds the pool. | Live launch path. |
| Mint/redeem discipline around NAV | `NavProofAdapter.requireFreshSnapshot()` gates `NavMintRedeemController.mint()` and `redeem()`, while `NavAllocationRegistry` and `NavBridgeController` enforce venue/global supply caps. | Live launch stack. |
| Ongoing PFTL venue evidence/market ops | `NAVGuardHook`, `MarketOpsEnvelope`, `PFTLBridgeAdapter`, `MarketOpsVault`, and `MintController` provide the future PFTL-finalized venue-control path. | Not the live a651 pool hook. Controlled-launch/follow-up work. |

So the answer is: yes, there are smart contracts linking a651 to NAV, but the
live Uniswap pool is not itself a custom NAV hook. It is a hookless Uniswap v4
pool seeded at NAV, with proof/NAV/mint/redeem/supply logic enforced by the
surrounding a651 contracts.

## What this pool is

- a live Ethereum mainnet a651/USDC Uniswap v4 venue;
- a secondary-market access point for a651;
- a real pool seeded around the proven launch NAV;
- a venue that can be referenced by future market-operation and venue-evidence
  work.

## What this pool is not

- It is not the canonical PFTL NAV/supply ledger.
- It is not a live cross-chain bridge for a651.
- It is not a separate reserve portfolio.
- It is not proof that every a651 representation on every future chain is live.
- It is not a production `NAVGuardHook` deployment. The current
  `NAVGuardHook.sol` is a controlled-launch, Uniswap-v4-shaped venue-evidence
  adapter, not a completed native Uniswap v4 hook.

## Inspection commands

Use an Ethereum mainnet RPC endpoint:

```bash
export ETH_RPC_URL=<ethereum-mainnet-rpc>

cast call 0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e \
  "name()(string)" \
  --rpc-url "$ETH_RPC_URL"

cast call 0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e \
  "symbol()(string)" \
  --rpc-url "$ETH_RPC_URL"

cast call 0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e \
  "totalSupply()(uint256)" \
  --rpc-url "$ETH_RPC_URL"
```

The pool id to use with Uniswap v4 tooling is:

```text
0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84
```

## Relationship to PFTL

The target architecture is one verified reserve portfolio, one canonical
NAV/supply ledger, and many access venues. PFTL is the intended canonical
NAV/supply ledger. Ethereum is an access and execution venue.

In the full design, Ethereum contracts should enforce compact PFTL-finalized
outputs, such as market-operation envelopes or bridge withdrawal packets. They
should not independently reinterpret the full NAV calculation. The PFTL side
keeps the replayable reserve packet, valid supply accounting, freshness policy,
and proof-profile semantics.
