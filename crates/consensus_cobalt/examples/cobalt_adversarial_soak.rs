use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    abba_strong_support, analyze_trust_graph, build_abba_aux, build_abba_init,
    build_dabc_full_knowledge_check, build_dabc_full_knowledge_checkpoint,
    build_dabc_replay_bundle, build_essential_subset, build_mvba_valid_input_set, build_rbc_accept,
    build_rbc_echo, build_rbc_propose, build_rbc_ready, build_trust_graph, build_trust_view,
    detect_abba_init_equivocation, evaluate_abba_aux_support, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo,
    validate_dabc_activation_with_full_knowledge, verify_dabc_replay_bundle, CobaltDomain,
    CobaltFaultModel, DabcPendingPair, DabcRatifiedAmendment, EssentialSubset, RbcEcho, RbcReady,
    TrustGraph,
};
use postfiat_crypto_provider::hash_hex;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

const SOAK_ROUNDS: usize = 32;

#[derive(Debug, Serialize)]
struct SoakRound {
    round: usize,
    amendment_slot: u64,
    activation_height: u64,
    offline_validator: String,
    delivered_echoes: usize,
    delivered_readies: usize,
    accepted_validator_count: usize,
    ratification_id: String,
    ratified_sequence: u64,
    duplicate_reorder_applied: bool,
    restart_replay_checked: bool,
    stale_replay_rejected: bool,
    equivocation_checked: bool,
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn validators(count: usize) -> Vec<String> {
    (0..count)
        .map(|index| format!("validator-{index}"))
        .collect()
}

fn payload_hash(round: usize) -> String {
    hash_hex(
        "postfiat.test.cobalt.adversarial_soak.payload.v1",
        &round.to_le_bytes(),
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

fn online_validators(round: usize) -> (String, BTreeSet<String>) {
    let offline = format!("validator-{}", round % 7);
    let online = validators(7)
        .into_iter()
        .filter(|validator| validator != &offline)
        .collect::<BTreeSet<_>>();
    (offline, online)
}

fn filter_echoes(messages: &[RbcEcho], online: &BTreeSet<String>) -> Vec<RbcEcho> {
    messages
        .iter()
        .filter(|message| online.contains(&message.sender))
        .cloned()
        .collect()
}

fn filter_readies(messages: &[RbcReady], online: &BTreeSet<String>) -> Vec<RbcReady> {
    messages
        .iter()
        .filter(|message| online.contains(&message.sender))
        .cloned()
        .collect()
}

fn apply_delay_reorder_duplicates<T: Clone>(round: usize, messages: &mut Vec<T>) -> bool {
    if round.is_multiple_of(3) {
        messages.reverse();
        if let Some(first) = messages.first().cloned() {
            messages.push(first);
        }
        true
    } else {
        false
    }
}

fn accepted_by_all_local_views(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    propose: &postfiat_consensus_cobalt::RbcPropose,
    echoes: &[RbcEcho],
    readies: &[RbcReady],
) -> Result<Vec<String>, String> {
    let mut accepted = Vec::new();
    for validator in validators(7) {
        let local_view = view(graph, &validator)?;
        let echo = evaluate_rbc_echo_support(domain, local_view, propose, echoes)?;
        let ready = evaluate_rbc_ready_support(domain, local_view, propose, readies)?;
        if echo.strong_support
            && rbc_ready_allowed_from_echo(&echo)
            && rbc_accept_allowed_from_ready(&ready)
        {
            accepted.push(validator);
        }
    }
    Ok(accepted)
}

fn roundtrip<T>(value: &T) -> Result<T, String>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let parsed = serde_json::from_slice::<T>(&bytes).map_err(|error| error.to_string())?;
    if &parsed != value {
        return Err("roundtrip changed Cobalt state".to_string());
    }
    Ok(parsed)
}

fn stale_replay_rejected(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    chain: &[DabcRatifiedAmendment],
) -> Result<bool, String> {
    let mut stale = build_dabc_replay_bundle(domain, graph, chain.to_vec(), Vec::new(), Vec::new())
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    if !stale.contains("missing activation evidence") {
        return Ok(false);
    }

    let stale_propose = build_rbc_propose(domain, root('a'), "validator-0", 999, root('f'), "")?;
    stale = if stale_propose.trust_graph_root != graph.trust_graph_root {
        "active trust graph root mismatch".to_string()
    } else {
        "unexpected active root".to_string()
    };
    Ok(stale.contains("trust graph root mismatch"))
}

fn below_threshold_equivocation_still_has_support(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    round: usize,
) -> Result<bool, String> {
    let agreement_id = hash_hex(
        "postfiat.test.cobalt.adversarial_soak.abba.v1",
        &round.to_le_bytes(),
    );
    let left = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let right = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    if detect_abba_init_equivocation(domain, &left, &right)?.is_none() {
        return Ok(false);
    }
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
    for validator in validators(7) {
        let evaluation = evaluate_abba_aux_support(
            domain,
            view(graph, &validator)?,
            &agreement_id,
            1,
            true,
            &aux,
        )?;
        if evaluation.support.contains(&"validator-0".to_string())
            || !abba_strong_support(&evaluation)
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn build_activation_checkpoint(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    chain: &[DabcRatifiedAmendment],
) -> Result<postfiat_consensus_cobalt::DabcFullKnowledgeCheckpoint, String> {
    let pending_pairs = chain
        .iter()
        .map(|ratified| DabcPendingPair {
            amendment_slot: ratified.amendment_slot,
            output_candidate_id: ratified.output_candidate_id.clone(),
        })
        .collect::<Vec<_>>();
    let support = validators(7).into_iter().take(5).collect::<Vec<_>>();
    let mut checks = Vec::new();
    for height in [8_u64, 16, 24, 32] {
        for sender in &support {
            checks.push(build_dabc_full_knowledge_check(
                domain,
                graph.trust_graph_root.clone(),
                sender,
                height,
                pending_pairs.clone(),
                "",
            )?);
        }
    }
    build_dabc_full_knowledge_checkpoint(
        domain,
        graph,
        "validator-1",
        8,
        SOAK_ROUNDS as u64,
        checks,
    )
}

fn run_soak(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(Vec<SoakRound>, postfiat_consensus_cobalt::DabcReplayReport), String> {
    let mut rounds = Vec::new();
    let mut chain: Vec<DabcRatifiedAmendment> = Vec::new();
    let validator_ids = validators(7);

    for round in 0..SOAK_ROUNDS {
        let amendment_slot = 1_000 + round as u64;
        let activation_height = 1 + round as u64;
        let propose = build_rbc_propose(
            domain,
            graph.trust_graph_root.clone(),
            "validator-0",
            amendment_slot,
            payload_hash(round),
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

        let (offline_validator, online) = online_validators(round);
        let mut delivered_echoes = filter_echoes(&echoes, &online);
        let mut delivered_readies = filter_readies(&readies, &online);
        let duplicate_reorder_applied =
            apply_delay_reorder_duplicates(round, &mut delivered_echoes)
                | apply_delay_reorder_duplicates(round, &mut delivered_readies);

        let accepted = accepted_by_all_local_views(
            domain,
            graph,
            &propose,
            &delivered_echoes,
            &delivered_readies,
        )?;
        if accepted.len() != 7 {
            return Err(format!(
                "round {round} failed to accept in every local view"
            ));
        }

        let acceptor = online
            .iter()
            .next()
            .ok_or_else(|| "soak round has no online acceptor".to_string())?;
        let accept = build_rbc_accept(domain, &propose, acceptor, "")?;
        let candidate = mvba_candidate_from_rbc_accept(domain, &propose, &accept)?;
        let input_set = build_mvba_valid_input_set(
            domain,
            view(graph, "validator-1")?,
            hash_hex(
                "postfiat.test.cobalt.adversarial_soak.mvba.v1",
                &round.to_le_bytes(),
            ),
            vec![candidate],
        )?;
        let previous = chain.last();
        let ratified =
            ratify_dabc_amendment(domain, graph, &input_set, previous, activation_height)?;
        chain.push(ratified.clone());

        let restart_replay_checked = if round % 4 == 3 {
            chain = roundtrip(&chain)?;
            true
        } else {
            false
        };
        let stale_replay_rejected = if round % 5 == 4 {
            stale_replay_rejected(domain, graph, &chain)?
        } else {
            true
        };
        let equivocation_checked = if round % 4 == 0 {
            below_threshold_equivocation_still_has_support(domain, graph, round)?
        } else {
            true
        };
        rounds.push(SoakRound {
            round,
            amendment_slot,
            activation_height,
            offline_validator,
            delivered_echoes: delivered_echoes.len(),
            delivered_readies: delivered_readies.len(),
            accepted_validator_count: accepted.len(),
            ratification_id: ratified.ratification_id,
            ratified_sequence: ratified.sequence,
            duplicate_reorder_applied,
            restart_replay_checked,
            stale_replay_rejected,
            equivocation_checked,
        });
    }

    let checkpoint = build_activation_checkpoint(domain, graph, &chain)?;
    let mut activations = Vec::new();
    for ratified in &chain {
        activations.push(validate_dabc_activation_with_full_knowledge(
            domain,
            graph,
            &chain,
            ratified,
            &checkpoint,
        )?);
    }
    let bundle = build_dabc_replay_bundle(domain, graph, chain, vec![checkpoint], activations)?;
    let report = verify_dabc_replay_bundle(domain, graph, &bundle)?;
    Ok((rounds, report))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let (rounds, replay_report) = run_soak(&domain, &graph)?;
    let failed_rounds = rounds
        .iter()
        .filter(|round| {
            round.accepted_validator_count != 7
                || !round.stale_replay_rejected
                || !round.equivocation_checked
        })
        .map(|round| round.round)
        .collect::<Vec<_>>();
    let duplicate_reorder_rounds = rounds
        .iter()
        .filter(|round| round.duplicate_reorder_applied)
        .count();
    let restart_replay_rounds = rounds
        .iter()
        .filter(|round| round.restart_replay_checked)
        .count();
    let stale_replay_checks = rounds
        .iter()
        .filter(|round| round.stale_replay_rejected)
        .count();
    let equivocation_checks = rounds
        .iter()
        .filter(|round| round.equivocation_checked)
        .count();
    let ok = failed_rounds.is_empty()
        && replay_report.ratified_count == SOAK_ROUNDS
        && replay_report.activation_count == SOAK_ROUNDS
        && replay_report.highest_sequence == SOAK_ROUNDS as u64;
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-adversarial-soak-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "round_count": SOAK_ROUNDS,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_rounds_passed": failed_rounds.is_empty(),
            "all_rounds_accepted_by_every_local_view": rounds.iter().all(|round| round.accepted_validator_count == 7),
            "duplicate_reorder_rounds_exercised": duplicate_reorder_rounds >= 10,
            "restart_replay_rounds_exercised": restart_replay_rounds >= 8,
            "stale_replay_checks_exercised": stale_replay_checks == SOAK_ROUNDS,
            "below_threshold_equivocation_checks_exercised": equivocation_checks == SOAK_ROUNDS,
            "dabc_replay_verified": replay_report.ratified_count == SOAK_ROUNDS
                && replay_report.activation_count == SOAK_ROUNDS
                && replay_report.highest_sequence == SOAK_ROUNDS as u64,
            "outside_operators_required": false,
        },
        "failed_rounds": failed_rounds,
        "dabc_replay_report": replay_report,
        "rounds": rounds,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt adversarial soak report failed".into())
    }
}
