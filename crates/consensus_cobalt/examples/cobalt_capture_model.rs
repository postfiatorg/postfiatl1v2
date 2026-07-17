use std::collections::{BTreeMap, BTreeSet};

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_view,
    has_strong_support, CobaltDomain, CobaltFaultModel, EssentialSubset, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Clone)]
struct ValidatorPlacement {
    validator: &'static str,
    host_group: &'static str,
    operator_group: &'static str,
    funding_group: &'static str,
    jurisdiction_group: &'static str,
}

#[derive(Debug, Serialize)]
struct CaptureEvaluation {
    profile: &'static str,
    source: &'static str,
    label: String,
    captured: Vec<String>,
    captured_count: usize,
    within_all_subset_fault_bounds: bool,
    fault_bound_violation_count: usize,
    graph_safe: bool,
    unsafe_pair_count: usize,
    captured_strong_views: Vec<String>,
    liveness_blocked_views: Vec<String>,
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn validators(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{index}"))
        .collect()
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

fn strong_views(graph: &TrustGraph, support: &[String]) -> Result<Vec<String>, String> {
    let mut views = Vec::new();
    for view in &graph.trust_views {
        if has_strong_support(view, support)? {
            views.push(view.validator.clone());
        }
    }
    Ok(views)
}

fn fault_bound_violation_count(graph: &TrustGraph, captured: &[String]) -> usize {
    let captured = captured.iter().map(String::as_str).collect::<BTreeSet<_>>();
    graph
        .trust_views
        .iter()
        .flat_map(|view| view.essential_subsets.iter())
        .filter(|subset| {
            subset
                .validators
                .iter()
                .filter(|validator| captured.contains(validator.as_str()))
                .count()
                > subset.max_active_byzantine
        })
        .count()
}

fn evaluate_capture(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    all_validators: &[String],
    profile: &'static str,
    source: &'static str,
    label: String,
    captured: Vec<String>,
) -> Result<CaptureEvaluation, String> {
    let captured_set = captured.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let honest = all_validators
        .iter()
        .filter(|validator| !captured_set.contains(validator.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let captured_strong_views = strong_views(graph, &captured)?;
    let honest_strong_views = strong_views(graph, &honest)?;
    let honest_strong_set = honest_strong_views
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let liveness_blocked_views = graph
        .trust_views
        .iter()
        .map(|view| view.validator.clone())
        .filter(|validator| !honest_strong_set.contains(validator.as_str()))
        .collect::<Vec<_>>();
    let fault_bound_violation_count = fault_bound_violation_count(graph, &captured);
    let linkage = analyze_trust_graph(
        domain,
        graph,
        &CobaltFaultModel {
            actively_byzantine: captured.clone(),
        },
    )?;
    Ok(CaptureEvaluation {
        profile,
        source,
        label,
        captured_count: captured.len(),
        captured,
        within_all_subset_fault_bounds: fault_bound_violation_count == 0,
        fault_bound_violation_count,
        graph_safe: linkage.unsafe_pairs.is_empty(),
        unsafe_pair_count: linkage.unsafe_pairs.len(),
        captured_strong_views,
        liveness_blocked_views,
    })
}

fn profile_groups(
    profile: &'static str,
    placements: &[ValidatorPlacement],
    field: &'static str,
) -> Vec<(&'static str, &'static str, String, Vec<String>)> {
    let mut groups: BTreeMap<&'static str, Vec<String>> = BTreeMap::new();
    for placement in placements {
        let label = match field {
            "host_group" => placement.host_group,
            "operator_group" => placement.operator_group,
            "funding_group" => placement.funding_group,
            "jurisdiction_group" => placement.jurisdiction_group,
            _ => unreachable!("known placement field"),
        };
        groups
            .entry(label)
            .or_default()
            .push(placement.validator.to_string());
    }
    groups
        .into_iter()
        .map(|(label, captured)| (profile, field, label.to_string(), captured))
        .collect()
}

fn controlled_reused_profile() -> Vec<ValidatorPlacement> {
    vec![
        ValidatorPlacement {
            validator: "validator-0",
            host_group: "host-a",
            operator_group: "operator-a",
            funding_group: "funding-a",
            jurisdiction_group: "jurisdiction-a",
        },
        ValidatorPlacement {
            validator: "validator-1",
            host_group: "host-a",
            operator_group: "operator-a",
            funding_group: "funding-a",
            jurisdiction_group: "jurisdiction-a",
        },
        ValidatorPlacement {
            validator: "validator-2",
            host_group: "host-a",
            operator_group: "operator-a",
            funding_group: "funding-a",
            jurisdiction_group: "jurisdiction-a",
        },
        ValidatorPlacement {
            validator: "validator-3",
            host_group: "host-b",
            operator_group: "operator-b",
            funding_group: "funding-b",
            jurisdiction_group: "jurisdiction-b",
        },
        ValidatorPlacement {
            validator: "validator-4",
            host_group: "host-b",
            operator_group: "operator-b",
            funding_group: "funding-b",
            jurisdiction_group: "jurisdiction-b",
        },
        ValidatorPlacement {
            validator: "validator-5",
            host_group: "host-c",
            operator_group: "operator-c",
            funding_group: "funding-c",
            jurisdiction_group: "jurisdiction-c",
        },
        ValidatorPlacement {
            validator: "validator-6",
            host_group: "host-c",
            operator_group: "operator-c",
            funding_group: "funding-c",
            jurisdiction_group: "jurisdiction-c",
        },
    ]
}

fn strict_independent_profile() -> Vec<ValidatorPlacement> {
    vec![
        ValidatorPlacement {
            validator: "validator-0",
            host_group: "host-0",
            operator_group: "operator-0",
            funding_group: "funding-0",
            jurisdiction_group: "jurisdiction-0",
        },
        ValidatorPlacement {
            validator: "validator-1",
            host_group: "host-1",
            operator_group: "operator-1",
            funding_group: "funding-1",
            jurisdiction_group: "jurisdiction-1",
        },
        ValidatorPlacement {
            validator: "validator-2",
            host_group: "host-2",
            operator_group: "operator-2",
            funding_group: "funding-2",
            jurisdiction_group: "jurisdiction-2",
        },
        ValidatorPlacement {
            validator: "validator-3",
            host_group: "host-3",
            operator_group: "operator-3",
            funding_group: "funding-3",
            jurisdiction_group: "jurisdiction-3",
        },
        ValidatorPlacement {
            validator: "validator-4",
            host_group: "host-4",
            operator_group: "operator-4",
            funding_group: "funding-4",
            jurisdiction_group: "jurisdiction-4",
        },
        ValidatorPlacement {
            validator: "validator-5",
            host_group: "host-5",
            operator_group: "operator-5",
            funding_group: "funding-5",
            jurisdiction_group: "jurisdiction-5",
        },
        ValidatorPlacement {
            validator: "validator-6",
            host_group: "host-6",
            operator_group: "operator-6",
            funding_group: "funding-6",
            jurisdiction_group: "jurisdiction-6",
        },
    ]
}

fn single_funding_profile() -> Vec<ValidatorPlacement> {
    strict_independent_profile()
        .into_iter()
        .map(|mut placement| {
            placement.funding_group = "funding-single-source";
            placement
        })
        .collect()
}

fn evaluate_profile(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    all_validators: &[String],
    profile: &'static str,
    placements: &[ValidatorPlacement],
) -> Result<Vec<CaptureEvaluation>, String> {
    let mut evaluations = Vec::new();
    for field in [
        "host_group",
        "operator_group",
        "funding_group",
        "jurisdiction_group",
    ] {
        for (profile, source, label, captured) in profile_groups(profile, placements, field) {
            evaluations.push(evaluate_capture(
                domain,
                graph,
                all_validators,
                profile,
                source,
                label,
                captured,
            )?);
        }
    }
    Ok(evaluations)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let all_validators = validators(7);
    let mut evaluations = Vec::new();
    evaluations.extend(evaluate_profile(
        &domain,
        &graph,
        &all_validators,
        "controlled_reused_groups",
        &controlled_reused_profile(),
    )?);
    evaluations.extend(evaluate_profile(
        &domain,
        &graph,
        &all_validators,
        "strict_independent_groups",
        &strict_independent_profile(),
    )?);
    evaluations.extend(evaluate_profile(
        &domain,
        &graph,
        &all_validators,
        "single_funding_source",
        &single_funding_profile(),
    )?);
    for (label, captured) in [
        (
            "injected-two-validator-liveness-block",
            vec!["validator-0".to_string(), "validator-1".to_string()],
        ),
        (
            "injected-three-validator-linkage-break",
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
            ],
        ),
        (
            "injected-five-validator-captured-strong",
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string(),
                "validator-4".to_string(),
            ],
        ),
    ] {
        evaluations.push(evaluate_capture(
            &domain,
            &graph,
            &all_validators,
            "injected_capture_sets",
            "injected_capture_set",
            label.to_string(),
            captured,
        )?);
    }

    let controlled_risk_detected = evaluations.iter().any(|evaluation| {
        evaluation.profile == "controlled_reused_groups"
            && (!evaluation.graph_safe || !evaluation.liveness_blocked_views.is_empty())
    });
    let strict_single_group_safe = evaluations
        .iter()
        .filter(|evaluation| evaluation.profile == "strict_independent_groups")
        .all(|evaluation| {
            evaluation.within_all_subset_fault_bounds
                && evaluation.graph_safe
                && evaluation.captured_strong_views.is_empty()
                && evaluation.liveness_blocked_views.is_empty()
        });
    let single_funding_capture_detected = evaluations.iter().any(|evaluation| {
        evaluation.profile == "single_funding_source"
            && evaluation.source == "funding_group"
            && !evaluation.captured_strong_views.is_empty()
            && !evaluation.graph_safe
    });
    let injected_liveness_detected = evaluations.iter().any(|evaluation| {
        evaluation.label == "injected-two-validator-liveness-block"
            && !evaluation.liveness_blocked_views.is_empty()
    });
    let injected_linkage_detected = evaluations.iter().any(|evaluation| {
        evaluation.label == "injected-three-validator-linkage-break" && !evaluation.graph_safe
    });
    let injected_strong_detected = evaluations.iter().any(|evaluation| {
        evaluation.label == "injected-five-validator-captured-strong"
            && !evaluation.captured_strong_views.is_empty()
    });
    let checks = json!({
        "seven_logical_validators": graph.trust_views.len() == 7,
        "non_identical_trust_views": graph.trust_views.iter().map(|view| &view.trust_view_id).collect::<BTreeSet<_>>().len() >= 3,
        "profiles_evaluated": evaluations.iter().map(|evaluation| evaluation.profile).collect::<BTreeSet<_>>().len() == 4,
        "controlled_reused_group_risk_detected": controlled_risk_detected,
        "strict_independent_single_group_safe": strict_single_group_safe,
        "single_funding_source_capture_detected": single_funding_capture_detected,
        "injected_liveness_capture_detected": injected_liveness_detected,
        "injected_linkage_capture_detected": injected_linkage_detected,
        "injected_captured_strong_support_detected": injected_strong_detected,
        "outside_operators_required": false,
    });
    let ok = checks
        .as_object()
        .expect("checks object")
        .iter()
        .all(|(key, value)| {
            if key == "outside_operators_required" {
                value.as_bool() == Some(false)
            } else {
                value.as_bool() == Some(true)
            }
        });
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-capture-model-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": all_validators.len(),
        "trust_graph_root": graph.trust_graph_root,
        "profile_count": evaluations.iter().map(|evaluation| evaluation.profile).collect::<BTreeSet<_>>().len(),
        "evaluation_count": evaluations.len(),
        "checks": checks,
        "evaluations": evaluations,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt capture model report failed".into())
    }
}
