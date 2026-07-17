# NAVCoin Assets And Venues

NAVCoins separate the asset's backing from the places where users access or
trade the asset. A venue can host liquidity without becoming a separate backing
pool.

## a651

`a651` is the first named NAVCoin instance. It represents the six-leg portfolio
used throughout the current proof-of-reserve, SP1, bridge, and shielded-swap
work.

| Context | Status | Notes |
|---|---|---|
| PFTL WAN devnet | Canonical-target NAV/supply path exercised end to end. | Used for primary mint/redeem, pfUSDC settlement, a651/a652 swap evidence, and Asset-Orchard shielded swap work. |
| Ethereum mainnet | Live a651 token and a651/USDC Uniswap v4 venue. | This documents a real venue/representation, but PFTL remains the intended canonical NAV/supply ledger for the architecture. There is no live cross-chain a651 bridge. |
| Shielded Asset-Orchard | Implemented for internal private a651/pfUSDC swaps. | Internal swap actions hide raw asset ids, values, owners, recipients, and price. Boundary ingress/egress still discloses public asset/value. |

The current PFTL position is: do not bootstrap parallel fake a651 assets for
proof runs. Use the real registered WAN devnet a651 when proving the NAVCoin
round trip.

## Ethereum a651 and Uniswap

The Ethereum mainnet a651 launch produced a live ERC-20 token and a Uniswap v4
a651/USDC pool. The dedicated pool page is
[a651 Uniswap Pool](uniswap-pool.md).

| Component | Value |
|---|---|
| Chain | Ethereum mainnet (`chain_id 1`) |
| a651 token | `0x1e55EDa7ce0788E8b624456C4d401A33bD83b62e` |
| Uniswap v4 pool id | `0xabacd0ca774d387525599100a27a3f0e2cfcb5e9694a4d3543c39057447a5a84` |
| Leverage verifier | `0xD665fAFe48137268D6Bb17E9609E9CBF21048068` |
| NAV proof adapter | `0xD351fF828B83F9426D69E780b6B64E1550F137cD` |
| Mint/redeem controller | `0xd5c4200b74929952dCa4DB70FDc65317c2705207` |
| Bridge controller | `0x74f4A27Acd503B3aABE955659BFEda33082e3340` |
| Arbitrum venue token rep registry entry | `0xe66dA7fef6925d3FC8D66586A3a61fCD8e7d00A8` |
| Base venue token rep registry entry | `0xF226c4f3d01ba63900032aD72203e09Ca6A62Cb3` |
| Uniswap v4 PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| Uniswap v4 PositionManager | `0xbD216513d74C8cf14cf4747E6AaA6420FF64ee9e` |
| Universal Router | `0x66a9893cC07D91D95644AEDD05D03f95e1dBA8Af` |
| USDC | `0xA0b86991c6218b36c1d19D4A2e9Eb0cE3606eB48` |

Launch configuration:

| Parameter | Value |
|---|---:|
| Genesis supply | 4,000.000 a651 |
| NAV per unit | $5.912 |
| Proven portfolio NAV | $23,648.69 |
| Pool liquidity | 676.57 a651 + $4,000.00 USDC |
| Pool fee | 100 bps |
| Redemption buffer | $990 USDC |
| Smoke swap | $1 USDC |

The Arbitrum and Base venue token reps above were deployed on Ethereum as
registry representatives. They are not live Arbitrum/Base a651 bridge
deployments.

## pfUSDC

`pfUSDC` is the product name for a PFTL-side cash receipt backed by source-chain
USDC evidence. The implementation path is intentionally generic:

```text
source ERC-20 deposit
  -> ERC20BridgeVault event
  -> PFTL relay/proposal/attestation/finality
  -> vault bridge asset mint/count on PFTL
  -> NAVCoin subscription allocation
  -> NAV reserve packet includes counted cash
```

This is not a hardcoded `pfUSDC` consensus subsystem. The same vault-bridge
primitive can represent other source ERC-20 assets by configuration. For USDC,
the product label `pfUSDC` is useful only if the receipt remains source-labeled,
finality-checked, haircut-aware, and replayable.

## a652

`a652` is a second NAVCoin instance used in WAN devnet evidence to prove
NAVCoin-to-NAVCoin swap mechanics. It has demonstrated an a651/a652 offer-book
crossing, but it is not documented here as a production public asset.

## Contract surfaces

The current EVM contract suite lives under `crates/ethereum-contracts/src/`:

| Contract | Role |
|---|---|
| `ERC20BridgeVault.sol` | Holds source-chain ERC-20 deposits and pays withdrawals accepted through the PFTL withdrawal verifier. |
| `PFTLWithdrawalVerifier.sol` | Controlled-launch verifier for threshold-signed PFTL withdrawal packets and challenge/finality windows. |
| `PFTLBridgeAdapter.sol` | Accepts PFTL-finalized market-operation envelopes through an optimistic/challenge/freeze path. |
| `MarketOpsEnvelope.sol` | Solidity struct mirroring the PFTL market-operation authorization packet. |
| `MarketOpsVault.sol` | Custodies alignment reserves and executes bounded below-NAV buy operations. |
| `PolicyRegistry.sol` | Registers accepted market-policy identities for envelope validation. |
| `MintController.sol` | Escrows above-NAV mints until settlement or locked-liquidity proof satisfies backing rules. |
| `NAVGuardHook.sol` | Controlled-launch, Uniswap-v4-shaped venue-evidence adapter. It is not yet a production native Uniswap v4 hook. |

The relevant design principle is that Ethereum contracts enforce compact PFTL
outputs. They should not reinterpret the full NAV, reserve, supply, or market
policy calculation independently.
