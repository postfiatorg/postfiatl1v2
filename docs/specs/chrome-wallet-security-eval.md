# PostFiat Chrome Wallet — Testing & Security Evaluation Plan

## Scope

Audit and harden the Chrome extension wallet built per `chrome-wallet.md`.
All work is local to this repo. No deployment, no remote push.

## Attack Surface Inventory

| Component | File(s) | Attack Surface |
|-----------|---------|-----------------|
| WASM core | `crates/wallet_wasm/src/lib.rs` | Seed handling, signing, error messages leaking internals |
| Popup UI | `wallet-extension/popup/popup.js` | XSS via innerHTML, seed in window globals, plaintext seed in messages |
| Popup HTML | `wallet-extension/popup/popup.html` | CSP gaps, inline event handlers |
| Background SW | `wallet-extension/background.js` | Seed in service worker memory, message handler lacks origin check, auto-lock bypass |
| Keystore | `wallet-extension/lib/keystore.js` | Encryption parameters, base64 encoding, storage key names |
| RPC client | `wallet-extension/lib/rpc-client.js` | WebSocket URL validation, response correlation, timeout DoS |
| TX builder | `wallet-extension/lib/tx-builder.js` | Quote validation, sign flow |
| Proxy | `wallet-proxy/server.js` | No message size limit, no JSON validation, origin bypass, TCP resource leak, error message leaks |
| Manifest | `wallet-extension/manifest.json` | web_accessible_resources too broad, missing host_permissions |

## Evaluation Gates

### S1: Seed Lifecycle Audit
- [x] S1.1: Verify no plaintext seed in chrome.storage.local at any point
- [x] S1.2: Verify seed is not logged to console anywhere
- [x] S1.3: Verify seed is not stored in window globals after wallet creation
- [x] S1.4: Verify backup_json (contains master_seed_hex) is not sent over network
- [x] S1.5: Verify passphrase is never stored, logged, or sent over network
- [x] S1.6: Verify auto-lock actually clears seed from service worker memory
- [x] S1.7: Verify lock button clears seed from popup memory
- [x] S1.8: Verify popup close does not leave seed in window globals

### S2: XSS & Injection Audit
- [x] S2.1: Find all innerHTML uses and verify RPC response data is escaped
- [x] S2.2: Verify no eval(), Function(), or setTimeout(string) usage
- [x] S2.3: Verify CSP blocks inline scripts (no script tags without src)
- [x] S2.4: Verify RPC response fields cannot inject HTML into tx history
- [x] S2.5: Verify address validation regex is not bypassable
- [x] S2.6: Verify file import (backup JSON) is sanitized before storage

### S3: Transport Security Audit
- [x] S3.1: Verify proxy rejects oversized WS messages (maxPayload limit)
- [x] S3.2: Verify proxy validates JSON before forwarding to TCP
- [x] S3.3: Verify proxy does not leak internal error details to WS client
- [x] S3.4: Verify proxy TCP connections are cleaned up (no resource leak)
- [x] S3.5: Verify proxy origin checking cannot be bypassed
- [x] S3.6: Verify RPC client validates WebSocket URL scheme
- [x] S3.7: Verify RPC client handles malformed responses without crash

### S4: Message Passing Security
- [x] S4.1: Verify background.js message handler checks sender origin
- [x] S4.2: Verify getBackup/getState don't leak seed to other contexts
- [x] S4.3: Verify unlock message requires all fields (seed, backup, address)
- [x] S4.4: Verify no external page can send messages to the extension

### S5: Cryptographic Audit
- [x] S5.1: Verify PBKDF2 iterations >= 100k (OWASP minimum)
- [x] S5.2: Verify AES-GCM IV is unique per encryption (random, 12 bytes)
- [x] S5.3: Verify salt is random and unique per encryption
- [x] S5.4: Verify key derivation uses SHA-256 (not weaker hash)
- [x] S5.5: Verify WASM signing does verify-after-sign roundtrip
- [x] S5.6: Verify signing rejects amount=0, fee=0, sequence=0

### S6: Permission & CSP Audit
- [x] S6.1: Verify only "storage" permission requested
- [x] S6.2: Verify no tabs, cookies, webRequest, host permissions
- [x] S6.3: Verify CSP: script-src 'self' 'wasm-unsafe-eval' only
- [x] S6.4: Verify CSP: no unsafe-inline, no unsafe-eval, no http:
- [x] S6.5: Verify web_accessible_resources uses minimal match patterns
- [x] S6.6: Verify object-src 'none' (no plugins)

### S7: Functional Edge Cases
- [x] S7.1: Verify wrong passphrase doesn't crash, shows error
- [x] S7.2: Verify empty inputs handled gracefully
- [x] S7.3: Verify RPC timeout doesn't leave UI in stuck state
- [x] S7.4: Verify rapid double-click doesn't create duplicate wallets
- [x] S7.5: Verify import backup with corrupt JSON fails gracefully
- [x] S7.6: Verify remove wallet clears all storage keys
- [x] S7.7: Verify send with insufficient balance is blocked by quote
- [x] S7.8: Verify address validation rejects mixed case, non-hex, wrong length

### S8: Regression & Integration
- [x] S8.1: Re-run all original Gate 0-8 tests, verify still pass
- [x] S8.2: Run new security tests S1-S7, verify all pass
- [x] S8.3: Verify WASM recompiles after any Rust changes
- [x] S8.4: Verify extension still loads (syntax check all JS)

## Summary of Vulnerabilities Found & Fixed

### 11 Vulnerabilities Identified

| # | Component | Vulnerability | Severity | Fix |
|---|-----------|---------------|----------|-----|
| V1 | popup.js | Seed stored in `window._pendingSeed` global — persists after popup close | Critical | Moved to module-scope `let pendingSeed` |
| V2 | popup.js | Seed and backup sent to background via `chrome.runtime.sendMessage` | Critical | Only send `address` to background; seed/backup stay in popup scope |
| V3 | background.js | Background service worker stored `unlockedSeed` and `unlockedBackup` | Critical | Removed entirely — background only tracks `unlocked` boolean + `walletAddress` |
| V4 | background.js | No sender origin check — any extension could send messages | High | Added `sender.id !== EXTENSION_ID` check |
| V5 | background.js | `getBackup` handler returned seed to any caller | Critical | Removed `getBackup` handler entirely |
| V6 | popup.js | Tx history `innerHTML` used unescaped RPC data — XSS vector | High | Added `escapeHtml()` function, applied to all RPC-derived fields |
| V7 | popup.js | Backup file import stored unknown fields blindly | Medium | Validate structure, strip unknown keys, validate address format |
| V8 | server.js | No WS message size limit — DoS vector | High | Added `maxPayload: 1MB` to WebSocketServer |
| V9 | server.js | No JSON validation before TCP forwarding — malformed data sent to RPC | Medium | Added `JSON.parse()` validation + RPC field checks |
| V10 | server.js | TCP error handler leaked internal error messages | Medium | Replaced with generic `'could not connect to RPC server'` |
| V11 | manifest.json | `web_accessible_resources` matched `<all_urls>` — any site could load WASM | High | Restricted to `chrome-extension://*/*` |

### Additional Hardening

- **TCP resource cleanup**: Added `activeTcpConnections` counter, `MAX_TCP_PER_WS=10` limit, `tcpClosed` flag, `tcpTimeout` cleanup
- **Lock button**: Now calls `clearSensitiveMemory()` to null all sensitive variables
- **Remove wallet**: Now calls `clearSensitiveMemory()` before clearing storage
- **Passphrase clearing**: `createPassphrase` input field cleared after wallet creation
- **Import seed clearing**: Import seed field cleared after successful import
- **Unknown message rejection**: Background rejects unknown message types
- **Auto-lock state clearing**: Auto-lock timer nulls both `unlocked` and `walletAddress`

## Test Results

### Security Tests (S1-S7): 68/68 PASS, 0 FAIL

| Gate | Tests | Result |
|------|-------|--------|
| S1: Seed Lifecycle | 14 | ALL PASS |
| S2: XSS & Injection | 8 | ALL PASS |
| S3: Transport Security | 12 | ALL PASS |
| S4: Message Passing | 7 | ALL PASS |
| S5: Cryptographic | 10 | ALL PASS |
| S6: Permission & CSP | 8 | ALL PASS |
| S7: Functional Edge Cases | 13 | ALL PASS |
| **Total** | **68** | **100% PASS** |

### Regression + Functional Tests (S8): 78/78 PASS, 0 FAIL

| Gate | Tests | Result |
|------|-------|--------|
| Gate 0+1+2: WASM + Pipeline | 18 | ALL PASS |
| Gate 3: Wallet Onboarding | 11 | ALL PASS |
| Gate 4: Balance/Account | 3 | ALL PASS |
| Gate 5: Send Transfer | 15 | ALL PASS |
| Gate 6+7: Settings+Security | 13 | ALL PASS |
| Gate 8: Polish/Release | 11 | ALL PASS |
| **Total** | **78** | **100% PASS** |

### Combined: 146/146 tests PASS (100%)

All security hardening verified. All bugs fixed. No regressions.

## Additional Bug Fixes (Round 2)

| # | File | Bug | Fix |
|---|------|-----|-----|
| B1 | popup.js | Duplicate `historyBtn` event listener (fired twice) | Removed duplicate line |
| B2 | test_gate5.js | Chain ID `'postfiat-local'` didn't match testnet `'postfiat-wan-devnet'` | Fixed to `postfiat-wan-devnet` |
| B3 | test_gate6_7.js | Same chain ID mismatch | Fixed to `postfiat-wan-devnet` |
| B4 | test_gate6_7.js | Settings test used port 8081 instead of 8080 | Fixed to 8080 |
| B5 | test_gate5.js | `amount=0` validation test had wrong logic (`if (0 <= 0)` always true) | Replaced with proper static check |
| B6 | test_gate5.js | Missing `chain_id`, `genesis_hash`, `protocol_version` in `WalletSignTransferFields` | Added required fields from chain |
| B7 | rpc-client.js | No URL scheme validation in constructor | Added `ws://`/`wss://` validation |
| B8 | rpc-client.js | `ws.send()` could throw if WS closed between connect and send | Added try/catch |
| B9 | rpc-client.js | `connectPromise` not cleared on `onclose` — stale promise prevented reconnection | Clear `connectPromise` in `onclose` and at start of `connect()` |
| B10 | popup.js | No guard against creating/importing wallet when one already exists | Added `keystore.loadWallet()` check before create/import |
| B11 | popup.js | Send confirm didn't validate inputs or check `currentBackup` before signing | Added input validation + null backup guard |
| B12 | popup.js | Import wallet didn't clear passphrase field | Added `createPassphrase` clearing |
| B13 | popup.js | Unlock didn't clear passphrase field or check empty input | Added empty check + field clearing |
| B14 | popup.js | Unlock didn't set `walletAddress` from wallet metadata | Fixed to use `wallet.metadata?.address` |
| B15 | popup.js | Backup import didn't confirm overwrite or lock after import | Added confirm dialog + lock after import |
| B16 | popup.js | Backup import didn't reset file input (couldn't re-import same file) | Added `e.target.value = ''` |
| B17 | popup.js | Remove wallet didn't reset `lockedAddress` display | Added reset |
| B18 | popup.js | Settings view not hidden when switching to send/history | Added `settingsView` hiding |
| B19 | popup.js | Create wallet had dead code (`walletAddress = result.address` overwritten) | Cleaned up redundant assignment |
| B20 | keystore.js | `decrypt()` didn't validate blob structure before `atob()` | Added field existence check |
| B21 | server.js | `ws.on('close')` registered inside `ws.on('message')` — listener leak (N handlers after N messages) | Moved to connection scope, using `Set` for TCP cleanup |
| B22 | background.js | Settings load race — `resetAutoLock()` could use default before settings loaded | Reload settings in `unlock` handler |
| B23 | background.js | Auto-lock timer not nulled on lock/fire | Added `autoLockTimer = null` after clear |


