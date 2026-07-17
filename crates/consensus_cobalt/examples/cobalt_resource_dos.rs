use postfiat_consensus_cobalt::{
    build_abba_init, build_abba_round_state, build_dabc_full_knowledge_check,
    build_essential_subset, build_rbc_echo, build_rbc_propose, build_rbc_ready, build_trust_graph,
    build_trust_view, detect_abba_round_equivocations, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo,
    validate_dabc_full_knowledge_checkpoint, CobaltDomain, DabcFullKnowledgeCheckpoint,
    DabcPendingPair, EssentialSubset, RbcEcho, RbcReady, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct ResourceScenario {
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

fn rejection(
    name: &'static str,
    attack: Vec<&'static str>,
    expected: &'static str,
    result: Result<(), String>,
) -> ResourceScenario {
    let error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    ResourceScenario {
        name,
        attack,
        expected,
        observed: json!({ "error": error }),
        ok: error.contains(expected),
    }
}

fn scenario_oversized_signatures(domain: &CobaltDomain, graph: &TrustGraph) -> ResourceScenario {
    let oversized = "a".repeat(8194);
    let rbc = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        10,
        root('a'),
        oversized.clone(),
    )
    .map(|_| ());
    let abba = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        root('b'),
        1,
        true,
        oversized.clone(),
    )
    .map(|_| ());
    let dabc = build_dabc_full_knowledge_check(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        1,
        Vec::new(),
        oversized,
    )
    .map(|_| ());
    let errors = [rbc, abba, dabc]
        .into_iter()
        .map(|result| {
            result
                .err()
                .unwrap_or_else(|| "unexpected success".to_string())
        })
        .collect::<Vec<_>>();
    ResourceScenario {
        name: "oversized_cobalt_signatures_rejected",
        attack: vec!["oversized_signature", "ml_dsa_fanout"],
        expected: "RBC, ABBA, and DABC reject signatures above the configured Cobalt hex bound",
        observed: json!({ "errors": errors }),
        ok: errors
            .iter()
            .all(|error| error.contains("signature exceeds maximum hex length")),
    }
}

fn scenario_malformed_payload(domain: &CobaltDomain, graph: &TrustGraph) -> ResourceScenario {
    rejection(
        "malformed_rbc_payload_hash_rejected",
        vec!["malformed_payload"],
        "RBC payload hash must be 96 lowercase hex characters",
        build_rbc_propose(
            domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            11,
            "not-a-payload-hash",
            "",
        )
        .map(|_| ()),
    )
}

fn scenario_pending_pairs_bound(domain: &CobaltDomain, graph: &TrustGraph) -> ResourceScenario {
    let pairs = (0..1025_u64)
        .map(|slot| DabcPendingPair {
            amendment_slot: slot,
            output_candidate_id: root('c'),
        })
        .collect::<Vec<_>>();
    rejection(
        "dabc_pending_pairs_bound_enforced",
        vec!["pending_pair_flood"],
        "too many pending pairs",
        build_dabc_full_knowledge_check(
            domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            1,
            pairs,
            "",
        )
        .map(|_| ()),
    )
}

fn scenario_checkpoint_checks_bound(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ResourceScenario, String> {
    let check = build_dabc_full_knowledge_check(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        1,
        Vec::new(),
        "",
    )?;
    let checkpoint = DabcFullKnowledgeCheckpoint {
        checkpoint_id: root('d'),
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        registry_root: graph.registry_root.clone(),
        trust_graph_root: graph.trust_graph_root.clone(),
        trust_view_id: view(graph, "validator-0")?.trust_view_id.clone(),
        local_validator: "validator-0".to_string(),
        interval_height: 1,
        wait_until_height: 1,
        covered_heights: vec![1],
        checks: vec![check; 65_537],
    };
    Ok(rejection(
        "dabc_checkpoint_check_count_bound_enforced",
        vec!["checkpoint_check_flood"],
        "too many checks",
        validate_dabc_full_knowledge_checkpoint(domain, graph, &checkpoint),
    ))
}

fn duplicate_messages<T: Clone>(messages: &[T], copies: usize) -> Vec<T> {
    let mut duplicated = Vec::with_capacity(messages.len() * copies);
    for _ in 0..copies {
        duplicated.extend_from_slice(messages);
    }
    duplicated
}

fn scenario_duplicate_rbc_dedup(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ResourceScenario, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        12,
        root('e'),
        "",
    )?;
    let echoes = validators(7)
        .iter()
        .map(|sender| build_rbc_echo(domain, &propose, sender, ""))
        .collect::<Result<Vec<RbcEcho>, _>>()?;
    let readies = validators(7)
        .iter()
        .map(|sender| build_rbc_ready(domain, &propose, sender, ""))
        .collect::<Result<Vec<RbcReady>, _>>()?;
    let duplicated_echoes = duplicate_messages(&echoes, 100);
    let duplicated_readies = duplicate_messages(&readies, 100);
    let echo_eval = evaluate_rbc_echo_support(
        domain,
        view(graph, "validator-0")?,
        &propose,
        &duplicated_echoes,
    )?;
    let ready_eval = evaluate_rbc_ready_support(
        domain,
        view(graph, "validator-0")?,
        &propose,
        &duplicated_readies,
    )?;
    let ready_allowed = rbc_ready_allowed_from_echo(&echo_eval);
    let accept_allowed = rbc_accept_allowed_from_ready(&ready_eval);
    Ok(ResourceScenario {
        name: "duplicate_rbc_messages_deduped_before_support",
        attack: vec!["duplicate_message_retention", "verification_fanout"],
        expected: "700 duplicate echoes/readies collapse to seven support senders",
        observed: json!({
            "duplicated_echo_count": duplicated_echoes.len(),
            "deduped_echo_support": echo_eval.support.len(),
            "duplicated_ready_count": duplicated_readies.len(),
            "deduped_ready_support": ready_eval.support.len(),
            "ready_allowed": ready_allowed,
            "accept_allowed": accept_allowed,
        }),
        ok: duplicated_echoes.len() == 700
            && duplicated_readies.len() == 700
            && echo_eval.support.len() == 7
            && ready_eval.support.len() == 7
            && ready_allowed
            && accept_allowed,
    })
}

fn scenario_abba_equivocation_dedup(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<ResourceScenario, String> {
    let agreement_id = root('f');
    let true_init = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let false_init = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    let mut state = build_abba_round_state(graph.trust_graph_root.clone(), agreement_id, 1)?;
    state.init_messages = duplicate_messages(&[true_init, false_init], 100);
    let evidence = detect_abba_round_equivocations(domain, &state)?;
    Ok(ResourceScenario {
        name: "duplicate_abba_equivocations_deduped",
        attack: vec!["duplicate_message_retention", "per_round_memory_growth"],
        expected: "duplicated ABBA equivocation messages produce one evidence item",
        observed: json!({
            "init_message_count": state.init_messages.len(),
            "equivocation_evidence_count": evidence.len(),
        }),
        ok: state.init_messages.len() == 200 && evidence.len() == 1,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_oversized_signatures(&domain, &graph),
        scenario_malformed_payload(&domain, &graph),
        scenario_pending_pairs_bound(&domain, &graph),
        scenario_checkpoint_checks_bound(&domain, &graph)?,
        scenario_duplicate_rbc_dedup(&domain, &graph)?,
        scenario_abba_equivocation_dedup(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-resource-dos-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_resource_dos_scenarios_passed": ok,
            "oversized_signatures_rejected": scenarios.iter().any(|scenario| scenario.name == "oversized_cobalt_signatures_rejected" && scenario.ok),
            "malformed_payload_rejected": scenarios.iter().any(|scenario| scenario.name == "malformed_rbc_payload_hash_rejected" && scenario.ok),
            "dabc_pending_pair_bound_enforced": scenarios.iter().any(|scenario| scenario.name == "dabc_pending_pairs_bound_enforced" && scenario.ok),
            "dabc_checkpoint_check_bound_enforced": scenarios.iter().any(|scenario| scenario.name == "dabc_checkpoint_check_count_bound_enforced" && scenario.ok),
            "duplicate_rbc_messages_deduped": scenarios.iter().any(|scenario| scenario.name == "duplicate_rbc_messages_deduped_before_support" && scenario.ok),
            "duplicate_abba_equivocations_deduped": scenarios.iter().any(|scenario| scenario.name == "duplicate_abba_equivocations_deduped" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt resource DoS report failed".into())
    }
}
