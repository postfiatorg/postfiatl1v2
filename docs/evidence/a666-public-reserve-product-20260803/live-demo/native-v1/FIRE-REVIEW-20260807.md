# FIRE REVIEW 2026-08-07

## 1. WHAT THIS IS
Read-only readiness check for the held live-money campaign. Nothing has moved; live state remains unchanged at h778.

## 2. CURRENT BALANCES
- Holder: **1,358,493 atoms (1.358493 pfUSDC)**.
- EVM wallet: **74,161,443 atoms (74.161443 USDC)**, nonce 304.
- Protected wA666 baseline: **103,000,000 atoms (103.000000 wA666)**, never touched.
- Route finalized-unclaimed: **10,000,000 atoms (10.000000 pfUSDC)** from the mined deposit, block **25,698,310**, finalized h778, expires h1776, **998 blocks left**. Wall-clock rate is unavailable from read-only headers, so fire promptly.
- Constrained signer `0xe01e…5424`: **0 ETH**. Leg3b0 funds it as designed.
- Fleet: 6/6 at h778, identical state root, mempool empty. Running binary is pre-orchardfix `05330fb2…`; stage 0 is first.

Evidence: `/tmp/snaga-lr/{fleet,evm,pftl,expiry-blocks}.txt`.

## 3. WHAT WOULD MOVE, IN ORDER
- Stage 0: rolling orchardfix upgrade; no funds move; binary `25e60759…`.
- Leg1 claim: **10,000,000 atoms (10.000000 pfUSDC)**, holder becomes **11,358,493 atoms (11.358493 pfUSDC)**.
- Legs 2a/2b subscribe: **10,000,000 atoms (10.000000 pfUSDC)** to **11,027,135 atoms (11.027135 A666)**.
- Legs 3a/3b/3b0 export to EVM plus signer funding; gas-only, **<=0.0201 USDC**.
- Legs 3c-3e forward swap: approximately **8,057,858 atoms (8.057858 USDC)**, freshly simulated at fire time.
- Legs 3f-3h reverse: approximately **11,013,374 atoms (11.013374 wA666)**.
- Leg4 return import.
- Leg5a redeem.
- Leg5b bridge-out returns external USDC to the wallet.
- Maximum principal: **10.000000 USDC** (already deposited). Maximum gas ceilings: **0.9220 USDC**. Cumulative cap: **511.946845 USDC <= 530.000000 USDC**, headroom **18.975155 USDC**.

## 4. MANDATORY PRE-FIRE STEP
`/tmp/ghash-lr/s2-determination.txt` rules the current S1 binding **not fire-ready** because leg3e pinned calldata is validation-proven stale and reverting. Before any fire, run a fresh two-RPC fork simulation, issue a new S1b binding with fresh deadline/min-out/gas/nonce, then issue S2 only after leg3e finalizes.

## 5. STOP-NO-RETRY GATES
- Any packet or binding hash mismatch.
- Any unresolved `PENDING-FIRE-TIME` at execution.
- Any 128-block freshness breach on simulation.
- Any receipt other than status 1 or any wrong balance delta.
- Any cap projection above **530.000000 USDC**.
- Any replay or idempotency hit. The pftl_uniswap replay read returns empty-ledger NotFound (market_bridge.rs:1969): zero prior transitions exist, so replay is vacuously clean; vault-side replay verified consistent via deposit/claims sums.
- Any fleet divergence from 6/6.
- Any deviation anywhere means HOLD everything.

## 6. THE EXACT GO
Nothing executes without Sauron replying with the exact phrase: **GO FIRE-20L-EXEC-3** — and nothing else. No “looks good” or “proceed” authorizes the fire sequence. Each HELD packet boundary still gates individually after the GO.

## 7. READINESS TABLE
| Item | Status | Basis |
|---|---|---|
| Fleet 6/6 | GREEN | h778, common root, empty mempool |
| Packet hashes | GREEN | 16/16 |
| Credentials | GREEN | 7/7 existence checks |
| Cap arithmetic | GREEN | 511.946845 USDC <= 530.000000 USDC |
| Signer funding | GREEN-by-design | leg3b0 funds the 0 ETH signer |
| Service binaries | AMBER-planned | old release; stage 0 upgrades first |
| Replay | GREEN | pftl_uniswap ledger empty (no prior transitions possible); vault sums consistent |
| Expiry | GREEN | 998 blocks |
| S1 binding | RED-until-S1b | Fresh two-RPC simulation and S1b binding required, approximately 30 minutes, no funds at risk |

Sources: `/tmp/snaga-lr/` and `/tmp/ghash-lr/s2-determination.txt`.
