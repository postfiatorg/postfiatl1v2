# PostFiat Web Wallet — Spec

## Objective

A browser-based self-custody wallet for PostFiat L1, built as a Vite + React web app. All key material is generated, encrypted, and signed entirely in the browser via WASM. The server never sees seeds, passphrases, or private keys. The operator is not a money transmitter.

This replaces the Chrome extension wallet for the swap tool use case. The same WASM module, the same RPC proxy, and the same chain parameters are reused. The only thing that changes is the shell: a web page instead of an extension popup.

## Money Transmitter Boundary (Hard Constraint)

The operator must never:
- receive, store, log, or transmit a user's master seed, passphrase, or decrypted backup
- hold a signable transaction and a private key at the same time
- submit transactions on behalf of a user without the user's browser having signed them

The web app satisfies this by design:
- Keygen, encryption, and signing happen in WASM in the browser
- The encrypted vault is stored in IndexedDB (browser-local, per-origin)
- The server only provides static assets (HTML, JS, WASM) and proxies RPC
- The server never participates in the signing flow

## Chain Parameters (Live Testnet)

| Parameter | Value |
|---|---|
| chain_id | `postfiat-wan-devnet` |
| genesis_hash | `231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870` |
| protocol_version | `1` |
| validator_count | 6 |
| RPC endpoint (validator-0) | `192.0.2.10:27650` (raw TCP, JSON line protocol) |
| account_reserve | 10 |
| minimum_transfer_fee | 1 |
| transfer_account_creation_fee | 10 |
| transfer_fee_byte_quantum | 512 |
| transfer_fee_per_quantum | 1 |

## Cryptographic Parameters

| Parameter | Value |
|---|---|
| Signature algorithm | ML-DSA-65 (FIPS 204, category 3) |
| Public key size | 1,952 bytes (3,904 hex chars) |
| Private key size | 4,032 bytes (8,064 hex chars) |
| Signature size | 3,309 bytes (6,618 hex chars) |
| Key derivation | SHA3-384 domain-separated, truncate to 32 bytes |
| Derivation domain | `postfiat.wallet.seed.v1` |
| Address scheme | `pf` + SHA3-384(`postfiat.address.v1` \x00 public_key)[0:20] |
| Address length | 42 chars (`pf` + 40 hex chars) |
| Signing context | `postfiat-l1-v2/tx/v1` (embedded in `ml_dsa_65_sign`) |

## Vault Encryption Parameters

| Parameter | Value |
|---|---|
| Encryption algorithm | AES-256-GCM (Web Crypto API) |
| Key derivation | PBKDF2, SHA-256, 310,000 iterations |
| Salt | 16 bytes, random per encryption |
| IV | 12 bytes, random per encryption |
| Vault storage | IndexedDB (primary), with localStorage fallback |
| Passphrase minimum length | 10 characters |
| Auto-lock default | 15 minutes (configurable 5/15/30/60) |

## RPC Wire Protocol

Transport: raw TCP, one JSON request per line, one JSON response per line. The browser connects via WebSocket to the proxy, which bridges to TCP.

Request format:
```json
{"version":"postfiat-local-rpc-v1","id":"<string>","method":"<method>","params":{<key>:<value>}}
```

Response format:
```json
{"version":"postfiat-local-rpc-v1","id":"<string>","ok":true,"result":{...},"error":null,"events":[...]}
```

Constraint: `id` must be a string. `params` must be a flat JSON object. Max request size 8 MB.

### RPC Methods Used

| Method | Purpose | Key params |
|---|---|---|
| `status` | Chain height, state root, validator count | none |
| `fee` | Fee policy | none |
| `account` | Account balance, sequence, public key | `address` |
| `account_tx` | Transaction history | `address`, `from_height?`, `to_height?`, `limit?` |
| `owned_objects` | FastPay owned-object balance and object inventory | `owner_public_key_hex`, `asset?`, `limit?` |
| `owned_sign` | Validator vote after owner-auth and live-state admission | `order_json` containing `order` + `owner_pubkey_hex` + `owner_signature_hex`, `validator_id` |
| `owned_apply` | Apply a quorum-certified FastPay owned-transfer certificate | `cert_json` |
| `owned_unwrap_sign` | Validator vote for a signed FastPay unwrap order | `order_json`, `validator_id` |
| `owned_unwrap_apply` | Apply a quorum-certified FastPay unwrap certificate | `cert_json` |
| `wrap_owned` | Wrap account PFT into a FastPay owned object | `from_address`, `owner_pubkey_hex`, `amount`, `asset` |
| `unwrap_owned` | Disabled compatibility path; do not use from public wallet flows | `object_id`, `owner_pubkey_hex`, `to_address` |
| `transfer_fee_quote` | Quote a native PFT transfer or memo payment: fee, sequence, validity | `from`, `to`, `amount`, `sequence?`, `memo_type?`, `memo_format?`, `memo_data?` |
| `asset_fee_quote` | Quote an asset transaction | `source`, `operation_json` |
| `asset_info` | Asset metadata | `asset_id` |
| `account_assets` | Asset balances for an account | `address` |
| `account_lines` | Trust lines for an account | `address` |
| `issuer_assets` | Assets issued by an address | `issuer` |
| `offer_fee_quote` | Quote an offer transaction | `source`, `operation_json` |
| `offer_info` | Offer details | `offer_id` |
| `account_offers` | Offers for an account | `address` |
| `book_offers` | Order book for a pair | `pays_asset`, `gets_asset` |
| `mempool_submit_signed_transfer` | Submit signed transfer | `signed_transfer_json` |
| `mempool_submit_signed_payment_v2` | Submit signed native PFT payment with memos | `signed_payment_v2_json` |
| `mempool_submit_signed_asset_transaction` | Submit signed asset tx | `signed_asset_json` |
| `mempool_submit_signed_offer_transaction` | Submit signed offer tx | `signed_offer_json` |
| `receipts` | Transaction receipt | `tx_id?`, `limit?` |
| `blocks` | Recent blocks | `from_height?`, `limit?` |
| `validators` | Validator registry | none |

## Architecture

```
┌──────────────────────────────────────────────────┐
│              Web App (Vite + React)               │
│                                                   │
│  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  UI Layer    │  │  WASM Module              │  │
│  │  (React +    │  │  (postfiat-wallet-wasm)    │  │
│  │   Vite)      │  │                           │  │
│  │              │  │  wallet_keygen()          │  │
│  │  - Onboard   │  │  wallet_address()         │  │
│  │  - Balance   │  │  wallet_sign_transfer()   │  │
│  │  - Send PFT  │  │  wallet_sign_payment_v2()  │  │
│  │  - Send Asset│  │  wallet_sign_asset_tx()   │  │
│  │  - Offers    │  │  wallet_sign_offer_tx()   │  │
│  │  - History   │  │  make_rpc_request()       │  │
│  │  - Settings  │  │  parse_rpc_response()     │  │
│  └──────────────┘  └──────────────────────────┘  │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │  Storage (IndexedDB)                         │ │
│  │  - encrypted_vault (AES-256-GCM)             │ │
│  │  - wallet_metadata (address, public_key)     │ │
│  │  - tx_history_cache                          │ │
│  │  - settings (rpc_endpoint, auto_lock_min)    │ │
│  └─────────────────────────────────────────────┘ │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │  Transport (WebSocket → proxy)               │ │
│  └─────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
          │
          │ WebSocket (ws:// or wss://)
          ▼
┌─────────────────────────┐
│  WebSocket Proxy         │
│  (Node.js, existing)     │
│  ws → raw TCP JSON-RPC   │
│  Origin-allowed only     │
│  maxPayload: 1MB         │
└─────────────────────────┘
          │
          │ TCP
          ▼
┌─────────────────────┐
│  Validator RPC       │
│  192.0.2.10:27650 │
└─────────────────────┘
```

## Component Specs

### Component 1: WASM Wallet Core (existing, extend)

The existing `crates/wallet_wasm/` module already exports:
- `wallet_keygen(chain_id, master_seed_hex, account_index)` → `{ address, public_key_hex, backup_json }`
- `wallet_address_from_seed(chain_id, master_seed_hex, account_index)` → address string
- `wallet_sign_transfer(backup_json, quote_json)` → SignedTransfer
- `wallet_sign_transfer_fields(backup_json, fields_json)` → SignedTransfer
- `wallet_sign_payment_v2(backup_json, fields_json)` → SignedPaymentV2
- `make_rpc_request(method, params_json)` → JSON string
- `parse_rpc_response(response_json)` → { ok, result, error }
- `random_master_seed()` → 64-char hex string

**New WASM exports needed for asset/offer signing:**
- `wallet_sign_asset_transaction(backup_json, quote_json)` → SignedAssetTransaction
  Wraps `wallet_sign_asset_transaction_from_quote`
- `wallet_sign_asset_transaction_fields(backup_json, fields_json)` → SignedAssetTransaction
  Wraps `wallet_sign_asset_transaction_from_fields`
- `wallet_sign_offer_transaction(backup_json, quote_json)` → SignedOfferTransaction
  Wraps `wallet_sign_offer_transaction_from_quote`
- `wallet_sign_offer_transaction_fields(backup_json, fields_json)` → SignedOfferTransaction
  Wraps `wallet_sign_offer_transaction_from_fields`

### Component 2: WebSocket Proxy (existing, reuse)

The existing `wallet-proxy/server.js` is reused as-is. It already has:
- WebSocket-to-TCP bridge with per-message TCP connections
- Origin allowlist checking
- 1MB maxPayload limit
- JSON validation before forwarding
- TCP connection cleanup with timeout and concurrency limits
- Generic error messages (no internal leak)

### Component 3: Vite + React Web App (new)

**Directory structure:**
```
postfiatl1v2/
├── wallet-web/
│   ├── index.html
│   ├── package.json
│   ├── vite.config.js
│   ├── src/
│   │   ├── main.jsx
│   │   ├── App.jsx
│   │   ├── components/
│   │   │   ├── Onboard.jsx
│   │   │   ├── WalletView.jsx
│   │   │   ├── SendTransfer.jsx
│   │   │   ├── SendAsset.jsx
│   │   │   ├── OfferBook.jsx
│   │   │   ├── TransactionHistory.jsx
│   │   │   ├── Settings.jsx
│   │   │   └── LockScreen.jsx
│   │   ├── lib/
│   │   │   ├── vault.js          // IndexedDB encrypted vault
│   │   │   ├── rpc-client.js      // WebSocket RPC client
│   │   │   ├── wasm-loader.js     // WASM module loader
│   │   │   ├── tx-builder.js      // Quote → sign → submit → poll
│   │   │   └── utils.js           // Address validation, formatting
│   │   ├── styles/
│   │   │   └── main.css
│   │   └── wasm/
│   │       ├── postfiat_wallet_wasm.js
│   │       └── postfiat_wallet_wasm_bg.wasm
│   └── public/
│       └── favicon.ico
```

### Component 4: Encrypted Vault (`src/lib/vault.js`)

Browser-side encrypted storage using IndexedDB + Web Crypto API.

**Encryption:**
- Master seed encrypted with AES-256-GCM via Web Crypto
- Key derived from passphrase via PBKDF2 (SHA-256, 310,000 iterations, 16-byte salt)
- Vault blob: `{ salt: base64, iv: base64, ciphertext: base64 }`

**IndexedDB schema:**
- Database: `postfiat-wallet`
- Object store: `vaults` (keyPath: `accountId`)
- Records: `{ accountId, vault, metadata, updatedAt }`

**In-memory state:**
- Decrypted master seed held in module-scope variable (not window global)
- Cleared on lock, auto-lock, or page unload
- Never written to disk unencrypted

**Functions:**
- `encryptVault(masterSeedHex, passphrase)` → blob
- `decryptVault(blob, passphrase)` → masterSeedHex
- `saveVault(accountId, blob, metadata)` → void
- `loadVault(accountId)` → { blob, metadata } | null
- `removeVault(accountId)` → void
- `saveSettings(settings)` → void
- `loadSettings()` → settings

### Component 5: RPC Client (`src/lib/rpc-client.js`)

WebSocket transport to the proxy. Reuse the extension's `rpc-client.js` logic, adapted for web context (no `chrome.storage`).

**Functions:**
- `connect()` → Promise
- `call(method, params, timeoutMs)` → Promise<RpcResponse>
- Convenience: `status()`, `fee()`, `account(addr)`, `accountTx(addr, opts)`, `transferFeeQuote(from, to, amt, sequenceOrOptions)`, `assetFeeQuote(source, opJson)`, `accountAssets(addr)`, `offerFeeQuote(source, opJson)`, `bookOffers(pays, gets)`, `submitSignedTransfer(json)`, `submitSignedTransferFinality(json)`, `submitSignedPaymentV2(json)`, `submitSignedAssetTransaction(json)`, `submitSignedOfferTransaction(json)`, `receipts(txId)`, `close()`

**Balance parsing and error handling shipped in v0.1.2:**
- `account` responses are parsed defensively. The client accepts both the current flat validator response (`result.balance`) and a nested account envelope (`result.account.balance`).
- A genuine on-chain zero is displayed only when the RPC response is `ok: true` and the parsed balance is exactly `0`.
- RPC transport failures, `ok: false` responses, malformed account envelopes, and missing balances are surfaced as UI error banners. They must not be coerced to `0 PFT`.
- Wallet and Send refetch account and FastPay state when their tab becomes active. They also refresh when the browser tab returns to foreground.
- The WebSocket heartbeat is a keepalive `status` request. Heartbeat responses are not registered in the pending-request map; the proxy opens a separate TCP connection per WebSocket message, so heartbeat traffic does not share a line stream with account queries.

**FastPay wrap refresh shipped in v0.1.2:**
- After a successful `wrap_owned`, the UI records the pre-wrap `owned_objects` `total_value`.
- The UI polls `owned_objects(owner_public_key_hex, { asset: "PFT", limit: 2048 })` every 500 ms until the visible total is at least `pre_wrap_total + wrapped_amount`, or until 10 seconds elapse.
- While polling, the UI shows a FastPay refresh indicator. If polling times out or fails after the wrap succeeds, the wrap success remains distinct from the refresh error.
- A single immediate FastPay refresh is not sufficient because `wrap_owned` can return before the validator read path has indexed the newly created owned object.

**Standard FastPay unwrap shipped in v0.1.2:**
- The wallet no longer calls public `unwrap_owned` for default unwraps.
- The user enters an amount. `TxBuilder.unwrapOwnedTransfer()` selects one or more FastPay owned objects, up to the 2048 input cap, signs an `OwnedUnwrapOrder` with the wallet WASM key, collects validator votes with `owned_unwrap_sign`, and applies the certificate with `owned_unwrap_apply`.
- Certified apply credits exactly the requested amount to the account lane and returns any remainder as one FastPay change object owned by the same wallet.
- The live wallet feed subscribes with `owned_limit: 2048`, so fragmented FastPay wallets can update account and FastPay balances without a manual refresh.

### Component 6: Transaction Builder (`src/lib/tx-builder.js`)

Orchestrates the full send flow:
1. Get fee quote from RPC
2. Validate quote (balance sufficient, sequence correct, fee > 0)
3. Call WASM to sign
4. Submit to mempool
5. Poll for receipt (2s interval, 30s timeout)
6. Return finality result
7. Update local tx history cache

**Native PFT memo payments:**
- `sendTransfer(backupJson, fromAddress, toAddress, amount, memos?)` accepts optional `{ memo_type, memo_format, memo_data }` strings from the form.
- If all three memo fields are empty or omitted, the native v1 flow is unchanged: quote with `transfer_fee_quote(from, to, amount)`, sign with `wallet_sign_transfer(backup_json, quote_json)`, submit with `mempool_submit_signed_transfer_finality` and fall back to `mempool_submit_signed_transfer` if finality submit is unavailable.
- If any memo field is non-empty, the builder uses the payment v2 flow: UTF-8 encode each memo string to lower hex, quote with `transfer_fee_quote` including `memo_type`, `memo_format`, and `memo_data`, sign with `wallet_sign_payment_v2(backup_json, fields_json)`, submit with `mempool_submit_signed_payment_v2`, then poll receipts.
- The WASM `WalletSignPaymentV2Fields` JSON uses `memos: [{ memo_type, memo_format, memo_data }]`, not top-level memo fields. The RPC quote still receives the flat memo params.
- Memo byte limits are enforced before quote/sign: `memo_type` <= 64 bytes, `memo_format` <= 64 bytes, `memo_data` <= 256 bytes, total memo bytes <= 512. The chain currently accepts one memo entry from the web form.

## UI Screens

### 1. Onboarding (Onboard.jsx)

- "Create New Wallet" button
  - Generate master seed via WASM `random_master_seed()`
  - Call WASM `wallet_keygen(chain_id, seed, 0)`
  - Display address + 64-char hex seed with "SAVE THIS" warning
  - Require user to check "I have saved my seed"
  - Prompt for encryption passphrase (min 10 chars)
  - Encrypt seed, store vault in IndexedDB
  - Clear plaintext seed from memory
- "Import Wallet" button
  - Accept 64-char hex master seed paste
  - Validate: 64 hex chars
  - Derive address via WASM
  - Display address for confirmation
  - Encrypt and store same as create

### 2. Lock Screen (LockScreen.jsx)

- Shown when wallet exists but is locked
- Display wallet address (from metadata, no decryption needed)
- Passphrase input
- Unlock button
- Wrong passphrase: clear error, no crash

### 3. Wallet View (WalletView.jsx)

- Address (with copy-to-clipboard)
- Balance (PFT, raw + human-readable)
- Sequence number
- Chain status indicator (online/offline, block height)
- Asset balances section (pfUSDC, a651, other issued assets)
- Navigation: Send PFT | Send Asset | Swap | Offers | History | Settings | Lock

### 4. Send Transfer (SendTransfer.jsx)

- Recipient address input (validate: `pf` prefix, 42 chars, hex after `pf`)
- Amount input (positive integer)
- Collapsed "Memo (optional)" section for account-lane native PFT sends with text inputs: Memo Type, Memo Format, Memo Data
- "Get Quote" button → RPC `transfer_fee_quote`
- Quote display: fee, total, sequence, balance after, recipient exists
- "Confirm & Sign" button → no memos: WASM `wallet_sign_transfer` → RPC `mempool_submit_signed_transfer_finality` / `mempool_submit_signed_transfer`; any memo: WASM `wallet_sign_payment_v2` → RPC `mempool_submit_signed_payment_v2`
- Poll for receipt → display accepted/rejected
- Rate limit: min 3s between sign attempts

### 5. Send Asset (SendAsset.jsx)

- Asset selector (from `account_assets` RPC)
- Recipient address input
- Amount input
- "Get Quote" → RPC `asset_fee_quote`
- Quote display: fee, sequence, balance after
- "Confirm & Sign" → WASM `wallet_sign_asset_transaction` → RPC `mempool_submit_signed_asset_transaction`
- Poll for receipt

### 6. Offer Book (OfferBook.jsx)

- View order book: `book_offers` for a selected pair
- View my offers: `account_offers`
- Create offer: asset pair, amount, price → `offer_fee_quote` → `wallet_sign_offer_transaction` → submit
- Cancel offer: select existing offer → `offer_fee_quote` (cancel) → sign → submit

### 7. Transaction History (TransactionHistory.jsx)

- RPC `account_tx` with pagination (limit 50, load more)
- Display: block height, direction, counterparty, amount, fee, finality
- Click a transaction for full detail

### 8. Settings (Settings.jsx)

- RPC endpoint selector (dropdown: WAN devnet, local, custom)
- Auto-lock timer (5/15/30/60 minutes)
- Export backup (download encrypted JSON)
- Import backup (upload JSON, validate, restore)
- Remove wallet (confirm, clear all IndexedDB)

## Swap Flows

This web app is the user-facing surface for two swap modes that PostFiat L1 supports. Both modes are crypto-only: no GitHub login, no OAuth, no email, no account system. The user creates a PFTL wallet in the browser and interacts directly with the chain.

### Transparent Swap (No Orchard)

The transparent swap is a public PFTL-only roundtrip with no shielded/Orchard layer. All transactions are visible on-chain.

**Flow:**
```
User wallet (pfUSDC balance)
  → transparent asset transfer: pfUSDC → a651 (issuer payment)
  → user holds public a651
  → transparent asset transfer: a651 → pfUSDC (issuer redemption)
  → user holds public pfUSDC again
```

**Chain operations used:**
1. `asset_fee_quote` — quote the fee for an `issued_payment` asset transaction
2. `wallet_sign_asset_transaction` — sign the issued payment in the browser (WASM)
3. `mempool_submit_signed_asset_transaction` — submit to chain
4. Poll `receipts` for finality

**What is public:** sender, recipient, asset id, amount, fee, sequence, everything.

**What the web app does:**
- Shows the user's pfUSDC and a651 balances (via `account` + `account_assets` RPC)
- User selects "Swap pfUSDC → a651", enters amount
- App gets `asset_fee_quote` for an `issued_payment` operation from user → issuer
- App displays quote (fee, sequence, balance after)
- User confirms → WASM signs asset transaction → app submits → polls for receipt
- Balance updates

**Trust model:** The issuer (NAVCoin operator) sets the exchange rate. The chain enforces that the issued payment debits the sender and credits the recipient. The rate is off-chain policy — the user trusts the issuer to honor mint/redeem at the quoted rate. The chain does not enforce a price; it enforces conservation.

### Private Swap (Asset-Orchard Shielded)

The private swap uses the Asset-Orchard shielded layer. The swap middle is private: asset ids, amounts, counterparties, and bilateral price are hidden from public observers. Ingress and egress edges are visible.

**Flow (based on StakeHub's 12-step shielded NAV swap):**
```
Step 0:  Create PFTL wallet (browser-side, WASM keygen)
Step 1:  Fund PFTL gas (faucet or existing balance)
Step 2:  NAV snapshot before (read-only)
Step 3:  Warm shielded wallet (load proving keys)
Step 4:  Confirm pfUSDC balance (bridge-in or existing)
Step 5:  Bridge USDC → pfUSDC (EVM deposit → PFTL bridge relay → claim)
Step 6:  Shield pfUSDC into Orchard (public ingress: burn pfUSDC → private note)
Step 7:  Private swap pfUSDC → a651 (Orchard Halo2 circuit, private middle)
Step 8:  Certify and finalize (poll for block finality)
Step 9:  Private egress to public a651 (proof-verified exit, note opening hidden)
Step 10: Redeem and withdraw (a651 → pfUSDC → USDC bridge-out, public exit)
Step 11: NAV snapshot after (read-only)
```

**Privacy boundaries:**
- **Public:** wallet address, pfUSDC balance, bridge deposit/relay, ingress (asset + amount shielded), egress destination + amount, NAV reserves
- **Private (hidden in the swap action):** raw note owner, recipient, asset ids, amounts, bilateral price, note openings
- **Private egress (direct v1):** hides the spent shielded note opening (nk, rivk, rho, psi, rcm, output_commitment, spend randomizer, signing key). Reveals public exit destination, asset, amount, fee, nullifier, anchor, proof material.

**Chain operations used:**
1. `status` — chain height for anchor references
2. `account` / `account_assets` — balance checks
3. `shield_batch_orchard` / `shield_batch_orchard_deposit` / `shield_batch_swap` — Orchard batch operations (currently server-side, require proving key)
4. `mempool_submit_signed_asset_transaction` — for the transparent bridge/egress legs
5. `receipts` — poll for finality

**What the web app does for the private swap:**

The private swap's ZK proof generation (Halo2 circuit, K=15 proving key) is CPU-intensive (~6 seconds hot path, ~346 seconds cold path). This cannot run in the browser WASM. The proving must happen server-side or in a local prover daemon.

**Architecture split:**
- **Browser (this web app):** wallet custody, transparent transaction signing (pfUSDC transfers, a651 transfers, bridge-out asset transactions), balance display, receipt polling, UX
- **Server (separate, not part of this web app):** proving key management, Orchard batch creation, proof generation, certified round submission

**The web app's role in the private swap:**
1. User creates/unlocks wallet (browser-side, same as transparent flow)
2. User bridges USDC → pfUSDC (signs the EVM-side deposit; PFTL bridge relay is server-side)
3. User shields pfUSDC into Orchard (signs the public ingress burn transaction in browser; Orchard batch certification is server-side)
4. Server runs the private swap proof (warm prover, K=15 key) — browser polls for status
5. Server runs private egress proof — browser polls for status
6. User signs the public exit asset transactions (bridge-out, redemption) in the browser
7. Browser displays before/after balances and receipt evidence

**Key insight:** The browser always signs transparent transactions (transfers, asset payments, bridge operations). The server handles the ZK proof generation for Orchard shielded actions because Halo2 proving is not WASM-feasible. The server never sees the user's seed — it only sees public Orchard action parameters and produces proofs.

### Swap UI Components

The web app needs these additional screens beyond the basic wallet:

**SwapView.jsx** — Swap entry point
- Mode toggle: "Transparent" vs "Private (Shielded)"
- Transparent mode: asset selector, amount, recipient (or issuer auto-fill), quote, sign, submit
- Private mode: displays the 12-step rail with current step status, delegates proof-heavy steps to server API

**BridgeView.jsx** — Bridge USDC ↔ pfUSDC
- Bridge in: EVM chain selector, USDC amount, deposit address, relay status, claim pfUSDC
- Bridge out: pfUSDC amount, EVM destination, burn, withdrawal proof, EVM claim
- All PFTL-side transactions signed in browser; EVM-side requires separate EVM wallet (MetaMask/wallet connect — future)

**NavSnapshot.jsx** — NAV/proof-of-reserves display
- Read-only display of a651 NAV, verified_net_assets, nav_floor, supply
- Before/after comparison for swap flows
- Data from server API (aggregates chain state)

### Server API for Private Swap (Separate from Web App)

The web app needs a companion server API for the proof-heavy steps. This is NOT part of the web app build — it's a separate service (like StakeHub's dashboard_server). The web app calls it via HTTP.

**Endpoints (reference, based on StakeHub):**
```
GET  /api/swap/status                    — current run state
GET  /api/swap/balances                  — pfUSDC, a651, PFTL balances
GET  /api/swap/nav?phase=before|after    — NAV snapshot
POST /api/swap/action                    — execute a swap step
  Actions: shield_ingress, shield_swap, private_egress, bridge_in, bridge_out
```

**The server never receives the user's seed.** It receives:
- The user's PFTL address (public)
- Public Orchard action parameters (asset, amount, pool_id — the ingress boundary)
- Signed transactions from the browser (for transparent legs)

The server produces:
- ZK proofs for Orchard shielded actions
- Certified round submissions
- Status updates and receipts

This separation is what keeps the operator out of money transmitter territory: the server proves and submits, but only the browser can sign.

## Security Requirements

1. **Private keys never leave the browser.** All signing in WASM. No remote signing.
2. **Master seed encrypted at rest.** AES-256-GCM with PBKDF2-derived key (310k iterations).
3. **Passphrase never stored, logged, or sent over network.**
4. **No `eval`, `Function()`, or `setTimeout(string)`.** CSP: `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'`.
5. **No third-party scripts on wallet pages.** No analytics, chat widgets, or CDNs that could exfiltrate seeds.
6. **Phishing protection.** Full recipient address shown on confirmation. No truncation.
7. **Rate limiting.** Min 3s between sign attempts.
8. **Auto-lock.** Clear decrypted seed after configurable timeout.
9. **XSS prevention.** All RPC-derived text escaped before rendering. React's JSX escaping is sufficient by default; no `dangerouslySetInnerHTML`.
10. **Seed not in window globals.** Module-scope `let` only, not `window._seed`.
11. **Seed cleared on lock.** Lock nulls the in-memory seed variable.
12. **Seed cleared on page unload.** `beforeunload` handler nulls sensitive variables.
13. **WebSocket URL validation.** Only `ws://` and `wss://` schemes allowed.

## Differences from the Chrome Extension

| Aspect | Chrome Extension | Web App |
|--------|-----------------|---------|
| Storage | `chrome.storage.local` (extension-sandboxed) | IndexedDB (per-origin) |
| UI shell | 360px popup | Full-page React app |
| Background | Service worker (MV3) | None (tab-scoped) |
| WASM loading | Extension-packaged | Served from same origin |
| Key isolation | Extension sandbox | Origin isolation (CORS, CSP) |
| Cross-app signing | `externally_connectable` | Not supported (different origins) |
| Background WS | Service worker holds connection | Tab must be open |

## Gates and Completion Criteria

### Gate 0: Project Scaffold and WASM Integration

**Objective:** Vite + React app loads, WASM module initializes, WASM functions callable from browser console.

- [ ] G0.1: Create `wallet-web/` directory with `package.json`, `vite.config.js`, `index.html`
- [ ] G0.2: `npm install` succeeds with React, Vite, and no third-party runtime deps
- [ ] G0.3: Copy WASM package from `wallet-extension/wasm/` into `wallet-web/src/wasm/`
- [ ] G0.4: Create `src/lib/wasm-loader.js` that dynamically imports the WASM module
- [ ] G0.5: `npm run dev` starts Vite dev server on localhost:5173 without errors
- [ ] G0.6: Open browser, call `wallet_keygen('postfiat-wan-devnet', 'a'.repeat(64), 0)` from console, verify returns object with `address` starting with `pf` and 42 chars
- [ ] G0.7: Call `random_master_seed()` from console, verify returns 64-char hex string
- [ ] G0.8: Call `wallet_sign_transfer_fields(backupJson, fieldsJson)` from console with known seed, verify `signature_hex` is 6618 chars and `public_key_hex` is 3904 chars
- [ ] G0.9: Verify WASM `.wasm` file is served with `application/wasm` MIME type
- [ ] G0.10: Verify CSP headers block inline scripts (set via Vite config or meta tag)

**Gate 0 PASS criteria:** Dev server runs, WASM loads and executes keygen + signing in the browser, no console errors.

### Gate 1: Encrypted Vault (IndexedDB)

**Objective:** Master seed can be encrypted, stored in IndexedDB, and decrypted back correctly.

- [ ] G1.1: Create `src/lib/vault.js` with `encryptVault(seedHex, passphrase)` using Web Crypto PBKDF2 (310k iterations, SHA-256, 16-byte salt) + AES-256-GCM
- [ ] G1.2: `encryptVault` returns blob `{ salt: base64, iv: base64, ciphertext: base64 }`
- [ ] G1.3: Create `decryptVault(blob, passphrase)` that reverses encryption
- [ ] G1.4: Round-trip test: encrypt a known seed, decrypt it, verify output matches input
- [ ] G1.5: Decrypt with wrong passphrase throws error (no crash, no partial output)
- [ ] G1.6: Decrypt with corrupt blob (missing field) throws clear error
- [ ] G1.7: Create IndexedDB database `postfiat-wallet` with object store `vaults` (keyPath: `accountId`)
- [ ] G1.8: `saveVault(accountId, blob, metadata)` writes to IndexedDB
- [ ] G1.9: `loadVault(accountId)` reads from IndexedDB, returns `{ blob, metadata }` or `null`
- [ ] G1.10: `removeVault(accountId)` deletes from IndexedDB
- [ ] G1.11: `saveSettings(settings)` and `loadSettings()` persist to IndexedDB
- [ ] G1.12: Verify no plaintext seed appears in IndexedDB at any point (inspect with DevTools → Application → IndexedDB)
- [ ] G1.13: Verify passphrase is never stored in IndexedDB, localStorage, or sessionStorage

**Gate 1 PASS criteria:** Encryption round-trips, wrong passphrase fails gracefully, vault persists across page reloads, no plaintext seed in storage.

### Gate 2: RPC Client and Chain Connectivity

**Objective:** Web app connects to the proxy and reaches the live testnet.

- [ ] G2.1: Create `src/lib/rpc-client.js` with WebSocket connection logic
- [ ] G2.2: URL validation: reject non-`ws://`/`wss://` schemes
- [ ] G2.3: `connect()` opens WebSocket to `ws://192.0.2.20:8080`
- [ ] G2.4: `call('status', {})` returns response with `ok: true` and `block_height` > 0
- [ ] G2.5: `call('fee', {})` returns fee policy with `account_reserve` and `minimum_transfer_fee`
- [ ] G2.6: `account('pf...')` returns balance and sequence for a known funded address
- [ ] G2.7: `account('pf' + '0'.repeat(40))` returns "account not found" without crash
- [ ] G2.8: Request timeout: call with 1ms timeout, verify rejects with timeout error
- [ ] G2.9: Connection close: close WebSocket mid-pending-request, verify pending rejects
- [ ] G2.10: Reconnect: after close, next `call()` auto-reconnects and succeeds
- [ ] G2.11: `accountTx(addr, { limit: 5 })` returns transaction list

**Gate 2 PASS criteria:** All RPC methods work against live testnet through the proxy, errors handled gracefully, reconnection works.

### Gate 3: Wallet Onboarding (Create + Import)

**Objective:** User can create a new wallet or import an existing seed, with encrypted storage.

- [ ] G3.1: `Onboard.jsx` renders "Create Wallet" and "Import Wallet" buttons when no vault exists
- [ ] G3.2: Create flow: click "Create Wallet" → generate seed via WASM `random_master_seed()`
- [ ] G3.3: Call WASM `wallet_keygen` → display address + seed with "SAVE THIS SEED" warning
- [ ] G3.4: Require user to check "I have saved my seed" checkbox before proceeding
- [ ] G3.5: Passphrase input (min 10 chars), passphrase confirmation input
- [ ] G3.6: Passphrase mismatch shows error, blocks save
- [ ] G3.7: On confirm: encrypt seed via `vault.encryptVault()`, save to IndexedDB
- [ ] G3.8: Clear plaintext seed from memory after save (variable set to null)
- [ ] G3.9: Clear passphrase input fields after save
- [ ] G3.10: Import flow: paste 64-char hex seed, validate format (64 hex chars)
- [ ] G3.11: Invalid seed (wrong length, non-hex) shows error, blocks save
- [ ] G3.12: Import derives address via WASM, displays for confirmation
- [ ] G3.13: Import encrypts and stores same as create flow
- [ ] G3.14: After create/import, app navigates to wallet view
- [ ] G3.15: Reload page: app detects existing vault, shows lock screen (not onboarding)
- [ ] G3.16: Rapid double-click on "Create Wallet" does not create duplicate vaults (guard check)

**Gate 3 PASS criteria:** Create and import both work, seed is encrypted at rest, page reload shows lock screen, no duplicate wallets.

### Gate 4: Unlock, Lock, and Auto-Lock

**Objective:** Wallet can be unlocked with passphrase, locked manually, and auto-locks on timeout.

- [ ] G4.1: `LockScreen.jsx` shows wallet address (from metadata, no decryption) and passphrase input
- [ ] G4.2: Correct passphrase: decrypts vault, loads seed into memory, navigates to wallet view
- [ ] G4.3: Wrong passphrase: shows "incorrect passphrase" error, no crash
- [ ] G4.4: Empty passphrase: shows error, does not attempt decrypt
- [ ] G4.5: Passphrase field cleared after successful unlock
- [ ] G4.6: "Lock" button: nulls in-memory seed, shows lock screen
- [ ] G4.7: After lock, seed variable is null (verify via debug getter or state check)
- [ ] G4.8: Auto-lock timer fires after configured timeout, nulls seed, shows lock screen
- [ ] G4.9: Auto-lock timer resets on user activity (click, keypress)
- [ ] G4.10: Auto-lock timeout configurable (5/15/30/60 minutes)
- [ ] G4.11: `beforeunload` handler nulls seed on page close
- [ ] G4.12: Seed is not in `window` globals at any point (verify via `window` keys in console)

**Gate 4 PASS criteria:** Unlock/lock/auto-lock all work, seed is cleared from memory on lock, no seed in window globals.

### Gate 5: Balance and Transaction History

**Objective:** Wallet view shows balance, sequence, address, and transaction history.

- [ ] G5.1: After unlock, `WalletView.jsx` displays wallet address
- [ ] G5.2: "Copy address" button copies to clipboard, shows toast notification
- [ ] G5.3: RPC `account` called with wallet address, displays balance (raw PFT)
- [ ] G5.4: Balance displays with human-readable formatting (e.g., 1,234,567 PFT)
- [ ] G5.5: Sequence number displayed
- [ ] G5.6: Chain status indicator: green dot when online, red dot when offline
- [ ] G5.7: "Account not found" (unfunded) shows balance 0 gracefully, no error
- [ ] G5.8: `TransactionHistory.jsx` calls `account_tx` with limit 50
- [ ] G5.9: Each transaction displays: block height, direction (sent/received), counterparty, amount, fee
- [ ] G5.10: "Load more" button fetches next page (from_height pagination)
- [ ] G5.11: Empty history shows "No transactions yet" message
- [ ] G5.12: Click a transaction → detail view with all fields
- [ ] G5.13: Balance auto-refreshes on page focus (visibilitychange event)
- [ ] G5.14: All RPC-derived text is escaped (React JSX, no `dangerouslySetInnerHTML`)

**Gate 5 PASS criteria:** Balance and history display correctly for a funded account, empty states handled, no XSS vectors.

### Gate 6: Send PFT Transfer (Full Flow)

**Objective:** User can send a PFT transfer end-to-end through the web app.

- [ ] G6.1: `SendTransfer.jsx` renders recipient address input + amount input
- [ ] G6.2: Address validation: rejects non-`pf` prefix, wrong length, non-hex after `pf`
- [ ] G6.3: Amount validation: rejects 0, negative, non-integer
- [ ] G6.4: "Get Quote" → RPC `transfer_fee_quote` → displays fee, total, sequence, balance after
- [ ] G6.4a: Optional memo inputs are collapsed by default and accept string values for Memo Type, Memo Format, and Memo Data
- [ ] G6.4b: Memo quote forwards `memo_type`, `memo_format`, and `memo_data` to `transfer_fee_quote` so memo bytes are included in the fee
- [ ] G6.4c: Memo validation enforces 64-byte type, 64-byte format, 256-byte data, and 512-byte total limits before RPC
- [ ] G6.5: Quote with insufficient balance shows warning, blocks send
- [ ] G6.6: Quote shows whether recipient account exists (will_create_recipient_account)
- [ ] G6.7: "Confirm & Sign" button: calls WASM `wallet_sign_transfer(backup_json, quote_json)`
- [ ] G6.7a: With any memo field set, "Confirm & Sign" calls WASM `wallet_sign_payment_v2(backup_json, fields_json)` with `memos: [{ memo_type, memo_format, memo_data }]`
- [ ] G6.8: Signed output verified: `signature_hex` is 6618 chars, `public_key_hex` is 3904 chars
- [ ] G6.9: Signed transfer submitted via `mempool_submit_signed_transfer`
- [ ] G6.9a: Signed payment v2 submitted via `mempool_submit_signed_payment_v2` with param `signed_payment_v2_json`
- [ ] G6.10: Poll `receipts` every 2s, timeout 30s
- [ ] G6.11: Receipt accepted: display success with block height
- [ ] G6.12: Receipt rejected: display error code + message
- [ ] G6.13: Timeout: display "pending" with tx_id for manual lookup
- [ ] G6.14: Rate limit: min 3s between sign attempts (disable button during cooldown)
- [ ] G6.15: Full recipient address shown on confirmation screen (no truncation)
- [ ] G6.16: Amount shown in raw units
- [ ] G6.17: After successful send, balance refreshes
- [ ] G6.18: After successful send, transaction appears in history

**Gate 6 PASS criteria:** End-to-end transfer from web app wallet reaches the live testnet and is finalized.

### Gate 7: Send Asset Transfer

**Objective:** User can send an issued asset (e.g., pfUSDC) to another address.

- [ ] G7.1: Extend WASM with `wallet_sign_asset_transaction` and `wallet_sign_asset_transaction_fields` exports
- [ ] G7.2: Recompile WASM: `wasm-pack build crates/wallet_wasm --target web --out-dir ../wallet-web/src/wasm`
- [ ] G7.3: Verify new WASM exports callable from browser console
- [ ] G7.4: `SendAsset.jsx` fetches `account_assets` for wallet address, displays asset list
- [ ] G7.5: Asset selector dropdown populated from account assets
- [ ] G7.6: Recipient address input with same validation as PFT transfer
- [ ] G7.7: Amount input with validation
- [ ] G7.8: "Get Quote" → RPC `asset_fee_quote` → displays fee, sequence, balance after
- [ ] G7.9: "Confirm & Sign" → WASM `wallet_sign_asset_transaction` → RPC `mempool_submit_signed_asset_transaction`
- [ ] G7.10: Poll for receipt, display result
- [ ] G7.11: Test: send asset to a new address, verify receipt accepted
- [ ] G7.12: After successful send, asset balance refreshes

**Gate 7 PASS criteria:** Asset transfer signed in browser, submitted to chain, receipt confirmed.

### Gate 8: Offer Book (DEX)

**Objective:** User can view the order book, create offers, and cancel offers.

- [ ] G8.1: Extend WASM with `wallet_sign_offer_transaction` and `wallet_sign_offer_transaction_fields` exports
- [ ] G8.2: Recompile WASM
- [ ] G8.3: `OfferBook.jsx` fetches `book_offers` for a selected asset pair
- [ ] G8.4: Order book displays bids and asks with price and amount
- [ ] G8.5: "My Offers" tab fetches `account_offers` for wallet address
- [ ] G8.6: Create offer form: select pays asset, gets asset, amount, price
- [ ] G8.7: "Get Quote" → RPC `offer_fee_quote` → displays fee, sequence
- [ ] G8.8: "Confirm & Sign" → WASM `wallet_sign_offer_transaction` → RPC `mempool_submit_signed_offer_transaction`
- [ ] G8.9: Poll for receipt, display result
- [ ] G8.10: Cancel offer: select from "My Offers", confirm → sign cancel → submit
- [ ] G8.11: After create/cancel, offer list refreshes

**Gate 8 PASS criteria:** Offer create and cancel signed in browser, submitted to chain, order book updates.

### Gate 9: Settings and Wallet Management

**Objective:** User can configure settings, export/import backups, and remove the wallet.

- [ ] G9.1: `Settings.jsx` renders RPC endpoint dropdown (WAN devnet, local, custom)
- [ ] G9.2: Custom endpoint input shows when "Custom..." selected
- [ ] G9.3: Save settings persists to IndexedDB
- [ ] G9.4: Changing RPC endpoint reconnects RPC client and re-checks chain status
- [ ] G9.5: Auto-lock dropdown (5/15/30/60 minutes), save persists
- [ ] G9.6: Export backup: downloads JSON file with encrypted vault blob + metadata
- [ ] G9.7: Export backup JSON does NOT contain plaintext seed (verify by inspecting file)
- [ ] G9.8: Import backup: file upload, validate JSON structure, validate address matches
- [ ] G9.9: Import backup confirms overwrite if wallet exists
- [ ] G9.10: After import, wallet locks (forces re-unlock with new passphrase)
- [ ] G9.11: Remove wallet: confirmation dialog, clears IndexedDB vault + metadata + settings
- [ ] G9.12: After remove, app shows onboarding screen
- [ ] G9.13: Import corrupt JSON: fails gracefully with error message

**Gate 9 PASS criteria:** Settings persist, backup export/import round-trips, remove clears all data.

### Gate 10: Security Hardening

**Objective:** Pass a security audit equivalent to the Chrome extension's S1-S7 evaluation.

- [ ] G10.1: No plaintext seed in IndexedDB at any point (DevTools inspection)
- [ ] G10.2: No seed logged to console anywhere (grep source for `console.log` near seed vars)
- [ ] G10.3: Seed not stored in window globals (module-scope only)
- [ ] G10.4: Seed not in `sessionStorage` or `localStorage` (only encrypted blob in IndexedDB)
- [ ] G10.5: Passphrase never stored, logged, or sent over network
- [ ] G10.6: No `eval()`, `Function()`, or `setTimeout(string)` in source
- [ ] G10.7: No `dangerouslySetInnerHTML` in any React component
- [ ] G10.8: CSP: `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'` enforced
- [ ] G10.9: No third-party scripts loaded (no analytics, no CDNs, no external `src=`)
- [ ] G10.10: WebSocket URL validated (ws:// or wss:// only)
- [ ] G10.11: RPC response data escaped by React JSX (no raw HTML injection)
- [ ] G10.12: File import (backup JSON) validated before storage (structure + address format)
- [ ] G10.13: Auto-lock clears seed from memory
- [ ] G10.14: Lock button clears seed from memory
- [ ] G10.15: `beforeunload` clears seed from memory
- [ ] G10.16: Rapid double-click on create/import does not create duplicate wallets
- [ ] G10.17: Send with insufficient balance blocked by quote validation
- [ ] G10.18: Address validation rejects mixed case, non-hex, wrong length
- [ ] G10.19: Backup file does not contain plaintext seed
- [ ] G10.20: No `externally_connectable` (not an extension — verify no message passing)

**Gate 10 PASS criteria:** All 20 security checks pass, no plaintext seed leaks, CSP enforced, no XSS vectors.

### Gate 11: Local Build and Run

**Objective:** The web app builds and runs entirely locally with no external dependencies beyond npm.

- [ ] G11.1: `cd wallet-web && npm install` succeeds with no errors
- [ ] G11.2: `npm run dev` starts Vite dev server on localhost:5173
- [ ] G11.3: `npm run build` produces optimized `dist/` directory
- [ ] G11.4: `dist/` contains index.html, JS bundle, WASM file
- [ ] G11.5: Preview build: `npm run preview` serves dist/ on localhost:4173
- [ ] G11.6: Preview build loads wallet, creates wallet, signs transaction — all client-side
- [ ] G11.7: Proxy runs: `cd wallet-proxy && RPC_HOST=192.0.2.10 RPC_PORT=27650 node server.js`
- [ ] G11.8: Full local stack: dev server + proxy + live testnet = working wallet
- [ ] G11.9: No external API keys, no remote services, no cloud dependencies
- [ ] G11.10: README.md in `wallet-web/` with local run instructions

**Gate 11 PASS criteria:** Full wallet runs locally with zero external dependencies beyond the chain RPC (via proxy).

### Gate 12: Transparent Swap (pfUSDC ↔ a651)

**Objective:** User can perform a transparent (non-shielded) swap between pfUSDC and a651 through the web app. All transactions are public on-chain. The browser signs every transaction.

- [ ] G12.1: `SwapView.jsx` renders with "Transparent" mode selected by default
- [ ] G12.2: Asset balance display: fetch `account_assets` for wallet address, show pfUSDC and a651 balances
- [ ] G12.3: "Swap pfUSDC → a651" form: amount input, displays current pfUSDC balance
- [ ] G12.4: On submit: build `issued_payment` asset transaction operation (from=wallet, to=issuer, asset=pfUSDC, amount)
- [ ] G12.5: Get `asset_fee_quote` for the operation → display fee, sequence, balance after
- [ ] G12.6: "Confirm & Sign" → WASM `wallet_sign_asset_transaction` → `mempool_submit_signed_asset_transaction`
- [ ] G12.7: Poll for receipt → display accepted/rejected
- [ ] G12.8: On success: pfUSDC balance decreases, a651 balance increases (refresh both)
- [ ] G12.9: "Swap a651 → pfUSDC" form: amount input, displays current a651 balance
- [ ] G12.10: Same flow: build issued payment (a651 → issuer), quote, sign, submit, poll
- [ ] G12.11: On success: a651 balance decreases, pfUSDC balance increases
- [ ] G12.12: Round-trip test: swap pfUSDC → a651 → pfUSDC, verify balances return to start (minus fees)
- [ ] G12.13: All transaction details visible in history with asset transfer type
- [ ] G12.14: Insufficient balance: quote shows `sender_meets_reserve_after_fee: false`, UI blocks

**Gate 12 PASS criteria:** Transparent pfUSDC ↔ a651 round-trip works end-to-end, browser signs all transactions, balances update correctly.

### Gate 13: Asset Balance Display and NAV Snapshot

**Objective:** User can see their asset balances and the NAVCoin NAV/proof-of-reserves state.

- [ ] G13.1: `WalletView.jsx` shows a "Assets" section below PFT balance
- [ ] G13.2: `account_assets` RPC call returns all asset balances for the wallet address
- [ ] G13.3: Each asset displays: asset code (pfUSDC, a651), balance (raw + formatted), issuer address
- [ ] G13.4: Zero-balance assets hidden or shown as "0" based on settings toggle
- [ ] G13.5: `NavSnapshot.jsx` fetches NAV data from server API (`GET /api/swap/nav`)
- [ ] G13.6: NAV display: verified_net_assets, valid_global_supply, nav_floor, nav_per_unit
- [ ] G13.7: NAV invariant shown: `verified_net_assets ≥ valid_global_supply × nav_floor` with pass/fail indicator
- [ ] G13.8: "Before" and "After" buttons capture snapshots for swap comparison
- [ ] G13.9: Reserve composition displayed: source bucket line items (pfUSDC/USDC amounts)
- [ ] G13.10: All NAV data is read-only, no signing required

**Gate 13 PASS criteria:** Asset balances and NAV/proof-of-reserves display correctly, before/after comparison works.

### Gate 14: Private Swap — Browser-Side Steps

**Objective:** The web app can initiate and monitor the private (shielded) swap flow, with the browser handling all transparent transaction signing and the server handling ZK proof generation.

- [ ] G14.1: `SwapView.jsx` "Private (Shielded)" mode renders the 12-step rail
- [ ] G14.2: Step 0 (Create wallet): reuses existing wallet creation flow, no new code
- [ ] G14.3: Step 1 (Fund gas): shows PFTL balance, faucet fund button (calls server API if needed)
- [ ] G14.4: Step 2 (NAV before): calls `GET /api/swap/nav?phase=before`, displays snapshot
- [ ] G14.5: Step 3 (Warm prover): calls `POST /api/swap/action { action: "prewarm", execute: true }`, polls until `prewarmed`
- [ ] G14.6: Step 4 (Confirm pfUSDC): displays pfUSDC balance, bridge-in button if insufficient
- [ ] G14.7: Step 5 (Bridge in): if needed, calls server API for bridge; browser signs the PFTL claim transaction
- [ ] G14.8: Step 6 (Shield ingress): browser signs the public pfUSDC burn transaction (asset transaction, `asset_burn` operation); server handles Orchard batch certification
- [ ] G14.9: Step 7 (Private swap): calls `POST /api/swap/action { action: "shield_swap", execute: true }`, polls until `complete` (may take 5-7 minutes with warm prover)
- [ ] G14.10: Step 8 (Finality): calls `POST /api/swap/action { action: "finality_status" }`, verifies certificate height
- [ ] G14.11: Step 9 (Private egress): calls `POST /api/swap/action { action: "private_egress", amount, destination_ref }`, polls until `private_egressed`
- [ ] G14.12: Step 10 (Bridge out): browser signs public a651 → pfUSDC redemption and pfUSDC burn-to-redeem transactions; server handles EVM withdrawal
- [ ] G14.13: Step 11 (NAV after): calls `GET /api/swap/nav?phase=after`, displays snapshot + delta from before
- [ ] G14.14: Each step shows: pending → running → complete/failed with elapsed time
- [ ] G14.15: Failed step shows error message and retry button
- [ ] G14.16: User can resume from any completed step (step selector)
- [ ] G14.17: Browser never sends seed to server API — only sends signed transactions and public parameters
- [ ] G14.18: Privacy scan: verify private egress receipt does not contain note openings (nk, rivk, rho, psi, rcm, output_commitment)

**Gate 14 PASS criteria:** All 12 steps execute from the web app, browser signs all transparent legs, server handles proof generation, receipts verified, privacy scan passes.

### Gate 15: Swap Server API Integration

**Objective:** The web app can communicate with the companion swap server API for proof-heavy operations. This gate validates the integration layer, not the server itself (which is a separate build, based on StakeHub).

- [ ] G15.1: Create `src/lib/swap-server.js` — HTTP client for the swap server API
- [ ] G15.2: `swapServer.getStatus()` → `GET /api/swap/status` returns run state
- [ ] G15.3: `swapServer.getBalances()` → `GET /api/swap/balances` returns pfUSDC, a651, PFTL balances
- [ ] G15.4: `swapServer.getNav(phase)` → `GET /api/swap/nav?phase=before|after` returns NAV snapshot
- [ ] G15.5: `swapServer.action(body)` → `POST /api/swap/action` executes a swap step
- [ ] G15.6: Polling helper: `swapServer.pollAction(action, statusSet, timeout)` polls until status matches
- [ ] G15.7: Server URL configurable in Settings (default: `http://localhost:8787`)
- [ ] G15.8: Error handling: server unreachable shows "Swap server offline" with retry
- [ ] G15.9: Error handling: server returns error, display error code + message
- [ ] G15.10: No seed, passphrase, or private key ever sent to swap server (verify by inspecting request bodies)
- [ ] G15.11: Only signed transactions and public parameters sent to server
- [ ] G15.12: CORS: swap server must allow the web app origin (configured server-side)

**Gate 15 PASS criteria:** Web app communicates with swap server for proof-heavy steps, no key material leaves the browser, errors handled gracefully.

## File Layout

```
postfiatl1v2/
├── crates/wallet_wasm/          (existing, extended)
│   ├── Cargo.toml
│   └── src/lib.rs               (add asset/offer signing exports)
├── wallet-web/                  (new)
│   ├── package.json
│   ├── vite.config.js
│   ├── index.html
│   ├── README.md
│   ├── src/
│   │   ├── main.jsx
│   │   ├── App.jsx
│   │   ├── components/
│   │   │   ├── Onboard.jsx
│   │   │   ├── LockScreen.jsx
│   │   │   ├── WalletView.jsx
│   │   │   ├── SendTransfer.jsx
│   │   │   ├── SendAsset.jsx
│   │   │   ├── SwapView.jsx
│   │   │   ├── NavSnapshot.jsx
│   │   │   ├── OfferBook.jsx
│   │   │   ├── TransactionHistory.jsx
│   │   │   └── Settings.jsx
│   │   ├── lib/
│   │   │   ├── vault.js
│   │   │   ├── rpc-client.js
│   │   │   ├── wasm-loader.js
│   │   │   ├── tx-builder.js
│   │   │   ├── swap-server.js
│   │   │   └── utils.js
│   │   ├── styles/
│   │   │   └── main.css
│   │   └── wasm/
│   │       ├── postfiat_wallet_wasm.js
│   │       └── postfiat_wallet_wasm_bg.wasm
│   └── public/
│       └── favicon.ico
├── wallet-proxy/                (existing, reused)
│   ├── server.js
│   └── package.json
└── wallet-extension/            (existing, maintained for extension path)
```

## Build Commands

```bash
# Build WASM wallet core (with new asset/offer exports)
cd postfiatl1v2
wasm-pack build crates/wallet_wasm --target web --out-dir ../../wallet-web/src/wasm

# Run WebSocket proxy
cd wallet-proxy
RPC_HOST=192.0.2.10 RPC_PORT=27650 ALLOWED_ORIGINS=http://localhost:5173 node server.js

# Run web app (dev)
cd wallet-web
npm install
npm run dev
# Open http://localhost:5173

# Build for production
cd wallet-web
npm run build
npm run preview
# Open http://localhost:4173
```

## Deferred / Out of Scope for v1

- ZK proof generation in the browser — Halo2 proving (K=15 key) is not WASM-feasible; proofs are server-side
- EVM wallet integration (MetaMask) — IN SCOPE: the user connects MetaMask to bridge USDC from Arbitrum to pfUSDC on PFTL. The web app uses `window.ethereum` (MetaMask provider) for EVM-side deposit approval and deposit. PFTL-side claim is signed in browser via WASM.
- NFT transactions
- Escrow transactions
- Mnemonic/BIP39 support (current: raw hex seed; future: 24-word phrase per `wallet-mnemonic-design.md`)
- Multi-account support (v1 uses account_index=0 only)
- Hardware wallet integration
- Mobile-responsive layout (v1 is desktop-first)
- QR code scanning
- Address book / contacts
- Background notifications (requires extension or service worker)
- Cross-origin signing (requires extension with `externally_connectable`)
- Swap server itself — the companion API for proof generation is a separate build (based on StakeHub's `dashboard_server.py`); this spec covers the web app that calls it
- Account/login system — this is a crypto-only app; no GitHub, OAuth, email, or accounts; the wallet IS the identity

## Transaction Finality Model

See `docs/specs/wallet-wan-devnet-finality-fix.md` for the full spec.

### Wallet Send Path

The canonical wallet transaction flow is:

1. The wallet derives/holds user keys locally (browser IndexedDB).
2. The wallet signs the transaction locally (WASM ML-DSA-65).
3. The wallet submits the signed transaction to a write-enabled RPC edge.
4. The validator network orders and certifies the transaction (consensus loop).
5. The wallet polls transaction status or receives finality evidence.

The wallet must **not** need validator private keys, proposer keys, SSH access, or local validator data directories.

### `apply-batch` Is Not A Wallet Path

`apply-batch` is a local harness/operator tool that requires local filesystem access to validator data directories. It is **not** a public wallet RPC method and must not be used for WAN/testnet wallet sends.

### RPC Capability Discovery

The `server_info` RPC method returns capability fields under `rpc`:

| Field | Type | Meaning |
|---|---|---|
| `read_only` | bool | True if write methods are disabled. |
| `mempool_submit_enabled` | bool | True if mempool submit is allowed. |
| `mempool_submit_finality_enabled` | bool | True if finality RPC is allowed. |
| `max_mempool_submit_per_peer` | int | Per-peer rate limit. |
| `max_mempool_submit_total` | int | Global rate limit. |

### Chain Health States

The wallet UI shows distinct health states:

- **Online/readable**: RPC status and read methods respond.
- **Writable**: RPC advertises mempool submit is enabled.
- **Finalizing**: Chain height is advancing and submitted transactions can reach receipts.
- **Stalled**: RPC reads work, but height does not advance within the configured window.
- **Read-only**: RPC reads work, but write submit methods are disabled.

When the wallet cannot send, it shows the specific reason (read-only, stalled, pending, finalized).

### Python Send Modes

The Python wallet helpers (`postfiat_rpc.wallet`) have explicit modes:

| Mode | Intended use | Behavior |
|---|---|---|
| `submit_only` | WAN/testnet wallet path | Sign and submit to RPC, return `tx_id`, no local finalization. |
| `submit_and_poll` | WAN/testnet wallet path | Sign, submit, poll `tx`/`receipts` until finality or timeout. |
| `local_apply` | local harness only | Use `mempool-batch` plus `apply-batch` against local validator dirs. |
| `peer_certified` | controlled operator path | Use peer-certified transport with explicit topology/key files. |
