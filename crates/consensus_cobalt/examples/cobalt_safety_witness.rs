use postfiat_consensus_cobalt::{
    build_canonical_unl_trust_graph, emit_example_report, verify_cobalt_safety_witness,
    CobaltDomain, CobaltSafetyWitnessInput, CobaltSafetyWitnessProfile, TrustGraph,
    COBALT_CHALLENGE_STATE_CLEARED,
};
use postfiat_crypto_provider::hash_hex;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct WitnessScenario {
    name: &'static str,
    expected: &'static str,
    accepted: bool,
    reason: String,
    report_hash: String,
    rejected_counterexamples: usize,
    observed: serde_json::Value,
    ok: bool,
}

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

fn input(
    old_graph: &TrustGraph,
    new_graph: &TrustGraph,
    profile: CobaltSafetyWitnessProfile,
) -> CobaltSafetyWitnessInput {
    CobaltSafetyWitnessInput {
        previous_registry_root: old_graph.registry_root.clone(),
        new_registry_root: new_graph.registry_root.clone(),
        previous_trust_graph_root: old_graph.trust_graph_root.clone(),
        new_trust_graph_root: new_graph.trust_graph_root.clone(),
        activation_height: new_graph.activation_height,
        challenge_state: COBALT_CHALLENGE_STATE_CLEARED.to_string(),
        profile,
    }
}

fn scenario(
    name: &'static str,
    expected: &'static str,
    accepted: bool,
    reason_contains: &'static str,
    report: postfiat_consensus_cobalt::CobaltSafetyWitnessReport,
) -> WitnessScenario {
    let ok = report.accepted == accepted && report.reason.contains(reason_contains);
    WitnessScenario {
        name,
        expected,
        accepted: report.accepted,
        reason: report.reason.clone(),
        report_hash: report.report_hash.clone(),
        rejected_counterexamples: report.rejected_counterexamples.len(),
        observed: json!({
            "old_cover": report.old_cover.len(),
            "new_cover": report.new_cover.len(),
            "intersections": report.intersections.len(),
            "rejected_counterexamples": report.rejected_counterexamples,
        }),
        ok,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = domain();

    let (old_graph, one_rotation_graph) = transition(
        &domain,
        ids(&["A", "B", "C", "D", "E", "F", "G"]),
        ids(&["A", "B", "C", "D", "E", "F", "H"]),
    )?;
    let accepted_report = verify_cobalt_safety_witness(
        &domain,
        &old_graph,
        &one_rotation_graph,
        input(&old_graph, &one_rotation_graph, profile(16)),
    )?;

    let (_old_graph_unsafe, unsafe_graph) = transition(
        &domain,
        ids(&["A", "B", "C", "D", "E", "F", "G"]),
        ids(&["A", "B", "H", "I", "J", "K", "L"]),
    )?;
    let unsafe_report = verify_cobalt_safety_witness(
        &domain,
        &old_graph,
        &unsafe_graph,
        input(&old_graph, &unsafe_graph, profile(16)),
    )?;

    let mut stale_input = input(&old_graph, &one_rotation_graph, profile(16));
    stale_input.previous_trust_graph_root = root('f');
    let stale_report =
        verify_cobalt_safety_witness(&domain, &old_graph, &one_rotation_graph, stale_input)?;

    let mut challenge_input = input(&old_graph, &one_rotation_graph, profile(16));
    challenge_input.challenge_state = "open".to_string();
    let challenge_report =
        verify_cobalt_safety_witness(&domain, &old_graph, &one_rotation_graph, challenge_input)?;

    let oversized_report = verify_cobalt_safety_witness(
        &domain,
        &old_graph,
        &one_rotation_graph,
        input(&old_graph, &one_rotation_graph, profile(1)),
    )?;
    let mut over_budget_profile = profile(16);
    over_budget_profile.byzantine_budget = 3;
    let over_budget_report = verify_cobalt_safety_witness(
        &domain,
        &old_graph,
        &one_rotation_graph,
        input(&old_graph, &one_rotation_graph, over_budget_profile),
    )?;

    let scenarios = vec![
        scenario(
            "one_validator_rotation_accepted",
            "one-validator rotation keeps six shared validators and passes B=2",
            true,
            "accepted",
            accepted_report,
        ),
        scenario(
            "ab_to_hijkl_rejected",
            "large simultaneous delta has only A,B in common and fails B=2",
            false,
            "old-new intersection bound failed",
            unsafe_report,
        ),
        scenario(
            "stale_parent_root_rejected",
            "transition input cannot bind a root other than the active parent",
            false,
            "previous graph root mismatch",
            stale_report,
        ),
        scenario(
            "open_challenge_rejected",
            "challenge state must be cleared before activation",
            false,
            "challenge state not cleared",
            challenge_report,
        ),
        scenario(
            "oversized_cover_rejected",
            "cover size is governed and fails closed when it exceeds the profile",
            false,
            "essential subset cover exceeds profile limit",
            oversized_report,
        ),
        scenario(
            "budget_above_weakest_subset_rejected",
            "global B must not exceed the weakest covered local t_S",
            false,
            "byzantine budget exceeds weakest covered subset",
            over_budget_report,
        ),
    ];

    let scenario_failures = scenarios
        .iter()
        .filter(|scenario| !scenario.ok)
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    let ok = scenario_failures.is_empty();
    let encoded = serde_json::to_vec(&scenarios)?;
    let report = json!({
        "schema": "postfiat-cobalt-safety-witness-evidence-v1",
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "local-consensus-crate",
        "checker": "verify_cobalt_safety_witness",
        "profile": {
            "byzantine_budget": 2,
            "max_cover_subsets": 16,
            "require_cleared_challenge_state": true
        },
        "checks": {
            "bounded_rotation_accepted": scenarios.iter().any(|scenario| scenario.name == "one_validator_rotation_accepted" && scenario.ok),
            "large_delta_rejected": scenarios.iter().any(|scenario| scenario.name == "ab_to_hijkl_rejected" && scenario.ok),
            "stale_parent_rejected": scenarios.iter().any(|scenario| scenario.name == "stale_parent_root_rejected" && scenario.ok),
            "open_challenge_rejected": scenarios.iter().any(|scenario| scenario.name == "open_challenge_rejected" && scenario.ok),
            "oversized_cover_rejected": scenarios.iter().any(|scenario| scenario.name == "oversized_cover_rejected" && scenario.ok),
            "budget_above_weakest_subset_rejected": scenarios.iter().any(|scenario| scenario.name == "budget_above_weakest_subset_rejected" && scenario.ok)
        },
        "scenario_failures": scenario_failures,
        "scenario_hash": hash_hex("postfiat.cobalt.safety_witness_evidence.v1", &encoded),
        "scenarios": scenarios,
    });
    emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt safety witness evidence failed".into())
    }
}
