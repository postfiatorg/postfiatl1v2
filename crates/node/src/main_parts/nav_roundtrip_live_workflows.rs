#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct NavRoundtripBackgroundAuditRequest {
    schema: String,
    artifact_file: String,
    roundtrip_summary_file: String,
    data_dir: String,
    topology_file: String,
    timeout_ms: u64,
    final_height: u64,
    final_state_root: String,
    final_mempool_pending: u64,
    final_validator_state_source: String,
    certified_round_validator_states: Vec<NavRoundtripValidatorStateEvidence>,
    required_checks: Vec<String>,
    suggested_command: String,
}

fn nav_roundtrip_pftl_only(
    options: NavRoundtripPftlOnlyOptions,
) -> Result<NavRoundtripPftlOnlyReport, String> {
    let total_start = std::time::Instant::now();
    let mut stage_start = total_start;
    nav_roundtrip_reject_degraded_live_options(
        "PFTL-only NAV roundtrip",
        options.allow_peer_failures,
        options.defer_certified_sends,
    )?;
    if options.subscriber != options.owner {
        return Err(
            "PFTL-only mode currently requires --subscriber and --owner to be the same account"
                .to_string(),
        );
    }

    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create PFTL-only NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("pftl-only-summary.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing PFTL-only NAV roundtrip summary `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripPftlOnlyReport>(&raw).map_err(|error| {
            format!(
                "existing PFTL-only NAV roundtrip summary `{}` is invalid: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "PFTL-only NAV roundtrip summary `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let fleet_preflight = nav_roundtrip_live_fleet_preflight(
        &options.data_dir,
        &options.topology_file,
        &options.artifact_dir.join("fleet-preflight"),
        options.timeout_ms,
        options.resume || options.fast_demo_preflight,
        options.overwrite,
        options.fast_demo_preflight,
    )
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "fleet_preflight", &error);
        error
    })?;
    let fleet_preflight_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let primary_mint = nav_roundtrip_live_demo_primary_mint(NavRoundtripPrimaryMintOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        validator_key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        artifact_dir: options.artifact_dir.join("flow1-primary-mint"),
        deposit_relay_report_file: None,
        nav_asset_id: options.nav_asset_id.clone(),
        settlement_asset_id: options.settlement_asset_id.clone(),
        subscriber: options.subscriber.clone(),
        issuer_key_file: options.issuer_key_file.clone(),
        subscriber_key_file: Some(options.owner_key_file.clone()),
        settlement_receipt_id: options.settlement_receipt_id.clone(),
        settlement_supply_allocation_id: options.settlement_supply_allocation_id.clone(),
        consume_issued_settlement: true,
        settlement_amount_atoms: options.settlement_amount_atoms,
        mint_amount: options.mint_amount,
        nav_epoch: None,
        nav_reserve_packet_hash: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: false,
        batch_only: options.batch_only,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "primary_mint", &error);
        error
    })?;
    nav_roundtrip_require_certified_ops_ok(
        "PFTL-only primary mint",
        &primary_mint.certified_ops,
        &options.artifact_dir,
    )?;
    let primary_mint_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);
    let expected_money_in_vna_delta = i128::from(nav_roundtrip_vault_bridge_atoms_to_nav_value(
        primary_mint.settlement_amount_atoms,
        &primary_mint.nav_valuation_unit,
        &primary_mint.settlement_valuation_unit,
        primary_mint.settlement_asset_precision,
    )?);

    let nav_money_in = nav_roundtrip_live_demo_nav_checkpoint(NavRoundtripNavCheckpointOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        validator_key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        artifact_dir: options.artifact_dir.join("flow2-nav-money-in"),
        nav_asset_id: options.nav_asset_id.clone(),
        issuer_key_file: options.issuer_key_file.clone(),
        submitter_key_file: options.submitter_key_file.clone(),
        epoch: None,
        expected_vna_delta: Some(expected_money_in_vna_delta),
        reserve_packet_hash: None,
        attestor_root: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: false,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_money_in", &error);
        error
    })?;
    nav_roundtrip_require_nav_checkpoint_ok(
        "PFTL-only NAV money-in checkpoint",
        &nav_money_in,
        &options.artifact_dir,
    )?;
    let nav_money_in_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let nav_exit = nav_roundtrip_live_demo_nav_exit(NavRoundtripNavExitOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        validator_key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        artifact_dir: options.artifact_dir.join("flow3-nav-exit"),
        primary_mint_report_file: options
            .artifact_dir
            .join("flow1-primary-mint")
            .join("primary-mint.json"),
        nav_asset_id: options.nav_asset_id.clone(),
        settlement_asset_id: options.settlement_asset_id.clone(),
        owner: Some(options.owner.clone()),
        owner_key_file: options.owner_key_file.clone(),
        issuer_key_file: options.issuer_key_file.clone(),
        amount: Some(options.mint_amount),
        settlement_amount_atoms: None,
        settlement_receipt_hash: None,
        redemption_id: None,
        same_round_settlement: options.same_round_nav_exit,
        nav_epoch: None,
        nav_reserve_packet_hash: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: false,
        batch_only: options.batch_only,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_exit", &error);
        error
    })?;
    if nav_exit.same_round_settlement {
        nav_roundtrip_require_certified_ops_ok(
            "PFTL-only NAV exit redeem/settle",
            &nav_exit.redeem_certified_ops,
            &options.artifact_dir,
        )?;
    } else {
        nav_roundtrip_require_certified_ops_ok(
            "PFTL-only NAV exit redeem",
            &nav_exit.redeem_certified_ops,
            &options.artifact_dir,
        )?;
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            nav_roundtrip_require_certified_ops_ok(
                "PFTL-only NAV exit settle",
                settle_ops,
                &options.artifact_dir,
            )?;
        } else {
            let error =
                "PFTL-only NAV exit did not produce a settlement certified-ops report".to_string();
            nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_exit", &error);
            return Err(error);
        }
    }
    let nav_exit_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let expected_money_out_vna_delta = -expected_money_in_vna_delta;
    let nav_money_out = nav_roundtrip_live_demo_nav_checkpoint(NavRoundtripNavCheckpointOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        validator_key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        artifact_dir: options.artifact_dir.join("flow4-nav-money-out"),
        nav_asset_id: options.nav_asset_id.clone(),
        issuer_key_file: options.issuer_key_file.clone(),
        submitter_key_file: options.submitter_key_file.clone(),
        epoch: None,
        expected_vna_delta: Some(expected_money_out_vna_delta),
        reserve_packet_hash: None,
        attestor_root: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: false,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_money_out", &error);
        error
    })?;
    nav_roundtrip_require_nav_checkpoint_ok(
        "PFTL-only NAV money-out checkpoint",
        &nav_money_out,
        &options.artifact_dir,
    )?;
    let nav_money_out_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let bridge_out_resume_file = options.artifact_dir.join("bridge-out-resume.json");
    let destination_ref_arg = options
        .destination_ref
        .clone()
        .map(nav_roundtrip_normalize_destination_ref)
        .unwrap_or_else(|| "DESTINATION_REF".to_string());
    let suggested_command = format!(
        "postfiat-node nav-roundtrip-live-demo --burn-to-redeem-only --data-dir {} --topology {} --key-file {} --artifact-dir {} --nav-exit-report {} --pfusdc {} --owner {} --owner-key-file {} --destination-ref {} --amount-atoms {} --timeout-ms {} --send-retries {} --retry-backoff-ms {} --resume",
        options.data_dir.display(),
        options.topology_file.display(),
        options.validator_key_file.display(),
        options.artifact_dir.join("bridge-out").display(),
        options
            .artifact_dir
            .join("flow3-nav-exit")
            .join("nav-exit.json")
            .display(),
        &options.settlement_asset_id,
        &options.owner,
        options.owner_key_file.display(),
        &destination_ref_arg,
        nav_exit.settlement_amount_atoms,
        options.timeout_ms,
        options.send_retries,
        options.retry_backoff_ms
    );
    let bridge_out_resume = NavRoundtripPftlOnlyBridgeOutResumeReport {
        schema: NAV_ROUNDTRIP_PFTL_ONLY_BRIDGE_OUT_RESUME_SCHEMA.to_string(),
        artifact_file: bridge_out_resume_file.display().to_string(),
        run_class: NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY.to_string(),
        completion_status: NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED.to_string(),
        settlement_asset_id: options.settlement_asset_id.clone(),
        owner: options.owner.clone(),
        amount_atoms: nav_exit.settlement_amount_atoms,
        nav_exit_report_file: options
            .artifact_dir
            .join("flow3-nav-exit")
            .join("nav-exit.json")
            .display()
            .to_string(),
        destination_ref: options
            .destination_ref
            .clone()
            .map(nav_roundtrip_normalize_destination_ref),
        next_stage: "burn_to_redeem".to_string(),
        suggested_command,
    };
    write_json_file(&bridge_out_resume_file, &bridge_out_resume)?;

    let mut pftl_certified_rounds = Vec::new();
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "primary_mint",
        "batch",
        &primary_mint.certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_in",
        "reserve_submit",
        &nav_money_in.submit_certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_in",
        "epoch_finalize",
        &nav_money_in.finalize_certified_ops,
    ));
    if nav_exit.same_round_settlement {
        pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
            "nav_exit",
            "redeem_settle",
            &nav_exit.redeem_certified_ops,
        ));
    } else {
        pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
            "nav_exit",
            "redeem",
            &nav_exit.redeem_certified_ops,
        ));
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
                "nav_exit",
                "settle",
                settle_ops,
            ));
        }
    }
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_out",
        "reserve_submit",
        &nav_money_out.submit_certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_out",
        "epoch_finalize",
        &nav_money_out.finalize_certified_ops,
    ));
    let pftl_certified_round_count = pftl_certified_rounds.len();
    let pftl_certified_operation_count = pftl_certified_rounds
        .iter()
        .map(|round| round.operation_count)
        .sum::<usize>();

    let mut pftl_replay_equivalence_required_count = 0usize;
    let mut pftl_candidate_batch_classes = Vec::new();
    let mut pftl_live_round_compression_blockers = Vec::new();
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "primary_mint",
        "batch",
        &primary_mint.certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_in",
        "reserve_submit",
        &nav_money_in.submit_certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_in",
        "epoch_finalize",
        &nav_money_in.finalize_certified_ops,
    );
    if nav_exit.same_round_settlement {
        nav_roundtrip_collect_certified_ops_compression_gate(
            &mut pftl_replay_equivalence_required_count,
            &mut pftl_candidate_batch_classes,
            &mut pftl_live_round_compression_blockers,
            "nav_exit",
            "redeem_settle",
            &nav_exit.redeem_certified_ops,
        );
    } else {
        nav_roundtrip_collect_certified_ops_compression_gate(
            &mut pftl_replay_equivalence_required_count,
            &mut pftl_candidate_batch_classes,
            &mut pftl_live_round_compression_blockers,
            "nav_exit",
            "redeem",
            &nav_exit.redeem_certified_ops,
        );
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            nav_roundtrip_collect_certified_ops_compression_gate(
                &mut pftl_replay_equivalence_required_count,
                &mut pftl_candidate_batch_classes,
                &mut pftl_live_round_compression_blockers,
                "nav_exit",
                "settle",
                settle_ops,
            );
        }
    }
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_out",
        "reserve_submit",
        &nav_money_out.submit_certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_out",
        "epoch_finalize",
        &nav_money_out.finalize_certified_ops,
    );
    pftl_candidate_batch_classes.sort();
    pftl_candidate_batch_classes.dedup();
    let pftl_live_round_compression_ready = pftl_live_round_compression_blockers.is_empty();

    let final_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("PFTL-only NAV roundtrip final status failed: {error}"))?;
    let operator_local_state =
        nav_roundtrip_validator_state_from_status(&final_status, "operator_local_state");
    let certified_round_validator_states =
        nav_roundtrip_certified_round_validator_states(&nav_money_out.finalize_certified_ops)?;
    let final_audit_profile = nav_roundtrip_final_audit_profile(
        options.reuse_final_certified_state,
        options.background_audit,
    );
    let (public_validator_states, final_validator_states, final_validator_state_source) =
        nav_roundtrip_select_final_validator_states(
            &options.topology_file,
            &final_status,
            options.timeout_ms,
            &certified_round_validator_states,
            nav_roundtrip_should_reuse_final_certified_state(
                options.reuse_final_certified_state,
                options.background_audit,
            ),
        )?;
    let final_validator_consensus_ok =
        nav_roundtrip_validator_states_consensus_ok(&final_validator_states);

    let mut failure_reasons = Vec::new();
    if final_status.mempool_pending != 0 {
        failure_reasons.push(format!(
            "final mempool has {} pending transactions",
            final_status.mempool_pending
        ));
    }
    if !final_validator_consensus_ok {
        failure_reasons.push(format!(
            "final validator evidence from {final_validator_state_source} does not agree on height/root"
        ));
    }
    if let Some(first_validator_state) = final_validator_states.first() {
        if first_validator_state.block_height != final_status.block_height
            || first_validator_state.state_root != final_status.state_root
        {
            failure_reasons.push(format!(
                "final local status height/root {}/{} does not match {final_validator_state_source} evidence {}/{}",
                final_status.block_height,
                final_status.state_root,
                first_validator_state.block_height,
                first_validator_state.state_root
            ));
        }
        for certified_state in &certified_round_validator_states {
            if certified_state.block_height != first_validator_state.block_height
                || certified_state.state_root != first_validator_state.state_root
            {
                failure_reasons.push(format!(
                    "certified-round evidence from {} height/root {}/{} does not match {final_validator_state_source} evidence {}/{}",
                    certified_state.source,
                    certified_state.block_height,
                    certified_state.state_root,
                    first_validator_state.block_height,
                    first_validator_state.state_root
                ));
            }
        }
    } else {
        failure_reasons.push(format!(
            "no final validator evidence was collected from {final_validator_state_source}"
        ));
    }
    let final_summary_ok = failure_reasons.is_empty();
    let final_verification_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);
    let background_audit_request_file = if options.background_audit {
        Some(nav_roundtrip_write_background_audit_request(
            &options.artifact_dir,
            &artifact_file,
            &options.data_dir,
            &options.topology_file,
            options.timeout_ms,
            &final_status,
            &final_validator_state_source,
            &certified_round_validator_states,
        )?)
    } else {
        None
    };

    let report = NavRoundtripPftlOnlyReport {
        schema: NAV_ROUNDTRIP_PFTL_ONLY_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        data_dir: options.data_dir.display().to_string(),
        run_class: NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY.to_string(),
        completion_status: NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED.to_string(),
        custody_location: NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE.to_string(),
        timing_scope: nav_roundtrip_default_pftl_only_timing_scope(),
        protocol_clock_started_at_stage: nav_roundtrip_default_pftl_protocol_clock_start(),
        protocol_clock_stopped_at_stage: nav_roundtrip_default_pftl_protocol_clock_stop(),
        setup_or_recovery_work_included_in_total: options.resume,
        nav_asset_id: options.nav_asset_id,
        settlement_asset_id: options.settlement_asset_id,
        subscriber: options.subscriber,
        owner: options.owner,
        mint_amount: options.mint_amount,
        expected_money_in_vna_delta,
        expected_money_out_vna_delta,
        fleet_preflight: Some(fleet_preflight),
        primary_mint,
        nav_money_in,
        nav_exit,
        nav_money_out,
        bridge_out_resume,
        bridge_out_resume_file: bridge_out_resume_file.display().to_string(),
        final_height: final_status.block_height,
        final_state_root: final_status.state_root,
        final_mempool_pending: final_status.mempool_pending,
        operator_local_state: Some(operator_local_state),
        public_validator_states: public_validator_states.clone(),
        certified_round_validator_states,
        final_validator_state_source,
        final_validator_states,
        final_validator_consensus_ok,
        background_audit_enabled: options.background_audit,
        final_audit_profile,
        background_audit_request_file,
        final_summary_ok,
        failure_reasons,
        pftl_certified_round_count,
        pftl_certified_operation_count,
        pftl_certified_rounds,
        pftl_replay_equivalence_required_count,
        pftl_candidate_batch_classes,
        pftl_live_round_compression_ready,
        pftl_live_round_compression_blockers,
        timings_ms: NavRoundtripLiveDemoTimingsReport {
            total_ms: monotonic_elapsed_ms(total_start),
            readiness_preflight_ms: fleet_preflight_ms,
            protocol_clock_ms: primary_mint_ms
                + nav_money_in_ms
                + nav_exit_ms
                + nav_money_out_ms
            + final_verification_ms,
            fleet_preflight_ms,
            preflight_ms: 0.0,
            stakehub_session_ms: 0.0,
            stakehub_session_close_ms: 0.0,
            evm_deposit_ms: 0.0,
            deposit_relay_ms: 0.0,
            primary_mint_ms,
            nav_money_in_ms,
            nav_exit_ms,
            nav_money_out_ms,
            burn_to_redeem_ms: 0.0,
            withdrawal_signature_ms: 0.0,
            evm_withdrawal_ms: 0.0,
            pftl_settle_ms: 0.0,
            final_verification_ms,
        },
    };
    write_json_file(&artifact_file, &report)?;
    if !report.final_summary_ok {
        return Err(format!(
            "PFTL-only NAV roundtrip final summary failed: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}

fn nav_roundtrip_live_demo(
    options: NavRoundtripLiveDemoOptions,
) -> Result<NavRoundtripLiveDemoReport, String> {
    if options.signatures_file.is_some() && options.withdrawal_signer_key_file.is_some() {
        return Err("use only one of --signatures-file or --withdrawal-signer-key-file".to_string());
    }
    let total_start = std::time::Instant::now();
    let mut stage_start = total_start;
    nav_roundtrip_reject_degraded_live_options(
        "full NAV roundtrip",
        options.allow_peer_failures,
        options.defer_certified_sends,
    )?;
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("roundtrip-summary.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing NAV roundtrip summary `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripLiveDemoReport>(&raw).map_err(|error| {
            format!(
                "existing NAV roundtrip summary `{}` is invalid: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "NAV roundtrip summary `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let subscriber = options
        .subscriber
        .clone()
        .unwrap_or_else(|| options.pftl_recipient.clone());
    let owner = options.owner.clone().unwrap_or_else(|| subscriber.clone());
    let settlement_key_file = options
        .settlement_key_file
        .clone()
        .unwrap_or_else(|| options.issuer_key_file.clone());
    let destination_ref = nav_roundtrip_normalize_destination_ref(
        options.destination_ref.clone().unwrap_or_else(|| {
            format!(
                "evm-erc20:{}:{}",
                options.source_chain_id, options.stakehub_wallet
            )
        }),
    );

    let preflight_profile = if options.fast_demo_preflight {
        "fast_demo_precomputed_fleet_required"
    } else {
        "conservative_blocking"
    }
    .to_string();
    let fleet_preflight = nav_roundtrip_live_fleet_preflight(
        &options.data_dir,
        &options.topology_file,
        &options.artifact_dir.join("fleet-preflight"),
        options.timeout_ms,
        options.resume || options.fast_demo_preflight,
        options.overwrite,
        options.fast_demo_preflight,
    )
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "fleet_preflight", &error);
        error
    })?;
    let fleet_preflight_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let preflight = nav_roundtrip_live_demo_preflight(NavRoundtripPreflightOptions {
        data_dir: options.data_dir.clone(),
        artifact_dir: options.artifact_dir.join("flow0-preflight"),
        source_rpc_url: options.source_rpc_url.clone(),
        cast_binary: options.cast_binary.clone(),
        vault_address: options.vault_address.clone(),
        verifier_address: options.verifier_address.clone(),
        usdc_address: options.usdc_address.clone(),
        stakehub_wallet: options.stakehub_wallet.clone(),
        amount_atoms: options.amount_atoms,
        min_gas_wei: options.min_gas_wei,
        resume: options.resume,
        overwrite: options.overwrite,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(
            &options.artifact_dir,
            "preflight",
            &error,
        );
        error
    })?;
    if !preflight.preflight_ok {
        let error = format!(
            "NAV roundtrip preflight failed: {:?}",
            preflight.failure_reasons
        );
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "preflight", &error);
        return Err(error);
    }
    if options.require_warm_usdc_allowance {
        let allowance = preflight
            .usdc_allowance_atoms
            .as_deref()
            .and_then(|value| value.parse::<u128>().ok())
            .unwrap_or(0);
        if allowance < u128::from(options.amount_atoms) {
            let error = format!(
                "NAV roundtrip benchmark requires warm USDC allowance: allowance {} atoms, required {} atoms",
                allowance, options.amount_atoms
            );
            nav_roundtrip_write_failure_artifact(&options.artifact_dir, "preflight", &error);
            return Err(error);
        }
    }
    let preflight_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let evm_deposit_artifact_file = options
        .artifact_dir
        .join("flow1-evm-deposit")
        .join("evm-deposit.json");
    let evm_withdrawal_artifact_file = options
        .artifact_dir
        .join("flow8-evm-withdrawal")
        .join("evm-withdrawal.json");
    let should_open_warm_stakehub_session = !(options.resume
        && evm_deposit_artifact_file.is_file()
        && evm_withdrawal_artifact_file.is_file());
    let mut warm_stakehub_session = if should_open_warm_stakehub_session {
        Some(
            NavRoundtripStakeHubLaunchSessionGuard::open(
                &options.stakehub_home,
                &options.artifact_dir.join("stakehub-launch-session"),
                &options.session_id,
                options.source_chain_id,
                &options.stakehub_wallet,
                &options.usdc_address,
                &options.vault_address,
                &options.verifier_address,
                options.amount_atoms,
                options.agent_timeout_secs,
            )
            .map_err(|error| {
                nav_roundtrip_write_failure_artifact(
                    &options.artifact_dir,
                    "stakehub_launch_session",
                    &error,
                );
                error
            })?,
        )
    } else {
        None
    };
    let stakehub_session_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);
    let stakehub_launch_session_mode = if warm_stakehub_session.is_some() {
        "full_run_warm_session"
    } else {
        "resume_existing_evm_artifacts_no_session"
    }
    .to_string();
    let stakehub_launch_session_open_file = warm_stakehub_session
        .as_ref()
        .map(|session| session.open_file.display().to_string());
    let stakehub_launch_session_close_file = warm_stakehub_session
        .as_ref()
        .map(|session| session.close_file.display().to_string());
    let evm_stages_use_external_launch_session = warm_stakehub_session.is_some();

    let evm_deposit =
        nav_roundtrip_live_demo_evm_deposit(NavRoundtripEvmDepositOptions {
            artifact_dir: options.artifact_dir.join("flow1-evm-deposit"),
            source_rpc_url: options.source_rpc_url.clone(),
            cast_binary: options.cast_binary.clone(),
            stakehub_home: options.stakehub_home.clone(),
            source_chain_id: options.source_chain_id,
            vault_address: options.vault_address.clone(),
            usdc_address: options.usdc_address.clone(),
            stakehub_wallet: options.stakehub_wallet.clone(),
            pftl_recipient: options.pftl_recipient.clone(),
            amount_atoms: options.amount_atoms,
            nonce: options.nonce.clone(),
            session_id: options.session_id.clone(),
            resume: options.resume,
            overwrite: options.overwrite,
            agent_timeout_secs: options.agent_timeout_secs,
            launch_session_managed_externally: evm_stages_use_external_launch_session,
            require_warm_allowance: options.require_warm_usdc_allowance,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "evm_deposit",
                &error,
            );
            error
        })?;
    if !evm_deposit.delta_ok {
        let error = format!(
            "NAV roundtrip EVM deposit delta check failed: {:?}",
            evm_deposit.failure_reasons
        );
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "evm_deposit", &error);
        return Err(error);
    }
    let evm_deposit_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let deposit_relay =
        nav_roundtrip_live_demo_deposit_relay(NavRoundtripDepositRelayOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow2-deposit-relay"),
            evm_deposit_report_file: options
                .artifact_dir
                .join("flow1-evm-deposit")
                .join("evm-deposit.json"),
            source_rpc_url: options.source_rpc_url.clone(),
            cast_binary: options.cast_binary.clone(),
            vault_address: options.vault_address.clone(),
            token_address: options.usdc_address.clone(),
            asset_id: options.settlement_asset_id.clone(),
            policy_hash: options.policy_hash.clone(),
            proposer: options.proposer.clone(),
            attestor: options.attestor.clone(),
            finalizer: options.finalizer.clone(),
            claimer: options.claimer.clone(),
            proposer_key_file: options.proposer_key_file.clone(),
            attestor_key_file: options.attestor_key_file.clone(),
            finalizer_key_file: options.finalizer_key_file.clone(),
            claimer_key_file: options.claimer_key_file.clone(),
            receipt_operator_key_file: Some(options.issuer_key_file.clone()),
            claim_deposit: false,
            expires_at_height: options.expires_at_height,
            source_proof_kind: options.source_proof_kind.clone(),
            source_proof_hash: options.source_proof_hash.clone(),
            source_public_values_hash: options.source_public_values_hash.clone(),
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
            batch_only: options.batch_only,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "deposit_relay",
                &error,
            );
            error
        })?;
    nav_roundtrip_require_certified_ops_ok(
        "deposit relay",
        &deposit_relay.certified_ops,
        &options.artifact_dir,
    )?;
    let deposit_relay_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let primary_mint =
        nav_roundtrip_live_demo_primary_mint(NavRoundtripPrimaryMintOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow3-primary-mint"),
            deposit_relay_report_file: Some(
                options
                    .artifact_dir
                    .join("flow2-deposit-relay")
                    .join("deposit-relay.json"),
            ),
            nav_asset_id: options.nav_asset_id.clone(),
            settlement_asset_id: options.settlement_asset_id.clone(),
            subscriber: subscriber.clone(),
            issuer_key_file: options.issuer_key_file.clone(),
            subscriber_key_file: None,
            settlement_receipt_id: None,
            settlement_supply_allocation_id: None,
            consume_issued_settlement: false,
            settlement_amount_atoms: None,
            mint_amount: options.mint_amount,
            nav_epoch: None,
            nav_reserve_packet_hash: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
            batch_only: options.batch_only,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "primary_mint",
                &error,
            );
            error
        })?;
    nav_roundtrip_require_certified_ops_ok(
        "primary mint",
        &primary_mint.certified_ops,
        &options.artifact_dir,
    )?;
    let primary_mint_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);
    let expected_money_in_vna_delta = i128::from(nav_roundtrip_vault_bridge_atoms_to_nav_value(
        primary_mint.settlement_amount_atoms,
        &primary_mint.nav_valuation_unit,
        &primary_mint.settlement_valuation_unit,
        primary_mint.settlement_asset_precision,
    )?);

    let nav_money_in =
        nav_roundtrip_live_demo_nav_checkpoint(NavRoundtripNavCheckpointOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow4-nav-money-in"),
            nav_asset_id: options.nav_asset_id.clone(),
            issuer_key_file: options.issuer_key_file.clone(),
            submitter_key_file: options.submitter_key_file.clone(),
            epoch: None,
            expected_vna_delta: Some(expected_money_in_vna_delta),
            reserve_packet_hash: None,
            attestor_root: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "nav_money_in",
                &error,
            );
            error
        })?;
    nav_roundtrip_require_nav_checkpoint_ok(
        "NAV money-in checkpoint",
        &nav_money_in,
        &options.artifact_dir,
    )?;
    let nav_money_in_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let nav_exit = nav_roundtrip_live_demo_nav_exit(NavRoundtripNavExitOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        validator_key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        artifact_dir: options.artifact_dir.join("flow5-nav-exit"),
        primary_mint_report_file: options
            .artifact_dir
            .join("flow3-primary-mint")
            .join("primary-mint.json"),
        nav_asset_id: options.nav_asset_id.clone(),
        settlement_asset_id: options.settlement_asset_id.clone(),
        owner: Some(owner.clone()),
        owner_key_file: options.owner_key_file.clone(),
        issuer_key_file: options.issuer_key_file.clone(),
        amount: Some(options.mint_amount),
        settlement_amount_atoms: None,
        settlement_receipt_hash: None,
        redemption_id: None,
        same_round_settlement: options.same_round_nav_exit,
        nav_epoch: None,
        nav_reserve_packet_hash: None,
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        allow_existing_mempool: options.allow_existing_mempool,
        resume: options.resume,
        overwrite: options.overwrite,
        prepare_only: false,
        batch_only: options.batch_only,
    })
    .map_err(|error| {
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_exit", &error);
        error
    })?;
    if nav_exit.same_round_settlement {
        nav_roundtrip_require_certified_ops_ok(
            "NAV exit redeem/settle",
            &nav_exit.redeem_certified_ops,
            &options.artifact_dir,
        )?;
    } else {
        nav_roundtrip_require_certified_ops_ok(
            "NAV exit redeem",
            &nav_exit.redeem_certified_ops,
            &options.artifact_dir,
        )?;
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            nav_roundtrip_require_certified_ops_ok(
                "NAV exit settle",
                settle_ops,
                &options.artifact_dir,
            )?;
        } else {
            let error = "NAV exit did not produce a settlement certified-ops report".to_string();
            nav_roundtrip_write_failure_artifact(&options.artifact_dir, "nav_exit", &error);
            return Err(error);
        }
    }
    let nav_exit_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let expected_money_out_vna_delta = -expected_money_in_vna_delta;
    let nav_money_out =
        nav_roundtrip_live_demo_nav_checkpoint(NavRoundtripNavCheckpointOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow6-nav-money-out"),
            nav_asset_id: options.nav_asset_id.clone(),
            issuer_key_file: options.issuer_key_file.clone(),
            submitter_key_file: options.submitter_key_file.clone(),
            epoch: None,
            expected_vna_delta: Some(expected_money_out_vna_delta),
            reserve_packet_hash: None,
            attestor_root: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "nav_money_out",
                &error,
            );
            error
        })?;
    nav_roundtrip_require_nav_checkpoint_ok(
        "NAV money-out checkpoint",
        &nav_money_out,
        &options.artifact_dir,
    )?;
    let nav_money_out_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let burn_to_redeem =
        nav_roundtrip_live_demo_burn_to_redeem(NavRoundtripBurnToRedeemOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow7-burn-to-redeem"),
            nav_exit_report_file: options
                .artifact_dir
                .join("flow5-nav-exit")
                .join("nav-exit.json"),
            settlement_asset_id: options.settlement_asset_id.clone(),
            owner: Some(owner.clone()),
            owner_key_file: options.owner_key_file.clone(),
            amount_atoms: Some(nav_exit.settlement_amount_atoms),
            destination_ref,
            issuer: None,
            bucket_id: None,
            epoch: None,
            reserve_packet_hash: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
            batch_only: options.batch_only,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "burn_to_redeem",
                &error,
            );
            error
        })?;
    nav_roundtrip_require_certified_ops_ok(
        "burn-to-redeem",
        &burn_to_redeem.certified_ops,
        &options.artifact_dir,
    )?;
    let burn_to_redeem_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let signatures_file = match options.signatures_file.clone() {
        Some(path) => path,
        None => {
            let bundle_dir = options
                .artifact_dir
                .join("flow8-withdrawal-signature-request");
            let redemption_id = burn_to_redeem.redemption_id.clone().ok_or_else(|| {
                "burn-to-redeem report did not include a redemption id".to_string()
            })?;
            let signature_bundle = vault_bridge_withdrawal_signature_bundle(
                VaultBridgeWithdrawalSignatureBundleOptions {
                    plan_options: VaultBridgeWithdrawalPlanOptions {
                        data_dir: options.data_dir.clone(),
                        asset_id: options.settlement_asset_id.clone(),
                        redemption_id,
                        pftl_finalized_height: options.pftl_finalized_height,
                        evm_chain_id: Some(options.source_chain_id),
                        verifier_address: Some(options.verifier_address.clone()),
                        signatures_file: None,
                    },
                    bundle_dir: bundle_dir.clone(),
                    relay_bundle_dir: None,
                    overwrite: options.overwrite,
                },
            )
            .map_err(|error| {
                format!(
                    "failed to create withdrawal signature request bundle `{}`: {error}",
                    bundle_dir.display()
                )
            })?;
            let Some(signer_key_file) = options.withdrawal_signer_key_file.as_ref() else {
                let error = format!(
                    "NAV roundtrip needs verifier signatures before source-chain withdrawal; signature request written to `{}` and empty signatures file to `{}`. Fill that JSON array and rerun with --resume --signatures-file {}, or rerun with --resume --withdrawal-signer-key-file PATH",
                    signature_bundle.signature_request_file,
                    signature_bundle.signatures_file,
                    signature_bundle.signatures_file
                );
                nav_roundtrip_write_failure_artifact(
                    &options.artifact_dir,
                    "evm_withdrawal_signatures",
                    &error,
                );
                return Err(error);
            };
            nav_roundtrip_align_withdrawal_signature_request_with_live_abi(
                &options.cast_binary,
                &options.source_rpc_url,
                &options.vault_address,
                &options.verifier_address,
                &options.usdc_address,
                &options.stakehub_wallet,
                std::path::Path::new(&signature_bundle.plan_file),
                std::path::Path::new(&signature_bundle.signature_request_file),
            )
            .map_err(|error| {
                nav_roundtrip_write_failure_artifact(
                    &options.artifact_dir,
                    "evm_withdrawal_signatures",
                    &error,
                );
                error
            })?;
            let auto_signature = nav_roundtrip_auto_sign_withdrawal_bundle(
                std::path::Path::new(&signature_bundle.signature_request_file),
                std::path::Path::new(&signature_bundle.signatures_file),
                signer_key_file,
            )
            .map_err(|error| {
                nav_roundtrip_write_failure_artifact(
                    &options.artifact_dir,
                    "evm_withdrawal_signatures",
                    &error,
                );
                error
            })?;
            nav_roundtrip_require_verifier_signer(
                &options.cast_binary,
                &options.source_rpc_url,
                &options.verifier_address,
                &auto_signature.signer_address,
            )
            .map_err(|error| {
                nav_roundtrip_write_failure_artifact(
                    &options.artifact_dir,
                    "evm_withdrawal_signatures",
                    &error,
                );
                error
            })?;
            let auto_report_file = bundle_dir.join("auto-signature.json");
            write_json_file(&auto_report_file, &auto_signature)?;
            std::path::PathBuf::from(signature_bundle.signatures_file)
        }
    };
    let signatures = nav_roundtrip_read_evm_signatures(&signatures_file)?;
    if signatures.is_empty() {
        let error = format!(
            "withdrawal signatures file `{}` is empty",
            signatures_file.display()
        );
        nav_roundtrip_write_failure_artifact(
            &options.artifact_dir,
            "evm_withdrawal_signatures",
            &error,
        );
        return Err(error);
    }
    let withdrawal_signature_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let evm_withdrawal =
        nav_roundtrip_live_demo_evm_withdrawal(NavRoundtripEvmWithdrawalOptions {
            data_dir: options.data_dir.clone(),
            artifact_dir: options.artifact_dir.join("flow8-evm-withdrawal"),
            burn_to_redeem_report_file: options
                .artifact_dir
                .join("flow7-burn-to-redeem")
                .join("burn-to-redeem.json"),
            source_rpc_url: options.source_rpc_url.clone(),
            cast_binary: options.cast_binary.clone(),
            stakehub_home: options.stakehub_home.clone(),
            source_chain_id: options.source_chain_id,
            vault_address: options.vault_address.clone(),
            verifier_address: options.verifier_address.clone(),
            usdc_address: options.usdc_address.clone(),
            stakehub_wallet: options.stakehub_wallet.clone(),
            settlement_asset_id: options.settlement_asset_id.clone(),
            redemption_id: burn_to_redeem.redemption_id.clone(),
            pftl_finalized_height: options.pftl_finalized_height,
            signatures_file: Some(signatures_file),
            withdrawal_signer_key_file: None,
            session_id: options.session_id.clone(),
            challenge_wait_secs: options.challenge_wait_secs,
            resume: options.resume,
            overwrite: options.overwrite,
            agent_timeout_secs: options.agent_timeout_secs,
            launch_session_managed_externally: evm_stages_use_external_launch_session,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "evm_withdrawal",
                &error,
            );
            error
        })?;
    if !evm_withdrawal.delta_ok {
        let error = format!(
            "NAV roundtrip EVM withdrawal delta check failed: {:?}",
            evm_withdrawal.failure_reasons
        );
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "evm_withdrawal", &error);
        return Err(error);
    }
    let evm_withdrawal_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let pftl_settle =
        nav_roundtrip_live_demo_pftl_settle(NavRoundtripPftlSettleOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            validator_key_file: options.validator_key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone(),
            artifact_dir: options.artifact_dir.join("flow9-pftl-settle"),
            evm_withdrawal_report_file: options
                .artifact_dir
                .join("flow8-evm-withdrawal")
                .join("evm-withdrawal.json"),
            settlement_asset_id: options.settlement_asset_id.clone(),
            issuer_or_redemption_account: None,
            settlement_key_file,
            settlement_receipt_hash: None,
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            allow_existing_mempool: options.allow_existing_mempool,
            resume: options.resume,
            overwrite: options.overwrite,
            prepare_only: false,
            batch_only: options.batch_only,
        })
        .map_err(|error| {
            nav_roundtrip_write_failure_artifact(&options.artifact_dir, "pftl_settle", &error);
            error
        })?;
    nav_roundtrip_require_certified_ops_ok(
        "PFTL settle",
        &pftl_settle.certified_ops,
        &options.artifact_dir,
    )?;
    if pftl_settle.accounting_ok != Some(true) {
        let error = format!(
            "NAV roundtrip PFTL settle accounting check failed: {:?}",
            pftl_settle.failure_reasons
        );
        nav_roundtrip_write_failure_artifact(&options.artifact_dir, "pftl_settle", &error);
        return Err(error);
    }
    let pftl_settle_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);

    let mut pftl_certified_rounds = Vec::new();
    nav_roundtrip_push_certified_rounds(
        &mut pftl_certified_rounds,
        "deposit_relay",
        "aggregate",
        &deposit_relay.certified_ops_stages,
        &deposit_relay.certified_ops,
    );
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "primary_mint",
        "batch",
        &primary_mint.certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_in",
        "reserve_submit",
        &nav_money_in.submit_certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_in",
        "epoch_finalize",
        &nav_money_in.finalize_certified_ops,
    ));
    if nav_exit.same_round_settlement {
        pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
            "nav_exit",
            "redeem_settle",
            &nav_exit.redeem_certified_ops,
        ));
    } else {
        pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
            "nav_exit",
            "redeem",
            &nav_exit.redeem_certified_ops,
        ));
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
                "nav_exit",
                "settle",
                settle_ops,
            ));
        }
    }
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_out",
        "reserve_submit",
        &nav_money_out.submit_certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "nav_money_out",
        "epoch_finalize",
        &nav_money_out.finalize_certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "burn_to_redeem",
        "batch",
        &burn_to_redeem.certified_ops,
    ));
    pftl_certified_rounds.push(nav_roundtrip_certified_round_summary(
        "pftl_settle",
        "batch",
        &pftl_settle.certified_ops,
    ));
    let pftl_certified_round_count = pftl_certified_rounds.len();
    let pftl_certified_operation_count = pftl_certified_rounds
        .iter()
        .map(|round| round.operation_count)
        .sum::<usize>();
    let mut pftl_replay_equivalence_required_count = 0usize;
    let mut pftl_candidate_batch_classes = Vec::new();
    let mut pftl_live_round_compression_blockers = Vec::new();
    nav_roundtrip_collect_certified_ops_compression_gates(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "deposit_relay",
        "aggregate",
        &deposit_relay.certified_ops_stages,
        &deposit_relay.certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "primary_mint",
        "batch",
        &primary_mint.certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_in",
        "reserve_submit",
        &nav_money_in.submit_certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_in",
        "epoch_finalize",
        &nav_money_in.finalize_certified_ops,
    );
    if nav_exit.same_round_settlement {
        nav_roundtrip_collect_certified_ops_compression_gate(
            &mut pftl_replay_equivalence_required_count,
            &mut pftl_candidate_batch_classes,
            &mut pftl_live_round_compression_blockers,
            "nav_exit",
            "redeem_settle",
            &nav_exit.redeem_certified_ops,
        );
    } else {
        nav_roundtrip_collect_certified_ops_compression_gate(
            &mut pftl_replay_equivalence_required_count,
            &mut pftl_candidate_batch_classes,
            &mut pftl_live_round_compression_blockers,
            "nav_exit",
            "redeem",
            &nav_exit.redeem_certified_ops,
        );
        if let Some(settle_ops) = nav_exit.settle_certified_ops.as_ref() {
            nav_roundtrip_collect_certified_ops_compression_gate(
                &mut pftl_replay_equivalence_required_count,
                &mut pftl_candidate_batch_classes,
                &mut pftl_live_round_compression_blockers,
                "nav_exit",
                "settle",
                settle_ops,
            );
        }
    }
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_out",
        "reserve_submit",
        &nav_money_out.submit_certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "nav_money_out",
        "epoch_finalize",
        &nav_money_out.finalize_certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "burn_to_redeem",
        "batch",
        &burn_to_redeem.certified_ops,
    );
    nav_roundtrip_collect_certified_ops_compression_gate(
        &mut pftl_replay_equivalence_required_count,
        &mut pftl_candidate_batch_classes,
        &mut pftl_live_round_compression_blockers,
        "pftl_settle",
        "batch",
        &pftl_settle.certified_ops,
    );
    pftl_candidate_batch_classes.sort();
    pftl_candidate_batch_classes.dedup();
    let pftl_live_round_compression_ready = pftl_live_round_compression_blockers.is_empty();

    let final_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("NAV roundtrip final status failed: {error}"))?;
    let operator_local_state =
        nav_roundtrip_validator_state_from_status(&final_status, "operator_local_state");
    let certified_round_validator_states =
        nav_roundtrip_certified_round_validator_states(&pftl_settle.certified_ops)?;
    let final_audit_profile = nav_roundtrip_final_audit_profile(
        options.reuse_final_certified_state,
        options.background_audit,
    );
    let (public_validator_states, final_validator_states, final_validator_state_source) =
        nav_roundtrip_select_final_validator_states(
            &options.topology_file,
            &final_status,
            options.timeout_ms,
            &certified_round_validator_states,
            nav_roundtrip_should_reuse_final_certified_state(
                options.reuse_final_certified_state,
                options.background_audit,
            ),
        )?;
    let final_validator_consensus_ok =
        nav_roundtrip_validator_states_consensus_ok(&final_validator_states);

    let mut failure_reasons = Vec::new();
    if final_status.mempool_pending != 0 {
        failure_reasons.push(format!(
            "final mempool has {} pending transactions",
            final_status.mempool_pending
        ));
    }
    if !final_validator_consensus_ok {
        failure_reasons.push(format!(
            "final validator evidence from {final_validator_state_source} does not agree on height/root"
        ));
    }
    if let Some(first_validator_state) = final_validator_states.first() {
        if first_validator_state.block_height != final_status.block_height
            || first_validator_state.state_root != final_status.state_root
        {
            failure_reasons.push(format!(
                "final local status height/root {}/{} does not match {final_validator_state_source} evidence {}/{}",
                final_status.block_height,
                final_status.state_root,
                first_validator_state.block_height,
                first_validator_state.state_root
            ));
        }
        for certified_state in &certified_round_validator_states {
            if certified_state.block_height != first_validator_state.block_height
                || certified_state.state_root != first_validator_state.state_root
            {
                failure_reasons.push(format!(
                    "certified-round evidence from {} height/root {}/{} does not match {final_validator_state_source} evidence {}/{}",
                    certified_state.source,
                    certified_state.block_height,
                    certified_state.state_root,
                    first_validator_state.block_height,
                    first_validator_state.state_root
                ));
            }
        }
    } else {
        failure_reasons.push(format!(
            "no final validator evidence was collected from {final_validator_state_source}"
        ));
    }
    let final_summary_ok = failure_reasons.is_empty();
    let final_verification_ms = nav_roundtrip_checkpoint_elapsed_ms(&mut stage_start);
    let stakehub_session_close_ms = if let Some(session) = warm_stakehub_session.as_mut() {
        let close_start = std::time::Instant::now();
        session.close().map_err(|error| {
            nav_roundtrip_write_failure_artifact(
                &options.artifact_dir,
                "stakehub_launch_session_close",
                &error,
            );
            error
        })?;
        monotonic_elapsed_ms(close_start)
    } else {
        0.0
    };
    let background_audit_request_file = if options.background_audit {
        Some(nav_roundtrip_write_background_audit_request(
            &options.artifact_dir,
            &artifact_file,
            &options.data_dir,
            &options.topology_file,
            options.timeout_ms,
            &final_status,
            &final_validator_state_source,
            &certified_round_validator_states,
        )?)
    } else {
        None
    };

    let source_rpc_provider_class =
        nav_roundtrip_source_rpc_provider_class(&options.source_rpc_url);
    let report = NavRoundtripLiveDemoReport {
        schema: NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        data_dir: options.data_dir.display().to_string(),
        run_class: NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP.to_string(),
        completion_status: NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP.to_string(),
        custody_location: NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC.to_string(),
        timing_scope: nav_roundtrip_default_full_timing_scope(),
        protocol_clock_started_at_stage: nav_roundtrip_default_full_protocol_clock_start(),
        protocol_clock_stopped_at_stage: nav_roundtrip_default_full_protocol_clock_stop(),
        setup_or_recovery_work_included_in_total: options.resume,
        source_rpc_url: options.source_rpc_url,
        source_rpc_provider_class,
        source_chain_id: options.source_chain_id,
        bridge_class: preflight.bridge_class.clone(),
        vault_address: options.vault_address,
        verifier_address: options.verifier_address,
        usdc_address: options.usdc_address,
        stakehub_wallet: options.stakehub_wallet,
        nav_asset_id: options.nav_asset_id,
        settlement_asset_id: options.settlement_asset_id,
        pftl_recipient: options.pftl_recipient,
        subscriber,
        owner,
        amount_atoms: options.amount_atoms,
        mint_amount: options.mint_amount,
        expected_money_in_vna_delta,
        expected_money_out_vna_delta,
        preflight_profile,
        fleet_preflight: Some(fleet_preflight),
        preflight,
        evm_deposit,
        deposit_relay,
        primary_mint,
        nav_money_in,
        nav_exit,
        nav_money_out,
        burn_to_redeem,
        evm_withdrawal,
        pftl_settle,
        final_height: final_status.block_height,
        final_state_root: final_status.state_root,
        final_mempool_pending: final_status.mempool_pending,
        operator_local_state: Some(operator_local_state),
        public_validator_states: public_validator_states.clone(),
        certified_round_validator_states,
        final_validator_state_source,
        final_validator_states,
        final_validator_consensus_ok,
        background_audit_enabled: options.background_audit,
        final_audit_profile,
        background_audit_request_file,
        final_summary_ok,
        failure_reasons,
        pftl_certified_round_count,
        pftl_certified_operation_count,
        pftl_certified_rounds,
        pftl_replay_equivalence_required_count,
        pftl_candidate_batch_classes,
        pftl_live_round_compression_ready,
        pftl_live_round_compression_blockers,
        stakehub_launch_session_mode: Some(stakehub_launch_session_mode),
        stakehub_launch_session_open_file,
        stakehub_launch_session_close_file,
        timings_ms: NavRoundtripLiveDemoTimingsReport {
            total_ms: monotonic_elapsed_ms(total_start),
            readiness_preflight_ms: fleet_preflight_ms + preflight_ms + stakehub_session_ms,
            protocol_clock_ms: evm_deposit_ms
                + deposit_relay_ms
                + primary_mint_ms
                + nav_money_in_ms
                + nav_exit_ms
                + nav_money_out_ms
                + burn_to_redeem_ms
                + withdrawal_signature_ms
                + evm_withdrawal_ms
                + pftl_settle_ms
                + final_verification_ms,
            fleet_preflight_ms,
            preflight_ms,
            stakehub_session_ms,
            stakehub_session_close_ms,
            evm_deposit_ms,
            deposit_relay_ms,
            primary_mint_ms,
            nav_money_in_ms,
            nav_exit_ms,
            nav_money_out_ms,
            burn_to_redeem_ms,
            withdrawal_signature_ms,
            evm_withdrawal_ms,
            pftl_settle_ms,
            final_verification_ms,
        },
    };
    write_json_file(&artifact_file, &report)?;
    if !report.final_summary_ok {
        return Err(format!(
            "NAV roundtrip final summary failed: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}
