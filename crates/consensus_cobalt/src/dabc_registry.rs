pub fn ratify_dabc_amendment(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    input_set: &MvbaValidInputSet,
    previous: Option<&DabcRatifiedAmendment>,
    activation_height: u64,
) -> Result<DabcRatifiedAmendment, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    validate_mvba_valid_input_set(domain, graph, input_set)?;
    let candidate = mvba_output_candidate(input_set)?.clone();
    if candidate.trust_graph_root != graph.trust_graph_root {
        return Err("DABC output candidate trust graph root mismatch".to_string());
    }
    let (sequence, parent_ratification_id) = match previous {
        Some(previous) => {
            validate_dabc_ratified_amendment_core(domain, graph, previous)?;
            (
                previous
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| "DABC ratified amendment sequence overflow".to_string())?,
                previous.ratification_id.clone(),
            )
        }
        None => (1, dabc_genesis_parent_id()),
    };
    let mut ratified = DabcRatifiedAmendment {
        ratification_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        sequence,
        amendment_slot: candidate.amendment_slot,
        parent_ratification_id,
        mvba_agreement_id: input_set.agreement_id.clone(),
        output_candidate_id: input_set.output_candidate_id.clone(),
        candidate,
        activation_height,
    };
    ratified.ratification_id = dabc_ratification_id(domain, &ratified)?;
    validate_dabc_ratified_amendment(domain, graph, &ratified, previous)?;
    Ok(ratified)
}

pub fn validate_dabc_ratified_amendment(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified: &DabcRatifiedAmendment,
    previous: Option<&DabcRatifiedAmendment>,
) -> Result<(), String> {
    validate_dabc_ratified_amendment_core(domain, graph, ratified)?;
    match previous {
        Some(previous) => {
            let expected_sequence = previous
                .sequence
                .checked_add(1)
                .ok_or_else(|| "DABC previous ratified amendment sequence overflow".to_string())?;
            if ratified.sequence != expected_sequence {
                return Err("DABC ratified amendment sequence must extend previous".to_string());
            }
            let expected_slot = previous
                .amendment_slot
                .checked_add(1)
                .ok_or_else(|| "DABC previous amendment slot overflow".to_string())?;
            if ratified.amendment_slot != expected_slot {
                return Err("DABC ratified amendment slot must extend previous".to_string());
            }
            if ratified.parent_ratification_id != previous.ratification_id {
                return Err("DABC ratified amendment parent mismatch".to_string());
            }
        }
        None => {
            if ratified.sequence != 1 {
                return Err("DABC first ratified amendment sequence must be one".to_string());
            }
            if ratified.parent_ratification_id != dabc_genesis_parent_id() {
                return Err("DABC first ratified amendment parent mismatch".to_string());
            }
        }
    }
    Ok(())
}

pub fn dabc_ratification_id(
    domain: &CobaltDomain,
    ratified: &DabcRatifiedAmendment,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        ratified.registry_root.as_str(),
        ratified.trust_graph_root.as_str(),
        ratified.sequence,
        ratified.amendment_slot,
        ratified.parent_ratification_id.as_str(),
        ratified.mvba_agreement_id.as_str(),
        ratified.output_candidate_id.as_str(),
        &ratified.candidate,
        ratified.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.dabc.ratification.v1", &encoded))
}

pub fn build_dabc_full_knowledge_check(
    domain: &CobaltDomain,
    trust_graph_root: TrustGraphRoot,
    sender: impl Into<String>,
    checkpoint_height: u64,
    pending_pairs: Vec<DabcPendingPair>,
    signature_hex: impl Into<String>,
) -> Result<DabcFullKnowledgeCheck, String> {
    validate_domain(domain)?;
    validate_hash_hex("DABC full-knowledge trust graph root", &trust_graph_root)?;
    let mut pending_pairs = pending_pairs;
    pending_pairs.sort_by(dabc_pending_pair_cmp);
    pending_pairs.dedup_by(|left, right| {
        left.amendment_slot == right.amendment_slot
            && left.output_candidate_id == right.output_candidate_id
    });
    let mut check = DabcFullKnowledgeCheck {
        message_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root,
        sender: sender.into(),
        checkpoint_height,
        pending_pairs,
        signature_hex: signature_hex.into(),
    };
    check.message_id = dabc_full_knowledge_check_message_id(&check)?;
    validate_dabc_full_knowledge_check(domain, &check)?;
    Ok(check)
}

pub fn validate_dabc_full_knowledge_check(
    domain: &CobaltDomain,
    check: &DabcFullKnowledgeCheck,
) -> Result<(), String> {
    validate_domain(domain)?;
    if check.chain_id != domain.chain_id
        || check.genesis_hash != domain.genesis_hash
        || check.protocol_version != domain.protocol_version
    {
        return Err("DABC full-knowledge check domain mismatch".to_string());
    }
    validate_hash_hex(
        "DABC full-knowledge trust graph root",
        &check.trust_graph_root,
    )?;
    validate_node_id("DABC full-knowledge sender", &check.sender)?;
    if check.checkpoint_height == 0 {
        return Err("DABC full-knowledge check height must be nonzero".to_string());
    }
    validate_dabc_pending_pairs(&check.pending_pairs)?;
    validate_rbc_signature_hex(&check.signature_hex)?;
    let expected_id = dabc_full_knowledge_check_message_id(check)?;
    if check.message_id != expected_id {
        return Err("DABC full-knowledge check message id mismatch".to_string());
    }
    Ok(())
}

pub fn dabc_full_knowledge_check_signing_payload_bytes(
    check: &DabcFullKnowledgeCheck,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&(
        check.chain_id.as_str(),
        check.genesis_hash.as_str(),
        check.protocol_version,
        check.trust_graph_root.as_str(),
        check.sender.as_str(),
        check.checkpoint_height,
        check.pending_pairs.as_slice(),
    ))
    .map_err(|error| error.to_string())
}

pub fn dabc_full_knowledge_check_message_id(
    check: &DabcFullKnowledgeCheck,
) -> Result<String, String> {
    Ok(hash_hex(
        "postfiat.cobalt.dabc.full_knowledge_check.v1",
        &dabc_full_knowledge_check_signing_payload_bytes(check)?,
    ))
}

pub fn build_dabc_full_knowledge_checkpoint(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    local_validator: impl Into<String>,
    interval_height: u64,
    wait_until_height: u64,
    checks: Vec<DabcFullKnowledgeCheck>,
) -> Result<DabcFullKnowledgeCheckpoint, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    let local_validator = local_validator.into();
    let view = trust_view_for_validator(graph, &local_validator)?;
    let covered_heights = dabc_required_checkpoint_heights(interval_height, wait_until_height)?;
    let mut checks = checks;
    checks.sort_by(dabc_full_knowledge_check_cmp);
    reject_duplicate_full_knowledge_check_keys(&checks)?;
    let mut checkpoint = DabcFullKnowledgeCheckpoint {
        checkpoint_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        trust_view_id: view.trust_view_id.clone(),
        local_validator,
        interval_height,
        wait_until_height,
        covered_heights,
        checks,
    };
    checkpoint.checkpoint_id = dabc_full_knowledge_checkpoint_id(domain, &checkpoint)?;
    validate_dabc_full_knowledge_checkpoint(domain, graph, &checkpoint)?;
    Ok(checkpoint)
}

pub fn validate_dabc_full_knowledge_checkpoint(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    checkpoint: &DabcFullKnowledgeCheckpoint,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    if checkpoint.chain_id != domain.chain_id
        || checkpoint.genesis_hash != domain.genesis_hash
        || checkpoint.protocol_version != domain.protocol_version
    {
        return Err("DABC full-knowledge checkpoint domain mismatch".to_string());
    }
    if checkpoint.registry_root != graph.registry_root {
        return Err("DABC full-knowledge checkpoint registry root mismatch".to_string());
    }
    if checkpoint.trust_graph_root != graph.trust_graph_root {
        return Err("DABC full-knowledge checkpoint trust graph root mismatch".to_string());
    }
    let view = trust_view_for_validator(graph, &checkpoint.local_validator)?;
    if checkpoint.trust_view_id != view.trust_view_id {
        return Err("DABC full-knowledge checkpoint trust view id mismatch".to_string());
    }
    let expected_heights =
        dabc_required_checkpoint_heights(checkpoint.interval_height, checkpoint.wait_until_height)?;
    if checkpoint.covered_heights != expected_heights {
        return Err("DABC full-knowledge checkpoint covered heights mismatch".to_string());
    }
    if checkpoint.checks.len() > MAX_DABC_FULL_KNOWLEDGE_CHECKS {
        return Err("DABC full-knowledge checkpoint has too many checks".to_string());
    }
    if !dabc_full_knowledge_checks_sorted_unique(&checkpoint.checks) {
        return Err("DABC full-knowledge checkpoint checks must be sorted unique".to_string());
    }

    let covered: BTreeSet<u64> = checkpoint.covered_heights.iter().copied().collect();
    let allowed: BTreeSet<&str> = view.derived_unl.iter().map(String::as_str).collect();
    let mut support_by_height: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    for check in &checkpoint.checks {
        validate_dabc_full_knowledge_check(domain, check)?;
        if check.trust_graph_root != checkpoint.trust_graph_root {
            return Err(
                "DABC full-knowledge checkpoint check trust graph root mismatch".to_string(),
            );
        }
        if !covered.contains(&check.checkpoint_height) {
            return Err("DABC full-knowledge checkpoint check height is not covered".to_string());
        }
        if !allowed.contains(check.sender.as_str()) {
            return Err(
                "DABC full-knowledge checkpoint check sender outside local view".to_string(),
            );
        }
        support_by_height
            .entry(check.checkpoint_height)
            .or_default()
            .push(check.sender.clone());
    }

    for height in &checkpoint.covered_heights {
        let mut support = support_by_height.get(height).cloned().unwrap_or_default();
        support = sorted_unique(&support);
        validate_support_in_view(view, &support)?;
        if !has_strong_support(view, &support)? {
            return Err(format!(
                "DABC full-knowledge checkpoint lacks strong support at height {height}"
            ));
        }
    }

    let expected_id = dabc_full_knowledge_checkpoint_id(domain, checkpoint)?;
    if checkpoint.checkpoint_id != expected_id {
        return Err("DABC full-knowledge checkpoint id mismatch".to_string());
    }
    Ok(())
}

pub fn dabc_full_knowledge_checkpoint_id(
    domain: &CobaltDomain,
    checkpoint: &DabcFullKnowledgeCheckpoint,
) -> Result<String, String> {
    validate_domain(domain)?;
    let check_ids: Vec<&str> = checkpoint
        .checks
        .iter()
        .map(|check| check.message_id.as_str())
        .collect();
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        checkpoint.registry_root.as_str(),
        checkpoint.trust_graph_root.as_str(),
        checkpoint.trust_view_id.as_str(),
        checkpoint.local_validator.as_str(),
        checkpoint.interval_height,
        checkpoint.wait_until_height,
        checkpoint.covered_heights.as_slice(),
        check_ids.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.dabc.full_knowledge_checkpoint.v1",
        &encoded,
    ))
}

pub fn validate_dabc_activation_with_full_knowledge(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified_chain: &[DabcRatifiedAmendment],
    ratified: &DabcRatifiedAmendment,
    checkpoint: &DabcFullKnowledgeCheckpoint,
) -> Result<DabcActivationEvidence, String> {
    validate_dabc_ratified_chain(domain, graph, ratified_chain)?;
    validate_dabc_full_knowledge_checkpoint(domain, graph, checkpoint)?;
    if ratified.activation_height == 0 {
        return Err("DABC activation height must be nonzero".to_string());
    }
    if checkpoint.wait_until_height < ratified.activation_height {
        return Err("DABC full-knowledge checkpoint is before activation height".to_string());
    }
    let chain_member = ratified_chain
        .iter()
        .find(|candidate| candidate.ratification_id == ratified.ratification_id)
        .ok_or_else(|| "DABC activation target is not in ratified chain".to_string())?;
    if chain_member != ratified {
        return Err("DABC activation target does not match ratified chain entry".to_string());
    }

    let ratified_slots: BTreeSet<u64> = ratified_chain
        .iter()
        .map(|entry| entry.amendment_slot)
        .collect();
    for check in &checkpoint.checks {
        for pair in &check.pending_pairs {
            if !ratified_slots.contains(&pair.amendment_slot) {
                return Err(format!(
                    "DABC full-knowledge pending slot {} is not ratified",
                    pair.amendment_slot
                ));
            }
        }
    }

    let mut evidence = DabcActivationEvidence {
        activation_id: String::new(),
        ratification_id: ratified.ratification_id.clone(),
        checkpoint_id: checkpoint.checkpoint_id.clone(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        trust_view_id: checkpoint.trust_view_id.clone(),
        local_validator: checkpoint.local_validator.clone(),
        ratified_sequence: ratified.sequence,
        amendment_slot: ratified.amendment_slot,
        activation_height: ratified.activation_height,
        wait_until_height: checkpoint.wait_until_height,
    };
    evidence.activation_id = dabc_activation_evidence_id(domain, &evidence)?;
    Ok(evidence)
}

pub fn dabc_activation_evidence_id(
    domain: &CobaltDomain,
    evidence: &DabcActivationEvidence,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        evidence.registry_root.as_str(),
        evidence.trust_graph_root.as_str(),
        evidence.trust_view_id.as_str(),
        evidence.local_validator.as_str(),
        evidence.ratification_id.as_str(),
        evidence.checkpoint_id.as_str(),
        evidence.ratified_sequence,
        evidence.amendment_slot,
        evidence.activation_height,
        evidence.wait_until_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.dabc.activation.v1", &encoded))
}

pub fn build_dabc_replay_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified_amendments: Vec<DabcRatifiedAmendment>,
    full_knowledge_checkpoints: Vec<DabcFullKnowledgeCheckpoint>,
    activation_evidence: Vec<DabcActivationEvidence>,
) -> Result<DabcReplayBundle, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    let mut full_knowledge_checkpoints = full_knowledge_checkpoints;
    full_knowledge_checkpoints.sort_by(|left, right| left.checkpoint_id.cmp(&right.checkpoint_id));
    let mut activation_evidence = activation_evidence;
    activation_evidence.sort_by(|left, right| left.activation_id.cmp(&right.activation_id));
    let mut bundle = DabcReplayBundle {
        bundle_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        ratified_amendments,
        full_knowledge_checkpoints,
        activation_evidence,
    };
    bundle.bundle_id = dabc_replay_bundle_id(domain, &bundle)?;
    verify_dabc_replay_bundle(domain, graph, &bundle)?;
    Ok(bundle)
}

pub fn verify_dabc_replay_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    bundle: &DabcReplayBundle,
) -> Result<DabcReplayReport, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    if bundle.chain_id != domain.chain_id
        || bundle.genesis_hash != domain.genesis_hash
        || bundle.protocol_version != domain.protocol_version
    {
        return Err("DABC replay bundle domain mismatch".to_string());
    }
    if bundle.registry_root != graph.registry_root {
        return Err("DABC replay bundle registry root mismatch".to_string());
    }
    if bundle.trust_graph_root != graph.trust_graph_root {
        return Err("DABC replay bundle trust graph root mismatch".to_string());
    }
    validate_dabc_ratified_chain(domain, graph, &bundle.ratified_amendments)?;

    let mut previous_activation_height = None;
    for ratified in &bundle.ratified_amendments {
        if let Some(previous) = previous_activation_height {
            if ratified.activation_height < previous {
                return Err(
                    "DABC replay bundle activation heights must be nondecreasing".to_string(),
                );
            }
        }
        previous_activation_height = Some(ratified.activation_height);
    }

    let mut checkpoints_by_id = BTreeMap::new();
    let mut checkpoint_ids = Vec::with_capacity(bundle.full_knowledge_checkpoints.len());
    for checkpoint in &bundle.full_knowledge_checkpoints {
        validate_dabc_full_knowledge_checkpoint(domain, graph, checkpoint)?;
        if checkpoints_by_id
            .insert(checkpoint.checkpoint_id.clone(), checkpoint)
            .is_some()
        {
            return Err("DABC replay bundle duplicate checkpoint id".to_string());
        }
        checkpoint_ids.push(checkpoint.checkpoint_id.clone());
    }
    if sorted_unique(&checkpoint_ids) != checkpoint_ids {
        return Err("DABC replay bundle checkpoints must be sorted unique".to_string());
    }

    let mut activation_by_ratification = BTreeMap::new();
    let mut activation_ids = Vec::with_capacity(bundle.activation_evidence.len());
    for evidence in &bundle.activation_evidence {
        let expected_id = dabc_activation_evidence_id(domain, evidence)?;
        if evidence.activation_id != expected_id {
            return Err("DABC replay bundle activation evidence id mismatch".to_string());
        }
        if evidence.chain_id != domain.chain_id
            || evidence.genesis_hash != domain.genesis_hash
            || evidence.protocol_version != domain.protocol_version
            || evidence.registry_root != graph.registry_root
            || evidence.trust_graph_root != graph.trust_graph_root
        {
            return Err("DABC replay bundle activation evidence domain mismatch".to_string());
        }
        let checkpoint = checkpoints_by_id
            .get(&evidence.checkpoint_id)
            .ok_or_else(|| {
                "DABC replay bundle activation evidence references unknown checkpoint".to_string()
            })?;
        let ratified = bundle
            .ratified_amendments
            .iter()
            .find(|ratified| ratified.ratification_id == evidence.ratification_id)
            .ok_or_else(|| {
                "DABC replay bundle activation evidence references unknown ratification".to_string()
            })?;
        let expected = validate_dabc_activation_with_full_knowledge(
            domain,
            graph,
            &bundle.ratified_amendments,
            ratified,
            checkpoint,
        )?;
        if evidence != &expected {
            return Err("DABC replay bundle activation evidence mismatch".to_string());
        }
        if activation_by_ratification
            .insert(evidence.ratification_id.clone(), evidence)
            .is_some()
        {
            return Err(
                "DABC replay bundle duplicate activation evidence for ratification".to_string(),
            );
        }
        activation_ids.push(evidence.activation_id.clone());
    }
    if sorted_unique(&activation_ids) != activation_ids {
        return Err("DABC replay bundle activation evidence must be sorted unique".to_string());
    }

    for ratified in &bundle.ratified_amendments {
        if !activation_by_ratification.contains_key(&ratified.ratification_id) {
            return Err("DABC replay bundle missing activation evidence".to_string());
        }
    }

    let expected_bundle_id = dabc_replay_bundle_id(domain, bundle)?;
    if bundle.bundle_id != expected_bundle_id {
        return Err("DABC replay bundle id mismatch".to_string());
    }

    let ratification_ids = bundle
        .ratified_amendments
        .iter()
        .map(|ratified| ratified.ratification_id.clone())
        .collect::<Vec<_>>();
    let highest_sequence = bundle
        .ratified_amendments
        .last()
        .map_or(0, |ratified| ratified.sequence);
    let highest_activation_height = bundle
        .ratified_amendments
        .iter()
        .map(|ratified| ratified.activation_height)
        .max()
        .unwrap_or(0);
    Ok(DabcReplayReport {
        bundle_id: bundle.bundle_id.clone(),
        ratified_count: bundle.ratified_amendments.len(),
        activation_count: bundle.activation_evidence.len(),
        checkpoint_count: bundle.full_knowledge_checkpoints.len(),
        highest_sequence,
        highest_activation_height,
        ratification_ids,
        checkpoint_ids,
        activation_ids,
    })
}

pub fn dabc_replay_bundle_id(
    domain: &CobaltDomain,
    bundle: &DabcReplayBundle,
) -> Result<String, String> {
    validate_domain(domain)?;
    let ratification_ids = bundle
        .ratified_amendments
        .iter()
        .map(|ratified| ratified.ratification_id.as_str())
        .collect::<Vec<_>>();
    let checkpoint_ids = bundle
        .full_knowledge_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.checkpoint_id.as_str())
        .collect::<Vec<_>>();
    let activation_ids = bundle
        .activation_evidence
        .iter()
        .map(|evidence| evidence.activation_id.as_str())
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        bundle.registry_root.as_str(),
        bundle.trust_graph_root.as_str(),
        ratification_ids.as_slice(),
        checkpoint_ids.as_slice(),
        activation_ids.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.dabc.replay_bundle.v1", &encoded))
}

pub fn validator_registry_lifecycle_payload_hash(
    domain: &CobaltDomain,
    update: &ValidatorRegistryUpdateRecord,
) -> Result<String, String> {
    verify_validator_registry_update(domain, update)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        update.schema.as_str(),
        update.update_id.as_str(),
        update.operation.as_str(),
        update.subject_node_id.as_str(),
        update.previous_registry_root.as_str(),
        update.new_registry_root.as_str(),
        update.previous_trust_graph_root.as_deref(),
        update.new_trust_graph_root.as_deref(),
        update.trust_graph_transition_id.as_deref(),
        update.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.validator_lifecycle.payload.v1",
        &encoded,
    ))
}

pub fn bind_dabc_ratification_to_validator_registry_update(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified: &DabcRatifiedAmendment,
    previous: Option<&DabcRatifiedAmendment>,
    update: &ValidatorRegistryUpdateRecord,
) -> Result<DabcValidatorLifecycleRatification, String> {
    validate_dabc_ratified_amendment(domain, graph, ratified, previous)?;
    verify_validator_registry_update(domain, update)?;
    if ratified.registry_root != update.previous_registry_root {
        return Err("DABC validator lifecycle registry root mismatch".to_string());
    }
    if ratified.activation_height != update.activation_height {
        return Err("DABC validator lifecycle activation height mismatch".to_string());
    }
    let payload_hash = validator_registry_lifecycle_payload_hash(domain, update)?;
    if ratified.candidate.payload_hash != payload_hash {
        return Err("DABC validator lifecycle payload hash mismatch".to_string());
    }
    let mut lifecycle = DabcValidatorLifecycleRatification {
        lifecycle_ratification_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        dabc_ratification_id: ratified.ratification_id.clone(),
        registry_update_id: update.update_id.clone(),
        operation: update.operation.clone(),
        subject_node_id: update.subject_node_id.clone(),
        previous_registry_root: update.previous_registry_root.clone(),
        new_registry_root: update.new_registry_root.clone(),
        payload_hash,
        activation_height: update.activation_height,
    };
    lifecycle.lifecycle_ratification_id =
        dabc_validator_lifecycle_ratification_id(domain, &lifecycle)?;
    Ok(lifecycle)
}

pub fn dabc_validator_lifecycle_ratification_id(
    domain: &CobaltDomain,
    lifecycle: &DabcValidatorLifecycleRatification,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        lifecycle.dabc_ratification_id.as_str(),
        lifecycle.registry_update_id.as_str(),
        lifecycle.operation.as_str(),
        lifecycle.subject_node_id.as_str(),
        lifecycle.previous_registry_root.as_str(),
        lifecycle.new_registry_root.as_str(),
        lifecycle.payload_hash.as_str(),
        lifecycle.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.dabc.validator_lifecycle_ratification.v1",
        &encoded,
    ))
}

pub fn trust_graph_lifecycle_payload_hash(
    domain: &CobaltDomain,
    record: &TrustGraphLifecycleRecord,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&serde_json::json!([
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        record.record_id.as_str(),
        record.operation.as_str(),
        record.subject_validator.as_str(),
        record.previous_registry_root.as_str(),
        record.new_registry_root.as_str(),
        record.previous_trust_graph_root.as_str(),
        record.new_trust_graph_root.as_str(),
        record.trust_graph_transition_id.as_str(),
        record.activation_height,
        record.previous_trust_view_id.as_str(),
        record.new_trust_view_id.as_str(),
        record.previous_subset_ids.as_slice(),
        record.new_subset_ids.as_slice(),
        record.linkage_report_hash.as_str(),
    ]))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.trust_graph_lifecycle.payload.v1",
        &encoded,
    ))
}

pub fn bind_dabc_ratification_to_trust_graph_lifecycle_record(
    domain: &CobaltDomain,
    previous_graph: &TrustGraph,
    new_graph: &TrustGraph,
    linkage_report: &LinkageReport,
    ratified: &DabcRatifiedAmendment,
    previous_ratified: Option<&DabcRatifiedAmendment>,
    record: &TrustGraphLifecycleRecord,
) -> Result<DabcTrustGraphLifecycleRatification, String> {
    validate_trust_graph_lifecycle_record(
        domain,
        previous_graph,
        new_graph,
        linkage_report,
        record,
    )?;
    validate_dabc_ratified_amendment(domain, previous_graph, ratified, previous_ratified)?;
    if ratified.activation_height != record.activation_height {
        return Err("DABC trust graph lifecycle activation height mismatch".to_string());
    }
    let payload_hash = trust_graph_lifecycle_payload_hash(domain, record)?;
    if ratified.candidate.payload_hash != payload_hash {
        return Err("DABC trust graph lifecycle payload hash mismatch".to_string());
    }
    let mut lifecycle = DabcTrustGraphLifecycleRatification {
        lifecycle_ratification_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        dabc_ratification_id: ratified.ratification_id.clone(),
        trust_graph_lifecycle_record_id: record.record_id.clone(),
        operation: record.operation.clone(),
        subject_validator: record.subject_validator.clone(),
        previous_trust_graph_root: record.previous_trust_graph_root.clone(),
        new_trust_graph_root: record.new_trust_graph_root.clone(),
        payload_hash,
        activation_height: record.activation_height,
    };
    lifecycle.lifecycle_ratification_id =
        dabc_trust_graph_lifecycle_ratification_id(domain, &lifecycle)?;
    Ok(lifecycle)
}

pub fn dabc_trust_graph_lifecycle_ratification_id(
    domain: &CobaltDomain,
    lifecycle: &DabcTrustGraphLifecycleRatification,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        lifecycle.dabc_ratification_id.as_str(),
        lifecycle.trust_graph_lifecycle_record_id.as_str(),
        lifecycle.operation.as_str(),
        lifecycle.subject_validator.as_str(),
        lifecycle.previous_trust_graph_root.as_str(),
        lifecycle.new_trust_graph_root.as_str(),
        lifecycle.payload_hash.as_str(),
        lifecycle.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.dabc.trust_graph_lifecycle_ratification.v1",
        &encoded,
    ))
}

pub fn trust_graph_rollback_payload_hash(
    domain: &CobaltDomain,
    record: &TrustGraphRollbackRecord,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&serde_json::json!([
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        record.record_id.as_str(),
        record.authority_trust_graph_root.as_str(),
        record.failed_trust_graph_root.as_str(),
        record.rollback_trust_graph_root.as_str(),
        record.registry_root.as_str(),
        record.failed_activation_height,
        record.rollback_activation_height,
        record.bad_linkage_report_hash.as_str(),
        record.rollback_linkage_report_hash.as_str(),
        record.trust_graph_transition_id.as_str(),
        record.reason.as_str(),
    ]))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.trust_graph_rollback.payload.v1",
        &encoded,
    ))
}

pub fn bind_dabc_ratification_to_trust_graph_rollback_record(
    input: TrustGraphRollbackRatificationInput<'_>,
) -> Result<DabcTrustGraphRollbackRatification, String> {
    validate_trust_graph_rollback_record(
        input.domain,
        input.authority_graph,
        input.failed_graph,
        input.rollback_graph,
        input.bad_linkage_report,
        input.rollback_linkage_report,
        input.record,
    )?;
    validate_dabc_ratified_amendment(
        input.domain,
        input.authority_graph,
        input.ratified,
        input.previous_ratified,
    )?;
    if input.ratified.activation_height != input.record.rollback_activation_height {
        return Err("DABC trust graph rollback activation height mismatch".to_string());
    }
    let payload_hash = trust_graph_rollback_payload_hash(input.domain, input.record)?;
    if input.ratified.candidate.payload_hash != payload_hash {
        return Err("DABC trust graph rollback payload hash mismatch".to_string());
    }
    let mut rollback = DabcTrustGraphRollbackRatification {
        rollback_ratification_id: String::new(),
        chain_id: input.domain.chain_id.clone(),
        genesis_hash: input.domain.genesis_hash.clone(),
        protocol_version: input.domain.protocol_version,
        dabc_ratification_id: input.ratified.ratification_id.clone(),
        rollback_record_id: input.record.record_id.clone(),
        authority_trust_graph_root: input.record.authority_trust_graph_root.clone(),
        failed_trust_graph_root: input.record.failed_trust_graph_root.clone(),
        rollback_trust_graph_root: input.record.rollback_trust_graph_root.clone(),
        payload_hash,
        activation_height: input.record.rollback_activation_height,
    };
    rollback.rollback_ratification_id =
        dabc_trust_graph_rollback_ratification_id(input.domain, &rollback)?;
    Ok(rollback)
}

pub fn dabc_trust_graph_rollback_ratification_id(
    domain: &CobaltDomain,
    rollback: &DabcTrustGraphRollbackRatification,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        rollback.dabc_ratification_id.as_str(),
        rollback.rollback_record_id.as_str(),
        rollback.authority_trust_graph_root.as_str(),
        rollback.failed_trust_graph_root.as_str(),
        rollback.rollback_trust_graph_root.as_str(),
        rollback.payload_hash.as_str(),
        rollback.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.dabc.trust_graph_rollback_ratification.v1",
        &encoded,
    ))
}

pub fn build_transaction_network_membership(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    governance_epoch: u64,
    validators: Vec<String>,
    quorum: usize,
    activation_height: u64,
) -> Result<TransactionNetworkMembership, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    let validators = sorted_unique(&validators);
    let mut membership = TransactionNetworkMembership {
        transaction_network_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        governance_epoch,
        validators,
        quorum,
        activation_height,
    };
    membership.transaction_network_id = transaction_network_membership_id(domain, &membership)?;
    validate_transaction_network_membership(domain, graph, &membership)?;
    Ok(membership)
}

pub fn validate_transaction_network_membership(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    membership: &TransactionNetworkMembership,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    if membership.chain_id != domain.chain_id
        || membership.genesis_hash != domain.genesis_hash
        || membership.protocol_version != domain.protocol_version
    {
        return Err("transaction network membership domain mismatch".to_string());
    }
    if membership.registry_root != graph.registry_root {
        return Err("transaction network membership registry root mismatch".to_string());
    }
    if membership.trust_graph_root != graph.trust_graph_root {
        return Err("transaction network membership trust graph root mismatch".to_string());
    }
    if membership.governance_epoch == 0 {
        return Err("transaction network membership governance epoch must be nonzero".to_string());
    }
    if membership.activation_height == 0 {
        return Err("transaction network membership activation height must be nonzero".to_string());
    }
    validate_validator_scope("transaction network", &membership.validators)?;
    if membership.quorum == 0 || membership.quorum > membership.validators.len() {
        return Err("transaction network membership quorum is invalid".to_string());
    }
    let graph_validators: BTreeSet<&str> = graph
        .trust_views
        .iter()
        .map(|view| view.validator.as_str())
        .collect();
    if membership
        .validators
        .iter()
        .any(|validator| !graph_validators.contains(validator.as_str()))
    {
        return Err(
            "transaction network membership includes validator outside trust graph".to_string(),
        );
    }
    let expected_id = transaction_network_membership_id(domain, membership)?;
    if membership.transaction_network_id != expected_id {
        return Err("transaction network membership id mismatch".to_string());
    }
    Ok(())
}

pub fn transaction_network_membership_id(
    domain: &CobaltDomain,
    membership: &TransactionNetworkMembership,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        membership.registry_root.as_str(),
        membership.trust_graph_root.as_str(),
        membership.governance_epoch,
        membership.validators.as_slice(),
        membership.quorum,
        membership.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.transaction_network.membership.v1",
        &encoded,
    ))
}

pub fn transaction_network_membership_payload_hash(
    domain: &CobaltDomain,
    membership: &TransactionNetworkMembership,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        membership.transaction_network_id.as_str(),
        membership.registry_root.as_str(),
        membership.trust_graph_root.as_str(),
        membership.governance_epoch,
        membership.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.transaction_network.payload.v1",
        &encoded,
    ))
}

pub fn bind_dabc_ratification_to_transaction_network_membership(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified: &DabcRatifiedAmendment,
    previous_ratified: Option<&DabcRatifiedAmendment>,
    membership: &TransactionNetworkMembership,
) -> Result<DabcTransactionNetworkRatification, String> {
    validate_transaction_network_membership(domain, graph, membership)?;
    validate_dabc_ratified_amendment(domain, graph, ratified, previous_ratified)?;
    if ratified.activation_height != membership.activation_height {
        return Err("DABC transaction network activation height mismatch".to_string());
    }
    let payload_hash = transaction_network_membership_payload_hash(domain, membership)?;
    if ratified.candidate.payload_hash != payload_hash {
        return Err("DABC transaction network payload hash mismatch".to_string());
    }
    let mut binding = DabcTransactionNetworkRatification {
        transaction_network_ratification_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        dabc_ratification_id: ratified.ratification_id.clone(),
        transaction_network_id: membership.transaction_network_id.clone(),
        registry_root: membership.registry_root.clone(),
        trust_graph_root: membership.trust_graph_root.clone(),
        payload_hash,
        governance_epoch: membership.governance_epoch,
        activation_height: membership.activation_height,
    };
    binding.transaction_network_ratification_id =
        dabc_transaction_network_ratification_id(domain, &binding)?;
    Ok(binding)
}

pub fn dabc_transaction_network_ratification_id(
    domain: &CobaltDomain,
    binding: &DabcTransactionNetworkRatification,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        binding.dabc_ratification_id.as_str(),
        binding.transaction_network_id.as_str(),
        binding.registry_root.as_str(),
        binding.trust_graph_root.as_str(),
        binding.payload_hash.as_str(),
        binding.governance_epoch,
        binding.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.dabc.transaction_network_ratification.v1",
        &encoded,
    ))
}

pub fn build_cobalt_block_membership_binding(
    domain: &CobaltDomain,
    membership: &TransactionNetworkMembership,
    block_hash: impl Into<String>,
    block_height: u64,
    proposer: impl Into<String>,
) -> Result<CobaltBlockMembershipBinding, String> {
    validate_domain(domain)?;
    let mut binding = CobaltBlockMembershipBinding {
        binding_id: String::new(),
        block_hash: block_hash.into(),
        block_height,
        proposer: proposer.into(),
        registry_root: membership.registry_root.clone(),
        trust_graph_root: membership.trust_graph_root.clone(),
        governance_epoch: membership.governance_epoch,
        transaction_network_id: membership.transaction_network_id.clone(),
    };
    binding.binding_id = cobalt_block_membership_binding_id(domain, &binding)?;
    validate_cobalt_block_membership_binding(domain, membership, &binding)?;
    Ok(binding)
}

pub fn validate_cobalt_block_membership_binding(
    domain: &CobaltDomain,
    membership: &TransactionNetworkMembership,
    binding: &CobaltBlockMembershipBinding,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_hash_hex("Cobalt block binding block hash", &binding.block_hash)?;
    if binding.block_height < membership.activation_height {
        return Err("Cobalt block binding is before transaction network activation".to_string());
    }
    validate_node_id("Cobalt block binding proposer", &binding.proposer)?;
    if !membership
        .validators
        .iter()
        .any(|validator| validator == &binding.proposer)
    {
        return Err("Cobalt block binding proposer is outside transaction network".to_string());
    }
    if binding.registry_root != membership.registry_root
        || binding.trust_graph_root != membership.trust_graph_root
        || binding.governance_epoch != membership.governance_epoch
        || binding.transaction_network_id != membership.transaction_network_id
    {
        return Err("Cobalt block binding transaction network metadata mismatch".to_string());
    }
    let expected_id = cobalt_block_membership_binding_id(domain, binding)?;
    if binding.binding_id != expected_id {
        return Err("Cobalt block binding id mismatch".to_string());
    }
    Ok(())
}

pub fn cobalt_block_membership_binding_id(
    domain: &CobaltDomain,
    binding: &CobaltBlockMembershipBinding,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        binding.block_hash.as_str(),
        binding.block_height,
        binding.proposer.as_str(),
        binding.registry_root.as_str(),
        binding.trust_graph_root.as_str(),
        binding.governance_epoch,
        binding.transaction_network_id.as_str(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.block_membership_binding.v1",
        &encoded,
    ))
}

pub fn validate_transaction_network_transition(
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<(), String> {
    if previous.chain_id != next.chain_id
        || previous.genesis_hash != next.genesis_hash
        || previous.protocol_version != next.protocol_version
    {
        return Err("transaction network transition domain mismatch".to_string());
    }
    if next.governance_epoch <= previous.governance_epoch {
        return Err("transaction network transition governance epoch must increase".to_string());
    }
    if next.activation_height <= previous.activation_height {
        return Err("transaction network transition activation height must increase".to_string());
    }
    if next.transaction_network_id == previous.transaction_network_id {
        return Err("transaction network transition id must change".to_string());
    }
    Ok(())
}

pub fn validate_cobalt_block_against_transaction_network_transition(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
    binding: &CobaltBlockMembershipBinding,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_transaction_network_transition(previous, next)?;
    let active = if binding.block_height >= next.activation_height {
        next
    } else {
        previous
    };
    validate_cobalt_block_membership_binding(domain, active, binding)
}

pub fn certify_validator_registry_update(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    request: ValidatorRegistryUpdateRequest,
    support: Vec<String>,
) -> Result<ValidatorRegistryUpdateRecord, String> {
    validate_domain(domain)?;
    validate_config(config)?;
    validate_validator_registry_update_request(domain, &request)?;
    let allowed: BTreeSet<&str> = config.validators.iter().map(String::as_str).collect();
    let unique_support: BTreeSet<String> = support
        .into_iter()
        .filter(|validator| allowed.contains(validator.as_str()))
        .collect();
    if unique_support.len() < config.quorum {
        return Err(format!(
            "insufficient registry update support: got {}, need {}",
            unique_support.len(),
            config.quorum
        ));
    }

    let support: Vec<String> = unique_support.into_iter().collect();
    let proposer = config
        .validators
        .first()
        .cloned()
        .ok_or_else(|| "validator set must be nonempty".to_string())?;
    let instance_id = validator_registry_update_instance_id(domain, config, &request)?;
    let proposal_id = validator_registry_update_proposal_id(domain, &instance_id, &proposer);
    let votes: Vec<GovernanceVote> = support
        .iter()
        .map(|validator| GovernanceVote {
            vote_id: validator_registry_update_vote_id(
                domain,
                &instance_id,
                &proposal_id,
                validator,
                true,
            ),
            validator: validator.clone(),
            accept: true,
        })
        .collect();
    let certificate_id = validator_registry_update_certificate_id(
        domain,
        &instance_id,
        &proposal_id,
        config.quorum,
        &votes,
    )?;
    let update_id =
        validator_registry_update_id(domain, &instance_id, &certificate_id, &request, &support)?;

    Ok(ValidatorRegistryUpdateRecord {
        schema: VALIDATOR_REGISTRY_UPDATE_SCHEMA.to_string(),
        update_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        instance_id,
        proposal_id,
        certificate_id,
        proposer,
        validators: config.validators.clone(),
        quorum: config.quorum,
        support,
        votes,
        signed_authorizations: Vec::new(),
        activation_height: request.activation_height,
        previous_registry_root: request.previous_registry_root,
        new_registry_root: request.new_registry_root,
        previous_trust_graph_root: request.previous_trust_graph_root,
        new_trust_graph_root: request.new_trust_graph_root,
        trust_graph_transition_id: request.trust_graph_transition_id,
        previous_validators: request.previous_validators,
        new_validators: request.new_validators,
        operation: request.operation,
        subject_node_id: request.subject_node_id,
        previous_record: request.previous_record,
        new_record: request.new_record,
    })
}

pub fn certify_validator_registry_update_with_trust_graph_transition(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    mut request: ValidatorRegistryUpdateRequest,
    transition: TrustGraphTransition,
    support: Vec<String>,
) -> Result<ValidatorRegistryUpdateRecord, String> {
    validate_trust_graph_transition(domain, &transition)?;
    if request.previous_registry_root != transition.previous_registry_root
        || request.new_registry_root != transition.new_registry_root
        || request.activation_height != transition.activation_height
    {
        return Err("validator registry update does not match trust graph transition".to_string());
    }
    request.previous_trust_graph_root = Some(transition.previous_trust_graph_root);
    request.new_trust_graph_root = Some(transition.new_trust_graph_root);
    request.trust_graph_transition_id = Some(transition.transition_id);
    certify_validator_registry_update(domain, config, request, support)
}

pub fn verify_validator_registry_update(
    domain: &CobaltDomain,
    update: &ValidatorRegistryUpdateRecord,
) -> Result<(), String> {
    validate_domain(domain)?;
    if update.schema != VALIDATOR_REGISTRY_UPDATE_SCHEMA {
        return Err("validator registry update schema mismatch".to_string());
    }
    if update.chain_id != domain.chain_id
        || update.genesis_hash != domain.genesis_hash
        || update.protocol_version != domain.protocol_version
    {
        return Err("validator registry update domain mismatch".to_string());
    }
    let request = ValidatorRegistryUpdateRequest {
        activation_height: update.activation_height,
        previous_registry_root: update.previous_registry_root.clone(),
        new_registry_root: update.new_registry_root.clone(),
        previous_trust_graph_root: update.previous_trust_graph_root.clone(),
        new_trust_graph_root: update.new_trust_graph_root.clone(),
        trust_graph_transition_id: update.trust_graph_transition_id.clone(),
        previous_validators: update.previous_validators.clone(),
        new_validators: update.new_validators.clone(),
        operation: update.operation.clone(),
        subject_node_id: update.subject_node_id.clone(),
        previous_record: update.previous_record.clone(),
        new_record: update.new_record.clone(),
    };
    validate_validator_registry_update_request(domain, &request)?;
    if update.validators.is_empty() {
        return Err("validator registry update validators must be nonempty".to_string());
    }
    if sorted_unique(&update.validators) != update.validators {
        return Err("validator registry update validators must be sorted unique".to_string());
    }
    if update.quorum == 0 || update.quorum > update.validators.len() {
        return Err("validator registry update quorum is invalid".to_string());
    }
    if update.proposer != update.validators[0] {
        return Err("validator registry update proposer mismatch".to_string());
    }
    if sorted_unique(&update.support) != update.support {
        return Err("validator registry update support must be sorted unique".to_string());
    }
    let validator_set: BTreeSet<&str> = update.validators.iter().map(String::as_str).collect();
    if update
        .support
        .iter()
        .any(|validator| !validator_set.contains(validator.as_str()))
    {
        return Err("validator registry update support includes non-validator".to_string());
    }
    if update.support.len() < update.quorum {
        return Err("validator registry update support is below quorum".to_string());
    }

    let config = EssentialSubsetConfig {
        validators: update.validators.clone(),
        quorum: update.quorum,
    };
    let expected_instance_id = validator_registry_update_instance_id(domain, &config, &request)?;
    if update.instance_id != expected_instance_id {
        return Err("validator registry update instance mismatch".to_string());
    }
    let expected_proposal_id =
        validator_registry_update_proposal_id(domain, &update.instance_id, &update.proposer);
    if update.proposal_id != expected_proposal_id {
        return Err("validator registry update proposal mismatch".to_string());
    }
    if update.votes.len() != update.support.len() {
        return Err("validator registry update votes do not match support".to_string());
    }
    let mut vote_support = Vec::with_capacity(update.votes.len());
    for vote in &update.votes {
        if !vote.accept {
            return Err("validator registry update vote is not accepting".to_string());
        }
        if !validator_set.contains(vote.validator.as_str()) {
            return Err("validator registry update vote includes non-validator".to_string());
        }
        let expected_vote_id = validator_registry_update_vote_id(
            domain,
            &update.instance_id,
            &update.proposal_id,
            &vote.validator,
            vote.accept,
        );
        if vote.vote_id != expected_vote_id {
            return Err("validator registry update vote id mismatch".to_string());
        }
        vote_support.push(vote.validator.clone());
    }
    if vote_support != update.support {
        return Err("validator registry update votes do not match support".to_string());
    }

    let expected_certificate_id = validator_registry_update_certificate_id(
        domain,
        &update.instance_id,
        &update.proposal_id,
        update.quorum,
        &update.votes,
    )?;
    if update.certificate_id != expected_certificate_id {
        return Err("validator registry update certificate mismatch".to_string());
    }
    let expected_update_id = validator_registry_update_id(
        domain,
        &update.instance_id,
        &update.certificate_id,
        &request,
        &update.support,
    )?;
    if update.update_id != expected_update_id {
        return Err("validator registry update id mismatch".to_string());
    }

    Ok(())
}
