pub fn history_status(options: HistoryOptions) -> io::Result<HistoryStatusReport> {
    let data_dir = options.data_dir.clone();
    let store = NodeStore::new(&data_dir);
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })?;
    let blocks = store.read_blocks()?;
    let receipts = store.read_receipts()?;
    let archive = store.read_batch_archive()?;
    let ordered_batches = store.read_ordered_batches()?;
    let governance = store.read_governance()?;
    let block_log_verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map(|report| report.verified)
    .unwrap_or(false);
    Ok(HistoryStatusReport {
        schema: "postfiat-history-status-v1".to_string(),
        chain_id: status_report.chain_id,
        genesis_hash: status_report.genesis_hash,
        protocol_version: status_report.protocol_version,
        node_id: status_report.node_id,
        current_height: status_report.block_height,
        block_tip_hash: status_report.block_tip_hash,
        policy: history_policy_report(&options),
        local_block_range: history_block_range(&blocks),
        receipt_count: receipts.len(),
        archived_batch_count: archive.batches.len(),
        ordered_batch_count: ordered_batches.len(),
        governance_amendment_count: governance.amendments.len(),
        governance_registry_update_count: governance.validator_registry_updates.len(),
        block_log_verified,
        storage_files: history_storage_files(&store)?,
        partial_history_ready: block_log_verified && options.mode == DEFAULT_HISTORY_MODE,
    })
}

pub fn create_history_archive_handoff(
    options: HistoryArchiveHandoffCreateOptions,
) -> io::Result<HistoryArchiveHandoffProof> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "history archive handoff proof",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let proof = build_history_archive_handoff_proof(
        &store,
        options.from_height,
        options.to_height,
        options.archive_uri.unwrap_or_default(),
    )?;
    write_history_archive_handoff_proof_file(&options.output_file, &proof)?;
    Ok(proof)
}

pub fn verify_history_archive_handoff(
    options: HistoryArchiveHandoffVerifyOptions,
) -> io::Result<HistoryArchiveHandoffVerifyReport> {
    let store = NodeStore::new(&options.data_dir);
    let proof: HistoryArchiveHandoffProof =
        read_json_file(&options.proof_file, "history archive handoff proof")?;
    if proof.schema != "postfiat-history-archive-handoff-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history archive handoff schema `{}`",
                proof.schema
            ),
        ));
    }
    let expected = build_history_archive_handoff_proof(
        &store,
        proof.from_height,
        proof.to_height,
        proof.archive_uri.clone(),
    )?;
    if proof != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history archive handoff proof does not match local history",
        ));
    }
    Ok(HistoryArchiveHandoffVerifyReport {
        schema: "postfiat-history-archive-handoff-verify-v1".to_string(),
        verified: true,
        proof_file: options.proof_file.display().to_string(),
        chain_id: proof.chain_id,
        genesis_hash: proof.genesis_hash,
        protocol_version: proof.protocol_version,
        archive_uri: proof.archive_uri,
        from_height: proof.from_height,
        to_height: proof.to_height,
        block_count: proof.block_count,
        batch_count: proof.batch_count,
        receipt_count: proof.receipt_count,
        proof_hash: proof.proof_hash,
        block_range_root: proof.block_range_root,
        batch_payload_root: proof.batch_payload_root,
        receipt_root: proof.receipt_root,
    })
}

pub fn export_history_archive_window(
    options: HistoryArchiveWindowExportOptions,
) -> io::Result<HistoryArchiveWindowBundle> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "history archive window bundle",
    )?;
    let bundle = build_history_archive_window(HistoryArchiveWindowBuildOptions {
        data_dir: options.data_dir,
        from_height: options.from_height,
        to_height: options.to_height,
        archive_uri: options.archive_uri,
    })?;
    write_history_archive_window_bundle_file(&options.output_file, &bundle)?;
    Ok(bundle)
}

pub fn build_history_archive_window(
    options: HistoryArchiveWindowBuildOptions,
) -> io::Result<HistoryArchiveWindowBundle> {
    let store = NodeStore::new(&options.data_dir);
    verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let genesis = store.read_genesis()?;
    let chain_id = genesis.chain_id.clone();
    let genesis_hash_hex = genesis_hash(&genesis);
    let protocol_version = genesis.protocol_version;
    let blocks = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let (selected_blocks, selected_batches, selected_receipts) = select_history_archive_window(
        &blocks,
        &archive,
        &receipts,
        options.from_height,
        options.to_height,
    )?;
    let proof = history_archive_handoff_proof_from_window(HistoryArchiveWindowProofInput {
        chain_id: chain_id.as_str(),
        genesis_hash: genesis_hash_hex.as_str(),
        protocol_version,
        archive_uri: options.archive_uri.as_deref().unwrap_or_default(),
        from_height: options.from_height,
        to_height: options.to_height,
        selected_blocks: &selected_blocks,
        selected_batches: &selected_batches,
        selected_receipts: &selected_receipts,
    })?;
    let mut bundle = HistoryArchiveWindowBundle {
        schema: "postfiat-history-archive-window-v1".to_string(),
        proof,
        blocks: selected_blocks,
        batches: selected_batches,
        receipts: selected_receipts,
        bundle_hash: String::new(),
    };
    bundle.bundle_hash = history_archive_window_bundle_hash(&bundle)?;
    Ok(bundle)
}

pub fn verify_history_archive_window_bundle(
    options: HistoryArchiveWindowVerifyOptions,
) -> io::Result<HistoryArchiveWindowVerifyReport> {
    Ok(verify_history_archive_window_bundle_data(&options.bundle_file)?.report)
}

struct VerifiedHistoryArchiveWindowBundle {
    report: HistoryArchiveWindowVerifyReport,
    bundle: HistoryArchiveWindowBundle,
}

fn verify_history_archive_window_bundle_data(
    bundle_file: &Path,
) -> io::Result<VerifiedHistoryArchiveWindowBundle> {
    let bundle: HistoryArchiveWindowBundle =
        read_json_file(bundle_file, "history archive window bundle")?;
    if bundle.schema != "postfiat-history-archive-window-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history archive window schema `{}`",
                bundle.schema
            ),
        ));
    }
    let expected_bundle_hash = history_archive_window_bundle_hash(&bundle)?;
    if bundle.bundle_hash != expected_bundle_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history archive window bundle hash mismatch",
        ));
    }
    let expected_proof =
        history_archive_handoff_proof_from_window(HistoryArchiveWindowProofInput {
            chain_id: bundle.proof.chain_id.as_str(),
            genesis_hash: bundle.proof.genesis_hash.as_str(),
            protocol_version: bundle.proof.protocol_version,
            archive_uri: bundle.proof.archive_uri.as_str(),
            from_height: bundle.proof.from_height,
            to_height: bundle.proof.to_height,
            selected_blocks: &bundle.blocks,
            selected_batches: &bundle.batches,
            selected_receipts: &bundle.receipts,
        })?;
    if bundle.proof != expected_proof {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history archive window proof does not match bundled contents",
        ));
    }
    let report = HistoryArchiveWindowVerifyReport {
        schema: "postfiat-history-archive-window-verify-v1".to_string(),
        verified: true,
        bundle_file: bundle_file.display().to_string(),
        chain_id: bundle.proof.chain_id.clone(),
        genesis_hash: bundle.proof.genesis_hash.clone(),
        protocol_version: bundle.proof.protocol_version,
        from_height: bundle.proof.from_height,
        to_height: bundle.proof.to_height,
        block_count: bundle.blocks.len(),
        batch_count: bundle.batches.len(),
        receipt_count: bundle.receipts.len(),
        proof_hash: bundle.proof.proof_hash.clone(),
        bundle_hash: bundle.bundle_hash.clone(),
    };
    Ok(VerifiedHistoryArchiveWindowBundle { report, bundle })
}

pub fn import_history_archive_window(
    options: HistoryArchiveWindowImportOptions,
) -> io::Result<HistoryArchiveWindowImportReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let verified = verify_history_archive_window_bundle_data(&options.bundle_file)?;
    let report = verified.report;
    let bundle = verified.bundle;
    if report.chain_id != genesis.chain_id
        || report.genesis_hash != genesis_hash(&genesis)
        || report.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history archive window domain does not match local genesis",
        ));
    }
    let archive_file = history_archive_window_file_path(&store, &bundle);
    let archive_file_display = archive_file.display().to_string();
    let mut index = read_history_archive_window_index(&store)?;
    let entry = HistoryArchiveWindowIndexEntry {
        from_height: report.from_height,
        to_height: report.to_height,
        block_count: report.block_count,
        batch_count: report.batch_count,
        receipt_count: report.receipt_count,
        proof_hash: report.proof_hash.clone(),
        bundle_hash: report.bundle_hash.clone(),
        archive_file: archive_file_display.clone(),
    };

    let existing_same_hash = index
        .windows
        .iter()
        .position(|existing| existing.bundle_hash == entry.bundle_hash);
    if let Some(index_position) = existing_same_hash {
        if index.windows[index_position] != entry {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "history archive window index contains conflicting entry for bundle hash",
            ));
        }
    } else {
        for existing in &index.windows {
            let overlaps =
                entry.from_height <= existing.to_height && existing.from_height <= entry.to_height;
            if overlaps {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "history archive window overlaps an existing imported archive window",
                ));
            }
        }
    }

    let mut imported = false;
    if archive_file.exists() && !options.overwrite {
        let existing: HistoryArchiveWindowBundle =
            read_json_file(&archive_file, "history archive window bundle")?;
        if existing != bundle {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "history archive window file exists with different contents",
            ));
        }
    } else {
        write_history_archive_window_bundle_file(&archive_file, &bundle)?;
        imported = true;
    }

    if existing_same_hash.is_none() {
        index.windows.push(entry);
        sort_history_archive_window_index(&mut index);
        write_history_archive_window_index(&store, &index)?;
        imported = true;
    }

    Ok(HistoryArchiveWindowImportReport {
        schema: "postfiat-history-archive-window-import-v1".to_string(),
        imported,
        bundle_file: options.bundle_file.display().to_string(),
        archive_file: archive_file_display,
        chain_id: report.chain_id,
        genesis_hash: report.genesis_hash,
        protocol_version: report.protocol_version,
        from_height: report.from_height,
        to_height: report.to_height,
        block_count: report.block_count,
        batch_count: report.batch_count,
        receipt_count: report.receipt_count,
        proof_hash: report.proof_hash,
        bundle_hash: report.bundle_hash,
        archived_window_count: index.windows.len(),
    })
}

struct VerifiedHistoryArchivePrefix {
    blocks: BlockLog,
    archive: BatchArchive,
    receipts: Vec<Receipt>,
    proof: HistoryArchiveHandoffProof,
    window_count: usize,
}

struct HistoryVerificationState<'a> {
    governance: &'a GovernanceState,
    ledger: &'a LedgerState,
    ordered_batches: &'a [String],
    shielded: &'a ShieldedState,
    bridge: &'a BridgeState,
    validator_registry: &'a ValidatorRegistry,
    blocks: &'a BlockLog,
    archive: &'a BatchArchive,
    receipts: &'a [Receipt],
    checkpoint: Option<&'a HistoryCheckpointState>,
}

/// Rebuilds an unverifiable v1 prune checkpoint solely from imported, hash-bound
/// archive windows and the canonical genesis replay base.
///
/// The node must be stopped while this operator recovery runs. The legacy
/// checkpoint contributes only its domain and prune boundary; none of its
/// economic state is trusted. Both the rebuilt prefix and the retained suffix
/// are verified in isolated shadow stores before the live checkpoint is
/// replaced atomically.
pub fn history_checkpoint_rebuild_from_archive(
    options: HistoryCheckpointRebuildFromArchiveOptions,
) -> io::Result<HistoryCheckpointRebuildFromArchiveReport> {
    let store = NodeStore::new(&options.data_dir);
    if read_history_prune_pending_optional(&store)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint rebuild refused while prune recovery is pending",
        ));
    }
    let checkpoint_file = store.data_dir().join(HISTORY_CHECKPOINT_FILE);
    if options.backup_file == checkpoint_file {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "history checkpoint backup path must differ from the live checkpoint path",
        ));
    }
    ensure_output_can_be_written(&options.backup_file, false, "legacy history checkpoint backup")?;
    let legacy_bytes = std::fs::read(&checkpoint_file)?;
    let legacy: HistoryCheckpointState =
        serde_json::from_slice(&legacy_bytes).map_err(invalid_data)?;
    if legacy.schema != "postfiat-history-checkpoint-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "history checkpoint archive rebuild requires schema `postfiat-history-checkpoint-v1`, found `{}`",
                legacy.schema
            ),
        ));
    }
    let genesis = store.read_genesis()?;
    if legacy.chain_id != genesis.chain_id
        || legacy.genesis_hash != genesis_hash(&genesis)
        || legacy.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy history checkpoint domain does not match local genesis",
        ));
    }
    if legacy.pruned_up_to_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "legacy history checkpoint prune boundary must be positive",
        ));
    }

    let prefix = verified_history_archive_prefix(&store, legacy.pruned_up_to_height)?;
    let rebuilt = build_history_checkpoint_state_from_sources(
        &store,
        legacy.pruned_up_to_height,
        &prefix.proof,
        None,
        &prefix.blocks,
        &prefix.archive,
    )?;
    validate_history_checkpoint_state(&store, &rebuilt)?;
    if rebuilt.checkpoint_block_hash != legacy.checkpoint_block_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive-rebuilt checkpoint boundary block does not match the legacy checkpoint",
        ));
    }

    let prefix_verification = verify_history_rebuild_shadow(
        &store,
        "prefix",
        HistoryVerificationState {
            governance: &rebuilt.governance,
            ledger: &rebuilt.ledger,
            ordered_batches: &rebuilt.ordered_batches,
            shielded: &rebuilt.shielded,
            bridge: &rebuilt.bridge,
            validator_registry: &rebuilt.validator_registry,
            blocks: &prefix.blocks,
            archive: &prefix.archive,
            receipts: &prefix.receipts,
            checkpoint: None,
        },
    )?;

    let live_governance = store.read_governance()?;
    let live_ledger = store.read_ledger()?;
    let live_ordered_batches = store.read_ordered_batches()?;
    let live_shielded = store.read_shielded()?;
    let live_bridge = store.read_bridge()?;
    let live_validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let live_blocks = store.read_blocks()?;
    let live_archive = store.read_batch_archive()?;
    let live_receipts = store.read_receipts()?;
    let retained_suffix_verification = verify_history_rebuild_shadow(
        &store,
        "suffix",
        HistoryVerificationState {
            governance: &live_governance,
            ledger: &live_ledger,
            ordered_batches: &live_ordered_batches,
            shielded: &live_shielded,
            bridge: &live_bridge,
            validator_registry: &live_validator_registry,
            blocks: &live_blocks,
            archive: &live_archive,
            receipts: &live_receipts,
            checkpoint: Some(&rebuilt),
        },
    )?;

    if std::fs::read(&checkpoint_file)? != legacy_bytes {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "live history checkpoint changed during archive rebuild; retry with the node stopped",
        ));
    }
    atomic_write(&options.backup_file, &legacy_bytes)?;
    let rebuilt_json = serde_json::to_string_pretty(&rebuilt).map_err(invalid_data)?;
    atomic_write_checked(&checkpoint_file, format!("{rebuilt_json}\n"), |candidate_file| {
        let candidate: HistoryCheckpointState =
            read_json_file(candidate_file, "rebuilt history checkpoint candidate")?;
        if candidate != rebuilt {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "rebuilt history checkpoint changed during atomic write",
            ));
        }
        validate_history_checkpoint_state(&store, &candidate)
    })?;
    let final_verification = match verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    }) {
        Ok(report) => report,
        Err(error) => {
            atomic_write(&checkpoint_file, &legacy_bytes)?;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "rebuilt history checkpoint failed final verification and the legacy checkpoint was restored: {error}"
                ),
            ));
        }
    };
    if final_verification != retained_suffix_verification {
        atomic_write(&checkpoint_file, &legacy_bytes)?;
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "live history changed during checkpoint replacement; legacy checkpoint restored",
        ));
    }

    Ok(HistoryCheckpointRebuildFromArchiveReport {
        schema: "postfiat-history-checkpoint-rebuild-from-archive-v1".to_string(),
        rebuilt: true,
        legacy_schema: legacy.schema,
        legacy_checkpoint_file: checkpoint_file.display().to_string(),
        backup_file: options.backup_file.display().to_string(),
        pruned_up_to_height: rebuilt.pruned_up_to_height,
        archive_from_height: prefix.proof.from_height,
        archive_to_height: prefix.proof.to_height,
        archive_window_count: prefix.window_count,
        prefix_verification,
        retained_suffix_verification: final_verification,
        checkpoint: rebuilt,
    })
}

fn verified_history_archive_prefix(
    store: &NodeStore,
    to_height: u64,
) -> io::Result<VerifiedHistoryArchivePrefix> {
    let genesis = store.read_genesis()?;
    let expected_chain_id = genesis.chain_id.as_str();
    let expected_genesis_hash = genesis_hash(&genesis);
    let index = read_history_archive_window_index(store)?;
    let mut blocks = Vec::new();
    let mut batches = Vec::new();
    let mut receipts = Vec::new();
    let mut expected_from_height = 1u64;
    let mut window_count = 0usize;

    for entry in &index.windows {
        if expected_from_height > to_height {
            break;
        }
        if entry.from_height != expected_from_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "imported archive windows do not continuously cover height {expected_from_height}"
                ),
            ));
        }
        let archive_file = PathBuf::from(&entry.archive_file);
        let verified = verify_history_archive_window_bundle_data(&archive_file)?;
        let expected_file = history_archive_window_file_path(store, &verified.bundle);
        if archive_file != expected_file
            || verified.report.from_height != entry.from_height
            || verified.report.to_height != entry.to_height
            || verified.report.block_count != entry.block_count
            || verified.report.batch_count != entry.batch_count
            || verified.report.receipt_count != entry.receipt_count
            || verified.report.proof_hash != entry.proof_hash
            || verified.report.bundle_hash != entry.bundle_hash
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "imported archive window index entry does not match its verified bundle",
            ));
        }
        if verified.report.chain_id != expected_chain_id
            || verified.report.genesis_hash != expected_genesis_hash
            || verified.report.protocol_version != genesis.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "imported archive window domain does not match local genesis",
            ));
        }
        blocks.extend(verified.bundle.blocks);
        batches.extend(verified.bundle.batches);
        receipts.extend(verified.bundle.receipts);
        expected_from_height = entry.to_height.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "archive window height overflow")
        })?;
        window_count = window_count.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "archive window count overflow")
        })?;
    }
    if expected_from_height <= to_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("imported archive windows do not cover height {to_height}"),
        ));
    }

    let all_blocks = BlockLog { blocks };
    let all_archive = BatchArchive { batches };
    let (selected_blocks, selected_batches, selected_receipts) = select_history_archive_window(
        &all_blocks,
        &all_archive,
        &receipts,
        1,
        to_height,
    )?;
    let proof = history_archive_handoff_proof_from_window(HistoryArchiveWindowProofInput {
        chain_id: expected_chain_id,
        genesis_hash: expected_genesis_hash.as_str(),
        protocol_version: genesis.protocol_version,
        archive_uri: "archive://postfiat/imported-window-rebuild-v1",
        from_height: 1,
        to_height,
        selected_blocks: &selected_blocks,
        selected_batches: &selected_batches,
        selected_receipts: &selected_receipts,
    })?;
    Ok(VerifiedHistoryArchivePrefix {
        blocks: BlockLog {
            blocks: selected_blocks,
        },
        archive: BatchArchive {
            batches: selected_batches,
        },
        receipts: selected_receipts,
        proof,
        window_count,
    })
}

fn verify_history_rebuild_shadow(
    source_store: &NodeStore,
    label: &str,
    state: HistoryVerificationState<'_>,
) -> io::Result<BlockVerificationReport> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(invalid_data)?
        .as_nanos();
    let parent = source_store
        .data_dir()
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let shadow_dir = parent.join(format!(
        ".postfiat-history-rebuild-{label}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir(&shadow_dir)?;
    let result = (|| {
        for file_name in [GENESIS_FILE, FAUCET_ACCOUNT_FILE, VALIDATOR_REGISTRY_GENESIS_FILE] {
            std::fs::copy(
                source_store.data_dir().join(file_name),
                shadow_dir.join(file_name),
            )?;
        }
        let shadow_store = NodeStore::new(&shadow_dir);
        write_validator_registry_file(
            &shadow_dir.join(VALIDATOR_REGISTRY_FILE),
            state.validator_registry,
        )?;
        shadow_store.write_governance(state.governance)?;
        shadow_store.write_ledger(state.ledger)?;
        shadow_store.write_ordered_batches(state.ordered_batches)?;
        shadow_store.write_shielded(state.shielded)?;
        shadow_store.write_bridge(state.bridge)?;
        shadow_store.write_blocks(state.blocks)?;
        shadow_store.write_batch_archive(state.archive)?;
        shadow_store.write_receipts(state.receipts)?;
        if let Some(checkpoint) = state.checkpoint {
            write_history_checkpoint_state_file(
                &shadow_dir.join(HISTORY_CHECKPOINT_FILE),
                checkpoint,
            )?;
        }
        verify_blocks(NodeOptions {
            data_dir: shadow_dir.clone(),
        })
    })();
    let cleanup = std::fs::remove_dir_all(&shadow_dir);
    match (result, cleanup) {
        (Ok(report), Ok(())) => Ok(report),
        (Err(error), _) => Err(error),
        (Ok(_), Err(error)) => Err(io::Error::new(
            error.kind(),
            format!("history checkpoint rebuild shadow cleanup failed: {error}"),
        )),
    }
}

pub fn history_prune_plan(options: HistoryOptions) -> io::Result<HistoryPrunePlanReport> {
    let data_dir = options.data_dir.clone();
    let store = NodeStore::new(&data_dir);
    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })?;
    let blocks = store.read_blocks()?;
    let receipts = store.read_receipts()?;
    let archive = store.read_batch_archive()?;
    let base_height = history_base_height(&store)?;
    let block_log_verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map(|report| report.verified)
    .unwrap_or(false);
    let effective_retain_blocks = options
        .retain_recent_blocks
        .max(options.minimum_replay_window_blocks);
    let computed_prune_up_to_height = status_report
        .block_height
        .checked_sub(effective_retain_blocks)
        .filter(|height| *height > 0);
    let effective_prune_up_to_height =
        match (options.prune_up_to_height, computed_prune_up_to_height) {
            (Some(requested), Some(computed)) => Some(requested.min(computed)),
            (None, computed) => computed,
            (Some(_), None) => None,
        };
    let retain_from_height = effective_prune_up_to_height
        .and_then(|height| height.checked_add(1))
        .unwrap_or(1);
    let eligible_block_count = effective_prune_up_to_height
        .map(|boundary| {
            blocks
                .blocks
                .iter()
                .filter(|block| block.header.height <= boundary)
                .count()
        })
        .unwrap_or(0);
    let eligible_batch_ids = effective_prune_up_to_height
        .map(|boundary| {
            blocks
                .blocks
                .iter()
                .filter(|block| block.header.height <= boundary)
                .map(|block| {
                    (
                        block.header.batch_kind.clone(),
                        block.header.batch_id.clone(),
                    )
                })
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let eligible_receipt_ids = effective_prune_up_to_height
        .map(|boundary| {
            blocks
                .blocks
                .iter()
                .filter(|block| block.header.height <= boundary)
                .flat_map(|block| block.receipt_ids.iter().cloned())
                .collect::<HashSet<_>>()
        })
        .unwrap_or_default();
    let eligible_batch_count = archive
        .batches
        .iter()
        .filter(|entry| {
            eligible_batch_ids.contains(&(entry.batch_kind.clone(), entry.batch_id.clone()))
        })
        .count();
    let eligible_receipt_count = receipts
        .iter()
        .filter(|receipt| eligible_receipt_ids.contains(&receipt.tx_id))
        .count();
    let (archive_handoff_present, archive_handoff_verified, archive_handoff_error) =
        match options.archive_handoff_file.as_ref() {
            Some(proof_file) => {
                match verify_history_archive_handoff(HistoryArchiveHandoffVerifyOptions {
                    data_dir: data_dir.clone(),
                    proof_file: proof_file.clone(),
                }) {
                    Ok(report) => {
                        let covers_boundary =
                            effective_prune_up_to_height.is_some_and(|boundary| {
                                report.from_height <= base_height.saturating_add(1)
                                    && report.to_height >= boundary
                            });
                        if covers_boundary {
                            (true, true, None)
                        } else {
                            (
                                true,
                                false,
                                Some(
                                    "archive handoff proof does not cover the prune boundary"
                                        .to_string(),
                                ),
                            )
                        }
                    }
                    Err(error) => (true, false, Some(error.to_string())),
                }
            }
            None => (false, false, None),
        };
    let mut refusal_reasons = Vec::new();
    if computed_prune_up_to_height.is_none() {
        refusal_reasons
            .push("current height is inside the configured retention window".to_string());
    }
    if let (Some(requested), Some(computed)) =
        (options.prune_up_to_height, computed_prune_up_to_height)
    {
        if requested > computed {
            refusal_reasons.push(format!(
                "requested prune height {requested} exceeds computed safe boundary {computed}"
            ));
        }
    }
    if !block_log_verified {
        refusal_reasons.push("block log verification failed".to_string());
    }
    if options.archive_handoff_required && !archive_handoff_verified {
        if archive_handoff_present {
            let detail = archive_handoff_error
                .as_deref()
                .unwrap_or("archive handoff proof did not verify");
            refusal_reasons.push(format!(
                "archive handoff proof is required but invalid: {detail}"
            ));
        } else {
            refusal_reasons.push("archive handoff proof is required but not present".to_string());
        }
    }
    let prune_allowed = refusal_reasons.is_empty();
    Ok(HistoryPrunePlanReport {
        schema: "postfiat-history-prune-plan-v1".to_string(),
        dry_run: true,
        chain_id: status_report.chain_id,
        genesis_hash: status_report.genesis_hash,
        protocol_version: status_report.protocol_version,
        node_id: status_report.node_id,
        current_height: status_report.block_height,
        policy: history_policy_report(&options),
        requested_prune_up_to_height: options.prune_up_to_height,
        computed_prune_up_to_height,
        retain_from_height,
        eligible_block_count,
        eligible_batch_count,
        eligible_receipt_count,
        block_log_verified,
        archive_handoff_present,
        archive_handoff_verified,
        archive_handoff_error,
        prune_allowed,
        refusal_reasons,
    })
}

pub(super) struct HistoryPruneArtifacts {
    pub(super) pending: HistoryPrunePending,
    pub(super) before_block_range: HistoryRangeReport,
    pub(super) after_block_range: HistoryRangeReport,
    pub(super) remaining_blocks: BlockLog,
    pub(super) remaining_archive: BatchArchive,
    pub(super) remaining_receipts: Vec<Receipt>,
}

pub fn history_prune(options: HistoryOptions) -> io::Result<HistoryPruneReport> {
    let store = NodeStore::new(&options.data_dir);
    if read_history_prune_pending_optional(&store)?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history prune recovery is pending; run history-prune-recover before pruning again",
        ));
    }
    let plan = history_prune_plan(options.clone())?;
    if !plan.prune_allowed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("history prune refused: {}", plan.refusal_reasons.join("; ")),
        ));
    }
    let prune_up_to_height = plan
        .requested_prune_up_to_height
        .or(plan.computed_prune_up_to_height)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "history prune has no effective prune boundary",
            )
        })?;
    let proof_file = options.archive_handoff_file.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "history prune requires --archive-handoff-file",
        )
    })?;
    let proof: HistoryArchiveHandoffProof =
        read_json_file(proof_file, "history archive handoff proof")?;
    if proof.to_height < prune_up_to_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "history archive handoff proof does not cover prune boundary",
        ));
    }

    let blocks = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let checkpoint = build_history_checkpoint_state(&store, prune_up_to_height, &proof)?;
    let artifacts = build_history_prune_artifacts(&plan, checkpoint, blocks, archive, receipts)?;

    write_history_prune_pending_file(&store, &artifacts.pending)?;
    write_history_checkpoint_state_file(
        &store.data_dir().join(HISTORY_CHECKPOINT_FILE),
        &artifacts.pending.checkpoint,
    )?;
    store.write_blocks(&artifacts.remaining_blocks)?;
    store.write_batch_archive(&artifacts.remaining_archive)?;
    store.write_receipts(&artifacts.remaining_receipts)?;

    let verify_after_prune = verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    append_history_prune_journal_record(&store, artifacts.pending.journal_record.clone())?;
    remove_history_prune_pending_file(&store)?;
    let remaining_receipt_count = artifacts.remaining_receipts.len();
    let remaining_batch_count = artifacts.remaining_archive.batches.len();
    let pruned_block_count = artifacts.pending.journal_record.pruned_block_count;
    let pruned_batch_count = artifacts.pending.journal_record.pruned_batch_count;
    let pruned_receipt_count = artifacts.pending.journal_record.pruned_receipt_count;

    Ok(HistoryPruneReport {
        schema: "postfiat-history-prune-v1".to_string(),
        pruned: true,
        plan,
        checkpoint: artifacts.pending.checkpoint,
        journal_record: artifacts.pending.journal_record,
        before_block_range: artifacts.before_block_range,
        after_block_range: artifacts.after_block_range,
        pruned_block_count,
        pruned_batch_count,
        pruned_receipt_count,
        remaining_receipt_count,
        remaining_batch_count,
        verify_after_prune,
    })
}

pub fn history_prune_recover(options: NodeOptions) -> io::Result<HistoryPruneRecoveryReport> {
    let store = NodeStore::new(&options.data_dir);
    let pending_file = store.data_dir().join(HISTORY_PRUNE_PENDING_FILE);
    let Some(pending) = read_history_prune_pending_optional(&store)? else {
        return Ok(HistoryPruneRecoveryReport {
            schema: "postfiat-history-prune-recovery-v1".to_string(),
            recovered: false,
            pending_file: pending_file.display().to_string(),
            pruned_up_to_height: None,
            checkpoint_hash: None,
            prune_id: None,
            pruned_block_count: None,
            pruned_batch_count: None,
            pruned_receipt_count: None,
            before_block_range: None,
            after_block_range: None,
            verify_after_recovery: None,
        });
    };

    let blocks = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let before_block_range = history_block_range(&blocks);
    let pruned_batch_keys = pending
        .pruned_batch_keys
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let pruned_receipt_ids = pending
        .pruned_receipt_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let remaining_blocks = BlockLog {
        blocks: blocks
            .blocks
            .into_iter()
            .filter(|block| block.header.height > pending.checkpoint.pruned_up_to_height)
            .collect(),
    };
    let remaining_archive = BatchArchive {
        batches: archive
            .batches
            .into_iter()
            .filter(|entry| {
                !pruned_batch_keys.contains(&HistoryPruneBatchKey {
                    batch_kind: entry.batch_kind.clone(),
                    batch_id: entry.batch_id.clone(),
                })
            })
            .collect(),
    };
    let remaining_receipts = receipts
        .into_iter()
        .filter(|receipt| !pruned_receipt_ids.contains(&receipt.tx_id))
        .collect::<Vec<_>>();
    let after_block_range = history_block_range(&remaining_blocks);
    if after_block_range.count != pending.journal_record.remaining_block_count
        || after_block_range.first_height != pending.journal_record.remaining_first_height
        || after_block_range.last_height != pending.journal_record.remaining_last_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history prune recovery retained block range does not match pending journal record",
        ));
    }

    write_history_checkpoint_state_file(
        &store.data_dir().join(HISTORY_CHECKPOINT_FILE),
        &pending.checkpoint,
    )?;
    store.write_blocks(&remaining_blocks)?;
    store.write_batch_archive(&remaining_archive)?;
    store.write_receipts(&remaining_receipts)?;
    let verify_after_recovery = verify_blocks(NodeOptions {
        data_dir: options.data_dir,
    })?;
    append_history_prune_journal_record(&store, pending.journal_record.clone())?;
    remove_history_prune_pending_file(&store)?;

    Ok(HistoryPruneRecoveryReport {
        schema: "postfiat-history-prune-recovery-v1".to_string(),
        recovered: true,
        pending_file: pending_file.display().to_string(),
        pruned_up_to_height: Some(pending.checkpoint.pruned_up_to_height),
        checkpoint_hash: Some(pending.checkpoint.checkpoint_hash),
        prune_id: Some(pending.journal_record.prune_id),
        pruned_block_count: Some(pending.journal_record.pruned_block_count),
        pruned_batch_count: Some(pending.journal_record.pruned_batch_count),
        pruned_receipt_count: Some(pending.journal_record.pruned_receipt_count),
        before_block_range: Some(before_block_range),
        after_block_range: Some(after_block_range),
        verify_after_recovery: Some(verify_after_recovery),
    })
}

fn history_policy_report(options: &HistoryOptions) -> HistoryRetentionPolicyReport {
    HistoryRetentionPolicyReport {
        mode: options.mode.clone(),
        retain_recent_blocks: options.retain_recent_blocks,
        retain_recent_receipts: options.retain_recent_receipts,
        retain_recent_batches: options.retain_recent_batches,
        retain_recent_governance: options.retain_recent_governance,
        minimum_replay_window_blocks: options.minimum_replay_window_blocks,
        advisory_prune: options.advisory_prune,
        archive_handoff_required: options.archive_handoff_required,
    }
}

fn history_block_range(blocks: &BlockLog) -> HistoryRangeReport {
    HistoryRangeReport {
        first_height: blocks.blocks.first().map(|block| block.header.height),
        last_height: blocks.blocks.last().map(|block| block.header.height),
        count: blocks.blocks.len(),
    }
}

fn history_storage_files(store: &NodeStore) -> io::Result<Vec<HistoryStorageFileReport>> {
    [
        GENESIS_FILE,
        GOVERNANCE_FILE,
        LEDGER_FILE,
        NODE_STATE_FILE,
        BLOCKS_FILE,
        BLOCKS_APPEND_FILE,
        BATCH_ARCHIVE_FILE,
        BATCH_ARCHIVE_APPEND_FILE,
        ORDERED_BATCHES_FILE,
        RECEIPTS_FILE,
        MEMPOOL_FILE,
        SHIELDED_FILE,
        BRIDGE_FILE,
        VALIDATOR_REGISTRY_FILE,
        VALIDATOR_REGISTRY_GENESIS_FILE,
        HISTORY_CHECKPOINT_FILE,
        HISTORY_PRUNE_PENDING_FILE,
        HISTORY_PRUNE_JOURNAL_FILE,
        HISTORY_ARCHIVE_INDEX_FILE,
    ]
    .into_iter()
    .map(|file_name| history_storage_file_report(store, file_name))
    .collect()
}

fn history_storage_file_report(
    store: &NodeStore,
    file_name: &str,
) -> io::Result<HistoryStorageFileReport> {
    let path = store.data_dir().join(file_name);
    let bytes = match std::fs::metadata(&path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error),
    };
    Ok(HistoryStorageFileReport {
        path: path.display().to_string(),
        bytes,
    })
}

pub(super) fn build_history_prune_artifacts(
    plan: &HistoryPrunePlanReport,
    checkpoint: HistoryCheckpointState,
    blocks: BlockLog,
    archive: BatchArchive,
    receipts: Vec<Receipt>,
) -> io::Result<HistoryPruneArtifacts> {
    let prune_up_to_height = checkpoint.pruned_up_to_height;
    let before_block_range = history_block_range(&blocks);
    let pruned_batch_keys = blocks
        .blocks
        .iter()
        .filter(|block| block.header.height <= prune_up_to_height)
        .map(|block| HistoryPruneBatchKey {
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
        })
        .collect::<HashSet<_>>();
    let pruned_receipt_ids = blocks
        .blocks
        .iter()
        .filter(|block| block.header.height <= prune_up_to_height)
        .flat_map(|block| block.receipt_ids.iter().cloned())
        .collect::<HashSet<_>>();

    let remaining_blocks = BlockLog {
        blocks: blocks
            .blocks
            .into_iter()
            .filter(|block| block.header.height > prune_up_to_height)
            .collect(),
    };
    let remaining_archive = BatchArchive {
        batches: archive
            .batches
            .into_iter()
            .filter(|entry| {
                !pruned_batch_keys.contains(&HistoryPruneBatchKey {
                    batch_kind: entry.batch_kind.clone(),
                    batch_id: entry.batch_id.clone(),
                })
            })
            .collect(),
    };
    let remaining_receipts = receipts
        .into_iter()
        .filter(|receipt| !pruned_receipt_ids.contains(&receipt.tx_id))
        .collect::<Vec<_>>();

    let pruned_block_count = plan.eligible_block_count;
    let pruned_batch_count = plan.eligible_batch_count;
    let pruned_receipt_count = plan.eligible_receipt_count;
    if pruned_block_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "history prune would not remove any blocks",
        ));
    }
    if pruned_block_count + remaining_blocks.blocks.len() != before_block_range.count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history prune block count mismatch",
        ));
    }

    let after_block_range = history_block_range(&remaining_blocks);
    let journal_record = build_history_prune_journal_record(
        &checkpoint,
        pruned_block_count,
        pruned_batch_count,
        pruned_receipt_count,
        &after_block_range,
    )?;
    let pending = HistoryPrunePending {
        schema: "postfiat-history-prune-pending-v1".to_string(),
        checkpoint,
        journal_record,
        pruned_batch_keys: sorted_history_prune_batch_keys(pruned_batch_keys),
        pruned_receipt_ids: sorted_history_prune_receipt_ids(pruned_receipt_ids),
    };

    Ok(HistoryPruneArtifacts {
        pending,
        before_block_range,
        after_block_range,
        remaining_blocks,
        remaining_archive,
        remaining_receipts,
    })
}

fn sorted_history_prune_batch_keys(
    keys: HashSet<HistoryPruneBatchKey>,
) -> Vec<HistoryPruneBatchKey> {
    let mut keys = keys.into_iter().collect::<Vec<_>>();
    keys.sort_by(|left, right| {
        left.batch_kind
            .cmp(&right.batch_kind)
            .then_with(|| left.batch_id.cmp(&right.batch_id))
    });
    keys
}

fn sorted_history_prune_receipt_ids(ids: HashSet<String>) -> Vec<String> {
    let mut ids = ids.into_iter().collect::<Vec<_>>();
    ids.sort();
    ids
}

pub(super) fn read_history_checkpoint_state_optional(
    store: &NodeStore,
) -> io::Result<Option<HistoryCheckpointState>> {
    let path = store.data_dir().join(HISTORY_CHECKPOINT_FILE);
    match read_json_file(&path, "history checkpoint") {
        Ok(checkpoint) => {
            validate_history_checkpoint_state(store, &checkpoint)?;
            Ok(Some(checkpoint))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn validate_history_checkpoint_state(
    store: &NodeStore,
    checkpoint: &HistoryCheckpointState,
) -> io::Result<()> {
    if checkpoint.schema != "postfiat-history-checkpoint-v2" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history checkpoint schema `{}`",
                checkpoint.schema
            ),
        ));
    }
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    if checkpoint.chain_id != genesis.chain_id
        || checkpoint.genesis_hash != genesis_hash_hex
        || checkpoint.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint domain does not match local genesis",
        ));
    }
    let expected_hash = history_checkpoint_hash(checkpoint)?;
    if checkpoint.checkpoint_hash != expected_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint hash mismatch",
        ));
    }
    let checkpoint_ordered_len = usize::try_from(checkpoint.pruned_up_to_height).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint height overflows local usize",
        )
    })?;
    if checkpoint.ordered_batches.len() != checkpoint_ordered_len {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint ordered batch prefix length does not match pruned height",
        ));
    }
    let expected_state_root = replicated_state_root(
        &genesis,
        &checkpoint.governance,
        &checkpoint.ledger,
        &checkpoint.ordered_batches,
        &checkpoint.shielded,
        &checkpoint.bridge,
    )?;
    if checkpoint.checkpoint_state_root != expected_state_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint state root mismatch",
        ));
    }
    let checkpoint_native_supply =
        native_pft_live_total(&checkpoint.ledger, &checkpoint.shielded)?;
    let checkpoint_native_fee_burn = checkpoint.native_fee_burn_total.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "history checkpoint v2 is missing cumulative native fee burn",
        )
    })?;
    let checkpoint_accounted_supply = checkpoint_native_supply
        .checked_add(checkpoint_native_fee_burn)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "history checkpoint native supply accounting overflow",
            )
        })?;
    if checkpoint_accounted_supply != u128::from(genesis.expected_native_supply_atoms()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "history checkpoint live native supply {checkpoint_native_supply} plus cumulative fee burn {checkpoint_native_fee_burn} does not equal genesis native supply {}",
                genesis.expected_native_supply_atoms()
            ),
        ));
    }
    validate_validator_registry_for_count(
        &checkpoint.validator_registry,
        checkpoint.governance.active_validator_count,
    )
}

fn read_history_prune_pending_optional(
    store: &NodeStore,
) -> io::Result<Option<HistoryPrunePending>> {
    let path = store.data_dir().join(HISTORY_PRUNE_PENDING_FILE);
    match read_json_file(&path, "history prune pending") {
        Ok(pending) => {
            validate_history_prune_pending(store, &pending)?;
            Ok(Some(pending))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn validate_history_prune_pending(
    store: &NodeStore,
    pending: &HistoryPrunePending,
) -> io::Result<()> {
    if pending.schema != "postfiat-history-prune-pending-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history prune pending schema `{}`",
                pending.schema
            ),
        ));
    }
    validate_history_checkpoint_state(store, &pending.checkpoint)?;
    if pending.journal_record.schema != "postfiat-history-prune-journal-record-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history prune journal record schema `{}`",
                pending.journal_record.schema
            ),
        ));
    }
    if pending.journal_record.pruned_up_to_height != pending.checkpoint.pruned_up_to_height
        || pending.journal_record.checkpoint_hash != pending.checkpoint.checkpoint_hash
        || pending.journal_record.archive_handoff_proof_hash
            != pending.checkpoint.archive_handoff_proof_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history prune pending journal does not match checkpoint",
        ));
    }
    let mut batch_keys = HashSet::new();
    for key in &pending.pruned_batch_keys {
        if !batch_keys.insert(key) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "history prune pending contains duplicate batch keys",
            ));
        }
    }
    let mut receipt_ids = HashSet::new();
    for receipt_id in &pending.pruned_receipt_ids {
        if !receipt_ids.insert(receipt_id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "history prune pending contains duplicate receipt ids",
            ));
        }
    }
    Ok(())
}

pub(super) fn write_history_prune_pending_file(
    store: &NodeStore,
    pending: &HistoryPrunePending,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(pending).map_err(invalid_data)?;
    atomic_write(
        store.data_dir().join(HISTORY_PRUNE_PENDING_FILE),
        format!("{json}\n"),
    )
}

fn remove_history_prune_pending_file(store: &NodeStore) -> io::Result<()> {
    let path = store.data_dir().join(HISTORY_PRUNE_PENDING_FILE);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

pub(super) fn write_history_checkpoint_state_file(
    path: &Path,
    checkpoint: &HistoryCheckpointState,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(checkpoint).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_history_prune_journal(store: &NodeStore) -> io::Result<HistoryPruneJournal> {
    let path = store.data_dir().join(HISTORY_PRUNE_JOURNAL_FILE);
    match read_json_file(&path, "history prune journal") {
        Ok(journal) => Ok(journal),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HistoryPruneJournal {
            schema: "postfiat-history-prune-journal-v1".to_string(),
            records: Vec::new(),
        }),
        Err(error) => Err(error),
    }
}

fn write_history_prune_journal(store: &NodeStore, journal: &HistoryPruneJournal) -> io::Result<()> {
    let json = serde_json::to_string_pretty(journal).map_err(invalid_data)?;
    atomic_write(
        store.data_dir().join(HISTORY_PRUNE_JOURNAL_FILE),
        format!("{json}\n"),
    )
}

fn append_history_prune_journal_record(
    store: &NodeStore,
    record: HistoryPruneJournalRecord,
) -> io::Result<()> {
    let mut journal = read_history_prune_journal(store)?;
    if journal.schema != "postfiat-history-prune-journal-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history prune journal schema `{}`",
                journal.schema
            ),
        ));
    }
    if let Some(existing) = journal
        .records
        .iter()
        .find(|existing| existing.prune_id == record.prune_id)
    {
        if existing == &record {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "history prune journal contains conflicting record for prune id",
        ));
    }
    journal.records.push(record);
    write_history_prune_journal(store, &journal)
}

fn history_base_height(store: &NodeStore) -> io::Result<u64> {
    Ok(read_history_checkpoint_state_optional(store)?
        .map(|checkpoint| checkpoint.pruned_up_to_height)
        .unwrap_or(0))
}

pub(super) fn logical_tip_hash(store: &NodeStore, blocks: &BlockLog) -> io::Result<String> {
    if let Some(tip) = blocks.blocks.last() {
        return Ok(tip.header.block_hash.clone());
    }
    Ok(read_history_checkpoint_state_optional(store)?
        .map(|checkpoint| checkpoint.checkpoint_block_hash)
        .unwrap_or_else(|| "genesis".to_string()))
}

pub(super) fn build_history_checkpoint_state(
    store: &NodeStore,
    prune_up_to_height: u64,
    proof: &HistoryArchiveHandoffProof,
) -> io::Result<HistoryCheckpointState> {
    let existing_checkpoint = read_history_checkpoint_state_optional(store)?;
    if existing_checkpoint
        .as_ref()
        .is_some_and(|checkpoint| prune_up_to_height < checkpoint.pruned_up_to_height)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "history prune boundary is older than the existing checkpoint",
        ));
    }
    if let Some(checkpoint) = existing_checkpoint.as_ref() {
        if prune_up_to_height == checkpoint.pruned_up_to_height {
            return Ok(checkpoint.clone());
        }
    }
    let blocks = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    build_history_checkpoint_state_from_sources(
        store,
        prune_up_to_height,
        proof,
        existing_checkpoint,
        &blocks,
        &archive,
    )
}

fn build_history_checkpoint_state_from_sources(
    store: &NodeStore,
    prune_up_to_height: u64,
    proof: &HistoryArchiveHandoffProof,
    existing_checkpoint: Option<HistoryCheckpointState>,
    blocks: &BlockLog,
    archive: &BatchArchive,
) -> io::Result<HistoryCheckpointState> {
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    if proof.chain_id != genesis.chain_id
        || proof.genesis_hash != genesis_hash_hex
        || proof.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff proof domain does not match local genesis",
        ));
    }
    if proof.to_height < prune_up_to_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff proof does not cover checkpoint boundary",
        ));
    }

    let has_existing_checkpoint = existing_checkpoint.is_some();

    let faucet_account = read_faucet_account_file(&store.data_dir().join(FAUCET_ACCOUNT_FILE))?;
    let live_validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let mut validator_registry;
    let mut governance;
    let mut ledger;
    let mut ordered_batches;
    let mut shielded;
    let mut bridge;
    let base_height;
    let mut registry_update_ids;
    let mut native_fee_burn_total;
    if let Some(checkpoint) = existing_checkpoint {
        base_height = checkpoint.pruned_up_to_height;
        native_fee_burn_total = checkpoint.native_fee_burn_total.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "history checkpoint v2 is missing cumulative native fee burn",
            )
        })?;
        validator_registry = checkpoint.validator_registry;
        governance = checkpoint.governance;
        ledger = checkpoint.ledger;
        ordered_batches = checkpoint.ordered_batches;
        shielded = checkpoint.shielded;
        bridge = checkpoint.bridge;
        registry_update_ids = governance
            .validator_registry_updates
            .iter()
            .filter(|update| update.activation_height <= base_height)
            .map(|update| update.update_id.clone())
            .collect::<HashSet<_>>();
    } else {
        base_height = 0;
        native_fee_burn_total = 0;
        validator_registry = read_validator_registry_replay_base(store)?;
        governance = GovernanceState::new(genesis.validator_count);
        ledger = LedgerState::new(vec![faucet_account]);
        ordered_batches = Vec::new();
        shielded = ShieldedState::empty();
        bridge = BridgeState::empty();
        registry_update_ids = HashSet::new();
    }
    let replay_base_native_supply = native_pft_live_total(&ledger, &shielded)?;
    if !has_existing_checkpoint
        && replay_base_native_supply != u128::from(genesis.expected_native_supply_atoms())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "history checkpoint height-zero live native supply {replay_base_native_supply} does not equal genesis native supply {}",
                genesis.expected_native_supply_atoms()
            ),
        ));
    }

    let selected_blocks = blocks
        .blocks
        .iter()
        .filter(|block| {
            block.header.height > base_height && block.header.height <= prune_up_to_height
        })
        .collect::<Vec<_>>();
    let expected_block_count = prune_up_to_height
        .checked_sub(base_height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid prune boundary"))?
        as usize;
    if selected_blocks.len() != expected_block_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "checkpoint block range is not contiguous in local retained history",
        ));
    }

    for block in selected_blocks {
        let native_supply_before = native_pft_live_total(&ledger, &shielded)?;
        let Some(archive_entry) = archive.find(&block.header.batch_kind, &block.header.batch_id)
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} batch payload is not archived for checkpoint replay",
                    block.header.height
                ),
            ));
        };
        activate_validator_registry_updates_for_height(
            &genesis,
            &mut validator_registry,
            &mut governance,
            &mut registry_update_ids,
            block.header.height,
        )?;
        let replay_validators = active_validator_ids(&governance)?;
        backfill_legacy_validator_registry_records(
            &mut validator_registry,
            &live_validator_registry,
            &replay_validators,
            &format!("block {} checkpoint replay", block.header.height),
        )?;
        let receipts = replay_archived_payload(
            &genesis,
            block,
            archive_entry,
            ArchivedReplayState {
                governance: &mut governance,
                ledger: &mut ledger,
                shielded: &mut shielded,
                bridge: &mut bridge,
                validator_registry: &validator_registry,
            },
        )?;
        let native_supply_after = native_pft_live_total(&ledger, &shielded)?;
        verify_native_pft_transition(
            block.header.height,
            native_supply_before,
            native_supply_after,
            &receipts,
        )?;
        native_fee_burn_total = native_fee_burn_total
            .checked_add(native_pft_fee_burn_total(block.header.height, &receipts)?)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "history checkpoint cumulative native fee burn overflow",
                )
            })?;
        let replay_receipt_ids = receipts
            .iter()
            .map(|receipt| receipt.tx_id.clone())
            .collect::<Vec<_>>();
        if replay_receipt_ids != block.receipt_ids {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} checkpoint receipt ids mismatch",
                    block.header.height
                ),
            ));
        }
        ordered_batches.push(block.header.batch_id.clone());
        let replay_state_root = replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )?;
        if replay_state_root != block.header.state_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "block {} checkpoint state root mismatch",
                    block.header.height
                ),
            ));
        }
    }

    let checkpoint_block = blocks
        .blocks
        .iter()
        .find(|block| block.header.height == prune_up_to_height)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint boundary block is missing from local history",
            )
        })?;
    let mut checkpoint = HistoryCheckpointState {
        schema: "postfiat-history-checkpoint-v2".to_string(),
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        pruned_up_to_height: prune_up_to_height,
        checkpoint_block_hash: checkpoint_block.header.block_hash.clone(),
        checkpoint_state_root: checkpoint_block.header.state_root.clone(),
        archive_handoff_proof_hash: proof.proof_hash.clone(),
        block_range_root: proof.block_range_root.clone(),
        batch_payload_root: proof.batch_payload_root.clone(),
        receipt_root: proof.receipt_root.clone(),
        native_fee_burn_total: Some(native_fee_burn_total),
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        validator_registry,
        checkpoint_hash: String::new(),
    };
    checkpoint.checkpoint_hash = history_checkpoint_hash(&checkpoint)?;
    Ok(checkpoint)
}

fn history_checkpoint_hash(checkpoint: &HistoryCheckpointState) -> io::Result<String> {
    let mut canonical = checkpoint.clone();
    canonical.checkpoint_hash.clear();
    let encoded = serde_json::to_vec(&canonical).map_err(invalid_data)?;
    Ok(hash_hex("postfiat.history_checkpoint.v2", &encoded))
}

fn build_history_prune_journal_record(
    checkpoint: &HistoryCheckpointState,
    pruned_block_count: usize,
    pruned_batch_count: usize,
    pruned_receipt_count: usize,
    after_block_range: &HistoryRangeReport,
) -> io::Result<HistoryPruneJournalRecord> {
    let encoded = serde_json::to_vec(&(
        checkpoint.chain_id.as_str(),
        checkpoint.genesis_hash.as_str(),
        checkpoint.protocol_version,
        checkpoint.pruned_up_to_height,
        checkpoint.archive_handoff_proof_hash.as_str(),
        checkpoint.checkpoint_hash.as_str(),
        pruned_block_count,
        pruned_batch_count,
        pruned_receipt_count,
        after_block_range.first_height,
        after_block_range.last_height,
        after_block_range.count,
    ))
    .map_err(invalid_data)?;
    Ok(HistoryPruneJournalRecord {
        schema: "postfiat-history-prune-journal-record-v1".to_string(),
        prune_id: hash_hex("postfiat.history_prune.record.v1", &encoded),
        chain_id: checkpoint.chain_id.clone(),
        genesis_hash: checkpoint.genesis_hash.clone(),
        protocol_version: checkpoint.protocol_version,
        pruned_up_to_height: checkpoint.pruned_up_to_height,
        archive_handoff_proof_hash: checkpoint.archive_handoff_proof_hash.clone(),
        checkpoint_hash: checkpoint.checkpoint_hash.clone(),
        pruned_block_count,
        pruned_batch_count,
        pruned_receipt_count,
        remaining_first_height: after_block_range.first_height,
        remaining_last_height: after_block_range.last_height,
        remaining_block_count: after_block_range.count,
    })
}

fn build_history_archive_handoff_proof(
    store: &NodeStore,
    from_height: u64,
    to_height: u64,
    archive_uri: String,
) -> io::Result<HistoryArchiveHandoffProof> {
    if from_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff from_height must be positive",
        ));
    }
    if to_height < from_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff to_height must be greater than or equal to from_height",
        ));
    }
    verify_blocks(NodeOptions {
        data_dir: store.data_dir().to_path_buf(),
    })?;
    let genesis = store.read_genesis()?;
    let genesis_hash = genesis_hash(&genesis);
    let blocks = store.read_blocks()?;
    let archive = store.read_batch_archive()?;
    let receipts = store.read_receipts()?;
    let status_report = status(NodeOptions {
        data_dir: store.data_dir().to_path_buf(),
    })?;
    if to_height > status_report.block_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "archive handoff to_height {to_height} exceeds current height {}",
                status_report.block_height
            ),
        ));
    }

    let (selected_blocks, selected_batches, selected_receipts) =
        select_history_archive_window(&blocks, &archive, &receipts, from_height, to_height)?;
    history_archive_handoff_proof_from_window(HistoryArchiveWindowProofInput {
        chain_id: genesis.chain_id.as_str(),
        genesis_hash: genesis_hash.as_str(),
        protocol_version: genesis.protocol_version,
        archive_uri: archive_uri.as_str(),
        from_height,
        to_height,
        selected_blocks: &selected_blocks,
        selected_batches: &selected_batches,
        selected_receipts: &selected_receipts,
    })
}

fn select_history_archive_window(
    blocks: &BlockLog,
    archive: &BatchArchive,
    receipts: &[Receipt],
    from_height: u64,
    to_height: u64,
) -> io::Result<(Vec<BlockRecord>, Vec<BatchArchiveEntry>, Vec<Receipt>)> {
    if from_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive window from_height must be positive",
        ));
    }
    if to_height < from_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive window to_height must be greater than or equal to from_height",
        ));
    }
    let selected_blocks = blocks
        .blocks
        .iter()
        .filter(|block| block.header.height >= from_height && block.header.height <= to_height)
        .cloned()
        .collect::<Vec<_>>();
    let expected_block_count = (to_height - from_height + 1) as usize;
    if selected_blocks.len() != expected_block_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive handoff block range is not contiguous in local history",
        ));
    }

    let archive_by_key = archive
        .batches
        .iter()
        .map(|entry| ((entry.batch_kind.as_str(), entry.batch_id.as_str()), entry))
        .collect::<HashMap<_, _>>();
    let receipts_by_id = receipts
        .iter()
        .map(|receipt| (receipt.tx_id.as_str(), receipt))
        .collect::<HashMap<_, _>>();

    let mut selected_batches = Vec::with_capacity(selected_blocks.len());
    let mut selected_receipts = Vec::new();
    for block in &selected_blocks {
        let archive_entry = archive_by_key
            .get(&(
                block.header.batch_kind.as_str(),
                block.header.batch_id.as_str(),
            ))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "missing archived batch {}:{} for archive window block {}",
                        block.header.batch_kind, block.header.batch_id, block.header.height
                    ),
                )
            })?;
        selected_batches.push((*archive_entry).clone());
        for receipt_id in &block.receipt_ids {
            let receipt = receipts_by_id.get(receipt_id.as_str()).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "missing receipt `{receipt_id}` for archive window block {}",
                        block.header.height
                    ),
                )
            })?;
            selected_receipts.push((*receipt).clone());
        }
    }
    Ok((selected_blocks, selected_batches, selected_receipts))
}

struct HistoryArchiveWindowProofInput<'a> {
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    archive_uri: &'a str,
    from_height: u64,
    to_height: u64,
    selected_blocks: &'a [BlockRecord],
    selected_batches: &'a [BatchArchiveEntry],
    selected_receipts: &'a [Receipt],
}

fn history_archive_handoff_proof_from_window(
    input: HistoryArchiveWindowProofInput<'_>,
) -> io::Result<HistoryArchiveHandoffProof> {
    let HistoryArchiveWindowProofInput {
        chain_id,
        genesis_hash,
        protocol_version,
        archive_uri,
        from_height,
        to_height,
        selected_blocks,
        selected_batches,
        selected_receipts,
    } = input;
    if from_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff from_height must be positive",
        ));
    }
    if to_height < from_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "archive handoff to_height must be greater than or equal to from_height",
        ));
    }
    let expected_block_count = (to_height - from_height + 1) as usize;
    if selected_blocks.len() != expected_block_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive handoff block range is not contiguous in bundled history",
        ));
    }
    for (index, block) in selected_blocks.iter().enumerate() {
        let expected_height = from_height
            .checked_add(index as u64)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
        if block.header.height != expected_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "archive handoff block height mismatch at index {index}: expected {expected_height}, got {}",
                    block.header.height
                ),
            ));
        }
    }
    let mut archive_by_key = HashMap::new();
    for entry in selected_batches {
        let key = (entry.batch_kind.as_str(), entry.batch_id.as_str());
        if archive_by_key.insert(key, entry).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate archived batch {}:{} in archive window",
                    entry.batch_kind, entry.batch_id
                ),
            ));
        }
    }
    let mut receipts_by_id = HashMap::new();
    for receipt in selected_receipts {
        if receipts_by_id
            .insert(receipt.tx_id.as_str(), receipt)
            .is_some()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate receipt `{}` in archive window", receipt.tx_id),
            ));
        }
    }

    let mut block_items = Vec::with_capacity(selected_blocks.len());
    let mut batch_items = Vec::with_capacity(selected_blocks.len());
    let mut receipt_items = Vec::new();
    for block in selected_blocks {
        block_items.push((
            block.header.height,
            block.header.view,
            block.header.parent_hash.as_str(),
            block.header.proposer.as_str(),
            block.header.batch_kind.as_str(),
            block.header.batch_id.as_str(),
            block.header.state_root.as_str(),
            block.header.certificate_id.as_str(),
            block.header.certificate.registry_root.as_str(),
            block.header.block_hash.as_str(),
        ));
        let archive_entry = archive_by_key
            .get(&(
                block.header.batch_kind.as_str(),
                block.header.batch_id.as_str(),
            ))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "missing archived batch {}:{} for handoff block {}",
                        block.header.batch_kind, block.header.batch_id, block.header.height
                    ),
                )
            })?;
        let payload_json_hash = hash_hex(
            "postfiat.history_archive_handoff.batch_payload_json.v1",
            archive_entry.payload_json.as_bytes(),
        );
        batch_items.push((
            block.header.height,
            archive_entry.batch_kind.as_str(),
            archive_entry.batch_id.as_str(),
            archive_entry.payload_hash.as_str(),
            payload_json_hash,
        ));
        for receipt_id in &block.receipt_ids {
            let receipt = receipts_by_id.get(receipt_id.as_str()).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "missing receipt `{receipt_id}` for handoff block {}",
                        block.header.height
                    ),
                )
            })?;
            let receipt_json = serde_json::to_vec(receipt).map_err(invalid_data)?;
            let receipt_hash = hash_hex(
                "postfiat.history_archive_handoff.receipt_json.v1",
                &receipt_json,
            );
            receipt_items.push((block.header.height, receipt_id.as_str(), receipt_hash));
        }
    }
    if batch_items.len() != selected_batches.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive window contains unreferenced batch payloads",
        ));
    }
    if receipt_items.len() != selected_receipts.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "archive window contains unreferenced receipts",
        ));
    }

    let block_range_root = history_root_hash(
        "postfiat.history_archive_handoff.block_range.v1",
        &block_items,
    )?;
    let batch_payload_root = history_root_hash(
        "postfiat.history_archive_handoff.batch_payloads.v1",
        &batch_items,
    )?;
    let receipt_root = history_root_hash(
        "postfiat.history_archive_handoff.receipts.v1",
        &receipt_items,
    )?;

    let first_block_hash = selected_blocks
        .first()
        .map(|block| block.header.block_hash.clone())
        .unwrap_or_default();
    let last_block_hash = selected_blocks
        .last()
        .map(|block| block.header.block_hash.clone())
        .unwrap_or_default();
    let mut proof = HistoryArchiveHandoffProof {
        schema: "postfiat-history-archive-handoff-v1".to_string(),
        chain_id: chain_id.to_string(),
        genesis_hash: genesis_hash.to_string(),
        protocol_version,
        archive_uri: archive_uri.to_string(),
        from_height,
        to_height,
        block_count: selected_blocks.len(),
        batch_count: batch_items.len(),
        receipt_count: receipt_items.len(),
        first_block_hash,
        last_block_hash,
        block_range_root,
        batch_payload_root,
        receipt_root,
        proof_hash: String::new(),
    };
    proof.proof_hash = history_archive_handoff_proof_hash(&proof)?;
    Ok(proof)
}

fn history_root_hash<T: Serialize>(domain: &str, value: &T) -> io::Result<String> {
    let encoded = serde_json::to_vec(value).map_err(invalid_data)?;
    Ok(hash_hex(domain, &encoded))
}

fn history_archive_handoff_proof_hash(proof: &HistoryArchiveHandoffProof) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        proof.schema.as_str(),
        proof.chain_id.as_str(),
        proof.genesis_hash.as_str(),
        proof.protocol_version,
        proof.archive_uri.as_str(),
        proof.from_height,
        proof.to_height,
        proof.block_count,
        proof.batch_count,
        proof.receipt_count,
        proof.first_block_hash.as_str(),
        proof.last_block_hash.as_str(),
        proof.block_range_root.as_str(),
        proof.batch_payload_root.as_str(),
        proof.receipt_root.as_str(),
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.history_archive_handoff.proof.v1",
        &encoded,
    ))
}

fn history_archive_window_bundle_hash(bundle: &HistoryArchiveWindowBundle) -> io::Result<String> {
    let mut canonical = bundle.clone();
    canonical.bundle_hash.clear();
    let encoded = serde_json::to_vec(&canonical).map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.history_archive_window.bundle.v1",
        &encoded,
    ))
}

fn write_history_archive_handoff_proof_file(
    path: &Path,
    proof: &HistoryArchiveHandoffProof,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(proof).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn write_history_archive_window_bundle_file(
    path: &Path,
    bundle: &HistoryArchiveWindowBundle,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(bundle).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

fn history_archive_window_file_path(
    store: &NodeStore,
    bundle: &HistoryArchiveWindowBundle,
) -> PathBuf {
    store
        .data_dir()
        .join(HISTORY_ARCHIVE_WINDOWS_DIR)
        .join(format!(
            "{:020}-{:020}-{}.json",
            bundle.proof.from_height, bundle.proof.to_height, bundle.bundle_hash
        ))
}

fn read_history_archive_window_index(store: &NodeStore) -> io::Result<HistoryArchiveWindowIndex> {
    let path = store.data_dir().join(HISTORY_ARCHIVE_INDEX_FILE);
    match read_json_file(&path, "history archive window index") {
        Ok(index) => {
            validate_history_archive_window_index(&index)?;
            Ok(index)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(HistoryArchiveWindowIndex {
            schema: "postfiat-history-archive-window-index-v1".to_string(),
            windows: Vec::new(),
        }),
        Err(error) => Err(error),
    }
}

fn write_history_archive_window_index(
    store: &NodeStore,
    index: &HistoryArchiveWindowIndex,
) -> io::Result<()> {
    validate_history_archive_window_index(index)?;
    let json = serde_json::to_string_pretty(index).map_err(invalid_data)?;
    atomic_write(
        store.data_dir().join(HISTORY_ARCHIVE_INDEX_FILE),
        format!("{json}\n"),
    )
}

fn validate_history_archive_window_index(index: &HistoryArchiveWindowIndex) -> io::Result<()> {
    if index.schema != "postfiat-history-archive-window-index-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported history archive window index schema `{}`",
                index.schema
            ),
        ));
    }
    let mut hashes = HashSet::new();
    let mut previous_to_height = None;
    for window in &index.windows {
        if window.from_height == 0 || window.to_height < window.from_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "history archive window index contains an invalid height range",
            ));
        }
        if !hashes.insert(window.bundle_hash.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "history archive window index contains duplicate bundle hashes",
            ));
        }
        if let Some(previous_to_height) = previous_to_height {
            if window.from_height <= previous_to_height {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "history archive window index contains overlapping ranges",
                ));
            }
        }
        previous_to_height = Some(window.to_height);
    }
    Ok(())
}

fn sort_history_archive_window_index(index: &mut HistoryArchiveWindowIndex) {
    index.windows.sort_by(|left, right| {
        left.from_height
            .cmp(&right.from_height)
            .then_with(|| left.to_height.cmp(&right.to_height))
            .then_with(|| left.bundle_hash.cmp(&right.bundle_hash))
    });
}

#[cfg(test)]
mod history_tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn verified_archive_window_helper_returns_the_verified_bundle() {
        let block = BlockRecord {
            header: BlockHeader {
                height: 1,
                view: 0,
                parent_hash: "genesis".to_string(),
                proposer: "validator-0".to_string(),
                batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
                batch_id: "batch-1".to_string(),
                state_root: "state-root-1".to_string(),
                receipt_count: 0,
                certificate_id: "certificate-1".to_string(),
                certificate: BlockCertificate {
                    validators: vec!["validator-0".to_string()],
                    quorum: 1,
                    registry_root: String::new(),
                    votes: Vec::new(),
                },
                consensus_v2_commit: None,
                block_hash: "block-1".to_string(),
            },
            receipt_ids: Vec::new(),
            fastpay_pre_state_effects: Vec::new(),
        };
        let batch = BatchArchiveEntry {
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: "batch-1".to_string(),
            payload_hash: "payload-hash-1".to_string(),
            payload_json: "{\"batch\":1}".to_string(),
        };
        let proof = history_archive_handoff_proof_from_window(HistoryArchiveWindowProofInput {
            chain_id: "postfiat-local",
            genesis_hash: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            protocol_version: 1,
            archive_uri: "",
            from_height: 1,
            to_height: 1,
            selected_blocks: std::slice::from_ref(&block),
            selected_batches: std::slice::from_ref(&batch),
            selected_receipts: &[],
        })
        .expect("proof");
        let mut bundle = HistoryArchiveWindowBundle {
            schema: "postfiat-history-archive-window-v1".to_string(),
            proof,
            blocks: vec![block],
            batches: vec![batch],
            receipts: Vec::new(),
            bundle_hash: String::new(),
        };
        bundle.bundle_hash = history_archive_window_bundle_hash(&bundle).expect("bundle hash");

        let dir = std::env::temp_dir().join(format!(
            "postfiat-history-window-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).expect("dir");
        let bundle_file = dir.join("bundle.json");
        write_history_archive_window_bundle_file(&bundle_file, &bundle).expect("write bundle");

        let verified =
            verify_history_archive_window_bundle_data(&bundle_file).expect("verify bundle");
        assert_eq!(verified.bundle, bundle);
        assert_eq!(verified.report.bundle_hash, verified.bundle.bundle_hash);
        assert_eq!(verified.report.block_count, verified.bundle.blocks.len());

        std::fs::remove_dir_all(dir).expect("cleanup");
    }
}
