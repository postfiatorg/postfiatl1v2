# NAVCoin Assets And Venues

> **Current-state amendment (2026-07-30):** The earlier status descriptions on
> this page were overtaken by the A666 v2 mainnet deployment. Use
> [A666 Current State](../status/A666-PFUSDC-PRIVATE-SWAP-CURRENT-STATE-20260730.md)
> for live identifiers, economics, proof boundaries, and release status.

NAVCoins separate the asset's backing from the places where users access or
trade the asset. A venue can host liquidity without becoming a separate backing
pool.

## a651

`a651` is the first named NAVCoin instance. It represents the six-leg portfolio
used throughout the current proof-of-reserve, SP1, bridge, and shielded-swap
work.

| Context | Status | Notes |
|---|---|---|
| PFTL WAN devnet | Historical NAV/supply and private-swap evidence. | Used for primary mint/redeem, pfUSDC settlement, a651/a652 swap evidence, and early Asset-Orchard work. |
| Ethereum mainnet | Legacy a651 token and historical a651/USDC Uniswap v4 venue. | Pool-specific liquidity was zero at the last a651 inspection. There is no live cross-chain a651 bridge. |
| Shielded Asset-Orchard | Historical private a651/pfUSDC swap lineage. | The private infrastructure progressed to A666 private-primary issue/redeem; boundary disclosure caveats still apply. |

New production acceptance must use A666 v2. a651 may be used only for explicit
legacy regression, migration, or historical-reproduction work.

## a666 and wA666

`a666` is the replacement PFTL-native NAVCoin lineage for the trustless
large-capacity primary subscription product. `wA666` is the required
bridge-aware Ethereum representation. It must be minted only after the
corresponding native PFTL a666 has been debited or made unspendable and the
PFTL export has been accepted through the route's disclosed finality verifier.

| Context | Status | Notes |
|---|---|---|
| PFTL WAN fleet | **Production A666 v2 is finalized and its issue/redeem/export route is active.** | Asset `521c…74b6` has no permanent maximum supply. Transparent and private primary issue/redeem transitions have committed. |
| Ethereum Sepolia | Controlled wA666 stack and route evidence exist. | Controlled testing only; the route is not a trustless mainnet product. |
| Ethereum mainnet | **wA666 contracts and the hookless v4 pool are deployed and functionally exercised.** | wA666 is `0xeE4C…9bE5`; pool `0xc5f1…6e98` was seeded with `3,000 wA666 + 3,000 USDC` and third-party swaps finalized. Re-read chain state before making a current liquidity or price claim. |

The required primary acquisition flow is:

```text
verified pfUSDC
  -> user-signed primary subscription
  -> new native a666 issued at finalized pre-inflow NAV plus posted spread
  -> supply-conserving PFTL export
  -> wA666 delivered on Ethereum mainnet
```

This flow must support large subscriptions independently of pool depth. For
example, at a $1.00 NAV and a `1.005` mint multiplier, 100,500 USDC creates
100,000 new a666 before export. The 100,000-USDC base value enters counted NAV
reserves and the 500-USDC spread is separately accounted outside NAV assets,
preserving the stated $1.00 NAV. A small Uniswap pool is a price anchor and
secondary venue, not the inventory source for that acquisition.

The deployed policy exposes `2,000,000 A666` issue and redemption capacity per
policy epoch, a `1,000,000 A666` maximum primary order, a `250,000 A666`
per-export-packet cap, and a `2,000,000 A666` net wrapped cap.
Available capacity must be bounded by proven backing and policy caps, not
issuer inventory or AMM liquidity. Redemption performs the inverse economic
transition: retire NAVCoin supply and release settlement value under the
posted redemption policy. This posted capacity is not a permanent supply
ceiling; production a666 must be created with `max_supply` absent (`None`) so
verified subscriptions and redemptions can expand and contract supply.

The former six-decimal controlled configuration is not production capacity:
its route cap is 10 a666, its packet cap is 1 a666, and its native asset
maximum is 1,000,000 a666. It is superseded by the fresh production v2 asset,
which has no static maximum. The deployed Ethereum controller encodes a
2,000,000-a666 route cap and 250,000-a666 packet cap. These are risk bounds,
not evidence that million-A666 live orders have been qualified. See
`../plans/A666-END-TO-END-MAINNET-PRIMARY-ISSUANCE-SPEC-20260727.md` and
`../status/A666-MAINNET-DEPLOYMENT-20260727.md`.

## Legacy Ethereum a651 and Uniswap

The Ethereum mainnet a651 launch produced an ERC-20 token and a Uniswap v4
a651/USDC pool. The pool is now legacy and had zero pool-specific liquidity at
the last a651 inspection. The dedicated historical page is
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
| Launch-time pool liquidity | 676.57 a651 + $4,000.00 USDC |
| Pool fee | 100 bps |
| Legacy a651 controller redemption allocation | $990 USDC |
| Smoke swap | $1 USDC |

The `$990` allocation describes the historical, Ethereum-native a651 launch
controller. It is not the canonical NAVCoin primary-redemption model and must
not be carried into a666. a666 redemption releases subscription-funded NAV
reserve principal as it retires supply.

The Arbitrum and Base venue token reps above were deployed on Ethereum as
registry representatives. They are not live Arbitrum/Base a651 bridge
deployments.

## pfUSDC

`pfUSDC` is the product name for a PFTL-side cash receipt backed by source-chain
USDC evidence. The current production source is canonical Ethereum-mainnet
USDC; Arbitrum is deprecated for new ingress because its trustless
confirmation path took approximately `6.4 days`. The implementation primitive
remains intentionally generic:

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
