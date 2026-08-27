use super::*;

pub const STORAGE_ACTIVATION_TEMPLATE_REPORT_SCHEMA_V1: &str =
    "postfiat-storage-activation-template-report-v1";
pub const STORAGE_CANCELLATION_TEMPLATE_REPORT_SCHEMA_V1: &str =
    "postfiat-storage-cancellation-template-report-v1";

#[derive(Debug, Clone)]
pub struct StorageActivationTemplateOptions {
    pub data_dir: PathBuf,
    pub activation_height: u64,
    pub record_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageActivationTemplateReportV1 {
    pub schema: String,
    pub authorization_kind: String,
    pub record: postfiat_types::StorageCommitmentActivationRecordV1,
}

#[derive(Debug, Clone)]
pub struct StorageActivationRatificationOptions {
    pub data_dir: PathBuf,
    pub record_file: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub amendment_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StorageActivationBatchOptions {
    pub data_dir: PathBuf,
    pub record_file: PathBuf,
    pub authorization_amendment_file: PathBuf,
    pub batch_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StorageCancellationTemplateOptions {
    pub data_dir: PathBuf,
    pub activation_id: String,
    pub reason: String,
    pub record_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageCancellationTemplateReportV1 {
    pub schema: String,
    pub authorization_kind: String,
    pub record: postfiat_types::StorageCommitmentCancellationRecordV1,
}

#[derive(Debug, Clone)]
pub struct StorageCancellationRatificationOptions {
    pub data_dir: PathBuf,
    pub record_file: PathBuf,
    pub validators: Vec<String>,
    pub support: Vec<String>,
    pub amendment_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct StorageCancellationBatchOptions {
    pub data_dir: PathBuf,
    pub record_file: PathBuf,
    pub authorization_amendment_file: PathBuf,
    pub batch_file: PathBuf,
}

fn validate_sha3_384_digest(label: &str, value: &str) -> io::Result<()> {
    if value.len() != 96
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be a lowercase SHA3-384 digest"),
        ));
    }
    Ok(())
}

fn write_exclusive_json<T: Serialize>(path: &Path, value: &T, label: &str) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{label} already exists: {}", path.display()),
        ));
    }
    let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn fully_verified_migration_meta(
    store: &NodeStore,
    tip: &ChainTipState,
) -> io::Result<postfiat_storage::TransactionalStoreMetaV1> {
    if !store.transactional_storage_configured()? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "storage_activation_migration_missing: no transactional generation is published",
        ));
    }
    let meta = store.transactional_store()?.meta()?;
    if meta.chain_tip(tip.schema.clone()) != *tip
        || meta.last_full_verification_height != Some(tip.height)
        || meta.migration_packet_root.is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_activation_migration_stale: transactional generation is not fully verified at the certified tip",
        ));
    }
    if meta.scheduled_activation_height.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "storage_activation_already_scheduled: transactional generation already has an activation height",
        ));
    }
    Ok(meta)
}

pub fn create_storage_activation_template(
    options: StorageActivationTemplateOptions,
) -> io::Result<StorageActivationTemplateReportV1> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    if governance.storage_commitment_activation_height().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "storage_activation_already_recorded: governance already contains an uncancelled activation",
        ));
    }
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let scheduling_block_height = tip.height.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_activation_height_overflow: scheduling height exceeds u64",
        )
    })?;
    if options.activation_height <= scheduling_block_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_activation_not_future: activation height must follow its scheduling block",
        ));
    }
    if tip.ordered_batch_count != tip.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_activation_frozen_count_mismatch: ordered count does not equal frozen height",
        ));
    }
    let meta = fully_verified_migration_meta(&store, &tip)?;
    let migration_packet_root = meta.migration_packet_root.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_activation_packet_root_missing: migrated generation has no packet root",
        )
    })?;
    let mut record = postfiat_types::StorageCommitmentActivationRecordV1 {
        schema: postfiat_types::STORAGE_COMMITMENT_ACTIVATION_SCHEMA_V1.to_owned(),
        feature_id: postfiat_types::STORAGE_COMMITMENT_FEATURE_ID_V1.to_owned(),
        activation_id: "0".repeat(96),
        authorization_amendment_id: "0".repeat(96),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        scheduling_block_height,
        activation_height: options.activation_height,
        legacy_commitment_version: postfiat_types::STORAGE_COMMITMENT_LEGACY_VERSION_V1.to_owned(),
        new_commitment_version: postfiat_types::STORAGE_COMMITMENT_NEW_VERSION_V1.to_owned(),
        pre_activation_finalized_height: tip.height,
        pre_activation_block_hash: tip.block_hash,
        pre_activation_state_root: tip.state_root,
        pre_activation_ordered_count: tip.ordered_batch_count,
        pre_activation_ordered_accumulator: meta.ordered_history_accumulator,
        migration_packet_root,
        required_verifier_version: postfiat_types::STORAGE_COMMITMENT_VERIFIER_VERSION_V2
            .to_owned(),
    };
    record.activation_id = record
        .expected_activation_id()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_exclusive_json(&options.record_file, &record, "storage activation record")?;
    Ok(StorageActivationTemplateReportV1 {
        schema: STORAGE_ACTIVATION_TEMPLATE_REPORT_SCHEMA_V1.to_owned(),
        authorization_kind: record.authorization_kind(),
        record,
    })
}

pub fn ratify_storage_activation(
    options: StorageActivationRatificationOptions,
) -> io::Result<GovernanceAmendment> {
    let record: postfiat_types::StorageCommitmentActivationRecordV1 =
        read_json_file(&options.record_file, "storage activation record")?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if record.authorization_amendment_id != "0".repeat(96) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_activation_template_already_authorized: record does not contain the authorization placeholder",
        ));
    }
    let value = u32::try_from(record.activation_height).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_activation_height_out_of_range: activation height exceeds governance value range",
        )
    })?;
    ratify_governance(RatifyGovernanceOptions {
        data_dir: options.data_dir,
        validators: options.validators,
        support: options.support,
        kind: record.authorization_kind(),
        value,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: options.amendment_file,
    })
}

pub fn create_storage_activation_batch(
    options: StorageActivationBatchOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let mut record: postfiat_types::StorageCommitmentActivationRecordV1 =
        read_json_file(&options.record_file, "storage activation record")?;
    let authorization: GovernanceAmendment = read_json_file(
        &options.authorization_amendment_file,
        "storage activation authorization amendment",
    )?;
    if authorization.kind != record.authorization_kind() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_activation_authorization_kind_mismatch: amendment kind does not bind the activation ID",
        ));
    }
    record.authorization_amendment_id = authorization.amendment_id.clone();
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let batch = build_storage_commitment_activation_batch(&genesis, authorization, record)?;
    verify_storage_commitment_action_readiness(&store, &genesis, &batch, &tip)?;
    write_exclusive_json(&options.batch_file, &batch, "storage activation batch")?;
    Ok(batch)
}

pub fn create_storage_cancellation_template(
    options: StorageCancellationTemplateOptions,
) -> io::Result<StorageCancellationTemplateReportV1> {
    validate_sha3_384_digest("activation id", &options.activation_id)?;
    if options.reason.trim().is_empty() || options.reason.len() > 1024 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_cancellation_reason_invalid: reason must contain 1..=1024 bytes",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let activation = governance
        .scheduled_storage_commitment_activation()
        .filter(|activation| activation.activation_id == options.activation_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "storage_cancellation_activation_missing: activation ID is not active in governance",
            )
        })?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let cancellation_height = tip.height.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_cancellation_height_overflow: cancellation height exceeds u64",
        )
    })?;
    if cancellation_height >= activation.activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_cancellation_too_late: cancellation block must precede activation",
        ));
    }
    let mut record = postfiat_types::StorageCommitmentCancellationRecordV1 {
        schema: postfiat_types::STORAGE_COMMITMENT_CANCELLATION_SCHEMA_V1.to_owned(),
        cancellation_id: "0".repeat(96),
        activation_id: options.activation_id,
        authorization_amendment_id: "0".repeat(96),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        cancellation_height,
        reason: options.reason,
    };
    record.cancellation_id = record
        .expected_cancellation_id()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    write_exclusive_json(&options.record_file, &record, "storage cancellation record")?;
    Ok(StorageCancellationTemplateReportV1 {
        schema: STORAGE_CANCELLATION_TEMPLATE_REPORT_SCHEMA_V1.to_owned(),
        authorization_kind: record.authorization_kind(),
        record,
    })
}

pub fn ratify_storage_cancellation(
    options: StorageCancellationRatificationOptions,
) -> io::Result<GovernanceAmendment> {
    let record: postfiat_types::StorageCommitmentCancellationRecordV1 =
        read_json_file(&options.record_file, "storage cancellation record")?;
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if record.authorization_amendment_id != "0".repeat(96) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_cancellation_template_already_authorized: record does not contain the authorization placeholder",
        ));
    }
    let value = u32::try_from(record.cancellation_height).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_cancellation_height_out_of_range: cancellation height exceeds governance value range",
        )
    })?;
    ratify_governance(RatifyGovernanceOptions {
        data_dir: options.data_dir,
        validators: options.validators,
        support: options.support,
        kind: record.authorization_kind(),
        value,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: options.amendment_file,
    })
}

pub fn create_storage_cancellation_batch(
    options: StorageCancellationBatchOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let mut record: postfiat_types::StorageCommitmentCancellationRecordV1 =
        read_json_file(&options.record_file, "storage cancellation record")?;
    let authorization: GovernanceAmendment = read_json_file(
        &options.authorization_amendment_file,
        "storage cancellation authorization amendment",
    )?;
    if authorization.kind != record.authorization_kind() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_cancellation_authorization_kind_mismatch: amendment kind does not bind the cancellation ID",
        ));
    }
    record.authorization_amendment_id = authorization.amendment_id.clone();
    record
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let batch = build_storage_commitment_cancellation_batch(&genesis, authorization, record)?;
    verify_storage_commitment_action_readiness(&store, &genesis, &batch, &tip)?;
    write_exclusive_json(&options.batch_file, &batch, "storage cancellation batch")?;
    Ok(batch)
}
