use super::*;

pub const DEFAULT_FAUCET_BALANCE: u64 = postfiat_types::GENESIS_NATIVE_SUPPLY_ATOMS;
pub const FAUCET_KEY_FILE: &str = "faucet_key.json";
pub const FAUCET_ACCOUNT_FILE: &str = "faucet_account.json";
pub const VALIDATOR_KEYS_FILE: &str = "validator_keys.json";
pub const VALIDATOR_REGISTRY_FILE: &str = "validator_registry.json";
pub const VALIDATOR_REGISTRY_GENESIS_FILE: &str = "validator_registry_genesis.json";
pub const SNAPSHOT_MANIFEST_FILE: &str = "snapshot_manifest.json";
pub const DEFAULT_BRIDGE_WITNESS_SIGNER: &str = "validator-0";
pub const MAX_MEMPOOL_PENDING_TRANSACTIONS: usize = 1024;
pub const MAX_MEMPOOL_PENDING_PER_SENDER: usize = 64;
pub const MAX_READ_QUERY_LIMIT: usize = 512;
pub const MAX_SIGNED_TRANSFER_JSON_BYTES: usize = 64 * 1024;
pub const WALLET_BACKUP_FILE_SCHEMA: &str = "postfiat-wallet-backup-v1";
pub const WALLET_KEY_REPORT_SCHEMA: &str = "postfiat-wallet-key-report-v1";
pub const WALLET_DERIVATION_DOMAIN: &str = "postfiat.wallet.seed.v1";
pub const WALLET_DERIVATION_KDF: &str = "sha3-384-domain-truncate32";
pub const WALLET_KEY_ROLE_TRANSPARENT_SPEND: &str = "transparent-spend";
pub const ORCHARD_WALLET_FILE_SCHEMA: &str = "postfiat-orchard-wallet-v1";
pub const ORCHARD_VIEW_KEY_FILE_SCHEMA: &str = "postfiat-orchard-view-key-v1";
pub const ORCHARD_WALLET_KEY_REPORT_SCHEMA: &str = "postfiat-orchard-wallet-key-report-v1";
pub const ORCHARD_VIEW_KEY_REPORT_SCHEMA: &str = "postfiat-orchard-view-key-report-v1";
pub const ORCHARD_OUTPUT_ACTION_REPORT_SCHEMA: &str = "postfiat-orchard-output-action-report-v1";
pub const ORCHARD_DEPOSIT_ACTION_FILE_SCHEMA: &str = "postfiat-orchard-deposit-action-file-v1";
pub const ORCHARD_DEPOSIT_ACTION_REPORT_SCHEMA: &str = "postfiat-orchard-deposit-action-report-v1";
pub const ASSET_ORCHARD_INGRESS_FILE_SCHEMA: &str = "postfiat-asset-orchard-ingress-file-v2";
pub const ASSET_ORCHARD_INGRESS_REPORT_SCHEMA: &str = "postfiat-asset-orchard-ingress-report-v1";
pub const ASSET_ORCHARD_EGRESS_FILE_SCHEMA: &str = "postfiat-asset-orchard-egress-file-v1";
pub const ASSET_ORCHARD_EGRESS_REPORT_SCHEMA: &str = "postfiat-asset-orchard-egress-report-v1";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA: &str =
    "postfiat-asset-orchard-private-egress-file-v1";
pub const ASSET_ORCHARD_PRIVATE_EGRESS_REPORT_SCHEMA: &str =
    "postfiat-asset-orchard-private-egress-report-v1";
pub const ASSET_ORCHARD_SWAP_CREATE_REPORT_SCHEMA: &str =
    "postfiat-asset-orchard-swap-create-report-v1";
pub const ASSET_ORCHARD_NOTE_STATUS_REPORT_SCHEMA: &str =
    "postfiat-asset-orchard-note-status-report-v1";
pub const ASSET_ORCHARD_SCAN_REPORT_SCHEMA: &str = "postfiat-asset-orchard-scan-report-v1";
pub const ORCHARD_SPEND_ACTION_REPORT_SCHEMA: &str = "postfiat-orchard-spend-action-report-v1";
pub const ORCHARD_WITHDRAW_ACTION_REPORT_SCHEMA: &str =
    "postfiat-orchard-withdraw-action-report-v1";
pub const ORCHARD_DISCLOSURE_PACKET_SCHEMA: &str = "postfiat-orchard-disclosure-packet-v1";
pub const ORCHARD_DISCLOSURE_VERIFY_REPORT_SCHEMA: &str =
    "postfiat-orchard-disclosure-verify-report-v1";
pub const ORCHARD_OPERATOR_POLICY_REPORT_SCHEMA: &str = "postfiat-orchard-operator-policy-v1";
pub const ORCHARD_FEE_RESOURCE_POLICY_REPORT_SCHEMA: &str =
    "postfiat-orchard-fee-resource-policy-v1";
pub const ORCHARD_POOL_REPORT_SCHEMA: &str = "postfiat-orchard-pool-report-v1";
pub const ORCHARD_FRONTIER_CACHE_WARM_REPORT_SCHEMA: &str =
    "postfiat-orchard-frontier-cache-warm-v1";
pub const ORCHARD_WALLET_DERIVATION_DOMAIN: &str = "postfiat.wallet.orchard.seed.v1";
pub const ORCHARD_WALLET_DERIVATION_KDF: &str = "sha3-384-domain-truncate32-retry-canonical";
pub const ORCHARD_DEFAULT_POOL_ID: &str = "orchard-v1";
pub const ORCHARD_DEPOSIT_POLICY_ID: &str = "postfiat.orchard.deposit.v1";
pub const ORCHARD_WITHDRAW_POLICY_ID: &str = "postfiat.orchard.withdraw.v1";
pub const ORCHARD_FEE_BURN_MIN_FEE: u64 = 2;
pub const ORCHARD_FEE_BURN_BYTE_QUANTUM: usize = 1_048_576;
pub const ORCHARD_FEE_BURN_FEE_PER_QUANTUM: u64 = 1;
pub const DEFAULT_ORCHARD_VERIFIER_MAX_CONCURRENCY: usize = 1;
pub const DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS: u64 = 30_000;
pub const DEFAULT_ORCHARD_ROOT_RETENTION: u64 = 50_000;
pub const OPERATOR_MANIFEST_FILE_SCHEMA: &str = "postfiat-operator-manifest-v1";
pub const GOVERNANCE_GENESIS_BUNDLE_SCHEMA: &str = "postfiat-governance-genesis-bundle-v1";
pub const GOVERNANCE_GENESIS_VERIFY_REPORT_SCHEMA: &str = "postfiat-governance-genesis-verify-v1";
pub const OPERATOR_MANIFEST_VERIFY_REPORT_SCHEMA: &str = "postfiat-operator-manifest-verify-v1";
pub const DEFAULT_HISTORY_MODE: &str = "partial";
pub const DEFAULT_HISTORY_RETAIN_RECENT_BLOCKS: u64 = 50_000;
pub const DEFAULT_HISTORY_RETAIN_RECENT_RECEIPTS: u64 = 50_000;
pub const DEFAULT_HISTORY_RETAIN_RECENT_BATCHES: u64 = 50_000;
pub const DEFAULT_HISTORY_RETAIN_RECENT_GOVERNANCE: u64 = 100_000;
pub const DEFAULT_HISTORY_MINIMUM_REPLAY_WINDOW_BLOCKS: u64 = 5_000;
pub const HISTORY_CHECKPOINT_FILE: &str = "history_checkpoint.json";
pub const HISTORY_PRUNE_PENDING_FILE: &str = "history_prune_pending.json";
pub const HISTORY_PRUNE_JOURNAL_FILE: &str = "history_prune_journal.json";
pub const HISTORY_ARCHIVE_INDEX_FILE: &str = "history_archive_index.json";
pub const ACCOUNT_TX_INDEX_FILE: &str = "account_tx_index.json";
pub const HISTORY_ARCHIVE_WINDOWS_DIR: &str = "history_archive_windows";
pub(super) const OWNED_LOCKS_FILE: &str = "owned_locks.json";
pub(super) const OWNED_LOCKS_WAL_FILE: &str = "owned_locks.wal";
pub(super) const FASTPAY_SPECULATIVE_JOURNAL_FILE: &str = "fastpay_speculative_effects_v1.json";
pub(super) const PROOF_LATENCY_METRICS_FILE: &str = "proof_latency_metrics.json";
pub(super) const PROOF_LATENCY_METRICS_SCHEMA: &str = "postfiat.proof_latency_metrics.v1";

pub(super) const MAX_LOCAL_JSON_FILE_BYTES: u64 = 8 * 1024 * 1024;
pub(super) const MAX_OPERATOR_MANIFEST_TEXT_BYTES: usize = 256;
pub(super) const OPERATOR_MANIFEST_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/operator-manifest/v1";
pub(super) const DEV_KEY_SELF_CHECK_CONTEXT: &[u8] = b"postfiat-l1-v2/dev-key-self-check/v1";
pub(super) const DEV_KEY_SELF_CHECK_SEED: [u8; 32] = [23u8; 32];
pub(super) const VALIDATOR_KEY_SELF_CHECK_CONTEXT: &[u8] =
    b"postfiat-l1-v2/validator-key-self-check/v1";
pub(super) const VALIDATOR_KEY_SELF_CHECK_SEED: [u8; 32] = [87u8; 32];
pub(super) const BLOCK_PROPOSAL_FILE_SCHEMA: &str = "postfiat.block_proposal.v1";
pub(super) const BLOCK_VOTE_FILE_SCHEMA: &str = "postfiat.block_vote.v1";
pub(super) const BLOCK_CERTIFICATE_FILE_SCHEMA: &str = "postfiat.block_certificate.v1";
pub(super) const BLOCK_TIMEOUT_VOTE_FILE_SCHEMA: &str = "postfiat.block_timeout_vote.v1";
pub(super) const BLOCK_TIMEOUT_CERTIFICATE_FILE_SCHEMA: &str =
    "postfiat.block_timeout_certificate.v1";
pub(super) const BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA: &str =
    "postfiat.block_equivocation_evidence.v1";
pub(super) const BLOCK_PROPOSAL_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/block-proposal/v1";
pub(super) const BLOCK_TIMEOUT_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/block-timeout/v1";
pub(super) const BATCH_KIND_TRANSPARENT: &str = "transparent";
pub(super) const BATCH_KIND_GOVERNANCE: &str = "governance";
pub(super) const BATCH_KIND_SHIELDED: &str = "shielded";
pub(super) const BATCH_KIND_BRIDGE: &str = "bridge";

pub(super) const SNAPSHOT_FILES: &[&str] = &[
    GENESIS_FILE,
    NODE_STATE_FILE,
    GOVERNANCE_FILE,
    LEDGER_FILE,
    BLOCKS_FILE,
    BATCH_ARCHIVE_FILE,
    ORDERED_BATCHES_FILE,
    RECEIPTS_FILE,
    MEMPOOL_FILE,
    SHIELDED_FILE,
    BRIDGE_FILE,
    FAUCET_ACCOUNT_FILE,
    VALIDATOR_REGISTRY_GENESIS_FILE,
    VALIDATOR_REGISTRY_FILE,
    CONSENSUS_V2_SAFETY_SNAPSHOT_FILE,
    CONSENSUS_V2_QC_SNAPSHOT_FILE,
    OWNED_LOCKS_FILE,
    OWNED_LOCKS_WAL_FILE,
    FASTPAY_SPECULATIVE_JOURNAL_FILE,
];
pub(super) const LEGACY_SNAPSHOT_VERSION: u32 = 5;
pub(super) const LEGACY_SNAPSHOT_FILES: &[&str] = &[
    GENESIS_FILE,
    NODE_STATE_FILE,
    GOVERNANCE_FILE,
    LEDGER_FILE,
    BLOCKS_FILE,
    BATCH_ARCHIVE_FILE,
    ORDERED_BATCHES_FILE,
    RECEIPTS_FILE,
    MEMPOOL_FILE,
    SHIELDED_FILE,
    BRIDGE_FILE,
    FAUCET_ACCOUNT_FILE,
    VALIDATOR_REGISTRY_GENESIS_FILE,
    VALIDATOR_REGISTRY_FILE,
];
pub(super) const CONSENSUS_V2_SAFETY_SNAPSHOT_FILE: &str = "consensus_v2_safety.snapshot.json";
pub(super) const CONSENSUS_V2_QC_SNAPSHOT_FILE: &str = "consensus_v2_qcs.snapshot.json";
pub(super) const SIGNED_SNAPSHOT_MANIFEST_FILE: &str = "snapshot.signed-manifest.json";
pub(super) const SIGNED_SNAPSHOT_MANIFEST_SCHEMA: &str = "postfiat.signed_snapshot_manifest.v1";
pub(super) const SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA: &str = "postfiat.snapshot_publisher_key.v1";
pub(super) const SNAPSHOT_MANIFEST_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/snapshot-manifest/v1";
pub(super) const DEPLOYMENT_MANIFEST_SCHEMA: &str = "postfiat.deployment_manifest.v2";
pub(super) const DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA: &str =
    "postfiat.deployment_validator_bindings.v1";
pub(super) const DEPLOYMENT_VALIDATOR_UNIT_STAGE_SCHEMA: &str =
    "postfiat.deployment_validator_unit_stage.v1";
pub(super) const DEPLOYMENT_PUBLISHER_PRIVATE_KEY_SCHEMA: &str =
    "postfiat.deployment_publisher_private_key.v1";
pub(super) const DEPLOYMENT_PUBLISHER_KEY_PURPOSE: &str = "deployment-manifest-publisher";
pub(super) const DEPLOYMENT_PUBLISHER_PUBLIC_KEY_SCHEMA: &str =
    "postfiat.deployment_publisher_key.v1";
pub(super) const DEPLOYMENT_MANIFEST_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/deployment-manifest/v1";
pub(super) const DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT: &[u8] =
    b"postfiat-l1-v2/deployment-publisher-key/v1";

include!("node_types.rs");

thread_local! {
    static ASSET_ORCHARD_PRIVATE_EGRESS_NODE_TIMINGS: RefCell<AssetOrchardPrivateEgressNodeTimingReport> =
        RefCell::new(AssetOrchardPrivateEgressNodeTimingReport {
            schema: "postfiat.asset_orchard_private_egress.node_timings.v1".to_string(),
            state_applications: Vec::new(),
        });
}

pub(super) fn node_timing_elapsed_ms(start: std::time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LocalProofLatencyMetrics {
    schema: String,
    last_verify_micros: u64,
    last_observed_unix_ms: u64,
}

pub(super) fn record_local_proof_verify_latency(
    store: &NodeStore,
    proof_verify_ms: f64,
) -> io::Result<()> {
    if !proof_verify_ms.is_finite() || proof_verify_ms <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "proof verification latency must be finite and positive",
        ));
    }
    let micros = proof_verify_ms * 1_000.0;
    if micros > u64::MAX as f64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "proof verification latency overflow",
        ));
    }
    let last_verify_micros = micros.round() as u64;
    let last_observed_unix_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("system clock before Unix epoch: {error}"),
                )
            })?
            .as_millis(),
    )
    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "system clock overflow"))?;
    let report = LocalProofLatencyMetrics {
        schema: PROOF_LATENCY_METRICS_SCHEMA.to_string(),
        last_verify_micros,
        last_observed_unix_ms,
    };
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(
        store.data_dir().join(PROOF_LATENCY_METRICS_FILE),
        format!("{json}\n"),
    )
}

fn read_local_proof_latency_metrics(store: &NodeStore) -> io::Result<ProofMetrics> {
    let path = store.data_dir().join(PROOF_LATENCY_METRICS_FILE);
    let report: LocalProofLatencyMetrics = match read_json_file(&path, "proof latency metrics") {
        Ok(report) => report,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ProofMetrics::default()),
        Err(error) => return Err(error),
    };
    if report.schema != PROOF_LATENCY_METRICS_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported proof latency metrics schema `{}`",
                report.schema
            ),
        ));
    }
    Ok(ProofMetrics {
        last_verify_micros: report.last_verify_micros,
        last_observed_unix_ms: report.last_observed_unix_ms,
    })
}

pub fn reset_asset_orchard_private_egress_node_timings() {
    ASSET_ORCHARD_PRIVATE_EGRESS_NODE_TIMINGS.with(|collector| {
        *collector.borrow_mut() = AssetOrchardPrivateEgressNodeTimingReport {
            schema: "postfiat.asset_orchard_private_egress.node_timings.v1".to_string(),
            state_applications: Vec::new(),
        };
    });
}

pub fn take_asset_orchard_private_egress_node_timings() -> AssetOrchardPrivateEgressNodeTimingReport
{
    ASSET_ORCHARD_PRIVATE_EGRESS_NODE_TIMINGS.with(|collector| {
        let mut collector = collector.borrow_mut();
        AssetOrchardPrivateEgressNodeTimingReport {
            schema: collector.schema.clone(),
            state_applications: std::mem::take(&mut collector.state_applications),
        }
    })
}

pub(super) fn record_asset_orchard_private_egress_state_apply_timing(
    timing: AssetOrchardPrivateEgressStateApplyTimingReport,
) {
    ASSET_ORCHARD_PRIVATE_EGRESS_NODE_TIMINGS.with(|collector| {
        collector.borrow_mut().state_applications.push(timing);
    });
}

pub fn init(options: InitOptions) -> io::Result<StatusReport> {
    let genesis = Genesis::try_new_with_validator_count(options.chain_id, options.validator_count)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    init_with_genesis(options.data_dir, options.node_id, genesis)
}

pub fn init_consensus_v2(options: InitConsensusV2Options) -> io::Result<StatusReport> {
    if options.activation_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "consensus v2 activation height must be positive",
        ));
    }
    let mut genesis =
        Genesis::try_new_with_validator_count(options.chain_id, options.validator_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    genesis.consensus_v2_activation_height = Some(options.activation_height);
    genesis
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    init_with_genesis(options.data_dir, options.node_id, genesis)
}

fn init_with_genesis(
    data_dir: PathBuf,
    node_id: String,
    genesis: Genesis,
) -> io::Result<StatusReport> {
    let store = NodeStore::new(&data_dir);
    let state = NodeState::initialized(node_id);
    let faucet_key = create_dev_key_file()?;
    let faucet_account = Account::new(
        faucet_key.address.clone(),
        DEFAULT_FAUCET_BALANCE,
        Some(faucet_key.public_key_hex.clone()),
    );
    store.init(&genesis, &state)?;
    store.write_ledger(&LedgerState::new(vec![faucet_account.clone()]))?;
    write_faucet_account_file(&data_dir.join(FAUCET_ACCOUNT_FILE), &faucet_account)?;
    write_key_file(&data_dir.join(FAUCET_KEY_FILE), &faucet_key)?;
    ensure_validator_keys(&store, genesis.validator_count)?;
    ensure_validator_registry_genesis(&store)?;
    status(NodeOptions { data_dir })
}

pub fn validator_keys(options: ValidatorKeysOptions) -> io::Result<ValidatorKeyFile> {
    let store = NodeStore::new(options.data_dir);
    ensure_validator_keys(&store, options.validators)
}

pub fn stage_validator_key(
    options: ValidatorKeyStageOptions,
) -> io::Result<ValidatorKeyStageReport> {
    let store = NodeStore::new(&options.data_dir);
    let key_path = options.data_dir.join(VALIDATOR_KEYS_FILE);
    let source_validator_id = options
        .source_validator_id
        .clone()
        .unwrap_or_else(|| options.validator_id.clone());
    let source_key_file = read_validator_key_file(&options.source_key_file)?;
    validate_validator_key_file(&source_key_file)?;
    let mut staged_record = validator_key_record(&source_key_file, &source_validator_id)?.clone();
    staged_record.node_id = options.validator_id.clone();
    validate_validator_key_file(&ValidatorKeyFile {
        validators: vec![staged_record.clone()],
    })?;

    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let registry_record = validator_registry_record(&registry, &options.validator_id)?;
    if registry_record.algorithm_id != staged_record.algorithm_id
        || registry_record.public_key_hex != staged_record.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "staged validator key `{}` does not match current validator registry",
                options.validator_id
            ),
        ));
    }

    let mut key_file = match read_validator_key_file(&key_path) {
        Ok(key_file) => key_file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => ValidatorKeyFile {
            validators: Vec::new(),
        },
        Err(error) => return Err(error),
    };
    validate_validator_key_file(&key_file)?;
    let action = match key_file
        .validators
        .iter()
        .position(|record| record.node_id == options.validator_id)
    {
        Some(index)
            if key_file.validators[index].algorithm_id == staged_record.algorithm_id
                && key_file.validators[index].public_key_hex == staged_record.public_key_hex
                && key_file.validators[index].private_key_hex == staged_record.private_key_hex =>
        {
            "unchanged"
        }
        Some(index) => {
            if !options.replace {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "validator key `{}` already exists; pass --replace to update it",
                        options.validator_id
                    ),
                ));
            }
            key_file.validators[index] = staged_record;
            "replaced"
        }
        None => {
            key_file.validators.push(staged_record);
            "added"
        }
    };
    sort_validator_key_records(&mut key_file.validators);
    validate_validator_key_file(&key_file)?;
    write_validator_key_file(&key_path, &key_file)?;
    Ok(ValidatorKeyStageReport {
        schema: "postfiat-validator-key-stage-v1".to_string(),
        validator_id: options.validator_id,
        source_validator_id,
        action: action.to_string(),
        validator_key_count: key_file.validators.len() as u32,
        registry_public_key_matched: true,
        key_file: key_path.display().to_string(),
        source_key_file: options.source_key_file.display().to_string(),
    })
}

pub fn validate_local_keys(options: ValidatorKeysOptions) -> io::Result<LocalKeyValidationReport> {
    let store = NodeStore::new(&options.data_dir);
    let state = store.read_node_state()?;
    let faucet_key_path = options.data_dir.join(FAUCET_KEY_FILE);
    let validator_key_path = options.data_dir.join(VALIDATOR_KEYS_FILE);
    validate_private_file_permissions(&faucet_key_path, "development key")?;
    validate_private_file_permissions(&validator_key_path, "validator key file")?;
    let faucet_key = read_key_file(&faucet_key_path)?;
    let validator_keys = read_validator_key_file(&validator_key_path)?;
    validate_validator_key_file(&validator_keys)?;
    let required_validator_ids = if options.local_only {
        let active_validator_ids = local_validator_ids(options.validators)?;
        if !active_validator_ids.contains(&state.node_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "local node id `{}` is not in active validator set",
                    state.node_id
                ),
            ));
        }
        vec![state.node_id.clone()]
    } else {
        local_validator_ids(options.validators)?
    };
    if validator_keys.validators.len() < required_validator_ids.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator private key file has {} keys, expected at least {}",
                validator_keys.validators.len(),
                required_validator_ids.len()
            ),
        ));
    }
    for node_id in &required_validator_ids {
        validator_key_record(&validator_keys, node_id)?;
    }
    Ok(LocalKeyValidationReport {
        schema: "postfiat-local-key-validation-v1".to_string(),
        node_id: state.node_id,
        faucet_key_valid: true,
        faucet_key_permissions_valid: true,
        faucet_address: faucet_key.address,
        validator_keys_valid: true,
        validator_key_permissions_valid: true,
        validator_key_count: validator_keys.validators.len() as u32,
        required_validator_count: required_validator_ids.len() as u32,
    })
}

pub fn write_local_topology(options: TopologyOptions) -> io::Result<NetworkTopology> {
    let genesis = Genesis::try_new_with_validator_count(options.chain_id, options.validators)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_topology_for_genesis(
        genesis,
        options.validators,
        options.base_port,
        options.rpc_base_port,
        options.hosts,
        options.output_file,
    )
}

pub fn write_consensus_v2_topology(
    options: TopologyConsensusV2Options,
) -> io::Result<NetworkTopology> {
    if options.activation_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "consensus v2 activation height must be positive",
        ));
    }
    let mut genesis = Genesis::try_new_with_validator_count(options.chain_id, options.validators)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    genesis.consensus_v2_activation_height = Some(options.activation_height);
    genesis
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_topology_for_genesis(
        genesis,
        options.validators,
        options.base_port,
        options.rpc_base_port,
        options.hosts,
        options.output_file,
    )
}

fn write_topology_for_genesis(
    genesis: Genesis,
    validators: u32,
    base_port: u16,
    rpc_base_port: Option<u16>,
    hosts: Option<Vec<String>>,
    output_file: PathBuf,
) -> io::Result<NetworkTopology> {
    let domain = NetworkDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
    };
    let topology = match hosts {
        Some(hosts) => {
            if hosts.len() != validators as usize {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "host count {} does not match validator count {}",
                        hosts.len(),
                        validators
                    ),
                ));
            }
            let rpc_base_port = rpc_base_port.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "--rpc-base-port is required when --hosts is used",
                )
            })?;
            remote_topology(domain, &hosts, base_port, rpc_base_port).map_err(invalid_data)?
        }
        None => local_topology(domain, validators, base_port).map_err(invalid_data)?,
    };
    let json = serde_json::to_string_pretty(&topology).map_err(invalid_data)?;
    atomic_write(&output_file, format!("{json}\n"))?;
    Ok(topology)
}

pub fn run_once(options: NodeOptions) -> io::Result<StatusReport> {
    let store = NodeStore::new(&options.data_dir);
    let mut state = store.read_node_state()?;
    state.mark_running();
    store.write_node_state(&state)?;
    status(options)
}

pub fn status(options: NodeOptions) -> io::Result<StatusReport> {
    let store = NodeStore::new(options.data_dir);
    recover_ordered_commit_journal(&store)?;
    let genesis = store.read_genesis()?;
    let state = store.read_node_state()?;
    let governance = store.read_governance()?;
    let mempool = store.read_mempool()?;
    let ledger = store.read_ledger()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let block_height = chain_tip.height;
    let block_tip_hash = chain_tip.block_hash;
    let genesis_hash_hex = genesis_hash(&genesis);
    let deployment_identity = deployment_runtime_identity_from_env()?;
    if let Some(validator_id) = &deployment_identity.validator_id {
        if validator_id != &state.node_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "deployment validator binding `{validator_id}` does not match local node `{}`",
                    state.node_id
                ),
            ));
        }
    }
    let mut active_nav_profiles = ledger
        .nav_assets
        .iter()
        .map(|asset| {
            let profile = ledger
                .nav_proof_profile(&asset.proof_profile)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "NAV asset `{}` references missing proof profile `{}`",
                            asset.asset_id, asset.proof_profile
                        ),
                    )
                })?;
            Ok(postfiat_types::ActiveNavProfileStatus {
                asset_id: asset.asset_id.clone(),
                profile_id: profile.profile_id.clone(),
                verifier_kind: profile.verifier_kind.clone(),
                source_class: profile.source_class.clone(),
                max_snapshot_age_blocks: profile.max_snapshot_age_blocks,
                challenge_window_blocks: profile.challenge_window_blocks,
                max_epoch_gap_blocks: profile.max_epoch_gap_blocks,
                settle_deadline_blocks: profile.settle_deadline_blocks,
                min_attestations: profile.min_attestations,
                tolerance_bp: profile.tolerance_bp,
                bridge_observer_min_confirmations: profile.bridge_observer_min_confirmations,
                valuation_policy_hash: profile.valuation_policy_hash.clone(),
                finalized_epoch: asset.finalized_epoch,
                nav_per_unit: asset.nav_per_unit,
                finalized_reserve_packet_hash: asset.finalized_reserve_packet_hash.clone(),
                halted: asset.halted,
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    active_nav_profiles.sort_by(|left, right| left.asset_id.cmp(&right.asset_id));
    Ok(StatusReport {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        rpc_schema: "postfiat-local-rpc-v1".to_string(),
        build_git_revision: option_env!("POSTFIAT_BUILD_GIT_REV")
            .unwrap_or("unknown")
            .to_string(),
        build_profile: option_env!("POSTFIAT_BUILD_PROFILE")
            .unwrap_or("unknown")
            .to_string(),
        active_nav_profiles,
        deployment_manifest_sha256: deployment_identity.manifest_sha256,
        deployment_validator_id: deployment_identity.validator_id,
        deployment_service_artifacts: deployment_identity.service_artifacts,
        deployment_runtime_artifacts: deployment_identity.runtime_artifacts,
        validator_count: governance.active_validator_count,
        node_id: state.node_id,
        status: state.status,
        last_run_unix: state.last_run_unix,
        state_root: chain_tip.state_root,
        block_height,
        block_tip_hash,
        mempool_pending: mempool.len() as u64,
    })
}

#[derive(Debug, Default)]
pub(super) struct DeploymentRuntimeIdentity {
    pub(super) manifest_sha256: Option<String>,
    pub(super) validator_id: Option<String>,
    pub(super) service_artifacts: Vec<DeploymentServiceArtifact>,
    pub(super) runtime_artifacts: Option<DeploymentRuntimeArtifactHashes>,
}

pub(super) fn deployment_runtime_identity_from_env() -> io::Result<DeploymentRuntimeIdentity> {
    deployment_runtime_identity_from_config(
        std::env::var_os("POSTFIAT_DEPLOYMENT_MANIFEST"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_VALIDATOR_ID"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_VALIDATOR_BINDINGS_FILE"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_BINARY"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_TOPOLOGY"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_SWAP_CIRCUIT_METADATA"),
        std::env::var_os("POSTFIAT_DEPLOYMENT_PRIVATE_EGRESS_CIRCUIT_METADATA"),
    )
}

pub(super) fn deployment_runtime_identity_from_config(
    manifest_path: Option<std::ffi::OsString>,
    validator_id_value: Option<std::ffi::OsString>,
    bindings_file: Option<std::ffi::OsString>,
    binary_file: Option<std::ffi::OsString>,
    topology_file: Option<std::ffi::OsString>,
    swap_circuit_metadata_file: Option<std::ffi::OsString>,
    private_egress_circuit_metadata_file: Option<std::ffi::OsString>,
) -> io::Result<DeploymentRuntimeIdentity> {
    let has_partial_identity = validator_id_value.is_some()
        || bindings_file.is_some()
        || binary_file.is_some()
        || topology_file.is_some()
        || swap_circuit_metadata_file.is_some()
        || private_egress_circuit_metadata_file.is_some();
    let Some(path) = manifest_path else {
        if has_partial_identity {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment runtime identity requires POSTFIAT_DEPLOYMENT_MANIFEST",
            ));
        }
        return Ok(DeploymentRuntimeIdentity::default());
    };
    if path.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "POSTFIAT_DEPLOYMENT_MANIFEST is empty",
        ));
    }
    let bytes = std::fs::read(PathBuf::from(path)).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("configured deployment manifest could not be read: {error}"),
        )
    })?;
    let manifest: DeploymentManifest = serde_json::from_slice(&bytes).map_err(invalid_data)?;
    let mut hasher = Sha256::new();
    Sha2Digest::update(&mut hasher, &bytes);
    let manifest_sha256 = bytes_to_hex(&hasher.finalize());
    let validator_id = validator_id_value
        .map(|value| {
            value.into_string().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "POSTFIAT_DEPLOYMENT_VALIDATOR_ID is not valid Unicode",
                )
            })
        })
        .transpose()?;
    let (validator_id, service_artifacts) = match (validator_id, bindings_file) {
        (None, None) => (None, Vec::new()),
        (Some(validator_id), Some(bindings_file)) => {
            validate_deployment_identifier(&validator_id, "validator_id")?;
            if bindings_file.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "POSTFIAT_DEPLOYMENT_VALIDATOR_BINDINGS_FILE is empty",
                ));
            }
            let bindings = read_deployment_validator_bindings_file(Path::new(&bindings_file))?;
            let binding = bindings
                .into_iter()
                .find(|binding| binding.validator_id == validator_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "configured deployment validator is absent from local bindings",
                    )
                })?;
            let signed_binding = manifest
                .validator_bindings
                .iter()
                .find(|binding| binding.validator_id == validator_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "configured deployment validator is absent from manifest",
                    )
                })?;
            if &binding != signed_binding {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "configured deployment service artifacts do not match manifest",
                ));
            }
            (Some(validator_id), binding.services)
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment validator ID and bindings file must be configured together",
            ))
        }
    };
    let runtime_artifacts = match (
        binary_file,
        topology_file,
        swap_circuit_metadata_file,
        private_egress_circuit_metadata_file,
    ) {
        (None, None, None, None) => None,
        (Some(binary), Some(topology), Some(swap), Some(private_egress)) => {
            if [&binary, &topology, &swap, &private_egress]
                .iter()
                .any(|path| path.is_empty())
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "deployment runtime artifact path is empty",
                ));
            }
            let actual = DeploymentRuntimeArtifactHashes {
                binary_sha256: sha256_file_hex(
                    &PathBuf::from(binary),
                    "deployment runtime binary",
                )?,
                topology_sha256: sha256_file_hex(
                    &PathBuf::from(topology),
                    "deployment runtime topology",
                )?,
                swap_circuit_metadata_sha256: sha256_file_hex(
                    &PathBuf::from(swap),
                    "deployment runtime swap circuit metadata",
                )?,
                private_egress_circuit_metadata_sha256: sha256_file_hex(
                    &PathBuf::from(private_egress),
                    "deployment runtime private-egress circuit metadata",
                )?,
            };
            let signed = DeploymentRuntimeArtifactHashes {
                binary_sha256: manifest.binary_sha256.clone(),
                topology_sha256: manifest.topology_sha256.clone(),
                swap_circuit_metadata_sha256: manifest.swap_circuit_metadata_sha256.clone(),
                private_egress_circuit_metadata_sha256: manifest
                    .private_egress_circuit_metadata_sha256
                    .clone(),
            };
            if actual != signed {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "configured deployment runtime artifacts do not match manifest",
                ));
            }
            Some(actual)
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment runtime artifact paths must be configured together",
            ))
        }
    };
    Ok(DeploymentRuntimeIdentity {
        manifest_sha256: Some(manifest_sha256),
        validator_id,
        service_artifacts,
        runtime_artifacts,
    })
}

pub(super) fn current_replicated_state_root(
    store: &NodeStore,
    genesis: &Genesis,
) -> io::Result<String> {
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let ordered_batches = store.read_ordered_batches()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    replicated_state_root(
        genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
    )
}

pub(super) fn next_block_height_from_chain_tip(
    store: &NodeStore,
    genesis: &Genesis,
) -> io::Result<u64> {
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, genesis)?;
    chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))
}

pub(super) fn asset_execution_compatibility_from_store(
    store: &NodeStore,
) -> io::Result<AssetExecutionCompatibility> {
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    Ok(asset_execution_compatibility_for_genesis_and_governance(
        &genesis,
        &governance,
    ))
}

pub fn block_proposer(options: BlockProposerOptions) -> io::Result<BlockProposerReport> {
    if options.block_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--height must be positive",
        ));
    }
    let data_dir = options.data_dir;
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })?;
    let store = NodeStore::new(data_dir);
    let governance = store.read_governance()?;
    let validators = active_validator_ids(&governance)?;
    let proposer =
        leader_for_view(&validators, options.block_height, options.view).map_err(invalid_data)?;
    Ok(BlockProposerReport {
        schema: "postfiat-block-proposer-v1".to_string(),
        chain_id: status_report.chain_id,
        genesis_hash: status_report.genesis_hash,
        protocol_version: status_report.protocol_version,
        local_node_id: status_report.node_id.clone(),
        block_height: options.block_height,
        view: options.view,
        active_validator_count: governance.active_validator_count,
        local_is_proposer: status_report.node_id == proposer,
        proposer,
    })
}

pub fn metrics(options: NodeOptions) -> io::Result<NodeMetrics> {
    let data_dir = options.data_dir;
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })?;
    let store = NodeStore::new(data_dir);
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let ordered_batches = store.read_ordered_batches()?;
    let archive = store.read_batch_archive()?;
    let blocks = store.read_blocks()?;
    let receipts = store.read_receipts()?;
    let mempool = store.read_mempool()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let burned_fee_total = receipts.iter().try_fold(0u64, |total, receipt| {
        total
            .checked_add(receipt.fee_burned)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "burned fee total overflow"))
    })?;
    let block_certificate_vote_count = blocks.blocks.iter().try_fold(0_u64, |total, block| {
        total
            .checked_add(block.header.certificate.votes.len() as u64)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "block certificate vote count overflow",
                )
            })
    })?;
    const RECENT_CERTIFICATE_WINDOW: usize = 128;
    let recent_start = blocks
        .blocks
        .len()
        .saturating_sub(RECENT_CERTIFICATE_WINDOW);
    let recent_blocks = &blocks.blocks[recent_start..];
    let recent_certificate_vote_count = recent_blocks.iter().try_fold(0_u64, |total, block| {
        total
            .checked_add(block.header.certificate.votes.len() as u64)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "recent certificate vote count overflow",
                )
            })
    })?;
    let local_recent_certificate_vote_count = recent_blocks
        .iter()
        .filter(|block| {
            block
                .header
                .certificate
                .votes
                .iter()
                .any(|vote| vote.accept && vote.validator == status_report.node_id)
        })
        .count() as u64;
    let local_recent_certificate_participation_ppm = if recent_blocks.is_empty() {
        0
    } else {
        u64::try_from(
            u128::from(local_recent_certificate_vote_count) * 1_000_000_u128
                / recent_blocks.len() as u128,
        )
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "certificate participation ratio overflow",
            )
        })?
    };
    let observed_unix_ms = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("system clock before Unix epoch: {error}"),
                )
            })?
            .as_millis(),
    )
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "system clock milliseconds overflow",
        )
    })?;
    let filesystem_capacity = store.filesystem_capacity()?;
    let proof_metrics = read_local_proof_latency_metrics(&store)?;
    if filesystem_capacity.total_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage filesystem reports zero total capacity",
        ));
    }
    let filesystem_available_ppm = u64::try_from(
        u128::from(filesystem_capacity.available_bytes)
            .checked_mul(1_000_000)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage availability ratio overflow",
                )
            })?
            / u128::from(filesystem_capacity.total_bytes),
    )
    .map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage availability ratio overflow",
        )
    })?;

    Ok(NodeMetrics {
        schema: "postfiat-node-metrics-v1".to_string(),
        chain_id: status_report.chain_id,
        genesis_hash: status_report.genesis_hash,
        protocol_version: status_report.protocol_version,
        node_id: status_report.node_id.clone(),
        observed_unix_ms,
        consensus: ConsensusMetrics {
            active_validator_count: governance.active_validator_count,
            crypto_policy_version: governance.crypto_policy_version,
            bridge_witness_epoch: governance.bridge_witness_epoch,
            authority_mode: governance.authority_mode,
            amendment_count: governance.amendments.len() as u64,
            validator_registry_update_count: governance.validator_registry_updates.len() as u64,
            block_certificate_count: blocks.blocks.len() as u64,
            block_certificate_vote_count,
            recent_certificate_window_blocks: recent_blocks.len() as u64,
            recent_certificate_vote_count,
            local_recent_certificate_vote_count,
            local_recent_certificate_participation_ppm,
        },
        ordering: OrderingMetrics {
            block_height: status_report.block_height,
            block_tip_hash: status_report.block_tip_hash,
            ordered_batch_count: ordered_batches.len() as u64,
            archived_batch_count: archive.len() as u64,
        },
        execution: ExecutionMetrics {
            account_count: ledger.accounts.len() as u64,
            receipt_count: receipts.len() as u64,
            burned_fee_total,
            account_reserve: ACCOUNT_RESERVE,
            minimum_transfer_fee: MIN_TRANSFER_FEE,
            transfer_account_creation_fee: TRANSFER_ACCOUNT_CREATION_FEE,
            transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
            transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
            state_root: status_report.state_root,
        },
        assets: asset_metrics(&ledger)?,
        mempool: MempoolMetrics {
            pending: mempool.len() as u64,
        },
        storage: StorageMetrics {
            replicated_state_file_count: SNAPSHOT_FILES.len() as u64,
            filesystem_total_bytes: filesystem_capacity.total_bytes,
            filesystem_available_bytes: filesystem_capacity.available_bytes,
            filesystem_available_ppm,
        },
        proofs: proof_metrics,
        shielded: ShieldedMetrics {
            note_count: shielded.notes.len() as u64,
            nullifier_count: shielded.nullifiers.len() as u64,
            turnstile_event_count: shielded.turnstile_events.len() as u64,
        },
        bridge: BridgeMetrics {
            domain_count: bridge.domains.len() as u64,
            transfer_count: bridge.transfers.len() as u64,
            replay_cache_count: bridge.replay_cache.len() as u64,
        },
    })
}

pub fn transfer_fee_quote(options: TransferFeeQuoteOptions) -> io::Result<TransferFeeQuoteReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let sender = ledger.account(&options.from).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("sender account `{}` not found", options.from),
        )
    })?;
    let sequence = match options.sequence {
        Some(sequence) if sequence > 0 => sequence,
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sequence must be nonzero",
            ));
        }
        None => next_pending_sender_sequence(&mempool, &options.from, sender.sequence)?,
    };
    let mempool_pending_for_sender = mempool_pending_count_for_sender(&mempool, &options.from);
    let memos = payment_v2_quote_memos(&options)?;
    let quote_transaction_kind = if memos.is_empty() {
        TRANSFER_TRANSACTION_KIND
    } else {
        PAYMENT_V2_TRANSACTION_KIND
    };
    let memo_count = u64::try_from(memos.len()).unwrap_or(u64::MAX);
    let memo_bytes = u64::try_from(
        memos
            .iter()
            .map(PaymentMemo::byte_len)
            .fold(0usize, usize::saturating_add),
    )
    .unwrap_or(u64::MAX);

    let mut fee = MIN_TRANSFER_FEE;
    let (quote_from, quote_to, base_transfer_fee, state_expansion_fee, minimum_fee, weight_bytes) = loop {
        let (quote_from, quote_to, base_transfer_fee, state_expansion_fee, weight_bytes) =
            if memos.is_empty() {
                let signed = quote_signed_transfer(
                    &genesis,
                    options.from.clone(),
                    options.to.clone(),
                    options.amount,
                    fee,
                    sequence,
                )?;
                (
                    signed.unsigned.from.clone(),
                    signed.unsigned.to.clone(),
                    minimum_transfer_fee(&signed),
                    transfer_state_expansion_fee(&ledger, &signed),
                    transfer_weight_bytes(&signed),
                )
            } else {
                let signed = quote_signed_payment_v2(
                    &genesis,
                    options.from.clone(),
                    options.to.clone(),
                    options.amount,
                    fee,
                    sequence,
                    memos.clone(),
                )?;
                (
                    signed.unsigned.from.clone(),
                    signed.unsigned.to.clone(),
                    minimum_payment_v2_fee(&signed),
                    payment_v2_state_expansion_fee(&ledger, &signed),
                    payment_v2_weight_bytes(&signed),
                )
            };
        let minimum_fee = base_transfer_fee.saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            break (
                quote_from,
                quote_to,
                base_transfer_fee,
                state_expansion_fee,
                minimum_fee,
                weight_bytes,
            );
        }
        fee = minimum_fee;
    };

    let recipient = ledger.account(&options.to);
    let recipient_exists = recipient.is_some();
    let will_create_recipient_account = quote_from != quote_to && !recipient_exists;
    let total_debit = options.amount.checked_add(minimum_fee);
    let sender_balance_after_amount_and_fee =
        total_debit.and_then(|total| sender.balance.checked_sub(total));
    let recipient_balance_after_amount = if quote_from == quote_to {
        sender_balance_after_amount_and_fee
            .and_then(|balance_after_debit| balance_after_debit.checked_add(options.amount))
    } else {
        recipient
            .map(|account| account.balance)
            .unwrap_or_default()
            .checked_add(options.amount)
    };
    let sender_final_balance = if quote_from == quote_to {
        recipient_balance_after_amount
    } else {
        sender_balance_after_amount_and_fee
    };
    let sender_meets_reserve_after_transfer = sender_final_balance.is_some_and(|balance| {
        balance >= postfiat_execution::required_account_reserve(&options.from)
    });
    let recipient_meets_reserve_after_transfer = if quote_from == quote_to {
        sender_meets_reserve_after_transfer
    } else {
        recipient_balance_after_amount.is_some_and(|balance| {
            balance >= postfiat_execution::required_account_reserve(&options.to)
        })
    };
    let sequence_source = if options.sequence.is_some() {
        "explicit".to_string()
    } else {
        "ledger_mempool".to_string()
    };

    Ok(TransferFeeQuoteReport {
        schema: "postfiat-transfer-fee-quote-v1".to_string(),
        transaction_kind: Some(quote_transaction_kind.to_string()),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        from: options.from,
        to: options.to,
        amount: options.amount,
        sequence,
        sequence_source,
        sender_balance: sender.balance,
        sender_sequence: sender.sequence,
        mempool_pending_for_sender,
        recipient_exists,
        will_create_recipient_account,
        base_transfer_fee,
        state_expansion_fee,
        minimum_fee,
        account_reserve: ACCOUNT_RESERVE,
        transfer_account_creation_fee: TRANSFER_ACCOUNT_CREATION_FEE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        transfer_weight_bytes: u64::try_from(weight_bytes).unwrap_or(u64::MAX),
        memo_count: Some(memo_count),
        memo_bytes: Some(memo_bytes),
        sender_balance_after_amount_and_fee,
        sender_meets_reserve_after_transfer,
        recipient_balance_after_amount,
        recipient_meets_reserve_after_transfer,
    })
}

pub fn asset_fee_quote(options: AssetFeeQuoteOptions) -> io::Result<AssetFeeQuoteReport> {
    if options.operation_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("asset operation JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let operation: AssetTransactionOperation =
        serde_json::from_str(&options.operation_json).map_err(invalid_data)?;
    operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let ledger_sender = ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{}` not found", options.source),
        )
    })?;
    let sequence = match options.sequence {
        Some(sequence) if sequence > 0 => sequence,
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sequence must be nonzero",
            ));
        }
        None => next_pending_sender_sequence(&mempool, &options.source, ledger_sender.sequence)?,
    };
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let quote_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let sender = quote_ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "source account `{}` not found after pending mempool dry-run",
                options.source
            ),
        )
    })?;
    let mempool_pending_for_sender = mempool_pending_count_for_sender(&mempool, &options.source);

    let mut fee = MIN_TRANSFER_FEE;
    let (base_asset_fee, state_expansion_fee, minimum_fee, weight_bytes) = loop {
        let signed = quote_signed_asset_transaction(
            &genesis,
            options.source.clone(),
            fee,
            sequence,
            operation.clone(),
        )?;
        let base_asset_fee = minimum_asset_transaction_fee(&signed);
        let state_expansion_fee = asset_transaction_state_expansion_fee(&quote_ledger, &signed);
        let minimum_fee = base_asset_fee.saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            break (
                base_asset_fee,
                state_expansion_fee,
                minimum_fee,
                asset_transaction_weight_bytes(&signed),
            );
        }
        fee = minimum_fee;
    };

    let sender_balance_after_fee = sender.balance.checked_sub(minimum_fee);
    let sender_meets_reserve_after_fee = sender_balance_after_fee.is_some_and(|balance| {
        balance >= postfiat_execution::required_account_reserve(&options.source)
    });
    let sequence_source = if options.sequence.is_some() {
        "explicit".to_string()
    } else {
        "ledger_mempool".to_string()
    };

    Ok(AssetFeeQuoteReport {
        schema: "postfiat-asset-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: options.source,
        sequence,
        sequence_source,
        sender_balance: sender.balance,
        sender_sequence: sender.sequence,
        mempool_pending_for_sender,
        base_asset_fee,
        state_expansion_fee,
        minimum_fee,
        account_reserve: ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        asset_weight_bytes: u64::try_from(weight_bytes).unwrap_or(u64::MAX),
        sender_balance_after_fee,
        sender_meets_reserve_after_fee,
        operation,
    })
}

pub fn escrow_fee_quote(options: EscrowFeeQuoteOptions) -> io::Result<EscrowFeeQuoteReport> {
    if options.operation_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("escrow operation JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let operation: EscrowTransactionOperation =
        serde_json::from_str(&options.operation_json).map_err(invalid_data)?;
    operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let ledger_sender = ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{}` not found", options.source),
        )
    })?;
    let sequence = match options.sequence {
        Some(sequence) if sequence > 0 => sequence,
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sequence must be nonzero",
            ));
        }
        None => next_pending_sender_sequence(&mempool, &options.source, ledger_sender.sequence)?,
    };
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let quote_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let sender = quote_ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "source account `{}` not found after pending mempool dry-run",
                options.source
            ),
        )
    })?;
    let mempool_pending_for_sender = mempool_pending_count_for_sender(&mempool, &options.source);

    let mut fee = MIN_TRANSFER_FEE;
    let (base_escrow_fee, state_expansion_fee, minimum_fee, weight_bytes) = loop {
        let signed = quote_signed_escrow_transaction(
            &genesis,
            options.source.clone(),
            fee,
            sequence,
            operation.clone(),
        )?;
        let base_escrow_fee = minimum_escrow_transaction_fee(&signed);
        let state_expansion_fee = escrow_transaction_state_expansion_fee(&quote_ledger, &signed);
        let minimum_fee = base_escrow_fee.saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            break (
                base_escrow_fee,
                state_expansion_fee,
                minimum_fee,
                escrow_transaction_weight_bytes(&signed),
            );
        }
        fee = minimum_fee;
    };

    let sender_balance_after_fee = sender.balance.checked_sub(minimum_fee);
    let sender_meets_reserve_after_fee = sender_balance_after_fee.is_some_and(|balance| {
        balance >= postfiat_execution::required_account_reserve(&options.source)
    });
    let sequence_source = if options.sequence.is_some() {
        "explicit".to_string()
    } else {
        "ledger_mempool".to_string()
    };

    Ok(EscrowFeeQuoteReport {
        schema: "postfiat-escrow-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: options.source,
        sequence,
        sequence_source,
        sender_balance: sender.balance,
        sender_sequence: sender.sequence,
        mempool_pending_for_sender,
        base_escrow_fee,
        state_expansion_fee,
        minimum_fee,
        account_reserve: ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        escrow_weight_bytes: u64::try_from(weight_bytes).unwrap_or(u64::MAX),
        sender_balance_after_fee,
        sender_meets_reserve_after_fee,
        operation,
    })
}

pub fn nft_fee_quote(options: NftFeeQuoteOptions) -> io::Result<NftFeeQuoteReport> {
    if options.operation_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("nft operation JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let operation: NftTransactionOperation =
        serde_json::from_str(&options.operation_json).map_err(invalid_data)?;
    operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let ledger_sender = ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{}` not found", options.source),
        )
    })?;
    let sequence = match options.sequence {
        Some(sequence) if sequence > 0 => sequence,
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sequence must be nonzero",
            ));
        }
        None => next_pending_sender_sequence(&mempool, &options.source, ledger_sender.sequence)?,
    };
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let quote_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let (operation, issuer_transfer_fee, issuer_transfer_fee_recipient) =
        normalize_nft_fee_quote_operation(&quote_ledger, operation)?;
    let sender = quote_ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "source account `{}` not found after pending mempool dry-run",
                options.source
            ),
        )
    })?;
    let mempool_pending_for_sender = mempool_pending_count_for_sender(&mempool, &options.source);

    let mut fee = MIN_TRANSFER_FEE;
    let (base_nft_fee, state_expansion_fee, minimum_fee, weight_bytes) = loop {
        let signed = quote_signed_nft_transaction(
            &genesis,
            options.source.clone(),
            fee,
            sequence,
            operation.clone(),
        )?;
        let base_nft_fee = minimum_nft_transaction_fee(&signed);
        let state_expansion_fee = nft_transaction_state_expansion_fee(&quote_ledger, &signed);
        let minimum_fee = base_nft_fee.saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            break (
                base_nft_fee,
                state_expansion_fee,
                minimum_fee,
                nft_transaction_weight_bytes(&signed),
            );
        }
        fee = minimum_fee;
    };

    let sender_balance_after_fee = sender.balance.checked_sub(minimum_fee);
    let sender_meets_reserve_after_fee = sender_balance_after_fee.is_some_and(|balance| {
        balance >= postfiat_execution::required_account_reserve(&options.source)
    });
    let sender_balance_after_fee_and_issuer_transfer_fee =
        sender_balance_after_fee.and_then(|balance| balance.checked_sub(issuer_transfer_fee));
    let sender_meets_reserve_after_fee_and_issuer_transfer_fee =
        sender_balance_after_fee_and_issuer_transfer_fee.is_some_and(|balance| {
            balance >= postfiat_execution::required_account_reserve(&options.source)
        });
    let sequence_source = if options.sequence.is_some() {
        "explicit".to_string()
    } else {
        "ledger_mempool".to_string()
    };

    Ok(NftFeeQuoteReport {
        schema: "postfiat-nft-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: options.source,
        sequence,
        sequence_source,
        sender_balance: sender.balance,
        sender_sequence: sender.sequence,
        mempool_pending_for_sender,
        base_nft_fee,
        state_expansion_fee,
        minimum_fee,
        account_reserve: ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        nft_weight_bytes: u64::try_from(weight_bytes).unwrap_or(u64::MAX),
        sender_balance_after_fee,
        sender_meets_reserve_after_fee,
        issuer_transfer_fee,
        issuer_transfer_fee_recipient,
        sender_balance_after_fee_and_issuer_transfer_fee,
        sender_meets_reserve_after_fee_and_issuer_transfer_fee,
        operation,
    })
}

pub(super) fn normalize_nft_fee_quote_operation(
    ledger: &LedgerState,
    operation: NftTransactionOperation,
) -> io::Result<(NftTransactionOperation, u64, Option<String>)> {
    match operation {
        NftTransactionOperation::NftTransfer(mut transfer) => {
            let (issuer, issuer_transfer_fee) = nft_transfer_issuer_fee_terms(ledger, &transfer)
                .map_err(|(code, message)| {
                    io::Error::new(io::ErrorKind::InvalidInput, format!("{code}: {message}"))
                })?;
            transfer.issuer_transfer_fee = issuer_transfer_fee;
            transfer.issuer = if issuer_transfer_fee == 0 {
                String::new()
            } else {
                issuer.clone()
            };
            let recipient = (issuer_transfer_fee != 0).then_some(issuer);
            Ok((
                NftTransactionOperation::NftTransfer(transfer),
                issuer_transfer_fee,
                recipient,
            ))
        }
        operation => Ok((operation, 0, None)),
    }
}

pub fn offer_fee_quote(options: OfferFeeQuoteOptions) -> io::Result<OfferFeeQuoteReport> {
    if options.operation_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("offer operation JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let operation: OfferTransactionOperation =
        serde_json::from_str(&options.operation_json).map_err(invalid_data)?;
    operation
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let ledger_sender = ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{}` not found", options.source),
        )
    })?;
    let sequence = match options.sequence {
        Some(sequence) if sequence > 0 => sequence,
        Some(_) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "sequence must be nonzero",
            ));
        }
        None => next_pending_sender_sequence(&mempool, &options.source, ledger_sender.sequence)?,
    };
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let quote_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let sender = quote_ledger.account(&options.source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "source account `{}` not found after pending mempool dry-run",
                options.source
            ),
        )
    })?;
    let mempool_pending_for_sender = mempool_pending_count_for_sender(&mempool, &options.source);

    let mut fee = MIN_TRANSFER_FEE;
    let (
        base_offer_fee,
        match_fee,
        state_expansion_fee,
        estimated_cross_count,
        will_create_residual_offer,
        minimum_fee,
        weight_bytes,
    ) = loop {
        let signed = quote_signed_offer_transaction(
            &genesis,
            options.source.clone(),
            fee,
            sequence,
            operation.clone(),
        )?;
        let base_offer_fee = minimum_offer_transaction_fee(&signed);
        let match_fee = offer_transaction_match_fee(&quote_ledger, &signed, block_height);
        let state_expansion_fee =
            offer_transaction_state_expansion_fee(&quote_ledger, &signed, block_height);
        let estimated_cross_count =
            offer_transaction_estimated_cross_count(&quote_ledger, &signed, block_height);
        let will_create_residual_offer =
            offer_transaction_will_create_residual_offer(&quote_ledger, &signed, block_height);
        let minimum_fee = base_offer_fee
            .saturating_add(match_fee)
            .saturating_add(state_expansion_fee);
        if fee >= minimum_fee {
            break (
                base_offer_fee,
                match_fee,
                state_expansion_fee,
                estimated_cross_count,
                will_create_residual_offer,
                minimum_fee,
                offer_transaction_weight_bytes(&signed),
            );
        }
        fee = minimum_fee;
    };

    let sender_balance_after_fee = sender.balance.checked_sub(minimum_fee);
    let offer_object_reserve = if will_create_residual_offer {
        OFFER_OBJECT_RESERVE
    } else {
        0
    };
    let sender_balance_after_fee_and_reserve =
        sender_balance_after_fee.and_then(|balance| balance.checked_sub(offer_object_reserve));
    let sender_meets_reserve_after_fee = sender_balance_after_fee.is_some_and(|balance| {
        balance >= postfiat_execution::required_account_reserve(&options.source)
    });
    let sender_meets_reserve_after_fee_and_reserve = sender_balance_after_fee_and_reserve
        .is_some_and(|balance| {
            balance >= postfiat_execution::required_account_reserve(&options.source)
        });
    let sequence_source = if options.sequence.is_some() {
        "explicit".to_string()
    } else {
        "ledger_mempool".to_string()
    };

    Ok(OfferFeeQuoteReport {
        schema: "postfiat-offer-fee-quote-v1".to_string(),
        transaction_kind: operation.transaction_kind().to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        source: options.source,
        sequence,
        sequence_source,
        sender_balance: sender.balance,
        sender_sequence: sender.sequence,
        mempool_pending_for_sender,
        base_offer_fee,
        match_fee,
        state_expansion_fee,
        estimated_cross_count: estimated_cross_count as u64,
        max_dex_crosses_per_transaction: postfiat_types::MAX_DEX_CROSSES_PER_TRANSACTION as u64,
        will_create_residual_offer,
        offer_object_reserve,
        minimum_fee,
        account_reserve: ACCOUNT_RESERVE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        offer_weight_bytes: u64::try_from(weight_bytes).unwrap_or(u64::MAX),
        sender_balance_after_fee,
        sender_balance_after_fee_and_reserve,
        sender_meets_reserve_after_fee,
        sender_meets_reserve_after_fee_and_reserve,
        operation,
    })
}

pub fn atomic_settlement_template(
    options: AtomicSettlementTemplateOptions,
) -> io::Result<AtomicSettlementTemplateReport> {
    let data_dir = options.data_dir.clone();
    let left_operation =
        EscrowTransactionOperation::EscrowCreate(postfiat_types::EscrowCreateOperation {
            owner: options.left_owner.clone(),
            recipient: options.left_recipient.clone(),
            asset_id: options.left_asset_id.clone(),
            amount: options.left_amount,
            condition: options.condition.clone(),
            finish_after: options.finish_after,
            cancel_after: options.cancel_after,
        });
    let right_operation =
        EscrowTransactionOperation::EscrowCreate(postfiat_types::EscrowCreateOperation {
            owner: options.right_owner.clone(),
            recipient: options.right_recipient.clone(),
            asset_id: options.right_asset_id.clone(),
            amount: options.right_amount,
            condition: options.condition.clone(),
            finish_after: options.finish_after,
            cancel_after: options.cancel_after,
        });

    let left_quote = escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: data_dir.clone(),
        source: options.left_owner.clone(),
        operation_json: serde_json::to_string(&left_operation).map_err(invalid_data)?,
        sequence: options.left_sequence,
    })?;
    let right_quote = escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: data_dir.clone(),
        source: options.right_owner.clone(),
        operation_json: serde_json::to_string(&right_operation).map_err(invalid_data)?,
        sequence: options.right_sequence,
    })?;

    if left_quote.chain_id != right_quote.chain_id
        || left_quote.genesis_hash != right_quote.genesis_hash
        || left_quote.protocol_version != right_quote.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "atomic settlement quotes resolved different chain domains",
        ));
    }

    let template = AtomicSettlementTemplate {
        left: AtomicSettlementTemplateLeg {
            owner: options.left_owner,
            recipient: options.left_recipient,
            asset_id: options.left_asset_id,
            amount: options.left_amount,
            owner_sequence: left_quote.sequence,
        },
        right: AtomicSettlementTemplateLeg {
            owner: options.right_owner,
            recipient: options.right_recipient,
            asset_id: options.right_asset_id,
            amount: options.right_amount,
            owner_sequence: right_quote.sequence,
        },
        condition: options.condition,
        finish_after: options.finish_after,
        cancel_after: options.cancel_after,
    };
    template
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;

    let store = NodeStore::new(&data_dir);
    let ledger = store.read_ledger()?;
    validate_atomic_settlement_assets_exist(&ledger, &template)?;

    let settlement_id =
        postfiat_types::atomic_settlement_template_id(&left_quote.chain_id, &template)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let condition_hash = postfiat_types::escrow_condition_hash(&template.condition)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let left_escrow_id = postfiat_types::escrow_id(
        &left_quote.chain_id,
        &template.left.owner,
        template.left.owner_sequence,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let right_escrow_id = postfiat_types::escrow_id(
        &left_quote.chain_id,
        &template.right.owner,
        template.right.owner_sequence,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    Ok(AtomicSettlementTemplateReport {
        schema: "postfiat-atomic-settlement-template-v1".to_string(),
        chain_id: left_quote.chain_id.clone(),
        genesis_hash: left_quote.genesis_hash.clone(),
        protocol_version: left_quote.protocol_version,
        settlement_id,
        condition_hash,
        condition: template.condition.clone(),
        finish_after: template.finish_after,
        cancel_after: template.cancel_after,
        left: atomic_settlement_template_leg_report(left_quote, &template.left, left_escrow_id),
        right: atomic_settlement_template_leg_report(right_quote, &template.right, right_escrow_id),
    })
}

pub fn asset_info(options: AssetInfoOptions) -> io::Result<AssetInfoReport> {
    validate_issued_asset_query_id("asset_id", &options.asset_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let asset = ledger
        .asset_definition(&options.asset_id)
        .map(|asset| issued_asset_report(&ledger, asset))
        .transpose()?;
    Ok(AssetInfoReport {
        schema: "postfiat-asset-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        asset_id: options.asset_id,
        found: asset.is_some(),
        asset,
    })
}

pub fn account_lines(options: AccountLinesOptions) -> io::Result<AccountLinesReport> {
    validate_query_text_field("account", &options.account)?;
    if let Some(issuer) = options.issuer.as_ref() {
        validate_query_text_field("issuer", issuer)?;
    }
    if let Some(asset_id) = options.asset_id.as_ref() {
        validate_issued_asset_query_id("asset_id", asset_id)?;
    }
    let limit = bounded_read_query_limit(options.limit, "account_lines")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut lines = issued_asset_line_reports(&ledger, |line| {
        line.account == options.account
            && options
                .issuer
                .as_ref()
                .is_none_or(|issuer| line.issuer == *issuer)
            && options
                .asset_id
                .as_ref()
                .is_none_or(|asset_id| line.asset_id == *asset_id)
    })?;
    let truncated = truncate_reports(&mut lines, limit);
    Ok(AccountLinesReport {
        schema: "postfiat-account-lines-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        account: options.account,
        issuer: options.issuer,
        asset_id: options.asset_id,
        limit: limit as u64,
        truncated,
        line_count: lines.len() as u64,
        lines,
    })
}

pub fn account_assets(options: AccountAssetsOptions) -> io::Result<AccountAssetsReport> {
    validate_query_text_field("account", &options.account)?;
    if let Some(asset_id) = options.asset_id.as_ref() {
        validate_issued_asset_query_id("asset_id", asset_id)?;
    }
    let limit = bounded_read_query_limit(options.limit, "account_assets")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut assets = issued_asset_line_reports(&ledger, |line| {
        line.account == options.account
            && line.balance > 0
            && options
                .asset_id
                .as_ref()
                .is_none_or(|asset_id| line.asset_id == *asset_id)
    })?;
    let truncated = truncate_reports(&mut assets, limit);
    Ok(AccountAssetsReport {
        schema: "postfiat-account-assets-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        account: options.account,
        asset_id: options.asset_id,
        limit: limit as u64,
        truncated,
        asset_count: assets.len() as u64,
        assets,
    })
}

pub fn owned_objects(options: OwnedObjectsOptions) -> io::Result<OwnedObjectsReport> {
    validate_owner_public_key_hex(&options.owner_public_key_hex)?;
    let owner_public_key = hex_to_bytes(&options.owner_public_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("owner_public_key_hex must be valid hex: {error}"),
        )
    })?;
    ml_dsa_65_validate_public_key(&owner_public_key).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("owner_public_key_hex is not a valid ML-DSA-65 public key: {error}"),
        )
    })?;
    if let Some(asset) = options.asset.as_ref() {
        validate_query_text_field("asset", asset)?;
    }
    let limit = bounded_read_query_limit_with_max(
        options.limit,
        "owned_objects",
        postfiat_types::MAX_OWNED_INPUTS_PER_TRANSFER,
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut objects = ledger
        .owned_objects
        .iter()
        .filter(|object| {
            object.owner_pubkey_hex == options.owner_public_key_hex
                && options
                    .asset
                    .as_ref()
                    .is_none_or(|asset| object.asset == *asset)
        })
        .cloned()
        .collect::<Vec<_>>();
    objects.sort_by(|left, right| {
        left.asset
            .cmp(&right.asset)
            .then_with(|| left.id.cmp(&right.id))
            .then_with(|| left.version.cmp(&right.version))
    });
    let total_value = objects.iter().try_fold(0u64, |sum, object| {
        sum.checked_add(object.value).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "owned object balance overflow while summing values",
            )
        })
    })?;
    let truncated = truncate_reports(&mut objects, limit);
    Ok(OwnedObjectsReport {
        schema: "postfiat-owned-objects-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        owner_public_key_hex: options.owner_public_key_hex,
        asset: options.asset,
        limit: limit as u64,
        truncated,
        object_count: objects.len() as u64,
        total_value,
        objects,
    })
}

pub fn issuer_assets(options: IssuerAssetsOptions) -> io::Result<IssuerAssetsReport> {
    validate_query_text_field("issuer", &options.issuer)?;
    let limit = bounded_read_query_limit(options.limit, "issuer_assets")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut assets = ledger
        .asset_definitions
        .iter()
        .filter(|asset| asset.issuer == options.issuer)
        .map(|asset| issued_asset_report(&ledger, asset))
        .collect::<io::Result<Vec<_>>>()?;
    assets.sort_by(|left, right| {
        (
            left.code.as_str(),
            left.version,
            left.asset_id.as_str(),
            left.issuer.as_str(),
        )
            .cmp(&(
                right.code.as_str(),
                right.version,
                right.asset_id.as_str(),
                right.issuer.as_str(),
            ))
    });
    let truncated = truncate_reports(&mut assets, limit);
    Ok(IssuerAssetsReport {
        schema: "postfiat-issuer-assets-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        issuer: options.issuer,
        limit: limit as u64,
        truncated,
        asset_count: assets.len() as u64,
        assets,
    })
}

pub fn escrow_info(options: EscrowInfoOptions) -> io::Result<EscrowInfoReport> {
    validate_escrow_query_id("escrow_id", &options.escrow_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let escrow = ledger
        .escrow(&options.escrow_id)
        .map(escrow_report)
        .transpose()?;
    Ok(EscrowInfoReport {
        schema: "postfiat-escrow-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        escrow_id: options.escrow_id,
        found: escrow.is_some(),
        escrow,
    })
}

pub fn account_escrows(options: AccountEscrowsOptions) -> io::Result<AccountEscrowsReport> {
    validate_query_text_field("account", &options.account)?;
    validate_escrow_role_filter(options.role.as_deref())?;
    validate_escrow_state_filter(options.state.as_deref())?;
    let limit = bounded_read_query_limit(options.limit, "account_escrows")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut escrows = ledger
        .escrows
        .iter()
        .filter(|escrow| {
            let role_matches = match options.role.as_deref() {
                Some("owner") => escrow.owner == options.account,
                Some("recipient") => escrow.recipient == options.account,
                None => escrow.owner == options.account || escrow.recipient == options.account,
                Some(_) => false,
            };
            role_matches
                && options
                    .state
                    .as_ref()
                    .is_none_or(|state| escrow.state == *state)
        })
        .map(escrow_report)
        .collect::<io::Result<Vec<_>>>()?;
    escrows.sort_by(|left, right| {
        (
            left.created_height,
            left.owner_sequence,
            left.escrow_id.as_str(),
            left.owner.as_str(),
            left.recipient.as_str(),
        )
            .cmp(&(
                right.created_height,
                right.owner_sequence,
                right.escrow_id.as_str(),
                right.owner.as_str(),
                right.recipient.as_str(),
            ))
    });
    let truncated = truncate_reports(&mut escrows, limit);
    Ok(AccountEscrowsReport {
        schema: "postfiat-account-escrows-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        account: options.account,
        role: options.role,
        state: options.state,
        limit: limit as u64,
        truncated,
        escrow_count: escrows.len() as u64,
        escrows,
    })
}

pub fn nft_info(options: NftInfoOptions) -> io::Result<NftInfoReport> {
    validate_nft_query_id("nft_id", &options.nft_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let nft = ledger.nft(&options.nft_id).map(nft_report);
    Ok(NftInfoReport {
        schema: "postfiat-nft-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        nft_id: options.nft_id,
        found: nft.is_some(),
        nft,
    })
}

pub fn account_nfts(options: AccountNftsOptions) -> io::Result<AccountNftsReport> {
    validate_query_text_field("account", &options.account)?;
    let limit = bounded_read_query_limit(options.limit, "account_nfts")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let indexes = ledger
        .nft_indexes(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut nfts = if options.include_burned {
        ledger
            .nfts
            .iter()
            .filter(|nft| nft.owner == options.account)
            .map(nft_report)
            .collect::<Vec<_>>()
    } else {
        indexes
            .by_owner
            .get(&options.account)
            .into_iter()
            .flat_map(|nft_ids| nft_ids.iter())
            .filter_map(|nft_id| ledger.nft(nft_id))
            .map(nft_report)
            .collect::<Vec<_>>()
    };
    nfts.sort_by(|left, right| {
        (
            left.collection_id.as_str(),
            left.serial,
            left.nft_id.as_str(),
            left.issuer.as_str(),
        )
            .cmp(&(
                right.collection_id.as_str(),
                right.serial,
                right.nft_id.as_str(),
                right.issuer.as_str(),
            ))
    });
    let truncated = truncate_reports(&mut nfts, limit);
    Ok(AccountNftsReport {
        schema: "postfiat-account-nfts-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        account: options.account,
        include_burned: options.include_burned,
        limit: limit as u64,
        truncated,
        nft_count: nfts.len() as u64,
        nfts,
    })
}

pub fn issuer_nfts(options: IssuerNftsOptions) -> io::Result<IssuerNftsReport> {
    validate_query_text_field("issuer", &options.issuer)?;
    if let Some(collection_id) = options.collection_id.as_ref() {
        validate_nft_collection_query_id(collection_id)?;
    }
    let limit = bounded_read_query_limit(options.limit, "issuer_nfts")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    ledger
        .nft_indexes(&genesis.chain_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut nfts = ledger
        .nfts
        .iter()
        .filter(|nft| {
            nft.issuer == options.issuer
                && options
                    .collection_id
                    .as_ref()
                    .is_none_or(|collection_id| nft.collection_id == *collection_id)
                && (options.include_burned || !nft.burned)
        })
        .map(nft_report)
        .collect::<Vec<_>>();
    nfts.sort_by(|left, right| {
        (
            left.collection_id.as_str(),
            left.serial,
            left.nft_id.as_str(),
            left.owner.as_str(),
        )
            .cmp(&(
                right.collection_id.as_str(),
                right.serial,
                right.nft_id.as_str(),
                right.owner.as_str(),
            ))
    });
    let truncated = truncate_reports(&mut nfts, limit);
    Ok(IssuerNftsReport {
        schema: "postfiat-issuer-nfts-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        issuer: options.issuer,
        collection_id: options.collection_id,
        include_burned: options.include_burned,
        limit: limit as u64,
        truncated,
        nft_count: nfts.len() as u64,
        nfts,
    })
}

pub fn offer_info(options: OfferInfoOptions) -> io::Result<OfferInfoReport> {
    validate_offer_query_id("offer_id", &options.offer_id)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let offer = ledger.offer(&options.offer_id).map(offer_report);
    Ok(OfferInfoReport {
        schema: "postfiat-offer-info-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        offer_id: options.offer_id,
        found: offer.is_some(),
        offer,
    })
}

pub fn account_offers(options: AccountOffersOptions) -> io::Result<AccountOffersReport> {
    validate_query_text_field("account", &options.account)?;
    validate_offer_state_filter(options.state.as_deref())?;
    let limit = bounded_read_query_limit(options.limit, "account_offers")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut offers = ledger
        .offers
        .iter()
        .filter(|offer| {
            offer.owner == options.account
                && options
                    .state
                    .as_ref()
                    .is_none_or(|state| offer.state == *state)
        })
        .map(offer_report)
        .collect::<Vec<_>>();
    offers.sort_by(|left, right| {
        (
            left.created_height,
            left.owner_sequence,
            left.offer_id.as_str(),
        )
            .cmp(&(
                right.created_height,
                right.owner_sequence,
                right.offer_id.as_str(),
            ))
    });
    let truncated = truncate_reports(&mut offers, limit);
    Ok(AccountOffersReport {
        schema: "postfiat-account-offers-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        account: options.account,
        state: options.state,
        limit: limit as u64,
        truncated,
        offer_count: offers.len() as u64,
        offers,
    })
}

pub fn book_offers(options: BookOffersOptions) -> io::Result<BookOffersReport> {
    validate_dex_asset_query_id("taker_gets_asset_id", &options.taker_gets_asset_id)?;
    validate_dex_asset_query_id("taker_pays_asset_id", &options.taker_pays_asset_id)?;
    postfiat_types::offer_book_key(&options.taker_gets_asset_id, &options.taker_pays_asset_id)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let limit = bounded_read_query_limit(options.limit, "book_offers")?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut offers = ledger
        .offers
        .iter()
        .filter(|offer| {
            offer.state == OFFER_STATE_OPEN
                && offer.taker_gets_asset_id == options.taker_gets_asset_id
                && offer.taker_pays_asset_id == options.taker_pays_asset_id
        })
        .map(offer_report)
        .collect::<Vec<_>>();
    offers.sort_by(compare_offer_book_reports);
    let truncated = truncate_reports(&mut offers, limit);
    Ok(BookOffersReport {
        schema: "postfiat-book-offers-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        taker_gets_asset_id: options.taker_gets_asset_id,
        taker_pays_asset_id: options.taker_pays_asset_id,
        limit: limit as u64,
        truncated,
        offer_count: offers.len() as u64,
        offers,
    })
}

pub(super) fn issued_asset_report(
    ledger: &LedgerState,
    asset: &AssetDefinition,
) -> io::Result<IssuedAssetReport> {
    let (outstanding_supply, trustline_count, holder_count) =
        issued_asset_stats(ledger, &asset.asset_id)?;
    Ok(IssuedAssetReport {
        asset_id: asset.asset_id.clone(),
        issuer: asset.issuer.clone(),
        code: asset.code.clone(),
        version: asset.version,
        precision: asset.precision,
        display_name: asset.display_name.clone(),
        max_supply: asset.max_supply,
        requires_authorization: asset.requires_authorization,
        freeze_enabled: asset.freeze_enabled,
        clawback_enabled: asset.clawback_enabled,
        outstanding_supply,
        trustline_count,
        holder_count,
    })
}

pub(super) fn issued_asset_line_reports(
    ledger: &LedgerState,
    mut include_line: impl FnMut(&TrustLine) -> bool,
) -> io::Result<Vec<AssetLineReport>> {
    let mut lines = ledger
        .trustlines
        .iter()
        .filter(|line| include_line(line))
        .map(|line| {
            let asset = ledger.asset_definition(&line.asset_id).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "trustline `{}` references missing asset `{}`",
                        line.trustline_id, line.asset_id
                    ),
                )
            })?;
            Ok(asset_line_report(line, asset))
        })
        .collect::<io::Result<Vec<_>>>()?;
    lines.sort_by(|left, right| {
        (
            left.asset_id.as_str(),
            left.issuer.as_str(),
            left.account.as_str(),
            left.trustline_id.as_str(),
        )
            .cmp(&(
                right.asset_id.as_str(),
                right.issuer.as_str(),
                right.account.as_str(),
                right.trustline_id.as_str(),
            ))
    });
    Ok(lines)
}

pub(super) fn asset_line_report(line: &TrustLine, asset: &AssetDefinition) -> AssetLineReport {
    AssetLineReport {
        trustline_id: line.trustline_id.clone(),
        account: line.account.clone(),
        issuer: line.issuer.clone(),
        asset_id: line.asset_id.clone(),
        code: asset.code.clone(),
        version: asset.version,
        precision: asset.precision,
        balance: line.balance,
        limit: line.limit,
        authorized: line.authorized,
        frozen: line.frozen,
        reserve_paid: line.reserve_paid,
    }
}

pub(super) fn metric_add(total: &mut u64, amount: u64, label: &str) -> io::Result<()> {
    *total = total.checked_add(amount).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("asset metrics {label} overflowed"),
        )
    })?;
    Ok(())
}

pub(super) fn metric_increment(total: &mut u64, label: &str) -> io::Result<()> {
    metric_add(total, 1, label)
}

pub(super) fn asset_metrics(ledger: &LedgerState) -> io::Result<AssetMetrics> {
    let mut metrics = AssetMetrics {
        asset_count: ledger.asset_definitions.len() as u64,
        ..AssetMetrics::default()
    };

    for asset in &ledger.asset_definitions {
        if asset.requires_authorization {
            metric_increment(
                &mut metrics.authorization_required_asset_count,
                "authorization required asset count",
            )?;
        }
        if asset.freeze_enabled {
            metric_increment(
                &mut metrics.freeze_enabled_asset_count,
                "freeze enabled asset count",
            )?;
        }
        if asset.clawback_enabled {
            metric_increment(
                &mut metrics.clawback_enabled_asset_count,
                "clawback enabled asset count",
            )?;
        }
    }

    for line in &ledger.trustlines {
        metric_increment(&mut metrics.trustline_count, "trustline count")?;
        if line.balance > 0 {
            metric_increment(&mut metrics.holder_count, "holder count")?;
        }
        metric_add(
            &mut metrics.total_outstanding_supply,
            line.balance,
            "total outstanding supply",
        )?;
        if ledger
            .asset_definition(&line.asset_id)
            .map(|asset| asset.requires_authorization)
            .unwrap_or(false)
            && !line.authorized
        {
            metric_increment(
                &mut metrics.unauthorized_trustline_count,
                "unauthorized trustline count",
            )?;
        }
        if line.frozen {
            metric_increment(
                &mut metrics.frozen_trustline_count,
                "frozen trustline count",
            )?;
        }
    }

    for escrow in ledger.escrows.iter().filter(|escrow| {
        escrow.state == ESCROW_STATE_OPEN && ledger.asset_definition(&escrow.asset_id).is_some()
    }) {
        metric_increment(
            &mut metrics.open_issued_escrow_count,
            "open issued escrow count",
        )?;
        metric_add(
            &mut metrics.open_issued_escrow_amount,
            escrow.amount,
            "open issued escrow amount",
        )?;
        metric_add(
            &mut metrics.total_outstanding_supply,
            escrow.amount,
            "total outstanding supply",
        )?;
    }

    for offer in ledger.offers.iter().filter(|offer| {
        offer.state == OFFER_STATE_OPEN
            && ledger
                .asset_definition(&offer.taker_gets_asset_id)
                .is_some()
    }) {
        metric_increment(
            &mut metrics.open_issued_offer_count,
            "open issued offer count",
        )?;
        metric_add(
            &mut metrics.open_issued_offer_amount,
            offer.taker_gets_amount_remaining,
            "open issued offer amount",
        )?;
        metric_add(
            &mut metrics.total_outstanding_supply,
            offer.taker_gets_amount_remaining,
            "total outstanding supply",
        )?;
    }

    Ok(metrics)
}

pub(super) fn issued_asset_open_offer_stats(
    ledger: &LedgerState,
    asset_id: &str,
) -> io::Result<(u64, u64)> {
    let mut open_offer_count = 0_u64;
    let mut open_offer_amount = 0_u64;
    for offer in ledger
        .offers
        .iter()
        .filter(|offer| offer.taker_gets_asset_id == asset_id && offer.state == OFFER_STATE_OPEN)
    {
        metric_increment(&mut open_offer_count, "open issued offer count")?;
        metric_add(
            &mut open_offer_amount,
            offer.taker_gets_amount_remaining,
            "open issued offer amount",
        )?;
    }
    Ok((open_offer_count, open_offer_amount))
}

pub(super) fn issued_asset_stats(
    ledger: &LedgerState,
    asset_id: &str,
) -> io::Result<(u64, u64, u64)> {
    let mut outstanding_supply = 0_u64;
    let mut trustline_count = 0_u64;
    let mut holder_count = 0_u64;
    for line in ledger
        .trustlines
        .iter()
        .filter(|line| line.asset_id == asset_id)
    {
        trustline_count = trustline_count.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "trustline count overflowed")
        })?;
        if line.balance > 0 {
            holder_count = holder_count.checked_add(1).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "holder count overflowed")
            })?;
        }
        outstanding_supply = outstanding_supply
            .checked_add(line.balance)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "issued asset outstanding supply overflowed",
                )
            })?;
    }
    for escrow in ledger.escrows.iter().filter(|escrow| {
        escrow.asset_id == asset_id && escrow.state == postfiat_types::ESCROW_STATE_OPEN
    }) {
        outstanding_supply = outstanding_supply
            .checked_add(escrow.amount)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "issued asset outstanding supply overflowed",
                )
            })?;
    }
    let (_, open_offer_amount) = issued_asset_open_offer_stats(ledger, asset_id)?;
    outstanding_supply = outstanding_supply
        .checked_add(open_offer_amount)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "issued asset outstanding supply overflowed",
            )
        })?;
    Ok((outstanding_supply, trustline_count, holder_count))
}

pub(super) fn offer_report(offer: &Offer) -> OfferReport {
    OfferReport {
        offer_id: offer.offer_id.clone(),
        owner: offer.owner.clone(),
        owner_sequence: offer.owner_sequence,
        taker_gets_asset_id: offer.taker_gets_asset_id.clone(),
        taker_gets_amount_remaining: offer.taker_gets_amount_remaining,
        taker_pays_asset_id: offer.taker_pays_asset_id.clone(),
        taker_pays_amount_remaining: offer.taker_pays_amount_remaining,
        original_taker_gets_amount: offer.original_taker_gets_amount,
        original_taker_pays_amount: offer.original_taker_pays_amount,
        created_height: offer.created_height,
        expiration_height: offer.expiration_height,
        reserve_paid: offer.reserve_paid,
        state: offer.state.clone(),
    }
}

pub(super) fn compare_offer_book_reports(
    left: &OfferReport,
    right: &OfferReport,
) -> std::cmp::Ordering {
    let left_price_num =
        (left.taker_pays_amount_remaining as u128) * (right.taker_gets_amount_remaining as u128);
    let right_price_num =
        (right.taker_pays_amount_remaining as u128) * (left.taker_gets_amount_remaining as u128);
    left_price_num
        .cmp(&right_price_num)
        .then_with(|| left.created_height.cmp(&right.created_height))
        .then_with(|| left.owner_sequence.cmp(&right.owner_sequence))
        .then_with(|| left.offer_id.cmp(&right.offer_id))
}

pub(super) fn escrow_report(escrow: &Escrow) -> io::Result<EscrowReport> {
    let condition_hash = if escrow.condition.is_empty() {
        None
    } else {
        Some(
            postfiat_types::escrow_condition_hash(&escrow.condition)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?,
        )
    };
    Ok(EscrowReport {
        escrow_id: escrow.escrow_id.clone(),
        owner: escrow.owner.clone(),
        owner_sequence: escrow.owner_sequence,
        recipient: escrow.recipient.clone(),
        asset_id: escrow.asset_id.clone(),
        amount: escrow.amount,
        fee: escrow.fee,
        condition_hash,
        finish_after: escrow.finish_after,
        cancel_after: escrow.cancel_after,
        state: escrow.state.clone(),
        created_height: escrow.created_height,
    })
}

pub(super) fn nft_report(nft: &NftDefinition) -> NftReport {
    NftReport {
        nft_id: nft.nft_id.clone(),
        issuer: nft.issuer.clone(),
        collection_id: nft.collection_id.clone(),
        serial: nft.serial,
        owner: nft.owner.clone(),
        metadata_hash: nft.metadata_hash.clone(),
        metadata_uri: nft.metadata_uri.clone(),
        flags: nft.flags,
        collection_flags: nft.collection_flags,
        issuer_transfer_fee: nft.issuer_transfer_fee,
        transferable: nft.flags & NFT_FLAG_TRANSFERABLE != 0,
        issuer_burnable: nft.flags & NFT_FLAG_ISSUER_BURNABLE != 0,
        collection_transfer_locked: nft.collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED != 0,
        collection_burn_locked: nft.collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED != 0,
        burned: nft.burned,
    }
}

pub(super) fn atomic_settlement_template_leg_report(
    quote: EscrowFeeQuoteReport,
    leg: &AtomicSettlementTemplateLeg,
    escrow_id: String,
) -> AtomicSettlementTemplateLegReport {
    AtomicSettlementTemplateLegReport {
        owner: leg.owner.clone(),
        recipient: leg.recipient.clone(),
        asset_id: leg.asset_id.clone(),
        amount: leg.amount,
        sequence: leg.owner_sequence,
        sequence_source: quote.sequence_source,
        escrow_id,
        transaction_kind: quote.transaction_kind,
        base_escrow_fee: quote.base_escrow_fee,
        state_expansion_fee: quote.state_expansion_fee,
        minimum_fee: quote.minimum_fee,
        escrow_weight_bytes: quote.escrow_weight_bytes,
        sender_balance: quote.sender_balance,
        sender_sequence: quote.sender_sequence,
        mempool_pending_for_sender: quote.mempool_pending_for_sender,
        sender_balance_after_fee: quote.sender_balance_after_fee,
        sender_meets_reserve_after_fee: quote.sender_meets_reserve_after_fee,
        operation: quote.operation,
    }
}

pub(super) fn validate_atomic_settlement_assets_exist(
    ledger: &LedgerState,
    template: &AtomicSettlementTemplate,
) -> io::Result<()> {
    for leg in [&template.left, &template.right] {
        if leg.asset_id != postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID
            && ledger.asset_definition(&leg.asset_id).is_none()
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("issued asset `{}` does not exist", leg.asset_id),
            ));
        }
    }
    Ok(())
}
pub(super) fn truncate_reports<T>(items: &mut Vec<T>, limit: usize) -> bool {
    if items.len() > limit {
        items.truncate(limit);
        true
    } else {
        false
    }
}

pub(super) fn validate_escrow_query_id(label: &str, value: &str) -> io::Result<()> {
    if value.len() != ESCROW_ID_HEX_LEN
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be {ESCROW_ID_HEX_LEN} lowercase hex characters"),
        ));
    }
    Ok(())
}

pub(super) fn validate_issued_asset_query_id(label: &str, value: &str) -> io::Result<()> {
    if value.len() != ISSUED_ASSET_ID_HEX_LEN
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be {ISSUED_ASSET_ID_HEX_LEN} lowercase hex characters"),
        ));
    }
    Ok(())
}

pub(super) fn validate_nft_query_id(label: &str, value: &str) -> io::Result<()> {
    if value.len() != NFT_ID_HEX_LEN
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be {NFT_ID_HEX_LEN} lowercase hex characters"),
        ));
    }
    Ok(())
}

pub(super) fn validate_offer_query_id(label: &str, value: &str) -> io::Result<()> {
    if value.len() != OFFER_ID_HEX_LEN
        || !value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be {OFFER_ID_HEX_LEN} lowercase hex characters"),
        ));
    }
    Ok(())
}

pub(super) fn validate_dex_asset_query_id(label: &str, value: &str) -> io::Result<()> {
    validate_query_text_field(label, value)?;
    if value == "PFT" {
        return Ok(());
    }
    validate_issued_asset_query_id(label, value)
}

pub(super) fn validate_nft_collection_query_id(value: &str) -> io::Result<()> {
    validate_query_text_field("collection_id", value)?;
    if value.len() > MAX_NFT_COLLECTION_ID_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("collection_id must not exceed {MAX_NFT_COLLECTION_ID_BYTES} bytes"),
        ));
    }
    Ok(())
}

pub(super) fn validate_escrow_role_filter(role: Option<&str>) -> io::Result<()> {
    match role {
        None | Some("owner") | Some("recipient") => Ok(()),
        Some(role) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("account_escrows role must be owner or recipient, got `{role}`"),
        )),
    }
}

pub(super) fn validate_escrow_state_filter(state: Option<&str>) -> io::Result<()> {
    match state {
        None
        | Some(ESCROW_STATE_OPEN)
        | Some(ESCROW_STATE_FINISHED)
        | Some(ESCROW_STATE_CANCELED) => Ok(()),
        Some(state) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "account_escrows state must be {ESCROW_STATE_OPEN}, {ESCROW_STATE_FINISHED}, or {ESCROW_STATE_CANCELED}, got `{state}`"
            ),
        )),
    }
}

pub(super) fn validate_offer_state_filter(state: Option<&str>) -> io::Result<()> {
    match state {
        None
        | Some(OFFER_STATE_OPEN)
        | Some(OFFER_STATE_FILLED)
        | Some(OFFER_STATE_CANCELED)
        | Some(OFFER_STATE_UNFUNDED) => Ok(()),
        Some(state) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "account_offers state must be {OFFER_STATE_OPEN}, {OFFER_STATE_FILLED}, {OFFER_STATE_CANCELED}, or {OFFER_STATE_UNFUNDED}, got `{state}`"
            ),
        )),
    }
}

pub(super) fn validate_query_text_field(label: &str, value: &str) -> io::Result<()> {
    if value.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be nonempty"),
        ));
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must not contain leading, trailing, or control whitespace"),
        ));
    }
    if value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} exceeds {MAX_TEXT_FIELD_BYTES} bytes"),
        ));
    }
    Ok(())
}

pub(super) fn validate_owner_public_key_hex(value: &str) -> io::Result<()> {
    if value.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "owner_public_key_hex must be nonempty",
        ));
    }
    if value != value.trim() || value.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "owner_public_key_hex must not contain leading, trailing, or control whitespace",
        ));
    }
    let expected_len = ML_DSA_65_PUBLIC_KEY_BYTES * 2;
    if value.len() != expected_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "owner_public_key_hex must be a hex-encoded ML-DSA-65 public key ({expected_len} characters)"
            ),
        ));
    }
    Ok(())
}

// Keep the included history test module after every production item in this
// module so strict Clippy can enforce the same module layout as ordinary files.
include!("history.rs");
