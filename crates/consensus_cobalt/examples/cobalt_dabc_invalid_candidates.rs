use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_dabc_full_knowledge_check, build_dabc_full_knowledge_checkpoint,
    build_dabc_replay_bundle, build_essential_subset, build_mvba_valid_input_set, build_rbc_accept,
    build_rbc_propose, build_trust_graph, build_trust_view, dabc_activation_evidence_id,
    dabc_ratification_id, mvba_candidate_from_rbc_accept, mvba_candidate_id, ratify_dabc_amendment,
    rbc_accept_message_id, validate_dabc_activation_with_full_knowledge,
    validate_dabc_ratified_amendment, verify_dabc_replay_bundle, CobaltDomain, CobaltFaultModel,
    DabcPendingPair, DabcRatifiedAmendment, DabcReplayBundle, EssentialSubset, MvbaCandidate,
    MvbaValidInputSet, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct DabcScenario {
    name: &'static str,
    attack: Vec<&'static str>,
    expected_rejection: &'static str,
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

fn candidate_for_slot(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    proposer: &str,
    acceptor: &str,
    amendment_slot: u64,
    payload_byte: char,
) -> Result<MvbaCandidate, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        proposer,
        amendment_slot,
        root(payload_byte),
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, acceptor, "")?;
    mvba_candidate_from_rbc_accept(domain, &propose, &accept)
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

fn valid_replay_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcReplayBundle, String> {
    let (first, second) = valid_chain(domain, graph)?;
    let support = validators(7).into_iter().take(5).collect::<Vec<_>>();
    let mut checks = Vec::new();
    for height in [10_u64, 20_u64] {
        for sender in &support {
            let pending_pairs = if height == 20 {
                vec![DabcPendingPair {
                    amendment_slot: second.amendment_slot,
                    output_candidate_id: second.output_candidate_id.clone(),
                }]
            } else {
                vec![DabcPendingPair {
                    amendment_slot: first.amendment_slot,
                    output_candidate_id: first.output_candidate_id.clone(),
                }]
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
    let first_activation = validate_dabc_activation_with_full_knowledge(
        domain,
        graph,
        &ratified_chain,
        &first,
        &checkpoint,
    )?;
    let second_activation = validate_dabc_activation_with_full_knowledge(
        domain,
        graph,
        &ratified_chain,
        &second,
        &checkpoint,
    )?;
    build_dabc_replay_bundle(
        domain,
        graph,
        ratified_chain,
        vec![checkpoint],
        vec![second_activation, first_activation],
    )
}

fn rejection_scenario(
    name: &'static str,
    attack: Vec<&'static str>,
    expected_rejection: &'static str,
    result: Result<(), String>,
) -> DabcScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    DabcScenario {
        name,
        attack,
        expected_rejection,
        observed: json!({ "error": observed_error }),
        ok: observed_error.contains(expected_rejection),
    }
}

fn scenario_invalid_rbc_accept(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        31,
        root('a'),
        "",
    )?;
    let mut accept = build_rbc_accept(domain, &propose, "validator-1", "")?;
    accept.payload_hash = root('b');
    accept.message_id = rbc_accept_message_id(&accept)?;
    Ok(rejection_scenario(
        "invalid_rbc_accept_rejected_before_mvba_candidate",
        vec!["invalid_rbc_accept", "payload_mismatch"],
        "RBC accept does not match RBC propose",
        mvba_candidate_from_rbc_accept(domain, &propose, &accept).map(|_| ()),
    ))
}

fn scenario_candidate_id_mismatch(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let mut candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 32, 'c')?;
    candidate.candidate_id = root('9');
    Ok(rejection_scenario(
        "conflicting_candidate_id_rejected",
        vec!["conflicting_candidate_id"],
        "MVBA candidate id mismatch",
        build_mvba_valid_input_set(
            domain,
            view(graph, "validator-1")?,
            root('d'),
            vec![candidate],
        )
        .map(|_| ()),
    ))
}

fn scenario_stale_propose_id_payload_mismatch(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let mut candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 33, 'd')?;
    candidate.payload_hash = root('e');
    candidate.candidate_id = mvba_candidate_id(domain, &candidate)?;
    Ok(rejection_scenario(
        "stale_propose_id_payload_mismatch_rejected",
        vec!["mismatched_payload_hash", "stale_propose_message_id"],
        "MVBA candidate propose message id mismatch",
        build_mvba_valid_input_set(
            domain,
            view(graph, "validator-1")?,
            root('e'),
            vec![candidate],
        )
        .map(|_| ()),
    ))
}

fn scenario_duplicate_raw_candidates(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 34, 'e')?;
    let local_view = view(graph, "validator-1")?;
    let raw_set = MvbaValidInputSet {
        trust_view_id: local_view.trust_view_id.clone(),
        local_validator: local_view.validator.clone(),
        agreement_id: root('f'),
        candidates: vec![candidate.clone(), candidate.clone()],
        output_candidate_id: candidate.candidate_id,
    };
    Ok(rejection_scenario(
        "duplicate_raw_mvba_candidates_rejected",
        vec!["duplicate_candidates"],
        "MVBA valid input set candidates must be sorted unique",
        ratify_dabc_amendment(domain, graph, &raw_set, None, 40).map(|_| ()),
    ))
}

fn scenario_output_candidate_not_in_set(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 35, 'f')?;
    let mut input_set = input_set_for_candidate(domain, graph, "validator-1", 'a', candidate)?;
    input_set.output_candidate_id = root('1');
    Ok(rejection_scenario(
        "output_candidate_id_not_in_valid_set_rejected",
        vec!["conflicting_output_candidate_id"],
        "MVBA output candidate id is not in valid input set",
        ratify_dabc_amendment(domain, graph, &input_set, None, 41).map(|_| ()),
    ))
}

fn scenario_conflicting_parent_hash(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let (first, second) = valid_chain(domain, graph)?;
    let mut tampered = second.clone();
    tampered.parent_ratification_id = root('6');
    tampered.ratification_id = dabc_ratification_id(domain, &tampered)?;
    Ok(rejection_scenario(
        "conflicting_parent_hash_rejected",
        vec!["conflicting_parent_hash"],
        "DABC ratified amendment parent mismatch",
        validate_dabc_ratified_amendment(domain, graph, &tampered, Some(&first)),
    ))
}

fn scenario_skipped_amendment_slot(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let first_candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 50, 'a')?;
    let first_set = input_set_for_candidate(domain, graph, "validator-1", 'b', first_candidate)?;
    let first = ratify_dabc_amendment(domain, graph, &first_set, None, 60)?;
    let skipped_candidate =
        candidate_for_slot(domain, graph, "validator-3", "validator-2", 52, 'c')?;
    let skipped_set =
        input_set_for_candidate(domain, graph, "validator-2", 'c', skipped_candidate)?;
    Ok(rejection_scenario(
        "skipped_amendment_slot_rejected",
        vec!["skipped_slot"],
        "DABC ratified amendment slot must extend previous",
        ratify_dabc_amendment(domain, graph, &skipped_set, Some(&first), 61).map(|_| ()),
    ))
}

fn scenario_zero_activation_height(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 70, 'd')?;
    let input_set = input_set_for_candidate(domain, graph, "validator-1", 'd', candidate)?;
    Ok(rejection_scenario(
        "zero_activation_height_rejected_at_ratification",
        vec!["wrong_activation_height"],
        "DABC ratified amendment activation height must be nonzero",
        ratify_dabc_amendment(domain, graph, &input_set, None, 0).map(|_| ()),
    ))
}

fn scenario_tampered_activation_evidence_height(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<DabcScenario, String> {
    let mut bundle = valid_replay_bundle(domain, graph)?;
    let first_evidence = bundle
        .activation_evidence
        .first_mut()
        .ok_or_else(|| "valid bundle missing activation evidence".to_string())?;
    first_evidence.activation_height = first_evidence
        .activation_height
        .checked_add(1)
        .ok_or_else(|| "activation height overflow".to_string())?;
    first_evidence.activation_id = dabc_activation_evidence_id(domain, first_evidence)?;
    Ok(rejection_scenario(
        "tampered_activation_evidence_height_rejected",
        vec!["wrong_activation_height", "activation_evidence_mismatch"],
        "DABC replay bundle activation evidence mismatch",
        verify_dabc_replay_bundle(domain, graph, &bundle).map(|_| ()),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_invalid_rbc_accept(&domain, &graph)?,
        scenario_candidate_id_mismatch(&domain, &graph)?,
        scenario_stale_propose_id_payload_mismatch(&domain, &graph)?,
        scenario_duplicate_raw_candidates(&domain, &graph)?,
        scenario_output_candidate_not_in_set(&domain, &graph)?,
        scenario_conflicting_parent_hash(&domain, &graph)?,
        scenario_skipped_amendment_slot(&domain, &graph)?,
        scenario_zero_activation_height(&domain, &graph)?,
        scenario_tampered_activation_evidence_height(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-dabc-invalid-candidates-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_dabc_invalid_candidate_scenarios_passed": ok,
            "invalid_rbc_accept_rejected": scenarios.iter().any(|scenario| scenario.name == "invalid_rbc_accept_rejected_before_mvba_candidate" && scenario.ok),
            "candidate_self_consistency_enforced": scenarios.iter().any(|scenario| scenario.name == "stale_propose_id_payload_mismatch_rejected" && scenario.ok),
            "skipped_slots_rejected": scenarios.iter().any(|scenario| scenario.name == "skipped_amendment_slot_rejected" && scenario.ok),
            "wrong_activation_heights_rejected": scenarios.iter().any(|scenario| scenario.name == "zero_activation_height_rejected_at_ratification" && scenario.ok)
                && scenarios.iter().any(|scenario| scenario.name == "tampered_activation_evidence_height_rejected" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt DABC invalid-candidate report failed".into())
    }
}
