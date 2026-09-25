# Chain stuck at 1036: validator-5 cannot propose 1037 after a FastPay payment

**Date:** 2026-09-25 UTC · **Network:** `postfiat-wan-devnet-2`, release
`fastpay-committee-20260925-r3` on all six validators · **Written from:** `hetnzerxxx`

## Summary

FastPay works on the r3 release. A 0.001 PFT FastPay payment from a test wallet
to the `testing` wallet (`pf6395ef…b38`) was accepted: validators 0–4 signed and
applied it.

**Since that payment the chain cannot produce block 1037.** It has been at height
1036 for hours. Every ordinary transfer, faucet grant and bridge operation is
blocked until 1037 is certified.

## Cause

1. After a FastPay payment, validators 0–4 hold a pending FastPay effect that the
   next certified block must anchor (the r3 design). Validator-5 is not an eligible
   FastPay signer, so it never applied the payment and does not hold the effect.
2. Validator-5 is the proposer for height 1037, view 0. Its proposal omits the
   effect, and the others do not vote for it: *"insufficient block votes: got 1,
   need 5"*.
3. Validator-5 has now recorded its own vote for 1037 view 0, so any new view-0
   round for 1037 fails: *"conflicting block proposal vote already recorded for
   validator `validator-5` at height 1037 (recorded view 0, attempted view 0)"*.
4. Views 1–3 (validators 0, 1 and 2) need a consensus-v2 **timeout certificate**:
   *"nonzero-view block proposal omitted timeout certificate"*. Nothing has
   produced one.

The fleet is otherwise healthy: all six agree exactly at height 1036.

The r3 handoff anchored its own FastPay effect at 1034 with a following
transfer. That worked because the next proposer then held the effect. Whenever
the height after a FastPay payment falls to validator-5 at view 0, the chain
stalls this way. **This is a liveness bug in r3.**

## How to unstick it (not yet done)

- **Most likely route: submit through the wallet proxy's finality path.** The
  proxy (`wallet-proxy/rpc-routing.js`, around line 820) collects
  `consensus_v2_timeout_vote` from the validators, builds the timeout
  certificate and retries at the next view (`proxy_consensus_view`). A signed
  transfer submitted with `mempool_submit_signed_transfer_finality` through the
  proxy should get 1037 certified at view 1 (validator-0). A test script is ready
  but was **not run**:
  `/tmp/claude-1001/-home-postfiatchad/b2b15610-1f98-4b4a-85a9-2774a19183f5/scratchpad/proxy_submit.py`.
  It signs a 1 PFT faucet grant to `testing` and submits it through the proxy.
- **Or** a validator operator builds the view-1 timeout certificate directly
  (`aggregate_block_timeout_certificate` in `crates/node/src/finality_view_recovery.rs`).
- **The proper fix is in the node.** Either validator-5 must hold accepted
  FastPay effects, or a proposer must include pending effects it can verify
  from the certificate. Otherwise every FastPay payment risks a stall.

## State on hetnzerxxx

| Item | State |
|---|---|
| Height | 1036, all six agree |
| Faucet `pfcd4cc8…a95c` | 93.999828 PFT, sequence 4 |
| `testing` wallet `pf6395ef…b38` | 28 PFT in the account plus 0.001 PFT in one FastPay coin |
| `pft-fastpay` (tunnels plus wallet proxy on 127.0.0.1:18091) | **running**; stop it with `pft-fastpay stop` |
| `~/.pft/config.toml` | points at r3 (`runtime_binary`, `topology_file`), the local r3 signer `~/.local/lib/postfiat/releases/fastpay-committee-20260925-r3/postfiat-node` (SHA-256 prefix `a82684e2`), and the SDK `~/.local/lib/postfiat/rpc-sdk/postfiat-rpc-sdk-a3b95b23` |

Lost test funds, all devnet: two throwaway test wallets were deleted while
still holding PFT, about 3 PFT in total. One held 1.998998 PFT (a cleanup step
ran after a failed return transfer); the other held 1 PFT in a FastPay coin.

## StakeHub wallet code

StakeHub branch `wallet/pay-transfer-fastpay-20260925`, pushed and **not merged**:

- **Standard PFT transfer for Pay** (`pft_wallet/transfer.py`, API `/pft-send`).
  The payment is signed locally and proven on all six validators. It is never
  sent twice, keeps the 10-atom account reserve, and takes the passphrase in the
  terminal wallet's review step.
- **Parallel six-validator reads**, which cut balance load time from 38 to 14
  seconds.
- **`send_pft` tries views 0–3.** This does not work yet: views above 0 need a
  timeout certificate. Replace it with the proxy submission path above.
- The wallet tests passed (134) before the view change. They were not re-run
  after it.

StakeHub branch `wallet/pft-registry-20260924` is also pushed and not merged. It
has the PFT wallet registry and `scripts/setup-wallet-registry.sh`.

## Next steps

1. Unstick 1037 using the proxy finality path or a timeout certificate. Then
   confirm all six validators move past 1036.
2. Fix the node so a FastPay effect cannot stall the chain.
3. In StakeHub, route standard transfers and faucet grants through the proxy
   finality path, which does view recovery, in place of the SSH view loop. Then
   re-run the tests and merge both branches.
