# Wallet, proxy, and RPC SDK review — 2026-09-10

Scope: the shipped browser wallet, maintained Chrome extension, wallet proxy,
Rust RPC SDK, and wallet WASM bindings. This was a fresh-eyes adversarial review
of authentication, reviewed-intent binding, response validation, custody, and
resource admission. It is not a deployment authorization.

## Findings

### 1. P1 — Transfer quote signing trusts an RPC-selected recipient and amount

**Location:** `crates/rpc_sdk/src/wallet_sdk.rs:43`,
`crates/wallet_wasm/src/lib.rs:258`,
`wallet-web/src/lib/tx-builder.js:435`, and
`wallet-extension/popup/popup.js:303`.

`wallet_sign_transfer_from_quote` checks only that the quote sender matches
the wallet. It then signs the quote's `to` and `amount`, without binding them
to the request or locally reviewed values. The Rust CLI and WASM wrapper expose
that unbound signer directly. Both browser transfer paths pass the RPC result
to it.

**Concrete failure scenario:** a compromised or misrouted RPC receives a
request to transfer 100 atoms to account A and returns a structurally valid
quote for the same sender, sequence, and domain but 10,000 atoms to account B.
The signer accepts the sender match and produces a valid transaction to B.
The existing Rust test named
`wallet_sdk_creates_identity_and_signs_quoted_transfer_without_key_file`
also demonstrates that recipient and amount are sourced from the quote; no
request-intent mismatch test existed.

### 2. P1 — The loopback session token endpoint accepts a DNS-rebinding Host

**Location:** `wallet-proxy/server.js:203`.

The token-returning `/api/bridge/local-session` endpoint requires a loopback
peer and `Sec-Fetch-Site: same-origin`, but does not bind `Host` or
`X-Forwarded-Host` to a configured wallet origin. Fetch Metadata describes
the browser's initiating relationship, not the authority that resolved to the
listener.

**Concrete failure scenario:** JavaScript loaded from
`https://attacker.example` rebinds that hostname to `127.0.0.1` and requests
`https://attacker.example/api/bridge/local-session`. The browser can mark the
request same-origin, the socket peer is loopback, and the current predicate
returns true for a mock request with `Host: attacker.example`. The response
contains the bearer token, which then authorizes wallet mutations.

### 3. P2 — WebSocket mutation admission releases the process-wide slot before work starts

**Location:** `wallet-proxy/server.js:1382`.

After authenticating a WebSocket mutation, the proxy acquires the shared
mutation admission and immediately calls `release()`. Endpoint resolution,
broadcast work, and upstream TCP I/O all happen later. The per-WebSocket TCP
counter cannot enforce the documented process-wide limit across separate
connections.

**Concrete failure scenario:** one authenticated principal opens many WebSocket
connections and submits one slow mutation on each. Every message observes a
free shared slot because the preceding message released it before I/O. All
connections then execute concurrently, exceeding
`WALLET_PROXY_MUTATION_CONCURRENCY` and amplifying upstream load.

### 4. P2 — The maintained extension permits cheaply brute-forced new vaults

**Location:** `wallet-extension/popup/popup.js:126`,
`wallet-extension/popup/popup.js:190`, and
`wallet-extension/lib/keystore.js:5`.

The extension accepts a four-character passphrase and derives its AES-GCM key
with 100,000 PBKDF2-SHA256 iterations. The maintained web-wallet path already
requires ten characters and uses 310,000 iterations.

**Concrete failure scenario:** an attacker copies `chrome.storage.local` from
a browser profile whose user selected the permitted four-character
passphrase. Salt and ciphertext are available offline, and authenticated
AES-GCM gives an oracle for exhaustive passphrase recovery. The recovered
plaintext is the wallet master seed.

### 5. P2 — The extension popup is invalid as a browser module

**Location:** `wallet-extension/popup/popup.js:90` and
`wallet-proxy/test_full_pipeline.js:194`.

The popup contained an unmatched closing brace. Its HTML loads the script with
`type="module"`, so Chrome rejects the file before wallet initialization.
The regression gate used `node --check FILE`, whose package-based source
detection parsed these un-packaged files differently and missed the module
failure.

**Concrete failure scenario:** loading the maintained unpacked extension opens
a popup whose module fails at parse time with `Unexpected token '}'`; no
wallet action is available. Piping the same file through
`node --input-type=module --check` reproduces the failure. The repair removes
the unmatched brace and makes the gate force browser-module parsing.

## Areas reviewed without P1/P2 findings

- RPC response envelopes enforce typed shapes, IDs, chain-domain fields, bounds,
  and private-key leak rejection.
- Payment-v2 and asset/escrow signing paths construct the reviewed operation
  locally or compare it before signing; atomic swaps already bind the exact
  request and quote.
- Browser custody material remains module-local, encrypted at rest, excluded
  from proxy requests, and cleared on lock.
- WebSocket response correlation and client timeouts reject duplicate or
  mismatched response IDs.

## Pre-repair verification

- `npm test --prefix wallet-proxy`: **35/35 passed**.
- `npm test --prefix wallet-web`: **259 passed, 0 failed**.
- `cargo test --locked -p postfiat-rpc-sdk -p postfiat-wallet-wasm`:
  **69 passed, 0 failed** across library, binary, and WASM tests.

These green baselines did not exercise the five adversarial cases above. The
repair unit adds focused regressions and leaves deployed services untouched.

## Repair

The repair closes all five findings without changing a live service or wallet:

- The Rust quote signer now requires the original typed
  `transfer_fee_quote` request and compares sender, recipient, amount, and an
  explicit sequence before signing. The CLI requires both request and response
  files and rejects an ID mismatch. Python and TCP example callers retain the
  exact request.
- Both browser applications reject a transfer quote unless its sender,
  recipient, and amount equal the locally reviewed intent before invoking the
  low-level WASM quote-field signer.
- Local-session issuance accepts only loopback authorities or an authority
  named by the exact origin allowlist, in addition to its loopback-peer and
  Fetch-Metadata checks.
- WebSocket mutation admission is held until broadcast or upstream TCP work
  completes, including error paths. A two-connection regression holds one
  upstream request and proves the second is rejected at a process-wide limit of
  one.
- New extension vaults require at least ten passphrase characters, use 310,000
  PBKDF2-SHA256 iterations, and record the KDF work factor. Existing unversioned
  100,000-iteration vaults remain readable; this compatibility does not claim
  that an old weak passphrase has been strengthened.
- The unmatched popup brace is removed, and the extension skeleton gate now
  parses every JavaScript file explicitly as a browser module.

## Post-repair verification

- `npm test --prefix wallet-proxy`: **36/36 passed**, including the new
  WebSocket lifecycle regression and the 18/18 browser-extension pipeline.
- `npm test --prefix wallet-web`: **260 passed, 0 failed**.
- `npm test --prefix wallet-extension`: **2 passed, 0 failed**.
- `PYTHONPATH=python python3 -m pytest -q python/tests/test_wallet.py
  python/tests/test_latency.py`: **79 passed**.
- `cargo test --locked -p postfiat-rpc-sdk -p postfiat-wallet-wasm`:
  **69 passed, 0 failed**.
- `cargo test --locked -p postfiat-node --test atomic_swap_local_six --no-run`:
  compile pass for the request-bound SDK call site.
- Strict Clippy for the RPC SDK and wallet WASM, including all targets: pass.
