fn nav_roundtrip_dashboard_status(
    options: NavRoundtripDashboardStatusOptions,
) -> Result<NavRoundtripDashboardStatusReport, String> {
    let raw = std::fs::read_to_string(&options.summary_file).map_err(|error| {
        format!(
            "failed to read NAV roundtrip summary `{}`: {error}",
            options.summary_file.display()
        )
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| {
        format!(
            "failed to parse NAV roundtrip summary `{}`: {error}",
            options.summary_file.display()
        )
    })?;
    let source_schema = nav_roundtrip_json_string(&value, "schema")
        .ok_or_else(|| "NAV roundtrip summary is missing `schema`".to_string())?;
    let (default_run_class, default_completion_status, default_custody_location) =
        match source_schema.as_str() {
            NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA => (
                NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP,
                NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP,
                NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC,
            ),
            NAV_ROUNDTRIP_PFTL_ONLY_REPORT_SCHEMA => (
                NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY,
                NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED,
                NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE,
            ),
            _ => {
                return Err(format!(
                    "unsupported NAV roundtrip summary schema `{source_schema}`"
                ));
            }
        };
    let run_class = nav_roundtrip_json_string(&value, "run_class")
        .unwrap_or_else(|| default_run_class.to_string());
    let completion_status = nav_roundtrip_json_string(&value, "completion_status")
        .unwrap_or_else(|| default_completion_status.to_string());
    let custody_location = nav_roundtrip_json_string(&value, "custody_location")
        .unwrap_or_else(|| default_custody_location.to_string());
    let timing_scope = nav_roundtrip_json_string(&value, "timing_scope");
    let protocol_clock_started_at_stage =
        nav_roundtrip_json_string(&value, "protocol_clock_started_at_stage");
    let protocol_clock_stopped_at_stage =
        nav_roundtrip_json_string(&value, "protocol_clock_stopped_at_stage");
    let setup_or_recovery_work_included_in_total = value
        .get("setup_or_recovery_work_included_in_total")
        .and_then(serde_json::Value::as_bool);
    let mut timing_parse_error = None;
    let timings_ms = match value.get("timings_ms") {
        Some(raw_timings) => match serde_json::from_value::<NavRoundtripLiveDemoTimingsReport>(
            raw_timings.clone(),
        ) {
            Ok(report) => Some(report),
            Err(error) => {
                timing_parse_error = Some(error.to_string());
                None
            }
        },
        None => None,
    };
    let total_ms = timings_ms.as_ref().map(|timings| timings.total_ms);
    let readiness_preflight_ms = timings_ms
        .as_ref()
        .map(|timings| timings.readiness_preflight_ms);
    let protocol_clock_ms = timings_ms
        .as_ref()
        .map(|timings| timings.protocol_clock_ms);
    let source_rpc_provider_class = nav_roundtrip_json_string(&value, "source_rpc_provider_class");
    let bridge_class = nav_roundtrip_json_string(&value, "bridge_class");
    let background_audit_enabled = value
        .get("background_audit_enabled")
        .and_then(serde_json::Value::as_bool);
    let final_audit_profile = nav_roundtrip_json_string(&value, "final_audit_profile");
    let final_validator_state_source =
        nav_roundtrip_json_string(&value, "final_validator_state_source");
    let final_summary_ok = value
        .get("final_summary_ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let final_validator_consensus_ok = value
        .get("final_validator_consensus_ok")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let final_mempool_pending = value
        .get("final_mempool_pending")
        .and_then(serde_json::Value::as_u64);
    let nav_money_in_delta_ok = value
        .get("nav_money_in")
        .and_then(|stage| stage.get("delta_ok"))
        .and_then(serde_json::Value::as_bool);
    let nav_money_out_delta_ok = value
        .get("nav_money_out")
        .and_then(|stage| stage.get("delta_ok"))
        .and_then(serde_json::Value::as_bool);
    let bridge_out_resume_file = nav_roundtrip_json_string(&value, "bridge_out_resume_file")
        .or_else(|| {
            value
                .get("bridge_out_resume")
                .and_then(|resume| nav_roundtrip_json_string(resume, "artifact_file"))
        });
    let bridge_out_resume_command = value
        .get("bridge_out_resume")
        .and_then(|resume| nav_roundtrip_json_string(resume, "suggested_command"));

    let (expected_timing_scope, expected_protocol_clock_start, expected_protocol_clock_stop) =
        match source_schema.as_str() {
            NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA => (
                nav_roundtrip_default_full_timing_scope(),
                nav_roundtrip_default_full_protocol_clock_start(),
                nav_roundtrip_default_full_protocol_clock_stop(),
            ),
            NAV_ROUNDTRIP_PFTL_ONLY_REPORT_SCHEMA => (
                nav_roundtrip_default_pftl_only_timing_scope(),
                nav_roundtrip_default_pftl_protocol_clock_start(),
                nav_roundtrip_default_pftl_protocol_clock_stop(),
            ),
            _ => unreachable!("unsupported schema is rejected above"),
        };
    let mut timing_failure_reasons = Vec::new();
    if timing_scope.as_deref() != Some(expected_timing_scope.as_str()) {
        timing_failure_reasons.push(format!(
            "timing_scope is {:?}, expected `{}`",
            timing_scope, expected_timing_scope
        ));
    }
    if protocol_clock_started_at_stage.as_deref()
        != Some(expected_protocol_clock_start.as_str())
    {
        timing_failure_reasons.push(format!(
            "protocol_clock_started_at_stage is {:?}, expected `{}`",
            protocol_clock_started_at_stage, expected_protocol_clock_start
        ));
    }
    if protocol_clock_stopped_at_stage.as_deref() != Some(expected_protocol_clock_stop.as_str()) {
        timing_failure_reasons.push(format!(
            "protocol_clock_stopped_at_stage is {:?}, expected `{}`",
            protocol_clock_stopped_at_stage, expected_protocol_clock_stop
        ));
    }
    if setup_or_recovery_work_included_in_total.is_none() {
        timing_failure_reasons
            .push("setup_or_recovery_work_included_in_total is missing".to_string());
    }
    if let Some(error) = timing_parse_error {
        timing_failure_reasons.push(format!("timings_ms could not be parsed: {error}"));
    }
    match timings_ms.as_ref() {
        Some(timings) => {
            for (field, value) in nav_roundtrip_timing_field_values(timings) {
                if !value.is_finite() || value < 0.0 {
                    timing_failure_reasons.push(format!(
                        "timings_ms.{field} is not finite and non-negative: {value}"
                    ));
                }
            }
            if timings.total_ms <= 0.0 {
                timing_failure_reasons.push(format!(
                    "timings_ms.total_ms is not positive: {}",
                    timings.total_ms
                ));
            }
            if timings.readiness_preflight_ms + timings.protocol_clock_ms > timings.total_ms + 5.0
            {
                timing_failure_reasons.push(format!(
                    "readiness_preflight_ms + protocol_clock_ms exceeds total_ms: {} + {} > {}",
                    timings.readiness_preflight_ms,
                    timings.protocol_clock_ms,
                    timings.total_ms
                ));
            }
        }
        None => timing_failure_reasons.push("timings_ms is missing".to_string()),
    }
    let timing_boundary_ok = timing_failure_reasons.is_empty();
    let benchmark_clean_timing =
        timing_boundary_ok && setup_or_recovery_work_included_in_total == Some(false);

    let full_arbitrum_roundtrip_complete = final_summary_ok
        && run_class == NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP
        && completion_status == NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP
        && custody_location == NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC;
    let pftl_only_complete = final_summary_ok
        && run_class == NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY
        && completion_status == NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED
        && custody_location == NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE;
    let bridge_out_deferred = pftl_only_complete;

    let mut failure_reasons = Vec::new();
    if !final_summary_ok {
        failure_reasons.push("final_summary_ok is false or missing".to_string());
    }
    if !final_validator_consensus_ok {
        failure_reasons.push("final validator consensus is false or missing".to_string());
    }
    if final_mempool_pending != Some(0) {
        failure_reasons.push(format!(
            "final_mempool_pending is {:?}, expected Some(0)",
            final_mempool_pending
        ));
    }
    if nav_money_in_delta_ok != Some(true) {
        failure_reasons.push("NAV money-in delta is not proven true".to_string());
    }
    if nav_money_out_delta_ok != Some(true) {
        failure_reasons.push("NAV money-out delta is not proven true".to_string());
    }
    failure_reasons.extend(timing_failure_reasons);
    match run_class.as_str() {
        NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP => {
            if completion_status != NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP {
                failure_reasons.push(format!(
                    "full roundtrip completion_status `{completion_status}` is not `{}`",
                    NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP
                ));
            }
            if custody_location != NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC {
                failure_reasons.push(format!(
                    "full roundtrip custody_location `{custody_location}` is not `{}`",
                    NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC
                ));
            }
        }
        NAV_ROUNDTRIP_RUN_CLASS_PFTL_ONLY => {
            if completion_status != NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED {
                failure_reasons.push(format!(
                    "PFTL-only completion_status `{completion_status}` is not `{}`",
                    NAV_ROUNDTRIP_COMPLETION_PFTL_ONLY_BRIDGE_OUT_DEFERRED
                ));
            }
            if custody_location != NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE {
                failure_reasons.push(format!(
                    "PFTL-only custody_location `{custody_location}` is not `{}`",
                    NAV_ROUNDTRIP_CUSTODY_PFTL_SETTLEMENT_ASSET_BALANCE
                ));
            }
            if bridge_out_resume_file.is_none() || bridge_out_resume_command.is_none() {
                failure_reasons.push(
                    "PFTL-only status is missing bridge-out resume file or command".to_string(),
                );
            }
        }
        _ => failure_reasons.push(format!("unknown run_class `{run_class}`")),
    }

    let display_status = if full_arbitrum_roundtrip_complete {
        "full Arbitrum roundtrip complete"
    } else if pftl_only_complete {
        "PFTL-only complete; bridge-out deferred"
    } else {
        "incomplete or failed"
    }
    .to_string();

    let report = NavRoundtripDashboardStatusReport {
        schema: NAV_ROUNDTRIP_DASHBOARD_STATUS_SCHEMA.to_string(),
        summary_file: options.summary_file.display().to_string(),
        source_schema,
        source_artifact_file: nav_roundtrip_json_string(&value, "artifact_file"),
        run_class,
        completion_status,
        custody_location,
        timing_scope,
        protocol_clock_started_at_stage,
        protocol_clock_stopped_at_stage,
        setup_or_recovery_work_included_in_total,
        timing_boundary_ok,
        benchmark_clean_timing,
        timings_ms,
        total_ms,
        readiness_preflight_ms,
        protocol_clock_ms,
        source_rpc_provider_class,
        bridge_class,
        background_audit_enabled,
        final_audit_profile,
        final_validator_state_source,
        display_status,
        full_arbitrum_roundtrip_complete,
        pftl_only_complete,
        bridge_out_deferred,
        bridge_out_resume_file,
        bridge_out_resume_command,
        final_summary_ok,
        final_validator_consensus_ok,
        final_mempool_pending,
        final_height: value.get("final_height").and_then(serde_json::Value::as_u64),
        final_state_root: nav_roundtrip_json_string(&value, "final_state_root"),
        nav_money_in_delta_ok,
        nav_money_out_delta_ok,
        failure_reasons,
    };
    if let Some(report_file) = options.report_file.as_ref() {
        write_json_file(report_file, &report)?;
    }
    Ok(report)
}

fn nav_roundtrip_json_string(value: &serde_json::Value, field: &str) -> Option<String> {
    value
        .get(field)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn nav_roundtrip_benchmark_plan(
    options: NavRoundtripBenchmarkPlanOptions,
) -> Result<NavRoundtripBenchmarkPlanReport, String> {
    let phase = options.phase.trim().to_ascii_lowercase();
    let (default_max_median_ms, default_max_p90_ms, phase2_required) = match phase.as_str() {
        "phase1" => (Some(95_000.0), Some(105_000.0), false),
        "phase2" => (Some(75_000.0), None, true),
        "phase3" => (Some(55_000.0), None, false),
        _ => {
            return Err(
                "nav-roundtrip-benchmark-plan currently supports `phase1`, `phase2`, or `phase3`"
                    .to_string(),
            )
        }
    };
    if options.run_count == 0 {
        return Err("--run-count must be greater than zero".to_string());
    }
    if phase2_required && options.replay_corpus_file.is_none() && options.replay_corpus_dir.is_none()
    {
        return Err(
            "Phase 2 benchmark plans require --replay-corpus-file or --replay-corpus-dir"
                .to_string(),
        );
    }
    let supplied_required_candidate_classes =
        nav_roundtrip_normalize_required_candidate_classes(
            options.required_candidate_classes.clone(),
        )?;
    let required_candidate_classes = if phase2_required {
        nav_roundtrip_merge_candidate_classes(
            nav_roundtrip_phase2_default_candidate_classes(),
            supplied_required_candidate_classes,
        )
    } else {
        supplied_required_candidate_classes
    };
    let raw = std::fs::read_to_string(&options.base_args_file).map_err(|error| {
        format!(
            "failed to read NAV roundtrip benchmark base args `{}`: {error}",
            options.base_args_file.display()
        )
    })?;
    let base_args_value: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
        format!(
            "failed to parse NAV roundtrip benchmark base args `{}`: {error}",
            options.base_args_file.display()
        )
    })?;
    let base_args = nav_roundtrip_benchmark_plan_args_from_json(&base_args_value)?;
    let base_args = nav_roundtrip_benchmark_normalize_live_demo_args(base_args)?;
    nav_roundtrip_benchmark_validate_base_args(&base_args, &phase)?;

    let topology_file = flag_value(&base_args, "--topology")
        .ok_or("benchmark base args must include --topology")?
        .to_string();
    let data_dir = flag_value(&base_args, "--data-dir")
        .unwrap_or(DEFAULT_DATA_DIR)
        .to_string();
    let source_rpc_url = flag_value(&base_args, "--source-rpc-url")
        .ok_or("benchmark base args must include --source-rpc-url")?
        .to_string();
    let source_chain_id = flag_value(&base_args, "--source-chain-id")
        .unwrap_or("42161")
        .to_string();
    let vault_address = flag_value(&base_args, "--vault")
        .ok_or("benchmark base args must include --vault")?
        .to_string();
    let verifier_address = flag_value(&base_args, "--verifier")
        .ok_or("benchmark base args must include --verifier")?
        .to_string();
    let usdc_address = flag_value(&base_args, "--usdc")
        .ok_or("benchmark base args must include --usdc")?
        .to_string();
    let stakehub_wallet = flag_value(&base_args, "--stakehub-wallet")
        .ok_or("benchmark base args must include --stakehub-wallet")?
        .to_string();
    let amount_atoms = flag_value(&base_args, "--amount-atoms")
        .ok_or("benchmark base args must include --amount-atoms")?
        .parse::<u64>()
        .map_err(|_| "benchmark base args --amount-atoms must be a u64".to_string())?;
    let allowance_deposit_count = options
        .run_count
        .checked_add(1)
        .ok_or("benchmark run count overflow while sizing allowance setup")?;
    let allowance_deposit_count_u64 = u64::try_from(allowance_deposit_count)
        .map_err(|_| "benchmark run count does not fit in u64".to_string())?;
    let required_allowance_atoms = amount_atoms
        .checked_mul(allowance_deposit_count_u64)
        .ok_or("benchmark allowance setup amount overflowed u64")?;
    let timeout_ms = flag_value(&base_args, "--timeout-ms")
        .unwrap_or("5000")
        .to_string();
    let agent_timeout_secs = flag_value(&base_args, "--agent-timeout-secs")
        .unwrap_or("1200")
        .to_string();
    let max_median_ms = options.max_median_ms.or(default_max_median_ms);
    let max_p90_ms = options.max_p90_ms.or(default_max_p90_ms);
    let thresholds = NavRoundtripBenchmarkPlanVerifierThresholds {
        min_clean_runs: options.run_count,
        max_median_ms,
        max_p90_ms,
    };
    let mut required_flags = vec![
        "--fast-demo-preflight".to_string(),
        "--background-audit".to_string(),
        "--reuse-final-certified-state".to_string(),
        "--require-warm-usdc-allowance".to_string(),
    ];
    if phase2_required {
        required_flags.push("--same-round-nav-exit".to_string());
    }

    let allowance_setup_dir = options.benchmark_dir.join("allowance-setup");
    let allowance_setup_file = allowance_setup_dir.join("allowance-setup.json");
    let mut allowance_setup_command = vec![
        options.binary.clone(),
        "nav-roundtrip-live-demo".to_string(),
        "--warm-usdc-allowance-only".to_string(),
        "--artifact-dir".to_string(),
        allowance_setup_dir.display().to_string(),
        "--source-rpc-url".to_string(),
        source_rpc_url,
        "--source-chain-id".to_string(),
        source_chain_id,
        "--vault".to_string(),
        vault_address,
        "--verifier".to_string(),
        verifier_address,
        "--usdc".to_string(),
        usdc_address,
        "--stakehub-wallet".to_string(),
        stakehub_wallet,
        "--required-allowance-atoms".to_string(),
        required_allowance_atoms.to_string(),
        "--session-id".to_string(),
        nav_roundtrip_benchmark_run_session_id(
            flag_value(&base_args, "--session-id")
                .ok_or("benchmark base args must include --session-id")?,
            "allowance-setup",
        ),
        "--agent-timeout-secs".to_string(),
        agent_timeout_secs,
    ];
    if let Some(cast_binary) = flag_value(&base_args, "--cast-bin") {
        allowance_setup_command.extend(["--cast-bin".to_string(), cast_binary.to_string()]);
    }
    if let Some(stakehub_home) = flag_value(&base_args, "--stakehub-home") {
        allowance_setup_command.extend(["--stakehub-home".to_string(), stakehub_home.to_string()]);
    }
    if options.overwrite {
        allowance_setup_command.push("--overwrite".to_string());
    }

    let smoke_artifact_dir = nav_roundtrip_benchmark_smoke_artifact_dir(&options.benchmark_dir);
    let smoke_summary_file = smoke_artifact_dir.join("roundtrip-summary.json");
    let smoke_fleet_preflight_dir = smoke_artifact_dir.join("fleet-preflight");
    let mut smoke_fleet_command = vec![
        options.binary.clone(),
        "nav-roundtrip-live-demo".to_string(),
        "--fleet-preflight-only".to_string(),
        "--data-dir".to_string(),
        data_dir.clone(),
        "--topology".to_string(),
        topology_file.clone(),
        "--artifact-dir".to_string(),
        smoke_fleet_preflight_dir.display().to_string(),
        "--timeout-ms".to_string(),
        timeout_ms.clone(),
    ];
    if options.overwrite {
        smoke_fleet_command.push("--overwrite".to_string());
    }
    let mut smoke_run_args = nav_roundtrip_benchmark_strip_plan_managed_args(&base_args)?;
    smoke_run_args.extend([
        "--artifact-dir".to_string(),
        smoke_artifact_dir.display().to_string(),
        "--nonce".to_string(),
        nav_roundtrip_benchmark_increment_nonce(
            flag_value(&base_args, "--nonce").ok_or("benchmark base args must include --nonce")?,
            options.run_count,
        )?,
        "--session-id".to_string(),
        nav_roundtrip_benchmark_run_session_id(
            flag_value(&base_args, "--session-id")
                .ok_or("benchmark base args must include --session-id")?,
            "smoke",
        ),
    ]);
    for required_flag in &required_flags {
        if !smoke_run_args.iter().any(|arg| arg == required_flag) {
            smoke_run_args.push(required_flag.clone());
        }
    }
    if options.overwrite {
        smoke_run_args.push("--overwrite".to_string());
    }
    let mut smoke_run_command = vec![
        options.binary.clone(),
        "nav-roundtrip-live-demo".to_string(),
    ];
    smoke_run_command.extend(smoke_run_args);
    let smoke_run = NavRoundtripBenchmarkPlanRun {
        run_index: 0,
        run_label: "smoke".to_string(),
        artifact_dir: smoke_artifact_dir.display().to_string(),
        summary_file: smoke_summary_file.display().to_string(),
        fleet_preflight_command: nav_roundtrip_benchmark_plan_command(
            "smoke/fleet-preflight".to_string(),
            smoke_fleet_command,
            Some(smoke_fleet_preflight_dir.display().to_string()),
            None,
        ),
        run_command: nav_roundtrip_benchmark_plan_command(
            "smoke/timed-run".to_string(),
            smoke_run_command,
            Some(smoke_artifact_dir.display().to_string()),
            Some(smoke_summary_file.display().to_string()),
        ),
    };

    let mut smoke_verifier_command = vec![
        options.binary.clone(),
        "nav-roundtrip-benchmark-verify".to_string(),
        "--phase".to_string(),
        phase.clone(),
        "--summary".to_string(),
        smoke_summary_file.display().to_string(),
        "--min-clean-runs".to_string(),
        "1".to_string(),
        "--max-median-ms".to_string(),
        "600000".to_string(),
        "--max-p90-ms".to_string(),
        "600000".to_string(),
        "--strict".to_string(),
    ];
    if let Some(replay_corpus_file) = options.replay_corpus_file.as_ref() {
        smoke_verifier_command.extend([
            "--replay-corpus-file".to_string(),
            replay_corpus_file.display().to_string(),
        ]);
    }
    if let Some(replay_corpus_dir) = options.replay_corpus_dir.as_ref() {
        smoke_verifier_command.extend([
            "--replay-corpus-dir".to_string(),
            replay_corpus_dir.display().to_string(),
        ]);
    }
    if !required_candidate_classes.is_empty() {
        smoke_verifier_command.extend([
            "--require-candidate-classes".to_string(),
            required_candidate_classes.join(","),
        ]);
    }

    let mut runs = Vec::with_capacity(options.run_count);
    let width = options.run_count.to_string().len().max(2);
    for run_index in 1..=options.run_count {
        let run_label = format!("{}{:0width$}", options.run_prefix, run_index, width = width);
        let artifact_dir = options.benchmark_dir.join(&run_label);
        let summary_file = artifact_dir.join("roundtrip-summary.json");
        let fleet_preflight_dir = artifact_dir.join("fleet-preflight");

        let mut fleet_command = vec![
            options.binary.clone(),
            "nav-roundtrip-live-demo".to_string(),
            "--fleet-preflight-only".to_string(),
            "--data-dir".to_string(),
            data_dir.clone(),
            "--topology".to_string(),
            topology_file.clone(),
            "--artifact-dir".to_string(),
            fleet_preflight_dir.display().to_string(),
            "--timeout-ms".to_string(),
            timeout_ms.clone(),
        ];
        if options.overwrite {
            fleet_command.push("--overwrite".to_string());
        }

        let mut run_args = nav_roundtrip_benchmark_strip_plan_managed_args(&base_args)?;
        run_args.extend([
            "--artifact-dir".to_string(),
            artifact_dir.display().to_string(),
            "--nonce".to_string(),
            nav_roundtrip_benchmark_increment_nonce(
                flag_value(&base_args, "--nonce").ok_or("benchmark base args must include --nonce")?,
                run_index - 1,
            )?,
            "--session-id".to_string(),
            nav_roundtrip_benchmark_run_session_id(
                flag_value(&base_args, "--session-id")
                    .ok_or("benchmark base args must include --session-id")?,
                &run_label,
            ),
        ]);
        for required_flag in &required_flags {
            if !run_args.iter().any(|arg| arg == required_flag) {
                run_args.push(required_flag.clone());
            }
        }
        if options.overwrite {
            run_args.push("--overwrite".to_string());
        }
        let mut run_command = vec![
            options.binary.clone(),
            "nav-roundtrip-live-demo".to_string(),
        ];
        run_command.extend(run_args);

        runs.push(NavRoundtripBenchmarkPlanRun {
            run_index,
            run_label: run_label.clone(),
            artifact_dir: artifact_dir.display().to_string(),
            summary_file: summary_file.display().to_string(),
            fleet_preflight_command: nav_roundtrip_benchmark_plan_command(
                format!("{run_label}/fleet-preflight"),
                fleet_command,
                Some(fleet_preflight_dir.display().to_string()),
                None,
            ),
            run_command: nav_roundtrip_benchmark_plan_command(
                format!("{run_label}/timed-run"),
                run_command,
                Some(artifact_dir.display().to_string()),
                Some(summary_file.display().to_string()),
            ),
        });
    }

    let mut verifier_command = vec![
        options.binary.clone(),
        "nav-roundtrip-benchmark-verify".to_string(),
        "--phase".to_string(),
        phase.clone(),
        "--benchmark-dir".to_string(),
        options.benchmark_dir.display().to_string(),
        "--min-clean-runs".to_string(),
        options.run_count.to_string(),
        "--strict".to_string(),
    ];
    if let Some(max_median_ms) = max_median_ms {
        verifier_command.extend([
            "--max-median-ms".to_string(),
            nav_roundtrip_benchmark_float_arg(max_median_ms),
        ]);
    }
    if let Some(max_p90_ms) = max_p90_ms {
        verifier_command.extend([
            "--max-p90-ms".to_string(),
            nav_roundtrip_benchmark_float_arg(max_p90_ms),
        ]);
    }
    if let Some(replay_corpus_file) = options.replay_corpus_file.as_ref() {
        verifier_command.extend([
            "--replay-corpus-file".to_string(),
            replay_corpus_file.display().to_string(),
        ]);
    }
    if let Some(replay_corpus_dir) = options.replay_corpus_dir.as_ref() {
        verifier_command.extend([
            "--replay-corpus-dir".to_string(),
            replay_corpus_dir.display().to_string(),
        ]);
    }
    if !required_candidate_classes.is_empty() {
        verifier_command.extend([
            "--require-candidate-classes".to_string(),
            required_candidate_classes.join(","),
        ]);
    }
    let verifier_report_file = options
        .report_file
        .as_ref()
        .map(|path| path.with_file_name(format!("{phase}-benchmark-verify.json")));
    if let Some(report_file) = &verifier_report_file {
        verifier_command.extend([
            "--report".to_string(),
            report_file.display().to_string(),
        ]);
    }

    let report_file = options.report_file.as_ref().map(|path| path.display().to_string());
    let report = NavRoundtripBenchmarkPlanReport {
        schema: NAV_ROUNDTRIP_BENCHMARK_PLAN_SCHEMA.to_string(),
        phase: options.phase,
        base_args_file: options.base_args_file.display().to_string(),
        benchmark_dir: options.benchmark_dir.display().to_string(),
        run_count: options.run_count,
        run_prefix: options.run_prefix,
        binary: options.binary,
        required_flags,
        required_candidate_classes,
        replay_corpus_file: options
            .replay_corpus_file
            .as_ref()
            .map(|path| path.display().to_string()),
        replay_corpus_dir: options
            .replay_corpus_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        verifier_thresholds: thresholds,
        base_args,
        allowance_setup_command: nav_roundtrip_benchmark_plan_command(
            format!("{phase}/allowance-setup"),
            allowance_setup_command,
            Some(allowance_setup_dir.display().to_string()),
            Some(allowance_setup_file.display().to_string()),
        ),
        smoke_run,
        smoke_verifier_command: nav_roundtrip_benchmark_plan_command(
            format!("{phase}/smoke-verify"),
            smoke_verifier_command,
            Some(smoke_artifact_dir.display().to_string()),
            Some(smoke_summary_file.display().to_string()),
        ),
        runs,
        verifier_command: nav_roundtrip_benchmark_plan_command(
            format!("{phase}/verify"),
            verifier_command,
            Some(options.benchmark_dir.display().to_string()),
            verifier_report_file.map(|path| path.display().to_string()),
        ),
        notes: vec![
            "Run allowance_setup_command once before smoke/timed runs; it warms a bounded USDC allowance for one smoke plus the acceptance battery and is not part of the protocol clock.".to_string(),
            "Run smoke_run.fleet_preflight_command, then smoke_run.run_command, then smoke_verifier_command before starting the acceptance battery; smoke artifacts live outside benchmark_dir and are not included in median/p90.".to_string(),
            "Run every fleet_preflight_command before starting timed runs so --fast-demo-preflight reuses green validator evidence.".to_string(),
            "Run each timed-run command exactly once per artifact directory; do not reuse a nonce across live deposits.".to_string(),
            format!("The verifier command is the {} acceptance gate and must pass before citing a benchmark claim.", phase.to_ascii_uppercase()),
        ],
        report_file,
    };
    if let Some(report_file) = &options.report_file {
        write_json_file(report_file, &report)?;
    }
    Ok(report)
}

fn nav_roundtrip_benchmark_smoke_artifact_dir(
    benchmark_dir: &std::path::Path,
) -> std::path::PathBuf {
    let Some(name) = benchmark_dir.file_name().and_then(|value| value.to_str()) else {
        return benchmark_dir.join("smoke");
    };
    benchmark_dir.with_file_name(format!("{name}-smoke"))
}

fn nav_roundtrip_benchmark_base_args(
    options: NavRoundtripBenchmarkBaseArgsOptions,
) -> Result<NavRoundtripBenchmarkBaseArgsReport, String> {
    if options.output_file.exists() && !options.overwrite {
        return Err(format!(
            "benchmark base args output `{}` already exists; pass --overwrite",
            options.output_file.display()
        ));
    }
    let summary_raw = std::fs::read_to_string(&options.summary_file).map_err(|error| {
        format!(
            "failed to read NAV roundtrip summary `{}`: {error}",
            options.summary_file.display()
        )
    })?;
    let summary_value = serde_json::from_str::<serde_json::Value>(&summary_raw).map_err(|error| {
        format!(
            "failed to parse NAV roundtrip summary JSON `{}`: {error}",
            options.summary_file.display()
        )
    })?;
    let summary = serde_json::from_str::<NavRoundtripLiveDemoReport>(&summary_raw).map_err(
        |error| {
            format!(
                "failed to parse NAV roundtrip summary `{}`: {error}",
                options.summary_file.display()
            )
        },
    )?;
    let mut failure_reasons = Vec::new();
    if summary.run_class != NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP {
        failure_reasons.push(format!(
            "summary run_class `{}` is not `{}`",
            summary.run_class, NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP
        ));
    }
    if summary.completion_status != NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP {
        failure_reasons.push(format!(
            "summary completion_status `{}` is not `{}`",
            summary.completion_status, NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP
        ));
    }
    if !summary.final_summary_ok {
        failure_reasons.push(format!(
            "summary final_summary_ok=false: {:?}",
            summary.failure_reasons
        ));
    }
    if !summary.final_validator_consensus_ok {
        failure_reasons.push("summary final validator consensus is not proven".to_string());
    }
    let topology_file = match options.topology_file.as_ref() {
        Some(path) => path.display().to_string(),
        None => summary
            .fleet_preflight
            .as_ref()
            .map(|fleet| fleet.topology_file.clone())
            .ok_or("missing --topology and summary has no fleet_preflight.topology_file")?,
    };
    let policy_hash = nav_roundtrip_summary_required_string(
        &summary_value,
        &[
            "deposit_relay",
            "relay_bundle",
            "relay_bundle",
            "plan",
            "policy_hash",
        ],
    )?;
    let expires_at_height = nav_roundtrip_summary_required_u64(
        &summary_value,
        &[
            "deposit_relay",
            "relay_bundle",
            "relay_bundle",
            "plan",
            "propose_operation",
            "expires_at_height",
        ],
    )?;
    let bridge_proposer = nav_roundtrip_summary_required_string(
        &summary_value,
        &[
            "deposit_relay",
            "relay_bundle",
            "relay_bundle",
            "plan",
            "propose_operation",
            "proposer",
        ],
    )?;
    let bridge_finalizer = nav_roundtrip_summary_required_string(
        &summary_value,
        &[
            "deposit_relay",
            "relay_bundle",
            "relay_bundle",
            "plan",
            "finalize_operation",
            "finalizer",
        ],
    )?;
    let bridge_attestor = nav_roundtrip_summary_optional_string(
        &summary_value,
        &[
            "deposit_relay",
            "relay_bundle",
            "relay_bundle",
            "plan",
            "attest_operation",
            "attestor",
        ],
    );
    let destination_ref = options.destination_ref.unwrap_or_else(|| {
        format!(
            "evm-erc20:{}:{}",
            summary.source_chain_id, summary.stakehub_wallet
        )
    });
    let data_dir = options
        .data_dir
        .unwrap_or_else(|| std::path::PathBuf::from(&summary.data_dir));
    let mut args = vec![
        "nav-roundtrip-live-demo".to_string(),
        "--data-dir".to_string(),
        data_dir.display().to_string(),
        "--topology".to_string(),
        topology_file,
        "--key-file".to_string(),
        options.key_file.display().to_string(),
    ];
    if let Some(path) = options.proposal_key_file.as_ref() {
        args.extend([
            "--proposal-key-file".to_string(),
            path.display().to_string(),
        ]);
    }
    args.extend([
        "--artifact-dir".to_string(),
        "ARTIFACT_DIR_PLACEHOLDER_REPLACED_BY_BENCHMARK_PLAN".to_string(),
        "--source-rpc-url".to_string(),
        summary.source_rpc_url.clone(),
        "--source-chain-id".to_string(),
        summary.source_chain_id.to_string(),
        "--vault".to_string(),
        summary.vault_address.clone(),
        "--verifier".to_string(),
        summary.verifier_address.clone(),
        "--usdc".to_string(),
        summary.usdc_address.clone(),
        "--stakehub-wallet".to_string(),
        summary.stakehub_wallet.clone(),
        "--nav-asset".to_string(),
        summary.nav_asset_id.clone(),
        "--pfusdc".to_string(),
        summary.settlement_asset_id.clone(),
        "--policy-hash".to_string(),
        policy_hash,
        "--pftl-recipient".to_string(),
        summary.pftl_recipient.clone(),
        "--subscriber".to_string(),
        summary.subscriber.clone(),
        "--owner".to_string(),
        summary.owner.clone(),
        "--proposer".to_string(),
        bridge_proposer,
        "--finalizer".to_string(),
        bridge_finalizer,
        "--claimer".to_string(),
        summary.owner.clone(),
        "--proposer-key-file".to_string(),
        options.proposer_key_file.display().to_string(),
    ]);
    if let Some(attestor) = bridge_attestor {
        args.extend(["--attestor".to_string(), attestor]);
    }
    if let Some(path) = options.attestor_key_file.as_ref() {
        args.extend([
            "--attestor-key-file".to_string(),
            path.display().to_string(),
        ]);
    }
    args.extend([
        "--finalizer-key-file".to_string(),
        options.finalizer_key_file.display().to_string(),
        "--claimer-key-file".to_string(),
        options.claimer_key_file.display().to_string(),
        "--issuer-key-file".to_string(),
        options.issuer_key_file.display().to_string(),
        "--owner-key-file".to_string(),
        options.owner_key_file.display().to_string(),
    ]);
    if let Some(path) = options.settlement_key_file.as_ref() {
        args.extend([
            "--settlement-key-file".to_string(),
            path.display().to_string(),
        ]);
    }
    if let Some(path) = options.submitter_key_file.as_ref() {
        args.extend([
            "--submitter-key-file".to_string(),
            path.display().to_string(),
        ]);
    }
    args.extend([
        "--amount-atoms".to_string(),
        summary.amount_atoms.to_string(),
        "--mint-amount".to_string(),
        summary.mint_amount.to_string(),
        "--nonce".to_string(),
        options.nonce_base,
        "--session-id".to_string(),
        options.session_id_base,
        "--withdrawal-signer-key-file".to_string(),
        options.withdrawal_signer_key_file.display().to_string(),
        "--destination-ref".to_string(),
        destination_ref,
        "--expires-at-height".to_string(),
        expires_at_height.to_string(),
        "--fast-demo-preflight".to_string(),
        "--background-audit".to_string(),
        "--reuse-final-certified-state".to_string(),
        "--require-warm-usdc-allowance".to_string(),
    ]);
    if let Some(timeout_ms) = options.timeout_ms {
        args.extend(["--timeout-ms".to_string(), timeout_ms.to_string()]);
    }
    if let Some(send_retries) = options.send_retries {
        args.extend(["--send-retries".to_string(), send_retries.to_string()]);
    }
    if let Some(retry_backoff_ms) = options.retry_backoff_ms {
        args.extend([
            "--retry-backoff-ms".to_string(),
            retry_backoff_ms.to_string(),
        ]);
    }
    if let Some(agent_timeout_secs) = options.agent_timeout_secs {
        args.extend([
            "--agent-timeout-secs".to_string(),
            agent_timeout_secs.to_string(),
        ]);
    }
    if let Some(min_gas_wei) = options.min_gas_wei.as_ref() {
        args.extend(["--min-gas-wei".to_string(), min_gas_wei.clone()]);
    }
    match nav_roundtrip_benchmark_validate_base_args(&args, "phase1") {
        Ok(()) => {}
        Err(error) => failure_reasons.push(error),
    }
    let output_value = serde_json::json!({
        "schema": NAV_ROUNDTRIP_BENCHMARK_BASE_ARGS_SCHEMA,
        "source_summary_file": options.summary_file.display().to_string(),
        "args": args,
    });
    write_json_file(&options.output_file, &output_value)?;
    let report = NavRoundtripBenchmarkBaseArgsReport {
        schema: NAV_ROUNDTRIP_BENCHMARK_BASE_ARGS_SCHEMA.to_string(),
        summary_file: options.summary_file.display().to_string(),
        output_file: options.output_file.display().to_string(),
        args: output_value
            .get("args")
            .and_then(serde_json::Value::as_array)
            .expect("args were just written as array")
            .iter()
            .map(|arg| {
                arg.as_str()
                    .expect("args were just written as strings")
                    .to_string()
            })
            .collect(),
        validation_ok: failure_reasons.is_empty(),
        failure_reasons,
    };
    if let Some(report_file) = options.report_file.as_ref() {
        write_json_file(report_file, &report)?;
    }
    if !report.validation_ok {
        return Err(format!(
            "generated NAV roundtrip benchmark base args failed validation: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}

fn nav_roundtrip_benchmark_plan_args_from_json(
    value: &serde_json::Value,
) -> Result<Vec<String>, String> {
    let args_value = value
        .get("args")
        .or_else(|| value.get("arguments"))
        .unwrap_or(value);
    let args = args_value
        .as_array()
        .ok_or("benchmark base args must be a JSON array or an object with `args`")?;
    args.iter()
        .map(|value| {
            value
                .as_str()
                .map(str::to_string)
                .ok_or("benchmark base args entries must all be strings".to_string())
        })
        .collect()
}

fn nav_roundtrip_summary_required_string(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<String, String> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment).ok_or_else(|| {
            format!(
                "summary is missing required field `{}`",
                path.join(".")
            )
        })?;
    }
    cursor
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "summary field `{}` must be a non-empty string",
                path.join(".")
            )
        })
}

fn nav_roundtrip_summary_optional_string(
    value: &serde_json::Value,
    path: &[&str],
) -> Option<String> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment)?;
    }
    cursor
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(str::to_string)
}

fn nav_roundtrip_summary_required_u64(
    value: &serde_json::Value,
    path: &[&str],
) -> Result<u64, String> {
    let mut cursor = value;
    for segment in path {
        cursor = cursor.get(*segment).ok_or_else(|| {
            format!(
                "summary is missing required field `{}`",
                path.join(".")
            )
        })?;
    }
    cursor.as_u64().ok_or_else(|| {
        format!(
            "summary field `{}` must be a u64",
            path.join(".")
        )
    })
}

fn nav_roundtrip_benchmark_normalize_live_demo_args(
    mut args: Vec<String>,
) -> Result<Vec<String>, String> {
    if args.first().is_some_and(|arg| arg == "postfiat-node") {
        args.remove(0);
    }
    if args.first().is_some_and(|arg| arg.ends_with("/postfiat-node")) {
        args.remove(0);
    }
    if args.first().is_some_and(|arg| arg == "nav-roundtrip-live-demo") {
        args.remove(0);
    }
    if args.iter().any(|arg| arg == "nav-roundtrip-live-demo") {
        return Err(
            "benchmark base args must contain only nav-roundtrip-live-demo flags after the command"
                .to_string(),
        );
    }
    Ok(args)
}

fn nav_roundtrip_benchmark_validate_base_args(args: &[String], phase: &str) -> Result<(), String> {
    let stage_flags = [
        "--fleet-preflight-only",
        "--preflight-only",
        "--warm-usdc-allowance-only",
        "--evm-deposit-only",
        "--deposit-relay-only",
        "--primary-mint-only",
        "--nav-checkpoint-only",
        "--nav-exit-only",
        "--burn-to-redeem-only",
        "--evm-withdrawal-only",
        "--pftl-settle-only",
        "--pftl-only",
    ];
    for flag in stage_flags {
        if args.iter().any(|arg| arg == flag) {
            return Err(format!(
                "benchmark base args must describe the full Arbitrum roundtrip, not run/stage flag {flag}"
            ));
        }
    }
    for rejected_flag in [
        "--allow-peer-failures",
        "--defer-certified-sends",
        "--batch-only",
    ] {
        if args.iter().any(|arg| arg == rejected_flag) {
            return Err(format!(
                "benchmark base args reject {rejected_flag} for live-value acceptance runs"
            ));
        }
    }
    if args.iter().any(|arg| arg == "--signatures-file") {
        return Err(
            "benchmark base args must use --withdrawal-signer-key-file, not --signatures-file"
                .to_string(),
        );
    }
    if phase == "phase1" && args.iter().any(|arg| arg == "--same-round-nav-exit") {
        return Err(
            "Phase 1 benchmark base args must not include --same-round-nav-exit; use --phase phase2"
                .to_string(),
        );
    }
    if !args.iter().any(|arg| arg == "--withdrawal-signer-key-file") {
        return Err(
            "benchmark base args must include --withdrawal-signer-key-file for unattended complete runs"
                .to_string(),
        );
    }
    for required_flag in [
        "--topology",
        "--key-file",
        "--source-rpc-url",
        "--vault",
        "--verifier",
        "--usdc",
        "--stakehub-wallet",
        "--nav-asset",
        "--pfusdc",
        "--policy-hash",
        "--pftl-recipient",
        "--proposer",
        "--finalizer",
        "--claimer",
        "--proposer-key-file",
        "--finalizer-key-file",
        "--claimer-key-file",
        "--issuer-key-file",
        "--owner-key-file",
        "--amount-atoms",
        "--mint-amount",
        "--nonce",
        "--session-id",
        "--expires-at-height",
    ] {
        if flag_value(args, required_flag).is_none() {
            return Err(format!("benchmark base args missing {required_flag}"));
        }
    }
    Ok(())
}

fn nav_roundtrip_benchmark_strip_plan_managed_args(args: &[String]) -> Result<Vec<String>, String> {
    let value_flags = [
        "--artifact-dir",
        "--nonce",
        "--session-id",
    ];
    let bool_flags = [
        "--resume",
        "--overwrite",
        "--fast-demo-preflight",
        "--background-audit",
        "--reuse-final-certified-state",
        "--require-warm-usdc-allowance",
    ];
    let mut stripped = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if value_flags.iter().any(|flag| flag == arg) {
            if index + 1 >= args.len() {
                return Err(format!("{arg} requires a value"));
            }
            index += 2;
            continue;
        }
        if bool_flags.iter().any(|flag| flag == arg) {
            index += 1;
            continue;
        }
        stripped.push(arg.clone());
        index += 1;
    }
    Ok(stripped)
}

fn nav_roundtrip_benchmark_increment_nonce(nonce: &str, offset: usize) -> Result<String, String> {
    let normalized = nonce.strip_prefix("0x").unwrap_or(nonce);
    if normalized.len() != 64 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err("--nonce must be a 32-byte hex string".to_string());
    }
    let mut bytes = [0u8; 32];
    for index in 0..32 {
        bytes[index] = u8::from_str_radix(&normalized[index * 2..index * 2 + 2], 16)
            .map_err(|_| "--nonce must be a 32-byte hex string".to_string())?;
    }
    let mut carry = offset as u128;
    for byte in bytes.iter_mut().rev() {
        let sum = *byte as u128 + (carry & 0xff);
        *byte = (sum & 0xff) as u8;
        carry = (carry >> 8) + (sum >> 8);
        if carry == 0 {
            break;
        }
    }
    if carry != 0 {
        return Err("benchmark nonce range overflows bytes32".to_string());
    }
    let mut out = String::from("0x");
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

fn nav_roundtrip_benchmark_run_session_id(base: &str, run_label: &str) -> String {
    format!("{base}-{run_label}")
}

fn nav_roundtrip_benchmark_plan_command(
    label: String,
    command: Vec<String>,
    artifact_dir: Option<String>,
    summary_file: Option<String>,
) -> NavRoundtripBenchmarkPlanCommand {
    let command_line = nav_roundtrip_shell_join(&command);
    NavRoundtripBenchmarkPlanCommand {
        label,
        command,
        command_line,
        artifact_dir,
        summary_file,
    }
}

fn nav_roundtrip_shell_join(args: &[String]) -> String {
    args.iter()
        .map(|arg| nav_roundtrip_shell_quote(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn nav_roundtrip_shell_quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "-_./:=,@%+".contains(ch))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn nav_roundtrip_benchmark_float_arg(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn nav_roundtrip_benchmark_verify(
    options: NavRoundtripBenchmarkVerifyOptions,
) -> Result<NavRoundtripBenchmarkVerifyReport, String> {
    let phase = options.phase.trim().to_ascii_lowercase();
    let (
        default_min_clean_runs,
        default_max_median_ms,
        default_max_p90_ms,
        phase2_required,
        phase3_required,
    ) =
        match phase.as_str() {
            "phase1" => (10usize, Some(95_000.0), Some(105_000.0), false, false),
            "phase2" => (10usize, Some(75_000.0), None, true, false),
            "phase3" => (10usize, Some(55_000.0), None, false, true),
            _ => {
                return Err(
                    "--phase must be `phase1`, `phase2`, or `phase3` for NAV roundtrip benchmark verification"
                        .to_string(),
                )
            }
        };
    let required_clean_runs = options.min_clean_runs.unwrap_or(default_min_clean_runs);
    let max_median_ms = options.max_median_ms.or(default_max_median_ms);
    let max_p90_ms = options.max_p90_ms.or(default_max_p90_ms);
    let mut summary_files = Vec::new();
    if let Some(summary_file) = options.summary_file.as_ref() {
        summary_files.push(summary_file.clone());
    }
    if let Some(benchmark_dir) = options.benchmark_dir.as_ref() {
        summary_files.extend(nav_roundtrip_discover_benchmark_summaries(benchmark_dir)?);
    }
    summary_files.sort();
    summary_files.dedup();
    if summary_files.is_empty() {
        return Err("provide --summary PATH and/or --benchmark-dir DIR".to_string());
    }

    let mut summaries = Vec::new();
    for summary_file in &summary_files {
        summaries.push(nav_roundtrip_verify_benchmark_summary(
            summary_file,
            false,
            phase3_required,
        )?);
    }
    let total_ms_values = summaries
        .iter()
        .filter(|summary| summary.passed)
        .map(|summary| summary.total_ms)
        .collect::<Vec<_>>();
    let readiness_preflight_ms_values = summaries
        .iter()
        .filter(|summary| summary.passed)
        .map(|summary| summary.readiness_preflight_ms)
        .collect::<Vec<_>>();
    let protocol_clock_ms_values = summaries
        .iter()
        .filter(|summary| summary.passed)
        .map(|summary| summary.protocol_clock_ms)
        .collect::<Vec<_>>();
    let clean_run_count = protocol_clock_ms_values.len();
    let average_ms = if protocol_clock_ms_values.is_empty() {
        None
    } else {
        Some(
            protocol_clock_ms_values.iter().sum::<f64>() / protocol_clock_ms_values.len() as f64,
        )
    };
    let median_ms = nav_roundtrip_median_ms(&protocol_clock_ms_values);
    let p90_ms = nav_roundtrip_nearest_rank_percentile_ms(&protocol_clock_ms_values, 0.90);
    let best_ms = protocol_clock_ms_values
        .iter()
        .copied()
        .min_by(|left, right| left.total_cmp(right));
    let worst_ms = protocol_clock_ms_values
        .iter()
        .copied()
        .max_by(|left, right| left.total_cmp(right));
    let mut failure_reasons = Vec::new();
    if clean_run_count < required_clean_runs {
        failure_reasons.push(format!(
            "clean run count {clean_run_count} is below required {required_clean_runs}"
        ));
    }
    for summary in &summaries {
        if !summary.passed {
            failure_reasons.push(format!(
                "summary `{}` failed: {}",
                summary.summary_file,
                summary.failure_reasons.join("; ")
            ));
        }
    }
    if let (Some(median_ms), Some(max_median_ms)) = (median_ms, max_median_ms) {
        if median_ms > max_median_ms {
            failure_reasons.push(format!(
                "median protocol runtime {median_ms:.2}ms exceeds phase limit {max_median_ms:.2}ms"
            ));
        }
    }
    if let (Some(p90_ms), Some(max_p90_ms)) = (p90_ms, max_p90_ms) {
        if p90_ms > max_p90_ms {
            failure_reasons.push(format!(
                "p90 protocol runtime {p90_ms:.2}ms exceeds phase limit {max_p90_ms:.2}ms"
            ));
        }
    }
    let phase2_summary_candidate_batch_classes =
        nav_roundtrip_summary_candidate_batch_classes(&summaries);
    let supplied_required_candidate_classes =
        nav_roundtrip_normalize_required_candidate_classes(
            options.required_candidate_classes.clone(),
        )?;
    let phase2_required_candidate_classes = if phase2_required {
        nav_roundtrip_merge_candidate_classes(
            nav_roundtrip_phase2_default_candidate_classes(),
            nav_roundtrip_merge_candidate_classes(
                supplied_required_candidate_classes.clone(),
                phase2_summary_candidate_batch_classes.clone(),
            ),
        )
    } else {
        supplied_required_candidate_classes
    };
    if phase2_required {
        let missing_summary_candidate_classes = phase2_required_candidate_classes
            .iter()
            .filter(|candidate| {
                !phase2_summary_candidate_batch_classes
                    .iter()
                    .any(|summary_candidate| summary_candidate == *candidate)
            })
            .cloned()
            .collect::<Vec<_>>();
        if !missing_summary_candidate_classes.is_empty() {
            failure_reasons.push(format!(
                "Phase 2 summaries are missing required candidate batch class(es): {}",
                missing_summary_candidate_classes.join(", ")
            ));
        }
    }
    let replay_corpus_report =
        if phase2_required || options.replay_corpus_file.is_some() || options.replay_corpus_dir.is_some() {
            if options.replay_corpus_file.is_none() && options.replay_corpus_dir.is_none() {
                if phase2_required {
                    failure_reasons.push(
                        "Phase 2 requires replay corpus evidence; provide --replay-corpus-file or --replay-corpus-dir"
                            .to_string(),
                    );
                }
                None
            } else {
                let report = nav_roundtrip_replay_corpus_verify(
                    NavRoundtripReplayCorpusVerifyOptions {
                        corpus_file: options.replay_corpus_file.clone(),
                        corpus_dir: options.replay_corpus_dir.clone(),
                        report_file: None,
                        require_live_compression_ready: phase2_required,
                        required_candidate_classes: phase2_required_candidate_classes.clone(),
                        strict_exit: false,
                    },
                )?;
                if phase2_required && !report.passed {
                    failure_reasons.push(format!(
                        "Phase 2 replay corpus verification failed: {:?}",
                        report.failure_reasons
                    ));
                }
                Some(report)
            }
        } else {
            None
        };
    if phase2_required {
        failure_reasons.extend(nav_roundtrip_phase2_replay_closure_failures(
            &summaries,
            replay_corpus_report.as_ref(),
        ));
    }
    let provenance = nav_roundtrip_benchmark_provenance();
    failure_reasons.extend(
        provenance
            .failure_reasons
            .iter()
            .map(|reason| format!("benchmark provenance: {reason}")),
    );
    let mut artifact_roots = summaries
        .iter()
        .map(|summary| summary.artifact_dir.clone())
        .collect::<Vec<_>>();
    artifact_roots.sort();
    artifact_roots.dedup();
    let clean_summaries = summaries
        .iter()
        .filter(|summary| summary.passed)
        .collect::<Vec<_>>();
    let source_rpc_provider_classes = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.source_rpc_provider_class.clone()),
    );
    let bridge_classes = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.bridge_class.clone()),
    );
    let vault_addresses = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.vault_address.clone()),
    );
    let verifier_addresses = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.verifier_address.clone()),
    );
    let usdc_addresses = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.usdc_address.clone()),
    );
    let stakehub_wallets = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .map(|summary| summary.stakehub_wallet.clone()),
    );
    let vault_challenge_delay_seconds = nav_roundtrip_benchmark_unique_u64s(
        clean_summaries
            .iter()
            .filter_map(|summary| summary.vault_challenge_delay_seconds),
    );
    let vault_execution_window_seconds = nav_roundtrip_benchmark_unique_u64s(
        clean_summaries
            .iter()
            .filter_map(|summary| summary.vault_execution_window_seconds),
    );
    let verifier_challenge_delay_seconds = nav_roundtrip_benchmark_unique_u64s(
        clean_summaries
            .iter()
            .filter_map(|summary| summary.verifier_challenge_delay_seconds),
    );
    let verifier_execution_window_seconds = nav_roundtrip_benchmark_unique_u64s(
        clean_summaries
            .iter()
            .filter_map(|summary| summary.verifier_execution_window_seconds),
    );
    let final_validator_node_ids = nav_roundtrip_benchmark_unique_strings(
        clean_summaries
            .iter()
            .flat_map(|summary| summary.final_validator_node_ids.clone()),
    );
    let slowest_stages = nav_roundtrip_benchmark_slowest_stages(&clean_summaries, 3);
    let report = NavRoundtripBenchmarkVerifyReport {
        schema: NAV_ROUNDTRIP_BENCHMARK_VERIFY_REPORT_SCHEMA.to_string(),
        phase,
        summary_file: options
            .summary_file
            .as_ref()
            .map(|path| path.display().to_string()),
        benchmark_dir: options
            .benchmark_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        summary_files: summary_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        artifact_roots,
        clean_run_definition: nav_roundtrip_benchmark_clean_run_definition(),
        provenance,
        run_count: summaries.len(),
        clean_run_count,
        required_clean_runs,
        benchmark_runtime_metric: "protocol_clock_ms".to_string(),
        total_ms_values,
        readiness_preflight_ms_values,
        protocol_clock_ms_values,
        average_ms,
        mean_ms: average_ms,
        median_ms,
        p90_ms,
        best_ms,
        worst_ms,
        max_median_ms,
        max_p90_ms,
        slowest_stages,
        source_rpc_provider_classes,
        bridge_classes,
        vault_addresses,
        verifier_addresses,
        usdc_addresses,
        stakehub_wallets,
        vault_challenge_delay_seconds,
        vault_execution_window_seconds,
        verifier_challenge_delay_seconds,
        verifier_execution_window_seconds,
        final_validator_node_ids,
        phase2_live_round_compression_required: phase2_required,
        phase2_summary_candidate_batch_classes,
        phase2_required_candidate_classes,
        phase3_consolidated_bridge_required: phase3_required,
        replay_corpus_report,
        passed: failure_reasons.is_empty(),
        failure_reasons,
        summaries,
    };
    if let Some(report_file) = options.report_file.as_ref() {
        write_json_file(report_file, &report)?;
    }
    if options.strict_exit && !report.passed {
        return Err(format!(
            "NAV roundtrip benchmark verification failed: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}

fn nav_roundtrip_benchmark_clean_run_definition() -> String {
    "full Arbitrum roundtrip only: benchmark runtime thresholds apply to protocol_clock_ms; setup_or_recovery_work_included_in_total=false, final_summary_ok=true, final validator height/state-root convergence proven, final mempool empty, EVM wallet/vault deposit and withdrawal deltas proven, NAV money-in and money-out VNA deltas proven, PFTL redemption settlement accounting proven, every certified PFTL round green, required timing/provenance fields present, and phase-specific replay or consolidated-bridge gates satisfied"
        .to_string()
}

fn nav_roundtrip_benchmark_unique_strings<I>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn nav_roundtrip_benchmark_unique_u64s<I>(values: I) -> Vec<u64>
where
    I: IntoIterator<Item = u64>,
{
    let mut values = values.into_iter().collect::<Vec<_>>();
    values.sort_unstable();
    values.dedup();
    values
}

fn nav_roundtrip_benchmark_slowest_stages(
    summaries: &[&NavRoundtripBenchmarkSummaryVerifyReport],
    limit: usize,
) -> Vec<NavRoundtripBenchmarkStageTimingReport> {
    let mut stage_reports = nav_roundtrip_benchmark_stage_names()
        .into_iter()
        .filter_map(|stage| {
            let values = summaries
                .iter()
                .map(|summary| {
                    nav_roundtrip_benchmark_stage_timing_value(&summary.timings_ms, stage)
                })
                .filter(|value| value.is_finite())
                .collect::<Vec<_>>();
            if values.is_empty() {
                return None;
            }
            let mean_ms = values.iter().sum::<f64>() / values.len() as f64;
            let best_ms = values
                .iter()
                .copied()
                .min_by(|left, right| left.total_cmp(right));
            let worst_ms = values
                .iter()
                .copied()
                .max_by(|left, right| left.total_cmp(right));
            Some(NavRoundtripBenchmarkStageTimingReport {
                stage: stage.to_string(),
                sample_count: values.len(),
                mean_ms,
                median_ms: nav_roundtrip_median_ms(&values),
                p90_ms: nav_roundtrip_nearest_rank_percentile_ms(&values, 0.90),
                best_ms,
                worst_ms,
            })
        })
        .collect::<Vec<_>>();
    stage_reports.sort_by(|left, right| {
        right
            .mean_ms
            .total_cmp(&left.mean_ms)
            .then_with(|| left.stage.cmp(&right.stage))
    });
    stage_reports.truncate(limit);
    stage_reports
}

fn nav_roundtrip_benchmark_stage_names() -> [&'static str; 15] {
    [
        "fleet_preflight_ms",
        "preflight_ms",
        "stakehub_session_ms",
        "stakehub_session_close_ms",
        "evm_deposit_ms",
        "deposit_relay_ms",
        "primary_mint_ms",
        "nav_money_in_ms",
        "nav_exit_ms",
        "nav_money_out_ms",
        "burn_to_redeem_ms",
        "withdrawal_signature_ms",
        "evm_withdrawal_ms",
        "pftl_settle_ms",
        "final_verification_ms",
    ]
}

fn nav_roundtrip_benchmark_stage_timing_value(
    timings: &NavRoundtripLiveDemoTimingsReport,
    stage: &str,
) -> f64 {
    match stage {
        "fleet_preflight_ms" => timings.fleet_preflight_ms,
        "preflight_ms" => timings.preflight_ms,
        "stakehub_session_ms" => timings.stakehub_session_ms,
        "stakehub_session_close_ms" => timings.stakehub_session_close_ms,
        "evm_deposit_ms" => timings.evm_deposit_ms,
        "deposit_relay_ms" => timings.deposit_relay_ms,
        "primary_mint_ms" => timings.primary_mint_ms,
        "nav_money_in_ms" => timings.nav_money_in_ms,
        "nav_exit_ms" => timings.nav_exit_ms,
        "nav_money_out_ms" => timings.nav_money_out_ms,
        "burn_to_redeem_ms" => timings.burn_to_redeem_ms,
        "withdrawal_signature_ms" => timings.withdrawal_signature_ms,
        "evm_withdrawal_ms" => timings.evm_withdrawal_ms,
        "pftl_settle_ms" => timings.pftl_settle_ms,
        "final_verification_ms" => timings.final_verification_ms,
        _ => f64::NAN,
    }
}

fn nav_roundtrip_benchmark_provenance() -> NavRoundtripBenchmarkProvenanceReport {
    let mut failure_reasons = Vec::new();
    let package_version = env!("CARGO_PKG_VERSION").to_string();
    let binary_path = match std::env::current_exe() {
        Ok(path) => Some(path.display().to_string()),
        Err(error) => {
            failure_reasons.push(format!("failed to resolve current executable path: {error}"));
            None
        }
    };
    let binary_sha3_384 = match binary_path.as_ref() {
        Some(path) => match nav_roundtrip_sha3_384_file_hex(path) {
            Ok(hash) => Some(hash),
            Err(error) => {
                failure_reasons.push(format!("failed to hash benchmark binary `{path}`: {error}"));
                None
            }
        },
        None => None,
    };
    if binary_sha3_384.is_none() {
        failure_reasons.push("binary_sha3_384 is unavailable".to_string());
    }
    let git_commit = match nav_roundtrip_git_output(&["rev-parse", "HEAD"]) {
        Ok(commit) if commit.len() == 40 && commit.chars().all(|ch| ch.is_ascii_hexdigit()) => {
            Some(commit)
        }
        Ok(commit) => {
            failure_reasons.push(format!("git rev-parse HEAD returned invalid commit `{commit}`"));
            None
        }
        Err(error) => {
            failure_reasons.push(format!("failed to resolve git commit: {error}"));
            None
        }
    };
    let (git_dirty, git_status_porcelain_line_count) =
        match nav_roundtrip_git_output(&["status", "--porcelain=v1"]) {
            Ok(status) => {
                let line_count = status.lines().filter(|line| !line.trim().is_empty()).count();
                (Some(line_count > 0), Some(line_count))
            }
            Err(error) => {
                failure_reasons.push(format!("failed to resolve git dirty status: {error}"));
                (None, None)
            }
        };
    NavRoundtripBenchmarkProvenanceReport {
        package_version,
        binary_path,
        binary_sha3_384,
        git_commit,
        git_dirty,
        git_status_porcelain_line_count,
        failure_reasons,
    }
}

fn nav_roundtrip_sha3_384_file_hex(path: &str) -> Result<String, String> {
    match nav_roundtrip_openssl_sha3_384_file_hex(path) {
        Ok(hash) => Ok(hash),
        Err(openssl_error) => nav_roundtrip_rust_sha3_384_file_hex(path)
            .map_err(|rust_error| format!("{openssl_error}; Rust fallback failed: {rust_error}")),
    }
}

fn nav_roundtrip_openssl_sha3_384_file_hex(path: &str) -> Result<String, String> {
    let output = std::process::Command::new("openssl")
        .args(["dgst", "-sha3-384", "-r", path])
        .output()
        .map_err(|error| format!("failed to run openssl sha3-384: {error}"))?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("openssl sha3-384 stdout was not UTF-8: {error}"))?;
    let stderr = String::from_utf8(output.stderr)
        .map_err(|error| format!("openssl sha3-384 stderr was not UTF-8: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "openssl sha3-384 failed with status {:?}: {}",
            output.status.code(),
            stderr.trim()
        ));
    }
    let hash = stdout
        .split_whitespace()
        .next()
        .ok_or_else(|| "openssl sha3-384 returned empty output".to_string())?
        .to_ascii_lowercase();
    if hash.len() != 96 || !hash.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(format!("openssl sha3-384 returned invalid hash `{hash}`"));
    }
    Ok(hash)
}

fn nav_roundtrip_rust_sha3_384_file_hex(path: &str) -> Result<String, String> {
    use std::io::Read as _;

    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("failed to open binary `{path}`: {error}"))?;
    use sha3::Digest as _;
    let mut hasher = sha3::Sha3_384::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read binary `{path}`: {error}"))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let hash = hasher.finalize();
    Ok(bytes_to_hex(&hash))
}

fn nav_roundtrip_git_output(args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(args)
        .output()
        .map_err(|error| format!("failed to run git {}: {error}", args.join(" ")))?;
    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("git {} stdout was not UTF-8: {error}", args.join(" ")))?;
    let stderr = String::from_utf8(output.stderr)
        .map_err(|error| format!("git {} stderr was not UTF-8: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed with status {:?}: {}",
            args.join(" "),
            output.status.code(),
            stderr.trim()
        ));
    }
    Ok(stdout.trim().to_string())
}

fn nav_roundtrip_replay_corpus_verify(
    options: NavRoundtripReplayCorpusVerifyOptions,
) -> Result<NavRoundtripReplayCorpusVerifyReport, String> {
    let mut corpus_files = Vec::new();
    if let Some(corpus_file) = options.corpus_file.as_ref() {
        corpus_files.push(corpus_file.clone());
    }
    if let Some(corpus_dir) = options.corpus_dir.as_ref() {
        corpus_files.extend(nav_roundtrip_discover_replay_corpus_files(corpus_dir)?);
    }
    corpus_files.sort();
    corpus_files.dedup();
    if corpus_files.is_empty() {
        return Err("provide --corpus-file PATH and/or --corpus-dir DIR with replay corpus JSON".to_string());
    }

    let mut cases = Vec::new();
    for corpus_file in &corpus_files {
        cases.push(nav_roundtrip_verify_replay_corpus_case(corpus_file));
    }
    let valid_case_count = cases
        .iter()
        .filter(|case| case.valid_corpus_case)
        .count();
    let live_ready_case_count = cases
        .iter()
        .filter(|case| case.live_round_compression_ready)
        .count();
    let mut live_ready_candidate_classes = cases
        .iter()
        .filter(|case| case.live_round_compression_ready)
        .map(|case| case.candidate_batch_class.clone())
        .collect::<Vec<_>>();
    live_ready_candidate_classes.sort();
    live_ready_candidate_classes.dedup();
    let required_candidate_classes = nav_roundtrip_normalize_required_candidate_classes(
        options.required_candidate_classes.clone(),
    )?;
    let missing_required_candidate_classes = required_candidate_classes
        .iter()
        .filter(|required_class| {
            !live_ready_candidate_classes
                .iter()
                .any(|live_class| live_class == *required_class)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut failure_reasons = Vec::new();
    for case in &cases {
        if !case.valid_corpus_case {
            failure_reasons.push(format!(
                "corpus case `{}` failed validation: {}",
                case.corpus_file,
                case.failure_reasons.join("; ")
            ));
        }
        if options.require_live_compression_ready && !case.live_round_compression_ready {
            failure_reasons.push(format!(
                "corpus case `{}` is not live-round-compression ready",
                case.corpus_file
            ));
        }
    }
    if options.require_live_compression_ready && live_ready_case_count == 0 {
        failure_reasons.push(
            "no replay corpus case is marked safe for live round compression".to_string(),
        );
    }
    for missing_class in &missing_required_candidate_classes {
        failure_reasons.push(format!(
            "required candidate batch class `{missing_class}` has no live-ready replay corpus case"
        ));
    }
    let report = NavRoundtripReplayCorpusVerifyReport {
        schema: NAV_ROUNDTRIP_REPLAY_CORPUS_VERIFY_REPORT_SCHEMA.to_string(),
        corpus_file: options
            .corpus_file
            .as_ref()
            .map(|path| path.display().to_string()),
        corpus_dir: options
            .corpus_dir
            .as_ref()
            .map(|path| path.display().to_string()),
        corpus_files: corpus_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        require_live_compression_ready: options.require_live_compression_ready,
        required_candidate_classes,
        live_ready_candidate_classes,
        missing_required_candidate_classes,
        case_count: cases.len(),
        valid_case_count,
        live_ready_case_count,
        passed: failure_reasons.is_empty(),
        failure_reasons,
        cases,
    };
    if let Some(report_file) = options.report_file.as_ref() {
        write_json_file(report_file, &report)?;
    }
    if options.strict_exit && !report.passed {
        return Err(format!(
            "NAV roundtrip replay corpus verification failed: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}

fn nav_roundtrip_normalize_required_candidate_classes(
    mut classes: Vec<String>,
) -> Result<Vec<String>, String> {
    for class in &mut classes {
        *class = class.trim().to_string();
        if class.is_empty() {
            return Err("required candidate batch class must not be empty".to_string());
        }
    }
    classes.sort();
    for window in classes.windows(2) {
        if window[0] == window[1] {
            return Err(format!(
                "duplicate required candidate batch class `{}`",
                window[0]
            ));
        }
    }
    Ok(classes)
}

fn nav_roundtrip_summary_candidate_batch_classes(
    summaries: &[NavRoundtripBenchmarkSummaryVerifyReport],
) -> Vec<String> {
    let mut classes = summaries
        .iter()
        .flat_map(|summary| summary.pftl_candidate_batch_classes.iter().cloned())
        .collect::<Vec<_>>();
    classes.sort();
    classes.dedup();
    classes
}

fn nav_roundtrip_merge_candidate_classes(
    mut left: Vec<String>,
    right: Vec<String>,
) -> Vec<String> {
    left.extend(right);
    left.sort();
    left.dedup();
    left
}

fn nav_roundtrip_phase2_default_candidate_classes() -> Vec<String> {
    nav_roundtrip_normalize_required_candidate_classes(
        NAV_ROUNDTRIP_PHASE2_DEFAULT_CANDIDATE_CLASSES
            .iter()
            .map(|class| (*class).to_string())
            .collect(),
    )
    .expect("static Phase 2 candidate class list is valid")
}

fn nav_roundtrip_phase2_replay_closure_failures(
    summaries: &[NavRoundtripBenchmarkSummaryVerifyReport],
    replay_corpus_report: Option<&NavRoundtripReplayCorpusVerifyReport>,
) -> Vec<String> {
    let mut failure_reasons = Vec::new();
    let live_ready_classes = replay_corpus_report
        .filter(|report| report.passed)
        .map(|report| report.live_ready_candidate_classes.clone())
        .unwrap_or_default();
    let replay_corpus_passed = replay_corpus_report.is_some_and(|report| report.passed);

    for summary in summaries {
        let replay_required = summary.pftl_replay_equivalence_required_count > 0
            || !summary.pftl_candidate_batch_classes.is_empty()
            || !summary.pftl_live_round_compression_blockers.is_empty()
            || !summary.pftl_live_round_compression_ready;
        if !replay_required {
            continue;
        }

        if replay_corpus_report.is_none() {
            if summary.pftl_replay_equivalence_required_count > 0 {
                failure_reasons.push(format!(
                    "Phase 2 requires replay-equivalence closure for summary `{}`, but {} same-round candidate(s) still require evidence",
                    summary.summary_file, summary.pftl_replay_equivalence_required_count
                ));
            }
            if !summary.pftl_live_round_compression_ready {
                failure_reasons.push(format!(
                    "Phase 2 live round compression is not ready for summary `{}` until replay corpus evidence closes: {:?}",
                    summary.summary_file, summary.pftl_live_round_compression_blockers
                ));
            }
            continue;
        }

        if !replay_corpus_passed {
            continue;
        }

        if summary.pftl_replay_equivalence_required_count > 0
            && summary.pftl_candidate_batch_classes.is_empty()
        {
            failure_reasons.push(format!(
                "Phase 2 summary `{}` requires replay-equivalence closure but reports no candidate batch classes",
                summary.summary_file
            ));
        }

        let missing_classes = summary
            .pftl_candidate_batch_classes
            .iter()
            .filter(|candidate| !live_ready_classes.iter().any(|class| class == *candidate))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_classes.is_empty() {
            failure_reasons.push(format!(
                "Phase 2 summary `{}` has candidate batch class(es) without live-ready replay corpus evidence: {}",
                summary.summary_file,
                missing_classes.join(", ")
            ));
        }

        let unresolved_blockers = summary
            .pftl_live_round_compression_blockers
            .iter()
            .filter(|blocker| {
                !blocker.contains("same_round dependency candidates require replay-equivalence corpus evidence before live round compression")
            })
            .cloned()
            .collect::<Vec<_>>();
        if !unresolved_blockers.is_empty() {
            failure_reasons.push(format!(
                "Phase 2 compression blockers remain for summary `{}` after replay corpus closure: {:?}",
                summary.summary_file, unresolved_blockers
            ));
        }
    }

    failure_reasons
}

fn nav_roundtrip_discover_replay_corpus_files(
    root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    if !root.is_dir() {
        return Err(format!(
            "replay corpus dir `{}` does not exist or is not a directory",
            root.display()
        ));
    }
    let mut pending = vec![root.to_path_buf()];
    let mut corpus_files = Vec::new();
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| {
            format!(
                "failed to read replay corpus directory `{}`: {error}",
                dir.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read replay corpus directory entry under `{}`: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!(
                    "failed to classify replay corpus path `{}`: {error}",
                    path.display()
                )
            })?;
            if file_type.is_dir() {
                pending.push(path);
                continue;
            }
            if !file_type.is_file() || path.extension().and_then(|value| value.to_str()) != Some("json") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            if value.get("schema").and_then(serde_json::Value::as_str)
                == Some(CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA)
            {
                corpus_files.push(path);
            }
        }
    }
    corpus_files.sort();
    Ok(corpus_files)
}

fn nav_roundtrip_verify_replay_corpus_case(
    corpus_file: &std::path::Path,
) -> NavRoundtripReplayCorpusCaseVerifyReport {
    let mut failure_reasons = Vec::new();
    let raw = match std::fs::read_to_string(corpus_file) {
        Ok(raw) => raw,
        Err(error) => {
            return NavRoundtripReplayCorpusCaseVerifyReport {
                corpus_file: corpus_file.display().to_string(),
                case: String::new(),
                candidate_batch_class: String::new(),
                state_root_match: false,
                ledger_facing_asset_definitions_match: None,
                ledger_facing_state_match: None,
                safe_for_live_round_compression: false,
                valid_corpus_case: false,
                live_round_compression_ready: false,
                failure_reasons: vec![format!("failed to read corpus file: {error}")],
            }
        }
    };
    let value = match serde_json::from_str::<serde_json::Value>(&raw) {
        Ok(value) => value,
        Err(error) => {
            return NavRoundtripReplayCorpusCaseVerifyReport {
                corpus_file: corpus_file.display().to_string(),
                case: String::new(),
                candidate_batch_class: String::new(),
                state_root_match: false,
                ledger_facing_asset_definitions_match: None,
                ledger_facing_state_match: None,
                safe_for_live_round_compression: false,
                valid_corpus_case: false,
                live_round_compression_ready: false,
                failure_reasons: vec![format!("failed to parse corpus JSON: {error}")],
            }
        }
    };
    let case = match serde_json::from_value::<CertifiedAssetOpsBatchEquivalenceCorpusCase>(value) {
        Ok(case) => case,
        Err(error) => {
            return NavRoundtripReplayCorpusCaseVerifyReport {
                corpus_file: corpus_file.display().to_string(),
                case: String::new(),
                candidate_batch_class: String::new(),
                state_root_match: false,
                ledger_facing_asset_definitions_match: None,
                ledger_facing_state_match: None,
                safe_for_live_round_compression: false,
                valid_corpus_case: false,
                live_round_compression_ready: false,
                failure_reasons: vec![format!("failed to parse replay corpus case: {error}")],
            }
        }
    };
    if case.schema != CERTIFIED_ASSET_OPS_BATCH_EQUIVALENCE_CORPUS_SCHEMA {
        failure_reasons.push(format!("unexpected corpus schema `{}`", case.schema));
    }
    if case.case.trim().is_empty() {
        failure_reasons.push("case name is empty".to_string());
    }
    if case.candidate_batch_class.trim().is_empty() {
        failure_reasons.push("candidate_batch_class is empty".to_string());
    }
    if case.unbatched_state_root.trim().is_empty() {
        failure_reasons.push("unbatched_state_root is empty".to_string());
    }
    if case.batched_state_root.trim().is_empty() {
        failure_reasons.push("batched_state_root is empty".to_string());
    }
    let roots_equal = case.unbatched_state_root == case.batched_state_root;
    if case.state_root_match != roots_equal {
        failure_reasons.push(format!(
            "state_root_match={} contradicts the supplied roots equality={roots_equal}",
            case.state_root_match
        ));
    }
    if !case.state_root_match {
        let documented = case
            .intended_state_root_difference
            .as_ref()
            .is_some_and(|reason| !reason.trim().is_empty());
        if !documented {
            failure_reasons.push(
                "state-root mismatch lacks intended_state_root_difference documentation"
                    .to_string(),
            );
        }
    }
    if case.ledger_facing_asset_definitions_match == Some(false) {
        failure_reasons
            .push("ledger_facing_asset_definitions_match=false for replay corpus case".to_string());
    }
    if case.ledger_facing_state_match == Some(false) {
        failure_reasons.push("ledger_facing_state_match=false for replay corpus case".to_string());
    }
    if case.safe_for_live_round_compression {
        if case.ledger_facing_asset_definitions_match != Some(true)
            && case.ledger_facing_state_match != Some(true)
        {
            failure_reasons.push(
                "safe live compression requires ledger_facing_state_match=true or ledger_facing_asset_definitions_match=true"
                    .to_string(),
            );
        }
        if !case.state_root_match {
            let gate = case.gate.as_ref().map(|value| value.trim()).unwrap_or("");
            if gate.is_empty() {
                failure_reasons.push(
                    "safe live compression with state-root difference requires a nonempty gate"
                        .to_string(),
                );
            }
            let normalized_gate = gate.to_ascii_lowercase();
            if normalized_gate.contains("do not use") || normalized_gate.contains("not use as live") {
                failure_reasons.push(format!(
                    "safe live compression gate is still prohibitive: `{gate}`"
                ));
            }
        }
    }
    let valid_corpus_case = failure_reasons.is_empty();
    let live_round_compression_ready =
        valid_corpus_case && case.safe_for_live_round_compression;
    NavRoundtripReplayCorpusCaseVerifyReport {
        corpus_file: corpus_file.display().to_string(),
        case: case.case,
        candidate_batch_class: case.candidate_batch_class,
        state_root_match: case.state_root_match,
        ledger_facing_asset_definitions_match: case.ledger_facing_asset_definitions_match,
        ledger_facing_state_match: case.ledger_facing_state_match,
        safe_for_live_round_compression: case.safe_for_live_round_compression,
        valid_corpus_case,
        live_round_compression_ready,
        failure_reasons,
    }
}

fn nav_roundtrip_discover_benchmark_summaries(
    root: &std::path::Path,
) -> Result<Vec<std::path::PathBuf>, String> {
    if !root.is_dir() {
        return Err(format!(
            "benchmark dir `{}` does not exist or is not a directory",
            root.display()
        ));
    }
    let mut pending = vec![root.to_path_buf()];
    let mut summaries = Vec::new();
    while let Some(dir) = pending.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|error| {
            format!(
                "failed to read benchmark directory `{}`: {error}",
                dir.display()
            )
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "failed to read benchmark directory entry under `{}`: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|error| {
                format!("failed to classify benchmark path `{}`: {error}", path.display())
            })?;
            if file_type.is_dir() {
                pending.push(path);
            } else if file_type.is_file()
                && path.file_name().and_then(|name| name.to_str()) == Some("roundtrip-summary.json")
            {
                summaries.push(path);
            }
        }
    }
    summaries.sort();
    Ok(summaries)
}

fn nav_roundtrip_verify_benchmark_summary(
    summary_file: &std::path::Path,
    require_live_round_compression_ready: bool,
    require_consolidated_bridge_evidence: bool,
) -> Result<NavRoundtripBenchmarkSummaryVerifyReport, String> {
    let raw = std::fs::read_to_string(summary_file).map_err(|error| {
        format!(
            "failed to read NAV roundtrip summary `{}`: {error}",
            summary_file.display()
        )
    })?;
    let raw_value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| {
        format!(
            "failed to parse NAV roundtrip summary JSON `{}`: {error}",
            summary_file.display()
        )
    })?;
    let report = serde_json::from_str::<NavRoundtripLiveDemoReport>(&raw).map_err(|error| {
        format!(
            "failed to parse NAV roundtrip summary `{}`: {error}",
            summary_file.display()
        )
    })?;
    let mut failure_reasons = Vec::new();
    if report.schema != NAV_ROUNDTRIP_LIVE_DEMO_REPORT_SCHEMA {
        failure_reasons.push(format!(
            "unexpected summary schema `{}`",
            report.schema
        ));
    }
    if report.run_class != NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP {
        failure_reasons.push(format!(
            "run_class `{}` is not `{}`",
            report.run_class, NAV_ROUNDTRIP_RUN_CLASS_FULL_ARBITRUM_ROUNDTRIP
        ));
    }
    if report.completion_status != NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP {
        failure_reasons.push(format!(
            "completion_status `{}` is not `{}`",
            report.completion_status, NAV_ROUNDTRIP_COMPLETION_FULL_ARBITRUM_ROUNDTRIP
        ));
    }
    if report.custody_location != NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC {
        failure_reasons.push(format!(
            "custody_location `{}` is not `{}`",
            report.custody_location, NAV_ROUNDTRIP_CUSTODY_ARBITRUM_WALLET_USDC
        ));
    }
    for required_field in [
        "timing_scope",
        "protocol_clock_started_at_stage",
        "protocol_clock_stopped_at_stage",
        "setup_or_recovery_work_included_in_total",
        "source_rpc_provider_class",
        "bridge_class",
        "vault_address",
        "verifier_address",
        "usdc_address",
        "stakehub_wallet",
        "stakehub_launch_session_mode",
    ] {
        if raw_value.get(required_field).is_none() {
            failure_reasons.push(format!(
                "summary is missing required benchmark field `{required_field}`"
            ));
        }
    }
    for (object_field, required_field) in [
        ("preflight", "vault_challenge_delay_seconds"),
        ("preflight", "vault_execution_window_seconds"),
        ("preflight", "verifier_challenge_delay_seconds"),
        ("preflight", "verifier_execution_window_seconds"),
        ("preflight", "usdc_allowance_atoms"),
        ("fleet_preflight", "reused_artifact"),
        ("evm_deposit", "approve_skipped"),
        ("evm_deposit", "source_rpc_provider_class"),
        ("evm_withdrawal", "source_rpc_provider_class"),
        ("evm_withdrawal", "verifier_challenge_wait_secs"),
        ("evm_withdrawal", "vault_challenge_wait_secs"),
    ] {
        if raw_value
            .get(object_field)
            .and_then(|object| object.get(required_field))
            .is_none()
        {
            failure_reasons.push(format!(
                "summary is missing required benchmark field `{object_field}.{required_field}`"
            ));
        }
    }
    if report.preflight.vault_challenge_delay_seconds.is_none() {
        failure_reasons.push(
            "preflight.vault_challenge_delay_seconds is not resolved".to_string(),
        );
    }
    if report.preflight.verifier_challenge_delay_seconds.is_none() {
        failure_reasons.push(
            "preflight.verifier_challenge_delay_seconds is not resolved".to_string(),
        );
    }
    if report.timing_scope != nav_roundtrip_default_full_timing_scope() {
        failure_reasons.push(format!(
            "timing_scope `{}` is not `{}`",
            report.timing_scope,
            nav_roundtrip_default_full_timing_scope()
        ));
    }
    if report.protocol_clock_started_at_stage != nav_roundtrip_default_full_protocol_clock_start() {
        failure_reasons.push(format!(
            "protocol_clock_started_at_stage `{}` is not `{}`",
            report.protocol_clock_started_at_stage,
            nav_roundtrip_default_full_protocol_clock_start()
        ));
    }
    if report.protocol_clock_stopped_at_stage != nav_roundtrip_default_full_protocol_clock_stop() {
        failure_reasons.push(format!(
            "protocol_clock_stopped_at_stage `{}` is not `{}`",
            report.protocol_clock_stopped_at_stage,
            nav_roundtrip_default_full_protocol_clock_stop()
        ));
    }
    if report.setup_or_recovery_work_included_in_total {
        failure_reasons.push(
            "setup_or_recovery_work_included_in_total=true; recovery/setup runs are not clean benchmark runs"
                .to_string(),
        );
    }
    if !report.final_summary_ok {
        failure_reasons.push(format!(
            "final_summary_ok=false: {:?}",
            report.failure_reasons
        ));
    }
    if !report.final_validator_consensus_ok {
        failure_reasons.push("final validator consensus is not proven".to_string());
    }
    if report.final_validator_states.is_empty() {
        failure_reasons.push("final validator state evidence is empty".to_string());
    }
    if report.final_mempool_pending != 0 {
        failure_reasons.push(format!(
            "final mempool pending count is {}",
            report.final_mempool_pending
        ));
    }
    match report.fleet_preflight.as_ref() {
        Some(fleet_preflight) => {
            if !fleet_preflight.preflight_ok {
                failure_reasons.push(format!(
                    "fleet preflight failed: {:?}",
                    fleet_preflight.failure_reasons
                ));
            }
            if !fleet_preflight.public_validator_consensus_ok {
                failure_reasons.push("fleet preflight public validator consensus failed".to_string());
            }
            if report.preflight_profile == "fast_demo_precomputed_fleet_required"
                && !fleet_preflight.reused_artifact
            {
                failure_reasons.push(
                    "fast demo preflight profile requires reused fleet-preflight evidence"
                        .to_string(),
                );
            }
        }
        None => failure_reasons.push("fleet preflight evidence is missing".to_string()),
    }
    if !report.preflight.preflight_ok {
        failure_reasons.push(format!(
            "preflight failed: {:?}",
            report.preflight.failure_reasons
        ));
    }
    if !report.evm_deposit.delta_ok {
        failure_reasons.push(format!(
            "EVM deposit USDC delta failed: {:?}",
            report.evm_deposit.failure_reasons
        ));
    }
    if !report.evm_deposit.approve_skipped {
        failure_reasons.push(
            "EVM deposit included USDC approval; clean benchmark runs require warm allowance before the protocol clock"
                .to_string(),
        );
    }
    match report
        .preflight
        .usdc_allowance_atoms
        .as_deref()
        .and_then(|value| value.parse::<u128>().ok())
    {
        Some(allowance) if allowance >= u128::from(report.amount_atoms) => {}
        Some(allowance) => failure_reasons.push(format!(
            "preflight USDC allowance {} atoms is below benchmark amount {} atoms",
            allowance, report.amount_atoms
        )),
        None => failure_reasons
            .push("preflight USDC allowance evidence is missing or invalid".to_string()),
    }
    if report.nav_money_in.delta_ok != Some(true) {
        failure_reasons.push(format!(
            "NAV money-in VNA delta not proven: {:?}",
            report.nav_money_in.failure_reasons
        ));
    }
    if report.nav_money_out.delta_ok != Some(true) {
        failure_reasons.push(format!(
            "NAV money-out VNA delta not proven: {:?}",
            report.nav_money_out.failure_reasons
        ));
    }
    if !report.evm_withdrawal.delta_ok {
        failure_reasons.push(format!(
            "EVM withdrawal USDC delta failed: {:?}",
            report.evm_withdrawal.failure_reasons
        ));
    }
    if report.pftl_settle.accounting_ok != Some(true) {
        failure_reasons.push(format!(
            "PFTL redemption settle accounting not proven: {:?}",
            report.pftl_settle.failure_reasons
        ));
    }
    if report.pftl_certified_rounds.is_empty() {
        failure_reasons.push("PFTL certified-round timing table is missing".to_string());
    }
    for round in &report.pftl_certified_rounds {
        if round.round_ok != Some(true) {
            failure_reasons.push(format!(
                "PFTL certified round {}/{} is not proven green",
                round.stage, round.round
            ));
        }
    }
    let missing_timing_keys = nav_roundtrip_missing_benchmark_timing_keys(&raw_value);
    if !missing_timing_keys.is_empty() {
        failure_reasons.push(format!(
            "summary timings_ms is missing required fields: {}",
            missing_timing_keys.join(", ")
        ));
    }
    if !report.timings_ms.total_ms.is_finite() || report.timings_ms.total_ms <= 0.0 {
        failure_reasons.push(format!(
            "total runtime is not positive and finite: {}",
            report.timings_ms.total_ms
        ));
    }
    for (field, value) in nav_roundtrip_timing_field_values(&report.timings_ms) {
        if !value.is_finite() || value < 0.0 {
            failure_reasons.push(format!(
                "timing field {field} is not finite and nonnegative: {value}"
            ));
        }
    }
    if report.timings_ms.readiness_preflight_ms + report.timings_ms.protocol_clock_ms
        > report.timings_ms.total_ms + 5.0
    {
        failure_reasons.push(format!(
            "readiness_preflight_ms + protocol_clock_ms exceeds total_ms: {} + {} > {}",
            report.timings_ms.readiness_preflight_ms,
            report.timings_ms.protocol_clock_ms,
            report.timings_ms.total_ms
        ));
    }
    if require_live_round_compression_ready {
        if report.pftl_replay_equivalence_required_count != 0 {
            failure_reasons.push(format!(
                "Phase 2 requires replay-equivalence closure, but {} same-round candidate(s) still require evidence",
                report.pftl_replay_equivalence_required_count
            ));
        }
        if !report.pftl_live_round_compression_ready {
            failure_reasons.push(format!(
                "Phase 2 live round compression is not ready: {:?}",
                report.pftl_live_round_compression_blockers
            ));
        }
        if !report.pftl_live_round_compression_blockers.is_empty() {
            failure_reasons.push(format!(
                "Phase 2 compression blockers remain: {:?}",
                report.pftl_live_round_compression_blockers
            ));
        }
    }
    let withdrawal_receipt_labels = report
        .evm_withdrawal
        .receipt_watches
        .iter()
        .map(|watch| watch.label.clone())
        .collect::<Vec<_>>();
    let phase3_consolidated_bridge_evidence_ok = if require_consolidated_bridge_evidence {
        let phase3_failures =
            nav_roundtrip_phase3_consolidated_bridge_failures(&report, &withdrawal_receipt_labels);
        if !phase3_failures.is_empty() {
            failure_reasons.extend(phase3_failures);
            Some(false)
        } else {
            Some(true)
        }
    } else {
        None
    };
    Ok(NavRoundtripBenchmarkSummaryVerifyReport {
        summary_file: summary_file.display().to_string(),
        artifact_dir: report.artifact_dir,
        data_dir: report.data_dir,
        run_class: report.run_class,
        completion_status: report.completion_status,
        custody_location: report.custody_location,
        timing_scope: report.timing_scope,
        protocol_clock_started_at_stage: report.protocol_clock_started_at_stage,
        protocol_clock_stopped_at_stage: report.protocol_clock_stopped_at_stage,
        setup_or_recovery_work_included_in_total: report.setup_or_recovery_work_included_in_total,
        total_ms: report.timings_ms.total_ms,
        readiness_preflight_ms: report.timings_ms.readiness_preflight_ms,
        protocol_clock_ms: report.timings_ms.protocol_clock_ms,
        timings_ms: report.timings_ms.clone(),
        source_rpc_provider_class: report.source_rpc_provider_class,
        preflight_source_rpc_provider_class: report.preflight.source_rpc_provider_class,
        evm_deposit_source_rpc_provider_class: report.evm_deposit.source_rpc_provider_class,
        evm_withdrawal_source_rpc_provider_class: report.evm_withdrawal.source_rpc_provider_class,
        source_chain_id: report.source_chain_id,
        vault_address: report.vault_address,
        verifier_address: report.verifier_address,
        usdc_address: report.usdc_address,
        stakehub_wallet: report.stakehub_wallet,
        vault_challenge_delay_seconds: report.preflight.vault_challenge_delay_seconds,
        vault_execution_window_seconds: report.preflight.vault_execution_window_seconds,
        verifier_challenge_delay_seconds: report.preflight.verifier_challenge_delay_seconds,
        verifier_execution_window_seconds: report.preflight.verifier_execution_window_seconds,
        evm_withdrawal_verifier_challenge_wait_secs: report
            .evm_withdrawal
            .verifier_challenge_wait_secs,
        evm_withdrawal_vault_challenge_wait_secs: report.evm_withdrawal.vault_challenge_wait_secs,
        approve_skipped: report.evm_deposit.approve_skipped,
        allowance_before_atoms: report.evm_deposit.allowance_before_atoms,
        stakehub_launch_session_mode: report.stakehub_launch_session_mode,
        evm_deposit_launch_session_managed_externally: report
            .evm_deposit
            .launch_session_managed_externally,
        evm_withdrawal_launch_session_managed_externally: report
            .evm_withdrawal
            .launch_session_managed_externally,
        background_audit_enabled: report.background_audit_enabled,
        final_audit_profile: report.final_audit_profile,
        final_validator_state_source: report.final_validator_state_source,
        final_validator_node_ids: report
            .final_validator_states
            .iter()
            .map(|state| state.node_id.clone())
            .collect(),
        final_height: report.final_height,
        final_state_root: report.final_state_root,
        bridge_class: report.bridge_class,
        final_summary_ok: report.final_summary_ok,
        final_validator_consensus_ok: report.final_validator_consensus_ok,
        final_mempool_pending: report.final_mempool_pending,
        evm_deposit_wallet_usdc_before_atoms: report.evm_deposit.wallet_usdc_before_atoms,
        evm_deposit_wallet_usdc_after_atoms: report.evm_deposit.wallet_usdc_after_atoms,
        evm_deposit_vault_usdc_before_atoms: report.evm_deposit.vault_usdc_before_atoms,
        evm_deposit_vault_usdc_after_atoms: report.evm_deposit.vault_usdc_after_atoms,
        evm_deposit_delta_ok: report.evm_deposit.delta_ok,
        nav_money_in_expected_vna_delta: report.nav_money_in.expected_verified_net_assets_delta,
        nav_money_in_actual_vna_delta: report.nav_money_in.verified_net_assets_delta,
        nav_money_in_delta_ok: report.nav_money_in.delta_ok,
        nav_money_out_expected_vna_delta: report.nav_money_out.expected_verified_net_assets_delta,
        nav_money_out_actual_vna_delta: report.nav_money_out.verified_net_assets_delta,
        nav_money_out_delta_ok: report.nav_money_out.delta_ok,
        evm_withdrawal_wallet_usdc_before_atoms: report.evm_withdrawal.wallet_usdc_before_atoms,
        evm_withdrawal_wallet_usdc_after_atoms: report.evm_withdrawal.wallet_usdc_after_atoms,
        evm_withdrawal_vault_usdc_before_atoms: report.evm_withdrawal.vault_usdc_before_atoms,
        evm_withdrawal_vault_usdc_after_atoms: report.evm_withdrawal.vault_usdc_after_atoms,
        evm_withdrawal_delta_ok: report.evm_withdrawal.delta_ok,
        evm_deposit_receipt_watch_count: report.evm_deposit.receipt_watches.len(),
        evm_withdrawal_receipt_watch_count: report.evm_withdrawal.receipt_watches.len(),
        evm_withdrawal_receipt_watch_labels: withdrawal_receipt_labels,
        phase3_consolidated_bridge_evidence_ok,
        pftl_redemption_queue_before_atoms: report.pftl_settle.redemption_queue_before_atoms,
        pftl_redemption_queue_after_atoms: report.pftl_settle.redemption_queue_after_atoms,
        pftl_counted_value_before_atoms: report.pftl_settle.counted_value_before_atoms,
        pftl_counted_value_after_atoms: report.pftl_settle.counted_value_after_atoms,
        pftl_settle_accounting_ok: report.pftl_settle.accounting_ok,
        pftl_certified_round_count: report.pftl_certified_round_count,
        pftl_certified_operation_count: report.pftl_certified_operation_count,
        pftl_replay_equivalence_required_count: report.pftl_replay_equivalence_required_count,
        pftl_candidate_batch_classes: report.pftl_candidate_batch_classes,
        pftl_live_round_compression_ready: report.pftl_live_round_compression_ready,
        pftl_live_round_compression_blockers: report.pftl_live_round_compression_blockers,
        passed: failure_reasons.is_empty(),
        failure_reasons,
    })
}

fn nav_roundtrip_phase3_consolidated_bridge_failures(
    report: &NavRoundtripLiveDemoReport,
    withdrawal_receipt_labels: &[String],
) -> Vec<String> {
    let mut failure_reasons = Vec::new();
    if report.bridge_class != NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED {
        failure_reasons.push(format!(
            "Phase 3 requires bridge_class `{}`, got `{}`",
            NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED,
            report.bridge_class
        ));
    }
    if report.evm_withdrawal.bridge_class != NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED {
        failure_reasons.push(format!(
            "Phase 3 requires EVM withdrawal bridge_class `{}`, got `{}`",
            NAV_ROUNDTRIP_BRIDGE_CLASS_FIXED_REDEPLOYED_CONSOLIDATED,
            report.evm_withdrawal.bridge_class
        ));
    }
    let normalized_labels = withdrawal_receipt_labels
        .iter()
        .map(|label| label.replace('-', "_"))
        .collect::<Vec<_>>();
    for required_label in [
        "submit_proof",
        "finalize_proof_and_submit_withdrawal",
        "finalize_withdrawal_and_claim",
    ] {
        if !normalized_labels
            .iter()
            .any(|label| label == required_label)
        {
            failure_reasons.push(format!(
                "Phase 3 requires consolidated EVM withdrawal receipt label `{required_label}`"
            ));
        }
    }
    for forbidden_label in [
        "finalize_proof",
        "submit_withdrawal",
        "finalize_withdrawal",
        "claim_withdrawal",
    ] {
        if normalized_labels
            .iter()
            .any(|label| label == forbidden_label)
        {
            failure_reasons.push(format!(
                "Phase 3 consolidated benchmark cannot use old separate EVM withdrawal receipt label `{forbidden_label}`"
            ));
        }
    }
    if report.evm_withdrawal.receipt_watches.is_empty() {
        failure_reasons.push(
            "Phase 3 requires EVM withdrawal receipt watcher evidence".to_string(),
        );
    }
    for watch in &report.evm_withdrawal.receipt_watches {
        if watch.status != "confirmed" {
            failure_reasons.push(format!(
                "Phase 3 EVM withdrawal receipt `{}` is not confirmed: {}",
                watch.label, watch.status
            ));
        }
        if watch.tx_hash.trim().is_empty() {
            failure_reasons.push(format!(
                "Phase 3 EVM withdrawal receipt `{}` has empty tx hash",
                watch.label
            ));
        }
    }
    failure_reasons
}

fn nav_roundtrip_missing_benchmark_timing_keys(raw_value: &serde_json::Value) -> Vec<String> {
    let Some(timings) = raw_value
        .get("timings_ms")
        .and_then(serde_json::Value::as_object)
    else {
        return vec!["timings_ms".to_string()];
    };
    [
        "total_ms",
        "readiness_preflight_ms",
        "protocol_clock_ms",
        "fleet_preflight_ms",
        "preflight_ms",
        "stakehub_session_ms",
        "stakehub_session_close_ms",
        "evm_deposit_ms",
        "deposit_relay_ms",
        "primary_mint_ms",
        "nav_money_in_ms",
        "nav_exit_ms",
        "nav_money_out_ms",
        "burn_to_redeem_ms",
        "withdrawal_signature_ms",
        "evm_withdrawal_ms",
        "pftl_settle_ms",
        "final_verification_ms",
    ]
    .into_iter()
    .filter(|field| !timings.contains_key(*field))
    .map(str::to_string)
    .collect()
}

fn nav_roundtrip_timing_field_values(
    timings: &NavRoundtripLiveDemoTimingsReport,
) -> [(&'static str, f64); 18] {
    [
        ("total_ms", timings.total_ms),
        ("readiness_preflight_ms", timings.readiness_preflight_ms),
        ("protocol_clock_ms", timings.protocol_clock_ms),
        ("fleet_preflight_ms", timings.fleet_preflight_ms),
        ("preflight_ms", timings.preflight_ms),
        ("stakehub_session_ms", timings.stakehub_session_ms),
        (
            "stakehub_session_close_ms",
            timings.stakehub_session_close_ms,
        ),
        ("evm_deposit_ms", timings.evm_deposit_ms),
        ("deposit_relay_ms", timings.deposit_relay_ms),
        ("primary_mint_ms", timings.primary_mint_ms),
        ("nav_money_in_ms", timings.nav_money_in_ms),
        ("nav_exit_ms", timings.nav_exit_ms),
        ("nav_money_out_ms", timings.nav_money_out_ms),
        ("burn_to_redeem_ms", timings.burn_to_redeem_ms),
        ("withdrawal_signature_ms", timings.withdrawal_signature_ms),
        ("evm_withdrawal_ms", timings.evm_withdrawal_ms),
        ("pftl_settle_ms", timings.pftl_settle_ms),
        ("final_verification_ms", timings.final_verification_ms),
    ]
}

fn nav_roundtrip_median_ms(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let midpoint = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        Some((sorted[midpoint - 1] + sorted[midpoint]) / 2.0)
    } else {
        Some(sorted[midpoint])
    }
}

fn nav_roundtrip_nearest_rank_percentile_ms(values: &[f64], percentile: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|left, right| left.total_cmp(right));
    let rank = (percentile * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    Some(sorted[index])
}
