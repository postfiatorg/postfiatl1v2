# FastPay For Agents

FastPay is the Post Fiat Ledger's consensusless payment lane for prefunded
single-owner `PFT` objects (`1 PFT = 1,000,000 atoms`). Instead of waiting for
block consensus, a client or x402 facilitator collects `>= 2f+1` distinct
validator ML-DSA-65 approval votes directly to assemble a finality certificate
in a single network round trip (`11.7 ms` co-located `owned_sign_v3`; `273 ms`
p50 / `307 ms` p95 direct 1-RTT certificate finality and `322 ms` p50 /
`350 ms` p95 x402 `402 -> 200` settlement across a 3-continent `5-of-5` active
WAN quorum in a 6-node fleet, with `9.33 calls/s` across `K = 8` concurrent
`OwnedObjectPool` calls).

## When to use FastPay vs. consensus `payment_v2`

| Workload | Lane | Reason |
| --- | --- | --- |
| Pay-per-call AI agent HTTP APIs & MCP tools (`x402`) | FastPay (`OwnedTransferOrderV3`) | Sub-second 1-RTT finality, immutable 48-byte `order_hash` deduplication, 51-byte inline `request_hash` memo |
| High-frequency `PFT` payouts and micropayments | FastPay (`OwnedTransferOrderV3`) | Parallel execution via `OwnedObjectPool`, indexed in `account_tx` and `tx` by `order_hash` |
| Sub-second anchored `pf.ptr` v5 messages | FastPay transfer or self-transfer | Up to 4 memos (`<= 256 B` each, lowercase hex) without consensus latency |
| Account-balance transfers without prefunded objects | Consensus (`payment_v2`) | No prior `OwnedDepositV1` wrap needed |
| Issued assets (`pfUSDC`, NAVCoins), escrows, NFTs, DEX offers | Consensus (`issued_payment_v2`, etc.) | FastPay v3 moves native `PFT` owned objects only |

## End-to-end lifecycle

1. **Preflight (fail closed):**
   - Call `server_info` and require `rpc.owned_lane_enabled == true`.
   - Call `owned_recovery_capabilities` for the active recovery domain (`chain_id`, `genesis_hash`, `protocol_version`), `committee_epoch`, and bounded height window (`valid_from_height`, `expires_at_height`, `recovery_closes_at_height`).
   - Call `fastpay_recovery_committee` (no params = active epoch) and pin validator IDs, ML-DSA-65 public keys, `registry_root`, and quorum threshold (`2f+1`).
2. **Fund (wrap) `PFT` into an owned object:**
   - Submit a source-signed consensus `OwnedDepositV1` via `mempool_submit_fastlane_primary` (or `postfiat_rpc.wrap_fastpay`) and verify receipt code `owned_deposit_applied`.
   - Legacy `wrap_owned` and `unwrap_owned` RPCs are disabled.
3. **Read owned objects completely:**
   - Call `owned_objects` with `owner_public_key_hex`, `limit: 2048`, `omit_owner_keys: true`.
   - Pages cap at `960 KiB`. When `truncated == true`, paginate with `next_cursor` until `next_cursor == null` before selecting inputs. Restart from page 1 on `owned_objects_view_changed`.
4. **Compute the minimum fee (`fastpay_fee_quote`):**

```text
min_fee_atoms = 25
              + ceil(tx_bytes / 512)
              + max(0, outputs - inputs) * 1,000
              + inputs
```

   - 1-in / 1-out sweep (`4,582 B` canonical unsigned order, `ceil / 512 = 9`): `35 atoms` (`0.000035 PFT`).
   - 1-in / 2-out transfer with change (`8,513 B`, `ceil / 512 = 17`, 1 net-new object): `1,043 atoms` (`0.001043 PFT`).
   - 10-in / 1-out merge (`5,590 B`, `ceil / 512 = 11`, up to `2,048` inputs via JSON-RPC v1): `46 atoms` (`0.000046 PFT`, zero new-object surcharge). Accounts have a `4,096` owned-object soft cap (`10,000,000` global ceiling).
5. **Settle over JSON-RPC v1 or Direct TLS 1.3 Protobuf RPC v2:**
   - Collect `>= 2f+1` distinct active-committee votes via `owned_sign_v3`.
   - **1-RTT (`certificate`) mode:** durably `fsync` the verified certificate to a `FileCertificateStore` WAL, synchronously apply to the co-located validator (`coLocatedValidatorSyncApply: true`), return HTTP `200`, and converge remaining validators asynchronously with exponential retry (`GET /x402/certificates/:orderHash`).
   - **2-RTT (`applied_quorum`) mode:** collect `>= 2f+1` signed `FastPayApplyAckV1` acknowledgments via `owned_apply_v3_fast` (with automatic `OrderNotCached` -> `owned_apply_v3_full` fallback) or `owned_apply_v3`.

## Zero-`redb` hot path & persistent TLS transport

On the `train-a-r8` release (`postfiat-wan-devnet-2`), FastPay v3 achieves `0`
hot-path `redb` opens and `0` cache misses across 19 live consensus blocks
(`1,370` server RPCs) via four invariants that future changes must preserve:

- **Persist-before-vote & speculative WAL:** `owned_sign_v3` persists input
  locks to `owned_locks.wal` (`fdatasync`) under `owned_locks.guard`;
  `owned_apply_v3` appends CRC32C-framed CBOR records to
  `fastpay_speculative_effects_v2.wal`.
- **Post-block ledger snapshot sidecar & synchronized pre-warmer:** Every
  finalized block commit atomically writes `chain_tip_generation.v1` and
  `fastpay_ledger_snapshot.v1.json` (keyed by inode + finalized tip commitment)
  so post-block FastPay reads never open `redb`. A background pre-warmer parses
  the snapshot off-lock once per finalized tip, then acquires
  `lock_ordered_commit()` before reading `v2.wal` and installing the cache
  (never read or truncate `v2.wal` off-lock, or an in-flight `owned_apply_v3`
  append can race it).
- **Commit-time WAL pruning:** `prune_fastpay_speculative_wal_after_block_commit`
  (`crates/node/src/storage_commit.rs`) compacts `fastpay_speculative_effects_v2.wal`
  during `apply_transactional_ordered_commit_delta()` so post-block
  `owned_apply_v3` never re-opens `redb` to check whether prior speculative
  records were anchored.
- **Persistent TLS 1.3 idle timeout (`300 s`):** `FASTPAY_V2_IDLE_READ_TIMEOUT_SECS = 300`
  (`crates/node/src/fastpay_rpc_v2.rs`) decouples the idle socket read timeout
  between framed requests from short per-request `--timeout-ms` values (e.g.
  `30 s`), preventing 3-RTT TLS reconnect spikes after consensus block pauses.
- **Devnet epoch-1 committee quirk:** On `postfiat-wan-devnet-2`, `validator-5`
  rotated its consensus key at height `917` while FastPay committee epoch 1
  remained pinned to its prior key. Active signing is therefore `5-of-5` across
  `validator-0..4`, and `validator-5` acts as a non-voting holder returning
  unsigned `postfiat-fastpay-held-effect-v1` receipts until an epoch-2 rotation
  is enacted.

## Concurrent agent calls (`OwnedObjectPool`)

An owned object `(id, version)` can only be spent once. Signing two conflicting orders for the same `(object_id, version)` triggers an equivocation lock until ordered recovery resolves it.

Agents issuing concurrent x402 HTTP/MCP calls should pre-split `K` owned objects and manage them with `OwnedObjectPool` (`tools/x402-facilitator/mcp-client.js`):

```js
const { X402HttpClient, X402McpClient, OwnedObjectPool } = require('./tools/x402-facilitator/mcp-client');

const pool = new OwnedObjectPool(initialObjects, { lowWaterRatio: 0.5 });

const httpClient = new X402HttpClient({
  chainId: 'postfiat-wan-devnet-2',
  payerAccount,
  payerPublicKey,
  payerSecretKey,
  maxBudgetAtoms: 500_000,
  allowedNetworks: ['postfiat:wan-devnet-2'],
  allowedAssets: ['PFT'],
  nativeFastPay: async ({ required }) => {
    const obj = pool.checkout(required.max_amount_atoms + 2_000, required.asset_id);
    if (!obj) throw new Error('OwnedObjectPool exhausted');
    return {
      domain: activeRecoveryDomain,
      recovery: activeRecoveryWindow,
      payeePubkeyHex,
      inputs: [{ id: obj.id, version: obj.version }],
      inputTotalAtoms: obj.value,
      nextNonce: () => nextNonce++,
    };
  },
});

const mcp = new X402McpClient({ endpointUrl: 'https://api.example.com/mcp', httpClient });
const { result, paymentReceipt } = await mcp.callTool(
  'market_quote',
  { ticker: 'PFT' },
  { expectedPayment: { amountAtoms: 1_000, assetId: 'PFT', payTo: payeeAccount } }
);
```

## Python agent flow (`postfiat_rpc`)

```python
from pathlib import Path
from postfiat_rpc import PostFiatRpcClient, create_wallet, wrap_fastpay, send_fastpay, unwrap_fastpay

client = PostFiatRpcClient("127.0.0.1:27650")
wallet = create_wallet(chain_id="postfiat-wan-devnet-2", wallet_dir="wallets/agent")

wrap_fastpay(client, wallet=wallet, amount=100_000)

# Always pass a durable work_dir so crash/timeout retries replay the exact
# journaled order_hash/certificate instead of signing a conflicting order.
send_fastpay(
    client,
    wallet=wallet,
    recipient_public_key_hex=recipient_pubkey_hex,
    amount=5_000,
    fee=1_043,
    work_dir=Path("state/fastpay-journal/payout-001"),
)

unwrap_fastpay(client, wallet=wallet, amount=50_000)
```

## Mandatory safety invariants

1. **Never sign two conflicting orders for the same `(object_id, version)`.** Persist `(object_id, version, order_hash, signed_order)` before broadcast and retry that exact `order_hash` on timeout or crash.
2. **Deduplicate strictly on immutable 48-byte `order_hash` (SHA3-384), never `certificate_digest`.**
3. **Never delete a lock locally on timeout.** Resolve stuck locks through ordered recovery (`owned_certificate`, `owned_recovery_status`, and quorum-verified `owned_fence_status`).
4. **Always set `maxBudgetAtoms` and `expectedPayment`** on x402 clients: a signed FastPay order has no protocol-level refund if a server withholds HTTP `200`.
5. **Pin `registry_root` and verify `>= 2f+1` distinct active-committee ML-DSA-65 signatures locally.** Ignore unsigned `postfiat-fastpay-held-effect-v1` receipts from non-voting holders when counting committee quorum acks.

## Source anchors

- `crates/node/src/fastpay_rpc_v2.rs` — Direct TLS 1.3 + mTLS binary Protobuf FastPay RPC v2 listener, codec, and `FASTPAY_V2_IDLE_READ_TIMEOUT_SECS = 300`
- `crates/node/src/fastpay_recovery_node.rs` — Append-only CRC32C speculative WAL (`v2.wal`), lock-free `chain_tip_generation.v1` sidecar, `fastpay_ledger_snapshot.v1.json` sidecar, synchronized single-per-tip pre-warmer, and commit-time WAL pruning helper
- `crates/node/src/storage_commit.rs` — `apply_transactional_ordered_commit_delta()` atomic sidecar writes and commit-time FastPay WAL pruning
- `tools/x402-facilitator/chain-adapter.js` — Production FastPay/consensus settlement adapter, `FileCertificateStore`, 1-RTT/2-RTT racing, full multi-validator error reporting & background convergence
- `tools/x402-facilitator/fastpay-rpc-v2.js` — Persistent Node.js TLS 1.3 Protobuf v2 client
- `tools/x402-facilitator/mcp-client.js` — `X402HttpClient`, `X402McpClient`, and `OwnedObjectPool`
- [RPC Methods](../rpc/methods.md#fastpay-wallet-methods)
- [Python Wallet Functions](../python/wallet-functions.md#fastpay-helpers)
- [FastPay Inventory Ordering](../wallets/fastpay-inventory-ordering.md)
