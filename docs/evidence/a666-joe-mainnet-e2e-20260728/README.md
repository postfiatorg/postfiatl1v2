# Joe USDC-to-A666 Mainnet Acceptance

**Date:** 2026-07-28 UTC

**Functional verdict:** PASS

**25-minute SLO:** FAIL (`42.2 minutes`)

This campaign executed the live buyer-funded path on Ethereum mainnet and
PFTL:

1. Joe deposited exactly `100.500000 USDC` into the proof-gated pfUSDC vault.
2. Ethereum finality and the SP1 ingress proof authorized exactly
   `100.500000 pfUSDC` on PFTL.
3. Joe subscribed at `1.005 × NAV`; `100.000000` new A666 was issued, increasing
   production A666 supply rather than transferring operator inventory.
4. Joe exported all `100.000000 A666` from PFTL.
5. A finalized PFTL receipt proof minted exactly `100.000000 wA666` to Joe on
   Ethereum.
6. The live hookless USDC/wA666 Uniswap v4 pool remained available with
   `3,000,000,000` liquidity units.

## Buyer and assets

| Item | Value |
|---|---|
| Joe Ethereum address | `0x1455Bd7FBfBF92a171eF36025E13959E3b0ad8c0` |
| Joe PFTL address | `pfab9b9228942e5c529633a13aa271d5297bec6353` |
| Production A666 asset | `521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c` |
| Mainnet wA666 | `0xeE4C92eDB03efdD9B519339edc19ad70C69A9bE5` |
| Uniswap v4 pool | `0xc5f1e4b5bb07c0718eddcc3d102dc751b8953ec25bb05cdc14d95419d4d16e98` |

## Finalized transitions

| Transition | Result |
|---|---|
| Ethereum USDC deposit | block `25,627,947`, tx `0x136512df7ea2764e1878c70d51ba85939596c1d18d159c4e21e162d0b83f7155` |
| PFTL ingress propose/finalize/claim | heights `352` / `353` / `355` |
| A666 reserve/subscription/export | heights `356` / `357` / `358` |
| Ethereum proof acceptance | block `25,628,155`, tx `0x1865f032c0714b1a415931479a555096d9540df346bb3537428af86deff42112` |
| Ethereum wA666 mint | block `25,628,156`, tx `0xb4e91d0799ac566c50031329df06e9a51e638ceda20ef8ebcb6c6178429ca5a8` |
| Final Joe wA666 balance | `100,000,000` atoms (`100.000000 wA666`) |
| Final wA666 total supply | `31,486,197,455` atoms |

The PFTL export packet was
`5564c9dde876bfefdf14469495205a04b4b02cd04c39ba2d135cc1499231223609703206a68808f5f95a729927d98272`.
Its Ethereum packet digest was
`0x6285152bac1f632d597a04250d676d680a5bd9460a440a06ffa023e3bb952600`
and is consumed exactly once.

## Timing and gas

Deposit confirmation to wA666 mint took `2,532 seconds` (`42.2 minutes`).
Ethereum finality consumed about `15.3 minutes`; the remaining excess came
from first-run operator defects: an initially missing CUDA prover setting,
relay key staging, a legacy private-pfUSDC custody reconciliation, and a
historical governance mismatch in the generic snapshot exporter.

The Ethereum gas cost from deposit through proof acceptance and mint was
`0.000053731236825777 ETH`; including the USDC approval it was
`0.000059522310558017 ETH`.

The legacy private-pfUSDC reconciliation at height `354` moved an already
issued `100,000`-atom note from AssetOrchard back to Joe's transparent balance.
It did not mint pfUSDC or change global supply. The claim then finalized under
the global conservation checks at height `355`.

## Evidence index

- Exact final balances, Uniswap readback, timing, gas, and transaction receipts:
  [`ethereum/final-readback.json`](ethereum/final-readback.json)
- Proof acceptance and exact recipient supply deltas:
  [`ethereum/mint-state.json`](ethereum/mint-state.json)
- Ethereum ingress proof:
  [`ingress/proof/`](ingress/proof/)
- PFTL ingress and reconciliation reports:
  [`pftl/`](pftl/)
- A666 reservation, primary subscription, and export rounds:
  [`a666/`](a666/)
- Finalized export witness and Groth16 proof:
  [`export-proof/`](export-proof/)

The functional path is proven, but this result is not a claim that the
25-minute product SLO is met. A clean automated rerun and the inverse
burn/import/redemption campaign remain release work.
