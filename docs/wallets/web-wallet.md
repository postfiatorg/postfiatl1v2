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
| `WALLET_PROXY_NATIVE_SESSION_ORIGIN` | empty | Exact `https://host[:port]` origin native apps use. Unset disables native sessions (the open and info routes return 404). |
| `WALLET_PROXY_NATIVE_SESSION_SECRET` | empty | Token HMAC key, at least 32 random bytes. Required when native sessions are enabled. Unique per gateway. |
| `WALLET_PROXY_NATIVE_SESSION_CHAIN_ID` | empty | Network id native sessions are bound to, such as `postfiat-wan-devnet-2`. Required when enabled. |
| `WALLET_PROXY_NATIVE_SESSION_GENESIS_HASH` | empty | 96-hex genesis hash native sessions are bound to. Required when enabled. |
| `WALLET_PROXY_NATIVE_SESSION_TTL_MS` | `900000` | Native token lifetime, 60000 to 3600000. |
| `WALLET_PROXY_NATIVE_SESSION_ALLOWLIST_FILE` | empty | Optional install allowlist. When set, only listed installs may open a session. Re-read when the file changes. |
| `WALLET_PROXY_NATIVE_SESSION_DENYLIST_FILE` | empty | Optional install denylist. Listed installs are refused on open and on every token use. Re-read when the file changes. |
| `WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS` | `256` | Installs that may hold an unexpired token at once, 1 to 1000000. A new install past it gets 503 `native_session_busy`; a live install can renew. 256 is 16 installs per slot of the default mutation concurrency; past that the shared slots, not the per-install windows, bound throughput. |
| `WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_LIMIT` | `10` | Session opens admitted per source address and window, 1 to 100000. Further opens get 429 `native_session_rate_limited` with `retry_after_ms`. |
| `WALLET_PROXY_NATIVE_SESSION_OPEN_RATE_WINDOW_MS` | `60000` | Open rate window, 1000 to 86400000. |
| `WALLET_PROXY_PRIVATE_ACTIONS_CHAIN_ID` | empty | Chain id the private action routes bind to. With the genesis hash unset as well, the routes return 404. |
| `WALLET_PROXY_PRIVATE_ACTIONS_GENESIS_HASH` | empty | 96-hex genesis hash the routes bind to. Required with the chain id. |
| `WALLET_PROXY_PRIVATE_ACTIONS_PROTOCOL_VERSION` | `1` | Protocol version in the intent domain. |
| `WALLET_PROXY_PRIVATE_ACTIONS_POOL_ID` | `asset-orchard-v1` | The only pool an action may name. |
| `WALLET_PROXY_PRIVATE_ACTIONS_BASE_ASSET_ID` | empty | 96-hex base asset of the pair; with the quote asset, required for `snapshot` and the only assets ingress and egress may use. |
| `WALLET_PROXY_PRIVATE_ACTIONS_QUOTE_ASSET_ID` | empty | 96-hex quote asset of the pair. |
| `WALLET_PROXY_PRIVATE_ACTIONS_RPC_SOURCES` | all of `RPC_FLEET` | Two or more `validator-id=host:port` sources. A submission goes to every source, because a node holds a record until it proposes; answers are reconciled across all of them. |
| `WALLET_PROXY_PRIVATE_ACTIONS_STORE` | `~/.local/share/postfiat/wallet-proxy/private-actions` | Directory of the durable intent records and their dedupe indexes. Must be writable. |
| `WALLET_PROXY_PRIVATE_ACTIONS_RPC_TIMEOUT_MS` | `15000` | Timeout of each validator read or submission. |
| `WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION` | `off` | `off` (the default) retires the proxy's legacy shielded certification: submissions take the validator-owned admission at `/api/private/v1/`, the legacy ingress, swap and egress routes answer 410 `legacy_shielded_certification_retired`, and no validator key file, data directory or local Asset-Orchard service is read. `on` keeps the legacy path for one release. |
| `WALLET_STATIC_DIR` | `wallet-web/dist` | Immutable production wallet build served same-origin by the hardened proxy. |
| `INJECT_RPC_CAPS` | `true` | Injects RPC capability fields into `server_info` responses. Like every wallet-proxy on/off setting, it takes `true`/`false` (any case) or `1`/`0`; any other value stops startup, or fails the request for a setting read per request (`<NAME> must be true or false`). Exception: `PFUSDC_WITHDRAWAL_ENABLED` is on only for exactly `true`. |

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

### Native app sessions

Native apps cannot use the loopback-only `GET /api/bridge/local-session`. They
call `POST /api/native-session/v1/open` with a per-install ML-DSA-65 public key,
a 32-byte nonce, the gateway origin, chain id, genesis hash and a timestamp,
signed under the context `postfiat-l1-v2/native-session/v1`. The gateway checks
the signature, origin, network, timestamp (±5 minutes), nonce reuse and a 16-KiB
body limit. It returns a bearer token for the principal `native:<install id>`.
The signed bytes are the newline-joined schema, origin, chain id, genesis hash,
public key, nonce and timestamp. `wallet-proxy/fixtures/native-session-rust-vector.json`
is a request signed by `crates/crypto_provider`
(`examples/native_session_vector.rs`), and the proxy tests open a session with it.

Opens are limited per source address before any signature check (10 per minute
by default). Behind the bundled TLS edge every request arrives from the edge, so
the limit applies to all native clients together; size it for that.
`docker-compose.wallet-public.yml` passes the native-session variables through
unset, so native sessions stay off until an operator sets them. An allowlist
file must also be mounted into the container.

The token has the browser relay's caps: the same routes, per-principal rate
window, concurrency and body limits. It is refused when expired (401
`native_session_expired`), minted for another origin or network (403), signed by
another gateway's secret (401), minted before the gateway last restarted (401
`native_session_restarted`), or sent with a foreign `Origin` header (403).
On either 401 code the wallet opens a new session.
Any request that carries a token in its query string gets 400. Add the native
origin to `ALLOWED_ORIGINS`, because authenticated writes still need an allowed
`Origin` header. WebSocket calls carry the token as `proxy_auth_token`, with the
same origin binding.

The allowlist and denylist files must each be a regular file owned by the
service user, not a symlink, not group- or world-writable and no larger than
4 MiB (4194304 bytes, about 1,070 allowlisted keys or 58,000 denylisted ids).
The size is checked before the file is read; a larger file counts as a bad file:

- `{"schema":"postfiat-native-session-allowlist-v1","install_public_keys_hex":["<3904 hex>"]}`
- `{"schema":"postfiat-native-session-denylist-v1","install_ids_hex":["<64 hex>"]}`,
  where an install id is the `install_id` the open response returns. Every
  element must be a lowercase hex string of that length.

The gateway reads and hashes each file at most once per second and loads new
contents, so revoking an install needs no restart. A denied install, or one
taken off the allowlist, gets 403 `native_session_install_revoked` on open and
on every request with its token, and its live-install slot is freed at once. An
install never listed gets 403 `native_session_install_not_allowed` on open. A bad
file stops the gateway at startup; after startup it keeps the last good list,
retries on each check and logs each distinct error once. Rotating the secret
ends all native sessions.

The live-install count behind `WALLET_PROXY_NATIVE_SESSION_MAX_ACTIVE_INSTALLS`
is in memory. Each token carries the gateway's boot generation, so a restart
ends every native session and each install takes a slot again by opening. An
install revoked and then listed again needs a free slot before its token works
again (503 `native_session_busy`).

Each open writes one JSON line to stdout:
`{"event":"native_session_open","install_id_prefix":"<16 hex>","source":"<address>","outcome":"ok"}`.
The outcome is `ok` or the refusal code. The prefix is `null` until the
signature is verified. The line never holds the token, the signature or the
secret. Behind the TLS edge the source is the edge's address.

`GET /api/native-session/v1/info` returns `ok`, `schema`
(`postfiat-native-session-info-v1`), `enabled`, `origin`, `chain_id`,
`genesis_hash_hex`, `ttl_ms`, `allowlist` (whether admission is limited to
listed installs) and `max_active_installs`. It returns no secret and no list
contents. With native sessions disabled it returns 404 `native_session_disabled`,
like the open route. It is a plain HTTP read answered before the proxy's auth
layer, like `/healthz`, not an RPC method.

Switching native sessions on remains an operator decision: the secret, a TLS
certificate the app trusts for the origin, the chain id and genesis hash, the
TTL, allowlist or open admission, and the install cap.

### Private actions

`POST /api/private/v1/snapshot`, `/submit` and `/receipt` serve the native app's
user-proven shielded actions (W-1). They take a browser relay or native session
token; `submit` counts against the mutation rate window. The contract is in
`crates/node/tests/fixtures/private-actions-v1/`.

- **Submit** recomputes the intent id, reserves the intent durably once, and
  relays the exact action text to every RPC source through the node methods
  `private_action_submit` and `private_action_receipt`. The gateway holds no
  key and reads no validator data directory.
- **Snapshot** pins one finalized token on every source (a mixed tip is 503),
  then pages every section under the wallet's ceilings: 4096 commitments,
  nullifiers or anchors, 64 ciphertexts, 2 MiB of ciphertext, 16 assets and a
  4 MiB response. If the tip moves while paging it retries once, then answers 503. A
  section stopped at a ceiling or reported incomplete by the node has its
  `complete` flag false.

The WebSocket RPC serves `private_readiness`, `private_pool_snapshot`,
`private_pool_page`, `asset_orchard_private_egress_binding` and
`external_lane_info` as public reads, while the `/api/private/v1/` routes,
`private_action_submit`, `private_action_receipt`, `private_account_lines`,
`private_pool_receipts` and validator signing need authentication.

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


### Retiring the legacy shielded certification

The browser wallet's shielded swaps still use the proxy's legacy path. That
path runs `transport-peer-certified-batch-round` with a validator key file,
calls the local Asset-Orchard `/asset-orchard/swap-batch`, and certifies the
ingress, swap and egress on the HTTP side. `WALLET_PROXY_LEGACY_SHIELDED_CERTIFICATION`
controls it. The default is `off`; `docker-compose.wallet-public.yml` sets `on`
until the app moves. The proxy logs the active mode at startup, and
`GET /api/shielded-nav-swap/status` reports it as `certification_path.active`:
`validator_owned_admission` or `legacy_http_certification`.

- **Off (default):** `/api/shielded-nav-swap/ingress`, `/swap` and `/egress`
  answer 410 `legacy_shielded_certification_retired`, pointing to
  `/api/private/v1/`. The legacy entry points themselves (the certified round
  and loop with `--key-file`, the `/asset-orchard/swap-batch` call and the three
  executions) refuse with the same code. The proxy starts with no key file,
  data directory or local Asset-Orchard service.
- **On:** if `NAVSWAP_ENABLE_SHIELDED_INGRESS`, `_SWAPS` or `_EGRESS` is set but
  the validator key file or data directory is missing, the proxy still starts.
  It logs one warning naming what is missing, and those routes answer 409 as
  before. Only an invalid switch value stops startup.

The cutover:

1. Move the apps to `/api/private/v1/`.
2. Set the switch `off` on the public deployment (the proxy's default is already `off`).
3. Remove the legacy code in a later release.

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
| `owned_objects` | `owner_public_key_hex`, optional `asset`, `limit`, `cursor`, `omit_owner_keys` | Read, one page; follow `next_cursor` (see [Owned objects pages](../rpc/methods.md#owned-objects-pages)). |
| `owned_recovery_capabilities` | none | Read active v3 FastPay committee/domain/windows. |
| `owned_certificate` | `certificate_digest` or `lock_id` | Read persisted recovery evidence. |
| `owned_recovery_status` | `lock_id` | Read ordered recovery state. |
| `owned_fence_status` | `lock_id` | Read; the gateway returns every validator's signed fence status (see [FastPay Terminal Proofs](#fastpay-terminal-proofs)). |
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

## FastPay Committee Pin

A wallet that pins the FastPay committee and policy, as the Mac wallet's
release build must before FastPay opens, takes the pin from consensus state.
`postfiat-node fastpay-admission-export --data-dir DIR --out FILE` writes
`{"committee": …, "policy": …}`, the committee admitting new orders and the
committed policy, after checking them against `owned_recovery_capabilities`
(domain, epoch, validator count, quorum, policy); it refuses on any difference.
The public read `fastpay_recovery_committee` returns the same committee with
`chain_id`, `genesis_hash` and the height read, so a wallet can compare its pin
with live keys and fail closed on drift. Reviewing the file and building it
into a signed app is the release owner's step. The pin is valid only while
its epoch admits orders (`new_orders_through_height`); every committee
rotation needs a fresh export, review and build.

## FastPay Terminal Proofs

`owned_fence_status` lets a wallet prove, without trusting the gateway or one
node, that an unwrap or transfer is ordered (anchored by a certified block) or
that its lock was cancelled by ordered recovery. Each validator signs a
`FastPayFenceStatusV2` from its certified blocks only, under its own context
`postfiat-l1-v2/fastpay-fence-status/v2`: the decision, the `fence_digest`
(equal to the apply acknowledgement's `terminal_state_digest`),
`anchored_at_or_before_height` and the `observed_height` it answered at.
`anchored_at_or_before_height` is the lowest certified height at which that
validator can show the effect anchored: the anchoring block's height while it
holds that block, or its history checkpoint's height after a prune or a
checkpoint restore, so honest validators may sign different values. (v1 signed
it as an exact `anchored_height` and refused honest pruned validators as
conflicting; v1 was never merged.) A speculative apply, a mempool decision or
an effect that a certified block rolled back is answered, and signed, as "not
terminal at `observed_height`". The gateway asks every validator and returns
each answer with its validator and `answered_count`; a partial answer is
marked `partial`. It never merges them. It takes a connection permit per read,
shares one fan-out between identical reads in flight, and answers
`proxy_fence_status_busy` (with `retry_after_ms`) above
`FASTPAY_FENCE_STATUS_MAX_FANOUTS_PER_CALLER` (default 2 per connection) or
`FASTPAY_FENCE_STATUS_MAX_FANOUTS` (default 16) fan-outs in flight. Because
each new fan-out costs every validator a signature, it also spends one token
of its source address's budget and one of the gateway's, and is answered
`proxy_fence_status_busy` with the wait for the next token when either is
empty. A budget holds `FASTPAY_FENCE_STATUS_SOURCE_BURST` (default 8) tokens
per address and regains one every `FASTPAY_FENCE_STATUS_SOURCE_REFILL_MS`
(default 2000), and `FASTPAY_FENCE_STATUS_GLOBAL_BURST` (default 64) and
`FASTPAY_FENCE_STATUS_GLOBAL_REFILL_MS` (default 250) for the gateway. A read
joining a fan-out in flight spends nothing; the gateway caches no answer. The
source address is the socket's peer address; the gateway trusts no
forwarding header, so behind a reverse proxy every client shares the proxy's
budget until a trusted proxy header rule is added and configured.

The wallet must, before releasing a reservation after a cancelled fence or
showing an unwrap credit as ordered:

- pin the registry root of the order's committee in its own configuration
  (the root commits to chain, genesis, protocol version, epoch and members)
  and pass it as the required `pinned_registry_root`; a committee read with
  `fastpay_recovery_committee` through the gateway is only a candidate;
- validate that committee completely: its registry root recomputed from the
  members, the protocol quorum for its member count, its schema, order range
  and canonical member order (`invalid_committee`), then its root against the
  pinned one (`untrusted_committee`);
- check each status on its own (shape, lock, domain, epoch and root,
  membership, ML-DSA-65 signature against that committee's keys); a status
  that fails is listed in the result's `invalid` with its `validator_id` and
  reason and counts for nothing, without refusing the others;
- require the committee's `quorum` of distinct members with the same
  decision, operation and `fence_digest`; anchored and observed heights may
  differ. The proved `anchored_at_or_before_height` is the highest in that
  quorum, the height by which the quorum agrees the effect was anchored.

The rule assumes at most the committee's fault bound, `(n - 1) / 3` of `n`
members, is faulty, so a faulty minority can neither prove an answer nor veto
one. A "not terminal" status contradicts a terminal quorum only when observed
at or above the quorum-supported bound, the `(f + 1)`-th smallest
`anchored_at_or_before_height` of its distinct signers (`f = (n - 1) / 3`): at
least one honest signer claims a bound at or below it, so up to `f` false
bounds, low or high, cannot move it below the true anchoring height.
Likewise a terminal status contradicts a "not terminal" quorum only when
anchored at or below the quorum-supported height, the `(f + 1)`-th largest
`observed_height` of its distinct signers, so up to `f` false high heights
cannot move it above what an honest signer observed. A valid status that
contradicts the proved answer (another terminal result, or such a "not
terminal" or terminal status) is returned unchanged in `contradicting`; a
signer whose own bound is below the quorum-supported one, or whose observed
height is above it, is returned in `inconsistent_bounds`. An entry there can
be an honest member whose bound or observed height is simply tighter than the
quorum's `(f + 1)`-th statistic, so the list is a note for operators, never a
finding against the member.
From heights the verifier concludes only this: a "not terminal" below that
bound (or a terminal status anchored above that height) is compatible
history, and one on the other side conflicts with an honest signer's claim.
It attributes nothing from a single member's height: no status is judged
against one signer's bound or observed height, and an inconsistent one may be
a lie or an honest value, so it is neither counted toward a refusal nor used
to name the others. The wallet acts on the proved answer and logs
`contradicting`, `inconsistent_bounds` and `invalid` for operators as signed
evidence to investigate. Only when the contradicting statuses also come from
a quorum of distinct members (more faulty members than the bound) is the set
refused as `conflicting`; with no answer at quorum it is `below_quorum`.

The Rust check is `postfiat_execution::verify_fastpay_fence_status_quorum_ml_dsa_v2`
(rules in `postfiat_types::verify_fastpay_fence_status_quorum_v2`; for statuses
as received, `verify_fastpay_fence_status_quorum_v2_json` parses each with
`FastPayFenceStatusV2::from_json_strict` and reports one that does not parse
as `malformed`); the Python check is
`postfiat_rpc.fastpay_fence_status.verify_fence_status_quorum`, with an
ML-DSA verifier supplied by the caller. Both run the same acceptance vectors,
`crates/types/test-vectors/fastpay_fence_status_acceptance_v2.json`. A
validator whose rpc-serve signs through the signer socket answers
`fastpay_fence_status_signer_unavailable` and does not count.

The limitation is FastPay-wide, not this read alone: no FastPay transfer or unwrap vote, signed apply acknowledgement or fence status can be signed on the two-user signer shape: rpc-serve signs all of them with the validator key file (`sign_for_fastpay_committee`, `crates/node/src/fastpay_recovery_node.rs`; `crates/node/src/consensus_artifacts.rs`), which it cannot read there, and the signer process signs only consensus and transport contexts. Enough validators on that shape and FastPay collects no vote or acknowledgement quorum at all, not only no terminal-status quorum. This lasts until the signer process gets a design that lets it check certified-state evidence itself; a key copy readable by rpc-serve would make the old paths work and defeat the isolation.

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

The live wallet feed (`wallet_subscribe`) reads one `owned_objects` page per
validator per tick and marks it `owned_complete` (`false` when truncated). The
wallet selects only from a complete inventory: a complete feed answer replaces
it; a partial page never does. When the partial page's `view_digest` equals the
last complete read the inventory stands, otherwise the wallet reads every page
(`fetchOwnedObjectsSnapshot`) and publishes the result in one step. When that
read fails, the wallet shows why, either the node is too old for paging or the
objects kept changing between pages (a busy account, or validators serving
different views), with a Retry button; partial feed pages neither clear that
error nor retry on their own.

## FastPay Standard Unwrap

Default unwrap is amount-based and certified. The wallet no longer unwraps a
whole object through `unwrap_owned(object_id, owner_pubkey_hex, to_address)`.
That RPC shape does not prove ownership of the private key and is disabled for
public wallet flows.

The current unwrap flow is:

1. Load the FastPay object snapshot with `owned_objects`
   (`fetchOwnedObjectsSnapshot`): `omit_owner_keys`, every page through
   `next_cursor`, and a restart from the first page (at most 3) on
   `owned_objects_view_changed` or another `view_digest`. A truncated answer
   without `next_cursor` (a node from before paging) or a view that keeps
   changing is an error, never a partial list. Selection uses every page.
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
2. Sign with `wallet_sign_transfer(backup_json, quote_json, intent_json)`.
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
