# A666/pfUSDC reserve demo — live run 01

**Date:** 2026-07-30

**Verdict:** PASS

**Classification:** controlled live run with Ethereum-mainnet USDC and the
deployed PFTL validator fleet

The run proved the narrow business claim required for the Monday, 2026-08-03
demonstration:

> An ordinary funded user can deposit Ethereum-mainnet USDC, claim pfUSDC on
> PFTL, exchange it for newly issued A666, have the base pfUSDC counted in the
> A666 reserve and fresh NAV, and redeem A666 back into pfUSDC without trading
> through Uniswap or receiving transferred operator inventory.

The canonical machine-readable result is [`summary.json`](summary.json).

## What happened

1. The user deposited `90.553629 USDC` into the governed mainnet pfUSDC vault.
2. PFTL finalized the deposit claim at height 546, giving the user
   `92.179855 pfUSDC` including the prior balance.
3. A fresh six-leg StakeHub witness was executed and proved with the exact
   governed SP1 verifier key.
4. A666 NAV epoch 3 finalized at `$0.90115750`; route epoch 4 pinned it.
5. The ordinary user paid `90.566329 pfUSDC` and received `100.000000` newly
   issued A666. The route credited `90.115750 pfUSDC` to base reserve and
   `0.450579 pfUSDC` to spread.
6. The unused Ethereum export entitlement was released. No wrapped A666 was
   minted and no Ethereum export occurred.
7. NAV epoch 4 counted the new `203.111605 pfUSDC` settlement-reserve total.
   Verified assets and supply rose proportionally, so NAV remained exactly
   `$0.90115750`. Route epoch 5 pinned the fresh NAV packet.
8. The user redeemed `1.000000 A666`, received `0.900707 pfUSDC`, and retained
   `99.000000 A666`.
9. All six validators converged at height 556 with identical route state,
   empty active reservation/export-entitlement state, and the supply invariant
   true.

## Economic deltas

| Measure | Before issue | After issue | Final after 1 A666 redemption |
|---|---:|---:|---:|
| Authorized A666 supply | 31,489.197455 | 31,589.197455 | 31,588.197455 |
| User A666 | 0.000000 | 100.000000 | 99.000000 |
| Settlement reserve | 112.995855 pfUSDC | 203.111605 pfUSDC | 202.210447 pfUSDC |
| User pfUSDC | 92.179855 | 1.613526 | 2.514233 |
| NAV | $0.90115750 | $0.90115750 | priced from epoch 4 |

The final route retained `89.214592 pfUSDC` of same-run base reserve. This is
the intended Monday proof: buyer-funded pfUSDC remains in reserve while the
buyer holds newly issued A666.

## Proof and consensus anchors

- Governed SP1 vkey:
  `0x00580ee8c389192568a29dc23d54c22e73a3a45203b22e3d5a934801871e11a7`
- Governed ELF SHA-256:
  `dd743c3867140fa5b8824272fcaba6e5d91f036b4848494a7f93bf21bbeb249e`
- Public values SHA-256:
  `e842676cc4a2b7f604651b6396559813b186d50c703942c45282e38c7523eb90`
- Pre-issue NAV packet:
  `559da38033e4c523f86b7d7570b6a2e828548dbf4fa54729ed9bfa9d3f5964ff68ebd657ffdd098fd06867c0ef9831f7`
- Post-issue NAV packet:
  `d9a798a26732f377c48bc38876b969df6deb2227bdbc5792721052a9db80df227a20a50de369604097868d8851e9d8d2`
- Final PFTL height: 556
- Final state root:
  `155fa39ea8d16f0175cf1f53ac4a88fdf7fb0ef49d498f787ddf17066b264280e6f0e94ed40a561f671623f1fb495723`

The post-issue NAV mark reused the fresh governed StakeHub proof because no
StakeHub portfolio transaction occurred between epochs 3 and 4. It replaced
the independently captured PFTL reserve overlay and circulating supply. It did
not reuse the old reserve value.

## Timing and Monday presentation scope

The complete controlled run from Ethereum deposit start through final fleet
capture took 71 minutes 18 seconds. This included preparing and proving a fresh
six-leg reserve witness. The Ethereum deposit itself finalized in 28 seconds;
the Ethereum-to-pfUSDC claim artifact was complete about 20 minutes after
deposit start. Once the pre-issue NAV and route were ready, the live
issue/NAV-refresh/redeem/fleet sequence completed in 8 minutes 24 seconds.

For Monday, prepare the fresh StakeHub proof and fund/claim pfUSDC before the
audience-facing segment. The live segment should demonstrate issue, the
reserve/NAV delta, route refresh, and partial redemption. Do not imply that the
full proof-generation path is sub-ten-minute.

## Deliberately not proved here

- private issuance or redemption;
- A666 bridge-out or wrapped-token minting;
- a Uniswap swap or liquidity change;
- the proposed generic multi-asset NRRS facility;
- large-capacity, concurrent, or production-GA behavior.

Those are separate workstreams. None is a prerequisite for the narrow Monday
proof.
