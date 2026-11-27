# Post Fiat x402 v2 Facilitator & MCP Payment Wrapper (prototype)

HTTP `402 Payment Required -> payment -> 200 OK` facilitator, resource-server
gate, and thin MCP JSON-RPC client wrapper. The three headers follow Post Fiat
Pillar 2 and Contract C1 (`postfiat.message.v1.X402PaymentRequired`,
`X402PaymentSignature`, `X402PaymentResponse`).

## Status: prototype, no chain adapter

Nothing in this folder reads or writes the chain. Do not gate anything of value
with it yet.

- **It ships no settlement adapter.** A payment counts as settled when the
  `settlementAdapter` that the embedding code passes to `createFacilitatorCore`
  says so. Without one, every payment is refused with
  `503 settlement_backend_unavailable`; the only exception is
  `pf-fastpay-settled` on the FastPay lane, which rests on the certificate
  format described below.
- **The object the payer signs is not a chain order.** It is a JSON format made
  up in `facilitator.js` (`signFastPayOrder`, `signConsensusTransfer`), signed
  with ML-DSA-65 under labels that exist only in that file. The chain's own
  order type is `OwnedTransferOrderV3`; a validator would not accept the object
  signed here, and the `version` label inside the payload does not change that.
- **The `pf-fastpay-settled` certificate is not a chain certificate.** Its vote
  format is also made up in `facilitator.js`, and it is checked against a
  committee that the embedding code supplies by hand.
- **The payer's key is not tied to the payer's account.** The signature is
  checked against the key inside the same payload.

It stays a prototype until its settlement adapter is built on the chain's real
RPC and on `OwnedTransferOrderV3`. The daemon enforces this: with
`X402_MODE=production` it refuses to start without an adapter, and `server.js`
has none, so from the command line it runs only as `X402_MODE=prototype` (the
default). Code that embeds the daemon passes its adapter as
`createFacilitatorFromEnv(env, { settlementAdapter })`.

## Wire Contract

- **`PAYMENT-REQUIRED`** (`402 Payment Required`): unpadded base64url canonical
  protobuf `postfiat.message.v1.X402PaymentRequired` (`x402_version = 2`).
- **`PAYMENT-SIGNATURE`** (client retry): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentSignature`. Its `signed_payload` carries this
  prototype's signed transfer object, whose `pf.ptr` v5 `INLINE` memo
  (`kind = CONTENT_KIND_X402 (14)`, `schema = 402`) embeds the 32-byte
  `request_hash`, and whose `nonce_hex` names the challenge it answers.
- **`PAYMENT-RESPONSE`** (`200 OK`): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentResponse` carrying `settled = true` and the
  payment reference as the adapter reported it.

## Resource Gate

`createX402Gate` answers an unpaid request with a challenge and serves a paid
one. It serves only when all of this holds:

- The challenge is one this gate issued. Every gate keeps its own challenges;
  a challenge from another gate or from `POST /v1/challenge` is unknown to it.
- The challenge and the signed transfer are for exactly the gate's own scheme,
  lane, asset, recipient and price (not less, not more), and for the hash the
  gate computes of the request in front of it (method, path, content type,
  body).
- The signed transfer names this challenge (`nonce_hex`). The
  `PAYMENT-SIGNATURE` wrapper itself is not signed, so without this a transfer
  could be moved to another challenge.
- The challenge has not been used. It is held while its payment settles,
  removed when the payment has settled, and given back when settlement fails
  so the payer can present the same payment again.

Open challenges live in memory. After a restart of the gate, a challenge it
gave out before is unknown, and a payer who already settled a
`pf-fastpay-settled` payment for it cannot present that payment any more.

## Security Invariants

1. **Immutable deduplication key:** Spent-payment deduplication keys strictly on
   immutable `order_hash` (`PAYMENT_LANE_FASTPAY`) or `tx_hash`
   (`PAYMENT_LANE_CONSENSUS_PFT` / `PAYMENT_LANE_CONSENSUS_ISSUED`), never on
   `certificate_digest` (eliminating validator-signature malleability replays).
   `FileSpentStore` keeps the used payments in an append-only file, one JSON
   line each, that several processes on one machine may share. A payment is
   marked by appending its line and syncing it to disk before anything is
   served; when two processes append the same payment, the first line in the
   file wins. If the file cannot be read or written, the payment is refused
   with `503 spent_store_unavailable`. The file must be on a local filesystem.
   `MemorySpentStore` forgets everything on restart and is for tests only.
2. **Local verification before the settlement adapter is called:** Rate limits,
   challenge expiry, `network`/`lane`/`asset_id`/`pay_to`/amount,
   `pf.ptr` v5 `INLINE` `request_hash` memo binding, Sui-inspired FastPay fee
   floor (`25 + ceil(weight/512) + 1000 * net_new_objects + inputs`), and
   ML-DSA-65 signatures are all verified locally before the adapter is called.
3. **Committee rotation:** `pf-fastpay-settled` certificates (this prototype's
   format) verify against epoch-tagged ML-DSA-65 committees and reject stale
   epochs.
4. **Limits that cannot be turned against a payer:** asking for a challenge
   and presenting a payment are each limited per address, in separate counts.
   The per-payer limit counts only a payment that passed every local check and
   is not used yet, so junk that names a payer's key, or an old payment of that
   payer sent again, does not use up the payer's allowance. Every issuer of
   challenges (each gate, and `POST /v1/challenge`) holds at most
   `maxOpenChallenges` (default 10,000) and refuses new ones with
   `503 challenge_store_full` until older ones expire; it never drops a
   challenge a payer may already have paid for. A challenge lives at most 15
   minutes. A limiter holds at most `maxRateLimitBuckets` (default 50,000)
   counters and forgets a counter when its window is over. A full limiter
   forgets its oldest counter instead of refusing a new key, so a flood of
   throwaway keys cannot lock other callers out. Payers are counted in a table
   of their own, so made-up API keys cannot push a payer's counter out of it.

## Paying Client

`X402HttpClient` and `X402McpClient` (`mcp-client.js`) pay a `402` challenge
once per call and retry.

- `maxBudgetAtoms` is a total for the lifetime of the client object, fees
  included. A payment counts the moment it is signed and is never given back:
  whoever receives a signed payment can settle it, whatever they answer. When
  the next payment does not fit, the call fails with `budget_exceeded`.
- A caller can state what it agreed to pay:
  `request(url, { expectedPayment: { amountAtoms, assetId, payTo } })` or
  `callTool(name, args, { expectedPayment })`. A challenge with another price,
  asset or recipient is refused with `payment_terms_mismatch` and nothing is
  signed. Without `expectedPayment` the server's challenge sets all three,
  limited only by the budget and the `allowedAssets` / `allowedNetworks` lists.

## Endpoints

- `GET /healthz` — liveness and active committee epoch metadata.
- `POST /v1/challenge` — issue a tracked `X402PaymentRequired` challenge. Open
  to any caller, who chooses amount and recipient; no gate accepts it.
- `POST /v1/verify` — locally verify a `PAYMENT-SIGNATURE` header without settling.
- `POST /v1/settle` — verify locally, check spent-payment deduplication, settle
  through the injected adapter, and return `PAYMENT-RESPONSE`. The caller may
  hand in the challenge terms itself (`payment_required_header`), so this
  endpoint does not prove that anybody's price was paid.

## Running

```bash
# Run automated end-to-end smoke + replay + committee-rotation + MCP tests:
node tools/x402-facilitator/test_x402_e2e.js

# Run the refusal tests (gate terms, used payments, client budget, limits, daemon start):
node tools/x402-facilitator/test_x402_refusals.js

# Run the facilitator daemon as a prototype (binds 127.0.0.1:4020 by default).
# X402_SPENT_STORE_PATH is required: the file that keeps used payments across restarts.
X402_SPENT_STORE_PATH=/var/lib/postfiat-x402/spent.jsonl node tools/x402-facilitator/server.js
```

Both test files and the daemon load `../../wallet-proxy/message-protocol.js`, so
run `npm ci` in `wallet-proxy` first.
