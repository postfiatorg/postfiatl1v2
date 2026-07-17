use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{self, Child, Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use flate2::read::GzDecoder;
use serde::de::DeserializeOwned;
use zeroize::Zeroizing;

#[cfg(test)]
use postfiat_consensus_cobalt::VALIDATOR_REGISTRY_OP_ROTATE_KEY;
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_sign_with_context,
    ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM, ML_DSA_65_PUBLIC_KEY_BYTES,
    ML_DSA_65_SIGNATURE_BYTES,
};
use postfiat_network::{
    frame_message, verify_message_payload, FramedMessage, NetworkDomain, NetworkTopology,
    DEFAULT_BASE_PORT,
};
use postfiat_node::{
    account, account_assets, account_escrows, account_lines, account_nfts, account_offers,
    account_tx, account_tx_index_status, active_validator_ids_for_node,
    admit_fastlane_primary_to_mempool, aggregate_block_certificate,
    aggregate_block_timeout_certificate, aggregate_verified_block_certificate, apply_amendment,
    apply_batch, apply_batch_with_expected_commit_identity, apply_batch_with_replay,
    apply_batch_with_verified_certificate_with_timings, apply_bridge_batch,
    apply_bridge_batch_with_replay, apply_governance_batch, apply_governance_batch_with_replay,
    apply_shielded_batch, apply_shielded_batch_with_replay, apply_validator_registry_update,
    assemble_consensus_v2_commit, assemble_ethereum_checkpoint_certificate,
    assemble_signed_fastpay_recovery_governance_bootstrap,
    assemble_signed_fastswap_governance_bootstrap, assemble_signed_governance_amendment,
    assemble_signed_validator_registry_update,
    assemble_signed_vault_bridge_route_profile_governance, asset_fee_quote, asset_info,
    asset_orchard_note_status, asset_orchard_scan, atomic_settlement_template,
    atomic_swap_fee_quote, atomic_swap_fee_quote_typed_error_code, batch_archive, block_proposer,
    blocks, book_offers, bridge_pause, bridge_state, bridge_transfer, bridge_upsert_domain,
    build_ethereum_receipt_proof, build_history_archive_window,
    certify_and_persist_consensus_v2_votes, certify_batch_round, checkpoint_pending,
    consensus_v2_active_at, create_asset_orchard_egress, create_asset_orchard_egress_batch,
    create_asset_orchard_ingress, create_asset_orchard_ingress_batch,
    create_asset_orchard_private_egress, create_asset_orchard_private_egress_batch,
    create_asset_orchard_swap_action, create_asset_orchard_swap_action_verified,
    create_atomic_swap_mempool_batch_for_tx_id, create_block_timeout_vote, create_block_vote,
    create_block_vote_for_verified_proposal, create_block_vote_with_timings,
    create_bridge_domain_batch, create_bridge_pause_batch, create_bridge_transfer_batch,
    create_consensus_v2_precommit_vote, create_consensus_v2_prepare_vote,
    create_consensus_v2_proposal_for_block, create_deployment_manifest,
    create_deployment_publisher_private_key, create_fastpay_recovery_governance_bootstrap,
    create_fastswap_governance_bootstrap,
    create_governance_batch, create_governance_genesis_bundle, create_governance_replay_package,
    create_history_archive_handoff, create_mempool_batch, create_operator_manifest,
    create_orchard_action_batch, create_orchard_deposit_action,
    create_orchard_deposit_action_batch, create_orchard_output_action, create_orchard_spend_action,
    create_orchard_withdraw_action, create_orchard_withdraw_action_batch,
    create_shielded_migrate_batch, create_shielded_mint_batch, create_shielded_spend_batch,
    create_shielded_swap_action_batch, create_signed_asset_transaction_batch,
    create_transfer_batch, create_validator_registry_update,
    create_vault_bridge_route_profile_governance,
    create_verified_asset_orchard_swap_action_batch, detect_block_proposal_equivocation,
    detect_block_vote_equivocation, escrow_fee_quote, escrow_info,
    export_deployment_publisher_public_key, export_history_archive_window,
    export_market_ops_replay_bundle, export_signed_snapshot, export_snapshot,
    export_snapshot_publisher_public_key, export_vault_bridge_reserve_replay_bundle, faucet_key,
    governance_agent_evidence_lineage_audit, governance_agent_gate_10_1,
    governance_agent_gate_10_5, governance_agent_gate_14, governance_agent_gate_15,
    governance_agent_gate_1_5, governance_agent_gate_3_5, governance_agent_gate_3_6,
    governance_agent_gate_7_5, governance_agent_gate_7_6, governance_agent_gate_8_5,
    governance_agent_gate_9_5, governance_agent_implementation_execution,
    governance_agent_model_request, governance_agent_round_seed_input_from_optional_parts,
    history_prune, history_prune_plan, history_prune_recover, history_status,
    import_history_archive_window, import_signed_snapshot, import_snapshot, init,
    init_consensus_v2, issuer_assets, issuer_nfts, live_consensus_v2_context,
    market_ops_operation_bundle, market_ops_status, mempool_state, metrics, navcoin_bridge_claims,
    navcoin_bridge_destination_consume, navcoin_bridge_export_debit, navcoin_bridge_import_return,
    navcoin_bridge_launch_config_init, navcoin_bridge_launch_config_template,
    navcoin_bridge_packet, navcoin_bridge_packet_preflight, navcoin_bridge_primary_subscribe,
    navcoin_bridge_receipt_replay, navcoin_bridge_record_fork_rehearsal,
    navcoin_bridge_record_return_burn, navcoin_bridge_refund_source,
    navcoin_bridge_return_burn_request, navcoin_bridge_route_init, navcoin_bridge_routes,
    navcoin_bridge_supply_status, nft_fee_quote, nft_info, observe_ethereum_checkpoint,
    offer_fee_quote, offer_info, orchard_disclosure_packet, orchard_disclosure_verify,
    orchard_fee_resource_policy, orchard_frontier_cache_warm, orchard_operator_policy,
    orchard_pool_report, orchard_test_vector, orchard_view_key_export, orchard_wallet_keygen,
    orchard_wallet_scan, owned_apply, owned_apply_report, owned_apply_v3,
    owned_certificate_domain, owned_certificate_v3, owned_objects,
    owned_recovery_capabilities_v3, owned_recovery_status_v3, owned_safe_unlock, owned_sign,
    owned_sign_v3, owned_unwrap_apply,
    owned_unwrap_apply_report, owned_unwrap_apply_v3, owned_unwrap_sign,
    owned_unwrap_sign_v3, propose_batch, propose_batch_with_required_parent_with_timings,
    propose_batch_with_timings, ratify_governance, ratify_validator_set,
    read_consensus_v2_qc_graph, rebuild_account_tx_index,
    receipts, replay_market_ops_bundle,
    reconcile_terminal_mempool_entries, replay_vault_bridge_reserve_bundle, run_once,
    shield_disclose, shield_mint, shield_scan,
    shield_spend, shield_turnstile, shielded_tree_root, sign_ethereum_checkpoint_vote,
    sign_governance_amendment_authorization, sign_validator_registry_update_authorization,
    stage_deployment_validator_units, stage_validator_key, status,
    submit_signed_asset_transaction_json_to_mempool,
    submit_signed_atomic_swap_transaction_json_to_mempool,
    submit_signed_escrow_transaction_json_to_mempool,
    submit_signed_nft_transaction_json_to_mempool, submit_signed_offer_transaction_json_to_mempool,
    submit_signed_payment_v2_json_to_mempool, submit_signed_transfer_json_to_mempool,
    submit_signed_transfer_to_mempool, submit_transfer_to_mempool, transfer, transfer_fee_quote,
    tx_finality, validate_local_keys, validator_keys, validator_registry_root_report,
    vault_bridge_asset_id, vault_bridge_bootstrap_bundle, vault_bridge_burn_to_redeem_bundle,
    vault_bridge_conservation_audit,
    vault_bridge_deposit_intent, vault_bridge_deposit_plan, vault_bridge_deposit_relay_bundle,
    vault_bridge_deposit_relay_rpc_bundle, vault_bridge_receipts, vault_bridge_route,
    vault_bridge_status,
    vault_bridge_withdrawal_plan, vault_bridge_withdrawal_relay_bundle,
    vault_bridge_withdrawal_signature_bundle, pfusdc_checkpoint_witness,
    pfusdc_egress_witness,
    verify_block_proposal_equivocation,
    verify_block_timeout_certificate_file, verify_block_vote_equivocation, verify_blocks,
    verify_bridge, verify_consensus_v2_proposal_matches_block, verify_deployment_manifest,
    verify_governance_amendment_replay_bundle, verify_governance_genesis_bundle,
    verify_governance_replay_package, verify_governance_with_options,
    history_checkpoint_rebuild_from_archive, verify_history_archive_handoff,
    verify_history_archive_window_bundle, verify_mempool,
    verify_operator_manifest, verify_or_apply_orchard_action, verify_shielded, verify_state,
    verify_validator_registry_lifecycle_replay_bundle, verify_validator_registry_update_file,
    wallet_keygen, wallet_restore, wallet_sign_asset_transaction, wallet_sign_escrow_transaction,
    wallet_sign_offer_transaction, wallet_sign_transfer, wallet_test_vector,
    write_consensus_v2_block_certificate_file, write_consensus_v2_topology, write_local_topology,
    AccountAssetsOptions, AccountEscrowsOptions, AccountLinesOptions, AccountNftsOptions,
    AccountOffersOptions, AccountTxIndexOptions, AccountTxQueryOptions, ApplyAmendmentOptions,
    ApplyBatchOptions, ApplyBatchTimingReport, AssetFeeQuoteOptions, AssetFeeQuoteReport,
    AssetInfoOptions, AssetOrchardEgressBatchOptions, AssetOrchardEgressCreateOptions,
    AssetOrchardIngressBatchOptions, AssetOrchardIngressCreateOptions,
    AssetOrchardNoteStatusOptions, AssetOrchardPrivateEgressBatchOptions,
    AssetOrchardPrivateEgressCreateOptions, AssetOrchardScanOptions, AssetOrchardSwapCreateOptions,
    AssetOrchardSwapCreateReport, AtomicSettlementTemplateOptions, AtomicSwapFeeQuoteOptions,
    AtomicSwapQuoteLegInput, AtomicSwapTargetBatchOptions, BatchArchiveQueryOptions,
    BatchCertificateRoundOptions, BatchCertificateRoundReport, BatchProposalOptions,
    BatchProposalTimingReport, BatchTransferOptions, BlockCertificateFile,
    BlockCertificateFromArchiveOptions, BlockCertificateOptions, BlockProposalEquivocationOptions,
    BlockProposalFile, BlockProposerOptions, BlockQueryOptions, BlockTimeoutCertificateOptions,
    BlockTimeoutCertificateVerifyOptions, BlockTimeoutVoteFile, BlockTimeoutVoteOptions,
    BlockVoteCreationTimingReport,
    BlockVoteEquivocationOptions, BlockVoteFile, BlockVoteForVerifiedProposalOptions,
    BlockVoteOptions, BookOffersOptions, BridgeDomainBatchOptions, BridgeDomainOptions,
    BridgePauseBatchOptions, BridgePauseOptions, BridgeTransferBatchOptions, BridgeTransferOptions,
    CertificateRoundPrivateKeyPolicy, DeploymentManifestCreateOptions,
    DeploymentManifestVerifyOptions, DeploymentPublisherKeyCreateOptions,
    DeploymentPublisherKeyExportOptions, DeploymentValidatorUnitsStageOptions,
    EscrowFeeQuoteOptions, EscrowFeeQuoteReport, EscrowInfoOptions,
    EthereumCheckpointCertificateAssembleOptions, EthereumCheckpointObserveOptions,
    EthereumCheckpointVoteSignOptions, EthereumReceiptProofBuildOptions,
    ExpectedBatchCommitIdentity, FastPayRecoveryGovernanceBootstrapOptions,
    FastSwapGovernanceBootstrapOptions,
    GovernanceAgentEvidenceLineageAuditOptions, GovernanceAgentGate10_1Options,
    GovernanceAgentGate10_5Options, GovernanceAgentGate14Options, GovernanceAgentGate15Options,
    GovernanceAgentGate3_5Options, GovernanceAgentGate3_6Options, GovernanceAgentGate7_5Options,
    GovernanceAgentGate7_6Options, GovernanceAgentGate8_5Options, GovernanceAgentGate9_5Options,
    GovernanceAgentGateOptions, GovernanceAgentImplementationExecutionOptions,
    GovernanceAgentModelRequestOptions, GovernanceAmendmentAssembleOptions,
    GovernanceAmendmentReplayVerifyOptions, GovernanceAuthorizationSignOptions,
    GovernanceBatchOptions, GovernanceGenesisBundleOptions, GovernanceGenesisVerifyOptions,
    GovernanceReplayBuildOptions, GovernanceReplayVerifyOptions, GovernanceVerifyOptions,
    HistoryArchiveHandoffCreateOptions, HistoryArchiveHandoffVerifyOptions,
    HistoryArchiveWindowBuildOptions, HistoryArchiveWindowBundle,
    HistoryArchiveWindowExportOptions, HistoryArchiveWindowImportOptions,
    HistoryArchiveWindowVerifyOptions, HistoryCheckpointRebuildFromArchiveOptions, HistoryOptions,
    InitConsensusV2Options, InitOptions,
    IssuerAssetsOptions, IssuerNftsOptions, MarketOpsOperationBundleOptions,
    MarketOpsReplayBundleExportOptions, MarketOpsReplayBundleVerifyOptions, MarketOpsStatusOptions,
    MempoolBatchOptions, NavcoinBridgeClaimsOptions, NavcoinBridgeDestinationConsumeOptions,
    NavcoinBridgeExportDebitOptions, NavcoinBridgeImportReturnOptions,
    NavcoinBridgeLaunchConfigInitOptions, NavcoinBridgeLaunchConfigTemplateOptions,
    NavcoinBridgePacketOptions, NavcoinBridgePacketPreflightOptions,
    NavcoinBridgePrimarySubscribeOptions, NavcoinBridgeReceiptReplayOptions,
    NavcoinBridgeRecordForkRehearsalOptions, NavcoinBridgeRecordReturnBurnOptions,
    PfUsdcCheckpointWitnessOptions, PfUsdcEgressWitnessOptions,
    NavcoinBridgeRefundSourceOptions, NavcoinBridgeReturnBurnRequestOptions,
    NavcoinBridgeRouteInitOptions, NavcoinBridgeRoutesOptions, NavcoinBridgeSupplyStatusOptions,
    NftFeeQuoteOptions, NftInfoOptions, NodeOptions, OfferFeeQuoteOptions, OfferFeeQuoteReport,
    OfferInfoOptions, OperatorManifestCreateOptions, OperatorManifestVerifyOptions,
    OrchardActionBatchOptions, OrchardActionOptions, OrchardDepositActionBatchOptions,
    OrchardDepositActionOptions, OrchardDisclosureOptions, OrchardDisclosureVerifyOptions,
    OrchardFeeResourcePolicyOptions, OrchardOperatorPolicyOptions, OrchardOutputActionOptions,
    OrchardPoolReportOptions, OrchardSpendActionOptions, OrchardTestVectorOptions,
    OrchardViewKeyExportOptions, OrchardWalletKeygenOptions, OrchardWalletScanOptions,
    OrchardWithdrawActionBatchOptions, OrchardWithdrawActionOptions, OwnedObjectsOptions,
    RatifyGovernanceOptions, RatifyValidatorSetOptions, ReceiptQueryOptions, RequiredBlockParent,
    ShieldMigrateBatchOptions, ShieldMintBatchOptions, ShieldMintOptions, ShieldSpendBatchOptions,
    ShieldSpendOptions, ShieldedSwapActionBatchOptions, SignedAssetTransactionBatchOptions,
    SignedAssetTransactionJsonSubmitOptions, SignedAtomicSwapTransactionJsonSubmitOptions,
    SignedEscrowTransactionJsonSubmitOptions, SignedFastPayRecoveryGovernanceBootstrapOptions,
    SignedFastSwapGovernanceBootstrapOptions,
    SignedNftTransactionJsonSubmitOptions, SignedOfferTransactionJsonSubmitOptions,
    SignedPaymentV2JsonSubmitOptions, SignedSnapshotExportOptions, SignedSnapshotImportOptions,
    SignedTransferJsonSubmitOptions, SignedTransferSubmitOptions,
    SignedVaultBridgeRouteProfileGovernanceOptions, SnapshotExportOptions,
    SnapshotImportOptions, SnapshotPublisherKeyExportOptions, TopologyConsensusV2Options,
    TopologyOptions, TransferFeeQuoteOptions, TransferFeeQuoteReport, TransferOptions,
    TxFinalityQueryOptions, TxFinalityReport, ValidatorKeyFile, ValidatorKeyRecord,
    ValidatorKeyStageOptions, ValidatorKeysOptions, ValidatorRegistry,
    ValidatorRegistryAuthorizationSignOptions, ValidatorRegistryLifecycleReplayVerifyOptions,
    ValidatorRegistryRootOptions, ValidatorRegistryUpdateApplyOptions,
    ValidatorRegistryUpdateAssembleOptions, ValidatorRegistryUpdateOptions,
    ValidatorRegistryUpdateVerifyOptions, VaultBridgeAssetIdOptions,
    VaultBridgeBootstrapBundleOptions, VaultBridgeBurnToRedeemBundleOptions,
    VaultBridgeConservationOptions,
    VaultBridgeDepositIntentOptions, VaultBridgeDepositPlanOptions,
    VaultBridgeDepositRelayBundleOptions, VaultBridgeDepositRelayRpcBundleOptions,
    VaultBridgeRouteProfileGovernanceOptions,
    VaultBridgeReceiptsOptions, VaultBridgeReserveReplayBundleExportOptions,
    VaultBridgeReserveReplayBundleVerifyOptions, VaultBridgeRouteOptions,
    VaultBridgeStatusOptions,
    VaultBridgeWithdrawalPlanOptions, VaultBridgeWithdrawalRelayBundleOptions,
    VaultBridgeWithdrawalSignatureBundleOptions, VerifiedBlockCertificateFile, WalletKeygenOptions,
    WalletRestoreOptions, WalletSignAssetTransactionOptions, WalletSignEscrowTransactionOptions,
    WalletSignOfferTransactionOptions, WalletSignTransferOptions, WalletTestVectorOptions,
    DEFAULT_BRIDGE_WITNESS_SIGNER, DEFAULT_GOVERNANCE_AGENT_COMPARISON_DIR,
    DEFAULT_GOVERNANCE_AGENT_DIR, DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE,
    DEFAULT_GOVERNANCE_AGENT_GATE_10_1_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT,
    DEFAULT_GOVERNANCE_AGENT_GATE_14_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_15_REPORT,
    DEFAULT_GOVERNANCE_AGENT_GATE_1_5_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_3_5_REPORT,
    DEFAULT_GOVERNANCE_AGENT_GATE_3_6_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_7_5_REPORT,
    DEFAULT_GOVERNANCE_AGENT_GATE_7_6_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPLAY_BUNDLE,
    DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPORT, DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT,
    DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_EVIDENCE_FILE,
    DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE,
    DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_EXECUTION_REPORT,
    DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_FILE,
    DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE, MAX_READ_QUERY_LIMIT, VALIDATOR_KEYS_FILE,
    VALIDATOR_REGISTRY_FILE,
};
use postfiat_ordering_fast::bft_quorum_threshold;
use postfiat_privacy_orchard::{AssetOrchardSwapProvingKey, AssetOrchardSwapVerifyingKey};
use postfiat_rpc_sdk::{
    error_response, read_request_file, success_response, to_pretty_json, RpcEvent, RpcRequest,
    RpcResponse, MAX_RPC_REQUEST_BYTES,
};
use postfiat_storage::NodeStore;
#[cfg(test)]
use postfiat_types::ValidatorRegistryEntry;
use postfiat_types::NAV_PROFILE_VERIFIER_MULTI_FETCH;
use postfiat_types::{
    BatchArchiveEntry, BlockHeader, BlockRecord, Receipt, StatusReport, BRIDGE_DIRECTION_INBOUND,
    DEFAULT_BRIDGE_DOMAIN_ID, DEFAULT_SHIELDED_ASSET_ID, GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
    GOVERNANCE_AUTHORITY_MODE_FOUNDATION, GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, GOVERNANCE_KIND_AUTHORITY_MODE,
    GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT, GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
    GOVERNANCE_KIND_CRYPTO_POLICY, GOVERNANCE_KIND_ORCHARD_POOL_PAUSE,
    GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT,
};

const DEFAULT_DATA_DIR: &str = ".postfiat/node0";
const DEFAULT_CHAIN_ID: &str = "postfiat-local";
const DEFAULT_NODE_ID: &str = "validator-0";
const DEFAULT_TOPOLOGY_FILE: &str = "devnet/configs/topology.json";
const MAX_TRANSPORT_SEND_RETRIES: usize = 16;
const DEFAULT_RPC_CATCH_UP_MAX_BLOCKS: usize = 64;
const DEFAULT_RPC_MEMPOOL_SUBMIT_PER_PEER: u64 = 16;
const DEFAULT_RPC_MEMPOOL_SUBMIT_TOTAL: u64 = 64;
const DEFAULT_RPC_ORCHARD_BATCH_CREATE_PER_PEER: u64 = 2;
const DEFAULT_RPC_ORCHARD_BATCH_CREATE_TOTAL: u64 = 8;
const DEFAULT_RPC_ORCHARD_BATCH_CREATE_CONCURRENT: u64 = 1;
const DEFAULT_RUN_PEER_CERTIFIED_MAX_ROUNDS: usize = 1_000_000;
const DIRECT_STATE_ENV: &str = "POSTFIAT_ALLOW_DIRECT_STATE";
const TRANSPORT_AUTH_SCHEMA: &str = "postfiat-transport-auth-v1";
const TRANSPORT_AUTH_CONTEXT: &[u8] = b"postfiat-l1-v2/transport-auth/v1";
const MAX_RPC_SERVE_ACTIVE_CONNECTIONS: usize = 64;
const RPC_ORCHARD_ACTION_SPOOL_DIR: &str = "rpc-orchard-actions";
const RPC_ORCHARD_BATCH_SPOOL_DIR: &str = "rpc-orchard-batches";

#[derive(Debug, Clone, serde::Serialize)]
struct AssetOrchardSwapLiveRoundTimingReport {
    total_ms: f64,
    prewarm_prover_cache_ms: Option<f64>,
    swap_create_ms: f64,
    batch_wrap_ms: f64,
    transport_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
struct AssetOrchardSwapLiveRoundReport {
    schema: String,
    data_dir: String,
    topology_file: String,
    action_file: String,
    batch_file: String,
    output_note_files: [String; 2],
    artifact_dir: String,
    prewarm_prover_cache: bool,
    duplicate_local_verification_skipped: bool,
    swap_create: AssetOrchardSwapCreateReport,
    batch_id: String,
    batch_action_count: usize,
    transport: TransportPeerCertifiedBatchRoundReport,
    timings: AssetOrchardSwapLiveRoundTimingReport,
    report_file: Option<String>,
    round_ok: bool,
}

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let is_rpc = args.first().is_some_and(|command| command == "rpc");
    if let Err(error) = run_cli(args.clone()) {
        if is_rpc {
            let id = rpc_error_id(&args[1..]);
            let (code, message) = rpc_dispatch::rpc_dispatch_error_response_parts(&error);
            if let Err(print_error) = print_rpc_error(&id, code, message) {
                eprintln!("error: {error}");
                eprintln!("rpc error serialization failed: {print_error}");
            }
        } else {
            eprintln!("error: {error}");
            print_usage();
        }
        process::exit(1);
    }
}

fn run_cli(args: Vec<String>) -> Result<(), String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Err("missing command".to_string());
    };
    let flags = &args[1..];

    match command {
        "init"
        | "init-consensus-v2"
        | "topology"
        | "topology-consensus-v2"
        | "transport-listen"
        | "transport-dial"
        | "transport-batch-listen"
        | "transport-batch-serve"
        | "transport-batch-send"
        | "transport-block-vote-listen"
        | "transport-validator-serve"
        | "transport-block-vote-request"
        | "transport-certified-batch-round"
        | "transport-peer-certified-batch-round"
        | "transport-peer-certified-mempool-round"
        | "pftl-submit-certified-asset-ops"
        | "submit-certified-asset-ops"
        | "pftl-certified-asset-ops-from-bundle"
        | "nav-roundtrip-dashboard-status"
        | "nav-roundtrip-benchmark-base-args"
        | "nav-roundtrip-benchmark-plan"
        | "nav-roundtrip-benchmark-verify"
        | "nav-roundtrip-replay-corpus-verify" => run_cli_group_01(command, flags),
        "nav-roundtrip-live-demo"
        | "tx-latency-benchmark"
        | "real-transaction-latency-benchmark" => run_cli_group_02(command, flags),
        "transport-certified-batch-loop"
        | "transport-peer-certified-batch-loop"
        | "transport-peer-certified-private-egress-loop"
        | "transport-certified-send-outbox-resume"
        | "rpc-serve"
        | "validator-keys"
        | "validator-key-stage"
        | "validate-local-keys"
        | "run"
        | "status"
        | "history-status"
        | "history-prune-plan"
        | "history-can-prune"
        | "history-prune"
        | "history-prune-recover"
        | "history-checkpoint-rebuild-from-archive"
        | "history-archive-handoff-create"
        | "history-archive-handoff-verify"
        | "archive-export-window"
        | "archive-window-verify"
        | "archive-window-import"
        | "archive-window-backfill"
        | "block-proposer"
        | "metrics"
        | "faucet"
        | "owned-apply"
        | "owned-sign"
        | "checkpoint-pending"
        | "owned-safe-unlock"
        | "wallet-keygen"
        | "wallet-restore"
        | "wallet-sign-transfer"
        | "wallet_sign_transfer"
        | "wallet-sign-asset-transaction"
        | "wallet_sign_asset_transaction"
        | "wallet-sign-escrow-transaction"
        | "wallet_sign_escrow_transaction"
        | "wallet-sign-offer-transaction"
        | "wallet_sign_offer_transaction"
        | "wallet-test-vector"
        | "orchard-test-vector"
        | "transfer"
        | "batch-transfer"
        | "mempool-submit-transfer"
        | "mempool-submit-signed-transfer"
        | "mempool-submit-signed-payment-v2"
        | "mempool-submit-signed-asset-transaction"
        | "mempool-submit-signed-escrow-transaction"
        | "mempool-submit-signed-nft-transaction"
        | "mempool-submit-signed-offer-transaction"
        | "mempool-batch"
        | "signed-asset-batch"
        | "mempool-status"
        | "propose-batch"
        | "apply-batch"
        | "ratify-validator-set"
        | "ratify-crypto-policy"
        | "ratify-bridge-witness-epoch"
        | "ratify-authority-mode"
        | "ratify-orchard-pool-pause"
        | "ratify-atomic-swap-pause"
        | "ratify-bridge-verification-activation-height"
        | "ratify-atomic-swap-activation-height"
        | "ratify-replicated-state-v2-activation-height"
        | "governance-authorization-sign"
        | "governance-amendment-assemble"
        | "ethereum-checkpoint-observe"
        | "ethereum-receipt-proof-build"
        | "ethereum-checkpoint-vote-sign"
        | "ethereum-checkpoint-certificate-assemble"
        | "validator-registry-root"
        | "validator-registry-update"
        | "validator-registry-authorization-sign"
        | "validator-registry-update-assemble"
        | "validator-registry-update-verify"
        | "validator-registry-update-apply"
        | "validator-registry-lifecycle-replay-verify"
        | "governance-replay-verify" => run_cli_group_03(command, flags),
        "governance-amendment-replay-verify"
        | "governance-replay-build"
        | "operator-manifest-create"
        | "operator-manifest-verify"
        | "governance-genesis-bundle"
        | "governance-genesis-verify"
        | "governance-agent-gate1-5"
        | "governance-agent-model-request"
        | "governance-agent-gate3-5"
        | "governance-agent-gate3-6"
        | "governance-agent-gate7-5"
        | "governance-agent-gate7-6"
        | "governance-agent-gate8-5"
        | "governance-agent-gate9-5"
        | "governance-agent-gate10-1"
        | "governance-agent-gate10-5"
        | "governance-agent-gate14"
        | "governance-agent-gate15"
        | "governance-agent-evidence-lineage-audit"
        | "governance-agent-implementation-execution"
        | "apply-amendment"
        | "governance-batch"
        | "fastswap-governance-bootstrap"
        | "fastswap-governance-bootstrap-assemble"
        | "fastpay-recovery-governance-bootstrap"
        | "fastpay-recovery-governance-bootstrap-assemble"
        | "vault-bridge-route-profile-governance"
        | "vault-bridge-route-profile-governance-assemble"
        | "apply-governance-batch"
        | "account"
        | "account-tx"
        | "account_tx"
        | "account-tx-index-build"
        | "account-tx-index-status"
        | "transfer-fee-quote"
        | "transfer_fee_quote"
        | "asset-fee-quote"
        | "asset_fee_quote"
        | "escrow-fee-quote"
        | "escrow_fee_quote"
        | "nft-fee-quote"
        | "nft_fee_quote"
        | "offer-fee-quote"
        | "offer_fee_quote"
        | "offer-info"
        | "offer_info"
        | "account-offers"
        | "account_offers"
        | "book-offers"
        | "book_offers"
        | "atomic-settlement-template"
        | "atomic_settlement_template"
        | "asset-info"
        | "asset_info"
        | "account-lines"
        | "account_lines"
        | "account-assets"
        | "account_assets"
        | "owned-objects"
        | "owned_objects"
        | "issuer-assets"
        | "issuer_assets"
        | "escrow-info"
        | "escrow_info"
        | "account-escrows"
        | "account_escrows"
        | "nft-info"
        | "nft_info"
        | "account-nfts"
        | "account_nfts"
        | "issuer-nfts"
        | "issuer_nfts"
        | "receipts"
        | "blocks"
        | "pfusdc-egress-witness"
        | "pfusdc-checkpoint-witness"
        | "block-vote"
        | "block-vote-equivocation"
        | "block-vote-equivocation-verify"
        | "block-proposal-equivocation"
        | "block-proposal-equivocation-verify"
        | "block-certificate"
        | "block-certificate-from-archive"
        | "rpc-catch-up"
        | "rpc-catch-up-certified-delta"
        | "block-timeout-vote"
        | "block-timeout-certificate"
        | "block-timeout-verify"
        | "certify-batch"
        | "batch-archive"
        | "export-envelope-bundle"
        | "replay-envelope" => run_cli_group_04(command, flags),
        "market-ops-status"
        | "market-ops-operation-bundle"
        | "vault-bridge-status"
        | "vault-bridge-conservation-audit"
        | "navcoin-bridge-routes"
        | "navcoin-bridge-packet"
        | "navcoin-bridge-claims"
        | "navcoin-bridge-supply-status"
        | "navcoin-bridge-receipt-replay"
        | "navcoin-bridge-route-init"
        | "navcoin-bridge-launch-config-template"
        | "navcoin-bridge-launch-config-init"
        | "navcoin-bridge-record-fork-rehearsal"
        | "navcoin-bridge-packet-preflight"
        | "navcoin-bridge-primary-subscribe"
        | "navcoin-bridge-export-debit"
        | "navcoin-bridge-destination-consume"
        | "navcoin-bridge-refund-source"
        | "navcoin-bridge-return-burn-request"
        | "navcoin-bridge-record-return-burn"
        | "navcoin-bridge-import-return"
        | "vault-bridge-receipts"
        | "vault-bridge-asset-id"
        | "vault-bridge-bootstrap-bundle"
        | "vault-bridge-deposit-intent"
        | "vault-bridge-deposit-plan"
        | "vault-bridge-deposit-relay-bundle"
        | "vault-bridge-deposit-relay-rpc-bundle"
        | "vault-bridge-burn-to-redeem-bundle"
        | "vault-bridge-withdrawal-plan"
        | "vault-bridge-withdrawal-signature-bundle"
        | "vault-bridge-withdrawal-relay-bundle"
        | "vault-bridge-export-reserve-packet"
        | "vault-bridge-replay-reserve-packet"
        | "verify-blocks"
        | "verify-state"
        | "verify-governance"
        | "verify-bridge"
        | "verify-mempool"
        | "verify-shielded"
        | "orchard-action"
        | "orchard-operator-policy"
        | "orchard-fee-resource-policy"
        | "orchard-frontier-cache-warm"
        | "orchard-pool-report"
        | "orchard-output-create"
        | "orchard-deposit-create"
        | "asset-orchard-ingress-create"
        | "asset-orchard-egress-create"
        | "asset-orchard-private-egress-create"
        | "asset-orchard-note-status"
        | "asset-orchard-scan"
        | "asset-orchard-swap-create"
        | "asset-orchard-swap-live-round"
        | "orchard-spend-create"
        | "orchard-withdraw-create"
        | "orchard-keygen"
        | "orchard-view-key-export"
        | "orchard-scan"
        | "orchard-disclose"
        | "orchard-disclosure-verify"
        | "shield-mint"
        | "shield-spend"
        | "shield-batch-mint"
        | "shield-batch-spend"
        | "shield-batch-migrate"
        | "shield-batch-orchard"
        | "shield-batch-orchard-deposit"
        | "shield-batch-asset-orchard-ingress"
        | "shield-batch-asset-orchard-egress"
        | "shield-batch-asset-orchard-private-egress"
        | "shield-batch-orchard-withdraw"
        | "shield-batch-swap"
        | "apply-shield-batch"
        | "shield-scan"
        | "shield-disclose"
        | "shield-turnstile"
        | "shield-root"
        | "bridge-domain"
        | "bridge-transfer"
        | "bridge-pause"
        | "bridge-resume"
        | "bridge-status"
        | "bridge-batch-domain"
        | "bridge-batch-transfer"
        | "bridge-batch-pause"
        | "bridge-batch-resume"
        | "apply-bridge-batch"
        | "snapshot-export"
        | "snapshot-import"
        | "snapshot-publisher-key-export"
        | "snapshot-export-signed"
        | "snapshot-import-signed"
        | "deployment-publisher-key-export"
        | "deployment-manifest-create"
        | "deployment-manifest-verify"
        | "rpc"
        | "help"
        | "--help"
        | "-h" => run_cli_group_05(command, flags),
        "deployment-publisher-key-create" | "deployment-validator-units-stage" => {
            run_cli_group_05(command, flags)
        }
        other => Err(format!("unknown command `{other}`")),
    }
}

include!("cli_dispatch_parts/group_01.rs");
include!("cli_dispatch_parts/group_02.rs");
include!("cli_dispatch_parts/group_03.rs");
include!("cli_dispatch_parts/group_04.rs");
include!("cli_dispatch_parts/group_05.rs");

fn wait_for_asset_orchard_swap_start_signal(path: &Path, timeout_ms: u64) -> Result<(), String> {
    let start = Instant::now();
    let timeout = (timeout_ms > 0).then(|| Duration::from_millis(timeout_ms));
    loop {
        if path.is_file() {
            return Ok(());
        }
        if let Some(timeout) = timeout {
            if start.elapsed() >= timeout {
                return Err(format!(
                    "asset-orchard-swap-live-round timed out waiting for start signal `{}`",
                    path.display()
                ));
            }
        }
        thread::sleep(Duration::from_millis(50));
    }
}
