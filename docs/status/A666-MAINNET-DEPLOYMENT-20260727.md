# A666 Mainnet Deployment Status

**As of:** 2026-07-27
**Status:** native A666 and StakeHub NAV finalized; Ethereum contracts and
empty Uniswap pool deployed; public issue/export and seeded venue not live

## What is deployed

The six-validator PFTL WAN fleet runs release `a666-mainnet-0f0a621` from
commit `0f0a621`. All validators converged at height `344`, state root
`651677815a2d7cb196706bda619a0fb9a147607973560a835b95a792d6f926ffd1b553c873dc786105e06391c758428a`,
with empty mempools.

| PFTL item | Final value |
|---|---|
| Asset | a666 v2 |
| Asset ID | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` |
| Precision | 6 |
| Permanent maximum supply | none |
| Finalized NAV epoch | 1 |
| StakeHub NAV per unit | `$1.00000000` |
| Verified net assets | `$31,386.19745591` |
| Opening/outstanding supply | `31,386.197455 a666` |
| Rounding overcollateralization | `$0.00000091` |

The StakeHub SP1 Groth16 reserve proof is bound to vkey
`0x00f96064937f05d891b13a80667bdf5ecd62a7d5ed245724ab294bad311a2164`,
profile
`8c0244fe0cfb216fb5ab471d0c9e060a5c8ba052b5a29952d6e7aad76b24523af2b7e0ed82885c11d2c6308ddfcc9118`,
and reserve packet
`c8bbb35b7b0eb4a567f04945eb977b3fe5dc539cd7d845f1266d39959eb301ffbe263ebec9ceb985ef09370f314e5b3e`.

The finalized PFTL operations were:

| Operation | Height | Transaction |
|---|---:|---|
| Submit StakeHub reserve proof | 342 | `fbb5f08e001be39e77d998a5503b05d14403934eccc77f7310f3f6151277767cbf44a564454c3ca994d2fc23a652a935` |
| Finalize NAV epoch one | 343 | `847dbf37c3396d735bdf6ffaa9e1c2bfe77d8e3cd28650316c44587875a05ba0fa7b300e5cd06411aa4826dca7254c67` |
| Mint exact opening supply | 344 | `9891c9f45eafd55aba71cf03a9c71c88c93b0ea4a138e321b148b2d652ee0e786a4302bda41050544fd6864c0735dacc` |

The following contracts are deployed on Ethereum mainnet (`chain_id 1`):

| Component | Address |
|---|---|
| wA666 token | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5` |
| SP1 receipt verifier | `0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A` |
| Proof-gated primary controller | `0x9A0262C0572fb4DB08765408eB225E207F40c3d9` |
| Ownerless a651-to-a666 migration | `0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c` |

Post-deployment readback confirms that wA666 uses six decimals, its controller
is set and irreversibly locked, total wrapped supply is zero, controller
minting is paused, and wrapped outstanding is zero. The controller encodes a
`2,000,000 a666` route cap and `250,000 a666` packet cap. The verifier pins
PFTL height `344`, its checkpoint commitment, and receipt-program vkey
`0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`.
The legacy a651 supply controller authorizes the ownerless migration.

The seven Ethereum transactions are recorded in
`../../deployments/a666-mainnet-20260727/ethereum/deployment-state.json`.
Deployment spent `0.000360365542428692 ETH`.

The hookless wA666/USDC Uniswap v4 pool is also initialized:

| Pool item | Value |
|---|---|
| Pool ID | `0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98` |
| PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |
| Pair ordering | mainnet USDC / wA666 |
| Fee / tick spacing / hooks | `500` / `10` / none |
| Initial price | Q96, `$1.00` per wA666 |
| Initialization transaction | `0xfbd64d619516722b7e3d7dea09fb20b633301d4a49c32f0246aab891d3bb016f` |
| Block | `25,626,970` |
| Current liquidity | `0` |

Initialization did not mint or transfer either token. It spent
`0.000003555755564036 ETH` and left the controller paused, wrapped supply at
zero, and pool liquidity at zero.

## What is not live

This deployment does not yet let Bob turn USDC into spendable wA666:

- no production PFTL primary issue/export route is active;
- the Ethereum controller is paused and has minted no wA666;
- the migration has no exported wA666 inventory;
- the initialized wA666/USDC pool has no liquidity and cannot trade; and
- the route receipt guest has a pinned ELF/vkey but still needs a genuine
  Groth16 proof and acceptance campaign.

Keeping wrapped supply at zero prevents the native opening supply from being
double-counted. The controller must not be unpaused and the pool must not be
presented as live until a finalized PFTL export makes the corresponding native
units unspendable and a genuine receipt proof authorizes the exact Ethereum
mint.

## Remaining release sequence

1. Freeze and initialize the disabled production PFTL route using the deployed
   token/controller bindings and the finalized epoch-one policy.
2. Export opening migration/venue inventory on PFTL so those native units
   become unspendable.
3. Produce and verify the genuine SP1 receipt proof, then mint exactly the
   proven amount to its designated Ethereum recipient.
4. Exercise the ownerless a651 migration and reconcile old burns, new releases,
   rounding dust, and total supply.
5. Seed the initialized wA666/USDC pool only from proved wA666 plus available
   USDC, and record the position NFT, owner, ticks, and exact token amounts.
6. Pass conservation, replay, invalid-proof, capacity, and 25-minute
   end-to-end latency gates before unpausing public issuance.

## Evidence

- Opening proof manifest:
  `../../deployments/a666-mainnet-20260727/opening-nav-proof-manifest.json`
- Reserve proof finality:
  `../../deployments/a666-mainnet-20260727/06b-opening-reserve-submit/`
- Epoch finality:
  `../../deployments/a666-mainnet-20260727/07-opening-epoch-finalize/`
- Opening mint finality:
  `../../deployments/a666-mainnet-20260727/08-opening-nav-mint/`
- Ethereum deployment/readback:
  `../../deployments/a666-mainnet-20260727/ethereum/deployment-state.json`
- Uniswap pool initialization/readback:
  `../../deployments/a666-mainnet-20260727/ethereum/pool-state.json`
