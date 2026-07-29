fn has_duplicate_strings<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = HashSet::new();
    for value in values {
        if !seen.insert(value) {
            return true;
        }
    }
    false
}

pub fn shield_spend(options: ShieldSpendOptions) -> io::Result<ShieldedSpendResult> {
    let _ = options;
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "legacy cleartext shield_spend is disabled; use an Asset-Orchard action",
    ))
}

pub fn create_shielded_mint_batch(
    options: ShieldMintBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let _ = options;
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "legacy cleartext shield_mint batches are disabled; use an Asset-Orchard ingress action",
    ))
}

pub fn create_shielded_spend_batch(
    options: ShieldSpendBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let _ = options;
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "legacy cleartext shield_spend batches are disabled; use an Asset-Orchard action",
    ))
}

pub fn create_shielded_migrate_batch(
    options: ShieldMigrateBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let actions = vec![ShieldedAction::Migrate(ShieldMigrateAction {
        note_id: options.note_id,
        target_pool: options.target_pool,
        memo: options.memo,
    })];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_orchard_action_batch(
    options: OrchardActionBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let action = read_orchard_action_file(&options.action_file)?;
    let domain = orchard_authorizing_domain(&genesis, &action.pool_id)?;
    verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let action_json = serde_json::to_string(&action).map_err(invalid_data)?;
    let actions = vec![ShieldedAction::OrchardV1(OrchardActionPayload {
        action_json,
    })];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_orchard_deposit_action_batch(
    options: OrchardDepositActionBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let file = read_orchard_deposit_action_file(&options.deposit_file)?;
    let domain = orchard_authorizing_domain(&genesis, &file.action.pool_id)?;
    let verified = verify_serialized_orchard_action_with_built_key(&file.action, &domain)
        .map_err(invalid_data)?;
    let payload = OrchardDepositActionPayload {
        action_json: serde_json::to_string(&file.action).map_err(invalid_data)?,
        funding_transfer: file.funding_transfer,
        amount: file.amount,
        fee: file.fee,
        policy_id: file.policy_id,
        disclosure_hash: file.disclosure_hash,
    };
    validate_orchard_deposit_payload(&payload)?;
    let funding_transfer_id = transfer_tx_id(&payload.funding_transfer);
    let expected_binding =
        orchard_deposit_external_binding_hash(OrchardDepositExternalBindingInput {
            genesis: &genesis,
            pool_id: &file.action.pool_id,
            funding_transfer_id: &funding_transfer_id,
            from_address: &payload.funding_transfer.unsigned.from,
            amount: payload.amount,
            fee: payload.fee,
            policy_id: &payload.policy_id,
            disclosure_hash: &payload.disclosure_hash,
        })?;
    if file.external_binding_hash != expected_binding
        || file.action.external_binding_hash.as_deref() != Some(expected_binding.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit action external binding does not match deposit payload",
        ));
    }
    if file.action.fee != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit action fee must be zero; deposit resource fee is charged by the funding envelope",
        ));
    }
    let deposit_value = orchard_turnstile_deposit_amount(verified.value_balance)?;
    if deposit_value != payload.amount {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard deposit action value {deposit_value} does not match payload amount {}",
                payload.amount
            ),
        ));
    }
    let minimum_fee = orchard_minimum_resource_fee_for_action(&file.action);
    if payload.fee < minimum_fee {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("minimum Orchard deposit fee is {minimum_fee}"),
        ));
    }
    let actions = vec![ShieldedAction::OrchardDepositV1(payload)];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_orchard_withdraw_action_batch(
    options: OrchardWithdrawActionBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    validate_orchard_withdraw_recipient(&options.to)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let action = read_orchard_action_file(&options.action_file)?;
    let domain = orchard_authorizing_domain(&genesis, &action.pool_id)?;
    verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let (policy_id, disclosure_hash) =
        orchard_withdraw_payload_values(options.policy_id, options.disclosure_hash)?;
    let payload = OrchardWithdrawActionPayload {
        action_json: serde_json::to_string(&action).map_err(invalid_data)?,
        to: options.to,
        amount: options.amount,
        fee: options.fee,
        policy_id,
        disclosure_hash,
    };
    validate_orchard_withdraw_payload(&payload)?;
    let expected_binding = orchard_withdraw_external_binding_hash(
        &genesis,
        &action.pool_id,
        &payload.to,
        payload.amount,
        payload.fee,
        &payload.policy_id,
        &payload.disclosure_hash,
    )?;
    if action.external_binding_hash.as_deref() != Some(expected_binding.as_str()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw action external binding does not match batch payload",
        ));
    }
    if action.fee != payload.fee {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw action fee does not match batch payload fee",
        ));
    }
    let actions = vec![ShieldedAction::OrchardWithdrawV1(payload)];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_shielded_swap_action_batch(
    options: ShieldedSwapActionBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let raw_swap_json = read_bounded_json_text_file(&options.swap_file, "shielded swap action")?;
    let swap_json = if let Ok(action) = serde_json::from_str::<AssetOrchardSwapAction>(&raw_swap_json) {
        let domain = orchard_authorizing_domain(&genesis, &action.pool_id)?;
        verify_serialized_asset_orchard_swap_action(&action, &domain).map_err(invalid_data)?;
        serde_json::to_string(&action).map_err(invalid_data)?
    } else {
        let action: ShieldedSwapAction =
            serde_json::from_str(&raw_swap_json).map_err(invalid_data)?;
        let domain = orchard_authorizing_domain(&genesis, &action.pool_id)?;
        verify_serialized_shielded_swap_action(&action, &domain).map_err(invalid_data)?;
        serde_json::to_string(&action).map_err(invalid_data)?
    };
    let actions = vec![ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
        swap_json,
    })];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_shielded_atomic_batch(
    options: ShieldedAtomicBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    if options.source_batch_files.is_empty() || options.source_batch_files.len() > 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shielded atomic batch requires between one and three source batches",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let execution_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let activation_height = governance
        .shielded_atomic_batch_activation_height()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "shielded atomic batch execution is not governed",
            )
        })?;
    if execution_height < activation_height {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "shielded atomic batch execution activates at height {activation_height}; next height is {execution_height}"
            ),
        ));
    }

    let mut actions = Vec::with_capacity(options.source_batch_files.len());
    for source_path in &options.source_batch_files {
        let source = read_shielded_action_batch_file(source_path)?;
        verify_shielded_action_batch_id(&genesis, &source)?;
        if source.atomic {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "nested shielded atomic batches are not allowed",
            ));
        }
        if source.actions.len() != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "each shielded atomic source batch must contain exactly one action",
            ));
        }
        actions.push(source.actions[0].clone());
    }
    let batch = build_atomic_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_verified_asset_orchard_swap_action_batch(
    data_dir: PathBuf,
    action: &AssetOrchardSwapAction,
    batch_file: PathBuf,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis()?;
    let swap_json = serde_json::to_string(action).map_err(invalid_data)?;
    let actions = vec![ShieldedAction::ShieldedSwapV1(ShieldedSwapActionPayload {
        swap_json,
    })];
    let batch = build_shielded_action_batch(&genesis, actions)?;
    write_shielded_action_batch_file(&batch_file, &batch)?;
    Ok(batch)
}

pub fn simulate_shielded_batch(
    options: ShieldedBatchSimulateOptions,
) -> io::Result<ShieldedBatchSimulationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let ordered_batches = store.read_ordered_batches()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let execution_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let batch = read_shielded_action_batch_file(&options.batch_file)?;
    verify_shielded_action_batch_id(&genesis, &batch)?;
    reject_live_legacy_cleartext_shielded_actions(&batch)?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("shielded batch `{}` already applied", batch.batch_id),
        ));
    }

    simulate_shielded_batch_against_state(
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        &batch,
    )
}

fn simulate_shielded_batch_against_state(
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
    execution_height: u64,
    batch: &ShieldedActionBatch,
) -> io::Result<ShieldedBatchSimulationReport> {
    verify_shielded_action_batch_id(genesis, batch)?;
    reject_live_legacy_cleartext_shielded_actions(batch)?;
    let pre_state_root = replicated_state_root(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
    )?;
    let mut post_ledger = ledger.clone();
    let mut post_shielded = shielded.clone();
    let receipts = execute_shielded_batch(
        genesis,
        &mut post_ledger,
        &mut post_shielded,
        batch,
        execution_height,
        asset_execution_compatibility_for_genesis_and_governance(genesis, governance),
        governance.orchard_pool_paused,
        governance.shielded_atomic_batch_activation_height(),
        false,
    );
    let all_accepted = receipts.iter().all(|receipt| receipt.accepted);
    let mut post_ordered_batches = ordered_batches.to_vec();
    if all_accepted {
        post_ordered_batches.push(batch.batch_id.clone());
    }
    let post_state_root = replicated_state_root(
        genesis,
        governance,
        &post_ledger,
        &post_ordered_batches,
        &post_shielded,
        bridge,
    )?;
    let state_changed = pre_state_root != post_state_root;
    Ok(ShieldedBatchSimulationReport {
        schema: "postfiat.shielded_batch_simulation.v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        batch_id: batch.batch_id.clone(),
        atomic: batch.atomic,
        execution_height,
        activation_height: governance.shielded_atomic_batch_activation_height(),
        action_count: batch.actions.len(),
        receipts,
        all_accepted,
        pre_state_root,
        post_state_root,
        state_changed,
        rejection_left_state_unchanged: !all_accepted && !state_changed,
    })
}

pub fn conformance_shielded_batch(
    options: ShieldedBatchConformanceOptions,
) -> io::Result<ShieldedBatchConformanceReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let ordered_batches = store.read_ordered_batches()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let execution_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let source = read_shielded_action_batch_file(&options.batch_file)?;
    verify_shielded_action_batch_id(&genesis, &source)?;
    reject_live_legacy_cleartext_shielded_actions(&source)?;
    if !source.atomic || source.actions.len() != 3 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shielded batch conformance requires one atomic three-action ingress/issue/redeem batch",
        ));
    }
    if ordered_batches.contains(&source.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("shielded batch `{}` already applied", source.batch_id),
        ));
    }
    let (ingress, issue, redeem) = match (
        &source.actions[0],
        &source.actions[1],
        &source.actions[2],
    ) {
        (
            ShieldedAction::AssetOrchardIngressV2(ingress),
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue),
            ShieldedAction::AssetOrchardPrivatePrimaryRedeemV1(redeem),
        ) if issue.route_id == redeem.route_id => (ingress, issue, redeem),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "shielded batch conformance source must be ingress v2 -> private issue -> private redeem for one route",
            ))
        }
    };
    let baseline = simulate_shielded_batch_against_state(
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        &source,
    )?;
    if !baseline.all_accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "shielded batch conformance baseline is not fully accepted",
        ));
    }

    let mut cases = Vec::new();
    let mut invalid_ingress = source.actions.clone();
    if let ShieldedAction::AssetOrchardIngressV2(action) = &mut invalid_ingress[0] {
        corrupt_hex_field(&mut action.burn_transaction.signature_hex)?;
    }
    push_atomic_rejection_case(
        &mut cases,
        "invalid_action_0_signature",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        invalid_ingress,
    )?;

    let mut invalid_issue = source.actions.clone();
    if let ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(action) = &mut invalid_issue[1] {
        corrupt_hex_field(&mut action.proof)?;
    }
    push_atomic_rejection_case(
        &mut cases,
        "invalid_action_1_proof",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        invalid_issue,
    )?;

    let mut invalid_redeem = source.actions.clone();
    if let ShieldedAction::AssetOrchardPrivatePrimaryRedeemV1(action) = &mut invalid_redeem[2] {
        corrupt_hex_field(&mut action.proof)?;
    }
    push_atomic_rejection_case(
        &mut cases,
        "invalid_action_2_proof",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        invalid_redeem,
    )?;

    push_atomic_rejection_case(
        &mut cases,
        "duplicate_nullifier",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        vec![
            ShieldedAction::AssetOrchardIngressV2(ingress.clone()),
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue.clone()),
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue.clone()),
        ],
    )?;
    push_atomic_rejection_case(
        &mut cases,
        "reordered_actions",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        vec![
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue.clone()),
            ShieldedAction::AssetOrchardIngressV2(ingress.clone()),
            ShieldedAction::AssetOrchardPrivatePrimaryRedeemV1(redeem.clone()),
        ],
    )?;
    push_atomic_rejection_case(
        &mut cases,
        "missing_batch_local_commitment",
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        vec![
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue.clone()),
            ShieldedAction::AssetOrchardPrivatePrimaryRedeemV1(redeem.clone()),
        ],
    )?;

    let ingress_batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::AssetOrchardIngressV2(ingress.clone())],
    )?;
    let mut post_ingress_ledger = ledger.clone();
    let mut post_ingress_shielded = shielded.clone();
    let ingress_receipts = execute_shielded_batch(
        &genesis,
        &mut post_ingress_ledger,
        &mut post_ingress_shielded,
        &ingress_batch,
        execution_height,
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance),
        governance.orchard_pool_paused,
        governance.shielded_atomic_batch_activation_height(),
        false,
    );
    if !ingress_receipts.iter().all(|receipt| receipt.accepted) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "stale-anchor conformance could not construct an accepted ingress pre-state",
        ));
    }
    make_orchard_anchor_stale(&mut post_ingress_shielded, &issue.anchor)?;
    let post_ingress_issue_redeem = build_atomic_shielded_action_batch(
        &genesis,
        vec![
            ShieldedAction::AssetOrchardPrivatePrimaryIssueV1(issue.clone()),
            ShieldedAction::AssetOrchardPrivatePrimaryRedeemV1(redeem.clone()),
        ],
    )?;
    push_existing_batch_rejection_case(
        &mut cases,
        "stale_anchor",
        &genesis,
        &governance,
        &post_ingress_ledger,
        &ordered_batches,
        &post_ingress_shielded,
        &bridge,
        execution_height,
        &post_ingress_issue_redeem,
    )?;

    let mut changed_policy_ledger = ledger.clone();
    let changed_policy_v2 =
        private_primary_route_v2_mut(&mut changed_policy_ledger, &issue.route_id)?;
    changed_policy_v2.primary_market_policy.policy_epoch = changed_policy_v2
        .primary_market_policy
        .policy_epoch
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "policy epoch overflow"))?;
    changed_policy_v2.primary_market_policy.policy_hash =
        changed_policy_v2.primary_market_policy.computed_hash();
    for reservation in changed_policy_v2.active_reservations.values_mut() {
        reservation.policy_epoch = changed_policy_v2.primary_market_policy.policy_epoch;
        reservation.policy_hash = changed_policy_v2.primary_market_policy.policy_hash.clone();
    }
    for entitlement in changed_policy_v2.export_entitlements.values_mut() {
        entitlement.policy_epoch = changed_policy_v2.primary_market_policy.policy_epoch;
        entitlement.policy_hash = changed_policy_v2.primary_market_policy.policy_hash.clone();
    }
    push_existing_batch_rejection_case(
        &mut cases,
        "changed_policy",
        &genesis,
        &governance,
        &changed_policy_ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        &source,
    )?;

    let mut exhausted_capacity_ledger = ledger.clone();
    let exhausted_v2 =
        private_primary_route_v2_mut(&mut exhausted_capacity_ledger, &issue.route_id)?;
    exhausted_v2.issue_capacity_used_atoms =
        exhausted_v2.primary_market_policy.issue_capacity_atoms;
    push_existing_batch_rejection_case(
        &mut cases,
        "issue_capacity_exhausted",
        &genesis,
        &governance,
        &exhausted_capacity_ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        &source,
    )?;

    let mut changed_nav_ledger = ledger.clone();
    let native_nav_asset_id = changed_nav_ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| route.route_id == issue.route_id)
        .map(|route| route.native_nav_asset_id.clone())
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "private-primary route missing"))?;
    let nav_asset = changed_nav_ledger
        .nav_assets
        .iter_mut()
        .find(|asset| asset.asset_id == native_nav_asset_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "private-primary pricing NAV asset missing",
            )
        })?;
    nav_asset.finalized_epoch = nav_asset
        .finalized_epoch
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "NAV epoch overflow"))?;
    push_existing_batch_rejection_case(
        &mut cases,
        "changed_nav",
        &genesis,
        &governance,
        &changed_nav_ledger,
        &ordered_batches,
        &shielded,
        &bridge,
        execution_height,
        &source,
    )?;

    let malformed_error = build_atomic_shielded_action_batch(
        &genesis,
        vec![
            source.actions[0].clone(),
            source.actions[1].clone(),
            source.actions[2].clone(),
            source.actions[2].clone(),
        ],
    )
    .expect_err("four-action atomic batch must be rejected");
    cases.push(admission_rejection_case(
        "malformed_action_bounds",
        malformed_error.to_string(),
        &baseline.pre_state_root,
    ));
    cases.push(admission_rejection_case(
        "replayed_batch_id",
        format!("shielded batch `{}` already applied", source.batch_id),
        &baseline.post_state_root,
    ));

    for (case, expected_code) in [
        (
            "invalid_action_0_signature",
            "asset_orchard_ingress_burn_rejected",
        ),
        (
            "invalid_action_1_proof",
            "asset_orchard_private_primary_issue_bad_proof",
        ),
        (
            "invalid_action_2_proof",
            "asset_orchard_private_primary_issue_bad_proof",
        ),
        ("duplicate_nullifier", "duplicate_nullifier"),
        ("reordered_actions", "unretained_orchard_anchor"),
        (
            "missing_batch_local_commitment",
            "unretained_orchard_anchor",
        ),
        ("stale_anchor", "unretained_orchard_anchor"),
        (
            "changed_policy",
            "pftl_uniswap_private_primary_policy_mismatch",
        ),
        (
            "issue_capacity_exhausted",
            "pftl_uniswap_issue_capacity_exceeded",
        ),
        ("changed_nav", "pftl_uniswap_pricing_binding_mismatch"),
    ] {
        require_conformance_receipt_code(&cases, case, expected_code)?;
    }
    let all_cases_rejected_without_state_change = cases
        .iter()
        .all(|case| !case.all_accepted && case.rollback_preserved);
    if !all_cases_rejected_without_state_change {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "one or more shielded atomic conformance cases did not reject without state change",
        ));
    }
    Ok(ShieldedBatchConformanceReport {
        schema: "postfiat.shielded_batch_conformance.v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        execution_height,
        activation_height: governance.shielded_atomic_batch_activation_height(),
        source_batch_id: source.batch_id,
        baseline,
        cases,
        all_cases_rejected_without_state_change,
    })
}

fn push_atomic_rejection_case(
    cases: &mut Vec<ShieldedBatchConformanceCaseReport>,
    case: &str,
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
    execution_height: u64,
    actions: Vec<ShieldedAction>,
) -> io::Result<()> {
    let batch = build_atomic_shielded_action_batch(genesis, actions)?;
    push_existing_batch_rejection_case(
        cases,
        case,
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        execution_height,
        &batch,
    )
}

#[allow(clippy::too_many_arguments)]
fn push_existing_batch_rejection_case(
    cases: &mut Vec<ShieldedBatchConformanceCaseReport>,
    case: &str,
    genesis: &Genesis,
    governance: &GovernanceState,
    ledger: &LedgerState,
    ordered_batches: &[String],
    shielded: &ShieldedState,
    bridge: &BridgeState,
    execution_height: u64,
    batch: &ShieldedActionBatch,
) -> io::Result<()> {
    let report = simulate_shielded_batch_against_state(
        genesis,
        governance,
        ledger,
        ordered_batches,
        shielded,
        bridge,
        execution_height,
        batch,
    )?;
    if report.all_accepted || !report.rejection_left_state_unchanged {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("conformance case `{case}` did not reject with full rollback"),
        ));
    }
    cases.push(ShieldedBatchConformanceCaseReport {
        case: case.to_string(),
        batch_id: Some(report.batch_id),
        admission_error: None,
        receipts: report.receipts,
        all_accepted: report.all_accepted,
        pre_state_root: report.pre_state_root,
        post_state_root: report.post_state_root,
        state_changed: report.state_changed,
        rollback_preserved: report.rejection_left_state_unchanged,
    });
    Ok(())
}

fn require_conformance_receipt_code(
    cases: &[ShieldedBatchConformanceCaseReport],
    case: &str,
    expected_code: &str,
) -> io::Result<()> {
    let report = cases
        .iter()
        .find(|report| report.case == case)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("conformance case `{case}` is missing"),
            )
        })?;
    if report
        .receipts
        .iter()
        .any(|receipt| receipt.code == expected_code)
    {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "conformance case `{case}` did not produce expected rejection code `{expected_code}`"
            ),
        ))
    }
}

fn admission_rejection_case(
    case: &str,
    error: String,
    state_root: &str,
) -> ShieldedBatchConformanceCaseReport {
    ShieldedBatchConformanceCaseReport {
        case: case.to_string(),
        batch_id: None,
        admission_error: Some(error),
        receipts: Vec::new(),
        all_accepted: false,
        pre_state_root: state_root.to_string(),
        post_state_root: state_root.to_string(),
        state_changed: false,
        rollback_preserved: true,
    }
}

fn corrupt_hex_field(value: &mut String) -> io::Result<()> {
    let first = value.as_bytes().first().copied().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "cannot corrupt an empty hex field")
    })?;
    if !first.is_ascii_hexdigit() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "cannot corrupt a non-hex field",
        ));
    }
    value.replace_range(..1, if first == b'0' { "1" } else { "0" });
    Ok(())
}

fn make_orchard_anchor_stale(shielded: &mut ShieldedState, stale_anchor: &str) -> io::Result<()> {
    let pool = shielded.orchard.as_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stale-anchor conformance requires an Orchard pool",
        )
    })?;
    let commitment = pool.output_commitments.first_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "stale-anchor conformance requires an existing Orchard commitment",
        )
    })?;
    let original = commitment.clone();
    corrupt_hex_field(commitment)?;
    let replacement = commitment.clone();
    let encrypted = pool
        .asset_orchard_outputs
        .iter_mut()
        .find(|output| output.output_commitment == original)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "stale-anchor conformance commitment has no encrypted output",
            )
    })?;
    encrypted.output_commitment = replacement;
    pool.root_history
        .retain(|record| record.root != stale_anchor);
    append_orchard_current_root(pool)
}

fn private_primary_route_v2_mut<'a>(
    ledger: &'a mut LedgerState,
    route_id: &str,
) -> io::Result<&'a mut PftlUniswapRouteV2State> {
    ledger
        .pftl_uniswap_routes
        .iter_mut()
        .find(|route| route.route_id == route_id)
        .and_then(|route| route.v2.as_mut())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "private-primary v2 route missing",
            )
        })
}

pub fn apply_shielded_batch(options: ApplyBatchOptions) -> io::Result<Vec<Receipt>> {
    apply_shielded_batch_with_replay(options, None)
}

pub fn apply_shielded_batch_with_replay(
    options: ApplyBatchOptions,
    replay_block_file: Option<PathBuf>,
) -> io::Result<Vec<Receipt>> {
    let store = NodeStore::new(&options.data_dir);
    let commit_lock = store.lock_ordered_commit()?;
    recover_ordered_commit_journal_locked(&store, &commit_lock)?;
    let genesis = store.read_genesis()?;
    let mut shielded = store.read_shielded()?;
    let mut governance = store.read_governance()?;
    let batch = read_shielded_action_batch_file(&options.batch_file)?;
    verify_shielded_action_batch_id(&genesis, &batch)?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("shielded batch `{}` already applied", batch.batch_id),
        ));
    }

    let mut ledger = store.read_ledger()?;
    let bridge = store.read_bridge()?;
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
    let receipts = execute_shielded_batch(
        &genesis,
        &mut ledger,
        &mut shielded,
        &batch,
        block_height,
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance),
        governance.orchard_pool_paused,
        governance.shielded_atomic_batch_activation_height(),
        historical_replay.is_some(),
    );
    ensure_atomic_shielded_batch_accepted(&batch, &receipts)?;
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
        batch_kind: BATCH_KIND_SHIELDED,
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
        batch_kind: "shielded",
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
            ledger: Some(ledger),
            governance: due_activations.governance_changed.then_some(governance),
            shielded: Some(shielded),
            bridge: None,
            commit,
            validator_registry: live_registry_update.or(due_activations.registry),
        },
    )?;
    Ok(receipts)
}

pub fn shield_scan(options: NodeOptions, owner: &str) -> io::Result<Vec<ShieldedNote>> {
    let store = NodeStore::new(options.data_dir);
    let shielded = store.read_shielded()?;
    Ok(scan_owner(&shielded, owner))
}

pub fn shield_disclose(options: NodeOptions, note_id: &str) -> io::Result<ShieldedDisclosure> {
    let store = NodeStore::new(options.data_dir);
    let shielded = store.read_shielded()?;
    disclose_note(&shielded, note_id).map_err(shielded_error)
}

pub fn shield_turnstile(options: NodeOptions) -> io::Result<TurnstileSummary> {
    let store = NodeStore::new(options.data_dir);
    let shielded = store.read_shielded()?;
    Ok(turnstile_summary(&shielded))
}

pub fn shielded_tree_root(options: NodeOptions) -> io::Result<String> {
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    chain_bound_shielded_tree_root(&genesis, &shielded)
}

pub fn verify_shielded(options: NodeOptions) -> io::Result<ShieldedVerificationReport> {
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    verify_shielded_state(&shielded)?;
    let tree_root = chain_bound_shielded_tree_root(&genesis, &shielded)?;
    let summary = turnstile_summary(&shielded);
    let spent_note_count = shielded
        .notes
        .iter()
        .filter(|note| shielded.is_nullified(&debug_nullifier(&note.note_id)))
        .count();
    let (
        orchard_pool_id,
        orchard_nullifier_count,
        orchard_output_count,
        orchard_anchor_count,
        orchard_root_count,
        orchard_latest_root,
        orchard_value_balance_total,
        orchard_turnstile_deposit_total,
        orchard_fee_burn_total,
        orchard_withdraw_total,
    ) = shielded
        .orchard
        .as_ref()
        .map(|pool| {
            (
                pool.pool_id.clone(),
                pool.nullifiers.len(),
                pool.output_commitments.len(),
                pool.accepted_anchors.len(),
                pool.root_history.len(),
                pool.root_history
                    .last()
                    .map(|record| record.root.clone())
                    .unwrap_or_default(),
                pool.value_balance_total,
                pool.turnstile_deposit_total,
                pool.fee_burn_total,
                pool.withdraw_total,
            )
        })
        .unwrap_or_else(|| (String::new(), 0, 0, 0, 0, String::new(), 0, 0, 0, 0));
    Ok(ShieldedVerificationReport {
        verified: true,
        note_count: shielded.notes.len(),
        nullifier_count: shielded.nullifiers.len(),
        turnstile_event_count: shielded.turnstile_events.len(),
        orchard_pool_id,
        orchard_nullifier_count,
        orchard_output_count,
        orchard_anchor_count,
        orchard_root_count,
        orchard_latest_root,
        orchard_value_balance_total,
        orchard_turnstile_deposit_total,
        orchard_fee_burn_total,
        orchard_withdraw_total,
        tree_root,
        bootstrap_deposit_total: summary.bootstrap_deposit_total,
        migration_total: summary.migration_total,
        orchard_deposit_total: summary.orchard_deposit_total,
        spent_note_count,
        live_note_count: shielded.notes.len().saturating_sub(spent_note_count),
        latest_turnstile_event_id: shielded
            .turnstile_events
            .last()
            .map(|event| event.event_id.clone())
            .unwrap_or_default(),
    })
}

fn chain_bound_shielded_tree_root(
    genesis: &Genesis,
    shielded: &ShieldedState,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        &shielded.notes,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.shielded.note_tree.v2", &encoded))
}

#[cfg(test)]
fn direct_shielded_mint_creator(genesis: &Genesis) -> String {
    format!(
        "direct-shielded-mint:{}:{}:{}",
        genesis.chain_id,
        genesis_hash(genesis),
        genesis.protocol_version
    )
}

fn direct_rejection_id<T: Serialize>(
    genesis: &Genesis,
    hash_domain: &str,
    seed: &T,
) -> io::Result<String> {
    direct_receipt_id(genesis, hash_domain, seed)
}

fn direct_bridge_domain_receipt_id(
    genesis: &Genesis,
    operation: &str,
    domain: &BridgeDomain,
) -> io::Result<String> {
    direct_receipt_id(
        genesis,
        "postfiat.bridge.direct_domain_receipt.v1",
        &(
            operation,
            domain.domain_id.as_str(),
            domain.name.as_str(),
            domain.source_chain.as_str(),
            domain.target_chain.as_str(),
            domain.bridge_id.as_str(),
            domain.door_account.as_str(),
            domain.inbound_cap,
            domain.outbound_cap,
            domain.inbound_used,
            domain.outbound_used,
            domain.paused,
        ),
    )
}

fn direct_receipt_id<T: Serialize>(
    genesis: &Genesis,
    hash_domain: &str,
    seed: &T,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        seed,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(hash_domain, &encoded))
}

fn bridge_direct_rejection_seed<'a>(
    request: &'a BridgeTransferRequest,
    code: &'a str,
) -> (
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    &'a str,
    u64,
    &'a str,
    u32,
    &'a str,
) {
    (
        &request.domain_id,
        &request.direction,
        &request.from,
        &request.to,
        &request.asset_id,
        request.amount,
        &request.witness_id,
        request.witness_epoch,
        code,
    )
}

fn verify_shielded_state(shielded: &ShieldedState) -> io::Result<()> {
    let mut notes_by_id = HashMap::<String, &ShieldedNote>::new();
    let mut commitments = HashSet::<String>::new();
    let mut valid_nullifiers = HashSet::<String>::new();
    for (index, note) in shielded.notes.iter().enumerate() {
        if note.note_id.trim().is_empty()
            || note.commitment.trim().is_empty()
            || note.rho.trim().is_empty()
            || note.owner.trim().is_empty()
            || note.asset_id.trim().is_empty()
            || note.created_by.trim().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded note at position {index} has incomplete metadata"),
            ));
        }
        if note.value == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded note `{}` has zero value", note.note_id),
            ));
        }
        if note.position != index as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "shielded note `{}` position mismatch: expected {}, got {}",
                    note.note_id, index, note.position
                ),
            ));
        }
        if notes_by_id.insert(note.note_id.clone(), note).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate shielded note id `{}`", note.note_id),
            ));
        }
        if !commitments.insert(note.commitment.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate shielded note commitment `{}`", note.commitment),
            ));
        }

        let expected_commitment = debug_note_commitment(
            &note.owner,
            &note.asset_id,
            note.value,
            &note.memo,
            note.position,
            &note.created_by,
        )
        .map_err(shielded_state_error)?;
        if note.commitment != expected_commitment {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded note `{}` commitment mismatch", note.note_id),
            ));
        }
        let expected_note_id = debug_note_id(&note.commitment);
        if note.note_id != expected_note_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded note `{}` id mismatch", note.note_id),
            ));
        }
        let expected_rho = debug_note_rho(&note.note_id);
        if note.rho != expected_rho {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded note `{}` rho mismatch", note.note_id),
            ));
        }
        valid_nullifiers.insert(debug_nullifier(&note.note_id));
    }

    let mut nullifiers = HashSet::<String>::new();
    for nullifier in &shielded.nullifiers {
        if nullifier.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "shielded nullifier is empty",
            ));
        }
        if !nullifiers.insert(nullifier.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate shielded nullifier `{nullifier}`"),
            ));
        }
        if !valid_nullifiers.contains(nullifier) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("shielded nullifier `{nullifier}` does not match a persisted note"),
            ));
        }
    }

    let mut event_ids = HashSet::<String>::new();
    let mut migration_keys = HashSet::<(String, String)>::new();
    for event in &shielded.turnstile_events {
        if event.event_id.trim().is_empty()
            || event.kind.trim().is_empty()
            || event.owner.trim().is_empty()
            || event.asset_id.trim().is_empty()
            || event.note_id.trim().is_empty()
            || event.source_pool.trim().is_empty()
            || event.target_pool.trim().is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "shielded turnstile event has incomplete metadata",
            ));
        }
        if event.amount == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "shielded turnstile event `{}` has zero amount",
                    event.event_id
                ),
            ));
        }
        if !event_ids.insert(event.event_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate shielded turnstile event `{}`", event.event_id),
            ));
        }

        match event.kind.as_str() {
            TURNSTILE_KIND_BOOTSTRAP_DEPOSIT => {
                let note = notes_by_id.get(&event.note_id).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded turnstile event `{}` references missing note `{}`",
                            event.event_id, event.note_id
                        ),
                    )
                })?;
                if event.owner != note.owner
                    || event.asset_id != note.asset_id
                    || event.amount != note.value
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded turnstile event `{}` does not match referenced note `{}`",
                            event.event_id, event.note_id
                        ),
                    ));
                }
                let expected_event_id = debug_turnstile_event_id(
                    &event.kind,
                    &event.owner,
                    &event.asset_id,
                    event.amount,
                    &event.note_id,
                    &event.source_pool,
                    &event.target_pool,
                )
                .map_err(shielded_state_error)?;
                if event.event_id != expected_event_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("shielded turnstile event `{}` id mismatch", event.event_id),
                    ));
                }
                if event.source_pool != TRANSPARENT_BOOTSTRAP_POOL_ID
                    || event.target_pool != DEBUG_SHIELDED_POOL_ID
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded bootstrap event `{}` has invalid pool route",
                            event.event_id
                        ),
                    ));
                }
            }
            TURNSTILE_KIND_POOL_MIGRATION => {
                let note = notes_by_id.get(&event.note_id).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded turnstile event `{}` references missing note `{}`",
                            event.event_id, event.note_id
                        ),
                    )
                })?;
                if event.owner != note.owner
                    || event.asset_id != note.asset_id
                    || event.amount != note.value
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded turnstile event `{}` does not match referenced note `{}`",
                            event.event_id, event.note_id
                        ),
                    ));
                }
                let expected_event_id = debug_turnstile_event_id(
                    &event.kind,
                    &event.owner,
                    &event.asset_id,
                    event.amount,
                    &event.note_id,
                    &event.source_pool,
                    &event.target_pool,
                )
                .map_err(shielded_state_error)?;
                if event.event_id != expected_event_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("shielded turnstile event `{}` id mismatch", event.event_id),
                    ));
                }
                if event.source_pool != DEBUG_SHIELDED_POOL_ID
                    || event.target_pool == DEBUG_SHIELDED_POOL_ID
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded migration event `{}` has invalid pool route",
                            event.event_id
                        ),
                    ));
                }
                let migration_nullifier = debug_nullifier(&event.note_id);
                if !shielded.is_nullified(&migration_nullifier) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "shielded migration event `{}` references note `{}` without nullifying it",
                            event.event_id, event.note_id
                        ),
                    ));
                }
                if !migration_keys.insert((event.note_id.clone(), event.target_pool.clone())) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "duplicate shielded migration for note `{}` into `{}`",
                            event.note_id, event.target_pool
                        ),
                    ));
                }
            }
            TURNSTILE_KIND_ORCHARD_DEPOSIT => {
                if event.source_pool != TRANSPARENT_BOOTSTRAP_POOL_ID
                    || event.target_pool == DEBUG_SHIELDED_POOL_ID
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "Orchard deposit event `{}` has invalid pool route",
                            event.event_id
                        ),
                    ));
                }
                validate_hex_string(
                    "Orchard deposit funding transfer id",
                    &event.note_id,
                    Some(96),
                )?;
                let expected_event_id = orchard_deposit_turnstile_event_id(event)?;
                if event.event_id != expected_event_id {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Orchard deposit event `{}` id mismatch", event.event_id),
                    ));
                }
                if !migration_keys.insert((event.note_id.clone(), event.target_pool.clone())) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "duplicate Orchard deposit funding transfer `{}` into `{}`",
                            event.note_id, event.target_pool
                        ),
                    ));
                }
            }
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("unknown shielded turnstile event kind `{other}`"),
                ));
            }
        }
    }

    if let Some(orchard) = &shielded.orchard {
        verify_orchard_pool_state(orchard)?;
        verify_asset_orchard_asset_balances(orchard)?;
        verify_orchard_turnstile_accounting(shielded, orchard)?;
    }
    note_tree_root(shielded).map_err(shielded_state_error)?;
    Ok(())
}

fn verify_orchard_pool_state(pool: &OrchardPoolState) -> io::Result<()> {
    if pool.pool_id.trim().is_empty() || pool.pool_id != pool.pool_id.trim() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard pool id is empty or non-canonical",
        ));
    }
    let mut nullifiers = HashSet::<String>::new();
    for nullifier in &pool.nullifiers {
        validate_lower_hex_field("Orchard nullifier", nullifier, ORCHARD_NULLIFIER_BYTES * 2)?;
        if !nullifiers.insert(nullifier.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate Orchard nullifier `{nullifier}`"),
            ));
        }
    }

    let mut output_commitments = HashSet::<String>::new();
    for commitment in &pool.output_commitments {
        validate_lower_hex_field(
            "Orchard output commitment",
            commitment,
            ORCHARD_COMMITMENT_BYTES * 2,
        )?;
        if !output_commitments.insert(commitment.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate Orchard output commitment `{commitment}`"),
            ));
        }
    }

    let recorded_output_count = pool
        .encrypted_outputs
        .len()
        .saturating_add(pool.asset_orchard_outputs.len());
    if recorded_output_count != pool.output_commitments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard encrypted output records do not cover output commitment count",
        ));
    }
    for output in &pool.encrypted_outputs {
        validate_lower_hex_field(
            "Orchard encrypted output cmx",
            &output.cmx,
            ORCHARD_COMMITMENT_BYTES * 2,
        )?;
        if !output_commitments.contains(&output.cmx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Orchard encrypted output references missing commitment `{}`",
                    output.cmx
                ),
            ));
        }
        validate_lower_hex_field("Orchard epk", &output.epk, ORCHARD_EPK_BYTES * 2)?;
        validate_lower_hex_field(
            "Orchard enc_ciphertext",
            &output.enc_ciphertext,
            ORCHARD_ENC_CIPHERTEXT_BYTES * 2,
        )?;
        validate_lower_hex_field(
            "Orchard out_ciphertext",
            &output.out_ciphertext,
            ORCHARD_OUT_CIPHERTEXT_BYTES * 2,
        )?;
        if let Some(compact) = &output.compact_ciphertext {
            validate_lower_hex_field(
                "Orchard compact_ciphertext",
                compact,
                ORCHARD_COMPACT_CIPHERTEXT_BYTES * 2,
            )?;
        }
    }

    let mut asset_orchard_record_outputs = HashSet::<String>::new();
    for record in &pool.asset_orchard_outputs {
        validate_lower_hex_field(
            "AssetOrchard output commitment",
            &record.output_commitment,
            ORCHARD_COMMITMENT_BYTES * 2,
        )?;
        validate_bounded_lower_hex_field(
            "AssetOrchard encrypted output",
            &record.encrypted_output,
            ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES * 2,
        )?;
        if !output_commitments.contains(&record.output_commitment) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "AssetOrchard encrypted output references missing commitment `{}`",
                    record.output_commitment
                ),
            ));
        }
        if !asset_orchard_record_outputs.insert(record.output_commitment.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate AssetOrchard encrypted output for commitment `{}`",
                    record.output_commitment
                ),
            ));
        }
    }

    let mut asset_record_outputs = HashSet::<String>::new();
    for record in &pool.asset_commitment_records {
        validate_lower_hex_field(
            "Orchard asset record output commitment",
            &record.output_commitment,
            ORCHARD_COMMITMENT_BYTES * 2,
        )?;
        validate_lower_hex_field(
            "Orchard asset commitment",
            &record.asset_commitment,
            SHIELDED_SWAP_COMMITMENT_BYTES * 2,
        )?;
        validate_lower_hex_field(
            "Orchard value commitment",
            &record.value_commitment,
            SHIELDED_SWAP_COMMITMENT_BYTES * 2,
        )?;
        if !output_commitments.contains(&record.output_commitment) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Orchard asset commitment record references missing output `{}`",
                    record.output_commitment
                ),
            ));
        }
        if !asset_record_outputs.insert(record.output_commitment.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate Orchard asset commitment record for output `{}`",
                    record.output_commitment
                ),
            ));
        }
    }

    let mut anchors = HashSet::<String>::new();
    for anchor in &pool.accepted_anchors {
        validate_lower_hex_field("Orchard accepted anchor", anchor, ORCHARD_ANCHOR_BYTES * 2)?;
        if !anchors.insert(anchor.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate Orchard accepted anchor `{anchor}`"),
            ));
        }
    }
    verify_orchard_root_history(pool)?;
    if !pool.root_history.is_empty() {
        for anchor in &pool.accepted_anchors {
            if !pool
                .root_history
                .iter()
                .any(|record| record.root == *anchor)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Orchard accepted anchor `{anchor}` is not in retained root history"),
                ));
            }
        }
    }
    Ok(())
}

fn verify_asset_orchard_asset_balances(pool: &OrchardPoolState) -> io::Result<()> {
    let mut assets = HashSet::<String>::new();
    let mut previous_asset_id: Option<&str> = None;
    for balance in &pool.asset_orchard_balances {
        validate_asset_orchard_accounting_asset_id(&balance.asset_id).map_err(|(_, message)| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("AssetOrchard balance asset id invalid: {message}"),
            )
        })?;
        if let Some(previous) = previous_asset_id {
            if previous >= balance.asset_id.as_str() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "AssetOrchard asset balances must be sorted by unique asset id",
                ));
            }
        }
        previous_asset_id = Some(&balance.asset_id);
        if !assets.insert(balance.asset_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate AssetOrchard asset balance for `{}`",
                    balance.asset_id
                ),
            ));
        }
        let expected_live = balance
            .ingress_total
            .checked_sub(balance.egress_total)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "AssetOrchard egress total {} exceeds ingress total {} for asset `{}`",
                        balance.egress_total, balance.ingress_total, balance.asset_id
                    ),
                )
            })?;
        if expected_live != balance.live_total {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "AssetOrchard asset `{}` live total {} does not match ingress {} minus egress {}",
                    balance.asset_id, balance.live_total, balance.ingress_total, balance.egress_total
                ),
            ));
        }
    }
    Ok(())
}

fn verify_orchard_turnstile_accounting(
    shielded: &ShieldedState,
    pool: &OrchardPoolState,
) -> io::Result<()> {
    let issued_value = orchard_pool_issued_value(pool)?;
    let accounted_value = issued_value
        .checked_add(pool.fee_burn_total)
        .and_then(|value| value.checked_add(pool.withdraw_total))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard accounted value overflow",
            )
        })?;
    if pool.turnstile_deposit_total != accounted_value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard turnstile deposit total {} does not match issued value {} plus fee burn {} plus withdraw {}",
                pool.turnstile_deposit_total, issued_value, pool.fee_burn_total, pool.withdraw_total
            ),
        ));
    }
    let budget_total = orchard_turnstile_budget_total(shielded, &pool.pool_id)?;
    if pool.turnstile_deposit_total > budget_total {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard turnstile deposit total {} exceeds turnstile budget total {}",
                pool.turnstile_deposit_total, budget_total
            ),
        ));
    }
    Ok(())
}

fn verify_orchard_root_history(pool: &OrchardPoolState) -> io::Result<()> {
    if pool.root_history.is_empty() {
        if pool.output_commitments.is_empty() {
            return Ok(());
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard root history is empty despite persisted output commitments",
        ));
    }

    let commitments = orchard_pool_commitments(pool)?;
    let mut roots = HashSet::<String>::new();
    let mut previous_output_count = 0_u64;
    let mut previous_prefix_len = 0_usize;
    let mut frontier_snapshot: Option<OrchardFrontierSnapshot> = None;
    for record in &pool.root_history {
        validate_lower_hex_field(
            "Orchard retained root",
            &record.root,
            ORCHARD_ANCHOR_BYTES * 2,
        )?;
        if !roots.insert(record.root.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate Orchard retained root `{}`", record.root),
            ));
        }
        if record.output_count < previous_output_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard root history output_count is not monotonic",
            ));
        }
        previous_output_count = record.output_count;

        let prefix_len = usize::try_from(record.output_count).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard root history output_count does not fit this platform",
            )
        })?;
        if prefix_len > commitments.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard root history output_count exceeds output commitment count",
            ));
        }
        let snapshot = orchard_frontier_snapshot_append_commitments(
            frontier_snapshot.as_ref(),
            &commitments[previous_prefix_len..prefix_len],
        )
        .map_err(invalid_data)?;
        if record.root != snapshot.root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Orchard retained root for output_count {} does not match commitments",
                    record.output_count
                ),
            ));
        }
        frontier_snapshot = Some(snapshot);
        previous_prefix_len = prefix_len;
    }

    let Some(latest) = pool.root_history.last() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard root history is empty",
        ));
    };
    if latest.output_count != pool.output_commitments.len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard latest root output_count does not match output commitment count",
        ));
    }
    Ok(())
}

fn validate_lower_hex_field(label: &str, value: &str, expected_len: usize) -> io::Result<()> {
    if value.len() != expected_len
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be {expected_len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn validate_bounded_lower_hex_field(label: &str, value: &str, max_len: usize) -> io::Result<()> {
    if value.is_empty()
        || value.len() > max_len
        || value.len() % 2 != 0
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_hexdigit() || byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} must be non-empty lowercase hex with even length up to {max_len}, got `{value}`"
            ),
        ));
    }
    Ok(())
}
