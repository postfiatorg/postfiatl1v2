#![allow(
    clippy::io_other_error,
    clippy::large_enum_variant,
    clippy::manual_is_multiple_of,
    clippy::needless_borrow,
    clippy::too_many_arguments,
    clippy::unnecessary_map_or
)]

use sha2::{Digest as Sha2Digest, Sha256, Sha384};
use sha3::{Digest as Sha3Digest, Keccak256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

use postfiat_bridge::{
    apply_simulated_transfer, bridge_witness_attestation_id, bridge_witness_attestation_message,
    pftl_uniswap_apply_primary_subscription_with_receipt, pftl_uniswap_bridge_claims_status,
    pftl_uniswap_bridge_ledger_from_config, pftl_uniswap_bridge_ledger_hash,
    pftl_uniswap_bridge_packet_status, pftl_uniswap_bridge_routes_status,
    pftl_uniswap_bridge_supply_status, pftl_uniswap_export_debit_with_receipt,
    pftl_uniswap_fork_rehearsal_evidence_digest, pftl_uniswap_import_return_with_receipt,
    pftl_uniswap_launch_config_digest, pftl_uniswap_mark_destination_consumed_with_receipt,
    pftl_uniswap_packet_id, pftl_uniswap_record_return_burn_with_receipt,
    pftl_uniswap_refund_source_with_receipt, pftl_uniswap_return_burn_id,
    pftl_uniswap_route_config_digest, pftl_uniswap_transition_receipt_hash,
    pftl_uniswap_verify_transition_receipt_replay, set_domain_paused, upsert_domain_with_metadata,
    validate_pftl_uniswap_bridge_ledger, validate_pftl_uniswap_fork_rehearsal_evidence,
    validate_pftl_uniswap_launch_config, validate_pftl_uniswap_packet_against_launch_config,
    BridgeError, BridgeTransferRequest, BridgeWitnessChainDomain, PftlUniswapBridgeLedger,
    PftlUniswapClaimsStatusReport, PftlUniswapExportDebitRequest, PftlUniswapExportPacketStatus,
    PftlUniswapExportPacketStatusRow, PftlUniswapForkRehearsalEvidence, PftlUniswapLaunchConfig,
    PftlUniswapMintAndSwapPacket, PftlUniswapNativeBalanceRow,
    PftlUniswapOfficialUniswapV4Deployments, PftlUniswapPacketStatusReport,
    PftlUniswapPoolSeedConfig, PftlUniswapPrimarySubscriptionRequest,
    PftlUniswapReceiptReplayReport, PftlUniswapRefundRequest, PftlUniswapReturnBurnRequest,
    PftlUniswapRouteConfig, PftlUniswapRouteStatusRow, PftlUniswapRoutesStatusReport,
    PftlUniswapSupplyStatusReport, PftlUniswapTransitionReceipt, PFTL_UNISWAP_STATUS_MAX_ROWS,
    ROUTE_TRUST_CLASS_DISABLED,
};
use postfiat_consensus_cobalt::{
    certify_validator_registry_update, ratify_governance_amendment_with_lifecycle,
    ratify_validator_set_amendment_with_lifecycle,
    verify_governance_amendment as verify_cobalt_governance_amendment,
    verify_governance_amendment_for_mode,
    verify_validator_registry_update as verify_cobalt_validator_registry_update, CobaltDomain,
    CobaltGovernanceMode, EssentialSubsetConfig, GovernanceAmendmentLifecycle,
    ValidatorRegistryUpdateRequest, VALIDATOR_REGISTRY_OP_ADMIT, VALIDATOR_REGISTRY_OP_REACTIVATE,
    VALIDATOR_REGISTRY_OP_REMOVE, VALIDATOR_REGISTRY_OP_ROTATE_KEY, VALIDATOR_REGISTRY_OP_SUSPEND,
};
#[cfg(test)]
use postfiat_crypto_provider::MlDsa65KeyPair;
use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, hash_bytes, hash_hex, hex_to_bytes, ml_dsa_65_keygen,
    ml_dsa_65_keygen_from_seed, ml_dsa_65_sign, ml_dsa_65_sign_with_context,
    ml_dsa_65_sign_with_context_seed, ml_dsa_65_validate_public_key, ml_dsa_65_verify,
    ml_dsa_65_verify_with_context, BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
    BRIDGE_WITNESS_SIGNATURE_CONTEXT, ML_DSA_65_ALGORITHM, ML_DSA_65_PUBLIC_KEY_BYTES,
    ML_DSA_65_SIGNATURE_BYTES, TX_SIGNATURE_CONTEXT,
};
use postfiat_execution::fastlane_primary::execute_fastlane_primary_transaction;
use postfiat_execution::{
    asset_transaction_state_expansion_fee, asset_transaction_tx_id, asset_transaction_weight_bytes,
    atomic_swap_leg_state_expansion_fee, atomic_swap_transaction_tx_id,
    atomic_swap_transaction_weight_bytes, credit_issued_asset_from_shielded_pool,
    escrow_transaction_state_expansion_fee, escrow_transaction_tx_id,
    escrow_transaction_weight_bytes, execute_asset_transaction_with_compatibility,
    execute_asset_transaction_with_replay_compatibility,
    execute_asset_transaction_with_replay_preimage_and_compatibility,
    execute_atomic_swap_transaction_with_compatibility, execute_escrow_transaction,
    execute_nft_transaction, execute_offer_transaction, execute_payment_v2, execute_transfer,
    genesis_hash, issued_asset_supply, minimum_asset_transaction_fee, minimum_atomic_swap_fee,
    minimum_escrow_transaction_fee, minimum_nft_transaction_fee, minimum_offer_transaction_fee,
    minimum_payment_v2_fee, minimum_transfer_fee, minimum_transfer_fee_for_ledger,
    nft_transaction_state_expansion_fee, nft_transaction_tx_id, nft_transaction_weight_bytes,
    nft_transfer_issuer_fee_terms, offer_transaction_estimated_cross_count,
    offer_transaction_match_fee, offer_transaction_state_expansion_fee, offer_transaction_tx_id,
    offer_transaction_weight_bytes, offer_transaction_will_create_residual_offer,
    payment_v2_state_expansion_fee, payment_v2_tx_id, payment_v2_weight_bytes,
    pftl_uniswap_route_state_hash, transfer_state_expansion_fee, transfer_tx_id,
    transfer_weight_bytes, validate_atomic_swap_market_binding, AssetExecutionCompatibility,
    ACCOUNT_RESERVE, FEE_COLLECTOR_ADDRESS, MIN_TRANSFER_FEE, TRANSFER_ACCOUNT_CREATION_FEE,
    TRANSFER_FEE_BYTE_QUANTUM, TRANSFER_FEE_PER_QUANTUM,
};
#[cfg(test)]
use postfiat_execution::{execute_asset_transaction, TRUSTLINE_STATE_EXPANSION_FEE};
use postfiat_mempool_dag::{
    attach_fastlane_primary_transactions, build_mixed_transaction_batch_with_atomic_swaps,
    build_mixed_transaction_batch_with_offers, build_transaction_batch, reference_for_batch,
    verify_batch_payload, MempoolBatchDomain, MAX_BATCH_TRANSACTIONS,
};
use postfiat_network::{local_topology, remote_topology, NetworkDomain, NetworkTopology};
use postfiat_ordering_fast::{
    bft_quorum_threshold, certify_timeout, consensus_v2_genesis_parent_id, leader_for_view,
    next_reference, verify_timeout_certificate, ConsensusDomain, TimeoutVote, ValidatorSet,
};
#[cfg(test)]
use postfiat_privacy::mint_debug_note_with_creator;
use postfiat_privacy::{
    debug_note_commitment, debug_note_id, debug_note_rho, debug_nullifier,
    debug_shielded_pool_enabled_for_chain, debug_turnstile_event_id, disclose_note,
    migrate_debug_note, mint_debug_note_with_creator_for_chain, note_tree_root, scan_owner,
    spend_debug_note_for_chain, turnstile_summary, ShieldedError, TRANSPARENT_BOOTSTRAP_POOL_ID,
};
use postfiat_privacy_orchard::DEFAULT_MAX_ORCHARD_ACTIONS;
use postfiat_privacy_orchard::{
    asset_orchard_accounting_commitment_sum, asset_orchard_domain_genesis_hash,
    asset_orchard_scalar_from_hex, asset_orchard_wallet_note_nullifier,
    build_asset_orchard_disclosed_egress_authorization, build_asset_orchard_private_egress_action,
    build_asset_orchard_swap_action, build_asset_orchard_wallet_note,
    decrypt_asset_orchard_wallet_note, encrypt_asset_orchard_wallet_note,
    orchard_anchor_from_commitments, orchard_authorizing_sighash_with_external_binding,
    orchard_build_output_action, orchard_build_output_action_test_vector,
    orchard_build_output_action_with_external_binding, orchard_build_spend_action,
    orchard_build_withdraw_action, orchard_bundle_from_action,
    orchard_default_address_from_full_viewing_key, orchard_default_address_from_spending_key,
    orchard_empty_anchor, orchard_frontier_snapshot_append_commitments,
    orchard_frontier_snapshot_from_commitments, orchard_full_viewing_key_from_spending_key,
    orchard_merkle_witness_from_commitments, orchard_scan_encrypted_outputs_with_full_viewing_key,
    orchard_scan_encrypted_outputs_with_spending_key, orchard_spending_key_from_zip32_seed,
    reset_asset_orchard_private_egress_timings, reset_asset_orchard_swap_timings,
    take_asset_orchard_private_egress_timings, take_asset_orchard_swap_timings,
    verify_asset_orchard_disclosed_egress, verify_serialized_asset_orchard_private_egress_action,
    verify_serialized_asset_orchard_private_egress_action_for_archive_replay,
    verify_serialized_asset_orchard_swap_action,
    verify_serialized_asset_orchard_swap_action_for_archive_replay,
    verify_serialized_orchard_action_with_built_key, verify_serialized_shielded_swap_action,
    AssetOrchardBoundedBytes, AssetOrchardDisclosedEgressCheck,
    AssetOrchardDisclosedEgressPreimage, AssetOrchardFieldElement, AssetOrchardPoint,
    AssetOrchardPrivateEgressAction, AssetOrchardPrivateEgressTimingReport, AssetOrchardProofBytes,
    AssetOrchardPublicNoteOpening, AssetOrchardSpendAuthSignature,
    AssetOrchardSwapAccountingRecord, AssetOrchardSwapAction, AssetOrchardSwapBindingHash,
    AssetOrchardSwapTimingReport, AssetOrchardWalletNote, AssetTag, BoundedHexBlob,
    EncryptedShieldedOutput, OrchardAnchor, OrchardAuthorizingDomain, OrchardDecryptedOutput,
    OrchardFrontierSnapshot, OrchardNullifier, OrchardOutputCommitment, OrchardProofBytes,
    OrchardShieldedAction, OrchardSpendNote, ShieldedSwapAction, VerifiedAssetOrchardSwap,
    VerifiedOrchardBundle, VerifiedShieldedSwap, ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES,
    ASSET_ORCHARD_FIELD_BYTES, ASSET_ORCHARD_LEG_COUNT, ASSET_ORCHARD_MAX_ASSET_ID_BYTES,
    ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC, ASSET_ORCHARD_POOL_ID_V1, ORCHARD_ANCHOR_BYTES,
    ORCHARD_CIPHERTEXT_MAX_BYTES, ORCHARD_COMMITMENT_BYTES, ORCHARD_COMPACT_CIPHERTEXT_BYTES,
    ORCHARD_ENC_CIPHERTEXT_BYTES, ORCHARD_EPK_BYTES, ORCHARD_MEMO_BYTES, ORCHARD_NULLIFIER_BYTES,
    ORCHARD_OUT_CIPHERTEXT_BYTES, ORCHARD_PROOF_MAX_BYTES, ORCHARD_RAW_ADDRESS_BYTES,
    ORCHARD_REDPALLAS_SIGNATURE_BYTES, SHIELDED_SWAP_COMMITMENT_BYTES,
};
use postfiat_storage::{
    atomic_write, atomic_write_checked, NodeStore, StorageMutationLock, BATCH_ARCHIVE_APPEND_FILE,
    BATCH_ARCHIVE_FILE, BLOCKS_APPEND_FILE, BLOCKS_FILE, BRIDGE_FILE, GENESIS_FILE,
    GOVERNANCE_FILE, LEDGER_FILE, MEMPOOL_FILE, NODE_STATE_FILE, ORDERED_BATCHES_FILE,
    RECEIPTS_FILE, SHIELDED_FILE,
};
#[cfg(test)]
use postfiat_types::ShieldMintAction;
use postfiat_types::{
    bridge_exit_merkle_root_v1, issued_asset_id, market_ops_asset_id, market_ops_evidence_root,
    market_ops_reserve_packet_hash, market_ops_supply_packet_hash, nav_proof_profile_id,
    nav_proof_profile_id_with_bridge_observer_min_confirmations,
    vault_bridge_counted_value_for_asset, vault_bridge_deposit_evidence_root,
    vault_bridge_deposit_id, vault_bridge_deposit_observation_root,
    vault_bridge_deposit_public_values_hash, vault_bridge_pftl_recipient_hash,
    vault_bridge_route_binding, vault_bridge_source_root_for_asset, Account, AssetBurnOperation,
    AssetCreateOperation, AssetDefinition, AssetOrchardAssetBalance,
    AssetOrchardEgressActionPayload, AssetOrchardEncryptedOutputRecord,
    AssetOrchardIngressActionPayload, AssetOrchardIngressNote, AssetOrchardIngressV2ActionPayload,
    AssetOrchardPrivateEgressActionPayload, AssetTransactionOperation, AtomicSettlementTemplate,
    AtomicSettlementTemplateLeg, BatchArchive, BatchArchiveEntry, BlockCertificate,
    BlockCertificateVote, BlockHeader, BlockLog, BlockRecord, BridgeAction, BridgeActionBatch,
    BridgeDomain, BridgeDomainAction, BridgeDomainSpec, BridgeExitLeafV1, BridgePauseAction,
    BridgeState, BridgeTransfer, BridgeTransferAction, BridgeWitnessAttestation, ChainTipState,
    DeploymentRuntimeArtifactHashes, DeploymentServiceArtifact, Escrow, EscrowTransactionOperation,
    FastLaneReserveBalanceV1, FinalizedMarketOpsEnvelope, Genesis, GovernanceActionBatch,
    GovernanceAgentDryRunAmendment, GovernanceAgentDryRunRecord, GovernanceAmendment,
    GovernanceAmendmentActivationRecord, GovernanceAmendmentRollbackRecord,
    GovernanceAmendmentSupersessionRecord, GovernanceState, GovernanceVote, LedgerState,
    MarketOpsEnvelope, MarketOpsFinalizeOperation, MarketOpsPolicyInputs,
    MarketOpsPolicyRegisterOperation, MarketOpsPolicyRegistration, MarketOpsVenueObservation,
    MempoolAssetTransactionEntry, MempoolAtomicSwapEntry, MempoolEntry,
    MempoolEscrowTransactionEntry, MempoolFastLanePrimaryEntry, MempoolNftTransactionEntry,
    MempoolOfferTransactionEntry, MempoolPaymentV2Entry, MempoolState, NavAssetRegisterOperation,
    NavAttestor, NavProfileRegisterOperation, NavProofProfile, NavRedemption,
    NavReserveAttestation, NavReservePacket, NavTrackedAsset, NftDefinition,
    NftTransactionOperation, NodeState, Offer, OfferTransactionOperation, OrchardActionPayload,
    OrchardAssetCommitmentRecord, OrchardDepositActionPayload, OrchardEncryptedOutputRecord,
    OrchardFrontierCache, OrchardPoolState, OrchardRootRecord, OrchardWithdrawActionPayload,
    OwnedObject, PaymentMemo, PfUsdcEgressFinalityStepV1, PfUsdcEgressProofWitnessV1,
    PftlUniswapConsensusExportPacket, PftlUniswapConsensusReceipt,
    PftlUniswapConsensusReturnImport, PftlUniswapConsensusRouteState, Receipt, ShieldMigrateAction,
    ShieldedAction, ShieldedActionBatch, ShieldedDisclosure, ShieldedNote, ShieldedSpendResult,
    ShieldedState, ShieldedSwapActionPayload, SignedAssetTransaction, SignedAtomicSwapTransaction,
    SignedEscrowTransaction, SignedGovernanceAuthorizationV2, SignedNftTransaction,
    SignedOfferTransaction, SignedPaymentV2, SignedTransfer, SnapshotFile, SnapshotManifest,
    StatusReport, TransactionBatch, TrustLine, TrustSetOperation, TurnstileEvent, TurnstileSummary,
    UnsignedAssetTransaction, UnsignedEscrowTransaction, UnsignedNftTransaction,
    UnsignedOfferTransaction, UnsignedPaymentV2, UnsignedTransfer, ValidatorRegistryEntry,
    ValidatorRegistryUpdateRecord, VaultBridgeAllocation, VaultBridgeBucketState,
    VaultBridgeBurnToRedeemOperation, VaultBridgeDepositAttestOperation,
    VaultBridgeDepositAttestation, VaultBridgeDepositClaimOperation, VaultBridgeDepositEvidence,
    VaultBridgeDepositFinalizeOperation, VaultBridgeDepositObservation,
    VaultBridgeDepositProposeOperation, VaultBridgeDepositRecord, VaultBridgeReceipt,
    VaultBridgeRedemption, VaultBridgeWithdrawalExecutionAttestation,
    VaultBridgeWithdrawalExecutionObservation, VaultBridgeWithdrawalPacket, ADDRESS_NAMESPACE,
    ASSET_BURN_TRANSACTION_KIND, ATOMIC_SWAP_TRANSACTION_KIND, BRIDGE_DIRECTION_INBOUND,
    BRIDGE_DIRECTION_OUTBOUND, BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE, DEBUG_SHIELDED_POOL_ID,
    DEFAULT_SHIELDED_ASSET_ID, ESCROW_ID_HEX_LEN, ESCROW_STATE_CANCELED, ESCROW_STATE_FINISHED,
    ESCROW_STATE_OPEN, GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE,
    GOVERNANCE_AGENT_DRY_RUN_AMENDMENT_SCHEMA, GOVERNANCE_AGENT_DRY_RUN_RECORD_SCHEMA,
    GOVERNANCE_AMENDMENT_ACTIVATION_SCHEMA, GOVERNANCE_AMENDMENT_ROLLBACK_SCHEMA,
    GOVERNANCE_AMENDMENT_SUPERSESSION_SCHEMA, GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, GOVERNANCE_KIND_AUTHORITY_MODE,
    GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT, GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH,
    GOVERNANCE_KIND_CRYPTO_POLICY, GOVERNANCE_KIND_ORCHARD_POOL_PAUSE,
    GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT, GOVERNANCE_KIND_VALIDATOR_SET,
    ISSUED_ASSET_ID_HEX_LEN, MAX_NFT_COLLECTION_ID_BYTES, MAX_TEXT_FIELD_BYTES,
    NAV_PROFILE_ID_DOMAIN, NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1,
    NAV_PROFILE_VERIFIER_SP1_GROTH16, NAV_RESERVE_STATE_FINALIZED, NFT_COLLECTION_FLAG_BURN_LOCKED,
    NFT_COLLECTION_FLAG_TRANSFER_LOCKED, NFT_FLAG_ISSUER_BURNABLE, NFT_FLAG_TRANSFERABLE,
    NFT_ID_HEX_LEN, OFFER_ID_HEX_LEN, OFFER_OBJECT_RESERVE, OFFER_STATE_CANCELED,
    OFFER_STATE_FILLED, OFFER_STATE_OPEN, OFFER_STATE_UNFUNDED, OFFER_TX_ROLE_CANCEL,
    OFFER_TX_ROLE_MAKER, OFFER_TX_ROLE_TAKER, PAYMENT_V2_TRANSACTION_KIND,
    PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED, PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED,
    PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED, PFTL_UNISWAP_RETURN_STATUS_IMPORTED,
    PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1, SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2,
    SNAPSHOT_VERSION, TRANSFER_TRANSACTION_KIND, TURNSTILE_KIND_BOOTSTRAP_DEPOSIT,
    TURNSTILE_KIND_ORCHARD_DEPOSIT, TURNSTILE_KIND_POOL_MIGRATION,
    VAULT_BRIDGE_BUCKET_STATUS_ACTIVE, VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED,
    VAULT_BRIDGE_REDEMPTION_STATE_PENDING,
};
use postfiat_types::{AtomicSwapAuthorization, AtomicSwapLeg, UnsignedAtomicSwapTransaction};
use serde::{de::DeserializeOwned, Deserialize, Serialize};

mod lifecycle_queries;
pub use lifecycle_queries::*;
mod atomic_swap_rpc;
pub use atomic_swap_rpc::*;
#[allow(unused_imports)]
use lifecycle_queries::{
    asset_execution_compatibility_from_store, asset_line_report, asset_metrics,
    atomic_settlement_template_leg_report, compare_offer_book_reports,
    current_replicated_state_root, deployment_runtime_identity_from_config,
    deployment_runtime_identity_from_env, escrow_report, issued_asset_line_reports,
    issued_asset_open_offer_stats, issued_asset_report, issued_asset_stats, logical_tip_hash,
    metric_add, metric_increment, next_block_height_from_chain_tip, nft_report,
    node_timing_elapsed_ms, normalize_nft_fee_quote_operation, offer_report,
    read_history_checkpoint_state_optional, record_asset_orchard_private_egress_state_apply_timing,
    record_local_proof_verify_latency, truncate_reports, validate_atomic_settlement_assets_exist,
    validate_dex_asset_query_id, validate_escrow_query_id, validate_escrow_role_filter,
    validate_escrow_state_filter, validate_issued_asset_query_id, validate_lower_hex_len,
    validate_nft_collection_query_id, validate_nft_query_id, validate_offer_query_id,
    validate_offer_state_filter, validate_owner_public_key_hex, validate_query_text_field,
    DeploymentRuntimeIdentity, BATCH_KIND_BRIDGE, BATCH_KIND_GOVERNANCE, BATCH_KIND_SHIELDED,
    BATCH_KIND_TRANSPARENT, BLOCK_CERTIFICATE_FILE_SCHEMA, BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA,
    BLOCK_PROPOSAL_FILE_SCHEMA, BLOCK_PROPOSAL_SIGNATURE_CONTEXT,
    BLOCK_TIMEOUT_CERTIFICATE_FILE_SCHEMA, BLOCK_TIMEOUT_SIGNATURE_CONTEXT,
    BLOCK_TIMEOUT_VOTE_FILE_SCHEMA, BLOCK_VOTE_FILE_SCHEMA, DEPLOYMENT_MANIFEST_SCHEMA,
    DEPLOYMENT_MANIFEST_SIGNATURE_CONTEXT, DEPLOYMENT_PUBLISHER_KEY_PURPOSE,
    DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT, DEPLOYMENT_PUBLISHER_PRIVATE_KEY_SCHEMA,
    DEPLOYMENT_PUBLISHER_PUBLIC_KEY_SCHEMA, DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA,
    DEPLOYMENT_VALIDATOR_UNIT_STAGE_SCHEMA, DEV_KEY_SELF_CHECK_CONTEXT, DEV_KEY_SELF_CHECK_SEED,
    MAX_LOCAL_JSON_FILE_BYTES, MAX_OPERATOR_MANIFEST_TEXT_BYTES,
    OPERATOR_MANIFEST_SIGNATURE_CONTEXT, SIGNED_SNAPSHOT_MANIFEST_FILE,
    SIGNED_SNAPSHOT_MANIFEST_SCHEMA, SNAPSHOT_FILES, SNAPSHOT_MANIFEST_SIGNATURE_CONTEXT,
    SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA, VALIDATOR_KEY_SELF_CHECK_CONTEXT,
    VALIDATOR_KEY_SELF_CHECK_SEED,
};
#[cfg(test)]
use lifecycle_queries::{
    build_history_checkpoint_state, build_history_prune_artifacts, read_history_prune_journal,
    write_history_checkpoint_state_file, write_history_prune_pending_file,
};
mod mempool_proposals;
pub use mempool_proposals::{
    admit_fastlane_primary_to_mempool, create_mempool_batch, create_signed_asset_transaction_batch,
    create_transfer_batch, mempool_state, propose_batch,
    propose_batch_with_required_parent_with_timings, propose_batch_with_timings,
    reconcile_terminal_mempool_entries, submit_signed_asset_transaction_json_to_mempool,
    submit_signed_escrow_transaction_json_to_mempool,
    submit_signed_nft_transaction_json_to_mempool, submit_signed_offer_transaction_json_to_mempool,
    submit_signed_payment_v2_json_to_mempool, submit_signed_transfer_json_to_mempool,
    submit_signed_transfer_to_mempool, submit_transfer_to_mempool, transfer, verify_mempool,
};
use mempool_proposals::{admit_signed_atomic_swap_to_mempool, enforce_mempool_state_limits};
use mempool_proposals::{
    build_ordered_batch_proposal_with_timings, ledger_after_executable_mempool,
    mempool_pending_count_for_sender, normalize_block_proposal_batch_kind,
};
#[cfg(test)]
use mempool_proposals::{enforce_mempool_admission_limits, verify_mempool_state};
mod batch_snapshot;
mod market_bridge;
mod pfusdc_tier4;
mod vault_bridge_conservation;
mod vault_bridge_workflows;
#[cfg(test)]
use batch_snapshot::read_deployment_publisher_private_key;
pub use batch_snapshot::*;
use batch_snapshot::{
    apply_batch_elapsed_ms, build_block_proposal_from_state,
    read_deployment_validator_bindings_file, sha256_file_hex, validate_deployment_identifier,
    BlockProposalPlan,
};
pub use market_bridge::*;
pub use pfusdc_tier4::*;
pub use vault_bridge_conservation::*;
pub use vault_bridge_workflows::*;
use vault_bridge_workflows::{
    build_market_ops_public_status, derive_market_ops_operation_nonce,
    issued_asset_supply_for_status, market_ops_operation_relay_commands,
    market_ops_operation_relay_commands_script, market_ops_parse_hex_array,
    market_ops_replay_bundle_file, recompute_market_ops_replay_envelope,
    replay_market_ops_bundle_data, vault_bridge_append_abi_address,
    vault_bridge_append_abi_dynamic_bytes, vault_bridge_append_abi_u256_u64,
    vault_bridge_evm_address_bytes, vault_bridge_hex_bytes_exact, vault_bridge_keccak256,
    vault_bridge_write_json_file, EVM_ABI_WORD_BYTES,
};
include!("governance.rs");
include!("governance_agent.rs");
include!("privacy.rs");
include!("block_finality.rs");
mod block_replay_wallet;
pub use block_replay_wallet::*;
#[allow(unused_imports)]
use block_replay_wallet::{
    activate_due_validator_registry_updates_for_commit,
    activate_validator_registry_updates_for_height, archived_transparent_legacy_batch_id_allowed,
    archived_wan_devnet2_legacy_receipt_id_drift_allowed,
    archived_wan_devnet2_pre_pricing_swap_allowed,
    archived_wan_devnet2_pre_repin_private_egress_allowed,
    archived_wan_devnet_legacy_cash_omitted_sp1_nav_allowed,
    archived_wan_devnet_legacy_domainless_withdrawal_packet_emit_allowed,
    archived_wan_devnet_legacy_nav_profile_id_allowed,
    archived_wan_devnet_legacy_nav_profile_id_schema_allowed,
    archived_wan_devnet_legacy_strict_domain_validation_allowed,
    backfill_legacy_validator_registry_records, governance_with_due_validator_registry_activations,
    legacy_asset_transaction_unsigned_prefix, legacy_nav_profile_id_without_empty_sp1_fields,
    legacy_nav_profile_register_signing_byte_candidates, legacy_nav_profile_register_signing_bytes,
    legacy_nav_reserve_submit_signing_bytes_without_sp1_evidence_fields,
    live_validator_registry_after_due_updates, native_pft_fee_burn_total, parse_archived_payload,
    receipt_replay_summary, replay_archived_payload, replay_legacy_wan_devnet_nav_profile_ids,
    replayed_receipt_matches_persisted, update_governance_for_certificate_replay,
    validator_registry_update_can_live_apply, verified_legacy_wan_asset_transaction_signing_bytes,
    verify_archived_payload, verify_archived_payload_id, verify_archived_payload_receipt_count,
    verify_archived_transparent_batch_id, verify_native_pft_transition, verify_replayed_blocks,
    ArchivedReplayState, DueValidatorRegistryActivations, WAN_DEVNET2_PRE_PRICING_SWAP_BATCHES,
    WAN_DEVNET2_PRE_REPIN_PRIVATE_EGRESS_BATCHES,
    WAN_DEVNET_LEGACY_CASH_OMITTED_SP1_NAV_MAX_HEIGHT,
    WAN_DEVNET_LEGACY_DOMAINLESS_WITHDRAWAL_BATCHES,
    WAN_DEVNET_LEGACY_NAV_PROFILE_ID_SCHEMA_MAX_HEIGHT, WAN_DEVNET_LEGACY_NAV_REPLAY_MAX_HEIGHT,
    WAN_DEVNET_LEGACY_STRICT_DOMAIN_VALIDATION_HEIGHT,
};
mod state_commitment;
pub use state_commitment::global_issued_asset_supply;
#[allow(unused_imports)]
use state_commitment::{
    append_account, append_asset_definition, append_asset_orchard_asset_balance,
    append_asset_orchard_encrypted_output_record, append_bridge_domain, append_bridge_state,
    append_bridge_transfer, append_bridge_witness_attestation, append_canonical_bool,
    append_canonical_bytes_commitment, append_canonical_i64, append_canonical_str,
    append_canonical_u128, append_canonical_u32, append_canonical_u64, append_canonical_u8,
    append_canonical_usize, append_escrow, append_finalized_market_ops_envelope,
    append_governance_activation_record, append_governance_agent_dry_run_record,
    append_governance_amendment, append_governance_rollback_record, append_governance_state,
    append_governance_supersession_record, append_governance_vote, append_ledger_state,
    append_market_ops_envelope, append_market_ops_observations, append_market_ops_policy,
    append_market_ops_policy_inputs, append_nav_attestor, append_nav_proof_profile,
    append_nav_redemption, append_nav_reserve_attestation, append_nav_reserve_packet,
    append_nav_tracked_asset, append_nft, append_offer, append_option_str, append_option_u64,
    append_option_validator_registry_entry, append_orchard_asset_commitment_record,
    append_orchard_encrypted_output, append_orchard_pool_state, append_orchard_root_record,
    append_owned_object, append_pftl_uniswap_export_packet, append_pftl_uniswap_receipt,
    append_pftl_uniswap_return_import, append_pftl_uniswap_route, append_shielded_note,
    append_shielded_state, append_string_list, append_trustline, append_turnstile_event,
    append_u128_list, append_validator_registry_entry, append_validator_registry_update_record,
    append_vault_bridge_allocation, append_vault_bridge_bucket,
    append_vault_bridge_deposit_attestation, append_vault_bridge_deposit_evidence,
    append_vault_bridge_deposit_observation, append_vault_bridge_deposit_record,
    append_vault_bridge_receipt, append_vault_bridge_redemption,
    append_vault_bridge_route_profile_record, append_vault_bridge_withdrawal_execution_attestation,
    append_vault_bridge_withdrawal_execution_observation, append_vault_bridge_withdrawal_packet,
    archived_wan_devnet_legacy_nav_asset_commitment_allowed,
    bridge_verification_legacy_replay_allowed,
    legacy_domainless_vault_bridge_withdrawal_packet_evm_digest,
    legacy_domainless_vault_bridge_withdrawal_packet_hash, legacy_json_replicated_state_root,
    legacy_nav_asset_uncommitted_replicated_state_root,
    legacy_nav_incomplete_replicated_state_root,
    legacy_nav_profile_sp1_uncommitted_replicated_state_root,
    legacy_vault_bridge_deposit_attestation_replicated_state_root,
    legacy_vault_bridge_domainless_withdrawal_replicated_state_root, replicated_state_root,
    replicated_state_root_with_nav_completeness, verify_global_issued_asset_supply_caps,
    LegacyJsonGovernanceState, LegacyJsonLedgerState, LegacyJsonShieldedState,
};
mod execution_actions;
#[allow(unused_imports)]
use execution_actions::{
    apply_archived_wan_devnet2_pre_pricing_swap, apply_governance_amendment_with_lifecycle_records,
    archived_pre_pricing_swap_execution_allowed,
    archived_pre_repin_private_egress_execution_allowed,
    asset_execution_compatibility_for_genesis_and_governance,
    asset_execution_compatibility_with_chain_activation, asset_orchard_nav_ratio_denominator,
    atomic_swap_activation_height_for_chain, bridge_exit_root_activation_height_for_chain,
    bridge_verification_activation_height_for_chain, bridge_witness_registry_error,
    debug_shielded_pool_disabled_receipt, ensure_atomic_swap_batch_allowed,
    ensure_governance_batch_lifecycle_ready, execute_asset_orchard_egress_action,
    execute_asset_orchard_ingress_action, execute_asset_orchard_private_egress_action,
    execute_asset_transaction_for_archive_replay, execute_bridge_batch, execute_governance_batch,
    execute_orchard_deposit_shielded_action, execute_orchard_shielded_action,
    execute_orchard_withdraw_shielded_action, execute_shielded_batch, execute_shielded_swap_action,
    execute_transparent_batch, execute_transparent_batch_for_archive_replay,
    expected_governance_amendment_rollbacks, expected_governance_amendment_supersessions,
    governance_agent_dry_run_amendment_id, governance_agent_dry_run_record,
    governance_agent_dry_run_record_id, governance_agent_dry_run_rejection,
    governance_amendment_activation_record, governance_amendment_activation_record_id,
    governance_amendment_current_value, governance_amendment_lifecycle_rejection,
    governance_amendment_rollback_record, governance_amendment_rollback_record_id,
    governance_amendment_supersession_record, governance_amendment_supersession_record_id,
    validate_asset_orchard_swap_pricing_against_ledger,
    validate_governance_agent_dry_run_amendment, validate_governance_agent_dry_run_record,
    validate_vault_bridge_route_profile_against_ledger,
    verify_governance_amendment_activation_record, verify_governance_amendment_activation_records,
    verify_governance_amendment_rollback_record,
    verify_governance_amendment_rollback_record_for_domain,
    verify_governance_amendment_rollback_records, verify_governance_amendment_supersession_record,
    verify_governance_amendment_supersession_record_for_domain,
    verify_governance_amendment_supersession_records, ArchivedAssetOrchardSwapReplayAction,
    ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT,
};
mod storage_commit;
pub use storage_commit::*;
#[allow(unused_imports)]
use storage_commit::{
    apply_historical_validator_registry_update_to_registry, apply_ordered_commit_delta_journal,
    apply_ordered_commit_delta_journal_timed, apply_ordered_commit_journal,
    apply_ordered_commit_journal_timed, apply_stored_ordered_commit_journal,
    apply_validator_registry_update_to_registry,
    apply_verified_validator_registry_update_to_registry,
    apply_verified_validator_registry_update_to_registry_for_domain,
    apply_verified_validator_registry_update_to_registry_inner, batch_archive_payload_hash,
    bridge_witness_chain_domain_error, build_signed_transfer, build_signed_transfer_for_key,
    chain_tip_after_delta, cobalt_domain_from_validator_registry_update, create_dev_key_file,
    create_validator_key_record, derive_wallet_dev_key_file, derive_wallet_seed,
    dev_key_self_check_message, empty_apply_batch_prepare_timing_report,
    empty_apply_batch_write_timing_report, ensure_output_can_be_written,
    ensure_registry_record_matches_entry, ensure_validator_keys,
    ensure_validator_keys_for_validators, ensure_validator_registry_for_validators,
    ensure_validator_registry_genesis, fixed_32_byte_hex, historical_replay_commit_inputs,
    mutate_first_hex_nibble, next_pending_sender_sequence, normalized_wallet_master_seed_hex,
    ordered_commit_delta_journal, payment_v2_quote_memos, prepare_ordered_commit,
    prepare_ordered_commit_timed, quote_signed_asset_transaction, quote_signed_escrow_transaction,
    quote_signed_nft_transaction, quote_signed_offer_transaction, quote_signed_payment_v2,
    quote_signed_transfer, read_block_certificate_file, read_block_proposal_file,
    read_block_timeout_certificate_file, read_block_timeout_vote_file, read_block_vote_file,
    read_chain_tip_or_reconstruct_for_genesis, read_commit_certificate_material,
    read_faucet_account_file, read_key_file, read_orchard_view_key_file,
    read_orchard_wallet_key_file, read_transfer_key_file, read_validator_key_file,
    read_validator_registry_file, read_validator_registry_replay_base, read_wallet_backup_file,
    receipts_for_block, reconstruct_chain_tip_for_genesis, recover_ordered_commit_journal,
    recover_ordered_commit_journal_locked, registry_validator_index, sort_validator_key_records,
    sort_validator_registry_records, validate_chain_tip_domain, validate_dev_key_file,
    validate_faucet_account, validate_orchard_view_key_file, validate_orchard_wallet_key_file,
    validate_validator_key_file, validate_validator_registry,
    validate_validator_registry_for_count, validate_wallet_backup_file, validator_index,
    validator_key_record, validator_key_self_check_message,
    validator_registry_from_keys_for_validators, validator_registry_record,
    validator_registry_record_from_entry, validator_registry_root,
    validator_registry_subset_for_validators, validator_registry_update_new_validators,
    validator_registry_update_previous_validators,
    verify_historical_cobalt_validator_registry_update, wallet_key_report,
    wallet_master_seed_bytes, wallet_test_vector_signature_seed_bytes,
    write_block_certificate_file, write_block_equivocation_evidence_file,
    write_block_proposal_file, write_block_timeout_certificate_file, write_block_timeout_vote_file,
    write_block_vote_file, write_faucet_account_file, write_key_file, write_orchard_view_key_file,
    write_orchard_wallet_key_file, write_ordered_commit_with_journal_locked,
    write_ordered_commit_with_journal_timed_locked, write_validator_key_file,
    write_validator_registry_file, write_wallet_backup_file, BlockEvidence,
    CommitCertificateMaterial, HistoricalBlockReplay, OrderedCommitArtifacts,
    OrderedCommitDeltaJournal, OrderedCommitJournal, OrderedCommitPlan, OrderedCommitWithTimings,
    OrderedCommitWrite, OwnedBlockEvidence, StoredOrderedCommitJournal, CHAIN_TIP_SCHEMA,
};
mod consensus_artifacts;
mod consensus_v2_finality;
mod consensus_v2_store;
mod ethereum_checkpoint_signing;
mod ethereum_receipt_proof_builder;
mod fastpay_recovery_node;
#[cfg(test)]
use consensus_artifacts::write_signed_transfer_file;
pub use consensus_artifacts::*;
#[allow(unused_imports)]
use consensus_artifacts::{
    active_validator_ids, block_certificate, block_certificate_id, block_certificate_quorum,
    block_certificate_signature_seed, block_certificate_vote_id, block_certificate_vote_message,
    block_hash, block_proposal_signature_message, block_proposal_signature_seed,
    block_timeout_certificate_id, block_timeout_signature_seed, block_timeout_vote_id,
    block_timeout_vote_message, bridge_action_rejection_id, bridge_error,
    build_bridge_action_batch, build_governance_action_batch,
    build_governance_action_batch_with_agent_dry_runs,
    build_governance_action_batch_with_fastpay_recovery_bootstrap,
    build_governance_action_batch_with_fastswap_bootstraps,
    build_governance_action_batch_with_vault_bridge_route_profile_activation,
    build_shielded_action_batch, canonical_governance_replay_batch_payload_json,
    certificate_registry_root_or_legacy, chain_bound_action_batch_id,
    chain_bound_action_batch_id_for_genesis_hash, cobalt_domain,
    common_embedded_registry_update_genesis_hash, current_registry_id,
    decode_ml_dsa_65_public_key_hex, decode_ml_dsa_65_signature_hex,
    display_relative_artifact_path, governance_action_batch_id_matches_genesis_hash,
    governance_genesis_bundle_hash, governance_replay_path_reference, invalid_data,
    load_validator_pubkeys, local_validator_ids, mempool_batch_domain,
    operator_cobalt_trust_binding_from_options, operator_manifest_hash,
    operator_manifest_signing_payload, operator_manifest_signing_payload_bytes,
    ordered_shielded_mint_creator, read_amendment_file, read_batch_file,
    read_bounded_json_text_file, read_bridge_action_batch_file, read_governance_action_batch_file,
    read_governance_genesis_bundle_file, read_json_file, read_operator_manifest_file,
    read_shielded_action_batch_file, read_signed_transfer_file, read_snapshot_manifest,
    read_validator_registry_entry_file, read_validator_registry_update_file,
    record_pending_checkpoint, reject_operator_manifest_private_material,
    resolve_governance_genesis_path, resolve_governance_replay_path, set_active_validator_ids,
    set_private_file_permissions, shielded_action_rejection_id, shielded_error,
    shielded_state_error, tx_finality_proof_id, unix_now, validate_active_validator_ids,
    validate_block_certificate_vote_set, validate_block_timeout_vote_set, validate_finality_tx_id,
    validate_governance_genesis_cobalt_trust, validate_governance_genesis_manifest_ref,
    validate_governance_genesis_quorum, validate_hex_string, validate_manifest_text_field,
    validate_operator_cobalt_trust_binding, validate_operator_manifest_fields,
    validate_operator_manifest_fields_for_signing, validate_operator_manifest_for_genesis,
    validate_private_file_permissions, validate_snapshot_manifest_files,
    verify_archived_governance_action_batch_id, verify_block_certificate_evidence,
    verify_block_certificate_vote, verify_block_certificate_vote_for_evidence,
    verify_block_timeout_certificate_material, verify_block_timeout_vote_for_target,
    verify_bridge_action_batch_id, verify_external_block_certificate,
    verify_external_block_certificate_timed, verify_governance_action_batch_id,
    verify_governance_amendment_evidence, verify_hotstuff_timeout_certificate,
    verify_operator_manifest_record, verify_preverified_external_block_certificate_timed,
    verify_shielded_action_batch_id, write_amendment_file, write_batch_file,
    write_bridge_action_batch_file, write_governance_action_batch_file,
    write_governance_genesis_bundle_file, write_operator_manifest_file,
    write_shielded_action_batch_file, write_snapshot_manifest,
    write_validator_registry_update_file, BlockVoteTarget, GovernanceGenesisBundleHashPayload,
    OperatorManifestHashPayload, OperatorManifestSigningPayload,
};
pub use consensus_v2_finality::*;
pub use consensus_v2_store::*;
pub use ethereum_checkpoint_signing::*;
pub use ethereum_receipt_proof_builder::*;
pub use fastpay_recovery_node::*;

#[cfg(test)]
include!("lib_tests.rs");
