# A666 Mainnet Deployment Status

**As of:** 2026-07-28 UTC
**Status:** opening inventory exported and proof-minted; ownerless a651
migration funded; wA666/USDC Uniswap v4 pool seeded and trading. A fresh
buyer-funded primary subscription and export passed on mainnet. Full
redemption acceptance and the 25-minute performance SLO remain release gates.

## Live identifiers

| Component | Identifier |
|---|---|
| PFTL a666 v2 asset | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` |
| PFTL production route | `pftl-a666-ethereum-wA666-usdc-v1` |
| Mainnet wA666 | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5` |
| SP1 receipt verifier | `0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A` |
| Proof-gated controller | `0x9A0262C0572fb4DB08765408eB225E207F40c3d9` |
| Ownerless a651 migration | `0xFfBBae0eb8450D10F8C0D26C2952b089D56e517c` |
| Uniswap v4 pool | `0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98` |
| PoolManager | `0x000000000004444c5dc75cB358380D2e3dE08A90` |

The token controller is irreversibly locked. The receipt verifier pins SP1
program vkey
`0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9`.
The controller has a `2,000,000 a666` net wrapped cap and a `250,000 a666`
per-packet cap. These are route risk limits, not a permanent a666 maximum
supply.

## Finalized PFTL state

Production a666 v2 has six decimals and no permanent maximum supply. Its
opening supply is `31,386.197455 a666` against `$31,386.19745591` of
StakeHub-verified net assets at epoch-one NAV of `$1.00000000`.

The production route was initialized disabled at height `345`, activated at
height `346`, and its NAV valuation unit corrected from the ambiguous legacy
`USDC` label to canonical `USD_1E8` at height `347`. The full untouched opening
inventory was exported at height `348`:

| Item | Value |
|---|---|
| Exported amount | `31,386.197455 a666` |
| Export packet | `2385ff333b16f4dac45b2845313d0b34ff6ca28a052dba727ebbe5fae4707c23722c192d8e5258c2b262b6f78f1d97a3` |
| Export receipt | `b7bfa196b87c79dde837e8de6026b5150059fa7ace8f11fc44bfd5f01f6e77f6896ac5716d7801d9804bf3a7246265c6` |
| Receipt root | `4ebd8753bfd6363e4584b10305970fa3ba810e073bc2a7dd5cb76c1280ef3e3454a6a53f67925e36ab1206d3cd367fdc` |
| Height-348 block | `c95e3c33fafa204e66dbd2d8a23c8efabc177e27545cc7b3ef1b9ae1525f433fa0c4acde9e29a274df4139e4d68498f1` |
| Export transaction | `35e7f6a88346f90280c28d6e0769c6b9a333a9036d96518b1af83b64fe500dff1437f8df8fe7e3f8418c2d826736f3bd` |

After export, native route spendability is zero and the route records
`31,386.197455` outstanding bridge-claim units. This prevents the opening
inventory from being spendable on PFTL while its Ethereum representation
exists.

## Buyer-funded primary subscription acceptance

On 2026-07-28, Joe executed the complete live primary path:

`100.500000 USDC` on Ethereum → `100.500000 pfUSDC` on PFTL →
`100.000000` newly issued A666 on PFTL → `100.000000 wA666` on Ethereum.

This was primary issuance at `1.005 × NAV`, not an OTC transfer from existing
A666 inventory. The production A666 supply and proof-gated wrapped supply each
increased by exactly `100,000,000` atoms.

| Transition | Finalized result |
|---|---|
| Joe USDC deposit | Ethereum block `25,627,947`, tx `0x136512df7ea2764e1878c70d51ba85939596c1d18d159c4e21e162d0b83f7155` |
| pfUSDC ingress propose/finalize/claim | PFTL heights `352` / `353` / `355` |
| A666 reserve/subscription/export | PFTL heights `356` / `357` / `358` |
| Accept A666 export proof | Ethereum block `25,628,155`, tx `0x1865f032c0714b1a415931479a555096d9540df346bb3537428af86deff42112` |
| Mint 100 wA666 to Joe | Ethereum block `25,628,156`, tx `0xb4e91d0799ac566c50031329df06e9a51e638ceda20ef8ebcb6c6178429ca5a8` |

Joe's native A666 balance went from zero to `100.000000` at subscription and
back to zero at export. His wA666 balance went from zero to `100.000000`;
total wA666 supply rose from `31,386.197455` to `31,486.197455`. The export
packet is consumed exactly once and the migration reserve was unchanged.

The live USDC/wA666 pool remained available with `3,000,000,000` liquidity
units, so Joe's received wA666 is directly tradeable. No secondary-market swap
was included because the acceptance objective ended with delivery of the
Uniswap asset.

Functional acceptance passed. Deposit confirmation to wA666 mint took `42.2`
minutes, missing the `25-minute` SLO. Ethereum finality consumed about `15.3`
minutes; first-run operator defects accounted for the avoidable excess. The
deposit-through-mint gas cost was `0.000053731236825777 ETH`, or
`0.000059522310558017 ETH` including approval.

## Genuine proof and Ethereum mint

The height-348 witness executed inside the deployed SP1 guest in `89,972,346`
RISC-V instructions. The locally generated and verified Groth16 proof is 356
bytes and commits 1,120 bytes of canonical public values. Proof generation
took `1,087,965 ms`.

| Ethereum action | Block | Transaction |
|---|---:|---|
| Accept finalized receipt proof | 25,627,666 | `0xdeb26364d653642f9619439eb1abad1c91e3b94f0f7ec8ed811b0d7b5bbde460` |
| Unpause proof-gated controller | 25,627,667 | `0x80b72d5a1128bd805d071cf30012cc86dd08d0aa183a8b8bc2f891b4ea9f7e17` |
| Consume exact mint packet | 25,627,668 | `0x6ec3cf06df852076f86f6344053872d8cd2f86ef670a992656310b7d839a47d4` |

Readback after mint:

- verifier checkpoint advanced from PFTL height `344` to `348`;
- packet digest
  `0x3f4a57859cd56bd2978d709aa5671f0651cff3ad72fd1272c6abee6f9bc48798`
  is consumed exactly once;
- total wA666 supply is exactly `31,386.197455`;
- all `31,386.197455 wA666` initially landed in the ownerless migration
  contract; and
- controller outstanding equals the proof-minted supply.

## Migration and live Uniswap venue

The operator burned exactly `382.333668078301459218 a651` through the
authorized ownerless migration. The fixed ratio released exactly
`3,000.000000 wA666`; no owner mint was used.

The hookless USDC/wA666 pool was initialized at Q96 (`$1.00`), fee `500`, tick
spacing `10`, then seeded through the official PositionManager:

| Action | Block | Transaction |
|---|---:|---|
| Burn a651 / release 3,000 wA666 | 25,627,674 | `0xfaabbcc9b91beec0440c21935a0905ce855cdd14b62307a0ef9172aaecdc79ff` |
| Add up to 3,000 USDC + 3,000 wA666 | 25,627,679 | `0xc9569c05f3582efc9266ae63b33fdd68145dcdaef732bc865e264a93fd7425e8` |

The position uses full-range ticks `-887270` / `887270`, is owned by the
operator address, and has liquidity `3,000,000,000`. All temporary
PositionManager and Universal Router Permit2 allowances and both ERC-20
allowances were revoked to zero after use.

The pool began receiving third-party swaps before the operator smoke
transaction could pass its `$0.99` minimum-output bound. Ten external swaps
finalized from blocks `25,627,682` through `25,627,694`. Net event deltas were
`-1,324.254197 USDC` and `+917.918330 wA666` under PoolManager event sign
convention. Actual price discovery moved tick from `0` to `-7308`, or about
`$2.0767` per wA666. The operator did not chase that price: both attempted
smoke transactions stopped during gas simulation and were never broadcast.
External finalized swaps are the live-trading evidence; total wA666 supply
remained unchanged.

After seeding, the operator retained `334.712789 USDC`. No more USDC was
required for this launch seed. The buyer, not the operator, funds future
primary subscriptions.

## What remains

The opening migration, secondary venue, fresh primary issuance, and export to
the buyer are live. The complete product is not yet declared generally
available. The binding campaign order is documented in
`../plans/A666-TRANSPARENT-PRIVATE-ISSUE-REDEEM-ACCEPTANCE-SPEC-20260728.md`;
these acceptance items remain:

1. automate the proven primary path and pass a clean deposit-to-wA666 rerun
   within the `25-minute` SLO;
2. execute the inverse burn/import/primary-redemption path at `0.9995 × NAV`;
3. pass replay, invalid-proof, refund/cancellation, packet-splitting, and
   capacity tests; and
4. expose a wallet/product workflow so a user does not assemble proof packets
   manually.

Uniswap liquidity is a secondary venue only. Its live price must never be used
as the NAV oracle or as the primary issue/redemption price.

## Evidence

- PFTL route configuration and finality:
  `../../deployments/a666-mainnet-20260727/09-production-route-config.json`
  through `11b-opening-inventory-export/`
- Finalized height-348 proof snapshot:
  `../../deployments/a666-mainnet-20260727/12-opening-export-proof-snapshot/`
- Witness, deployed-key ELF, execution, and Groth16 proof:
  `../../deployments/a666-mainnet-20260727/13-opening-export-proof/`
- Ethereum proof/mint readback:
  `../../deployments/a666-mainnet-20260727/ethereum/opening-mint-state.json`
- Pool seed, revocations, external swaps, and final readback:
  `../../deployments/a666-mainnet-20260727/ethereum/pool-seed-state.json`
- Fresh Joe primary subscription, PFTL export, Ethereum mint, timing, and pool
  readback:
  `../evidence/a666-joe-mainnet-e2e-20260728/README.md`
- Ordered transparent/private issue and redemption acceptance program:
  `../plans/A666-TRANSPARENT-PRIVATE-ISSUE-REDEEM-ACCEPTANCE-SPEC-20260728.md`
