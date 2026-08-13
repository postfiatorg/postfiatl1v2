# pfUSDC repair live acceptance

**Date:** 2026-08-13

**Verdict:** PASS

**Amount:** 1.000000 USDC
**Source:** one bounded slice of the already recovered 15.000000-USDC epoch-6 deposit

No new Ethereum deposit, MetaMask approval, contract deployment, operator
balance edit, or manual supply-cap change was used.

## Exact route

```text
epoch-6 source-series pfUSDC
  -> Asset Orchard private custody
  -> same epoch-6 source-series pfUSDC
  -> source-specific burn
  -> Ethereum USDC
  -> PFTL redemption settlement
```

The original ingress leg is the finalized 15.000000-USDC deposit recovered at
height 906. This acceptance uses exactly 1.000000 of that source series so it
does not violate the repair plan's instruction not to create a second deposit.

## Results

- Height 907: exactly 1.000000 source-series pfUSDC entered Asset Orchard.
- Height 908: the same note exited to the same source-series balance.
- Height 909: burn transaction
  `b9f05a5e0848410fe69c562681ca10471cc04e4ea2275afe68d87cebe5f96f60deb71fc2c27fb7cec0fe6ce3ba67f606`
  created a redemption against epoch-6 bucket
  `c4b2691e5e25df47ec1f9b9b2b977413de84f914e288a0547bdd9b607a3aaf5becdc3bad87c5b3134f0c1a3d9d5351e7`.
- Ethereum transaction
  `0xb97d8c6d7c7856907875991d888c23167589fd61235caeaefbecca54a1d0d5d5`
  transferred exactly 1.000000 USDC to
  `0x0c30d0a57f4f9bc035ca8e8be6bd2abae054b882`.
- Recipient USDC changed from 733.264791 to 734.264791.
- Successor-vault USDC changed from 15.081552 to 14.081552.
- The proof nullifier, burn, and withdrawal were consumed and replay was
  rejected.
- Height 910 settled PFTL accounting. All six validators converged on state
  root
  `661f2019d17c26540fc065f1ffaf658094457791013b6c207546d265c9b225eb15465f0502eba3671fe9bab0909e281e`
  with an empty mempool.

## Terminal balances

- Legacy pooled pfUSDC: 73.097570 before and after; it was not used.
- Epoch-6 source-series pfUSDC: 15.000000 before, 14.000000 after.
- Ethereum USDC: +1.000000.
- PFT fees: 45 atoms total (22 for Orchard ingress and 23 for the burn).
- Ethereum gas: 61,416,892,746,362 wei, paid by the relayer.

## Operational correction

The wallet withdrawal worker was still pinned to the pre-repair validator
release. Its live configuration now uses release
`pfusdc-pool-repair-8a62cf9`, the matching topology, and local binary SHA-256
`e6b31e715a025170747b4222f4afd703e0d9a4e7fe7f6ac998715848905d0ec5`.

The local Groth16 proof took 4,070,024 ms (67 minutes 50 seconds), so the
wallet's former “usually 20–40 minutes” claim was inaccurate. The wallet now
says local proof generation may take an hour or longer.

## Evidence index

- `live-acceptance-summary.json`: concise machine-readable result.
- `orchard-ingress/finality-r2/summary.json`: accepted six-validator ingress.
- `orchard-egress/finality/summary.json`: accepted six-validator egress.
- `burn/finality/summary.json`: source-specific burn finality.
- `withdrawal/proof-report.json`: Groth16 proof identity and size.
- `withdrawal/ethereum-withdrawal-result.json`: exact USDC deltas and replay rejection.
- `withdrawal/ethereum-receipt.json`: Ethereum receipt.
- `withdrawal/pftl-settle-summary.json`: height-910 accounting finality.
- `final/fleet-status-h910.json`: six-validator convergence.
- `final/wallet-assets.json`: terminal family and source-series balances.
- `final/vault-bridge-status.json`: settled redemption and source bucket.
