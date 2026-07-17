pub fn build_essential_subset(
    domain: &CobaltDomain,
    validators: Vec<String>,
    max_active_byzantine: usize,
    quorum: usize,
    operator_labels: Vec<String>,
    activation_height: u64,
    deactivation_height: Option<u64>,
) -> Result<EssentialSubset, String> {
    validate_domain(domain)?;
    let validators = sorted_unique(&validators);
    let operator_labels = sorted_unique(&operator_labels);
    let subset_id = essential_subset_id(domain, &validators, max_active_byzantine, quorum)?;
    let subset = EssentialSubset {
        subset_id,
        validator_count: validators.len(),
        validators,
        max_active_byzantine,
        quorum,
        operator_labels,
        activation_height,
        deactivation_height,
    };
    validate_essential_subset(domain, &subset)?;
    Ok(subset)
}

pub fn build_trust_view(
    domain: &CobaltDomain,
    validator: impl Into<String>,
    view_version: u64,
    essential_subsets: Vec<EssentialSubset>,
    signature_hex: impl Into<String>,
) -> Result<TrustView, String> {
    validate_domain(domain)?;
    let validator = validator.into();
    validate_node_id("trust view validator", &validator)?;
    if view_version == 0 {
        return Err("trust view version must be nonzero".to_string());
    }
    let mut essential_subsets = essential_subsets;
    essential_subsets.sort_by(|left, right| left.subset_id.cmp(&right.subset_id));
    let derived_unl = derive_trust_view_unl(&essential_subsets)?;
    let trust_view_id = trust_view_id(
        domain,
        &validator,
        view_version,
        &essential_subsets,
        &derived_unl,
    )?;
    let view = TrustView {
        trust_view_id,
        validator,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        view_version,
        essential_subsets,
        derived_unl,
        signature_hex: signature_hex.into(),
    };
    validate_trust_view(domain, &view)?;
    Ok(view)
}

pub fn build_trust_graph(
    domain: &CobaltDomain,
    graph_version: u64,
    registry_root: impl Into<String>,
    activation_height: u64,
    previous_trust_graph_root: Option<String>,
    trust_views: Vec<TrustView>,
) -> Result<TrustGraph, String> {
    validate_domain(domain)?;
    if graph_version == 0 {
        return Err("trust graph version must be nonzero".to_string());
    }
    let registry_root = registry_root.into();
    validate_root("trust graph registry root", &registry_root)?;
    let previous_trust_graph_root = match previous_trust_graph_root {
        Some(root) if root.is_empty() => None,
        Some(root) => {
            validate_hash_hex("previous trust graph root", &root)?;
            Some(root)
        }
        None => None,
    };
    let mut trust_views = trust_views;
    trust_views.sort_by(|left, right| left.validator.cmp(&right.validator));
    let trust_graph_root = trust_graph_root(
        domain,
        graph_version,
        &registry_root,
        activation_height,
        previous_trust_graph_root.as_deref(),
        &trust_views,
    )?;
    let graph = TrustGraph {
        trust_graph_root,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        graph_version,
        registry_root,
        activation_height,
        previous_trust_graph_root,
        trust_views,
    };
    validate_trust_graph(domain, &graph)?;
    Ok(graph)
}

pub fn build_canonical_unl_trust_graph(
    domain: &CobaltDomain,
    graph_version: u64,
    registry_root: impl Into<String>,
    activation_height: u64,
    previous_trust_graph_root: Option<String>,
    validators: Vec<String>,
    quorum: usize,
) -> Result<TrustGraph, String> {
    validate_domain(domain)?;
    validate_validator_scope("canonical UNL trust graph", &validators)?;
    if quorum == 0 {
        return Err("canonical UNL quorum must be nonzero".to_string());
    }
    if quorum > validators.len() {
        return Err("canonical UNL quorum exceeds validator count".to_string());
    }
    let validators = sorted_unique(&validators);
    let max_active_byzantine = validators
        .len()
        .checked_sub(quorum)
        .ok_or_else(|| "canonical UNL quorum exceeds validator count".to_string())?;
    let all_validators_subset = build_essential_subset(
        domain,
        validators.clone(),
        max_active_byzantine,
        quorum,
        Vec::new(),
        activation_height,
        None,
    )?;
    let mut trust_views = Vec::with_capacity(validators.len());
    for validator in &validators {
        trust_views.push(build_trust_view(
            domain,
            validator,
            graph_version,
            vec![all_validators_subset.clone()],
            "",
        )?);
    }
    build_trust_graph(
        domain,
        graph_version,
        registry_root,
        activation_height,
        previous_trust_graph_root,
        trust_views,
    )
}

pub fn essential_subset_id(
    domain: &CobaltDomain,
    validators: &[String],
    max_active_byzantine: usize,
    quorum: usize,
) -> Result<String, String> {
    validate_domain(domain)?;
    let validators = sorted_unique(validators);
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        validators.as_slice(),
        max_active_byzantine,
        quorum,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.essential_subset.v1", &encoded))
}

pub fn trust_view_id(
    domain: &CobaltDomain,
    validator: &str,
    view_version: u64,
    essential_subsets: &[EssentialSubset],
    derived_unl: &[String],
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        validator,
        view_version,
        essential_subsets,
        derived_unl,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.trust_view.v1", &encoded))
}

pub fn trust_graph_root(
    domain: &CobaltDomain,
    graph_version: u64,
    registry_root: &str,
    activation_height: u64,
    previous_trust_graph_root: Option<&str>,
    trust_views: &[TrustView],
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        graph_version,
        registry_root,
        activation_height,
        previous_trust_graph_root,
        trust_views,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.cobalt.trust_graph.v1", &encoded))
}

pub fn trust_graph_transition_id(
    domain: &CobaltDomain,
    transition: &TrustGraphTransition,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        transition.previous_registry_root.as_str(),
        transition.new_registry_root.as_str(),
        transition.previous_trust_graph_root.as_str(),
        transition.new_trust_graph_root.as_str(),
        transition.activation_height,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.trust_graph_transition.v1",
        &encoded,
    ))
}

pub fn build_trust_graph_transition(
    domain: &CobaltDomain,
    previous_registry_root: impl Into<String>,
    new_registry_root: impl Into<String>,
    previous_trust_graph_root: impl Into<String>,
    new_trust_graph_root: impl Into<String>,
    activation_height: u64,
) -> Result<TrustGraphTransition, String> {
    validate_domain(domain)?;
    let mut transition = TrustGraphTransition {
        previous_registry_root: previous_registry_root.into(),
        new_registry_root: new_registry_root.into(),
        previous_trust_graph_root: previous_trust_graph_root.into(),
        new_trust_graph_root: new_trust_graph_root.into(),
        activation_height,
        transition_id: String::new(),
    };
    transition.transition_id = trust_graph_transition_id(domain, &transition)?;
    validate_trust_graph_transition(domain, &transition)?;
    Ok(transition)
}

pub fn validate_trust_graph_transition(
    domain: &CobaltDomain,
    transition: &TrustGraphTransition,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_root(
        "trust graph transition previous registry root",
        &transition.previous_registry_root,
    )?;
    validate_root(
        "trust graph transition new registry root",
        &transition.new_registry_root,
    )?;
    validate_hash_hex(
        "trust graph transition previous trust graph root",
        &transition.previous_trust_graph_root,
    )?;
    validate_hash_hex(
        "trust graph transition new trust graph root",
        &transition.new_trust_graph_root,
    )?;
    if transition.activation_height == 0 {
        return Err("trust graph transition activation height must be nonzero".to_string());
    }
    if transition.previous_trust_graph_root == transition.new_trust_graph_root {
        return Err("trust graph transition trust graph roots must change".to_string());
    }
    if transition.previous_registry_root == transition.new_registry_root
        && transition.previous_trust_graph_root == transition.new_trust_graph_root
    {
        return Err("trust graph transition must change registry or trust graph root".to_string());
    }
    let expected_id = trust_graph_transition_id(domain, transition)?;
    if transition.transition_id != expected_id {
        return Err("trust graph transition id mismatch".to_string());
    }
    Ok(())
}

pub const COBALT_SAFETY_WITNESS_SCHEMA: &str = "postfiat-cobalt-safety-witness-v1";
pub const COBALT_CHALLENGE_STATE_CLEARED: &str = "cleared";

pub fn verify_cobalt_safety_witness(
    domain: &CobaltDomain,
    previous_graph: &TrustGraph,
    new_graph: &TrustGraph,
    input: CobaltSafetyWitnessInput,
) -> Result<CobaltSafetyWitnessReport, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, previous_graph)?;
    validate_trust_graph(domain, new_graph)?;
    validate_root(
        "Cobalt safety witness previous registry root",
        &input.previous_registry_root,
    )?;
    validate_root(
        "Cobalt safety witness new registry root",
        &input.new_registry_root,
    )?;
    validate_hash_hex(
        "Cobalt safety witness previous trust graph root",
        &input.previous_trust_graph_root,
    )?;
    validate_hash_hex(
        "Cobalt safety witness new trust graph root",
        &input.new_trust_graph_root,
    )?;

    let cover_report =
        extract_cobalt_safety_cover(domain, previous_graph, new_graph, &input.profile)?;
    let old_cover = cover_report.old_cover.clone();
    let new_cover = cover_report.new_cover.clone();

    let preflight_rejection = if input.previous_registry_root != previous_graph.registry_root {
        Some("previous registry root mismatch")
    } else if input.new_registry_root != new_graph.registry_root {
        Some("new registry root mismatch")
    } else if input.previous_trust_graph_root != previous_graph.trust_graph_root {
        Some("previous graph root mismatch")
    } else if input.new_trust_graph_root != new_graph.trust_graph_root {
        Some("new graph root mismatch")
    } else if new_graph.previous_trust_graph_root.as_deref()
        != Some(previous_graph.trust_graph_root.as_str())
    {
        Some("new graph parent root mismatch")
    } else if input.activation_height != new_graph.activation_height {
        Some("activation height mismatch")
    } else if input.activation_height <= previous_graph.activation_height {
        Some("activation height must increase")
    } else if !cover_report.accepted {
        Some(cover_report.reason.as_str())
    } else if input.profile.require_cleared_challenge_state
        && input.challenge_state != COBALT_CHALLENGE_STATE_CLEARED
    {
        Some("challenge state not cleared")
    } else {
        None
    };

    let (intersections, rejected_counterexamples, reason) = if let Some(reason) = preflight_rejection
    {
        (Vec::new(), Vec::new(), reason)
    } else {
        let intersections =
            safety_witness_intersections(&old_cover, &new_cover, input.profile.byzantine_budget);
        let rejected_counterexamples: Vec<CobaltSafetyWitnessIntersectionRow> = intersections
            .iter()
            .filter(|row| !row.safe)
            .cloned()
            .collect();
        let reason = if rejected_counterexamples.is_empty() {
            "accepted"
        } else {
            "old-new intersection bound failed"
        };
        (intersections, rejected_counterexamples, reason)
    };

    let reason = if !rejected_counterexamples.is_empty() {
        "old-new intersection bound failed"
    } else {
        reason
    };
    let accepted = reason == "accepted";

    build_cobalt_safety_witness_report(
        input,
        old_cover,
        new_cover,
        intersections,
        rejected_counterexamples,
        accepted,
        reason.to_string(),
    )
}

pub fn cobalt_safety_witness_report_hash(
    report: &CobaltSafetyWitnessReport,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        report.schema.as_str(),
        report.accepted,
        report.reason.as_str(),
        report.previous_registry_root.as_str(),
        report.new_registry_root.as_str(),
        report.previous_trust_graph_root.as_str(),
        report.new_trust_graph_root.as_str(),
        report.activation_height,
        report.challenge_state.as_str(),
        report.byzantine_budget,
        report.max_cover_subsets,
        report.old_cover.as_slice(),
        report.new_cover.as_slice(),
        report.intersections.as_slice(),
        report.rejected_counterexamples.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.safety_witness_report.v1",
        &encoded,
    ))
}

pub fn build_trust_view_update_transition(
    domain: &CobaltDomain,
    current_graph: &TrustGraph,
    new_view: TrustView,
    activation_height: u64,
    fault_model: &CobaltFaultModel,
) -> Result<(TrustGraph, TrustGraphLifecycleRecord), String> {
    build_trust_graph_lifecycle_transition(
        domain,
        current_graph,
        new_view,
        activation_height,
        fault_model,
        TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE,
    )
}

pub fn build_essential_subset_update_transition(
    domain: &CobaltDomain,
    current_graph: &TrustGraph,
    new_view: TrustView,
    activation_height: u64,
    fault_model: &CobaltFaultModel,
) -> Result<(TrustGraph, TrustGraphLifecycleRecord), String> {
    build_trust_graph_lifecycle_transition(
        domain,
        current_graph,
        new_view,
        activation_height,
        fault_model,
        TRUST_GRAPH_LIFECYCLE_OP_ESSENTIAL_SUBSET_UPDATE,
    )
}

pub fn validate_trust_graph_lifecycle_record(
    domain: &CobaltDomain,
    previous_graph: &TrustGraph,
    new_graph: &TrustGraph,
    linkage_report: &LinkageReport,
    record: &TrustGraphLifecycleRecord,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, previous_graph)?;
    validate_trust_graph(domain, new_graph)?;
    if record.chain_id != domain.chain_id
        || record.genesis_hash != domain.genesis_hash
        || record.protocol_version != domain.protocol_version
    {
        return Err("trust graph lifecycle record domain mismatch".to_string());
    }
    if record.previous_registry_root != previous_graph.registry_root
        || record.new_registry_root != new_graph.registry_root
        || record.previous_trust_graph_root != previous_graph.trust_graph_root
        || record.new_trust_graph_root != new_graph.trust_graph_root
        || record.activation_height != new_graph.activation_height
    {
        return Err("trust graph lifecycle record transition mismatch".to_string());
    }
    let transition = TrustGraphTransition {
        previous_registry_root: record.previous_registry_root.clone(),
        new_registry_root: record.new_registry_root.clone(),
        previous_trust_graph_root: record.previous_trust_graph_root.clone(),
        new_trust_graph_root: record.new_trust_graph_root.clone(),
        activation_height: record.activation_height,
        transition_id: record.trust_graph_transition_id.clone(),
    };
    validate_trust_graph_transition(domain, &transition)?;
    if linkage_report.trust_graph_root != new_graph.trust_graph_root
        || linkage_report.registry_root != new_graph.registry_root
    {
        return Err("trust graph lifecycle linkage report root mismatch".to_string());
    }
    if !linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph lifecycle record has unsafe linkage report".to_string());
    }
    let expected_report = analyze_trust_graph(
        domain,
        new_graph,
        &CobaltFaultModel {
            actively_byzantine: linkage_report.actively_byzantine.clone(),
        },
    )?;
    if linkage_report != &expected_report {
        return Err("trust graph lifecycle linkage report does not match graph".to_string());
    }
    if record.linkage_report_hash != linkage_report.report_hash {
        return Err("trust graph lifecycle linkage report hash mismatch".to_string());
    }
    let previous_view = trust_view_for_validator(previous_graph, &record.subject_validator)?;
    let new_view = trust_view_for_validator(new_graph, &record.subject_validator)?;
    if record.previous_trust_view_id != previous_view.trust_view_id
        || record.new_trust_view_id != new_view.trust_view_id
    {
        return Err("trust graph lifecycle trust view id mismatch".to_string());
    }
    if record.previous_subset_ids != trust_view_subset_ids(previous_view)
        || record.new_subset_ids != trust_view_subset_ids(new_view)
    {
        return Err("trust graph lifecycle subset ids mismatch".to_string());
    }
    let expected_record_id = trust_graph_lifecycle_record_id(domain, record)?;
    if record.record_id != expected_record_id {
        return Err("trust graph lifecycle record id mismatch".to_string());
    }
    Ok(())
}

pub fn build_trust_graph_rollback_transition(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
    failed_graph: &TrustGraph,
    rollback_activation_height: u64,
    bad_linkage_report: &LinkageReport,
) -> Result<(TrustGraph, LinkageReport, TrustGraphRollbackRecord), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, authority_graph)?;
    validate_trust_graph(domain, failed_graph)?;
    if failed_graph.registry_root != authority_graph.registry_root {
        return Err("trust graph rollback registry root mismatch".to_string());
    }
    if failed_graph.previous_trust_graph_root.as_deref()
        != Some(authority_graph.trust_graph_root.as_str())
    {
        return Err(
            "trust graph rollback failed graph is not descended from authority graph".to_string(),
        );
    }
    if rollback_activation_height <= failed_graph.activation_height {
        return Err(
            "trust graph rollback activation height must be after failed graph".to_string(),
        );
    }
    let bad_fault_model = CobaltFaultModel {
        actively_byzantine: bad_linkage_report.actively_byzantine.clone(),
    };
    let expected_bad_report = analyze_trust_graph(domain, failed_graph, &bad_fault_model)?;
    if bad_linkage_report != &expected_bad_report {
        return Err("trust graph rollback bad linkage report does not match graph".to_string());
    }
    if bad_linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph rollback requires unsafe linkage evidence".to_string());
    }

    let rollback_graph = build_trust_graph(
        domain,
        failed_graph
            .graph_version
            .checked_add(1)
            .ok_or_else(|| "trust graph rollback graph version overflow".to_string())?,
        authority_graph.registry_root.clone(),
        rollback_activation_height,
        Some(failed_graph.trust_graph_root.clone()),
        authority_graph.trust_views.clone(),
    )?;
    let rollback_linkage_report = analyze_trust_graph(domain, &rollback_graph, &bad_fault_model)?;
    if !rollback_linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph rollback target is unsafe".to_string());
    }
    let transition = build_trust_graph_transition(
        domain,
        failed_graph.registry_root.clone(),
        rollback_graph.registry_root.clone(),
        failed_graph.trust_graph_root.clone(),
        rollback_graph.trust_graph_root.clone(),
        rollback_activation_height,
    )?;
    let mut record = TrustGraphRollbackRecord {
        record_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        authority_trust_graph_root: authority_graph.trust_graph_root.clone(),
        failed_trust_graph_root: failed_graph.trust_graph_root.clone(),
        rollback_trust_graph_root: rollback_graph.trust_graph_root.clone(),
        registry_root: authority_graph.registry_root.clone(),
        failed_activation_height: failed_graph.activation_height,
        rollback_activation_height,
        bad_linkage_report_hash: bad_linkage_report.report_hash.clone(),
        rollback_linkage_report_hash: rollback_linkage_report.report_hash.clone(),
        trust_graph_transition_id: transition.transition_id,
        reason: TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE.to_string(),
    };
    record.record_id = trust_graph_rollback_record_id(domain, &record)?;
    validate_trust_graph_rollback_record(
        domain,
        authority_graph,
        failed_graph,
        &rollback_graph,
        bad_linkage_report,
        &rollback_linkage_report,
        &record,
    )?;
    Ok((rollback_graph, rollback_linkage_report, record))
}

pub fn validate_trust_graph_rollback_record(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
    failed_graph: &TrustGraph,
    rollback_graph: &TrustGraph,
    bad_linkage_report: &LinkageReport,
    rollback_linkage_report: &LinkageReport,
    record: &TrustGraphRollbackRecord,
) -> Result<(), String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, authority_graph)?;
    validate_trust_graph(domain, failed_graph)?;
    validate_trust_graph(domain, rollback_graph)?;
    if record.chain_id != domain.chain_id
        || record.genesis_hash != domain.genesis_hash
        || record.protocol_version != domain.protocol_version
    {
        return Err("trust graph rollback record domain mismatch".to_string());
    }
    if failed_graph.registry_root != authority_graph.registry_root
        || rollback_graph.registry_root != authority_graph.registry_root
        || record.registry_root != authority_graph.registry_root
    {
        return Err("trust graph rollback registry root mismatch".to_string());
    }
    if record.authority_trust_graph_root != authority_graph.trust_graph_root
        || record.failed_trust_graph_root != failed_graph.trust_graph_root
        || record.rollback_trust_graph_root != rollback_graph.trust_graph_root
        || record.failed_activation_height != failed_graph.activation_height
        || record.rollback_activation_height != rollback_graph.activation_height
    {
        return Err("trust graph rollback record graph mismatch".to_string());
    }
    if failed_graph.previous_trust_graph_root.as_deref()
        != Some(authority_graph.trust_graph_root.as_str())
        || rollback_graph.previous_trust_graph_root.as_deref()
            != Some(failed_graph.trust_graph_root.as_str())
    {
        return Err("trust graph rollback ancestry mismatch".to_string());
    }
    if rollback_graph.activation_height <= failed_graph.activation_height {
        return Err(
            "trust graph rollback activation height must be after failed graph".to_string(),
        );
    }
    if rollback_graph.trust_views != authority_graph.trust_views {
        return Err(
            "trust graph rollback target does not restore authority trust views".to_string(),
        );
    }
    let bad_fault_model = CobaltFaultModel {
        actively_byzantine: bad_linkage_report.actively_byzantine.clone(),
    };
    let expected_bad_report = analyze_trust_graph(domain, failed_graph, &bad_fault_model)?;
    if bad_linkage_report != &expected_bad_report {
        return Err("trust graph rollback bad linkage report does not match graph".to_string());
    }
    if bad_linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph rollback requires unsafe linkage evidence".to_string());
    }
    let rollback_fault_model = CobaltFaultModel {
        actively_byzantine: rollback_linkage_report.actively_byzantine.clone(),
    };
    let expected_rollback_report =
        analyze_trust_graph(domain, rollback_graph, &rollback_fault_model)?;
    if rollback_linkage_report != &expected_rollback_report {
        return Err("trust graph rollback linkage report does not match graph".to_string());
    }
    if !rollback_linkage_report.unsafe_pairs.is_empty() {
        return Err("trust graph rollback target is unsafe".to_string());
    }
    if record.bad_linkage_report_hash != bad_linkage_report.report_hash
        || record.rollback_linkage_report_hash != rollback_linkage_report.report_hash
    {
        return Err("trust graph rollback linkage report hash mismatch".to_string());
    }
    let transition = TrustGraphTransition {
        previous_registry_root: failed_graph.registry_root.clone(),
        new_registry_root: rollback_graph.registry_root.clone(),
        previous_trust_graph_root: failed_graph.trust_graph_root.clone(),
        new_trust_graph_root: rollback_graph.trust_graph_root.clone(),
        activation_height: rollback_graph.activation_height,
        transition_id: record.trust_graph_transition_id.clone(),
    };
    validate_trust_graph_transition(domain, &transition)?;
    if record.reason != TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE {
        return Err("trust graph rollback reason mismatch".to_string());
    }
    let expected_record_id = trust_graph_rollback_record_id(domain, record)?;
    if record.record_id != expected_record_id {
        return Err("trust graph rollback record id mismatch".to_string());
    }
    Ok(())
}

pub fn derive_trust_view_unl(essential_subsets: &[EssentialSubset]) -> Result<Vec<String>, String> {
    if essential_subsets.is_empty() {
        return Err("trust view requires at least one essential subset".to_string());
    }
    let mut validators = BTreeSet::new();
    for subset in essential_subsets {
        for validator in &subset.validators {
            validators.insert(validator.clone());
        }
    }
    Ok(validators.into_iter().collect())
}

pub fn validate_essential_subset(
    domain: &CobaltDomain,
    subset: &EssentialSubset,
) -> Result<(), String> {
    validate_domain(domain)?;
    if subset.validators.is_empty() {
        return Err("essential subset validators must be nonempty".to_string());
    }
    validate_validator_scope("essential subset", &subset.validators)?;
    if subset.validator_count != subset.validators.len() {
        return Err("essential subset validator_count mismatch".to_string());
    }
    if subset.quorum > subset.validator_count {
        return Err("essential subset quorum exceeds validator_count".to_string());
    }
    if subset.max_active_byzantine > subset.validator_count {
        return Err("essential subset t_S exceeds validator_count".to_string());
    }
    if subset.max_active_byzantine
        >= subset
            .quorum
            .saturating_mul(2)
            .saturating_sub(subset.validator_count)
    {
        return Err("essential subset violates t_S < 2q_S - n_S".to_string());
    }
    if subset.max_active_byzantine.saturating_mul(2) >= subset.quorum {
        return Err("essential subset violates 2t_S < q_S".to_string());
    }
    if sorted_unique(&subset.operator_labels) != subset.operator_labels {
        return Err("essential subset operator labels must be sorted unique".to_string());
    }
    if subset
        .operator_labels
        .iter()
        .any(|label| label.trim().is_empty())
    {
        return Err("essential subset operator labels must be nonempty".to_string());
    }
    if let Some(deactivation_height) = subset.deactivation_height {
        if deactivation_height <= subset.activation_height {
            return Err(
                "essential subset deactivation height must be after activation".to_string(),
            );
        }
    }
    let expected_id = essential_subset_id(
        domain,
        &subset.validators,
        subset.max_active_byzantine,
        subset.quorum,
    )?;
    if subset.subset_id != expected_id {
        return Err("essential subset id mismatch".to_string());
    }
    Ok(())
}

pub fn validate_trust_view(domain: &CobaltDomain, view: &TrustView) -> Result<(), String> {
    validate_domain(domain)?;
    validate_node_id("trust view validator", &view.validator)?;
    if view.chain_id != domain.chain_id
        || view.genesis_hash != domain.genesis_hash
        || view.protocol_version != domain.protocol_version
    {
        return Err("trust view domain mismatch".to_string());
    }
    if view.view_version == 0 {
        return Err("trust view version must be nonzero".to_string());
    }
    if view.essential_subsets.is_empty() {
        return Err("trust view requires at least one essential subset".to_string());
    }
    let subset_ids: Vec<String> = view
        .essential_subsets
        .iter()
        .map(|subset| subset.subset_id.clone())
        .collect();
    if sorted_unique(&subset_ids) != subset_ids {
        return Err("trust view essential subsets must be sorted unique".to_string());
    }
    for subset in &view.essential_subsets {
        validate_essential_subset(domain, subset)?;
    }
    let expected_unl = derive_trust_view_unl(&view.essential_subsets)?;
    if view.derived_unl != expected_unl {
        return Err("trust view derived UNL mismatch".to_string());
    }
    if !view
        .derived_unl
        .iter()
        .any(|validator| validator == &view.validator)
    {
        return Err("trust view owner must be in derived UNL".to_string());
    }
    if !view.signature_hex.is_empty()
        && (!view.signature_hex.len().is_multiple_of(2) || !is_lower_hex(&view.signature_hex))
    {
        return Err("trust view signature must be lowercase hex".to_string());
    }
    let expected_id = trust_view_id(
        domain,
        &view.validator,
        view.view_version,
        &view.essential_subsets,
        &view.derived_unl,
    )?;
    if view.trust_view_id != expected_id {
        return Err("trust view id mismatch".to_string());
    }
    Ok(())
}

pub fn validate_trust_graph(domain: &CobaltDomain, graph: &TrustGraph) -> Result<(), String> {
    validate_domain(domain)?;
    if graph.chain_id != domain.chain_id
        || graph.genesis_hash != domain.genesis_hash
        || graph.protocol_version != domain.protocol_version
    {
        return Err("trust graph domain mismatch".to_string());
    }
    if graph.graph_version == 0 {
        return Err("trust graph version must be nonzero".to_string());
    }
    validate_root("trust graph registry root", &graph.registry_root)?;
    if let Some(root) = &graph.previous_trust_graph_root {
        validate_hash_hex("previous trust graph root", root)?;
    }
    if graph.trust_views.is_empty() {
        return Err("trust graph requires at least one trust view".to_string());
    }
    let validators: Vec<String> = graph
        .trust_views
        .iter()
        .map(|view| view.validator.clone())
        .collect();
    if sorted_unique(&validators) != validators {
        return Err("trust graph trust views must be sorted by unique validator".to_string());
    }
    for view in &graph.trust_views {
        validate_trust_view(domain, view)?;
    }
    let expected_root = trust_graph_root(
        domain,
        graph.graph_version,
        &graph.registry_root,
        graph.activation_height,
        graph.previous_trust_graph_root.as_deref(),
        &graph.trust_views,
    )?;
    if graph.trust_graph_root != expected_root {
        return Err("trust graph root mismatch".to_string());
    }
    Ok(())
}

pub fn has_strong_support(view: &TrustView, support: &[String]) -> Result<bool, String> {
    validate_support_scope(support)?;
    let support: BTreeSet<&str> = support.iter().map(String::as_str).collect();
    Ok(view.essential_subsets.iter().all(|subset| {
        subset
            .validators
            .iter()
            .filter(|validator| support.contains(validator.as_str()))
            .count()
            >= subset.quorum
    }))
}

pub fn has_weak_support(view: &TrustView, support: &[String]) -> Result<bool, String> {
    validate_support_scope(support)?;
    let support: BTreeSet<&str> = support.iter().map(String::as_str).collect();
    Ok(view.essential_subsets.iter().any(|subset| {
        subset
            .validators
            .iter()
            .filter(|validator| support.contains(validator.as_str()))
            .count()
            > subset.max_active_byzantine
    }))
}

pub fn analyze_trust_graph(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    fault_model: &CobaltFaultModel,
) -> Result<LinkageReport, String> {
    validate_trust_graph(domain, graph)?;
    let actively_byzantine = sorted_unique(&fault_model.actively_byzantine);
    validate_support_scope(&actively_byzantine)?;
    let known_validators: BTreeSet<&str> = graph
        .trust_views
        .iter()
        .map(|view| view.validator.as_str())
        .collect();
    if actively_byzantine
        .iter()
        .any(|validator| !known_validators.contains(validator.as_str()))
    {
        return Err("fault model includes unknown validator".to_string());
    }
    let fault_set: BTreeSet<&str> = actively_byzantine.iter().map(String::as_str).collect();
    let view_by_validator: BTreeMap<&str, &TrustView> = graph
        .trust_views
        .iter()
        .map(|view| (view.validator.as_str(), view))
        .collect();

    let mut linked_pairs = Vec::new();
    let mut fully_linked_pairs = Vec::new();
    let mut unsafe_pairs = Vec::new();
    for left_index in 0..graph.trust_views.len() {
        for right_index in (left_index + 1)..graph.trust_views.len() {
            let left = &graph.trust_views[left_index];
            let right = &graph.trust_views[right_index];
            let linked = linked_shared_subset(left, right, &fault_set).is_some();
            let fully_linked = fully_linked_shared_subset(left, right, &fault_set).is_some();
            if linked {
                linked_pairs.push(ValidatorPair {
                    left: left.validator.clone(),
                    right: right.validator.clone(),
                });
            }
            if fully_linked {
                fully_linked_pairs.push(ValidatorPair {
                    left: left.validator.clone(),
                    right: right.validator.clone(),
                });
            }
            if !linked || !fully_linked {
                let reason = match (linked, fully_linked) {
                    (false, false) => "no shared essential subset satisfies linkage".to_string(),
                    (true, false) => "linked but not fully linked for liveness".to_string(),
                    (false, true) => "internal linkage inconsistency".to_string(),
                    (true, true) => "safe pair".to_string(),
                };
                unsafe_pairs.push(UnsafePairReport {
                    left: left.validator.clone(),
                    right: right.validator.clone(),
                    reason,
                });
            }
        }
    }

    let mut connectivity = Vec::new();
    let mut weakly_connected_validators = Vec::new();
    let mut strongly_connected_validators = Vec::new();
    for view in &graph.trust_views {
        let extended_unl = extended_unl_for_view(view, &view_by_validator)?;
        let fully_linked_with: Vec<String> = extended_unl
            .iter()
            .filter(|validator| *validator != &view.validator)
            .filter_map(|validator| {
                view_by_validator
                    .get(validator.as_str())
                    .filter(|other| fully_linked_shared_subset(view, other, &fault_set).is_some())
                    .map(|_| validator.clone())
            })
            .collect();
        let weakly_connected_in_known_graph = extended_unl
            .iter()
            .filter(|validator| *validator != &view.validator)
            .all(|validator| {
                view_by_validator
                    .get(validator.as_str())
                    .is_some_and(|other| {
                        fully_linked_shared_subset(view, other, &fault_set).is_some()
                    })
            });
        let strongly_connected_known_closure =
            closure_is_fully_linked(&extended_unl, &view_by_validator, &fault_set);
        if weakly_connected_in_known_graph {
            weakly_connected_validators.push(view.validator.clone());
        }
        if strongly_connected_known_closure {
            strongly_connected_validators.push(view.validator.clone());
        }
        connectivity.push(ConnectivityReport {
            validator: view.validator.clone(),
            derived_unl: view.derived_unl.clone(),
            extended_unl,
            fully_linked_with,
            weakly_connected_in_known_graph,
            strongly_connected_known_closure,
        });
    }

    let report_hash = linkage_report_hash(LinkageReportHashInput {
        domain,
        graph,
        actively_byzantine: &actively_byzantine,
        linked_pairs: &linked_pairs,
        fully_linked_pairs: &fully_linked_pairs,
        unsafe_pairs: &unsafe_pairs,
        weakly_connected_validators: &weakly_connected_validators,
        strongly_connected_validators: &strongly_connected_validators,
        connectivity: &connectivity,
    })?;
    Ok(LinkageReport {
        trust_graph_root: graph.trust_graph_root.clone(),
        registry_root: graph.registry_root.clone(),
        trust_view_count: graph.trust_views.len(),
        actively_byzantine,
        linked_pairs,
        fully_linked_pairs,
        unsafe_pairs,
        weakly_connected_validators,
        strongly_connected_validators,
        connectivity,
        report_hash,
    })
}

pub fn ratify_validator_set_amendment(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    new_validator_count: u32,
    support: Vec<String>,
) -> Result<GovernanceAmendment, String> {
    ratify_validator_set_amendment_with_lifecycle(
        domain,
        config,
        new_validator_count,
        support,
        GovernanceAmendmentLifecycle::immediate(),
    )
}

pub fn ratify_validator_set_amendment_with_lifecycle(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    new_validator_count: u32,
    support: Vec<String>,
    lifecycle: GovernanceAmendmentLifecycle,
) -> Result<GovernanceAmendment, String> {
    ratify_governance_amendment_with_lifecycle(
        domain,
        config,
        GOVERNANCE_KIND_VALIDATOR_SET,
        new_validator_count,
        support,
        lifecycle,
    )
}

pub fn ratify_governance_amendment(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    kind: &str,
    value: u32,
    support: Vec<String>,
) -> Result<GovernanceAmendment, String> {
    ratify_governance_amendment_with_lifecycle(
        domain,
        config,
        kind,
        value,
        support,
        GovernanceAmendmentLifecycle::immediate(),
    )
}

pub fn ratify_governance_amendment_with_lifecycle(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    kind: &str,
    value: u32,
    support: Vec<String>,
    lifecycle: GovernanceAmendmentLifecycle,
) -> Result<GovernanceAmendment, String> {
    validate_amendment_kind(kind)?;
    validate_amendment_value(kind, value)?;
    validate_amendment_lifecycle(lifecycle)?;
    let (proposal, certificate, support) = certify_governance_amendment_with_lifecycle(
        domain, config, kind, value, support, lifecycle,
    )?;
    let amendment_id = governance_amendment_id(
        domain,
        &proposal.instance_id,
        &certificate.certificate_id,
        kind,
        value,
        &support,
    )?;
    Ok(GovernanceAmendment {
        amendment_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        instance_id: proposal.instance_id.clone(),
        proposal_id: proposal.proposal_id.clone(),
        certificate_id: certificate.certificate_id.clone(),
        proposer: proposal.proposer,
        validators: config.validators.clone(),
        quorum: config.quorum,
        kind: kind.to_string(),
        value,
        activation_height: lifecycle.activation_height,
        veto_until_height: lifecycle.veto_until_height,
        paused: lifecycle.paused,
        support,
        votes: certificate
            .votes
            .into_iter()
            .map(|vote| GovernanceVote {
                vote_id: vote.vote_id,
                validator: vote.validator,
                accept: vote.accept,
            })
            .collect(),
        signed_authorizations: Vec::new(),
    })
}

pub fn certify_validator_set(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    new_validator_count: u32,
    support: Vec<String>,
) -> Result<(CobaltProposal, CobaltCertificate, Vec<String>), String> {
    certify_governance_amendment(
        domain,
        config,
        GOVERNANCE_KIND_VALIDATOR_SET,
        new_validator_count,
        support,
    )
}

pub fn certify_governance_amendment(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    kind: &str,
    value: u32,
    support: Vec<String>,
) -> Result<(CobaltProposal, CobaltCertificate, Vec<String>), String> {
    certify_governance_amendment_with_lifecycle(
        domain,
        config,
        kind,
        value,
        support,
        GovernanceAmendmentLifecycle::immediate(),
    )
}

pub fn certify_governance_amendment_with_lifecycle(
    domain: &CobaltDomain,
    config: &EssentialSubsetConfig,
    kind: &str,
    value: u32,
    support: Vec<String>,
    lifecycle: GovernanceAmendmentLifecycle,
) -> Result<(CobaltProposal, CobaltCertificate, Vec<String>), String> {
    validate_domain(domain)?;
    validate_config(config)?;
    validate_amendment_kind(kind)?;
    validate_amendment_value(kind, value)?;
    validate_amendment_lifecycle(lifecycle)?;
    let allowed: BTreeSet<&str> = config.validators.iter().map(String::as_str).collect();
    let unique_support: BTreeSet<String> = support
        .into_iter()
        .filter(|validator| allowed.contains(validator.as_str()))
        .collect();

    if unique_support.len() < config.quorum {
        return Err(format!(
            "insufficient support: got {}, need {}",
            unique_support.len(),
            config.quorum
        ));
    }

    let support: Vec<String> = unique_support.into_iter().collect();
    let proposal = governance_proposal(domain, config, kind, value, lifecycle)?;
    let votes: Vec<CobaltVote> = support
        .iter()
        .map(|validator| CobaltVote {
            vote_id: vote_id(
                domain,
                &proposal.instance_id,
                &proposal.proposal_id,
                validator,
                true,
            ),
            instance_id: proposal.instance_id.clone(),
            proposal_id: proposal.proposal_id.clone(),
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
            validator: validator.clone(),
            accept: true,
        })
        .collect();
    let certificate = CobaltCertificate {
        certificate_id: certificate_id(
            domain,
            &proposal.instance_id,
            &proposal.proposal_id,
            config.quorum,
            &votes,
        )?,
        instance_id: proposal.instance_id.clone(),
        proposal_id: proposal.proposal_id.clone(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        quorum: config.quorum,
        votes,
    };
    Ok((proposal, certificate, support))
}

pub fn propose_nonuniform_governance_amendment(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    kind: &str,
    value: u32,
) -> Result<CobaltProposal, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, graph)?;
    validate_amendment_kind(kind)?;
    validate_amendment_value(kind, value)?;
    let proposer = graph
        .trust_views
        .first()
        .map(|view| view.validator.clone())
        .ok_or_else(|| "trust graph requires at least one trust view".to_string())?;
    let instance_id = nonuniform_governance_instance_id(domain, graph, kind, value);
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

pub fn certify_nonuniform_governance_amendment(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    linkage_report: &LinkageReport,
    local_validator: &str,
    proposal: &CobaltProposal,
    support: Vec<String>,
    current_height: u64,
) -> Result<NonUniformGovernanceCertificate, String> {
    validate_nonuniform_certificate_context(domain, graph, linkage_report, current_height)?;
    validate_nonuniform_proposal(domain, graph, proposal)?;
    let view = trust_view_for_validator(graph, local_validator)?;
    let support = sorted_unique(&support);
    validate_support_scope(&support)?;
    validate_support_in_view(view, &support)?;
    if !has_strong_support(view, &support)? {
        return Err(
            "non-uniform certificate support does not satisfy local trust view".to_string(),
        );
    }
    let satisfied_subsets = satisfied_subsets_for_view(view, &support)?;
    let votes = governance_votes_for_support(domain, proposal, &support);
    let mut certificate = NonUniformGovernanceCertificate {
        certificate_id: String::new(),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        trust_view_id: view.trust_view_id.clone(),
        local_validator: view.validator.clone(),
        instance_id: proposal.instance_id.clone(),
        proposal_id: proposal.proposal_id.clone(),
        support,
        satisfied_subsets,
        linkage_report_hash: linkage_report.report_hash.clone(),
        votes,
    };
    certificate.certificate_id = nonuniform_governance_certificate_id(domain, &certificate)?;
    verify_nonuniform_governance_certificate(
        domain,
        graph,
        linkage_report,
        proposal,
        &certificate,
        current_height,
    )?;
    Ok(certificate)
}

pub fn verify_nonuniform_governance_certificate(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    linkage_report: &LinkageReport,
    proposal: &CobaltProposal,
    certificate: &NonUniformGovernanceCertificate,
    current_height: u64,
) -> Result<(), String> {
    validate_nonuniform_certificate_context(domain, graph, linkage_report, current_height)?;
    validate_nonuniform_proposal(domain, graph, proposal)?;
    if certificate.chain_id != domain.chain_id
        || certificate.genesis_hash != domain.genesis_hash
        || certificate.protocol_version != domain.protocol_version
    {
        return Err("non-uniform certificate domain mismatch".to_string());
    }
    if certificate.registry_root != graph.registry_root {
        return Err("non-uniform certificate registry root mismatch".to_string());
    }
    if certificate.trust_graph_root != graph.trust_graph_root {
        return Err("non-uniform certificate trust graph root mismatch".to_string());
    }
    if certificate.linkage_report_hash != linkage_report.report_hash {
        return Err("non-uniform certificate linkage report hash mismatch".to_string());
    }
    if certificate.instance_id != proposal.instance_id
        || certificate.proposal_id != proposal.proposal_id
    {
        return Err("non-uniform certificate proposal mismatch".to_string());
    }
    let view = trust_view_for_validator(graph, &certificate.local_validator)?;
    if certificate.trust_view_id != view.trust_view_id {
        return Err("non-uniform certificate stale trust view id".to_string());
    }
    validate_support_scope(&certificate.support)?;
    validate_support_in_view(view, &certificate.support)?;
    if !has_strong_support(view, &certificate.support)? {
        return Err(
            "non-uniform certificate support does not satisfy local trust view".to_string(),
        );
    }
    let expected_satisfied_subsets = satisfied_subsets_for_view(view, &certificate.support)?;
    if certificate.satisfied_subsets != expected_satisfied_subsets {
        return Err("non-uniform certificate satisfied subsets mismatch".to_string());
    }
    validate_nonuniform_certificate_votes(domain, proposal, certificate)?;
    let expected_id = nonuniform_governance_certificate_id(domain, certificate)?;
    if certificate.certificate_id != expected_id {
        return Err("non-uniform certificate id mismatch".to_string());
    }
    Ok(())
}

pub fn nonuniform_governance_certificate_id(
    domain: &CobaltDomain,
    certificate: &NonUniformGovernanceCertificate,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        certificate.registry_root.as_str(),
        certificate.trust_graph_root.as_str(),
        certificate.trust_view_id.as_str(),
        certificate.local_validator.as_str(),
        certificate.instance_id.as_str(),
        certificate.proposal_id.as_str(),
        certificate.support.as_slice(),
        certificate.satisfied_subsets.as_slice(),
        certificate.linkage_report_hash.as_str(),
        certificate.votes.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.nonuniform_governance_certificate.v1",
        &encoded,
    ))
}
