fn nav_roundtrip_checkpoint_elapsed_ms(stage_start: &mut std::time::Instant) -> f64 {
    let now = std::time::Instant::now();
    let elapsed_ms = now.duration_since(*stage_start).as_secs_f64() * 1000.0;
    *stage_start = now;
    elapsed_ms
}

fn nav_roundtrip_certified_round_summary(
    stage: &str,
    round: &str,
    report: &CertifiedAssetOpsBatchReport,
) -> NavRoundtripPftlCertifiedRoundSummary {
    NavRoundtripPftlCertifiedRoundSummary {
        stage: stage.to_string(),
        round: round.to_string(),
        operation_count: report.operation_count,
        start_height: report.start_height,
        end_height: report.end_height,
        end_state_root: report.end_state_root.clone(),
        round_ok: report.round_ok,
        total_ms: report.timings_ms.total_ms,
        certify_ms: report.timings_ms.certify_ms,
    }
}

fn nav_roundtrip_push_certified_rounds(
    output: &mut Vec<NavRoundtripPftlCertifiedRoundSummary>,
    stage: &str,
    aggregate_round: &str,
    stages: &[CertifiedAssetOpsBatchReport],
    aggregate: &CertifiedAssetOpsBatchReport,
) {
    if stages.is_empty() {
        output.push(nav_roundtrip_certified_round_summary(
            stage,
            aggregate_round,
            aggregate,
        ));
        return;
    }
    for (index, report) in stages.iter().enumerate() {
        output.push(nav_roundtrip_certified_round_summary(
            stage,
            &format!("stage-{}", index + 1),
            report,
        ));
    }
}

fn nav_roundtrip_collect_certified_ops_compression_gates(
    replay_equivalence_required_count: &mut usize,
    candidate_batch_classes: &mut Vec<String>,
    blockers: &mut Vec<String>,
    stage: &str,
    aggregate_round: &str,
    stages: &[CertifiedAssetOpsBatchReport],
    aggregate: &CertifiedAssetOpsBatchReport,
) {
    if stages.is_empty() {
        nav_roundtrip_collect_certified_ops_compression_gate(
            replay_equivalence_required_count,
            candidate_batch_classes,
            blockers,
            stage,
            aggregate_round,
            aggregate,
        );
        return;
    }
    for (index, report) in stages.iter().enumerate() {
        nav_roundtrip_collect_certified_ops_compression_gate(
            replay_equivalence_required_count,
            candidate_batch_classes,
            blockers,
            stage,
            &format!("stage-{}", index + 1),
            report,
        );
    }
}

fn nav_roundtrip_collect_certified_ops_compression_gate(
    replay_equivalence_required_count: &mut usize,
    candidate_batch_classes: &mut Vec<String>,
    blockers: &mut Vec<String>,
    stage: &str,
    round: &str,
    report: &CertifiedAssetOpsBatchReport,
) {
    if report.dependency_report.replay_equivalence_required {
        *replay_equivalence_required_count += 1;
    }
    candidate_batch_classes.extend(certified_asset_ops_dependency_report_candidate_batch_classes(
        &report.dependency_report,
    ));
    if !report.dependency_report.live_round_compression_ready {
        if report.dependency_report.live_round_compression_blockers.is_empty() {
            blockers.push(format!(
                "{stage}/{round}: live round compression is not ready"
            ));
            return;
        }
        for blocker in &report.dependency_report.live_round_compression_blockers {
            blockers.push(format!("{stage}/{round}: {blocker}"));
        }
    }
}

fn nav_roundtrip_require_certified_ops_ok(
    label: &str,
    report: &CertifiedAssetOpsBatchReport,
    artifact_dir: &std::path::Path,
) -> Result<(), String> {
    if report.prepare_only {
        let error = format!("{label} ran in prepare-only mode during full roundtrip");
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        return Err(error);
    }
    if report.batch_only {
        let error = format!("{label} ran in batch-only mode during full roundtrip");
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        return Err(error);
    }
    if report.round_ok != Some(true) {
        let error = format!("{label} certified round did not report round_ok=true");
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        return Err(error);
    }
    nav_roundtrip_require_strict_round_report(label, report, artifact_dir)?;
    if report.end_mempool_pending != Some(0) {
        let error = format!(
            "{label} ended with mempool_pending={:?}, expected Some(0)",
            report.end_mempool_pending
        );
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        return Err(error);
    }
    Ok(())
}

fn nav_roundtrip_reject_degraded_live_options(
    label: &str,
    allow_peer_failures: bool,
    defer_certified_sends: bool,
) -> Result<(), String> {
    if allow_peer_failures {
        return Err(format!(
            "{label} is live-value mode and rejects --allow-peer-failures"
        ));
    }
    if defer_certified_sends {
        return Err(format!(
            "{label} is live-value mode and rejects --defer-certified-sends"
        ));
    }
    Ok(())
}

fn nav_roundtrip_require_strict_round_report(
    label: &str,
    report: &CertifiedAssetOpsBatchReport,
    artifact_dir: &std::path::Path,
) -> Result<(), String> {
    let round_report_file =
        nav_roundtrip_certified_ops_round_report_file(report).map_err(|error| {
            let error = format!("{label} strict live-value check failed to find round report: {error}");
            nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
            error
        })?;
    let raw = std::fs::read_to_string(&round_report_file).map_err(|error| {
        let error = format!(
            "{label} strict live-value check failed to read round report `{}`: {error}",
            round_report_file.display()
        );
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        error
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| {
        let error = format!(
            "{label} strict live-value check failed to parse round report `{}`: {error}",
            round_report_file.display()
        );
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        error
    })?;
    let round = value.get("round").unwrap_or(&value);
    let mut public_fleet_caught_up: Option<Vec<NavRoundtripValidatorStateEvidence>> = None;
    for (field, description) in [
        ("allow_peer_failures", "allowed peer failures"),
        ("certified_sends_deferred", "deferred certified sends"),
    ] {
        if round
            .get(field)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            let error = format!("{label} strict live-value check rejected {description}");
            nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
            return Err(error);
        }
    }
    for field in [
        "unresolved_vote_targets",
        "vote_request_failures",
        "send_failures",
        "skipped_certified_send_targets",
    ] {
        if !round
            .get(field)
            .and_then(serde_json::Value::as_array)
            .map(Vec::is_empty)
            .unwrap_or(false)
        {
            if field == "unresolved_vote_targets" {
                let unresolved_is_quorum_early_only = round
                    .get("quorum_early_full_propagation")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
                    && round
                        .get("local_apply_before_certified_send")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    && round
                        .get("all_sends_verified")
                        .and_then(serde_json::Value::as_bool)
                        == Some(true)
                    && round
                        .get("vote_request_failures")
                        .and_then(serde_json::Value::as_array)
                        .map(Vec::is_empty)
                        .unwrap_or(false)
                    && round
                        .get("send_failures")
                        .and_then(serde_json::Value::as_array)
                        .map(Vec::is_empty)
                        .unwrap_or(false);
                if unresolved_is_quorum_early_only {
                    continue;
                }
                let states = match &public_fleet_caught_up {
                    Some(states) => Some(states.clone()),
                    None => nav_roundtrip_strict_public_fleet_caught_up(report).ok(),
                };
                if let Some(states) = states {
                    public_fleet_caught_up = Some(states);
                    continue;
                }
            }
            let error = format!("{label} strict live-value check rejected non-empty `{field}`");
            nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
            return Err(error);
        }
    }
    for field in ["all_vote_requests_verified", "all_sends_verified", "round_ok"] {
        if round
            .get(field)
            .and_then(serde_json::Value::as_bool)
            != Some(true)
        {
            if field == "all_sends_verified" {
                let states = match &public_fleet_caught_up {
                    Some(states) => Some(states.clone()),
                    None => nav_roundtrip_strict_public_fleet_caught_up(report).ok(),
                };
                if let Some(states) = states {
                    public_fleet_caught_up = Some(states);
                    continue;
                }
            }
            let error = format!("{label} strict live-value check requires `{field}=true`");
            nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
            return Err(error);
        }
    }
    if let Some(states) = public_fleet_caught_up {
        let value = serde_json::json!({
            "schema": "postfiat-nav-roundtrip-strict-public-fleet-caught-up-v1",
            "label": label,
            "end_height": report.end_height,
            "end_state_root": report.end_state_root,
            "validator_states": states,
        });
        let _ = write_json_file(
            &artifact_dir.join("strict-public-fleet-caught-up.json"),
            &value,
        );
    }
    Ok(())
}

fn nav_roundtrip_certified_ops_round_report_file(
    report: &CertifiedAssetOpsBatchReport,
) -> Result<std::path::PathBuf, String> {
    let artifact_dir = std::path::PathBuf::from(&report.artifact_dir);
    let candidates = [
        artifact_dir.join("peer-certified-mempool-round.report.json"),
        artifact_dir.join("peer-certified-batch-round.report.json"),
    ];
    for candidate in candidates {
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "expected `peer-certified-mempool-round.report.json` or `peer-certified-batch-round.report.json` under `{}`",
        artifact_dir.display()
    ))
}

fn nav_roundtrip_strict_public_fleet_caught_up(
    report: &CertifiedAssetOpsBatchReport,
) -> Result<Vec<NavRoundtripValidatorStateEvidence>, String> {
    let expected_height = report
        .end_height
        .ok_or_else(|| "strict public fleet catch-up check missing end_height".to_string())?;
    let expected_root = report
        .end_state_root
        .as_ref()
        .ok_or_else(|| "strict public fleet catch-up check missing end_state_root".to_string())?;
    let final_status = status(NodeOptions {
        data_dir: std::path::PathBuf::from(&report.data_dir),
    })
    .map_err(|error| format!("strict public fleet catch-up local status failed: {error}"))?;
    let states = nav_roundtrip_public_validator_states(
        &std::path::PathBuf::from(&report.topology_file),
        &final_status,
        30_000,
    )?;
    if states.is_empty() {
        return Err("strict public fleet catch-up found no validator states".to_string());
    }
    let operator_local_state =
        nav_roundtrip_validator_state_from_status(&final_status, "operator_local_state");
    let mut effective_states = states.clone();
    let mut replaced_public_self = false;
    for state in &mut effective_states {
        if state.node_id == final_status.node_id {
            *state = operator_local_state.clone();
            replaced_public_self = true;
        }
    }
    if !replaced_public_self {
        effective_states.push(operator_local_state);
    }
    let mismatched = effective_states
        .iter()
        .filter(|state| state.block_height != expected_height || state.state_root != *expected_root)
        .map(|state| {
            format!(
                "{}@{}/{}",
                state.node_id, state.block_height, state.state_root
            )
        })
        .collect::<Vec<_>>();
    if !mismatched.is_empty() {
        return Err(format!(
            "strict public fleet catch-up mismatch, expected {expected_height}/{expected_root}: {}",
            mismatched.join(", ")
        ));
    }
    Ok(effective_states)
}

fn nav_roundtrip_require_nav_checkpoint_ok(
    label: &str,
    report: &NavRoundtripNavCheckpointReport,
    artifact_dir: &std::path::Path,
) -> Result<(), String> {
    nav_roundtrip_require_certified_ops_ok(
        &format!("{label} reserve submit"),
        &report.submit_certified_ops,
        artifact_dir,
    )?;
    nav_roundtrip_require_certified_ops_ok(
        &format!("{label} epoch finalize"),
        &report.finalize_certified_ops,
        artifact_dir,
    )?;
    if report.delta_ok != Some(true) {
        let error = format!("{label} delta check failed: {:?}", report.failure_reasons);
        nav_roundtrip_write_failure_artifact(artifact_dir, label, &error);
        return Err(error);
    }
    Ok(())
}

fn nav_roundtrip_write_failure_artifact(
    artifact_dir: &std::path::Path,
    stage: &str,
    error: &str,
) {
    let _ = std::fs::create_dir_all(artifact_dir);
    let failure_file = artifact_dir.join("roundtrip-failure.json");
    let value = serde_json::json!({
        "schema": NAV_ROUNDTRIP_FAILURE_REPORT_SCHEMA,
        "stage": stage,
        "error": error,
    });
    let _ = write_json_file(&failure_file, &value);
}

fn nav_roundtrip_certified_round_validator_states(
    report: &CertifiedAssetOpsBatchReport,
) -> Result<Vec<NavRoundtripValidatorStateEvidence>, String> {
    let round_report_file = nav_roundtrip_certified_ops_round_report_file(report)?;
    let raw = std::fs::read_to_string(&round_report_file).map_err(|error| {
        format!(
            "failed to read final certified round report `{}`: {error}",
            round_report_file.display()
        )
    })?;
    let value = serde_json::from_str::<serde_json::Value>(&raw).map_err(|error| {
        format!(
            "failed to parse final certified round report `{}`: {error}",
            round_report_file.display()
        )
    })?;
    let round = value.get("round").unwrap_or(&value);
    let mut states = Vec::new();
    if let Some(local_state) = round.get("local_state") {
        states.push(nav_roundtrip_validator_state_from_json(
            local_state,
            "local_state",
        )?);
    }
    if let Some(sends) = round.get("sends").and_then(serde_json::Value::as_array) {
        for (index, send) in sends.iter().enumerate() {
            if let Some(state) = send
                .get("ack")
                .and_then(|ack| ack.get("state"))
            {
                states.push(nav_roundtrip_validator_state_from_json(
                    state,
                    &format!("sends[{index}].ack.state"),
                )?);
            }
        }
    }
    if states.is_empty() {
        return Err("final certified round report contained no validator states".to_string());
    }
    Ok(states)
}

fn nav_roundtrip_should_reuse_final_certified_state(
    reuse_final_certified_state: bool,
    background_audit: bool,
) -> bool {
    reuse_final_certified_state || background_audit
}

fn nav_roundtrip_final_audit_profile(
    reuse_final_certified_state: bool,
    background_audit: bool,
) -> String {
    if background_audit {
        "background_audit_certified_round_hot_path".to_string()
    } else if reuse_final_certified_state {
        "certified_round_hot_path".to_string()
    } else {
        "blocking_public_rpc".to_string()
    }
}

fn nav_roundtrip_write_background_audit_request(
    artifact_dir: &std::path::Path,
    roundtrip_summary_file: &std::path::Path,
    data_dir: &std::path::Path,
    topology_file: &std::path::Path,
    timeout_ms: u64,
    final_status: &StatusReport,
    final_validator_state_source: &str,
    certified_round_validator_states: &[NavRoundtripValidatorStateEvidence],
) -> Result<String, String> {
    let audit_dir = artifact_dir.join("background-audit");
    std::fs::create_dir_all(&audit_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip background audit dir `{}`: {error}",
            audit_dir.display()
        )
    })?;
    let audit_file = audit_dir.join("background-audit-request.json");
    let public_audit_artifact_dir = audit_dir.join("post-run-public-validator-audit");
    let request = NavRoundtripBackgroundAuditRequest {
        schema: NAV_ROUNDTRIP_BACKGROUND_AUDIT_REQUEST_SCHEMA.to_string(),
        artifact_file: audit_file.display().to_string(),
        roundtrip_summary_file: roundtrip_summary_file.display().to_string(),
        data_dir: data_dir.display().to_string(),
        topology_file: topology_file.display().to_string(),
        timeout_ms,
        final_height: final_status.block_height,
        final_state_root: final_status.state_root.clone(),
        final_mempool_pending: final_status.mempool_pending,
        final_validator_state_source: final_validator_state_source.to_string(),
        certified_round_validator_states: certified_round_validator_states.to_vec(),
        required_checks: vec![
            "collect public validator status after the hot-path run".to_string(),
            "verify height and state-root convergence across the public topology".to_string(),
            "compare public validator state to the roundtrip summary final_height/final_state_root"
                .to_string(),
            "preserve the generated audit artifact next to the roundtrip summary".to_string(),
        ],
        suggested_command: format!(
            "postfiat-node nav-roundtrip-live-demo --fleet-preflight-only --data-dir {} --topology {} --artifact-dir {} --timeout-ms {} --overwrite",
            data_dir.display(),
            topology_file.display(),
            public_audit_artifact_dir.display(),
            timeout_ms
        ),
    };
    write_json_file(&audit_file, &request)?;
    Ok(audit_file.display().to_string())
}

fn nav_roundtrip_select_final_validator_states(
    topology_file: &std::path::PathBuf,
    final_status: &StatusReport,
    timeout_ms: u64,
    certified_round_validator_states: &[NavRoundtripValidatorStateEvidence],
    reuse_final_certified_state: bool,
) -> Result<
    (
        Vec<NavRoundtripValidatorStateEvidence>,
        Vec<NavRoundtripValidatorStateEvidence>,
        String,
    ),
    String,
> {
    if reuse_final_certified_state {
        return Ok((
            Vec::new(),
            certified_round_validator_states.to_vec(),
            "certified_round".to_string(),
        ));
    }
    let public_validator_states =
        nav_roundtrip_public_validator_states(topology_file, final_status, timeout_ms)?;
    Ok((
        public_validator_states.clone(),
        public_validator_states,
        "public_validator_rpc".to_string(),
    ))
}

fn nav_roundtrip_live_fleet_preflight(
    data_dir: &std::path::PathBuf,
    topology_file: &std::path::PathBuf,
    artifact_dir: &std::path::Path,
    timeout_ms: u64,
    resume: bool,
    overwrite: bool,
    require_existing: bool,
) -> Result<NavRoundtripFleetPreflightReport, String> {
    std::fs::create_dir_all(artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip fleet preflight artifact dir `{}`: {error}",
            artifact_dir.display()
        )
    })?;
    let artifact_file = artifact_dir.join("fleet-preflight.json");
    if require_existing && !artifact_file.is_file() {
        return Err(format!(
            "fast demo preflight requires precomputed NAV roundtrip fleet preflight `{}`; run the generated fleet_preflight_command before the timed run",
            artifact_file.display()
        ));
    }
    if resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing NAV roundtrip fleet preflight `{}`: {error}",
                artifact_file.display()
            )
        })?;
        let mut report = serde_json::from_str::<NavRoundtripFleetPreflightReport>(&raw)
            .map_err(|error| {
                format!(
                    "existing NAV roundtrip fleet preflight `{}` is invalid: {error}",
                    artifact_file.display()
                )
            })?;
        report.reused_artifact = true;
        nav_roundtrip_validate_reused_fleet_preflight(data_dir, topology_file, &report)?;
        return Ok(report);
    }
    if artifact_file.exists() && !overwrite {
        return Err(format!(
            "NAV roundtrip fleet preflight `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("NAV roundtrip fleet preflight local status failed: {error}"))?;
    let operator_local_state =
        nav_roundtrip_validator_state_from_status(&local_status, "operator_local_state");
    let topology = read_topology_file(topology_file)?;
    let mut failure_reasons = Vec::new();
    let public_validator_states =
        match nav_roundtrip_public_validator_states(topology_file, &local_status, timeout_ms) {
            Ok(states) => states,
            Err(error) => {
                failure_reasons.push(error);
                Vec::new()
            }
        };
    let strict_public_validator_consensus_ok =
        nav_roundtrip_validator_states_consensus_ok(&public_validator_states);
    let nonlocal_public_validator_states = public_validator_states
        .iter()
        .filter(|state| state.node_id != local_status.node_id)
        .cloned()
        .collect::<Vec<_>>();
    let operator_matches_nonlocal_public_consensus = !nonlocal_public_validator_states.is_empty()
        && nav_roundtrip_validator_states_consensus_ok(&nonlocal_public_validator_states)
        && nonlocal_public_validator_states.iter().all(|state| {
            state.block_height == local_status.block_height
                && state.state_root == local_status.state_root
        });
    let public_validator_consensus_ok =
        strict_public_validator_consensus_ok || operator_matches_nonlocal_public_consensus;
    if !public_validator_consensus_ok {
        failure_reasons
            .push("public validator endpoint states do not agree on height/root".to_string());
    }
    let local_peer = topology.peer(&local_status.node_id);
    let local_node_id_in_topology = local_peer.is_some();
    let local_node_id_public_host = local_peer.map(|peer| peer.host.clone());
    let operator_matches_public_endpoint = public_validator_states
        .iter()
        .find(|state| state.node_id == local_status.node_id)
        .is_some_and(|state| {
            state.block_height == local_status.block_height
                && state.state_root == local_status.state_root
        });
    let operator_matches_public_quorum =
        operator_matches_public_endpoint || operator_matches_nonlocal_public_consensus;
    if let Some(peer) = local_peer {
        if !operator_matches_public_quorum {
            failure_reasons.push(format!(
                "local data dir node_id `{}` does not match its public validator endpoint state at {}:{}",
                local_status.node_id, peer.host, peer.rpc_port
            ));
        }
    }
    let preflight_ok = failure_reasons.is_empty();
    let report = NavRoundtripFleetPreflightReport {
        schema: NAV_ROUNDTRIP_FLEET_PREFLIGHT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        data_dir: data_dir.display().to_string(),
        topology_file: topology_file.display().to_string(),
        reused_artifact: false,
        operator_local_state,
        public_validator_states,
        local_node_id_in_topology,
        local_node_id_public_host,
        public_validator_consensus_ok,
        operator_matches_public_endpoint,
        operator_matches_public_quorum,
        preflight_ok,
        failure_reasons,
    };
    write_json_file(&artifact_file, &report).map_err(|error| {
        format!(
            "failed to write NAV roundtrip fleet preflight report `{}`: {error}",
            artifact_file.display()
        )
    })?;
    if !report.preflight_ok {
        return Err(format!(
            "NAV roundtrip fleet preflight failed; see `{}`: {}",
            artifact_file.display(),
            report.failure_reasons.join("; ")
        ));
    }
    Ok(report)
}

fn nav_roundtrip_validate_reused_fleet_preflight(
    data_dir: &std::path::PathBuf,
    topology_file: &std::path::PathBuf,
    report: &NavRoundtripFleetPreflightReport,
) -> Result<(), String> {
    if report.schema != NAV_ROUNDTRIP_FLEET_PREFLIGHT_REPORT_SCHEMA {
        return Err(format!(
            "reused NAV roundtrip fleet preflight has unsupported schema `{}`",
            report.schema
        ));
    }
    if report.data_dir != data_dir.display().to_string() {
        return Err(format!(
            "reused NAV roundtrip fleet preflight data_dir `{}` does not match `{}`",
            report.data_dir,
            data_dir.display()
        ));
    }
    if report.topology_file != topology_file.display().to_string() {
        return Err(format!(
            "reused NAV roundtrip fleet preflight topology_file `{}` does not match `{}`",
            report.topology_file,
            topology_file.display()
        ));
    }
    if !report.preflight_ok {
        return Err(format!(
            "reused NAV roundtrip fleet preflight is not green: {:?}",
            report.failure_reasons
        ));
    }
    if report.public_validator_states.is_empty() {
        return Err("reused NAV roundtrip fleet preflight has no public validator evidence".to_string());
    }
    if !report.public_validator_consensus_ok {
        return Err("reused NAV roundtrip fleet preflight did not prove public validator consensus".to_string());
    }
    if !report.operator_matches_public_endpoint && !report.operator_matches_public_quorum {
        return Err("reused NAV roundtrip fleet preflight did not prove operator/public endpoint or quorum match".to_string());
    }
    let local_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(|error| format!("NAV roundtrip reused fleet preflight local status failed: {error}"))?;
    let current_local_state =
        nav_roundtrip_validator_state_from_status(&local_status, "operator_local_state");
    if report.operator_local_state.node_id != current_local_state.node_id
        || report.operator_local_state.block_height != current_local_state.block_height
        || report.operator_local_state.state_root != current_local_state.state_root
    {
        return Err(format!(
            "reused NAV roundtrip fleet preflight local state {}/{}/{} does not match current local status {}/{}/{}",
            report.operator_local_state.node_id,
            report.operator_local_state.block_height,
            report.operator_local_state.state_root,
            current_local_state.node_id,
            current_local_state.block_height,
            current_local_state.state_root
        ));
    }
    Ok(())
}

fn nav_roundtrip_public_validator_states(
    topology_file: &std::path::PathBuf,
    final_status: &StatusReport,
    timeout_ms: u64,
) -> Result<Vec<NavRoundtripValidatorStateEvidence>, String> {
    let topology = read_topology_file(topology_file)?;
    if topology.chain_id != final_status.chain_id
        || topology.genesis_hash != final_status.genesis_hash
        || topology.protocol_version != final_status.protocol_version
    {
        return Err("public validator topology domain does not match final local status".to_string());
    }
    let mut states = Vec::with_capacity(topology.peers.len());
    for peer in &topology.peers {
        let status = nav_roundtrip_rpc_status(peer, timeout_ms)?;
        if status.node_id != peer.node_id {
            return Err(format!(
                "public validator `{}` RPC reported node_id `{}`",
                peer.node_id, status.node_id
            ));
        }
        if status.chain_id != topology.chain_id
            || status.genesis_hash != topology.genesis_hash
            || status.protocol_version != topology.protocol_version
        {
            return Err(format!(
                "public validator `{}` RPC domain does not match topology",
                peer.node_id
            ));
        }
        states.push(nav_roundtrip_validator_state_from_status(
            &status,
            &format!("rpc://{}:{}", peer.host, peer.rpc_port),
        ));
    }
    Ok(states)
}

fn nav_roundtrip_rpc_status(
    peer: &postfiat_network::PeerInfo,
    timeout_ms: u64,
) -> Result<StatusReport, String> {
    let id = format!("nav-roundtrip-public-status-{}", peer.node_id);
    let request = RpcRequest::new(&id, "status", serde_json::json!({}));
    let mut stream = TcpStream::connect((peer.host.as_str(), peer.rpc_port)).map_err(|error| {
        format!(
            "public validator `{}` RPC connect {}:{} failed: {error}",
            peer.node_id, peer.host, peer.rpc_port
        )
    })?;
    set_stream_timeout(&stream, timeout_ms)?;
    write_json_line(&mut stream, &request)?;
    let line = read_transport_line(&stream, "public validator status response read")?;
    let response: RpcResponse = serde_json::from_str(&line)
        .map_err(|error| format!("public validator status response parse failed: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("public validator status response protocol failed: {error}"))?;
    if response.id != id {
        return Err(format!(
            "public validator `{}` status response id `{}` did not match `{id}`",
            peer.node_id, response.id
        ));
    }
    response
        .result_as::<StatusReport>()
        .map_err(|error| format!("public validator `{}` status failed: {error}", peer.node_id))
}

fn nav_roundtrip_validator_state_from_status(
    status: &StatusReport,
    source: &str,
) -> NavRoundtripValidatorStateEvidence {
    NavRoundtripValidatorStateEvidence {
        node_id: status.node_id.clone(),
        block_height: status.block_height,
        state_root: status.state_root.clone(),
        source: source.to_string(),
    }
}

fn nav_roundtrip_validator_state_from_json(
    state: &serde_json::Value,
    source: &str,
) -> Result<NavRoundtripValidatorStateEvidence, String> {
    let node_id = state
        .get("node_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{source} missing node_id"))?;
    let block_height = state
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("{source} missing block_height"))?;
    let state_root = state
        .get("state_root")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{source} missing state_root"))?;
    Ok(NavRoundtripValidatorStateEvidence {
        node_id: node_id.to_string(),
        block_height,
        state_root: state_root.to_string(),
        source: source.to_string(),
    })
}

fn nav_roundtrip_validator_states_consensus_ok(
    states: &[NavRoundtripValidatorStateEvidence],
) -> bool {
    let Some(first) = states.first() else {
        return false;
    };
    states.iter().all(|state| {
        state.block_height == first.block_height && state.state_root == first.state_root
    })
}

fn certified_asset_ops_batch(options: CertifiedAssetOpsBatchOptions) -> Result<CertifiedAssetOpsBatchReport, String> {
    let total_start = std::time::Instant::now();
    let summary_file = options.artifact_dir.join("summary.json");
    if options.resume && summary_file.is_file() {
        let raw = std::fs::read_to_string(&summary_file).map_err(|error| {
            format!(
                "failed to read existing summary `{}`: {error}",
                summary_file.display()
            )
        })?;
        let report = serde_json::from_str::<CertifiedAssetOpsBatchReport>(&raw).map_err(|error| {
            format!(
                "existing summary `{}` is not a certified asset ops report: {error}",
                summary_file.display()
            )
        })?;
        return Ok(report);
    }
    prepare_artifact_dir(&options.artifact_dir, options.overwrite, options.resume)?;

    let preflight_start = std::time::Instant::now();
    if options.prepare_only && options.batch_only {
        return Err("--prepare-only and --batch-only cannot be used together".to_string());
    }
    let request = read_certified_asset_ops_request(&options.ops_file)?;
    validate_certified_asset_ops_request(&request)?;
    let dependency_report = certified_asset_ops_dependency_report(&request);
    let start_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("certified asset ops preflight status failed: {error}"))?;
    if start_status.mempool_pending != 0 && !options.allow_existing_mempool {
        return Err(format!(
            "mempool has {} pending transactions; rerun with --allow-existing-mempool only after confirming they belong in this batch",
            start_status.mempool_pending
        ));
    }
    let preflight_ms = monotonic_elapsed_ms(preflight_start);

    write_json_file(
        &options.artifact_dir.join("request.normalized.json"),
        &request_to_json(&request)?,
    )?;

    let operations_start = std::time::Instant::now();
    let mut operation_reports = Vec::new();
    let mut next_sequences = std::collections::BTreeMap::<String, u64>::new();
    for op in &request.operations {
        let sequence_override = next_sequences.get(&op.source).copied();
        let report = run_certified_asset_op_stage(op, &options, true, sequence_override)?;
        if let Some(sequence) = report.sequence {
            let next_sequence = sequence.checked_add(1).ok_or_else(|| {
                format!(
                    "certified asset ops sequence overflow after `{}` from `{}`",
                    op.label, op.source
                )
            })?;
            next_sequences.insert(op.source.clone(), next_sequence);
        }
        operation_reports.push(report);
    }
    let operations_ms = monotonic_elapsed_ms(operations_start);

    let max_transactions = options
        .max_transactions
        .unwrap_or(request.operations.len());
    if max_transactions < request.operations.len() {
        return Err(format!(
            "--max-transactions {max_transactions} is smaller than operation count {}",
            request.operations.len()
        ));
    }

    let mut batch_file = None;
    let mut round_artifact_dir = None;
    let mut round_ok = None;
    let certify_start = std::time::Instant::now();
    if options.batch_only {
        let batch_path = options.artifact_dir.join("mempool-batch.json");
        create_mempool_batch(MempoolBatchOptions {
            data_dir: options.data_dir.clone(),
            batch_file: batch_path.clone(),
            max_transactions,
        })
        .map_err(|error| format!("certified asset ops batch create failed: {error}"))?;
        batch_file = Some(batch_path.display().to_string());
    } else if !options.prepare_only {
        let round_dir = options.artifact_dir.join("peer-certified-mempool-round");
        let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
            data_dir: options.data_dir.clone(),
            topology_file: options.topology_file.clone(),
            key_file: options.key_file.clone(),
            proposal_key_file: options.proposal_key_file.clone().or_else(|| Some(options.key_file.clone())),
            require_local_proposer: options.require_local_proposer,
            require_signed_proposal: options.require_signed_proposal,
            allow_peer_failures: options.allow_peer_failures,
            quorum_early_full_propagation: options.quorum_early_full_propagation,
            artifact_dir: round_dir.clone(),
            block_height: options.block_height,
            view: options.view,
            timeout_certificate_file: options.timeout_certificate_file.clone(),
            timeout_ms: options.timeout_ms,
            send_retries: options.send_retries,
            retry_backoff_ms: options.retry_backoff_ms,
            local_apply_before_certified_send: options.local_apply_before_certified_send,
            defer_certified_sends: options.defer_certified_sends,
            required_parent: None,
            max_transactions,
            signed_transfer_file: None,
            signed_transfer_json: None,
            signed_payment_v2_json: None,
            signed_asset_transaction_json: None,
            signed_atomic_swap_transaction_json: None,
            signed_escrow_transaction_json: None,
        })?;
        let round_report_file = options.artifact_dir.join("peer-certified-mempool-round.report.json");
        write_json_file(&round_report_file, &round)?;
        batch_file = Some(round.batch_file.clone());
        round_artifact_dir = Some(round.artifact_dir.clone());
        round_ok = Some(round.round_ok);
    }
    let certify_ms = monotonic_elapsed_ms(certify_start);

    let final_status_start = std::time::Instant::now();
    let end_status = if options.prepare_only {
        None
    } else {
        Some(
            status(NodeOptions {
                data_dir: options.data_dir.clone(),
            })
            .map_err(|error| format!("certified asset ops final status failed: {error}"))?,
        )
    };
    let final_status_ms = monotonic_elapsed_ms(final_status_start);

    let report = CertifiedAssetOpsBatchReport {
        schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
        request_schema: request.schema,
        data_dir: options.data_dir.display().to_string(),
        topology_file: options.topology_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        operation_count: operation_reports.len(),
        max_transactions,
        allow_existing_mempool: options.allow_existing_mempool,
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
        start_height: start_status.block_height,
        start_state_root: start_status.state_root,
        start_mempool_pending: start_status.mempool_pending,
        end_height: end_status.as_ref().map(|status| status.block_height),
        end_state_root: end_status.as_ref().map(|status| status.state_root.clone()),
        end_mempool_pending: end_status.as_ref().map(|status| status.mempool_pending),
        operations: operation_reports,
        dependency_report,
        batch_file,
        round_artifact_dir,
        round_ok,
        timings_ms: CertifiedAssetOpsTimingsReport {
            total_ms: monotonic_elapsed_ms(total_start),
            preflight_ms,
            operations_ms,
            certify_ms,
            final_status_ms,
        },
    };
    write_json_file(&summary_file, &report)?;
    Ok(report)
}

fn certified_asset_ops_direct_batch(
    options: CertifiedAssetOpsBatchOptions,
) -> Result<CertifiedAssetOpsBatchReport, String> {
    let total_start = std::time::Instant::now();
    let summary_file = options.artifact_dir.join("summary.json");
    if options.resume && summary_file.is_file() {
        let raw = std::fs::read_to_string(&summary_file).map_err(|error| {
            format!(
                "failed to read existing summary `{}`: {error}",
                summary_file.display()
            )
        })?;
        let report = serde_json::from_str::<CertifiedAssetOpsBatchReport>(&raw).map_err(|error| {
            format!(
                "existing summary `{}` is not a certified asset ops report: {error}",
                summary_file.display()
            )
        })?;
        return Ok(report);
    }
    if options.prepare_only || options.batch_only {
        return Err("direct certified asset ops batch is live-only".to_string());
    }
    prepare_artifact_dir(&options.artifact_dir, options.overwrite, options.resume)?;

    let preflight_start = std::time::Instant::now();
    let request = read_certified_asset_ops_request(&options.ops_file)?;
    validate_certified_asset_ops_request(&request)?;
    let dependency_report = certified_asset_ops_dependency_report(&request);
    let start_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("direct certified asset ops preflight status failed: {error}"))?;
    if start_status.mempool_pending != 0 && !options.allow_existing_mempool {
        return Err(format!(
            "mempool has {} pending transactions; rerun with --allow-existing-mempool only after confirming they are unrelated to this direct batch",
            start_status.mempool_pending
        ));
    }
    let preflight_ms = monotonic_elapsed_ms(preflight_start);

    write_json_file(
        &options.artifact_dir.join("request.normalized.json"),
        &request_to_json(&request)?,
    )?;

    let operations_start = std::time::Instant::now();
    let mut operation_reports = Vec::new();
    let mut next_sequences = std::collections::BTreeMap::<String, u64>::new();
    for op in &request.operations {
        let sequence_override = next_sequences.get(&op.source).copied();
        let report = run_certified_asset_op_stage(op, &options, false, sequence_override)?;
        if let Some(sequence) = report.sequence {
            let next_sequence = sequence.checked_add(1).ok_or_else(|| {
                format!(
                    "certified asset ops sequence overflow after `{}` from `{}`",
                    op.label, op.source
                )
            })?;
            next_sequences.insert(op.source.clone(), next_sequence);
        }
        operation_reports.push(report);
    }
    let operations_ms = monotonic_elapsed_ms(operations_start);

    let max_transactions = options
        .max_transactions
        .unwrap_or(request.operations.len());
    if max_transactions < request.operations.len() {
        return Err(format!(
            "--max-transactions {max_transactions} is smaller than operation count {}",
            request.operations.len()
        ));
    }

    let certify_start = std::time::Instant::now();
    let signed_files = operation_reports
        .iter()
        .map(|report| {
            report
                .signed_file
                .as_ref()
                .map(std::path::PathBuf::from)
                .ok_or_else(|| format!("operation `{}` did not produce a signed file", report.label))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let batch_path = options.artifact_dir.join("signed-asset-batch.json");
    create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
        data_dir: options.data_dir.clone(),
        batch_file: batch_path.clone(),
        signed_asset_transaction_files: signed_files,
    })
    .map_err(|error| format!("direct certified asset ops batch create failed: {error}"))?;
    let round_dir = options.artifact_dir.join("peer-certified-batch-round");
    let round = transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        batch_kind: Some("transparent".to_string()),
        batch_file: batch_path.clone(),
        key_file: options.key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone().or_else(|| Some(options.key_file.clone())),
        require_local_proposer: options.require_local_proposer,
        require_signed_proposal: options.require_signed_proposal,
        allow_peer_failures: options.allow_peer_failures,
        quorum_early_full_propagation: options.quorum_early_full_propagation,
        artifact_dir: round_dir.clone(),
        block_height: options.block_height,
        view: options.view,
        timeout_certificate_file: options.timeout_certificate_file.clone(),
        timeout_ms: options.timeout_ms,
        send_retries: options.send_retries,
        retry_backoff_ms: options.retry_backoff_ms,
        local_apply_before_certified_send: options.local_apply_before_certified_send,
        defer_certified_sends: options.defer_certified_sends,
        required_parent: None,
    })?;
    let round_report_file = options.artifact_dir.join("peer-certified-batch-round.report.json");
    write_json_file(&round_report_file, &round)?;
    let certify_ms = monotonic_elapsed_ms(certify_start);

    let final_status_start = std::time::Instant::now();
    let end_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("direct certified asset ops final status failed: {error}"))?;
    let final_status_ms = monotonic_elapsed_ms(final_status_start);

    let report = CertifiedAssetOpsBatchReport {
        schema: CERTIFIED_ASSET_OPS_REPORT_SCHEMA.to_string(),
        request_schema: request.schema,
        data_dir: options.data_dir.display().to_string(),
        topology_file: options.topology_file.display().to_string(),
        artifact_dir: options.artifact_dir.display().to_string(),
        operation_count: operation_reports.len(),
        max_transactions,
        allow_existing_mempool: options.allow_existing_mempool,
        prepare_only: false,
        batch_only: false,
        start_height: start_status.block_height,
        start_state_root: start_status.state_root,
        start_mempool_pending: start_status.mempool_pending,
        end_height: Some(end_status.block_height),
        end_state_root: Some(end_status.state_root),
        end_mempool_pending: Some(end_status.mempool_pending),
        operations: operation_reports,
        dependency_report,
        batch_file: Some(batch_path.display().to_string()),
        round_artifact_dir: Some(round.artifact_dir.clone()),
        round_ok: Some(round.round_ok),
        timings_ms: CertifiedAssetOpsTimingsReport {
            total_ms: monotonic_elapsed_ms(total_start),
            preflight_ms,
            operations_ms,
            certify_ms,
            final_status_ms,
        },
    };
    write_json_file(&summary_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_preflight(
    options: NavRoundtripPreflightOptions,
) -> Result<NavRoundtripPreflightReport, String> {
    let artifact_file = options.artifact_dir.join("preflight.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing preflight artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripPreflightReport>(&raw).map_err(|error| {
            format!(
                "existing preflight artifact `{}` is not a NAV roundtrip preflight report: {error}",
                artifact_file.display()
            )
        });
    }
    prepare_nav_roundtrip_artifact_dir(&options.artifact_dir, options.overwrite)?;
    let start_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("NAV roundtrip preflight PFTL status failed: {error}"))?;

    let wallet_usdc_atoms = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.stakehub_wallet.as_str()],
    )?;
    let vault_usdc_atoms = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.vault_address.as_str()],
    )?;
    let usdc_allowance_atoms = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "allowance(address,address)(uint256)",
        &[
            options.stakehub_wallet.as_str(),
            options.vault_address.as_str(),
        ],
    )?;
    let wallet_gas_wei = cast_balance(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.stakehub_wallet,
    )?;
    let vault_code_bytes = cast_code_bytes(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
    )?;
    let verifier_code_bytes = cast_code_bytes(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.verifier_address,
    )?;
    let vault_challenge_delay_seconds = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        "challenge_delay()(uint64)",
        &[],
    )?;
    let vault_execution_window_seconds = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        "execution_window()(uint64)",
        &[],
    )?;
    let verifier_challenge_delay_seconds = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.verifier_address,
        "challenge_delay()(uint64)",
        &[],
    )?;
    let verifier_execution_window_seconds = cast_optional_u64_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.verifier_address,
        "execution_window()(uint64)",
        &[],
    )?;
    let bridge_abi = classify_nav_roundtrip_vault_abi(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.vault_address,
        &options.usdc_address,
        &options.stakehub_wallet,
    )?;

    let mut failure_reasons = Vec::new();
    if wallet_usdc_atoms < u128::from(options.amount_atoms) {
        failure_reasons.push(format!(
            "stakehub wallet has {} USDC atoms, needs {}",
            wallet_usdc_atoms, options.amount_atoms
        ));
    }
    if wallet_gas_wei < options.min_gas_wei {
        failure_reasons.push(format!(
            "stakehub wallet gas balance {} wei is below minimum {} wei",
            wallet_gas_wei, options.min_gas_wei
        ));
    }
    if vault_code_bytes == 0 {
        failure_reasons.push("vault address has no deployed code".to_string());
    }
    if verifier_code_bytes == 0 {
        failure_reasons.push("withdrawal verifier address has no deployed code".to_string());
    }
    if bridge_abi.bridge_class == NAV_ROUNDTRIP_BRIDGE_CLASS_UNKNOWN {
        failure_reasons.push("vault withdrawal ABI is unknown".to_string());
    }

    let source_rpc_provider_class =
        nav_roundtrip_source_rpc_provider_class(&options.source_rpc_url);
    let report = NavRoundtripPreflightReport {
        schema: NAV_ROUNDTRIP_PREFLIGHT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        source_rpc_url: options.source_rpc_url,
        source_rpc_provider_class,
        vault_address: options.vault_address,
        verifier_address: options.verifier_address,
        usdc_address: options.usdc_address,
        stakehub_wallet: options.stakehub_wallet,
        amount_atoms: options.amount_atoms,
        min_gas_wei: options.min_gas_wei.to_string(),
        start_height: start_status.block_height,
        start_state_root: start_status.state_root,
        start_mempool_pending: start_status.mempool_pending,
        wallet_usdc_atoms: wallet_usdc_atoms.to_string(),
        vault_usdc_atoms: vault_usdc_atoms.to_string(),
        usdc_allowance_atoms: Some(usdc_allowance_atoms.to_string()),
        wallet_gas_wei: wallet_gas_wei.to_string(),
        vault_code_bytes,
        verifier_code_bytes,
        vault_challenge_delay_seconds,
        vault_execution_window_seconds,
        verifier_challenge_delay_seconds,
        verifier_execution_window_seconds,
        bridge_class: bridge_abi.bridge_class,
        withdrawal_digest_signature: bridge_abi.withdrawal_digest_signature,
        submit_withdrawal_signature: bridge_abi.submit_withdrawal_signature,
        preflight_ok: failure_reasons.is_empty(),
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_warm_usdc_allowance(
    options: NavRoundtripUsdcAllowanceSetupOptions,
) -> Result<NavRoundtripUsdcAllowanceSetupReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip allowance setup artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("allowance-setup.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing USDC allowance setup artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripUsdcAllowanceSetupReport>(&raw).map_err(
            |error| {
                format!(
                    "existing USDC allowance setup artifact `{}` is invalid: {error}",
                    artifact_file.display()
                )
            },
        );
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "USDC allowance setup artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let allowance_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "allowance(address,address)(uint256)",
        &[
            options.stakehub_wallet.as_str(),
            options.vault_address.as_str(),
        ],
    )?;
    let approve_data = cast_calldata(
        &options.cast_binary,
        "approve(address,uint256)",
        &[
            options.vault_address.as_str(),
            &options.required_allowance_atoms.to_string(),
        ],
    )?;
    let approve_calldata_file = options.artifact_dir.join("approve.calldata.txt");
    write_text_file(&approve_calldata_file, &approve_data)?;

    let source_rpc_provider_class =
        nav_roundtrip_source_rpc_provider_class(&options.source_rpc_url);
    let mut receipt_watches = Vec::new();
    let mut stakehub_launch_session_open_file = None;
    let mut stakehub_launch_session_close_file = None;
    let agent_approve_file = options.artifact_dir.join("agent-approve.json");
    let required = u128::from(options.required_allowance_atoms);
    let approve_skipped = allowance_before >= required;
    let approve_response = if approve_skipped {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "existing_allowance_sufficient",
            "allowance_atoms": allowance_before.to_string(),
            "required_atoms": options.required_allowance_atoms.to_string(),
        })
    } else {
        let mut launch_session = NavRoundtripStakeHubLaunchSessionGuard::open(
            &options.stakehub_home,
            &options.artifact_dir.join("stakehub-launch-session"),
            &options.session_id,
            options.source_chain_id,
            &options.stakehub_wallet,
            &options.usdc_address,
            &options.vault_address,
            &options.verifier_address,
            options.required_allowance_atoms,
            options.agent_timeout_secs,
        )?;
        stakehub_launch_session_open_file =
            Some(launch_session.open_file.display().to_string());
        stakehub_launch_session_close_file =
            Some(launch_session.close_file.display().to_string());
        let approve_start = std::time::Instant::now();
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "evm_contract_tx",
                "to": options.usdc_address,
                "data": approve_data,
                "rpc_url": options.source_rpc_url,
                "chain_id": options.source_chain_id,
                "label": "NAV roundtrip warm pfUSDC vault allowance",
                "session_id": options.session_id,
                "session_action": "approve_pfusdc_vault",
                "gas_usd": 10,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "warm USDC allowance")?;
        receipt_watches.push(nav_roundtrip_evm_receipt_watch(
            "approve_pfusdc_vault",
            &response,
            &source_rpc_provider_class,
            monotonic_elapsed_ms(approve_start),
        )?);
        launch_session.close()?;
        response
    };
    write_json_file(&agent_approve_file, &approve_response)?;

    let allowance_after = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "allowance(address,address)(uint256)",
        &[
            options.stakehub_wallet.as_str(),
            options.vault_address.as_str(),
        ],
    )?;
    let mut failure_reasons = Vec::new();
    if allowance_after < required {
        failure_reasons.push(format!(
            "USDC allowance after setup is {} atoms, below required {} atoms",
            allowance_after, options.required_allowance_atoms
        ));
    }
    let allowance_ok = failure_reasons.is_empty();
    let report = NavRoundtripUsdcAllowanceSetupReport {
        schema: NAV_ROUNDTRIP_USDC_ALLOWANCE_SETUP_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        source_rpc_url: options.source_rpc_url,
        source_rpc_provider_class,
        source_chain_id: options.source_chain_id,
        vault_address: options.vault_address,
        verifier_address: options.verifier_address,
        usdc_address: options.usdc_address,
        stakehub_wallet: options.stakehub_wallet,
        required_allowance_atoms: options.required_allowance_atoms.to_string(),
        session_id: options.session_id,
        allowance_before_atoms: allowance_before.to_string(),
        allowance_after_atoms: allowance_after.to_string(),
        approve_skipped,
        approve_tx: if approve_skipped {
            None
        } else {
            Some(agent_tx_hash(&approve_response, "warm USDC allowance")?)
        },
        approve_gas_used: if approve_skipped {
            None
        } else {
            Some(agent_gas_used(&approve_response, "warm USDC allowance")?)
        },
        approve_calldata_file: approve_calldata_file.display().to_string(),
        agent_approve_file: agent_approve_file.display().to_string(),
        stakehub_launch_session_open_file,
        stakehub_launch_session_close_file,
        receipt_watches,
        allowance_ok,
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    if !report.allowance_ok {
        return Err(format!(
            "NAV roundtrip USDC allowance setup failed: {:?}",
            report.failure_reasons
        ));
    }
    Ok(report)
}

fn nav_roundtrip_live_demo_evm_deposit(
    options: NavRoundtripEvmDepositOptions,
) -> Result<NavRoundtripEvmDepositReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("evm-deposit.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing EVM deposit artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripEvmDepositReport>(&raw).map_err(|error| {
            format!(
                "existing EVM deposit artifact `{}` is not a NAV roundtrip deposit report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "EVM deposit artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let status_response = stakehub_agent_call(
        &options.stakehub_home,
        &serde_json::json!({ "op": "status" }),
        options.agent_timeout_secs,
    )?;
    require_agent_ok(&status_response, "status")?;
    if status_response
        .get("unlocked")
        .and_then(serde_json::Value::as_bool)
        != Some(true)
    {
        return Err("StakeHub agent is locked; run `stakehub agent unlock` first".to_string());
    }

    let wallet_usdc_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.stakehub_wallet.as_str()],
    )?;
    let vault_usdc_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.vault_address.as_str()],
    )?;
    let allowance_before = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "allowance(address,address)(uint256)",
        &[
            options.stakehub_wallet.as_str(),
            options.vault_address.as_str(),
        ],
    )?;
    if wallet_usdc_before < u128::from(options.amount_atoms) {
        return Err(format!(
            "stakehub wallet has {} USDC atoms, needs {}",
            wallet_usdc_before, options.amount_atoms
        ));
    }
    if options.require_warm_allowance && allowance_before < u128::from(options.amount_atoms) {
        return Err(format!(
            "EVM deposit requires warm USDC allowance: allowance {} atoms, required {} atoms",
            allowance_before, options.amount_atoms
        ));
    }

    let approve_data = cast_calldata(
        &options.cast_binary,
        "approve(address,uint256)",
        &[options.vault_address.as_str(), &options.amount_atoms.to_string()],
    )?;
    let deposit_data = cast_calldata(
        &options.cast_binary,
        "deposit(uint256,string,bytes32)",
        &[
            &options.amount_atoms.to_string(),
            options.pftl_recipient.as_str(),
            options.nonce.as_str(),
        ],
    )?;
    let approve_calldata_file = options.artifact_dir.join("approve.calldata.txt");
    let deposit_calldata_file = options.artifact_dir.join("deposit.calldata.txt");
    write_text_file(&approve_calldata_file, &approve_data)?;
    write_text_file(&deposit_calldata_file, &deposit_data)?;

    let agent_open_session_file = options.artifact_dir.join("agent-open-session.json");
    let open_response = if options.launch_session_managed_externally {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "launch_session_managed_by_full_runner",
            "session_id": options.session_id,
        })
    } else {
        let close_existing = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": options.session_id,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&close_existing, "close existing launch session")?;

        let open_request = serde_json::json!({
            "op": "open_launch_session",
            "session_id": options.session_id,
            "chain_id": options.source_chain_id,
            "allowlist": [
                options.stakehub_wallet,
                options.usdc_address,
                options.vault_address,
            ],
            "expected_deploys": [{
                "label": "nav_roundtrip_noop_deploy",
                "bytecode_hash": "0x0000000000000000000000000000000000000000000000000000000000000000",
                "bytecode_len": 1,
            }],
            "usdc_address": options.usdc_address,
            "usdc_budget": options.amount_atoms,
            "close_after_action": "deposit_pfusdc_vault",
            "ttl_seconds": 900,
        });
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &open_request,
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "open launch session")?;
        response
    };
    write_json_file(&agent_open_session_file, &open_response)?;

    let source_rpc_provider_class =
        nav_roundtrip_source_rpc_provider_class(&options.source_rpc_url);
    let mut receipt_watches = Vec::new();
    let agent_approve_file = options.artifact_dir.join("agent-approve.json");
    let approve_skipped = allowance_before >= u128::from(options.amount_atoms);
    let approve_response = if approve_skipped {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "existing_allowance_sufficient",
            "allowance_atoms": allowance_before.to_string(),
            "required_atoms": options.amount_atoms.to_string(),
        })
    } else {
        let approve_start = std::time::Instant::now();
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "evm_contract_tx",
                "to": options.usdc_address,
                "data": approve_data,
                "rpc_url": options.source_rpc_url,
                "chain_id": options.source_chain_id,
                "label": "NAV roundtrip approve pfUSDC vault",
                "session_id": options.session_id,
                "session_action": "approve_pfusdc_vault",
                "gas_usd": 10,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "approve USDC")?;
        receipt_watches.push(nav_roundtrip_evm_receipt_watch(
            "approve_pfusdc_vault",
            &response,
            &source_rpc_provider_class,
            monotonic_elapsed_ms(approve_start),
        )?);
        response
    };
    write_json_file(&agent_approve_file, &approve_response)?;

    let deposit_start = std::time::Instant::now();
    let deposit_response = stakehub_agent_call(
        &options.stakehub_home,
        &serde_json::json!({
            "op": "evm_contract_tx",
            "to": options.vault_address,
            "data": deposit_data,
            "rpc_url": options.source_rpc_url,
            "chain_id": options.source_chain_id,
            "label": "NAV roundtrip deposit pfUSDC vault",
            "session_id": options.session_id,
            "session_action": "deposit_pfusdc_vault",
            "gas_usd": 10,
        }),
        options.agent_timeout_secs,
    )?;
    require_agent_ok(&deposit_response, "deposit USDC into vault")?;
    receipt_watches.push(nav_roundtrip_evm_receipt_watch(
        "deposit_pfusdc_vault",
        &deposit_response,
        &source_rpc_provider_class,
        monotonic_elapsed_ms(deposit_start),
    )?);
    let agent_deposit_file = options.artifact_dir.join("agent-deposit.json");
    write_json_file(&agent_deposit_file, &deposit_response)?;

    let agent_close_session_file = options.artifact_dir.join("agent-close-session.json");
    let close_response = if options.launch_session_managed_externally {
        serde_json::json!({
            "ok": true,
            "skipped": true,
            "reason": "launch_session_managed_by_full_runner",
            "session_id": options.session_id,
        })
    } else {
        let response = stakehub_agent_call(
            &options.stakehub_home,
            &serde_json::json!({
                "op": "close_launch_session",
                "session_id": options.session_id,
            }),
            options.agent_timeout_secs,
        )?;
        require_agent_ok(&response, "close launch session")?;
        response
    };
    write_json_file(&agent_close_session_file, &close_response)?;

    let wallet_usdc_after = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.stakehub_wallet.as_str()],
    )?;
    let vault_usdc_after = cast_u128_call(
        &options.cast_binary,
        &options.source_rpc_url,
        &options.usdc_address,
        "balanceOf(address)(uint256)",
        &[options.vault_address.as_str()],
    )?;
    let mut failure_reasons = Vec::new();
    let amount = u128::from(options.amount_atoms);
    let wallet_delta = wallet_usdc_before.saturating_sub(wallet_usdc_after);
    let vault_delta = vault_usdc_after.saturating_sub(vault_usdc_before);
    let mut delta_warnings = Vec::new();
    if wallet_delta != amount {
        let warning = format!(
            "wallet USDC delta was {}, expected {}",
            wallet_delta,
            amount
        );
        if vault_delta == amount {
            delta_warnings.push(warning);
        } else {
            failure_reasons.push(warning);
        }
    }
    if vault_delta != amount {
        failure_reasons.push(format!(
            "vault USDC delta was {}, expected {}",
            vault_delta,
            amount
        ));
    }

    let report = NavRoundtripEvmDepositReport {
        schema: NAV_ROUNDTRIP_EVM_DEPOSIT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        source_rpc_url: options.source_rpc_url,
        source_rpc_provider_class,
        source_chain_id: options.source_chain_id,
        vault_address: options.vault_address,
        usdc_address: options.usdc_address,
        stakehub_wallet: options.stakehub_wallet,
        pftl_recipient: options.pftl_recipient,
        amount_atoms: options.amount_atoms,
        nonce: options.nonce,
        session_id: options.session_id,
        wallet_usdc_before_atoms: wallet_usdc_before.to_string(),
        wallet_usdc_after_atoms: wallet_usdc_after.to_string(),
        vault_usdc_before_atoms: vault_usdc_before.to_string(),
        vault_usdc_after_atoms: vault_usdc_after.to_string(),
        launch_session_managed_externally: options.launch_session_managed_externally,
        allowance_before_atoms: Some(allowance_before.to_string()),
        approve_skipped,
        approve_tx: if approve_skipped {
            None
        } else {
            Some(agent_tx_hash(&approve_response, "approve USDC")?)
        },
        approve_gas_used: if approve_skipped {
            None
        } else {
            Some(agent_gas_used(&approve_response, "approve USDC")?)
        },
        deposit_tx: agent_tx_hash(&deposit_response, "deposit USDC")?,
        deposit_gas_used: agent_gas_used(&deposit_response, "deposit USDC")?,
        approve_calldata_file: approve_calldata_file.display().to_string(),
        deposit_calldata_file: deposit_calldata_file.display().to_string(),
        agent_open_session_file: agent_open_session_file.display().to_string(),
        agent_approve_file: agent_approve_file.display().to_string(),
        agent_deposit_file: agent_deposit_file.display().to_string(),
        agent_close_session_file: agent_close_session_file.display().to_string(),
        receipt_watches,
        delta_ok: failure_reasons.is_empty(),
        delta_warnings,
        failure_reasons,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_live_demo_deposit_relay(
    options: NavRoundtripDepositRelayOptions,
) -> Result<NavRoundtripDepositRelayReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("deposit-relay.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing deposit relay artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripDepositRelayReport>(&raw).map_err(|error| {
            format!(
                "existing deposit relay artifact `{}` is not a NAV roundtrip deposit relay report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "deposit relay artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }
    let evm_report_raw = std::fs::read_to_string(&options.evm_deposit_report_file).map_err(|error| {
        format!(
            "failed to read EVM deposit report `{}`: {error}",
            options.evm_deposit_report_file.display()
        )
    })?;
    let evm_report = serde_json::from_str::<NavRoundtripEvmDepositReport>(&evm_report_raw)
        .map_err(|error| {
            format!(
                "EVM deposit report `{}` is invalid: {error}",
                options.evm_deposit_report_file.display()
            )
        })?;
    if !evm_report.delta_ok {
        return Err(format!(
            "EVM deposit report `{}` did not verify deltas: {:?}",
            options.evm_deposit_report_file.display(),
            evm_report.failure_reasons
        ));
    }

    let relay_bundle_dir = options.artifact_dir.join("deposit-relay-bundle");
    let relay_bundle = if options.resume && relay_bundle_dir.join("plan.json").is_file() {
        nav_roundtrip_load_existing_deposit_relay_rpc_bundle(
            &relay_bundle_dir,
            &options.source_rpc_url,
            &evm_report.deposit_tx,
        )?
    } else {
        vault_bridge_deposit_relay_rpc_bundle(VaultBridgeDepositRelayRpcBundleOptions {
            source_rpc_url: options.source_rpc_url.clone(),
            tx_hash: evm_report.deposit_tx.clone(),
            cast_binary: options.cast_binary.clone(),
            plan_options: VaultBridgeDepositPlanOptions {
                log_file: None,
                receipt_file: None,
                vault_address: Some(options.vault_address.clone()),
                token_address: Some(options.token_address.clone()),
                asset_id: options.asset_id.clone(),
                policy_hash: options.policy_hash.clone(),
                proposer: options.proposer.clone(),
                finalizer: options.finalizer.clone(),
                claimer: options.claimer.clone(),
                attestor: options.attestor.clone(),
                observer_confirmation_depth: None,
                expires_at_height: options.expires_at_height,
                source_proof_kind: options.source_proof_kind.clone(),
                source_proof_hash: options.source_proof_hash.clone(),
                source_public_values_hash: options.source_public_values_hash.clone(),
            },
            bundle_dir: relay_bundle_dir.clone(),
            overwrite: options.overwrite,
        })
        .map_err(|error| format!("NAV roundtrip deposit relay bundle failed: {error}"))?
    };

    let certified_ops_file = options.artifact_dir.join("deposit-relay.certified-ops.json");
    let certified_ops_request_preexisting = options.resume && certified_ops_file.is_file();
    let bundle_adapter_operation_count = if certified_ops_request_preexisting {
        let request = read_certified_asset_ops_request(&certified_ops_file)?;
        validate_certified_asset_ops_request(&request)?;
        request.operations.len()
    } else {
        certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
            bundle_dir: relay_bundle_dir.clone(),
            output_file: certified_ops_file.clone(),
            proposer_key_file: Some(options.proposer_key_file.clone()),
            attestor_key_file: options.attestor_key_file.clone(),
            finalizer_key_file: Some(options.finalizer_key_file.clone()),
            claimer_key_file: options
                .claim_deposit
                .then(|| options.claimer_key_file.clone()),
            owner_key_file: None,
            include_deposit_claim: options.claim_deposit,
            overwrite: options.overwrite,
        })?
        .operation_count
    };
    if bundle_adapter_operation_count == 0 {
        return Err("deposit relay bundle adapter produced no certified operations".to_string());
    }
    if !options.claim_deposit && !certified_ops_request_preexisting {
        let receipt_operator_key_file = options.receipt_operator_key_file.as_ref().ok_or_else(|| {
            "deposit relay without --claim-deposit requires --issuer-key-file so the finalized bridge deposit can become a counted receipt for NAV primary mint".to_string()
        })?;
        let mut request = read_certified_asset_ops_request(&certified_ops_file)?;
        request.operations.extend(nav_roundtrip_bridge_receipt_ops(
            &options.data_dir,
            &relay_bundle.relay_bundle.plan,
            receipt_operator_key_file,
            options.expires_at_height,
        )?);
        validate_certified_asset_ops_request(&request)?;
        write_json_file(&certified_ops_file, &request_to_json(&request)?)?;
    }

    let certified_ops_artifact_dir = options.artifact_dir.join("deposit-relay-certified");
    let certified_ops_options = CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: certified_ops_file.clone(),
        artifact_dir: certified_ops_artifact_dir.clone(),
        max_transactions: None,
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
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
    };
    let (certified_ops, certified_ops_stages) = if options.prepare_only || options.batch_only {
        let certified_ops = certified_asset_ops_batch(certified_ops_options)?;
        (certified_ops.clone(), vec![certified_ops])
    } else {
        nav_roundtrip_certify_deposit_relay_stages(certified_ops_options)?
    };

    let relay_bundle_value = serde_json::to_value(&relay_bundle)
        .map_err(|error| format!("deposit relay bundle report serialization failed: {error}"))?;
    let report = NavRoundtripDepositRelayReport {
        schema: NAV_ROUNDTRIP_DEPOSIT_RELAY_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        evm_deposit_report_file: options.evm_deposit_report_file.display().to_string(),
        deposit_tx: evm_report.deposit_tx,
        claim_deposit: options.claim_deposit,
        relay_bundle_dir: relay_bundle_dir.display().to_string(),
        certified_ops_file: certified_ops_file.display().to_string(),
        certified_ops_artifact_dir: certified_ops_artifact_dir.display().to_string(),
        relay_bundle: relay_bundle_value,
        certified_ops_stages,
        certified_ops,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}

fn nav_roundtrip_load_existing_deposit_relay_rpc_bundle(
    bundle_dir: &std::path::Path,
    source_rpc_url: &str,
    tx_hash: &str,
) -> Result<postfiat_node::VaultBridgeDepositRelayRpcBundleReport, String> {
    let receipt_file = bundle_dir.join("source-receipt.json");
    let plan_file = bundle_dir.join("plan.json");
    let propose_operation_file = bundle_dir.join("propose.operation.json");
    let attest_operation_file = bundle_dir.join("attest.operation.json");
    let finalize_operation_file = bundle_dir.join("finalize.operation.json");
    let claim_operation_file = bundle_dir.join("claim.operation.json");
    let commands_file = bundle_dir.join("commands.sh");
    for path in [
        &receipt_file,
        &plan_file,
        &propose_operation_file,
        &finalize_operation_file,
        &claim_operation_file,
        &commands_file,
    ] {
        if !path.is_file() {
            return Err(format!(
                "existing deposit relay bundle is incomplete; missing `{}`",
                path.display()
            ));
        }
    }
    let receipt_raw = std::fs::read_to_string(&receipt_file).map_err(|error| {
        format!(
            "failed to read existing deposit relay source receipt `{}`: {error}",
            receipt_file.display()
        )
    })?;
    let receipt = serde_json::from_str::<serde_json::Value>(&receipt_raw).map_err(|error| {
        format!(
            "existing deposit relay source receipt `{}` is invalid JSON: {error}",
            receipt_file.display()
        )
    })?;
    let receipt_block_hash = nav_roundtrip_existing_receipt_hex_field(
        &receipt,
        &["blockHash", "block_hash"],
        "block hash",
    )?;
    let receipt_transaction_hash = nav_roundtrip_existing_receipt_hex_field(
        &receipt,
        &["transactionHash", "txHash", "tx_hash"],
        "transaction hash",
    )?;
    let normalized_tx_hash = nav_roundtrip_normalize_0x_hex(tx_hash)?;
    if receipt_transaction_hash != normalized_tx_hash {
        return Err(format!(
            "existing deposit relay receipt transaction hash `{receipt_transaction_hash}` does not match deposit report tx hash `{normalized_tx_hash}`"
        ));
    }
    let plan = nav_roundtrip_read_json_file::<postfiat_node::VaultBridgeDepositPlanReport>(
        &plan_file,
        "existing deposit relay plan",
    )?;
    let propose_operation =
        nav_roundtrip_read_json_file::<postfiat_types::AssetTransactionOperation>(
            &propose_operation_file,
            "existing deposit relay propose operation",
        )?;
    let attest_operation = if attest_operation_file.is_file() {
        Some(nav_roundtrip_read_json_file::<postfiat_types::AssetTransactionOperation>(
            &attest_operation_file,
            "existing deposit relay attest operation",
        )?)
    } else {
        None
    };
    let finalize_operation =
        nav_roundtrip_read_json_file::<postfiat_types::AssetTransactionOperation>(
            &finalize_operation_file,
            "existing deposit relay finalize operation",
        )?;
    let claim_operation = nav_roundtrip_read_json_file::<postfiat_types::AssetTransactionOperation>(
        &claim_operation_file,
        "existing deposit relay claim operation",
    )?;
    let commands_raw = std::fs::read_to_string(&commands_file).map_err(|error| {
        format!(
            "failed to read existing deposit relay commands `{}`: {error}",
            commands_file.display()
        )
    })?;
    let commands = commands_raw
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    Ok(postfiat_node::VaultBridgeDepositRelayRpcBundleReport {
        schema: postfiat_node::VAULT_BRIDGE_DEPOSIT_RELAY_RPC_BUNDLE_SCHEMA.to_string(),
        source_rpc_url: source_rpc_url.to_string(),
        tx_hash: normalized_tx_hash,
        receipt_file: receipt_file.display().to_string(),
        receipt_block_hash,
        receipt_transaction_hash,
        receipt_block_number: 0,
        current_block_number: 0,
        confirmation_depth: plan.deposit_confirmation_depth.unwrap_or(0),
        relay_bundle: postfiat_node::VaultBridgeDepositRelayBundleReport {
            schema: postfiat_node::VAULT_BRIDGE_DEPOSIT_RELAY_BUNDLE_SCHEMA.to_string(),
            bundle_dir: bundle_dir.display().to_string(),
            plan_file: plan_file.display().to_string(),
            propose_operation_file: propose_operation_file.display().to_string(),
            attest_operation_file: attest_operation_file
                .is_file()
                .then(|| attest_operation_file.display().to_string()),
            finalize_operation_file: finalize_operation_file.display().to_string(),
            claim_operation_file: claim_operation_file.display().to_string(),
            commands_file: commands_file.display().to_string(),
            commands,
            plan: postfiat_node::VaultBridgeDepositPlanReport {
                propose_operation,
                attest_operation,
                finalize_operation,
                claim_operation,
                ..plan
            },
        },
        trust_boundary:
            "source receipt was loaded from the existing relay bundle during NAV roundtrip resume; PFTL finality still depends on configured source-proof or challenge/finality policy"
                .to_string(),
    })
}

fn nav_roundtrip_read_json_file<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    label: &str,
) -> Result<T, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} `{}`: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("{label} `{}` is invalid: {error}", path.display()))
}

fn nav_roundtrip_existing_receipt_hex_field(
    value: &serde_json::Value,
    names: &[&str],
    label: &str,
) -> Result<String, String> {
    for name in names {
        if let Some(raw) = value.get(*name).and_then(serde_json::Value::as_str) {
            return nav_roundtrip_normalize_0x_hex(raw)
                .map_err(|error| format!("existing deposit relay receipt {label} invalid: {error}"));
        }
    }
    Err(format!(
        "existing deposit relay receipt missing {label} field {:?}",
        names
    ))
}

fn nav_roundtrip_normalize_0x_hex(value: &str) -> Result<String, String> {
    let stripped = value.strip_prefix("0x").unwrap_or(value);
    if stripped.is_empty()
        || stripped.len() % 2 != 0
        || !stripped.as_bytes().iter().all(u8::is_ascii_hexdigit)
    {
        return Err(format!("`{value}` is not even-length hex"));
    }
    Ok(format!("0x{}", stripped.to_ascii_lowercase()))
}

fn nav_roundtrip_bridge_receipt_ops(
    data_dir: &std::path::Path,
    plan: &postfiat_node::VaultBridgeDepositPlanReport,
    receipt_operator_key_file: &std::path::Path,
    expires_at_height: u64,
) -> Result<Vec<CertifiedAssetOpRequest>, String> {
    let store = postfiat_storage::NodeStore::new(data_dir);
    let genesis = store
        .read_genesis()
        .map_err(|error| format!("deposit relay receipt read genesis failed: {error}"))?;
    let ledger = store
        .read_ledger()
        .map_err(|error| format!("deposit relay receipt read ledger failed: {error}"))?;
    let nav_asset = ledger.nav_asset(&plan.asset_id).ok_or_else(|| {
        format!(
            "deposit relay receipt asset `{}` is not registered as a NAV asset",
            plan.asset_id
        )
    })?;
    let operator = nav_asset.issuer.clone();
    let receipt_id = postfiat_types::vault_bridge_receipt_id(
        &genesis.chain_id,
        &plan.asset_id,
        &plan.source_domain,
        &plan.source_tx_or_attestation,
        &plan.finality_ref,
        plan.evidence.amount_atoms,
        &plan.policy_hash,
    )
    .map_err(|error| format!("deposit relay receipt id derivation failed: {error}"))?;
    let submit_operation =
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptSubmit(
            postfiat_types::VaultBridgeReceiptSubmitOperation {
                operator: operator.clone(),
                asset_id: plan.asset_id.clone(),
                source_domain: plan.source_domain.clone(),
                source_asset: plan.evidence.source_asset_ref(),
                claim_type: postfiat_types::VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT.to_string(),
                amount_atoms: plan.evidence.amount_atoms,
                source_tx_or_attestation: plan.source_tx_or_attestation.clone(),
                finality_ref: plan.finality_ref.clone(),
                vault_id: plan.vault_id.clone(),
                policy_hash: plan.policy_hash.clone(),
                expires_at_height,
                bridge_deposit_evidence: Some(plan.evidence.clone()),
            },
        );
    let count_operation =
        postfiat_types::AssetTransactionOperation::VaultBridgeReceiptCount(
            postfiat_types::VaultBridgeReceiptCountOperation {
                operator: operator.clone(),
                asset_id: plan.asset_id.clone(),
                receipt_id,
                haircut_bps: 0,
                counted_value_atoms: plan.evidence.amount_atoms,
                evidence_root: plan.evidence_root.clone(),
                policy_hash: plan.policy_hash.clone(),
            },
        );
    submit_operation
        .validate()
        .map_err(|error| format!("deposit relay receipt submit operation invalid: {error}"))?;
    count_operation
        .validate()
        .map_err(|error| format!("deposit relay receipt count operation invalid: {error}"))?;
    Ok(vec![
        CertifiedAssetOpRequest {
            label: "receipt-submit".to_string(),
            source: operator.clone(),
            key_file: receipt_operator_key_file.to_path_buf(),
            operation: submit_operation,
            dependencies: Vec::new(),
        },
        CertifiedAssetOpRequest {
            label: "receipt-count".to_string(),
            source: operator,
            key_file: receipt_operator_key_file.to_path_buf(),
            operation: count_operation,
            dependencies: vec![CertifiedAssetOpDependency {
                label: "receipt-submit".to_string(),
                mode: "same_round".to_string(),
                reason: Some(
                    "receipt id is deterministic from the submitted bridge-deposit evidence"
                        .to_string(),
                ),
            }],
        },
    ])
}

fn nav_roundtrip_certify_deposit_relay_stages(
    options: CertifiedAssetOpsBatchOptions,
) -> Result<(CertifiedAssetOpsBatchReport, Vec<CertifiedAssetOpsBatchReport>), String> {
    let request = read_certified_asset_ops_request(&options.ops_file)?;
    validate_certified_asset_ops_request(&request)?;

    let mut propose_attest_ops = Vec::new();
    let mut finalize_claim_ops = Vec::new();
    let mut receipt_ops = Vec::new();
    for op in request.operations {
        match op.label.as_str() {
            "propose" | "attest" => propose_attest_ops.push(op),
            "finalize" | "claim" => finalize_claim_ops.push(op),
            "receipt-submit" | "receipt-count" => receipt_ops.push(op),
            other => {
                return Err(format!(
                    "deposit relay stage does not support bundle operation label `{other}`"
                ));
            }
        }
    }
    if !propose_attest_ops.iter().any(|op| op.label == "propose") {
        return Err("deposit relay bundle is missing propose operation".to_string());
    }
    if !finalize_claim_ops.iter().any(|op| op.label == "finalize") {
        return Err("deposit relay bundle is missing finalize operation".to_string());
    }
    certified_asset_op_add_dependency(
        &mut propose_attest_ops,
        "attest",
        "propose",
        "same_round",
        "attestation signs the same deterministic bridge evidence proposed in this round",
    )?;
    certified_asset_op_add_dependency(
        &mut finalize_claim_ops,
        "finalize",
        "propose",
        "prior_round",
        "finalize requires the proposal from the prior certified propose/attest round",
    )?;
    certified_asset_op_add_dependency(
        &mut finalize_claim_ops,
        "finalize",
        "attest",
        "prior_round",
        "finalize requires the attestation from the prior certified propose/attest round",
    )?;
    if finalize_claim_ops.iter().any(|op| op.label == "claim") {
        certified_asset_op_add_dependency(
            &mut finalize_claim_ops,
            "claim",
            "finalize",
            "same_round",
            "deposit claim uses the deterministic finalized deposit evidence",
        )?;
    }
    if !receipt_ops.is_empty() {
        let receipt_prior_label = if finalize_claim_ops.iter().any(|op| op.label == "claim") {
            "claim"
        } else {
            "finalize"
        };
        certified_asset_op_add_dependency(
            &mut receipt_ops,
            "receipt-submit",
            receipt_prior_label,
            "prior_round",
            "receipt submission counts bridge value only after the deposit relay has finalized",
        )?;
    }

    let staged_dir = options.artifact_dir.clone();
    std::fs::create_dir_all(&staged_dir).map_err(|error| {
        format!(
            "failed to create deposit relay staged artifact dir `{}`: {error}",
            staged_dir.display()
        )
    })?;
    let propose_attest_file = staged_dir.join("propose-attest.certified-ops.json");
    let finalize_claim_file = staged_dir.join("finalize-claim.certified-ops.json");
    let receipt_file = staged_dir.join("receipt.certified-ops.json");
    write_json_file(
        &propose_attest_file,
        &request_to_json(&CertifiedAssetOpsRequest {
            schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            operations: propose_attest_ops,
        })?,
    )?;
    write_json_file(
        &finalize_claim_file,
        &request_to_json(&CertifiedAssetOpsRequest {
            schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
            operations: finalize_claim_ops,
        })?,
    )?;
    if !receipt_ops.is_empty() {
        write_json_file(
            &receipt_file,
            &request_to_json(&CertifiedAssetOpsRequest {
                schema: Some(CERTIFIED_ASSET_OPS_REQUEST_SCHEMA.to_string()),
                operations: receipt_ops,
            })?,
        )?;
    }

    let mut propose_attest_options = options.clone();
    propose_attest_options.ops_file = propose_attest_file;
    propose_attest_options.artifact_dir = staged_dir.join("propose-attest");
    let propose_attest_report = certified_asset_ops_direct_batch(propose_attest_options)?;
    nav_roundtrip_require_certified_ops_ok(
        "deposit relay propose/attest",
        &propose_attest_report,
        &staged_dir,
    )?;

    let mut finalize_claim_options = options.clone();
    finalize_claim_options.ops_file = finalize_claim_file;
    finalize_claim_options.artifact_dir = staged_dir.join("finalize-claim");
    let finalize_claim_report = certified_asset_ops_direct_batch(finalize_claim_options)?;
    nav_roundtrip_require_certified_ops_ok(
        "deposit relay finalize/claim",
        &finalize_claim_report,
        &staged_dir,
    )?;

    let mut stages = vec![propose_attest_report, finalize_claim_report.clone()];
    let mut final_report = finalize_claim_report;
    if receipt_file.exists() {
        let mut receipt_options = options;
        receipt_options.ops_file = receipt_file;
        receipt_options.artifact_dir = staged_dir.join("receipt");
        let receipt_report = certified_asset_ops_direct_batch(receipt_options)?;
        nav_roundtrip_require_certified_ops_ok(
            "deposit relay receipt submit/count",
            &receipt_report,
            &staged_dir,
        )?;
        final_report = receipt_report.clone();
        stages.push(receipt_report);
    }

    Ok((final_report, stages))
}

fn nav_roundtrip_live_demo_primary_mint(
    options: NavRoundtripPrimaryMintOptions,
) -> Result<NavRoundtripPrimaryMintReport, String> {
    std::fs::create_dir_all(&options.artifact_dir).map_err(|error| {
        format!(
            "failed to create NAV roundtrip artifact dir `{}`: {error}",
            options.artifact_dir.display()
        )
    })?;
    let artifact_file = options.artifact_dir.join("primary-mint.json");
    if options.resume && artifact_file.is_file() {
        let raw = std::fs::read_to_string(&artifact_file).map_err(|error| {
            format!(
                "failed to read existing primary mint artifact `{}`: {error}",
                artifact_file.display()
            )
        })?;
        return serde_json::from_str::<NavRoundtripPrimaryMintReport>(&raw).map_err(|error| {
            format!(
                "existing primary mint artifact `{}` is not a NAV roundtrip primary mint report: {error}",
                artifact_file.display()
            )
        });
    }
    if artifact_file.exists() && !options.overwrite {
        return Err(format!(
            "primary mint artifact `{}` already exists; use --resume, --overwrite, or a new artifact dir",
            artifact_file.display()
        ));
    }

    let store = postfiat_storage::NodeStore::new(&options.data_dir);
    let genesis = store
        .read_genesis()
        .map_err(|error| format!("primary mint read genesis failed: {error}"))?;
    let ledger = store
        .read_ledger()
        .map_err(|error| format!("primary mint read ledger failed: {error}"))?;
    let nav_asset = ledger
        .nav_asset(&options.nav_asset_id)
        .ok_or_else(|| format!("missing NAV asset `{}`", options.nav_asset_id))?
        .clone();
    let nav_asset_definition = ledger
        .asset_definition(&options.nav_asset_id)
        .ok_or_else(|| format!("missing NAV asset definition `{}`", options.nav_asset_id))?
        .clone();
    let settlement_nav_asset = ledger
        .nav_asset(&options.settlement_asset_id)
        .ok_or_else(|| {
            format!(
                "missing settlement NAV asset `{}`",
                options.settlement_asset_id
            )
        })?
        .clone();
    let settlement_asset = ledger
        .asset_definition(&options.settlement_asset_id)
        .ok_or_else(|| {
            format!(
                "missing settlement asset definition `{}`",
                options.settlement_asset_id
            )
        })?
        .clone();
    if nav_asset.finalized_epoch == 0 && options.nav_epoch.is_none() {
        return Err(format!(
            "NAV asset `{}` has no finalized epoch; pass --nav-epoch only if this is intentional",
            options.nav_asset_id
        ));
    }
    let nav_epoch = options.nav_epoch.unwrap_or(nav_asset.finalized_epoch);
    let nav_reserve_packet_hash = options
        .nav_reserve_packet_hash
        .clone()
        .unwrap_or_else(|| nav_asset.finalized_reserve_packet_hash.clone());
    if nav_reserve_packet_hash.is_empty() {
        return Err(format!(
            "NAV asset `{}` has no finalized reserve packet hash; pass --nav-reserve-packet-hash",
            options.nav_asset_id
        ));
    }
    let settlement_amount_atoms = match options.settlement_amount_atoms {
        Some(value) => value,
        None => nav_roundtrip_required_vault_bridge_settlement_atoms(
            options.mint_amount,
            nav_asset_definition.precision,
            nav_asset.nav_per_unit,
            &nav_asset.valuation_unit,
            &settlement_nav_asset.valuation_unit,
            settlement_asset.precision,
        )?,
    };

    let deposit_relay_report = options
        .deposit_relay_report_file
        .as_ref()
        .map(|path| {
            let raw = std::fs::read_to_string(path).map_err(|error| {
                format!(
                    "failed to read deposit relay report `{}`: {error}",
                    path.display()
                )
            })?;
            serde_json::from_str::<NavRoundtripDepositRelayReport>(&raw).map_err(|error| {
                format!("deposit relay report `{}` is invalid: {error}", path.display())
            })
        })
        .transpose()?;
    let matched_deposit_tx = deposit_relay_report
        .as_ref()
        .map(|report| nav_roundtrip_normalize_hex_text(&report.deposit_tx));

    let settlement_status_before = vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
        data_dir: options.data_dir.clone(),
        asset_id: options.settlement_asset_id.clone(),
    })
    .map_err(|error| format!("primary mint settlement status failed: {error}"))?;
    let (settlement_receipt, consumed_supply_allocation_id) = if options.consume_issued_settlement {
        let subscriber_key_file = options.subscriber_key_file.as_ref().ok_or_else(|| {
            "--subscriber-key-file is required with --consume-issued-settlement".to_string()
        })?;
        if !subscriber_key_file.is_file() {
            return Err(format!(
                "subscriber key file `{}` does not exist",
                subscriber_key_file.display()
            ));
        }
        let subscriber_balance = ledger
            .trustline_for_account_asset(&options.subscriber, &options.settlement_asset_id)
            .map(|line| line.balance)
            .unwrap_or(0);
        if subscriber_balance < settlement_amount_atoms {
            return Err(format!(
                "subscriber `{}` has {} settlement atoms but primary mint requires {}",
                options.subscriber, subscriber_balance, settlement_amount_atoms
            ));
        }
        let source = nav_roundtrip_select_issued_settlement_source(
            &ledger,
            &settlement_status_before,
            &options.settlement_asset_id,
            settlement_amount_atoms,
            options.settlement_receipt_id.as_deref(),
            options.settlement_supply_allocation_id.as_deref(),
            matched_deposit_tx.as_deref(),
        )?;
        (source.receipt, Some(source.supply_allocation_id))
    } else {
        (
            nav_roundtrip_select_settlement_receipt(
                &settlement_status_before,
                settlement_amount_atoms,
                options.settlement_receipt_id.as_deref(),
                matched_deposit_tx.as_deref(),
            )?,
            None,
        )
    };
    let settlement_receipt_id = settlement_receipt.receipt_id.clone();
    let settlement_bucket_id = settlement_receipt.bucket_id.clone();
    let subscription_id = options
        .consume_issued_settlement
        .then(|| nav_roundtrip_primary_mint_subscription_id(&options.artifact_dir));
    let settlement_consumer_id = if options.consume_issued_settlement {
        subscription_id.as_ref().map_or_else(
            || {
                nav_roundtrip_nav_subscription_recipient_consumer_id(
                    &options.nav_asset_id,
                    &options.subscriber,
                )
            },
            |subscription_id| {
                nav_roundtrip_nav_subscription_recipient_order_consumer_id(
                    &options.nav_asset_id,
                    &options.subscriber,
                    subscription_id,
                )
            },
        )
    } else {
        nav_roundtrip_nav_subscription_consumer_id(&options.nav_asset_id)
    };
    let settlement_allocation_id = postfiat_types::vault_bridge_allocation_id(
        &genesis.chain_id,
        &settlement_receipt_id,
        &options.settlement_asset_id,
        &settlement_bucket_id,
        settlement_amount_atoms,
        postfiat_types::VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION,
        &settlement_consumer_id,
    )
    .map_err(|error| format!("primary mint allocation id derivation failed: {error}"))?;

    let consume_supply_owner = options
        .consume_issued_settlement
        .then(|| options.subscriber.clone());
    let allocate_operation = postfiat_types::AssetTransactionOperation::VaultBridgeNavSubscriptionAllocate(
        postfiat_types::VaultBridgeNavSubscriptionAllocateOperation {
            operator: nav_asset.issuer.clone(),
            nav_asset_id: options.nav_asset_id.clone(),
            settlement_asset_id: options.settlement_asset_id.clone(),
            settlement_bucket_id: settlement_bucket_id.clone(),
            settlement_receipt_id: settlement_receipt_id.clone(),
            settlement_amount_atoms,
            consume_supply_owner,
            consume_supply_allocation_id: consumed_supply_allocation_id.clone(),
            nav_recipient: options
                .consume_issued_settlement
                .then(|| options.subscriber.clone()),
            subscription_id: subscription_id.clone(),
        },
    );
    let mint_operation = postfiat_types::AssetTransactionOperation::NavMintAtNav(
        postfiat_types::NavMintAtNavOperation {
            issuer: nav_asset.issuer.clone(),
            to: options.subscriber.clone(),
            asset_id: options.nav_asset_id.clone(),
            amount: options.mint_amount,
            epoch: nav_epoch,
            reserve_packet_hash: nav_reserve_packet_hash.clone(),
            settlement_asset_id: options.settlement_asset_id.clone(),
            settlement_bucket_id: settlement_bucket_id.clone(),
            settlement_allocation_id: settlement_allocation_id.clone(),
            settlement_amount_atoms,
        },
    );
    allocate_operation
        .validate()
        .map_err(|error| format!("primary mint allocate operation invalid: {error}"))?;
    mint_operation
        .validate()
        .map_err(|error| format!("primary mint operation invalid: {error}"))?;

    let allocate_operation_file = options.artifact_dir.join("nav-subscription-allocate.operation.json");
    let mint_operation_file = options.artifact_dir.join("nav-mint-at-nav.operation.json");
    write_json_file(&allocate_operation_file, &allocate_operation)?;
    write_json_file(&mint_operation_file, &mint_operation)?;

    let operations_file = options.artifact_dir.join("primary-mint.certified-ops.json");
    let allocate_source = if options.consume_issued_settlement {
        options.subscriber.clone()
    } else {
        nav_asset.issuer.clone()
    };
    let allocate_key_file = if options.consume_issued_settlement {
        options
            .subscriber_key_file
            .as_ref()
            .expect("subscriber key file checked above")
    } else {
        &options.issuer_key_file
    };
    let request = serde_json::json!({
        "schema": CERTIFIED_ASSET_OPS_REQUEST_SCHEMA,
        "operations": [
            {
                "label": "nav-subscription-allocate",
                "source": allocate_source,
                "key_file": allocate_key_file.display().to_string(),
                "operation": allocate_operation,
                "dependencies": [],
            },
            {
                "label": "nav-mint-at-nav",
                "source": nav_asset.issuer.clone(),
                "key_file": options.issuer_key_file.display().to_string(),
                "operation": mint_operation,
                "dependencies": [{
                    "label": "nav-subscription-allocate",
                    "mode": "same_round",
                    "reason": "allocation id is deterministically derived before apply",
                }],
            },
        ],
    });
    write_json_file(&operations_file, &request)?;

    let certified_ops_artifact_dir = options.artifact_dir.join("primary-mint-certified");
    let certified_ops = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
        data_dir: options.data_dir.clone(),
        topology_file: options.topology_file.clone(),
        key_file: options.validator_key_file.clone(),
        proposal_key_file: options.proposal_key_file.clone(),
        ops_file: operations_file.clone(),
        artifact_dir: certified_ops_artifact_dir.clone(),
        max_transactions: None,
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
        prepare_only: options.prepare_only,
        batch_only: options.batch_only,
    })?;

    let settlement_status_after = if options.prepare_only {
        None
    } else {
        Some(
            vault_bridge_status(postfiat_node::VaultBridgeStatusOptions {
                data_dir: options.data_dir.clone(),
                asset_id: options.settlement_asset_id.clone(),
            })
            .map_err(|error| format!("primary mint settlement final status failed: {error}"))?,
        )
    };

    let report = NavRoundtripPrimaryMintReport {
        schema: NAV_ROUNDTRIP_PRIMARY_MINT_REPORT_SCHEMA.to_string(),
        artifact_file: artifact_file.display().to_string(),
        deposit_relay_report_file: options
            .deposit_relay_report_file
            .as_ref()
            .map(|path| path.display().to_string()),
        nav_asset_id: options.nav_asset_id,
        settlement_asset_id: options.settlement_asset_id,
        issuer: nav_asset.issuer,
        subscriber: options.subscriber.clone(),
        nav_epoch,
        nav_reserve_packet_hash,
        nav_per_unit: nav_asset.nav_per_unit,
        nav_valuation_unit: nav_asset.valuation_unit,
        settlement_valuation_unit: settlement_nav_asset.valuation_unit,
        settlement_asset_precision: settlement_asset.precision,
        mint_amount: options.mint_amount,
        settlement_amount_atoms,
        settlement_receipt_id,
        settlement_bucket_id,
        settlement_allocation_id,
        subscription_id,
        consume_issued_settlement: options.consume_issued_settlement,
        consumed_supply_owner: options
            .consume_issued_settlement
            .then(|| options.subscriber.clone()),
        consumed_supply_allocation_id,
        matched_deposit_tx,
        settlement_status_before,
        settlement_status_after,
        operations_file: operations_file.display().to_string(),
        allocate_operation_file: allocate_operation_file.display().to_string(),
        mint_operation_file: mint_operation_file.display().to_string(),
        certified_ops_artifact_dir: certified_ops_artifact_dir.display().to_string(),
        certified_ops,
    };
    write_json_file(&artifact_file, &report)?;
    Ok(report)
}
