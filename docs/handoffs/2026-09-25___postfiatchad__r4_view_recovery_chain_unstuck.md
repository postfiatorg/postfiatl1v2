# Chain unstuck: release r4 lets validators skip a stalled proposer

**Date:** 2026-09-25 UTC · **Network:** `postfiat-wan-devnet-2` · **Release:**
`fastpay-committee-20260925-r4` on all six validators · **Written from:** `hetnzerxxx`

## Summary

The chain was stuck at height 1036 after a FastPay payment (see
[the stall handoff](2026-09-25___postfiatchad__chain_stuck_at_1037_after_fastpay.md)).
It is moving again, and the failure that caused the stall is fixed.

- Release **r4** is live on all six validators. It is r3 plus one change:
  consensus-v2 timeout votes and timeout certificates are checked against the
  finalized checkpoint, not full-history replay.
- Block **1037** was certified at view 1 by validator-0, past validator-5, which
  could not propose.
- The exact failure was reproduced live and got through: a FastPay payment, then a
  transfer at validator-5's turn (height 1043).
- StakeHub's transfer path (faucet and wallet Pay) now skips a stalled proposer
  automatically.

## Root cause: why the chain could not recover by itself

Two faults combined.

1. **The trigger (r3 design).** After a FastPay payment, validators 0–4 hold a
   pending effect that the next block must anchor. Validator-5 is not an eligible
   FastPay signer, so it does not hold the effect. When the next height is
   validator-5's turn at view 0, the others will not vote for its proposal.
2. **The reason it became permanent.** The normal escape is a view change:
   validators sign timeout votes, and the next view's proposer builds the block with
   a timeout certificate. But `create_block_timeout_vote` and
   `aggregate_block_timeout_certificate` ran with `verify_block_log: true`, which
   replays the full block history first. On this chain, full-history replay fails at
   **block 1011**:

   ```
   verify-blocks failed: block 1011 replay state validation failed: issued asset
   supply exceeds finalized NAV circulating supply for `02c46a36…05d7b`:
   global supply 313700595 exceeds finalized supply 304700595
   ```

   So no validator could ever sign a timeout vote. Each attempt took about 345 s,
   held the RPC server's mempool lock, and failed. The r3 handoff lists the 1011
   anomaly as known and unchanged.

## The fix

Commit `c1c81119` on `release/fastpay-committee-20260925-r4`, cherry-picked to
`main`. In `crates/node/src/finality_view_recovery.rs`, both timeout-vote signing
and timeout-certificate aggregation now use `verify_block_log: false`, the same
finalized-checkpoint trust basis r3 already uses for restore. A timeout vote only
states that a height and view timed out. Certificates are still verified against
the validator registry and quorum by the peers that receive them.

**Still open:** the trigger itself. A FastPay payment followed by validator-5's
turn still fails at view 0 and needs a view change every time. The proper fix is
for validator-5 to receive and hold accepted FastPay effects, or for a governed
FastPay committee rotation.

## Deployment evidence

| Item | Value |
|---|---|
| Source | `943c4ca7` (r3 release source) + `c1c81119` |
| Node binary SHA-256 | `44b6794f2f8eab577713dffb6e59880f8ee8c811863e99ed96ed1b0f4ec66bab` (needs glibc 2.39, the same as the validators) |
| Deployment publisher | new key `pfc531e00ffefecb8fabe40c6f47c69148df31e49c`; the private key is at `~/.postfiat/deployments/fastpay-committee-20260925-r4/keys/` on `hetnzerxxx` (mode 0600) |
| Staged files | `deployment-validator-units-stage` from r3's topology and circuit metadata; the units, env and bindings are byte-identical to r3 except for the release name |
| Rollout | `scripts/postfiat-safe-rollout` with `--rpc-tunnel-base-port 27650`. Preflight passed (Vultr, SSH, six-way convergence, signer registry). The signed backup was taken from validator-1 at height 1036 (state root `ba7cc012…`, checkpoint basis, verified). Order: validator-1 (canary), 0, 2, 3, 4, 5, all exit 0 |
| Processes | all 12 validator and RPC processes run `44b6794f` |
| Evidence | `~/.postfiat/deployments/fastpay-committee-20260925-r4/evidence/` (rollout state SHA-256 prefix `08c60c49`, stage report `617b3741`) |

## Live verification

| Step | Height | Result |
|---|---:|---|
| Timeout votes for (1037, view 0) | — | all six signed (before r4: none) |
| Faucet grant 1 PFT to `testing` at view 1 via validator-0 | 1037 | certified, all six agree |
| Faucet grant, normal path | 1038 | validator-0, view 0 |
| Reproduction: fund a test wallet, then filler grants | 1039–1041 | ok |
| FastPay 0.001 PFT, test wallet to `testing` (coin funding) | 1042 | accepted |
| Grant at validator-5's turn: view 0 failed, **view 1 via validator-0** | 1043 | certified; `testing` holds 2 FastPay coins (0.002 PFT), so the FastPay effect is anchored |
| Test funds returned to the faucet | 1044 | ok |

## StakeHub changes (branch `wallet/pay-transfer-fastpay-20260925`)

`pft_wallet/ce22.py` `send_pft`, used by the faucet and by wallet Pay, now works
like this:

1. It tries view 0 as before.
2. If that fails **without consuming the sender's sequence**, it collects
   `consensus_v2_timeout_vote` from all six validators, reaching their loopback RPC
   over SSH so it does not depend on the wallet proxy.
3. It submits the same signed transfer to the next view's proposer with the votes
   (`mempool_submit_signed_transfer_finality`, `proxy_consensus_view`).
4. It settles from the ledger on all six, as before. The sequence number makes a
   double send impossible.

Balance reads also run in parallel across the six validators. All 136 wallet tests
pass, including the new view-recovery tests.

## Operating notes

- `~/.pft/config.toml` on `hetnzerxxx` points at r4 (`runtime_binary`,
  `topology_file`, and `local_node_binary` = a local copy of the r4 binary).
- `pft-fastpay` (six tunnels plus the wallet proxy) is running. The proxy's own
  view recovery can also work now, but StakeHub no longer depends on it.
- FastPay still needs validators 0–4 all online (quorum 5, and validator-5 is
  ineligible).
