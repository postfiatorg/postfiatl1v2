# RPC Root Cause Notes

Date: 2026-06-27

## Balance and State Findings

- Live WAN devnet `account` responses from `192.0.2.10:27650` are flat:
  `result.balance` and `result.sequence` are present directly under `result`.
- The reported zero-balance failure was client-side masking: Wallet and Send
  converted RPC errors or malformed responses into rendered `0 PFT`.
- The fix parses account envelopes explicitly, supports both `result.balance`
  and `result.account.balance`, and surfaces RPC failures as UI errors.
- Wallet and Send now refetch when their tab is active. The current app
  conditionally mounts each tab, but explicit `visible` props keep the behavior
  correct if views become persistent later.

## Heartbeat Finding

- The browser heartbeat sends a fire-and-forget `status` request and does not
  add a pending entry. `pending.size` does not grow from heartbeat responses.
- The proxy opens a separate TCP connection per WebSocket message, so heartbeat
  traffic does not interleave on the same validator TCP line stream as account
  or owned-object requests.

## FastPay Finding

- `wrap_owned` can return before `owned_objects` exposes the newly created
  object through the read path.
- Wallet and Send now poll `owned_objects` every 500 ms for up to 10 seconds
  after a successful wrap, waiting for `total_value >= pre_wrap_total + amount`.
