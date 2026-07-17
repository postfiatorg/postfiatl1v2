struct GovernanceAgentVerifierTierInputs {
    model_request_value: serde_json::Value,
    model_request: GovernanceAgentModelRequest,
    model_request_bytes: Vec<u8>,
    ruleset_value: serde_json::Value,
    ruleset_bytes: Vec<u8>,
    ruleset_hash: String,
    gate_9_5_report: GovernanceAgentGate9_5Report,
    gate_9_5_report_bytes: Vec<u8>,
    gate_9_5_report_hash: String,
    runtime_manifest_hash: String,
    request_hash_recomputed: bool,
    ruleset_hash_recomputed: bool,
    candidate_hash_recomputed: bool,
}

fn governance_agent_verifier_tier_inputs(
    model_request_file: &Path,
    ruleset_file: &Path,
    gate_9_5_report_file: &Path,
) -> io::Result<GovernanceAgentVerifierTierInputs> {
    let model_request_bytes =
        read_bounded_governance_agent_bytes(model_request_file, "governance agent model request")?;
    let model_request_value: serde_json::Value =
        serde_json::from_slice(&model_request_bytes).map_err(invalid_data)?;
    let model_request: GovernanceAgentModelRequest =
        serde_json::from_value(model_request_value.clone()).map_err(invalid_data)?;
    ensure_governance_agent_model_request_json_only(&model_request)?;
    validate_governance_agent_hash_hex("model_request_hash", &model_request.request_hash)?;
    let recomputed_request_hash = governance_agent_full_model_request_hash(&model_request)?;
    let request_hash_recomputed = model_request.request_hash == recomputed_request_hash;
    let ruleset_bytes =
        read_bounded_governance_agent_bytes(ruleset_file, "governance agent verifier ruleset")?;
    let ruleset_value: serde_json::Value =
        serde_json::from_slice(&ruleset_bytes).map_err(invalid_data)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        &ruleset_value,
    )?;
    let gate_9_5_report_bytes = read_bounded_governance_agent_bytes(
        gate_9_5_report_file,
        "governance agent Gate 9.5 report",
    )?;
    let gate_9_5_report_value: serde_json::Value =
        serde_json::from_slice(&gate_9_5_report_bytes).map_err(invalid_data)?;
    let gate_9_5_report: GovernanceAgentGate9_5Report =
        serde_json::from_value(gate_9_5_report_value.clone()).map_err(invalid_data)?;
    validate_governance_agent_gate_9_5_report(&gate_9_5_report)?;
    let gate_9_5_report_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.gate_9_5.report.v1",
        &gate_9_5_report_value,
    )?;
    let runtime_manifest_value =
        serde_json::to_value(&model_request.runtime_manifest).map_err(invalid_data)?;
    let runtime_manifest_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.runtime_manifest.v1",
        &runtime_manifest_value,
    )?;
    let candidate_hash = governance_agent_guarded_apply_candidate_hash(&gate_9_5_report.candidate)?;
    let candidate_hash_recomputed = gate_9_5_report.candidate_hash == candidate_hash
        && gate_9_5_report.candidate.candidate_hash == candidate_hash;
    let ruleset_hash_recomputed = gate_9_5_report.ruleset_hash == ruleset_hash;
    Ok(GovernanceAgentVerifierTierInputs {
        model_request_value,
        model_request,
        model_request_bytes,
        ruleset_value,
        ruleset_bytes,
        ruleset_hash,
        gate_9_5_report,
        gate_9_5_report_bytes,
        gate_9_5_report_hash,
        runtime_manifest_hash,
        request_hash_recomputed,
        ruleset_hash_recomputed,
        candidate_hash_recomputed,
    })
}

fn validate_governance_agent_gate_9_5_report(
    report: &GovernanceAgentGate9_5Report,
) -> io::Result<()> {
    if report.schema != GOVERNANCE_AGENT_GATE_9_5_REPORT_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent Gate 9.5 report schema",
        ));
    }
    validate_governance_agent_hash_hex("Gate 9.5 bundle_hash", &report.bundle_hash)?;
    validate_governance_agent_hash_hex("Gate 9.5 ruleset_hash", &report.ruleset_hash)?;
    validate_governance_agent_hash_hex(
        "Gate 9.5 compiled_policy_hash",
        &report.compiled_policy_hash,
    )?;
    validate_governance_agent_hash_hex(
        "Gate 9.5 evidence_snapshot_hash",
        &report.evidence_snapshot_hash,
    )?;
    validate_governance_agent_guarded_apply_candidate(&report.candidate, &report.hard_caps)?;
    if report.candidate_hash != report.candidate.candidate_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 9.5 candidate hash mismatch",
        ));
    }
    if !report.verified
        || !report.cobalt_acceptance_verified
        || !report.rollback_cobalt_acceptance_verified
        || !report.rollback_available
        || !report.rollback_restored_registry
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 9.5 report is not verified for verifier-tier input",
        ));
    }
    Ok(())
}

fn governance_agent_verifier_cost_benchmark(
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<GovernanceAgentVerifierCostBenchmark> {
    let candidate_value = serde_json::to_value(&inputs.gate_9_5_report.candidate)
        .map_err(invalid_data)?;
    let hash_replay_value = serde_json::json!({
        "bundle_hash": inputs.gate_9_5_report.bundle_hash.as_str(),
        "candidate_hash": inputs.gate_9_5_report.candidate_hash.as_str(),
        "compiled_policy_hash": inputs.gate_9_5_report.compiled_policy_hash.as_str(),
        "evidence_snapshot_hash": inputs.gate_9_5_report.evidence_snapshot_hash.as_str(),
        "model_request_hash": inputs.model_request.request_hash.as_str(),
        "rollback_update_id": inputs.gate_9_5_report.rollback_update_id.as_str(),
        "ruleset_hash": inputs.gate_9_5_report.ruleset_hash.as_str(),
        "update_id": inputs.gate_9_5_report.cobalt_update_id.as_str()
    });
    let hash_replay_bytes = governance_agent_canonical_json_bytes(&hash_replay_value)?.len();
    let schema_verifier_bytes = governance_agent_canonical_json_bytes(&inputs.ruleset_value)?.len();
    let selector_verifier_bytes = governance_agent_canonical_json_bytes(&candidate_value)?.len();
    let measured_verifier_work_units =
        hash_replay_bytes + schema_verifier_bytes + selector_verifier_bytes;
    let full_inference_input_bytes = inputs.model_request_bytes.len();
    let full_inference_output_bytes = inputs.ruleset_bytes.len() + inputs.gate_9_5_report_bytes.len();
    let full_inference_work_units = full_inference_input_bytes + full_inference_output_bytes;
    let verifier_to_full_cost_bps =
        governance_agent_ratio_bps(measured_verifier_work_units, full_inference_work_units)?;
    Ok(GovernanceAgentVerifierCostBenchmark {
        measurement_method:
            "deterministic local byte/work-unit measurement over PostFiat governed artifacts; no generic VeriLLM percentage is assumed"
                .to_string(),
        full_inference_input_bytes,
        full_inference_output_bytes,
        full_inference_work_units,
        hash_replay_bytes,
        schema_verifier_bytes,
        selector_verifier_bytes,
        measured_verifier_work_units,
        verifier_to_full_cost_bps,
        generic_one_percent_claim_assumed: false,
    })
}

fn governance_agent_compact_receipt_commitment(
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<GovernanceAgentCompactReceiptCommitment> {
    let prompt = governance_agent_model_request_prompt_text(&inputs.model_request_value);
    let tokens = governance_agent_token_proxy_values(&prompt);
    let prompt_token_proxy_count = tokens.len().max(1);
    let chunk_size_tokens = 32usize;
    let chunk_count = prompt_token_proxy_count.div_ceil(chunk_size_tokens);
    let mut chunk_hashes = Vec::with_capacity(chunk_count);
    for chunk in tokens.chunks(chunk_size_tokens) {
        let text = chunk.join(" ");
        chunk_hashes.push(hash_hex(
            "postfiat.governance_agent.gate_10_5.toploc_chunk.v1",
            text.as_bytes(),
        ));
    }
    if chunk_hashes.is_empty() {
        chunk_hashes.push(hash_hex(
            "postfiat.governance_agent.gate_10_5.toploc_chunk.v1",
            b"",
        ));
    }
    let compact_bytes = chunk_count * 258;
    let direct_embedding_bytes = chunk_count * 262_144;
    let compact_to_direct_bps = governance_agent_ratio_bps(compact_bytes, direct_embedding_bytes)?;
    let chunk_value = serde_json::json!({
        "chunk_hashes": chunk_hashes,
        "chunk_size_tokens": chunk_size_tokens,
        "model_request_hash": inputs.model_request.request_hash,
        "prompt_token_proxy_count": prompt_token_proxy_count
    });
    let chunk_commitment_root = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.gate_10_5.toploc_commitment.v1",
        &chunk_value,
    )?;
    Ok(GovernanceAgentCompactReceiptCommitment {
        algorithm: "toploc-style-locality-hash-prototype-v0".to_string(),
        token_proxy_method: "unicode-whitespace-token-proxy".to_string(),
        chunk_size_tokens,
        chunk_count,
        prompt_token_proxy_count,
        compact_bytes,
        direct_embedding_bytes,
        compact_to_direct_bps,
        chunk_commitment_root,
    })
}

fn governance_agent_inference_receipt_prototype(
    inputs: &GovernanceAgentVerifierTierInputs,
    compact_commitment: &GovernanceAgentCompactReceiptCommitment,
    verifier_attestation_root: &str,
) -> io::Result<GovernanceAgentInferenceReceiptPrototype> {
    let mut receipt = GovernanceAgentInferenceReceiptPrototype {
        schema: GOVERNANCE_AGENT_INFERENCE_RECEIPT_PROTOTYPE_SCHEMA.to_string(),
        receipt_id: String::new(),
        bundle_hash: inputs.gate_9_5_report.bundle_hash.clone(),
        evidence_snapshot_root: inputs.gate_9_5_report.evidence_snapshot_hash.clone(),
        model_request_hash: inputs.model_request.request_hash.clone(),
        model_response_hash: inputs.ruleset_hash.clone(),
        parsed_output_hash: inputs.ruleset_hash.clone(),
        generated_action_hash: inputs.gate_9_5_report.candidate_hash.clone(),
        provider: "local-deterministic-replay".to_string(),
        provider_run_id: "gate-10_5-local-prototype".to_string(),
        hardware_class: "controlled-testnet-local".to_string(),
        runtime_manifest_hash: inputs.runtime_manifest_hash.clone(),
        signer: "shadow-verifier-0".to_string(),
        signature_required: false,
        compact_commitment_root: compact_commitment.chunk_commitment_root.clone(),
        verifier_attestation_root: verifier_attestation_root.to_string(),
    };
    receipt.receipt_id = governance_agent_inference_receipt_id(&receipt)?;
    Ok(receipt)
}

fn validate_governance_agent_inference_receipt(
    receipt: &GovernanceAgentInferenceReceiptPrototype,
    inputs: &GovernanceAgentVerifierTierInputs,
    compact_commitment: &GovernanceAgentCompactReceiptCommitment,
) -> io::Result<()> {
    if receipt.schema != GOVERNANCE_AGENT_INFERENCE_RECEIPT_PROTOTYPE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent inference receipt schema",
        ));
    }
    validate_governance_agent_hash_hex("receipt_id", &receipt.receipt_id)?;
    validate_governance_agent_hash_hex("receipt bundle_hash", &receipt.bundle_hash)?;
    validate_governance_agent_hash_hex(
        "receipt evidence_snapshot_root",
        &receipt.evidence_snapshot_root,
    )?;
    validate_governance_agent_hash_hex("receipt model_request_hash", &receipt.model_request_hash)?;
    validate_governance_agent_hash_hex("receipt model_response_hash", &receipt.model_response_hash)?;
    validate_governance_agent_hash_hex("receipt parsed_output_hash", &receipt.parsed_output_hash)?;
    validate_governance_agent_hash_hex(
        "receipt generated_action_hash",
        &receipt.generated_action_hash,
    )?;
    validate_governance_agent_hash_hex(
        "receipt runtime_manifest_hash",
        &receipt.runtime_manifest_hash,
    )?;
    validate_governance_agent_hash_hex(
        "receipt compact_commitment_root",
        &receipt.compact_commitment_root,
    )?;
    if !receipt.verifier_attestation_root.is_empty() {
        validate_governance_agent_hash_hex(
            "receipt verifier_attestation_root",
            &receipt.verifier_attestation_root,
        )?;
    }
    if receipt.bundle_hash != inputs.gate_9_5_report.bundle_hash
        || receipt.evidence_snapshot_root != inputs.gate_9_5_report.evidence_snapshot_hash
        || receipt.model_request_hash != inputs.model_request.request_hash
        || receipt.model_response_hash != inputs.ruleset_hash
        || receipt.parsed_output_hash != inputs.ruleset_hash
        || receipt.generated_action_hash != inputs.gate_9_5_report.candidate_hash
        || receipt.runtime_manifest_hash != inputs.runtime_manifest_hash
        || receipt.compact_commitment_root != compact_commitment.chunk_commitment_root
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance inference receipt does not match verifier-tier inputs",
        ));
    }
    if receipt.signature_required {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "prototype receipt must not pretend production signatures are live",
        ));
    }
    if receipt.receipt_id != governance_agent_inference_receipt_id(receipt)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance inference receipt id mismatch",
        ));
    }
    Ok(())
}

fn governance_agent_receipt_verifier_attestations(
    receipt: &GovernanceAgentInferenceReceiptPrototype,
    inputs: &GovernanceAgentVerifierTierInputs,
    compact_commitment: &GovernanceAgentCompactReceiptCommitment,
) -> io::Result<Vec<GovernanceAgentVerifierAttestation>> {
    let hash_replay_ok = receipt.bundle_hash == inputs.gate_9_5_report.bundle_hash
        && receipt.generated_action_hash == inputs.gate_9_5_report.candidate_hash;
    let schema_ok = inputs.ruleset_hash_recomputed && inputs.gate_9_5_report.verified;
    let selector_ok = inputs.candidate_hash_recomputed
        && compact_commitment.chunk_commitment_root == receipt.compact_commitment_root;
    Ok(vec![
        governance_agent_verifier_attestation(
            &receipt.receipt_id,
            "shadow-verifier-hash-replay",
            "hash_replay",
            hash_replay_ok,
            (!hash_replay_ok).then_some("hash replay mismatch"),
        ),
        governance_agent_verifier_attestation(
            &receipt.receipt_id,
            "shadow-verifier-schema",
            "schema",
            schema_ok,
            (!schema_ok).then_some("schema verification mismatch"),
        ),
        governance_agent_verifier_attestation(
            &receipt.receipt_id,
            "shadow-verifier-selector",
            "selector",
            selector_ok,
            (!selector_ok).then_some("selector verification mismatch"),
        ),
    ])
}

fn governance_agent_verifier_attestation(
    receipt_id: &str,
    verifier_id: &str,
    verifier_kind: &str,
    accepted: bool,
    error: Option<&str>,
) -> GovernanceAgentVerifierAttestation {
    let payload = format!("{receipt_id}:{verifier_id}:{verifier_kind}:{accepted}:{error:?}");
    GovernanceAgentVerifierAttestation {
        attestation_id: hash_hex(
            "postfiat.governance_agent.verifier_attestation.v1",
            payload.as_bytes(),
        ),
        verifier_id: verifier_id.to_string(),
        verifier_kind: verifier_kind.to_string(),
        accepted,
        error: error.map(str::to_string),
    }
}

fn governance_agent_verifier_attestation_root(
    attestations: &[GovernanceAgentVerifierAttestation],
) -> io::Result<String> {
    let value = serde_json::to_value(attestations).map_err(invalid_data)?;
    governance_agent_canonical_json_hash(
        "postfiat.governance_agent.verifier_attestation_root.v1",
        &value,
    )
}

fn governance_agent_inference_receipt_id(
    receipt: &GovernanceAgentInferenceReceiptPrototype,
) -> io::Result<String> {
    let mut value = serde_json::to_value(receipt).map_err(invalid_data)?;
    let object = value.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance inference receipt must encode as a JSON object",
        )
    })?;
    object.remove("receipt_id");
    governance_agent_canonical_json_hash("postfiat.governance_agent.inference_receipt.v1", &value)
}

fn read_governance_agent_gate_10_5_report(
    path: &Path,
) -> io::Result<GovernanceAgentGate10_5Report> {
    let value = read_governance_agent_json_value(path, "governance agent Gate 10.5 report")?;
    let report: GovernanceAgentGate10_5Report =
        serde_json::from_value(value).map_err(invalid_data)?;
    if report.schema != GOVERNANCE_AGENT_GATE_10_5_REPORT_SCHEMA || !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent Gate 10.5 report is not verified",
        ));
    }
    Ok(report)
}

fn validate_governance_agent_gate_10_5_report_for_inputs(
    report: &GovernanceAgentGate10_5Report,
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<()> {
    if report.schema != GOVERNANCE_AGENT_GATE_10_5_REPORT_SCHEMA || !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent Gate 10.5 report is not verified",
        ));
    }
    validate_governance_agent_report_validator_evidence_lineage(
        "Gate 10.5 report",
        &report.validator_evidence_packet_schema_hash,
        &report.validator_evidence_field_registry_hash,
        &inputs.model_request,
    )?;
    validate_governance_agent_inference_receipt(
        &report.receipt,
        inputs,
        &report.compact_commitment,
    )?;
    let verifier_attestation_root =
        governance_agent_verifier_attestation_root(&report.verifier_attestations)?;
    if report.receipt.verifier_attestation_root != verifier_attestation_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 10.5 verifier attestation root mismatch",
        ));
    }
    let accepted_verifier_count = report
        .verifier_attestations
        .iter()
        .filter(|attestation| attestation.accepted)
        .count();
    if accepted_verifier_count != report.accepted_verifier_count
        || report.accepted_verifier_count < report.verifier_quorum
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 10.5 verifier quorum mismatch",
        ));
    }
    if !report.correct_receipt_accepted
        || !report.incorrect_receipt_rejected
        || !report.verifier_disagreement_recorded
        || report.consensus_critical
        || !report.prototype_only
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 10.5 report does not preserve shadow-only verifier semantics",
        ));
    }
    if !report
        .verifier_attestations
        .iter()
        .any(|attestation| !attestation.accepted && attestation.error.is_some())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 10.5 report is missing recorded verifier disagreement",
        ));
    }
    Ok(())
}

fn read_governance_agent_gate_14_report(path: &Path) -> io::Result<GovernanceAgentGate14Report> {
    let value = read_governance_agent_json_value(path, "governance agent Gate 14 report")?;
    let report: GovernanceAgentGate14Report =
        serde_json::from_value(value).map_err(invalid_data)?;
    if report.schema != GOVERNANCE_AGENT_GATE_14_REPORT_SCHEMA || !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent Gate 14 report is not verified",
        ));
    }
    Ok(report)
}

fn validate_governance_agent_gate_14_report_for_inputs(
    report: &GovernanceAgentGate14Report,
    receipt_report: &GovernanceAgentGate10_5Report,
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<()> {
    if report.schema != GOVERNANCE_AGENT_GATE_14_REPORT_SCHEMA || !report.verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent Gate 14 report is not verified",
        ));
    }
    validate_governance_agent_report_validator_evidence_lineage(
        "Gate 14 report",
        &report.validator_evidence_packet_schema_hash,
        &report.validator_evidence_field_registry_hash,
        &inputs.model_request,
    )?;
    if report.canonical_tensor_parallelism != 1
        || report.canonical_output_hash != inputs.ruleset_hash
        || !report.receipt_report_verified
        || !report.validator_side_shadow_path_defined
        || report.authority_transfer_live
        || report.shadow_plan.sidecars_live
        || report.shadow_plan.commit_reveal_live
        || report.shadow_plan.authority_transfer_live
        || report.tp_greater_than_one_admitted
        || report.tp_invariant_admission_ready
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 14 report does not preserve TP=1 shadow-only authority semantics",
        ));
    }
    let canonical_tp1 = report
        .tp_checks
        .iter()
        .find(|check| check.tensor_parallelism == 1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Gate 14 missing TP=1 check"))?;
    if !canonical_tp1.admitted
        || !canonical_tp1.matches_canonical_tp1
        || canonical_tp1.evidence_hash.as_deref()
            != Some(receipt_report.receipt.compact_commitment_root.as_str())
        || canonical_tp1.output_hash.as_deref() != Some(inputs.ruleset_hash.as_str())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 14 canonical TP=1 check does not match verifier inputs",
        ));
    }
    if report
        .tp_checks
        .iter()
        .filter(|check| check.tensor_parallelism > 1)
        .any(|check| {
            check.admitted
                || check.evidence_hash.is_some()
                || check.output_hash.is_some()
                || check.matches_canonical_tp1
        })
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 14 TP>1 check was admitted without cross-TP deterministic evidence",
        ));
    }
    Ok(())
}

fn governance_agent_receipt_tamper_probes(
    receipt_report: &GovernanceAgentGate10_5Report,
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<Vec<GovernanceAgentAdversarialProbe>> {
    let mut tampered_action = receipt_report.clone();
    tampered_action.receipt.generated_action_hash = hash_hex(
        "postfiat.governance_agent.gate_15.tampered_action.v1",
        receipt_report.receipt.generated_action_hash.as_bytes(),
    );
    tampered_action.receipt.receipt_id =
        governance_agent_inference_receipt_id(&tampered_action.receipt)?;

    let mut tampered_compact_root = receipt_report.clone();
    tampered_compact_root.receipt.compact_commitment_root = hash_hex(
        "postfiat.governance_agent.gate_15.tampered_compact_root.v1",
        receipt_report.receipt.compact_commitment_root.as_bytes(),
    );
    tampered_compact_root.receipt.receipt_id =
        governance_agent_inference_receipt_id(&tampered_compact_root.receipt)?;

    let mut tampered_attestation_root = receipt_report.clone();
    tampered_attestation_root.receipt.verifier_attestation_root = hash_hex(
        "postfiat.governance_agent.gate_15.tampered_attestation_root.v1",
        receipt_report.receipt.verifier_attestation_root.as_bytes(),
    );
    tampered_attestation_root.receipt.receipt_id =
        governance_agent_inference_receipt_id(&tampered_attestation_root.receipt)?;

    Ok(vec![
        governance_agent_rejected_probe(
            "tampered_generated_action_hash",
            "receipt_tamper",
            validate_governance_agent_gate_10_5_report_for_inputs(&tampered_action, inputs),
        ),
        governance_agent_rejected_probe(
            "tampered_compact_commitment_root",
            "receipt_tamper",
            validate_governance_agent_gate_10_5_report_for_inputs(&tampered_compact_root, inputs),
        ),
        governance_agent_rejected_probe(
            "tampered_verifier_attestation_root",
            "receipt_tamper",
            validate_governance_agent_gate_10_5_report_for_inputs(
                &tampered_attestation_root,
                inputs,
            ),
        ),
    ])
}

fn governance_agent_stale_or_missing_evidence_probes(
    receipt_report: &GovernanceAgentGate10_5Report,
    gate_14_report: &GovernanceAgentGate14Report,
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<Vec<GovernanceAgentAdversarialProbe>> {
    let mut unverified_gate_9_5 = inputs.gate_9_5_report.clone();
    unverified_gate_9_5.verified = false;

    let mut missing_candidate_lineage = inputs.gate_9_5_report.clone();
    missing_candidate_lineage.candidate_hash.clear();

    let mut wrong_model_request_root = receipt_report.clone();
    wrong_model_request_root.receipt.model_request_hash = hash_hex(
        "postfiat.governance_agent.gate_15.wrong_model_request_root.v1",
        inputs.model_request.request_hash.as_bytes(),
    );
    wrong_model_request_root.receipt.receipt_id =
        governance_agent_inference_receipt_id(&wrong_model_request_root.receipt)?;

    let mut drifted_receipt_packet_schema = receipt_report.clone();
    drifted_receipt_packet_schema.validator_evidence_packet_schema_hash = hash_hex(
        "postfiat.governance_agent.gate_15.drifted_gate_10_5_packet_schema.v1",
        receipt_report
            .validator_evidence_packet_schema_hash
            .as_bytes(),
    );

    let mut drifted_receipt_field_registry = receipt_report.clone();
    drifted_receipt_field_registry.validator_evidence_field_registry_hash = hash_hex(
        "postfiat.governance_agent.gate_15.drifted_gate_10_5_field_registry.v1",
        receipt_report
            .validator_evidence_field_registry_hash
            .as_bytes(),
    );

    let mut drifted_gate_14_packet_schema = gate_14_report.clone();
    drifted_gate_14_packet_schema.validator_evidence_packet_schema_hash = hash_hex(
        "postfiat.governance_agent.gate_15.drifted_gate_14_packet_schema.v1",
        gate_14_report
            .validator_evidence_packet_schema_hash
            .as_bytes(),
    );

    let mut drifted_gate_14_field_registry = gate_14_report.clone();
    drifted_gate_14_field_registry.validator_evidence_field_registry_hash = hash_hex(
        "postfiat.governance_agent.gate_15.drifted_gate_14_field_registry.v1",
        gate_14_report
            .validator_evidence_field_registry_hash
            .as_bytes(),
    );

    Ok(vec![
        governance_agent_rejected_probe(
            "stale_unverified_gate_9_5_report",
            "lineage_evidence",
            validate_governance_agent_gate_9_5_report(&unverified_gate_9_5),
        ),
        governance_agent_rejected_probe(
            "missing_gate_9_5_candidate_lineage",
            "lineage_evidence",
            validate_governance_agent_gate_9_5_report(&missing_candidate_lineage),
        ),
        governance_agent_rejected_probe(
            "wrong_model_request_root",
            "lineage_evidence",
            validate_governance_agent_gate_10_5_report_for_inputs(&wrong_model_request_root, inputs),
        ),
        governance_agent_rejected_probe(
            "drifted_gate_10_5_packet_schema_hash",
            "lineage_evidence",
            validate_governance_agent_gate_10_5_report_for_inputs(
                &drifted_receipt_packet_schema,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "drifted_gate_10_5_field_registry_hash",
            "lineage_evidence",
            validate_governance_agent_gate_10_5_report_for_inputs(
                &drifted_receipt_field_registry,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "drifted_gate_14_packet_schema_hash",
            "lineage_evidence",
            validate_governance_agent_gate_14_report_for_inputs(
                &drifted_gate_14_packet_schema,
                receipt_report,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "drifted_gate_14_field_registry_hash",
            "lineage_evidence",
            validate_governance_agent_gate_14_report_for_inputs(
                &drifted_gate_14_field_registry,
                receipt_report,
                inputs,
            ),
        ),
    ])
}

fn governance_agent_verifier_disagreement_probes(
    receipt_report: &GovernanceAgentGate10_5Report,
) -> Vec<GovernanceAgentAdversarialProbe> {
    let disagreement_recorded = receipt_report
        .verifier_attestations
        .iter()
        .any(|attestation| !attestation.accepted && attestation.error.is_some());
    vec![
        governance_agent_rejected_probe(
            "verifier_disagreement_authority_escalation",
            "verifier_disagreement",
            if receipt_report.verifier_disagreement_recorded
                && disagreement_recorded
                && receipt_report.prototype_only
                && !receipt_report.consensus_critical
            {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "verifier disagreement remains shadow evidence and cannot change authority",
                ))
            } else {
                Ok(())
            },
        ),
        governance_agent_rejected_probe(
            "verifier_disagreement_registry_apply",
            "verifier_disagreement",
            if disagreement_recorded {
                Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "recorded disagreement cannot apply a registry mutation",
                ))
            } else {
                Ok(())
            },
        ),
    ]
}

fn governance_agent_authority_transfer_guard_probes(
    gate_14_report: &GovernanceAgentGate14Report,
    receipt_report: &GovernanceAgentGate10_5Report,
    inputs: &GovernanceAgentVerifierTierInputs,
) -> io::Result<Vec<GovernanceAgentAdversarialProbe>> {
    let mut sidecar_attempt = gate_14_report.clone();
    sidecar_attempt.shadow_plan.sidecars_live = true;

    let mut commit_reveal_attempt = gate_14_report.clone();
    commit_reveal_attempt.shadow_plan.commit_reveal_live = true;

    let mut authority_transfer_attempt = gate_14_report.clone();
    authority_transfer_attempt.authority_transfer_live = true;
    authority_transfer_attempt.shadow_plan.authority_transfer_live = true;

    let mut tp2_attempt = gate_14_report.clone();
    if let Some(check) = tp2_attempt
        .tp_checks
        .iter_mut()
        .find(|check| check.tensor_parallelism == 2)
    {
        check.evidence_hash = Some(receipt_report.receipt.compact_commitment_root.clone());
        check.output_hash = Some(inputs.ruleset_hash.clone());
        check.matches_canonical_tp1 = true;
        check.admitted = true;
        check.reason = "adversarial TP=2 promotion attempt".to_string();
    }
    tp2_attempt.tp_greater_than_one_admitted = true;
    tp2_attempt.tp_invariant_admission_ready = true;

    Ok(vec![
        governance_agent_rejected_probe(
            "sidecar_live_authority_attempt",
            "authority_transfer",
            validate_governance_agent_gate_14_report_for_inputs(
                &sidecar_attempt,
                receipt_report,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "commit_reveal_live_authority_attempt",
            "authority_transfer",
            validate_governance_agent_gate_14_report_for_inputs(
                &commit_reveal_attempt,
                receipt_report,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "authority_transfer_live_attempt",
            "authority_transfer",
            validate_governance_agent_gate_14_report_for_inputs(
                &authority_transfer_attempt,
                receipt_report,
                inputs,
            ),
        ),
        governance_agent_rejected_probe(
            "tp2_admission_without_cross_tp_evidence",
            "authority_transfer",
            validate_governance_agent_gate_14_report_for_inputs(&tp2_attempt, receipt_report, inputs),
        ),
    ])
}

fn governance_agent_rejected_probe(
    name: &str,
    category: &str,
    result: io::Result<()>,
) -> GovernanceAgentAdversarialProbe {
    match result {
        Ok(()) => GovernanceAgentAdversarialProbe {
            name: name.to_string(),
            category: category.to_string(),
            rejected: false,
            authority_changed: false,
            error: None,
        },
        Err(error) => GovernanceAgentAdversarialProbe {
            name: name.to_string(),
            category: category.to_string(),
            rejected: true,
            authority_changed: false,
            error: Some(error.to_string()),
        },
    }
}

fn governance_agent_probe_count(probes: &[GovernanceAgentAdversarialProbe], category: &str) -> usize {
    probes
        .iter()
        .filter(|probe| probe.category == category)
        .count()
}

fn governance_agent_all_category_probes_rejected(
    probes: &[GovernanceAgentAdversarialProbe],
    category: &str,
) -> bool {
    probes
        .iter()
        .filter(|probe| probe.category == category)
        .all(|probe| probe.rejected && !probe.authority_changed)
}

fn governance_agent_missing_tp_check(tensor_parallelism: u32) -> GovernanceAgentTensorParallelCheck {
    GovernanceAgentTensorParallelCheck {
        tensor_parallelism,
        evidence_hash: None,
        output_hash: None,
        matches_canonical_tp1: false,
        admitted: false,
        reason:
            "no cross-TP deterministic hash-agreement evidence; TP>1 remains inadmissible".to_string(),
    }
}

fn governance_agent_validator_side_shadow_plan() -> GovernanceAgentValidatorSideShadowPlan {
    GovernanceAgentValidatorSideShadowPlan {
        status: "defined-shadow-only-not-live-authority".to_string(),
        sidecars_live: false,
        commit_reveal_live: false,
        authority_transfer_live: false,
        failure_behavior:
            "sidecar mismatch or missing reveal records disagreement and leaves canonical foundation publication unchanged"
                .to_string(),
        steps: vec![
            GovernanceAgentShadowPathStep {
                order: 1,
                name: "commit_receipt_root".to_string(),
                required_artifact: "receipt_root and verifier_attestation_root".to_string(),
                authority_effect: "shadow evidence only".to_string(),
            },
            GovernanceAgentShadowPathStep {
                order: 2,
                name: "reveal_replay_bundle".to_string(),
                required_artifact: "model request, model response, ruleset, policy, evidence, and candidate roots".to_string(),
                authority_effect: "shadow evidence only".to_string(),
            },
            GovernanceAgentShadowPathStep {
                order: 3,
                name: "compare_validator_side_hashes".to_string(),
                required_artifact: "local recomputed ruleset, candidate, receipt, and attestation roots".to_string(),
                authority_effect: "disagreement metric only".to_string(),
            },
            GovernanceAgentShadowPathStep {
                order: 4,
                name: "report_convergence".to_string(),
                required_artifact: "quorum summary over sidecar verifier outputs".to_string(),
                authority_effect: "no registry authority transfer".to_string(),
            },
        ],
    }
}

fn governance_agent_model_request_prompt_text(value: &serde_json::Value) -> String {
    value
        .get("openai_chat_request")
        .and_then(|request| request.get("messages"))
        .and_then(serde_json::Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .filter_map(|message| message.get("content").and_then(serde_json::Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn governance_agent_token_proxy_values(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_string).collect()
}

fn governance_agent_ratio_bps(numerator: usize, denominator: usize) -> io::Result<u32> {
    if denominator == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ratio denominator must be nonzero",
        ));
    }
    let bps = numerator
        .checked_mul(10_000)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ratio overflow"))?
        .div_ceil(denominator);
    u32::try_from(bps)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "ratio exceeds u32"))
}

fn ensure_governance_agent_policy_sandbox(
    policy: &GovernanceAgentCompiledPolicy,
) -> io::Result<()> {
    if policy.sandbox.network_access
        || policy.sandbox.model_access
        || policy.sandbox.filesystem_access
        || policy.sandbox.direct_state_mutation
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "compiled governance policy grants forbidden sandbox access",
        ));
    }
    Ok(())
}

fn ensure_governance_agent_evidence_covers_ruleset(
    ruleset: &GovernanceRuleset,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
) -> io::Result<()> {
    let available = evidence
        .available_inputs
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for input in &ruleset.inputs {
        validate_governance_agent_input_kind(&input.kind)?;
        if input.required && !available.contains(input.kind.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance agent frozen evidence missing required input `{}`",
                    input.kind
                ),
            ));
        }
    }
    Ok(())
}

fn validate_governance_agent_input_kind(kind: &str) -> io::Result<()> {
    if matches!(
        kind,
        GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND
            | "validator_registry_snapshot"
            | "cobalt_evidence_packet"
            | "operator_manifest_set"
    ) {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported governance agent input kind `{kind}`"),
        ))
    }
}

fn governance_agent_registry_delta_candidate_hash(
    candidate: &GovernanceAgentRegistryDeltaCandidate,
) -> io::Result<String> {
    let mut value = serde_json::to_value(candidate).map_err(invalid_data)?;
    let object = value.as_object_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance agent registry delta candidate must encode as a JSON object",
        )
    })?;
    object.remove("candidate_hash");
    governance_agent_canonical_json_hash("postfiat.governance_agent.registry_delta.v1", &value)
}

fn governance_agent_gate_7_5_malformed_checks(
    ruleset_value: &serde_json::Value,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
) -> io::Result<Vec<GovernanceAgentPolicyRejectionCheck>> {
    let mut invalid_weight = ruleset_value.clone();
    if let Some(decision) = invalid_weight
        .get_mut("decisions")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|decisions| decisions.first_mut())
        .and_then(serde_json::Value::as_object_mut)
    {
        decision.insert("weight".to_string(), serde_json::json!(1.25));
    }
    let mut unknown_evidence_field = ruleset_value.clone();
    if let Some(decision) = unknown_evidence_field
        .get_mut("decisions")
        .and_then(serde_json::Value::as_array_mut)
        .and_then(|decisions| decisions.first_mut())
        .and_then(serde_json::Value::as_object_mut)
    {
        decision.insert(
            "evidence_field_path".to_string(),
            serde_json::json!("validator.identity.reputation_score"),
        );
    }
    let mut missing_evidence = evidence.clone();
    missing_evidence
        .available_inputs
        .retain(|input| input != "cobalt_evidence_packet");
    let mut unsafe_evidence = evidence.clone();
    unsafe_evidence.network_access_allowed = true;
    Ok(vec![
        governance_agent_policy_rejection_check("invalid_weight", &invalid_weight, evidence),
        governance_agent_policy_rejection_check(
            "unknown_evidence_field",
            &unknown_evidence_field,
            evidence,
        ),
        governance_agent_policy_rejection_check(
            "missing_evidence_ref",
            ruleset_value,
            &missing_evidence,
        ),
        governance_agent_policy_rejection_check("unsafe_action", ruleset_value, &unsafe_evidence),
    ])
}

fn governance_agent_policy_rejection_check(
    name: &str,
    ruleset_value: &serde_json::Value,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
) -> GovernanceAgentPolicyRejectionCheck {
    match compile_and_execute_governance_agent_policy_for_check(ruleset_value, evidence) {
        Ok(()) => GovernanceAgentPolicyRejectionCheck {
            name: name.to_string(),
            accepted: true,
            error: None,
        },
        Err(error) => GovernanceAgentPolicyRejectionCheck {
            name: name.to_string(),
            accepted: false,
            error: Some(error.to_string()),
        },
    }
}

fn compile_and_execute_governance_agent_policy_for_check(
    ruleset_value: &serde_json::Value,
    evidence: &GovernanceAgentFrozenEvidenceSnapshot,
) -> io::Result<()> {
    let ruleset = validate_governance_ruleset_value(ruleset_value)?;
    let ruleset_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.ruleset_output.v1",
        ruleset_value,
    )?;
    validate_governance_agent_evidence_snapshot(evidence)?;
    let evidence_value = serde_json::to_value(evidence).map_err(invalid_data)?;
    let evidence_hash = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.frozen_evidence.v1",
        &evidence_value,
    )?;
    let policy = compile_governance_agent_ruleset_policy(&ruleset, &ruleset_hash)?;
    execute_governance_agent_policy(&ruleset, &policy, evidence, &evidence_hash).map(|_| ())
}

fn read_governance_agent_comparison_fixtures(
    comparison_dir: &Path,
) -> io::Result<Vec<(serde_json::Value, GovernanceAgentComparisonFixture)>> {
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(comparison_dir).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!(
                "failed to read governance agent comparison dir `{}`: {error}",
                comparison_dir.display()
            ),
        )
    })? {
        let entry = entry.map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to read governance agent comparison entry in `{}`: {error}",
                    comparison_dir.display()
                ),
            )
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|extension| extension == "json") {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "governance agent comparison dir `{}` has no JSON fixtures",
                comparison_dir.display()
            ),
        ));
    }
    paths
        .iter()
        .map(|path| {
            let value = read_governance_agent_json_value(path, "governance agent comparison fixture")?;
            let fixture: GovernanceAgentComparisonFixture =
                serde_json::from_value(value.clone()).map_err(invalid_data)?;
            validate_governance_agent_comparison_fixture(&fixture)?;
            Ok((value, fixture))
        })
        .collect()
}

fn validate_governance_agent_comparison_fixture(
    fixture: &GovernanceAgentComparisonFixture,
) -> io::Result<()> {
    if fixture.schema != GOVERNANCE_AGENT_COMPARISON_FIXTURE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported governance agent comparison fixture schema",
        ));
    }
    validate_governance_text_id("comparison case_id", &fixture.case_id)?;
    if !matches!(
        fixture.case_class.as_str(),
        "high_confidence" | "ambiguous" | "concentration_risk" | "stale_evidence"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported governance agent comparison case class `{}`",
                fixture.case_class
            ),
        ));
    }
    validate_governance_agent_evidence_snapshot(&fixture.evidence)?;
    validate_governance_agent_direct_baseline(&fixture.direct_baseline)?;
    if fixture.case_class == "high_confidence"
        && (fixture.direct_baseline.action != "no_op"
            || !fixture.direct_baseline.mutations.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Gate 7.6 high-confidence fixture must be a direct no_op baseline until mutation rules are enabled",
        ));
    }
    Ok(())
}

fn validate_governance_agent_direct_baseline(
    baseline: &GovernanceAgentDirectBaseline,
) -> io::Result<()> {
    validate_governance_text_id("direct baseline source", &baseline.source)?;
    validate_governance_text_id("direct baseline rationale", &baseline.rationale)?;
    if baseline.authoritative {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "direct governance agent baseline must be non-authoritative",
        ));
    }
    if baseline.confidence_bps > 10_000 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "direct governance agent baseline confidence_bps must be <= 10000",
        ));
    }
    if !matches!(
        baseline.action.as_str(),
        "no_op" | "admit" | "remove" | "suspend" | "reactivate" | "rotate_key"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported direct governance agent baseline action `{}`",
                baseline.action
            ),
        ));
    }
    if baseline.action == "no_op" && !baseline.mutations.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "direct governance agent no_op baseline cannot include mutations",
        ));
    }
    if baseline.action != "no_op" && baseline.mutations.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "direct governance agent mutation baseline must include at least one mutation",
        ));
    }
    for mutation in &baseline.mutations {
        validate_governance_text_id("direct baseline mutation operation", &mutation.operation)?;
        validate_governance_text_id(
            "direct baseline mutation subject_node_id",
            &mutation.subject_node_id,
        )?;
        if mutation.operation != baseline.action {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "direct governance agent mutation operation must match baseline action",
            ));
        }
    }
    Ok(())
}

fn governance_agent_comparison_case_requires_policy_no_op(
    fixture: &GovernanceAgentComparisonFixture,
) -> bool {
    matches!(
        fixture.case_class.as_str(),
        "ambiguous" | "concentration_risk" | "stale_evidence"
    )
}

fn governance_agent_policy_delta_matches_direct(
    policy_delta: &GovernanceAgentRegistryDeltaCandidate,
    direct_baseline: &GovernanceAgentDirectBaseline,
) -> bool {
    policy_delta.action == direct_baseline.action
        && policy_delta.mutations == direct_baseline.mutations
        && policy_delta.mutation_count == direct_baseline.mutations.len()
}

fn governance_agent_comparison_classes_complete(
    checks: &[GovernanceAgentComparisonCheck],
) -> bool {
    let classes = checks
        .iter()
        .map(|check| check.case_class.as_str())
        .collect::<BTreeSet<_>>();
    ["high_confidence", "ambiguous", "concentration_risk", "stale_evidence"]
        .iter()
        .all(|class| classes.contains(class))
}

fn governance_agent_statement_hash_by_name(
    statement_hashes: &[GovernanceAgentStatementHash],
    name: &str,
) -> io::Result<String> {
    statement_hashes
        .iter()
        .find(|statement| statement.name == name)
        .map(|statement| statement.hash.clone())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("governance agent statement hash `{name}` missing"),
            )
        })
}

fn governance_agent_replay_bundle_root_matches_file(
    replay_bundle_file: &Path,
    expected_root: &str,
) -> io::Result<bool> {
    let value = read_governance_agent_json_value(replay_bundle_file, "governance agent replay bundle")?;
    let root = governance_agent_canonical_json_hash(
        "postfiat.governance_agent.dry_run_replay_bundle.v1",
        &value,
    )?;
    Ok(root == expected_root)
}

fn governance_agent_gate_8_5_rejection_checks(
    governance: &GovernanceState,
    valid: &GovernanceAgentDryRunAmendment,
    latest_dry_run_id: &str,
) -> Vec<GovernanceAgentDryRunRejectionCheck> {
    let mut stale = valid.clone();
    stale.expected_previous_dry_run_id.clear();
    stale.report_root = hash_hex(
        "postfiat.governance_agent.gate_8_5.stale_probe.v1",
        b"stale-ruleset",
    );
    stale.dry_run_id = governance_agent_dry_run_amendment_id(&stale);

    let mut wrong_bundle = valid.clone();
    wrong_bundle.expected_previous_dry_run_id = latest_dry_run_id.to_string();
    wrong_bundle.ruleset_source_bundle_hash = hash_hex(
        "postfiat.governance_agent.gate_8_5.wrong_bundle_probe.v1",
        b"wrong-bundle",
    );
    wrong_bundle.dry_run_id = governance_agent_dry_run_amendment_id(&wrong_bundle);

    let mut missing_replay_root = valid.clone();
    missing_replay_root.expected_previous_dry_run_id = latest_dry_run_id.to_string();
    missing_replay_root.replay_bundle_root.clear();
    missing_replay_root.dry_run_id = governance_agent_dry_run_amendment_id(&missing_replay_root);

    vec![
        governance_agent_gate_8_5_rejection_check(governance, "stale_ruleset", &stale),
        governance_agent_gate_8_5_rejection_check(governance, "wrong_bundle", &wrong_bundle),
        governance_agent_gate_8_5_rejection_check(
            governance,
            "missing_replay_root",
            &missing_replay_root,
        ),
    ]
}

fn governance_agent_gate_8_5_rejection_check(
    governance: &GovernanceState,
    name: &str,
    dry_run: &GovernanceAgentDryRunAmendment,
) -> GovernanceAgentDryRunRejectionCheck {
    match governance_agent_dry_run_rejection(governance, dry_run) {
        Some((_code, message)) => GovernanceAgentDryRunRejectionCheck {
            name: name.to_string(),
            rejected: true,
            error: Some(message),
        },
        None => GovernanceAgentDryRunRejectionCheck {
            name: name.to_string(),
            rejected: false,
            error: None,
        },
    }
}

fn governance_agent_rejection_check_passed(
    checks: &[GovernanceAgentDryRunRejectionCheck],
    name: &str,
) -> bool {
    checks
        .iter()
        .any(|check| check.name == name && check.rejected && check.error.is_some())
}
