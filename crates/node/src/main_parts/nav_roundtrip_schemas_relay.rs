const CERTIFIED_ASSET_OPS_REQUEST_SCHEMA: &str = "postfiat-certified-asset-ops-request-v1";
const CERTIFIED_ASSET_OPS_REPORT_SCHEMA: &str = "postfiat-certified-asset-ops-report-v1";
const CERTIFIED_ASSET_OPS_FROM_BUNDLE_REPORT_SCHEMA: &str =
    "postfiat-certified-asset-ops-from-bundle-report-v1";
const NAV_ROUNDTRIP_PREFLIGHT_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-preflight-report-v1";
const NAV_ROUNDTRIP_FLEET_PREFLIGHT_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-fleet-preflight-report-v1";
const NAV_ROUNDTRIP_USDC_ALLOWANCE_SETUP_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-usdc-allowance-setup-report-v1";
const NAV_ROUNDTRIP_EVM_DEPOSIT_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-evm-deposit-report-v1";
const NAV_ROUNDTRIP_DEPOSIT_RELAY_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-deposit-relay-report-v1";
const NAV_ROUNDTRIP_PRIMARY_MINT_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-primary-mint-report-v1";
const NAV_ROUNDTRIP_NAV_EXIT_REPORT_SCHEMA: &str = "postfiat-nav-roundtrip-nav-exit-report-v1";
const NAV_ROUNDTRIP_BURN_TO_REDEEM_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-burn-to-redeem-report-v1";
const NAV_ROUNDTRIP_EVM_WITHDRAWAL_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-evm-withdrawal-report-v1";
const NAV_ROUNDTRIP_PFTL_SETTLE_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-pftl-settle-report-v1";
const NAV_ROUNDTRIP_NAV_CHECKPOINT_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-nav-checkpoint-report-v1";
const NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-live-demo-report-v1";
const NAV_ROUNDTRIP_PFTL_ONLY_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-pftl-only-report-v1";
const NAV_ROUNDTRIP_PFTL_ONLY_BRIDGE_OUT_RESUME_SCHEMA: &str =
    "postfiat-nav-roundtrip-pftl-only-bridge-out-resume-v1";
const NAV_ROUNDTRIP_DASHBOARD_STATUS_SCHEMA: &str =
    "postfiat-nav-roundtrip-dashboard-status-v1";
const NAV_ROUNDTRIP_BENCHMARK_VERIFY_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-benchmark-verify-report-v1";
const NAV_ROUNDTRIP_BENCHMARK_PLAN_SCHEMA: &str =
    "postfiat-nav-roundtrip-benchmark-plan-v1";
const NAV_ROUNDTRIP_BENCHMARK_BASE_ARGS_SCHEMA: &str =
    "postfiat-nav-roundtrip-benchmark-base-args-v1";
const NAV_ROUNDTRIP_REPLAY_CORPUS_VERIFY_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-replay-corpus-verify-report-v1";
const CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA: &str =
    "postfiat-certified-asset-ops-batch-equivalence-corpus-v1";
const NAV_ROUNDTRIP_BACKGROUND_AUDIT_REQUEST_SCHEMA: &str =
    "postfiat-nav-roundtrip-background-audit-request-v1";
const NAV_ROUNDTRIP_FAILURE_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-failure-report-v1";
const NAV_ROUNDTRIP_WITHDRAWAL_AUTO_SIGNATURE_REPORT_SCHEMA: &str =
    "postfiat-nav-roundtrip-withdrawal-auto-signature-report-v1";
const NAV_ROUNDTRIP_BRIDGE_CLASS_CONTROLLED_LAUNCH: &str =
    "controlled_launch_existing_contracts";
const NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED: &str = "fixed_contracts_redeployed";
const NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED: &str =
    "fixed_contracts_redeployed_consolidated";
const NAV_ROUNDTRIP_BRIDGE_CLASS_UNKNOWN: &str = "unknown";
const NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP: &str = "full-arbitrum-roundtrip";
const NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY: &str = "pftl-only";
const NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP: &str =
    "full_arbitrum_roundtrip_complete";
const NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED: &str =
    "on_pftl_complete_bridge_out_deferred";
const NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC: &str = "arbitrum_wallet_usdc";
const NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE: &str =
    "pftl_settlement_asset_balance";
const NAV_ROUNDTRIP_PHASE2_DEFAULT_CANDIDATE_CLASSES: [&str; 4] = [
    "vault_bridge_deposit_propose_attest",
    "vault_bridge_receipt_submit_count",
    "nav_subscription_allocate_mint_at_nav",
    "nav_redeem_at_nav_settle",
];

const NAV_ROUNDTRIP_FIXED_WITHDRAWAL_DIGEST_SIGNATURE: &str =
    "withdrawalPacketDigest((uint64,uint256,address,address,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes))(bytes32)";
const NAV_ROUNDTRIP_OLD_WITHDRAWAL_DIGEST_SIGNATURE: &str =
    "withdrawalPacketDigest((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes))(bytes32)";
const NAV_ROUNDTRIP_FIXED_SUBMIT_WITHDRAWAL_SIGNATURE: &str =
    "submitWithdrawal((uint64,uint256,address,address,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)";
const NAV_ROUNDTRIP_OLD_SUBMIT_WITHDRAWAL_SIGNATURE: &str =
    "submitWithdrawal((uint64,bytes,bytes,bytes,address,uint256,bytes,bytes,uint64,bytes),bytes)";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsRequest {
    #[serde(default)]
    schema: Option<String>,
    operations: Vec<CertifiedAssetOpRequest>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpRequest {
    label: String,
    source: String,
    key_file: std::path::PathBuf,
    operation: postfiat_types::AssetTransactionOperation,
    #[serde(default)]
    dependencies: Vec<CertifiedAssetOpDependency>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpDependency {
    label: String,
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone)]
struct CertifiedAssetOpsBatchOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    ops_file: std::path::PathBuf,
    artifact_dir: std::path::PathBuf,
    max_transactions: Option<usize>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct CertifiedAssetOpsFromBundleOptions {
    bundle_dir: std::path::PathBuf,
    output_file: std::path::PathBuf,
    proposer_key_file: Option<std::path::PathBuf>,
    attestor_key_file: Option<std::path::PathBuf>,
    finalizer_key_file: Option<std::path::PathBuf>,
    claimer_key_file: Option<std::path::PathBuf>,
    owner_key_file: Option<std::path::PathBuf>,
    include_deposit_claim: bool,
    overwrite: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripPreflightOptions {
    data_dir: std::path::PathBuf,
    artifact_dir: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    amount_atoms: u64,
    min_gas_wei: u128,
    resume: bool,
    overwrite: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripEvmDepositOptions {
    artifact_dir: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    stakehub_home: std::path::PathBuf,
    source_chain_id: u64,
    vault_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    pftl_recipient: String,
    amount_atoms: u64,
    nonce: String,
    session_id: String,
    resume: bool,
    overwrite: bool,
    agent_timeout_secs: u64,
    launch_session_managed_externally: bool,
    require_warm_allowance: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripUsdcAllowanceSetupOptions {
    artifact_dir: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    stakehub_home: std::path::PathBuf,
    source_chain_id: u64,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    required_allowance_atoms: u64,
    session_id: String,
    resume: bool,
    overwrite: bool,
    agent_timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct NavRoundtripDepositRelayOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    evm_deposit_report_file: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    vault_address: String,
    token_address: String,
    asset_id: String,
    policy_hash: String,
    proposer: String,
    attestor: Option<String>,
    finalizer: String,
    claimer: String,
    proposer_key_file: std::path::PathBuf,
    attestor_key_file: Option<std::path::PathBuf>,
    finalizer_key_file: std::path::PathBuf,
    claimer_key_file: std::path::PathBuf,
    receipt_operator_key_file: Option<std::path::PathBuf>,
    claim_deposit: bool,
    expires_at_height: u64,
    source_proof_kind: Option<String>,
    source_proof_hash: Option<String>,
    source_public_values_hash: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripPrimaryMintOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    deposit_relay_report_file: Option<std::path::PathBuf>,
    nav_asset_id: String,
    settlement_asset_id: String,
    subscriber: String,
    issuer_key_file: std::path::PathBuf,
    subscriber_key_file: Option<std::path::PathBuf>,
    settlement_receipt_id: Option<String>,
    settlement_supply_allocation_id: Option<String>,
    consume_issued_settlement: bool,
    settlement_amount_atoms: Option<u64>,
    mint_amount: u64,
    nav_epoch: Option<u64>,
    nav_reserve_packet_hash: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripNavExitOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    primary_mint_report_file: std::path::PathBuf,
    nav_asset_id: String,
    settlement_asset_id: String,
    owner: Option<String>,
    owner_key_file: std::path::PathBuf,
    issuer_key_file: std::path::PathBuf,
    amount: Option<u64>,
    settlement_amount_atoms: Option<u64>,
    settlement_receipt_hash: Option<String>,
    redemption_id: Option<String>,
    same_round_settlement: bool,
    nav_epoch: Option<u64>,
    nav_reserve_packet_hash: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripBurnToRedeemOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    nav_exit_report_file: std::path::PathBuf,
    settlement_asset_id: String,
    owner: Option<String>,
    owner_key_file: std::path::PathBuf,
    amount_atoms: Option<u64>,
    destination_ref: String,
    issuer: Option<String>,
    bucket_id: Option<String>,
    epoch: Option<u64>,
    reserve_packet_hash: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripEvmWithdrawalOptions {
    data_dir: std::path::PathBuf,
    artifact_dir: std::path::PathBuf,
    burn_to_redeem_report_file: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    stakehub_home: std::path::PathBuf,
    source_chain_id: u64,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    settlement_asset_id: String,
    redemption_id: Option<String>,
    pftl_finalized_height: Option<u64>,
    signatures_file: Option<std::path::PathBuf>,
    withdrawal_signer_key_file: Option<std::path::PathBuf>,
    session_id: String,
    challenge_wait_secs: Option<u64>,
    resume: bool,
    overwrite: bool,
    agent_timeout_secs: u64,
    launch_session_managed_externally: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripPftlSettleOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    evm_withdrawal_report_file: std::path::PathBuf,
    settlement_asset_id: String,
    issuer_or_redemption_account: Option<String>,
    settlement_key_file: std::path::PathBuf,
    settlement_receipt_hash: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
    batch_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripNavCheckpointOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    nav_asset_id: String,
    issuer_key_file: std::path::PathBuf,
    submitter_key_file: Option<std::path::PathBuf>,
    epoch: Option<u64>,
    expected_vna_delta: Option<i128>,
    reserve_packet_hash: Option<String>,
    attestor_root: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    resume: bool,
    overwrite: bool,
    prepare_only: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripLiveDemoOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    source_rpc_url: String,
    cast_binary: String,
    stakehub_home: std::path::PathBuf,
    source_chain_id: u64,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    nav_asset_id: String,
    settlement_asset_id: String,
    policy_hash: String,
    pftl_recipient: String,
    subscriber: Option<String>,
    owner: Option<String>,
    proposer: String,
    attestor: Option<String>,
    finalizer: String,
    claimer: String,
    proposer_key_file: std::path::PathBuf,
    attestor_key_file: Option<std::path::PathBuf>,
    finalizer_key_file: std::path::PathBuf,
    claimer_key_file: std::path::PathBuf,
    issuer_key_file: std::path::PathBuf,
    owner_key_file: std::path::PathBuf,
    settlement_key_file: Option<std::path::PathBuf>,
    submitter_key_file: Option<std::path::PathBuf>,
    amount_atoms: u64,
    mint_amount: u64,
    nonce: String,
    session_id: String,
    signatures_file: Option<std::path::PathBuf>,
    withdrawal_signer_key_file: Option<std::path::PathBuf>,
    destination_ref: Option<String>,
    expires_at_height: u64,
    source_proof_kind: Option<String>,
    source_proof_hash: Option<String>,
    source_public_values_hash: Option<String>,
    min_gas_wei: u128,
    challenge_wait_secs: Option<u64>,
    pftl_finalized_height: Option<u64>,
    same_round_nav_exit: bool,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    reuse_final_certified_state: bool,
    fast_demo_preflight: bool,
    background_audit: bool,
    require_warm_usdc_allowance: bool,
    resume: bool,
    overwrite: bool,
    batch_only: bool,
    agent_timeout_secs: u64,
}

#[derive(Debug, Clone)]
struct NavRoundtripPftlOnlyOptions {
    data_dir: std::path::PathBuf,
    topology_file: std::path::PathBuf,
    validator_key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    artifact_dir: std::path::PathBuf,
    nav_asset_id: String,
    settlement_asset_id: String,
    subscriber: String,
    owner: String,
    issuer_key_file: std::path::PathBuf,
    owner_key_file: std::path::PathBuf,
    submitter_key_file: Option<std::path::PathBuf>,
    mint_amount: u64,
    settlement_amount_atoms: Option<u64>,
    settlement_receipt_id: Option<String>,
    settlement_supply_allocation_id: Option<String>,
    same_round_nav_exit: bool,
    destination_ref: Option<String>,
    require_local_proposer: bool,
    require_signed_proposal: bool,
    allow_peer_failures: bool,
    quorum_early_full_propagation: bool,
    local_apply_before_certified_send: bool,
    defer_certified_sends: bool,
    block_height: Option<u64>,
    view: Option<u64>,
    timeout_certificate_file: Option<std::path::PathBuf>,
    timeout_ms: u64,
    send_retries: usize,
    retry_backoff_ms: u64,
    allow_existing_mempool: bool,
    reuse_final_certified_state: bool,
    fast_demo_preflight: bool,
    background_audit: bool,
    resume: bool,
    overwrite: bool,
    batch_only: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsBatchReport {
    schema: String,
    request_schema: Option<String>,
    data_dir: String,
    topology_file: String,
    artifact_dir: String,
    operation_count: usize,
    max_transactions: usize,
    allow_existing_mempool: bool,
    prepare_only: bool,
    batch_only: bool,
    start_height: u64,
    start_state_root: String,
    start_mempool_pending: u64,
    end_height: Option<u64>,
    end_state_root: Option<String>,
    end_mempool_pending: Option<u64>,
    operations: Vec<CertifiedAssetOpStageReport>,
    #[serde(default)]
    dependency_report: CertifiedAssetOpsDependencyReport,
    batch_file: Option<String>,
    round_artifact_dir: Option<String>,
    round_ok: Option<bool>,
    timings_ms: CertifiedAssetOpsTimingsReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsDependencyReport {
    declared_dependency_count: usize,
    same_round_dependency_count: usize,
    prior_round_dependency_count: usize,
    same_round_batch_eligible: bool,
    #[serde(default)]
    candidate_batch_classes: Vec<String>,
    #[serde(default)]
    replay_equivalence_required: bool,
    #[serde(default = "serde_default_true")]
    live_round_compression_ready: bool,
    #[serde(default)]
    live_round_compression_blockers: Vec<String>,
    declarations: Vec<CertifiedAssetOpsDependencyDeclarationReport>,
}

fn serde_default_true() -> bool {
    true
}

impl Default for CertifiedAssetOpsDependencyReport {
    fn default() -> Self {
        Self {
            declared_dependency_count: 0,
            same_round_dependency_count: 0,
            prior_round_dependency_count: 0,
            same_round_batch_eligible: true,
            candidate_batch_classes: Vec::new(),
            replay_equivalence_required: false,
            live_round_compression_ready: true,
            live_round_compression_blockers: Vec::new(),
            declarations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsDependencyDeclarationReport {
    operation: String,
    depends_on: String,
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    candidate_batch_class: Option<String>,
    reason: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpStageReport {
    label: String,
    source: String,
    transaction_kind: String,
    operation_file: String,
    quote_file: Option<String>,
    signed_file: Option<String>,
    submit_file: Option<String>,
    tx_id: Option<String>,
    sequence: Option<u64>,
    fee: Option<u64>,
    timings_ms: CertifiedAssetOpTimingsReport,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpTimingsReport {
    prepare_ms: f64,
    quote_ms: f64,
    sign_ms: f64,
    submit_ms: f64,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsTimingsReport {
    total_ms: f64,
    preflight_ms: f64,
    operations_ms: f64,
    certify_ms: f64,
    final_status_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CertifiedAssetOpsFromBundleReport {
    schema: String,
    bundle_dir: String,
    output_file: String,
    operation_count: usize,
    labels: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPreflightReport {
    schema: String,
    artifact_file: String,
    source_rpc_url: String,
    #[serde(default = "nav_roundtrip_default_source_rpc_provider_class")]
    source_rpc_provider_class: String,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    amount_atoms: u64,
    min_gas_wei: String,
    start_height: u64,
    start_state_root: String,
    start_mempool_pending: u64,
    wallet_usdc_atoms: String,
    vault_usdc_atoms: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    usdc_allowance_atoms: Option<String>,
    wallet_gas_wei: String,
    vault_code_bytes: usize,
    verifier_code_bytes: usize,
    vault_challenge_delay_seconds: Option<u64>,
    vault_execution_window_seconds: Option<u64>,
    verifier_challenge_delay_seconds: Option<u64>,
    verifier_execution_window_seconds: Option<u64>,
    bridge_class: String,
    withdrawal_digest_signature: Option<String>,
    submit_withdrawal_signature: Option<String>,
    preflight_ok: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripEvmDepositReport {
    schema: String,
    artifact_file: String,
    source_rpc_url: String,
    #[serde(default = "nav_roundtrip_default_source_rpc_provider_class")]
    source_rpc_provider_class: String,
    source_chain_id: u64,
    vault_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    pftl_recipient: String,
    amount_atoms: u64,
    nonce: String,
    session_id: String,
    wallet_usdc_before_atoms: String,
    wallet_usdc_after_atoms: String,
    vault_usdc_before_atoms: String,
    vault_usdc_after_atoms: String,
    #[serde(default)]
    launch_session_managed_externally: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    allowance_before_atoms: Option<String>,
    #[serde(default)]
    approve_skipped: bool,
    approve_tx: Option<String>,
    approve_gas_used: Option<u64>,
    deposit_tx: String,
    deposit_gas_used: u64,
    approve_calldata_file: String,
    deposit_calldata_file: String,
    agent_open_session_file: String,
    agent_approve_file: String,
    agent_deposit_file: String,
    agent_close_session_file: String,
    #[serde(default)]
    receipt_watches: Vec<NavRoundtripEvmReceiptWatchReport>,
    delta_ok: bool,
    #[serde(default)]
    delta_warnings: Vec<String>,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripEvmReceiptWatchReport {
    label: String,
    tx_hash: String,
    source_rpc_provider_class: String,
    confirmation_source: String,
    status: String,
    gas_used: u64,
    elapsed_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripUsdcAllowanceSetupReport {
    schema: String,
    artifact_file: String,
    source_rpc_url: String,
    #[serde(default = "nav_roundtrip_default_source_rpc_provider_class")]
    source_rpc_provider_class: String,
    source_chain_id: u64,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    required_allowance_atoms: String,
    session_id: String,
    allowance_before_atoms: String,
    allowance_after_atoms: String,
    approve_skipped: bool,
    approve_tx: Option<String>,
    approve_gas_used: Option<u64>,
    approve_calldata_file: String,
    agent_approve_file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stakehub_launch_session_open_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stakehub_launch_session_close_file: Option<String>,
    #[serde(default)]
    receipt_watches: Vec<NavRoundtripEvmReceiptWatchReport>,
    allowance_ok: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripDepositRelayReport {
    schema: String,
    artifact_file: String,
    evm_deposit_report_file: String,
    deposit_tx: String,
    #[serde(default)]
    claim_deposit: bool,
    relay_bundle_dir: String,
    certified_ops_file: String,
    certified_ops_artifact_dir: String,
    relay_bundle: serde_json::Value,
    #[serde(default)]
    certified_ops_stages: Vec<CertifiedAssetOpsBatchReport>,
    certified_ops: CertifiedAssetOpsBatchReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPrimaryMintReport {
    schema: String,
    artifact_file: String,
    deposit_relay_report_file: Option<String>,
    nav_asset_id: String,
    settlement_asset_id: String,
    issuer: String,
    subscriber: String,
    nav_epoch: u64,
    nav_reserve_packet_hash: String,
    nav_per_unit: u64,
    nav_valuation_unit: String,
    settlement_valuation_unit: String,
    settlement_asset_precision: u8,
    mint_amount: u64,
    settlement_amount_atoms: u64,
    settlement_receipt_id: String,
    settlement_bucket_id: String,
    settlement_allocation_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    subscription_id: Option<String>,
    #[serde(default)]
    consume_issued_settlement: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumed_supply_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consumed_supply_allocation_id: Option<String>,
    matched_deposit_tx: Option<String>,
    settlement_status_before: postfiat_node::VaultBridgeStatusReport,
    settlement_status_after: Option<postfiat_node::VaultBridgeStatusReport>,
    operations_file: String,
    allocate_operation_file: String,
    mint_operation_file: String,
    certified_ops_artifact_dir: String,
    certified_ops: CertifiedAssetOpsBatchReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripNavExitReport {
    schema: String,
    artifact_file: String,
    primary_mint_report_file: String,
    nav_asset_id: String,
    settlement_asset_id: String,
    owner: String,
    issuer: String,
    nav_epoch: u64,
    nav_reserve_packet_hash: String,
    redeem_amount: u64,
    settlement_amount_atoms: u64,
    settlement_bucket_id: String,
    settlement_allocation_id: String,
    settlement_receipt_hash: Option<String>,
    redemption_id: Option<String>,
    #[serde(default)]
    same_round_settlement: bool,
    nav_balance_before: Option<u64>,
    nav_balance_after: Option<u64>,
    settlement_balance_before: Option<u64>,
    settlement_balance_after: Option<u64>,
    settlement_status_before: postfiat_node::VaultBridgeStatusReport,
    settlement_status_after: Option<postfiat_node::VaultBridgeStatusReport>,
    redeem_operations_file: String,
    redeem_operation_file: String,
    redeem_certified_ops_artifact_dir: String,
    redeem_certified_ops: CertifiedAssetOpsBatchReport,
    settle_operations_file: Option<String>,
    settle_operation_file: Option<String>,
    settle_certified_ops_artifact_dir: Option<String>,
    settle_certified_ops: Option<CertifiedAssetOpsBatchReport>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripBurnToRedeemReport {
    schema: String,
    artifact_file: String,
    nav_exit_report_file: String,
    settlement_asset_id: String,
    owner: String,
    amount_atoms: u64,
    destination_ref: String,
    owner_balance_before: Option<u64>,
    owner_balance_after: Option<u64>,
    redemption_id: Option<String>,
    settlement_status_before: postfiat_node::VaultBridgeStatusReport,
    settlement_status_after: Option<postfiat_node::VaultBridgeStatusReport>,
    bundle_dir: String,
    bundle: postfiat_node::VaultBridgeBurnToRedeemBundleReport,
    certified_ops_file: String,
    certified_ops_artifact_dir: String,
    certified_ops: CertifiedAssetOpsBatchReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripEvmWithdrawalReport {
    schema: String,
    artifact_file: String,
    burn_to_redeem_report_file: String,
    source_rpc_url: String,
    #[serde(default = "nav_roundtrip_default_source_rpc_provider_class")]
    source_rpc_provider_class: String,
    source_chain_id: u64,
    bridge_class: String,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    settlement_asset_id: String,
    redemption_id: String,
    amount_atoms: u64,
    pftl_finalized_height: u64,
    pftl_withdrawal_hash: String,
    pftl_withdrawal_hash_commitment: String,
    withdrawal_packet_digest: String,
    verifier_pending_proof_id: String,
    verifier_proof_digest_to_sign: String,
    vault_pending_withdrawal_id: String,
    verifier_challenge_wait_secs: u64,
    vault_challenge_wait_secs: u64,
    session_id: String,
    wallet_usdc_before_atoms: String,
    wallet_usdc_after_atoms: String,
    vault_usdc_before_atoms: String,
    vault_usdc_after_atoms: String,
    #[serde(default)]
    launch_session_managed_externally: bool,
    submit_proof_tx: String,
    submit_proof_gas_used: u64,
    finalize_proof_tx: String,
    finalize_proof_gas_used: u64,
    submit_withdrawal_tx: String,
    submit_withdrawal_gas_used: u64,
    finalize_withdrawal_tx: String,
    finalize_withdrawal_gas_used: u64,
    claim_withdrawal_tx: String,
    claim_withdrawal_gas_used: u64,
    submit_proof_calldata_file: String,
    finalize_proof_calldata_file: String,
    submit_withdrawal_calldata_file: String,
    finalize_withdrawal_calldata_file: String,
    claim_withdrawal_calldata_file: String,
    agent_open_session_file: String,
    agent_submit_proof_file: String,
    agent_finalize_proof_file: String,
    agent_submit_withdrawal_file: String,
    agent_finalize_withdrawal_file: String,
    agent_claim_withdrawal_file: String,
    agent_close_session_file: String,
    #[serde(default)]
    receipt_watches: Vec<NavRoundtripEvmReceiptWatchReport>,
    delta_ok: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPftlSettleReport {
    schema: String,
    artifact_file: String,
    evm_withdrawal_report_file: String,
    settlement_asset_id: String,
    issuer_or_redemption_account: String,
    redemption_id: String,
    settlement_receipt_hash: String,
    settled_atoms: u64,
    redemption_state_before: Option<String>,
    redemption_state_after: Option<String>,
    redemption_queue_before_atoms: Option<u64>,
    redemption_queue_after_atoms: Option<u64>,
    counted_value_before_atoms: Option<u64>,
    counted_value_after_atoms: Option<u64>,
    operation_file: String,
    operations_file: String,
    certified_ops_artifact_dir: String,
    certified_ops: CertifiedAssetOpsBatchReport,
    accounting_ok: Option<bool>,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripNavCheckpointReport {
    schema: String,
    artifact_file: String,
    nav_asset_id: String,
    issuer: String,
    submitter: String,
    verifier_kind: Option<String>,
    source_class: Option<String>,
    epoch_before: u64,
    epoch_after: Option<u64>,
    checkpoint_epoch: u64,
    reserve_packet_hash_before: String,
    reserve_packet_hash_after: Option<String>,
    reserve_packet_hash: String,
    nav_per_unit_before: u64,
    nav_per_unit_after: Option<u64>,
    nav_per_unit: u64,
    circulating_supply_before: u64,
    circulating_supply_after: Option<u64>,
    circulating_supply: u64,
    verified_net_assets_before: Option<u64>,
    verified_net_assets_after: Option<u64>,
    verified_net_assets: u64,
    verified_net_assets_delta: Option<i128>,
    expected_verified_net_assets_delta: Option<i128>,
    delta_ok: Option<bool>,
    source_root: String,
    attestor_root: String,
    overlay_value_nav_units: Option<u64>,
    overlay_source_root: Option<String>,
    sp1_base_verified_net_assets: Option<u64>,
    submit_operation_file: String,
    finalize_operation_file: String,
    submit_operations_file: String,
    finalize_operations_file: String,
    submit_certified_ops_artifact_dir: String,
    finalize_certified_ops_artifact_dir: String,
    submit_certified_ops: CertifiedAssetOpsBatchReport,
    finalize_certified_ops: CertifiedAssetOpsBatchReport,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripValidatorStateEvidence {
    node_id: String,
    block_height: u64,
    state_root: String,
    source: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripFleetPreflightReport {
    schema: String,
    artifact_file: String,
    data_dir: String,
    topology_file: String,
    #[serde(default)]
    reused_artifact: bool,
    operator_local_state: NavRoundtripValidatorStateEvidence,
    public_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    local_node_id_in_topology: bool,
    local_node_id_public_host: Option<String>,
    public_validator_consensus_ok: bool,
    operator_matches_public_endpoint: bool,
    #[serde(default)]
    operator_matches_public_quorum: bool,
    preflight_ok: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripLiveDemoTimingsReport {
    total_ms: f64,
    #[serde(default)]
    readiness_preflight_ms: f64,
    #[serde(default)]
    protocol_clock_ms: f64,
    #[serde(default)]
    fleet_preflight_ms: f64,
    #[serde(default)]
    preflight_ms: f64,
    #[serde(default)]
    stakehub_session_ms: f64,
    #[serde(default)]
    stakehub_session_close_ms: f64,
    #[serde(default)]
    evm_deposit_ms: f64,
    #[serde(default)]
    deposit_relay_ms: f64,
    #[serde(default)]
    primary_mint_ms: f64,
    #[serde(default)]
    nav_money_in_ms: f64,
    #[serde(default)]
    nav_exit_ms: f64,
    #[serde(default)]
    nav_money_out_ms: f64,
    #[serde(default)]
    burn_to_redeem_ms: f64,
    #[serde(default)]
    withdrawal_signature_ms: f64,
    #[serde(default)]
    evm_withdrawal_ms: f64,
    #[serde(default)]
    pftl_settle_ms: f64,
    #[serde(default)]
    final_verification_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPftlCertifiedRoundSummary {
    stage: String,
    round: String,
    operation_count: usize,
    start_height: u64,
    end_height: Option<u64>,
    end_state_root: Option<String>,
    round_ok: Option<bool>,
    total_ms: f64,
    certify_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPftlOnlyBridgeOutResumeReport {
    schema: String,
    artifact_file: String,
    run_class: String,
    completion_status: String,
    settlement_asset_id: String,
    owner: String,
    amount_atoms: u64,
    nav_exit_report_file: String,
    destination_ref: Option<String>,
    next_stage: String,
    suggested_command: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripPftlOnlyReport {
    schema: String,
    artifact_file: String,
    artifact_dir: String,
    data_dir: String,
    #[serde(default = "nav_roundtrip_default_pftl_only_run_class")]
    run_class: String,
    #[serde(default = "nav_roundtrip_default_pftl_only_completion_status")]
    completion_status: String,
    #[serde(default = "nav_roundtrip_default_pftl_only_custody_location")]
    custody_location: String,
    #[serde(default = "nav_roundtrip_default_pftl_only_timing_scope")]
    timing_scope: String,
    #[serde(default = "nav_roundtrip_default_pftl_protocol_clock_start")]
    protocol_clock_started_at_stage: String,
    #[serde(default = "nav_roundtrip_default_pftl_protocol_clock_stop")]
    protocol_clock_stopped_at_stage: String,
    #[serde(default)]
    setup_or_recovery_work_included_in_total: bool,
    nav_asset_id: String,
    settlement_asset_id: String,
    subscriber: String,
    owner: String,
    mint_amount: u64,
    expected_money_in_vna_delta: i128,
    expected_money_out_vna_delta: i128,
    #[serde(default)]
    fleet_preflight: Option<NavRoundtripFleetPreflightReport>,
    primary_mint: NavRoundtripPrimaryMintReport,
    nav_money_in: NavRoundtripNavCheckpointReport,
    nav_exit: NavRoundtripNavExitReport,
    nav_money_out: NavRoundtripNavCheckpointReport,
    bridge_out_resume: NavRoundtripPftlOnlyBridgeOutResumeReport,
    bridge_out_resume_file: String,
    final_height: u64,
    final_state_root: String,
    final_mempool_pending: u64,
    #[serde(default)]
    operator_local_state: Option<NavRoundtripValidatorStateEvidence>,
    #[serde(default)]
    public_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    #[serde(default)]
    certified_round_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    #[serde(default = "nav_roundtrip_default_final_validator_state_source")]
    final_validator_state_source: String,
    final_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    final_validator_consensus_ok: bool,
    #[serde(default)]
    background_audit_enabled: bool,
    #[serde(default = "nav_roundtrip_default_final_audit_profile")]
    final_audit_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background_audit_request_file: Option<String>,
    final_summary_ok: bool,
    failure_reasons: Vec<String>,
    #[serde(default)]
    pftl_certified_round_count: usize,
    #[serde(default)]
    pftl_certified_operation_count: usize,
    #[serde(default)]
    pftl_certified_rounds: Vec<NavRoundtripPftlCertifiedRoundSummary>,
    #[serde(default)]
    pftl_replay_equivalence_required_count: usize,
    #[serde(default)]
    pftl_candidate_batch_classes: Vec<String>,
    #[serde(default)]
    pftl_live_round_compression_ready: bool,
    #[serde(default)]
    pftl_live_round_compression_blockers: Vec<String>,
    timings_ms: NavRoundtripLiveDemoTimingsReport,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripLiveDemoReport {
    schema: String,
    artifact_file: String,
    artifact_dir: String,
    data_dir: String,
    #[serde(default = "nav_roundtrip_default_full_run_class")]
    run_class: String,
    #[serde(default = "nav_roundtrip_default_full_completion_status")]
    completion_status: String,
    #[serde(default = "nav_roundtrip_default_full_custody_location")]
    custody_location: String,
    #[serde(default = "nav_roundtrip_default_full_timing_scope")]
    timing_scope: String,
    #[serde(default = "nav_roundtrip_default_full_protocol_clock_start")]
    protocol_clock_started_at_stage: String,
    #[serde(default = "nav_roundtrip_default_full_protocol_clock_stop")]
    protocol_clock_stopped_at_stage: String,
    #[serde(default)]
    setup_or_recovery_work_included_in_total: bool,
    source_rpc_url: String,
    #[serde(default = "nav_roundtrip_default_source_rpc_provider_class")]
    source_rpc_provider_class: String,
    source_chain_id: u64,
    bridge_class: String,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    nav_asset_id: String,
    settlement_asset_id: String,
    pftl_recipient: String,
    subscriber: String,
    owner: String,
    amount_atoms: u64,
    mint_amount: u64,
    expected_money_in_vna_delta: i128,
    expected_money_out_vna_delta: i128,
    #[serde(default = "nav_roundtrip_default_preflight_profile")]
    preflight_profile: String,
    #[serde(default)]
    fleet_preflight: Option<NavRoundtripFleetPreflightReport>,
    preflight: NavRoundtripPreflightReport,
    evm_deposit: NavRoundtripEvmDepositReport,
    deposit_relay: NavRoundtripDepositRelayReport,
    primary_mint: NavRoundtripPrimaryMintReport,
    nav_money_in: NavRoundtripNavCheckpointReport,
    nav_exit: NavRoundtripNavExitReport,
    nav_money_out: NavRoundtripNavCheckpointReport,
    burn_to_redeem: NavRoundtripBurnToRedeemReport,
    evm_withdrawal: NavRoundtripEvmWithdrawalReport,
    pftl_settle: NavRoundtripPftlSettleReport,
    final_height: u64,
    final_state_root: String,
    final_mempool_pending: u64,
    #[serde(default)]
    operator_local_state: Option<NavRoundtripValidatorStateEvidence>,
    #[serde(default)]
    public_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    #[serde(default)]
    certified_round_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    #[serde(default = "nav_roundtrip_default_final_validator_state_source")]
    final_validator_state_source: String,
    final_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    final_validator_consensus_ok: bool,
    #[serde(default)]
    background_audit_enabled: bool,
    #[serde(default = "nav_roundtrip_default_final_audit_profile")]
    final_audit_profile: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    background_audit_request_file: Option<String>,
    final_summary_ok: bool,
    failure_reasons: Vec<String>,
    #[serde(default)]
    pftl_certified_round_count: usize,
    #[serde(default)]
    pftl_certified_operation_count: usize,
    #[serde(default)]
    pftl_certified_rounds: Vec<NavRoundtripPftlCertifiedRoundSummary>,
    #[serde(default)]
    pftl_replay_equivalence_required_count: usize,
    #[serde(default)]
    pftl_candidate_batch_classes: Vec<String>,
    #[serde(default)]
    pftl_live_round_compression_ready: bool,
    #[serde(default)]
    pftl_live_round_compression_blockers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stakehub_launch_session_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stakehub_launch_session_open_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    stakehub_launch_session_close_file: Option<String>,
    timings_ms: NavRoundtripLiveDemoTimingsReport,
}

fn nav_roundtrip_default_final_validator_state_source() -> String {
    "public_validator_rpc".to_string()
}

fn nav_roundtrip_default_preflight_profile() -> String {
    "conservative_blocking".to_string()
}

fn nav_roundtrip_default_source_rpc_provider_class() -> String {
    "unknown_legacy_report".to_string()
}

fn nav_roundtrip_default_full_run_class() -> String {
    NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP.to_string()
}

fn nav_roundtrip_default_full_completion_status() -> String {
    NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP.to_string()
}

fn nav_roundtrip_default_full_custody_location() -> String {
    NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC.to_string()
}

fn nav_roundtrip_default_pftl_only_run_class() -> String {
    NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY.to_string()
}

fn nav_roundtrip_default_pftl_only_completion_status() -> String {
    NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED.to_string()
}

fn nav_roundtrip_default_pftl_only_custody_location() -> String {
    NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE.to_string()
}

fn nav_roundtrip_default_full_timing_scope() -> String {
    "full_arbitrum_roundtrip_protocol_clock_with_blocking_safety_checks".to_string()
}

fn nav_roundtrip_default_pftl_only_timing_scope() -> String {
    "pftl_only_protocol_clock_with_blocking_safety_checks".to_string()
}

fn nav_roundtrip_default_full_protocol_clock_start() -> String {
    "evm_deposit".to_string()
}

fn nav_roundtrip_default_full_protocol_clock_stop() -> String {
    "final_verification".to_string()
}

fn nav_roundtrip_default_pftl_protocol_clock_start() -> String {
    "primary_mint".to_string()
}

fn nav_roundtrip_default_pftl_protocol_clock_stop() -> String {
    "final_verification".to_string()
}

fn nav_roundtrip_source_rpc_provider_class(rpc_url: &str) -> String {
    let normalized = rpc_url.trim().to_ascii_lowercase();
    if normalized.starts_with("ws://") || normalized.starts_with("wss://") {
        return "websocket".to_string();
    }
    if normalized.contains("localhost")
        || normalized.contains("127.0.0.1")
        || normalized.contains("[::1]")
    {
        return "local".to_string();
    }
    for marker in [
        "alchemy",
        "infura",
        "quicknode",
        "blastapi",
        "ankr",
        "drpc",
        "tenderly",
    ] {
        if normalized.contains(marker) {
            return "dedicated_or_gateway_http".to_string();
        }
    }
    if normalized.starts_with("http://") || normalized.starts_with("https://") {
        "public_or_unknown_http".to_string()
    } else {
        "unknown".to_string()
    }
}

fn nav_roundtrip_default_final_audit_profile() -> String {
    "blocking_public_rpc".to_string()
}

#[derive(Debug, Clone)]
struct NavRoundtripBenchmarkVerifyOptions {
    phase: String,
    summary_file: Option<std::path::PathBuf>,
    benchmark_dir: Option<std::path::PathBuf>,
    replay_corpus_file: Option<std::path::PathBuf>,
    replay_corpus_dir: Option<std::path::PathBuf>,
    required_candidate_classes: Vec<String>,
    report_file: Option<std::path::PathBuf>,
    min_clean_runs: Option<usize>,
    max_median_ms: Option<f64>,
    max_p90_ms: Option<f64>,
    strict_exit: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripDashboardStatusOptions {
    summary_file: std::path::PathBuf,
    report_file: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone)]
struct NavRoundtripBenchmarkPlanOptions {
    phase: String,
    base_args_file: std::path::PathBuf,
    benchmark_dir: std::path::PathBuf,
    replay_corpus_file: Option<std::path::PathBuf>,
    replay_corpus_dir: Option<std::path::PathBuf>,
    required_candidate_classes: Vec<String>,
    report_file: Option<std::path::PathBuf>,
    run_count: usize,
    run_prefix: String,
    binary: String,
    max_median_ms: Option<f64>,
    max_p90_ms: Option<f64>,
    overwrite: bool,
}

#[derive(Debug, Clone)]
struct NavRoundtripBenchmarkBaseArgsOptions {
    summary_file: std::path::PathBuf,
    output_file: std::path::PathBuf,
    data_dir: Option<std::path::PathBuf>,
    topology_file: Option<std::path::PathBuf>,
    key_file: std::path::PathBuf,
    proposal_key_file: Option<std::path::PathBuf>,
    proposer_key_file: std::path::PathBuf,
    attestor_key_file: Option<std::path::PathBuf>,
    finalizer_key_file: std::path::PathBuf,
    claimer_key_file: std::path::PathBuf,
    issuer_key_file: std::path::PathBuf,
    owner_key_file: std::path::PathBuf,
    settlement_key_file: Option<std::path::PathBuf>,
    submitter_key_file: Option<std::path::PathBuf>,
    withdrawal_signer_key_file: std::path::PathBuf,
    nonce_base: String,
    session_id_base: String,
    timeout_ms: Option<u64>,
    send_retries: Option<u64>,
    retry_backoff_ms: Option<u64>,
    agent_timeout_secs: Option<u64>,
    min_gas_wei: Option<String>,
    destination_ref: Option<String>,
    overwrite: bool,
    report_file: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkBaseArgsReport {
    schema: String,
    summary_file: String,
    output_file: String,
    args: Vec<String>,
    validation_ok: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkPlanCommand {
    label: String,
    command: Vec<String>,
    command_line: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artifact_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    summary_file: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkPlanRun {
    run_index: usize,
    run_label: String,
    artifact_dir: String,
    summary_file: String,
    fleet_preflight_command: NavRoundtripBenchmarkPlanCommand,
    run_command: NavRoundtripBenchmarkPlanCommand,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkPlanReport {
    schema: String,
    phase: String,
    base_args_file: String,
    benchmark_dir: String,
    run_count: usize,
    run_prefix: String,
    binary: String,
    required_flags: Vec<String>,
    required_candidate_classes: Vec<String>,
    replay_corpus_file: Option<String>,
    replay_corpus_dir: Option<String>,
    verifier_thresholds: NavRoundtripBenchmarkPlanVerifierThresholds,
    base_args: Vec<String>,
    allowance_setup_command: NavRoundtripBenchmarkPlanCommand,
    smoke_run: NavRoundtripBenchmarkPlanRun,
    smoke_verifier_command: NavRoundtripBenchmarkPlanCommand,
    runs: Vec<NavRoundtripBenchmarkPlanRun>,
    verifier_command: NavRoundtripBenchmarkPlanCommand,
    notes: Vec<String>,
    report_file: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkPlanVerifierThresholds {
    min_clean_runs: usize,
    max_median_ms: Option<f64>,
    max_p90_ms: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkSummaryVerifyReport {
    summary_file: String,
    artifact_dir: String,
    data_dir: String,
    run_class: String,
    completion_status: String,
    custody_location: String,
    timing_scope: String,
    protocol_clock_started_at_stage: String,
    protocol_clock_stopped_at_stage: String,
    setup_or_recovery_work_included_in_total: bool,
    total_ms: f64,
    readiness_preflight_ms: f64,
    protocol_clock_ms: f64,
    timings_ms: NavRoundtripLiveDemoTimingsReport,
    source_rpc_provider_class: String,
    preflight_source_rpc_provider_class: String,
    evm_deposit_source_rpc_provider_class: String,
    evm_withdrawal_source_rpc_provider_class: String,
    source_chain_id: u64,
    vault_address: String,
    verifier_address: String,
    usdc_address: String,
    stakehub_wallet: String,
    vault_challenge_delay_seconds: Option<u64>,
    vault_execution_window_seconds: Option<u64>,
    verifier_challenge_delay_seconds: Option<u64>,
    verifier_execution_window_seconds: Option<u64>,
    evm_withdrawal_verifier_challenge_wait_secs: u64,
    evm_withdrawal_vault_challenge_wait_secs: u64,
    approve_skipped: bool,
    allowance_before_atoms: Option<String>,
    stakehub_launch_session_mode: Option<String>,
    evm_deposit_launch_session_managed_externally: bool,
    evm_withdrawal_launch_session_managed_externally: bool,
    background_audit_enabled: bool,
    final_audit_profile: String,
    final_validator_state_source: String,
    final_validator_node_ids: Vec<String>,
    final_height: u64,
    final_state_root: String,
    bridge_class: String,
    final_summary_ok: bool,
    final_validator_consensus_ok: bool,
    final_mempool_pending: u64,
    evm_deposit_wallet_usdc_before_atoms: String,
    evm_deposit_wallet_usdc_after_atoms: String,
    evm_deposit_vault_usdc_before_atoms: String,
    evm_deposit_vault_usdc_after_atoms: String,
    evm_deposit_delta_ok: bool,
    nav_money_in_expected_vna_delta: Option<i128>,
    nav_money_in_actual_vna_delta: Option<i128>,
    nav_money_in_delta_ok: Option<bool>,
    nav_money_out_expected_vna_delta: Option<i128>,
    nav_money_out_actual_vna_delta: Option<i128>,
    nav_money_out_delta_ok: Option<bool>,
    evm_withdrawal_wallet_usdc_before_atoms: String,
    evm_withdrawal_wallet_usdc_after_atoms: String,
    evm_withdrawal_vault_usdc_before_atoms: String,
    evm_withdrawal_vault_usdc_after_atoms: String,
    evm_withdrawal_delta_ok: bool,
    evm_deposit_receipt_watch_count: usize,
    evm_withdrawal_receipt_watch_count: usize,
    evm_withdrawal_receipt_watch_labels: Vec<String>,
    phase3_consolidated_bridge_evidence_ok: Option<bool>,
    pftl_redemption_queue_before_atoms: Option<u64>,
    pftl_redemption_queue_after_atoms: Option<u64>,
    pftl_counted_value_before_atoms: Option<u64>,
    pftl_counted_value_after_atoms: Option<u64>,
    pftl_settle_accounting_ok: Option<bool>,
    pftl_certified_round_count: usize,
    pftl_certified_operation_count: usize,
    pftl_replay_equivalence_required_count: usize,
    pftl_candidate_batch_classes: Vec<String>,
    pftl_live_round_compression_ready: bool,
    pftl_live_round_compression_blockers: Vec<String>,
    passed: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkStageTimingReport {
    stage: String,
    sample_count: usize,
    mean_ms: f64,
    median_ms: Option<f64>,
    p90_ms: Option<f64>,
    best_ms: Option<f64>,
    worst_ms: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripDashboardStatusReport {
    schema: String,
    summary_file: String,
    source_schema: String,
    source_artifact_file: Option<String>,
    run_class: String,
    completion_status: String,
    custody_location: String,
    timing_scope: Option<String>,
    protocol_clock_started_at_stage: Option<String>,
    protocol_clock_stopped_at_stage: Option<String>,
    setup_or_recovery_work_included_in_total: Option<bool>,
    timing_boundary_ok: bool,
    benchmark_clean_timing: bool,
    timings_ms: Option<NavRoundtripLiveDemoTimingsReport>,
    total_ms: Option<f64>,
    readiness_preflight_ms: Option<f64>,
    protocol_clock_ms: Option<f64>,
    source_rpc_provider_class: Option<String>,
    bridge_class: Option<String>,
    background_audit_enabled: Option<bool>,
    final_audit_profile: Option<String>,
    final_validator_state_source: Option<String>,
    display_status: String,
    full_arbitrum_roundtrip_complete: bool,
    pftl_only_complete: bool,
    bridge_out_deferred: bool,
    bridge_out_resume_file: Option<String>,
    bridge_out_resume_command: Option<String>,
    final_summary_ok: bool,
    final_validator_consensus_ok: bool,
    final_mempool_pending: Option<u64>,
    final_height: Option<u64>,
    final_state_root: Option<String>,
    nav_money_in_delta_ok: Option<bool>,
    nav_money_out_delta_ok: Option<bool>,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkVerifyReport {
    schema: String,
    phase: String,
    summary_file: Option<String>,
    benchmark_dir: Option<String>,
    summary_files: Vec<String>,
    artifact_roots: Vec<String>,
    clean_run_definition: String,
    provenance: NavRoundtripBenchmarkProvenanceReport,
    run_count: usize,
    clean_run_count: usize,
    required_clean_runs: usize,
    benchmark_runtime_metric: String,
    total_ms_values: Vec<f64>,
    readiness_preflight_ms_values: Vec<f64>,
    protocol_clock_ms_values: Vec<f64>,
    average_ms: Option<f64>,
    mean_ms: Option<f64>,
    median_ms: Option<f64>,
    p90_ms: Option<f64>,
    best_ms: Option<f64>,
    worst_ms: Option<f64>,
    max_median_ms: Option<f64>,
    max_p90_ms: Option<f64>,
    slowest_stages: Vec<NavRoundtripBenchmarkStageTimingReport>,
    source_rpc_provider_classes: Vec<String>,
    bridge_classes: Vec<String>,
    vault_addresses: Vec<String>,
    verifier_addresses: Vec<String>,
    usdc_addresses: Vec<String>,
    stakehub_wallets: Vec<String>,
    vault_challenge_delay_seconds: Vec<u64>,
    vault_execution_window_seconds: Vec<u64>,
    verifier_challenge_delay_seconds: Vec<u64>,
    verifier_execution_window_seconds: Vec<u64>,
    final_validator_node_ids: Vec<String>,
    phase2_live_round_compression_required: bool,
    phase2_summary_candidate_batch_classes: Vec<String>,
    phase2_required_candidate_classes: Vec<String>,
    phase3_consolidated_bridge_required: bool,
    replay_corpus_report: Option<NavRoundtripReplayCorpusVerifyReport>,
    passed: bool,
    failure_reasons: Vec<String>,
    summaries: Vec<NavRoundtripBenchmarkSummaryVerifyReport>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripBenchmarkProvenanceReport {
    package_version: String,
    binary_path: Option<String>,
    binary_sha3_384: Option<String>,
    git_commit: Option<String>,
    git_dirty: Option<bool>,
    git_status_porcelain_line_count: Option<usize>,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone)]
struct NavRoundtripReplayCorpusVerifyOptions {
    corpus_file: Option<std::path::PathBuf>,
    corpus_dir: Option<std::path::PathBuf>,
    report_file: Option<std::path::PathBuf>,
    require_live_compression_ready: bool,
    required_candidate_classes: Vec<String>,
    strict_exit: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CertifiedAssetOpsBatchEquivalenceCorpusCase {
    schema: String,
    case: String,
    candidate_batch_class: String,
    unbatched_block_height: u64,
    batched_block_height: u64,
    unbatched_state_root: String,
    batched_state_root: String,
    state_root_match: bool,
    #[serde(default)]
    intended_state_root_difference: Option<String>,
    #[serde(default)]
    ledger_facing_asset_definitions_match: Option<bool>,
    #[serde(default)]
    ledger_facing_state_match: Option<bool>,
    safe_for_live_round_compression: bool,
    #[serde(default)]
    gate: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripReplayCorpusCaseVerifyReport {
    corpus_file: String,
    case: String,
    candidate_batch_class: String,
    state_root_match: bool,
    ledger_facing_asset_definitions_match: Option<bool>,
    ledger_facing_state_match: Option<bool>,
    safe_for_live_round_compression: bool,
    valid_corpus_case: bool,
    live_round_compression_ready: bool,
    failure_reasons: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NavRoundtripReplayCorpusVerifyReport {
    schema: String,
    corpus_file: Option<String>,
    corpus_dir: Option<String>,
    corpus_files: Vec<String>,
    require_live_compression_ready: bool,
    required_candidate_classes: Vec<String>,
    live_ready_candidate_classes: Vec<String>,
    missing_required_candidate_classes: Vec<String>,
    case_count: usize,
    valid_case_count: usize,
    live_ready_case_count: usize,
    passed: bool,
    failure_reasons: Vec<String>,
    cases: Vec<NavRoundtripReplayCorpusCaseVerifyReport>,
}
