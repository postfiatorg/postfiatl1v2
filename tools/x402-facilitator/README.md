# Post Fiat x402 v2 Facilitator & MCP Payment Wrapper

Standalone HTTP `402 Payment Required -> FastPay / PFUSDC payment proof -> 200 OK`
facilitator, resource-server gate, and thin MCP JSON-RPC client wrapper conforming
to Post Fiat Pillar 2 and Contract C1 (`postfiat.message.v1.X402PaymentRequired`,
`X402PaymentSignature`, `X402PaymentResponse`).

## Wire Contract

- **`PAYMENT-REQUIRED`** (`402 Payment Required`): unpadded base64url canonical
  protobuf `postfiat.message.v1.X402PaymentRequired` (`x402_version = 2`).
- **`PAYMENT-SIGNATURE`** (client retry): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentSignature` carrying the ML-DSA-65 signed
  transfer payload whose `pf.ptr` v5 `INLINE` memo (`kind = CONTENT_KIND_X402 (14)`,
  `schema = 402`) embeds the 32-byte `request_hash`.
- **`PAYMENT-RESPONSE`** (`200 OK`): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentResponse` carrying `settled = true` and the
  verified `FastPayPaymentRef`.

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
2. **Local verification before validator calls:** Rate limits (per-IP and
   per-key), challenge expiry, `network`/`lane`/`asset_id`/`pay_to`/amount,
   `pf.ptr` v5 `INLINE` `request_hash` memo binding, Sui-inspired FastPay fee
   floor (`25 + ceil(weight/512) + 1000 * net_new_objects + inputs`), and
   ML-DSA-65 signatures are all verified locally before contacting validators.
3. **Committee rotation:** `pf-fastpay-settled` quorum certificates verify
   against epoch-tagged ML-DSA-65 validator committees and reject stale epochs.

## Endpoints

- `GET /healthz` — liveness and active committee epoch metadata.
- `POST /v1/challenge` — issue a tracked `X402PaymentRequired` challenge.
- `POST /v1/verify` — locally verify a `PAYMENT-SIGNATURE` header without settling.
- `POST /v1/settle` — verify locally, check spent-payment deduplication, settle
  via FastPay/consensus adapter, and return `PAYMENT-RESPONSE`.

## Running

```bash
# Run automated end-to-end smoke + replay + committee-rotation + MCP tests:
node tools/x402-facilitator/test_x402_e2e.js

# Run the refusal tests (gate terms, used payments, daemon start):
node tools/x402-facilitator/test_x402_refusals.js

# Run standalone facilitator daemon (binds 127.0.0.1:4020 by default).
# X402_SPENT_STORE_PATH is required: the file that keeps used payments across restarts.
X402_SPENT_STORE_PATH=/var/lib/postfiat-x402/spent.jsonl node tools/x402-facilitator/server.js
```
