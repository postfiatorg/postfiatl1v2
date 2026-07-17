use postfiat_consensus_cobalt::{
    analyze_trust_graph, bind_dabc_ratification_to_transaction_network_membership,
    build_cobalt_block_membership_binding, build_essential_subset, build_mvba_valid_input_set,
    build_rbc_accept, build_rbc_propose, build_transaction_network_membership, build_trust_graph,
    build_trust_view, cobalt_block_membership_binding_id, mvba_candidate_from_rbc_accept,
    ratify_dabc_amendment, transaction_network_membership_payload_hash,
    validate_cobalt_block_against_transaction_network_transition,
    validate_transaction_network_transition, CobaltBlockMembershipBinding, CobaltDomain,
    CobaltFaultModel, EssentialSubset, TransactionNetworkMembership, TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct MembershipRaceScenario {
    name: &'static str,
    race: Vec<&'static str>,
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

fn fixture() -> Result<
    (
        CobaltDomain,
        TrustGraph,
        TransactionNetworkMembership,
        TransactionNetworkMembership,
    ),
    String,
> {
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
    let previous = build_transaction_network_membership(&domain, &graph, 3, validators(5), 4, 20)?;
    let next = build_transaction_network_membership(
        &domain,
        &graph,
        4,
        validators(6).into_iter().skip(1).collect(),
        4,
        30,
    )?;
    Ok((domain, graph, previous, next))
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

fn rejection_scenario(
    name: &'static str,
    race: Vec<&'static str>,
    expected_rejection: &'static str,
    result: Result<(), String>,
) -> MembershipRaceScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    MembershipRaceScenario {
        name,
        race,
        expected_rejection,
        observed: json!({ "error": observed_error }),
        ok: observed_error.contains(expected_rejection),
    }
}

fn manual_binding(
    domain: &CobaltDomain,
    membership: &TransactionNetworkMembership,
    block_hash: impl Into<String>,
    block_height: u64,
    proposer: impl Into<String>,
) -> Result<CobaltBlockMembershipBinding, String> {
    let mut binding = CobaltBlockMembershipBinding {
        binding_id: String::new(),
        block_hash: block_hash.into(),
        block_height,
        proposer: proposer.into(),
        registry_root: membership.registry_root.clone(),
        trust_graph_root: membership.trust_graph_root.clone(),
        governance_epoch: membership.governance_epoch,
        transaction_network_id: membership.transaction_network_id.clone(),
    };
    binding.binding_id = cobalt_block_membership_binding_id(domain, &binding)?;
    Ok(binding)
}

fn scenario_old_set_after_activation(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let binding = build_cobalt_block_membership_binding(
        domain,
        previous,
        root('a'),
        next.activation_height,
        "validator-0",
    )?;
    Ok(rejection_scenario(
        "old_set_block_after_activation_rejected",
        vec!["old_set_after_activation"],
        "outside transaction network",
        validate_cobalt_block_against_transaction_network_transition(
            domain, previous, next, &binding,
        ),
    ))
}

fn scenario_new_set_before_activation(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let binding = manual_binding(
        domain,
        next,
        root('b'),
        next.activation_height - 1,
        "validator-1",
    )?;
    Ok(rejection_scenario(
        "new_set_block_before_activation_rejected",
        vec!["new_set_before_activation"],
        "transaction network metadata mismatch",
        validate_cobalt_block_against_transaction_network_transition(
            domain, previous, next, &binding,
        ),
    ))
}

fn scenario_mixed_old_new_metadata(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let mut binding = manual_binding(
        domain,
        next,
        root('c'),
        next.activation_height,
        "validator-1",
    )?;
    binding.transaction_network_id = previous.transaction_network_id.clone();
    binding.binding_id = cobalt_block_membership_binding_id(domain, &binding)?;
    Ok(rejection_scenario(
        "mixed_old_new_block_membership_metadata_rejected",
        vec!["mixed_old_new_certificate"],
        "transaction network metadata mismatch",
        validate_cobalt_block_against_transaction_network_transition(
            domain, previous, next, &binding,
        ),
    ))
}

fn scenario_stale_transaction_network_id(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let mut binding = manual_binding(
        domain,
        next,
        root('d'),
        next.activation_height + 1,
        "validator-2",
    )?;
    binding.transaction_network_id = previous.transaction_network_id.clone();
    binding.binding_id = cobalt_block_membership_binding_id(domain, &binding)?;
    Ok(rejection_scenario(
        "stale_transaction_network_id_rejected",
        vec!["stale_transaction_network_id"],
        "transaction network metadata mismatch",
        validate_cobalt_block_against_transaction_network_transition(
            domain, previous, next, &binding,
        ),
    ))
}

fn scenario_wrong_graph_root_binding(
    domain: &CobaltDomain,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let mut binding = manual_binding(
        domain,
        next,
        root('e'),
        next.activation_height,
        "validator-3",
    )?;
    binding.trust_graph_root = root('9');
    binding.binding_id = cobalt_block_membership_binding_id(domain, &binding)?;
    Ok(rejection_scenario(
        "wrong_graph_root_block_binding_rejected",
        vec!["wrong_graph_root"],
        "transaction network metadata mismatch",
        validate_cobalt_block_against_transaction_network_transition(
            domain, previous, next, &binding,
        ),
    ))
}

fn scenario_transition_activation_must_increase(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    previous: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let nonadvancing =
        build_transaction_network_membership(domain, graph, 4, validators(6), 4, 20)?;
    Ok(rejection_scenario(
        "transition_activation_height_must_increase",
        vec!["nonadvancing_activation_height"],
        "activation height must increase",
        validate_transaction_network_transition(previous, &nonadvancing),
    ))
}

fn scenario_transition_epoch_must_increase(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    previous: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let stale_epoch = build_transaction_network_membership(
        domain,
        graph,
        previous.governance_epoch,
        validators(6),
        4,
        30,
    )?;
    Ok(rejection_scenario(
        "transition_governance_epoch_must_increase",
        vec!["stale_governance_epoch"],
        "governance epoch must increase",
        validate_transaction_network_transition(previous, &stale_epoch),
    ))
}

fn scenario_dabc_payload_binding_rejects_stale_membership(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    previous: &TransactionNetworkMembership,
    next: &TransactionNetworkMembership,
) -> Result<MembershipRaceScenario, String> {
    let previous_payload = transaction_network_membership_payload_hash(domain, previous)?;
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        901,
        previous_payload,
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, "validator-1", "")?;
    let candidate = mvba_candidate_from_rbc_accept(domain, &propose, &accept)?;
    let input_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-1")?,
        root('f'),
        vec![candidate],
    )?;
    let ratified = ratify_dabc_amendment(domain, graph, &input_set, None, next.activation_height)?;
    Ok(rejection_scenario(
        "dabc_payload_binding_rejects_stale_membership",
        vec!["mixed_old_new_certificate", "stale_membership_payload"],
        "payload hash mismatch",
        bind_dabc_ratification_to_transaction_network_membership(
            domain, graph, &ratified, None, next,
        )
        .map(|_| ()),
    ))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph, previous, next) = fixture()?;
    let scenarios = vec![
        scenario_old_set_after_activation(&domain, &previous, &next)?,
        scenario_new_set_before_activation(&domain, &previous, &next)?,
        scenario_mixed_old_new_metadata(&domain, &previous, &next)?,
        scenario_stale_transaction_network_id(&domain, &previous, &next)?,
        scenario_wrong_graph_root_binding(&domain, &previous, &next)?,
        scenario_transition_activation_must_increase(&domain, &graph, &previous)?,
        scenario_transition_epoch_must_increase(&domain, &graph, &previous)?,
        scenario_dabc_payload_binding_rejects_stale_membership(&domain, &graph, &previous, &next)?,
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
        "schema": "postfiat-testnet-cobalt-membership-race-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "previous_transaction_network_id": previous.transaction_network_id,
        "next_transaction_network_id": next.transaction_network_id,
        "transition_activation_height": next.activation_height,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_membership_race_scenarios_passed": ok,
            "old_set_after_activation_rejected": scenarios.iter().any(|scenario| scenario.name == "old_set_block_after_activation_rejected" && scenario.ok),
            "new_set_before_activation_rejected": scenarios.iter().any(|scenario| scenario.name == "new_set_block_before_activation_rejected" && scenario.ok),
            "mixed_old_new_metadata_rejected": scenarios.iter().any(|scenario| scenario.name == "mixed_old_new_block_membership_metadata_rejected" && scenario.ok),
            "stale_transaction_network_id_rejected": scenarios.iter().any(|scenario| scenario.name == "stale_transaction_network_id_rejected" && scenario.ok),
            "wrong_graph_root_rejected": scenarios.iter().any(|scenario| scenario.name == "wrong_graph_root_block_binding_rejected" && scenario.ok),
            "dabc_payload_binding_rejects_stale_membership": scenarios.iter().any(|scenario| scenario.name == "dabc_payload_binding_rejects_stale_membership" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt membership race report failed".into())
    }
}
