use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256, Sha3_256, Sha3_384};

pub const PROTOCOL_VERSION: u32 = 1;
pub const GENESIS_NATIVE_SUPPLY_ATOMS: u64 = 1_000_000_000;
pub const ADDRESS_NAMESPACE: &str = "postfiat.address.v1";
pub const ATOMIC_SWAP_TRANSACTION_SIGNING_DOMAIN: &str = "postfiat.atomic_swap_transaction.v1";
pub const ATOMIC_SWAP_TRANSACTION_TX_ID_DOMAIN: &str = "postfiat.atomic_swap_transaction.tx_id.v1";
pub const ATOMIC_SWAP_TRANSACTION_KIND: &str = "atomic_swap";
pub const TRANSFER_TRANSACTION_KIND: &str = "transparent_transfer";
pub const PAYMENT_V2_TRANSACTION_KIND: &str = "payment_v2";
pub const ASSET_CREATE_TRANSACTION_KIND: &str = "asset_create";
pub const TRUST_SET_TRANSACTION_KIND: &str = "trust_set";
pub const ISSUED_PAYMENT_TRANSACTION_KIND: &str = "issued_payment";
pub const ASSET_BURN_TRANSACTION_KIND: &str = "asset_burn";
pub const OWNED_TRANSFER_TRANSACTION_KIND: &str = "owned_transfer";
pub const ASSET_CLAWBACK_TRANSACTION_KIND: &str = "asset_clawback";
pub const NAV_ASSET_REGISTER_TRANSACTION_KIND: &str = "nav_asset_register";
pub const NAV_RESERVE_SUBMIT_TRANSACTION_KIND: &str = "nav_reserve_submit";
pub const NAV_RESERVE_CHALLENGE_TRANSACTION_KIND: &str = "nav_reserve_challenge";
pub const NAV_EPOCH_FINALIZE_TRANSACTION_KIND: &str = "nav_epoch_finalize";
pub const MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND: &str = "market_ops_policy_register";
pub const MARKET_OPS_FINALIZE_TRANSACTION_KIND: &str = "market_ops_finalize";
pub const NAV_MINT_AT_NAV_TRANSACTION_KIND: &str = "nav_mint_at_nav";
pub const NAV_REDEEM_AT_NAV_TRANSACTION_KIND: &str = "nav_redeem_at_nav";
pub const NAV_HALT_TRANSACTION_KIND: &str = "nav_halt";
pub const NAV_PROFILE_REGISTER_TRANSACTION_KIND: &str = "nav_profile_register";
pub const NAV_REDEEM_SETTLE_TRANSACTION_KIND: &str = "nav_redeem_settle";
pub const NAV_RESERVE_ATTEST_TRANSACTION_KIND: &str = "nav_reserve_attest";
pub const NAV_ATTESTOR_REGISTER_TRANSACTION_KIND: &str = "nav_attestor_register";
pub const VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND: &str = "vault_bridge_receipt_submit";
pub const VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND: &str = "vault_bridge_deposit_propose";
pub const VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND: &str = "vault_bridge_deposit_challenge";
pub const VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND: &str = "vault_bridge_deposit_attest";
pub const VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND: &str = "vault_bridge_deposit_finalize";
pub const VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND: &str = "vault_bridge_deposit_claim";
pub const VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND: &str = "vault_bridge_receipt_count";
pub const VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND: &str =
    "vault_bridge_mint_from_receipts";
pub const VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND: &str = "vault_bridge_burn_to_redeem";
pub const VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND: &str = "vault_bridge_redeem_settle";
pub const VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND: &str = "vault_bridge_bucket_impair";
pub const VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND: &str =
    "vault_bridge_nav_subscription_allocate";
pub const PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND: &str = "pftl_uniswap_route_init";
pub const PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND: &str = "pftl_uniswap_primary_subscribe";
pub const PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND: &str = "pftl_uniswap_export_debit";
pub const PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND: &str =
    "pftl_uniswap_destination_consume";
pub const PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND: &str = "pftl_uniswap_refund_source";
pub const PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND: &str = "pftl_uniswap_return_import";
pub const ESCROW_CREATE_TRANSACTION_KIND: &str = "escrow_create";
pub const ESCROW_FINISH_TRANSACTION_KIND: &str = "escrow_finish";
pub const ESCROW_CANCEL_TRANSACTION_KIND: &str = "escrow_cancel";
pub const NFT_MINT_TRANSACTION_KIND: &str = "nft_mint";
pub const NFT_TRANSFER_TRANSACTION_KIND: &str = "nft_transfer";
pub const NFT_BURN_TRANSACTION_KIND: &str = "nft_burn";
pub const OFFER_CREATE_TRANSACTION_KIND: &str = "offer_create";
pub const OFFER_CANCEL_TRANSACTION_KIND: &str = "offer_cancel";
pub const ESCROW_ID_DOMAIN: &str = "postfiat.escrow_id.v1";
pub const ESCROW_CONDITION_HASH_DOMAIN: &str = "postfiat.escrow_condition_hash.v1";
pub const ATOMIC_SETTLEMENT_TEMPLATE_ID_DOMAIN: &str = "postfiat.atomic_settlement_template_id.v1";
pub const OFFER_ID_DOMAIN: &str = "postfiat.offer_id.v1";
pub const NAV_RESERVE_PACKET_ID_DOMAIN: &str = "postfiat.nav_reserve_packet_id.v1";
pub const MARKET_OPS_ASSET_ID_DOMAIN: &str = "postfiat.market_ops_asset_id.v1";
pub const MARKET_OPS_RESERVE_PACKET_HASH_DOMAIN: &str =
    "postfiat.market_ops_reserve_packet_hash.v1";
pub const MARKET_OPS_SUPPLY_PACKET_HASH_DOMAIN: &str = "postfiat.market_ops_supply_packet_hash.v1";
pub const MARKET_OPS_EVIDENCE_ROOT_DOMAIN: &str = "postfiat.market_ops_evidence_root.v1";
pub const MARKET_OPS_EVM_EVIDENCE_ROOT_DOMAIN: &str = "postfiat.market_ops_evm_evidence_root.v1";
pub const NAV_REDEMPTION_ID_DOMAIN: &str = "postfiat.nav_redemption_id.v1";
pub const NAV_PROFILE_ID_DOMAIN: &str = "postfiat.nav_proof_profile_id.v1";
pub const VAULT_BRIDGE_RECEIPT_ID_DOMAIN: &str = "postfiat.vault_bridge_receipt_id.v1";
pub const VAULT_BRIDGE_BUCKET_ID_DOMAIN: &str = "postfiat.vault_bridge_bucket_id.v1";
pub const VAULT_BRIDGE_ALLOCATION_ID_DOMAIN: &str = "postfiat.vault_bridge_allocation_id.v1";
pub const VAULT_BRIDGE_REDEMPTION_ID_DOMAIN: &str = "postfiat.vault_bridge_redemption_id.v1";
pub const VAULT_BRIDGE_SOURCE_ROOT_DOMAIN: &str = "postfiat.vault_bridge_source_root.v1";
pub const VAULT_BRIDGE_DEPOSIT_EVIDENCE_ROOT_DOMAIN: &str =
    "postfiat.vault_bridge.bridge_deposit_evidence_root.v1";
pub const VAULT_BRIDGE_DEPOSIT_PUBLIC_VALUES_DOMAIN: &str =
    "postfiat.vault_bridge.bridge_deposit_public_values.v1";
pub const VAULT_BRIDGE_DEPOSIT_OBSERVATION_ROOT_DOMAIN: &str =
    "postfiat.vault_bridge.bridge_deposit_observation_root.v1";
pub const VAULT_BRIDGE_WITHDRAWAL_PACKET_HASH_DOMAIN: &str =
    "postfiat.vault_bridge.withdrawal_packet_hash.v1";
pub const VAULT_BRIDGE_WITHDRAWAL_OBSERVATION_ROOT_DOMAIN: &str =
    "postfiat.vault_bridge.withdrawal_execution_observation_root.v1";
pub const ESCROW_ID_HEX_LEN: usize = 96;
pub const ESCROW_CONDITION_HASH_HEX_LEN: usize = 96;
pub const ATOMIC_SETTLEMENT_TEMPLATE_ID_HEX_LEN: usize = 96;
pub const OFFER_ID_HEX_LEN: usize = 96;
pub const NAV_RESERVE_PACKET_ID_HEX_LEN: usize = 96;
pub const NAV_REDEMPTION_ID_HEX_LEN: usize = 96;
pub const NAV_PROFILE_ID_HEX_LEN: usize = 96;
/// Fixed-point scale for NAV and other USD-e8 valuation fields.
pub const NAV_USD_E8_UNIT: u64 = 100_000_000;
pub const VAULT_BRIDGE_RECEIPT_ID_HEX_LEN: usize = 96;
pub const VAULT_BRIDGE_BUCKET_ID_HEX_LEN: usize = 96;
pub const VAULT_BRIDGE_ALLOCATION_ID_HEX_LEN: usize = 96;
pub const VAULT_BRIDGE_REDEMPTION_ID_HEX_LEN: usize = 96;
pub const VAULT_BRIDGE_UNIT: u64 = 1_000_000;
pub const VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX: &str = "vault_bridge:";
pub const VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT: &str = "bridge_deposit";
pub const VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX: &str = "erc20_bridge_vault";
pub const VAULT_BRIDGE_DEPOSIT_SOURCE_TX_PREFIX: &str = "erc20_bridge_deposit";
pub const VAULT_BRIDGE_EVM_DESTINATION_REF_PREFIX: &str = "evm-erc20";
pub const VAULT_BRIDGE_HEX_HASH_LEN: usize = 96;
pub const VAULT_BRIDGE_EVM_ADDRESS_TEXT_LEN: usize = 42;
pub const VAULT_BRIDGE_EVM_BYTES32_HEX_LEN: usize = 64;
pub const VAULT_BRIDGE_RECEIPT_STATUS_PENDING: &str = "pending";
pub const VAULT_BRIDGE_RECEIPT_STATUS_FINALIZED: &str = "finalized";
pub const VAULT_BRIDGE_RECEIPT_STATUS_COUNTED: &str = "counted";
pub const VAULT_BRIDGE_RECEIPT_STATUS_PAUSED: &str = "paused";
pub const VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED: &str = "impaired";
pub const VAULT_BRIDGE_RECEIPT_STATUS_REJECTED: &str = "rejected";
pub const VAULT_BRIDGE_DEPOSIT_STATUS_PENDING: &str = "pending";
pub const VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED: &str = "challenged";
pub const VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED: &str = "finalized";
pub const VAULT_BRIDGE_RECEIPT_STATUS_RETIRED: &str = "retired";
pub const VAULT_BRIDGE_BUCKET_STATUS_ACTIVE: &str = "active";
pub const VAULT_BRIDGE_BUCKET_STATUS_PAUSED: &str = "paused";
pub const VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED: &str = "impaired";
pub const VAULT_BRIDGE_BUCKET_STATUS_RETIRED: &str = "retired";
pub const VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY: &str = "vault_bridge_supply";
pub const VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION: &str = "nav_subscription";
pub const VAULT_BRIDGE_ALLOCATION_PURPOSE_REDEMPTION: &str = "redemption";
pub const VAULT_BRIDGE_ALLOCATION_PURPOSE_OTHER: &str = "other";
pub const VAULT_BRIDGE_REDEMPTION_STATE_PENDING: &str = "pending";
pub const VAULT_BRIDGE_REDEMPTION_STATE_SETTLED: &str = "settled";
pub const ESCROW_STATE_OPEN: &str = "open";
pub const ESCROW_STATE_FINISHED: &str = "finished";
pub const ESCROW_STATE_CANCELED: &str = "canceled";
pub const OFFER_STATE_OPEN: &str = "open";
pub const OFFER_STATE_FILLED: &str = "filled";
pub const OFFER_STATE_CANCELED: &str = "canceled";
pub const OFFER_STATE_UNFUNDED: &str = "unfunded";
pub const NAV_RESERVE_STATE_SUBMITTED: &str = "submitted";
pub const NAV_RESERVE_STATE_FINALIZED: &str = "finalized";
pub const NAV_RESERVE_STATE_CHALLENGED: &str = "challenged";
pub const NAV_REDEMPTION_STATE_PENDING: &str = "pending";
pub const NAV_REDEMPTION_STATE_SETTLED: &str = "settled";
pub const NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT: &str = "ledger-transparent";
pub const NAV_PROFILE_VERIFIER_PLACEHOLDER: &str = "placeholder";
pub const NAV_PROFILE_VERIFIER_MULTI_FETCH: &str = "multi-fetch-quorum";
/// NAV proof profile verifier kind for SP1 Groth16 aggregate reserve proofs.
pub const NAV_PROFILE_VERIFIER_SP1_GROTH16: &str = "sp1-groth16";
pub const NAV_SP1_PROOF_ENCODING_GROTH16: &str = "groth16";
pub const DEFAULT_MAX_NAV_SP1_PROOF_BYTES: u64 = 4096;
pub const DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES: u64 = 16384;
pub const NAV_SP1_PROGRAM_VKEY_HEX_LEN: usize = 66;
pub const NAV_SP1_POLICY_HASH_HEX_LEN: usize = 64;
pub const AGGREGATE_PUBLIC_VALUES_V2_SCHEMA_VERSION: u32 = 2;
pub const NAV_PROFILE_SOURCE_CLASS_LEDGER: &str = "ledger";
pub const MAX_NAV_ATTESTATIONS_PER_PACKET: usize = 64;
pub const MAX_NAV_RESERVE_ACCOUNTS: usize = 32;
pub const MAX_VAULT_BRIDGE_MINT_RECEIPTS: usize = 64;
pub const MAX_MARKET_OPS_OBSERVATIONS: usize = 4096;
pub const MAX_MARKET_OPS_COST_SAMPLES: usize = 4096;
pub const MAX_MARKET_OPS_EVM_HEADERS: usize = 1024;
pub const MAX_MARKET_OPS_EVM_RECEIPTS: usize = 4096;
pub const MAX_MARKET_OPS_EVM_LOGS: usize = 8192;
pub const MAX_MARKET_OPS_EVM_LOG_TOPICS: usize = 4;
pub const MAX_MARKET_OPS_EVM_CHECKPOINTS: usize = 4096;
pub const MAX_MARKET_OPS_EVM_POOL_STATES: usize = 4096;
pub const MAX_PFTL_UNISWAP_ROUTES: usize = 64;
pub const MAX_PFTL_UNISWAP_ROUTES_PER_NATIVE_ISSUER: usize = 8;
pub const MAX_PFTL_UNISWAP_ROUTE_ENTRIES: usize = 8192;
pub const MAX_PFTL_UNISWAP_RECEIPTS: usize = 131_072;
pub const MAX_PFTL_UNISWAP_PRICING_AGE_BLOCKS: u64 = 100;
pub const MAX_ESCROW_CONDITION_BYTES: usize = 128;
pub const MAX_ESCROW_FULFILLMENT_BYTES: usize = 128;
pub const MAX_DEX_CROSSES_PER_TRANSACTION: usize = 64;
pub const OFFER_OBJECT_RESERVE: u64 = 10;
pub const OFFER_TX_ROLE_TAKER: &str = "offer_taker";
pub const OFFER_TX_ROLE_MAKER: &str = "offer_maker";
pub const OFFER_TX_ROLE_CANCEL: &str = "offer_cancel";
pub const MAX_TEXT_FIELD_BYTES: usize = 256;
pub const MAX_TRANSFER_PUBLIC_KEY_HEX_LEN: usize = 4096;
pub const MAX_TRANSFER_SIGNATURE_HEX_LEN: usize = 8192;
pub const MAX_PAYMENT_MEMOS: usize = 4;
pub const MAX_PAYMENT_MEMO_TYPE_BYTES: usize = 64;
pub const MAX_PAYMENT_MEMO_FORMAT_BYTES: usize = 64;
pub const MAX_PAYMENT_MEMO_DATA_BYTES: usize = 256;
pub const MAX_PAYMENT_MEMO_TOTAL_BYTES: usize = 512;
pub const MAX_OWNED_INPUTS_PER_TRANSFER: usize = 2048;
pub const MAX_OWNED_OUTPUTS_PER_TRANSFER: usize = 8;
pub const MAX_OWNED_OBJECTS: usize = 100_000;
pub const ISSUED_ASSET_ID_DOMAIN: &str = "postfiat.issued_asset_id.v1";
pub const TRUSTLINE_ID_DOMAIN: &str = "postfiat.trustline_id.v1";
pub const NFT_ID_DOMAIN: &str = "postfiat.nft_id.v1";
pub const ISSUED_ASSET_ID_HEX_LEN: usize = 96;
pub const TRUSTLINE_ID_HEX_LEN: usize = 96;
pub const NFT_ID_HEX_LEN: usize = 96;
pub const MAX_ISSUED_ASSET_CODE_BYTES: usize = 32;
pub const MAX_ISSUED_ASSET_DISPLAY_NAME_BYTES: usize = 64;
pub const MAX_ISSUED_ASSET_PRECISION: u8 = 18;
pub const MAX_NFT_COLLECTION_ID_BYTES: usize = 64;
pub const MAX_NFT_METADATA_HASH_BYTES: usize = 64;
pub const MAX_NFT_METADATA_URI_BYTES: usize = 256;
pub const NFT_FLAG_TRANSFERABLE: u32 = 0x0000_0001;
pub const NFT_FLAG_ISSUER_BURNABLE: u32 = 0x0000_0002;
pub const NFT_ALLOWED_FLAGS: u32 = NFT_FLAG_TRANSFERABLE | NFT_FLAG_ISSUER_BURNABLE;
pub const NFT_COLLECTION_FLAG_TRANSFER_LOCKED: u32 = 0x0000_0001;
pub const NFT_COLLECTION_FLAG_BURN_LOCKED: u32 = 0x0000_0002;
pub const NFT_COLLECTION_ALLOWED_FLAGS: u32 =
    NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genesis {
    pub chain_id: String,
    pub protocol_version: u32,
    pub validator_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_supply_atoms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicated_state_v2_activation_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_verification_activation_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atomic_swap_activation_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_v2_activation_height: Option<u64>,
}

impl Genesis {
    pub fn new(chain_id: impl Into<String>) -> Self {
        Self {
            chain_id: chain_id.into(),
            protocol_version: PROTOCOL_VERSION,
            validator_count: 1,
            native_supply_atoms: Some(GENESIS_NATIVE_SUPPLY_ATOMS),
            replicated_state_v2_activation_height: Some(0),
            bridge_verification_activation_height: None,
            atomic_swap_activation_height: None,
            consensus_v2_activation_height: None,
        }
    }

    pub fn new_with_validator_count(chain_id: impl Into<String>, validator_count: u32) -> Self {
        Self {
            chain_id: chain_id.into(),
            protocol_version: PROTOCOL_VERSION,
            validator_count,
            native_supply_atoms: Some(GENESIS_NATIVE_SUPPLY_ATOMS),
            replicated_state_v2_activation_height: Some(0),
            bridge_verification_activation_height: None,
            atomic_swap_activation_height: None,
            consensus_v2_activation_height: None,
        }
    }

    pub fn try_new(chain_id: impl Into<String>) -> Result<Self, String> {
        let genesis = Self::new(chain_id);
        genesis.validate()?;
        Ok(genesis)
    }

    pub fn try_new_with_validator_count(
        chain_id: impl Into<String>,
        validator_count: u32,
    ) -> Result<Self, String> {
        let genesis = Self::new_with_validator_count(chain_id, validator_count);
        genesis.validate()?;
        Ok(genesis)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_chain_id(&self.chain_id)?;
        if self.protocol_version == 0 {
            return Err("genesis protocol_version must be nonzero".to_string());
        }
        if self.validator_count == 0 {
            return Err("genesis validator_count must be nonzero".to_string());
        }
        if let Some(native_supply_atoms) = self.native_supply_atoms {
            if native_supply_atoms != GENESIS_NATIVE_SUPPLY_ATOMS {
                return Err(format!(
                    "genesis native_supply_atoms must equal {GENESIS_NATIVE_SUPPLY_ATOMS}"
                ));
            }
        }
        if self.consensus_v2_activation_height == Some(0) {
            return Err("genesis consensus_v2_activation_height must be positive".to_string());
        }
        Ok(())
    }

    pub fn expected_native_supply_atoms(&self) -> u64 {
        self.native_supply_atoms
            .unwrap_or(GENESIS_NATIVE_SUPPLY_ATOMS)
    }

    pub fn to_json(&self) -> Result<String, ParseError> {
        serde_json::to_string_pretty(self)
            .map(|json| format!("{json}\n"))
            .map_err(|error| ParseError::new(format!("genesis serialization failed: {error}")))
    }

    pub fn from_json(input: &str) -> Result<Self, ParseError> {
        let genesis: Self = serde_json::from_str(input)
            .map_err(|error| ParseError::new(format!("invalid genesis JSON: {error}")))?;
        genesis.validate().map_err(ParseError::new)?;
        Ok(genesis)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeState {
    pub node_id: String,
    pub status: String,
    pub last_run_unix: u64,
}

impl NodeState {
    pub fn initialized(node_id: impl Into<String>) -> Self {
        Self {
            node_id: node_id.into(),
            status: "initialized".to_string(),
            last_run_unix: 0,
        }
    }

    pub fn mark_running(&mut self) {
        self.status = "running".to_string();
        self.last_run_unix = unix_now();
    }

    pub fn to_json(&self) -> Result<String, ParseError> {
        serde_json::to_string_pretty(self)
            .map(|json| format!("{json}\n"))
            .map_err(|error| ParseError::new(format!("node state serialization failed: {error}")))
    }

    pub fn from_json(input: &str) -> Result<Self, ParseError> {
        serde_json::from_str(input)
            .map_err(|error| ParseError::new(format!("invalid node state JSON: {error}")))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentServiceArtifact {
    pub service_id: String,
    pub service_unit_sha256: String,
    pub environment_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRuntimeArtifactHashes {
    pub binary_sha256: String,
    pub topology_sha256: String,
    pub swap_circuit_metadata_sha256: String,
    pub private_egress_circuit_metadata_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveNavProfileStatus {
    pub asset_id: String,
    pub profile_id: String,
    pub verifier_kind: String,
    pub source_class: String,
    pub max_snapshot_age_blocks: u64,
    pub challenge_window_blocks: u64,
    pub max_epoch_gap_blocks: u64,
    pub settle_deadline_blocks: u64,
    pub min_attestations: u64,
    pub tolerance_bp: u64,
    pub bridge_observer_min_confirmations: u64,
    pub valuation_policy_hash: String,
    pub finalized_epoch: u64,
    pub nav_per_unit: u64,
    pub finalized_reserve_packet_hash: String,
    pub halted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    #[serde(default)]
    pub rpc_schema: String,
    pub build_git_revision: String,
    pub build_profile: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub active_nav_profiles: Vec<ActiveNavProfileStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_manifest_sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_validator_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deployment_service_artifacts: Vec<DeploymentServiceArtifact>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_runtime_artifacts: Option<DeploymentRuntimeArtifactHashes>,
    pub validator_count: u32,
    pub node_id: String,
    pub status: String,
    pub last_run_unix: u64,
    pub state_root: String,
    pub block_height: u64,
    pub block_tip_hash: String,
    pub mempool_pending: u64,
}

impl StatusReport {
    pub fn to_json(&self) -> Result<String, ParseError> {
        serde_json::to_string_pretty(self)
            .map(|json| format!("{json}\n"))
            .map_err(|error| {
                ParseError::new(format!("status report serialization failed: {error}"))
            })
    }
}

pub const SNAPSHOT_VERSION: u32 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotFile {
    pub name: String,
    pub bytes: u64,
    pub hash_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotManifest {
    pub snapshot_version: u32,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub node_id: String,
    pub state_root: String,
    pub block_height: u64,
    pub block_tip_hash: String,
    pub exported_unix: u64,
    pub files: Vec<SnapshotFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockCertificateVote {
    pub vote_id: String,
    pub validator: String,
    pub accept: bool,
    pub algorithm_id: String,
    #[serde(default)]
    pub registry_root: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockCertificate {
    pub validators: Vec<String>,
    pub quorum: usize,
    #[serde(default)]
    pub registry_root: String,
    pub votes: Vec<BlockCertificateVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub height: u64,
    #[serde(default)]
    pub view: u64,
    pub parent_hash: String,
    #[serde(default)]
    pub proposer: String,
    pub batch_kind: String,
    pub batch_id: String,
    pub state_root: String,
    pub receipt_count: u64,
    pub certificate_id: String,
    pub certificate: BlockCertificate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_v2_commit: Option<ConsensusV2Commit>,
    pub block_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockRecord {
    pub header: BlockHeader,
    pub receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_pre_state_effects: Vec<FastPayVersionFenceV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockLog {
    pub blocks: Vec<BlockRecord>,
}

impl BlockLog {
    pub fn empty() -> Self {
        Self { blocks: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn tip_hash(&self) -> String {
        self.blocks
            .last()
            .map(|block| block.header.block_hash.clone())
            .unwrap_or_else(|| "genesis".to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainTipState {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub ordered_batch_count: u64,
    pub receipt_count: u64,
    pub history_base_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchArchiveEntry {
    pub batch_kind: String,
    pub batch_id: String,
    pub payload_hash: String,
    pub payload_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchArchive {
    pub batches: Vec<BatchArchiveEntry>,
}

impl BatchArchive {
    pub fn empty() -> Self {
        Self {
            batches: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.batches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.batches.is_empty()
    }

    pub fn find(&self, batch_kind: &str, batch_id: &str) -> Option<&BatchArchiveEntry> {
        self.batches
            .iter()
            .find(|entry| entry.batch_kind == batch_kind && entry.batch_id == batch_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}
