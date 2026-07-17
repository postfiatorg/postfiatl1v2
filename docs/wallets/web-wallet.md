# Web Wallet

The PostFiat web wallet is the browser self-custody wallet in `wallet-web/`.
It uses React, Vite, the generated wallet WASM module, an encrypted IndexedDB
vault, and a WebSocket-to-TCP proxy for validator RPC. Seeds, passphrases,
private keys, and decrypted WASM backup JSON stay in the browser process.

This page is the operator and developer guide for the current `v0.1.2` web
wallet behavior. The implementation details below are sourced from
`docs/specs/web-wallet.md`, `wallet-web/RPC_ROOT_CAUSE.md`, the live memo
blocker evidence file, and the code under `wallet-web/src/`.

## Setup

Install the browser app dependencies from the wallet directory:

```bash
cd postfiatl1v2/wallet-web
npm install
```

Start the local RPC proxy in a second shell. The README shows the same quick
start, but the important point is that the browser speaks WebSocket while the
validator RPC server speaks newline-delimited TCP JSON-RPC:

```bash
cd postfiatl1v2/wallet-proxy
npm install
RPC_HOST=127.0.0.1 \
RPC_PORT=27650 \
ALLOWED_ORIGINS=http://localhost:5173,https://localhost:5173,https://127.0.0.1:5173 \
node server.js
```

Start the Vite dev server:

```bash
cd postfiatl1v2/wallet-web
npm run dev
```

The default dev URL is `http://localhost:5173`. Production-style local checks
can use:

```bash
npm run build
npm run preview
```

## Config

`wallet-web/vite.config.js` runs the development server on loopback-only
`127.0.0.1:5173` and proxies WebSocket requests from `/rpc` to
`ws://127.0.0.1:8080`. Vite is a local development tool, never the public
wallet server. `npm run build` emits immutable static assets; the hardened
same-origin wallet proxy serves those assets with CSP and browser security
headers. The production CSP permits local WASM execution with
`script-src 'self' 'wasm-unsafe-eval'` but does not grant scheme-wide WebSocket
origins.

The optional HTTPS dev cert paths are controlled by:

| Variable | Default | Purpose |
| --- | --- | --- |
| `VITE_HTTPS_KEY` | `/tmp/vite-key.pem` | HTTPS private key for Vite dev. |
| `VITE_HTTPS_CERT` | `/tmp/vite-cert.pem` | HTTPS certificate for Vite dev. |

`wallet-proxy/server.js` accepts these environment variables:

| Variable | Default | Purpose |
| --- | --- | --- |
| `RPC_HOST` | `127.0.0.1` | Validator TCP RPC host. Remote infrastructure must be selected explicitly. |
| `RPC_PORT` | `27650` | Validator TCP RPC port. |
| `LISTEN_PORT` | `8080` | Local WebSocket proxy listen port. |
| `LISTEN_HOST` | `127.0.0.1` | Proxy listen address. Non-loopback requires both a token and an explicit origin allowlist. |
| `ALLOWED_ORIGINS` | local wallet origins | Exact comma-separated browser origin allowlist. |
| `WALLET_PROXY_API_TOKEN` | empty | Session mutation bearer. Required for non-loopback serving; use at least 32 random bytes. |
| `WALLET_PROXY_API_TOKENS_JSON` | empty | Optional principal-to-token JSON map for isolated sessions. Mutually exclusive with the single-token setting. |
| `WALLET_PROXY_API_TOKENS_FILE` | empty | Preferred production source: path to a nonempty, at-most-64-KiB principal-to-token JSON secret file. |
| `WALLET_PROXY_MAX_HTTP_BODY_BYTES` | `16777216` | Absolute HTTP body ceiling; route-specific limits may be smaller. |
| `WALLET_PROXY_MUTATION_RATE_LIMIT` | `120` | Authenticated mutations admitted per principal and rate window. |
| `WALLET_PROXY_MUTATION_RATE_WINDOW_MS` | `60000` | Principal mutation-rate window. |
| `WALLET_PROXY_MUTATION_CONCURRENCY` | `16` | Process-wide in-flight authenticated HTTP mutation ceiling. |
| `WALLET_STATIC_DIR` | `wallet-web/dist` | Immutable production wallet build served same-origin by the hardened proxy. |
| `INJECT_RPC_CAPS` | enabled unless set to `false` | Injects RPC capability fields into `server_info` responses. |

Capability injection currently reports `read_only: false`,
`mempool_submit_enabled: false`, and `mempool_submit_finality_enabled: true`
because the WAN devnet validators are expected to run with finality submit
enabled, while raw `server_info` does not expose those flags.

### Authenticated TLS deployment

`docker-compose.wallet-public.yml` is the supported Internet-facing profile.
Only the pinned Caddy edge publishes port 443; the wallet proxy is reachable
only on an internal Docker network and validators remain on private RPC
addresses. Caddy enforces TLS, a 16-MiB absolute request ceiling, HSTS, and
deletes Authorization/Cookie fields from access logs. The proxy independently
enforces exact browser origins, constant-time bearer matching, per-principal
rate limits, process-wide mutation concurrency, route-specific body limits,
and principal-scoped durable idempotency.

Create an operator-readable JSON secret such as
`{"demo-session":"<at-least-32-random-bytes>"}` outside the repository. Set
`WALLET_PROXY_API_TOKENS_FILE_HOST` to that file, the exact
`WALLET_PUBLIC_ORIGIN` (`https://...`), TLS certificate/key paths, public host,
and private RPC topology in an operator-owned environment file. Set
`WALLET_EDGE_UID` and `WALLET_EDGE_GID` to the numeric owner of the TLS private
key; the edge deliberately cannot read a key owned by another identity. Keep
the key mode at `0600`. Validate before starting:

```bash
docker compose --env-file /secure/wallet.env \
  -f docker-compose.wallet-public.yml config --quiet
docker compose --env-file /secure/wallet.env \
  -f docker-compose.wallet-public.yml up -d
```

The browser receives a session token out of band and keeps it only in
`sessionStorage`. Tokens are never URL parameters, validator RPC fields,
idempotency-store data, or access-log fields. Rotating one principal token
invalidates only that principal; another principal cannot replay or receive its
durable idempotency result.

In-app settings are stored in IndexedDB with the wallet settings record. The
More tab can change:

| Setting | Default | Behavior |
| --- | --- | --- |
| RPC endpoint | same-origin `/rpc` as `ws://` or `wss://` | Reconnects `RpcClient` and refreshes status/capabilities after save. |
| Swap server | `http://localhost:8787` | Used by the private swap companion API. |
| Auto-lock | `15` minutes | Controls how long decrypted seed and backup JSON stay in module memory after activity. |

The bridge vault is not a wallet setting or build-time destination. The wallet
calls `vault_bridge_route(asset_id)` and accepts only the complete active route
profile authenticated by replicated chain state. Before approval and deposit it
checks the connected source chain plus the exact vault and token runtime-code
hashes. The user-signed `depositV2` call commits the route-profile hash and epoch;
the emitted event, proxy relay, and PFTL validators all verify the same binding.
The proxy independently resolves the route from chain state and treats wallet
fields as assertions, so environment variables cannot redirect bridge funds.
Historical unbound `deposit` calls fail before token transfer on the v2 vault.

## RPC Endpoints Used

All RPC calls are sent through `RpcClient.call(method, params)` over the
WebSocket proxy. Read calls require no write capability. Mempool submission
methods require validator write capability; the finality transfer method
requires the finality submit capability.

| Method | Params sent by web wallet | Capability |
| --- | --- | --- |
| `account` | `address` | Read. |
| `account_tx` | `address`, optional `from_height`, `to_height`, `limit` | Read. |
| `account_assets` | `account` | Read. |
| `account_lines` | `account` | Read. |
| `account_offers` | `account` | Read. |
| `asset_info` | `asset_id` | Read. |
| `owned_objects` | `owner_public_key_hex`, optional `asset`, `limit` | Read. |
| `owned_recovery_capabilities` | none | Read active v3 FastPay committee/domain/windows. |
| `owned_certificate` | `certificate_digest` or `lock_id` | Read persisted recovery evidence. |
| `owned_recovery_status` | `lock_id` | Read ordered recovery state. |
| `owned_sign_v3` | `order_json`, `validator_id` | Persist-before-sign vote for a complete v3 owner-authorized transfer envelope. |
| `owned_apply_v3` | `cert_json` | Apply a v3 transfer certificate and return authenticated durable acknowledgements. |
| `owned_unwrap_sign_v3` | `order_json`, `validator_id` | Persist-before-sign vote for a complete v3 signed unwrap envelope. |
| `owned_unwrap_apply_v3` | `cert_json` | Apply a v3 unwrap certificate and return authenticated durable acknowledgements. |
| `mempool_submit_fastlane_primary` | `fastlane_primary_json` | Controlled signed submit for source-signed FastPay deposits and ordered recovery actions. |
| `wrap_owned` | legacy parameters | Disabled unsafe compatibility path; the wallet does not call it. |
| `unwrap_owned` | `object_id`, `owner_pubkey_hex`, `to_address` | Disabled compatibility path; default wallet unwrap must not call this. |
| `fee` | none | Read. |
| `transfer_fee_quote` | `from`, `to`, `amount`, optional `sequence`, `memo_type`, `memo_format`, `memo_data` | Read quote. Memo params must be included before signing v2 payments so fees include memo bytes. |
| `asset_fee_quote` | `source`, `operation_json` | Read quote. |
| `issuer_assets` | `issuer` | Read. |
| `offer_fee_quote` | `source`, `operation_json` | Read quote. |
| `offer_info` | `offer_id` | Read. |
| `book_offers` | `pays_asset`, `gets_asset` | Read. |
| `mempool_submit_signed_transfer_finality` | `signed_transfer_json` | Finality write; used first for non-memo native PFT transfers. |
| `mempool_submit_signed_transfer` | `signed_transfer_json` | Write; fallback for non-memo native PFT transfers if finality submit is unavailable. |
| `mempool_submit_signed_payment_v2` | `signed_payment_v2_json` | Finality-flag write; required for native PFT payments with memos and admitted by the RPC allowlist when `--allow-mempool-submit-finality` is enabled. |
| `mempool_submit_signed_asset_transaction` | `signed_asset_json` | Write; used for issued asset transactions. |
| `mempool_submit_signed_offer_transaction` | `signed_offer_json` | Write; used for DEX offer transactions. |
| `receipts` | `tx_id` | Read. |
| `tx` | `tx_id` | Read. |
| `status` | none | Read; also sent as a fire-and-forget heartbeat every 30 seconds. |
| `server_info` | none | Read; proxy may inject RPC capability fields. |
| `validators` | none | Read; used before FastPay owned-transfer vote collection. |
| `blocks` | optional `from_height`, `limit` | Read. |

## Wallet, Send, And Swap Flows

The unlocked app shell has five main tabs: Wallet, Send, Swap, NavCoins, and
More. Wallet state is initialized by loading settings and the encrypted vault,
initializing WASM, creating `RpcClient`, then creating `TxBuilder`. The sidebar
and mobile bottom nav switch tabs through `App.jsx`; Wallet and Send receive a
`visible` prop and refetch when they become active.

The Wallet tab shows total PFT as account balance plus indexed FastPay owned
objects when both are available. It fetches account state with `account`,
issued assets with `account_assets`, FastPay objects with `owned_objects`, and
recent history with `account_tx`. The FastPay tile can open the wrap dialog.

The Send tab has three lanes:

| Lane | Flow |
| --- | --- |
| Account PFT | Validate recipient and amount, encode optional memos, call `transfer_fee_quote`, show review, sign with WASM, submit, then poll `receipts`. Non-memo payments use v1 signing and finality submit; memo payments use payment v2. |
| FastPay | Load `owned_objects` and `owned_recovery_capabilities`, select inputs, derive the v3 lock/recovery window, sign the complete order with WASM, collect distinct votes through `owned_sign_v3`, assemble the quorum certificate, and submit it through `owned_apply_v3`. The wallet verifies a cryptographic quorum of authenticated durable apply acknowledgements before reporting finality; unknown/sub-quorum outcomes enter ordered recovery instead of being shown as success. |
| Issued asset | Build an `issued_payment` operation, quote with `asset_fee_quote`, sign with `wallet_sign_asset_transaction_fields`, submit with `mempool_submit_signed_asset_transaction`, then poll `receipts`. |

The Swap tab supports three route modes. The transparent route builds an issued
asset payment operation and sends it through the asset lane transaction builder.
The private route runs a 12-step companion-server flow through
`SwapServer.action()` and `SwapServer.getNav()` for bridge, shielded, and NAV
steps. The OTC route currently surfaces a quote request action in the UI.

## FastPay Funding

Funding moves account PFT into the FastPay reserve through an ordinary
consensus transaction; the unsafe unsigned wrapping RPC is not used. Both
Wallet and Send use the same source-signed flow:

1. Read `server_info`/`status` capabilities and the source account sequence.
2. Build a chain/genesis/protocol-bound PFT deposit with source address/public
   key, sequence, fee, destination owner key, expiry, and secure random nonce.
3. Sign locally with `wallet_sign_owned_deposit`.
4. Submit with `mempool_submit_fastlane_primary`.
5. Require the matching consensus receipt to be accepted with code
   `owned_deposit_applied`, then refresh `owned_objects`.

This prevents a caller from debiting an arbitrary named account, makes the
deposit deterministic across validators, and prevents a committed block with a
rejected receipt from appearing as funded.

## FastPay Address Activation

FastPay orders address recipients by their published ML-DSA public key. A fresh
account may have funds before its public key appears in account state. The
wallet normally resolves this automatically after first funding by signing a
one-atom self-transfer: the atom returns to the same account, only the quoted
fee is burned, and the wallet requires an accepted receipt before reporting the
key as published. The Wallet tab exposes a retry action if that automatic step
was interrupted. No user is asked to paste private material; a sender may use a
recipient's full public key directly only as an explicit alternative to address
lookup.

Owned-object snapshots and the live wallet feed request up to `2048` objects,
matching the protocol input cap for standard unwrap. This prevents a
fragmented wallet from hiding spendable FastPay value behind a smaller
client-side lookup limit.

## FastPay Standard Unwrap

Default unwrap is amount-based and certified. The wallet no longer unwraps a
whole object through `unwrap_owned(object_id, owner_pubkey_hex, to_address)`.
That RPC shape does not prove ownership of the private key and is disabled for
public wallet flows.

The current unwrap flow is:

1. Load the FastPay object snapshot with `owned_objects`.
2. Select one or more PFT objects that cover `amount + fee`, up to `2048`
   inputs. Exact match and smallest-covering-object are preferred before
   largest-first multi-input selection.
3. Build an `OwnedUnwrapOrder` containing input refs, account destination,
   requested amount, asset, fee, nonce, and memos.
4. Sign the order with `wallet_sign_owned_unwrap` in WASM.
5. Load v3 recovery capabilities, derive the lock ID/window, collect quorum
   votes with `owned_unwrap_sign_v3`, assemble the certificate, and submit it
   with `owned_unwrap_apply_v3`.
6. Verify a cryptographic quorum of durable apply acknowledgements. Certified
   apply credits exactly the requested amount to the account lane and returns
   any remainder as one FastPay change object.

On a six-validator committee, product finality requires the configured BFT
quorum (`5/6`) of valid acknowledgements; exact-six replication is an audit and
repair target, not a reason to weaken the quorum proof.

## Memo Field Support

The account PFT lane has a collapsed `Memo (optional)` section under the
recipient field with three optional text fields:

| Field | Limit |
| --- | --- |
| Memo Type | 64 UTF-8 bytes |
| Memo Format | 64 UTF-8 bytes |
| Memo Data | 256 UTF-8 bytes |

Total memo bytes must be at most 512. The web wallet currently creates one
memo entry from those fields.

The form accepts normal strings. Before quote/sign, `tx-builder.js` UTF-8
encodes each string and converts the bytes to lower hex. Empty memo fields
encode to the empty string.

`TxBuilder.sendTransfer()` preserves the v1 path when all memo fields are
empty:

1. Quote with `transfer_fee_quote(from, to, amount)`.
2. Sign with `wallet_sign_transfer(backup_json, quote_json)`.
3. Submit with `mempool_submit_signed_transfer_finality`.
4. Fall back to `mempool_submit_signed_transfer` if finality submit is
   unavailable.
5. Poll `receipts` unless inline finality evidence already contains the
   accepted receipt.

If any memo field is non-empty, the builder switches to payment v2:

1. Encode memo strings to lower hex.
2. Quote with `transfer_fee_quote(from, to, amount, { memo_type, memo_format, memo_data })`.
3. Sign with `wallet_sign_payment_v2(backup_json, fields_json)`.
4. Submit with `mempool_submit_signed_payment_v2` and param
   `signed_payment_v2_json`.
5. Poll `receipts`.

The payment v2 signing JSON uses chain fields from the quote plus a memo array:

```json
{
  "chain_id": "postfiat-wan-devnet",
  "genesis_hash": "...",
  "protocol_version": 1,
  "to": "pf...",
  "amount": 1000,
  "fee": 22,
  "sequence": 95,
  "memos": [
    {
      "memo_type": "",
      "memo_format": "",
      "memo_data": "746573742d6d656d6f2d6768617368"
    }
  ]
}
```
