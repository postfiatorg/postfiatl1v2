# A666 Wallet Round Trip — Live Acceptance — 2026-08-12

## Verdict

`PASS`. The wallet-driven workflow moved **10.000000 USDC** through the full
canonical A666 route defined in `docs/runbooks/A666-ROUND-TRIP-DEFINITION.md`.
It did not substitute the shorter USDC-to-pfUSDC-to-USDC bridge loop.

The run used workflow `a666-wallet-20260812-c2e3c3`. PFTL finished at height
891 with the same tip and state root on all six validators and an empty
mempool. Ethereum proof replay and PFTL transaction replay were rejected.

## Exact live-funds path

| Stage | Exact result |
|---|---|
| Ethereum deposit | 10.000000 USDC deposited to the epoch-6 successor vault |
| PFTL ingress | 10.000000 pfUSDC claimed at heights 876–878 |
| Verified-NAV issue | 10.000000 pfUSDC produced 11.012575 A666 at NAV epoch 6, NAV $0.90353505 |
| Ethereum export | 11.012575 wA666 proof-minted; destination consumption finalized at height 882 |
| Uniswap forward | 11.012575 wA666 sold for 8.427115 USDC |
| Uniswap reverse | 8.427115 USDC bought 10.998835 wA666 |
| PFTL return | 10.998835 wA666 burned and imported as A666 at height 883 |
| Verified-NAV redeem | 10.998835 A666 produced 9.921265 pfUSDC at NAV epoch 7, NAV $0.90248000; finalized at height 889 |
| Successor egress | 9.921265 pfUSDC burned at height 890 and withdrawn as 9.921265 Ethereum USDC |
| Accounting closure | PFTL settlement finalized at height 891 |

## Ethereum transaction anchors

| Mutation | Transaction |
|---|---|
| Successor-vault deposit | `0xb8da837db698f89fd25b18d8a0bd810f2d4c649636bf3b8aba81b07e4f931a0f` |
| Uniswap wA666 to USDC | `0x2a9574c3b9881bc8b3f0676ef5030d4e58bb4cc0c15d7b6d0f082f7b630614a6` |
| Uniswap USDC to wA666 | `0xfcaa4c140918e9895b46fc7846579712d8693fba0022e329c5d403296d3ff616` |
| wA666 return burn | `0x7d7266a06dba6d21d6dbc9ac9e260c04c952783343559787c9c4de9d27d42219` |
| Successor-vault withdrawal | `0xab45e6e420674b65a1481a0b79f79b2dd7c2af54881d13281928b72d363e4131` |

## Conservation and user impact

- Wallet USDC: **513.758626 before -> 513.679891 after**.
- USDC round-trip cost: **0.078735 USDC**, excluding ETH gas.
- Protected wallet wA666: **103.000000 before -> 103.000000 after**.
- Current-run market-loop loss: **0.013740 wA666**.
- Successor vault: **0.481552 USDC** after the withdrawal; all current-run
  obligations are closed.
- Final A666 bridge supply invariant: true, with zero active reservations and
  zero export entitlements.

The NAV refresh used for redemption excluded **33.264281 pfUSDC** of reported
settlement reserve that was not backed by an active successor-vault bucket. It
counted **114.923317 pfUSDC** of active proof-backed settlement inventory. The
Uniswap price was not used in NAV.

## Release and proof anchors

- Fleet release: `a666-nav-overlay-cbbf53e`
- Release commit: `cbbf53ec0415754562c3cb4f4a469a95a80a8298`
- Deployed node SHA-256: `983dcc11784c80bca937c55de6945bf40613aee239006779151f210401c1f95d`
- Successor egress program vkey:
  `0x0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87`
- Successor egress ELF SHA-256:
  `4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0`
- CUDA prover SHA-256:
  `04888e5579566e64cdd13d8e3b41b10ebd869a4e8bfafd9e5508a61d117ab078`
- Proof nullifier:
  `0x5bcb48dfbf32c4be1d1d0e6727b6cfbe50b9a726069b98e675430d5679b7a26c`

The dedicated RTX 4090 prover instance used for this acceptance was destroyed
after proof verification and accounting closure. No unrelated GPU instance was
modified.
