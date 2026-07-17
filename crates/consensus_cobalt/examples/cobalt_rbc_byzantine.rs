use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    build_essential_subset, build_rbc_accept, build_rbc_echo, build_rbc_propose, build_rbc_ready,
    build_trust_graph, build_trust_view, detect_rbc_conflicting_accept, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo,
    rbc_ready_allowed_from_ready, validate_rbc_accept, validate_rbc_echo, validate_rbc_ready,
    CobaltDomain, CobaltFaultModel, EssentialSubset, RbcAccept, RbcEcho, RbcPropose, RbcReady,
    TrustGraph,
};
use serde::Serialize;
use serde_json::json;

type RbcTranscript = (RbcPropose, Vec<RbcEcho>, Vec<RbcReady>, Vec<RbcAccept>);

#[derive(Debug, Serialize)]
struct RbcScenario {
    name: &'static str,
    behavior: Vec<&'static str>,
    expected: &'static str,
    observed: serde_json::Value,
    ok: bool,
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
    let linkage = postfiat_consensus_cobalt::analyze_trust_graph(
        &domain,
        &graph,
        &CobaltFaultModel::default(),
    )?;
    if !linkage.unsafe_pairs.is_empty() {
        return Err("fixture graph is unsafe".to_string());
    }
    Ok((domain, graph))
}

fn view<'a>(
    graph: &'a TrustGraph,
    validator: &str,
) -> Result<&'a postfiat_consensus_cobalt::TrustView, String> {
    graph
        .trust_views
        .iter()
        .find(|view| view.validator == validator)
        .ok_or_else(|| format!("missing trust view for {validator}"))
}

fn all_messages(domain: &CobaltDomain, graph: &TrustGraph) -> Result<RbcTranscript, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        88,
        root('a'),
        "",
    )?;
    let echoes = validators(7)
        .iter()
        .map(|sender| build_rbc_echo(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let readies = validators(7)
        .iter()
        .map(|sender| build_rbc_ready(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let accepts = validators(7)
        .iter()
        .map(|sender| build_rbc_accept(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((propose, echoes, readies, accepts))
}

fn scenario_double_propose_conflict(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let left = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        89,
        root('c'),
        "",
    )?;
    let right = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        89,
        root('d'),
        "",
    )?;
    let left_accept = build_rbc_accept(domain, &left, "validator-1", "")?;
    let right_accept = build_rbc_accept(domain, &right, "validator-2", "")?;
    let evidence =
        detect_rbc_conflicting_accept(domain, graph, &left, &left_accept, &right, &right_accept)?;
    Ok(RbcScenario {
        name: "double_propose_conflicting_accept_detected",
        behavior: vec!["double_propose", "conflicting_accept"],
        expected:
            "same proposer and slot with different payloads produces conflicting accept evidence",
        observed: json!({ "evidence": evidence }),
        ok: evidence.is_some(),
    })
}

fn scenario_conflicting_linked_messages_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let (propose, _echoes, _readies, _accepts) = all_messages(domain, graph)?;
    let conflicting = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        propose.amendment_slot,
        root('e'),
        "",
    )?;
    let echo = build_rbc_echo(domain, &conflicting, "validator-1", "")?;
    let ready = build_rbc_ready(domain, &conflicting, "validator-1", "")?;
    let accept = build_rbc_accept(domain, &conflicting, "validator-1", "")?;
    let echo_error = validate_rbc_echo(domain, &echo, &propose)
        .err()
        .unwrap_or_else(|| "unexpected echo success".to_string());
    let ready_error = validate_rbc_ready(domain, &ready, &propose)
        .err()
        .unwrap_or_else(|| "unexpected ready success".to_string());
    let accept_error = validate_rbc_accept(domain, &accept, &propose)
        .err()
        .unwrap_or_else(|| "unexpected accept success".to_string());
    Ok(RbcScenario {
        name: "conflicting_echo_ready_accept_rejected",
        behavior: vec![
            "conflicting_echo",
            "conflicting_ready",
            "conflicting_accept",
        ],
        expected:
            "messages bound to a conflicting proposal do not validate against the original proposal",
        observed: json!({
            "echo_error": echo_error,
            "ready_error": ready_error,
            "accept_error": accept_error,
        }),
        ok: echo_error.contains("does not match RBC propose")
            && ready_error.contains("does not match RBC propose")
            && accept_error.contains("does not match RBC propose"),
    })
}

fn scenario_triggerless_ready_and_accept(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let (propose, echoes, readies, _accepts) = all_messages(domain, graph)?;
    let one_echo = vec![echoes[0].clone()];
    let one_ready = vec![readies[0].clone()];
    let echo_eval =
        evaluate_rbc_echo_support(domain, view(graph, "validator-0")?, &propose, &one_echo)?;
    let ready_eval =
        evaluate_rbc_ready_support(domain, view(graph, "validator-0")?, &propose, &one_ready)?;
    let ready_allowed_from_echo = rbc_ready_allowed_from_echo(&echo_eval);
    let ready_allowed_from_ready = rbc_ready_allowed_from_ready(&ready_eval);
    let accept_allowed_from_ready = rbc_accept_allowed_from_ready(&ready_eval);
    Ok(RbcScenario {
        name: "triggerless_ready_and_accept_not_allowed",
        behavior: vec!["ready_without_valid_trigger", "accept_without_valid_ready"],
        expected: "single-sender echo/ready support cannot trigger ready or accept",
        observed: json!({
            "echo_support": echo_eval.support,
            "ready_support": ready_eval.support,
            "ready_allowed_from_echo": ready_allowed_from_echo,
            "ready_allowed_from_ready": ready_allowed_from_ready,
            "accept_allowed_from_ready": accept_allowed_from_ready,
        }),
        ok: !ready_allowed_from_echo && !ready_allowed_from_ready && !accept_allowed_from_ready,
    })
}

fn scenario_duplicate_messages_deduped(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let (propose, mut echoes, _readies, _accepts) = all_messages(domain, graph)?;
    echoes.push(echoes[0].clone());
    echoes.push(echoes[1].clone());
    let evaluation =
        evaluate_rbc_echo_support(domain, view(graph, "validator-0")?, &propose, &echoes)?;
    Ok(RbcScenario {
        name: "duplicate_messages_deduped",
        behavior: vec!["duplicate"],
        expected: "duplicate echo messages are deduped before support evaluation",
        observed: json!({
            "input_echo_count": echoes.len(),
            "deduped_support": evaluation.support,
            "strong_support": evaluation.strong_support,
        }),
        ok: evaluation.support.len() == 7 && evaluation.strong_support,
    })
}

fn scenario_invalid_signature_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let (propose, mut echoes, _readies, _accepts) = all_messages(domain, graph)?;
    echoes[0].signature_hex = "not-lower-hex".to_string();
    let error = evaluate_rbc_echo_support(domain, view(graph, "validator-0")?, &propose, &echoes)
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    Ok(RbcScenario {
        name: "invalid_signature_rejected",
        behavior: vec!["invalid_signature"],
        expected: "invalid RBC signature encoding fails before support succeeds",
        observed: json!({ "error": error }),
        ok: error.contains("RBC signature must be lowercase hex"),
    })
}

fn scenario_withheld_ready_no_accept(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<RbcScenario, String> {
    let (propose, _echoes, readies, _accepts) = all_messages(domain, graph)?;
    let retained = ["validator-0", "validator-1", "validator-2", "validator-3"]
        .into_iter()
        .collect::<BTreeSet<_>>();
    let readies = readies
        .into_iter()
        .filter(|ready| retained.contains(ready.sender.as_str()))
        .collect::<Vec<_>>();
    let evaluation =
        evaluate_rbc_ready_support(domain, view(graph, "validator-0")?, &propose, &readies)?;
    let accept_allowed = rbc_accept_allowed_from_ready(&evaluation);
    Ok(RbcScenario {
        name: "withheld_ready_prevents_accept",
        behavior: vec!["withhold"],
        expected: "four ready messages are not enough for strong ready support in the seven-validator view",
        observed: json!({
            "ready_support": evaluation.support,
            "strong_support": evaluation.strong_support,
            "accept_allowed": accept_allowed,
        }),
        ok: !evaluation.strong_support && !accept_allowed,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_double_propose_conflict(&domain, &graph)?,
        scenario_conflicting_linked_messages_rejected(&domain, &graph)?,
        scenario_triggerless_ready_and_accept(&domain, &graph)?,
        scenario_duplicate_messages_deduped(&domain, &graph)?,
        scenario_invalid_signature_rejected(&domain, &graph)?,
        scenario_withheld_ready_no_accept(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-rbc-byzantine-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_rbc_byzantine_scenarios_passed": ok,
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt RBC Byzantine report failed".into())
    }
}
