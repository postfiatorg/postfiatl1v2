# FastPay is down: the FastPay committee still lists validator-5's old key

**Date:** 2026-09-25 UTC · **Network:** `postfiat-wan-devnet-2`, height about 1031,
release `a666-source-route-20260907` on all six validators · **Source checked:**
`postfiatl1v2` `328d7d36`

## Summary

FastPay has not worked since block 924. On 2026-08-26 validator-5's key was
rotated through governance (block 924, recorded in
[the Cobalt adversarial verification handoff](2026-08-26___postfiatchad__cobalt_adversarial_verification.md)).
The FastPay committee on the ledger was not rotated with it, so it still lists
validator-5's old key. Before signing anything for FastPay, every validator
checks that its local validator registry exactly matches that committee. None
of them match, so every validator refuses every FastPay payment.

This cannot be fixed from a wallet or client. It needs a change on the
validators: a node release, and possibly a governance transaction.

## What happens

A FastPay payment gets as far as moving PFT into a FastPay coin (the `wrap`
step, which is an ordinary certified block). The FastPay transfer itself then
fails:

```
owned_recovery_capabilities failed with owned_recovery_capabilities_failed:
local validator registry does not match the replicated FastPay committee
```

Observed 2026-09-25 from StakeHub's `pft` FastPay flow, through a local wallet
proxy tunnelled to all six validators. The wrap step committed; the transfer
was refused.

## Cause, with evidence

All checks below were read-only.

| Check | Result |
|---|---|
| `validator_registry.json` on all six validators | Identical (SHA-256 prefix `43e588344d75348b`), dated 2026-08-26 |
| FastPay committees on the ledger (`fastpay_recovery_committees`) | One: epoch 1, valid from height 11, new orders through height **10000**, quorum **5 of 6** |
| Committee keys also in the current registry | validators 0–4: yes. **validator-5: no** (its old key) |
| FastPay recovery policy | Activated at height 11 |

The check that refuses is `validate_local_fastpay_committee` in
`crates/node/src/fastpay_recovery_node.rs` (line 253). It requires the whole
local registry to equal the committee's key list, and it runs on every
FastPay recovery and apply path (lines 322, 358, 438, 799 and 876).

## Why a normal committee rotation does not fix it

Governance can rotate the FastPay committee
(`execute_fastpay_recovery_governance_update_v1` in
`crates/execution/src/owned_transfer_recovery.rs`), but only under these rules:

- the new committee's epoch must be the previous epoch plus 1;
- it must start exactly at the previous committee's `new_orders_through_height`
  plus 1, which is **height 10001**;
- it cannot change the recovery policy.

The chain is at height about 1031, and it only makes blocks when there are
transactions (1020 on 2026-09-09, 1031 on 2026-09-25). A rotation submitted
today would be accepted but would not take effect until height 10001. Until
then, epoch 1, with the stale key, is the only committee admitting new orders.

## Options

### A. Node fix and release (recommended)

1. **Short-term fix, a small code change.** Relax the local check so a
   validator signs when its own key is in the active committee, not only when
   the whole registry matches. With quorum 5 of 6, validators 0–4 would sign
   and certificates would reach quorum without validator-5.
   - The trade-off: FastPay would have no margin left. If any one of
     validators 0–4 is down, FastPay stops.
   - The exact-match check was put there deliberately, so this change needs
     review.
2. **Proper fix, a ledger-rule change.** Let a FastPay committee rotation
   start from the next block when the validator registry itself has been
   rotated. Better still, rotate the FastPay committee automatically in the
   same governance action as a validator key rotation, so this can't happen
   again. Then submit an epoch-2 committee built from the current registry
   (`FastPayRecoveryCommitteeV1::from_public_keys` with the six current keys).

Both need a new release on all six validators. The second also changes how the
ledger applies governance, so it needs the usual qualification.

### B. Rotate now and wait for height 10001 (not practical)

Submit the epoch-2 rotation now, valid from 10001. FastPay stays down until
the chain reaches 10001, about 9,000 blocks from now, at a rate of a few blocks
a week.

### C. Put validator-5's old key back (not recommended)

This would undo a legitimate governed key rotation to satisfy the FastPay
check. It weakens the security reason the key was rotated in the first place.

## Already done on the client side

On `hetnzerxxx`, everything FastPay needs besides the network fix is in place:

- `pft-fastpay start | stop | status` (`~/.local/bin/pft-fastpay`) runs SSH
  tunnels from local ports 27650–27655 to each validator's loopback RPC, and
  the `wallet-proxy` on `127.0.0.1:18091`. Each tunnel reconnects on its own.
  It is **stopped** until the network is fixed.
- `postfiat-rpc-sdk` is built from `328d7d36` at
  `~/.local/lib/postfiat/rpc-sdk/postfiat-rpc-sdk` (SHA-256 prefix
  `b9cc718b12052dee`), and `~/.pft/config.toml` points FastPay at it and at
  the local `postfiatl1v2/python`.
- Until then, the StakeHub wallet's Pay button uses a standard PFT transfer
  (one certified block, about 10 seconds, checked on all six validators). That
  works today.

## How to confirm the fix

1. On the validators: `validate_local_fastpay_committee` no longer refuses. For
   the proper fix, check that the ledger holds a committee whose keys equal
   `validator_registry.json`.
2. On `hetnzerxxx`: run `pft-fastpay start`, then send a small FastPay payment
   between two wallets on this machine. Expect the wrap, then the transfer,
   accepted on all six validators in about a second.

## Open items

- The wallet proxy's route warm-up also logged
  `FastPay route warmup found only 4/6 converged validators`. This was not
  investigated, and may be separate from the committee mismatch.
- One test wallet holds 1 PFT in a FastPay coin from the failed 2026-09-25
  test. That coin can only be spent once FastPay works again.
