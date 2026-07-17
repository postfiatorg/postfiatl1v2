use postfiat_consensus_cobalt::{
    abba_common_coin, abba_strong_support, build_abba_aux, build_abba_conf, build_abba_finish,
    build_abba_init, build_abba_round_state, build_essential_subset, build_trust_graph,
    build_trust_view, detect_abba_aux_equivocation, detect_abba_conf_equivocation,
    detect_abba_conflicting_finish, detect_abba_finish_equivocation, detect_abba_init_equivocation,
    detect_abba_round_equivocations, evaluate_abba_aux_support, evaluate_abba_finish_support,
    AbbaCommonRandomSource, CobaltDomain, CobaltFaultModel, CobaltRuntimeMode, EssentialSubset,
    TrustGraph,
};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct AbbaScenario {
    name: &'static str,
    behavior: Vec<&'static str>,
    expected: &'static str,
    observed: serde_json::Value,
    ok: bool,
}

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn root64(byte: char) -> String {
    std::iter::repeat_n(byte, 64).collect()
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

fn scenario_equivocation_all_kinds(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('7');
    let root = graph.trust_graph_root.clone();
    let init_true = build_abba_init(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let init_false = build_abba_init(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    let aux_true = build_abba_aux(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let aux_false = build_abba_aux(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    let conf_true = build_abba_conf(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let conf_false = build_abba_conf(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    let finish_true = build_abba_finish(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let finish_false = build_abba_finish(
        domain,
        root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        false,
        "",
    )?;
    let init = detect_abba_init_equivocation(domain, &init_true, &init_false)?;
    let aux = detect_abba_aux_equivocation(domain, &aux_true, &aux_false)?;
    let conf = detect_abba_conf_equivocation(domain, &conf_true, &conf_false)?;
    let finish = detect_abba_finish_equivocation(domain, &finish_true, &finish_false)?;
    let mut round_state = build_abba_round_state(root, agreement_id, 1)?;
    round_state.init_messages = vec![init_true, init_false];
    round_state.aux_messages = vec![aux_true, aux_false];
    round_state.conf_messages = vec![conf_true, conf_false];
    round_state.finish_messages = vec![finish_true, finish_false];
    let round_evidence = detect_abba_round_equivocations(domain, &round_state)?;
    Ok(AbbaScenario {
        name: "abba_all_message_kind_equivocations_detected",
        behavior: vec!["equivocate"],
        expected: "same sender equivocation is detected for init, aux, conf, finish, and round-state scans",
        observed: json!({
            "init": init,
            "aux": aux,
            "conf": conf,
            "finish": finish,
            "round_evidence_count": round_evidence.len(),
        }),
        ok: init.is_some()
            && aux.is_some()
            && conf.is_some()
            && finish.is_some()
            && round_evidence.len() == 4,
    })
}

fn scenario_withheld_aux_no_strong(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('8');
    let messages = validators(4)
        .iter()
        .map(|sender| {
            build_abba_aux(
                domain,
                graph.trust_graph_root.clone(),
                sender,
                agreement_id.clone(),
                1,
                true,
                "",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let evaluation = evaluate_abba_aux_support(
        domain,
        view(graph, "validator-0")?,
        &agreement_id,
        1,
        true,
        &messages,
    )?;
    Ok(AbbaScenario {
        name: "withheld_aux_messages_do_not_reach_strong_support",
        behavior: vec!["withhold"],
        expected: "four aux messages are not enough for seven-validator strong support",
        observed: json!({
            "support": evaluation.support,
            "weak_support": evaluation.weak_support,
            "strong_support": evaluation.strong_support,
        }),
        ok: !abba_strong_support(&evaluation),
    })
}

fn scenario_invalid_signature_and_bad_round(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('9');
    let invalid_signature = build_abba_aux(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        1,
        true,
        "not-lower-hex",
    )
    .err()
    .unwrap_or_else(|| "unexpected invalid-signature success".to_string());
    let bad_round = build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id,
        0,
        true,
        "",
    )
    .err()
    .unwrap_or_else(|| "unexpected bad-round success".to_string());
    Ok(AbbaScenario {
        name: "invalid_signature_and_bad_round_rejected",
        behavior: vec!["invalid_signature", "bad_round"],
        expected: "bad ABBA signature encoding and round zero fail validation",
        observed: json!({
            "invalid_signature": invalid_signature,
            "bad_round": bad_round,
        }),
        ok: invalid_signature.contains("RBC signature must be lowercase hex")
            && bad_round.contains("ABBA round must be nonzero"),
    })
}

fn scenario_conflicting_finish(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('a');
    let left = build_abba_finish(
        domain,
        graph.trust_graph_root.clone(),
        "validator-1",
        agreement_id.clone(),
        2,
        true,
        "",
    )?;
    let right = build_abba_finish(
        domain,
        graph.trust_graph_root.clone(),
        "validator-2",
        agreement_id,
        2,
        false,
        "",
    )?;
    let evidence = detect_abba_conflicting_finish(domain, graph, &left, &right)?;
    Ok(AbbaScenario {
        name: "conflicting_finish_values_detected",
        behavior: vec!["conflicting_finish"],
        expected: "linked validators finishing different values produce deterministic evidence",
        observed: json!({ "evidence": evidence }),
        ok: evidence.is_some(),
    })
}

fn scenario_common_coin_guardrail(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('b');
    let source = AbbaCommonRandomSource::DeterministicTest {
        seed_hex: root64('c'),
    };
    let sim_first = abba_common_coin(
        domain,
        &agreement_id,
        3,
        &source,
        CobaltRuntimeMode::Simulation,
    )?;
    let sim_second = abba_common_coin(
        domain,
        &agreement_id,
        3,
        &source,
        CobaltRuntimeMode::Simulation,
    )?;
    let live_error = abba_common_coin(domain, &agreement_id, 3, &source, CobaltRuntimeMode::Live)
        .err()
        .unwrap_or_else(|| "unexpected live deterministic coin success".to_string());
    let beacon = AbbaCommonRandomSource::SignedBeacon {
        beacon_id: root('e'),
        output_hash: graph.trust_graph_root.clone(),
    };
    let live_beacon = abba_common_coin(domain, &agreement_id, 3, &beacon, CobaltRuntimeMode::Live)?;
    Ok(AbbaScenario {
        name: "deterministic_coin_rejected_in_live_mode",
        behavior: vec!["deterministic_coin_misuse"],
        expected: "deterministic test coin is stable in simulation and rejected in live mode",
        observed: json!({
            "simulation_stable": sim_first == sim_second,
            "live_error": live_error,
            "live_beacon_allowed": live_beacon,
        }),
        ok: sim_first == sim_second
            && live_error.contains("deterministic ABBA test CRS cannot be used in live mode"),
    })
}

fn scenario_single_sender_nonterminating(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<AbbaScenario, String> {
    let agreement_id = root('d');
    let finish = build_abba_finish(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        4,
        true,
        "",
    )?;
    let evaluation = evaluate_abba_finish_support(
        domain,
        view(graph, "validator-0")?,
        &agreement_id,
        4,
        true,
        &[finish],
    )?;
    Ok(AbbaScenario {
        name: "single_sender_finish_does_not_terminate",
        behavior: vec!["nonterminating_sender"],
        expected: "one finish message cannot satisfy strong finish support",
        observed: json!({
            "support": evaluation.support,
            "strong_support": evaluation.strong_support,
        }),
        ok: !abba_strong_support(&evaluation),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let scenarios = vec![
        scenario_equivocation_all_kinds(&domain, &graph)?,
        scenario_withheld_aux_no_strong(&domain, &graph)?,
        scenario_invalid_signature_and_bad_round(&domain, &graph)?,
        scenario_conflicting_finish(&domain, &graph)?,
        scenario_common_coin_guardrail(&domain, &graph)?,
        scenario_single_sender_nonterminating(&domain, &graph)?,
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
        "schema": "postfiat-testnet-cobalt-abba-byzantine-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_abba_byzantine_scenarios_passed": ok,
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt ABBA Byzantine report failed".into())
    }
}
