# Production Lightning/NAVcoin wallet edge

This is the only supported browser-serving path for the real-value demo.
It serves a reviewed `wallet-web` production build on loopback. It does not
run Vite, HMR, or a development server.

The edge has two narrow same-origin routes:

- `/api/lightning-navcoin/*` proxies to the loopback coordinator and injects
  the coordinator's 32-byte API token server-side. The token is never returned
  to JavaScript.
- `/rpc` proxies WebSocket JSON to the loopback PFTL wallet proxy. It
  overwrites any browser-supplied proxy token with an owner-only server token
  and permits only the RPC methods required by the Lightning escrow UI:
  `status`, `server_info`, `escrow_info`, `escrow_fee_quote`,
  `mempool_submit_signed_escrow_transaction_finality`, and `receipts`.

The coordinator remains responsible for its durable state machine, signed
operator permits, exposure caps, and PFTL/LND gates. This UI edge adds no
value-moving authority of its own.

## Release and run

After the combined source is committed, first refresh the coordinator's clean
source pin with `scripts/lightning-navcoin-mainnet-coordinator bootstrap`.
Then:

```bash
scripts/lightning-navcoin-mainnet-ui bootstrap
scripts/lightning-navcoin-mainnet-ui release
```

`release` performs a lockfile build and prints a `manifest_path` and
`manifest_sha256`. Review and record both. Serving requires the expected hash
explicitly; it is not learned from the manifest:

```bash
export PFTL_LN_UI_MANIFEST_PATH=/absolute/reviewed/manifest.json
export PFTL_LN_UI_MANIFEST_SHA256=<reviewed-64-hex-digest>
scripts/lightning-navcoin-mainnet-ui serve
```

The default origin is `http://127.0.0.1:18832`. The coordinator must use that
exact `--allowed-origin`, and the upstream wallet proxy must include that exact
origin in `ALLOWED_ORIGINS`.

The PFTL wallet proxy token defaults to the owner-only,
non-whitespace ASCII file:

```text
/home/postfiat/.pft/lightning-navcoin-mainnet/coordinator-secrets/wallet-proxy.token
```

Configure the same token as a principal on the upstream wallet proxy. The
coordinator token is read as raw 32 bytes from its existing
`api-session.token`. Both files must be regular, non-symlink, owner-owned mode
`0600` files.

Optional loopback overrides are:

```text
PFTL_LN_UI_PORT
PFTL_LN_COORDINATOR_URL
PFTL_LN_PFTL_PROXY_URL
PFTL_LN_PFTL_PROXY_TOKEN_FILE
```

The server rejects non-loopback upstreams and binds. It also rejects Host
rebinding and non-exact browser origins.

## Release guarantees

The release command fails unless the coordinator source pin matches clean
`HEAD`, including `tools/lightning_navcoin_demo`, `wallet-web`, and the UI
launcher. Its manifest pins:

- source commit and tree;
- wallet package-lock SHA-256;
- production UI server SHA-256;
- exact `ws` 8.21.1 runtime tree SHA-256;
- every dist file's path, size, and SHA-256; and
- a canonical dist tree SHA-256.

Serve requires an operator-supplied manifest SHA-256, re-verifies all pins,
rejects symlinks, traversal, source maps and Vite development artifacts, then
loads the verified bytes into memory. Disk changes after startup cannot change
the bytes served by that process.

Before Node loads the UI server or `ws`, the shell launcher runs an independent
standard-library verifier over the operator-pinned manifest, server bytes,
wallet lockfile, and complete installed `ws` tree. It atomically copies the
verified server and `ws` runtime into the owner-only artifact directory and
executes those copies. The server then repeats the checks in-process.

Every HTTP response carries a real CSP header with
`frame-ancestors 'none'`, `X-Frame-Options: DENY`, `nosniff`, a restrictive
Permissions Policy, and same-origin opener/resource policies. Content-hashed
assets are immutable-cacheable; HTML, API, errors, and health are `no-store`.

Run the isolated suite with:

```bash
cd tools/lightning_navcoin_demo/mainnet/ui-server
npm ci --ignore-scripts --no-audit --no-fund
npm test
```

These commands do not initialize LND, request liquidity, create invoices,
submit PFTL transactions, or move value.
