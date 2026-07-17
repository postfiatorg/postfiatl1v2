use postfiat_consensus_cobalt::{
    build_essential_subset, build_trust_graph, build_trust_view, emit_example_report,
    extract_cobalt_safety_cover, CobaltDomain, CobaltSafetyWitnessProfile, EssentialSubset,
    TrustGraph,
};
use postfiat_crypto_provider::hash_hex;
use serde_json::json;

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn domain() -> CobaltDomain {
    CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    }
}

fn validators(count: usize, offset: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{:03}", index + offset))
        .collect()
}

fn profile() -> CobaltSafetyWitnessProfile {
    CobaltSafetyWitnessProfile {
        byzantine_budget: 2,
        max_cover_subsets: 64,
        require_cleared_challenge_state: true,
    }
}

fn grouped_graph(
    domain: &CobaltDomain,
    graph_version: u64,
    registry_root: String,
    activation_height: u64,
    previous_root: Option<String>,
    validators: Vec<String>,
    group_size: usize,
) -> Result<TrustGraph, String> {
    let quorum = (validators.len() * 2 / 3) + 1;
    let global = build_essential_subset(
        domain,
        validators.clone(),
        validators.len() - quorum,
        quorum,
        Vec::new(),
        activation_height,
        None,
    )?;
    let mut groups = Vec::<EssentialSubset>::new();
    for group in validators.chunks(group_size) {
        let group = group.to_vec();
        let group_quorum = (group.len() * 2 / 3) + 1;
        groups.push(build_essential_subset(
            domain,
            group,
            group_size.saturating_sub(group_quorum),
            group_quorum,
            Vec::new(),
            activation_height,
            None,
        )?);
    }
    let mut views = Vec::with_capacity(validators.len());
    for validator in &validators {
        let group = groups
            .iter()
            .find(|subset| subset.validators.iter().any(|member| member == validator))
            .ok_or_else(|| format!("missing group subset for {validator}"))?;
        views.push(build_trust_view(
            domain,
            validator.as_str(),
            graph_version,
            vec![global.clone(), group.clone()],
            "",
        )?);
    }
    build_trust_graph(
        domain,
        graph_version,
        registry_root,
        activation_height,
        previous_root,
        views,
    )
}

fn sizing_case(
    domain: &CobaltDomain,
    name: &str,
    validator_count: usize,
    group_size: usize,
) -> Result<serde_json::Value, String> {
    let old_validators = validators(validator_count, 0);
    let mut new_validators = old_validators.clone();
    if let Some(last) = new_validators.last_mut() {
        *last = format!("validator-{:03}", validator_count + 100);
    }
    let old_graph = grouped_graph(domain, 1, root('a'), 10, None, old_validators, group_size)?;
    let new_graph = grouped_graph(
        domain,
        2,
        root('b'),
        11,
        Some(old_graph.trust_graph_root.clone()),
        new_validators,
        group_size,
    )?;
    let profile = profile();
    let report = extract_cobalt_safety_cover(domain, &old_graph, &new_graph, &profile)?;
    Ok(json!({
        "name": name,
        "validator_count": validator_count,
        "group_size": group_size,
        "max_cover_subsets": profile.max_cover_subsets,
        "accepted": report.accepted,
        "reason": report.reason,
        "old_cover": report.old_cover.len(),
        "new_cover": report.new_cover.len(),
        "total_cover_subsets": report.total_cover_subsets,
        "cover_fraction_of_limit": (report.total_cover_subsets as f64) / (profile.max_cover_subsets as f64),
        "report_hash": report.report_hash
    }))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = domain();
    let cases = vec![
        sizing_case(&domain, "35_validators_5_groups", 35, 7)?,
        sizing_case(&domain, "100_validators_10_groups", 100, 10)?,
    ];
    let ok = cases
        .iter()
        .all(|case| case["accepted"].as_bool().unwrap_or(false));
    let cases_hash = hash_hex(
        "postfiat.cobalt.cover_sizing.cases.v1",
        &serde_json::to_vec(&cases)?,
    );
    let report = json!({
        "schema": "postfiat-cobalt-cover-sizing-v1",
        "status": if ok { "passed" } else { "failed" },
        "checker": "extract_cobalt_safety_cover",
        "profile": {
            "byzantine_budget": profile().byzantine_budget,
            "max_cover_subsets": profile().max_cover_subsets,
            "require_cleared_challenge_state": profile().require_cleared_challenge_state
        },
        "cases_hash": cases_hash,
        "cases": cases
    });
    emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt cover sizing failed".into())
    }
}
