# L1 Upgrade Train B — Frozen Wire, State & RPC Contract Specification

> **Contract ID:** Phase 0 Contract C4 (Train B)  
> **Status:** Frozen for parallel implementation across Lane 1 (L1 Integrator), Lane 4 (Task Node), Lane 7 (Explorer), and Lane 8 (NAVCoin / Adapters)  
> **Scope:** Pillar 6 (native `identity_set`, 2-tier / 2-of-3 registrar handle reservations & reassignments, consensus uniqueness indexes, safe BIP-340 kill-switch), Pillar 7 (optional `NavFeeScheduleV1` with 5%/yr, 50% HWM, 5% spread, 1% fix, 7-day notice, deterministic `u256` epoch accrual, universal `floor(V * U / S)` check), Pillar 8 M2+M3 (permissionless-by-default DEX/escrow, `refuse_unsolicited_assets` & binding `holder_limit`, Issuer Registry, delegated 0-PFT `credential_create_and_accept`, `did_resolve` for `did:pftl`)  
> **Prerequisite:** [L1 Upgrade Train A Contract](l1-upgrade-train-a-contract.md) (Train B's `nav_fee_claim`, `credential_policy`, and permissionless DEX/escrow auto-lines build directly on Train A's unified `asset_gate(...)`)

---

## 1. Governance Activation Heights & Shared Protocol Clock (Train B)

All Train B consensus rules are gated behind governance amendments in `crates/types/src/shielded_bridge_governance.rs` (latest committed amendment wins):

```rust
pub const GOVERNANCE_KIND_HIVE_IDENTITY_V1_ACTIVATION_HEIGHT: &str =
    "hive_identity_v1_activation_height";
/// Classical proof kill-switch for Pillar 6 (BIP-340 Nostr & old-chain secp256k1 ECDSA).
/// Unset or 0 = classical proofs enabled. Positive height H = classical proofs disabled at >= H,
/// while pure ML-DSA-65 `update`, `rotate_account`, PQ `link`/`unlink`, and `revoke` keep working.
pub const GOVERNANCE_KIND_HIVE_IDENTITY_CLASSICAL_PROOF_KILL_SWITCH: &str =
    "hive_identity_classical_proof_kill_switch";
pub const GOVERNANCE_KIND_NAV_FEE_V1_ACTIVATION_HEIGHT: &str =
    "nav_fee_v1_activation_height";
pub const GOVERNANCE_KIND_CREDENTIAL_DID_V1_ACTIVATION_HEIGHT: &str =
    "credential_did_v1_activation_height";
```

### Shared Protocol Time Constants (1-second nominal block cadence)

Matches existing `MAX_PQ_EXIT_FREEZE_BLOCKS: u64 = 604_800` in `crates/types/src/shielded_bridge_governance.rs`:

```rust
pub const PROTOCOL_BLOCKS_PER_DAY: u64 = 86_400;
pub const PROTOCOL_BLOCKS_PER_7_DAYS: u64 = 604_800;
pub const PROTOCOL_BLOCKS_PER_30_DAYS: u64 = 2_592_000;
pub const PROTOCOL_BLOCKS_PER_YEAR: u64 = 31_536_000;
```

### Transaction Envelope Convention

All Train B write operations are variants of `AssetTransactionOperation` (`crates/types/src/transactions_mempool_receipts.rs`), executed strictly through **ordered consensus** (never FastPay), quoted via `asset_fee_quote`, and submitted via `mempool_submit_signed_asset_transaction` / `mempool_submit_signed_asset_transaction_finality`.

---

## 2. Pillar 6 — Native `identity_set` & 2-of-3 Registrar Handle Reservation

### 2.1 Protocol Constants & Inline Bounds (`crates/types/src/core_chain.rs`)

```rust
pub const IDENTITY_SET_TRANSACTION_KIND: &str = "identity_set";
pub const IDENTITY_REGISTRAR_REGISTER_TRANSACTION_KIND: &str = "identity_registrar_register";
pub const IDENTITY_HANDLE_RESERVE_TRANSACTION_KIND: &str = "identity_handle_reserve";
pub const IDENTITY_HANDLE_REASSIGN_PROPOSE_TRANSACTION_KIND: &str =
    "identity_handle_reassign_propose";
pub const IDENTITY_HANDLE_REASSIGN_EXECUTE_TRANSACTION_KIND: &str =
    "identity_handle_reassign_execute";
pub const IDENTITY_HANDLE_REASSIGN_CANCEL_TRANSACTION_KIND: &str =
    "identity_handle_reassign_cancel";

/// Burned state-expansion fees in PFT atoms (locked in Pillar 6).
pub const IDENTITY_MINT_STATE_EXPANSION_FEE: u64 = 20_000; // 0.02 PFT
pub const IDENTITY_LINK_STATE_EXPANSION_FEE: u64 = 10_000; // 0.01 PFT per new link

/// Strict inline consensus caps.
pub const IDENTITY_MAX_LINKS: usize = 16;
pub const IDENTITY_MAX_RELAYS: usize = 5;
pub const IDENTITY_MAX_RELAY_URL_BYTES: usize = 128;
pub const IDENTITY_HANDLE_MIN_LEN: usize = 2;
pub const IDENTITY_HANDLE_MAX_LEN: usize = 32;
pub const IDENTITY_NAMESPACE_MAX_LEN: usize = 128;
pub const IDENTITY_NOSTR_PUBKEY_HEX_LEN: usize = 64;   // 32-byte x-only secp256k1
pub const IDENTITY_BIP340_SIG_HEX_LEN: usize = 128;    // 64-byte Schnorr signature
pub const IDENTITY_XWING_PUBKEY_HEX_LEN: usize = 2432; // 1216-byte X-Wing (32B X25519 + 1184B ML-KEM-768)
pub const IDENTITY_VALIDATOR_COOLDOWN_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_30_DAYS; // 2_592_000
pub const IDENTITY_HANDLE_REASSIGN_MIN_TIMELOCK_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS; // 604_800
pub const IDENTITY_HANDLE_HOLDBACK_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS; // 604_800

/// Domain separation prefixes.
pub const IDENTITY_LINK_ID_DOMAIN: &str = "postfiat.identity.link_id.v1";
pub const IDENTITY_NOSTR_PROOF_DOMAIN: &str = "postfiat-l1-v2/identity-nostr/v1";
pub const IDENTITY_VALIDATOR_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-link/v1";
pub const IDENTITY_OLD_CHAIN_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-old-chain/v1";
pub const IDENTITY_CONTROLLER_LINK_DOMAIN: &str = "postfiat-l1-v2/identity-controller/v1";
pub const IDENTITY_UNLINK_SELF_DOMAIN: &str = "postfiat-l1-v2/identity-unlink/v1";
pub const IDENTITY_ROTATE_ACCOUNT_DOMAIN: &str = "postfiat-l1-v2/identity-rotate-account/v1";
pub const IDENTITY_HANDLE_RESERVE_DOMAIN: &str = "postfiat.identity.handle_reserve.v1";
pub const IDENTITY_HANDLE_ATTESTATION_DOMAIN: &str = "postfiat-l1-v2/handle-attestation/v1";
pub const IDENTITY_HANDLE_REASSIGN_DOMAIN: &str = "postfiat-l1-v2/handle-reassign/v1";
```

### 2.2 State Structs & Uniqueness Indexes (`crates/types/src/identity_types.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityKind {
    Human,
    Agent,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityRotateMode {
    Planned,
    Compromised,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLinkKind {
    ValidatorNode,    // consensus-verified (ML-DSA-65 node key), globally unique + 30d cooldown
    OldChainAccount,  // consensus-verified (low-S DER secp256k1 ECDSA), globally unique
    AgentController,  // consensus-verified (ML-DSA-65 human controller), non-unique
    CustomNip05,      // client-verified (DNS / nostr.json), non-unique (prevents squatting)
    EvmAddress,       // client-verified, non-unique
    SolanaAddress,    // client-verified, non-unique
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLinkStatus {
    Active,
    PendingReconfirmation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityLinkV1 {
    /// 96 lowercase hex = SHA3-384(IDENTITY_LINK_ID_DOMAIN || account || kind || target || seq).
    pub link_id: String,
    pub kind: IdentityLinkKind,
    pub target: String,
    pub consensus_verified: bool,
    pub status: IdentityLinkStatus,
    pub linked_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityRecordV1 {
    pub account: String,
    /// 64 lowercase hex (32-byte x-only Nostr secp256k1 pubkey).
    pub nostr_pubkey: String,
    /// 1 = legacy old-chain derived v1 npub; 2 = new-chain seed KDF v2 npub.
    pub derivation_version: u8,
    pub key_index: u32,
    pub identity_kind: IdentityKind,
    /// Monotonic identity-local revision counter (starts at 1 on mint).
    pub sequence: u64,
    pub status: IdentityStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registered_namespace: Option<String>,
    /// Canonical lowercase ASCII `[a-z0-9._-]`, 2..=32 chars, no leading/trailing punctuation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handle: Option<String>,
    /// Max 5 `wss://` URLs, each <= 128 bytes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub relays: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_nft_id: Option<String>,
    /// Optional Pillar 2 X-Wing public key (2432 lowercase hex = 1216 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message_key: Option<String>,
    /// Optional Pillar 5 shielded receive address (`pfpq1...`, <= 128 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pq_receive_address: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub links: Vec<IdentityLinkV1>,
    pub created_at_height: u64,
    pub updated_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleNamespaceRegistrarV1 {
    pub namespace: String,               // e.g. "tasknode.postfiat.org"
    pub hot_attestation_account: String, // issues new unclaimed handle attestations & reservations only
    pub cold_signers: [String; 3],       // 3 distinct canonical pf... addresses; 2-of-3 required for reassignment
    pub reassignment_timelock_blocks: u64, // >= 604_800 (7 days)
    pub active: bool,
    pub registered_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleReservationV1 {
    pub namespace: String,
    pub handle: String,
    /// 96 lowercase hex = SHA3-384("postfiat.identity.handle_reserve.v1\0" || handle || "\0" || npub_hex || "\0" || secret_salt_bytes).
    pub commitment_hash: String,
    pub reserved_at_height: u64,
    pub expires_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrarCosignatureV1 {
    pub signer_account: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingHandleReassignmentV1 {
    pub reassignment_id: String, // 96 lowercase hex
    pub namespace: String,
    pub handle: String,
    pub from_account: String,
    pub to_account: String,
    /// "lost_key" | "abuse_impersonation" | "emergency_compromise"
    pub reason_code: String,
    pub emergency_compromise: bool,
    pub proposed_at_height: u64,
    pub effective_at_height: u64,
    pub approvals: Vec<RegistrarCosignatureV1>, // >= 2 distinct cold_signers
    pub state: String, // "pending" | "executed" | "canceled"
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorIdentityBindingV1 {
    pub node_id: String,
    pub account: Option<String>,
    pub bound_at_height: u64,
    pub unbound_at_height: Option<u64>,
}
```

#### `LedgerState` Fields & Global Uniqueness Indexes

Added to `LedgerState` (`#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]` / `"BTreeSet::is_empty"`):
- `identities_by_account: BTreeMap<String, IdentityRecordV1>` — unique `(account)`
- `identity_account_by_nostr: BTreeMap<String, String>` — unique active `(nostr_pubkey)`
- `compromised_nostr_pubkeys: BTreeSet<String>` — permanently unbindable `nostr_pubkey` tombstones
- `identity_account_by_handle: BTreeMap<String, String>` — unique `(namespace, handle)` keyed by `format!("{namespace}\0{handle}")`
- `retired_handle_holdback: BTreeMap<String, u64>` — `(namespace, handle) -> holdback_until_height`
- `identity_validator_bindings: BTreeMap<String, ValidatorIdentityBindingV1>` — unique `(validator node_id)` with 30-day (`2_592_000` block) cooldown after unbind/revoke before binding to a different account
- `identity_account_by_old_chain: BTreeMap<String, String>` — unique `(old_chain_account)`
- `handle_namespace_registrars: BTreeMap<String, HandleNamespaceRegistrarV1>` — keyed by `namespace`
- `handle_reservations: BTreeMap<String, HandleReservationV1>` — keyed by `format!("{namespace}\0{handle}")`
- `pending_handle_reassignments: BTreeMap<String, PendingHandleReassignmentV1>` — keyed by `reassignment_id`

### 2.3 Action Structs (`AssetTransactionOperation` Variants)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandleAttestationV1 {
    pub namespace: String,
    pub handle: String,
    pub account: String,
    pub expires_at_height: u64,
    pub registrar_account: String,
    pub registrar_pubkey_hex: String,
    pub registrar_signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum IdentitySetActionV1 {
    Mint {
        nostr_pubkey: String,
        nostr_proof_sig_hex: String, // BIP-340 over domain + (mint, chain_id, account, tx_seq)
        derivation_version: u8,      // 1 or 2
        key_index: u32,
        identity_kind: IdentityKind,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        relays: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<String>,
    },
    Update {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        relays: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        avatar_nft_id: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message_key: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pq_receive_address: Option<Option<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        handle_attestation: Option<HandleAttestationV1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reservation_salt_hex: Option<String>,
    },
    Link {
        kind: IdentityLinkKind,
        target: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        proof_sig_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        metadata_hash: Option<String>,
    },
    Unlink {
        /// Commits strictly to `link_id` (not owner sequence) so linked-key self-unlink
        /// cannot be front-run by owner updates.
        link_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_pubkey_hex: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        self_unlink_sig_hex: Option<String>,
    },
    Rotate {
        mode: IdentityRotateMode, // planned | compromised
        new_nostr_pubkey: String,
        new_nostr_proof_sig_hex: String,
        derivation_version: u8,
        key_index: u32,
    },
    RotateAccount {
        new_account: String,
        /// Co-signature by destination account (ML-DSA-65 over IDENTITY_ROTATE_ACCOUNT_DOMAIN
        /// || chain_id || old_account || new_account || identity_sequence).
        new_account_pubkey_hex: String,
        new_account_sig_hex: String,
    },
    Revoke {
        reason_code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentitySetOperation {
    pub account: String,
    pub payload: IdentitySetActionV1,
}
```

Registrar operations (`AssetTransactionOperation` variants):
```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReserveOperation {
    pub registrar_account: String,
    pub namespace: String,
    pub handle: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignProposeOperation {
    pub proposer: String, // one of the 3 cold_signers
    pub namespace: String,
    pub handle: String,
    pub from_account: String,
    pub to_account: String,
    pub reason_code: String, // "lost_key" | "abuse_impersonation" | "emergency_compromise"
    pub emergency_compromise: bool,
    /// Must contain >= 2 valid ML-DSA-65 signatures from distinct `cold_signers`.
    pub approvals: Vec<RegistrarCosignatureV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignExecuteOperation {
    pub executor: String, // permissionless once block_height >= effective_at_height
    pub reassignment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityHandleReassignCancelOperation {
    pub caller: String, // current handle owner (unless emergency_compromise) OR cold signer
    pub reassignment_id: String,
    /// Required if caller is a cold signer rather than current handle owner (>= 2 cold signers).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub approvals: Vec<RegistrarCosignatureV1>,
}
```

### 2.4 Critical Consensus Rules (Pillar 6)

1. **BIP-340 & low-S DER verification:** BIP-340 proofs use pure-Rust `k256` with domain separation `(IDENTITY_NOSTR_PROOF_DOMAIN, op, chain_id, account, tx_sequence)`. Old-chain links verify strict low-S DER secp256k1 ECDSA over `(IDENTITY_OLD_CHAIN_LINK_DOMAIN, chain_id, account, old_chain_account, tx_sequence)`.
2. **Safe classical kill-switch:** When `block_height >= hive_identity_classical_proof_kill_switch` (if set and > 0): `Mint`, `Rotate`, and `Link(OldChainAccount)` fail with `"identity_classical_proof_disabled"`, while pure ML-DSA-65 `Update`, `Link(ValidatorNode | AgentController | CustomNip05 | EvmAddress | SolanaAddress)`, `Unlink`, `RotateAccount`, and `Revoke` continue functioning with zero deadlock.
3. **2-tier handle registrar:**
   - `hot_attestation_account` can ONLY attest **unclaimed** handles (and publish `HandleReservationV1`). If a handle is currently claimed or under `retired_handle_holdback`, hot attestations are rejected (`"identity_handle_already_claimed"` / `"identity_handle_in_holdback"`).
   - `identity_handle_reassign_propose` requires >= 2 valid ML-DSA-65 signatures from distinct `cold_signers`. Unless `emergency_compromise == true`, `effective_at_height = block_height + registrar.reassignment_timelock_blocks` (>= 7 days) and the current owner may cancel at any height `< effective_at_height`.
4. **Salted handle reservation claim:** If an unexpired `HandleReservationV1` exists for `(namespace, handle)`, claiming requires `reservation_salt_hex` such that `SHA3-384("postfiat.identity.handle_reserve.v1\0" || handle || "\0" || nostr_pubkey_hex || "\0" || salt_bytes) == reservation.commitment_hash`.
5. **Account rotation safety:** `RotateAccount` requires BOTH `tx.source == old_account` AND valid `new_account_sig_hex`. On apply, any `ValidatorNode` or `AgentController` links transition to `IdentityLinkStatus::PendingReconfirmation` until re-co-signed against `new_account`.
6. **Compromised key tombstone:** `Rotate { mode: Compromised, .. }` inserts the old `nostr_pubkey` into `compromised_nostr_pubkeys` permanently.

### 2.5 Pillar 6 Error Codes

| `receipt_code` | Trigger |
|---|---|
| `identity_already_exists` | `mint` for an account that already has an active identity |
| `identity_not_found` | `update` / `link` / `unlink` / `rotate` / `rotate_account` / `revoke` on missing identity |
| `identity_revoked` | Mutating action on an already-revoked identity |
| `identity_nostr_pubkey_taken` | `nostr_pubkey` already bound to another active identity |
| `identity_nostr_pubkey_compromised` | `nostr_pubkey` is in `compromised_nostr_pubkeys` |
| `identity_invalid_bip340_proof` | Invalid BIP-340 Schnorr proof or domain mismatch |
| `identity_classical_proof_disabled` | Classical proof submitted after `hive_identity_classical_proof_kill_switch` |
| `identity_invalid_handle` | Handle violates `[a-z0-9._-]`, length 2..=32, or leading/trailing punctuation |
| `identity_handle_already_claimed` | `(namespace, handle)` is currently bound |
| `identity_handle_in_holdback` | Retired handle still inside 7-day holdback window |
| `identity_handle_reservation_mismatch` | Active salted reservation exists and supplied salt/pubkey does not match commitment |
| `identity_invalid_registrar_attestation` | Invalid/expired hot registrar signature or unknown namespace |
| `identity_reassign_insufficient_quorum` | Fewer than 2 distinct valid `cold_signers` signatures |
| `identity_reassign_timelock_active` | `identity_handle_reassign_execute` called before `effective_at_height` |
| `identity_reassign_cannot_cancel_emergency` | Owner tried to cancel an `emergency_compromise` 2-of-3 reassignment |
| `identity_link_limit_exceeded` | `links.len() >= 16` |
| `identity_validator_cooldown_active` | Validator `node_id` unbound < 30 days (`2_592_000` blocks) ago |
| `identity_validator_already_linked` | Validator `node_id` already bound to another active account |
| `identity_old_chain_already_linked` | `old_chain_account` already bound to another active account |
| `identity_invalid_link_proof` | Invalid validator ML-DSA, controller ML-DSA, or low-S DER old-chain ECDSA signature |
| `identity_rotate_account_cosig_invalid` | Missing/invalid `new_account` co-signature or destination already has an identity |

### 2.6 RPC Request & Response JSON (Pillar 6)

All 4 read RPCs (`identity_info`, `identity_by_nostr`, `identity_by_handle`, `identity_by_link`) return a unified `IdentityLookupReport` (`schema = "postfiat-identity-lookup-v1"`):

| Method | Required `params` | Optional `params` |
|---|---|---|
| `identity_info` | `account` (`pf...`) | — |
| `identity_by_nostr` | `nostr_pubkey` (64 hex) | — |
| `identity_by_handle` | `handle` | `namespace` (default `"tasknode.postfiat.org"`) |
| `identity_by_link` | `link_kind`, `target` | — |

Example `identity_by_handle` Response:
```json
{
  "id": "id-1",
  "ok": true,
  "result": {
    "schema": "postfiat-identity-lookup-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca...96_hex",
    "protocol_version": 1,
    "block_height": 150200,
    "found": true,
    "compromised_nostr_pubkey": false,
    "identity": {
      "account": "pf1111111111111111111111111111111111111111",
      "nostr_pubkey": "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
      "derivation_version": 1,
      "key_index": 0,
      "identity_kind": "human",
      "sequence": 3,
      "status": "active",
      "registered_namespace": "tasknode.postfiat.org",
      "handle": "goodalexander",
      "relays": ["wss://relay.postfiat.org"],
      "avatar_nft_id": "0a11...96_hex",
      "message_key": "ab12...2432_hex",
      "pq_receive_address": "pfpq1...",
      "links": [
        {
          "link_id": "cc44...96_hex",
          "kind": "validator_node",
          "target": "validator-01",
          "consensus_verified": true,
          "status": "active",
          "linked_at_height": 150100
        }
      ],
      "created_at_height": 150000,
      "updated_at_height": 150100
    }
  },
  "events": [
    { "kind": "identity_by_handle", "target": "goodalexander@tasknode.postfiat.org", "message": "identity queried" }
  ]
}
```

---

## 3. Pillar 7 — Optional `NavFeeScheduleV1` & Universal `floor(V * U / S)` Check

### 3.1 Protocol Constants (`crates/types/src/core_chain.rs`)

```rust
pub const NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND: &str = "nav_fee_schedule_attach";
pub const NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND: &str = "nav_fee_schedule_set";
pub const NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND: &str = "nav_fee_schedule_cancel";
pub const NAV_FEE_CLAIM_TRANSACTION_KIND: &str = "nav_fee_claim";

/// Locked protocol caps (Pillar 7).
pub const NAV_FEE_BPS_DENOMINATOR: u64 = 10_000;
pub const NAV_FEE_MAX_ANNUAL_RATE_BPS: u32 = 500;       // 5% / year
pub const NAV_FEE_MAX_HWM_RATE_BPS: u32 = 5_000;         // 50% of gain above HWM
pub const NAV_FEE_MAX_SPREAD_BPS: u32 = 500;             // 5% primary issue/redeem spread
pub const NAV_FEE_MAX_FIX_FEE_CAP_BPS: u32 = 100;        // 1% FX fix fee cap
pub const NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS; // 604_800
pub const NAV_FEE_MAX_DISCLOSURE_REF_BYTES: usize = 128;
```

### 3.2 State Structs (`crates/types/src/account_owned_asset_types.rs` & `market_nav_asset_types.rs`)

Strictly optional on `NavTrackedAsset` (zero fee state and zero UI clutter for unscheduled NAVCoins):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleV1 {
    /// Annual time-based rate in bps (0..=500).
    pub annual_rate_bps: u32,
    /// High-water-mark gain rate in bps (0..=5_000).
    pub hwm_rate_bps: u32,
    /// Primary issue spread cap in bps (0..=500).
    pub issue_spread_bps: u32,
    /// Primary redeem spread cap in bps (0..=500).
    pub redeem_spread_bps: u32,
    /// FX fix fee cap in bps (0..=100).
    pub fix_fee_cap_bps: u32,
    /// Canonical `pf...` account receiving claimed fee units; MUST NOT equal `nav_asset.issuer`.
    pub recipient: String,
    /// Notice period in blocks for any fee increase (>= 604_800).
    pub increase_notice_blocks: u64,
    /// Max blocks accrued per finalization interval (> 0, <= profile.max_epoch_gap_blocks when set).
    pub accrual_cap_blocks: u64,
    /// One-way lock (`false -> true`): permanently freezes all rate ceilings & `accrual_cap_blocks`
    /// while still permitting visible `recipient` rotation (with 7-day notice if unreachable/lost).
    #[serde(default)]
    pub rates_locked: bool,
    /// Optional IPFS CID or Pillar 2 message pointer (<= 128 bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disclosure_ref: Option<String>,
    pub attached_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingNavFeeScheduleV1 {
    pub schedule: NavFeeScheduleV1,
    pub proposed_at_height: u64,
    /// First block height at which this pending schedule activates on finalization.
    pub effective_at_height: u64,
    pub is_increase: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeStateV1 {
    pub active: NavFeeScheduleV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending: Option<PendingNavFeeScheduleV1>,
    /// High-water mark in valuation atoms per unit (initialized to `N_current` on first S > 0 finalization).
    pub high_water_mark_nav_per_unit: u64,
    /// Block height of last accrual (initialized to `h_attach`, never 0).
    pub last_accrual_height: u64,
    /// Unclaimed accrued fee units in protocol bucket (included in `circulating_supply`).
    pub unclaimed_fee_units: u64,
    pub total_accrued_fee_units: u64,
    pub total_claimed_fee_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeAccrualV1 {
    pub asset_id: String,
    pub epoch: u64,
    pub block_height: u64,
    /// Exact protocol constant `PROTOCOL_BLOCKS_PER_YEAR` (31_536_000) used for recomputation.
    pub blocks_per_year: u64,
    pub delta_blocks: u64,
    pub verified_net_assets: u64,
    pub supply_before: u64,
    pub nav_before: u64,
    pub hwm_before: u64,
    pub annual_rate_bps_applied: u32,
    pub hwm_rate_bps_applied: u32,
    pub time_fee_value: u64,
    pub nav_after_time_fee: u64,
    pub hwm_fee_value: u64,
    pub total_fee_value: u64,
    pub minted_fee_units: u64,
    pub clipped_fee_units: u64,
    pub supply_after: u64,
    pub nav_after: u64,
    pub hwm_after: u64,
    /// "accrued" | "zero_supply" | "zero_net_assets" | "zero_rates" |
    /// "zero_delta_blocks" | "clamped_max_supply" | "clamped_full_value"
    pub reason_code: String,
}
```

Added to `NavTrackedAsset`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub fee_state: Option<NavFeeStateV1>,
```

Added to `LedgerState`:
```rust
/// Per-epoch deterministic fee accrual records, keyed by `format!("{asset_id}:{epoch:020}")`.
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub nav_fee_accruals: BTreeMap<String, NavFeeAccrualV1>,
```

### 3.3 Action Structs (`AssetTransactionOperation` Variants)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleInputV1 {
    pub annual_rate_bps: u32,
    pub hwm_rate_bps: u32,
    pub issue_spread_bps: u32,
    pub redeem_spread_bps: u32,
    pub fix_fee_cap_bps: u32,
    pub recipient: String,
    pub increase_notice_blocks: u64,
    pub accrual_cap_blocks: u64,
    #[serde(default)]
    pub rates_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disclosure_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleAttachOperation {
    pub issuer: String,
    pub asset_id: String,
    pub schedule: NavFeeScheduleInputV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleSetOperation {
    pub issuer: String,
    pub asset_id: String,
    pub schedule: NavFeeScheduleInputV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeScheduleCancelOperation {
    pub issuer: String,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavFeeClaimOperation {
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
}
```

- `source_matches`: `issuer == source` for `attach`/`set`/`cancel`; `recipient == source` for `nav_fee_claim`.
- **What counts as an increase (waits `>= current.increase_notice_blocks >= 604_800` blocks):**
  Any increase to `annual_rate_bps`, `hwm_rate_bps`, `issue_spread_bps`, `redeem_spread_bps`, `fix_fee_cap_bps`, or `accrual_cap_blocks`, OR any decrease to `increase_notice_blocks`.
- **`rates_locked == true` rule:** Any increase to any rate/cap or decrease to `increase_notice_blocks` is permanently rejected (`"nav_fee_schedule_rates_locked"`). Changing `recipient` alone is allowed (immediate if co-signed or visible event with 7-day notice if `recipient` changed unilaterally by issuer).
- **Pure fee decreases:** Take effect at the next `nav_epoch_finalize` (`effective_at_height = block_height`, `is_increase = false`).
- **Legacy spread bypass closed:** On unscheduled NAVCoins (`fee_state == None`), any `pftl_uniswap_route_epoch_advance` / policy update raising `issue_multiplier_bps` above the prior value or lowering `redeem_multiplier_bps` below the prior value requires the same 7-day (`604_800` block) pending notice before activation!
- **Exit capacity lock during pending increases:** While `fee_state.pending.is_some_and(|p| p.is_increase)`, any route update reducing `redeem_capacity_atoms` is rejected with `"nav_fee_pending_increase_exit_capacity_locked"`.

### 3.4 Exact `u256` Deterministic Epoch Accrual & Universal `floor(V * U / S)` Check

Active at `block_height >= nav_fee_v1_activation_height`:

1. **Universal `floor(V * U / S)` check (ALL NAVCoins, ALL proof profiles, scheduled OR unscheduled):**
   - Let `U = 10^asset.precision`.
   - When `packet.circulating_supply > 0`:
     `expected_nav = floor(u256(packet.verified_net_assets) * u256(U) / u256(packet.circulating_supply))`
     MUST equal `packet.nav_per_unit` (else reject with `"nav_per_unit_formula_mismatch"`).
   - At `nav_epoch_finalize`: `packet.circulating_supply` MUST equal `live_ledger_supply(ledger, asset_id)` (which includes `trustlines + escrows + offers + bridge_holds + orchard + unclaimed_fee_units`), else reject with `"nav_supply_snapshot_stale"`.
2. **Non-issuer attestor enforcement:**
   - If `nav_asset.fee_state.is_some()` and `packet.attested_value > 0`:
     Consensus requires at least one valid `NavReserveAttestation` with `pass == true` from a registered `NavAttestor` where `attestor != nav_asset.issuer`, else reject with `"nav_fee_issuer_self_attestation_forbidden"`.
3. **Deterministic 4-step `u256` accrual inside `nav_epoch_finalize` (when `fee_state` is `Some`):**
   - **Pending activation & mixed-rate interval rule:** If `pending` exists and `block_height >= pending.effective_at_height`, promote `pending.schedule` to `active` after computing the interval using `m = min(old.annual_rate_bps, new.annual_rate_bps)` and `p = min(old.hwm_rate_bps, new.hwm_rate_bps)` (preventing retroactive charging over prior blocks).
   - **Exact initialization:** On attach at height `h_attach`, `last_accrual_height = h_attach` (never `0`) and `high_water_mark_nav_per_unit = nav_asset.nav_per_unit` (or initialized to `N_before` on the first finalization where `S > 0` if `nav_per_unit == 0` at attach).
   - **Edge fail-safes (never fail finalization):** If `S == 0` -> record `NavFeeAccrualV1` with `minted_fee_units = 0`, `reason_code = "zero_supply"`, update `last_accrual_height = block_height`. If `V == 0` -> `reason_code = "zero_net_assets"`.
   - **Step 1 (Time-based fee):**
     - `Δh = min(block_height.saturating_sub(last_accrual_height), active.accrual_cap_blocks)`
     - `F_m = floor(u256(V) * u256(m) * u256(Δh) / (10_000 * u256(31_536_000)))`
     - `N_m = floor(u256(V - F_m) * u256(U) / u256(S))`
   - **Step 2 (High-water-mark gain fee):**
     - `g = N_m.saturating_sub(H)`
     - `F_p = floor(u256(g) * u256(S) * u256(p) / (10_000 * u256(U)))`
   - **Step 3 (Atomic unit mint into unclaimed bucket — bypasses `asset_gate`):**
     - `F = F_m + F_p` (if `F >= V`, clamp to `0` with `reason_code = "clamped_full_value"`)
     - `ΔS_raw = floor(u256(F) * u256(S) / u256(V - F))`
     - If `asset.max_supply` is `Some(cap)` and `S + ΔS_raw > cap`: `ΔS = cap.saturating_sub(S)`, `clipped = ΔS_raw - ΔS`, `reason_code = "clamped_max_supply"` (forgone; zero carry-forward debt).
   - **Step 4 (State update & Ethereum sync):**
     - `S' = S + ΔS`
     - `N_after = floor(u256(V) * u256(U) / u256(S'))`
     - `H' = max(H, N_after)`
     - `fee_state.unclaimed_fee_units += ΔS`; `fee_state.total_accrued_fee_units += ΔS`; `fee_state.high_water_mark_nav_per_unit = H'`; `fee_state.last_accrual_height = block_height`.
     - Store `nav_asset.nav_per_unit = N_after`, `nav_asset.circulating_supply = S'`.
     - Store `NavFeeAccrualV1` in `ledger.nav_fee_accruals`.
     - Signed epoch packets relayed to Ethereum (`NAVGuardHook.sol` / `PFTLUniswapPrimaryMarketV2.sol`) commit to post-fee `N_after`, and primary routes reject dealing on stale pre-fee epochs until re-pinned.
4. **FX Fix fee cap check:** At `fx_fix_register_v1`, if `base_asset_id` or `quote_asset_id` is a `NavTrackedAsset`, enforce `packet.fee_bps <= NAV_FEE_MAX_FIX_FEE_CAP_BPS` (`100`) and, when a `NavFeeScheduleV1` is attached, `packet.fee_bps <= schedule.fix_fee_cap_bps` (else reject with `"fx_fix_fee_exceeds_nav_cap"`).

### 3.5 Pillar 7 Error Codes

| `receipt_code` | Trigger |
|---|---|
| `nav_fee_schedule_already_attached` | `nav_fee_schedule_attach` on asset that already has `fee_state` |
| `nav_fee_schedule_missing` | `nav_fee_schedule_set` / `cancel` / `claim` when no schedule attached (for non-spread set) |
| `nav_fee_rate_cap_exceeded` | `annual_rate_bps > 500`, `hwm_rate_bps > 5_000`, `*_spread_bps > 500`, or `fix_fee_cap_bps > 100` |
| `nav_fee_notice_too_short` | `increase_notice_blocks < 604_800` |
| `nav_fee_recipient_equals_issuer` | `schedule.recipient == nav_asset.issuer` |
| `nav_fee_schedule_rates_locked` | Attempted rate/cap increase or notice reduction after `rates_locked = true` |
| `nav_fee_no_pending_schedule` | `nav_fee_schedule_cancel` when `fee_state.pending.is_none()` |
| `nav_fee_pending_increase_exit_capacity_locked` | Route update attempted to lower `redeem_capacity_atoms` while a fee increase is pending |
| `nav_fee_insufficient_unclaimed` | `nav_fee_claim.amount > fee_state.unclaimed_fee_units` |
| `nav_per_unit_formula_mismatch` | `packet.nav_per_unit != floor(V * U / S)` |
| `nav_supply_snapshot_stale` | `packet.circulating_supply != live_ledger_supply` at finalization |
| `nav_fee_issuer_self_attestation_forbidden` | Fee-enabled NAVCoin packet with `attested_value > 0` lacks non-issuer `NavAttestor` pass attestation |
| `fx_fix_fee_exceeds_nav_cap` | `fx_fix_register_v1` `fee_bps` exceeds `100` or attached `fix_fee_cap_bps` |

### 3.6 RPC Request & Response JSON (Pillar 7)

#### `nav_fee_schedule` (New Read Method)

Request:
```json
{
  "id": "nav-fee-1",
  "method": "nav_fee_schedule",
  "params": {
    "asset_id": "99bb...96_hex"
  }
}
```

Response (`schema = "postfiat-nav-fee-schedule-v1"`):
```json
{
  "id": "nav-fee-1",
  "ok": true,
  "result": {
    "schema": "postfiat-nav-fee-schedule-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca...96_hex",
    "protocol_version": 1,
    "block_height": 200500,
    "asset_id": "99bb...96_hex",
    "found": true,
    "has_fee_schedule": true,
    "blocks_per_year": 31536000,
    "fee_state": {
      "active": {
        "annual_rate_bps": 100,
        "hwm_rate_bps": 1000,
        "issue_spread_bps": 50,
        "redeem_spread_bps": 50,
        "fix_fee_cap_bps": 25,
        "recipient": "pf3333333333333333333333333333333333333333",
        "increase_notice_blocks": 604800,
        "accrual_cap_blocks": 86400,
        "rates_locked": false,
        "disclosure_ref": "ipfs://bafkrei...",
        "attached_at_height": 180000
      },
      "pending": null,
      "high_water_mark_nav_per_unit": 108990100,
      "last_accrual_height": 200000,
      "unclaimed_fee_units": 9265979203,
      "total_accrued_fee_units": 9265979203,
      "total_claimed_fee_units": 0
    }
  },
  "events": [
    { "kind": "nav_fee_schedule", "target": "99bb...96_hex", "message": "nav fee schedule queried" }
  ]
}
```
(When an asset has no fee schedule configured, `has_fee_schedule: false`, `fee_state: null`.)

#### `nav_fee_history` (New Read Method)

Request:
```json
{
  "id": "nav-fee-hist-1",
  "method": "nav_fee_history",
  "params": {
    "asset_id": "99bb...96_hex",
    "from_epoch": 1,
    "limit": 50,
    "cursor": null
  }
}
```

Response (`schema = "postfiat-nav-fee-history-v1"`):
```json
{
  "id": "nav-fee-hist-1",
  "ok": true,
  "result": {
    "schema": "postfiat-nav-fee-history-v1",
    "chain_id": "postfiat-wan-devnet-2",
    "genesis_hash": "ce22ca...96_hex",
    "protocol_version": 1,
    "asset_id": "99bb...96_hex",
    "accrual_count": 1,
    "truncated": false,
    "next_cursor": null,
    "accruals": [
      {
        "asset_id": "99bb...96_hex",
        "epoch": 12,
        "block_height": 200000,
        "blocks_per_year": 31536000,
        "delta_blocks": 315360,
        "verified_net_assets": 1100000000000,
        "supply_before": 1000000000000,
        "nav_before": 110000000,
        "hwm_before": 100000000,
        "annual_rate_bps_applied": 100,
        "hwm_rate_bps_applied": 1000,
        "time_fee_value": 110000000,
        "nav_after_time_fee": 109989000,
        "hwm_fee_value": 9989000000,
        "total_fee_value": 10099000000,
        "minted_fee_units": 9265979203,
        "clipped_fee_units": 0,
        "supply_after": 1009265979203,
        "nav_after": 108990100,
        "hwm_after": 108990100,
        "reason_code": "accrued"
      }
    ]
  },
  "events": [
    { "kind": "nav_fee_history", "target": "99bb...96_hex", "message": "nav fee history queried" }
  ]
}
```

---

## 4. Pillar 8 (M2 + M3) — Permissionless DEX/Escrow, Issuer Registry, Delegated Credentials & `did:pftl`

### 4.1 Permissionless-by-Default DEX/Escrow & Holder Controls (Pillar 8 M2)

Active at `block_height >= credential_did_v1_activation_height`:

1. **Account flag (`crates/types/src/account_owned_asset_types.rs`):**
   ```rust
   pub const ACCOUNT_FLAG_REQUIRE_MEMO: u32 = 0x0000_0001;
   pub const ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS: u32 = 0x0000_0002;
   pub const ACCOUNT_ALLOWED_FLAGS: u32 =
       ACCOUNT_FLAG_REQUIRE_MEMO | ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS;
   ```
   Toggled by account owner via existing `account_set` operation (off by default for normal users; opt-in for institutional/custodial wallets).
2. **Binding `holder_limit` & `implicit` flag on `TrustLine`:**
   ```rust
   pub struct TrustLine {
       // ... existing fields unchanged ...
       /// Binding ceiling explicitly set by the holder via `trust_set`.
       /// Once `Some(cap)`, credits NEVER auto-raise `limit` above `cap`.
       #[serde(default, skip_serializing_if = "Option::is_none")]
       pub holder_limit: Option<u64>,
       /// True when auto-created without explicit `trust_set`; auto-pruned when zeroed.
       #[serde(default, skip_serializing_if = "is_false")]
       pub implicit: bool,
   }
   ```
3. **Auto-create missing lines on `offer_create` and `escrow_create`:**
   - For default assets (`!asset.requires_authorization`): `offer_create` (buy-side), offer fills, and `escrow_create` / `escrow_finish` auto-create missing implicit `TrustLine` records with `authorized = false`, `implicit = true` (charging 10-atom `TRUSTLINE_STATE_EXPANSION_FEE`). Task Node removes its extra `trust_set` step for default assets.
   - For gated assets (`asset.requires_authorization == true`): auto-creates missing line (`authorized = false`, `implicit = true`) **only** when the holder passes `asset_gate(...)` at `block_height`.

### 4.2 Protocol Constants (Pillar 8 M3)

```rust
pub const ISSUER_REGISTER_TRANSACTION_KIND: &str = "issuer_register";
pub const ISSUER_UPDATE_TRANSACTION_KIND: &str = "issuer_update";
pub const ISSUER_ADMIN_TRANSFER_TRANSACTION_KIND: &str = "issuer_admin_transfer";
pub const ISSUER_DEREGISTER_TRANSACTION_KIND: &str = "issuer_deregister";
pub const CREDENTIAL_CREATE_TRANSACTION_KIND: &str = "credential_create";
pub const CREDENTIAL_ACCEPT_TRANSACTION_KIND: &str = "credential_accept";
pub const CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND: &str = "credential_create_and_accept";
pub const CREDENTIAL_REVOKE_TRANSACTION_KIND: &str = "credential_revoke";
pub const CREDENTIAL_DELETE_TRANSACTION_KIND: &str = "credential_delete";
pub const ASSET_ACCESS_SET_TRANSACTION_KIND: &str = "asset_access_set";

pub const CREDENTIAL_ISSUER_ID_DOMAIN: &str = "postfiat.credential_issuer_id.v1";
pub const CREDENTIAL_ID_DOMAIN: &str = "postfiat.credential_id.v1";
pub const CREDENTIAL_DELEGATED_ACCEPT_DOMAIN: &str = "postfiat.credential.delegated_accept.v1";

pub const CREDENTIAL_DEPOSIT_ATOMS: u64 = 10_000; // 0.01 PFT refundable deposit returned on delete
pub const MAX_ISSUER_SIGNING_ACCOUNTS: usize = 16;
pub const MAX_ISSUER_CREDENTIAL_TYPES: usize = 32;
pub const MAX_ISSUER_VENDOR_DIDS: usize = 8;
pub const MAX_CREDENTIAL_POLICY_OR_CLAUSES: usize = 8;
pub const MAX_CREDENTIAL_POLICY_AND_PAIRS: usize = 4;
pub const ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS: u64 = PROTOCOL_BLOCKS_PER_7_DAYS; // 604_800
```

### 4.3 State Structs (`crates/types/src/credential_did_types.rs`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerSigningAccountV1 {
    pub account: String,
    pub added_at_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
    /// Credentials signed by this account with `issued_height <= compromise_cutoff_height`
    /// are treated as invalid immediately by `asset_gate(...)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compromise_cutoff_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialIssuerV1 {
    /// 96 lowercase hex = SHA3-384(CREDENTIAL_ISSUER_ID_DOMAIN || chain_id || admin || nonce).
    pub issuer_id: String,
    pub admin_account: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_admin_account: Option<String>,
    /// Verified off-chain by Explorer & W3C DID resolvers (NEVER DNS inside consensus).
    pub domain: String,
    pub signing_accounts: Vec<IssuerSigningAccountV1>,
    /// Namespaced `<domain>/<type>` strings (e.g. `"vendor.example/kyc.basic"`).
    pub allowed_credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_dids: Vec<String>,
    /// "active" | "deregistered"
    pub status: String,
    pub created_at_height: u64,
    pub updated_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRecordV1 {
    /// 96 lowercase hex = SHA3-384(CREDENTIAL_ID_DOMAIN || subject || issuer_id || credential_type).
    pub credential_id: String,
    pub subject: String,
    pub issuer_id: String,
    pub credential_type: String,
    pub signing_account: String,
    /// 96 lowercase hex salted hash commitment — ZERO PII stored on chain.
    pub commitment_hash: String,
    pub issued_height: u64,
    pub expires_at_height: u64,
    /// "pending" | "accepted" | "revoked"
    pub status: String,
    pub deposit_paid: u64,
    pub deposit_payer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted_at_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_at_height: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetApprovalMode {
    Either, // manual line.authorized OR valid credential_policy match
    Both,   // manual line.authorized AND valid credential_policy match
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetLapsedExitRule {
    ReturnOnly,           // default: AssetBurn, NavRedeemAtNav, BridgeHoldReturn, pay-issuer allowed
    StrictFreezeClawback, // blocks all non-clawback outflows once lapsed
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetWrapperMode {
    PftlOnly,
    PftlPlusEthereumFreeWrapper,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRequirementPairV1 {
    pub issuer_id: String,
    pub credential_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyRuleV1 {
    /// AND-list of required (issuer_id, credential_type) pairs (1..=4).
    pub all_of: Vec<CredentialRequirementPairV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPolicyV1 {
    /// OR-list of AND-rules (0..=8; empty means manual-approval only).
    pub any_of: Vec<CredentialPolicyRuleV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingAssetAccessPolicyV1 {
    pub approval_mode: AssetApprovalMode,
    pub lapsed_exit_rule: AssetLapsedExitRule,
    pub wrapper_mode: AssetWrapperMode,
    pub credential_policy: CredentialPolicyV1,
    pub proposed_at_height: u64,
    pub effective_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessPolicyV1 {
    pub approval_mode: AssetApprovalMode,
    pub lapsed_exit_rule: AssetLapsedExitRule,
    pub wrapper_mode: AssetWrapperMode,
    pub credential_policy: CredentialPolicyV1,
    pub notice_blocks: u64, // >= 604_800
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_policy: Option<PendingAssetAccessPolicyV1>,
}
```

Added to `AssetDefinition`:
```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub access_policy: Option<AssetAccessPolicyV1>,
```

Added to `LedgerState`:
```rust
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub credential_issuers: BTreeMap<String, CredentialIssuerV1>,
#[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
pub credentials: BTreeMap<String, CredentialRecordV1>,
```

### 4.4 Action Structs (`AssetTransactionOperation` Variants)

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerRegisterOperation {
    pub admin_account: String,
    pub domain: String,
    pub signing_accounts: Vec<String>,
    pub allowed_credential_types: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vendor_dids: Vec<String>,
    pub registration_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerUpdateOperation {
    pub admin_account: String,
    pub issuer_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub add_signing_accounts: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revoke_signing_accounts: Vec<String>,
    /// Pairs of `(signing_account, compromise_cutoff_height)` invalidating older credentials.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compromise_cutoffs: Vec<(String, u64)>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_credential_types: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor_dids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssuerAdminTransferAction {
    Propose,
    Accept,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerAdminTransferOperation {
    pub caller: String,
    pub issuer_id: String,
    pub action: IssuerAdminTransferAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_admin_account: Option<String>, // required for Propose
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerDeregisterOperation {
    pub admin_account: String,
    pub issuer_id: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialCreateOperation {
    pub signing_account: String,
    pub issuer_id: String,
    pub subject: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialAcceptOperation {
    pub subject: String,
    pub credential_id: String,
}

/// Atomic delegated issuance + acceptance so brand-new 0-PFT accounts never hit
/// a gas chicken-and-egg deadlock. Issuer signing account pays gas + deposit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialCreateAndAcceptOperation {
    pub signing_account: String,
    pub issuer_id: String,
    pub subject: String,
    pub credential_type: String,
    pub commitment_hash: String,
    pub expires_at_height: u64,
    /// Subject's ML-DSA-65 public key (must derive to `subject` address).
    pub subject_pubkey_hex: String,
    /// Subject's ML-DSA-65 signature over `CREDENTIAL_DELEGATED_ACCEPT_DOMAIN` preimage
    /// binding `(chain_id, genesis_hash, subject, issuer_id, credential_type,
    /// commitment_hash, expires_at_height, subject_accept_nonce)`.
    pub subject_accept_signature_hex: String,
    pub subject_accept_nonce: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialRevokeOperation {
    pub caller: String, // issuer admin_account or active signing_account
    pub credential_id: String,
    pub reason_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialDeleteOperation {
    /// Subject, issuer admin/signing account, OR anyone once `block_height > expires_at_height`.
    pub caller: String,
    pub credential_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetAccessSetOperation {
    pub issuer: String,
    pub asset_id: String,
    /// One-way open: may flip `requires_authorization` from `true -> false`, NEVER `false -> true`.
    #[serde(default)]
    pub disable_authorization_permanently: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approval_mode: Option<AssetApprovalMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lapsed_exit_rule: Option<AssetLapsedExitRule>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrapper_mode: Option<AssetWrapperMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_policy: Option<CredentialPolicyV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice_blocks: Option<u64>,
    /// Emergency escape hatch: immediately strips a deregistered or compromised issuer_id
    /// from `credential_policy` without waiting 7 days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emergency_remove_issuer_id: Option<String>,
}
```

### 4.5 Critical Consensus Rules (Pillar 8 M3)

1. **Delegated 0-PFT `credential_create_and_accept`:**
   - Verifies `address_from_public_key(&op.subject_pubkey_hex) == op.subject`.
   - Verifies `ml_dsa_65_verify_with_context` over exact preimage:
     `postfiat.credential.delegated_accept.v1\nchain_id={}\ngenesis_hash={}\nsubject={}\nissuer_id={}\ncredential_type={}\ncommitment_hash={}\nexpires_at_height={}\nsubject_accept_nonce={}\n`.
   - Does **not** require `subject` to hold any PFT balance (creates `CredentialRecordV1` directly with `status = "accepted"`, `accepted_at_height = Some(block_height)`).
2. **Safe renewal (compromise-cutoff closure):** Calling `credential_create` or `credential_create_and_accept` for an existing `credential_id` **overwrites `issued_height = block_height`, `signing_account = op.signing_account`, `commitment_hash`, `expires_at_height`, and `status`** (reusing existing `deposit_paid` without double-charging), closing the bypass where a leaked key could renew an older record.
3. **Live validity check inside `asset_gate(...)`:** A credential satisfies `(issuer_id, credential_type)` at `block_height` iff:
   - `record.status == "accepted"`
   - `block_height <= record.expires_at_height`
   - `issuer.status == "active"`
   - The `IssuerSigningAccountV1` matching `record.signing_account` has `compromise_cutoff_height.is_none_or(|c| record.issued_height > c)`.
4. **Policy change rules (`asset_access_set`):**
   - `requires_authorization` can NEVER flip `false -> true` (`"asset_gating_retroactive_enable_forbidden"`).
   - Loosening policy (adding an OR-clause, removing an AND-requirement, switching `both -> either`, switching `strict_freeze_clawback -> return_only`) takes effect immediately.
   - Tightening policy schedules `pending_policy` with `effective_at_height = block_height + policy.notice_blocks` (`>= 604_800`).
   - `emergency_remove_issuer_id` takes effect immediately iff the target issuer is `status == "deregistered"` OR has at least one `compromise_cutoff_height.is_some()`.
5. **Refundable deposit:** `credential_create` / `credential_create_and_accept` locks `CREDENTIAL_DEPOSIT_ATOMS` (`10_000`) from `signing_account` (recorded in `deposit_payer`). `credential_delete` removes the record and refunds `deposit_paid` to `deposit_payer`.

### 4.6 Pillar 8 (M2 + M3) Error Codes

| `receipt_code` | Trigger |
|---|---|
| `issuer_already_exists` | Duplicate `issuer_id` on `issuer_register` |
| `issuer_not_found` | Referenced `issuer_id` does not exist |
| `issuer_deregistered` | Issuance or update attempted on a deregistered issuer |
| `issuer_unauthorized` | Caller is not `admin_account` / active `signing_account` / `pending_admin_account` |
| `issuer_invalid_credential_type` | `credential_type` not prefixed by `<domain>/` or not in `allowed_credential_types` |
| `issuer_limit_exceeded` | Signing accounts > 16, credential types > 32, or vendor DIDs > 8 |
| `credential_not_found` | `accept` / `revoke` / `delete` references missing `credential_id` |
| `credential_invalid_expiry` | `expires_at_height <= block_height` |
| `credential_delegated_accept_invalid` | Invalid `subject_pubkey_hex` -> `subject` derivation or invalid ML-DSA-65 signature |
| `credential_delete_unauthorized` | Non-subject/non-issuer called `credential_delete` while `block_height <= expires_at_height` |
| `asset_gating_retroactive_enable_forbidden` | Attempted to enable gating / attach `credential_policy` on a permissionless asset |
| `asset_access_policy_bounds_exceeded` | `any_of.len() > 8` or `all_of.len() > 4` or `notice_blocks < 604_800` |
| `asset_access_emergency_remove_not_eligible` | `emergency_remove_issuer_id` targeted an active, uncompromised issuer |

### 4.7 RPC Request & Response JSON (Pillar 8 + `did:pftl`)

#### `did_resolve` (New Read Method)

Method syntax: `did:pftl:<chain_id>:<account>` (canonical lowercase `pf` + 40 hex).

Request (accepts `did` OR `account`):
```json
{
  "id": "did-1",
  "method": "did_resolve",
  "params": {
    "did": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111"
  }
}
```

Response (`schema = "postfiat-did-resolve-v1"`):
```json
{
  "id": "did-1",
  "ok": true,
  "result": {
    "schema": "postfiat-did-resolve-v1",
    "didDocument": {
      "@context": [
        "https://www.w3.org/ns/did/v1",
        "https://postfiat.org/ns/did-pftl/v1"
      ],
      "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111",
      "alsoKnownAs": [
        "acct:goodalexander@tasknode.postfiat.org"
      ],
      "verificationMethod": [
        {
          "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#account-mldsa65",
          "type": "MLDSA65VerificationKey2025",
          "controller": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111",
          "publicKeyHex": "<ml_dsa_65_pubkey_hex>"
        },
        {
          "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#message-xwing",
          "type": "XWingKeyAgreementKey2025",
          "controller": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111",
          "publicKeyHex": "<xwing_pubkey_hex>"
        },
        {
          "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#nostr-secp256k1",
          "type": "Bip340Secp256k1VerificationKey2025",
          "controller": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111",
          "publicKeyHex": "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d"
        }
      ],
      "authentication": [
        "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#account-mldsa65"
      ],
      "assertionMethod": [
        "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#account-mldsa65"
      ],
      "keyAgreement": [
        "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#message-xwing"
      ],
      "service": [
        {
          "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#nostr-relays",
          "type": "NostrRelayService",
          "serviceEndpoint": ["wss://relay.postfiat.org"]
        },
        {
          "id": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111#pq-receive",
          "type": "PostFiatShieldedReceive",
          "serviceEndpoint": "pfpq1..."
        }
      ]
    },
    "didDocumentMetadata": {
      "chain_id": "postfiat-wan-devnet-2",
      "block_height": 200500,
      "account": "pf1111111111111111111111111111111111111111",
      "account_exists": true,
      "key_revealed": true,
      "identity_minted": true,
      "deactivated": false
    }
  },
  "events": [
    { "kind": "did_resolve", "target": "did:pftl:postfiat-wan-devnet-2:pf1111111111111111111111111111111111111111", "message": "DID resolved" }
  ]
}
```
(`#account-mldsa65` is omitted until `Account.public_key_hex` is revealed on chain; `#message-xwing`, `#nostr-secp256k1`, `alsoKnownAs`, and Nostr/PQ services are included only when an active Pillar 6 identity exists. Zero PII is ever returned.)

#### Credential, Issuer & Access-Policy Read RPCs

| Method | Request `params` | Response `schema` & Top-Level Fields |
|---|---|---|
| `issuer_info` | `{ "issuer_id": "<96 hex>" }` | `postfiat-issuer-info-v1`: `{ schema, chain_id, genesis_hash, protocol_version, issuer_id, found, issuer: Option<CredentialIssuerV1> }` |
| `issuer_list` | `{ "domain": Option<String>, "cursor": Option<String>, "limit": Option<usize> }` | `postfiat-issuer-list-v1`: `{ schema, chain_id, genesis_hash, protocol_version, issuer_count, truncated, next_cursor, issuers: Vec<CredentialIssuerV1> }` |
| `credential_info` | `{ "credential_id": "<96 hex>" }` OR `{ "subject", "issuer_id", "credential_type" }` | `postfiat-credential-info-v1`: `{ schema, chain_id, genesis_hash, protocol_version, block_height, found, currently_valid: bool, invalid_reason: Option<String>, credential: Option<CredentialRecordV1> }` |
| `account_credentials` | `{ "account", "issuer_id": Option<String>, "include_expired": bool, "cursor": Option<String>, "limit": Option<usize> }` | `postfiat-account-credentials-v1`: `{ schema, chain_id, genesis_hash, protocol_version, block_height, account, credential_count, truncated, next_cursor, credentials: Vec<CredentialRecordV1> }` |
| `asset_access_info` | `{ "asset_id": "<96 hex>" }` | `postfiat-asset-access-info-v1`: `{ schema, chain_id, genesis_hash, protocol_version, block_height, asset_id, found, requires_authorization, freeze_enabled, clawback_enabled, access_policy: Option<AssetAccessPolicyV1> }` |

*(Note: Existing `asset_info` also includes additive `access_policy: Option<AssetAccessPolicyV1>` when `Some`, and existing `account_lines` includes additive `holder_limit: Option<u64>` and `implicit: bool`.)*

---

## 5. Cross-Lane Freeze Summary (Train B)

| Consumer Lane | Frozen Contracts Consumed from Train B |
|---|---|
| **Lane 1 (L1 Integrator)** | 4 governance kinds, `IdentitySetOperation` + 2-of-3 registrar ops + 7 uniqueness indexes, `NavFeeScheduleV1` + `NavFeeAccrualV1` + universal `floor(V * U / S)` check + 4 `nav_fee_*` ops, `ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS` + `TrustLine.holder_limit`, `CredentialIssuerV1` + `CredentialRecordV1` + `AssetAccessPolicyV1` + 10 issuer/credential/access ops + 12 new read RPCs |
| **Lane 4 (Task Node)** | `identity_set` (`mint`/`update`/`link`/`unlink`/`rotate`/`rotate_account`/`revoke`), salted `identity_handle_reserve` + `HandleAttestationV1`, `identity_*` RPCs, optional `nav_fee_schedule` / `nav_fee_history` UI (hidden when `has_fee_schedule == false`), removal of mandatory `trust_set` on default assets, `credential_accept`/`credential_delete`/`account_credentials` |
| **Lane 7 (Explorer)** | `identity_info`/`identity_by_nostr`/`identity_by_handle`/`identity_by_link`, `nav_fee_schedule`/`nav_fee_history` (neutral factual labels), `did_resolve`, `issuer_info`/`issuer_list`, `asset_access_info`, `credential_info` (zero PII) |
| **Lane 8 (NAVCoin / Adapters)** | `NavFeeScheduleV1`, `NavFeeAccrualV1`, exact 4-step `u256` formula + `PROTOCOL_BLOCKS_PER_YEAR = 31_536_000`, `nav_fee_schedule_*` + `nav_fee_claim` ops, `issuer_register` + `credential_create_and_accept` (0-PFT delegated flow with exact ML-DSA-65 preimage), `asset_access_set`, `did_resolve` |
