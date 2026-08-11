# A666 NAV arbitrage probe — clip 01

Date: 2026-08-10
Workflow: `a666-arbclip01-20260810`
Probe size: **10.000000 USDC**

## Final result

The bridge accounting backlog was repaired without weakening the return-import
backing rule. The exact **13.571391 A666** burn was then imported on PFTL at
height **872** and redeemed against verified NAV at height **873**.

The completed economic path for this clip is:

1. **10.000000 Ethereum USDC** bought **13.571391 wA666** on Uniswap.
2. **13.571391 wA666** was burned and returned as **13.571391 PFTL A666**.
3. **13.571391 PFTL A666** was redeemed at epoch-6 NAV for
   **12.256096 pfUSDC**.

Joe's PFTL A666 balance returned from **112.571391** to its original
**99.000000 A666**. Joe's pfUSDC balance increased from **1.358493** to
**13.614589 pfUSDC**, an exact increase of **12.256096 pfUSDC**. The gross
token-denominated result is therefore **+2.256096** versus the original
**10.000000 USDC** input, before Ethereum gas and before any pfUSDC egress back
to Ethereum USDC.

## Initial failure and recovery

The Ethereum market buy succeeded, but the PFTL return import was rejected
before consensus because the route's Ethereum-spendable supply bucket was
smaller than the purchased amount.

This is a bounded 10-USDC probe, not a production-sized transaction.

## Exact amounts

1. Wallet USDC before: **573.758626 USDC**.
2. Uniswap input: **10.000000 USDC**.
3. Uniswap output: **13.571391 wA666**.
4. The exact acquired **13.571391 wA666** was burned for PFTL return.
5. Protected wallet inventory remained **103.000000 wA666**.
6. Route Ethereum-spendable supply: **9.096189 A666**.
7. Return-import amount: **13.571391 A666**.
8. Capacity shortfall: **4.475202 A666**.
9. Initial PFTL import attempt: **rejected before consensus**.
10. Recovered PFTL import: **13.571391 A666 accepted at height 872**.
11. Verified-NAV redemption: **12.256096 pfUSDC accepted at height 873**.
12. Wallet USDC after: **563.758626 USDC**.

The governed NAV was **$0.90353505 per A666**. Protocol rounding produced a
base value of **12.262228 pfUSDC**; the governed 9,995-bps redemption multiplier
paid **12.256096 pfUSDC**, with **0.006132 pfUSDC** retained as spread.

## Market impact

- Pool price before: **0.733530486 USDC/wA666**.
- Pool price after: **0.739247774 USDC/wA666**.
- Discount to verified NAV before: **18.8155%**.
- Discount to verified NAV after: **18.1827%**.

## Chain state and safety

- Buy transaction: `0x1ec72e97fa8927aaf86e8799887673f6da0080aecabc747341124f67e5f0382c`.
- Burn transaction: `0x8d03b3c6749cc144bf8bfebb1501f4f6afcb1a25a2a6a220b8f6804dd8191a0c`.
- Burn event hash: `abc191c1f287a251cef21e6b876d4953126a692f58a62189fba399ddedcc1877`.
- All six validators converged at PFTL height **873** and state root
  `b72768b13e5485f12de2577ddff87c46760d1aa0d246383d87c24cfaaaa76855146090cc370acfb3c008a6a255855009`.
- All six validator mempools are empty.
- PFTL A666 remains **99.000000**.
- PFTL pfUSDC is **13.614589**.
- Return-import transaction:
  `9779ca64dc6a093c84a46a5af2012cead78b7aab2095ead2ab48c4d7042cafde8952580fe325dba4af9e97401a6fb11c`.
- NAV-redemption transaction:
  `cefbdbecfb4db832301ca5768baefa6cbc1be06ffb8a8881384716ac3b251ae226208bf990c99b4777a64c451611bfe3`.
- Uniswap ERC-20 and Permit2 allowances were revoked to zero.
- Ten Ethereum transactions used **0.000049062159947084 ETH** total gas.

## What the probe established

Market ownership of wA666 alone does not guarantee immediate return capacity
under the current route accounting. A return import is additionally bounded by
`ethereum_spendable_supply_atoms`. The route had only **9.096189** atoms-worth
of token units available, so the **13.571391** burn could not be imported as a
single indivisible event.

The shortfall was caused by five historical exports whose Ethereum mints had
succeeded but whose proof-backed `PacketConsumed` acknowledgements had never
been applied on PFTL. The five acknowledgements restored **31,489.197455 A666**
of Ethereum-spendable accounting and reduced outstanding claims to zero. No
ledger state was edited and no return claim was allowed to consume an unrelated
outstanding export.

After the recovered import and redemption, route accounting is:

- PFTL spendable A666: **99.000000**.
- Ethereum-spendable wA666: **31,484.722253**.
- Outstanding export claims: **0.000000**.
- Pending return imports: **0.000000**.
- Settlement reserve: **198.150288 pfUSDC**.
- Supply invariant: **true**.
