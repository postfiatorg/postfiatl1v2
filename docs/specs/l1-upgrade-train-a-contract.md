# L1 Upgrade Train A — Frozen Wire, State & RPC Contract Specification

> **Contract ID:** Phase 0 Contract C4 (Train A)  
> **Status:** Frozen for parallel implementation across Lane 1 (L1 Integrator), Lane 4 (Task Node), Lane 7 (Explorer), and Lane 8 (NAVCoin / Adapters)  
> **Scope:** Pillar 2 (FastPay fee schedule, owned-object spam guard, `order_hash` `account_tx`/`tx` indexing), Pillar 4 (activation-gated keyed NFT storage, `used_nft_serials` tombstones, `ISSUER_BURNABLE`, permanent-lock spam guard, paginated NFT RPCs), Pillar 8 M1 (single unified `asset_gate(...)` across all 6 asset-movement bypass paths, aggregated `(asset, recipient)` bridge holds, DEX anti-poisoning, bounded clawback, fast-lane permit revocation)  
> **Companion Specification:** [L1 Upgrade Train B Contract](l1-upgrade-train-b-contract.md)

---

## 1. Ground-Truth Codebase Alignment & Wire Conventions

Every type, tag, constant, and RPC envelope in this contract extends the live `postfiatl1v2` workspace without breaking historical block replay or pre-activation state commitments:

| Layer | Authoritative Crate / File | Convention |
|---|---|---|
| Transaction kinds & constants | `crates/types/src/core_chain.rs` | `snake_case` `*_TRANSACTION_KIND` constants, 96-hex SHA3-384 object IDs |
| Accounts, assets, trustlines | `crates/types/src/account_owned_asset_types.rs` | `#[serde(default, skip_serializing_if = ...)]` for every additive field |
| Ledger state, NFTs, offers, escrows | `crates/types/src/market_nav_asset_types.rs` | `LedgerState` fields omit empty collections/options for byte-identical replay |
| Transaction operations & signing | `crates/types/src/transactions_mempool_receipts.rs` | `AssetTransactionOperation`, `NftTransactionOperation`, `OfferTransactionOperation` tagged with `#[serde(tag = "operation")]` and flattened into `Unsigned*Transaction` |
| FastPay V3 orders & fences | `crates/types/src/fastpay_recovery_types.rs` | `OwnedTransferOrderV3`, `OwnedUnwrapOrderV3`, `FastPayVersionFenceV1` (`#[serde(deny_unknown_fields)]`) |
| Fast-lane control & permits | `crates/types/src/fastswap_types.rs` | `FastHolderPermitV1`, `FastLaneControlActionV1` (`#[serde(tag = "kind", rename_all = "snake_case")]`) |
| Governance activation heights | `crates/types/src/shielded_bridge_governance.rs` | `GOVERNANCE_KIND_*_ACTIVATION_HEIGHT` (`latest_amendment_value` semantics) |
| Execution & fees | `crates/execution/src/{entrypoints,fees_offer_planning,issued_asset_ledger_helpers,nft_escrow_asset_execution,nav_vault_asset_execution,owned_transfer,owned_transfer_recovery,solana_bridge,fastswap}.rs` | Effect errors return `(&'static str, String)` (`receipt_code`, `message`); `tec_` prefix for fee-claimed rejections |
| Transactional storage | `crates/storage/src/transactional.rs` | `redb` tables (`current_state_v1`, `history_indexes_v1`, etc.) with HMAC-SHA3-384 table+key binding |
| RPC dispatch & schemas | `crates/node/src/{rpc_dispatch,node_types,account_tx_disk_index}.rs` | JSON envelope `{ "id", "method", "params" }` -> `{ "id", "ok", "result" | "error": { "code", "message" }, "events" }` |

### 1.1 Governance Activation Heights (Train A)

All consensus-visible behavioral changes in Train A are gated behind explicit governance amendments in `crates/types/src/shielded_bridge_governance.rs` (latest committed amendment wins, matching `GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT`):

```rust
pub const GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT: &str =
    "fastpay_fee_v1_activation_height";
pub const GOVERNANCE_KIND_NFT_SCALING_V1_ACTIVATION_HEIGHT: &str =
    "nft_scaling_v1_activation_height";
pub const GOVERNANCE_KIND_ASSET_GATE_V1_ACTIVATION_HEIGHT: &str =
    "asset_gate_v1_activation_height";
```

Corresponding accessors on `GovernanceState` and fields on `AssetExecutionCompatibility` (`crates/execution/src/nft_escrow_asset_execution.rs`):

```rust
pub struct AssetExecutionCompatibility {
    // ... existing fields unchanged ...
    pub fastpay_fee_v1_activation_height: Option<u64>,
    pub nft_scaling_v1_activation_height: Option<u64>,
    pub asset_gate_v1_activation_height: Option<u64>,
}
```

- **Activation rule:** A Train A rule is active at block height `h` iff `activation_height.is_some_and(|a| a > 0 && h >= a)`.
- **Coordinated activation:** Governance may set all three kinds to the same height `H_train_a` or stagger them independently.
- **Historical replay invariant:** Below activation height, execution, fee validation, state serialization, and state root calculation remain byte-for-byte identical to pre-Train-A `main`.

### 1.2 Standard RPC Envelope

All RPC request/response examples in this contract use the live `rpc-serve` / `--request-file` JSON envelope:

```json
// Request
{
  "id": "req-1",
  "method": "<method_name>",
  "params": { ... }
}

// Success Response
{
  "id": "req-1",
  "ok": true,
  "result": { ... },
  "events": [
    { "kind": "<method_name>", "target": "<target>", "message": "<summary>" }
  ]
}

// Error Response
{
  "id": "req-1",
  "ok": false,
  "error": {
    "code": "<error_code>",
    "message": "<human-readable detail>"
  },
  "events": [
    { "kind": "error", "target": "<error_code>", "message": "rpc request failed" }
  ]
}
```

---

## 2. Pillar 2 — Sui-Inspired FastPay Fee Schedule, Object Spam Guard & `order_hash` Account History

### 2.1 Protocol Constants (`crates/types/src/core_chain.rs` & `crates/execution/src/owned_transfer.rs`)

```rust
/// Sui-inspired FastPay fee constants (active at `fastpay_fee_v1_activation_height`).
pub const FASTPAY_MIN_BASE_FEE_ATOMS: u64 = 25;
pub const FASTPAY_FEE_BYTE_QUANTUM: usize = 512;
pub const FASTPAY_FEE_PER_BYTE_QUANTUM_ATOMS: u64 = 1;
pub const FASTPAY_FEE_PER_NEW_OBJECT_ATOMS: u64 = 1_000;
pub const FASTPAY_FEE_PER_INPUT_ATOMS: u64 = 1;

/// Owned-object limits (active at `fastpay_fee_v1_activation_height`).
/// Legacy `MAX_OWNED_OBJECTS = 100_000` remains used strictly pre-activation.
pub const MAX_OWNED_OBJECTS_GLOBAL_V2: usize = 10_000_000;
pub const MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP: usize = 4_096;
```

### 2.2 Deterministic Minimum Fee Formula

Active for both `OwnedTransferOrderV3` and `OwnedUnwrapOrderV3` once `block_height >= fastpay_fee_v1_activation_height` (enforced at both validator lock/sign admission AND consensusless/ordered certificate application):

```rust
pub fn fastpay_minimum_fee_atoms(
    weight_bytes: usize,
    inputs_consumed: usize,
    outputs_created: usize,
) -> Result<u64, OwnedTransferError> {
    let byte_quanta = u64::try_from(weight_bytes.div_ceil(FASTPAY_FEE_BYTE_QUANTUM))
        .map_err(|_| OwnedTransferError::Overflow)?;
    let byte_fee = byte_quanta
        .checked_mul(FASTPAY_FEE_PER_BYTE_QUANTUM_ATOMS)
        .ok_or(OwnedTransferError::Overflow)?;
    let net_new_objects = u64::try_from(outputs_created.saturating_sub(inputs_consumed))
        .map_err(|_| OwnedTransferError::Overflow)?;
    let new_object_fee = net_new_objects
        .checked_mul(FASTPAY_FEE_PER_NEW_OBJECT_ATOMS)
        .ok_or(OwnedTransferError::Overflow)?;
    let input_fee = u64::try_from(inputs_consumed)
        .map_err(|_| OwnedTransferError::Overflow)?
        .checked_mul(FASTPAY_FEE_PER_INPUT_ATOMS)
        .ok_or(OwnedTransferError::Overflow)?;

    FASTPAY_MIN_BASE_FEE_ATOMS
        .checked_add(byte_fee)
        .and_then(|v| v.checked_add(new_object_fee))
        .and_then(|v| v.checked_add(input_fee))
        .ok_or(OwnedTransferError::Overflow)
}
```

Exact inputs to `fastpay_minimum_fee_atoms`:

| Order Type | `weight_bytes` | `inputs_consumed` | `outputs_created` |
|---|---|---|---|
| `OwnedTransferOrderV3` | `owned_transfer_v3_signing_bytes(&order).len()` | `order.inputs.len()` | `order.outputs.len()` |
| `OwnedUnwrapOrderV3` | `owned_unwrap_v3_signing_bytes(&order).len()` | `order.inputs.len()` | `if change_value > 0 { 1 } else { 0 }` |

- **Fee enforcement:** Validators reject any order where `order.fee < required_fee` with `OwnedTransferError::InsufficientFee { fee: order.fee, required: required_fee }`.
- **Conservation:** Unchanged (`sum(inputs) == sum(outputs) + order.fee` for transfer; `sum(inputs) == order.amount + change_value + order.fee` for unwrap). The fee is burned atomically on apply.
- **Worked examples:**
  - Single-input, single-output transfer (`inputs = 1`, `outputs = 1`, `weight_bytes = 420`): `25 + 1 + 0 + 1 = 27 atoms`.
  - Single-input split into 2 outputs (1 recipient + 1 change, `weight_bytes = 600`): `25 + 2 + 1_000 + 1 = 1_028 atoms` (`0.001028 PFT`).
  - Merge 100 small payouts into 1 object (`inputs = 100`, `outputs = 1`, `weight_bytes = 8_192`): `25 + 16 + 0 + 100 = 141 atoms` (`0.000141 PFT`).
  - Unwrap 1 input fully to consensus lane (`inputs = 1`, `outputs = 0`, `weight_bytes = 380`): `25 + 1 + 0 + 1 = 27 atoms`.

### 2.3 Owned-Object Cap & Spam Guard Rules

Active when `block_height >= fastpay_fee_v1_activation_height`:

1. **Global safety ceiling:** Total live `LedgerState.owned_objects.len()` after apply must satisfy `next_total <= MAX_OWNED_OBJECTS_GLOBAL_V2` (`10_000_000`). Applies only when `outputs_created > inputs_consumed`; consolidations/unwraps never fail the global ceiling.
2. **Per-account soft cap:** For each distinct `owner_pubkey_hex` receiving outputs in the order, compute:
   - `current_for_owner`: count of live `OwnedObject` records with `obj.owner_pubkey_hex == target_pk` before apply.
   - `consumed_for_owner`: count of consumed inputs with `obj.owner_pubkey_hex == target_pk`.
   - `created_for_owner`: count of created outputs with `out.owner_pubkey_hex == target_pk`.
   - If `created_for_owner > consumed_for_owner` and `current_for_owner - consumed_for_owner + created_for_owner > MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP` (`4_096`), reject with `OwnedTransferError::AccountOwnedObjectCapExceeded`.
   - Consolidating (`created_for_owner <= consumed_for_owner`) or unwrapping is **always allowed** even if an account already sits above the cap.

### 2.4 FastPay Error Codes (`OwnedTransferError` & RPC)

Additive variants on `crates/execution/src/owned_transfer.rs::OwnedTransferError`:

```rust
pub enum OwnedTransferError {
    // ... existing 20 variants unchanged ...
    InsufficientFee { fee: u64, required: u64 },
    AccountOwnedObjectCapExceeded {
        owner_pubkey_hex: String,
        current: usize,
        limit: usize,
    },
}
```

| `OwnedTransferError` Variant | RPC Error `code` | Trigger |
|---|---|---|
| `InsufficientFee { fee, required }` | `fastpay_insufficient_fee` | `order.fee < fastpay_minimum_fee_atoms(...)` |
| `AccountOwnedObjectCapExceeded { .. }` | `fastpay_account_object_cap_exceeded` | Net-new owned objects push target pubkey above `4_096` |
| `ResourceLimitExceeded` | `fastpay_global_object_cap_exceeded` | Net-new owned objects push global count above `10_000_000` (or `100_000` pre-activation) |

### 2.5 Canonical `order_hash` Definition & Storage Keys

- **Canonical `order_hash`:** Literally the existing 96-lowercase-hex SHA3-384 `order_digest` computed over the signed order's immutable signing bytes (independent of validator vote arrival order — zero certificate-signature malleability):
  - `OwnedTransferOrderV3`: `fastpay_transfer_order_digest_v3(&order)` (domain `"postfiat.fastpay.transfer-order.v3"`)
  - `OwnedUnwrapOrderV3`: `fastpay_unwrap_order_digest_v3(&order)` (domain `"postfiat.fastpay.unwrap-order.v3"`)
  - Identical to `FastPayRecoveryDecisionV1::Confirmed { order_digest, .. }.order_digest` already persisted in `BlockRecord.fastpay_pre_state_effects`.
- **Transactional storage index key (`crates/storage/src/transactional.rs`, table `HISTORY_INDEXES` / `"history_indexes_v1"`):**
  - Prefix: `const FASTPAY_ORDER_INDEX_KEY_PREFIX: &[u8] = b"fastpay_order_v1\0";`
  - Key bytes: `b"fastpay_order_v1\0" || order_hash.as_bytes()` (17 + 96 = 113 bytes)
  - Value (canonical JSON, HMAC-protected by table+key):
    ```rust
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct FastPayOrderIndexEntryV1 {
        pub schema: String,            // "postfiat-fastpay-order-index-entry-v1"
        pub order_hash: String,        // 96 lowercase hex
        pub certificate_digest: String,// 96 lowercase hex
        pub lock_id: String,           // 96 lowercase hex
        pub operation: FastPayOperationKindV1, // "transfer" | "unwrap"
        pub block_height: u64,
        pub effect_index: u64,         // 0-based index in BlockRecord.fastpay_pre_state_effects
    }
    ```
- **Account history disk index schema bump (`crates/node/src/account_tx_disk_index.rs`):**
  - `ACCOUNT_TX_INDEX_SCHEMA = "postfiat-account-tx-index-v6"`
  - `ACCOUNT_TX_DISK_INDEX_SCHEMA = "postfiat-account-tx-disk-index-v6"`
  - `ACCOUNT_TX_ACCOUNT_HEAD_SCHEMA = "postfiat-account-tx-account-head-v6"`
  - `ACCOUNT_TX_ACCOUNT_CHUNK_SCHEMA = "postfiat-account-tx-account-chunk-v6"`
  - Older v1–v5 disk indexes fall back to the archive scan automatically until rebuilt via `postfiat-node account-tx-index-build`.

### 2.6 `AccountTxRow` & `tx` Schema Extensions for FastPay

Additive optional fields on `crates/node/src/node_types.rs::AccountTxRow` (omitted when `None` so all existing rows serialize identically):

```rust
pub struct AccountTxRow {
    // ... all existing fields unchanged ...
    /// Canonical immutable FastPay order hash (equals `tx_id` on FastPay rows).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order_hash: Option<String>,
    /// Applied validator-quorum certificate digest (informational; never used as primary key).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate_digest: Option<String>,
    /// FastPay lock_id from FastPayVersionFenceV1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fastpay_lock_id: Option<String>,
}
```

Exact row emission rules from `BlockRecord.fastpay_pre_state_effects` (for every fence where `fence.decision == FastPayRecoveryDecisionV1::Confirmed { order_digest, certificate_digest }` and `fence.certificate` is `Some(cert)`):

| Field | `OwnedTransferCertificateV3` | `OwnedUnwrapCertificateV3` |
|---|---|---|
| `tx_id` | `order_digest` (96 hex) | `order_digest` (96 hex) |
| `order_hash` | `Some(order_digest)` | `Some(order_digest)` |
| `certificate_digest` | `Some(certificate_digest)` | `Some(certificate_digest)` |
| `fastpay_lock_id` | `Some(fence.lock_id)` | `Some(fence.lock_id)` |
| `block_height` | `block.header.height` | `block.header.height` |
| `batch_kind` | `"fastpay"` | `"fastpay"` |
| `batch_id` | `fence.lock_id` | `fence.lock_id` |
| `transaction_index` | `effect_index` (`u64`) | `effect_index` (`u64`) |
| `transaction_kind` | `"owned_transfer_v3"` | `"owned_unwrap_v3"` |
| `from` | `address_from_public_key(&cert.owner_pubkey_hex)` | `address_from_public_key(&cert.owner_pubkey_hex)` |
| `to` | `address_from_public_key(&output.owner_pubkey_hex)` (one row per output with `fill_index = Some(output_idx)`) | `cert.order.to_address` |
| `amount` | `output.value` | `cert.order.amount` |
| `fee` | `cert.order.fee` (on `output_idx == 0`; `0` on subsequent output legs) | `cert.order.fee` |
| `sequence` | `cert.order.nonce` | `cert.order.nonce` |
| `memo_hash` / `memo_count` / `memo_bytes` / `memos` | Decoded `Vec<PaymentMemoView>` identical to `payment_v2` (empty `[]` when no memos) | Decoded `Vec<PaymentMemoView>` identical to `payment_v2` |
| `tx_role` | `"fastpay_self_transfer"` if `from == to`, else `"fastpay_transfer"` | `"fastpay_unwrap"` |
| `accepted` | `Some(true)` | `Some(true)` |
| `receipt_code` | `Some("accepted")` | `Some("accepted")` |

- **Deduplication rule:** For a transfer whose output 0 goes to `recipient` and output 1 is change back to `sender`, `recipient` receives the row for output 0 (`from = sender`, `to = recipient`, `fill_index = 0`), and `sender` sees both rows (or filters change rows via `tx_role == "fastpay_self_transfer"`). Self-transfers (`outputs.len() == 1` with `from == to`, e.g., pure Pillar 2 messages) emit exactly one row with `tx_role = "fastpay_self_transfer"`.
- **`tx` RPC lookup by `order_hash`:** Calling `tx` with `params.tx_id = <order_hash>` resolves via `fastpay_order_v1\0<order_hash>` and returns `TxFinalityReport` with `confirmed: true`, `verification_mode: "fastpay_version_fence_v1"`, a canonical receipt (`tx_id: order_hash`, `transaction_kind: "owned_transfer_v3" | "owned_unwrap_v3"`, `accepted: true`, `receipt_code: "accepted"`), `block`, and `transaction_rows` containing the exact `AccountTxRow` list above.

### 2.7 RPC Request & Response JSON (Pillar 2)

#### `fastpay_fee_quote` (New Read Method)

Request:
```json
{
  "id": "fp-quote-1",
  "method": "fastpay_fee_quote",
  "params": {
    "operation": "transfer",
    "owner_pubkey_hex": "<ml_dsa_65_pubkey_hex>",
    "inputs_count": 1,
    "outputs_count": 2,
    "memo_bytes": 180,
    "estimated_weight_bytes": 620
  }
}
```
(`operation`: `"transfer"` | `"unwrap"`. If `estimated_weight_bytes` is omitted, node computes conservative upper bound from `inputs_count`, `outputs_count`, and `memo_bytes`.)

Response (`schema = "postfiat-fastpay-fee-quote-v1"`):
```json
{
  "id": "fp-quote-1",
  "ok": true,
  "result": {
    "schema": "postfiat-fastpay-fee-quote-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "block_height": 124500,
    "fee_v1_active": true,
    "operation": "transfer",
    "weight_bytes": 620,
    "inputs_count": 1,
    "outputs_count": 2,
    "base_fee": 25,
    "byte_fee": 2,
    "new_object_fee": 1000,
    "input_fee": 1,
    "minimum_fee": 1028,
    "owner_current_object_count": 14,
    "owner_soft_cap": 4096,
    "global_current_object_count": 18420,
    "global_hard_cap": 10000000
  },
  "events": [
    { "kind": "fastpay_fee_quote", "target": "transfer", "message": "fastpay fee quoted" }
  ]
}
```

#### `account_tx` (Extended FastPay Row Example)

```json
{
  "tx_id": "a1b2c3...96_hex_order_hash",
  "order_hash": "a1b2c3...96_hex_order_hash",
  "certificate_digest": "d4e5f6...96_hex_cert_digest",
  "fastpay_lock_id": "778899...96_hex_lock_id",
  "block_height": 124501,
  "batch_kind": "fastpay",
  "batch_id": "778899...96_hex_lock_id",
  "transaction_index": 0,
  "transaction_kind": "owned_transfer_v3",
  "from": "pf1111111111111111111111111111111111111111",
  "to": "pf2222222222222222222222222222222222222222",
  "amount": 50000000,
  "fee": 1028,
  "sequence": 42,
  "memo_hash": "99aa...96_hex",
  "memo_count": 1,
  "memo_bytes": 68,
  "memos": [
    {
      "memo_type": "70662e707472",
      "memo_format": "7635",
      "memo_data": "08051224...",
      "memo_type_text": "pf.ptr",
      "memo_format_text": "v5"
    }
  ],
  "tx_role": "fastpay_transfer",
  "fill_index": 0,
  "accepted": true,
  "receipt_code": "accepted"
}
```

---

## 3. Pillar 4 — Activation-Gated Keyed NFT Lookups, `used_nft_serials` Tombstones & Execution Hardening

### 3.1 Protocol Constants (`crates/types/src/core_chain.rs`)

```rust
/// High-bit partition for migrated historical NFT serials (locked in Pillar 4):
/// - `serial & NFT_MIGRATED_SERIAL_BIT != 0`: deterministic hash-derived migrated serial
/// - `serial & NFT_MIGRATED_SERIAL_BIT == 0` (and `serial > 0`): sequential fresh mint
pub const NFT_MIGRATED_SERIAL_BIT: u64 = 1_u64 << 63;

/// Default and maximum page sizes for paginated NFT RPCs.
pub const DEFAULT_NFT_RPC_PAGE_LIMIT: usize = 100;
pub const MAX_NFT_RPC_PAGE_LIMIT: usize = 500;
```

### 3.2 Keyed State & Deterministic Activation Migration

Additive fields on `crates/types/src/market_nav_asset_types.rs::LedgerState` (omitted when empty so pre-activation state root is byte-identical):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftCollectionPolicyV1 {
    pub issuer: String,
    pub collection_id: String,
    pub collection_flags: u32,
    pub first_minted_height: u64,
}

pub struct LedgerState {
    // ... existing `pub nfts: Vec<NftDefinition>` preserved for pre-activation replay ...

    /// Keyed NFT store active at `nft_scaling_v1_activation_height`.
    /// Key: `nft_id` (96 lowercase hex).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nfts_by_id: BTreeMap<String, NftDefinition>,

    /// Permanent serial-reuse tombstones active at `nft_scaling_v1_activation_height`.
    /// Key: `nft_id` (96 lowercase hex = SHA3-384(chain_id, issuer, collection_id, serial)).
    /// Value: block height at which the NFT was minted (0 for pre-activation migrated records).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub used_nft_serials: BTreeMap<String, u64>,

    /// Keyed collection policy map eliminating O(N) linear scan on `nft_mint`.
    /// Key: `nft_collection_policy_key(issuer, collection_id)` (96 lowercase hex).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nft_collection_policies: BTreeMap<String, NftCollectionPolicyV1>,
}
```

Key derivation:
```rust
pub const NFT_COLLECTION_POLICY_KEY_DOMAIN: &str = "postfiat.nft_collection_policy_key.v1";

pub fn nft_collection_policy_key(issuer: &str, collection_id: &str) -> String {
    let preimage = format!(
        "issuer={issuer}\ncollection_id_bytes={}\ncollection_id={collection_id}\n",
        collection_id.len()
    );
    hash_hex_domain(NFT_COLLECTION_POLICY_KEY_DOMAIN, preimage.as_bytes())
}
```

#### Deterministic Activation Migration (at `block_height == nft_scaling_v1_activation_height`)

Before executing the first transaction of block `h == nft_scaling_v1_activation_height`, the state transition deterministically migrates `ledger.nfts` in a single canonical pass:
1. Move every `NftDefinition` from `ledger.nfts` into `ledger.nfts_by_id.insert(nft.nft_id.clone(), nft)` (clearing `ledger.nfts` to avoid duplicate storage).
2. For each migrated NFT, insert `ledger.used_nft_serials.insert(nft.nft_id.clone(), 0)`.
3. For each distinct `(nft.issuer, nft.collection_id)`, insert `NftCollectionPolicyV1 { issuer, collection_id, collection_flags: nft.collection_flags, first_minted_height: 0 }` into `ledger.nft_collection_policies`.
4. All `LedgerState::nft(&self, nft_id)` / `nft_mut` lookups check `nfts_by_id` in `O(log N)` (falling back to `nfts` only pre-activation).

#### Transactional Storage Secondary Indexes (`crates/storage/src/transactional.rs`)

Node-local transactional redb tables maintained atomically on every block commit (keyed by canonical UTF-8 / big-endian bytes, never consensus-visible):

| Logical Table / Key Prefix | Key Layout | Value |
|---|---|---|
| `nft_by_id_v1` | `nft_id_hex (96B)` | Encoded `NftDefinition` |
| `nft_by_owner_v1` | `owner_addr (42B) \|\| 0x00 \|\| burned_u8 (1B) \|\| nft_id_hex (96B)` | Empty marker `[]` |
| `nft_by_issuer_v1` | `issuer_addr (42B) \|\| 0x00 \|\| collection_id \|\| 0x00 \|\| burned_u8 (1B) \|\| nft_id_hex (96B)` | Empty marker `[]` |
| `nft_used_serial_v1` | `nft_id_hex (96B)` | `minted_at_height.to_be_bytes()` (`[u8; 8]`) |

### 3.3 Action Struct Update: `NftBurnOperation` (`ISSUER_BURNABLE` Enforcement)

Additive backwards-compatible field on `crates/types/src/transactions_mempool_receipts.rs::NftBurnOperation` (matching the exact optional-field signing pattern of `NftTransferOperation`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftBurnOperation {
    pub nft_id: String,
    pub owner: String,
    /// Optional issuer address for issuer-initiated burn when
    /// `nft.flags & NFT_FLAG_ISSUER_BURNABLE != 0`.
    /// Omitted (empty) for standard owner-initiated burn.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub issuer: String,
}
```

- **Signing bytes:**
  - When `self.issuer.is_empty()`: `format!("nft_id={}\nowner={}\n", self.nft_id, self.owner)` (byte-for-byte identical to pre-Train-A).
  - When `!self.issuer.is_empty()`: appends `format!("issuer={}\n", self.issuer)`.
- **Source authorization (`NftTransactionOperation::source_matches`):**
  - If `operation.issuer.is_empty()` -> `operation.owner == source`.
  - If `!operation.issuer.is_empty()` -> `operation.issuer == source` (active only at `block_height >= nft_scaling_v1_activation_height`; rejected pre-activation).
- **Execution rules for `nft_burn` (post-activation):**
  1. Lookup `nft` in `ledger.nfts_by_id` (`O(log N)`).
  2. Reject if `nft.burned` -> `"nft_burned"`.
  3. Reject if `nft.collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED != 0` -> `"nft_collection_burn_locked"`.
  4. Reject if `nft.owner != operation.owner` -> `"nft_owner_mismatch"`.
  5. If `!operation.issuer.is_empty()` (issuer-initiated burn):
     - Reject if `nft.issuer != operation.issuer` -> `"nft_issuer_mismatch"`.
     - Reject if `nft.flags & NFT_FLAG_ISSUER_BURNABLE == 0` -> `"nft_not_issuer_burnable"`.
  6. Mark `nft.burned = true`. **NEVER delete the `used_nft_serials` tombstone!**

### 3.4 Execution Rules for `nft_mint` (Post-Activation)

1. Compute `id = nft_id(&genesis.chain_id, &op.issuer, &op.collection_id, op.serial)`.
2. **Permanent tombstone check:** If `ledger.used_nft_serials.contains_key(&id)` -> reject with `"nft_serial_already_used"`.
3. **Duplicate live check:** If `ledger.nfts_by_id.contains_key(&id)` -> reject with `"duplicate_nft"`.
4. **Unsolicited permanent-lock spam guard:** If `op.owner != op.issuer` AND `op.collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED != 0` AND (`op.collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED != 0` || `op.flags & NFT_FLAG_TRANSFERABLE == 0`), reject with `"nft_unsolicited_permanent_lock_forbidden"`.
5. **Keyed collection policy check (`O(log N)`):** Lookup `policy_key = nft_collection_policy_key(&op.issuer, &op.collection_id)` in `ledger.nft_collection_policies`:
   - If present and `existing.collection_flags != op.collection_flags` -> reject with `"nft_collection_policy_mismatch"`.
   - If absent -> insert `NftCollectionPolicyV1 { issuer, collection_id, collection_flags: op.collection_flags, first_minted_height: block_height }`.
6. Insert `nft` into `ledger.nfts_by_id` and record `ledger.used_nft_serials.insert(id, block_height)`.

### 3.5 Pillar 4 Error Codes

| `receipt_code` | Trigger |
|---|---|
| `nft_serial_already_used` | `nft_mint` attempts to reuse an `(issuer, collection_id, serial)` recorded in `used_nft_serials` (including after burn) |
| `nft_not_issuer_burnable` | Issuer-initiated `nft_burn` on an NFT where `flags & NFT_FLAG_ISSUER_BURNABLE == 0` |
| `nft_issuer_burn_not_active` | `nft_burn` with non-empty `issuer` submitted prior to `nft_scaling_v1_activation_height` |
| `nft_unsolicited_permanent_lock_forbidden` | `nft_mint` to `owner != issuer` that is both burn-locked and non-transferable / transfer-locked |
| `invalid_nft_cursor` (RPC) | `account_nfts` or `issuer_nfts` called with malformed `cursor` (not 96 lowercase hex) |

### 3.6 Paginated `account_nfts` & `issuer_nfts` RPC Contracts

Additive optional parameters on `AccountNftsOptions` and `IssuerNftsOptions` (`crates/node/src/node_types.rs`):
- `cursor: Option<String>` — exclusive lower bound `nft_id` (96 lowercase hex); returns NFTs with `nft_id > cursor` in ascending lexicographic `nft_id` order.
- `limit: Option<usize>` — defaults to `100`, capped at `500` (`MAX_NFT_RPC_PAGE_LIMIT`).

Additive optional fields on `AccountNftsReport` and `IssuerNftsReport`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub next_cursor: Option<String>,
#[serde(default, skip_serializing_if = "Option::is_none")]
pub total_count: Option<u64>,
```

Example `account_nfts` Request:
```json
{
  "id": "nft-page-1",
  "method": "account_nfts",
  "params": {
    "account": "pf2222222222222222222222222222222222222222",
    "include_burned": false,
    "cursor": null,
    "limit": 50
  }
}
```

Example `account_nfts` Response:
```json
{
  "id": "nft-page-1",
  "ok": true,
  "result": {
    "schema": "postfiat-account-nfts-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca...96_hex",
    "protocol_version": 1,
    "account": "pf2222222222222222222222222222222222222222",
    "include_burned": false,
    "limit": 50,
    "truncated": true,
    "nft_count": 50,
    "total_count": 134,
    "next_cursor": "8f91ab...96_hex_last_nft_id",
    "nfts": [
      {
        "nft_id": "0a11...96_hex",
        "issuer": "pf9999999999999999999999999999999999999999",
        "collection_id": "tasknode-profile-v1",
        "serial": 9223372036854775809,
        "owner": "pf2222222222222222222222222222222222222222",
        "metadata_hash": "4c2d...96_hex",
        "metadata_uri": "ipfs://bafkrei...",
        "flags": 1,
        "collection_flags": 0,
        "issuer_transfer_fee": 0,
        "transferable": true,
        "issuer_burnable": false,
        "collection_transfer_locked": false,
        "collection_burn_locked": false,
        "burned": false
      }
    ]
  },
  "events": [
    { "kind": "account_nfts", "target": "pf2222222222222222222222222222222222222222", "message": "account nfts queried" }
  ]
}
```
(`issuer_nfts` follows the exact same pagination contract with `issuer` and optional `collection_id`.)

---

## 4. Pillar 8 M1 — Unified `asset_gate(...)`, 6-Path Bypass Fix & Aggregated Bridge Holds

### 4.1 The 6 Active Bypass Paths Closed at `asset_gate_v1_activation_height`

Before Train A, `issued_asset_credit_recipient_line_index` (`crates/execution/src/issued_asset_ledger_helpers.rs:407`) unconditionally created missing lines with `line.authorized = true`, bypassing `AssetDefinition.requires_authorization` across all 6 credit paths:

1. **Issued payments (`issued_payment`, `issued_payment_v2`)** — `crates/execution/src/issued_asset_ledger_helpers.rs:300`
2. **Shielded pool exits (`credit_issued_asset_from_shielded_pool`)** — `crates/execution/src/issued_asset_ledger_helpers.rs:393`
3. **NAV mints (`nav_mint_at_nav`)** — `crates/execution/src/nft_escrow_asset_execution.rs:2508`
4. **NAV redemption vault-bridge settlements (`nav_redeem_settle`)** — `crates/execution/src/nav_vault_asset_execution.rs:877`
5. **Vault-bridge Ethereum/Arc/Arbitrum deposit claims, receipt mints & PFTL-Uniswap return imports** — `crates/execution/src/nav_vault_asset_execution.rs:2552, 3421, 6888`
6. **Solana bridge deposit proofs (`solana_bridge_proof_v1`)** — `crates/execution/src/solana_bridge.rs:244`

**Post-activation fix:**
- Every implicit `TrustLine` created by `issued_asset_credit_recipient_line_index` is created **only after passing `asset_gate(...)`** and **ALWAYS with `line.authorized = false`** (never permanently setting `authorized = true` — only explicit issuer-signed `trust_set(authorized = true)` may set `line.authorized = true`).
- Every issued-asset debit, lock, credit, offer cross, escrow, burn, clawback, shielded entry/exit, bridge credit, fast-lane movement, and (in Train B) `nav_fee_claim` routes through `asset_gate(...)`.
- **Pre-activation remediation:** An automated audit/remediation script clears any unapproved legacy lines on `requires_authorization = true` assets before `asset_gate_v1_activation_height`.

### 4.2 Single Unified `asset_gate(...)` Contract (`crates/execution/src/issued_asset_ledger_helpers.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetMovementOpKind {
    IssuedPayment,
    AtomicSwap,
    OfferCreate,
    OfferFill,
    OfferCancelRefund,
    EscrowCreate,
    EscrowFinish,
    EscrowCancelRefund,
    NavMint,
    NavRedeem,
    NavRedeemSettle,
    NavFeeClaim,
    AssetBurn,
    AssetClawback,
    BridgeCredit,
    BridgeHoldClaim,
    BridgeHoldReturnOrBurn,
    ShieldedEntry,
    ShieldedExit,
    FastLaneDeposit,
    FastLaneRedeem,
    FastLaneIntent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetGateOutcome {
    /// Movement permitted under normal rules.
    Allow,
    /// Movement permitted strictly via Return-only exit path (lapsed/uncredentialed holder).
    AllowReturnOnly,
    /// Cross-chain finalized bridge credit cannot land on recipient line;
    /// MUST route into aggregated `(asset_id, recipient)` hold without failing the batch.
    RouteToHold { reason_code: &'static str },
    /// Local movement rejected with typed receipt code.
    Reject { code: &'static str, message: String },
}

pub fn asset_gate(
    ledger: &LedgerState,
    asset: &AssetDefinition,
    from: Option<&str>,
    to: Option<&str>,
    op_kind: AssetMovementOpKind,
    block_height: u64,
) -> AssetGateOutcome
```

#### Deterministic Evaluation Order inside `asset_gate(...)`

1. **Self-unlock refunds never strand funds:** For `OfferCancelRefund` and `EscrowCancelRefund`, return `Allow` immediately (locked units always return to the owner's own line).
2. **Shielded pool bar for gated assets:** If `op_kind` is `ShieldedEntry` or `ShieldedExit` and `asset.requires_authorization`, return `Reject { code: "gated_asset_shielded_pool_forbidden", .. }`.
3. **Issuer clawback:** For `AssetClawback`, check `asset.clawback_enabled`; if false return `Reject { code: "clawback_not_enabled", .. }`, else return `Allow`.
4. **Freeze check (when `asset.freeze_enabled`):**
   - If `from` has a `TrustLine` with `line.frozen == true`: reject unless `op_kind == AssetClawback` (`"trustline_frozen"`).
   - If `to` has a `TrustLine` with `line.frozen == true`:
     - If `op_kind == BridgeCredit`, return `RouteToHold { reason_code: "trustline_frozen" }`.
     - Else reject with `"trustline_frozen"`.
5. **Default (permissionless) assets (`!asset.requires_authorization`):**
   - Check recipient `ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS` (activated in Train B; no-op if bit 0x2 is unset) when recipient has no existing line and `from != to`:
     - If `op_kind == BridgeCredit` -> `RouteToHold { reason_code: "recipient_refuses_unsolicited_assets" }`.
     - Else -> `Reject { code: "recipient_refuses_unsolicited_assets", .. }`.
   - Check recipient `holder_limit` if set on existing line (for `BridgeCredit`, overflow routes to `RouteToHold { reason_code: "holder_limit_exceeded" }`).
   - Otherwise return `Allow`.
6. **Gated assets (`asset.requires_authorization == true`):**
   - Evaluate holder eligibility `is_eligible(ledger, asset, account, block_height)`:
     - In Train A (before Train B's `credential_policy` is attached): `line.is_some_and(|l| l.authorized)`.
     - In Train B (once `asset.access_policy` is present): evaluates `approval_mode` (`either`: `line.authorized || credential_policy_satisfied`; `both`: `line.authorized && credential_policy_satisfied`) live at `block_height`.
   - **Return-only exit check** (default `lapsed_exit_rule == return_only`):
     - Applies to: `AssetBurn`, `NavRedeem` (when a NAV route exists), `BridgeHoldReturnOrBurn`, and `IssuedPayment` where `to == Some(asset.issuer.as_str())`.
     - If `from` fails eligibility (expired/revoked/unauthorized) and is not frozen, return `AllowReturnOnly` (holder can always exit to issuer/NAV/burn; never stranded).
     - If `lapsed_exit_rule == strict_freeze_clawback`, reject with `"asset_gate_sender_ineligible"`.
   - **Normal sender check:** If `from` is `Some(s)` and `s != asset.issuer` and `!is_eligible(s)` -> `Reject { code: "asset_gate_sender_ineligible", .. }`.
   - **Normal recipient check:** If `to` is `Some(r)` and `r != asset.issuer`:
     - If `!is_eligible(r)`:
       - If `op_kind == BridgeCredit` -> `RouteToHold { reason_code: "asset_gate_recipient_ineligible" }`.
       - Else -> `Reject { code: "asset_gate_recipient_ineligible", .. }`.
     - If `r` has `ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS` and no existing line -> `RouteToHold` (bridge) or `Reject` (local).
   - Return `Allow`.
7. **Train B Pillar 7 interaction (invariant):** `nav_epoch_finalize` mints accrued fee units into `NavFeeStateV1.unclaimed_fee_units` **without calling `asset_gate`**. `nav_fee_claim` calls `asset_gate(..., to: Some(recipient), op_kind: AssetMovementOpKind::NavFeeClaim, block_height)`.

### 4.3 Consistent 10-Atom Line Creation Fee & Auto-Pruning

- **Consistent 10-atom burned fee (`TRUSTLINE_STATE_EXPANSION_FEE = 10`):** Charged whenever any user-initiated transaction (`trust_set`, `issued_payment`, `issued_payment_v2`, `atomic_swap`, `nav_mint_at_nav`, `offer_create`, `escrow_create`) creates a new `TrustLine`.
- **Auto-pruning empty implicit lines:** After any debit/zero-out (`issued_payment*`, `asset_burn`, `asset_clawback`, `nav_redeem_at_nav`, `offer_create`, `escrow_create`, `vault_bridge_burn_to_redeem`), if a `TrustLine` has `balance == 0 && !frozen && !authorized && holder_limit.is_none() && reserved_total == 0`, remove it automatically from `ledger.trustlines` (preventing permanent 10-atom dust bloat).

### 4.4 Aggregated `(asset, recipient)` Bridge Holds

Zero per-transfer state objects: every non-fatal recipient rejection on finalized Ethereum, Arbitrum, Arc, PFTL-Uniswap return-import, or Solana bridge credits aggregates into ONE hold record per `(asset_id, recipient)`.

#### State Struct & Key (`crates/types/src/account_owned_asset_types.rs` & `market_nav_asset_types.rs`)

```rust
pub const ASSET_BRIDGE_HOLD_KEY_DOMAIN: &str = "postfiat.asset_bridge_hold_key.v1";

pub fn asset_bridge_hold_key(asset_id: &str, recipient: &str) -> String {
    let preimage = format!("asset_id={asset_id}\nrecipient={recipient}\n");
    hash_hex_domain(ASSET_BRIDGE_HOLD_KEY_DOMAIN, preimage.as_bytes())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBridgeHoldV1 {
    /// 96 lowercase hex = `asset_bridge_hold_key(asset_id, recipient)`.
    pub hold_key: String,
    pub asset_id: String,
    pub issuer: String,
    pub recipient: String,
    /// Total held atoms (counted in circulating/outstanding asset supply!).
    pub amount: u64,
    /// Number of finalized bridge credits aggregated into this hold.
    pub credit_count: u64,
    pub created_at_height: u64,
    pub last_updated_height: u64,
    /// Most recent gate reason that routed a credit here.
    pub last_reason_code: String,
}

/// Emitted in transaction receipts for every individual bridge credit routed to a hold
/// so off-chain indexers retain exact per-transfer provenance without bloating ledger state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetBridgeHoldReceiptEventV1 {
    pub hold_key: String,
    pub asset_id: String,
    pub recipient: String,
    pub amount: u64,
    pub source_reference: String,
    pub reason_code: String,
}
```

Added to `LedgerState`:
```rust
/// Aggregated cross-chain bridge holds keyed strictly by `asset_bridge_hold_key(asset_id, recipient)`.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub asset_bridge_holds: BTreeMap<String, AssetBridgeHoldV1>,
```
- **Supply invariant:** `issued_asset_supply(ledger, asset_id)` includes the sum of `hold.amount` across `ledger.asset_bridge_holds` for `asset_id` so bridge conservation and Solana series reconciliation remain exact!
- **Deletion on zero:** When `hold.amount` reaches `0` (via claim, return, burn, or clawback), remove the entry from `ledger.asset_bridge_holds`.

### 4.5 New & Extended Action Structs (Pillar 8 M1)

#### 4.5.1 `bridge_hold_claim` (`AssetTransactionOperation::BridgeHoldClaim`)

```rust
pub const BRIDGE_HOLD_CLAIM_TRANSACTION_KIND: &str = "bridge_hold_claim";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeHoldClaimOperation {
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
}
```
- `source_matches`: `operation.recipient == source`.
- `signing_bytes`: `operation=bridge_hold_claim\nrecipient={}\nasset_id={}\namount={}\n`.
- Execution: Runs `asset_gate(ledger, &asset, None, Some(&op.recipient), AssetMovementOpKind::BridgeHoldClaim, block_height)`. Must return `Allow`. Debits `hold.amount -= op.amount` and credits recipient's trustline (auto-creating with `authorized = false` if missing, respecting `holder_limit`).

#### 4.5.2 `bridge_hold_return` (`AssetTransactionOperation::BridgeHoldReturn`)

```rust
pub const BRIDGE_HOLD_RETURN_TRANSACTION_KIND: &str = "bridge_hold_return";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeHoldReturnOperation {
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    /// `true` burns held units permanently; `false` returns units to issuer (decrements supply).
    #[serde(default)]
    pub burn: bool,
}
```
- `source_matches`: `operation.recipient == source`.
- `signing_bytes`: `operation=bridge_hold_return\nrecipient={}\nasset_id={}\namount={}\nburn={}\n`.
- Execution: Runs `asset_gate(..., AssetMovementOpKind::BridgeHoldReturnOrBurn, block_height)`. Succeeds under `Allow` or `AllowReturnOnly` (always available to lapsed/ineligible holders under Return-only!). Reduces `hold.amount` and total circulating supply by `op.amount`.

#### 4.5.3 `offer_prune_ineligible` (`OfferTransactionOperation::OfferPruneIneligible`)

```rust
pub const OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND: &str = "offer_prune_ineligible";
pub const MAX_OFFER_PRUNE_BATCH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferPruneIneligibleOperation {
    pub pruner: String,
    /// 1..=64 distinct open offer IDs (96 lowercase hex each).
    pub offer_ids: Vec<String>,
}
```
- `source_matches`: `operation.pruner == source` (permissionless — anyone may prune!).
- `signing_bytes`: `operation=offer_prune_ineligible\npruner={}\noffer_ids={}\n` (comma-joined in order).
- Execution: For each `offer_id`, verifies offer is `OFFER_STATE_OPEN` and maker fails `asset_gate` on either `taker_gets_asset_id` or `taker_pays_asset_id` at `block_height` (or `offer.expiration_height > 0 && block_height >= offer.expiration_height`). Marks offer `OFFER_STATE_CANCELED`, refunds remaining locked sell-side balance via `OfferCancelRefund`, and refunds `reserve_paid` to `offer.owner`. Rejects with `"offer_still_eligible"` if none of the listed offers are ineligible/expired.
- **Inline DEX anti-poisoning during `offer_create`:**
  - `pub const MAX_DEX_INELIGIBLE_SKIPS_PER_TRANSACTION: usize = 64;`
  - Ineligible resting makers skipped during matching do NOT consume `MAX_DEX_CROSSES_PER_TRANSACTION` (`64`).
  - Each skipped ineligible maker is canceled and refunded inline, and those cancellations persist even when the taker achieves a partial fill or rests on the book.

#### 4.5.4 Bounded `asset_clawback` Extension (`AssetClawbackOperation`)

Additive backwards-compatible fields on `crates/types/src/transactions_mempool_receipts.rs::AssetClawbackOperation`:

```rust
pub const MAX_CLAWBACK_LOCKED_TARGETS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetClawbackOperation {
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub amount: u64,
    /// Optional bounded list of open offers owned by `owner` selling `asset_id` to sweep.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offer_ids: Vec<String>,
    /// Optional bounded list of open escrows owned by `owner` locking `asset_id` to sweep.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escrow_ids: Vec<String>,
    /// When true, also sweep `owner`'s aggregated `(asset_id, owner)` bridge hold.
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_bridge_hold: bool,
}
```
- **Validation:** `offer_ids.len() + escrow_ids.len() + usize::from(include_bridge_hold) <= MAX_CLAWBACK_LOCKED_TARGETS` (`64`). All `offer_ids` / `escrow_ids` must be distinct 96-hex IDs.
- **Signing bytes:** Appends `offer_ids={}\n`, `escrow_ids={}\n`, `include_bridge_hold=true\n` **only** when non-empty / true, preserving 100% byte-compatibility with historical `asset_clawback` transactions.
- **Sweep order (deterministic):** Free trustline balance -> `include_bridge_hold` -> `offer_ids` (in listed order, canceling & refunding PFT reserve to `owner`) -> `escrow_ids` (in listed order, marking canceled) until `op.amount` is clawed back. Rejects with `"insufficient_clawback_balance"` if total reachable balance across the specified targets is `< op.amount`.

#### 4.5.5 Fast-Lane Holder Permit Revocation (`FastLaneControlActionV1`)

Additive variant on `crates/types/src/fastswap_types.rs::FastLaneControlActionV1`:

```rust
pub enum FastLaneControlActionV1 {
    // ... existing variants 1..=6 unchanged ...
    RevokeHolderPermit {
        permit_id: FastHolderPermitIdV1,
        revoked_at_height: u64,
        reason_code: String,
    },
}
```
- Canonical encoding tag: `encoder.u8(7); encoder.fixed(&permit_id.0)?; encoder.u64(*revoked_at_height); encoder.string(reason_code)?;`.
- `LedgerState` additive field:
  ```rust
  #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
  pub revoked_fast_lane_holder_permits: BTreeMap<String, u64>, // permit_id_hex -> revoked_at_height
  ```
- Fast-lane admission (`crates/execution/src/fastswap.rs`): rejects any intent touching a gated asset if either sender/receiver permit is in `revoked_fast_lane_holder_permits`, OR if either party is frozen or fails `asset_gate(...)` at the lane's finalized primary height (`FastSwapAdmissionError::HolderPermitRevoked` / `FastSwapAdmissionError::AssetGateRejected`).

### 4.6 Pillar 8 M1 Error Codes

| `receipt_code` / Error Code | Trigger |
|---|---|
| `asset_gate_sender_ineligible` | Sender fails `asset_gate` on a gated asset (and op is not Return-only) |
| `asset_gate_recipient_ineligible` | Recipient fails `asset_gate` on a local movement of a gated asset |
| `gated_asset_shielded_pool_forbidden` | `requires_authorization == true` asset attempts shielded pool entry or exit |
| `recipient_refuses_unsolicited_assets` | Recipient has `ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS` and no existing line (local movement) |
| `holder_limit_exceeded` | Credit would exceed recipient's binding `TrustLine.holder_limit` (local movement) |
| `missing_bridge_hold` | `bridge_hold_claim` / `bridge_hold_return` references non-existent `(asset_id, recipient)` hold |
| `insufficient_bridge_hold_balance` | Claim/return amount exceeds `hold.amount` |
| `clawback_target_limit_exceeded` | `offer_ids.len() + escrow_ids.len() + include_bridge_hold > 64` |
| `clawback_target_mismatch` | Listed offer/escrow does not belong to `owner`, is not open, or locks a different asset |
| `insufficient_clawback_balance` | Reachable balance across free line + listed targets is below `operation.amount` |
| `offer_prune_batch_limit_exceeded` | `offer_prune_ineligible.offer_ids.len()` is `0` or `> 64` |
| `offer_still_eligible` | `offer_prune_ineligible` targeted an offer that is still eligible and unexpired |
| `fast_lane_permit_revoked` | Fast-lane intent uses a revoked `FastHolderPermitV1` |

### 4.7 RPC Request & Response JSON (Pillar 8 M1)

#### `account_bridge_holds` (New Read Method)

Request:
```json
{
  "id": "holds-1",
  "method": "account_bridge_holds",
  "params": {
    "recipient": "pf2222222222222222222222222222222222222222",
    "asset_id": null,
    "limit": 100
  }
}
```

Response (`schema = "postfiat-account-bridge-holds-v1"`):
```json
{
  "id": "holds-1",
  "ok": true,
  "result": {
    "schema": "postfiat-account-bridge-holds-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca...96_hex",
    "protocol_version": 1,
    "recipient": "pf2222222222222222222222222222222222222222",
    "hold_count": 1,
    "truncated": false,
    "holds": [
      {
        "hold_key": "55aa...96_hex",
        "asset_id": "99bb...96_hex",
        "issuer": "pf9999999999999999999999999999999999999999",
        "recipient": "pf2222222222222222222222222222222222222222",
        "amount": 25000000,
        "credit_count": 2,
        "created_at_height": 124400,
        "last_updated_height": 124488,
        "last_reason_code": "asset_gate_recipient_ineligible"
      }
    ]
  },
  "events": [
    { "kind": "account_bridge_holds", "target": "pf2222222222222222222222222222222222222222", "message": "account bridge holds queried" }
  ]
}
```

#### `asset_bridge_holds` (New Read Method)

Request:
```json
{
  "id": "asset-holds-1",
  "method": "asset_bridge_holds",
  "params": {
    "asset_id": "99bb...96_hex",
    "cursor": null,
    "limit": 100
  }
}
```
Response (`schema = "postfiat-asset-bridge-holds-v1"`): returns `{ schema, chain_id, genesis_hash, protocol_version, asset_id, total_held_atoms, hold_count, truncated, next_cursor, holds: Vec<AssetBridgeHoldV1> }`.

---

## 5. Cross-Lane Freeze Summary (Train A)

| Consumer Lane | Frozen Contracts Consumed from Train A |
|---|---|
| **Lane 1 (L1 Integrator)** | 3 governance kinds, `fastpay_minimum_fee_atoms`, `OwnedTransferError` variants, `fastpay_order_v1` index, `AccountTxRow` v6 fields, `nfts_by_id` + `used_nft_serials` + `nft_collection_policies`, `NftBurnOperation.issuer`, `asset_gate(...)`, `AssetBridgeHoldV1`, `bridge_hold_claim`, `bridge_hold_return`, `offer_prune_ineligible`, bounded `asset_clawback`, `RevokeHolderPermit` |
| **Lane 4 (Task Node)** | `fastpay_fee_quote`, `order_hash` as canonical payout idempotency key in `account_tx`/`tx`, paginated `account_nfts`/`issuer_nfts` (`cursor`/`next_cursor`), `NFT_MIGRATED_SERIAL_BIT`, `used_nft_serials` replay guarantee, `account_bridge_holds` |
| **Lane 7 (Explorer)** | `AccountTxRow` (`batch_kind = "fastpay"`, `order_hash`, `certificate_digest`, `memos`), `tx` lookup by `order_hash`, paginated `account_nfts`/`issuer_nfts`, `account_bridge_holds`, `asset_bridge_holds` |
| **Lane 8 (NAVCoin / Adapters)** | `asset_gate(...)` error codes, `AssetBridgeHoldV1` & `AssetBridgeHoldReceiptEventV1`, `bridge_hold_claim` / `bridge_hold_return`, `gated_asset_shielded_pool_forbidden` |
