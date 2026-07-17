use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_rbc_accept, build_rbc_echo,
    build_rbc_propose, build_rbc_ready, build_trust_graph, build_trust_view,
    detect_rbc_conflicting_accept, evaluate_rbc_echo_support, evaluate_rbc_ready_support,
    rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo, CobaltDomain, CobaltFaultModel,
    EssentialSubset, RbcEcho, RbcPropose, RbcReady, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct PartitionScenario {
    name: &'static str,
    partition: Vec<Vec<&'static str>>,
    fault: Vec<&'static str>,
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

fn propose(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    slot: u64,
    payload_byte: char,
) -> Result<RbcPropose, String> {
    build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        slot,
        root(payload_byte),
        "",
    )
}

fn messages(
    domain: &CobaltDomain,
    propose: &RbcPropose,
) -> Result<(Vec<RbcEcho>, Vec<RbcReady>), String> {
    let echoes = validators(7)
        .iter()
        .map(|sender| build_rbc_echo(domain, propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    let readies = validators(7)
        .iter()
        .map(|sender| build_rbc_ready(domain, propose, sender, ""))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((echoes, readies))
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

fn accepted_by(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    propose: &RbcPropose,
    visible_senders: &BTreeSet<String>,
    local_validators: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    let (echoes, readies) = messages(domain, propose)?;
    let echoes = retain_senders(&echoes, |message| &message.sender, visible_senders);
    let readies = retain_senders(&readies, |message| &message.sender, visible_senders);
    let mut accepted = Vec::new();
    for local in local_validators {
        let trust_view = view(graph, local)?;
        let echo_eval = evaluate_rbc_echo_support(domain, trust_view, propose, &echoes)?;
        let ready_eval = evaluate_rbc_ready_support(domain, trust_view, propose, &readies)?;
        if echo_eval.strong_support
            && rbc_ready_allowed_from_echo(&echo_eval)
            && rbc_accept_allowed_from_ready(&ready_eval)
        {
            accepted.push(local.clone());
        }
    }
    Ok(accepted)
}

fn set(members: &[&str]) -> BTreeSet<String> {
    members.iter().map(|member| (*member).to_string()).collect()
}

fn scenario_three_four_partition(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let left = set(&["validator-0", "validator-1", "validator-2"]);
    let right = set(&["validator-3", "validator-4", "validator-5", "validator-6"]);
    let left_propose = propose(domain, graph, 1001, 'a')?;
    let right_propose = propose(domain, graph, 1001, 'c')?;
    let left_accepted = accepted_by(domain, graph, &left_propose, &left, &left)?;
    let right_accepted = accepted_by(domain, graph, &right_propose, &right, &right)?;
    Ok(PartitionScenario {
        name: "three_four_partition_has_no_conflicting_acceptance",
        partition: vec![
            vec!["validator-0", "validator-1", "validator-2"],
            vec!["validator-3", "validator-4", "validator-5", "validator-6"],
        ],
        fault: vec!["partition", "conflicting_payloads"],
        expected: "3/4 split does not create strong support for either conflicting payload; liveness waits for heal",
        observed: json!({
            "left_payload_accepted_by": left_accepted,
            "right_payload_accepted_by": right_accepted,
            "liveness_expected_before_heal": false,
        }),
        ok: left_accepted.is_empty() && right_accepted.is_empty(),
    })
}

fn scenario_two_two_three_partition(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let groups = [
        set(&["validator-0", "validator-1"]),
        set(&["validator-2", "validator-3"]),
        set(&["validator-4", "validator-5", "validator-6"]),
    ];
    let payloads = ['a', 'b', 'c'];
    let mut accepted_groups = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        let proposal = propose(domain, graph, 1002, payloads[index])?;
        accepted_groups.push(accepted_by(domain, graph, &proposal, group, group)?);
    }
    Ok(PartitionScenario {
        name: "two_two_three_partition_has_no_progress_no_conflict",
        partition: vec![
            vec!["validator-0", "validator-1"],
            vec!["validator-2", "validator-3"],
            vec!["validator-4", "validator-5", "validator-6"],
        ],
        fault: vec!["partition", "conflicting_payloads"],
        expected: "2/2/3 split cannot satisfy strong support in any partition",
        observed: json!({
            "accepted_by_group": accepted_groups,
            "liveness_expected_before_heal": false,
        }),
        ok: accepted_groups.iter().all(Vec::is_empty),
    })
}

fn scenario_single_isolated_validator(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let majority = set(&[
        "validator-0",
        "validator-1",
        "validator-2",
        "validator-3",
        "validator-4",
        "validator-5",
    ]);
    let isolated = set(&["validator-6"]);
    let proposal = propose(domain, graph, 1003, 'd')?;
    let majority_accepted = accepted_by(domain, graph, &proposal, &majority, &majority)?;
    let isolated_accepted = accepted_by(domain, graph, &proposal, &isolated, &isolated)?;
    Ok(PartitionScenario {
        name: "single_validator_isolation_preserves_majority_progress",
        partition: vec![
            vec![
                "validator-0",
                "validator-1",
                "validator-2",
                "validator-3",
                "validator-4",
                "validator-5",
            ],
            vec!["validator-6"],
        ],
        fault: vec!["single_validator_isolated"],
        expected:
            "six connected validators can still accept one payload; the isolated validator cannot",
        observed: json!({
            "majority_accepted_by": majority_accepted,
            "isolated_accepted_by": isolated_accepted,
            "liveness_expected_for_majority": true,
        }),
        ok: majority_accepted.len() == 6 && isolated_accepted.is_empty(),
    })
}

fn scenario_delay_reorder_duplicate(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let proposal = propose(domain, graph, 1004, 'e')?;
    let all = set(&[
        "validator-0",
        "validator-1",
        "validator-2",
        "validator-3",
        "validator-4",
        "validator-5",
        "validator-6",
    ]);
    let normal = accepted_by(domain, graph, &proposal, &all, &all)?;
    let (mut echoes, mut readies) = messages(domain, &proposal)?;
    echoes.reverse();
    readies.rotate_left(3);
    echoes.extend(echoes.clone());
    readies.extend(readies.clone());
    let mut disordered = Vec::new();
    for validator in validators(7) {
        let trust_view = view(graph, &validator)?;
        let echo_eval = evaluate_rbc_echo_support(domain, trust_view, &proposal, &echoes)?;
        let ready_eval = evaluate_rbc_ready_support(domain, trust_view, &proposal, &readies)?;
        if echo_eval.strong_support
            && rbc_ready_allowed_from_echo(&echo_eval)
            && rbc_accept_allowed_from_ready(&ready_eval)
        {
            disordered.push(validator);
        }
    }
    Ok(PartitionScenario {
        name: "delay_reorder_duplicate_preserves_support_decision",
        partition: vec![vec![
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ]],
        fault: vec!["delay", "reorder", "duplicate"],
        expected:
            "support evaluation is deterministic under delayed, reordered, and duplicated messages",
        observed: json!({
            "normal_accepted_by": normal,
            "disordered_accepted_by": disordered,
        }),
        ok: normal == disordered && disordered.len() == 7,
    })
}

fn scenario_healed_partition_converges(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let proposal = propose(domain, graph, 1005, 'f')?;
    let all = set(&[
        "validator-0",
        "validator-1",
        "validator-2",
        "validator-3",
        "validator-4",
        "validator-5",
        "validator-6",
    ]);
    let accepted = accepted_by(domain, graph, &proposal, &all, &all)?;
    Ok(PartitionScenario {
        name: "healed_partition_replay_accepts_single_payload",
        partition: vec![vec![
            "validator-0",
            "validator-1",
            "validator-2",
            "validator-3",
            "validator-4",
            "validator-5",
            "validator-6",
        ]],
        fault: vec!["healed_partition_replay"],
        expected: "after heal, replaying one payload to all validators restores strong support",
        observed: json!({
            "accepted_by": accepted,
            "liveness_expected_after_heal": true,
        }),
        ok: accepted.len() == 7,
    })
}

fn scenario_healed_conflict_replay_yields_evidence(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<PartitionScenario, String> {
    let left = propose(domain, graph, 1006, '1')?;
    let right = propose(domain, graph, 1006, '2')?;
    let left_accept = build_rbc_accept(domain, &left, "validator-1", "")?;
    let right_accept = build_rbc_accept(domain, &right, "validator-2", "")?;
    let evidence =
        detect_rbc_conflicting_accept(domain, graph, &left, &left_accept, &right, &right_accept)?;
    Ok(PartitionScenario {
        name: "healed_conflicting_accept_replay_yields_evidence",
        partition: vec![
            vec!["validator-0", "validator-1", "validator-2"],
            vec!["validator-3", "validator-4", "validator-5", "validator-6"],
        ],
        fault: vec!["healed_partition_replay", "conflicting_accept"],
        expected: "conflicting accepts after heal produce linked conflict evidence instead of silent divergence",
        observed: json!({
            "evidence": evidence,
        }),
        ok: evidence.is_some(),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_three_four_partition(&domain, &graph)?,
        scenario_two_two_three_partition(&domain, &graph)?,
        scenario_single_isolated_validator(&domain, &graph)?,
        scenario_delay_reorder_duplicate(&domain, &graph)?,
        scenario_healed_partition_converges(&domain, &graph)?,
        scenario_healed_conflict_replay_yields_evidence(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-partition-simulation-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_partition_scenarios_passed": ok,
            "three_four_partition_safe": scenarios.iter().any(|scenario| scenario.name == "three_four_partition_has_no_conflicting_acceptance" && scenario.ok),
            "two_two_three_partition_safe": scenarios.iter().any(|scenario| scenario.name == "two_two_three_partition_has_no_progress_no_conflict" && scenario.ok),
            "single_validator_isolation_progress_profiled": scenarios.iter().any(|scenario| scenario.name == "single_validator_isolation_preserves_majority_progress" && scenario.ok),
            "delay_reorder_duplicate_deterministic": scenarios.iter().any(|scenario| scenario.name == "delay_reorder_duplicate_preserves_support_decision" && scenario.ok),
            "healed_partition_replay_converges_or_yields_evidence": scenarios.iter().any(|scenario| scenario.name == "healed_partition_replay_accepts_single_payload" && scenario.ok)
                && scenarios.iter().any(|scenario| scenario.name == "healed_conflicting_accept_replay_yields_evidence" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt partition simulation report failed".into())
    }
}
