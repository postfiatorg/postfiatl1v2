use postfiat_consensus_cobalt::{
    build_canonical_unl_trust_graph, cobalt_cover_extraction_report_hash, emit_example_report,
    extract_cobalt_safety_cover, verify_cobalt_cover_extraction_report, CobaltDomain,
    CobaltSafetyWitnessProfile, TrustGraph,
};
use postfiat_crypto_provider::hash_hex;
use serde_json::json;

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn ids(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

fn domain() -> CobaltDomain {
    CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    }
}

fn profile(max_cover_subsets: usize) -> CobaltSafetyWitnessProfile {
    CobaltSafetyWitnessProfile {
        byzantine_budget: 2,
        max_cover_subsets,
        require_cleared_challenge_state: true,
    }
}

fn transition(
    domain: &CobaltDomain,
    old_validators: Vec<String>,
    new_validators: Vec<String>,
) -> Result<(TrustGraph, TrustGraph), String> {
    let old_graph =
        build_canonical_unl_trust_graph(domain, 1, root('a'), 10, None, old_validators, 5)?;
    let new_graph = build_canonical_unl_trust_graph(
        domain,
        2,
        root('b'),
        11,
        Some(old_graph.trust_graph_root.clone()),
        new_validators,
        5,
    )?;
    Ok((old_graph, new_graph))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = domain();
    let profile = profile(16);
    let (old_graph, new_graph) = transition(
        &domain,
        ids(&["A", "B", "C", "D", "E", "F", "G"]),
        ids(&["A", "B", "C", "D", "E", "F", "H"]),
    )?;

    let cover_report = extract_cobalt_safety_cover(&domain, &old_graph, &new_graph, &profile)?;
    verify_cobalt_cover_extraction_report(
        &domain,
        &old_graph,
        &new_graph,
        &profile,
        &cover_report,
    )?;
    let report_hash_check = cobalt_cover_extraction_report_hash(&cover_report)?;
    let evidence_hash = hash_hex(
        "postfiat.cobalt.cover_extractor_evidence.v1",
        &serde_json::to_vec(&cover_report)?,
    );

    let report = json!({
        "schema": "postfiat-cobalt-cover-extractor-evidence-v1",
        "status": if cover_report.accepted { "passed" } else { "failed" },
        "checker": "extract_cobalt_safety_cover",
        "scope": "local-consensus-crate",
        "profile": {
            "byzantine_budget": profile.byzantine_budget,
            "max_cover_subsets": profile.max_cover_subsets,
            "require_cleared_challenge_state": profile.require_cleared_challenge_state
        },
        "claims": {
            "cover_is_derived_from_graphs": true,
            "proposer_supplied_cover_pruning": false,
            "old_cover_subsets": cover_report.old_cover.len(),
            "new_cover_subsets": cover_report.new_cover.len(),
            "total_cover_subsets": cover_report.total_cover_subsets,
            "rejected_subsets": cover_report.rejected_subsets.len()
        },
        "transition": {
            "previous_registry_root": cover_report.previous_registry_root.as_str(),
            "new_registry_root": cover_report.new_registry_root.as_str(),
            "previous_trust_graph_root": cover_report.previous_trust_graph_root.as_str(),
            "new_trust_graph_root": cover_report.new_trust_graph_root.as_str(),
            "activation_height": cover_report.activation_height
        },
        "report_hash": cover_report.report_hash.as_str(),
        "report_hash_check": report_hash_check,
        "evidence_hash": evidence_hash,
        "report": cover_report
    });
    emit_example_report(&report)?;
    Ok(())
}
