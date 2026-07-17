use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_canonical_unl_trust_graph, build_essential_subset,
    build_essential_subset_update_transition, build_trust_graph, build_trust_view,
    build_trust_view_update_transition, validate_trust_graph_lifecycle_record, CobaltDomain,
    CobaltFaultModel, EssentialSubset, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct PoisonScenario {
    name: &'static str,
    attack: &'static str,
    expected_rejection: &'static str,
    observed_error: String,
    ok: bool,
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn subset(domain: &CobaltDomain, members: &[&str], t_s: usize, q_s: usize) -> EssentialSubset {
    build_essential_subset(
        domain,
        members
            .iter()
            .map(|validator| (*validator).to_string())
            .collect(),
        t_s,
        q_s,
        Vec::new(),
        1,
        None,
    )
    .expect("build essential subset")
}

fn fixture() -> Result<(CobaltDomain, TrustGraph), String> {
    let domain = CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    };
    let all = subset(
        &domain,
        &[
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ],
        2,
        5,
    );
    let first_five = subset(
        &domain,
        &[
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
        ],
        1,
        4,
    );
    let last_five = subset(
        &domain,
        &[
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ],
        1,
        4,
    );
    let views = vec![
        build_trust_view(&domain, "validator-0", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-1", 1, vec![all.clone(), first_five], "")?,
        build_trust_view(&domain, "validator-2", 1, vec![all.clone(), last_five], "")?,
        build_trust_view(&domain, "validator-3", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-4", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-5", 1, vec![all.clone()], "")?,
        build_trust_view(&domain, "validator-6", 1, vec![all], "")?,
    ];
    let graph = build_trust_graph(&domain, 2, root('b'), 7, None, views)?;
    let linkage = analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default())?;
    if !linkage.unsafe_pairs.is_empty() {
        return Err("fixture graph is unsafe".to_string());
    }
    Ok((domain, graph))
}

fn scenario(
    name: &'static str,
    attack: &'static str,
    expected_rejection: &'static str,
    result: Result<(), String>,
) -> PoisonScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    PoisonScenario {
        name,
        attack,
        expected_rejection,
        ok: observed_error.contains(expected_rejection),
        observed_error,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let previous_view = graph
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-1")
        .ok_or("missing validator-1 view")?;

    let unsafe_view = build_trust_view(
        &domain,
        "validator-1",
        previous_view.view_version + 1,
        vec![subset(&domain, &["validator-1"], 0, 1)],
        "",
    )?;
    let unsafe_update = build_essential_subset_update_transition(
        &domain,
        &graph,
        unsafe_view,
        40,
        &CobaltFaultModel::default(),
    )
    .map(|_| ());

    let invalid_subset = build_essential_subset(
        &domain,
        vec![
            "validator-0".to_string(),
            "validator-1".to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
            "validator-4".to_string(),
        ],
        2,
        4,
        Vec::new(),
        1,
        None,
    )
    .map(|_| ());

    let duplicate_validators = build_canonical_unl_trust_graph(
        &domain,
        3,
        root('c'),
        8,
        Some(graph.trust_graph_root.clone()),
        vec![
            "validator-0".to_string(),
            "validator-0".to_string(),
            "validator-1".to_string(),
        ],
        2,
    )
    .map(|_| ());

    let missing_validator_view = build_trust_view(
        &domain,
        "validator-1",
        previous_view.view_version + 2,
        vec![subset(&domain, &["validator-1", "validator-404"], 0, 2)],
        "",
    )?;
    let missing_validator_update = build_essential_subset_update_transition(
        &domain,
        &graph,
        missing_validator_view,
        41,
        &CobaltFaultModel::default(),
    )
    .map(|_| ());

    let stale_view = build_trust_view(
        &domain,
        "validator-1",
        previous_view.view_version,
        previous_view.essential_subsets.clone(),
        "",
    )?;
    let stale_update = build_trust_view_update_transition(
        &domain,
        &graph,
        stale_view,
        42,
        &CobaltFaultModel::default(),
    )
    .map(|_| ());

    let malformed_signature = build_trust_view(
        &domain,
        "validator-1",
        previous_view.view_version + 3,
        previous_view.essential_subsets.clone(),
        "AB",
    )
    .map(|_| ());

    let safe_view = build_trust_view(
        &domain,
        "validator-1",
        previous_view.view_version + 4,
        previous_view.essential_subsets.clone(),
        "",
    )?;
    let (safe_graph, mut safe_record) = build_trust_view_update_transition(
        &domain,
        &graph,
        safe_view,
        43,
        &CobaltFaultModel::default(),
    )?;
    let safe_linkage = analyze_trust_graph(&domain, &safe_graph, &CobaltFaultModel::default())?;
    safe_record.activation_height = 44;
    let tampered_record = validate_trust_graph_lifecycle_record(
        &domain,
        &graph,
        &safe_graph,
        &safe_linkage,
        &safe_record,
    );

    let scenarios = vec![
        scenario(
            "unsafe_linkage_update_rejected_before_activation",
            "replace a validator trust view with a hostile single-validator essential subset",
            "unsafe before activation",
            unsafe_update,
        ),
        scenario(
            "invalid_subset_parameters_rejected",
            "build an essential subset with t_S and q_S that violate Cobalt inequalities",
            "violates 2t_S < q_S",
            invalid_subset,
        ),
        scenario(
            "duplicate_validator_scope_rejected",
            "build a canonical trust graph with duplicate validator ids",
            "sorted unique",
            duplicate_validators,
        ),
        scenario(
            "missing_validator_reference_rejected",
            "activate a trust view whose derived UNL references a validator with no trust view",
            "derived UNL references validator without trust view",
            missing_validator_update,
        ),
        scenario(
            "stale_view_version_rejected",
            "submit a trust-view update whose owner view version did not increase",
            "view version must increase",
            stale_update,
        ),
        scenario(
            "malformed_owner_signature_rejected",
            "submit a trust view with noncanonical owner signature encoding",
            "signature must be lowercase hex",
            malformed_signature,
        ),
        scenario(
            "tampered_lifecycle_record_rejected",
            "mutate a lifecycle record after a safe transition was built",
            "transition mismatch",
            tampered_record,
        ),
    ];
    let scenario_failures = scenarios
        .iter()
        .filter(|scenario| !scenario.ok)
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    let ok = scenario_failures.is_empty();
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-trust-graph-poison-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_poison_scenarios_rejected": ok,
            "unsafe_graph_fails_before_activation": scenarios.iter().any(|scenario| scenario.name == "unsafe_linkage_update_rejected_before_activation" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt trust graph poison report failed".into())
    }
}
