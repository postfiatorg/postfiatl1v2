use super::*;

#[derive(Debug, Clone)]
pub struct StorageBackendConfigureOptions {
    pub data_dir: PathBuf,
    pub mode: postfiat_storage::StorageBackendMode,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StorageBackendConfigureReportV1 {
    pub schema: String,
    pub mode: String,
    pub comparison_only: bool,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub activation_height: u64,
    pub finalized_height: u64,
    pub finalized_block_hash: String,
    pub finalized_state_root: String,
    pub ordered_batch_count: u64,
    pub ordered_history_accumulator: String,
    pub transactional_generation_verified: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounded_index_generation: Option<String>,
}

/// Select one node-local backend on an offline clone and prove that it exposes
/// the same certified logical tip and ordered-history commitment before
/// returning. The mode file is authenticated by the destination node's local
/// integrity key and is deliberately excluded from portable snapshots.
pub fn configure_storage_backend(
    options: StorageBackendConfigureOptions,
) -> io::Result<StorageBackendConfigureReportV1> {
    let store = NodeStore::try_new(&options.data_dir)?;
    if store.read_ordered_commit_journal_raw()?.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_backend_mode_pending_commit: recover the ordered-commit journal before changing backend mode",
        ));
    }

    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let activation_height =
        effective_storage_commitment_activation_height(&genesis, &governance).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "storage_backend_mode_activation_missing: comparison requires a versioned storage commitment",
            )
        })?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    if tip.height < activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_backend_mode_not_active: comparison backend may be selected only at or above activation",
        ));
    }
    if !store.transactional_storage_configured()? {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "storage_backend_mode_transactional_reference_missing: comparison requires the selected transactional reference",
        ));
    }

    let transactional = store.transactional_store()?;
    let meta = transactional.meta()?;
    let transactional_commitment = meta.ordered_history_commitment();
    if meta.finalized_height != tip.height
        || meta.finalized_block_hash != tip.block_hash
        || meta.finalized_state_root != tip.state_root
        || meta.ordered_batch_count != tip.ordered_batch_count
        || meta.scheduled_activation_height != Some(activation_height)
        || meta.last_full_verification_height != Some(meta.finalized_height)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_backend_mode_transactional_reference_stale: selected store does not match the exact verified tip",
        ));
    }

    let legacy_commitment = store.legacy_ordered_history_commitment()?;
    if legacy_commitment != transactional_commitment {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_backend_mode_logical_mismatch: JSONL history and transactional commitment differ",
        ));
    }

    let bounded_index_generation = if options.mode
        == postfiat_storage::StorageBackendMode::BoundedJsonl
    {
        let report = store.rebuild_ordered_history_index()?;
        if report.record_count != tip.ordered_batch_count
            || report.accumulator != transactional_commitment.accumulator
            || report.finalized_height != tip.height
            || report.block_hash != tip.block_hash
            || report.state_root != tip.state_root
        {
            return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "storage_backend_mode_index_mismatch: rebuilt bounded index differs from the certified tip",
                ));
        }
        Some(report.generation)
    } else {
        None
    };

    drop(transactional);
    store.write_storage_backend_mode(options.mode)?;
    let selected = NodeStore::try_new(&options.data_dir)?;
    let selected_commitment = selected.backend_ordered_history_commitment()?;
    let selected_tip = selected.read_chain_tip()?;
    if selected_commitment != legacy_commitment
        || selected_tip.height != tip.height
        || selected_tip.block_hash != tip.block_hash
        || selected_tip.state_root != tip.state_root
        || selected_tip.ordered_batch_count != tip.ordered_batch_count
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "storage_backend_mode_selected_mismatch: configured backend changed the certified logical state",
        ));
    }

    Ok(StorageBackendConfigureReportV1 {
        schema: "postfiat-storage-backend-configure-report-v1".to_owned(),
        mode: options.mode.as_str().to_owned(),
        comparison_only: options.mode.is_comparison_only(),
        chain_id: genesis.chain_id,
        genesis_hash: tip.genesis_hash,
        protocol_version: genesis.protocol_version,
        activation_height,
        finalized_height: tip.height,
        finalized_block_hash: tip.block_hash,
        finalized_state_root: tip.state_root,
        ordered_batch_count: selected_commitment.count,
        ordered_history_accumulator: selected_commitment.accumulator,
        transactional_generation_verified: true,
        bounded_index_generation,
    })
}
