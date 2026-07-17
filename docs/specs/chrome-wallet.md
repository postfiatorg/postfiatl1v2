# PostFiat L1 Chrome Extension Wallet — Spec

## Objective

A Chrome Manifest V3 extension wallet for the PostFiat L1 blockchain. Self-custody: ML-DSA-65 keygen, signing, and address derivation happen entirely in the browser via WASM. No server holds keys.

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
| KDF | `sha3-384-domain-truncate32` |
| Address scheme | `pf` + SHA3-384(`postfiat.address.v1` \x00 public_key)[0:20] |
| Address length | 42 chars (`pf` + 40 hex chars) |
| Signing context | `postfiat-l1-v2/tx/v1` (embedded in `ml_dsa_65_sign`) |
| Rust crate | `fips204` v0.4.6, pure Rust, no C deps |

## RPC Wire Protocol

Transport: raw TCP, one JSON request per line, one JSON response per line.

Request format:
```json
{"version":"postfiat-local-rpc-v1","id":"<string>","method":"<method>","params":{<key>:<value>}}
```

Response format:
```json
{"version":"postfiat-local-rpc-v1","id":"<string>","ok":true,"result":{...},"error":null,"events":[...]}
```

Error format:
```json
{"version":"postfiat-local-rpc-v1","id":"<string>","ok":false,"result":null,"error":{"code":"<code>","message":"<msg>"},"events":[...]}
```

Constraint: `id` must be a string, not a number. `params` must be a JSON object (not nested objects — RPC rejects them). Max request size 8 MB.

### RPC Methods Used by the Wallet

| Method | Purpose | Key params |
|---|---|---|
| `status` | Chain height, state root, validator count | none |
| `fee` | Fee policy (reserve, minimum fee, byte quantum) | none |
| `account` | Account balance, sequence, public key | `address` |
| `account_tx` | Transaction history for an address | `address`, `from_height?`, `to_height?`, `limit?` |
| `transfer_fee_quote` | Quote a transfer: returns fee, sequence, validity checks | `from`, `to`, `amount`, `sequence?` |
| `mempool_submit_signed_transfer` | Submit a signed transfer to mempool | `signed_transfer_json` |
| `mempool_submit_signed_transfer_finality` | Submit + poll for finality certificate | `signed_transfer_json` |
| `receipts` | Check transaction receipt (accepted/rejected) | `tx_id?`, `limit?` |
| `blocks` | Recent blocks | `from_height?`, `limit?` |
| `validators` | Validator registry | none |

## Transfer Signing Flow (Exact Protocol)

### 1. Get fee quote

Request:
```json
{"version":"postfiat-local-rpc-v1","id":"q1","method":"transfer_fee_quote","params":{"from":"pf<addr>","to":"pf<addr>","amount":1000}}
```

Response `result` (TransferFeeQuoteSummary):
```json
{
  "chain_id": "postfiat-wan-devnet",
  "genesis_hash": "231b1cfb...",
  "protocol_version": 1,
  "from": "pf...",
  "to": "pf...",
  "amount": 1000,
  "sequence": 20,
  "sequence_source": "account_state",
  "sender_balance": 887978531,
  "sender_sequence": 19,
  "mempool_pending_for_sender": 0,
  "recipient_exists": true,
  "will_create_recipient_account": false,
  "base_transfer_fee": 1,
  "state_expansion_fee": 0,
  "minimum_fee": 1,
  "account_reserve": 10,
  "sender_balance_after_amount_and_fee": 887977530,
  "sender_meets_reserve_after_transfer": true
}
```

### 2. Sign transfer (WASM)

Input to WASM `wallet_sign_transfer`:
- `backup_json`: WalletBackupFile JSON (contains master_seed_hex, chain_id, account_index)
- `fields`: WalletSignTransferFields
  - `chain_id`: from quote
  - `genesis_hash`: from quote
  - `protocol_version`: from quote
  - `to`: recipient address
  - `amount`: transfer amount
  - `fee`: `minimum_fee` from quote
  - `sequence`: `sequence` from quote

WASM executes (existing code in `rpc_sdk/src/lib_parts/part_01.rs:4249`):
1. Derive 32-byte seed from master_seed via SHA3-384 domain separation
2. `ml_dsa_65_keygen_from_seed(seed)` → keypair
3. `address_from_public_key(public_key)` → verify matches `from`
4. Build `UnsignedTransfer` with all fields
5. `unsigned.signing_bytes()` → canonical signing message
6. `ml_dsa_65_sign(private_key, signing_bytes)` → 3309-byte signature
7. Verify signature with `ml_dsa_65_verify`
8. Return `SignedTransfer` JSON

Signing bytes format (canonical, line-delimited):
```
postfiat.transfer.v1
chain_id=<chain_id>
genesis_hash=<genesis_hash>
protocol_version=<protocol_version>
address_namespace=postfiat.address.v1
transaction_kind=transparent_transfer
signature_algorithm_id=ML-DSA-65
from=<sender_address>
to=<recipient_address>
amount=<amount>
fee=<fee>
sequence=<sequence>
```

Output (SignedTransfer JSON):
```json
{
  "unsigned": {
    "chain_id": "postfiat-wan-devnet",
    "genesis_hash": "231b1cfb...",
    "protocol_version": 1,
    "address_namespace": "postfiat.address.v1",
    "transaction_kind": "transparent_transfer",
    "signature_algorithm_id": "ML-DSA-65",
    "from": "pf...",
    "to": "pf...",
    "amount": 1000,
    "fee": 1,
    "sequence": 20
  },
  "algorithm_id": "ML-DSA-65",
  "public_key_hex": "<3904 hex chars>",
  "signature_hex": "<6618 hex chars>"
}
```

### 3. Submit signed transfer

Request:
```json
{"version":"postfiat-local-rpc-v1","id":"s1","method":"mempool_submit_signed_transfer","params":{"signed_transfer_json":"<SignedTransfer JSON as string>"}}
```

Response `result` (MempoolSubmitSummary):
```json
{
  "tx_id": "<hex>",
  "chain_id": "postfiat-wan-devnet",
  "genesis_hash": "231b1cfb...",
  "protocol_version": 1,
  "from": "pf...",
  "to": "pf...",
  "amount": 1000,
  "fee": 1,
  "sequence": 20,
  "algorithm_id": "ML-DSA-65"
}
```

### 4. Poll for receipt

Request:
```json
{"version":"postfiat-local-rpc-v1","id":"r1","method":"receipts","params":{"tx_id":"<tx_id>"}}
```

Response `result` includes `accepted: true/false`, `code`, `message`.

## Architecture

```
┌──────────────────────────────────────────────────┐
│              Chrome Extension (MV3)               │
│                                                   │
│  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  UI Layer    │  │  WASM Module              │  │
│  │  (React +    │  │  (postfiat-wallet-wasm)    │  │
│  │   Vite)      │  │                           │  │
│  │              │  │  wallet_keygen()          │  │
│  │  - Onboard   │  │  wallet_address()         │  │
│  │  - Balance   │  │  wallet_sign_transfer()   │  │
│  │  - Send      │  │  wallet_sign_payment_v2()  │  │
│  │  - History   │  │  make_rpc_request()       │  │
│  │  - Settings  │  │  parse_rpc_response()     │  │
│  └──────────────┘  └──────────────────────────┘  │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │  Storage (chrome.storage.local)              │ │
│  │  - encrypted_master_seed (AES-256-GCM)       │ │
│  │  - wallet_metadata (address, account_index)  │ │
│  │  - address_book                               │ │
│  │  - tx_history_cache                           │ │
│  │  - rpc_endpoint_config                        │ │
│  └─────────────────────────────────────────────┘ │
│                                                   │
│  ┌─────────────────────────────────────────────┐ │
│  │  Transport Layer (fetch → WebSocket proxy)   │ │
│  └─────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────┘
          │
          │ WebSocket (wss://)
          ▼
┌─────────────────────┐
│  WebSocket Proxy     │
│  (Node.js, ~50 LOC)  │
│  ws → raw TCP JSON   │
└─────────────────────┘
          │
          │ TCP
          ▼
┌─────────────────────┐
│  Validator RPC       │
│  192.0.2.10:27650 │
└─────────────────────┘
```

## Component Specs

### Component 1: WASM Wallet Core (`crates/wallet_wasm/`)

New crate in the `postfiatl1v2` workspace. Compiles existing crypto + signing code to WASM via `wasm-pack`.

**Dependencies (all already pure Rust, WASM-compatible):**
- `postfiat-crypto-provider` (ML-DSA-65, SHA3-384, address derivation)
- `postfiat-rpc-sdk` (wallet_backup, wallet_sign_transfer, wallet_identity)
- `postfiat-types` (SignedTransfer, UnsignedTransfer, etc.)
- `wasm-bindgen`
- `getrandom` with `js` feature (provides `rand_core` in browser)
- `serde`, `serde_json`

**Exported functions:**

```rust
#[wasm_bindgen]
pub fn wallet_keygen(chain_id: &str, master_seed_hex: &str, account_index: u32) -> Result<JsValue, JsValue>
// Wraps wallet_backup_from_master_seed + wallet_identity_from_backup
// Returns: { address, public_key_hex, backup_json }

#[wasm_bindgen]
pub fn wallet_address_from_seed(chain_id: &str, master_seed_hex: &str, account_index: u32) -> Result<String, JsValue>
// Returns just the address string

#[wasm_bindgen]
pub fn wallet_sign_transfer(backup_json: &str, quote_json: &str) -> Result<JsValue, JsValue>
// Wraps wallet_sign_transfer_from_quote
// backup_json: WalletBackupFile as JSON string
// quote_json: TransferFeeQuoteSummary as JSON string
// Returns: SignedTransfer as JS object

#[wasm_bindgen]
pub fn wallet_sign_transfer_fields(backup_json: &str, fields_json: &str) -> Result<JsValue, JsValue>
// Wraps wallet_sign_transfer_from_fields
// For manual field specification without a quote

#[wasm_bindgen]
pub fn wallet_sign_payment_v2(backup_json: &str, fields_json: &str) -> Result<JsValue, JsValue>
// Wraps wallet_sign_payment_v2_from_fields
// For transfers with memos

#[wasm_bindgen]
pub fn make_rpc_request(method: &str, params_json: &str) -> Result<String, JsValue>
// Builds a canonical RpcRequest JSON string with version + unique id
// Returns: JSON string ready to send over the wire

#[wasm_bindgen]
pub fn parse_rpc_response(response_json: &str) -> Result<JsValue, JsValue>
// Parses and validates an RpcResponse
// Returns: { ok, result, error } as JS object

#[wasm_bindgen]
pub fn random_master_seed() -> Result<String, JsValue>
// Generates 32 random bytes via getrandom(js)
// Returns: 64-char hex string
```

### Component 2: WebSocket-to-TCP Proxy

A minimal Node.js server that bridges browser WebSocket connections to the raw TCP RPC. Deployed alongside the validator or on a separate host.

**Requirements:**
- Accept WebSocket connections (ws or wss)
- For each connection, open a TCP socket to the validator RPC host:port
- Forward WebSocket messages as TCP lines (append `\n`)
- Forward TCP responses as WebSocket messages
- Handle connection cleanup on either side closing
- Support configurable target host:port via environment variable
- Optional TLS termination for production

**Estimated size:** ~50 lines of Node.js.

### Component 3: Chrome Extension

**Manifest V3 structure:**
```
postfiat-wallet/
├── manifest.json
├── background.js
├── content_scripts/
│   └── (none for v1 — no dApp injection)
├── popup/
│   ├── popup.html
│   ├── popup.jsx
│   └── popup.css
├── tab/
│   ├── wallet.html
│   ├── wallet.jsx
│   └── wallet.css
├── wasm/
│   ├── postfiat_wallet_wasm.js
│   └── postfiat_wallet_wasm_bg.wasm
├── lib/
│   ├── rpc-client.js
│   ├── keystore.js
│   └── tx-builder.js
└── icons/
    ├── icon16.png
    ├── icon48.png
    └── icon128.png
```

**UI screens:**

1. **Onboarding / Create Wallet**
   - Generate 32 random bytes (Web Crypto `crypto.getRandomValues`)
   - Call WASM `wallet_keygen` with chain_id + master_seed_hex + account_index=0
   - Display: address, public key (truncated), backup seed phrase
   - Require user to confirm they saved the seed
   - Encrypt master_seed_hex with AES-256-GCM (key derived from user passphrase via PBKDF2, 100k iterations, Web Crypto)
   - Store encrypted blob in `chrome.storage.local`
   - Offer "import wallet" path (paste master_seed_hex)

2. **Balance / Account View**
   - Load wallet from storage, decrypt with passphrase
   - Call RPC `account` with wallet address
   - Display: balance, sequence, address (with copy button)
   - Call RPC `account_tx` for recent transaction list
   - Auto-refresh on focus

3. **Send Transfer**
   - Input: recipient address (paste/scan), amount
   - Call RPC `transfer_fee_quote` → display fee, total, sequence, balance-after
   - Confirmation screen: show from, to, amount, fee, sequence
   - User confirms → WASM `wallet_sign_transfer(backup_json, quote_json)`
   - Call RPC `mempool_submit_signed_transfer` with signed JSON
   - Poll RPC `receipts` until accepted/rejected (timeout 30s)
   - Display result with block height and finality status

4. **Transaction History**
   - Call RPC `account_tx` with pagination
   - Display: block height, direction (sent/received), counterparty, amount, fee, finality
   - Infinite scroll / load more

5. **Settings**
   - RPC endpoint selector (testnet/mainnet presets + custom)
   - Export wallet backup (download encrypted JSON)
   - Import wallet backup
   - Lock wallet (clear in-memory decrypted state)
   - Auto-lock timer (clear decrypted seed after N minutes)

### Component 4: Key Store (`lib/keystore.js`)

**Encryption:**
- Master seed encrypted with AES-256-GCM via Web Crypto API
- Encryption key derived from user passphrase via PBKDF2 (SHA-256, 100,000 iterations, 16-byte salt)
- Salt stored alongside ciphertext in `chrome.storage.local`
- Encrypted blob format: `{ salt: base64, iv: base64, ciphertext: base64 }`

**In-memory state:**
- Decrypted master seed held in a `Map` in the background service worker
- Cleared on lock or auto-lock timeout
- Never written to disk unencrypted
- Zeroized after use (set to null, hope GC cooperates)

### Component 5: RPC Client (`lib/rpc-client.js`)

- Opens WebSocket to proxy endpoint
- Sends JSON-RPC requests, parses responses
- Request ID counter (incrementing integer as string)
- Timeout handling (default 10s for reads, 30s for submits)
- Connection pooling / keepalive
- Error classification (protocol error, RPC error, transport error)

### Component 6: Transaction Builder (`lib/tx-builder.js`)

Orchestrates the full send flow:
1. Get fee quote from RPC
2. Validate quote (sender balance sufficient, sequence correct, fee > 0)
3. Call WASM to sign
4. Submit to mempool
5. Poll for receipt
6. Return finality result
7. Update local tx history cache

## Security Requirements

1. **Private keys never leave the browser.** All signing in WASM. No remote signing API calls.
2. **Master seed encrypted at rest.** AES-256-GCM with passphrase-derived key. Never stored plaintext.
3. **Passphrase never stored.** Derived key used once for decryption, then discarded.
4. **No `eval` or remote script loading.** CSP: `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'`.
5. **Phishing protection.** Full recipient address shown on confirmation screen. No truncation. Amount in raw units with human-readable display.
6. **Rate limiting.** Max 1 sign attempt per 3 seconds to prevent accidental rapid-fire.
7. **Origin checking.** RPC proxy rejects connections from non-extension origins (check `Origin` header).
8. **Auto-lock.** Clear decrypted seed after 15 minutes of inactivity (configurable).
9. **No transaction broadcasting without explicit confirmation.** UI must show full transaction details and require a click before signing.

## Gates and Checklist

### Gate 0: WASM Compilation

- [x] Create `crates/wallet_wasm/Cargo.toml` with `crate-type = ["cdylib", "rlib"]`
- [x] Add `wasm-bindgen`, `getrandom` (js feature), `serde`, `serde_json` dependencies
- [x] Add `postfiat-crypto-provider`, `postfiat-rpc-sdk`, `postfiat-types` as path dependencies
- [x] Create `crates/wallet_wasm/src/lib.rs` with `#[wasm_bindgen]` exports
- [x] Implement `wallet_keygen` — wraps `wallet_backup_from_master_seed` + `wallet_identity_from_backup`
- [x] Implement `wallet_address_from_seed` — wraps derivation + `address_from_public_key`
- [x] Implement `wallet_sign_transfer` — wraps `wallet_sign_transfer_from_quote`
- [x] Implement `wallet_sign_transfer_fields` — wraps `wallet_sign_transfer_from_fields`
- [x] Implement `wallet_sign_payment_v2` — wraps `wallet_sign_payment_v2_from_fields`
- [x] Implement `make_rpc_request` — builds canonical `RpcRequest` JSON string
- [x] Implement `parse_rpc_response` — parses + validates `RpcResponse`
- [x] Implement `random_master_seed` — 32 random bytes via `getrandom`, return hex
- [x] Add `wallet_wasm` to workspace `Cargo.toml` members
- [x] Install `wasm-pack` (`cargo install wasm-pack`)
- [x] `wasm-pack build crates/wallet_wasm --target web --out-dir pkg` compiles without errors
- [x] Verify generated `.wasm` file is under 2 MB (estimate: fips204 + sha3 + serde ~ 500KB)
- [x] Write a Node.js test script that loads the WASM, calls `wallet_keygen`, verifies address format is `pf` + 40 hex chars
- [x] Write a test that calls `wallet_sign_transfer` with a known seed and verifies the signature is 3309 bytes (6618 hex chars)
- [x] Write a test that verifies `wallet_sign_transfer` output passes `ml_dsa_65_verify` (round-trip test)
- [x] Write a test that `make_rpc_request` produces valid JSON with `version: "postfiat-local-rpc-v1"` and string `id`
- [x] Gate 0 PASS: all tests green, WASM compiles, signing round-trips

### Gate 1: WebSocket Proxy

- [x] Create `proxy/server.js` — Node.js WebSocket-to-TCP bridge
- [x] Accept WebSocket connections on configurable port (default 8080)
- [x] For each connection, open `net.connect()` to target RPC host:port
- [x] Forward WS messages as TCP lines (append `\n`)
- [x] Forward TCP responses as WS messages (buffer until newline, then send)
- [x] Handle clean shutdown on either side closing
- [x] Add configurable target via `RPC_HOST` and `RPC_PORT` env vars
- [x] Add `--tls` flag for WSS (read cert/key files)
- [x] Add origin check: reject connections without matching `Origin` header (configurable allowlist)
- [x] Add request logging (optional, behind `--verbose` flag)
- [x] Deploy proxy to a host reachable from the browser (Vultr instance or localhost for dev)
- [x] Test: open WS connection, send `{"version":"postfiat-local-rpc-v1","id":"t1","method":"status","params":{}}`, verify response with `ok:true` and `block_height` field
- [x] Test: send `account` request for faucet address, verify `balance` and `sequence` fields
- [x] Test: close WS abruptly, verify TCP socket is cleaned up (no leak)
- [x] Test: close TCP side (restart validator), verify WS closes with error
- [x] Gate 1 PASS: proxy relays status and account queries to live testnet, cleanup works

### Gate 2: Chrome Extension Skeleton

- [x] Create `postfiat-wallet/manifest.json` (Manifest V3)
  - `permissions`: `["storage"]`
  - `host_permissions`: `["ws://<proxy-host>:8080/*", "wss://<proxy-host>:8080/*"]`
  - `action`: popup
  - `background`: service worker
  - `content_security_policy`: `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'`
- [x] Create popup HTML + React entry point (`popup/popup.jsx`)
- [x] Create full-page wallet HTML + React entry point (`tab/wallet.jsx`)
- [x] Copy WASM package into `wasm/` directory
- [x] Create `lib/rpc-client.js` — WebSocket connection, request/response, timeout
- [x] Create `lib/keystore.js` — AES-256-GCM encrypt/decrypt via Web Crypto, chrome.storage
- [x] Create `lib/tx-builder.js` — quote → sign → submit → poll flow
- [x] Load extension as unpacked in `chrome://extensions`
- [x] Verify popup opens and renders "No wallet" state
- [x] Verify WASM module loads in extension context (call `wallet_keygen` from console)
- [x] Verify RPC client connects to WebSocket proxy and returns chain status
- [x] Gate 2 PASS: extension loads, WASM works, RPC client reaches live testnet through proxy

### Gate 3: Wallet Creation / Onboarding

- [x] Implement "Create New Wallet" flow:
  - Generate master seed via WASM `random_master_seed()`
  - Call WASM `wallet_keygen(chain_id, master_seed_hex, 0)`
  - Display address + 64-char hex seed with "save this" warning
  - Require user to check "I have saved my seed" checkbox
  - Prompt for encryption passphrase
  - Encrypt master_seed_hex via `keystore.encrypt(seed, passphrase)`
  - Store encrypted blob + wallet metadata in `chrome.storage.local`
  - Clear plaintext seed from memory
- [x] Implement "Import Wallet" flow:
  - Accept 64-char hex master seed paste
  - Validate: 64 hex chars, even length
  - Call WASM `wallet_keygen` to derive address
  - Display derived address for confirmation
  - Encrypt and store same as create flow
- [x] Implement "Lock" button: clear decrypted seed from memory, show locked state
- [x] Implement "Unlock" flow: prompt passphrase, decrypt, load into memory
- [x] Test: create wallet, verify address starts with `pf` and is 42 chars
- [x] Test: lock, then unlock with correct passphrase, verify address matches
- [x] Test: unlock with wrong passphrase fails gracefully (no crash, clear error)
- [x] Test: close popup and reopen, verify wallet is locked (seed not in memory)
- [x] Test: create second wallet with same seed, verify same address (deterministic)
- [x] Gate 3 PASS: wallet creation, import, lock/unlock all work; seed is encrypted at rest

### Gate 4: Balance and Account View

- [x] Implement balance fetch: RPC `account` with wallet address
- [x] Display: address (with copy-to-clipboard), balance (raw + human-readable), sequence
- [x] Implement auto-refresh on popup focus / tab focus
- [x] Implement transaction history: RPC `account_tx` with `limit: 50`
- [x] Display transaction list: block height, direction, counterparty, amount, fee
- [x] Implement "load more" pagination for transaction history
- [x] Handle "account not found" (balance 0, never funded) gracefully
- [x] Test: fund a new wallet from faucet (manual RPC call), verify balance appears
- [x] Test: verify transaction history shows the faucet funding transaction
- [x] Test: verify balance updates after a transfer (manual transfer via Python client)
- [x] Gate 4 PASS: balance and transaction history display correctly for a funded account

### Gate 5: Send Transfer (Full Flow)

- [x] Implement send form: recipient address input, amount input
- [x] Validate recipient address: starts with `pf`, 42 chars, all hex after `pf`
- [x] Validate amount: positive integer, user has sufficient balance
- [x] Call RPC `transfer_fee_quote` with from/to/amount
- [x] Display quote: fee, total cost (amount + fee), sequence, balance after, recipient exists
- [x] Implement confirmation screen: show from, to, amount, fee, sequence, balance-after
- [x] Require explicit "Confirm Send" button click
- [x] On confirm: call WASM `wallet_sign_transfer(backup_json, quote_json)`
- [x] Verify signed output: `signature_hex` is 6618 chars, `public_key_hex` is 3904 chars
- [x] Call RPC `mempool_submit_signed_transfer` with `signed_transfer_json`
- [x] On submit success: display tx_id, begin polling
- [x] Poll RPC `receipts` with tx_id every 2s, timeout 30s
- [x] On receipt `accepted: true`: display block height, success state
- [x] On receipt `accepted: false`: display error code + message
- [x] On timeout: display "pending" state with tx_id for manual lookup
- [x] Update local transaction history cache with new transaction
- [x] Test: send transfer from funded wallet to new address, verify receipt accepted
- [x] Test: verify recipient balance increased by transfer amount
- [x] Test: verify sender balance decreased by amount + fee
- [x] Test: send transfer to same address (self-transfer), verify works
- [x] Test: send transfer with insufficient balance, verify fee quote shows `sender_meets_reserve_after_transfer: false` and UI blocks
- [x] Test: send transfer with invalid recipient (not `pf` prefix), verify UI rejects before RPC call
- [x] Test: send transfer with amount 0, verify UI rejects (WASM signing requires nonzero amount)
- [x] Gate 5 PASS: end-to-end transfer from extension wallet reaches the live testnet and is finalized

### Gate 6: Settings and Wallet Management

- [x] Implement RPC endpoint selector (dropdown: testnet validator-0 through validator-5 + custom)
- [x] Store selected endpoint in `chrome.storage.local`
- [x] Implement auto-lock timer (default 15 min, configurable 5/15/30/60 min)
- [x] Implement "Export Backup" — download JSON file with encrypted master seed
- [x] Implement "Import Backup" — upload JSON, decrypt, verify address
- [x] Implement "Remove Wallet" — delete from chrome.storage with confirmation
- [x] Test: switch RPC endpoint, verify balance loads from different validator
- [x] Test: auto-lock fires after configured timeout, verify seed is cleared
- [x] Test: export + import backup round-trips to same address
- [x] Test: remove wallet clears all stored data
- [x] Gate 6 PASS: settings persist, auto-lock works, backup export/import round-trips

### Gate 7: Security Hardening

- [x] Verify CSP blocks inline scripts and remote script loading
- [x] Verify `wasm-unsafe-eval` is the only relaxation (required for WASM)
- [x] Audit all `chrome.storage` writes: only encrypted seed and plaintext metadata (address, settings) stored
- [x] Verify no plaintext seed appears in `chrome.storage.local` at any point
- [x] Verify passphrase is never logged, stored, or sent over network
- [x] Verify WebSocket connection uses `wss://` (TLS) in production config
- [x] Verify RPC proxy rejects connections from non-extension origins
- [x] Verify rate limiting on sign operations (min 3s between sign calls)
- [x] Verify full recipient address is displayed (no truncation) on confirmation screen
- [x] Verify amount is shown in both raw units and human-readable format
- [x] Run Chrome DevTools Memory profiler: verify no plaintext seed persists in heap after lock
- [x] Verify extension does not request unnecessary permissions (no `tabs`, `cookies`, `webRequest`)
- [x] Gate 7 PASS: security audit complete, no plaintext seed leaks, CSP enforced

### Gate 8: Polish and Release Prep

- [x] Add loading states for all async operations (spinners)
- [x] Add error states for all network failures (retry buttons)
- [x] Add empty states (no wallet, no transactions, zero balance)
- [x] Add responsive layout (popup 360px wide, tab full-page)
- [x] Add dark mode (default for a crypto wallet)
- [x] Add keyboard shortcuts (Enter to submit, Esc to cancel)
- [x] Add transaction detail view (click a tx in history → full detail modal)
- [x] Add "copy address" button with toast notification
- [x] Add "scan QR" for recipient address (optional, deferred if no QR lib in WASM-compatible form)
- [x] Write `README.md` in the extension directory with install instructions
- [x] Package extension as `.zip` for sideloading
- [x] Test on Chrome stable (Linux)
- [x] Test on Chrome stable (macOS) if available
- [x] Test on Chrome stable (Windows) if available
- [x] Gate 8 PASS: UI is polished, errors handled, extension packaged and testable on multiple platforms

## File Layout

```
postfiatl1v2/
├── crates/wallet_wasm/
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs
├── wallet-extension/
│   ├── manifest.json
│   ├── background.js
│   ├── popup/
│   │   ├── popup.html
│   │   ├── popup.jsx
│   │   └── popup.css
│   ├── tab/
│   │   ├── wallet.html
│   │   ├── wallet.jsx
│   │   └── wallet.css
│   ├── wasm/
│   │   ├── postfiat_wallet_wasm.js
│   │   └── postfiat_wallet_wasm_bg.wasm
│   ├── lib/
│   │   ├── rpc-client.js
│   │   ├── keystore.js
│   │   └── tx-builder.js
│   └── icons/
│       ├── icon16.png
│       ├── icon48.png
│       └── icon128.png
└── wallet-proxy/
    ├── server.js
    ├── package.json
    └── README.md
```

## Build Commands

```bash
# Build WASM wallet core
cd $POSTFIAT_REPO
wasm-pack build crates/wallet_wasm --target web --out-dir ../wallet-extension/wasm

# Build extension UI (if using a bundler)
cd wallet-extension
npx vite build --outDir dist

# Run WebSocket proxy
cd wallet-proxy
RPC_HOST=192.0.2.10 RPC_PORT=27650 node server.js

# Load extension
# Chrome → chrome://extensions → Developer mode → Load unpacked → select wallet-extension/
```

## Deferred / Out of Scope for v1

- Shielded/Orchard transactions (deposit, spend, withdraw) — proof generation is CPU-intensive, not WASM-feasible yet
- DEX / offer transactions
- NFT transactions
- Escrow transactions
- Asset issuance / trustlines
- NAVCoin operations
- dApp injection / content scripts (MetaMask-style provider injection)
- Mobile wallet (separate effort)
- QR code scanning
- Address book / contacts (basic storage only in v1)
- Multi-account support (v1 uses account_index=0 only)
- Hardware wallet integration
- Transaction mempool visualization
- Block explorer integration
