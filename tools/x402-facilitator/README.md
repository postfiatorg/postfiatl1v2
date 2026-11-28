# Post Fiat x402 v2 Facilitator, FastPay RPC v2 Client & MCP Payment Wrapper

HTTP `402 Payment Required -> payment -> 200 OK` facilitator, resource-server
gate, native chain settlement adapter (`OwnedTransferOrderV3` & `SignedPaymentV2`),
direct TLS 1.3 Protobuf FastPay RPC v2 client, `OwnedObjectPool`, and thin MCP
JSON-RPC client wrapper. The three headers follow Post Fiat Pillar 2 and
Contract C1 (`postfiat.message.v1.X402PaymentRequired`, `X402PaymentSignature`,
`X402PaymentResponse`).

## Production vs. Prototype Settlement Modes

- **Production (`X402_MODE=production`):** Enforces **native chain payloads**
  (`OwnedTransferOrderV3` for FastPay and `SignedPaymentV2` for consensus PFT)
  verified against a pinned active committee (`X402_COMMITTEE_JSON` +
  `X402_DOMAIN_JSON`) and durable spent-payment storage (`X402_SPENT_STORE_PATH`).
  In `X402_SETTLEMENT_FINALITY_MODE=certificate` (1-RTT), durable certificate
  persistence (`X402_CERT_STORE_PATH`, backed by `FileCertificateStore` with
  `fsync`) is also mandatory.
- **Prototype (`X402_MODE=prototype`, default CLI mode):** Allows local JSON
  test orders (`signFastPayOrder`, `signConsensusTransfer`) for unit/local tests
  without a live validator committee. Do **not** use prototype mode with real funds.

## Sub-Second FastPay Settlement Architecture

`createChainSettlementAdapter` (`chain-adapter.js`) supports two transports and
two finality modes:

- **Direct TLS 1.3 Protobuf FastPay RPC v2 (`fastpay-rpc-v2.js`):** Pass
  `fastpayRpcV2Endpoints` and `fastpayRpcV2TlsOptions` (`ca`, `cert`, `key`) to
  talk directly to validator `postfiat-fastpay-rpc-v2` listeners (`7,611 B`
  `owned_sign_v3`, `16,716 B` `owned_apply_v3_fast` with automatic
  `OrderNotCached` -> `owned_apply_v3_full` fallback). JSON-RPC v1 (`rpcUrls`)
  remains supported as a fallback and for consensus/committee reads.
- **1-RTT Certificate Finality (`settlementFinalityMode: 'certificate'`):**
  Races `owned_sign_v3` across all active committee validators, verifies
  `>= 2f+1` distinct ML-DSA-65 votes locally against the pinned `registry_root`,
  durably `fsync`s the certificate to `FileCertificateStore`, optionally applies
  synchronously to a co-located validator (`coLocatedValidatorId` +
  `coLocatedValidatorSyncApply: true`), and returns HTTP `200` while a
  background worker converges `owned_apply_v3` across the remaining validators
  with exponential retry (treating unsigned `postfiat-fastpay-held-effect-v1`
  holder receipts cleanly). Call `adapter.replayPendingCertificates()` on
  startup to resume any unconfirmed WAL entries after a crash.
- **2-RTT Applied Quorum (`settlementFinalityMode: 'applied_quorum'`):**
  Waits for both `>= 2f+1` verified votes and `>= 2f+1` signed
  `FastPayApplyAckV1` acknowledgments before returning HTTP `200`.

## Wire Contract

- **`PAYMENT-REQUIRED`** (`402 Payment Required`): unpadded base64url canonical
  protobuf `postfiat.message.v1.X402PaymentRequired` (`x402_version = 2`).
- **`PAYMENT-SIGNATURE`** (client retry): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentSignature`. In production, `signed_payload`
  carries a native `SignedOwnedTransferOrderV3` (FastPay) or `SignedPaymentV2`
  (consensus), whose `pf.ptr` v5 `INLINE` memo (`kind = CONTENT_KIND_X402 (14)`,
  `schema = 402`) embeds the 32-byte `request_hash`.
- **`PAYMENT-RESPONSE`** (`200 OK`): unpadded base64url canonical protobuf
  `postfiat.message.v1.X402PaymentResponse` carrying `settled = true` and the
  immutable 48-byte `order_hash` (FastPay) or `tx_hash` (consensus).

## Resource Gate

`createX402Gate` answers an unpaid request with a challenge and serves a paid
one only when:

- The challenge was issued by this exact gate (never from another gate or
  `POST /v1/challenge`).
- The signed transfer matches the gate's exact scheme, lane, asset, recipient,
  price, and locally computed `request_hash` (method, path, content type, body).
- The challenge has not been consumed (held during settlement, consumed on
  success, released on settlement failure).

## Security Invariants

1. **Immutable deduplication key:** Spent-payment deduplication keys strictly on
   immutable 48-byte `order_hash` (`PAYMENT_LANE_FASTPAY`) or `tx_hash`
   (`PAYMENT_LANE_CONSENSUS_PFT` / `PAYMENT_LANE_CONSENSUS_ISSUED`), never on
   `certificate_digest`. `FileSpentStore` appends and `fsync`s to a local
   append-only JSONL file before serving; I/O errors fail closed with
   `503 spent_store_unavailable`.
2. **Local verification before touching validators:** Rate limits, challenge
   expiry, `network`/`lane`/`asset_id`/`pay_to`/amount, `pf.ptr` v5 `INLINE`
   `request_hash` memo binding, Sui-inspired FastPay fee floor
   (`25 + ceil(weight/512) + 1000 * max(0, outputs - inputs) + inputs`), and
   ML-DSA-65 signatures are all verified locally first.
3. **Committee rotation & quorum verification:** Certificates verify against
   epoch-tagged ML-DSA-65 committees (`CommitteeRegistry`) and reject stale
   epochs or duplicate validator votes.
4. **Abuse-resistant bounded limits:** Separate per-IP challenge and payment
   counters, per-payer buckets counted only after local verification passes,
   `maxOpenChallenges` (default `10,000`, 15-minute max TTL), and bounded
   sliding-window limiter tables (`maxRateLimitBuckets = 50,000`).

## Paying Client & `OwnedObjectPool` (`mcp-client.js`)

`X402HttpClient` and `X402McpClient` pay a `402` challenge once per call and
retry with `PAYMENT-SIGNATURE`:

- **`nativeFastPay`:** Pass a config object or callback returning `{ domain, recovery, payeePubkeyHex, inputs, inputTotalAtoms, nextNonce }` to sign real `OwnedTransferOrderV3` payloads (`native-chain.js`).
- **`OwnedObjectPool`:** Pre-split `K` owned objects so agents can run `K`
  concurrent 1-RTT x402 calls without blocking on remote change-output apply or
  risking equivocation locks (`checkout`, `recordSpent`, `confirmPending`,
  `releaseUnspent`, `needsReplenish`).
- **`maxBudgetAtoms`:** Hard lifetime cap across calls (fees included), reserved
  before signing because signed orders have no protocol-level refund.
- **`expectedPayment`:** Pass `{ expectedPayment: { amountAtoms, assetId, payTo } }`
  on `request(...)` or `callTool(...)` to refuse mismatched challenges with
  `payment_terms_mismatch` before signing.

## HTTP Endpoints (`server.js`)

- `GET /healthz` — liveness and active committee epoch metadata.
- `GET /x402/certificates/:orderHash` — durable certificate & background
  quorum-apply convergence status (`FileCertificateStore`).
- `POST /v1/challenge` — issue a tracked `X402PaymentRequired` challenge.
- `POST /v1/verify` — locally verify a `PAYMENT-SIGNATURE` header without settling.
- `POST /v1/settle` — verify locally, check spent-payment deduplication, settle
  through the configured adapter, and return `PAYMENT-RESPONSE`.

## Running Tests & Daemon

```bash
# First install wallet-proxy dependencies (message-protocol.js):
(cd wallet-proxy && npm ci)

# Run all 6 test suites (contract, e2e, native, refusals, W1 latency/replay, W3/W4 straggler):
for t in tools/x402-facilitator/test_*.js; do node "$t"; done

# Run facilitator daemon in production 1-RTT certificate mode:
X402_MODE=production \
X402_SPENT_STORE_PATH=/var/lib/postfiat-x402/spent.jsonl \
X402_CERT_STORE_PATH=/var/lib/postfiat-x402/certs.jsonl \
X402_SETTLEMENT_FINALITY_MODE=certificate \
X402_COLOCATED_VALIDATOR_ID=validator-2 \
X402_RPC_URLS="http://127.0.0.1:27650,..." \
X402_COMMITTEE_JSON='...' \
X402_DOMAIN_JSON='...' \
node tools/x402-facilitator/server.js
```
