pub fn governance_agent_gate_1_5(
    options: GovernanceAgentGateOptions,
) -> io::Result<GovernanceAgentGateReport> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 1.5 report",
    )?;
    let validation = validate_governance_agent_source_bundle(&options.agent_dir)?;

    let mut report = GovernanceAgentGateReport {
        schema: GOVERNANCE_AGENT_GATE_REPORT_SCHEMA.to_string(),
        gate: "1.5-constitutional-prompt-bundle".to_string(),
        verified: true,
        agent_dir: options.agent_dir.display().to_string(),
        bundle_hash: validation.bundle_hash,
        model_request_hash: validation.model_request_hash,
        statement_hashes: validation.statement_hashes,
        ruleset_schema_hash: validation.ruleset_schema_hash,
        valid_fixture: validation.valid_fixture,
        invalid_fixtures: validation.invalid_fixtures,
        canonical_json_key_order_stable: validation.canonical_json_key_order_stable,
        statement_hash_one_byte_edit_detected: validation.statement_hash_one_byte_edit_detected,
        bundle_hash_includes: vec![
            "architecture_statement_hash".to_string(),
            "objective_statement_hash".to_string(),
            "constitutional_constraints_hash".to_string(),
            "ruleset_schema_hash".to_string(),
            "model_id".to_string(),
            "runtime".to_string(),
            "deterministic_flags".to_string(),
            "rollback_policy".to_string(),
        ],
        model_request_hash_includes: vec![
            "bundle_hash".to_string(),
            "evidence_root".to_string(),
            "objective_statement_hash".to_string(),
            "round_seed_input".to_string(),
            "round_seed".to_string(),
        ],
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    Ok(report)
}

pub fn governance_agent_model_request(
    options: GovernanceAgentModelRequestOptions,
) -> io::Result<GovernanceAgentModelRequest> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent model request",
    )?;
    let request = match options.round_seed_input {
        Some(round_seed_input) => {
            build_governance_agent_model_request_with_seed(&options.agent_dir, round_seed_input)?
        }
        None => build_governance_agent_model_request(&options.agent_dir)?,
    };
    let json = serde_json::to_string_pretty(&request).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    Ok(request)
}

pub fn governance_agent_gate_3_5(
    options: GovernanceAgentGate3_5Options,
) -> io::Result<GovernanceAgentGate3_5Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 3.5 report",
    )?;
    if options.expected_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance agent Gate 3.5 expected count must be nonzero",
        ));
    }
    let request_value =
        read_governance_agent_json_value(&options.model_request_file, "governance model request")?;
    let request: GovernanceAgentModelRequest =
        serde_json::from_value(request_value).map_err(invalid_data)?;
    ensure_governance_agent_model_request_json_only(&request)?;
    let expected_round_seed_input =
        resolve_governance_agent_round_seed_input(options.round_seed_input)?;
    ensure_governance_agent_model_request_seed(&request, &expected_round_seed_input)?;

    let output_files = governance_agent_generation_output_files(&options.outputs_dir)?;
    let mut checks = Vec::with_capacity(output_files.len());
    let mut ruleset_hashes = BTreeSet::new();
    let mut compiled_policy_hashes = BTreeSet::new();
    let mut valid_count = 0usize;

    for (index, path) in output_files.iter().enumerate() {
        let raw = match read_bounded_json_text_file(path, "governance agent generation output") {
            Ok(raw) => raw,
            Err(error) => {
                checks.push(GovernanceAgentGenerationCheck {
                    index: index + 1,
                    path: path.display().to_string(),
                    accepted: false,
                    ruleset_hash: None,
                    compiled_policy_hash: None,
                    output_byte_len: None,
                    error: Some(error.to_string()),
                });
                continue;
            }
        };
        match validate_governance_ruleset_text_with_ruleset(&raw) {
            Ok((value, ruleset)) => {
                let ruleset_hash = governance_agent_canonical_json_hash(
                    "postfiat.governance_agent.ruleset_output.v1",
                    &value,
                )?;
                let compiled_policy_hash =
                    governance_agent_compiled_policy_hash(&ruleset, &ruleset_hash)?;
                ruleset_hashes.insert(ruleset_hash.clone());
                compiled_policy_hashes.insert(compiled_policy_hash.clone());
                valid_count += 1;
                checks.push(GovernanceAgentGenerationCheck {
                    index: index + 1,
                    path: path.display().to_string(),
                    accepted: true,
                    ruleset_hash: Some(ruleset_hash),
                    compiled_policy_hash: Some(compiled_policy_hash),
                    output_byte_len: Some(raw.len()),
                    error: None,
                });
            }
            Err(error) => checks.push(GovernanceAgentGenerationCheck {
                index: index + 1,
                path: path.display().to_string(),
                accepted: false,
                ruleset_hash: None,
                compiled_policy_hash: None,
                output_byte_len: Some(raw.len()),
                error: Some(error.to_string()),
            }),
        }
    }

    let distinct_ruleset_hashes = ruleset_hashes.into_iter().collect::<Vec<_>>();
    let distinct_compiled_policy_hashes = compiled_policy_hashes.into_iter().collect::<Vec<_>>();
    let deterministic_ruleset_hash = distinct_ruleset_hashes.len() == 1;
    let deterministic_compiled_policy_hash = distinct_compiled_policy_hashes.len() == 1;
    let observed_count = output_files.len();
    let verified = observed_count == options.expected_count
        && valid_count == options.expected_count
        && deterministic_ruleset_hash
        && deterministic_compiled_policy_hash;

    let mut report = GovernanceAgentGate3_5Report {
        schema: GOVERNANCE_AGENT_GATE_3_5_REPORT_SCHEMA.to_string(),
        gate: "3.5-deterministic-ruleset-generation".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        outputs_dir: options.outputs_dir.display().to_string(),
        expected_count: options.expected_count,
        observed_count,
        valid_count,
        model_request_hash: request.request_hash,
        validator_evidence_packet_schema_hash: request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        bundle_hash: request.bundle_hash,
        round_seed_input: request.round_seed_input,
        round_seed: request.round_seed,
        runtime_manifest: request.runtime_manifest,
        ruleset_hash: distinct_ruleset_hashes.first().cloned(),
        compiled_policy_hash: distinct_compiled_policy_hashes.first().cloned(),
        distinct_ruleset_hashes,
        distinct_compiled_policy_hashes,
        deterministic_ruleset_hash,
        deterministic_compiled_policy_hash,
        generation_checks: checks,
        compiled_policy_note: "Gate 3.5 hashes a deterministic dry-run policy descriptor from the generated ruleset; executable evidence evaluation remains DGA-070.".to_string(),
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 3.5 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_3_6(
    options: GovernanceAgentGate3_6Options,
) -> io::Result<GovernanceAgentGate3_6Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 3.6 report",
    )?;
    let expected_round_seed_input =
        resolve_governance_agent_round_seed_input(options.round_seed_input)?;
    let request = build_governance_agent_model_request_with_seed(
        &options.agent_dir,
        expected_round_seed_input.clone(),
    )?;
    ensure_governance_agent_model_request_seed(&request, &expected_round_seed_input)?;
    let replay_request = build_governance_agent_model_request_with_seed(
        &options.agent_dir,
        expected_round_seed_input.clone(),
    )?;
    ensure_governance_agent_model_request_seed(&replay_request, &expected_round_seed_input)?;
    let same_seed_replays = request.request_hash == replay_request.request_hash
        && request.round_seed == replay_request.round_seed;
    let same_evidence_universe_hashes = request
        .governed_inputs
        .validator_evidence_packet_schema_hash
        == replay_request
            .governed_inputs
            .validator_evidence_packet_schema_hash
        && request
            .governed_inputs
            .validator_evidence_field_registry_hash
            == replay_request
                .governed_inputs
                .validator_evidence_field_registry_hash;

    let wrong_seed_input = governance_agent_wrong_round_seed_input(&expected_round_seed_input);
    let wrong_seed_rejected = governance_agent_seed_rejection_check(
        "wrong_cobalt_certificate_hash",
        &options.agent_dir,
        wrong_seed_input,
        &expected_round_seed_input,
    );
    let stale_seed_input = governance_agent_stale_round_seed_input(&expected_round_seed_input);
    let stale_seed_rejected = governance_agent_seed_rejection_check(
        "stale_round_id",
        &options.agent_dir,
        stale_seed_input,
        &expected_round_seed_input,
    );
    let missing_seed_input = GovernanceAgentRoundSeedInput {
        schema: GOVERNANCE_AGENT_ROUND_SEED_SCHEMA.to_string(),
        cobalt_certificate_hash: String::new(),
        round_id: expected_round_seed_input.round_id.clone(),
        domain: expected_round_seed_input.domain.clone(),
    };
    let missing_seed_rejected = governance_agent_seed_rejection_check(
        "missing_cobalt_certificate_hash",
        &options.agent_dir,
        missing_seed_input,
        &expected_round_seed_input,
    );

    let rejection_checks = vec![
        wrong_seed_rejected,
        stale_seed_rejected,
        missing_seed_rejected,
    ];
    let verified = same_seed_replays
        && same_evidence_universe_hashes
        && rejection_checks.iter().all(|check| !check.accepted);
    let mut report = GovernanceAgentGate3_6Report {
        schema: GOVERNANCE_AGENT_GATE_3_6_REPORT_SCHEMA.to_string(),
        gate: "3.6-timelocked-governance-agent".to_string(),
        verified,
        agent_dir: options.agent_dir.display().to_string(),
        model_request_hash: request.request_hash,
        replay_model_request_hash: replay_request.request_hash,
        validator_evidence_packet_schema_hash: request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        bundle_hash: request.bundle_hash,
        round_seed_input: request.round_seed_input,
        round_seed: request.round_seed,
        same_seed_replays,
        rejection_checks,
        request_hash_includes: request.request_hash_includes,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 3.6 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_7_5(
    options: GovernanceAgentGate7_5Options,
) -> io::Result<GovernanceAgentGate7_5Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 7.5 report",
    )?;
    let (ruleset_value, ruleset) = read_governance_ruleset_file(&options.ruleset_file)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        &ruleset_value,
    )?;
    let evidence = read_governance_agent_evidence_snapshot(&options.evidence_file)?;
    let evidence_value = serde_json::to_value(&evidence).map_err(invalid_data)?;
    let evidence_snapshot_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.frozen_evidence.v1",
        &evidence_value,
    )?;
    let compiled_policy = compile_governance_agent_ruleset_policy(&ruleset, &ruleset_hash)?;
    let registry_delta_candidate = execute_governance_agent_policy(
        &ruleset,
        &compiled_policy,
        &evidence,
        &evidence_snapshot_hash,
    )?;
    let replay_candidate = execute_governance_agent_policy(
        &ruleset,
        &compiled_policy,
        &evidence,
        &evidence_snapshot_hash,
    )?;
    let deterministic_replay =
        registry_delta_candidate.candidate_hash == replay_candidate.candidate_hash
            && registry_delta_candidate == replay_candidate;
    let malformed_rule_checks =
        governance_agent_gate_7_5_malformed_checks(&ruleset_value, &evidence)?;
    let sandbox_verified = !compiled_policy.sandbox.network_access
        && !compiled_policy.sandbox.model_access
        && !compiled_policy.sandbox.filesystem_access
        && !compiled_policy.sandbox.direct_state_mutation
        && !evidence.network_access_allowed
        && !evidence.model_access_allowed
        && !evidence.filesystem_access_allowed
        && !evidence.direct_state_mutation_allowed;
    let verified = deterministic_replay
        && sandbox_verified
        && registry_delta_candidate.mutation_count == 0
        && malformed_rule_checks.iter().all(|check| !check.accepted);
    let mut report = GovernanceAgentGate7_5Report {
        schema: GOVERNANCE_AGENT_GATE_7_5_REPORT_SCHEMA.to_string(),
        gate: "7.5-ruleset-compiler".to_string(),
        verified,
        ruleset_file: options.ruleset_file.display().to_string(),
        evidence_file: options.evidence_file.display().to_string(),
        policy_shape_decision: "deterministic allowlisted Rust interpreter; no generated code, dynamic dispatch, network, model, filesystem, or direct state mutation".to_string(),
        ruleset_hash,
        evidence_snapshot_hash,
        compiled_policy_hash: compiled_policy.compiled_policy_hash.clone(),
        registry_delta_candidate_hash: registry_delta_candidate.candidate_hash.clone(),
        deterministic_replay,
        sandbox: compiled_policy.sandbox.clone(),
        compiled_policy,
        registry_delta_candidate,
        malformed_rule_checks,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 7.5 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_7_6(
    options: GovernanceAgentGate7_6Options,
) -> io::Result<GovernanceAgentGate7_6Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 7.6 report",
    )?;
    let (ruleset_value, ruleset) = read_governance_ruleset_file(&options.ruleset_file)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        &ruleset_value,
    )?;
    let compiled_policy = compile_governance_agent_ruleset_policy(&ruleset, &ruleset_hash)?;
    let fixtures = read_governance_agent_comparison_fixtures(&options.comparison_dir)?;
    let mut checks = Vec::with_capacity(fixtures.len());
    let mut high_confidence_agreement_count = 0usize;
    let mut unsafe_direct_delta_rejection_count = 0usize;
    let mut ambiguous_no_op_count = 0usize;
    let mut false_add_count = 0usize;
    let mut false_remove_count = 0usize;
    let mut direct_subjects = BTreeSet::new();
    let mut policy_subjects = BTreeSet::new();

    for (fixture_value, fixture) in fixtures {
        let fixture_hash = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.comparison_fixture.v1",
            &fixture_value,
        )?;
        let direct_baseline_value =
            serde_json::to_value(&fixture.direct_baseline).map_err(invalid_data)?;
        let direct_baseline_hash = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.direct_baseline.v1",
            &direct_baseline_value,
        )?;
        let evidence_value = serde_json::to_value(&fixture.evidence).map_err(invalid_data)?;
        let evidence_snapshot_hash = governance_agent_canonical_json_hash(
            "postfiat.governance_agent.frozen_evidence.v1",
            &evidence_value,
        )?;
        let policy_delta = execute_governance_agent_policy(
            &ruleset,
            &compiled_policy,
            &fixture.evidence,
            &evidence_snapshot_hash,
        )?;
        for mutation in &fixture.direct_baseline.mutations {
            direct_subjects.insert(mutation.subject_node_id.clone());
        }
        for mutation in &policy_delta.mutations {
            policy_subjects.insert(mutation.subject_node_id.clone());
        }
        let agrees_with_direct =
            governance_agent_policy_delta_matches_direct(&policy_delta, &fixture.direct_baseline);
        let unsafe_case = governance_agent_comparison_case_requires_policy_no_op(&fixture);
        let policy_no_ops_unsafe_case =
            unsafe_case && policy_delta.action == "no_op" && policy_delta.mutation_count == 0;
        let policy_rejected_unsafe_direct_delta =
            policy_no_ops_unsafe_case && !fixture.direct_baseline.mutations.is_empty();
        let high_confidence_case = fixture.case_class == "high_confidence";
        let passed = if high_confidence_case {
            agrees_with_direct
        } else {
            policy_no_ops_unsafe_case
        };
        if high_confidence_case && agrees_with_direct {
            high_confidence_agreement_count += 1;
        }
        if policy_rejected_unsafe_direct_delta {
            unsafe_direct_delta_rejection_count += 1;
        }
        if matches!(
            fixture.case_class.as_str(),
            "ambiguous" | "concentration_risk" | "stale_evidence"
        ) && policy_no_ops_unsafe_case
        {
            ambiguous_no_op_count += 1;
        }
        if fixture.direct_baseline.action == "admit" && !agrees_with_direct && !policy_no_ops_unsafe_case
        {
            false_add_count += 1;
        }
        if fixture.direct_baseline.action == "remove"
            && !agrees_with_direct
            && !policy_no_ops_unsafe_case
        {
            false_remove_count += 1;
        }
        checks.push(GovernanceAgentComparisonCheck {
            case_id: fixture.case_id,
            case_class: fixture.case_class,
            fixture_hash,
            direct_baseline_hash,
            evidence_snapshot_hash,
            policy_delta_hash: policy_delta.candidate_hash,
            direct_action: fixture.direct_baseline.action,
            policy_action: policy_delta.action,
            direct_mutation_count: fixture.direct_baseline.mutations.len(),
            policy_mutation_count: policy_delta.mutation_count,
            agrees_with_direct,
            policy_rejected_unsafe_direct_delta,
            policy_no_ops_unsafe_case,
            passed,
        });
    }

    let top_k_overlap_count = direct_subjects.intersection(&policy_subjects).count();
    let disagreement_case_ids = checks
        .iter()
        .filter(|check| !check.agrees_with_direct)
        .map(|check| check.case_id.clone())
        .collect::<Vec<_>>();
    let verified = !checks.is_empty()
        && governance_agent_comparison_classes_complete(&checks)
        && checks.iter().all(|check| check.passed)
        && high_confidence_agreement_count > 0
        && unsafe_direct_delta_rejection_count >= 3
        && false_add_count == 0
        && false_remove_count == 0;
    let mut report = GovernanceAgentGate7_6Report {
        schema: GOVERNANCE_AGENT_GATE_7_6_REPORT_SCHEMA.to_string(),
        gate: "7.6-ruleset-vs-llm-judgment".to_string(),
        verified,
        ruleset_file: options.ruleset_file.display().to_string(),
        comparison_dir: options.comparison_dir.display().to_string(),
        ruleset_hash,
        compiled_policy_hash: compiled_policy.compiled_policy_hash,
        fixture_count: checks.len(),
        direct_baseline_authoritative: false,
        manual_review_scope:
            "manual review is limited to Gate 7.6 evaluation; routine execution remains policy-driven"
                .to_string(),
        high_confidence_agreement_count,
        unsafe_direct_delta_rejection_count,
        ambiguous_no_op_count,
        top_k_overlap_count,
        false_add_count,
        false_remove_count,
        churn_behavior: "policy emitted zero mutations on all comparison fixtures".to_string(),
        concentration_behavior:
            "policy no-opped the concentration-risk fixture instead of admitting another correlated validator"
                .to_string(),
        disagreement_case_ids,
        comparison_checks: checks,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 7.6 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_8_5(
    options: GovernanceAgentGate8_5Options,
) -> io::Result<GovernanceAgentGate8_5Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 8.5 report",
    )?;
    ensure_output_can_be_written(
        &options.replay_bundle_file,
        options.overwrite,
        "governance agent Gate 8.5 replay bundle",
    )?;
    let validation = validate_governance_agent_source_bundle(&options.agent_dir)?;
    let architecture_statement_hash = governance_agent_statement_hash_by_name(
        &validation.statement_hashes,
        "architecture_statement",
    )?;
    let objective_statement_hash =
        governance_agent_statement_hash_by_name(&validation.statement_hashes, "objective_statement")?;
    let (ruleset_value, ruleset) = read_governance_ruleset_file(&options.ruleset_file)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        &ruleset_value,
    )?;
    let compiled_policy = compile_governance_agent_ruleset_policy(&ruleset, &ruleset_hash)?;
    let evidence = read_governance_agent_evidence_snapshot(&options.evidence_file)?;
    let evidence_value = serde_json::to_value(&evidence).map_err(invalid_data)?;
    let evidence_snapshot_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.frozen_evidence.v1",
        &evidence_value,
    )?;
    let validator_evidence_packet_root = evidence.validator_evidence_packet_root.clone();
    let registry_delta_candidate = execute_governance_agent_policy(
        &ruleset,
        &compiled_policy,
        &evidence,
        &evidence_snapshot_hash,
    )?;
    let replay_bundle = GovernanceAgentDryRunReplayBundle {
        schema: GOVERNANCE_AGENT_DRY_RUN_REPLAY_BUNDLE_SCHEMA.to_string(),
        bundle_hash: validation.bundle_hash.clone(),
        architecture_statement_hash: architecture_statement_hash.clone(),
        objective_statement_hash: objective_statement_hash.clone(),
        ruleset_hash: ruleset_hash.clone(),
        compiled_policy_hash: compiled_policy.compiled_policy_hash.clone(),
        evidence_snapshot_hash: evidence_snapshot_hash.clone(),
        registry_delta_candidate_hash: registry_delta_candidate.candidate_hash,
        validator_registry_root: evidence.validator_registry_root.clone(),
        validator_evidence_packet_root: validator_evidence_packet_root.clone(),
    };
    let replay_bundle_value = serde_json::to_value(&replay_bundle).map_err(invalid_data)?;
    let replay_bundle_root = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.dry_run_replay_bundle.v1",
        &replay_bundle_value,
    )?;
    let replay_bundle_json = serde_json::to_string_pretty(&replay_bundle).map_err(invalid_data)?;
    atomic_write(
        &options.replay_bundle_file,
        format!("{replay_bundle_json}\n"),
    )?;
    let replay_bundle_retrievable = governance_agent_replay_bundle_root_matches_file(
        &options.replay_bundle_file,
        &replay_bundle_root,
    )?;
    let bundle_hash = validation.bundle_hash.clone();
    let compiled_policy_hash = compiled_policy.compiled_policy_hash.clone();
    let validator_registry_root = evidence.validator_registry_root.clone();
    let report_root_value = serde_json::json!({
        "action_mode": GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE,
        "bundle_hash": bundle_hash.clone(),
        "compiled_policy_hash": compiled_policy_hash.clone(),
        "evidence_snapshot_hash": evidence_snapshot_hash.clone(),
        "gate": "8.5-cobalt-ruleset-dry-run",
        "registry_mutation_count": 0,
        "replay_bundle_root": replay_bundle_root.clone(),
        "ruleset_hash": ruleset_hash.clone(),
        "validator_evidence_packet_root": validator_evidence_packet_root.clone(),
        "validator_registry_root": validator_registry_root.clone(),
    });
    let report_root = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.gate_8_5.report_root.v1",
        &report_root_value,
    )?;
    let genesis = Genesis::new_with_validator_count("postfiat-dga-gate-8_5-local", 1);
    let mut dry_run = GovernanceAgentDryRunAmendment {
        schema: GOVERNANCE_AGENT_DRY_RUN_AMENDMENT_SCHEMA.to_string(),
        dry_run_id: String::new(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        action_mode: GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE.to_string(),
        expected_previous_dry_run_id: String::new(),
        bundle_hash,
        architecture_statement_hash,
        objective_statement_hash,
        ruleset_source_bundle_hash: validation.bundle_hash,
        ruleset_hash,
        compiled_policy_ruleset_hash: compiled_policy.ruleset_hash.clone(),
        compiled_policy_hash,
        replay_bundle_root,
        replay_bundle_uri: options.replay_bundle_file.display().to_string(),
        report_root,
        report_uri: options.output_file.display().to_string(),
        validator_registry_root_before: validator_registry_root.clone(),
        validator_registry_root_after: validator_registry_root,
        registry_mutation_count: 0,
    };
    dry_run.dry_run_id = governance_agent_dry_run_amendment_id(&dry_run);
    validate_governance_agent_dry_run_amendment(&dry_run)?;
    let batch = build_governance_action_batch_with_agent_dry_runs(
        &genesis,
        Vec::new(),
        Vec::new(),
        vec![dry_run.clone()],
    )?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    let mut governance = GovernanceState::new(genesis.validator_count);
    let receipts = execute_governance_batch(&mut governance, None, &batch, 1);
    let dry_run_recorded = receipts
        .iter()
        .any(|receipt| receipt.tx_id == dry_run.dry_run_id && receipt.accepted);
    let record = governance
        .governance_agent_dry_run_records
        .last()
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "governance agent dry run was not recorded",
            )
        })?;
    validate_governance_agent_dry_run_record("", &record)?;
    let rejection_checks =
        governance_agent_gate_8_5_rejection_checks(&governance, &dry_run, &record.dry_run_id);
    let stale_ruleset_rejected =
        governance_agent_rejection_check_passed(&rejection_checks, "stale_ruleset");
    let wrong_bundle_rejected =
        governance_agent_rejection_check_passed(&rejection_checks, "wrong_bundle");
    let missing_replay_root_rejected =
        governance_agent_rejection_check_passed(&rejection_checks, "missing_replay_root");
    let registry_unchanged = record.validator_registry_root_before == record.validator_registry_root_after
        && record.registry_mutation_count == 0;
    let verified = dry_run_recorded
        && registry_unchanged
        && governance.governance_agent_dry_run_records.len() == 1
        && stale_ruleset_rejected
        && wrong_bundle_rejected
        && missing_replay_root_rejected
        && replay_bundle_retrievable;
    let mut report = GovernanceAgentGate8_5Report {
        schema: GOVERNANCE_AGENT_GATE_8_5_REPORT_SCHEMA.to_string(),
        gate: "8.5-cobalt-ruleset-dry-run".to_string(),
        verified,
        action_mode: GOVERNANCE_AGENT_ACTION_MODE_DRY_RUN_VALIDATE.to_string(),
        dry_run_id: dry_run.dry_run_id,
        dry_run_record_id: record.record_id,
        governance_batch_id: batch.batch_id,
        bundle_hash: record.bundle_hash,
        architecture_statement_hash: record.architecture_statement_hash,
        objective_statement_hash: record.objective_statement_hash,
        ruleset_hash: record.ruleset_hash,
        compiled_policy_hash: record.compiled_policy_hash,
        evidence_snapshot_hash,
        validator_evidence_packet_root,
        replay_bundle_root: record.replay_bundle_root,
        replay_bundle_uri: record.replay_bundle_uri,
        report_root: record.report_root,
        report_uri: record.report_uri,
        validator_registry_root_before: record.validator_registry_root_before,
        validator_registry_root_after: record.validator_registry_root_after,
        registry_unchanged,
        registry_mutation_count: record.registry_mutation_count,
        cobalt_batch_verified: true,
        dry_run_recorded,
        governance_agent_record_count: governance.governance_agent_dry_run_records.len(),
        stale_ruleset_rejected,
        wrong_bundle_rejected,
        missing_replay_root_rejected,
        replay_bundle_retrievable,
        rejection_checks,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 8.5 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_9_5(
    options: GovernanceAgentGate9_5Options,
) -> io::Result<GovernanceAgentGate9_5Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 9.5 report",
    )?;
    let validation = validate_governance_agent_source_bundle(&options.agent_dir)?;
    let (ruleset_value, ruleset) =
        read_governance_guarded_apply_ruleset_file(&options.ruleset_file)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        &ruleset_value,
    )?;
    let compiled_policy =
        compile_governance_agent_guarded_apply_policy(&ruleset, &ruleset_hash)?;
    let evidence = read_governance_agent_evidence_snapshot(&options.evidence_file)?;
    let evidence_value = serde_json::to_value(&evidence).map_err(invalid_data)?;
    let evidence_snapshot_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.frozen_evidence.v1",
        &evidence_value,
    )?;

    let genesis = Genesis::new_with_validator_count("postfiat-dga-gate-9_5-local", 3);
    let domain = cobalt_domain(&genesis);
    let initial_validators = local_validator_ids(3)?;
    let admitted_validators = local_validator_ids(4)?;
    let mut registry = governance_agent_gate_9_5_registry(&initial_validators)?;
    let admitted_record = governance_agent_gate_9_5_validator_record("validator-3");
    let admitted_entry = governance_agent_registry_entry(&admitted_record, true);
    let initial_registry_root = validator_registry_root(&registry, &initial_validators)?;
    let mut admitted_registry = registry.clone();
    admitted_registry.validators.push(admitted_record);
    sort_validator_registry_records(&mut admitted_registry.validators);
    let guarded_apply_registry_root =
        validator_registry_root(&admitted_registry, &admitted_validators)?;

    let execution_context = GovernanceAgentGuardedApplyExecutionContext {
        evidence_snapshot_hash: &evidence_snapshot_hash,
        previous_validators: &initial_validators,
        new_validators: &admitted_validators,
        previous_registry_root: &initial_registry_root,
        new_registry_root: &guarded_apply_registry_root,
    };
    let mut candidate = execute_governance_agent_guarded_apply_policy(
        &ruleset,
        &compiled_policy,
        &evidence,
        &execution_context,
    )?;
    let hard_caps = governance_agent_guarded_apply_hard_caps();
    validate_governance_agent_guarded_apply_candidate(&candidate, &hard_caps)?;
    let rejection_checks =
        governance_agent_gate_9_5_rejection_checks(&candidate, &hard_caps)?;
    let max_one_add =
        governance_agent_guarded_apply_rejection_check_passed(&rejection_checks, "more_than_one_add");
    let zero_routine_removals =
        governance_agent_guarded_apply_rejection_check_passed(&rejection_checks, "routine_removal");
    let evidence_refs_valid = governance_agent_guarded_apply_evidence_refs_valid(&candidate);
    let concentration_caps_passed =
        governance_agent_guarded_apply_concentration_caps_passed(&candidate);
    let trust_graph_linkedness_passed =
        candidate.linkedness_passed && !candidate.linkedness_root.is_empty();
    let no_human_approval_after_activation = !candidate.human_approval_required;

    let admit_update = certify_validator_registry_update(
        &domain,
        &EssentialSubsetConfig::all_of(initial_validators.clone()),
        ValidatorRegistryUpdateRequest {
            activation_height: 1,
            previous_registry_root: initial_registry_root.clone(),
            new_registry_root: guarded_apply_registry_root.clone(),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: initial_validators.clone(),
            new_validators: admitted_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-3".to_string(),
            previous_record: None,
            new_record: Some(admitted_entry.clone()),
        },
        initial_validators.clone(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    verify_cobalt_validator_registry_update(&domain, &admit_update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let cobalt_batch = build_governance_action_batch_with_agent_dry_runs(
        &genesis,
        Vec::new(),
        vec![admit_update.clone()],
        Vec::new(),
    )?;
    verify_governance_action_batch_id(&genesis, &cobalt_batch)?;
    let mut governance = GovernanceState::new(genesis.validator_count);
    let pre_cobalt_registry_root = validator_registry_root(&registry, &initial_validators)?;
    let cobalt_receipts = execute_governance_batch(&mut governance, None, &cobalt_batch, 1);
    let cobalt_acceptance_verified = cobalt_receipts
        .iter()
        .any(|receipt| receipt.tx_id == admit_update.update_id && receipt.accepted)
        && governance
            .validator_registry_updates
            .iter()
            .any(|update| update.update_id == admit_update.update_id);
    if cobalt_acceptance_verified {
        apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &admit_update,
            admit_update.activation_height,
            "governance agent Gate 9.5 guarded apply",
        )?;
    }
    let post_apply_registry_root = validator_registry_root(&registry, &admitted_validators)?;
    let registry_changes_only_after_cobalt_acceptance = pre_cobalt_registry_root == initial_registry_root
        && cobalt_acceptance_verified
        && post_apply_registry_root == guarded_apply_registry_root;

    let rollback_update = certify_validator_registry_update(
        &domain,
        &EssentialSubsetConfig::all_of(admitted_validators.clone()),
        ValidatorRegistryUpdateRequest {
            activation_height: 2,
            previous_registry_root: guarded_apply_registry_root.clone(),
            new_registry_root: initial_registry_root.clone(),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: admitted_validators.clone(),
            new_validators: initial_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
            subject_node_id: "validator-3".to_string(),
            previous_record: Some(admitted_entry),
            new_record: None,
        },
        admitted_validators.clone(),
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    verify_cobalt_validator_registry_update(&domain, &rollback_update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let rollback_batch = build_governance_action_batch_with_agent_dry_runs(
        &genesis,
        Vec::new(),
        vec![rollback_update.clone()],
        Vec::new(),
    )?;
    verify_governance_action_batch_id(&genesis, &rollback_batch)?;
    let rollback_receipts = execute_governance_batch(&mut governance, None, &rollback_batch, 2);
    let rollback_cobalt_acceptance_verified = rollback_receipts
        .iter()
        .any(|receipt| receipt.tx_id == rollback_update.update_id && receipt.accepted)
        && governance
            .validator_registry_updates
            .iter()
            .any(|update| update.update_id == rollback_update.update_id);
    if rollback_cobalt_acceptance_verified {
        apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &rollback_update,
            rollback_update.activation_height,
            "governance agent Gate 9.5 rollback drill",
        )?;
    }
    let rollback_registry_root = validator_registry_root(&registry, &initial_validators)?;
    let rollback_restored_registry = rollback_registry_root == initial_registry_root;
    let rollback_available =
        candidate.rollback_required && rollback_cobalt_acceptance_verified && rollback_restored_registry;

    candidate.candidate_hash = governance_agent_guarded_apply_candidate_hash(&candidate)?;
    let verified = max_one_add
        && zero_routine_removals
        && evidence_refs_valid
        && concentration_caps_passed
        && trust_graph_linkedness_passed
        && no_human_approval_after_activation
        && registry_changes_only_after_cobalt_acceptance
        && rollback_available
        && rejection_checks
            .iter()
            .all(|check| check.rejected && check.error.is_some());
    let mut report = GovernanceAgentGate9_5Report {
        schema: GOVERNANCE_AGENT_GATE_9_5_REPORT_SCHEMA.to_string(),
        gate: "9.5-generated-ruleset-guarded-apply".to_string(),
        verified,
        bundle_hash: validation.bundle_hash,
        ruleset_file: options.ruleset_file.display().to_string(),
        evidence_file: options.evidence_file.display().to_string(),
        ruleset_hash,
        compiled_policy_hash: compiled_policy.compiled_policy_hash,
        evidence_snapshot_hash,
        hard_caps,
        candidate_hash: candidate.candidate_hash.clone(),
        candidate,
        initial_validator_count: initial_validators.len(),
        post_apply_validator_count: admitted_validators.len(),
        initial_registry_root,
        guarded_apply_registry_root,
        post_apply_registry_root,
        rollback_registry_root,
        cobalt_update_id: admit_update.update_id,
        cobalt_governance_batch_id: cobalt_batch.batch_id,
        rollback_update_id: rollback_update.update_id,
        rollback_governance_batch_id: rollback_batch.batch_id,
        cobalt_acceptance_verified,
        rollback_cobalt_acceptance_verified,
        registry_changes_only_after_cobalt_acceptance,
        max_one_add,
        zero_routine_removals,
        evidence_refs_valid,
        concentration_caps_passed,
        trust_graph_linkedness_passed,
        rollback_available,
        rollback_restored_registry,
        no_human_approval_after_activation,
        rejection_checks,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 9.5 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_10_1(
    options: GovernanceAgentGate10_1Options,
) -> io::Result<GovernanceAgentGate10_1Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 10.1 report",
    )?;
    let inputs = governance_agent_verifier_tier_inputs(
        &options.model_request_file,
        &options.ruleset_file,
        &options.gate_9_5_report_file,
    )?;
    let benchmark = governance_agent_verifier_cost_benchmark(&inputs)?;
    let verified = inputs.request_hash_recomputed
        && inputs.ruleset_hash_recomputed
        && inputs.candidate_hash_recomputed
        && inputs.gate_9_5_report.verified
        && !benchmark.generic_one_percent_claim_assumed
        && benchmark.full_inference_work_units > 0
        && benchmark.measured_verifier_work_units > 0
        && benchmark.verifier_to_full_cost_bps > 0;
    let mut report = GovernanceAgentGate10_1Report {
        schema: GOVERNANCE_AGENT_GATE_10_1_REPORT_SCHEMA.to_string(),
        gate: "10.1-verillm-postfiat-benchmark".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        ruleset_file: options.ruleset_file.display().to_string(),
        gate_9_5_report_file: options.gate_9_5_report_file.display().to_string(),
        model_request_hash: inputs.model_request.request_hash.clone(),
        validator_evidence_packet_schema_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        ruleset_hash: inputs.ruleset_hash.clone(),
        gate_9_5_report_hash: inputs.gate_9_5_report_hash.clone(),
        candidate_hash: inputs.gate_9_5_report.candidate_hash.clone(),
        benchmark,
        request_hash_recomputed: inputs.request_hash_recomputed,
        ruleset_hash_recomputed: inputs.ruleset_hash_recomputed,
        candidate_hash_recomputed: inputs.candidate_hash_recomputed,
        gate_9_5_report_verified: inputs.gate_9_5_report.verified,
        cost_measured_not_assumed: true,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 10.1 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_10_5(
    options: GovernanceAgentGate10_5Options,
) -> io::Result<GovernanceAgentGate10_5Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 10.5 report",
    )?;
    let inputs = governance_agent_verifier_tier_inputs(
        &options.model_request_file,
        &options.ruleset_file,
        &options.gate_9_5_report_file,
    )?;
    let compact_commitment = governance_agent_compact_receipt_commitment(&inputs)?;
    let mut receipt =
        governance_agent_inference_receipt_prototype(&inputs, &compact_commitment, "")?;
    let accepted_attestations =
        governance_agent_receipt_verifier_attestations(&receipt, &inputs, &compact_commitment)?;
    let accepted_root = governance_agent_verifier_attestation_root(&accepted_attestations)?;
    receipt.verifier_attestation_root = accepted_root;
    receipt.receipt_id = governance_agent_inference_receipt_id(&receipt)?;
    let correct_receipt_accepted =
        validate_governance_agent_inference_receipt(&receipt, &inputs, &compact_commitment).is_ok();
    let mut tampered = receipt.clone();
    tampered.generated_action_hash = hash_hex(
        "postfiat.governance_agent.gate_10_5.tampered_action.v1",
        receipt.generated_action_hash.as_bytes(),
    );
    tampered.receipt_id = governance_agent_inference_receipt_id(&tampered)?;
    let incorrect_receipt_rejected =
        validate_governance_agent_inference_receipt(&tampered, &inputs, &compact_commitment)
            .is_err();
    let mut verifier_attestations = accepted_attestations;
    verifier_attestations.push(governance_agent_verifier_attestation(
        &receipt.receipt_id,
        "shadow-verifier-disagreement",
        "tampered_receipt_probe",
        false,
        Some("generated action hash mismatch"),
    ));
    let verifier_attestation_root =
        governance_agent_verifier_attestation_root(&verifier_attestations)?;
    receipt.verifier_attestation_root = verifier_attestation_root;
    receipt.receipt_id = governance_agent_inference_receipt_id(&receipt)?;
    let accepted_verifier_count = verifier_attestations
        .iter()
        .filter(|attestation| attestation.accepted)
        .count();
    let verifier_quorum = 3usize;
    let verifier_disagreement_recorded = verifier_attestations
        .iter()
        .any(|attestation| !attestation.accepted && attestation.error.is_some());
    let verified = correct_receipt_accepted
        && incorrect_receipt_rejected
        && verifier_disagreement_recorded
        && accepted_verifier_count >= verifier_quorum
        && compact_commitment.chunk_count > 0
        && compact_commitment.compact_bytes < compact_commitment.direct_embedding_bytes;
    let mut report = GovernanceAgentGate10_5Report {
        schema: GOVERNANCE_AGENT_GATE_10_5_REPORT_SCHEMA.to_string(),
        gate: "10.5-toploc-receipt-prototype".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        ruleset_file: options.ruleset_file.display().to_string(),
        gate_9_5_report_file: options.gate_9_5_report_file.display().to_string(),
        validator_evidence_packet_schema_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        receipt,
        compact_commitment,
        verifier_attestations,
        accepted_verifier_count,
        verifier_quorum,
        correct_receipt_accepted,
        incorrect_receipt_rejected,
        verifier_disagreement_recorded,
        consensus_critical: false,
        prototype_only: true,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 10.5 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_14(
    options: GovernanceAgentGate14Options,
) -> io::Result<GovernanceAgentGate14Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 14 report",
    )?;
    let inputs = governance_agent_verifier_tier_inputs(
        &options.model_request_file,
        &options.ruleset_file,
        &options.gate_9_5_report_file,
    )?;
    let receipt_report =
        read_governance_agent_gate_10_5_report(&options.receipt_report_file)?;
    let receipt_report_verified =
        validate_governance_agent_gate_10_5_report_for_inputs(&receipt_report, &inputs).is_ok();
    let canonical_output_hash = inputs.ruleset_hash.clone();
    let tp_checks = vec![
        GovernanceAgentTensorParallelCheck {
            tensor_parallelism: 1,
            evidence_hash: Some(receipt_report.receipt.compact_commitment_root.clone()),
            output_hash: Some(canonical_output_hash.clone()),
            matches_canonical_tp1: true,
            admitted: true,
            reason: "canonical deterministic profile remains TP=1".to_string(),
        },
        governance_agent_missing_tp_check(2),
        governance_agent_missing_tp_check(4),
        governance_agent_missing_tp_check(8),
    ];
    let cross_tp_hash_agreement_demonstrated = tp_checks
        .iter()
        .filter(|check| check.tensor_parallelism > 1)
        .any(|check| check.matches_canonical_tp1 && check.output_hash.is_some());
    let tp_greater_than_one_admitted = tp_checks
        .iter()
        .any(|check| check.tensor_parallelism > 1 && check.admitted);
    let tp_invariant_admission_ready =
        cross_tp_hash_agreement_demonstrated && !tp_greater_than_one_admitted;
    let shadow_plan = governance_agent_validator_side_shadow_plan();
    let validator_side_shadow_path_defined = !shadow_plan.steps.is_empty()
        && !shadow_plan.sidecars_live
        && !shadow_plan.commit_reveal_live
        && !shadow_plan.authority_transfer_live;
    let authority_transfer_live = shadow_plan.authority_transfer_live;
    let verified = receipt_report_verified
        && validator_side_shadow_path_defined
        && !authority_transfer_live
        && !tp_greater_than_one_admitted
        && !tp_invariant_admission_ready;
    let mut report = GovernanceAgentGate14Report {
        schema: GOVERNANCE_AGENT_GATE_14_REPORT_SCHEMA.to_string(),
        gate: "14-tp-invariant-verifier-admission".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        ruleset_file: options.ruleset_file.display().to_string(),
        gate_9_5_report_file: options.gate_9_5_report_file.display().to_string(),
        receipt_report_file: options.receipt_report_file.display().to_string(),
        validator_evidence_packet_schema_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        canonical_tensor_parallelism: 1,
        canonical_output_hash,
        receipt_report_verified,
        tp_checks,
        cross_tp_hash_agreement_demonstrated,
        tp_greater_than_one_admitted,
        tp_invariant_admission_ready,
        validator_side_shadow_path_defined,
        shadow_plan,
        authority_transfer_live,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 14 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_gate_15(
    options: GovernanceAgentGate15Options,
) -> io::Result<GovernanceAgentGate15Report> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance agent Gate 15 report",
    )?;
    let inputs = governance_agent_verifier_tier_inputs(
        &options.model_request_file,
        &options.ruleset_file,
        &options.gate_9_5_report_file,
    )?;
    let receipt_report = read_governance_agent_gate_10_5_report(&options.receipt_report_file)?;
    validate_governance_agent_gate_10_5_report_for_inputs(&receipt_report, &inputs)?;
    let gate_14_report = read_governance_agent_gate_14_report(&options.gate_14_report_file)?;
    validate_governance_agent_gate_14_report_for_inputs(
        &gate_14_report,
        &receipt_report,
        &inputs,
    )?;

    let mut probes = Vec::new();
    probes.extend(governance_agent_receipt_tamper_probes(
        &receipt_report,
        &inputs,
    )?);
    probes.extend(governance_agent_stale_or_missing_evidence_probes(
        &receipt_report,
        &gate_14_report,
        &inputs,
    )?);
    probes.extend(governance_agent_verifier_disagreement_probes(&receipt_report));
    probes.extend(governance_agent_authority_transfer_guard_probes(
        &gate_14_report,
        &receipt_report,
        &inputs,
    )?);

    let receipt_probe_count = governance_agent_probe_count(&probes, "receipt_tamper");
    let evidence_probe_count = governance_agent_probe_count(&probes, "lineage_evidence");
    let disagreement_probe_count =
        governance_agent_probe_count(&probes, "verifier_disagreement");
    let authority_probe_count = governance_agent_probe_count(&probes, "authority_transfer");
    let tampered_receipts_rejected =
        governance_agent_all_category_probes_rejected(&probes, "receipt_tamper");
    let stale_or_missing_evidence_rejected =
        governance_agent_all_category_probes_rejected(&probes, "lineage_evidence");
    let verifier_disagreement_shadow_only =
        governance_agent_all_category_probes_rejected(&probes, "verifier_disagreement")
            && receipt_report.verifier_disagreement_recorded
            && receipt_report.prototype_only
            && !receipt_report.consensus_critical;
    let authority_transfer_guarded =
        governance_agent_all_category_probes_rejected(&probes, "authority_transfer")
            && !gate_14_report.authority_transfer_live
            && !gate_14_report.shadow_plan.sidecars_live
            && !gate_14_report.shadow_plan.commit_reveal_live;
    let tp_greater_than_one_still_inadmissible = !gate_14_report.tp_greater_than_one_admitted
        && gate_14_report
            .tp_checks
            .iter()
            .filter(|check| check.tensor_parallelism > 1)
            .all(|check| !check.admitted);
    let sidecars_live = gate_14_report.shadow_plan.sidecars_live;
    let commit_reveal_live = gate_14_report.shadow_plan.commit_reveal_live;
    let authority_transfer_live = gate_14_report.authority_transfer_live;
    let verified = receipt_probe_count >= 3
        && evidence_probe_count >= 3
        && disagreement_probe_count >= 2
        && authority_probe_count >= 4
        && tampered_receipts_rejected
        && stale_or_missing_evidence_rejected
        && verifier_disagreement_shadow_only
        && authority_transfer_guarded
        && tp_greater_than_one_still_inadmissible
        && !sidecars_live
        && !commit_reveal_live
        && !authority_transfer_live
        && probes
            .iter()
            .all(|probe| probe.rejected && !probe.authority_changed && probe.error.is_some());

    let mut report = GovernanceAgentGate15Report {
        schema: GOVERNANCE_AGENT_GATE_15_REPORT_SCHEMA.to_string(),
        gate: "15-adversarial-governance-probes".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        ruleset_file: options.ruleset_file.display().to_string(),
        gate_9_5_report_file: options.gate_9_5_report_file.display().to_string(),
        receipt_report_file: options.receipt_report_file.display().to_string(),
        gate_14_report_file: options.gate_14_report_file.display().to_string(),
        validator_evidence_packet_schema_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_packet_schema_hash
            .clone(),
        validator_evidence_field_registry_hash: inputs
            .model_request
            .governed_inputs
            .validator_evidence_field_registry_hash
            .clone(),
        receipt_probe_count,
        evidence_probe_count,
        disagreement_probe_count,
        authority_probe_count,
        tampered_receipts_rejected,
        stale_or_missing_evidence_rejected,
        verifier_disagreement_shadow_only,
        authority_transfer_guarded,
        tp_greater_than_one_still_inadmissible,
        sidecars_live,
        commit_reveal_live,
        authority_transfer_live,
        probes,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    let json = serde_json::to_string_pretty(&report).map_err(invalid_data)?;
    atomic_write(&options.output_file, format!("{json}\n"))?;
    if !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent Gate 15 failed; wrote report `{}`",
                options.output_file.display()
            ),
        ));
    }
    Ok(report)
}

pub fn governance_agent_evidence_lineage_audit(
    options: GovernanceAgentEvidenceLineageAuditOptions,
) -> io::Result<GovernanceAgentEvidenceLineageAuditReport> {
    let model_request_value =
        read_governance_agent_json_value(&options.model_request_file, "governance model request")?;
    let model_request: GovernanceAgentModelRequest =
        serde_json::from_value(model_request_value).map_err(invalid_data)?;
    ensure_governance_agent_model_request_json_only(&model_request)?;
    let recomputed_request_hash = governance_agent_full_model_request_hash(&model_request)?;
    let model_request_hash_recomputed = model_request.request_hash == recomputed_request_hash;
    let expected_packet_schema_hash = model_request
        .governed_inputs
        .validator_evidence_packet_schema_hash
        .clone();
    let expected_field_registry_hash = model_request
        .governed_inputs
        .validator_evidence_field_registry_hash
        .clone();
    validate_governance_agent_hash_hex(
        "model request validator_evidence_packet_schema_hash",
        &expected_packet_schema_hash,
    )?;
    validate_governance_agent_hash_hex(
        "model request validator_evidence_field_registry_hash",
        &expected_field_registry_hash,
    )?;

    let report_specs = [
        (
            "gate_3_5",
            options.gate_3_5_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_3_5_REPORT_SCHEMA,
            "3.5-deterministic-ruleset-generation",
        ),
        (
            "gate_3_6",
            options.gate_3_6_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_3_6_REPORT_SCHEMA,
            "3.6-timelocked-governance-agent",
        ),
        (
            "gate_10_1",
            options.gate_10_1_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_10_1_REPORT_SCHEMA,
            "10.1-verillm-postfiat-benchmark",
        ),
        (
            "gate_10_5",
            options.receipt_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_10_5_REPORT_SCHEMA,
            "10.5-toploc-receipt-prototype",
        ),
        (
            "gate_14",
            options.gate_14_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_14_REPORT_SCHEMA,
            "14-tp-invariant-verifier-admission",
        ),
        (
            "gate_15",
            options.gate_15_report_file.as_path(),
            GOVERNANCE_AGENT_GATE_15_REPORT_SCHEMA,
            "15-adversarial-governance-probes",
        ),
    ];
    let mut reports = Vec::with_capacity(report_specs.len());
    for (name, path, expected_schema, expected_gate) in report_specs {
        reports.push(governance_agent_evidence_lineage_audit_item(
            name,
            path,
            expected_schema,
            expected_gate,
            &model_request,
        )?);
    }

    let drift_count = reports
        .iter()
        .filter(|report| {
            !report.schema_matches_expected
                || !report.gate_matches_expected
                || !report.matches_model_request
        })
        .count();
    let all_reports_verified = reports.iter().all(|report| report.verified);
    let verified = model_request_hash_recomputed
        && all_reports_verified
        && drift_count == 0
        && !reports.is_empty();
    let mut report = GovernanceAgentEvidenceLineageAuditReport {
        schema: GOVERNANCE_AGENT_EVIDENCE_LINEAGE_AUDIT_SCHEMA.to_string(),
        gate: "validator-evidence-lineage-audit".to_string(),
        verified,
        model_request_file: options.model_request_file.display().to_string(),
        model_request_hash: model_request.request_hash,
        model_request_hash_recomputed,
        validator_evidence_packet_schema_hash: expected_packet_schema_hash,
        validator_evidence_field_registry_hash: expected_field_registry_hash,
        report_count: reports.len(),
        drift_count,
        all_reports_verified,
        no_network_access: true,
        no_model_access: true,
        no_filesystem_mutation: true,
        no_direct_state_mutation: true,
        zero_cobalt_registry_mutation: true,
        reports,
        redaction_checked: false,
    };
    let probe = serde_json::to_string(&report).map_err(invalid_data)?;
    ensure_governance_agent_report_redacted(&probe)?;
    report.redaction_checked = true;
    Ok(report)
}
