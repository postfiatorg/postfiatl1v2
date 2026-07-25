# Real-value coordinator process

This process keeps the executable claim bounded to:

> non-custodial, conditionally-atomic, COORDINATOR-TRUSTED timing

It does not create an LND wallet, acquire liquidity, sign a nazgul permit, or
alter PFTL consensus.

## Reproducible local runtime

The launcher installs only hash-pinned wheels into an owner-only virtual
environment and exports the exact LND v0.20.1 generated modules from the
pinned gRPC image:

```bash
scripts/lightning-navcoin-mainnet-coordinator bootstrap
scripts/lightning-navcoin-mainnet-coordinator prepare
```

`prepare` creates a 0700 coordinator state hierarchy, a mode-0600 Ed25519
quote seed, and a mode-0600 API session token. It prints the quote public key
and token SHA-256, never either secret.

The operator then installs these strict JSON files under
`$PFTL_LN_MAINNET_STATE_DIR/coordinator-config/`:

- `policy.json`: DRY_RUN or ARMED identity/budget policy, including the exact
  LND public key and PFTL handoff/build/NAV pins;
- `btc-price.json`: fresh, operator-reviewed integer BTC/USD observation;
- `lnd-connection.json`: loopback endpoint and SHA-256 pins for the TLS
  certificate and first-release receive-only macaroon.

Each file must be regular, non-symlink, bounded ASCII JSON and not
group/world writable. The quote public key in the policy must match the
private seed. The persistent PFTL handoff, asset precision, user address, and
binary revision must all match independently.

Because the current PFTL RPC exposes the raw NAV integer without its unit,
every pricing/capacity route read is also bound to the six validator
`ledger.json` and `chain_tip.json` files pinned by the handoff. All six local
tips must equal the six-RPC height/tip/root, and all six asset records must
independently agree on `valuation_unit=usd_1e8`, the exact raw NAV, epoch,
reserve packet, supply, issuer, and proof profile. A USDC-e6 interpretation,
stale ledger, divergent tip, or rescaled integer puts the process on HOLD.

`bootstrap` also records the current Git commit and tree plus cleanliness of
the coordinator, RPC client, launch scripts, and the entire `wallet-web`
source (ignored `node_modules`/`dist` are not execution inputs). ARMED startup
fails until those paths equal that clean commit exactly. Run `bootstrap`
again after the final reviewed commit; a dirty worktree is usable for
DRY_RUN development only.

## No-spend check

Use a policy whose mode is literally `DRY_RUN`:

```bash
scripts/lightning-navcoin-mainnet-coordinator dry-check
```

DRY_RUN rejects an arming acknowledgement or signer handle. It reads the
pinned LND/PFTL views and durable budget but cannot load a PFTL signer or
expose a payable invoice.

The same DRY_RUN policy can serve the loopback API. Use the exact production
wallet origin even for the no-spend rehearsal:

```bash
scripts/lightning-navcoin-mainnet-coordinator serve \
  --allowed-origin http://127.0.0.1:18832
```

For the controlled demo interface, use only the immutable production UI
launcher. Vite development/HMR and `vite preview` are not real-value paths.
After the combined source is committed, refresh the clean coordinator source
pin, install the exact UI runtime, and build a release:

```bash
scripts/lightning-navcoin-mainnet-coordinator bootstrap
scripts/lightning-navcoin-mainnet-ui bootstrap
scripts/lightning-navcoin-mainnet-ui release
```

The release command prints `manifest_path` and `manifest_sha256`. Record and
review both. Provision the PFTL wallet-proxy dispatch token as an owner-only,
non-symlink, mode-0600 ASCII file (default:
`coordinator-secrets/wallet-proxy.token`) and configure the upstream loopback
wallet proxy with that same token and
`ALLOWED_ORIGINS=http://127.0.0.1:18832`. Then serve only the reviewed release:

```bash
export PFTL_LN_UI_MANIFEST_PATH=/absolute/reviewed/manifest.json
export PFTL_LN_UI_MANIFEST_SHA256=<reviewed-64-lowercase-hex>
scripts/lightning-navcoin-mainnet-ui serve
```

The UI binds only `127.0.0.1:18832`. Before Node loads the server or WebSocket
runtime, the launcher independently verifies the operator-pinned manifest,
server, wallet lockfile, and exact `ws` runtime tree, then executes an
owner-only copy of the verified server and runtime. The server repeats the
checks, serves only manifest-hashed production bytes from memory, injects
both local credentials server-side, and restricts `/rpc` to the escrow
methods used by this interface. Browser responses carry a real CSP header
including `frame-ancestors 'none'` plus `X-Frame-Options: DENY`.

## ARMED process

ARMED mode requires both an exact acknowledgement and an explicit,
owner-only PFTL signer handle:

```bash
scripts/lightning-navcoin-mainnet-coordinator serve \
  --allowed-origin http://127.0.0.1:18832 \
  --armed-ack I_ACKNOWLEDGE_REAL_BTC_AND_PFTL_VALUE \
  --pftl-signer-handle /absolute/owner-only/signer.json
```

This only enables the signer boundary. Every swap still needs a fresh
nazgul-signed, single-use permit bound to the quote, policy, direction, cost
ceiling, and expiry. The authorization private key is never loaded by the
coordinator and the browser API has no authorization route.

The graduation release enables new ARMED `lightning_to_pftl` swaps only.
`pftl_to_lightning` remains visibly HOLD until its independent
refund/cadence drill passes. Recovery of an outgoing payment that was already
durably attempted remains enabled, because suppressing reconciliation would
increase loss risk.

Send an already-signed public permit through the owner-only, mode-0600 Unix
socket:

```bash
scripts/lightning-navcoin-mainnet-coordinator authorize \
  --swap-id 64-lowercase-hex-characters \
  --authorization /absolute/path/to/signed-permit.json
```

## Manual liquidity cost accounting

The coordinator has no LSP create-order or payment command. Before an operator
manually creates any external liquidity order, reserve its maximum cost from
the same durable `$5/run`, `$20/lifetime` budget:

```bash
chmod 600 /absolute/path/to/liquidity-setup-permit.json
scripts/lightning-navcoin-mainnet-coordinator liquidity-reserve \
  --authorization /absolute/path/to/liquidity-setup-permit.json
```

This command requires an ARMED policy, the exact clean source-release commit
and tree, a fresh operator-reviewed BTC/USD observation, and a valid
nazgul-signed permit with literal `category=LIQUIDITY_SETUP` and
`direction=not_applicable`. The signed msat ceiling is conservatively converted
at that price and must fit inside its signed USD ceiling. The command only
writes `RESERVED` to SQLite; it does not connect to an LSP, create an order,
pay an invoice, load a PFTL signer, or move value.

The external payment must be initiated before the signed permit's
liquidity-specific expiry, which may not exceed 15 minutes. This offline
authorization horizon is separate from the unchanged five-minute maximum for
executable swap quotes. A terminal-evidence v2 record carries the initiation
timestamp, and an already-initiated HODL payment has a separate hard six-hour
settlement grace for channel confirmations. The grace does not extend
initiation authority or the swap quote limit.

After the external invoice is independently proven `SUCCEEDED` and its funded
channel is confirmed, active, and has positive inbound capacity, record one
owner-only public terminal-evidence JSON and charge the **entire authorized
ceiling**, not the lower observed cost:

```bash
chmod 600 /absolute/path/to/liquidity-terminal-evidence.json
scripts/lightning-navcoin-mainnet-coordinator liquidity-mark-spent \
  --evidence /absolute/path/to/liquidity-terminal-evidence.json
```

The strict
`postfiat.lightning_liquidity_setup_terminal_evidence.v2` object binds the
authorization, policy and setup IDs to provider, successful payment hash,
initiation and settlement times, canonical channel point, remote public key,
capacity, positive inbound, confirmations, actual cost, and observation time.
Its payment must have started after the durable reservation and no later than
the signed permit expiry. Once started, it must settle within the separate
six-hour liquidity horizon. Actual msat cost may not exceed the signed
ceiling.

It has exactly this field set (replace every example value with independently
observed public evidence):

```json
{
  "schema": "postfiat.lightning_liquidity_setup_terminal_evidence.v2",
  "authorization_id": "<64 lowercase hex>",
  "policy_id": "<64 lowercase hex>",
  "setup_id": "<64 lowercase hex>",
  "category": "LIQUIDITY_SETUP",
  "direction": "not_applicable",
  "provider": "<reviewed provider>",
  "outcome": "EXTERNAL_PAYMENT_CONFIRMED_AND_CHANNEL_ACTIVE",
  "value_moved": true,
  "payment_status": "SUCCEEDED",
  "payment_hash": "<64 lowercase hex>",
  "actual_cost_msat": 1,
  "payment_initiated_at_unix": 1,
  "payment_settled_at_unix": 1,
  "channel_active": true,
  "channel_point": "<funding-txid>:<output-index>",
  "remote_pubkey": "<02-or-03 compressed public key>",
  "capacity_sat": 1,
  "inbound_msat": 1,
  "funding_confirmations": 1,
  "observed_at_unix": 1
}
```

There is deliberately no liquidity-release command. If order, payment, or
channel outcome is absent or ambiguous, keep the ceiling `RESERVED` and stop;
never infer `no value moved` from an RPC or operator transport error.

The ARMED process is protected by a kernel process lock. One bounded recovery
worker consumes the durable coordinator recovery plan and invokes the
idempotent runtime reconciliation path. Any ambiguous external outcome
remains `HOLD_CHECK_DURABLE_JOURNALS`; a transport error is never reported as
proof that no value moved.

Before the founder sends Coinbase-funded dust to the selected Phoenix build,
rehearse and record that build's successful-payment detail exporting the
32-byte preimage and verify `SHA256(preimage) == invoice payment_hash`
locally. If that exact payer interface cannot export the preimage, the
BTC-to-NAVcoin payment remains HOLD; do not substitute an undocumented
assumption.
