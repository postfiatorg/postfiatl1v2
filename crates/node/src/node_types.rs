#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOptions {
    pub data_dir: PathBuf,
    pub chain_id: String,
    pub node_id: String,
    pub validator_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitConsensusV2Options {
    pub data_dir: PathBuf,
    pub chain_id: String,
    pub node_id: String,
    pub validator_count: u32,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyOptions {
    pub chain_id: String,
    pub validators: u32,
    pub base_port: u16,
    pub rpc_base_port: Option<u16>,
    pub hosts: Option<Vec<String>>,
    pub output_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyConsensusV2Options {
    pub chain_id: String,
    pub validators: u32,
    pub base_port: u16,
    pub rpc_base_port: Option<u16>,
    pub hosts: Option<Vec<String>>,
    pub output_file: PathBuf,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeOptions {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceVerifyOptions {
    pub data_dir: PathBuf,
    pub cobalt_mode: String,
    pub trust_graph_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryOptions {
    pub data_dir: PathBuf,
    pub mode: String,
    pub retain_recent_blocks: u64,
    pub retain_recent_receipts: u64,
    pub retain_recent_batches: u64,
    pub retain_recent_governance: u64,
    pub minimum_replay_window_blocks: u64,
    pub advisory_prune: bool,
    pub archive_handoff_required: bool,
    pub prune_up_to_height: Option<u64>,
    pub archive_handoff_file: Option<PathBuf>,
}

impl HistoryOptions {
    pub fn with_defaults(data_dir: PathBuf) -> Self {
        Self {
            data_dir,
            mode: DEFAULT_HISTORY_MODE.to_string(),
            retain_recent_blocks: DEFAULT_HISTORY_RETAIN_RECENT_BLOCKS,
            retain_recent_receipts: DEFAULT_HISTORY_RETAIN_RECENT_RECEIPTS,
            retain_recent_batches: DEFAULT_HISTORY_RETAIN_RECENT_BATCHES,
            retain_recent_governance: DEFAULT_HISTORY_RETAIN_RECENT_GOVERNANCE,
            minimum_replay_window_blocks: DEFAULT_HISTORY_MINIMUM_REPLAY_WINDOW_BLOCKS,
            advisory_prune: true,
            archive_handoff_required: true,
            prune_up_to_height: None,
            archive_handoff_file: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveHandoffCreateOptions {
    pub data_dir: PathBuf,
    pub from_height: u64,
    pub to_height: u64,
    pub archive_uri: Option<String>,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveHandoffVerifyOptions {
    pub data_dir: PathBuf,
    pub proof_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveWindowExportOptions {
    pub data_dir: PathBuf,
    pub from_height: u64,
    pub to_height: u64,
    pub archive_uri: Option<String>,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveWindowVerifyOptions {
    pub bundle_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveWindowBuildOptions {
    pub data_dir: PathBuf,
    pub from_height: u64,
    pub to_height: u64,
    pub archive_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryArchiveWindowImportOptions {
    pub data_dir: PathBuf,
    pub bundle_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryCheckpointRebuildFromArchiveOptions {
    pub data_dir: PathBuf,
    pub backup_file: PathBuf,
}

pub const MARKET_OPS_REPLAY_BUNDLE_SCHEMA: &str = "postfiat-market-ops-replay-bundle-v1";
pub const MARKET_OPS_REPLAY_REPORT_SCHEMA: &str = "postfiat-market-ops-replay-report-v1";
pub const MARKET_OPS_OPERATION_BUNDLE_SCHEMA: &str = "postfiat-market-ops-operation-bundle-v1";
pub const MARKET_OPS_PUBLIC_STATUS_SCHEMA: &str = "postfiat-market-ops-public-status-v1";
pub const MARKET_OPS_PUBLIC_DISCLOSURE: &str = "The protocol proves NAV and may execute bounded market operations under public caps. Holders do not have a standing right to redeem at NAV, and market operations can pause.";
pub const MARKET_OPS_STATUS_ACTIVE: &str = "active";
pub const MARKET_OPS_STATUS_EXPIRED: &str = "expired";
pub const MARKET_OPS_STATUS_MISSING_SOURCE_PACKET: &str = "missing_source_packet";
pub const MARKET_OPS_STATUS_PAUSED: &str = "paused";
pub const MARKET_OPS_STATUS_STALE: &str = "stale";
pub const MARKET_OPS_STATUS_UNDERFUNDED: &str = "underfunded";
pub const VAULT_BRIDGE_STATUS_REPORT_SCHEMA: &str = "postfiat-vault-bridge-status-v1";
pub const VAULT_BRIDGE_RECEIPTS_REPORT_SCHEMA: &str = "postfiat-vault-bridge-receipts-v1";
pub const VAULT_BRIDGE_ASSET_ID_REPORT_SCHEMA: &str = "postfiat-vault-bridge-asset-id-v1";
pub const VAULT_BRIDGE_BOOTSTRAP_BUNDLE_SCHEMA: &str = "postfiat-vault-bridge-bootstrap-bundle-v1";
pub const VAULT_BRIDGE_DEPOSIT_INTENT_SCHEMA: &str = "postfiat-vault-bridge-deposit-intent-v1";
pub const VAULT_BRIDGE_DEPOSIT_PLAN_SCHEMA: &str = "postfiat-vault-bridge-deposit-plan-v1";
pub const VAULT_BRIDGE_DEPOSIT_RELAY_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-deposit-relay-bundle-v1";
pub const VAULT_BRIDGE_DEPOSIT_RELAY_RPC_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-deposit-relay-rpc-bundle-v1";
pub const VAULT_BRIDGE_BURN_TO_REDEEM_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-burn-to-redeem-bundle-v1";
pub const VAULT_BRIDGE_WITHDRAWAL_PLAN_SCHEMA: &str = "postfiat-vault-bridge-withdrawal-plan-v1";
pub const VAULT_BRIDGE_WITHDRAWAL_SIGNATURE_REQUEST_SCHEMA: &str =
    "postfiat-vault-bridge-withdrawal-signature-request-v1";
pub const VAULT_BRIDGE_WITHDRAWAL_SIGNATURE_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-withdrawal-signature-bundle-v1";
pub const VAULT_BRIDGE_WITHDRAWAL_RELAY_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-withdrawal-relay-bundle-v1";
pub const VAULT_BRIDGE_RESERVE_REPLAY_BUNDLE_SCHEMA: &str =
    "postfiat-vault-bridge-reserve-replay-bundle-v2";
pub const VAULT_BRIDGE_RESERVE_REPLAY_REPORT_SCHEMA: &str =
    "postfiat-vault-bridge-reserve-replay-report-v2";
pub const VAULT_BRIDGE_STATUS_DISCLOSURE: &str = "vault bridge asset is source-bound and bridge-backed on PFTL. Counted vault bridge asset receipts must reference a ERC20BridgeVault deposit event evidence root; withdrawals are PFTL burn packets claimed from the source vault after challenge/finality. No pooled or automatic par redemption is implied.";
pub const PFTL_UNISWAP_BRIDGE_LEDGER_FILE: &str = "pftl_uniswap_bridge_ledgers.json";
pub const PFTL_UNISWAP_BRIDGE_RECEIPTS_FILE: &str = "pftl_uniswap_bridge_receipts.json";
pub const PFTL_UNISWAP_LAUNCH_CONFIG_FILE: &str = "pftl_uniswap_launch_configs.json";
pub const PFTL_UNISWAP_FORK_REHEARSAL_FILE: &str = "pftl_uniswap_fork_rehearsals.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOpsStatusOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub epoch: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeStatusOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeRoutesOptions {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgePacketOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub packet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeClaimsOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub limit: Option<usize>,
    pub include_terminal: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeSupplyStatusOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeReceiptReplayOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeRouteInitOptions {
    pub data_dir: PathBuf,
    pub config_file: PathBuf,
    pub ethereum_chain_id: u64,
    pub latest_finalized_nav_epoch: u64,
    pub return_finality_blocks: u64,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeLaunchConfigTemplateOptions {
    pub route_config_file: PathBuf,
    pub official_uniswap_file: PathBuf,
    pub usdc_token: String,
    pub receipt_verifier: String,
    pub uniswap_pool_key_hash: String,
    pub pricing_reserve_packet_hash: String,
    pub nav_price_settlement_atoms_per_nav_atom: u64,
    pub tick_lower: i32,
    pub tick_upper: i32,
    pub fee_pips: u32,
    pub position_recipient: String,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeLaunchConfigInitOptions {
    pub data_dir: PathBuf,
    pub launch_config_file: PathBuf,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeRecordForkRehearsalOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub evidence_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgePacketPreflightOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub packet_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgePrimarySubscribeOptions {
    pub data_dir: PathBuf,
    pub request_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeExportDebitOptions {
    pub data_dir: PathBuf,
    pub request_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeDestinationConsumeOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub packet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeRefundSourceOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub request_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeRecordReturnBurnOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub request_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeReturnBurnRequestOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavcoinBridgeImportReturnOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub burn_event_hash: String,
    pub pftl_recipient: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeRouteInitReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub ledger_hash: String,
    pub ledger_file: String,
    pub route_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeLaunchConfigTemplateReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub launch_config_digest: String,
    pub output_file: String,
    pub launch_config: PftlUniswapLaunchConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeLaunchConfigInitReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub launch_config_digest: String,
    pub launch_config_file: String,
    pub launch_config_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeForkRehearsalRecordReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub launch_config_digest: String,
    pub rehearsal_id: String,
    pub rehearsal_evidence_digest: String,
    pub evidence_file: String,
    pub evidence_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgePacketPreflightReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub launch_config_digest: String,
    pub packet_digest: String,
    pub ledger_hash: String,
    pub packet_file: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeTransitionApplyReport {
    pub schema: String,
    pub route_id: String,
    pub transition: String,
    pub ledger_hash: String,
    pub receipt_hash: String,
    pub ledger_file: String,
    pub receipt_file: String,
    pub receipt: PftlUniswapTransitionReceipt,
    pub result: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeReturnBurnRequestReport {
    pub schema: String,
    pub route_id: String,
    pub burn_event_hash: String,
    pub output_file: String,
    pub request: PftlUniswapReturnBurnRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeReceiptReplayReport {
    pub schema: String,
    pub route_id: String,
    pub route_config_digest: String,
    pub initial_ledger_hash: String,
    pub final_ledger_hash: String,
    pub receipt_root: Option<String>,
    pub receipt_count: u64,
    pub ledger_file: String,
    pub receipt_file: String,
    pub status: String,
    pub replay: Option<PftlUniswapReceiptReplayReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeReceiptsOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub bucket_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeAssetIdOptions {
    pub pftl_chain_id: String,
    pub issuer: String,
    pub asset_code: String,
    pub asset_version: u32,
    pub env_file: Option<PathBuf>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeBootstrapBundleOptions {
    pub pftl_chain_id: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub issuer: String,
    pub reserve_operator: String,
    pub redemption_account: String,
    pub asset_code: String,
    pub asset_version: u32,
    pub asset_precision: u8,
    pub asset_display_name: String,
    pub max_supply: Option<u64>,
    pub valuation_unit: String,
    pub verifier_kind: String,
    pub max_snapshot_age_blocks: u64,
    pub challenge_window_blocks: u64,
    pub max_epoch_gap_blocks: u64,
    pub settle_deadline_blocks: u64,
    pub min_challenge_bond: u64,
    pub min_attestations: u64,
    pub tolerance_bp: u64,
    pub bridge_observer_min_confirmations: u64,
    pub valuation_policy_hash: String,
    pub vault_bridge_route_policy_hash: String,
    pub sp1_program_vkey: String,
    pub sp1_proof_encoding: String,
    pub max_proof_bytes: u64,
    pub max_public_values_bytes: u64,
    pub trust_accounts: Vec<String>,
    pub trust_limit: u64,
    pub trust_reserve_paid: u64,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeDepositIntentOptions {
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub amount_atoms: u64,
    pub pftl_recipient: String,
    pub nonce: String,
    pub asset_id: String,
    pub policy_hash: String,
    pub route_epoch: u32,
    pub proposer: Option<String>,
    pub expires_at_height: Option<u64>,
    pub bundle_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeDepositPlanOptions {
    pub log_file: Option<PathBuf>,
    pub receipt_file: Option<PathBuf>,
    pub vault_address: Option<String>,
    pub token_address: Option<String>,
    pub asset_id: String,
    pub policy_hash: String,
    pub proposer: String,
    pub finalizer: String,
    pub claimer: String,
    pub attestor: Option<String>,
    pub observer_confirmation_depth: Option<u64>,
    pub expires_at_height: u64,
    pub source_proof_kind: Option<String>,
    pub source_proof_hash: Option<String>,
    pub source_public_values_hash: Option<String>,
    pub source_proof_file: Option<PathBuf>,
    pub source_public_values_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeDepositRelayBundleOptions {
    pub plan_options: VaultBridgeDepositPlanOptions,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeDepositRelayRpcBundleOptions {
    pub source_rpc_url: String,
    pub tx_hash: String,
    pub cast_binary: String,
    pub plan_options: VaultBridgeDepositPlanOptions,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeBurnToRedeemBundleOptions {
    pub data_dir: PathBuf,
    pub owner: String,
    pub issuer: Option<String>,
    pub asset_id: String,
    pub bucket_id: Option<String>,
    pub amount_atoms: u64,
    pub epoch: Option<u64>,
    pub reserve_packet_hash: Option<String>,
    pub destination_ref: String,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeWithdrawalPlanOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub redemption_id: String,
    pub pftl_finalized_height: Option<u64>,
    pub evm_chain_id: Option<u64>,
    pub verifier_address: Option<String>,
    pub signatures_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeWithdrawalRelayBundleOptions {
    pub plan_options: VaultBridgeWithdrawalPlanOptions,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeWithdrawalSignatureBundleOptions {
    pub plan_options: VaultBridgeWithdrawalPlanOptions,
    pub bundle_dir: PathBuf,
    pub relay_bundle_dir: Option<PathBuf>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeReserveReplayBundleExportOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub epoch: u64,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeReserveReplayBundleVerifyOptions {
    pub bundle_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReceiptStatusRow {
    pub receipt_id: String,
    pub bucket_id: String,
    pub source_domain: String,
    pub source_asset: String,
    pub claim_type: String,
    pub amount_atoms: u64,
    pub haircut_bps: u64,
    pub counted_value_atoms: u64,
    pub allocated_value_atoms: u64,
    pub unallocated_value_atoms: u64,
    pub status: String,
    pub created_at_height: u64,
    pub counted_at_height: u64,
    pub expires_at_height: u64,
    pub source_tx_or_attestation: String,
    pub finality_ref: String,
    pub vault_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_deposit_evidence_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositAttestationStatusRow {
    pub attestor: String,
    pub pass: bool,
    pub observation_root: String,
    pub attested_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositStatusRow {
    pub evidence_root: String,
    pub policy_hash: String,
    pub source_proof_kind: String,
    pub source_proof_hash: String,
    pub source_public_values_hash: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub deposit_id: String,
    pub block_hash: String,
    pub tx_hash: String,
    pub log_index: u64,
    pub proposer: String,
    pub status: String,
    pub submitted_at_height: u64,
    pub finalized_at_height: u64,
    pub expires_at_height: u64,
    pub challenger: String,
    pub challenge_hash: String,
    pub challenge_bond: u64,
    pub pass_attestation_count: u64,
    pub fail_attestation_count: u64,
    pub attestations: Vec<VaultBridgeDepositAttestationStatusRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBucketStatusRow {
    pub bucket_id: String,
    pub source_domain: String,
    pub policy_hash: String,
    pub gross_receipt_atoms: u64,
    pub counted_value_atoms: u64,
    pub outstanding_vault_bridge_atoms: u64,
    pub nav_subscription_allocations_atoms: u64,
    pub redemption_queue_atoms: u64,
    pub other_allocations_atoms: u64,
    pub unallocated_counted_capacity_atoms: u64,
    pub impairment_factor_bps: u64,
    pub status: String,
    pub last_packet_epoch: u64,
    pub last_updated_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeAllocationStatusRow {
    pub allocation_id: String,
    pub receipt_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub released_atoms: u64,
    pub remaining_atoms: u64,
    pub purpose: String,
    pub consumer_id: String,
    pub created_at_height: u64,
    pub retired_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRedemptionStatusRow {
    pub redemption_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub issuer: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub destination_ref: String,
    pub settled_atoms: u64,
    pub state: String,
    pub created_at_height: u64,
    pub settlement_receipt_hash: String,
    pub burn_tx_id: String,
    pub withdrawal_recipient: String,
    pub withdrawal_evidence_root: String,
    pub withdrawal_packet_hash: String,
    pub withdrawal_packet_evm_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositIntentReport {
    pub schema: String,
    pub event_signature_topic: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub depositor: String,
    pub amount_atoms: u64,
    pub pftl_recipient: String,
    pub pftl_recipient_hash: String,
    pub nonce: String,
    pub route_binding: String,
    pub route_epoch: u32,
    pub expected_deposit_id: String,
    pub source_domain: String,
    pub source_asset: String,
    pub source_tx_or_attestation: String,
    pub approve_signature: String,
    pub approve_cast_args: Vec<String>,
    pub approve_cast_command: String,
    pub deposit_signature: String,
    pub deposit_cast_args: Vec<String>,
    pub deposit_cast_command: String,
    pub receipt_cast_command: String,
    pub relay_bundle_command: String,
    pub relay_rpc_bundle_command: String,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeAssetIdReport {
    pub schema: String,
    pub pftl_chain_id: String,
    pub issuer: String,
    pub asset_code: String,
    pub asset_version: u32,
    pub asset_id: String,
    pub evm_asset_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_file: Option<String>,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBootstrapBundleReport {
    pub schema: String,
    pub bundle_dir: String,
    pub pftl_chain_id: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub source_domain: String,
    pub source_class: String,
    pub asset_id: String,
    pub profile_id: String,
    pub issuer: String,
    pub reserve_operator: String,
    pub redemption_account: String,
    pub asset_code: String,
    pub asset_version: u32,
    pub asset_precision: u8,
    pub valuation_unit: String,
    pub profile_register_operation_file: String,
    pub asset_create_operation_file: String,
    pub nav_asset_register_operation_file: String,
    pub trust_set_operation_files: Vec<String>,
    pub commands_file: String,
    pub commands: Vec<String>,
    pub profile_register_operation: AssetTransactionOperation,
    pub asset_create_operation: AssetTransactionOperation,
    pub nav_asset_register_operation: AssetTransactionOperation,
    pub trust_set_operations: Vec<AssetTransactionOperation>,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositPlanReport {
    pub schema: String,
    pub event_signature_topic: String,
    pub asset_id: String,
    pub policy_hash: String,
    pub evidence_root: String,
    pub source_public_values_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_observation_root: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit_confirmation_depth: Option<u64>,
    pub evidence: VaultBridgeDepositEvidence,
    pub source_domain: String,
    pub source_tx_or_attestation: String,
    pub finality_ref: String,
    pub vault_id: String,
    pub propose_operation: AssetTransactionOperation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attest_operation: Option<AssetTransactionOperation>,
    pub finalize_operation: AssetTransactionOperation,
    pub claim_operation: AssetTransactionOperation,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositRelayBundleReport {
    pub schema: String,
    pub bundle_dir: String,
    pub plan_file: String,
    pub propose_operation_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attest_operation_file: Option<String>,
    pub finalize_operation_file: String,
    pub claim_operation_file: String,
    pub commands_file: String,
    pub commands: Vec<String>,
    pub plan: VaultBridgeDepositPlanReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeDepositRelayRpcBundleReport {
    pub schema: String,
    pub source_rpc_url: String,
    pub tx_hash: String,
    pub receipt_file: String,
    pub receipt_block_hash: String,
    pub receipt_transaction_hash: String,
    pub receipt_block_number: u64,
    pub current_block_number: u64,
    pub confirmation_depth: u64,
    pub relay_bundle: VaultBridgeDepositRelayBundleReport,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeBurnToRedeemBundleReport {
    pub schema: String,
    pub bundle_dir: String,
    pub operation_file: String,
    pub commands_file: String,
    pub owner: String,
    pub issuer: String,
    pub asset_id: String,
    pub bucket_id: String,
    pub amount_atoms: u64,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub destination_ref: String,
    pub operation: AssetTransactionOperation,
    pub commands: Vec<String>,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalPacketEvmArgs {
    pub pftl_chain_id: u64,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub token_address: String,
    pub vault_bridge_asset_id: String,
    pub burn_tx_id: String,
    pub withdrawal_id: String,
    pub recipient: String,
    pub amount: u64,
    pub source_bucket_id: String,
    pub destination_hash: String,
    pub finalized_height: u64,
    pub evidence_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalPlanReport {
    pub schema: String,
    pub asset_id: String,
    pub redemption_id: String,
    pub redemption_state: String,
    pub pftl_finalized_height: u64,
    pub withdrawal_packet: VaultBridgeWithdrawalPacket,
    pub withdrawal_packet_evm_args: VaultBridgeWithdrawalPacketEvmArgs,
    pub withdrawal_packet_tuple_arg: String,
    pub withdrawal_packet_hash: String,
    pub pftl_withdrawal_hash: String,
    pub pftl_withdrawal_hash_commitment: String,
    pub withdrawal_packet_evm_digest: String,
    pub verifier_pending_proof_id: String,
    pub verifier_withdrawal_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verifier_proof_digest_to_sign: Option<String>,
    pub verifier_submit_proof_signature: String,
    pub verifier_submit_proof_cast_args: Vec<String>,
    pub verifier_submit_proof_cast_command: String,
    pub vault_pending_withdrawal_id: String,
    pub vault_submit_withdrawal_signature: String,
    pub vault_submit_withdrawal_cast_args: Vec<String>,
    pub vault_submit_withdrawal_cast_command: String,
    pub signatures: Vec<String>,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalSignatureRequest {
    pub schema: String,
    pub asset_id: String,
    pub redemption_id: String,
    pub pftl_finalized_height: u64,
    pub evm_chain_id: u64,
    pub verifier_address: String,
    pub withdrawal_packet_evm_digest: String,
    pub pftl_withdrawal_hash_commitment: String,
    pub verifier_proof_digest_to_sign: String,
    pub verifier_pending_proof_id: String,
    pub verifier_withdrawal_key: String,
    pub cast_wallet_sign_command: String,
    pub signatures_file_note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalSignatureBundleReport {
    pub schema: String,
    pub bundle_dir: String,
    pub plan_file: String,
    pub signature_request_file: String,
    pub signatures_file: String,
    pub commands_file: String,
    pub relay_bundle_dir: String,
    pub sign_command: String,
    pub relay_bundle_command: String,
    pub plan: VaultBridgeWithdrawalPlanReport,
    pub signature_request: VaultBridgeWithdrawalSignatureRequest,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeWithdrawalRelayBundleReport {
    pub schema: String,
    pub bundle_dir: String,
    pub plan_file: String,
    pub commands_file: String,
    pub verifier_submit_proof_command: String,
    pub verifier_finalize_proof_command: String,
    pub vault_submit_withdrawal_command: String,
    pub vault_finalize_withdrawal_command: String,
    pub vault_claim_withdrawal_command: String,
    pub stages: Vec<String>,
    pub plan: VaultBridgeWithdrawalPlanReport,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeStatusReport {
    pub schema: String,
    pub asset_id: String,
    pub issuer: String,
    pub proof_profile: String,
    pub valuation_unit: String,
    pub finalized_epoch: u64,
    pub nav_per_unit: u64,
    pub circulating_supply: u64,
    pub finalized_reserve_packet_hash: String,
    pub issued_supply_atoms: u64,
    pub counted_value_atoms: u64,
    pub unallocated_counted_capacity_atoms: u64,
    pub source_root: String,
    pub bucket_count: u64,
    pub receipt_count: u64,
    pub bridge_deposit_count: u64,
    pub allocation_count: u64,
    pub redemption_count: u64,
    pub buckets: Vec<VaultBridgeBucketStatusRow>,
    pub receipts: Vec<VaultBridgeReceiptStatusRow>,
    pub bridge_deposits: Vec<VaultBridgeDepositStatusRow>,
    pub allocations: Vec<VaultBridgeAllocationStatusRow>,
    pub redemptions: Vec<VaultBridgeRedemptionStatusRow>,
    pub disclosure: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeRouteOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeRouteReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub current_height: u64,
    pub profile: postfiat_types::VaultBridgeRouteProfileV1,
    pub profile_hash: String,
    pub route_binding: String,
    pub governance_amendment_id: String,
    pub governance_activation_height: u64,
    pub governance_route_epoch: u32,
    pub nav_profile_id: String,
    pub nav_profile_source_class: String,
    pub nav_profile_verifier_kind: String,
    pub nav_profile_policy_hash: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReceiptsReport {
    pub schema: String,
    pub asset_id: String,
    pub bucket_id: Option<String>,
    pub receipt_count: u64,
    pub receipts: Vec<VaultBridgeReceiptStatusRow>,
    pub disclosure: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOpsReplayBundleExportOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub epoch: u64,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOpsReplayBundleVerifyOptions {
    pub bundle_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketOpsOperationBundleOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
    pub issuer: Option<String>,
    pub epoch: Option<u64>,
    pub policy_file: PathBuf,
    pub policy_inputs_file: PathBuf,
    pub bundle_dir: PathBuf,
    pub overwrite: bool,
    pub encoding_version: u32,
    pub evm_chain_id: u64,
    pub adapter_address: String,
    pub vault_address: String,
    pub mint_controller_address: String,
    pub funded_alignment_reserve_usd_e8: u128,
    pub discount_trigger_bps: u32,
    pub premium_trigger_bps: u32,
    pub data_window_start: u64,
    pub data_window_end: u64,
    pub valid_after: u64,
    pub expires_at: u64,
    pub cooldown_seconds: u64,
    pub nonce: Option<String>,
    pub previous_market_state_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsOperationBundle {
    pub schema: String,
    pub asset_id: String,
    pub issuer: String,
    pub epoch: u64,
    pub reserve_packet_hash: String,
    pub supply_packet_hash: String,
    pub evidence_root: String,
    pub expected_envelope_hash: String,
    pub policy: MarketOpsPolicyRegistration,
    pub policy_inputs: MarketOpsPolicyInputs,
    pub envelope: MarketOpsEnvelope,
    pub policy_register_operation: AssetTransactionOperation,
    pub market_ops_finalize_operation: AssetTransactionOperation,
    pub policy_register_operation_file: String,
    pub market_ops_finalize_operation_file: String,
    pub replay_bundle_file: String,
    pub commands_file: String,
    pub relay_commands: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsPublicStatus {
    pub schema: String,
    pub asset_id: String,
    pub nav_floor_usd_e8: u128,
    pub verified_net_assets_usd_e8: u128,
    pub valid_global_supply_atoms: u128,
    pub reserve_packet_fresh: bool,
    pub supply_packet_fresh: bool,
    pub reserve_packet_age_blocks: u64,
    pub supply_packet_age_blocks: u64,
    pub funded_alignment_reserve_usd_e8: u128,
    pub required_alignment_reserve_usd_e8: u128,
    pub current_reserve_deploy_cap_usd_e8: u128,
    pub current_mint_cap_atoms: u128,
    pub market_operations_status: String,
    pub accepted_policy_hash: String,
    pub envelope_hash: String,
    pub envelope_epoch: u64,
    pub packet_expires_at: u64,
    pub disclosure: String,
}

impl MarketOpsPublicStatus {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != MARKET_OPS_PUBLIC_STATUS_SCHEMA {
            return Err(format!(
                "market_ops_public_status.schema must be `{MARKET_OPS_PUBLIC_STATUS_SCHEMA}`"
            ));
        }
        validate_lower_hex_len(
            "market_ops_public_status.asset_id",
            &self.asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "market_ops_public_status.accepted_policy_hash",
            &self.accepted_policy_hash,
            64,
        )?;
        validate_lower_hex_len(
            "market_ops_public_status.envelope_hash",
            &self.envelope_hash,
            96,
        )?;
        if self.nav_floor_usd_e8 == 0 {
            return Err("market_ops_public_status.nav_floor_usd_e8 must be nonzero".to_string());
        }
        if self.verified_net_assets_usd_e8 == 0 {
            return Err(
                "market_ops_public_status.verified_net_assets_usd_e8 must be nonzero".to_string(),
            );
        }
        if self.valid_global_supply_atoms == 0 {
            return Err(
                "market_ops_public_status.valid_global_supply_atoms must be nonzero".to_string(),
            );
        }
        if self.envelope_epoch == 0 {
            return Err("market_ops_public_status.envelope_epoch must be nonzero".to_string());
        }
        validate_market_ops_public_status_value(&self.market_operations_status)?;
        validate_market_ops_disclosure_text(&self.disclosure)?;
        if self.market_operations_status != MARKET_OPS_STATUS_ACTIVE {
            if self.current_reserve_deploy_cap_usd_e8 != 0 {
                return Err(
                    "market_ops_public_status.current_reserve_deploy_cap_usd_e8 must be zero unless active"
                        .to_string(),
                );
            }
            if self.current_mint_cap_atoms != 0 {
                return Err(
                    "market_ops_public_status.current_mint_cap_atoms must be zero unless active"
                        .to_string(),
                );
            }
        }
        Ok(())
    }
}

fn validate_market_ops_public_status_value(status: &str) -> Result<(), String> {
    match status {
        MARKET_OPS_STATUS_ACTIVE
        | MARKET_OPS_STATUS_EXPIRED
        | MARKET_OPS_STATUS_MISSING_SOURCE_PACKET
        | MARKET_OPS_STATUS_PAUSED
        | MARKET_OPS_STATUS_STALE
        | MARKET_OPS_STATUS_UNDERFUNDED => Ok(()),
        _ => Err(format!(
            "market_ops_public_status.market_operations_status `{status}` is not recognized"
        )),
    }
}

fn validate_market_ops_disclosure_text(disclosure: &str) -> Result<(), String> {
    if disclosure != MARKET_OPS_PUBLIC_DISCLOSURE {
        return Err(
            "market_ops_public_status.disclosure must use the canonical disclosure".to_string(),
        );
    }
    for phrase in [
        "peg",
        "redemption facility",
        "guaranteed support",
        "guaranteed liquidity",
        "guaranteed exit liquidity",
        "stable value",
        "risk-free yield",
        "investment return",
        "instant exit at NAV",
        "automatic redemption at NAV",
    ] {
        if disclosure_contains_forbidden_phrase(disclosure, phrase) {
            return Err(format!(
                "market_ops_public_status.disclosure contains forbidden phrase `{phrase}`"
            ));
        }
    }
    Ok(())
}

fn disclosure_contains_forbidden_phrase(disclosure: &str, phrase: &str) -> bool {
    if phrase == "peg" {
        return disclosure
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .any(|token| token.eq_ignore_ascii_case("peg"));
    }
    disclosure
        .to_ascii_lowercase()
        .contains(&phrase.to_ascii_lowercase())
}

pub(super) fn validate_lower_hex_len(
    field: &str,
    value: &str,
    expected_len: usize,
) -> Result<(), String> {
    if value.len() != expected_len {
        return Err(format!(
            "{field} must be {expected_len} lowercase hex chars"
        ));
    }
    if value
        .bytes()
        .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be lowercase hex"));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsReplayBundle {
    pub schema: String,
    pub asset_id: String,
    pub epoch: u64,
    pub reserve_packet: NavReservePacket,
    pub envelope: MarketOpsEnvelope,
    pub policy_inputs: MarketOpsPolicyInputs,
    pub reserve_packet_hash: String,
    pub supply_packet_hash: String,
    pub evidence_root: String,
    pub program_id: String,
    pub policy_hash: String,
    pub parameter_hash: String,
    pub previous_market_state_hash: String,
    pub expected_envelope_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketOpsReplayReport {
    pub schema: String,
    pub bundle_file: String,
    pub asset_id: String,
    pub epoch: u64,
    pub expected_envelope_hash: String,
    pub computed_envelope_hash: String,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReserveReplayBundle {
    pub schema: String,
    pub chain_id: String,
    pub protocol_version: u32,
    pub asset_id: String,
    pub epoch: u64,
    pub asset_definition: AssetDefinition,
    pub nav_asset: NavTrackedAsset,
    pub reserve_packet: NavReservePacket,
    pub trustlines: Vec<TrustLine>,
    pub escrows: Vec<Escrow>,
    pub offers: Vec<Offer>,
    pub fast_lane_reserves: Vec<FastLaneReserveBalanceV1>,
    pub pftl_uniswap_routes: Vec<PftlUniswapConsensusRouteState>,
    pub asset_orchard_balances: Vec<AssetOrchardAssetBalance>,
    pub buckets: Vec<VaultBridgeBucketState>,
    pub receipts: Vec<VaultBridgeReceipt>,
    pub allocations: Vec<VaultBridgeAllocation>,
    pub redemptions: Vec<VaultBridgeRedemption>,
    pub expected_source_root: String,
    pub expected_counted_value_atoms: u64,
    pub expected_issued_supply_atoms: u64,
    pub expected_reserve_packet_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultBridgeReserveReplayReport {
    pub schema: String,
    pub bundle_file: String,
    pub asset_id: String,
    pub epoch: u64,
    pub expected_reserve_packet_hash: String,
    pub source_root: String,
    pub counted_value_atoms: u64,
    pub issued_supply_atoms: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRangeReport {
    pub first_height: Option<u64>,
    pub last_height: Option<u64>,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRetentionPolicyReport {
    pub mode: String,
    pub retain_recent_blocks: u64,
    pub retain_recent_receipts: u64,
    pub retain_recent_batches: u64,
    pub retain_recent_governance: u64,
    pub minimum_replay_window_blocks: u64,
    pub advisory_prune: bool,
    pub archive_handoff_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryStorageFileReport {
    pub path: String,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryStatusReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub node_id: String,
    pub current_height: u64,
    pub block_tip_hash: String,
    pub policy: HistoryRetentionPolicyReport,
    pub local_block_range: HistoryRangeReport,
    pub receipt_count: usize,
    pub archived_batch_count: usize,
    pub ordered_batch_count: usize,
    pub governance_amendment_count: usize,
    pub governance_registry_update_count: usize,
    pub block_log_verified: bool,
    pub storage_files: Vec<HistoryStorageFileReport>,
    pub partial_history_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPrunePlanReport {
    pub schema: String,
    pub dry_run: bool,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub node_id: String,
    pub current_height: u64,
    pub policy: HistoryRetentionPolicyReport,
    pub requested_prune_up_to_height: Option<u64>,
    pub computed_prune_up_to_height: Option<u64>,
    pub retain_from_height: u64,
    pub eligible_block_count: usize,
    pub eligible_batch_count: usize,
    pub eligible_receipt_count: usize,
    pub block_log_verified: bool,
    pub archive_handoff_present: bool,
    pub archive_handoff_verified: bool,
    pub archive_handoff_error: Option<String>,
    pub prune_allowed: bool,
    pub refusal_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryCheckpointState {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pruned_up_to_height: u64,
    pub checkpoint_block_hash: String,
    pub checkpoint_state_root: String,
    pub archive_handoff_proof_hash: String,
    pub block_range_root: String,
    pub batch_payload_root: String,
    pub receipt_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub native_fee_burn_total: Option<u128>,
    pub governance: GovernanceState,
    pub ledger: LedgerState,
    pub ordered_batches: Vec<String>,
    pub shielded: ShieldedState,
    pub bridge: BridgeState,
    pub validator_registry: ValidatorRegistry,
    pub checkpoint_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryCheckpointRebuildFromArchiveReport {
    pub schema: String,
    pub rebuilt: bool,
    pub legacy_schema: String,
    pub legacy_checkpoint_file: String,
    pub backup_file: String,
    pub pruned_up_to_height: u64,
    pub archive_from_height: u64,
    pub archive_to_height: u64,
    pub archive_window_count: usize,
    pub prefix_verification: BlockVerificationReport,
    pub retained_suffix_verification: BlockVerificationReport,
    pub checkpoint: HistoryCheckpointState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPruneJournalRecord {
    pub schema: String,
    pub prune_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pruned_up_to_height: u64,
    pub archive_handoff_proof_hash: String,
    pub checkpoint_hash: String,
    pub pruned_block_count: usize,
    pub pruned_batch_count: usize,
    pub pruned_receipt_count: usize,
    pub remaining_first_height: Option<u64>,
    pub remaining_last_height: Option<u64>,
    pub remaining_block_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPruneJournal {
    pub schema: String,
    pub records: Vec<HistoryPruneJournalRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct HistoryPruneBatchKey {
    pub batch_kind: String,
    pub batch_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPrunePending {
    pub schema: String,
    pub checkpoint: HistoryCheckpointState,
    pub journal_record: HistoryPruneJournalRecord,
    pub pruned_batch_keys: Vec<HistoryPruneBatchKey>,
    pub pruned_receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPruneReport {
    pub schema: String,
    pub pruned: bool,
    pub plan: HistoryPrunePlanReport,
    pub checkpoint: HistoryCheckpointState,
    pub journal_record: HistoryPruneJournalRecord,
    pub before_block_range: HistoryRangeReport,
    pub after_block_range: HistoryRangeReport,
    pub pruned_block_count: usize,
    pub pruned_batch_count: usize,
    pub pruned_receipt_count: usize,
    pub remaining_receipt_count: usize,
    pub remaining_batch_count: usize,
    pub verify_after_prune: BlockVerificationReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryPruneRecoveryReport {
    pub schema: String,
    pub recovered: bool,
    pub pending_file: String,
    pub pruned_up_to_height: Option<u64>,
    pub checkpoint_hash: Option<String>,
    pub prune_id: Option<String>,
    pub pruned_block_count: Option<usize>,
    pub pruned_batch_count: Option<usize>,
    pub pruned_receipt_count: Option<usize>,
    pub before_block_range: Option<HistoryRangeReport>,
    pub after_block_range: Option<HistoryRangeReport>,
    pub verify_after_recovery: Option<BlockVerificationReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveHandoffProof {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub archive_uri: String,
    pub from_height: u64,
    pub to_height: u64,
    pub block_count: usize,
    pub batch_count: usize,
    pub receipt_count: usize,
    pub first_block_hash: String,
    pub last_block_hash: String,
    pub block_range_root: String,
    pub batch_payload_root: String,
    pub receipt_root: String,
    pub proof_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveHandoffVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub proof_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub archive_uri: String,
    pub from_height: u64,
    pub to_height: u64,
    pub block_count: usize,
    pub batch_count: usize,
    pub receipt_count: usize,
    pub proof_hash: String,
    pub block_range_root: String,
    pub batch_payload_root: String,
    pub receipt_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveWindowBundle {
    pub schema: String,
    pub proof: HistoryArchiveHandoffProof,
    pub blocks: Vec<BlockRecord>,
    pub batches: Vec<BatchArchiveEntry>,
    pub receipts: Vec<Receipt>,
    pub bundle_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveWindowVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub bundle_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub from_height: u64,
    pub to_height: u64,
    pub block_count: usize,
    pub batch_count: usize,
    pub receipt_count: usize,
    pub proof_hash: String,
    pub bundle_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveWindowIndexEntry {
    pub from_height: u64,
    pub to_height: u64,
    pub block_count: usize,
    pub batch_count: usize,
    pub receipt_count: usize,
    pub proof_hash: String,
    pub bundle_hash: String,
    pub archive_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveWindowIndex {
    pub schema: String,
    pub windows: Vec<HistoryArchiveWindowIndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryArchiveWindowImportReport {
    pub schema: String,
    pub imported: bool,
    pub bundle_file: String,
    pub archive_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub from_height: u64,
    pub to_height: u64,
    pub block_count: usize,
    pub batch_count: usize,
    pub receipt_count: usize,
    pub proof_hash: String,
    pub bundle_hash: String,
    pub archived_window_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletKeygenOptions {
    pub chain_id: String,
    pub master_seed_hex: String,
    pub account_index: u32,
    pub key_file: PathBuf,
    pub backup_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletRestoreOptions {
    pub backup_file: PathBuf,
    pub key_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletTestVectorOptions {
    pub chain_id: String,
    pub validator_count: u32,
    pub master_seed_hex: String,
    pub account_index: u32,
    pub to: String,
    pub amount: u64,
    pub sequence: u64,
    pub signature_seed_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSignTransferOptions {
    pub key_file: PathBuf,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSignAssetTransactionOptions {
    pub key_file: PathBuf,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub fee: u64,
    pub sequence: u64,
    pub expected_source: Option<String>,
    pub operation: AssetTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSignEscrowTransactionOptions {
    pub key_file: PathBuf,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub fee: u64,
    pub sequence: u64,
    pub expected_source: Option<String>,
    pub operation: EscrowTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalletSignOfferTransactionOptions {
    pub key_file: PathBuf,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub fee: u64,
    pub sequence: u64,
    pub expected_source: Option<String>,
    pub operation: OfferTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardWalletKeygenOptions {
    pub master_seed_hex: String,
    pub account_index: u32,
    pub key_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardViewKeyExportOptions {
    pub key_file: PathBuf,
    pub view_key_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardOutputActionOptions {
    pub data_dir: PathBuf,
    pub recipient_address_raw_hex: Option<String>,
    pub recipient_key_file: Option<PathBuf>,
    pub recipient_view_key_file: Option<PathBuf>,
    pub memo_hex: Option<String>,
    pub value: u64,
    pub fee: u64,
    pub action_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardTestVectorOptions {
    pub chain_id: String,
    pub validator_count: u32,
    pub spending_key_seed_hex: String,
    pub value: u64,
    pub fee: u64,
    pub build_seed_hex: String,
    pub proof_seed_hex: String,
    pub signature_seed_hex: String,
    pub external_binding_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardDepositActionOptions {
    pub data_dir: PathBuf,
    pub key_file: Option<PathBuf>,
    pub recipient_address_raw_hex: Option<String>,
    pub recipient_key_file: Option<PathBuf>,
    pub recipient_view_key_file: Option<PathBuf>,
    pub memo_hex: Option<String>,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
    pub deposit_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardDepositActionBatchOptions {
    pub data_dir: PathBuf,
    pub deposit_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardDepositActionFile {
    pub schema: String,
    pub action: OrchardShieldedAction,
    pub funding_transfer: SignedTransfer,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub external_binding_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardDepositActionReport {
    pub schema: String,
    pub deposit_file: String,
    pub pool_id: String,
    pub anchor: String,
    pub from: String,
    pub recipient_address_raw_hex: String,
    pub amount: u64,
    pub fee: u64,
    pub minimum_fee: u64,
    pub funding_transfer_fee: u64,
    pub funding_transfer_id: String,
    pub external_binding_hash: String,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub value_balance: i64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardIngressCreateOptions {
    pub data_dir: PathBuf,
    pub key_file: PathBuf,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
    pub note_seed_hex: String,
    pub encrypted_output_hex: Option<String>,
    pub ingress_file: PathBuf,
    pub note_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardIngressBatchOptions {
    pub data_dir: PathBuf,
    pub ingress_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardEgressCreateOptions {
    pub data_dir: PathBuf,
    pub note_file: PathBuf,
    pub to: String,
    pub amount: Option<u64>,
    pub egress_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardEgressBatchOptions {
    pub data_dir: PathBuf,
    pub egress_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPrivateEgressCreateOptions {
    pub data_dir: PathBuf,
    pub note_file: PathBuf,
    pub to: String,
    pub asset_id: Option<String>,
    pub amount: Option<u64>,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub egress_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardPrivateEgressBatchOptions {
    pub data_dir: PathBuf,
    pub egress_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardNoteStatusOptions {
    pub data_dir: PathBuf,
    pub note_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardScanOptions {
    pub data_dir: PathBuf,
    pub note_seed_hex: String,
    pub note_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardIngressFile {
    pub schema: String,
    pub payload: AssetOrchardIngressV2ActionPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardEgressFile {
    pub schema: String,
    pub payload: AssetOrchardEgressActionPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressFile {
    pub schema: String,
    pub payload: AssetOrchardPrivateEgressActionPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardIngressReport {
    pub schema: String,
    pub ingress_file: String,
    pub note_file: String,
    pub pool_id: String,
    pub from: String,
    pub asset_id: String,
    pub amount: u64,
    pub asset_tag_lo: u128,
    pub asset_tag_hi: u128,
    pub output_commitment: String,
    pub encrypted_output_bytes: usize,
    pub burn_transaction_id: String,
    pub burn_fee: u64,
    pub minimum_burn_fee: u64,
    pub sequence: u64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardEgressReport {
    pub schema: String,
    pub egress_file: String,
    pub note_file: String,
    pub pool_id: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub output_commitment: String,
    pub nullifier: String,
    pub spend_auth_verification_key: String,
    pub randomized_verification_key: String,
    pub sighash: String,
    pub verified: bool,
    pub privacy: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressReport {
    pub schema: String,
    pub egress_file: String,
    pub note_file: String,
    pub pool_id: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub anchor: String,
    pub nullifier: String,
    pub randomized_verification_key: String,
    pub exit_binding_hash: String,
    pub proof_bytes: usize,
    pub verified: bool,
    pub privacy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressCreateTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub store_state_load_ms: f64,
    pub note_read_prep_ms: f64,
    pub action_build_ms: f64,
    pub serialized_action_verification_ms: f64,
    pub payload_genesis_validation_ms: f64,
    pub file_write_report_ms: f64,
    pub action_build_breakdown: AssetOrchardPrivateEgressTimingReport,
    pub serialized_action_verification_breakdown: AssetOrchardPrivateEgressTimingReport,
    pub payload_genesis_validation_breakdown: AssetOrchardPrivateEgressTimingReport,
}

impl Default for AssetOrchardPrivateEgressCreateTimingReport {
    fn default() -> Self {
        Self {
            schema: "postfiat.asset_orchard_private_egress.create_timing.v1".to_string(),
            total_ms: 0.0,
            store_state_load_ms: 0.0,
            note_read_prep_ms: 0.0,
            action_build_ms: 0.0,
            serialized_action_verification_ms: 0.0,
            payload_genesis_validation_ms: 0.0,
            file_write_report_ms: 0.0,
            action_build_breakdown: AssetOrchardPrivateEgressTimingReport::default(),
            serialized_action_verification_breakdown:
                AssetOrchardPrivateEgressTimingReport::default(),
            payload_genesis_validation_breakdown: AssetOrchardPrivateEgressTimingReport::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressBatchTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub store_genesis_load_ms: f64,
    pub egress_file_read_ms: f64,
    pub payload_genesis_validation_ms: f64,
    pub batch_build_ms: f64,
    pub batch_write_ms: f64,
    pub payload_genesis_validation_breakdown: AssetOrchardPrivateEgressTimingReport,
}

impl Default for AssetOrchardPrivateEgressBatchTimingReport {
    fn default() -> Self {
        Self {
            schema: "postfiat.asset_orchard_private_egress.batch_timing.v1".to_string(),
            total_ms: 0.0,
            store_genesis_load_ms: 0.0,
            egress_file_read_ms: 0.0,
            payload_genesis_validation_ms: 0.0,
            batch_build_ms: 0.0,
            batch_write_ms: 0.0,
            payload_genesis_validation_breakdown: AssetOrchardPrivateEgressTimingReport::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardNoteStatusReport {
    pub schema: String,
    pub note_file: String,
    pub pool_id: String,
    pub asset_id: String,
    pub amount: u64,
    pub output_commitment: String,
    pub nullifier: String,
    pub pool_output: bool,
    pub spent: bool,
    pub spendable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardScanReport {
    pub schema: String,
    pub note_file: String,
    pub pool_id: String,
    pub chain_output_count: usize,
    pub encrypted_output_v1_count: usize,
    pub legacy_output_count: usize,
    pub nonmatching_output_count: usize,
    pub output_commitment: String,
    pub recovered: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetOrchardPrivateEgressNodeTimingReport {
    pub schema: String,
    pub state_applications: Vec<AssetOrchardPrivateEgressStateApplyTimingReport>,
}

impl AssetOrchardPrivateEgressNodeTimingReport {
    pub fn observed_total_ms(&self) -> f64 {
        self.state_applications
            .iter()
            .map(|timing| timing.total_ms)
            .sum()
    }

    pub fn is_empty(&self) -> bool {
        self.state_applications.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetOrchardPrivateEgressStateApplyTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub payload_decode_ms: f64,
    pub domain_ms: f64,
    pub verifier_ms: f64,
    pub pool_lookup_ms: f64,
    pub retained_anchor_check_ms: f64,
    pub nullifier_check_ms: f64,
    pub trial_ledger_clone_ms: f64,
    pub trial_ledger_credit_ms: f64,
    pub trial_ledger_validate_asset_ms: f64,
    pub trial_ledger_validate_nav_ms: f64,
    pub trial_shielded_clone_ms: f64,
    pub trial_shielded_nullifier_push_ms: f64,
    pub verify_shielded_state_ms: f64,
    pub commit_state_ms: f64,
    pub receipt_ms: f64,
    pub result: String,
    pub accepted: bool,
    pub receipt_code: String,
    pub verifier_breakdown: AssetOrchardPrivateEgressTimingReport,
}

impl Default for AssetOrchardPrivateEgressStateApplyTimingReport {
    fn default() -> Self {
        Self {
            schema: "postfiat.asset_orchard_private_egress.state_apply_timing.v1".to_string(),
            total_ms: 0.0,
            payload_decode_ms: 0.0,
            domain_ms: 0.0,
            verifier_ms: 0.0,
            pool_lookup_ms: 0.0,
            retained_anchor_check_ms: 0.0,
            nullifier_check_ms: 0.0,
            trial_ledger_clone_ms: 0.0,
            trial_ledger_credit_ms: 0.0,
            trial_ledger_validate_asset_ms: 0.0,
            trial_ledger_validate_nav_ms: 0.0,
            trial_shielded_clone_ms: 0.0,
            trial_shielded_nullifier_push_ms: 0.0,
            verify_shielded_state_ms: 0.0,
            commit_state_ms: 0.0,
            receipt_ms: 0.0,
            result: "unknown".to_string(),
            accepted: false,
            receipt_code: String::new(),
            verifier_breakdown: AssetOrchardPrivateEgressTimingReport::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShieldedBatchProposalTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub read_genesis_ms: f64,
    pub read_ledger_ms: f64,
    pub read_governance_ms: f64,
    pub read_shielded_ms: f64,
    pub read_batch_ms: f64,
    pub verify_batch_id_ms: f64,
    pub read_ordered_batches_ms: f64,
    pub duplicate_check_ms: f64,
    pub read_bridge_ms: f64,
    pub chain_tip_ms: f64,
    pub activation_ms: f64,
    pub state_exec_ms: f64,
    pub build_proposal_from_state_ms: f64,
    pub verifier_setup_ms: f64,
    pub private_egress_verifier_breakdown: AssetOrchardPrivateEgressTimingReport,
    pub private_egress_state_breakdown: AssetOrchardPrivateEgressNodeTimingReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatchProposalTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub verify_block_log_ms: f64,
    pub store_init_ms: f64,
    pub batch_kind_ms: f64,
    pub ordered_proposal_ms: f64,
    pub timeout_evidence_ms: f64,
    pub signing_ms: f64,
    pub signature_verify_ms: f64,
    pub serialization_ms: f64,
    pub verifier_setup_ms: f64,
    pub state_exec_ms: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shielded_breakdown: Option<ShieldedBatchProposalTimingReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchProposalWithTimingsReport {
    pub proposal: BlockProposalFile,
    pub timings: BatchProposalTimingReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockVoteTargetTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub proposal_read_ms: f64,
    pub proposal_validate_ms: f64,
    pub proposal_signature_verify_ms: f64,
    pub proposal_height_check_ms: f64,
    pub timeout_evidence_ms: f64,
    pub proposal_rebuild_compare_ms: f64,
    pub local_proof_verify_ms: f64,
    pub governance_ms: f64,
    pub active_validators_ms: f64,
    pub proposal_hash_ms: f64,
    pub block_read_ms: f64,
    pub private_egress_verifier_breakdown: AssetOrchardPrivateEgressTimingReport,
    pub private_egress_state_breakdown: AssetOrchardPrivateEgressNodeTimingReport,
    #[serde(default)]
    pub asset_orchard_swap_verifier_breakdown: AssetOrchardSwapTimingReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BlockVoteCreationTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub verify_block_log_ms: f64,
    pub store_init_ms: f64,
    pub read_genesis_ms: f64,
    pub target_ms: f64,
    pub key_read_ms: f64,
    pub key_validation_ms: f64,
    pub validator_membership_ms: f64,
    pub registry_read_ms: f64,
    pub registry_key_check_ms: f64,
    pub vote_lock_reservation_ms: f64,
    pub message_build_ms: f64,
    pub private_key_decode_ms: f64,
    pub mldsa_signing_ms: f64,
    pub vote_construct_ms: f64,
    pub vote_validation_ms: f64,
    pub json_serde_ms: f64,
    pub vote_file_write_ms: f64,
    pub process_spawn_ms: f64,
    pub target_breakdown: BlockVoteTargetTimingReport,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockVoteWithTimingsReport {
    pub vote: BlockVoteFile,
    pub timings: BlockVoteCreationTimingReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetOrchardSwapCreateOptions {
    pub data_dir: PathBuf,
    pub input_note_files: [PathBuf; 2],
    pub output_note_seed_hexes: [String; 2],
    pub pricing_claim_file: PathBuf,
    pub action_file: PathBuf,
    pub output_note_files: [PathBuf; 2],
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetOrchardSwapCreateReport {
    pub schema: String,
    pub action_file: String,
    pub output_note_files: [String; 2],
    pub pool_id: String,
    pub anchor: String,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    pub proof_bytes: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProposerOptions {
    pub data_dir: PathBuf,
    pub block_height: u64,
    pub view: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockProposerReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub local_node_id: String,
    pub block_height: u64,
    pub view: u64,
    pub active_validator_count: u32,
    pub proposer: String,
    pub local_is_proposer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeMetrics {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub node_id: String,
    #[serde(default)]
    pub observed_unix_ms: u64,
    pub consensus: ConsensusMetrics,
    pub ordering: OrderingMetrics,
    pub execution: ExecutionMetrics,
    pub assets: AssetMetrics,
    pub mempool: MempoolMetrics,
    pub storage: StorageMetrics,
    #[serde(default)]
    pub proofs: ProofMetrics,
    pub shielded: ShieldedMetrics,
    pub bridge: BridgeMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusMetrics {
    pub active_validator_count: u32,
    pub crypto_policy_version: u32,
    pub bridge_witness_epoch: u32,
    pub authority_mode: u32,
    pub amendment_count: u64,
    pub validator_registry_update_count: u64,
    #[serde(default)]
    pub block_certificate_count: u64,
    #[serde(default)]
    pub block_certificate_vote_count: u64,
    #[serde(default)]
    pub recent_certificate_window_blocks: u64,
    #[serde(default)]
    pub recent_certificate_vote_count: u64,
    #[serde(default)]
    pub local_recent_certificate_vote_count: u64,
    #[serde(default)]
    pub local_recent_certificate_participation_ppm: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderingMetrics {
    pub block_height: u64,
    pub block_tip_hash: String,
    pub ordered_batch_count: u64,
    pub archived_batch_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    pub account_count: u64,
    pub receipt_count: u64,
    pub burned_fee_total: u64,
    pub account_reserve: u64,
    pub minimum_transfer_fee: u64,
    pub transfer_account_creation_fee: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AssetMetrics {
    pub asset_count: u64,
    pub trustline_count: u64,
    pub holder_count: u64,
    pub total_outstanding_supply: u64,
    pub open_issued_escrow_count: u64,
    pub open_issued_escrow_amount: u64,
    pub open_issued_offer_count: u64,
    pub open_issued_offer_amount: u64,
    pub authorization_required_asset_count: u64,
    pub freeze_enabled_asset_count: u64,
    pub clawback_enabled_asset_count: u64,
    pub unauthorized_trustline_count: u64,
    pub frozen_trustline_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolMetrics {
    pub pending: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMetrics {
    pub replicated_state_file_count: u64,
    #[serde(default)]
    pub filesystem_total_bytes: u64,
    #[serde(default)]
    pub filesystem_available_bytes: u64,
    #[serde(default)]
    pub filesystem_available_ppm: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ProofMetrics {
    pub last_verify_micros: u64,
    pub last_observed_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedMetrics {
    pub note_count: u64,
    pub nullifier_count: u64,
    pub turnstile_event_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeMetrics {
    pub domain_count: u64,
    pub transfer_count: u64,
    pub replay_cache_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferFeeQuoteReport {
    pub schema: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_kind: Option<String>,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub recipient_exists: bool,
    pub will_create_recipient_account: bool,
    pub base_transfer_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_account_creation_fee: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub transfer_weight_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memo_bytes: Option<u64>,
    pub sender_balance_after_amount_and_fee: Option<u64>,
    pub sender_meets_reserve_after_transfer: bool,
    pub recipient_balance_after_amount: Option<u64>,
    pub recipient_meets_reserve_after_transfer: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorKeysOptions {
    pub data_dir: PathBuf,
    pub validators: u32,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorKeyStageOptions {
    pub data_dir: PathBuf,
    pub source_key_file: PathBuf,
    pub validator_id: String,
    pub source_validator_id: Option<String>,
    pub replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorKeyStageReport {
    pub schema: String,
    pub validator_id: String,
    pub source_validator_id: String,
    pub action: String,
    pub validator_key_count: u32,
    pub registry_public_key_matched: bool,
    pub key_file: String,
    pub source_key_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferOptions {
    pub data_dir: PathBuf,
    pub key_file: Option<PathBuf>,
    pub to: String,
    pub amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub from: String,
    pub to: String,
    pub amount: u64,
    pub sequence: Option<u64>,
    pub memo_type: Option<String>,
    pub memo_format: Option<String>,
    pub memo_data: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub source: String,
    pub operation_json: String,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscrowFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub source: String,
    pub operation_json: String,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AtomicSettlementTemplateOptions {
    pub data_dir: PathBuf,
    pub left_owner: String,
    pub left_recipient: String,
    pub left_asset_id: String,
    pub left_amount: u64,
    pub right_owner: String,
    pub right_recipient: String,
    pub right_asset_id: String,
    pub right_amount: u64,
    pub condition: String,
    pub finish_after: u64,
    pub cancel_after: u64,
    pub left_sequence: Option<u64>,
    pub right_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetInfoOptions {
    pub data_dir: PathBuf,
    pub asset_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountLinesOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub issuer: Option<String>,
    pub asset_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAssetsOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub asset_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedObjectsOptions {
    pub data_dir: PathBuf,
    pub owner_public_key_hex: String,
    pub asset: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerAssetsOptions {
    pub data_dir: PathBuf,
    pub issuer: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EscrowInfoOptions {
    pub data_dir: PathBuf,
    pub escrow_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountEscrowsOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub role: Option<String>,
    pub state: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NftInfoOptions {
    pub data_dir: PathBuf,
    pub nft_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountNftsOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub include_burned: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuerNftsOptions {
    pub data_dir: PathBuf,
    pub issuer: String,
    pub collection_id: Option<String>,
    pub include_burned: bool,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetFeeQuoteReport {
    pub schema: String,
    pub transaction_kind: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_asset_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub asset_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
    pub operation: AssetTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowFeeQuoteReport {
    pub schema: String,
    pub transaction_kind: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_escrow_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub escrow_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
    pub operation: EscrowTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NftFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub source: String,
    pub operation_json: String,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferFeeQuoteOptions {
    pub data_dir: PathBuf,
    pub source: String,
    pub operation_json: String,
    pub sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferInfoOptions {
    pub data_dir: PathBuf,
    pub offer_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountOffersOptions {
    pub data_dir: PathBuf,
    pub account: String,
    pub state: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookOffersOptions {
    pub data_dir: PathBuf,
    pub taker_gets_asset_id: String,
    pub taker_pays_asset_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftFeeQuoteReport {
    pub schema: String,
    pub transaction_kind: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_nft_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub nft_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
    pub issuer_transfer_fee: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer_transfer_fee_recipient: Option<String>,
    pub sender_balance_after_fee_and_issuer_transfer_fee: Option<u64>,
    pub sender_meets_reserve_after_fee_and_issuer_transfer_fee: bool,
    pub operation: NftTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferFeeQuoteReport {
    pub schema: String,
    pub transaction_kind: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source: String,
    pub sequence: u64,
    pub sequence_source: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub base_offer_fee: u64,
    pub match_fee: u64,
    pub state_expansion_fee: u64,
    pub estimated_cross_count: u64,
    pub max_dex_crosses_per_transaction: u64,
    pub will_create_residual_offer: bool,
    pub offer_object_reserve: u64,
    pub minimum_fee: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub offer_weight_bytes: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_balance_after_fee_and_reserve: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
    pub sender_meets_reserve_after_fee_and_reserve: bool,
    pub operation: OfferTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSettlementTemplateLegReport {
    pub owner: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    pub sequence: u64,
    pub sequence_source: String,
    pub escrow_id: String,
    pub transaction_kind: String,
    pub base_escrow_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub escrow_weight_bytes: u64,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub mempool_pending_for_sender: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
    pub operation: EscrowTransactionOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSettlementTemplateReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub settlement_id: String,
    pub condition_hash: String,
    pub condition: String,
    pub finish_after: u64,
    pub cancel_after: u64,
    pub left: AtomicSettlementTemplateLegReport,
    pub right: AtomicSettlementTemplateLegReport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedAssetReport {
    pub asset_id: String,
    pub issuer: String,
    pub code: String,
    pub version: u32,
    pub precision: u8,
    pub display_name: String,
    pub max_supply: Option<u64>,
    pub requires_authorization: bool,
    pub freeze_enabled: bool,
    pub clawback_enabled: bool,
    pub outstanding_supply: u64,
    pub trustline_count: u64,
    pub holder_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetLineReport {
    pub trustline_id: String,
    pub account: String,
    pub issuer: String,
    pub asset_id: String,
    pub code: String,
    pub version: u32,
    pub precision: u8,
    pub balance: u64,
    pub limit: u64,
    pub authorized: bool,
    pub frozen: bool,
    pub reserve_paid: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub asset_id: String,
    pub found: bool,
    pub asset: Option<IssuedAssetReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountLinesReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account: String,
    pub issuer: Option<String>,
    pub asset_id: Option<String>,
    pub limit: u64,
    pub truncated: bool,
    pub line_count: u64,
    pub lines: Vec<AssetLineReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountAssetsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account: String,
    pub asset_id: Option<String>,
    pub limit: u64,
    pub truncated: bool,
    pub asset_count: u64,
    pub assets: Vec<AssetLineReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedObjectsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub owner_public_key_hex: String,
    pub asset: Option<String>,
    pub limit: u64,
    pub truncated: bool,
    pub object_count: u64,
    pub total_value: u64,
    pub objects: Vec<OwnedObject>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedApplyReport {
    pub schema: String,
    pub quorum: usize,
    pub validator_count: usize,
    pub consumed_count: usize,
    pub created_count: usize,
    pub created_objects: Vec<OwnedObject>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnedUnwrapApplyReport {
    pub schema: String,
    pub quorum: usize,
    pub validator_count: usize,
    pub consumed_count: usize,
    pub credited: u64,
    pub credited_to: String,
    pub change_object: Option<OwnedObject>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerAssetsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub issuer: String,
    pub limit: u64,
    pub truncated: bool,
    pub asset_count: u64,
    pub assets: Vec<IssuedAssetReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowReport {
    pub escrow_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_hash: Option<String>,
    pub finish_after: u64,
    pub cancel_after: u64,
    pub state: String,
    pub created_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub escrow_id: String,
    pub found: bool,
    pub escrow: Option<EscrowReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountEscrowsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account: String,
    pub role: Option<String>,
    pub state: Option<String>,
    pub limit: u64,
    pub truncated: bool,
    pub escrow_count: u64,
    pub escrows: Vec<EscrowReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftReport {
    pub nft_id: String,
    pub issuer: String,
    pub collection_id: String,
    pub serial: u64,
    pub owner: String,
    pub metadata_hash: String,
    pub metadata_uri: String,
    pub flags: u32,
    pub collection_flags: u32,
    pub issuer_transfer_fee: u64,
    pub transferable: bool,
    pub issuer_burnable: bool,
    pub collection_transfer_locked: bool,
    pub collection_burn_locked: bool,
    pub burned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub nft_id: String,
    pub found: bool,
    pub nft: Option<NftReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountNftsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account: String,
    pub include_burned: bool,
    pub limit: u64,
    pub truncated: bool,
    pub nft_count: u64,
    pub nfts: Vec<NftReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuerNftsReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub issuer: String,
    pub collection_id: Option<String>,
    pub include_burned: bool,
    pub limit: u64,
    pub truncated: bool,
    pub nft_count: u64,
    pub nfts: Vec<NftReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferReport {
    pub offer_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub taker_gets_asset_id: String,
    pub taker_gets_amount_remaining: u64,
    pub taker_pays_asset_id: String,
    pub taker_pays_amount_remaining: u64,
    pub original_taker_gets_amount: u64,
    pub original_taker_pays_amount: u64,
    pub created_height: u64,
    pub expiration_height: u64,
    pub reserve_paid: u64,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfferInfoReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub offer_id: String,
    pub found: bool,
    pub offer: Option<OfferReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountOffersReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub account: String,
    pub state: Option<String>,
    pub limit: u64,
    pub truncated: bool,
    pub offer_count: u64,
    pub offers: Vec<OfferReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookOffersReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub taker_gets_asset_id: String,
    pub taker_pays_asset_id: String,
    pub limit: u64,
    pub truncated: bool,
    pub offer_count: u64,
    pub offers: Vec<OfferReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTransferSubmitOptions {
    pub data_dir: PathBuf,
    pub transfer_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedTransferJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_transfer_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedPaymentV2JsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_payment_v2_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAssetTransactionJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_asset_transaction_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedEscrowTransactionJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_escrow_transaction_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedNftTransactionJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_nft_transaction_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedOfferTransactionJsonSubmitOptions {
    pub data_dir: PathBuf,
    pub signed_offer_transaction_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTransferOptions {
    pub data_dir: PathBuf,
    pub key_file: Option<PathBuf>,
    pub to: String,
    pub amount: u64,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MempoolBatchOptions {
    pub data_dir: PathBuf,
    pub batch_file: PathBuf,
    pub max_transactions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedAssetTransactionBatchOptions {
    pub data_dir: PathBuf,
    pub batch_file: PathBuf,
    pub signed_asset_transaction_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyBatchOptions {
    pub data_dir: PathBuf,
    pub batch_file: PathBuf,
    pub certificate_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchProposalOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub batch_kind: Option<String>,
    pub batch_file: PathBuf,
    pub proposal_file: PathBuf,
    pub view: Option<u64>,
    pub timeout_certificate_file: Option<PathBuf>,
    pub key_file: Option<PathBuf>,
    pub validator_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredBlockParent {
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReceiptQueryOptions {
    pub data_dir: PathBuf,
    pub tx_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TxFinalityQueryOptions {
    pub data_dir: PathBuf,
    pub tx_id: String,
    pub audit_block_log: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockQueryOptions {
    pub data_dir: PathBuf,
    pub from_height: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTxQueryOptions {
    pub data_dir: PathBuf,
    pub address: String,
    pub from_height: Option<u64>,
    pub to_height: Option<u64>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockVoteOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub key_file: PathBuf,
    pub validator_id: Option<String>,
    pub batch_file: Option<PathBuf>,
    pub proposal_file: Option<PathBuf>,
    pub timeout_certificate_file: Option<PathBuf>,
    pub block_height: Option<u64>,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockVoteForVerifiedProposalOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub key_file: PathBuf,
    pub validator_id: Option<String>,
    pub proposal: BlockProposalFile,
    pub block_height: Option<u64>,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCertificateOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub batch_file: Option<PathBuf>,
    pub proposal_file: Option<PathBuf>,
    pub timeout_certificate_file: Option<PathBuf>,
    pub block_height: Option<u64>,
    pub vote_files: Vec<PathBuf>,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockCertificateFromArchiveOptions {
    pub data_dir: PathBuf,
    pub block_file: PathBuf,
    pub batch_file: PathBuf,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTimeoutVoteOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub key_file: PathBuf,
    pub validator_id: Option<String>,
    pub block_height: u64,
    pub view: u64,
    pub high_qc_id: String,
    pub vote_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTimeoutCertificateOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub block_height: u64,
    pub view: u64,
    pub vote_files: Vec<PathBuf>,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockTimeoutCertificateVerifyOptions {
    pub data_dir: PathBuf,
    pub verify_block_log: bool,
    pub certificate_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockVoteEquivocationOptions {
    pub data_dir: PathBuf,
    pub first_proposal_file: PathBuf,
    pub second_proposal_file: PathBuf,
    pub first_vote_file: PathBuf,
    pub second_vote_file: PathBuf,
    pub evidence_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockProposalEquivocationOptions {
    pub data_dir: PathBuf,
    pub first_proposal_file: PathBuf,
    pub second_proposal_file: PathBuf,
    pub evidence_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchCertificateRoundOptions {
    pub data_dir: PathBuf,
    pub batch_kind: Option<String>,
    pub batch_file: PathBuf,
    pub validator_key_dir: PathBuf,
    pub vote_dir: PathBuf,
    pub proposal_file: PathBuf,
    pub certificate_file: PathBuf,
    pub block_height: Option<u64>,
    pub view: Option<u64>,
    pub timeout_certificate_file: Option<PathBuf>,
    pub skip_block_log_verify: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchCertificateRoundReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub batch_kind: String,
    pub batch_id: String,
    pub block_height: u64,
    pub view: u64,
    pub proposal_hash: String,
    pub certificate_id: String,
    pub validators: Vec<String>,
    pub vote_count: usize,
    pub proposal_file: String,
    pub certificate_file: String,
    pub vote_dir: String,
    pub vote_files: Vec<String>,
    pub private_key_policy: CertificateRoundPrivateKeyPolicy,
    pub round_ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CertificateRoundPrivateKeyPolicy {
    pub split_key_files: bool,
    pub private_key_material_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchArchiveQueryOptions {
    pub data_dir: PathBuf,
    pub batch_kind: Option<String>,
    pub batch_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockVerificationReport {
    pub verified: bool,
    pub block_count: usize,
    pub tip_hash: String,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxFinalityReport {
    pub schema: String,
    pub proof_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub tx_id: String,
    pub confirmed: bool,
    pub verification_mode: String,
    pub receipt: Receipt,
    pub receipt_index: u64,
    pub receipt_count: u64,
    pub block: BlockRecord,
    pub block_log_verified: bool,
    pub block_count: u64,
    pub tip_hash: String,
    pub tip_state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTxReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub address: String,
    pub from_height: Option<u64>,
    pub to_height: Option<u64>,
    pub scan_limit: u64,
    #[serde(default)]
    pub index_used: bool,
    pub scanned_block_count: u64,
    pub archive_lookup_count: u64,
    pub truncated: bool,
    pub row_count: u64,
    pub rows: Vec<AccountTxRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTxRow {
    pub tx_id: String,
    pub block_height: u64,
    pub batch_kind: String,
    pub batch_id: String,
    pub transaction_index: u64,
    #[serde(default)]
    pub transaction_kind: String,
    #[serde(rename = "from")]
    pub from_address: String,
    #[serde(rename = "to")]
    pub to_address: String,
    pub amount: u64,
    pub fee: u64,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memo_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trustline_authorized: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trustline_frozen: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_issuer_transfer_fee: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nft_collection_flags: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub escrow_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub counterparty_offer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill_index: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_hash: Option<String>,
    pub accepted: Option<bool>,
    pub receipt_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountTxIndexOptions {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTxIndex {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub indexed_from_height: Option<u64>,
    pub indexed_to_height: Option<u64>,
    pub indexed_block_count: u64,
    pub indexed_row_count: u64,
    pub account_count: u64,
    pub tip_hash: String,
    pub accounts: BTreeMap<String, Vec<AccountTxRow>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTxIndexBuildReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub index_path: String,
    #[serde(default)]
    pub disk_index_path: String,
    pub indexed_from_height: Option<u64>,
    pub indexed_to_height: Option<u64>,
    pub indexed_block_count: u64,
    pub indexed_row_count: u64,
    pub account_count: u64,
    pub tip_hash: String,
    pub index_usable: bool,
    #[serde(default)]
    pub disk_index_usable: bool,
    #[serde(default)]
    pub disk_account_shard_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountTxIndexStatusReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub index_path: String,
    #[serde(default)]
    pub disk_index_path: String,
    pub index_present: bool,
    pub index_usable: bool,
    pub reason: Option<String>,
    #[serde(default)]
    pub disk_index_present: bool,
    #[serde(default)]
    pub disk_index_usable: bool,
    #[serde(default)]
    pub disk_index_reason: Option<String>,
    pub indexed_from_height: Option<u64>,
    pub indexed_to_height: Option<u64>,
    pub indexed_block_count: u64,
    pub indexed_row_count: u64,
    pub account_count: u64,
    #[serde(default)]
    pub disk_account_shard_count: u64,
    pub tip_hash: String,
    pub current_tip_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockVoteFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    #[serde(default)]
    pub view: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_hash: Option<String>,
    pub vote: BlockCertificateVote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockProposalFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    #[serde(default)]
    pub view: u64,
    pub parent_hash: String,
    #[serde(default)]
    pub proposer: String,
    pub batch_kind: String,
    pub batch_id: String,
    pub payload_hash: String,
    pub state_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_exit_root: Option<String>,
    pub receipt_count: u64,
    pub receipt_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_pre_state_effects: Vec<postfiat_types::FastPayVersionFenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<BlockProposalSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockProposalSignature {
    pub signer: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockCertificateFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    #[serde(default)]
    pub view: u64,
    #[serde(default)]
    pub proposer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proposal_hash: Option<String>,
    pub certificate_id: String,
    pub certificate: BlockCertificate,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_pre_state_effects: Vec<postfiat_types::FastPayVersionFenceV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_v2_commit: Option<postfiat_types::ConsensusV2Commit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTimeoutVote {
    pub vote_id: String,
    pub validator: String,
    pub high_qc_id: String,
    pub algorithm_id: String,
    #[serde(default)]
    pub registry_root: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTimeoutVoteFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub view: u64,
    pub vote: BlockTimeoutVote,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_v2_vote: Option<postfiat_types::ConsensusV2TimeoutVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTimeoutCertificate {
    pub validators: Vec<String>,
    pub quorum: usize,
    #[serde(default)]
    pub registry_root: String,
    pub high_qc_id: String,
    pub votes: Vec<BlockTimeoutVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockTimeoutCertificateFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_height: u64,
    pub view: u64,
    pub hotstuff_certificate_id: String,
    pub certificate_id: String,
    pub certificate: BlockTimeoutCertificate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consensus_v2_certificate: Option<postfiat_types::ConsensusV2TimeoutCertificate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockEquivocationEvidenceFile {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub kind: String,
    pub block_height: u64,
    pub view: u64,
    pub validator: String,
    pub first_evidence_kind: String,
    pub second_evidence_kind: String,
    pub first_evidence_id: String,
    pub second_evidence_id: String,
    pub first_target_kind: String,
    pub second_target_kind: String,
    pub first_target_hash: String,
    pub second_target_hash: String,
    pub evidence_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceVerificationReport {
    pub verified: bool,
    pub cobalt_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_graph_root: Option<String>,
    pub active_validator_count: u32,
    pub active_validators: Vec<String>,
    pub crypto_policy_version: u32,
    pub bridge_witness_epoch: u32,
    pub authority_mode: u32,
    pub amendment_count: usize,
    pub latest_amendment_id: String,
    pub amendment_activation_record_count: usize,
    pub latest_amendment_activation_record_id: String,
    pub amendment_supersession_record_count: usize,
    pub latest_amendment_supersession_record_id: String,
    pub amendment_rollback_record_count: usize,
    pub latest_amendment_rollback_record_id: String,
    pub validator_registry_update_count: usize,
    pub latest_validator_registry_update_id: String,
    pub governance_agent_dry_run_count: usize,
    pub latest_governance_agent_dry_run_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolVerificationReport {
    pub verified: bool,
    pub pending_count: usize,
    pub sender_count: usize,
    pub total_amount: u64,
    pub total_fee: u64,
    pub latest_tx_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BridgeVerificationReport {
    pub verified: bool,
    pub domain_count: usize,
    pub transfer_count: usize,
    pub attestation_count: usize,
    pub replay_cache_count: usize,
    pub inbound_used: u64,
    pub outbound_used: u64,
    pub latest_transfer_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShieldedVerificationReport {
    pub verified: bool,
    pub note_count: usize,
    pub nullifier_count: usize,
    pub turnstile_event_count: usize,
    pub orchard_pool_id: String,
    pub orchard_nullifier_count: usize,
    pub orchard_output_count: usize,
    pub orchard_anchor_count: usize,
    pub orchard_root_count: usize,
    pub orchard_latest_root: String,
    pub orchard_value_balance_total: i64,
    pub orchard_turnstile_deposit_total: u64,
    pub orchard_fee_burn_total: u64,
    pub orchard_withdraw_total: u64,
    pub tree_root: String,
    pub bootstrap_deposit_total: u64,
    pub migration_total: u64,
    pub orchard_deposit_total: u64,
    pub spent_note_count: usize,
    pub live_note_count: usize,
    pub latest_turnstile_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateVerificationReport {
    pub schema: String,
    pub verified: bool,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub block_log: BlockVerificationReport,
    pub governance: GovernanceVerificationReport,
    pub bridge: BridgeVerificationReport,
    pub shielded: ShieldedVerificationReport,
    pub mempool: MempoolVerificationReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatifyValidatorSetOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub validator_count: u32,
    pub activation_height: u64,
    pub veto_until_height: u64,
    pub paused: bool,
    pub amendment_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatifyGovernanceOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub kind: String,
    pub value: u32,
    pub activation_height: u64,
    pub veto_until_height: u64,
    pub paused: bool,
    pub amendment_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAuthorizationSignOptions {
    pub data_dir: PathBuf,
    pub amendment_file: PathBuf,
    pub validator: String,
    pub validator_key_file: PathBuf,
    pub proposal_slot: u64,
    pub expires_at_height: u64,
    pub authorization_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAmendmentAssembleOptions {
    pub data_dir: PathBuf,
    pub amendment_file: PathBuf,
    pub authorization_files: Vec<PathBuf>,
    pub proposal_slot: u64,
    pub output_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyAmendmentOptions {
    pub data_dir: PathBuf,
    pub amendment_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceBatchOptions {
    pub data_dir: PathBuf,
    pub amendment_file: Option<PathBuf>,
    pub registry_update_file: Option<PathBuf>,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastSwapGovernanceBootstrapOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub activation_height: u64,
    pub veto_until_height: u64,
    pub paused: bool,
    pub payload_file: PathBuf,
    pub amendment_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedFastSwapGovernanceBootstrapOptions {
    pub data_dir: PathBuf,
    pub payload_file: PathBuf,
    pub signed_amendment_file: PathBuf,
    pub proposal_slot: u64,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastPayRecoveryGovernanceBootstrapOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub veto_until_height: u64,
    pub payload_file: PathBuf,
    pub amendment_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedFastPayRecoveryGovernanceBootstrapOptions {
    pub data_dir: PathBuf,
    pub payload_file: PathBuf,
    pub signed_amendment_file: PathBuf,
    pub proposal_slot: u64,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultBridgeRouteProfileGovernanceOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub veto_until_height: u64,
    pub profile_file: PathBuf,
    pub tier4_finality_bootstrap_file: Option<PathBuf>,
    pub amendment_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedVaultBridgeRouteProfileGovernanceOptions {
    pub data_dir: PathBuf,
    pub profile_file: PathBuf,
    pub tier4_finality_bootstrap_file: Option<PathBuf>,
    pub signed_amendment_file: PathBuf,
    pub proposal_slot: u64,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryRootOptions {
    pub data_dir: PathBuf,
    pub registry_file: Option<PathBuf>,
    pub validators: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryRootReport {
    pub schema: String,
    pub validators: Vec<String>,
    pub validator_count: usize,
    pub registry_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryUpdateOptions {
    pub data_dir: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub activation_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_validators: Vec<String>,
    pub new_validators: Vec<String>,
    pub operation: String,
    pub subject_node_id: String,
    pub previous_record_file: Option<PathBuf>,
    pub new_record_file: Option<PathBuf>,
    pub update_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryAuthorizationSignOptions {
    pub data_dir: PathBuf,
    pub update_file: PathBuf,
    pub validator: String,
    pub validator_key_file: PathBuf,
    pub proposal_slot: u64,
    pub expires_at_height: u64,
    pub authorization_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryUpdateAssembleOptions {
    pub data_dir: PathBuf,
    pub update_file: PathBuf,
    pub authorization_files: Vec<PathBuf>,
    pub proposal_slot: u64,
    pub output_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryUpdateVerifyOptions {
    pub data_dir: PathBuf,
    pub update_file: PathBuf,
    pub previous_registry_file: Option<PathBuf>,
    pub new_registry_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryUpdateVerificationReport {
    pub schema: String,
    pub verified: bool,
    pub update_id: String,
    pub operation: String,
    pub subject_node_id: String,
    pub activation_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_registry_root_verified: bool,
    pub new_registry_root_verified: bool,
    pub previous_validator_count: usize,
    pub new_validator_count: usize,
    pub support_count: usize,
    pub vote_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryUpdateApplyOptions {
    pub data_dir: PathBuf,
    pub update_file: PathBuf,
    pub current_height: u64,
    pub previous_registry_file: Option<PathBuf>,
    pub output_registry_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryUpdateApplyReport {
    pub schema: String,
    pub applied: bool,
    pub update_id: String,
    pub operation: String,
    pub subject_node_id: String,
    pub activation_height: u64,
    pub current_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_validator_count: usize,
    pub new_validator_count: usize,
    pub output_registry_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorRegistryLifecycleReplayVerifyOptions {
    pub bundle_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryLifecycleReplayBundle {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub initial_registry: ValidatorRegistry,
    pub initial_validators: Vec<String>,
    pub ordered_updates: Vec<ValidatorRegistryUpdateRecord>,
    pub final_validators: Vec<String>,
    pub final_registry_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryLifecycleReplayVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub bundle_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub initial_validator_count: usize,
    pub final_validator_count: usize,
    pub initial_registry_root: String,
    pub final_registry_root: String,
    pub update_count: usize,
    pub latest_update_id: String,
    pub operations: Vec<String>,
    pub update_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceReplayVerifyOptions {
    pub data_dir: PathBuf,
    pub package_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceReplayBuildOptions {
    pub data_dir: PathBuf,
    pub genesis_bundle_file: Option<PathBuf>,
    pub previous_registry_file: PathBuf,
    pub update_file: PathBuf,
    pub new_registry_file: PathBuf,
    pub amendment_replay_bundle_file: Option<PathBuf>,
    pub governance_batch_file: Option<PathBuf>,
    pub post_change_block_file: Option<PathBuf>,
    pub post_change_batch_file: Option<PathBuf>,
    pub post_change_certificate_file: Option<PathBuf>,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceReplayPackage {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chain_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub genesis_bundle_file: Option<String>,
    pub previous_registry_file: String,
    pub update_file: String,
    pub new_registry_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amendment_replay_bundle_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub governance_batch_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_change_block_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_change_batch_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post_change_certificate_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_update_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_batch_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceReplayVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub package_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub update_id: String,
    pub operation: String,
    pub subject_node_id: String,
    pub activation_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_registry_root_verified: bool,
    pub new_registry_root_verified: bool,
    pub previous_validator_count: usize,
    pub new_validator_count: usize,
    pub support_count: usize,
    pub vote_count: usize,
    pub governance_genesis_bundle_verified: bool,
    pub governance_genesis_bundle_hash: Option<String>,
    pub governance_genesis_registry_root: Option<String>,
    pub governance_genesis_operator_manifest_count: Option<usize>,
    pub governance_batch_verified: bool,
    pub governance_batch_id: String,
    pub governance_batch_contains_update: bool,
    pub amendment_replay_verified: bool,
    pub amendment_replay_bundle_file: Option<String>,
    pub amendment_replay_amendment_count: Option<usize>,
    pub amendment_replay_activation_record_count: Option<usize>,
    pub amendment_replay_supersession_record_count: Option<usize>,
    pub amendment_replay_rollback_record_count: Option<usize>,
    pub post_change_certificate_verified: bool,
    pub post_change_block_height: Option<u64>,
    pub post_change_certificate_id: Option<String>,
    pub post_change_certificate_registry_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAmendmentReplayVerifyOptions {
    pub bundle_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentReplayFinalGovernance {
    pub active_validator_count: u32,
    pub crypto_policy_version: u32,
    pub bridge_witness_epoch: u32,
    #[serde(default)]
    pub authority_mode: u32,
    pub amendment_count: usize,
    pub latest_amendment_id: String,
    pub amendment_activation_record_count: usize,
    pub latest_amendment_activation_record_id: String,
    pub amendment_supersession_record_count: usize,
    pub latest_amendment_supersession_record_id: String,
    pub amendment_rollback_record_count: usize,
    pub latest_amendment_rollback_record_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentReplayBundle {
    pub schema: String,
    pub ordered_amendment_count: usize,
    pub ordered_activation_record_count: usize,
    pub ordered_supersession_record_count: usize,
    pub ordered_rollback_record_count: usize,
    pub ordered_amendments: Vec<GovernanceAmendment>,
    pub ordered_activation_records: Vec<GovernanceAmendmentActivationRecord>,
    pub ordered_supersession_records: Vec<GovernanceAmendmentSupersessionRecord>,
    pub ordered_rollback_records: Vec<GovernanceAmendmentRollbackRecord>,
    pub final_governance: GovernanceAmendmentReplayFinalGovernance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAmendmentReplayVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub bundle_file: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub active_validator_count: u32,
    pub crypto_policy_version: u32,
    pub bridge_witness_epoch: u32,
    pub authority_mode: u32,
    pub amendment_count: usize,
    pub latest_amendment_id: String,
    pub activation_record_count: usize,
    pub latest_activation_record_id: String,
    pub supersession_record_count: usize,
    pub latest_supersession_record_id: String,
    pub rollback_record_count: usize,
    pub latest_rollback_record_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorManifestCreateOptions {
    pub master_key_file: PathBuf,
    pub chain_id: String,
    pub network: String,
    pub validator_id: String,
    pub hot_public_key_hex: String,
    pub operator: String,
    pub contact: String,
    pub provider_group: String,
    pub region_group: String,
    pub jurisdiction_group: String,
    pub legal_domain_group: String,
    pub funding_domain_group: String,
    pub rotation_state: String,
    pub effective_height: u64,
    pub trust_graph_root: Option<String>,
    pub trust_graph_version: Option<u64>,
    pub trust_view_id: Option<String>,
    pub trust_view_version: Option<u64>,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorManifestVerifyOptions {
    pub manifest_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceGenesisBundleOptions {
    pub data_dir: PathBuf,
    pub manifest_dir: PathBuf,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub network: String,
    pub output_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceGenesisVerifyOptions {
    pub data_dir: PathBuf,
    pub bundle_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorInfrastructureLabels {
    pub provider_group: String,
    pub region_group: String,
    pub jurisdiction_group: String,
    pub legal_domain_group: String,
    pub funding_domain_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorCobaltTrustBinding {
    pub trust_graph_root: String,
    pub trust_graph_version: u64,
    pub trust_view_id: String,
    pub trust_view_version: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorManifest {
    pub schema: String,
    pub chain_id: String,
    pub network: String,
    pub validator_id: String,
    pub master_public_key_hex: String,
    pub hot_public_key_hex: String,
    pub algorithm_id: String,
    pub key_role: String,
    pub operator: String,
    pub contact: String,
    pub infrastructure: OperatorInfrastructureLabels,
    pub rotation_state: String,
    pub effective_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobalt_trust: Option<OperatorCobaltTrustBinding>,
    pub manifest_signing_key_hex: String,
    pub signature_hex: String,
    pub manifest_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperatorManifestVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub manifest_file: String,
    pub manifest_hash: String,
    pub chain_id: String,
    pub network: String,
    pub validator_id: String,
    pub algorithm_id: String,
    pub key_role: String,
    pub rotation_state: String,
    pub effective_height: u64,
    pub hot_public_key_hex: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobalt_trust: Option<OperatorCobaltTrustBinding>,
    pub manifest_signer_matches_master: bool,
    pub signature_verified: bool,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceGenesisOperatorManifestRef {
    pub validator_id: String,
    pub manifest_file: String,
    pub manifest_hash: String,
    pub hot_public_key_hex: String,
    pub provider_group: String,
    pub region_group: String,
    pub jurisdiction_group: String,
    pub legal_domain_group: String,
    pub funding_domain_group: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cobalt_trust: Option<OperatorCobaltTrustBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceGenesisBundle {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub network: String,
    pub validators: Vec<String>,
    pub validator_count: usize,
    pub quorum: usize,
    pub registry_root: String,
    pub registry_records: Vec<ValidatorRegistryRecord>,
    pub operator_manifests: Vec<GovernanceGenesisOperatorManifestRef>,
    pub bundle_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceGenesisVerifyReport {
    pub schema: String,
    pub verified: bool,
    pub bundle_file: String,
    pub bundle_hash: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub network: String,
    pub validators: Vec<String>,
    pub validator_count: usize,
    pub quorum: usize,
    pub registry_root: String,
    pub operator_manifest_count: usize,
    pub operator_manifests_verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldMintOptions {
    pub data_dir: PathBuf,
    pub owner: String,
    pub asset_id: String,
    pub amount: u64,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldSpendOptions {
    pub data_dir: PathBuf,
    pub note_id: String,
    pub to: String,
    pub amount: u64,
    pub memo: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldMintBatchOptions {
    pub data_dir: PathBuf,
    pub owner: String,
    pub asset_id: String,
    pub amount: u64,
    pub memo: String,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldSpendBatchOptions {
    pub data_dir: PathBuf,
    pub note_id: String,
    pub to: String,
    pub amount: u64,
    pub memo: String,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardActionOptions {
    pub data_dir: PathBuf,
    pub action_file: PathBuf,
    pub apply: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardActionBatchOptions {
    pub data_dir: PathBuf,
    pub action_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardOperatorPolicyOptions {
    pub data_dir: PathBuf,
    pub privacy_enabled: bool,
    pub max_concurrent_verifiers: usize,
    pub verifier_timeout_ms: u64,
    pub root_retention_roots: u64,
    pub indexing_role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardFeeResourcePolicyOptions {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardPoolReportOptions {
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardFrontierCacheWarmReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub data_dir: String,
    pub pool_initialized: bool,
    pub pool_id: String,
    pub output_count: u64,
    pub retained_root_count: u64,
    pub root: String,
    pub cache_present_before: bool,
    pub cache_present_after: bool,
    pub cache_written: bool,
    pub state_root_before: String,
    pub state_root_after: String,
    pub state_root_unchanged: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardActionReport {
    pub verified: bool,
    pub applied: bool,
    pub pool_id: String,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub value_balance: i64,
    pub fee: u64,
    pub receipt: Receipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardProtocolLimitsReport {
    pub max_action_json_bytes: u64,
    pub max_actions_per_orchard_bundle: usize,
    pub max_proof_bytes: usize,
    pub max_ciphertext_blob_bytes: usize,
    pub enc_ciphertext_bytes: usize,
    pub out_ciphertext_bytes: usize,
    pub compact_ciphertext_bytes: usize,
    pub epk_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardOperatorEnforcementReport {
    pub protocol_size_bounds_enforced: bool,
    pub action_count_bound_enforced: bool,
    pub verifier_runs_in_process: bool,
    pub verifier_timeout_enforced_in_process: bool,
    pub verifier_concurrency_enforced_in_process: bool,
    pub rpc_child_timeout_available_for_remote_batch_create: bool,
    pub remote_batch_create_requires_action_json: bool,
    pub remote_batch_create_uses_server_controlled_spool: bool,
    pub remote_batch_create_rate_limited: bool,
    pub remote_batch_create_concurrency_limited: bool,
    pub public_write_edge_allowed: bool,
    pub requires_worker_isolation_for_public_write_edge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardOperatorPolicyReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub privacy_enabled: bool,
    pub indexing_role: String,
    pub max_concurrent_verifiers: usize,
    pub verifier_timeout_ms: u64,
    pub root_retention_roots: u64,
    pub protocol_limits: OrchardProtocolLimitsReport,
    pub enforcement: OrchardOperatorEnforcementReport,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardTransparentFeeScheduleReport {
    pub min_transfer_fee: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub transfer_account_creation_fee: u64,
    pub account_reserve: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardResourceFeeScheduleReport {
    pub minimum_orchard_resource_fee: u64,
    pub orchard_fee_byte_quantum: u64,
    pub orchard_fee_per_quantum: u64,
    pub resource_weight_formula: String,
    pub fee_formula: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardFlowFeeScheduleReport {
    pub operation: String,
    pub fee_payer: String,
    pub minimum_fee_components: Vec<String>,
    pub burn_accounting: Vec<String>,
    pub receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardResourceBoundsReport {
    pub max_action_json_bytes: u64,
    pub max_actions_per_orchard_bundle: usize,
    pub max_proof_bytes: usize,
    pub max_ciphertext_blob_bytes: usize,
    pub enc_ciphertext_bytes: usize,
    pub out_ciphertext_bytes: usize,
    pub compact_ciphertext_bytes: usize,
    pub epk_bytes: usize,
    pub default_root_retention_roots: u64,
    pub default_verifier_timeout_ms: u64,
    pub default_verifier_max_concurrency: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardAntiSpamPolicyReport {
    pub protocol_size_bounds_enforced: bool,
    pub action_count_bound_enforced: bool,
    pub minimum_fee_enforced_for_positive_value_balance: bool,
    pub direct_deposit_resource_fee_enforced: bool,
    pub withdraw_state_expansion_fee_enforced: bool,
    pub public_write_edge_allowed: bool,
    pub remote_batch_create_rate_limited: bool,
    pub remote_batch_create_concurrency_limited: bool,
    pub requires_worker_isolation_for_public_write_edge: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardFeeResourcePolicyChecks {
    pub schema: bool,
    pub nonzero_minimum_orchard_fee: bool,
    pub nonzero_orchard_fee_quantum: bool,
    pub nonzero_orchard_fee_per_quantum: bool,
    pub transparent_fee_schedule_visible: bool,
    pub protocol_bounds_visible: bool,
    pub public_write_edge_closed: bool,
    pub positive_value_balance_fee_required: bool,
    pub direct_deposit_resource_fee_required: bool,
    pub withdraw_state_expansion_fee_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardFeeResourcePolicyReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
    pub transparent_fee_schedule: OrchardTransparentFeeScheduleReport,
    pub orchard_resource_fee_schedule: OrchardResourceFeeScheduleReport,
    pub flow_fee_schedule: Vec<OrchardFlowFeeScheduleReport>,
    pub resource_bounds: OrchardResourceBoundsReport,
    pub anti_spam_policy: OrchardAntiSpamPolicyReport,
    pub checks: OrchardFeeResourcePolicyChecks,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolCountersReport {
    pub pool_initialized: bool,
    pub output_count: u64,
    pub nullifier_count: u64,
    pub retained_root_count: u64,
    pub accepted_anchor_count: u64,
    pub latest_retained_root: String,
    pub turnstile_event_count: u64,
    pub legacy_migration_total: u64,
    pub direct_deposit_total: u64,
    pub accounted_pool_deposit_total: u64,
    pub fee_burn_total: u64,
    pub withdraw_total: u64,
    pub value_balance_total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolActiveNoteBoundsReport {
    pub exact_active_note_count_publicly_available: bool,
    pub conservative_public_floor: u64,
    pub public_upper_bound: u64,
    pub method: String,
    pub limitation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolPrivacyClaimReport {
    pub safe_claim: String,
    pub not_claimed: Vec<String>,
    pub public_fields: Vec<String>,
    pub omitted_private_material: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolReportChecks {
    pub schema: bool,
    pub state_verified: bool,
    pub turnstile_accounting_verified: bool,
    pub no_private_material_fields: bool,
    pub exact_active_note_count_not_claimed: bool,
    pub pool_id_visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardPoolReport {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
    pub counters: OrchardPoolCountersReport,
    pub active_note_bounds: OrchardPoolActiveNoteBoundsReport,
    pub privacy_claim: OrchardPoolPrivacyClaimReport,
    pub checks: OrchardPoolReportChecks,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWalletKeyFile {
    pub schema: String,
    pub kdf: String,
    pub derivation_domain: String,
    pub account_index: u32,
    pub spending_key_hex: String,
    pub address_raw_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardViewKeyFile {
    pub schema: String,
    pub source_schema: String,
    pub account_index: u32,
    pub full_viewing_key_hex: String,
    pub address_raw_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWalletKeyReport {
    pub schema: String,
    pub key_file: String,
    pub account_index: u32,
    pub address_raw_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardViewKeyReport {
    pub schema: String,
    pub view_key_file: String,
    pub account_index: u32,
    pub address_raw_hex: String,
    pub spend_authority_exported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardOutputActionReport {
    pub schema: String,
    pub action_file: String,
    pub pool_id: String,
    pub anchor: String,
    pub recipient_address_raw_hex: String,
    pub value: u64,
    pub fee: u64,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardTestVectorEncryptedOutputReport {
    pub epk_bytes: usize,
    pub enc_ciphertext_bytes: usize,
    pub out_ciphertext_bytes: usize,
    pub compact_ciphertext_bytes: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardTestVectorTamperReport {
    pub proof_error_code: String,
    pub fee_error_code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardTestVectorReport {
    pub schema: String,
    pub fixture_warning: String,
    pub chain_id: String,
    pub validator_count: u32,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
    pub proof_system_id: String,
    pub circuit_id: String,
    pub recipient_address_raw_hex: String,
    pub empty_anchor: String,
    pub external_binding_hash: Option<String>,
    pub value: u64,
    pub fee: u64,
    pub action_hash: String,
    pub action_json_bytes: usize,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub proof_bytes: usize,
    pub value_balance: i64,
    pub authorizing_sighash_hex: String,
    pub root_after_outputs: String,
    pub nullifiers: Vec<String>,
    pub output_commitments: Vec<String>,
    pub encrypted_outputs: Vec<OrchardTestVectorEncryptedOutputReport>,
    pub tamper: OrchardTestVectorTamperReport,
    pub deterministic_rebuild_matches: bool,
    pub private_key_material_redacted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardSpendActionOptions {
    pub data_dir: PathBuf,
    pub spending_key_hex: Option<String>,
    pub key_file: Option<PathBuf>,
    pub input_output_index: usize,
    pub amount: Option<u64>,
    pub recipient_address_raw_hex: Option<String>,
    pub recipient_key_file: Option<PathBuf>,
    pub recipient_view_key_file: Option<PathBuf>,
    pub change_address_raw_hex: Option<String>,
    pub change_key_file: Option<PathBuf>,
    pub change_view_key_file: Option<PathBuf>,
    pub memo_hex: Option<String>,
    pub fee: u64,
    pub action_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardSpendActionReport {
    pub schema: String,
    pub action_file: String,
    pub pool_id: String,
    pub anchor: String,
    pub input_output_index: usize,
    pub input_nullifier: String,
    pub recipient_address_raw_hex: String,
    pub input_value: u64,
    pub output_value: u64,
    pub recipient_value: u64,
    pub change_value: u64,
    pub change_address_raw_hex: String,
    pub fee: u64,
    pub minimum_fee: u64,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub value_balance: i64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardWithdrawActionOptions {
    pub data_dir: PathBuf,
    pub spending_key_hex: Option<String>,
    pub key_file: Option<PathBuf>,
    pub input_output_index: usize,
    pub to: String,
    pub amount: u64,
    pub change_address_raw_hex: Option<String>,
    pub change_key_file: Option<PathBuf>,
    pub change_view_key_file: Option<PathBuf>,
    pub memo_hex: Option<String>,
    pub fee: u64,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
    pub action_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardWithdrawActionBatchOptions {
    pub data_dir: PathBuf,
    pub action_file: PathBuf,
    pub to: String,
    pub amount: u64,
    pub fee: u64,
    pub policy_id: Option<String>,
    pub disclosure_hash: Option<String>,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldedSwapActionBatchOptions {
    pub data_dir: PathBuf,
    pub swap_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWithdrawActionReport {
    pub schema: String,
    pub action_file: String,
    pub pool_id: String,
    pub anchor: String,
    pub input_output_index: usize,
    pub input_nullifier: String,
    pub input_value: u64,
    pub withdraw_amount: u64,
    pub change_value: u64,
    pub change_address_raw_hex: String,
    pub to: String,
    pub fee: u64,
    pub minimum_fee: u64,
    pub state_expansion_fee: u64,
    pub external_binding_hash: String,
    pub policy_id: String,
    pub disclosure_hash: String,
    pub action_count: usize,
    pub nullifier_count: usize,
    pub output_count: usize,
    pub value_balance: i64,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardWalletScanOptions {
    pub data_dir: PathBuf,
    pub spending_key_hex: Option<String>,
    pub key_file: Option<PathBuf>,
    pub view_key_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWalletScanReport {
    pub schema: String,
    pub pool_id: String,
    pub address_raw_hex: String,
    pub latest_retained_root: String,
    pub latest_retained_output_count: u64,
    pub output_count: usize,
    pub decrypted_count: usize,
    pub spent_count: usize,
    pub outputs: Vec<OrchardWalletDecryptedOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardWalletDecryptedOutput {
    pub output_index: usize,
    pub merkle_position: u32,
    pub commitment: String,
    pub nullifier: String,
    pub rho: String,
    pub rseed: String,
    pub value: u64,
    pub spent: bool,
    pub witness_anchor: String,
    pub witness_output_count: u64,
    pub witness_auth_path: Vec<String>,
    pub address_raw_hex: String,
    pub memo_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardDisclosureOptions {
    pub data_dir: PathBuf,
    pub spending_key_hex: Option<String>,
    pub key_file: Option<PathBuf>,
    pub view_key_file: Option<PathBuf>,
    pub output_index: usize,
    pub packet_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrchardDisclosureVerifyOptions {
    pub data_dir: PathBuf,
    pub packet_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrchardDisclosureVerifyReport {
    pub schema: String,
    pub packet_file: String,
    pub disclosure_hash: String,
    pub chain_id: String,
    pub pool_id: String,
    pub output_index: usize,
    pub commitment: String,
    pub nullifier: String,
    pub packet_hash_verified: bool,
    pub local_context_verified: bool,
    pub finality_verified: bool,
    pub verified: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchardDisclosureFinality {
    pub batch_kind: String,
    pub batch_id: String,
    pub batch_payload_hash: String,
    pub block_height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub certificate_id: String,
    pub receipt_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrchardDisclosurePacket {
    pub schema: String,
    pub disclosure_hash: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub pool_id: String,
    pub address_raw_hex: String,
    pub output_index: usize,
    pub merkle_position: u32,
    pub commitment: String,
    pub nullifier: String,
    pub value: u64,
    pub spent: bool,
    pub memo_hex: String,
    pub witness_anchor: String,
    pub witness_output_count: u64,
    pub latest_retained_root: String,
    pub latest_retained_output_count: u64,
    pub finality: Option<OrchardDisclosureFinality>,
    pub private_witness_redacted: bool,
    pub auditor_instructions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShieldMigrateBatchOptions {
    pub data_dir: PathBuf,
    pub note_id: String,
    pub target_pool: String,
    pub memo: String,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeDomainOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub name: String,
    pub source_chain: String,
    pub target_chain: String,
    pub bridge_id: String,
    pub door_account: String,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeTransferOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: Option<u32>,
    pub witness_signer: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgePauseOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub paused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeDomainBatchOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub name: String,
    pub source_chain: String,
    pub target_chain: String,
    pub bridge_id: String,
    pub door_account: String,
    pub inbound_cap: u64,
    pub outbound_cap: u64,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeTransferBatchOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub direction: String,
    pub from: String,
    pub to: String,
    pub asset_id: String,
    pub amount: u64,
    pub witness_id: String,
    pub witness_epoch: Option<u32>,
    pub witness_signer: String,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgePauseBatchOptions {
    pub data_dir: PathBuf,
    pub domain_id: String,
    pub paused: bool,
    pub batch_file: PathBuf,
}

include!("node_types_snapshot_deployment.rs");
