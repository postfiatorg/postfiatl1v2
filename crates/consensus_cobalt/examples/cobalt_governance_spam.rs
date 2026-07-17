use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_dabc_full_knowledge_check, build_dabc_full_knowledge_checkpoint,
    build_dabc_replay_bundle, build_essential_subset, build_mvba_valid_input_set, build_rbc_accept,
    build_rbc_propose, build_trust_graph, build_trust_view, dabc_ratification_id,
    mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    validate_dabc_activation_with_full_knowledge, validate_dabc_ratified_amendment, CobaltDomain,
    CobaltFaultModel, DabcPendingPair, DabcRatifiedAmendment, EssentialSubset, MvbaCandidate,
    MvbaValidInputSet, TrustGraph, MAX_MVBA_CANDIDATES_PER_SET,
};
use postfiat_crypto_provider::hash_hex;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct GovernanceSpamScenario {
    name: &'static str,
    attack: Vec<&'static str>,
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

fn payload_hash(index: usize) -> String {
    hash_hex(
        "postfiat.test.cobalt.governance_spam.payload.v1",
        &index.to_le_bytes(),
    )
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

fn candidate_for_slot_payload(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    proposer: &str,
    acceptor: &str,
    amendment_slot: u64,
    payload_hash: String,
) -> Result<MvbaCandidate, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        proposer,
        amendment_slot,
        payload_hash,
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, acceptor, "")?;
    mvba_candidate_from_rbc_accept(domain, &propose, &accept)
}

fn candidate_for_slot(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    proposer: &str,
    acceptor: &str,
    amendment_slot: u64,
    payload_byte: char,
) -> Result<MvbaCandidate, String> {
    candidate_for_slot_payload(
        domain,
        graph,
        proposer,
        acceptor,
        amendment_slot,
        root(payload_byte),
    )
}

fn many_candidates(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    amendment_slot: u64,
    count: usize,
) -> Result<Vec<MvbaCandidate>, String> {
    let validators = validators(7);
    (0..count)
        .map(|index| {
            let proposer = validators[index % validators.len()].as_str();
            let acceptor = validators[(index + 1) % validators.len()].as_str();
            candidate_for_slot_payload(
                domain,
                graph,
                proposer,
                acceptor,
                amendment_slot,
                payload_hash(index),
            )
        })
        .collect()
}

fn input_set_for_candidate(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    local_validator: &str,
    agreement_byte: char,
    candidate: MvbaCandidate,
) -> Result<MvbaValidInputSet, String> {
    build_mvba_valid_input_set(
        domain,
        view(graph, local_validator)?,
        root(agreement_byte),
        vec![candidate],
    )
}

fn valid_chain(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(DabcRatifiedAmendment, DabcRatifiedAmendment), String> {
    let first_candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 11, 'a')?;
    let first_set = input_set_for_candidate(domain, graph, "validator-1", 'd', first_candidate)?;
    let first = ratify_dabc_amendment(domain, graph, &first_set, None, 20)?;

    let second_candidate =
        candidate_for_slot(domain, graph, "validator-3", "validator-2", 12, 'c')?;
    let second_set = input_set_for_candidate(domain, graph, "validator-2", 'e', second_candidate)?;
    let second = ratify_dabc_amendment(domain, graph, &second_set, Some(&first), 21)?;
    Ok((first, second))
}

fn rejection_scenario(
    name: &'static str,
    attack: Vec<&'static str>,
    expected: &'static str,
    result: Result<(), String>,
) -> GovernanceSpamScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    GovernanceSpamScenario {
        name,
        attack,
        expected,
        observed: json!({ "error": observed_error }),
        ok: observed_error.contains(expected),
    }
}

fn scenario_many_amendments_deterministic(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let local_view = view(graph, "validator-1")?;
    let candidates = many_candidates(domain, graph, 100, 64)?;
    let reversed = candidates.iter().cloned().rev().collect::<Vec<_>>();
    let first = build_mvba_valid_input_set(domain, local_view, root('a'), candidates)?;
    let second = build_mvba_valid_input_set(domain, local_view, root('a'), reversed)?;
    let output_matches = first.output_candidate_id == second.output_candidate_id;
    Ok(GovernanceSpamScenario {
        name: "many_governance_amendments_select_deterministically",
        attack: vec!["amendment_flood_under_bound", "message_reorder"],
        expected: "many candidates under the configured bound sort to one deterministic output",
        observed: json!({
            "candidate_count": first.candidates.len(),
            "max_mvba_candidates_per_set": MAX_MVBA_CANDIDATES_PER_SET,
            "first_output_candidate_id": first.output_candidate_id,
            "second_output_candidate_id": second.output_candidate_id,
        }),
        ok: first.candidates.len() == 64 && output_matches,
    })
}

fn scenario_candidate_flood_bound(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let candidates = many_candidates(domain, graph, 101, MAX_MVBA_CANDIDATES_PER_SET + 1)?;
    Ok(rejection_scenario(
        "mvba_candidate_flood_bound_enforced",
        vec!["amendment_flood", "resource_policy"],
        "MVBA valid input set has too many candidates",
        build_mvba_valid_input_set(domain, view(graph, "validator-1")?, root('b'), candidates)
            .map(|_| ()),
    ))
}

fn scenario_raw_candidate_flood_bound(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 102, 'd')?;
    let local_view = view(graph, "validator-1")?;
    let raw_set = MvbaValidInputSet {
        trust_view_id: local_view.trust_view_id.clone(),
        local_validator: local_view.validator.clone(),
        agreement_id: root('c'),
        candidates: vec![candidate; MAX_MVBA_CANDIDATES_PER_SET + 1],
        output_candidate_id: root('d'),
    };
    Ok(rejection_scenario(
        "raw_mvba_candidate_flood_rejected_at_ratification",
        vec!["amendment_flood", "manual_replay_bundle"],
        "MVBA valid input set has too many candidates",
        ratify_dabc_amendment(domain, graph, &raw_set, None, 30).map(|_| ()),
    ))
}

fn scenario_duplicate_slots_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let first_candidate =
        candidate_for_slot(domain, graph, "validator-0", "validator-1", 110, 'e')?;
    let first_set = input_set_for_candidate(domain, graph, "validator-1", 'e', first_candidate)?;
    let first = ratify_dabc_amendment(domain, graph, &first_set, None, 40)?;
    let duplicate_candidate =
        candidate_for_slot(domain, graph, "validator-2", "validator-3", 110, 'f')?;
    let duplicate_set =
        input_set_for_candidate(domain, graph, "validator-2", 'f', duplicate_candidate)?;
    let mut duplicate = ratify_dabc_amendment(domain, graph, &duplicate_set, None, 41)?;
    duplicate.sequence = 2;
    duplicate.parent_ratification_id = first.ratification_id.clone();
    duplicate.ratification_id = dabc_ratification_id(domain, &duplicate)?;
    Ok(rejection_scenario(
        "duplicate_amendment_slots_rejected",
        vec!["duplicate_slots", "amendment_replay"],
        "DABC ratified chain contains duplicate amendment slot",
        build_dabc_replay_bundle(
            domain,
            graph,
            vec![first, duplicate],
            Vec::new(),
            Vec::new(),
        )
        .map(|_| ()),
    ))
}

fn scenario_future_slot_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let (first, second) = valid_chain(domain, graph)?;
    let support = validators(7).into_iter().take(5).collect::<Vec<_>>();
    let mut checks = Vec::new();
    for height in [10_u64, 20_u64] {
        for sender in &support {
            let pending_pairs = if height == 20 {
                vec![DabcPendingPair {
                    amendment_slot: 9_999,
                    output_candidate_id: root('8'),
                }]
            } else {
                Vec::new()
            };
            checks.push(build_dabc_full_knowledge_check(
                domain,
                graph.trust_graph_root.clone(),
                sender,
                height,
                pending_pairs,
                "",
            )?);
        }
    }
    let checkpoint = build_dabc_full_knowledge_checkpoint(
        domain,
        graph,
        "validator-1",
        10,
        second.activation_height,
        checks,
    )?;
    let ratified_chain = vec![first.clone(), second.clone()];
    Ok(rejection_scenario(
        "future_pending_amendment_slot_rejected",
        vec!["future_slot", "unratified_pending_pair"],
        "DABC full-knowledge pending slot 9999 is not ratified",
        validate_dabc_activation_with_full_knowledge(
            domain,
            graph,
            &ratified_chain,
            &second,
            &checkpoint,
        )
        .map(|_| ()),
    ))
}

fn scenario_invalid_parent_chain_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<GovernanceSpamScenario, String> {
    let (first, second) = valid_chain(domain, graph)?;
    let mut tampered = second;
    tampered.parent_ratification_id = root('6');
    tampered.ratification_id = dabc_ratification_id(domain, &tampered)?;
    Ok(rejection_scenario(
        "invalid_parent_chain_rejected",
        vec!["invalid_parent_chain", "conflicting_parent_hash"],
        "DABC ratified amendment parent mismatch",
        validate_dabc_ratified_amendment(domain, graph, &tampered, Some(&first)),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_many_amendments_deterministic(&domain, &graph)?,
        scenario_candidate_flood_bound(&domain, &graph)?,
        scenario_raw_candidate_flood_bound(&domain, &graph)?,
        scenario_duplicate_slots_rejected(&domain, &graph)?,
        scenario_future_slot_rejected(&domain, &graph)?,
        scenario_invalid_parent_chain_rejected(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-governance-spam-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "max_mvba_candidates_per_set": MAX_MVBA_CANDIDATES_PER_SET,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_governance_spam_scenarios_passed": ok,
            "many_amendments_deterministic": scenarios.iter().any(|scenario| scenario.name == "many_governance_amendments_select_deterministically" && scenario.ok),
            "candidate_flood_bound_enforced": scenarios.iter().any(|scenario| scenario.name == "mvba_candidate_flood_bound_enforced" && scenario.ok)
                && scenarios.iter().any(|scenario| scenario.name == "raw_mvba_candidate_flood_rejected_at_ratification" && scenario.ok),
            "duplicate_slots_rejected": scenarios.iter().any(|scenario| scenario.name == "duplicate_amendment_slots_rejected" && scenario.ok),
            "future_slots_rejected": scenarios.iter().any(|scenario| scenario.name == "future_pending_amendment_slot_rejected" && scenario.ok),
            "invalid_parent_chains_rejected": scenarios.iter().any(|scenario| scenario.name == "invalid_parent_chain_rejected" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt governance spam report failed".into())
    }
}
