# A666 Six-Round-Trip Campaign Summary

## Units

All token amounts below are human-readable decimal token amounts. USDC,
pfUSDC, A666, and wA666 each use six decimal places. For example,
`10,000,000` integer atoms means **10.000000 tokens**, not ten million tokens.

## Result

Six distinct, live-funds A666 round trips completed with `PASS` receipts. Each
round performed the complete route: USDC deposit, pfUSDC claim, verified-NAV
A666 issue, A666 export, proof-backed wA666 mint, both Uniswap swaps, wA666
return burn, PFTL return import, verified-NAV A666 redemption, pfUSDC burn,
successor-vault USDC withdrawal, and final settlement.

| Round | Deposit USDC | Issued A666 | Forward swap output USDC | Returned wA666 | Withdrawn USDC | Final wallet USDC | Receipt |
|---:|---:|---:|---:|---:|---:|---:|---|
| 1 | 10.000000 | 11.012575 | 8.047913 | 10.998834 | 9.932864 | 74.094307 | [PASS](round-01/roundtrip-PASS.json) |
| 2 | 10.000000 | 11.012575 | 8.047882 | 10.998834 | 9.932864 | 74.027171 | [PASS](round-02/roundtrip-PASS.json) |
| 3 | 10.000000 | 11.012575 | 8.047850 | 10.998834 | 9.932864 | 73.960035 | [PASS](round-03/roundtrip-PASS.json) |
| 4 | 10.000000 | 11.012575 | 8.047819 | 10.998834 | 9.932864 | 73.892899 | [PASS](round-04/roundtrip-PASS.json) |
| 5 | 10.000000 | 11.012575 | 8.047787 | 10.998833 | 9.932863 | 73.825762 | [PASS](round-05/roundtrip-PASS.json) |
| 6 | 10.000000 | 11.012575 | 8.047756 | 10.998834 | 9.932864 | 73.758626 | [PASS](round-06/roundtrip-PASS.json) |

The same capital was cycled sequentially. The campaign did **not** place
60 USDC at risk simultaneously.

## Aggregate economics

- Sequential deposits processed: **60.000000 USDC** across six rounds.
- A666 issued and exported: **66.075450 A666/wA666**.
- Forward Uniswap USDC proceeds: **48.287007 USDC**, all used in the paired
  reverse swaps.
- wA666 returned and burned: **65.993003 wA666**.
- Successor-vault withdrawals: **59.597183 USDC**.
- Wallet USDC: **74.161443 → 73.758626 USDC**, a campaign delta of
  **-0.402817 USDC**.
- Successor-vault USDC after settlement: **0.402817 USDC**. This exactly
  reconciles the wallet delta. The successor redemption queue is zero.
- Protected wallet wA666: **103.000000 before and after**.

## Terminal audit

- Six unique deposit transactions, six unique forward swaps, six unique
  reverse swaps, and six unique withdrawal transactions.
- Every withdrawal reports its burn consumed, proof nullifier consumed,
  withdrawal consumed, and replay rejected.
- All six PFTL validators converge at height **866**, block tip
  `19028fe85a0098fae9fc98a9db20a9ff15033c72920beceb38959626fd0ace19ea804b9115a0c2c72272e1f50e1dc9f7`,
  and state root
  `b12fa21587231f4c8a88978795e99a2ee04125e92488221c6052689fc1528c18df9985d2f4af1efc097e613c50c9ed8e`.
  Every mempool is empty.
- USDC and wA666 ERC-20-to-Permit2 allowances are zero, and both
  Permit2-to-router allowances are zero. See
  [allowance revocation evidence](uniswap-allowances-revoke.json).
- The campaign A100 prover instance was destroyed; its former SSH endpoint
  refuses connections.
- All six validator SSH control sockets were closed.
