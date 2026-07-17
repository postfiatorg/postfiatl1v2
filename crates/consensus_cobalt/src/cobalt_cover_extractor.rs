pub const COBALT_COVER_EXTRACTION_SCHEMA: &str = "postfiat-cobalt-cover-extraction-v1";

pub fn extract_cobalt_safety_cover(
    domain: &CobaltDomain,
    previous_graph: &TrustGraph,
    new_graph: &TrustGraph,
    profile: &CobaltSafetyWitnessProfile,
) -> Result<CobaltCoverExtractionReport, String> {
    validate_domain(domain)?;
    validate_trust_graph(domain, previous_graph)?;
    validate_trust_graph(domain, new_graph)?;

    let (old_cover, mut rejected_subsets) =
        extract_cobalt_cover_rows_for_graph(domain, previous_graph)?;
    let (new_cover, mut new_rejected_subsets) =
        extract_cobalt_cover_rows_for_graph(domain, new_graph)?;
    rejected_subsets.append(&mut new_rejected_subsets);

    let total_cover_subsets = old_cover.len().saturating_add(new_cover.len());
    let weakest_subset_t = old_cover
        .iter()
        .chain(new_cover.iter())
        .map(|row| row.max_active_byzantine)
        .min();
    let reason = if new_graph.previous_trust_graph_root.as_deref()
        != Some(previous_graph.trust_graph_root.as_str())
    {
        "new graph parent root mismatch"
    } else if new_graph.activation_height <= previous_graph.activation_height {
        "activation height must increase"
    } else if profile.max_cover_subsets == 0 {
        "essential subset cover limit must be nonzero"
    } else if old_cover.is_empty() || new_cover.is_empty() {
        "cover extraction requires at least one active subset per graph"
    } else if !rejected_subsets.is_empty() {
        "cover extraction found inactive or conflicting subset"
    } else if total_cover_subsets > profile.max_cover_subsets {
        "essential subset cover exceeds profile limit"
    } else if weakest_subset_t.is_some_and(|max_active_byzantine| {
        profile.byzantine_budget > max_active_byzantine
    }) {
        "byzantine budget exceeds weakest covered subset"
    } else {
        "cover extraction complete"
    };
    let accepted = reason == "cover extraction complete";

    let mut report = CobaltCoverExtractionReport {
        schema: COBALT_COVER_EXTRACTION_SCHEMA.to_string(),
        accepted,
        complete: accepted,
        reason: reason.to_string(),
        previous_registry_root: previous_graph.registry_root.clone(),
        new_registry_root: new_graph.registry_root.clone(),
        previous_trust_graph_root: previous_graph.trust_graph_root.clone(),
        new_trust_graph_root: new_graph.trust_graph_root.clone(),
        activation_height: new_graph.activation_height,
        byzantine_budget: profile.byzantine_budget,
        max_cover_subsets: profile.max_cover_subsets,
        old_cover,
        new_cover,
        total_cover_subsets,
        rejected_subsets,
        report_hash: String::new(),
    };
    report.report_hash = cobalt_cover_extraction_report_hash(&report)?;
    Ok(report)
}

pub fn verify_cobalt_cover_extraction_report(
    domain: &CobaltDomain,
    previous_graph: &TrustGraph,
    new_graph: &TrustGraph,
    profile: &CobaltSafetyWitnessProfile,
    report: &CobaltCoverExtractionReport,
) -> Result<(), String> {
    let expected = extract_cobalt_safety_cover(domain, previous_graph, new_graph, profile)?;
    if report != &expected {
        return Err("Cobalt cover extraction report mismatch".to_string());
    }
    Ok(())
}

pub fn verify_cobalt_cover_extraction_matches_safety_witness(
    cover_report: &CobaltCoverExtractionReport,
    safety_witness: &CobaltSafetyWitnessReport,
) -> Result<(), String> {
    if !cover_report.accepted || !cover_report.complete {
        return Err("Cobalt cover extraction report is not complete".to_string());
    }
    if cover_report.previous_registry_root != safety_witness.previous_registry_root
        || cover_report.new_registry_root != safety_witness.new_registry_root
        || cover_report.previous_trust_graph_root != safety_witness.previous_trust_graph_root
        || cover_report.new_trust_graph_root != safety_witness.new_trust_graph_root
        || cover_report.activation_height != safety_witness.activation_height
        || cover_report.byzantine_budget != safety_witness.byzantine_budget
        || cover_report.max_cover_subsets != safety_witness.max_cover_subsets
    {
        return Err("Cobalt cover extraction and safety witness transition mismatch".to_string());
    }
    if cover_report.old_cover != safety_witness.old_cover
        || cover_report.new_cover != safety_witness.new_cover
    {
        return Err("Cobalt safety witness does not match extracted complete cover".to_string());
    }
    Ok(())
}

pub fn cobalt_cover_extraction_report_hash(
    report: &CobaltCoverExtractionReport,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(&(
        report.schema.as_str(),
        report.accepted,
        report.complete,
        report.reason.as_str(),
        report.previous_registry_root.as_str(),
        report.new_registry_root.as_str(),
        report.previous_trust_graph_root.as_str(),
        report.new_trust_graph_root.as_str(),
        report.activation_height,
        report.byzantine_budget,
        report.max_cover_subsets,
        report.old_cover.as_slice(),
        report.new_cover.as_slice(),
        report.total_cover_subsets,
        report.rejected_subsets.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex(
        "postfiat.cobalt.cover_extraction_report.v1",
        &encoded,
    ))
}

fn extract_cobalt_cover_rows_for_graph(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<
    (
        Vec<CobaltSafetyWitnessSubsetRow>,
        Vec<CobaltCoverRejectedSubset>,
    ),
    String,
> {
    validate_trust_graph(domain, graph)?;
    let mut by_subset: BTreeMap<String, CobaltSafetyWitnessSubsetRow> = BTreeMap::new();
    let mut rejected = Vec::new();
    for view in &graph.trust_views {
        for subset in &view.essential_subsets {
            if let Some(reason) = cover_subset_rejection_reason(graph, subset) {
                rejected.push(CobaltCoverRejectedSubset {
                    graph_root: graph.trust_graph_root.clone(),
                    trust_view_validator: view.validator.clone(),
                    subset_id: subset.subset_id.clone(),
                    reason,
                });
                continue;
            }
            let row = CobaltSafetyWitnessSubsetRow {
                graph_root: graph.trust_graph_root.clone(),
                subset_id: subset.subset_id.clone(),
                validators: subset.validators.clone(),
                validator_count: subset.validator_count,
                max_active_byzantine: subset.max_active_byzantine,
                quorum: subset.quorum,
            };
            match by_subset.get(&row.subset_id) {
                Some(existing) if existing != &row => rejected.push(CobaltCoverRejectedSubset {
                    graph_root: graph.trust_graph_root.clone(),
                    trust_view_validator: view.validator.clone(),
                    subset_id: subset.subset_id.clone(),
                    reason: "subset id maps to conflicting cover row".to_string(),
                }),
                Some(_) => {}
                None => {
                    by_subset.insert(row.subset_id.clone(), row);
                }
            }
        }
    }
    Ok((by_subset.into_values().collect(), rejected))
}

fn cover_subset_rejection_reason(graph: &TrustGraph, subset: &EssentialSubset) -> Option<String> {
    if subset.activation_height > graph.activation_height {
        return Some("subset activation is after graph activation".to_string());
    }
    if subset
        .deactivation_height
        .is_some_and(|height| height <= graph.activation_height)
    {
        return Some("subset deactivated at or before graph activation".to_string());
    }
    None
}
