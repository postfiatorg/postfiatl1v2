# PostFiat Wallet — Chrome Extension

A post-quantum self-custody wallet for the PostFiat L1 v2 blockchain. All cryptographic
operations (ML-DSA-65 keygen, signing, address derivation) happen client-side in the
browser via WebAssembly. No keys ever leave the browser.

## Architecture

```
Browser Extension (Chrome MV3)
├── WASM Core (Rust → wasm-pack)
│   ├── ML-DSA-65 keygen/signing (fips204 crate, pure Rust)
│   ├── SHA3-384 address derivation
│   └── RPC request/response builders
├── WebSocket Proxy (Node.js)
│   └── Bridges browser WS ↔ raw TCP RPC server
└── Chrome Storage (encrypted at rest)
    └── AES-256-GCM via Web Crypto API
```

## Install (Developer Mode)

1. **Start the WebSocket proxy:**
   ```bash
   cd wallet-proxy
   npm install
   node server.js  # listens on :8080, forwards to testnet RPC
   ```

   For a local node:
   ```bash
   LISTEN_PORT=8081 RPC_HOST=127.0.0.1 RPC_PORT=27651 node server.js
   ```

2. **Load the extension:**
   - Open `chrome://extensions`
   - Enable "Developer mode" (top right toggle)
   - Click "Load unpacked"
   - Select the `wallet-extension/` directory
   - The PostFiat Wallet icon should appear in your toolbar

3. **Create a wallet:**
   - Click the extension icon
   - Enter an encryption passphrase
   - Click "Create Wallet"
   - **SAVE YOUR SEED** — it's your only recovery method
   - Check "I have saved my seed" and click Create again

## Cryptographic Parameters

| Parameter | Value |
|-----------|-------|
| Signature algorithm | ML-DSA-65 (FIPS 204, post-quantum) |
| Public key | 1952 bytes (3904 hex chars) |
| Signature | 3309 bytes (6618 hex chars) |
| Address | 42 chars (`pf` + 40 hex chars) |
| Key derivation | SHA3-384 domain-separated, truncate to 32 bytes |
| Encryption at rest | AES-256-GCM via PBKDF2 (100k iterations) |

## RPC Methods Used

| Method | Purpose |
|--------|---------|
| `status` | Chain height, chain ID, validator count |
| `fee` | Transfer fee parameters |
| `account` | Wallet balance and sequence |
| `account_tx` | Transaction history |
| `transfer_fee_quote` | Get fee quote before signing |
| `mempool_submit_signed_transfer` | Submit signed transfer |
| `receipts` | Poll for transaction receipt |
| `validators` | Validator registry |

## Security

- **CSP:** `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'` — no inline scripts, no remote scripts
- **Permissions:** Only `storage` — no tabs, cookies, or webRequest
- **Seed encryption:** AES-256-GCM with PBKDF2 key derivation (100k iterations, SHA-256)
- **Auto-lock:** Configurable 5/15/30/60 minute timeout, clears decrypted seed from memory
- **No plaintext seed** stored in chrome.storage at any point
- **No eval()** in any JavaScript file

## File Layout

```
wallet-extension/
├── manifest.json          # MV3 manifest
├── background.js          # Service worker (auto-lock, unlocked state)
├── popup/
│   ├── popup.html         # Wallet UI
│   └── popup.js           # Wallet logic
├── lib/
│   ├── rpc-client.js      # WebSocket RPC client
│   ├── keystore.js        # AES-256-GCM encryption + chrome.storage
│   └── tx-builder.js       # Quote → sign → submit → poll flow
├── wasm/
│   ├── postfiat_wallet_wasm.js     # WASM JS bindings
│   └── postfiat_wallet_wasm_bg.wasm  # Compiled Rust → WASM
└── icons/
    ├── icon16.png
    ├── icon48.png
    └── icon128.png
```

## Building WASM from Source

```bash
cd /path/to/postfiatl1v2
wasm-pack build crates/wallet_wasm --target web --out-dir pkg
cp crates/wallet_wasm/pkg/postfiat_wallet_wasm.js wallet-extension/wasm/
cp crates/wallet_wasm/pkg/postfiat_wallet_wasm_bg.wasm wallet-extension/wasm/
```

WASM binary size: ~377 KB
