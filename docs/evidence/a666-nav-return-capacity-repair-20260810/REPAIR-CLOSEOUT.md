# A666 return-capacity protocol repair closeout

Date: 2026-08-10

## Outcome

The protocol invariant was preserved and the stranded **13.571391 wA666** burn
was recovered end to end. No arbitrary outstanding claim was consumed and no
ledger file was edited.

Five historical PFTL export packets had been debited and minted on Ethereum,
but their Ethereum `PacketConsumed` receipts had never been acknowledged on
PFTL:

| PFTL acknowledgement height | Amount | Result |
| ---: | ---: | --- |
| 867 | 31,386.197455 A666 | Accepted after a HotStuff view change |
| 868 | 1.000000 A666 | Accepted |
| 869 | 1.000000 A666 | Accepted |
| 870 | 100.000000 A666 | Accepted |
| 871 | 1.000000 A666 | Accepted |
| **Total** | **31,489.197455 A666** | **Outstanding claims reduced to zero** |

The original **13.571391 wA666** burn was imported at height 872. Its PFTL
A666 was redeemed at verified epoch-6 NAV at height 873:

- NAV: **$0.90353505 per A666**.
- A666 redeemed: **13.571391**.
- Rounded base value: **12.262228 pfUSDC**.
- Governed redemption multiplier: **9,995 bps**.
- pfUSDC paid: **12.256096**.
- Spread retained: **0.006132 pfUSDC**.
- Joe A666: **112.571391 -> 99.000000**.
- Joe pfUSDC: **1.358493 -> 13.614589**.
- Final outstanding claims: **0**.
- Final pending return imports: **0**.
- Final supply invariant: **true**.

Return-import transaction:
`9779ca64dc6a093c84a46a5af2012cead78b7aab2095ead2ab48c4d7042cafde8952580fe325dba4af9e97401a6fb11c`

NAV-redemption transaction:
`cefbdbecfb4db832301ca5768baefa6cbc1be06ffb8a8881384716ac3b251ae226208bf990c99b4777a64c451611bfe3`

## Permanent guardrails

1. Supply status exposes the count and exact atom total of source-debited export
   packets and verifies that the total equals outstanding bridge claims.
2. Supply status exposes `available_return_import_atoms`, which is distinct
   from primary-market redemption capacity and equals acknowledged Ethereum
   spendable supply only while the route is active.
3. The Ethereum burn driver requires a fresh, route-bound PFTL supply status and
   refuses to sign a burn larger than return-import capacity.
4. Remote finality tooling verifies the active executable path, binary SHA-256,
   topology argument, and topology SHA-256 for transport and RPC processes on
   all six validators before quoting or signing.

The live h873 finality artifacts, including the 12-service runtime identity
report, are under
`../a666-nav-arbitrage-probe-20260810/clip-01/primary-redeem/finality-h873/`.
