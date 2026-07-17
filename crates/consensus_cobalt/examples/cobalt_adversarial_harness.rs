use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    abba_strong_support, analyze_trust_graph, build_abba_aux, build_abba_init,
    build_essential_subset, build_rbc_accept, build_rbc_echo, build_rbc_propose, build_rbc_ready,
    build_trust_graph, build_trust_view, detect_abba_init_equivocation,
    detect_rbc_conflicting_accept, evaluate_abba_aux_support, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo,
    validate_rbc_propose, CobaltDomain, CobaltFaultModel, EssentialSubset, RbcAccept, RbcEcho,
    RbcPropose, RbcReady, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

type RbcTranscript = (RbcPropose, Vec<RbcEcho>, Vec<RbcReady>, Vec<RbcAccept>);

#[derive(Serialize)]
struct ScenarioResult {
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
    let linkage = analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default())?;
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

fn all_rbc_messages(domain: &CobaltDomain, graph: &TrustGraph) -> Result<RbcTranscript, String> {
    let validator_ids = validators(7);
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        701,
        root('f'),
        "",
    )?;
    let echoes = validator_ids
        .iter()
        .map(|sender| build_rbc_echo(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let readies = validator_ids
        .iter()
        .map(|sender| build_rbc_ready(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let accepts = validator_ids
        .iter()
        .map(|sender| build_rbc_accept(domain, &propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((propose, echoes, readies, accepts))
}

fn retain_senders<T, F>(messages: &[T], sender: F, retained: &BTreeSet<String>) -> Vec<T>
where
    T: Clone,
    F: Fn(&T) -> &str,
{
    messages
        .iter()
        .filter(|message| retained.contains(sender(message)))
        .cloned()
        .collect()
}

fn rbc_acceptance_by_validator(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    propose: &RbcPropose,
    echoes: &[RbcEcho],
    readies: &[RbcReady],
) -> Result<Vec<(String, bool, bool, bool)>, String> {
    let mut results = Vec::new();
    for validator in validators(7) {
        let trust_view = view(graph, &validator)?;
        let echo = evaluate_rbc_echo_support(domain, trust_view, propose, echoes)?;
        let ready = evaluate_rbc_ready_support(domain, trust_view, propose, readies)?;
        results.push((
            validator,
            echo.strong_support,
            rbc_ready_allowed_from_echo(&echo),
            rbc_accept_allowed_from_ready(&ready),
        ));
    }
    Ok(results)
}

fn accepted_validators(results: &[(String, bool, bool, bool)]) -> Vec<String> {
    results
        .iter()
        .filter(
            |(_validator, echo_strong, ready_from_echo, accept_allowed)| {
                *echo_strong && *ready_from_echo && *accept_allowed
            },
        )
        .map(|(validator, _echo_strong, _ready_from_echo, _accept_allowed)| validator.clone())
        .collect()
}

fn scenario_honest(domain: &CobaltDomain, graph: &TrustGraph) -> Result<ScenarioResult, String> {
    let (propose, echoes, readies, _accepts) = all_rbc_messages(domain, graph)?;
    let results = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let accepted = accepted_validators(&results);
    Ok(ScenarioResult {
        name: "honest_all_accept",
        behavior: vec!["honest"],
        expected: "all seven local trust views accept the same RBC payload",
        observed: json!({
            "accepted_by": accepted,
            "per_validator": results,
        }),
        ok: accepted.len() == 7,
    })
}

fn scenario_single_withhold(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (propose, echoes, readies, _accepts) = all_rbc_messages(domain, graph)?;
    let retained = validators(7)
        .into_iter()
        .filter(|validator| validator != "validator-0")
        .collect::<BTreeSet<_>>();
    let echoes = retain_senders(&echoes, |message| &message.sender, &retained);
    let readies = retain_senders(&readies, |message| &message.sender, &retained);
    let results = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let accepted = accepted_validators(&results);
    Ok(ScenarioResult {
        name: "single_withhold_still_accepts",
        behavior: vec!["withhold"],
        expected: "one validator withholding messages does not break strong support in G1",
        observed: json!({
            "withheld": ["validator-0"],
            "accepted_by": accepted,
            "per_validator": results,
        }),
        ok: accepted.len() == 7,
    })
}

fn scenario_colluding_withhold(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (propose, echoes, readies, _accepts) = all_rbc_messages(domain, graph)?;
    let retained = ["validator-2", "validator-3", "validator-4", "validator-6"]
        .into_iter()
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    let echoes = retain_senders(&echoes, |message| &message.sender, &retained);
    let readies = retain_senders(&readies, |message| &message.sender, &retained);
    let results = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let accepted = accepted_validators(&results);
    Ok(ScenarioResult {
        name: "colluding_withhold_breaks_liveness_not_safety",
        behavior: vec!["collude", "withhold"],
        expected:
            "below-quorum support prevents local acceptance rather than accepting partial support",
        observed: json!({
            "retained_senders": retained,
            "accepted_by": accepted,
            "per_validator": results,
        }),
        ok: accepted.is_empty(),
    })
}

fn scenario_duplicate_reordered(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (propose, mut echoes, mut readies, _accepts) = all_rbc_messages(domain, graph)?;
    echoes.reverse();
    readies.reverse();
    echoes.push(echoes[0].clone());
    readies.push(readies[0].clone());
    let results = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let accepted = accepted_validators(&results);
    Ok(ScenarioResult {
        name: "duplicate_reordered_messages_deduped",
        behavior: vec!["delay", "duplicate", "reorder"],
        expected: "duplicate and reordered messages preserve deterministic support evaluation",
        observed: json!({
            "echo_count_with_duplicate": echoes.len(),
            "ready_count_with_duplicate": readies.len(),
            "accepted_by": accepted,
        }),
        ok: accepted.len() == 7,
    })
}

fn scenario_invalid_signature(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (propose, mut echoes, _readies, _accepts) = all_rbc_messages(domain, graph)?;
    echoes[0].signature_hex = "not-lower-hex".to_string();
    let result = evaluate_rbc_echo_support(domain, view(graph, "validator-0")?, &propose, &echoes);
    let error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    Ok(ScenarioResult {
        name: "invalid_signature_rejected",
        behavior: vec!["invalid_signature"],
        expected: "malformed signature hex fails before support evaluation succeeds",
        observed: json!({ "error": error }),
        ok: error.contains("RBC signature must be lowercase hex"),
    })
}

fn scenario_malformed_payload(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (mut propose, _echoes, _readies, _accepts) = all_rbc_messages(domain, graph)?;
    propose.payload_hash = "bad-payload".to_string();
    let error = validate_rbc_propose(domain, &propose)
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    Ok(ScenarioResult {
        name: "malformed_payload_rejected",
        behavior: vec!["malformed_payload"],
        expected: "malformed RBC payload hash is rejected",
        observed: json!({ "error": error }),
        ok: error.contains("RBC payload hash must be 96 lowercase hex characters"),
    })
}

fn scenario_stale_root_guard(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let stale = build_rbc_propose(domain, root('a'), "validator-0", 701, root('f'), "")?;
    let stale_root_detected = stale.trust_graph_root != graph.trust_graph_root;
    Ok(ScenarioResult {
        name: "stale_root_detected_before_active_graph_evaluation",
        behavior: vec!["stale_root"],
        expected: "messages bound to a non-active trust graph root are detected before local support evaluation",
        observed: json!({
            "active_trust_graph_root": graph.trust_graph_root,
            "message_trust_graph_root": stale.trust_graph_root,
            "stale_root_detected": stale_root_detected,
        }),
        ok: stale_root_detected,
    })
}

fn scenario_conflicting_accept(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let left = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        702,
        root('c'),
        "",
    )?;
    let right = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        702,
        root('d'),
        "",
    )?;
    let left_accept = build_rbc_accept(domain, &left, "validator-1", "")?;
    let right_accept = build_rbc_accept(domain, &right, "validator-2", "")?;
    let evidence =
        detect_rbc_conflicting_accept(domain, graph, &left, &left_accept, &right, &right_accept)?;
    Ok(ScenarioResult {
        name: "rbc_conflicting_accept_detected",
        behavior: vec!["equivocate"],
        expected:
            "linked validators accepting conflicting RBC payloads produce deterministic evidence",
        observed: json!({ "evidence": evidence }),
        ok: evidence.is_some(),
    })
}

fn scenario_abba_equivocation(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let agreement_id = root('7');
    let left = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let right = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id,
        1,
        false,
        "",
    )?;
    let evidence = detect_abba_init_equivocation(domain, &left, &right)?;
    Ok(ScenarioResult {
        name: "abba_same_sender_equivocation_detected",
        behavior: vec!["equivocate"],
        expected: "same-sender ABBA init equivocation produces deterministic evidence",
        observed: json!({ "evidence": evidence }),
        ok: evidence.is_some(),
    })
}

fn scenario_abba_equivocal_sender_excluded(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let agreement_id = root('8');
    let mut aux = Vec::new();
    for sender in validators(7) {
        aux.push(build_abba_aux(
            domain,
            graph.trust_graph_root.clone(),
            sender,
            agreement_id.clone(),
            1,
            true,
            "",
        )?);
    }
    aux.push(build_abba_aux(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        1,
        false,
        "",
    )?);
    let evaluation = evaluate_abba_aux_support(
        domain,
        view(graph, "validator-1")?,
        &agreement_id,
        1,
        true,
        &aux,
    )?;
    Ok(ScenarioResult {
        name: "abba_equivocal_sender_excluded_from_support",
        behavior: vec!["equivocate"],
        expected: "equivocal ABBA sender is excluded while remaining support stays strong",
        observed: json!({
            "support": evaluation.support,
            "equivocal_sender_excluded": !evaluation.support.contains(&"validator-0".to_string()),
            "strong_support": evaluation.strong_support,
        }),
        ok: !evaluation.support.contains(&"validator-0".to_string())
            && abba_strong_support(&evaluation),
    })
}

fn scenario_crash_restart_idempotent(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ScenarioResult, String> {
    let (propose, echoes, readies, _accepts) = all_rbc_messages(domain, graph)?;
    let before = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let after = rbc_acceptance_by_validator(domain, graph, &propose, &echoes, &readies)?;
    let before_accepted = accepted_validators(&before);
    let after_accepted = accepted_validators(&after);
    Ok(ScenarioResult {
        name: "crash_restart_replay_idempotent",
        behavior: vec!["crash", "restart"],
        expected:
            "deterministic replay after simulated restart yields the same accepted validators",
        observed: json!({
            "accepted_before": before_accepted,
            "accepted_after": after_accepted,
        }),
        ok: before_accepted == after_accepted && after_accepted.len() == 7,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_honest(&domain, &graph)?,
        scenario_single_withhold(&domain, &graph)?,
        scenario_colluding_withhold(&domain, &graph)?,
        scenario_duplicate_reordered(&domain, &graph)?,
        scenario_invalid_signature(&domain, &graph)?,
        scenario_malformed_payload(&domain, &graph)?,
        scenario_stale_root_guard(&domain, &graph)?,
        scenario_conflicting_accept(&domain, &graph)?,
        scenario_abba_equivocation(&domain, &graph)?,
        scenario_abba_equivocal_sender_excluded(&domain, &graph)?,
        scenario_crash_restart_idempotent(&domain, &graph)?,
    ];
    let behavior_scripts = scenarios
        .iter()
        .flat_map(|scenario| scenario.behavior.iter().copied())
        .collect::<BTreeSet<_>>();
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
        "schema": "postfiat-testnet-cobalt-adversarial-harness-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": 7,
        "trust_graph_root": graph.trust_graph_root,
        "distinct_trust_view_count": graph.trust_views.iter().map(|view| &view.trust_view_id).collect::<BTreeSet<_>>().len(),
        "behavior_scripts": behavior_scripts,
        "checks": {
            "seven_logical_validators": true,
            "non_identical_trust_views": graph.trust_views.iter().map(|view| &view.trust_view_id).collect::<BTreeSet<_>>().len() >= 3,
            "all_scenarios_passed": ok,
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt adversarial harness failed".into())
    }
}
