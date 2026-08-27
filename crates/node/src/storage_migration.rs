use super::*;
use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::sync::Arc;

pub const STORAGE_MIGRATION_MANIFEST_SCHEMA_V1: &str = "postfiat-storage-migration-manifest-v1";
pub const STORAGE_MIGRATION_REPORT_SCHEMA_V1: &str = "postfiat-storage-migration-report-v1";
pub const STORAGE_MIGRATION_MANIFEST_FILE: &str = "storage-migration-manifest.json";
pub const STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE: &str = "storage-migration-manifest.sha3-384";
pub const STORAGE_CANONICAL_EXPORT_FILE: &str = "canonical-history.jsonl";

#[derive(Debug, Clone)]
pub struct StorageMigrationOptions {
    pub data_dir: PathBuf,
    pub output_dir: PathBuf,
    pub expected_tip: String,
    pub expected_state_root: String,
    pub verify_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationManifestV1 {
    pub schema: String,
    pub verifier_version: String,
    pub storage_format: String,
    pub backend: String,
    pub backend_version: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub source_tip: ChainTipState,
    pub block_count: u64,
    pub receipt_count: u64,
    pub archive_count: u64,
    pub ordered_batch_count: u64,
    pub ordered_history_accumulator: String,
    pub blocks_root: String,
    pub receipts_root: String,
    pub archive_root: String,
    pub ordered_batches_root: String,
    pub current_state_root: String,
    pub history_checkpoint_root: Option<String>,
    pub validator_registry_root: String,
    pub logical_store_report: postfiat_storage::transactional::LogicalIntegrityReport,
    pub migration_packet_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageMigrationReportV1 {
    pub schema: String,
    pub verify_only: bool,
    pub published: bool,
    pub output_dir: PathBuf,
    pub required_disk_bytes: u64,
    pub available_disk_bytes: u64,
    pub source_tip: ChainTipState,
    pub migration_packet_root: String,
    pub manifest_file: PathBuf,
    pub manifest_checksum_file: PathBuf,
    pub canonical_export_file: PathBuf,
    pub canonical_export_receipt: postfiat_storage::CanonicalExportReceiptV1,
    pub generation_pointer:
        Option<postfiat_storage::transactional::TransactionalGenerationPointerV1>,
    pub logical_store_report: postfiat_storage::transactional::LogicalIntegrityReport,
}

pub fn rebuild_transactional_storage(
    options: StorageMigrationOptions,
) -> io::Result<StorageMigrationReportV1> {
    validate_expected_digest("expected tip", &options.expected_tip)?;
    validate_expected_digest("expected state root", &options.expected_state_root)?;
    let source = NodeStore::new(&options.data_dir);
    recover_ordered_commit_journal(&source)?;
    let genesis = source.read_genesis()?;
    let recorded_tip = read_chain_tip_or_reconstruct_for_genesis(&source, &genesis)?;
    let source_governance = source.read_governance()?;
    let governed_activation_height = source_governance.storage_commitment_activation_height();
    let source_activation_height = genesis
        .ordered_history_v2_activation_height
        .or(governed_activation_height);
    if genesis.ordered_history_v2_activation_height.is_none()
        && governed_activation_height.is_some_and(|height| recorded_tip.height < height)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "storage_migration_activation_already_scheduled: pre-activation rebuild must finish before scheduling activation",
        ));
    }

    // This authenticates every legacy history object and independently replays
    // execution, receipts, state roots, certificates, and the certified tip.
    verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let history_checkpoint = read_history_checkpoint_state_optional(&source)?;
    let mut source_tip = reconstruct_chain_tip_for_genesis(&source, &genesis)?;
    source_tip.receipt_count = source
        .read_blocks()?
        .blocks
        .iter()
        .try_fold(0_u64, |count, block| {
            count.checked_add(block.receipt_ids.len() as u64).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage_migration_receipt_count_overflow: canonical receipt occurrences overflow u64",
                )
            })
        })?;
    if recorded_tip.height != source_tip.height
        || recorded_tip.block_hash != source_tip.block_hash
        || recorded_tip.state_root != source_tip.state_root
        || recorded_tip.ordered_batch_count != source_tip.ordered_batch_count
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_recorded_tip_mismatch: recorded chain tip conflicts with authenticated retained history",
        ));
    }
    if source_tip.block_hash != options.expected_tip {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_expected_tip_mismatch: certified source tip does not match --expected-tip",
        ));
    }
    if source_tip.state_root != options.expected_state_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_expected_state_root_mismatch: certified source root does not match --expected-state-root",
        ));
    }

    let required_disk_bytes = required_rebuild_disk_bytes(&options.data_dir)?;
    let available_disk_bytes = available_disk_bytes(
        options
            .output_dir
            .parent()
            .unwrap_or_else(|| Path::new(".")),
    )?;
    if !options.verify_only && available_disk_bytes < required_disk_bytes {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            format!(
                "storage_migration_insufficient_disk: required {required_disk_bytes} bytes, available {available_disk_bytes} bytes"
            ),
        ));
    }

    if options.verify_only {
        return verify_existing_transactional_generation(
            &source,
            &genesis,
            &source_tip,
            &options.output_dir,
            required_disk_bytes,
            available_disk_bytes,
        );
    }
    prepare_empty_output_directory(&options.output_dir)?;

    let blocks = source.read_blocks()?;
    let receipts = source.read_receipts()?;
    let archive = source.read_batch_archive()?;
    let ordered_batches = source.read_ordered_batches()?;
    if blocks.blocks.len() as u64
        != source_tip
            .height
            .checked_sub(source_tip.history_base_height)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage_migration_source_height_underflow: retained history base exceeds the tip",
                )
            })?
        || ordered_batches.len() as u64 != source_tip.ordered_batch_count
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_source_count_mismatch: retained blocks or ordered history do not match the certified tip",
        ));
    }
    let receipts_by_block = persisted_receipts_by_block(&genesis, &blocks, &receipts)?;
    let canonical_receipts = receipts_by_block
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>();
    let ledger = source.read_ledger()?;
    let governance = source.read_governance()?;
    let shielded = source.read_shielded()?;
    let bridge = source.read_bridge()?;
    let node_state = source.read_node_state()?;
    let validator_registry =
        read_validator_registry_file(&source.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let validator_registry_bytes = serde_json::to_vec(&validator_registry).map_err(invalid_data)?;
    let history_checkpoint_bytes = history_checkpoint
        .as_ref()
        .map(serde_json::to_vec)
        .transpose()
        .map_err(invalid_data)?;
    let mut final_additional = vec![postfiat_storage::transactional::NamedStateValue {
        domain: "validator_registry".to_owned(),
        canonical_bytes: validator_registry_bytes,
    }];
    if let Some(bytes) = history_checkpoint_bytes.as_ref() {
        final_additional.push(postfiat_storage::transactional::NamedStateValue {
            domain: "retained_history_checkpoint".to_owned(),
            canonical_bytes: bytes.clone(),
        });
    }

    let genesis_hash_hex = genesis_hash(&genesis);
    let mut ordered_history = postfiat_storage::OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash_hex,
        genesis.protocol_version,
    )?;
    let mut rebuilt_tip = ChainTipState {
        schema: CHAIN_TIP_SCHEMA.to_owned(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        height: 0,
        block_hash: "genesis".to_owned(),
        state_root: "genesis".to_owned(),
        ordered_batch_count: 0,
        receipt_count: 0,
        history_base_height: 0,
    };
    let mut target = source.open_transactional_store_at(&options.output_dir)?;
    if let Some(checkpoint) = history_checkpoint.as_ref() {
        for batch_id in &checkpoint.ordered_batches {
            ordered_history = ordered_history.append(batch_id)?;
        }
        rebuilt_tip = ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_owned(),
            chain_id: checkpoint.chain_id.clone(),
            genesis_hash: checkpoint.genesis_hash.clone(),
            protocol_version: checkpoint.protocol_version,
            height: checkpoint.pruned_up_to_height,
            block_hash: checkpoint.checkpoint_block_hash.clone(),
            state_root: checkpoint.checkpoint_state_root.clone(),
            ordered_batch_count: checkpoint.ordered_batches.len() as u64,
            receipt_count: 0,
            history_base_height: checkpoint.pruned_up_to_height,
        };
        let checkpoint_registry_bytes =
            serde_json::to_vec(&checkpoint.validator_registry).map_err(invalid_data)?;
        let checkpoint_bytes = history_checkpoint_bytes.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "storage_migration_checkpoint_encoding_missing: retained checkpoint bytes are unavailable",
            )
        })?;
        let checkpoint_additional = [
            postfiat_storage::transactional::NamedStateValue {
                domain: "validator_registry".to_owned(),
                canonical_bytes: checkpoint_registry_bytes,
            },
            postfiat_storage::transactional::NamedStateValue {
                domain: "retained_history_checkpoint".to_owned(),
                canonical_bytes: checkpoint_bytes,
            },
        ];
        target.initialize_from_retained_checkpoint_with_activation(
            &rebuilt_tip,
            &ordered_history,
            &checkpoint.ordered_batches,
            postfiat_storage::CurrentStateUpdate {
                ledger: Some(&checkpoint.ledger),
                governance: Some(&checkpoint.governance),
                shielded: Some(&checkpoint.shielded),
                bridge: Some(&checkpoint.bridge),
                node_state: Some(&node_state),
                additional: &checkpoint_additional,
            },
            source_activation_height,
        )?;
    } else {
        target.initialize_with_activation(
            &rebuilt_tip,
            &ordered_history,
            if blocks.blocks.is_empty() {
                postfiat_storage::CurrentStateUpdate {
                    ledger: Some(&ledger),
                    governance: Some(&governance),
                    shielded: Some(&shielded),
                    bridge: Some(&bridge),
                    node_state: Some(&node_state),
                    additional: &final_additional,
                }
            } else {
                postfiat_storage::CurrentStateUpdate::default()
            },
            source_activation_height,
        )?;
    }

    for (index, block) in blocks.blocks.iter().enumerate() {
        let archive_entry = archive
            .find(&block.header.batch_kind, &block.header.batch_id)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "storage_migration_archive_missing: block {} has no archived payload",
                        block.header.height
                    ),
                )
            })?;
        let block_receipts = receipts_by_block.get(index).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "storage_migration_receipt_map_missing: block receipt mapping is incomplete",
            )
        })?;
        ordered_history = ordered_history.append(&block.header.batch_id)?;
        let next_receipt_count = rebuilt_tip
            .receipt_count
            .checked_add(block_receipts.len() as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "receipt count overflow"))?;
        let next_tip = ChainTipState {
            schema: rebuilt_tip.schema.clone(),
            chain_id: rebuilt_tip.chain_id.clone(),
            genesis_hash: rebuilt_tip.genesis_hash.clone(),
            protocol_version: rebuilt_tip.protocol_version,
            height: block.header.height,
            block_hash: block.header.block_hash.clone(),
            state_root: block.header.state_root.clone(),
            ordered_batch_count: ordered_history.count,
            receipt_count: next_receipt_count,
            history_base_height: rebuilt_tip.history_base_height,
        };
        let is_final = index + 1 == blocks.blocks.len();
        target.commit_finalized_block(postfiat_storage::CommitFinalizedBlock {
            expected_tip: &rebuilt_tip,
            new_tip: &next_tip,
            block,
            receipts: block_receipts,
            archive_entry,
            batch_id: &block.header.batch_id,
            ordered_history: &ordered_history,
            current_state: if is_final {
                postfiat_storage::CurrentStateUpdate {
                    ledger: Some(&ledger),
                    governance: Some(&governance),
                    shielded: Some(&shielded),
                    bridge: Some(&bridge),
                    node_state: Some(&node_state),
                    additional: &final_additional,
                }
            } else {
                postfiat_storage::CurrentStateUpdate::default()
            },
            scheduled_activation_height: source_activation_height,
            allow_legacy_receipt_id_mismatch: true,
        })?;
        rebuilt_tip = next_tip;
    }
    if rebuilt_tip != source_tip {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_rebuilt_tip_mismatch: rebuilt transactional tip differs from the certified source tip",
        ));
    }
    let logical_report = target.verify_logical_integrity()?;
    if !target.check_database_integrity()? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_database_integrity_failed: backend integrity check returned false",
        ));
    }
    let mut manifest = build_migration_manifest(
        &genesis,
        &source_tip,
        &blocks,
        &canonical_receipts,
        &archive,
        &ordered_batches,
        &ordered_history,
        &ledger,
        &governance,
        &shielded,
        &bridge,
        &node_state,
        history_checkpoint.as_ref(),
        &validator_registry,
        logical_report.clone(),
    )?;
    let transactional_manifest =
        build_transactional_migration_manifest(&target, &genesis, &source_tip, logical_report)?;
    if manifest != transactional_manifest {
        let fields = migration_manifest_mismatch_fields(&manifest, &transactional_manifest)?;
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "storage_migration_rebuilt_logical_mismatch: canonical transactional records differ from the authenticated legacy source; fields={fields}"
            ),
        ));
    }
    manifest.migration_packet_root = migration_manifest_root(&manifest)?;
    let logical_store_report = target.verify_and_bind_migration(&manifest.migration_packet_root)?;
    let canonical_export_file = options.output_dir.join(STORAGE_CANONICAL_EXPORT_FILE);
    let canonical_export_receipt = target.write_canonical_jsonl_export(&canonical_export_file)?;
    if target.verify_canonical_jsonl_export(&canonical_export_file)? != canonical_export_receipt {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_canonical_export_integrity_failure: written migration export did not reverify",
        ));
    }
    write_migration_manifest(&options.output_dir, &manifest)?;
    drop(target);

    let pointer = source
        .publish_transactional_generation(&options.output_dir, &manifest.migration_packet_root)?;
    Ok(StorageMigrationReportV1 {
        schema: STORAGE_MIGRATION_REPORT_SCHEMA_V1.to_owned(),
        verify_only: false,
        published: true,
        output_dir: fs::canonicalize(&options.output_dir)?,
        required_disk_bytes,
        available_disk_bytes,
        source_tip,
        migration_packet_root: manifest.migration_packet_root,
        manifest_file: options.output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE),
        manifest_checksum_file: options
            .output_dir
            .join(STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE),
        canonical_export_file,
        canonical_export_receipt,
        generation_pointer: Some(pointer),
        logical_store_report,
    })
}

fn verify_existing_transactional_generation(
    source: &NodeStore,
    genesis: &Genesis,
    source_tip: &ChainTipState,
    output_dir: &Path,
    required_disk_bytes: u64,
    available_disk_bytes: u64,
) -> io::Result<StorageMigrationReportV1> {
    if !output_dir.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "storage_migration_verify_output_missing: --verify-only requires an existing output directory",
        ));
    }
    let manifest = read_migration_manifest(output_dir)?;
    if manifest.chain_id != genesis.chain_id
        || manifest.genesis_hash != genesis_hash(genesis)
        || manifest.protocol_version != genesis.protocol_version
        || manifest.source_tip != *source_tip
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_manifest_domain_mismatch: manifest does not match the authenticated source",
        ));
    }
    let canonical_output = fs::canonicalize(output_dir)?;
    let target = match source.transactional_generation_pointer()? {
        Some(pointer) if pointer.database_directory == canonical_output => {
            source.transactional_store()?
        }
        _ => Arc::new(source.open_transactional_store_at(output_dir)?),
    };
    let logical_store_report = target.verify_logical_integrity()?;
    let blocks = source.read_blocks()?;
    let receipts = source.read_receipts()?;
    let canonical_receipts = persisted_receipts_by_block(genesis, &blocks, &receipts)?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let archive = source.read_batch_archive()?;
    let ordered_batches = source.read_ordered_batches()?;
    let ledger = source.read_ledger()?;
    let governance = source.read_governance()?;
    let shielded = source.read_shielded()?;
    let bridge = source.read_bridge()?;
    let node_state = source.read_node_state()?;
    let history_checkpoint = read_history_checkpoint_state_optional(source)?;
    let validator_registry =
        read_validator_registry_file(&source.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let mut ordered_history = postfiat_storage::OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash(genesis),
        genesis.protocol_version,
    )?;
    for batch_id in &ordered_batches {
        ordered_history = ordered_history.append(batch_id)?;
    }
    let mut expected_manifest = build_migration_manifest(
        genesis,
        source_tip,
        &blocks,
        &canonical_receipts,
        &archive,
        &ordered_batches,
        &ordered_history,
        &ledger,
        &governance,
        &shielded,
        &bridge,
        &node_state,
        history_checkpoint.as_ref(),
        &validator_registry,
        logical_store_report.clone(),
    )?;
    expected_manifest.migration_packet_root = migration_manifest_root(&expected_manifest)?;
    let mut transactional_manifest = build_transactional_migration_manifest(
        &target,
        genesis,
        source_tip,
        logical_store_report.clone(),
    )?;
    transactional_manifest.migration_packet_root =
        migration_manifest_root(&transactional_manifest)?;
    if manifest != expected_manifest || manifest != transactional_manifest {
        let source_fields = migration_manifest_mismatch_fields(&manifest, &expected_manifest)?;
        let transactional_fields =
            migration_manifest_mismatch_fields(&manifest, &transactional_manifest)?;
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "storage_migration_manifest_logical_mismatch: manifest roots differ from the authenticated source or canonical transactional export; source_fields={source_fields}; transactional_fields={transactional_fields}"
            ),
        ));
    }

    let meta = target.meta()?;
    if meta.chain_tip(source_tip.schema.clone()) != *source_tip
        || meta.last_full_verification_height != Some(source_tip.height)
        || meta.migration_packet_root.as_deref() != Some(manifest.migration_packet_root.as_str())
        || meta.verifier_version.as_deref()
            != Some(postfiat_storage::transactional::TRANSACTIONAL_VERIFIER_VERSION)
        || logical_store_report != manifest.logical_store_report
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_verify_mismatch: database metadata or logical entries differ from the manifest",
        ));
    }
    let canonical_export_file = output_dir.join(STORAGE_CANONICAL_EXPORT_FILE);
    let canonical_export_receipt = target.verify_canonical_jsonl_export(&canonical_export_file)?;
    Ok(StorageMigrationReportV1 {
        schema: STORAGE_MIGRATION_REPORT_SCHEMA_V1.to_owned(),
        verify_only: true,
        published: false,
        output_dir: fs::canonicalize(output_dir)?,
        required_disk_bytes,
        available_disk_bytes,
        source_tip: source_tip.clone(),
        migration_packet_root: manifest.migration_packet_root,
        manifest_file: output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE),
        manifest_checksum_file: output_dir.join(STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE),
        canonical_export_file,
        canonical_export_receipt,
        generation_pointer: None,
        logical_store_report,
    })
}

fn persisted_receipts_by_block(
    genesis: &Genesis,
    blocks: &BlockLog,
    persisted: &[Receipt],
) -> io::Result<Vec<Vec<Receipt>>> {
    let mut by_id = BTreeMap::<&str, Vec<&Receipt>>::new();
    for receipt in persisted {
        by_id
            .entry(receipt.tx_id.as_str())
            .or_default()
            .push(receipt);
    }
    let mut consumed = HashMap::<&str, usize>::new();
    let mut result = Vec::with_capacity(blocks.blocks.len());
    for block in &blocks.blocks {
        let mut block_receipts = Vec::with_capacity(block.receipt_ids.len());
        for receipt_id in &block.receipt_ids {
            let cursor = consumed.entry(receipt_id.as_str()).or_default();
            let receipt = by_id
                .get(receipt_id.as_str())
                .and_then(|receipts| receipts.get(*cursor))
                .copied()
                .or_else(|| {
                    archived_wan_devnet2_deduplicated_governance_receipt_allowed(
                        genesis,
                        block,
                        receipt_id,
                    )
                    .then(|| {
                        by_id
                            .get(receipt_id.as_str())
                            .and_then(|receipts| receipts.first())
                            .copied()
                    })
                    .flatten()
                })
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "storage_migration_receipt_missing: block {} receipt `{receipt_id}` has no literal persisted value",
                            block.header.height
                        ),
                    )
                })?;
            *cursor += 1;
            block_receipts.push(receipt.clone());
        }
        if block.header.receipt_count != block_receipts.len() as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "storage_migration_receipt_count_mismatch: block {} receipt count is inconsistent",
                    block.header.height
                ),
            ));
        }
        result.push(block_receipts);
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn build_migration_manifest(
    genesis: &Genesis,
    source_tip: &ChainTipState,
    blocks: &BlockLog,
    receipts: &[Receipt],
    archive: &BatchArchive,
    ordered_batches: &[String],
    ordered_history: &postfiat_storage::OrderedHistoryCommitment,
    ledger: &LedgerState,
    governance: &GovernanceState,
    shielded: &ShieldedState,
    bridge: &BridgeState,
    node_state: &NodeState,
    history_checkpoint: Option<&HistoryCheckpointState>,
    validator_registry: &ValidatorRegistry,
    logical_store_report: postfiat_storage::transactional::LogicalIntegrityReport,
) -> io::Result<StorageMigrationManifestV1> {
    Ok(StorageMigrationManifestV1 {
        schema: STORAGE_MIGRATION_MANIFEST_SCHEMA_V1.to_owned(),
        verifier_version: postfiat_storage::transactional::TRANSACTIONAL_VERIFIER_VERSION
            .to_owned(),
        storage_format: postfiat_storage::transactional::TRANSACTIONAL_STORAGE_FORMAT.to_owned(),
        backend: postfiat_storage::transactional::TRANSACTIONAL_BACKEND.to_owned(),
        backend_version: postfiat_storage::transactional::TRANSACTIONAL_BACKEND_VERSION.to_owned(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        source_tip: source_tip.clone(),
        block_count: blocks.blocks.len() as u64,
        receipt_count: receipts.len() as u64,
        archive_count: archive.batches.len() as u64,
        ordered_batch_count: ordered_batches.len() as u64,
        ordered_history_accumulator: ordered_history.accumulator.clone(),
        blocks_root: logical_root("postfiat.storage_migration.blocks.v1", blocks)?,
        receipts_root: logical_root("postfiat.storage_migration.receipts.v1", receipts)?,
        archive_root: logical_root("postfiat.storage_migration.archive.v1", archive)?,
        ordered_batches_root: logical_root(
            "postfiat.storage_migration.ordered_batches.v1",
            ordered_batches,
        )?,
        current_state_root: logical_root(
            "postfiat.storage_migration.current_state.v1",
            &(ledger, governance, shielded, bridge, node_state),
        )?,
        history_checkpoint_root: history_checkpoint
            .map(|checkpoint| {
                logical_root(
                    "postfiat.storage_migration.retained_history_checkpoint.v1",
                    checkpoint,
                )
            })
            .transpose()?,
        validator_registry_root: logical_root(
            "postfiat.storage_migration.validator_registry.v1",
            validator_registry,
        )?,
        logical_store_report,
        migration_packet_root: String::new(),
    })
}

fn build_transactional_migration_manifest(
    target: &postfiat_storage::TransactionalStore,
    genesis: &Genesis,
    source_tip: &ChainTipState,
    logical_store_report: postfiat_storage::transactional::LogicalIntegrityReport,
) -> io::Result<StorageMigrationManifestV1> {
    fn required_state<T: serde::de::DeserializeOwned>(
        target: &postfiat_storage::TransactionalStore,
        domain: &str,
    ) -> io::Result<T> {
        target.current_state(domain)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "storage_migration_transactional_state_missing: `{domain}` is absent from the rebuilt generation"
                ),
            )
        })
    }

    let blocks = BlockLog {
        blocks: target.blocks_in_height_order()?,
    };
    let receipts = target.receipts_in_block_order()?;
    let archive = BatchArchive {
        batches: target.archived_batches_in_block_order()?,
    };
    let ordered_batches = target.ordered_batches()?;
    let ledger = target.ledger()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_transactional_state_missing: `ledger` is absent from the rebuilt generation",
        )
    })?;
    let governance = target.governance()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_transactional_state_missing: `governance` is absent from the rebuilt generation",
        )
    })?;
    let shielded = target.shielded()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_transactional_state_missing: `shielded` is absent from the rebuilt generation",
        )
    })?;
    let bridge = target.bridge()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_transactional_state_missing: `bridge` is absent from the rebuilt generation",
        )
    })?;
    let node_state = target.node_state()?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_transactional_state_missing: `node_state` is absent from the rebuilt generation",
        )
    })?;
    let validator_registry: ValidatorRegistry = required_state(target, "validator_registry")?;
    let history_checkpoint: Option<HistoryCheckpointState> =
        target.current_state("retained_history_checkpoint")?;
    let mut ordered_history = postfiat_storage::OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash(genesis),
        genesis.protocol_version,
    )?;
    for batch_id in &ordered_batches {
        ordered_history = ordered_history.append(batch_id)?;
    }
    build_migration_manifest(
        genesis,
        source_tip,
        &blocks,
        &receipts,
        &archive,
        &ordered_batches,
        &ordered_history,
        &ledger,
        &governance,
        &shielded,
        &bridge,
        &node_state,
        history_checkpoint.as_ref(),
        &validator_registry,
        logical_store_report,
    )
}

fn migration_manifest_mismatch_fields(
    expected: &StorageMigrationManifestV1,
    observed: &StorageMigrationManifestV1,
) -> io::Result<String> {
    let expected = serde_json::to_value(expected).map_err(invalid_data)?;
    let observed = serde_json::to_value(observed).map_err(invalid_data)?;
    let expected = expected.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "storage migration manifest did not encode as an object",
        )
    })?;
    let observed = observed.as_object().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "transactional migration manifest did not encode as an object",
        )
    })?;
    let fields = expected
        .iter()
        .filter_map(|(field, value)| (observed.get(field) != Some(value)).then_some(field.as_str()))
        .collect::<Vec<_>>();
    Ok(if fields.is_empty() {
        "none".to_owned()
    } else {
        fields.join(",")
    })
}

fn logical_root<T: Serialize + ?Sized>(domain: &str, value: &T) -> io::Result<String> {
    let encoded = serde_json::to_vec(value).map_err(invalid_data)?;
    Ok(hash_hex(domain, &encoded))
}

fn migration_manifest_root(manifest: &StorageMigrationManifestV1) -> io::Result<String> {
    let mut canonical = manifest.clone();
    canonical.migration_packet_root.clear();
    logical_root("postfiat.storage_migration.packet.v1", &canonical)
}

fn write_migration_manifest(
    output_dir: &Path,
    manifest: &StorageMigrationManifestV1,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(manifest).map_err(invalid_data)?;
    atomic_write(
        output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE),
        format!("{json}\n"),
    )?;
    atomic_write(
        output_dir.join(STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE),
        format!(
            "{}  {}\n",
            manifest.migration_packet_root, STORAGE_MIGRATION_MANIFEST_FILE
        ),
    )
}

fn read_migration_manifest(output_dir: &Path) -> io::Result<StorageMigrationManifestV1> {
    let path = output_dir.join(STORAGE_MIGRATION_MANIFEST_FILE);
    let raw = fs::read_to_string(&path).map_err(|error| {
        let reason = if error.kind() == io::ErrorKind::NotFound {
            "storage_migration_manifest_missing"
        } else {
            "storage_migration_manifest_read_failed"
        };
        io::Error::new(error.kind(), format!("{reason}: {error}"))
    })?;
    let manifest: StorageMigrationManifestV1 = serde_json::from_str(&raw).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("storage_migration_manifest_invalid: {error}"),
        )
    })?;
    let computed_root = migration_manifest_root(&manifest).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("storage_migration_manifest_invalid: {error}"),
        )
    })?;
    if manifest.schema != STORAGE_MIGRATION_MANIFEST_SCHEMA_V1
        || manifest.verifier_version
            != postfiat_storage::transactional::TRANSACTIONAL_VERIFIER_VERSION
        || computed_root != manifest.migration_packet_root
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_manifest_invalid: schema, verifier, or packet root mismatch",
        ));
    }
    let checksum = fs::read_to_string(output_dir.join(STORAGE_MIGRATION_MANIFEST_CHECKSUM_FILE))
        .map_err(|error| {
            let reason = if error.kind() == io::ErrorKind::NotFound {
                "storage_migration_manifest_checksum_missing"
            } else {
                "storage_migration_manifest_checksum_read_failed"
            };
            io::Error::new(error.kind(), format!("{reason}: {error}"))
        })?;
    if checksum
        != format!(
            "{}  {}\n",
            manifest.migration_packet_root, STORAGE_MIGRATION_MANIFEST_FILE
        )
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_migration_manifest_checksum_mismatch: checksum file is not exact",
        ));
    }
    Ok(manifest)
}

fn prepare_empty_output_directory(output_dir: &Path) -> io::Result<()> {
    if output_dir.exists() {
        if !output_dir.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "storage_migration_output_not_directory: output path exists and is not a directory",
            ));
        }
        if fs::read_dir(output_dir)?.next().transpose()?.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "storage_migration_output_not_empty: refusing to overwrite an existing generation",
            ));
        }
    } else {
        fs::create_dir_all(output_dir)?;
    }
    Ok(())
}

fn required_rebuild_disk_bytes(data_dir: &Path) -> io::Result<u64> {
    fn walk(path: &Path, total: &mut u64) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                walk(&entry.path(), total)?;
            } else if file_type.is_file() {
                *total = total.checked_add(entry.metadata()?.len()).ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "source size overflow")
                })?;
            }
        }
        Ok(())
    }
    let mut source_bytes = 0u64;
    walk(data_dir, &mut source_bytes)?;
    source_bytes
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(64 * 1024 * 1024))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "required disk overflow"))
}

fn available_disk_bytes(path: &Path) -> io::Result<u64> {
    let existing = path
        .ancestors()
        .find(|candidate| candidate.exists())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no existing output ancestor"))?;
    let path = CString::new(existing.as_os_str().as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "output path contains a NUL byte",
        )
    })?;
    let mut stats = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: `path` is a NUL-terminated CString and `stats` points to writable
    // memory of the exact type required by statvfs.
    if unsafe { libc::statvfs(path.as_ptr(), stats.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statvfs returned success and initialized the output structure.
    let stats = unsafe { stats.assume_init() };
    stats
        .f_bavail
        .checked_mul(stats.f_frsize)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "available disk overflow"))
}

fn validate_expected_digest(label: &str, value: &str) -> io::Result<()> {
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
