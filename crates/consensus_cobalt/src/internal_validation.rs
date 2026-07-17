fn governance_proposal(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    kind: &str,
    value: u32,
    lifecycle: GovernanceAmendmentLifecycle,
) -> Result<CobaltProposal, String> {
    let proposer = config
        .validators
        .first()
        .cloned()
        .ok_or_else(|| "validator set must be nonempty".to_string())?;
    let instance_id = governance_instance_id(
        domain,
        &config.validators,
        config.quorum,
        kind,
        value,
        lifecycle,
    );
    let proposal_id = governance_proposal_id(domain, &instance_id, &proposer, kind, value);
    Ok(CobaltProposal {
        instance_id,
        proposal_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        proposer,
        kind: kind.to_string(),
        value,
    })
}

fn governance_instance_id(
    domain: &CobaltDomain,
    validators: &[String],
    quorum: usize,
    kind: &str,
    value: u32,
    lifecycle: GovernanceAmendmentLifecycle,
) -> String {
    let lifecycle_payload = governance_lifecycle_payload(lifecycle);
    hash_hex(
        "postfiat.cobalt.instance.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\nkind={kind}\nvalue={value}\nvalidators={}\nquorum={quorum}\n{}",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            validators.join(","),
            lifecycle_payload,
        )
        .as_bytes(),
    )
}

fn nonuniform_governance_instance_id(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    kind: &str,
    value: u32,
) -> String {
    hash_hex(
        "postfiat.cobalt.nonuniform_governance_instance.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\nkind={kind}\nvalue={value}\nregistry_root={}\ntrust_graph_root={}\ntrust_graph_version={}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version,
            graph.registry_root,
            graph.trust_graph_root,
            graph.graph_version,
        )
        .as_bytes(),
    )
}

fn governance_lifecycle_payload(lifecycle: GovernanceAmendmentLifecycle) -> String {
    if lifecycle.is_immediate() {
        String::new()
    } else {
        format!(
            "activation_height={}\nveto_until_height={}\npaused={}\n",
            lifecycle.activation_height, lifecycle.veto_until_height, lifecycle.paused
        )
    }
}

fn governance_proposal_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposer: &str,
    kind: &str,
    value: u32,
) -> String {
    hash_hex(
        "postfiat.cobalt.proposal.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={instance_id}\nproposer={proposer}\nkind={kind}\nvalue={value}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version
        )
        .as_bytes(),
    )
}

fn vote_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposal_id: &str,
    validator: &str,
    accept: bool,
) -> String {
    hash_hex(
        "postfiat.cobalt.vote.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={instance_id}\nproposal_id={proposal_id}\nvalidator={validator}\naccept={accept}\n",
            domain.chain_id,
            domain.genesis_hash,
            domain.protocol_version
        )
        .as_bytes(),
    )
}

fn certificate_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposal_id: &str,
    quorum: usize,
    votes: &[CobaltVote],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        instance_id,
        proposal_id,
        quorum,
        votes,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.certificate.v1", &encoded))
}

fn validate_nonuniform_certificate_context(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    linkage_report: &LinkageReport,
    current_height: u64,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    if current_height < graph.activation_height {
        return Err("non-uniform certificate trust graph is not active".to_string());
    }
    if linkage_report.trust_graph_root != graph.trust_graph_root {
        return Err("non-uniform certificate linkage report trust graph root mismatch".to_string());
    }
    if linkage_report.registry_root != graph.registry_root {
        return Err("non-uniform certificate linkage report registry root mismatch".to_string());
    }
    if linkage_report.trust_view_count != graph.trust_views.len() {
        return Err("non-uniform certificate linkage report trust view count mismatch".to_string());
    }
    let expected_report_hash = linkage_report_hash(LinkageReportHashInput {
        domain,
        graph,
        actively_byzantine: &linkage_report.actively_byzantine,
        linked_pairs: &linkage_report.linked_pairs,
        fully_linked_pairs: &linkage_report.fully_linked_pairs,
        unsafe_pairs: &linkage_report.unsafe_pairs,
        weakly_connected_validators: &linkage_report.weakly_connected_validators,
        strongly_connected_validators: &linkage_report.strongly_connected_validators,
        connectivity: &linkage_report.connectivity,
    })?;
    if linkage_report.report_hash != expected_report_hash {
        return Err("non-uniform certificate linkage report hash mismatch".to_string());
    }
    let expected_report = analyze_trust_graph(
        domain,
        graph,
        &CobaltFaultModel {
            actively_byzantine: linkage_report.actively_byzantine.clone(),
        },
    )?;
    if linkage_report != &expected_report {
        return Err(
            "non-uniform certificate linkage report does not match trust graph".to_string(),
        );
    }
    if !linkage_report.unsafe_pairs.is_empty() {
        return Err("non-uniform certificate linkage report contains unsafe pairs".to_string());
    }
    Ok(())
}

fn validate_nonuniform_proposal(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    proposal: &CobaltProposal,
) -> Result<(), String> {
    if proposal.chain_id != domain.chain_id
        || proposal.genesis_hash != domain.genesis_hash
        || proposal.protocol_version != domain.protocol_version
    {
        return Err("non-uniform governance proposal domain mismatch".to_string());
    }
    validate_amendment_kind(&proposal.kind)?;
    validate_amendment_value(&proposal.kind, proposal.value)?;
    let expected_instance_id =
        nonuniform_governance_instance_id(domain, graph, &proposal.kind, proposal.value);
    if proposal.instance_id != expected_instance_id {
        return Err("non-uniform governance proposal instance mismatch".to_string());
    }
    let expected_proposal_id = governance_proposal_id(
        domain,
        &proposal.instance_id,
        &proposal.proposer,
        &proposal.kind,
        proposal.value,
    );
    if proposal.proposal_id != expected_proposal_id {
        return Err("non-uniform governance proposal id mismatch".to_string());
    }
    if !graph
        .trust_views
        .iter()
        .any(|view| view.validator == proposal.proposer)
    {
        return Err("non-uniform governance proposal proposer is not in trust graph".to_string());
    }
    Ok(())
}

fn trust_view_for_validator<'a>(
    graph: &'a TrustGraph,
    local_validator: &str,
) -> Result<&'a TrustView, String> {
    graph
        .trust_views
        .iter()
        .find(|view| view.validator == local_validator)
        .ok_or_else(|| "non-uniform certificate local trust view not found".to_string())
}

fn build_trust_graph_lifecycle_transition(
    domain: &CobaltDomain,
    current_graph: &TrustGraph,
    new_view: TrustView,
    activation_height: u64,
    fault_model: &CobaltFaultModel,
    operation: &str,
) -> Result<(TrustGraph, TrustGraphLifecycleRecord), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, current_graph)?;
    validate_trust_view(domain, &new_view)?;
    match operation {
        TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE
        | TRUST_GRAPH_LIFECYCLE_OP_ESSENTIAL_SUBSET_UPDATE => {}
        other => {
            return Err(format!(
                "unsupported trust graph lifecycle operation `{other}`"
            ))
        }
    }
    if activation_height == 0 {
        return Err("trust graph lifecycle activation height must be nonzero".to_string());
    }
    let previous_view = trust_view_for_validator(current_graph, &new_view.validator)?;
    if new_view.view_version <= previous_view.view_version {
        return Err("trust graph lifecycle view version must increase".to_string());
    }
    if operation == TRUST_GRAPH_LIFECYCLE_OP_ESSENTIAL_SUBSET_UPDATE
        && previous_view.essential_subsets == new_view.essential_subsets
    {
        return Err("essential subset update must change essential subsets".to_string());
    }
    let mut views = current_graph.trust_views.clone();
    let replacement = views
        .iter_mut()
        .find(|view| view.validator == new_view.validator)
        .ok_or_else(|| "trust graph lifecycle previous view not found".to_string())?;
    *replacement = new_view.clone();
    let new_graph = build_trust_graph(
        domain,
        current_graph
            .graph_version
            .checked_add(1)
            .ok_or_else(|| "trust graph lifecycle graph version overflow".to_string())?,
        current_graph.registry_root.clone(),
        activation_height,
        Some(current_graph.trust_graph_root.clone()),
        views,
    )?;
    let linkage_report = analyze_trust_graph(domain, &new_graph, fault_model)?;
    if !linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph lifecycle update is unsafe before activation".to_string());
    }
    let transition = build_trust_graph_transition(
        domain,
        current_graph.registry_root.clone(),
        new_graph.registry_root.clone(),
        current_graph.trust_graph_root.clone(),
        new_graph.trust_graph_root.clone(),
        activation_height,
    )?;
    let mut record = TrustGraphLifecycleRecord {
        record_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        operation: operation.to_string(),
        subject_validator: new_view.validator.clone(),
        previous_registry_root: transition.previous_registry_root,
        new_registry_root: transition.new_registry_root,
        previous_trust_graph_root: transition.previous_trust_graph_root,
        new_trust_graph_root: transition.new_trust_graph_root,
        trust_graph_transition_id: transition.transition_id,
        activation_height,
        previous_trust_view_id: previous_view.trust_view_id.clone(),
        new_trust_view_id: new_view.trust_view_id.clone(),
        previous_subset_ids: trust_view_subset_ids(previous_view),
        new_subset_ids: trust_view_subset_ids(&new_view),
        linkage_report_hash: linkage_report.report_hash.clone(),
    };
    record.record_id = trust_graph_lifecycle_record_id(domain, &record)?;
    validate_trust_graph_lifecycle_record(
        domain,
        current_graph,
        &new_graph,
        &linkage_report,
        &record,
    )?;
    Ok((new_graph, record))
}

fn trust_view_subset_ids(view: &TrustView) -> Vec<EssentialSubsetId> {
    view.essential_subsets
        .iter()
        .map(|subset| subset.subset_id.clone())
        .collect()
}

fn trust_graph_lifecycle_record_id(
    domain: &CobaltDomain,
    record: &TrustGraphLifecycleRecord,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
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
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.trust_graph_lifecycle_record.v1",
        &encoded,
    ))
}

fn trust_graph_rollback_record_id(
    domain: &CobaltDomain,
    record: &TrustGraphRollbackRecord,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
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
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.trust_graph_rollback_record.v1",
        &encoded,
    ))
}

fn validate_support_in_view(view: &TrustView, support: &[String]) -> Result<(), String> {
    let derived_unl: BTreeSet<&str> = view.derived_unl.iter().map(String::as_str).collect();
    if support
        .iter()
        .any(|validator| !derived_unl.contains(validator.as_str()))
    {
        return Err(
            "non-uniform certificate support includes validator outside local view".to_string(),
        );
    }
    Ok(())
}

fn satisfied_subsets_for_view(
    view: &TrustView,
    support: &[String],
) -> Result<Vec<NonUniformSatisfiedSubset>, String> {
    validate_support_scope(support)?;
    let support: BTreeSet<&str> = support.iter().map(String::as_str).collect();
    let mut satisfied = Vec::with_capacity(view.essential_subsets.len());
    for subset in &view.essential_subsets {
        let subset_support: Vec<String> = subset
            .validators
            .iter()
            .filter(|validator| support.contains(validator.as_str()))
            .cloned()
            .collect();
        if subset_support.len() < subset.quorum {
            return Err(format!(
                "non-uniform certificate support does not satisfy essential subset {}",
                subset.subset_id
            ));
        }
        satisfied.push(NonUniformSatisfiedSubset {
            subset_id: subset.subset_id.clone(),
            validator_count: subset.validator_count,
            max_active_byzantine: subset.max_active_byzantine,
            quorum: subset.quorum,
            support: subset_support,
        });
    }
    Ok(satisfied)
}

fn governance_votes_for_support(
    domain: &CobaltDomain,
    proposal: &CobaltProposal,
    support: &[String],
) -> Vec<GovernanceVote> {
    support
        .iter()
        .map(|validator| GovernanceVote {
            vote_id: vote_id(
                domain,
                &proposal.instance_id,
                &proposal.proposal_id,
                validator,
                true,
            ),
            validator: validator.clone(),
            accept: true,
        })
        .collect()
}

fn validate_nonuniform_certificate_votes(
    domain: &CobaltDomain,
    proposal: &CobaltProposal,
    certificate: &NonUniformGovernanceCertificate,
) -> Result<(), String> {
    if certificate.votes.len() != certificate.support.len() {
        return Err("non-uniform certificate votes do not match support".to_string());
    }
    let mut vote_support = Vec::with_capacity(certificate.votes.len());
    for vote in &certificate.votes {
        if !vote.accept {
            return Err("non-uniform certificate vote is not accepting".to_string());
        }
        let expected_vote_id = vote_id(
            domain,
            &proposal.instance_id,
            &proposal.proposal_id,
            &vote.validator,
            vote.accept,
        );
        if vote.vote_id != expected_vote_id {
            return Err("non-uniform certificate vote id mismatch".to_string());
        }
        vote_support.push(vote.validator.clone());
    }
    if vote_support != certificate.support {
        return Err("non-uniform certificate votes do not match support".to_string());
    }
    Ok(())
}

fn validate_rbc_domain(
    domain: &CobaltDomain,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
) -> Result<(), String> {
    validate_domain(domain)?;
    if chain_id != domain.chain_id
        || genesis_hash != domain.genesis_hash
        || protocol_version != domain.protocol_version
    {
        return Err("RBC message domain mismatch".to_string());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_rbc_linked_message(
    domain: &CobaltDomain,
    label: &str,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    trust_graph_root: &str,
    sender: &str,
    proposer: &str,
    amendment_slot: u64,
    payload_hash: &str,
    propose_message_id: &str,
    signature_hex: &str,
    propose: &RbcPropose,
) -> Result<(), String> {
    validate_rbc_domain(domain, chain_id, genesis_hash, protocol_version)?;
    validate_hash_hex("RBC trust graph root", trust_graph_root)?;
    validate_node_id("RBC sender", sender)?;
    validate_node_id("RBC proposer", proposer)?;
    validate_hash_hex("RBC payload hash", payload_hash)?;
    validate_hash_hex("RBC propose message id", propose_message_id)?;
    validate_rbc_signature_hex(signature_hex)?;
    if trust_graph_root != propose.trust_graph_root
        || proposer != propose.sender
        || amendment_slot != propose.amendment_slot
        || payload_hash != propose.payload_hash
        || propose_message_id != propose.message_id
    {
        return Err(format!("{label} does not match RBC propose"));
    }
    Ok(())
}

fn validate_rbc_signature_hex(signature_hex: &str) -> Result<(), String> {
    if signature_hex.is_empty() {
        return Ok(());
    }
    if signature_hex.len() > MAX_COBALT_SIGNATURE_HEX_LEN {
        return Err("Cobalt signature exceeds maximum hex length".to_string());
    }
    if !signature_hex.len().is_multiple_of(2) || !is_lower_hex(signature_hex) {
        return Err("RBC signature must be lowercase hex".to_string());
    }
    Ok(())
}

fn evaluate_rbc_support(
    view: &TrustView,
    message_kind: &str,
    propose: &RbcPropose,
    support: Vec<String>,
) -> Result<RbcSupportEvaluation, String> {
    let support = sorted_unique(&support);
    validate_support_scope(&support)?;
    validate_support_in_view(view, &support)?;
    let weak_support = has_weak_support(view, &support)?;
    let strong_support = has_strong_support(view, &support)?;
    let strong_satisfied_subsets = if strong_support {
        satisfied_subsets_for_view(view, &support)?
    } else {
        Vec::new()
    };
    Ok(RbcSupportEvaluation {
        trust_view_id: view.trust_view_id.clone(),
        local_validator: view.validator.clone(),
        message_kind: message_kind.to_string(),
        propose_message_id: propose.message_id.clone(),
        amendment_slot: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        support,
        weak_support,
        strong_support,
        strong_satisfied_subsets,
    })
}

fn rbc_conflicting_accept_evidence_id(
    domain: &CobaltDomain,
    evidence: &RbcConflictingAcceptEvidence,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        evidence.trust_graph_root.as_str(),
        evidence.amendment_slot,
        evidence.proposer.as_str(),
        evidence.left_sender.as_str(),
        evidence.right_sender.as_str(),
        evidence.left_payload_hash.as_str(),
        evidence.right_payload_hash.as_str(),
        evidence.left_propose_message_id.as_str(),
        evidence.right_propose_message_id.as_str(),
        evidence.linked,
        evidence.fully_linked,
        evidence.reason.as_str(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.rbc.conflicting_accept_evidence.v1",
        &encoded,
    ))
}

#[allow(clippy::too_many_arguments)]
fn validate_abba_message(
    domain: &CobaltDomain,
    kind: &str,
    message_id: &str,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    trust_graph_root: &str,
    sender: &str,
    agreement_id: &str,
    round: u64,
    signature_hex: &str,
    expected_message_id: &str,
) -> Result<(), String> {
    validate_rbc_domain(domain, chain_id, genesis_hash, protocol_version)?;
    validate_hash_hex("ABBA trust graph root", trust_graph_root)?;
    validate_node_id("ABBA sender", sender)?;
    validate_hash_hex("ABBA agreement id", agreement_id)?;
    if round == 0 {
        return Err("ABBA round must be nonzero".to_string());
    }
    validate_rbc_signature_hex(signature_hex)?;
    if message_id != expected_message_id {
        return Err(format!("ABBA {kind} message id mismatch"));
    }
    Ok(())
}

#[derive(Serialize)]
struct AbbaSigningPayload<'a> {
    kind: &'a str,
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    trust_graph_root: &'a str,
    sender: &'a str,
    agreement_id: &'a str,
    round: u64,
    value: bool,
}

#[allow(clippy::too_many_arguments)]
fn abba_signing_payload_bytes(
    kind: &str,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    trust_graph_root: &str,
    sender: &str,
    agreement_id: &str,
    round: u64,
    value: bool,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&AbbaSigningPayload {
        kind,
        chain_id,
        genesis_hash,
        protocol_version,
        trust_graph_root,
        sender,
        agreement_id,
        round,
        value,
    })
    .map_err(|error| error.to_string())
}

#[derive(Serialize)]
struct RbcSigningPayload<'a> {
    kind: &'a str,
    chain_id: &'a str,
    genesis_hash: &'a str,
    protocol_version: u32,
    trust_graph_root: &'a str,
    sender: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    proposer: Option<&'a str>,
    amendment_slot: u64,
    payload_hash: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    propose_message_id: Option<&'a str>,
}

#[allow(clippy::too_many_arguments)]
fn rbc_signing_payload_bytes(
    kind: &str,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    trust_graph_root: &str,
    sender: &str,
    proposer: Option<&str>,
    amendment_slot: u64,
    payload_hash: &str,
    propose_message_id: Option<&str>,
) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&RbcSigningPayload {
        kind,
        chain_id,
        genesis_hash,
        protocol_version,
        trust_graph_root,
        sender,
        proposer,
        amendment_slot,
        payload_hash,
        propose_message_id,
    })
    .map_err(|error| error.to_string())
}

fn evaluate_abba_support(
    view: &TrustView,
    message_kind: &str,
    agreement_id: &str,
    round: u64,
    value: bool,
    support: Vec<String>,
) -> Result<AbbaSupportEvaluation, String> {
    validate_hash_hex("ABBA agreement id", agreement_id)?;
    if round == 0 {
        return Err("ABBA round must be nonzero".to_string());
    }
    let support = sorted_unique(&support);
    validate_support_scope(&support)?;
    validate_support_in_view(view, &support)?;
    let weak_support = has_weak_support(view, &support)?;
    let strong_support = has_strong_support(view, &support)?;
    let strong_satisfied_subsets = if strong_support {
        satisfied_subsets_for_view(view, &support)?
    } else {
        Vec::new()
    };
    Ok(AbbaSupportEvaluation {
        trust_view_id: view.trust_view_id.clone(),
        local_validator: view.validator.clone(),
        message_kind: message_kind.to_string(),
        agreement_id: agreement_id.to_string(),
        round,
        value,
        support,
        weak_support,
        strong_support,
        strong_satisfied_subsets,
    })
}

fn abba_conflicting_finish_evidence_id(
    domain: &CobaltDomain,
    evidence: &AbbaConflictingFinishEvidence,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        evidence.trust_graph_root.as_str(),
        evidence.agreement_id.as_str(),
        evidence.round,
        evidence.left_sender.as_str(),
        evidence.right_sender.as_str(),
        evidence.left_value,
        evidence.right_value,
        evidence.linked,
        evidence.fully_linked,
        evidence.reason.as_str(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.abba.conflicting_finish_evidence.v1",
        &encoded,
    ))
}

#[derive(Clone, Copy)]
struct AbbaEquivocationCandidate<'a> {
    message_kind: &'static str,
    message_id: &'a str,
    trust_graph_root: &'a str,
    sender: &'a str,
    agreement_id: &'a str,
    round: u64,
    value: bool,
}

fn abba_init_equivocation_candidate(message: &AbbaInit) -> AbbaEquivocationCandidate<'_> {
    AbbaEquivocationCandidate {
        message_kind: "init",
        message_id: &message.message_id,
        trust_graph_root: &message.trust_graph_root,
        sender: &message.sender,
        agreement_id: &message.agreement_id,
        round: message.round,
        value: message.value,
    }
}

fn abba_aux_equivocation_candidate(message: &AbbaAux) -> AbbaEquivocationCandidate<'_> {
    AbbaEquivocationCandidate {
        message_kind: "aux",
        message_id: &message.message_id,
        trust_graph_root: &message.trust_graph_root,
        sender: &message.sender,
        agreement_id: &message.agreement_id,
        round: message.round,
        value: message.value,
    }
}

fn abba_conf_equivocation_candidate(message: &AbbaConf) -> AbbaEquivocationCandidate<'_> {
    AbbaEquivocationCandidate {
        message_kind: "conf",
        message_id: &message.message_id,
        trust_graph_root: &message.trust_graph_root,
        sender: &message.sender,
        agreement_id: &message.agreement_id,
        round: message.round,
        value: message.value,
    }
}

fn abba_finish_equivocation_candidate(message: &AbbaFinish) -> AbbaEquivocationCandidate<'_> {
    AbbaEquivocationCandidate {
        message_kind: "finish",
        message_id: &message.message_id,
        trust_graph_root: &message.trust_graph_root,
        sender: &message.sender,
        agreement_id: &message.agreement_id,
        round: message.round,
        value: message.value,
    }
}

fn detect_abba_equivocation_candidates(
    domain: &CobaltDomain,
    left: AbbaEquivocationCandidate<'_>,
    right: AbbaEquivocationCandidate<'_>,
) -> Result<Option<AbbaEquivocationEvidence>, String> {
    validate_domain(domain)?;
    if left.message_kind != right.message_kind {
        return Ok(None);
    }
    if left.trust_graph_root != right.trust_graph_root {
        return Err("ABBA equivocation evidence trust graph root mismatch".to_string());
    }
    if left.sender != right.sender || left.agreement_id != right.agreement_id {
        return Ok(None);
    }
    if left.round != right.round {
        return Ok(None);
    }
    if left.value == right.value {
        return Ok(None);
    }
    let (left_value, left_message_id, right_value, right_message_id) = if left.value {
        (
            right.value,
            right.message_id.to_string(),
            left.value,
            left.message_id.to_string(),
        )
    } else {
        (
            left.value,
            left.message_id.to_string(),
            right.value,
            right.message_id.to_string(),
        )
    };
    let mut evidence = AbbaEquivocationEvidence {
        evidence_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        trust_graph_root: left.trust_graph_root.to_string(),
        agreement_id: left.agreement_id.to_string(),
        round: left.round,
        message_kind: left.message_kind.to_string(),
        sender: left.sender.to_string(),
        left_value,
        right_value,
        left_message_id,
        right_message_id,
        reason: "same validator sent conflicting ABBA values for one round".to_string(),
    };
    evidence.evidence_id = abba_equivocation_evidence_id(domain, &evidence)?;
    Ok(Some(evidence))
}

fn collect_abba_equivocations<'a>(
    domain: &CobaltDomain,
    state: &AbbaRoundState,
    candidates: impl IntoIterator<Item = AbbaEquivocationCandidate<'a>>,
    evidence_by_id: &mut BTreeMap<String, AbbaEquivocationEvidence>,
) -> Result<(), String> {
    let candidates = candidates.into_iter().collect::<Vec<_>>();
    for candidate in &candidates {
        if candidate.trust_graph_root != state.trust_graph_root
            || candidate.agreement_id != state.agreement_id
            || candidate.round != state.round
        {
            return Err("ABBA round state contains message outside its round".to_string());
        }
    }
    for left_index in 0..candidates.len() {
        for right_index in (left_index + 1)..candidates.len() {
            if let Some(evidence) = detect_abba_equivocation_candidates(
                domain,
                candidates[left_index],
                candidates[right_index],
            )? {
                evidence_by_id.insert(evidence.evidence_id.clone(), evidence);
            }
        }
    }
    Ok(())
}

fn abba_equivocating_senders(candidates: &[AbbaEquivocationCandidate<'_>]) -> BTreeSet<String> {
    let mut value_by_sender = BTreeMap::new();
    let mut equivocal_senders = BTreeSet::new();
    for candidate in candidates {
        match value_by_sender.insert(candidate.sender, candidate.value) {
            Some(first_value) if first_value != candidate.value => {
                equivocal_senders.insert(candidate.sender.to_string());
            }
            Some(first_value) => {
                value_by_sender.insert(candidate.sender, first_value);
            }
            None => {}
        }
    }
    equivocal_senders
}

fn abba_equivocation_evidence_id(
    domain: &CobaltDomain,
    evidence: &AbbaEquivocationEvidence,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        evidence.trust_graph_root.as_str(),
        evidence.agreement_id.as_str(),
        evidence.round,
        evidence.message_kind.as_str(),
        evidence.sender.as_str(),
        evidence.left_value,
        evidence.right_value,
        evidence.left_message_id.as_str(),
        evidence.right_message_id.as_str(),
        evidence.reason.as_str(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.abba.equivocation_evidence.v1",
        &encoded,
    ))
}

fn validate_mvba_candidate(domain: &CobaltDomain, candidate: &MvbaCandidate) -> Result<(), String> {
    if candidate.chain_id != domain.chain_id
        || candidate.genesis_hash != domain.genesis_hash
        || candidate.protocol_version != domain.protocol_version
    {
        return Err("MVBA candidate domain mismatch".to_string());
    }
    validate_hash_hex("MVBA trust graph root", &candidate.trust_graph_root)?;
    validate_node_id("MVBA proposer", &candidate.proposer)?;
    validate_hash_hex("MVBA payload hash", &candidate.payload_hash)?;
    validate_hash_hex("MVBA propose message id", &candidate.propose_message_id)?;
    let expected_propose_message_id = rbc_propose_message_id(&RbcPropose {
        message_id: String::new(),
        chain_id: candidate.chain_id.clone(),
        genesis_hash: candidate.genesis_hash.clone(),
        protocol_version: candidate.protocol_version,
        trust_graph_root: candidate.trust_graph_root.clone(),
        sender: candidate.proposer.clone(),
        amendment_slot: candidate.amendment_slot,
        payload_hash: candidate.payload_hash.clone(),
        signature_hex: String::new(),
    })?;
    if candidate.propose_message_id != expected_propose_message_id {
        return Err("MVBA candidate propose message id mismatch".to_string());
    }
    let expected_id = mvba_candidate_id(domain, candidate)?;
    if candidate.candidate_id != expected_id {
        return Err("MVBA candidate id mismatch".to_string());
    }
    Ok(())
}

fn validate_mvba_valid_input_set(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    input_set: &MvbaValidInputSet,
) -> Result<(), String> {
    validate_hash_hex("MVBA agreement id", &input_set.agreement_id)?;
    let view = trust_view_for_validator(graph, &input_set.local_validator)?;
    if input_set.trust_view_id != view.trust_view_id {
        return Err("MVBA valid input set trust view id mismatch".to_string());
    }
    if input_set.candidates.is_empty() {
        return Err("MVBA valid input set requires at least one candidate".to_string());
    }
    if input_set.candidates.len() > MAX_MVBA_CANDIDATES_PER_SET {
        return Err("MVBA valid input set has too many candidates".to_string());
    }
    let candidate_ids: Vec<String> = input_set
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect();
    if sorted_unique(&candidate_ids) != candidate_ids {
        return Err("MVBA valid input set candidates must be sorted unique".to_string());
    }
    for candidate in &input_set.candidates {
        validate_mvba_candidate(domain, candidate)?;
        if candidate.trust_graph_root != graph.trust_graph_root {
            return Err("MVBA valid input candidate trust graph root mismatch".to_string());
        }
    }
    if !candidate_ids
        .iter()
        .any(|candidate_id| candidate_id == &input_set.output_candidate_id)
    {
        return Err("MVBA output candidate id is not in valid input set".to_string());
    }
    if input_set.output_candidate_id != input_set.candidates[0].candidate_id {
        return Err("MVBA output candidate must be deterministic first candidate".to_string());
    }
    Ok(())
}

fn validate_dabc_ratified_amendment_core(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified: &DabcRatifiedAmendment,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    if ratified.chain_id != domain.chain_id
        || ratified.genesis_hash != domain.genesis_hash
        || ratified.protocol_version != domain.protocol_version
    {
        return Err("DABC ratified amendment domain mismatch".to_string());
    }
    if ratified.registry_root != graph.registry_root {
        return Err("DABC ratified amendment registry root mismatch".to_string());
    }
    if ratified.trust_graph_root != graph.trust_graph_root {
        return Err("DABC ratified amendment trust graph root mismatch".to_string());
    }
    validate_hash_hex(
        "DABC parent ratification id",
        &ratified.parent_ratification_id,
    )?;
    validate_hash_hex("DABC MVBA agreement id", &ratified.mvba_agreement_id)?;
    validate_hash_hex("DABC output candidate id", &ratified.output_candidate_id)?;
    validate_mvba_candidate(domain, &ratified.candidate)?;
    if ratified.candidate.trust_graph_root != ratified.trust_graph_root {
        return Err("DABC ratified candidate trust graph root mismatch".to_string());
    }
    if ratified.candidate.amendment_slot != ratified.amendment_slot {
        return Err("DABC ratified candidate amendment slot mismatch".to_string());
    }
    if ratified.candidate.candidate_id != ratified.output_candidate_id {
        return Err("DABC ratified output candidate mismatch".to_string());
    }
    if ratified.activation_height == 0 {
        return Err("DABC ratified amendment activation height must be nonzero".to_string());
    }
    let expected_id = dabc_ratification_id(domain, ratified)?;
    if ratified.ratification_id != expected_id {
        return Err("DABC ratified amendment id mismatch".to_string());
    }
    Ok(())
}

fn dabc_genesis_parent_id() -> String {
    "0".repeat(96)
}

fn validate_dabc_ratified_chain(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    ratified_chain: &[DabcRatifiedAmendment],
) -> Result<(), String> {
    if ratified_chain.is_empty() {
        return Err("DABC ratified chain must be nonempty".to_string());
    }
    let mut slots = BTreeSet::new();
    let mut previous = None;
    for (index, ratified) in ratified_chain.iter().enumerate() {
        let expected_sequence = u64::try_from(index)
            .map_err(|_| "DABC ratified chain index overflow".to_string())?
            .checked_add(1)
            .ok_or_else(|| "DABC ratified chain sequence overflow".to_string())?;
        if ratified.sequence != expected_sequence {
            return Err("DABC ratified chain must be sorted by sequence".to_string());
        }
        if !slots.insert(ratified.amendment_slot) {
            return Err("DABC ratified chain contains duplicate amendment slot".to_string());
        }
        validate_dabc_ratified_amendment(domain, graph, ratified, previous)?;
        previous = Some(ratified);
    }
    Ok(())
}

fn validate_dabc_pending_pairs(pairs: &[DabcPendingPair]) -> Result<(), String> {
    if pairs.len() > MAX_DABC_PENDING_PAIRS_PER_CHECK {
        return Err("DABC full-knowledge check has too many pending pairs".to_string());
    }
    if !dabc_pending_pairs_sorted_unique(pairs) {
        return Err("DABC full-knowledge pending pairs must be sorted unique".to_string());
    }
    for pair in pairs {
        validate_hash_hex(
            "DABC full-knowledge pending output candidate id",
            &pair.output_candidate_id,
        )?;
    }
    Ok(())
}

fn dabc_pending_pairs_sorted_unique(pairs: &[DabcPendingPair]) -> bool {
    let mut previous: Option<(u64, &str)> = None;
    for pair in pairs {
        let current = (pair.amendment_slot, pair.output_candidate_id.as_str());
        if previous.is_some_and(|previous| previous >= current) {
            return false;
        }
        previous = Some(current);
    }
    true
}

fn dabc_pending_pair_cmp(left: &DabcPendingPair, right: &DabcPendingPair) -> std::cmp::Ordering {
    left.amendment_slot
        .cmp(&right.amendment_slot)
        .then_with(|| left.output_candidate_id.cmp(&right.output_candidate_id))
}

fn dabc_required_checkpoint_heights(
    interval_height: u64,
    wait_until_height: u64,
) -> Result<Vec<u64>, String> {
    if interval_height == 0 {
        return Err("DABC full-knowledge interval height must be nonzero".to_string());
    }
    let interval_count = wait_until_height / interval_height;
    let interval_count_usize = usize::try_from(interval_count)
        .map_err(|_| "DABC full-knowledge interval count overflow".to_string())?;
    if interval_count_usize > MAX_DABC_FULL_KNOWLEDGE_INTERVALS {
        return Err("DABC full-knowledge checkpoint covers too many intervals".to_string());
    }
    let mut heights = Vec::with_capacity(interval_count_usize);
    let mut height = interval_height;
    while height <= wait_until_height {
        heights.push(height);
        height = height
            .checked_add(interval_height)
            .ok_or_else(|| "DABC full-knowledge checkpoint height overflow".to_string())?;
    }
    Ok(heights)
}

fn dabc_full_knowledge_check_cmp(
    left: &DabcFullKnowledgeCheck,
    right: &DabcFullKnowledgeCheck,
) -> std::cmp::Ordering {
    left.checkpoint_height
        .cmp(&right.checkpoint_height)
        .then_with(|| left.sender.cmp(&right.sender))
}

fn dabc_full_knowledge_checks_sorted_unique(checks: &[DabcFullKnowledgeCheck]) -> bool {
    let mut previous: Option<(u64, &str)> = None;
    for check in checks {
        let current = (check.checkpoint_height, check.sender.as_str());
        if previous.is_some_and(|previous| previous >= current) {
            return false;
        }
        previous = Some(current);
    }
    true
}

fn reject_duplicate_full_knowledge_check_keys(
    checks: &[DabcFullKnowledgeCheck],
) -> Result<(), String> {
    let mut previous: Option<(u64, &str)> = None;
    for check in checks {
        let current = (check.checkpoint_height, check.sender.as_str());
        if previous == Some(current) {
            return Err(
                "DABC full-knowledge checkpoint contains duplicate check sender/height".to_string(),
            );
        }
        previous = Some(current);
    }
    Ok(())
}

pub fn governance_amendment_id(
    domain: &CobaltDomain,
    instance_id: &str,
    certificate_id: &str,
    kind: &str,
    value: u32,
    support: &[String],
) -> Result<String, String> {
    validate_domain(domain)?;
    let payload = format!(
        "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={instance_id}\ncertificate_id={certificate_id}\nkind={kind}\nvalue={value}\nsupport={}\n",
        domain.chain_id,
        domain.genesis_hash,
        domain.protocol_version,
        support.join(","),
    );
    Ok(hash_hex("postfiat.cobalt.amendment.v1", payload.as_bytes()))
}

fn validator_registry_update_instance_id(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    request: &ValidatorRegistryUpdateRequest,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        config.validators.as_slice(),
        config.quorum,
        request,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.validator_registry_update.instance.v1",
        &encoded,
    ))
}

fn validator_registry_update_proposal_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposer: &str,
) -> String {
    hash_hex(
        "postfiat.cobalt.validator_registry_update.proposal.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={instance_id}\nproposer={proposer}\n",
            domain.chain_id, domain.genesis_hash, domain.protocol_version
        )
        .as_bytes(),
    )
}

fn validator_registry_update_vote_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposal_id: &str,
    validator: &str,
    accept: bool,
) -> String {
    hash_hex(
        "postfiat.cobalt.validator_registry_update.vote.v1",
        format!(
            "chain_id={}\ngenesis_hash={}\nprotocol_version={}\ninstance_id={instance_id}\nproposal_id={proposal_id}\nvalidator={validator}\naccept={accept}\n",
            domain.chain_id, domain.genesis_hash, domain.protocol_version
        )
        .as_bytes(),
    )
}

fn validator_registry_update_certificate_id(
    domain: &CobaltDomain,
    instance_id: &str,
    proposal_id: &str,
    quorum: usize,
    votes: &[GovernanceVote],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        instance_id,
        proposal_id,
        quorum,
        votes,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.validator_registry_update.certificate.v1",
        &encoded,
    ))
}

fn validator_registry_update_id(
    domain: &CobaltDomain,
    instance_id: &str,
    certificate_id: &str,
    request: &ValidatorRegistryUpdateRequest,
    support: &[String],
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        instance_id,
        certificate_id,
        request,
        support,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.validator_registry_update.v1",
        &encoded,
    ))
}

pub fn verify_governance_amendment(
    domain: &CobaltDomain,
    amendment: &GovernanceAmendment,
) -> Result<(), String> {
    validate_domain(domain)?;
    if amendment.chain_id != domain.chain_id
        || amendment.genesis_hash != domain.genesis_hash
        || amendment.protocol_version != domain.protocol_version
    {
        return Err("governance amendment domain mismatch".to_string());
    }
    validate_amendment_kind(&amendment.kind)?;
    validate_amendment_value(&amendment.kind, amendment.value)?;
    let lifecycle = GovernanceAmendmentLifecycle {
        activation_height: amendment.activation_height,
        veto_until_height: amendment.veto_until_height,
        paused: amendment.paused,
    };
    validate_amendment_lifecycle(lifecycle)?;
    if amendment.validators.is_empty() {
        return Err("governance amendment validators must be nonempty".to_string());
    }
    if sorted_unique(&amendment.validators) != amendment.validators {
        return Err("governance amendment validators must be sorted unique".to_string());
    }
    if amendment.quorum == 0 || amendment.quorum > amendment.validators.len() {
        return Err("governance amendment quorum is invalid".to_string());
    }
    if !amendment
        .validators
        .iter()
        .any(|validator| validator == &amendment.proposer)
    {
        return Err("governance amendment proposer is not a validator".to_string());
    }
    if amendment.proposer != amendment.validators[0] {
        return Err("governance amendment proposer mismatch".to_string());
    }
    if sorted_unique(&amendment.support) != amendment.support {
        return Err("governance amendment support must be sorted unique".to_string());
    }
    let validator_set: BTreeSet<&str> = amendment.validators.iter().map(String::as_str).collect();
    if amendment
        .support
        .iter()
        .any(|validator| !validator_set.contains(validator.as_str()))
    {
        return Err("governance amendment support includes non-validator".to_string());
    }
    if amendment.support.len() < amendment.quorum {
        return Err("governance amendment support is below quorum".to_string());
    }

    let expected_instance_id = governance_instance_id(
        domain,
        &amendment.validators,
        amendment.quorum,
        &amendment.kind,
        amendment.value,
        lifecycle,
    );
    if amendment.instance_id != expected_instance_id {
        return Err("governance amendment instance mismatch".to_string());
    }
    let expected_proposal_id = governance_proposal_id(
        domain,
        &amendment.instance_id,
        &amendment.proposer,
        &amendment.kind,
        amendment.value,
    );
    if amendment.proposal_id != expected_proposal_id {
        return Err("governance amendment proposal mismatch".to_string());
    }

    if amendment.votes.len() != amendment.support.len() {
        return Err("governance amendment votes do not match support".to_string());
    }
    let mut cobalt_votes = Vec::with_capacity(amendment.votes.len());
    let mut vote_support = Vec::with_capacity(amendment.votes.len());
    for vote in &amendment.votes {
        if !vote.accept {
            return Err("governance amendment vote is not accepting".to_string());
        }
        if !validator_set.contains(vote.validator.as_str()) {
            return Err("governance amendment vote includes non-validator".to_string());
        }
        let expected_vote_id = vote_id(
            domain,
            &amendment.instance_id,
            &amendment.proposal_id,
            &vote.validator,
            vote.accept,
        );
        if vote.vote_id != expected_vote_id {
            return Err("governance amendment vote id mismatch".to_string());
        }
        vote_support.push(vote.validator.clone());
        cobalt_votes.push(CobaltVote {
            vote_id: vote.vote_id.clone(),
            instance_id: amendment.instance_id.clone(),
            proposal_id: amendment.proposal_id.clone(),
            chain_id: amendment.chain_id.clone(),
            genesis_hash: amendment.genesis_hash.clone(),
            protocol_version: amendment.protocol_version,
            validator: vote.validator.clone(),
            accept: vote.accept,
        });
    }
    if vote_support != amendment.support {
        return Err("governance amendment votes do not match support".to_string());
    }

    let expected_certificate_id = certificate_id(
        domain,
        &amendment.instance_id,
        &amendment.proposal_id,
        amendment.quorum,
        &cobalt_votes,
    )?;
    if amendment.certificate_id != expected_certificate_id {
        return Err("governance amendment certificate mismatch".to_string());
    }
    let expected_amendment_id = governance_amendment_id(
        domain,
        &amendment.instance_id,
        &amendment.certificate_id,
        &amendment.kind,
        amendment.value,
        &amendment.support,
    )?;
    if amendment.amendment_id != expected_amendment_id {
        return Err("governance amendment id mismatch".to_string());
    }

    Ok(())
}

pub fn verify_governance_amendment_for_mode(
    domain: &CobaltDomain,
    amendment: &GovernanceAmendment,
    mode: CobaltGovernanceMode,
) -> Result<(), String> {
    match mode {
        CobaltGovernanceMode::Canonical => verify_governance_amendment(domain, amendment),
        CobaltGovernanceMode::NonUniform => Err(
            "canonical governance amendment evidence is not valid in non-uniform Cobalt mode"
                .to_string(),
        ),
    }
}

fn sorted_unique(values: &[String]) -> Vec<String> {
    values
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn safety_witness_intersections(
    old_cover: &[CobaltSafetyWitnessSubsetRow],
    new_cover: &[CobaltSafetyWitnessSubsetRow],
    byzantine_budget: usize,
) -> Vec<CobaltSafetyWitnessIntersectionRow> {
    let mut rows = Vec::with_capacity(old_cover.len().saturating_mul(new_cover.len()));
    for old_subset in old_cover {
        for new_subset in new_cover {
            let intersection = sorted_intersection(&old_subset.validators, &new_subset.validators);
            let intersection_size = intersection.len();
            rows.push(CobaltSafetyWitnessIntersectionRow {
                old_subset_id: old_subset.subset_id.clone(),
                new_subset_id: new_subset.subset_id.clone(),
                intersection,
                intersection_size,
                byzantine_budget,
                safe: intersection_size > byzantine_budget,
            });
        }
    }
    rows
}

fn sorted_intersection(left: &[String], right: &[String]) -> Vec<String> {
    let left: BTreeSet<&str> = left.iter().map(String::as_str).collect();
    right.iter()
        .filter(|validator| left.contains(validator.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn build_cobalt_safety_witness_report(
    input: CobaltSafetyWitnessInput,
    old_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    new_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    intersections: Vec<CobaltSafetyWitnessIntersectionRow>,
    rejected_counterexamples: Vec<CobaltSafetyWitnessIntersectionRow>,
    accepted: bool,
    reason: String,
) -> Result<CobaltSafetyWitnessReport, String> {
    let mut report = CobaltSafetyWitnessReport {
        schema: COBALT_SAFETY_WITNESS_SCHEMA.to_string(),
        accepted,
        reason,
        previous_registry_root: input.previous_registry_root,
        new_registry_root: input.new_registry_root,
        previous_trust_graph_root: input.previous_trust_graph_root,
        new_trust_graph_root: input.new_trust_graph_root,
        activation_height: input.activation_height,
        challenge_state: input.challenge_state,
        byzantine_budget: input.profile.byzantine_budget,
        max_cover_subsets: input.profile.max_cover_subsets,
        old_cover,
        new_cover,
        intersections,
        rejected_counterexamples,
        report_hash: String::new(),
    };
    report.report_hash = cobalt_safety_witness_report_hash(&report)?;
    Ok(report)
}

fn validate_amendment_kind(kind: &str) -> Result<(), String> {
    if let Some(payload_hash) = kind.strip_prefix(FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1) {
        if payload_hash.len() == 96
            && payload_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Ok(());
        }
        return Err("FastPay recovery governance kind must bind one lowercase 48-byte payload hash".to_string());
    }
    if let Some(payload_hash) = kind.strip_prefix(FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1) {
        if payload_hash.len() == 96
            && payload_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Ok(());
        }
        return Err("FastSwap bootstrap governance kind must bind one lowercase 48-byte payload hash".to_string());
    }
    if let Some(route) = kind.strip_prefix(&format!(
        "{GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1}:"
    )) {
        let Some((asset_id, manifest_hash)) = route.split_once(':') else {
            return Err(
                "vault bridge route governance kind must bind an asset and manifest hash"
                    .to_string(),
            );
        };
        let canonical_hash = |value: &str| {
            value.len() == 96
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        };
        if canonical_hash(asset_id) && canonical_hash(manifest_hash) {
            return Ok(());
        }
        return Err(
            "vault bridge route governance kind must bind lowercase 48-byte asset and manifest hashes"
                .to_string(),
        );
    }
    match kind {
        GOVERNANCE_KIND_VALIDATOR_SET
        | GOVERNANCE_KIND_CRYPTO_POLICY
        | GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH
        | GOVERNANCE_KIND_AUTHORITY_MODE
        | GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT
        | GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT
        | GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT
        | GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT
        | GOVERNANCE_KIND_ORCHARD_POOL_PAUSE
        | GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE => Ok(()),
        other => Err(format!("unsupported governance amendment kind `{other}`")),
    }
}

fn validate_amendment_value(kind: &str, value: u32) -> Result<(), String> {
    if kind.starts_with(FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1) {
        return if value == FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1 {
            Ok(())
        } else {
            Err("FastPay recovery amendment value must equal schema version 1".to_string())
        };
    }
    if kind.starts_with(FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1) {
        return if value == FASTSWAP_SCHEMA_VERSION_V1 {
            Ok(())
        } else {
            Err("FastSwap bootstrap amendment value must equal schema version 1".to_string())
        };
    }
    if kind == GOVERNANCE_KIND_AUTHORITY_MODE {
        return match value {
            GOVERNANCE_AUTHORITY_MODE_FOUNDATION | GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED => {
                Ok(())
            }
            _ => Err("authority_mode amendment value must be 0 or 1".to_string()),
        };
    }
    if matches!(
        kind,
        GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE | GOVERNANCE_KIND_ORCHARD_POOL_PAUSE
    ) {
        return match value {
            0 | 1 => Ok(()),
            _ => Err(format!(
                "{kind} amendment value must be 0 or 1"
            )),
        };
    }
    if value == 0 {
        return Err(format!(
            "governance amendment `{kind}` value must be nonzero"
        ));
    }
    Ok(())
}

fn validate_amendment_lifecycle(lifecycle: GovernanceAmendmentLifecycle) -> Result<(), String> {
    if lifecycle.paused && lifecycle.activation_height == 0 && lifecycle.veto_until_height == 0 {
        return Err(
            "paused governance amendment must declare activation or veto metadata".to_string(),
        );
    }
    Ok(())
}

fn validate_validator_registry_update_request(
    domain: &CobaltDomain,
    request: &ValidatorRegistryUpdateRequest,
) -> Result<(), String> {
    if request.activation_height == 0 {
        return Err("validator registry update activation height must be nonzero".to_string());
    }
    if !is_lower_hex_len(&request.previous_registry_root, 96)
        || !is_lower_hex_len(&request.new_registry_root, 96)
    {
        return Err(
            "validator registry update roots must be 96 lowercase hex characters".to_string(),
        );
    }
    if request.previous_registry_root == request.new_registry_root {
        return Err("validator registry update roots must change".to_string());
    }
    validate_validator_registry_update_trust_graph_binding(domain, request)?;
    if request.subject_node_id.trim().is_empty() {
        return Err("validator registry update subject must be nonempty".to_string());
    }
    let has_root_scopes =
        !request.previous_validators.is_empty() || !request.new_validators.is_empty();
    if has_root_scopes {
        if request.previous_validators.is_empty() || request.new_validators.is_empty() {
            return Err(
                "validator registry update requires previous and new validator scopes".to_string(),
            );
        }
        validate_validator_scope("previous", &request.previous_validators)?;
        validate_validator_scope("new", &request.new_validators)?;
    }
    if let Some(record) = &request.previous_record {
        validate_validator_registry_entry(record)?;
        if record.node_id != request.subject_node_id {
            return Err("validator registry previous record subject mismatch".to_string());
        }
    }
    if let Some(record) = &request.new_record {
        validate_validator_registry_entry(record)?;
        if record.node_id != request.subject_node_id {
            return Err("validator registry new record subject mismatch".to_string());
        }
    }

    match request.operation.as_str() {
        VALIDATOR_REGISTRY_OP_ADMIT => {
            if request.previous_record.is_some() || request.new_record.is_none() {
                return Err("validator registry admit requires only a new record".to_string());
            }
            if request
                .new_record
                .as_ref()
                .is_some_and(|record| !record.active)
            {
                return Err("validator registry admit new record must be active".to_string());
            }
            if has_root_scopes {
                require_subject_absent(&request.previous_validators, &request.subject_node_id)?;
                require_subject_present(&request.new_validators, &request.subject_node_id)?;
            }
        }
        VALIDATOR_REGISTRY_OP_REMOVE => {
            if request.previous_record.is_none() || request.new_record.is_some() {
                return Err("validator registry remove requires only a previous record".to_string());
            }
            if has_root_scopes {
                require_subject_present(&request.previous_validators, &request.subject_node_id)?;
                require_subject_absent(&request.new_validators, &request.subject_node_id)?;
            }
        }
        VALIDATOR_REGISTRY_OP_SUSPEND => {
            let previous = request.previous_record.as_ref().ok_or_else(|| {
                "validator registry suspend requires a previous record".to_string()
            })?;
            let new = request
                .new_record
                .as_ref()
                .ok_or_else(|| "validator registry suspend requires a new record".to_string())?;
            if !previous.active || new.active {
                return Err("validator registry suspend must move active to inactive".to_string());
            }
            if previous.algorithm_id != new.algorithm_id
                || previous.public_key_hex != new.public_key_hex
            {
                return Err("validator registry suspend cannot rotate key material".to_string());
            }
            if has_root_scopes {
                require_subject_present(&request.previous_validators, &request.subject_node_id)?;
                require_subject_absent(&request.new_validators, &request.subject_node_id)?;
            }
        }
        VALIDATOR_REGISTRY_OP_REACTIVATE => {
            let previous = request.previous_record.as_ref().ok_or_else(|| {
                "validator registry reactivation requires a previous record".to_string()
            })?;
            let new = request.new_record.as_ref().ok_or_else(|| {
                "validator registry reactivation requires a new record".to_string()
            })?;
            if previous.active || !new.active {
                return Err(
                    "validator registry reactivation must move inactive to active".to_string(),
                );
            }
            if previous.algorithm_id != new.algorithm_id
                || previous.public_key_hex != new.public_key_hex
            {
                return Err(
                    "validator registry reactivation cannot rotate key material".to_string()
                );
            }
            if has_root_scopes {
                require_subject_absent(&request.previous_validators, &request.subject_node_id)?;
                require_subject_present(&request.new_validators, &request.subject_node_id)?;
            }
        }
        VALIDATOR_REGISTRY_OP_ROTATE_KEY => {
            let previous = request.previous_record.as_ref().ok_or_else(|| {
                "validator registry key rotation requires a previous record".to_string()
            })?;
            let new = request.new_record.as_ref().ok_or_else(|| {
                "validator registry key rotation requires a new record".to_string()
            })?;
            if previous.active != new.active {
                return Err(
                    "validator registry key rotation cannot change active status".to_string(),
                );
            }
            if previous.public_key_hex == new.public_key_hex {
                return Err("validator registry key rotation must change public key".to_string());
            }
            if has_root_scopes {
                if request.previous_validators != request.new_validators {
                    return Err(
                        "validator registry key rotation cannot change validator scope".to_string(),
                    );
                }
                if previous.active {
                    require_subject_present(
                        &request.previous_validators,
                        &request.subject_node_id,
                    )?;
                } else {
                    require_subject_absent(&request.previous_validators, &request.subject_node_id)?;
                }
            }
        }
        other => {
            return Err(format!(
                "unsupported validator registry update operation `{other}`"
            ))
        }
    }

    Ok(())
}

fn validate_validator_registry_update_trust_graph_binding(
    domain: &CobaltDomain,
    request: &ValidatorRegistryUpdateRequest,
) -> Result<(), String> {
    match (
        request.previous_trust_graph_root.as_deref(),
        request.new_trust_graph_root.as_deref(),
        request.trust_graph_transition_id.as_deref(),
    ) {
        (None, None, None) => Ok(()),
        (Some(previous_trust_graph_root), Some(new_trust_graph_root), Some(transition_id)) => {
            validate_hash_hex(
                "validator registry update previous trust graph root",
                previous_trust_graph_root,
            )?;
            validate_hash_hex(
                "validator registry update new trust graph root",
                new_trust_graph_root,
            )?;
            if previous_trust_graph_root == new_trust_graph_root {
                return Err(
                    "validator registry update trust graph roots must change".to_string(),
                );
            }
            let transition = TrustGraphTransition {
                previous_registry_root: request.previous_registry_root.clone(),
                new_registry_root: request.new_registry_root.clone(),
                previous_trust_graph_root: previous_trust_graph_root.to_string(),
                new_trust_graph_root: new_trust_graph_root.to_string(),
                activation_height: request.activation_height,
                transition_id: transition_id.to_string(),
            };
            validate_trust_graph_transition(domain, &transition)?;
            Ok(())
        }
        _ => Err(
            "validator registry update trust graph binding must include old root, new root, and transition id"
                .to_string(),
        ),
    }
}

fn validate_validator_scope(label: &str, validators: &[String]) -> Result<(), String> {
    if validators
        .iter()
        .any(|validator| validator.trim().is_empty())
    {
        return Err(format!(
            "validator registry update {label} validators must be nonempty"
        ));
    }
    if sorted_unique(validators) != validators {
        return Err(format!(
            "validator registry update {label} validators must be sorted unique"
        ));
    }
    Ok(())
}

fn validate_support_scope(support: &[String]) -> Result<(), String> {
    if support.iter().any(|validator| validator.trim().is_empty()) {
        return Err("support validators must be nonempty".to_string());
    }
    if sorted_unique(support) != support {
        return Err("support validators must be sorted unique".to_string());
    }
    Ok(())
}

fn require_subject_present(validators: &[String], subject: &str) -> Result<(), String> {
    if validators.iter().any(|validator| validator == subject) {
        Ok(())
    } else {
        Err("validator registry update subject missing from required validator scope".to_string())
    }
}

fn require_subject_absent(validators: &[String], subject: &str) -> Result<(), String> {
    if validators.iter().any(|validator| validator == subject) {
        Err("validator registry update subject unexpectedly present in validator scope".to_string())
    } else {
        Ok(())
    }
}

fn validate_validator_registry_entry(record: &ValidatorRegistryEntry) -> Result<(), String> {
    if record.node_id.trim().is_empty() {
        return Err("validator registry record node id is empty".to_string());
    }
    if record.algorithm_id.trim().is_empty() {
        return Err("validator registry record algorithm id is empty".to_string());
    }
    if record.public_key_hex.is_empty()
        || !record.public_key_hex.len().is_multiple_of(2)
        || !is_lower_hex(&record.public_key_hex)
    {
        return Err("validator registry record public key must be lowercase hex".to_string());
    }
    Ok(())
}

fn validate_domain(domain: &CobaltDomain) -> Result<(), String> {
    if domain.chain_id.trim().is_empty() {
        return Err("Cobalt domain chain_id is empty".to_string());
    }
    if domain.genesis_hash.trim().is_empty() {
        return Err("Cobalt domain genesis_hash is empty".to_string());
    }
    if !is_lower_hex_len(&domain.genesis_hash, 96) {
        return Err("Cobalt domain genesis_hash must be 96 lowercase hex characters".to_string());
    }
    if domain.protocol_version == 0 {
        return Err("Cobalt domain protocol_version must be nonzero".to_string());
    }
    Ok(())
}

fn validate_node_id(label: &str, node_id: &str) -> Result<(), String> {
    if node_id.trim().is_empty() {
        Err(format!("{label} must be nonempty"))
    } else {
        Ok(())
    }
}

fn validate_hash_hex(label: &str, value: &str) -> Result<(), String> {
    if is_lower_hex_len(value, 96) {
        Ok(())
    } else {
        Err(format!("{label} must be 96 lowercase hex characters"))
    }
}

fn validate_lower_hex_len(label: &str, value: &str, len: usize) -> Result<(), String> {
    if is_lower_hex_len(value, len) {
        Ok(())
    } else {
        Err(format!("{label} must be {len} lowercase hex characters"))
    }
}

fn hex_low_bit(value: &str) -> Result<bool, String> {
    let byte = value
        .as_bytes()
        .last()
        .copied()
        .ok_or_else(|| "hex value must be nonempty".to_string())?;
    let nibble = match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => 10 + byte - b'a',
        _ => return Err("hex value must be lowercase hex".to_string()),
    };
    Ok(nibble & 1 == 1)
}

fn validate_root(label: &str, value: &str) -> Result<(), String> {
    validate_hash_hex(label, value)
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn validate_config(config: &EssentialSubsetConfig) -> Result<(), String> {
    if config.validators.is_empty() {
        return Err("Cobalt validator set must be nonempty".to_string());
    }
    if config
        .validators
        .iter()
        .any(|validator| validator.trim().is_empty())
    {
        return Err("Cobalt validator ids must be nonempty".to_string());
    }
    if sorted_unique(&config.validators) != config.validators {
        return Err("Cobalt validator set must be sorted unique".to_string());
    }
    if config.quorum == 0 || config.quorum > config.validators.len() {
        return Err("Cobalt quorum is invalid".to_string());
    }
    Ok(())
}

fn shared_subset_by_id<'a>(
    left: &'a TrustView,
    right: &'a TrustView,
    subset_id: &str,
) -> Option<(&'a EssentialSubset, &'a EssentialSubset)> {
    let left_subset = left
        .essential_subsets
        .iter()
        .find(|subset| subset.subset_id == subset_id)?;
    let right_subset = right
        .essential_subsets
        .iter()
        .find(|subset| subset.subset_id == subset_id)?;
    Some((left_subset, right_subset))
}

fn linked_shared_subset<'a>(
    left: &'a TrustView,
    right: &'a TrustView,
    actively_byzantine: &BTreeSet<&str>,
) -> Option<&'a EssentialSubset> {
    for left_subset in &left.essential_subsets {
        if let Some((shared, _)) = shared_subset_by_id(left, right, &left_subset.subset_id) {
            let active_faults = shared
                .validators
                .iter()
                .filter(|validator| actively_byzantine.contains(validator.as_str()))
                .count();
            if active_faults <= shared.max_active_byzantine {
                return Some(shared);
            }
        }
    }
    None
}

fn fully_linked_shared_subset<'a>(
    left: &'a TrustView,
    right: &'a TrustView,
    actively_byzantine: &BTreeSet<&str>,
) -> Option<&'a EssentialSubset> {
    for left_subset in &left.essential_subsets {
        if let Some((shared, _)) = shared_subset_by_id(left, right, &left_subset.subset_id) {
            let active_faults = shared
                .validators
                .iter()
                .filter(|validator| actively_byzantine.contains(validator.as_str()))
                .count();
            let correct_nodes = shared.validator_count.saturating_sub(active_faults);
            if active_faults <= shared.max_active_byzantine
                && correct_nodes >= shared.quorum
                && shared.max_active_byzantine
                    <= shared.validator_count.saturating_sub(shared.quorum)
            {
                return Some(shared);
            }
        }
    }
    None
}

fn extended_unl_for_view(
    view: &TrustView,
    view_by_validator: &BTreeMap<&str, &TrustView>,
) -> Result<Vec<String>, String> {
    let mut closure: BTreeSet<String> = BTreeSet::from([view.validator.clone()]);
    let mut frontier: Vec<String> = view.derived_unl.clone();
    while let Some(validator) = frontier.pop() {
        if !closure.insert(validator.clone()) {
            continue;
        }
        if let Some(next_view) = view_by_validator.get(validator.as_str()) {
            for next_validator in &next_view.derived_unl {
                if !closure.contains(next_validator) {
                    frontier.push(next_validator.clone());
                }
            }
        } else {
            return Err("derived UNL references validator without trust view".to_string());
        }
    }
    Ok(closure.into_iter().collect())
}

fn closure_is_fully_linked(
    extended_unl: &[String],
    view_by_validator: &BTreeMap<&str, &TrustView>,
    actively_byzantine: &BTreeSet<&str>,
) -> bool {
    for left_index in 0..extended_unl.len() {
        for right_index in (left_index + 1)..extended_unl.len() {
            let Some(left) = view_by_validator.get(extended_unl[left_index].as_str()) else {
                return false;
            };
            let Some(right) = view_by_validator.get(extended_unl[right_index].as_str()) else {
                return false;
            };
            if fully_linked_shared_subset(left, right, actively_byzantine).is_none() {
                return false;
            }
        }
    }
    true
}

struct LinkageReportHashInput<'a> {
    domain: &'a CobaltDomain,
    graph: &'a TrustGraph,
    actively_byzantine: &'a [String],
    linked_pairs: &'a [ValidatorPair],
    fully_linked_pairs: &'a [ValidatorPair],
    unsafe_pairs: &'a [UnsafePairReport],
    weakly_connected_validators: &'a [String],
    strongly_connected_validators: &'a [String],
    connectivity: &'a [ConnectivityReport],
}

fn linkage_report_hash(input: LinkageReportHashInput<'_>) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        input.domain.chain_id.as_str(),
        input.domain.genesis_hash.as_str(),
        input.domain.protocol_version,
        input.graph.trust_graph_root.as_str(),
        input.graph.registry_root.as_str(),
        input.actively_byzantine,
        input.linked_pairs,
        input.fully_linked_pairs,
        input.unsafe_pairs,
        input.weakly_connected_validators,
        input.strongly_connected_validators,
        input.connectivity,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.linkage_report.v1", &encoded))
}
