# A666 NAV arbitrage probe — clip 02

Date: 2026-08-11
Workflow: `a666-arbclip02-20260811`
Input: **50.000000 Ethereum USDC**

## Result

The larger clip completed successfully through the requested terminal asset,
pfUSDC:

1. **50.000000 USDC** bought **66.309478 wA666** on Uniswap.
2. The pool spot moved from **$0.7392477738** to **$0.7681671318** per wA666,
   an increase of **3.9119979868%**.
3. The post-trade discount to verified NAV narrowed from **18.1827230934%** to
   **14.9820328680%**.
4. Exactly **66.309478 wA666** was burned and imported as
   **66.309478 PFTL A666**.
5. Exactly **66.309478 A666** was redeemed at verified epoch-6 NAV of
   **$0.90353505**.
6. The redemption paid **59.882981 pfUSDC**.

The token-denominated gross result is **+9.882981 pfUSDC**, or **19.765962%**
versus the 50.000000 USDC input, before Ethereum gas and before pfUSDC egress
to external Ethereum USDC. This is not yet an external-USDC profit claim.

## Exact accounting

- Average Uniswap execution price: **$0.7540400182 per wA666**.
- Average execution premium to the initial spot: **2.0009859953%**.
- Rounded NAV base value: **59.912938 pfUSDC**.
- Governed redemption payout: **59.882981 pfUSDC**.
- Redemption spread retained: **0.029957 pfUSDC**.
- Joe A666: **99.000000 -> 165.309478 -> 99.000000**.
- Joe pfUSDC: **13.614589 -> 73.497570**.
- Settlement reserve: **198.150288 -> 138.237350 pfUSDC**.
- Protected wallet wA666: **103.000000 -> 169.309478 -> 103.000000**.
- Wallet USDC: **563.758626 -> 513.758626**.
- All Uniswap ERC-20 and Permit2 allowances: **0** at close.
- Outstanding bridge claims: **0**.
- Pending return imports: **0**.
- Supply invariant: **true**.

## Finality and transactions

- Uniswap buy:
  `0xa06d4bb290914c3197f6ad567f1eab95241d1802bde73c3883b3019f52da2612`.
- Ethereum return burn:
  `0xba6aca4f7224037a7e873389e1bcc87dcb5e643c9b7d61049390237839292826`.
- PFTL return import:
  `9be82a9b8cebc63b551d6be7370cf4f181e9139b0ed6f5d2e93e031c3adedcadb5f126458704ee5e37a6975018c54f7d`
  at height 874.
- PFTL NAV redemption:
  `34a2f9dbfe9da76aac139c34ceed5ae92413125ae0b261c09ebf06b1acbd64d4549357ee35079ef0ce7d3a8d7adea3da`
  at height 875.
- All six validators converged at height 875, state root
  `a7f60c236dbdae8a77677999ad41bc529ccee59ed74f07dde1bae3227ac4f669e990b94cc1938a0f7d3037b6e3e872cf`,
  with empty mempools.
- Ten Ethereum transactions, including temporary allowance preparation and
  revocation, used **0.000054866059140779 ETH** total gas.

The first proof observation attempt used an HTTPS URL directly and was refused
before proof construction because validator proof commands require a loopback
HTTP execution-client endpoint. That partial evidence was preserved under
`return/proof-attempt-https-rejected/`. The accepted proof used the established
validator-local HTTP proxy; no PFTL mutation or duplicate Ethereum transaction
occurred during the refused attempt.
