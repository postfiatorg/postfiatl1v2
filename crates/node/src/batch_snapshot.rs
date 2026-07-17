use super::*;

pub(super) struct BlockProposalPlan<'a, T> {
    pub(super) genesis: &'a Genesis,
    pub(super) governance: &'a GovernanceState,
    pub(super) ledger: &'a LedgerState,
    pub(super) ordered_batches: &'a [String],
    pub(super) shielded: &'a ShieldedState,
    pub(super) bridge: &'a BridgeState,
    pub(super) block_height: u64,
    pub(super) parent_hash: String,
    pub(super) view: u64,
    pub(super) batch_kind: &'a str,
    pub(super) batch_id: &'a str,
    pub(super) payload: &'a T,
    pub(super) receipts: &'a [Receipt],
    pub(super) fastpay_pre_state_effects: Vec<postfiat_types::FastPayVersionFenceV1>,
}

pub(super) fn build_block_proposal_from_state<T: Serialize>(
    plan: BlockProposalPlan<'_, T>,
) -> io::Result<BlockProposalFile> {
    let state_root = replicated_state_root(
        plan.genesis,
        plan.governance,
        plan.ledger,
        plan.ordered_batches,
        plan.shielded,
        plan.bridge,
    )?;
    let payload_json = serde_json::to_string(plan.payload).map_err(invalid_data)?;
    let payload_hash =
        batch_archive_payload_hash(plan.genesis, plan.batch_kind, plan.batch_id, &payload_json)?;
    let receipt_ids = plan
        .receipts
        .iter()
        .map(|receipt| receipt.tx_id.clone())
        .collect::<Vec<_>>();
    let block_height = plan.block_height;
    let view = plan.view;
    let validators = active_validator_ids(plan.governance)?;
    let proposer = leader_for_view(&validators, block_height, view).map_err(invalid_data)?;
    Ok(BlockProposalFile {
        schema: BLOCK_PROPOSAL_FILE_SCHEMA.to_string(),
        chain_id: plan.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(plan.genesis),
        protocol_version: plan.genesis.protocol_version,
        block_height,
        view,
        parent_hash: plan.parent_hash,
        proposer,
        batch_kind: plan.batch_kind.to_string(),
        batch_id: plan.batch_id.to_string(),
        payload_hash,
        state_root,
        receipt_count: receipt_ids.len() as u64,
        receipt_ids,
        fastpay_pre_state_effects: plan.fastpay_pre_state_effects,
        signature: None,
    })
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyBatchPrepareTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub ordered_batches_ms: f64,
    pub state_root_ms: f64,
    pub receipts_ms: f64,
    pub archive_clone_ms: f64,
    pub payload_json_ms: f64,
    pub payload_hash_ms: f64,
    pub archive_update_ms: f64,
    pub blocks_clone_ms: f64,
    pub proposer_selection_ms: f64,
    pub receipt_ids_ms: f64,
    pub proposal_hash_ms: f64,
    pub certificate_ms: f64,
    pub certificate_structural_ms: f64,
    pub certificate_vote_set_ms: f64,
    pub certificate_registry_root_ms: f64,
    pub certificate_vote_signature_ms: f64,
    pub certificate_id_ms: f64,
    pub certificate_block_hash_ms: f64,
    pub certificate_clone_ms: f64,
    pub certificate_local_signing_ms: f64,
    pub block_hash_ms: f64,
    pub block_push_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyBatchWriteTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub write_journal_ms: f64,
    pub write_ledger_ms: f64,
    pub write_governance_ms: f64,
    pub write_shielded_ms: f64,
    pub write_bridge_ms: f64,
    pub write_receipts_ms: f64,
    pub write_ordered_batches_ms: f64,
    pub write_batch_archive_ms: f64,
    pub write_blocks_ms: f64,
    pub write_validator_registry_ms: f64,
    pub refresh_account_tx_index_ms: f64,
    pub remove_journal_ms: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyBatchTimingReport {
    pub schema: String,
    pub total_ms: f64,
    pub store_init_ms: f64,
    pub journal_recovery_ms: f64,
    pub read_core_state_ms: f64,
    pub read_batch_ms: f64,
    pub verify_batch_ms: f64,
    pub ordered_reference_ms: f64,
    pub duplicate_check_ms: f64,
    pub read_blocks_ms: f64,
    pub activation_ms: f64,
    pub active_validator_ids_ms: f64,
    pub certificate_material_ms: f64,
    pub read_aux_state_ms: f64,
    pub execute_batch_ms: f64,
    pub prepare_commit_ms: f64,
    pub prepare_commit_breakdown: ApplyBatchPrepareTimingReport,
    pub live_registry_update_ms: f64,
    pub write_commit_ms: f64,
    pub write_commit_breakdown: ApplyBatchWriteTimingReport,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApplyBatchWithTimingsReport {
    pub receipts: Vec<Receipt>,
    pub timings: ApplyBatchTimingReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedBatchCommitIdentity {
    pub block_height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub certificate_id: String,
}

#[derive(Debug, Clone)]
pub struct VerifiedBlockCertificateFile {
    certificate_file: BlockCertificateFile,
}

impl VerifiedBlockCertificateFile {
    pub fn as_block_certificate_file(&self) -> &BlockCertificateFile {
        &self.certificate_file
    }

    pub fn attach_consensus_v2_commit(
        mut self,
        data_dir: &Path,
        proposal: &BlockProposalFile,
        commit: postfiat_types::ConsensusV2Commit,
    ) -> io::Result<Self> {
        let genesis = NodeStore::new(data_dir).read_genesis()?;
        if !consensus_v2_active_at(&genesis, proposal.block_height) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cannot attach consensus v2 commit before activation",
            ));
        }
        verify_consensus_v2_commit_for_block(data_dir, proposal, &commit)?;
        let expected_proposal_hash = block_proposal_hash(proposal)?;
        if self.certificate_file.block_height != proposal.block_height
            || self.certificate_file.view != proposal.view
            || self.certificate_file.proposer != proposal.proposer
            || self.certificate_file.proposal_hash.as_deref()
                != Some(expected_proposal_hash.as_str())
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy prepare certificate does not match consensus v2 proposal",
            ));
        }
        self.certificate_file.consensus_v2_commit = Some(commit);
        Ok(self)
    }
}

pub fn aggregate_verified_block_certificate(
    options: BlockCertificateOptions,
) -> io::Result<VerifiedBlockCertificateFile> {
    aggregate_block_certificate(options)
        .map(|certificate_file| VerifiedBlockCertificateFile { certificate_file })
}

pub(super) fn apply_batch_elapsed_ms(start: std::time::Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

pub fn apply_batch(options: ApplyBatchOptions) -> io::Result<Vec<Receipt>> {
    apply_batch_with_replay(options, None).map(|report| report.receipts)
}

pub fn apply_batch_with_timings(
    options: ApplyBatchOptions,
) -> io::Result<ApplyBatchWithTimingsReport> {
    apply_batch_with_replay(options, None)
}

pub fn apply_batch_with_replay(
    options: ApplyBatchOptions,
    replay_block_file: Option<PathBuf>,
) -> io::Result<ApplyBatchWithTimingsReport> {
    apply_batch_with_timings_inner(options, None, replay_block_file, None)
}

pub fn apply_batch_with_expected_commit_identity(
    options: ApplyBatchOptions,
    expected: &ExpectedBatchCommitIdentity,
) -> io::Result<ApplyBatchWithTimingsReport> {
    apply_batch_with_timings_inner(options, None, None, Some(expected))
}

pub fn apply_batch_with_verified_certificate_with_timings(
    options: ApplyBatchOptions,
    verified_certificate: VerifiedBlockCertificateFile,
) -> io::Result<ApplyBatchWithTimingsReport> {
    if options.certificate_file.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "verified certificate apply requires a certificate file for artifact consistency",
        ));
    }
    apply_batch_with_timings_inner(options, Some(verified_certificate), None, None)
}

fn apply_batch_with_timings_inner(
    options: ApplyBatchOptions,
    verified_certificate: Option<VerifiedBlockCertificateFile>,
    replay_block_file: Option<PathBuf>,
    expected_commit: Option<&ExpectedBatchCommitIdentity>,
) -> io::Result<ApplyBatchWithTimingsReport> {
    let total_start = std::time::Instant::now();
    let stage_start = std::time::Instant::now();
    let store = NodeStore::new(&options.data_dir);
    let store_init_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let commit_lock = store.lock_ordered_commit()?;
    recover_ordered_commit_journal_locked(&store, &commit_lock)?;
    let journal_recovery_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let genesis = store.read_genesis()?;
    let mut ledger = store.read_ledger()?;
    let mut governance = store.read_governance()?;
    let read_core_state_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let batch = read_batch_file(&options.batch_file)?;
    let read_batch_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let batch_domain = mempool_batch_domain(&genesis);
    let reference = reference_for_batch(&batch_domain, &batch).map_err(invalid_data)?;
    verify_batch_payload(&batch_domain, &batch, &reference).map_err(invalid_data)?;
    let verify_batch_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let ordered_reference = next_reference(vec![reference]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "ordering produced no batch reference",
        )
    })?;
    let ordered_reference_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&ordered_reference.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("batch `{}` already applied", ordered_reference.batch_id),
        ));
    }
    let duplicate_check_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let read_blocks_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let due_activations = activate_due_validator_registry_updates_for_commit(
        &store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    let activation_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let certificate_validators = active_validator_ids(&governance)?;
    let active_validator_ids_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let certificate_material = read_commit_certificate_material(
        &store,
        &certificate_validators,
        options.certificate_file.as_deref(),
        verified_certificate.as_ref(),
    )?;
    let certificate_material_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let read_aux_state_ms = apply_batch_elapsed_ms(stage_start);

    let fastpay_pre_state_effects = match certificate_material.external_certificate.as_ref() {
        Some(certificate) => reconcile_certified_fastpay_pre_state_effects(
            &store,
            &mut ledger,
            &shielded,
            &certificate.fastpay_pre_state_effects,
        )?,
        None => fastpay_pre_state_effects_for_next_block(&store, &ledger)?,
    };

    let stage_start = std::time::Instant::now();
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    ensure_atomic_swap_batch_allowed(&batch, block_height, compatibility)?;
    let receipts = execute_transparent_batch(
        &genesis,
        &governance,
        &mut ledger,
        &batch,
        block_height,
        compatibility,
    );
    let execute_batch_ms = apply_batch_elapsed_ms(stage_start);

    let batch_id = ordered_reference.batch_id;
    let mut proposed_ordered_batches = ordered_batches.clone();
    proposed_ordered_batches.push(batch_id.clone());
    let consensus_proposal = build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash: parent_hash.clone(),
        view: certificate_material
            .external_certificate
            .as_ref()
            .map_or(0, |certificate| certificate.view),
        batch_kind: BATCH_KIND_TRANSPARENT,
        batch_id: &batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects: fastpay_pre_state_effects.clone(),
    })?;
    verify_consensus_v2_finality_requirement(
        store.data_dir(),
        &genesis,
        &consensus_proposal,
        certificate_material.external_certificate.as_ref(),
    )?;
    let (historical_replay, archived_payload_json) = historical_replay_commit_inputs(
        &certificate_material,
        replay_block_file.as_deref(),
        &options.batch_file,
    )?;
    let stage_start = std::time::Instant::now();
    let prepared_commit = prepare_ordered_commit_timed(OrderedCommitPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        batch_kind: "transparent",
        batch_id: &batch_id,
        payload: &batch,
        batch_receipts: &receipts,
        archived_payload_json: archived_payload_json.as_deref(),
        validator_keys: certificate_material.validator_keys.as_ref(),
        external_certificate: certificate_material.external_certificate.as_ref(),
        external_validator_registry: certificate_material.external_validator_registry.as_ref(),
        external_certificate_preverified: certificate_material.external_certificate_preverified,
        historical_replay,
        certificate_validators: &certificate_validators,
        fastpay_pre_state_effects: &fastpay_pre_state_effects,
    })?;
    let prepare_commit_breakdown = prepared_commit.timings;
    let commit = prepared_commit.artifacts;
    let prepare_commit_ms = apply_batch_elapsed_ms(stage_start);

    if let Some(expected) = expected_commit {
        if commit.height != expected.block_height
            || commit.block.header.block_hash != expected.block_hash
            || commit.block.header.state_root != expected.state_root
            || commit.block.header.certificate_id != expected.certificate_id
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "prepared commit identity mismatch: expected height {} hash {} root {} certificate {}, prepared height {} hash {} root {} certificate {}",
                    expected.block_height,
                    expected.block_hash,
                    expected.state_root,
                    expected.certificate_id,
                    commit.height,
                    commit.block.header.block_hash,
                    commit.block.header.state_root,
                    commit.block.header.certificate_id,
                ),
            ));
        }
    }

    let stage_start = std::time::Instant::now();
    let live_registry_update =
        live_validator_registry_after_due_updates(&store, &genesis, &governance, commit.height)?;
    let live_registry_update_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let write_commit_breakdown = write_ordered_commit_with_journal_timed_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: Some(ledger),
            governance: due_activations.governance_changed.then_some(governance),
            shielded: None,
            bridge: None,
            commit,
            validator_registry: live_registry_update.or(due_activations.registry),
        },
    )?;
    let write_commit_ms = apply_batch_elapsed_ms(stage_start);

    Ok(ApplyBatchWithTimingsReport {
        receipts,
        timings: ApplyBatchTimingReport {
            schema: "postfiat-apply-batch-timings-v1".to_string(),
            total_ms: apply_batch_elapsed_ms(total_start),
            store_init_ms,
            journal_recovery_ms,
            read_core_state_ms,
            read_batch_ms,
            verify_batch_ms,
            ordered_reference_ms,
            duplicate_check_ms,
            read_blocks_ms,
            activation_ms,
            active_validator_ids_ms,
            certificate_material_ms,
            read_aux_state_ms,
            execute_batch_ms,
            prepare_commit_ms,
            prepare_commit_breakdown,
            live_registry_update_ms,
            write_commit_ms,
            write_commit_breakdown,
        },
    })
}

pub fn bridge_upsert_domain(options: BridgeDomainOptions) -> io::Result<BridgeDomain> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let mut bridge = store.read_bridge()?;
    let domain = upsert_domain_with_metadata(
        &mut bridge,
        BridgeDomainSpec {
            domain_id: options.domain_id,
            name: options.name,
            source_chain: options.source_chain,
            target_chain: options.target_chain,
            bridge_id: options.bridge_id,
            door_account: options.door_account,
            inbound_cap: options.inbound_cap,
            outbound_cap: options.outbound_cap,
        },
    )
    .map_err(bridge_error)?;
    store.write_bridge(&bridge)?;
    store.append_receipt(Receipt::accepted(
        direct_bridge_domain_receipt_id(&genesis, "upsert", &domain)?,
        "bridge domain upserted",
    ))?;
    Ok(domain)
}

pub fn bridge_transfer(options: BridgeTransferOptions) -> io::Result<BridgeTransfer> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let witness_epoch =
        resolve_bridge_witness_epoch(options.witness_epoch, governance.bridge_witness_epoch)?;
    let mut bridge = store.read_bridge()?;
    let mut request = BridgeTransferRequest {
        domain_id: options.domain_id.clone(),
        direction: options.direction.clone(),
        from: options.from.clone(),
        to: options.to.clone(),
        asset_id: options.asset_id.clone(),
        amount: options.amount,
        witness_id: options.witness_id.clone(),
        witness_epoch,
        witness_attestation: None,
    };
    request.witness_attestation = Some(build_bridge_witness_attestation(
        &store,
        &genesis,
        &bridge,
        &request,
        &options.witness_signer,
    )?);

    if request.witness_epoch != governance.bridge_witness_epoch {
        let error = io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "bridge witness epoch {} does not match governed epoch {}",
                request.witness_epoch, governance.bridge_witness_epoch
            ),
        );
        let reject_id = direct_rejection_id(
            &genesis,
            "postfiat.bridge.rejected_transfer.sim.v2",
            &bridge_direct_rejection_seed(&request, "bad_witness_epoch"),
        )?;
        store.append_receipt(Receipt::rejected(
            reject_id,
            "bad_witness_epoch",
            error.to_string(),
        ))?;
        return Err(error);
    }

    match apply_simulated_transfer(&mut bridge, request.clone()) {
        Ok(transfer) => {
            store.write_bridge(&bridge)?;
            store.append_receipt(Receipt::accepted(
                transfer.transfer_id.clone(),
                "bridge transfer applied",
            ))?;
            Ok(transfer)
        }
        Err(error) => {
            let reject_id = direct_rejection_id(
                &genesis,
                "postfiat.bridge.rejected_transfer.sim.v2",
                &bridge_direct_rejection_seed(&request, error.code()),
            )?;
            store.append_receipt(Receipt::rejected(
                reject_id,
                error.code(),
                error.to_string(),
            ))?;
            Err(bridge_error(error))
        }
    }
}

pub fn bridge_pause(options: BridgePauseOptions) -> io::Result<BridgeDomain> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let mut bridge = store.read_bridge()?;
    let domain =
        set_domain_paused(&mut bridge, &options.domain_id, options.paused).map_err(bridge_error)?;
    store.write_bridge(&bridge)?;
    let operation = if options.paused { "pause" } else { "resume" };
    store.append_receipt(Receipt::accepted(
        direct_bridge_domain_receipt_id(&genesis, operation, &domain)?,
        if options.paused {
            "bridge domain paused"
        } else {
            "bridge domain resumed"
        },
    ))?;
    Ok(domain)
}

pub fn create_bridge_domain_batch(
    options: BridgeDomainBatchOptions,
) -> io::Result<BridgeActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let actions = vec![BridgeAction::Domain(BridgeDomainAction {
        domain_id: options.domain_id,
        name: options.name,
        source_chain: options.source_chain,
        target_chain: options.target_chain,
        bridge_id: options.bridge_id,
        door_account: options.door_account,
        inbound_cap: options.inbound_cap,
        outbound_cap: options.outbound_cap,
    })];
    let batch = build_bridge_action_batch(&genesis, actions)?;
    write_bridge_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_bridge_transfer_batch(
    options: BridgeTransferBatchOptions,
) -> io::Result<BridgeActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let witness_epoch =
        resolve_bridge_witness_epoch(options.witness_epoch, governance.bridge_witness_epoch)?;
    let bridge = store.read_bridge()?;
    let mut request = BridgeTransferRequest {
        domain_id: options.domain_id,
        direction: options.direction,
        from: options.from,
        to: options.to,
        asset_id: options.asset_id,
        amount: options.amount,
        witness_id: options.witness_id,
        witness_epoch,
        witness_attestation: None,
    };
    request.witness_attestation = Some(build_bridge_witness_attestation(
        &store,
        &genesis,
        &bridge,
        &request,
        &options.witness_signer,
    )?);
    let actions = vec![BridgeAction::Transfer(BridgeTransferAction {
        domain_id: request.domain_id,
        direction: request.direction,
        from: request.from,
        to: request.to,
        asset_id: request.asset_id,
        amount: request.amount,
        witness_id: request.witness_id,
        witness_epoch: request.witness_epoch,
        witness_attestation: request.witness_attestation,
    })];
    let batch = build_bridge_action_batch(&genesis, actions)?;
    write_bridge_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_bridge_pause_batch(
    options: BridgePauseBatchOptions,
) -> io::Result<BridgeActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let actions = vec![BridgeAction::Pause(BridgePauseAction {
        domain_id: options.domain_id,
        paused: options.paused,
    })];
    let batch = build_bridge_action_batch(&genesis, actions)?;
    write_bridge_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn apply_bridge_batch(options: ApplyBatchOptions) -> io::Result<Vec<Receipt>> {
    apply_bridge_batch_with_replay(options, None)
}

pub fn apply_bridge_batch_with_replay(
    options: ApplyBatchOptions,
    replay_block_file: Option<PathBuf>,
) -> io::Result<Vec<Receipt>> {
    let store = NodeStore::new(&options.data_dir);
    let commit_lock = store.lock_ordered_commit()?;
    recover_ordered_commit_journal_locked(&store, &commit_lock)?;
    let genesis = store.read_genesis()?;
    let mut bridge = store.read_bridge()?;
    let mut governance = store.read_governance()?;
    let validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let batch = read_bridge_action_batch_file(&options.batch_file)?;
    verify_bridge_action_batch_id(&genesis, &batch)?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("bridge batch `{}` already applied", batch.batch_id),
        ));
    }
    let mut ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let due_activations = activate_due_validator_registry_updates_for_commit(
        &store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    let certificate_validators = active_validator_ids(&governance)?;
    let certificate_material = read_commit_certificate_material(
        &store,
        &certificate_validators,
        options.certificate_file.as_deref(),
        None,
    )?;
    let (historical_replay, archived_payload_json) = historical_replay_commit_inputs(
        &certificate_material,
        replay_block_file.as_deref(),
        &options.batch_file,
    )?;
    let fastpay_pre_state_effects = match certificate_material.external_certificate.as_ref() {
        Some(certificate) => reconcile_certified_fastpay_pre_state_effects(
            &store,
            &mut ledger,
            &shielded,
            &certificate.fastpay_pre_state_effects,
        )?,
        None => fastpay_pre_state_effects_for_next_block(&store, &ledger)?,
    };

    let receipts = execute_bridge_batch(
        &genesis,
        &mut bridge,
        &batch,
        governance.bridge_witness_epoch,
        &validator_registry,
    );
    let mut proposed_ordered_batches = ordered_batches.clone();
    proposed_ordered_batches.push(batch.batch_id.clone());
    let consensus_proposal = build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash: parent_hash.clone(),
        view: certificate_material
            .external_certificate
            .as_ref()
            .map_or(0, |certificate| certificate.view),
        batch_kind: BATCH_KIND_BRIDGE,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects: fastpay_pre_state_effects.clone(),
    })?;
    verify_consensus_v2_finality_requirement(
        store.data_dir(),
        &genesis,
        &consensus_proposal,
        certificate_material.external_certificate.as_ref(),
    )?;
    let commit = prepare_ordered_commit(OrderedCommitPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        batch_kind: "bridge",
        batch_id: &batch.batch_id,
        payload: &batch,
        batch_receipts: &receipts,
        archived_payload_json: archived_payload_json.as_deref(),
        validator_keys: certificate_material.validator_keys.as_ref(),
        external_certificate: certificate_material.external_certificate.as_ref(),
        external_validator_registry: certificate_material.external_validator_registry.as_ref(),
        external_certificate_preverified: false,
        historical_replay,
        certificate_validators: &certificate_validators,
        fastpay_pre_state_effects: &fastpay_pre_state_effects,
    })?;
    let live_registry_update =
        live_validator_registry_after_due_updates(&store, &genesis, &governance, commit.height)?;

    write_ordered_commit_with_journal_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: None,
            governance: due_activations.governance_changed.then_some(governance),
            shielded: None,
            bridge: Some(bridge),
            commit,
            validator_registry: live_registry_update.or(due_activations.registry),
        },
    )?;
    Ok(receipts)
}

pub fn bridge_state(options: NodeOptions) -> io::Result<BridgeState> {
    let store = NodeStore::new(options.data_dir);
    store.read_bridge()
}

pub fn verify_bridge(options: NodeOptions) -> io::Result<BridgeVerificationReport> {
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let bridge = store.read_bridge()?;
    let validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    verify_bridge_state(&genesis, &bridge, &validator_registry)?;
    Ok(BridgeVerificationReport {
        verified: true,
        domain_count: bridge.domains.len(),
        transfer_count: bridge.transfers.len(),
        attestation_count: bridge
            .transfers
            .iter()
            .filter(|transfer| transfer.witness_attestation.is_some())
            .count(),
        replay_cache_count: bridge.replay_cache.len(),
        inbound_used: bridge
            .domains
            .iter()
            .map(|domain| domain.inbound_used)
            .sum(),
        outbound_used: bridge
            .domains
            .iter()
            .map(|domain| domain.outbound_used)
            .sum(),
        latest_transfer_id: bridge
            .transfers
            .last()
            .map(|transfer| transfer.transfer_id.clone())
            .unwrap_or_default(),
    })
}

fn verify_bridge_state(
    genesis: &Genesis,
    bridge: &BridgeState,
    validator_registry: &ValidatorRegistry,
) -> io::Result<()> {
    let mut replay = BridgeState {
        domains: bridge
            .domains
            .iter()
            .map(|domain| {
                let mut domain = domain.clone();
                domain.inbound_used = 0;
                domain.outbound_used = 0;
                domain.paused = false;
                domain
            })
            .collect(),
        transfers: Vec::new(),
        replay_cache: Vec::new(),
    };

    for domain in &bridge.domains {
        if domain.domain_id.trim().is_empty()
            || domain.source_chain.trim().is_empty()
            || domain.target_chain.trim().is_empty()
            || domain.bridge_id.trim().is_empty()
            || domain.door_account.trim().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge domain `{}` has incomplete metadata",
                    domain.domain_id
                ),
            ));
        }
        if domain.inbound_used > domain.inbound_cap || domain.outbound_used > domain.outbound_cap {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge domain `{}` cap accounting exceeds cap",
                    domain.domain_id
                ),
            ));
        }
    }

    for transfer in &bridge.transfers {
        if transfer.direction != BRIDGE_DIRECTION_INBOUND
            && transfer.direction != BRIDGE_DIRECTION_OUTBOUND
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge transfer `{}` has invalid direction `{}`",
                    transfer.transfer_id, transfer.direction
                ),
            ));
        }
        let action = BridgeTransferAction {
            domain_id: transfer.domain_id.clone(),
            direction: transfer.direction.clone(),
            from: transfer.from.clone(),
            to: transfer.to.clone(),
            asset_id: transfer.asset_id.clone(),
            amount: transfer.amount,
            witness_id: transfer.witness_id.clone(),
            witness_epoch: transfer.witness_epoch,
            witness_attestation: transfer.witness_attestation.clone(),
        };
        if let Some((code, message)) = bridge_witness_registry_error(&action, validator_registry) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge transfer `{}` {code}: {message}",
                    transfer.transfer_id
                ),
            ));
        }
        if let Some((code, message)) = bridge_witness_chain_domain_error(&action, genesis) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge transfer `{}` {code}: {message}",
                    transfer.transfer_id
                ),
            ));
        }
        let request = BridgeTransferRequest {
            domain_id: transfer.domain_id.clone(),
            direction: transfer.direction.clone(),
            from: transfer.from.clone(),
            to: transfer.to.clone(),
            asset_id: transfer.asset_id.clone(),
            amount: transfer.amount,
            witness_id: transfer.witness_id.clone(),
            witness_epoch: transfer.witness_epoch,
            witness_attestation: transfer.witness_attestation.clone(),
        };
        let replayed = apply_simulated_transfer(&mut replay, request).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge transfer `{}` replay failed: {error}",
                    transfer.transfer_id
                ),
            )
        })?;
        if replayed.transfer_id != transfer.transfer_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("bridge transfer `{}` id mismatch", transfer.transfer_id),
            ));
        }
    }

    if replay.replay_cache != bridge.replay_cache {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bridge replay cache does not match replayed transfers",
        ));
    }
    for replayed_domain in &replay.domains {
        let stored_domain = bridge.domain(&replayed_domain.domain_id).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge domain `{}` missing after replay",
                    replayed_domain.domain_id
                ),
            )
        })?;
        if replayed_domain.inbound_used != stored_domain.inbound_used
            || replayed_domain.outbound_used != stored_domain.outbound_used
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "bridge domain `{}` cap accounting mismatch",
                    replayed_domain.domain_id
                ),
            ));
        }
    }
    Ok(())
}

fn resolve_bridge_witness_epoch(requested: Option<u32>, governed_epoch: u32) -> io::Result<u32> {
    let witness_epoch = requested.unwrap_or(governed_epoch);
    if witness_epoch == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "bridge witness epoch must be nonzero",
        ));
    }
    Ok(witness_epoch)
}

fn build_bridge_witness_attestation(
    store: &NodeStore,
    genesis: &Genesis,
    bridge: &BridgeState,
    request: &BridgeTransferRequest,
    signer: &str,
) -> io::Result<BridgeWitnessAttestation> {
    let signer = if signer.trim().is_empty() {
        DEFAULT_BRIDGE_WITNESS_SIGNER
    } else {
        signer
    };
    let domain = bridge.domain(&request.domain_id).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("bridge domain `{}` not found", request.domain_id),
        )
    })?;
    let validator_count = validator_index(signer)
        .map(|index| index + 1)
        .unwrap_or(1)
        .max(1);
    let validator_keys = ensure_validator_keys(store, validator_count)?;
    let key_record = validator_key_record(&validator_keys, signer)?;
    let genesis_hash_hex = genesis_hash(genesis);
    let chain_domain = BridgeWitnessChainDomain {
        chain_id: &genesis.chain_id,
        genesis_hash: &genesis_hash_hex,
        protocol_version: genesis.protocol_version,
    };
    let message = bridge_witness_attestation_message(
        chain_domain,
        domain,
        request,
        signer,
        &key_record.algorithm_id,
        &key_record.public_key_hex,
    )
    .map_err(bridge_error)?;
    let private_key =
        Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    let signature_seed = bridge_witness_signature_seed(&message)?;
    let signature = ml_dsa_65_sign_with_context_seed(
        &private_key,
        &message,
        BRIDGE_WITNESS_SIGNATURE_CONTEXT,
        &signature_seed,
    )
    .map_err(invalid_data)?;
    let attestation_id = bridge_witness_attestation_id(
        chain_domain,
        domain,
        request,
        signer,
        &key_record.algorithm_id,
        &key_record.public_key_hex,
    )
    .map_err(bridge_error)?;
    Ok(BridgeWitnessAttestation {
        attestation_id,
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        signer: signer.to_string(),
        algorithm_id: key_record.algorithm_id.clone(),
        public_key_hex: key_record.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    })
}

fn bridge_witness_signature_seed(message: &[u8]) -> io::Result<[u8; 32]> {
    let digest = hash_bytes("postfiat.bridge_witness.signature_seed.v1", message);
    digest[..32].try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "bridge witness signature seed length mismatch",
        )
    })
}

pub fn export_snapshot(options: SnapshotExportOptions) -> io::Result<SnapshotManifest> {
    let report = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    verify_governance(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot export governance verification failed: {error}"),
        )
    })?;
    verify_bridge(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot export bridge verification failed: {error}"),
        )
    })?;
    verify_shielded(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot export shielded verification failed: {error}"),
        )
    })?;
    verify_mempool(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot export mempool verification failed: {error}"),
        )
    })?;
    verify_blocks(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot export block verification failed: {error}"),
        )
    })?;
    let snapshot_dir = options.snapshot_dir;
    std::fs::create_dir_all(&snapshot_dir)?;
    let store = NodeStore::new(&options.data_dir);

    let mut files = Vec::new();
    for &file_name in SNAPSHOT_FILES {
        let bytes = snapshot_file_bytes(&store, &options.data_dir, file_name)?;
        atomic_write(snapshot_dir.join(file_name), &bytes)?;
        files.push(SnapshotFile {
            name: (*file_name).to_string(),
            bytes: bytes.len() as u64,
            hash_hex: hash_hex("postfiat.snapshot.file.v1", &bytes),
        });
    }

    let manifest = SnapshotManifest {
        snapshot_version: SNAPSHOT_VERSION,
        chain_id: report.chain_id,
        genesis_hash: report.genesis_hash,
        protocol_version: report.protocol_version,
        node_id: report.node_id,
        state_root: report.state_root,
        block_height: report.block_height,
        block_tip_hash: report.block_tip_hash,
        exported_unix: unix_now(),
        files,
    };
    write_snapshot_manifest(&snapshot_dir.join(SNAPSHOT_MANIFEST_FILE), &manifest)?;
    Ok(manifest)
}

fn signed_snapshot_manifest_payload(signed: &SignedSnapshotManifest) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        signed.schema.as_str(),
        signed.publisher.as_str(),
        signed.algorithm_id.as_str(),
        signed.public_key_hex.as_str(),
        signed.source_build_git_revision.as_str(),
        signed.source_build_profile.as_str(),
        signed.last_certificate_id.as_deref(),
        signed.mempool_policy.as_str(),
        signed.signer_material_included,
        &signed.manifest,
    ))
    .map_err(invalid_data)
}

fn verify_signed_snapshot_manifest(
    signed: &SignedSnapshotManifest,
    trusted: &SnapshotPublisherPublicKey,
) -> io::Result<()> {
    if signed.schema != SIGNED_SNAPSHOT_MANIFEST_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported signed snapshot schema `{}`", signed.schema),
        ));
    }
    if trusted.schema != SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported snapshot publisher key schema `{}`",
                trusted.schema
            ),
        ));
    }
    if signed.publisher != trusted.publisher
        || signed.algorithm_id != trusted.algorithm_id
        || signed.public_key_hex != trusted.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signed snapshot publisher does not match trusted publisher key",
        ));
    }
    if signed.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported signed snapshot algorithm `{}`",
                signed.algorithm_id
            ),
        ));
    }
    if signed.signer_material_included
        || signed.mempool_policy != "preserve-verified-empty-or-pending"
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signed snapshot signer isolation or mempool policy is invalid",
        ));
    }
    let public_key = hex_to_bytes(&signed.public_key_hex).map_err(invalid_data)?;
    let signature = hex_to_bytes(&signed.signature_hex).map_err(invalid_data)?;
    let payload = signed_snapshot_manifest_payload(signed)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &payload,
        &signature,
        SNAPSHOT_MANIFEST_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "signed snapshot manifest signature verification failed",
        ));
    }
    Ok(())
}

pub fn export_snapshot_publisher_public_key(
    options: SnapshotPublisherKeyExportOptions,
) -> io::Result<SnapshotPublisherPublicKey> {
    let key = read_key_file(&options.publisher_key_file)?;
    let public = SnapshotPublisherPublicKey {
        schema: SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA.to_string(),
        publisher: key.address,
        algorithm_id: key.algorithm_id,
        public_key_hex: key.public_key_hex,
    };
    let json = serde_json::to_string_pretty(&public).map_err(invalid_data)?;
    atomic_write(&options.public_key_file, format!("{json}\n"))?;
    Ok(public)
}

pub fn export_signed_snapshot(
    options: SignedSnapshotExportOptions,
) -> io::Result<SignedSnapshotManifest> {
    let publisher_key = read_key_file(&options.publisher_key_file)?;
    let source_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })?;
    let blocks = NodeStore::new(&options.data_dir).read_blocks()?;
    let last_certificate_id = blocks
        .blocks
        .last()
        .map(|block| block.header.certificate_id.clone());
    let manifest = export_snapshot(SnapshotExportOptions {
        data_dir: options.data_dir,
        snapshot_dir: options.snapshot_dir.clone(),
    })?;
    let mut signed = SignedSnapshotManifest {
        schema: SIGNED_SNAPSHOT_MANIFEST_SCHEMA.to_string(),
        publisher: publisher_key.address,
        algorithm_id: publisher_key.algorithm_id,
        public_key_hex: publisher_key.public_key_hex,
        source_build_git_revision: source_status.build_git_revision,
        source_build_profile: source_status.build_profile,
        last_certificate_id,
        mempool_policy: "preserve-verified-empty-or-pending".to_string(),
        signer_material_included: false,
        manifest,
        signature_hex: String::new(),
    };
    let private_key =
        Zeroizing::new(hex_to_bytes(&publisher_key.private_key_hex).map_err(invalid_data)?);
    let payload = signed_snapshot_manifest_payload(&signed)?;
    let signature =
        ml_dsa_65_sign_with_context(&private_key, &payload, SNAPSHOT_MANIFEST_SIGNATURE_CONTEXT)
            .map_err(invalid_data)?;
    signed.signature_hex = bytes_to_hex(&signature);
    let trusted_self = SnapshotPublisherPublicKey {
        schema: SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA.to_string(),
        publisher: signed.publisher.clone(),
        algorithm_id: signed.algorithm_id.clone(),
        public_key_hex: signed.public_key_hex.clone(),
    };
    verify_signed_snapshot_manifest(&signed, &trusted_self)?;
    let json = serde_json::to_string_pretty(&signed).map_err(invalid_data)?;
    atomic_write(
        options.snapshot_dir.join(SIGNED_SNAPSHOT_MANIFEST_FILE),
        format!("{json}\n"),
    )?;
    Ok(signed)
}

pub fn import_signed_snapshot(options: SignedSnapshotImportOptions) -> io::Result<StatusReport> {
    let signed: SignedSnapshotManifest = read_json_file(
        &options.snapshot_dir.join(SIGNED_SNAPSHOT_MANIFEST_FILE),
        "signed snapshot manifest",
    )?;
    let trusted: SnapshotPublisherPublicKey = read_json_file(
        &options.trusted_publisher_key_file,
        "trusted snapshot publisher key",
    )?;
    verify_signed_snapshot_manifest(&signed, &trusted)?;
    let unsigned = read_snapshot_manifest(&options.snapshot_dir.join(SNAPSHOT_MANIFEST_FILE))?;
    if unsigned != signed.manifest {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsigned snapshot manifest does not match signed manifest",
        ));
    }
    import_snapshot(SnapshotImportOptions {
        data_dir: options.data_dir,
        snapshot_dir: options.snapshot_dir,
        node_id: options.node_id,
    })
}

pub(super) fn sha256_file_hex(path: &Path, label: &str) -> io::Result<String> {
    let bytes = std::fs::read(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("{label} `{}` read failed: {error}", path.display()),
        )
    })?;
    let mut hasher = Sha256::new();
    Sha2Digest::update(&mut hasher, &bytes);
    Ok(bytes_to_hex(&hasher.finalize()))
}

pub(super) fn validate_deployment_identifier(value: &str, label: &str) -> io::Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("deployment {label} is not a canonical identifier"),
        ));
    }
    Ok(())
}

fn validate_deployment_sha256(hash: &str, label: &str) -> io::Result<()> {
    if hash.len() != 64 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("deployment manifest {label} sha256 is invalid"),
        ));
    }
    Ok(())
}

fn validate_deployment_validator_bindings(
    bindings: &[DeploymentValidatorBinding],
    expected_validator_ids: Option<&[String]>,
) -> io::Result<()> {
    if bindings.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment manifest contains no validator bindings",
        ));
    }
    let binding_ids = bindings
        .iter()
        .map(|binding| binding.validator_id.clone())
        .collect::<Vec<_>>();
    if binding_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment validator bindings are not strictly sorted by validator_id",
        ));
    }
    if let Some(expected) = expected_validator_ids {
        if binding_ids != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "deployment validator bindings do not exactly match topology validators",
            ));
        }
    }
    for binding in bindings {
        validate_deployment_identifier(&binding.validator_id, "validator_id")?;
        if binding.services.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "deployment validator binding `{}` has no service artifacts",
                    binding.validator_id
                ),
            ));
        }
        let service_ids = binding
            .services
            .iter()
            .map(|service| service.service_id.clone())
            .collect::<Vec<_>>();
        if service_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "deployment services for validator `{}` are not strictly sorted",
                    binding.validator_id
                ),
            ));
        }
        let expected_service_ids = ["rpc", "transport"];
        if service_ids.len() != expected_service_ids.len()
            || service_ids
                .iter()
                .map(String::as_str)
                .zip(expected_service_ids)
                .any(|(actual, expected)| actual != expected)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "deployment validator binding `{}` must include exactly rpc and transport services",
                    binding.validator_id
                ),
            ));
        }
        for service in &binding.services {
            validate_deployment_identifier(&service.service_id, "service_id")?;
            validate_deployment_sha256(&service.service_unit_sha256, "service unit")?;
            validate_deployment_sha256(&service.environment_sha256, "environment")?;
        }
    }
    Ok(())
}

pub(super) fn read_deployment_validator_bindings_file(
    path: &Path,
) -> io::Result<Vec<DeploymentValidatorBinding>> {
    let input: DeploymentValidatorBindingsFile =
        read_json_file(path, "deployment validator bindings")?;
    if input.schema != DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported deployment validator bindings schema `{}`",
                input.schema
            ),
        ));
    }
    let mut bindings = Vec::with_capacity(input.validators.len());
    for validator in input.validators {
        validate_deployment_identifier(&validator.validator_id, "validator_id")?;
        let mut services = Vec::with_capacity(validator.services.len());
        for service in validator.services {
            validate_deployment_identifier(&service.service_id, "service_id")?;
            if !service.service_unit_file.is_absolute() || !service.environment_file.is_absolute() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "deployment validator service paths must be absolute",
                ));
            }
            services.push(DeploymentServiceArtifact {
                service_id: service.service_id,
                service_unit_sha256: sha256_file_hex(
                    &service.service_unit_file,
                    "deployment validator service unit",
                )?,
                environment_sha256: sha256_file_hex(
                    &service.environment_file,
                    "deployment validator environment",
                )?,
            });
        }
        bindings.push(DeploymentValidatorBinding {
            validator_id: validator.validator_id,
            services,
        });
    }
    validate_deployment_validator_bindings(&bindings, None)?;
    Ok(bindings)
}

fn deployment_manifest_payload(manifest: &DeploymentManifest) -> io::Result<Vec<u8>> {
    let mut unsigned = manifest.clone();
    unsigned.signature_hex.clear();
    serde_json::to_vec(&unsigned).map_err(invalid_data)
}

fn deployment_publisher_public_key(
    key: &DeploymentPublisherPrivateKey,
) -> DeploymentPublisherPublicKey {
    DeploymentPublisherPublicKey {
        schema: DEPLOYMENT_PUBLISHER_PUBLIC_KEY_SCHEMA.to_string(),
        publisher: key.publisher.clone(),
        algorithm_id: key.algorithm_id.clone(),
        public_key_hex: key.public_key_hex.clone(),
    }
}

fn validate_deployment_publisher_private_key(
    key: &DeploymentPublisherPrivateKey,
) -> io::Result<()> {
    if key.schema != DEPLOYMENT_PUBLISHER_PRIVATE_KEY_SCHEMA
        || key.purpose != DEPLOYMENT_PUBLISHER_KEY_PURPOSE
        || key.algorithm_id != ML_DSA_65_ALGORITHM
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment publisher key has an unsupported schema, purpose, or algorithm",
        ));
    }
    let public_key = hex_to_bytes(&key.public_key_hex).map_err(invalid_data)?;
    ml_dsa_65_validate_public_key(&public_key).map_err(invalid_data)?;
    if key.publisher != address_from_public_key(&public_key) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment publisher key publisher does not match public key",
        ));
    }
    let private_key = Zeroizing::new(hex_to_bytes(&key.private_key_hex).map_err(invalid_data)?);
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT,
        DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT,
    )
    .map_err(invalid_data)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT,
        &signature,
        DEPLOYMENT_PUBLISHER_KEY_SELF_CHECK_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment publisher private key does not match public key",
        ));
    }
    Ok(())
}

pub(super) fn read_deployment_publisher_private_key(
    path: &Path,
) -> io::Result<DeploymentPublisherPrivateKey> {
    validate_private_file_permissions(path, "deployment publisher key")?;
    let key: DeploymentPublisherPrivateKey = read_json_file(path, "deployment publisher key")?;
    validate_deployment_publisher_private_key(&key)?;
    Ok(key)
}

fn write_new_deployment_publisher_private_key(path: &Path, contents: &[u8]) -> io::Result<()> {
    {
        #[cfg(unix)]
        let mut file = {
            use std::os::unix::fs::OpenOptionsExt;

            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(path)?
        };
        #[cfg(not(unix))]
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        use std::io::Write;

        file.write_all(contents)?;
        file.sync_all()?;
    }
    set_private_file_permissions(path)?;
    #[cfg(unix)]
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::File::open(parent)?.sync_all()?;
    }
    Ok(())
}

pub fn create_deployment_publisher_private_key(
    options: DeploymentPublisherKeyCreateOptions,
) -> io::Result<DeploymentPublisherPublicKey> {
    if options.publisher_key_file.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "deployment publisher key `{}` already exists",
                options.publisher_key_file.display()
            ),
        ));
    }
    let parent = options.publisher_key_file.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment publisher key path has no parent directory",
        )
    })?;
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "deployment publisher key directory `{}` does not exist",
                parent.display()
            ),
        ));
    }
    let key_pair = ml_dsa_65_keygen().map_err(invalid_data)?;
    let key = DeploymentPublisherPrivateKey {
        schema: DEPLOYMENT_PUBLISHER_PRIVATE_KEY_SCHEMA.to_string(),
        purpose: DEPLOYMENT_PUBLISHER_KEY_PURPOSE.to_string(),
        publisher: address_from_public_key(&key_pair.public_key),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    };
    validate_deployment_publisher_private_key(&key)?;
    let json = serde_json::to_string_pretty(&key).map_err(invalid_data)?;
    write_new_deployment_publisher_private_key(
        &options.publisher_key_file,
        format!("{json}\n").as_bytes(),
    )?;
    Ok(deployment_publisher_public_key(&key))
}

fn deployment_staged_root_path(rootfs: &Path, target: &Path) -> io::Result<PathBuf> {
    let relative = target.strip_prefix("/").map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment staged target path must be absolute",
        )
    })?;
    Ok(rootfs.join(relative))
}

fn deployment_stage_write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let json = serde_json::to_string_pretty(value).map_err(invalid_data)?;
    write_public_deployment_artifact(path, format!("{json}\n"))
}

#[cfg(unix)]
fn set_public_deployment_artifact_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o644);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_public_deployment_artifact_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn write_public_deployment_artifact(path: &Path, contents: impl AsRef<[u8]>) -> io::Result<()> {
    atomic_write(path, contents)?;
    set_public_deployment_artifact_permissions(path)
}

#[cfg(unix)]
fn normalize_deployment_rootfs_permissions(rootfs: &Path, binary: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fn normalize(directory: &Path, binary: &Path) -> io::Result<()> {
        let mut directory_permissions = std::fs::metadata(directory)?.permissions();
        directory_permissions.set_mode(0o755);
        std::fs::set_permissions(directory, directory_permissions)?;
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                normalize(&path, binary)?;
                continue;
            }
            let mut permissions = entry.metadata()?.permissions();
            permissions.set_mode(if path == binary { 0o755 } else { 0o644 });
            std::fs::set_permissions(path, permissions)?;
        }
        Ok(())
    }

    normalize(rootfs, binary)
}

#[cfg(not(unix))]
fn normalize_deployment_rootfs_permissions(_rootfs: &Path, _binary: &Path) -> io::Result<()> {
    Ok(())
}

fn deployment_runtime_environment(
    release_config_dir: &Path,
    binary_path: &Path,
    validator_id: &str,
) -> String {
    format!(
        "POSTFIAT_DEPLOYMENT_MANIFEST={}\nPOSTFIAT_DEPLOYMENT_VALIDATOR_ID={}\nPOSTFIAT_DEPLOYMENT_VALIDATOR_BINDINGS_FILE={}/{}.bindings.json\nPOSTFIAT_DEPLOYMENT_BINARY={}\nPOSTFIAT_DEPLOYMENT_TOPOLOGY={}/topology.json\nPOSTFIAT_DEPLOYMENT_SWAP_CIRCUIT_METADATA={}/swap.metadata.json\nPOSTFIAT_DEPLOYMENT_PRIVATE_EGRESS_CIRCUIT_METADATA={}/private-egress.metadata.json\n",
        release_config_dir.join("deployment-manifest.json").display(),
        validator_id,
        release_config_dir.display(),
        validator_id,
        binary_path.display(),
        release_config_dir.display(),
        release_config_dir.display(),
        release_config_dir.display(),
    )
}

fn deployment_manifest_verify_command(
    release_config_dir: &Path,
    binary_path: &Path,
    validator_id: &str,
) -> String {
    format!(
        "{} deployment-manifest-verify --manifest-file {}/deployment-manifest.json --trusted-publisher-key-file {}/deployment.public.json --validator-id {} --validator-bindings-file {}/{}.bindings.json --runtime-binary-file {} --runtime-topology-file {}/topology.json --runtime-swap-circuit-metadata-file {}/swap.metadata.json --runtime-private-egress-circuit-metadata-file {}/private-egress.metadata.json",
        binary_path.display(),
        release_config_dir.display(),
        release_config_dir.display(),
        validator_id,
        release_config_dir.display(),
        validator_id,
        binary_path.display(),
        release_config_dir.display(),
        release_config_dir.display(),
        release_config_dir.display(),
    )
}

fn validate_private_deployment_bind_host(host: &str) -> io::Result<()> {
    let normalized = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_ascii_lowercase();
    let private = normalized == "localhost"
        || normalized
            .parse::<std::net::IpAddr>()
            .map(|address| match address {
                std::net::IpAddr::V4(address) => {
                    address.is_loopback() || address.is_private() || address.is_link_local()
                }
                std::net::IpAddr::V6(address) => {
                    address.is_loopback()
                        || address.is_unique_local()
                        || address.is_unicast_link_local()
                }
            })
            .unwrap_or(false);
    if private {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("deployment bind host `{host}` is not loopback or a private-overlay IP address"),
    ))
}

fn deployment_transport_unit(
    release_config_dir: &Path,
    binary_path: &Path,
    data_dir: &Path,
    log_dir: &Path,
    validator_id: &str,
    bind_host: &str,
) -> String {
    let ready_file = data_dir.join("readiness/transport-validator.ready.json");
    format!(
        "[Unit]\nDescription=PostFiat L1 validator {validator_id}\nAfter=network-online.target\nWants=network-online.target\nStartLimitIntervalSec=300\nStartLimitBurst=5\n\n[Service]\nType=simple\nUser=postfiat\nGroup=postfiat\nEnvironmentFile={release_config_dir}/{validator_id}.transport.env\nExecStartPre=/usr/bin/rm -f {ready_file}\nExecStartPre={verify_command}\nExecStart={binary_path} transport-validator-serve --unsafe-devnet-file-signer --unsafe-devnet-json-storage --data-dir {data_dir} --topology {release_config_dir}/topology.json --key-file {data_dir}/validator_keys.json --vote-dir {data_dir}/transport_votes --bind-host {bind_host} --max-connections 10000 --timeout-ms 900000 --event-log {log_dir}/transport-validator-events.ndjson\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectControlGroups=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectKernelLogs=true\nRestrictSUIDSGID=true\nLockPersonality=true\nRestrictRealtime=true\nSystemCallArchitectures=native\nCapabilityBoundingSet=\nAmbientCapabilities=\nReadWritePaths={data_dir} {log_dir}\nUMask=0077\nLimitNOFILE=65536\nLimitCORE=0\nTasksMax=1024\nKillMode=control-group\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=30\nStandardOutput=append:{log_dir}/stdout.log\nStandardError=append:{log_dir}/stderr.log\n\n[Install]\nWantedBy=multi-user.target\n",
        release_config_dir = release_config_dir.display(),
        ready_file = ready_file.display(),
        verify_command = deployment_manifest_verify_command(
            release_config_dir,
            binary_path,
            validator_id,
        ),
        binary_path = binary_path.display(),
        data_dir = data_dir.display(),
        log_dir = log_dir.display(),
        bind_host = bind_host,
    )
}

fn deployment_rpc_unit(
    release_config_dir: &Path,
    binary_path: &Path,
    data_dir: &Path,
    log_dir: &Path,
    validator_id: &str,
    rpc_port: u16,
) -> String {
    let ready_file = data_dir.join("readiness/rpc.ready.json");
    format!(
        "[Unit]\nDescription=PostFiat L1 RPC {validator_id}\nAfter=network-online.target postfiat-{validator_id}.service\nWants=network-online.target\nStartLimitIntervalSec=300\nStartLimitBurst=5\n\n[Service]\nType=simple\nUser=postfiat\nGroup=postfiat\nEnvironmentFile={release_config_dir}/{validator_id}.rpc.env\nExecStartPre=+/usr/bin/install -d -o postfiat -g postfiat -m 0700 {data_dir}/finality-artifacts\nExecStartPre=/usr/bin/rm -f {ready_file}\nExecStartPre={verify_command}\nExecStart={binary_path} rpc-serve --unsafe-devnet-json-storage --data-dir {data_dir} --spool-dir {data_dir}/runtime/rpc-spool --ready-file {ready_file} --bind-host 127.0.0.1 --port {rpc_port} --max-requests 10000 --timeout-ms 30000 --child-timeout-ms 30000 --event-log {log_dir}/rpc-events.ndjson --allow-mempool-submit-finality --finality-topology {release_config_dir}/topology.json --finality-key-file {data_dir}/validator_keys.json --finality-proposal-key-file {data_dir}/validator_keys.json --finality-artifact-root {data_dir}/finality-artifacts --finality-timeout-ms 30000 --finality-send-retries 16 --finality-retry-backoff-ms 250 --finality-quorum-early-full-propagation --keep-alive\nNoNewPrivileges=true\nPrivateTmp=true\nProtectSystem=strict\nProtectHome=true\nProtectControlGroups=true\nProtectKernelTunables=true\nProtectKernelModules=true\nProtectKernelLogs=true\nRestrictSUIDSGID=true\nLockPersonality=true\nRestrictRealtime=true\nSystemCallArchitectures=native\nCapabilityBoundingSet=\nAmbientCapabilities=\nReadWritePaths={data_dir} {log_dir}\nUMask=0077\nLimitNOFILE=65536\nLimitCORE=0\nTasksMax=1024\nKillMode=control-group\nRestart=on-failure\nRestartSec=5\nTimeoutStopSec=30\nStandardOutput=append:{log_dir}/rpc-stdout.log\nStandardError=append:{log_dir}/rpc-stderr.log\n\n[Install]\nWantedBy=multi-user.target\n",
        release_config_dir = release_config_dir.display(),
        ready_file = ready_file.display(),
        verify_command = deployment_manifest_verify_command(
            release_config_dir,
            binary_path,
            validator_id,
        ),
        binary_path = binary_path.display(),
        data_dir = data_dir.display(),
        log_dir = log_dir.display(),
    )
}

#[cfg(unix)]
fn deployment_stage_set_executable(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn deployment_stage_set_executable(_path: &Path) -> io::Result<()> {
    Ok(())
}

pub fn stage_deployment_validator_units(
    options: DeploymentValidatorUnitsStageOptions,
) -> io::Result<DeploymentValidatorUnitsStageReport> {
    validate_deployment_identifier(&options.release_id, "release_id")?;
    if !options.output_dir.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment stage output directory must be absolute",
        ));
    }
    if options.output_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "deployment stage output `{}` already exists",
                options.output_dir.display()
            ),
        ));
    }
    let parent = options.output_dir.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment stage output has no parent directory",
        )
    })?;
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "deployment stage output parent directory does not exist",
        ));
    }
    let topology: NetworkTopology =
        read_json_file(&options.topology_file, "deployment stage topology")?;
    if topology.peers.len() != 6 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "controlled-testnet deployment stage requires exactly six validators",
        ));
    }
    let validator_ids = topology
        .peers
        .iter()
        .map(|peer| peer.node_id.clone())
        .collect::<Vec<_>>();
    if validator_ids.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment stage topology validators must be strictly sorted",
        ));
    }
    for peer in &topology.peers {
        validate_deployment_identifier(&peer.node_id, "validator_id")?;
        if peer.host.trim().is_empty() || peer.p2p_port == 0 || peer.rpc_port == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment stage topology contains an incomplete peer",
            ));
        }
        validate_private_deployment_bind_host(&peer.host).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "deployment stage topology requires a private validator overlay for {}: {error}",
                    peer.node_id
                ),
            )
        })?;
    }

    std::fs::create_dir(&options.output_dir)?;
    let result = (|| -> io::Result<DeploymentValidatorUnitsStageReport> {
        let rootfs = options.output_dir.join("rootfs");
        let release_config_dir = PathBuf::from("/etc/postfiat/releases").join(&options.release_id);
        let binary_path = PathBuf::from("/opt/postfiat/releases")
            .join(&options.release_id)
            .join("postfiat-node");
        let topology_path = release_config_dir.join("topology.json");
        let swap_metadata_path = release_config_dir.join("swap.metadata.json");
        let private_egress_metadata_path = release_config_dir.join("private-egress.metadata.json");
        let staged_binary = deployment_staged_root_path(&rootfs, &binary_path)?;
        let staged_topology = deployment_staged_root_path(&rootfs, &topology_path)?;
        let staged_swap_metadata = deployment_staged_root_path(&rootfs, &swap_metadata_path)?;
        let staged_private_egress_metadata =
            deployment_staged_root_path(&rootfs, &private_egress_metadata_path)?;
        atomic_write(&staged_binary, std::fs::read(&options.binary_file)?)?;
        deployment_stage_set_executable(&staged_binary)?;
        write_public_deployment_artifact(&staged_topology, std::fs::read(&options.topology_file)?)?;
        write_public_deployment_artifact(
            &staged_swap_metadata,
            std::fs::read(&options.swap_circuit_metadata_file)?,
        )?;
        write_public_deployment_artifact(
            &staged_private_egress_metadata,
            std::fs::read(&options.private_egress_circuit_metadata_file)?,
        )?;

        let mut signing_entries = Vec::with_capacity(topology.peers.len());
        let mut rows = Vec::with_capacity(topology.peers.len());
        for peer in &topology.peers {
            let validator_id = &peer.node_id;
            let data_dir = PathBuf::from("/var/lib/postfiat").join(validator_id);
            let log_dir = PathBuf::from("/var/log/postfiat").join(validator_id);
            let rpc_unit_target = PathBuf::from(format!(
                "/etc/systemd/system/postfiat-{validator_id}-rpc.service"
            ));
            let transport_unit_target = PathBuf::from(format!(
                "/etc/systemd/system/postfiat-{validator_id}.service"
            ));
            let rpc_environment_target = release_config_dir.join(format!("{validator_id}.rpc.env"));
            let transport_environment_target =
                release_config_dir.join(format!("{validator_id}.transport.env"));
            let runtime_bindings_target =
                release_config_dir.join(format!("{validator_id}.bindings.json"));
            let rpc_unit = deployment_staged_root_path(&rootfs, &rpc_unit_target)?;
            let transport_unit = deployment_staged_root_path(&rootfs, &transport_unit_target)?;
            let rpc_environment = deployment_staged_root_path(&rootfs, &rpc_environment_target)?;
            let transport_environment =
                deployment_staged_root_path(&rootfs, &transport_environment_target)?;
            let runtime_bindings = deployment_staged_root_path(&rootfs, &runtime_bindings_target)?;

            let base_environment =
                deployment_runtime_environment(&release_config_dir, &binary_path, validator_id);
            write_public_deployment_artifact(&rpc_environment, &base_environment)?;
            write_public_deployment_artifact(
                &transport_environment,
                format!(
                    "{base_environment}POSTFIAT_PREWARM_SHIELDED_VERIFIER=1\nPOSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=1\nPOSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1\nPOSTFIAT_TRANSPORT_VALIDATOR_READY_FILE={}/readiness/transport-validator.ready.json\nPOSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE={}/readiness/transport-block-vote.ready.json\n",
                    data_dir.display(),
                    data_dir.display(),
                ),
            )?;
            write_public_deployment_artifact(
                &rpc_unit,
                deployment_rpc_unit(
                    &release_config_dir,
                    &binary_path,
                    &data_dir,
                    &log_dir,
                    validator_id,
                    peer.rpc_port,
                ),
            )?;
            write_public_deployment_artifact(
                &transport_unit,
                deployment_transport_unit(
                    &release_config_dir,
                    &binary_path,
                    &data_dir,
                    &log_dir,
                    validator_id,
                    &peer.host,
                ),
            )?;

            let runtime_binding = DeploymentValidatorBindingsFile {
                schema: DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA.to_string(),
                validators: vec![DeploymentValidatorBindingFileEntry {
                    validator_id: validator_id.clone(),
                    services: vec![
                        DeploymentServiceBindingFileEntry {
                            service_id: "rpc".to_string(),
                            service_unit_file: rpc_unit_target,
                            environment_file: rpc_environment_target,
                        },
                        DeploymentServiceBindingFileEntry {
                            service_id: "transport".to_string(),
                            service_unit_file: transport_unit_target,
                            environment_file: transport_environment_target,
                        },
                    ],
                }],
            };
            deployment_stage_write_json(&runtime_bindings, &runtime_binding)?;
            signing_entries.push(DeploymentValidatorBindingFileEntry {
                validator_id: validator_id.clone(),
                services: vec![
                    DeploymentServiceBindingFileEntry {
                        service_id: "rpc".to_string(),
                        service_unit_file: rpc_unit.clone(),
                        environment_file: rpc_environment.clone(),
                    },
                    DeploymentServiceBindingFileEntry {
                        service_id: "transport".to_string(),
                        service_unit_file: transport_unit.clone(),
                        environment_file: transport_environment.clone(),
                    },
                ],
            });
            rows.push(DeploymentValidatorUnitsStageRow {
                validator_id: validator_id.clone(),
                rpc_unit_file: rpc_unit,
                rpc_environment_file: rpc_environment,
                transport_unit_file: transport_unit,
                transport_environment_file: transport_environment,
                runtime_bindings_file: runtime_bindings,
            });
        }
        let signing_bindings_file = options.output_dir.join("validator-bindings.signing.json");
        deployment_stage_write_json(
            &signing_bindings_file,
            &DeploymentValidatorBindingsFile {
                schema: DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA.to_string(),
                validators: signing_entries,
            },
        )?;
        normalize_deployment_rootfs_permissions(&rootfs, &staged_binary)?;
        let report = DeploymentValidatorUnitsStageReport {
            schema: DEPLOYMENT_VALIDATOR_UNIT_STAGE_SCHEMA.to_string(),
            release_id: options.release_id,
            rootfs_dir: rootfs,
            binary_file: staged_binary,
            topology_file: staged_topology,
            swap_circuit_metadata_file: staged_swap_metadata,
            private_egress_circuit_metadata_file: staged_private_egress_metadata,
            signing_bindings_file,
            validators: rows,
        };
        deployment_stage_write_json(&options.output_dir.join("stage-report.json"), &report)?;
        Ok(report)
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&options.output_dir);
    }
    result
}

fn verify_deployment_manifest_record(
    manifest: &DeploymentManifest,
    trusted: &DeploymentPublisherPublicKey,
    now_unix: u64,
) -> io::Result<()> {
    if manifest.schema != DEPLOYMENT_MANIFEST_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported deployment manifest schema `{}`",
                manifest.schema
            ),
        ));
    }
    if trusted.schema != DEPLOYMENT_PUBLISHER_PUBLIC_KEY_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported deployment publisher key schema `{}`",
                trusted.schema
            ),
        ));
    }
    if manifest.publisher != trusted.publisher
        || manifest.algorithm_id != trusted.algorithm_id
        || manifest.public_key_hex != trusted.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment manifest publisher does not match trusted publisher key",
        ));
    }
    if manifest.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported deployment manifest algorithm `{}`",
                manifest.algorithm_id
            ),
        ));
    }
    if manifest.deployment_id.trim().is_empty()
        || manifest.chain_id.trim().is_empty()
        || manifest.genesis_hash.trim().is_empty()
        || manifest.git_revision.trim().is_empty()
        || manifest.build_profile.trim().is_empty()
        || manifest.rpc_schema.trim().is_empty()
        || manifest.build_features.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment manifest contains an empty required identity field",
        ));
    }
    let mut canonical_features = manifest.build_features.clone();
    canonical_features.sort();
    canonical_features.dedup();
    if canonical_features != manifest.build_features
        || manifest.valid_from_unix > manifest.valid_until_unix
        || now_unix < manifest.valid_from_unix
        || now_unix > manifest.valid_until_unix
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment manifest is non-canonical, inactive, or expired",
        ));
    }
    for (label, hash) in [
        ("binary", &manifest.binary_sha256),
        ("service unit", &manifest.service_unit_sha256),
        ("environment", &manifest.environment_sha256),
        ("topology", &manifest.topology_sha256),
        (
            "swap circuit metadata",
            &manifest.swap_circuit_metadata_sha256,
        ),
        (
            "private-egress circuit metadata",
            &manifest.private_egress_circuit_metadata_sha256,
        ),
    ] {
        validate_deployment_sha256(hash, label)?;
    }
    validate_deployment_validator_bindings(&manifest.validator_bindings, None)?;
    let public_key = hex_to_bytes(&manifest.public_key_hex).map_err(invalid_data)?;
    let signature = hex_to_bytes(&manifest.signature_hex).map_err(invalid_data)?;
    let payload = deployment_manifest_payload(manifest)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &payload,
        &signature,
        DEPLOYMENT_MANIFEST_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "deployment manifest signature verification failed",
        ));
    }
    Ok(())
}

pub fn export_deployment_publisher_public_key(
    options: DeploymentPublisherKeyExportOptions,
) -> io::Result<DeploymentPublisherPublicKey> {
    let key = read_deployment_publisher_private_key(&options.publisher_key_file)?;
    let public = deployment_publisher_public_key(&key);
    let json = serde_json::to_string_pretty(&public).map_err(invalid_data)?;
    write_public_deployment_artifact(&options.public_key_file, format!("{json}\n"))?;
    Ok(public)
}

pub fn create_deployment_manifest(
    options: DeploymentManifestCreateOptions,
) -> io::Result<DeploymentManifest> {
    if options.valid_from_unix > options.valid_until_unix {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment manifest validity window is inverted",
        ));
    }
    let publisher_key = read_deployment_publisher_private_key(&options.publisher_key_file)?;
    let mut features = options
        .build_features
        .into_iter()
        .map(|feature| feature.trim().to_string())
        .filter(|feature| !feature.is_empty())
        .collect::<Vec<_>>();
    features.sort();
    features.dedup();
    let topology: NetworkTopology = read_json_file(&options.topology_file, "deployment topology")?;
    if topology.chain_id != options.chain_id
        || topology.genesis_hash != options.genesis_hash
        || topology.protocol_version != options.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment topology does not match manifest chain, genesis, or protocol",
        ));
    }
    let mut expected_validator_ids = topology
        .peers
        .iter()
        .map(|peer| peer.node_id.clone())
        .collect::<Vec<_>>();
    expected_validator_ids.sort();
    if expected_validator_ids
        .windows(2)
        .any(|pair| pair[0] == pair[1])
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "deployment topology contains duplicate validator IDs",
        ));
    }
    let validator_bindings =
        read_deployment_validator_bindings_file(&options.validator_bindings_file)?;
    validate_deployment_validator_bindings(&validator_bindings, Some(&expected_validator_ids))?;
    let mut manifest = DeploymentManifest {
        schema: DEPLOYMENT_MANIFEST_SCHEMA.to_string(),
        deployment_id: options.deployment_id,
        created_unix: unix_now(),
        valid_from_unix: options.valid_from_unix,
        valid_until_unix: options.valid_until_unix,
        chain_id: options.chain_id,
        genesis_hash: options.genesis_hash,
        git_revision: options.git_revision,
        binary_sha256: sha256_file_hex(&options.binary_file, "deployment binary")?,
        build_profile: options.build_profile,
        build_features: features,
        protocol_version: options.protocol_version,
        rpc_schema: options.rpc_schema,
        service_unit_sha256: sha256_file_hex(
            &options.service_unit_file,
            "deployment service unit",
        )?,
        environment_sha256: sha256_file_hex(&options.environment_file, "deployment environment")?,
        validator_bindings,
        topology_sha256: sha256_file_hex(&options.topology_file, "deployment topology")?,
        swap_circuit_metadata_sha256: sha256_file_hex(
            &options.swap_circuit_metadata_file,
            "deployment swap circuit metadata",
        )?,
        private_egress_circuit_metadata_sha256: sha256_file_hex(
            &options.private_egress_circuit_metadata_file,
            "deployment private-egress circuit metadata",
        )?,
        publisher: publisher_key.publisher,
        algorithm_id: publisher_key.algorithm_id,
        public_key_hex: publisher_key.public_key_hex,
        signature_hex: String::new(),
    };
    let private_key =
        Zeroizing::new(hex_to_bytes(&publisher_key.private_key_hex).map_err(invalid_data)?);
    let payload = deployment_manifest_payload(&manifest)?;
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &payload,
        DEPLOYMENT_MANIFEST_SIGNATURE_CONTEXT,
    )
    .map_err(invalid_data)?;
    manifest.signature_hex = bytes_to_hex(&signature);
    let trusted_self = DeploymentPublisherPublicKey {
        schema: DEPLOYMENT_PUBLISHER_PUBLIC_KEY_SCHEMA.to_string(),
        publisher: manifest.publisher.clone(),
        algorithm_id: manifest.algorithm_id.clone(),
        public_key_hex: manifest.public_key_hex.clone(),
    };
    verify_deployment_manifest_record(&manifest, &trusted_self, unix_now())?;
    let json = serde_json::to_string_pretty(&manifest).map_err(invalid_data)?;
    write_public_deployment_artifact(&options.manifest_file, format!("{json}\n"))?;
    Ok(manifest)
}

pub fn verify_deployment_manifest(
    options: DeploymentManifestVerifyOptions,
) -> io::Result<DeploymentManifest> {
    let manifest: DeploymentManifest =
        read_json_file(&options.manifest_file, "deployment manifest")?;
    let trusted: DeploymentPublisherPublicKey = read_json_file(
        &options.trusted_publisher_key_file,
        "trusted deployment publisher key",
    )?;
    verify_deployment_manifest_record(
        &manifest,
        &trusted,
        options.now_unix.unwrap_or_else(unix_now),
    )?;
    match (&options.validator_id, &options.validator_bindings_file) {
        (None, None) => {}
        (Some(validator_id), Some(bindings_file)) => {
            validate_deployment_identifier(validator_id, "validator_id")?;
            let local_bindings = read_deployment_validator_bindings_file(bindings_file)?;
            let local = local_bindings
                .iter()
                .find(|binding| binding.validator_id == *validator_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "local deployment bindings do not contain configured validator",
                    )
                })?;
            let signed = manifest
                .validator_bindings
                .iter()
                .find(|binding| binding.validator_id == *validator_id)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "deployment manifest does not contain configured validator",
                    )
                })?;
            if local != signed {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "local deployment service artifacts do not match signed validator binding",
                ));
            }
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment validator ID and bindings file must be supplied together",
            ))
        }
    }
    match (
        &options.runtime_binary_file,
        &options.runtime_topology_file,
        &options.runtime_swap_circuit_metadata_file,
        &options.runtime_private_egress_circuit_metadata_file,
    ) {
        (None, None, None, None) => {}
        (Some(binary), Some(topology), Some(swap), Some(private_egress)) => {
            let actual = DeploymentRuntimeArtifactHashes {
                binary_sha256: sha256_file_hex(binary, "deployment runtime binary")?,
                topology_sha256: sha256_file_hex(topology, "deployment runtime topology")?,
                swap_circuit_metadata_sha256: sha256_file_hex(
                    swap,
                    "deployment runtime swap circuit metadata",
                )?,
                private_egress_circuit_metadata_sha256: sha256_file_hex(
                    private_egress,
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
                    "deployment runtime artifacts do not match signed manifest",
                ));
            }
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "deployment runtime artifact files must be supplied together",
            ))
        }
    }
    Ok(manifest)
}

const CONSENSUS_V2_ARTIFACT_SNAPSHOT_SCHEMA: &str = "postfiat.consensus_v2_artifact_snapshot.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConsensusV2ArtifactSnapshot {
    schema: String,
    directory: String,
    files: Vec<ConsensusV2ArtifactSnapshotFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ConsensusV2ArtifactSnapshotFile {
    name: String,
    bytes_hex: String,
    hash_hex: String,
}

fn consensus_v2_artifact_snapshot_bytes(data_dir: &Path, directory: &str) -> io::Result<Vec<u8>> {
    let source_dir = data_dir.join(directory);
    let mut paths = match std::fs::read_dir(&source_dir) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    paths.sort();
    let mut files = Vec::new();
    for path in paths {
        if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "consensus v2 artifact filename is not UTF-8",
                )
            })?
            .to_string();
        let bytes = std::fs::read(&path)?;
        if bytes.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("consensus v2 artifact `{}` is oversized", path.display()),
            ));
        }
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "consensus v2 artifact `{}` is invalid JSON: {error}",
                    path.display()
                ),
            )
        })?;
        files.push(ConsensusV2ArtifactSnapshotFile {
            name,
            bytes_hex: bytes_to_hex(&bytes),
            hash_hex: hash_hex("postfiat.consensus_v2.snapshot_artifact.v1", &bytes),
        });
    }
    snapshot_json_bytes(&ConsensusV2ArtifactSnapshot {
        schema: CONSENSUS_V2_ARTIFACT_SNAPSHOT_SCHEMA.to_string(),
        directory: directory.to_string(),
        files,
    })
}

fn restore_consensus_v2_artifact_snapshot(
    data_dir: &Path,
    snapshot_file: &str,
    expected_directory: &str,
) -> io::Result<()> {
    let path = data_dir.join(snapshot_file);
    let snapshot: ConsensusV2ArtifactSnapshot = read_json_file(&path, "consensus v2 snapshot")?;
    if snapshot.schema != CONSENSUS_V2_ARTIFACT_SNAPSHOT_SCHEMA
        || snapshot.directory != expected_directory
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "consensus v2 artifact snapshot schema or directory mismatch",
        ));
    }
    let target = data_dir.join(expected_directory);
    if target.exists() && std::fs::read_dir(&target)?.next().is_some() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "consensus v2 restore target `{}` is not empty",
                target.display()
            ),
        ));
    }
    std::fs::create_dir_all(&target)?;
    let mut prior_name = None::<String>;
    for file in snapshot.files {
        let candidate = Path::new(&file.name);
        if candidate.components().count() != 1
            || candidate.extension().and_then(|value| value.to_str()) != Some("json")
            || prior_name.as_ref().is_some_and(|prior| prior >= &file.name)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 snapshot artifact names are unsafe or noncanonical",
            ));
        }
        let bytes = hex_to_bytes(&file.bytes_hex).map_err(invalid_data)?;
        if bytes.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES
            || hash_hex("postfiat.consensus_v2.snapshot_artifact.v1", &bytes) != file.hash_hex
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 snapshot artifact hash or size mismatch",
            ));
        }
        serde_json::from_slice::<serde_json::Value>(&bytes).map_err(invalid_data)?;
        atomic_write(target.join(&file.name), bytes)?;
        prior_name = Some(file.name);
    }
    std::fs::remove_file(path)?;
    Ok(())
}

fn snapshot_file_bytes(store: &NodeStore, data_dir: &Path, file_name: &str) -> io::Result<Vec<u8>> {
    match file_name {
        BLOCKS_FILE => snapshot_json_bytes(&store.read_blocks()?),
        BATCH_ARCHIVE_FILE => snapshot_json_bytes(&store.read_batch_archive()?),
        ORDERED_BATCHES_FILE => snapshot_json_bytes(&store.read_ordered_batches()?),
        RECEIPTS_FILE => snapshot_json_bytes(&store.read_receipts()?),
        CONSENSUS_V2_SAFETY_SNAPSHOT_FILE => {
            consensus_v2_artifact_snapshot_bytes(data_dir, CONSENSUS_V2_SAFETY_DIR)
        }
        CONSENSUS_V2_QC_SNAPSHOT_FILE => {
            consensus_v2_artifact_snapshot_bytes(data_dir, CONSENSUS_V2_QC_DIR)
        }
        OWNED_LOCKS_FILE => snapshot_json_bytes(&load_owned_input_locks_for_snapshot(data_dir)?),
        OWNED_LOCKS_WAL_FILE => Ok(Vec::new()),
        FASTPAY_SPECULATIVE_JOURNAL_FILE => fastpay_speculative_journal_snapshot_bytes(data_dir),
        _ => {
            let source = data_dir.join(file_name);
            if !source.exists() {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!("snapshot source file `{}` is missing", source.display()),
                ));
            }
            std::fs::read(&source)
        }
    }
}

fn snapshot_json_bytes<T: Serialize + ?Sized>(value: &T) -> io::Result<Vec<u8>> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(format!("{json}\n").into_bytes())
}

pub fn import_snapshot(options: SnapshotImportOptions) -> io::Result<StatusReport> {
    let manifest = read_snapshot_manifest(&options.snapshot_dir.join(SNAPSHOT_MANIFEST_FILE))?;
    let data_dir = options.data_dir;
    if manifest.snapshot_version != SNAPSHOT_VERSION
        && manifest.snapshot_version != LEGACY_SNAPSHOT_VERSION
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported snapshot version {}, expected {LEGACY_SNAPSHOT_VERSION} or {SNAPSHOT_VERSION}",
                manifest.snapshot_version,
            ),
        ));
    }
    validate_snapshot_manifest_files(&manifest)?;

    if data_dir.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "snapshot import destination must not already exist: `{}`; import into a fresh path to prevent state overlay",
                data_dir.display()
            ),
        ));
    }
    if let Some(parent) = data_dir
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(&data_dir).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists {
            io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "snapshot import destination must not already exist: `{}`; import into a fresh path to prevent state overlay",
                    data_dir.display()
                ),
            )
        } else {
            error
        }
    })?;
    for file in &manifest.files {
        let source = options.snapshot_dir.join(&file.name);
        let bytes = std::fs::read(&source)?;
        let actual_hash = hash_hex("postfiat.snapshot.file.v1", &bytes);
        if actual_hash != file.hash_hex {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("snapshot file `{}` hash mismatch", file.name),
            ));
        }
        if bytes.len() as u64 != file.bytes {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("snapshot file `{}` byte length mismatch", file.name),
            ));
        }
        atomic_write(data_dir.join(&file.name), bytes)?;
    }

    if manifest.snapshot_version == SNAPSHOT_VERSION {
        restore_consensus_v2_artifact_snapshot(
            &data_dir,
            CONSENSUS_V2_SAFETY_SNAPSHOT_FILE,
            CONSENSUS_V2_SAFETY_DIR,
        )?;
        restore_consensus_v2_artifact_snapshot(
            &data_dir,
            CONSENSUS_V2_QC_SNAPSHOT_FILE,
            CONSENSUS_V2_QC_DIR,
        )?;
    } else {
        let legacy_store = NodeStore::new(&data_dir);
        let legacy_genesis = legacy_store.read_genesis()?;
        if legacy_genesis.consensus_v2_activation_height.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy snapshot cannot restore an activated consensus v2 signer without safety state",
            ));
        }
        let legacy_ledger = legacy_store.read_ledger()?;
        if legacy_ledger.fastpay_recovery_policy.is_some()
            || !legacy_ledger.fastpay_recovery_committees.is_empty()
            || !legacy_ledger.fastpay_recovery_reveals.is_empty()
            || !legacy_ledger.fastpay_version_fences.is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "legacy snapshot cannot restore activated FastPay recovery without lock and speculative-effect safety state",
            ));
        }
    }

    if let Some(node_id) = options.node_id {
        let store = NodeStore::new(&data_dir);
        let mut state = store.read_node_state()?;
        state.node_id = node_id;
        store.write_node_state(&state)?;
    }

    let restored = status(NodeOptions {
        data_dir: data_dir.clone(),
    })?;
    if restored.chain_id != manifest.chain_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored chain id {} does not match snapshot {}",
                restored.chain_id, manifest.chain_id
            ),
        ));
    }
    if restored.genesis_hash != manifest.genesis_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored genesis hash {} does not match snapshot {}",
                restored.genesis_hash, manifest.genesis_hash
            ),
        ));
    }
    if restored.protocol_version != manifest.protocol_version {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored protocol version {} does not match snapshot {}",
                restored.protocol_version, manifest.protocol_version
            ),
        ));
    }
    if restored.state_root != manifest.state_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored state root {} does not match snapshot {}",
                restored.state_root, manifest.state_root
            ),
        ));
    }
    if restored.block_height != manifest.block_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored block height {} does not match snapshot {}",
                restored.block_height, manifest.block_height
            ),
        ));
    }
    if restored.block_tip_hash != manifest.block_tip_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "restored block tip {} does not match snapshot {}",
                restored.block_tip_hash, manifest.block_tip_hash
            ),
        ));
    }
    verify_governance(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot import governance verification failed: {error}"),
        )
    })?;
    verify_bridge(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot import bridge verification failed: {error}"),
        )
    })?;
    verify_shielded(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot import shielded verification failed: {error}"),
        )
    })?;
    verify_mempool(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot import mempool verification failed: {error}"),
        )
    })?;
    verify_blocks(NodeOptions { data_dir }).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("snapshot import block verification failed: {error}"),
        )
    })?;
    Ok(restored)
}
