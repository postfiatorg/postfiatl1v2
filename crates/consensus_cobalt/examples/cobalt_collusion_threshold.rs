use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_view,
    has_strong_support, CobaltDomain, CobaltFaultModel, EssentialSubset, TrustGraph, TrustView,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct SubsetFaultViolation {
    local_validator: String,
    subset_id: String,
    captured_in_subset: usize,
    max_active_byzantine: usize,
    quorum: usize,
    validator_count: usize,
}

#[derive(Debug, Serialize)]
struct CaptureSetReport {
    captured: Vec<String>,
    captured_count: usize,
    within_all_subset_fault_bounds: bool,
    fault_bound_violations: Vec<SubsetFaultViolation>,
    graph_safe: bool,
    unsafe_pair_count: usize,
    unsafe_pairs: Vec<serde_json::Value>,
    captured_strong_views: Vec<String>,
    honest_strong_views: Vec<String>,
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

fn all_capture_sets(validators: &[String]) -> Vec<Vec<String>> {
    let mut sets = Vec::new();
    for mask in 0..(1usize << validators.len()) {
        let mut captured = Vec::new();
        for (index, validator) in validators.iter().enumerate() {
            if (mask & (1usize << index)) != 0 {
                captured.push(validator.clone());
            }
        }
        sets.push(captured);
    }
    sets
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

fn fault_bound_violations(graph: &TrustGraph, captured: &[String]) -> Vec<SubsetFaultViolation> {
    let captured = captured.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let mut violations = Vec::new();
    for view in &graph.trust_views {
        for subset in &view.essential_subsets {
            let captured_in_subset = subset
                .validators
                .iter()
                .filter(|validator| captured.contains(validator.as_str()))
                .count();
            if captured_in_subset > subset.max_active_byzantine {
                violations.push(SubsetFaultViolation {
                    local_validator: view.validator.clone(),
                    subset_id: subset.subset_id.clone(),
                    captured_in_subset,
                    max_active_byzantine: subset.max_active_byzantine,
                    quorum: subset.quorum,
                    validator_count: subset.validator_count,
                });
            }
        }
    }
    violations
}

fn summarize_capture_set(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    validators: &[String],
    captured: Vec<String>,
) -> Result<CaptureSetReport, String> {
    let captured_set = captured.iter().map(String::as_str).collect::<BTreeSet<_>>();
    let honest = validators
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
    let fault_bound_violations = fault_bound_violations(graph, &captured);
    let linkage = analyze_trust_graph(
        domain,
        graph,
        &CobaltFaultModel {
            actively_byzantine: captured.clone(),
        },
    )?;
    let unsafe_pairs = linkage
        .unsafe_pairs
        .iter()
        .map(|pair| {
            json!({
                "left": pair.left,
                "right": pair.right,
                "reason": pair.reason,
            })
        })
        .collect::<Vec<_>>();
    Ok(CaptureSetReport {
        captured_count: captured.len(),
        captured,
        within_all_subset_fault_bounds: fault_bound_violations.is_empty(),
        fault_bound_violations,
        graph_safe: linkage.unsafe_pairs.is_empty(),
        unsafe_pair_count: linkage.unsafe_pairs.len(),
        unsafe_pairs,
        captured_strong_views,
        honest_strong_views,
        liveness_blocked_views,
    })
}

fn view_summary(view: &TrustView) -> serde_json::Value {
    json!({
        "validator": view.validator,
        "trust_view_id": view.trust_view_id,
        "derived_unl": view.derived_unl,
        "essential_subsets": view.essential_subsets.iter().map(|subset| {
            json!({
                "subset_id": subset.subset_id,
                "validator_count": subset.validator_count,
                "max_active_byzantine": subset.max_active_byzantine,
                "quorum": subset.quorum,
            })
        }).collect::<Vec<_>>(),
    })
}

fn first_matching<F>(reports: &[CaptureSetReport], mut predicate: F) -> Option<&CaptureSetReport>
where
    F: FnMut(&CaptureSetReport) -> bool,
{
    reports
        .iter()
        .filter(|report| predicate(report))
        .min_by(|left, right| {
            left.captured_count
                .cmp(&right.captured_count)
                .then_with(|| left.captured.cmp(&right.captured))
        })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let validators = validators(7);
    let mut capture_reports = Vec::new();
    for captured in all_capture_sets(&validators) {
        capture_reports.push(summarize_capture_set(
            &domain,
            &graph,
            &validators,
            captured,
        )?);
    }

    let within_fault_bound_count = capture_reports
        .iter()
        .filter(|report| report.within_all_subset_fault_bounds)
        .count();
    let within_fault_bound_unsafe = capture_reports
        .iter()
        .filter(|report| report.within_all_subset_fault_bounds && !report.graph_safe)
        .count();
    let within_fault_bound_captured_strong = capture_reports
        .iter()
        .filter(|report| {
            report.within_all_subset_fault_bounds && !report.captured_strong_views.is_empty()
        })
        .count();
    let within_fault_bound_liveness_blocked = capture_reports
        .iter()
        .filter(|report| {
            report.within_all_subset_fault_bounds && !report.liveness_blocked_views.is_empty()
        })
        .count();

    let first_liveness_block = first_matching(&capture_reports, |report| {
        !report.within_all_subset_fault_bounds && !report.liveness_blocked_views.is_empty()
    });
    let first_graph_break = first_matching(&capture_reports, |report| {
        !report.within_all_subset_fault_bounds && !report.graph_safe
    });
    let first_captured_strong = first_matching(&capture_reports, |report| {
        !report.within_all_subset_fault_bounds && !report.captured_strong_views.is_empty()
    });

    let checks = json!({
        "seven_logical_validators": graph.trust_views.len() == 7,
        "non_identical_trust_views": graph.trust_views.iter().map(|view| &view.trust_view_id).collect::<BTreeSet<_>>().len() >= 3,
        "all_capture_sets_enumerated": capture_reports.len() == 128,
        "within_fault_bound_no_unsafe_graphs": within_fault_bound_unsafe == 0,
        "within_fault_bound_no_captured_strong_support": within_fault_bound_captured_strong == 0,
        "within_fault_bound_no_liveness_block": within_fault_bound_liveness_blocked == 0,
        "over_fault_bound_liveness_block_evidence_present": first_liveness_block.is_some(),
        "over_fault_bound_linkage_break_evidence_present": first_graph_break.is_some(),
        "over_fault_bound_captured_strong_support_evidence_present": first_captured_strong.is_some(),
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
        "schema": "postfiat-testnet-cobalt-collusion-threshold-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": validators.len(),
        "capture_set_count": capture_reports.len(),
        "within_fault_bound_capture_set_count": within_fault_bound_count,
        "trust_graph_root": graph.trust_graph_root,
        "trust_views": graph.trust_views.iter().map(view_summary).collect::<Vec<_>>(),
        "checks": checks,
        "first_over_fault_bound_liveness_block": first_liveness_block,
        "first_over_fault_bound_linkage_break": first_graph_break,
        "first_over_fault_bound_captured_strong_support": first_captured_strong,
        "capture_sets": capture_reports,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt collusion threshold report failed".into())
    }
}
