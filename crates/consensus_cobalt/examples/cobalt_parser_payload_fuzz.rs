use std::collections::BTreeSet;

use postfiat_consensus_cobalt::{
    abba_aux_signing_payload_bytes, abba_conf_signing_payload_bytes,
    abba_finish_signing_payload_bytes, abba_init_signing_payload_bytes, build_abba_aux,
    build_abba_conf, build_abba_finish, build_abba_init, build_dabc_full_knowledge_check,
    build_dabc_full_knowledge_checkpoint, build_dabc_replay_bundle, build_essential_subset,
    build_mvba_valid_input_set, build_rbc_accept, build_rbc_echo, build_rbc_propose,
    build_rbc_ready, build_trust_graph, build_trust_graph_transition, build_trust_view,
    dabc_activation_evidence_id, dabc_full_knowledge_check_signing_payload_bytes,
    dabc_replay_bundle_id, mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    rbc_accept_signing_payload_bytes, rbc_echo_signing_payload_bytes,
    rbc_propose_signing_payload_bytes, rbc_ready_signing_payload_bytes, trust_graph_transition_id,
    validate_abba_aux, validate_abba_conf, validate_abba_finish, validate_abba_init,
    validate_dabc_activation_with_full_knowledge, validate_dabc_full_knowledge_check,
    validate_rbc_accept, validate_rbc_echo, validate_rbc_propose, validate_rbc_ready,
    validate_trust_graph, validate_trust_graph_transition, verify_dabc_replay_bundle, AbbaAux,
    AbbaConf, AbbaFinish, AbbaInit, CobaltDomain, DabcFullKnowledgeCheck, DabcPendingPair,
    DabcRatifiedAmendment, DabcReplayBundle, EssentialSubset, RbcAccept, RbcEcho, RbcPropose,
    RbcReady, TrustGraph, TrustGraphTransition,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct ParserFuzzScenario {
    name: &'static str,
    attack: Vec<&'static str>,
    expected: &'static str,
    observed: serde_json::Value,
    ok: bool,
}

struct Corpus {
    domain: CobaltDomain,
    graph: TrustGraph,
    propose: RbcPropose,
    echo: RbcEcho,
    ready: RbcReady,
    accept: RbcAccept,
    init: AbbaInit,
    aux: AbbaAux,
    conf: AbbaConf,
    finish: AbbaFinish,
    dabc_check: DabcFullKnowledgeCheck,
    replay_bundle: DabcReplayBundle,
    transition: TrustGraphTransition,
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

fn fixture_graph() -> Result<(CobaltDomain, TrustGraph), String> {
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

fn candidate_for_slot(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    proposer: &str,
    acceptor: &str,
    amendment_slot: u64,
    payload_byte: char,
) -> Result<postfiat_consensus_cobalt::MvbaCandidate, String> {
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

fn valid_chain(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(DabcRatifiedAmendment, DabcRatifiedAmendment), String> {
    let first_candidate = candidate_for_slot(domain, graph, "validator-0", "validator-1", 11, 'a')?;
    let first_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-1")?,
        root('d'),
        vec![first_candidate],
    )?;
    let first = ratify_dabc_amendment(domain, graph, &first_set, None, 20)?;

    let second_candidate =
        candidate_for_slot(domain, graph, "validator-3", "validator-2", 12, 'c')?;
    let second_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-2")?,
        root('e'),
        vec![second_candidate],
    )?;
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
    let checkpoint =
        build_dabc_full_knowledge_checkpoint(domain, graph, "validator-1", 10, 21, checks)?;
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

fn corpus() -> Result<Corpus, String> {
    let (domain, graph) = fixture_graph()?;
    let propose = build_rbc_propose(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        31,
        root('a'),
        "",
    )?;
    let echo = build_rbc_echo(&domain, &propose, "validator-1", "")?;
    let ready = build_rbc_ready(&domain, &propose, "validator-2", "")?;
    let accept = build_rbc_accept(&domain, &propose, "validator-3", "")?;
    let agreement_id = root('f');
    let init = build_abba_init(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let aux = build_abba_aux(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-1",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let conf = build_abba_conf(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-2",
        agreement_id.clone(),
        1,
        true,
        "",
    )?;
    let finish = build_abba_finish(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id,
        1,
        true,
        "",
    )?;
    let dabc_check = build_dabc_full_knowledge_check(
        &domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        1,
        Vec::new(),
        "",
    )?;
    let replay_bundle = valid_replay_bundle(&domain, &graph)?;
    let transition = build_trust_graph_transition(
        &domain,
        graph.registry_root.clone(),
        root('c'),
        graph.trust_graph_root.clone(),
        root('d'),
        40,
    )?;
    Ok(Corpus {
        domain,
        graph,
        propose,
        echo,
        ready,
        accept,
        init,
        aux,
        conf,
        finish,
        dabc_check,
        replay_bundle,
        transition,
    })
}

fn roundtrip<T>(value: &T) -> Result<(T, usize), String>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let parsed = serde_json::from_slice::<T>(&bytes).map_err(|error| error.to_string())?;
    if &parsed != value {
        return Err("serde roundtrip changed artifact".to_string());
    }
    Ok((parsed, bytes.len()))
}

fn truncated_json_rejected<T>(value: &T) -> Result<bool, String>
where
    T: Serialize + DeserializeOwned,
{
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut cuts = BTreeSet::new();
    cuts.insert(1);
    cuts.insert(bytes.len() / 3);
    cuts.insert(bytes.len() / 2);
    cuts.insert(bytes.len().saturating_sub(1));
    for cut in cuts
        .into_iter()
        .filter(|cut| *cut > 0 && *cut < bytes.len())
    {
        if serde_json::from_slice::<T>(&bytes[..cut]).is_ok() {
            return Ok(false);
        }
    }
    Ok(true)
}

fn protocol_version_type_mutation_rejected<T>(value: &T) -> Result<bool, String>
where
    T: Serialize + DeserializeOwned,
{
    let json = serde_json::to_string(value).map_err(|error| error.to_string())?;
    let mutated = json.replacen("\"protocol_version\":1", "\"protocol_version\":\"1\"", 1);
    if mutated == json {
        return Err("artifact has no protocol_version field".to_string());
    }
    Ok(serde_json::from_str::<T>(&mutated).is_err())
}

fn scenario_roundtrip(corpus: &Corpus) -> Result<ParserFuzzScenario, String> {
    let (propose, propose_bytes) = roundtrip::<RbcPropose>(&corpus.propose)?;
    let (echo, echo_bytes) = roundtrip::<RbcEcho>(&corpus.echo)?;
    let (ready, ready_bytes) = roundtrip::<RbcReady>(&corpus.ready)?;
    let (accept, accept_bytes) = roundtrip::<RbcAccept>(&corpus.accept)?;
    let (init, init_bytes) = roundtrip::<AbbaInit>(&corpus.init)?;
    let (aux, aux_bytes) = roundtrip::<AbbaAux>(&corpus.aux)?;
    let (conf, conf_bytes) = roundtrip::<AbbaConf>(&corpus.conf)?;
    let (finish, finish_bytes) = roundtrip::<AbbaFinish>(&corpus.finish)?;
    let (dabc_check, dabc_check_bytes) = roundtrip::<DabcFullKnowledgeCheck>(&corpus.dabc_check)?;
    let (graph, graph_bytes) = roundtrip::<TrustGraph>(&corpus.graph)?;
    let (bundle, bundle_bytes) = roundtrip::<DabcReplayBundle>(&corpus.replay_bundle)?;
    let (transition, transition_bytes) = roundtrip::<TrustGraphTransition>(&corpus.transition)?;

    validate_rbc_propose(&corpus.domain, &propose)?;
    validate_rbc_echo(&corpus.domain, &echo, &propose)?;
    validate_rbc_ready(&corpus.domain, &ready, &propose)?;
    validate_rbc_accept(&corpus.domain, &accept, &propose)?;
    validate_abba_init(&corpus.domain, &init)?;
    validate_abba_aux(&corpus.domain, &aux)?;
    validate_abba_conf(&corpus.domain, &conf)?;
    validate_abba_finish(&corpus.domain, &finish)?;
    validate_dabc_full_knowledge_check(&corpus.domain, &dabc_check)?;
    validate_trust_graph(&corpus.domain, &graph)?;
    verify_dabc_replay_bundle(&corpus.domain, &corpus.graph, &bundle)?;
    validate_trust_graph_transition(&corpus.domain, &transition)?;

    let payloads_stable = rbc_propose_signing_payload_bytes(&corpus.propose)?
        == rbc_propose_signing_payload_bytes(&propose)?
        && rbc_echo_signing_payload_bytes(&corpus.echo)? == rbc_echo_signing_payload_bytes(&echo)?
        && rbc_ready_signing_payload_bytes(&corpus.ready)?
            == rbc_ready_signing_payload_bytes(&ready)?
        && rbc_accept_signing_payload_bytes(&corpus.accept)?
            == rbc_accept_signing_payload_bytes(&accept)?
        && abba_init_signing_payload_bytes(&corpus.init)?
            == abba_init_signing_payload_bytes(&init)?
        && abba_aux_signing_payload_bytes(&corpus.aux)? == abba_aux_signing_payload_bytes(&aux)?
        && abba_conf_signing_payload_bytes(&corpus.conf)?
            == abba_conf_signing_payload_bytes(&conf)?
        && abba_finish_signing_payload_bytes(&corpus.finish)?
            == abba_finish_signing_payload_bytes(&finish)?
        && dabc_full_knowledge_check_signing_payload_bytes(&corpus.dabc_check)?
            == dabc_full_knowledge_check_signing_payload_bytes(&dabc_check)?;

    Ok(ParserFuzzScenario {
        name: "valid_corpus_roundtrips_and_preserves_canonical_payloads",
        attack: vec!["parser_roundtrip", "canonical_payload_drift"],
        expected:
            "valid corpus artifacts parse, validate, and preserve canonical signing payload bytes",
        observed: json!({
            "artifact_count": 12,
            "serialized_bytes": {
                "rbc_propose": propose_bytes,
                "rbc_echo": echo_bytes,
                "rbc_ready": ready_bytes,
                "rbc_accept": accept_bytes,
                "abba_init": init_bytes,
                "abba_aux": aux_bytes,
                "abba_conf": conf_bytes,
                "abba_finish": finish_bytes,
                "dabc_full_knowledge_check": dabc_check_bytes,
                "trust_graph": graph_bytes,
                "dabc_replay_bundle": bundle_bytes,
                "trust_graph_transition": transition_bytes,
            },
            "canonical_payloads_stable": payloads_stable,
        }),
        ok: payloads_stable,
    })
}

fn scenario_truncated_json(corpus: &Corpus) -> Result<ParserFuzzScenario, String> {
    let checks = vec![
        (
            "rbc_propose",
            truncated_json_rejected::<RbcPropose>(&corpus.propose)?,
        ),
        (
            "rbc_echo",
            truncated_json_rejected::<RbcEcho>(&corpus.echo)?,
        ),
        (
            "rbc_ready",
            truncated_json_rejected::<RbcReady>(&corpus.ready)?,
        ),
        (
            "rbc_accept",
            truncated_json_rejected::<RbcAccept>(&corpus.accept)?,
        ),
        (
            "abba_init",
            truncated_json_rejected::<AbbaInit>(&corpus.init)?,
        ),
        ("abba_aux", truncated_json_rejected::<AbbaAux>(&corpus.aux)?),
        (
            "abba_conf",
            truncated_json_rejected::<AbbaConf>(&corpus.conf)?,
        ),
        (
            "abba_finish",
            truncated_json_rejected::<AbbaFinish>(&corpus.finish)?,
        ),
        (
            "dabc_full_knowledge_check",
            truncated_json_rejected::<DabcFullKnowledgeCheck>(&corpus.dabc_check)?,
        ),
        (
            "trust_graph",
            truncated_json_rejected::<TrustGraph>(&corpus.graph)?,
        ),
        (
            "dabc_replay_bundle",
            truncated_json_rejected::<DabcReplayBundle>(&corpus.replay_bundle)?,
        ),
        (
            "trust_graph_transition",
            truncated_json_rejected::<TrustGraphTransition>(&corpus.transition)?,
        ),
    ];
    let rejected_count = checks.iter().filter(|(_, rejected)| *rejected).count();
    Ok(ParserFuzzScenario {
        name: "truncated_json_corpus_rejected",
        attack: vec!["truncated_json", "partial_payload"],
        expected: "truncated Cobalt JSON artifacts fail to deserialize",
        observed: json!({
            "artifact_count": checks.len(),
            "rejected_count": rejected_count,
            "checks": checks,
        }),
        ok: rejected_count == checks.len(),
    })
}

fn scenario_type_mutations(corpus: &Corpus) -> Result<ParserFuzzScenario, String> {
    let checks = vec![
        (
            "rbc_propose",
            protocol_version_type_mutation_rejected::<RbcPropose>(&corpus.propose)?,
        ),
        (
            "abba_init",
            protocol_version_type_mutation_rejected::<AbbaInit>(&corpus.init)?,
        ),
        (
            "dabc_full_knowledge_check",
            protocol_version_type_mutation_rejected::<DabcFullKnowledgeCheck>(&corpus.dabc_check)?,
        ),
        (
            "trust_graph",
            protocol_version_type_mutation_rejected::<TrustGraph>(&corpus.graph)?,
        ),
        (
            "dabc_replay_bundle",
            protocol_version_type_mutation_rejected::<DabcReplayBundle>(&corpus.replay_bundle)?,
        ),
    ];
    let rejected_count = checks.iter().filter(|(_, rejected)| *rejected).count();
    Ok(ParserFuzzScenario {
        name: "protocol_version_type_mutations_rejected",
        attack: vec!["json_type_confusion", "canonical_domain_binding"],
        expected: "string-typed protocol_version mutations fail serde parsing",
        observed: json!({
            "artifact_count": checks.len(),
            "rejected_count": rejected_count,
            "checks": checks,
        }),
        ok: rejected_count == checks.len(),
    })
}

fn scenario_canonical_id_tampering(corpus: &Corpus) -> Result<ParserFuzzScenario, String> {
    let mut propose = corpus.propose.clone();
    propose.message_id = root('1');
    let mut init = corpus.init.clone();
    init.message_id = root('2');
    let mut dabc_check = corpus.dabc_check.clone();
    dabc_check.message_id = root('3');
    let mut graph = corpus.graph.clone();
    graph.trust_graph_root = root('4');
    let mut transition = corpus.transition.clone();
    transition.transition_id = root('5');
    let mut bundle = corpus.replay_bundle.clone();
    bundle.bundle_id = root('6');
    let mut activation_tamper = corpus.replay_bundle.clone();
    let activation = activation_tamper
        .activation_evidence
        .first_mut()
        .ok_or_else(|| "replay bundle missing activation evidence".to_string())?;
    activation.activation_height = activation
        .activation_height
        .checked_add(1)
        .ok_or_else(|| "activation height overflow".to_string())?;
    activation.activation_id = dabc_activation_evidence_id(&corpus.domain, activation)?;

    let checks = vec![
        (
            "rbc_propose_message_id",
            validate_rbc_propose(&corpus.domain, &propose)
                .expect_err("tampered RBC propose id should fail"),
        ),
        (
            "abba_init_message_id",
            validate_abba_init(&corpus.domain, &init)
                .expect_err("tampered ABBA init id should fail"),
        ),
        (
            "dabc_check_message_id",
            validate_dabc_full_knowledge_check(&corpus.domain, &dabc_check)
                .expect_err("tampered DABC check id should fail"),
        ),
        (
            "trust_graph_root",
            validate_trust_graph(&corpus.domain, &graph)
                .expect_err("tampered trust graph root should fail"),
        ),
        (
            "trust_graph_transition_id",
            validate_trust_graph_transition(&corpus.domain, &transition)
                .expect_err("tampered transition id should fail"),
        ),
        (
            "dabc_replay_bundle_id",
            verify_dabc_replay_bundle(&corpus.domain, &corpus.graph, &bundle)
                .expect_err("tampered replay bundle id should fail"),
        ),
        (
            "dabc_activation_evidence_binding",
            verify_dabc_replay_bundle(&corpus.domain, &corpus.graph, &activation_tamper)
                .expect_err("tampered activation evidence should fail"),
        ),
    ];
    let ok = checks.iter().all(|(_, error)| {
        error.contains("mismatch")
            || error.contains("root mismatch")
            || error.contains("evidence mismatch")
    });
    Ok(ParserFuzzScenario {
        name: "canonical_id_and_binding_tampering_rejected",
        attack: vec!["canonical_id_tamper", "replay_binding_tamper"],
        expected: "tampered Cobalt ids, roots, and activation bindings fail validation",
        observed: json!({ "checks": checks }),
        ok,
    })
}

fn scenario_canonical_id_recompute(corpus: &Corpus) -> Result<ParserFuzzScenario, String> {
    let replay_id = dabc_replay_bundle_id(&corpus.domain, &corpus.replay_bundle)?;
    let transition_id = trust_graph_transition_id(&corpus.domain, &corpus.transition)?;
    Ok(ParserFuzzScenario {
        name: "canonical_ids_recompute_from_parsed_payloads",
        attack: vec!["noncanonical_payload_encoding", "id_recompute"],
        expected: "recomputed ids match parsed replay bundle and trust graph transition ids",
        observed: json!({
            "replay_bundle_id_matches": replay_id == corpus.replay_bundle.bundle_id,
            "trust_graph_transition_id_matches": transition_id == corpus.transition.transition_id,
        }),
        ok: replay_id == corpus.replay_bundle.bundle_id
            && transition_id == corpus.transition.transition_id,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let corpus = corpus()?;
    let scenarios = vec![
        scenario_roundtrip(&corpus)?,
        scenario_truncated_json(&corpus)?,
        scenario_type_mutations(&corpus)?,
        scenario_canonical_id_tampering(&corpus)?,
        scenario_canonical_id_recompute(&corpus)?,
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
        "schema": "postfiat-testnet-cobalt-parser-payload-fuzz-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": corpus.graph.trust_views.len(),
        "trust_graph_root": corpus.graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": corpus.graph.trust_views.len() == 7,
            "all_parser_payload_fuzz_scenarios_passed": ok,
            "valid_corpus_roundtrips": scenarios.iter().any(|scenario| scenario.name == "valid_corpus_roundtrips_and_preserves_canonical_payloads" && scenario.ok),
            "truncated_json_rejected": scenarios.iter().any(|scenario| scenario.name == "truncated_json_corpus_rejected" && scenario.ok),
            "type_mutations_rejected": scenarios.iter().any(|scenario| scenario.name == "protocol_version_type_mutations_rejected" && scenario.ok),
            "canonical_tampering_rejected": scenarios.iter().any(|scenario| scenario.name == "canonical_id_and_binding_tampering_rejected" && scenario.ok),
            "canonical_ids_recompute": scenarios.iter().any(|scenario| scenario.name == "canonical_ids_recompute_from_parsed_payloads" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt parser/canonical payload fuzz report failed".into())
    }
}
