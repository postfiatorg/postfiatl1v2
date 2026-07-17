# PostFiat Web Wallet

Browser-based self-custody wallet for PostFiat L1. All key material is generated, encrypted, and signed entirely in the browser via WASM. The server never sees seeds, passphrases, or private keys.

## Quick Start

```bash
# 1. Install dependencies
cd wallet-web
npm install

# 2. Start the RPC proxy (in another terminal)
cd ../wallet-proxy
export WALLET_DEMO_PROFILE=/path/to/wallet-demo-profile.json
export RPC_ENDPOINT="$(jq -r '.services.pftl_finality_rpcs["validator-0"]' "$WALLET_DEMO_PROFILE")"
RPC_HOST="${RPC_ENDPOINT%:*}" RPC_PORT="${RPC_ENDPOINT##*:}" ALLOWED_ORIGINS=http://localhost:5173,https://localhost:5173,https://127.0.0.1:5173 node server.js

# 3. Start the web wallet dev server
npm run dev

# 4. Open the printed Vite URL.
```

## Build for Production

```bash
npm run build    # outputs to dist/
npm run preview
```

## Features

- **Self-custody:** ML-DSA-65 keygen and signing in WASM, keys never leave the browser
- **Encrypted vault:** AES-256-GCM with PBKDF2 (310k iterations), stored in IndexedDB
- **PFT transfers:** Send PFT to any address
- **Asset transfers:** Send pfUSDC, a651, and other issued assets
- **Swap:** Transparent (pfUSDC ↔ a651) and private (Asset-Orchard shielded) swap modes
- **Bridge:** MetaMask integration for USDC ↔ pfUSDC bridge between Arbitrum and PFTL
- **DEX:** Create, cancel, and view offers on the order book
- **History:** Transaction history with pagination
- **Settings:** RPC endpoint, auto-lock, swap server, bridge vault configuration
- **Security:** CSP enforced, no third-party scripts, no eval, auto-lock

## Architecture

```
Browser (this app) ←→ WebSocket Proxy ←→ Validator RPC (TCP)
                  ←→ Swap Server API (for private swap proofs)
                  ←→ MetaMask (for EVM bridge deposits)
```

## Money Transmitter Boundary

The server never receives the user's seed, passphrase, or private keys. The browser signs all PFTL transactions. The swap server only handles ZK proof generation and sees public parameters.

## Security

- No `eval()`, `Function()`, or `setTimeout(string)`
- No `dangerouslySetInnerHTML` (React JSX escaping only)
- CSP: `script-src 'self' 'wasm-unsafe-eval'; object-src 'none'`
- No third-party scripts (no analytics, no CDNs)
- Seed in module-scope only, never on `window`
- Auto-lock clears seed from memory
- `beforeunload` clears seed on page close
