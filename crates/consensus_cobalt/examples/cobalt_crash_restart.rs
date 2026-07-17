#[path = "support/atomic_swap_batch.rs"]
mod atomic_swap_batch;

use atomic_swap_batch::{build_atomic_swap_batch, AtomicSwapBatchFixture};
use postfiat_consensus_cobalt::{
    analyze_trust_graph, bind_dabc_ratification_to_validator_registry_update, build_abba_init,
    build_abba_round_state, build_dabc_full_knowledge_check, build_dabc_full_knowledge_checkpoint,
    build_dabc_replay_bundle, build_essential_subset, build_mvba_valid_input_set, build_rbc_accept,
    build_rbc_echo, build_rbc_propose, build_rbc_ready, build_trust_graph,
    build_trust_graph_rollback_transition, build_trust_view, build_trust_view_update_transition,
    certify_validator_registry_update, detect_abba_round_equivocations, evaluate_rbc_echo_support,
    evaluate_rbc_ready_support, mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    rbc_accept_allowed_from_ready, rbc_ready_allowed_from_echo, trust_graph_rollback_payload_hash,
    validate_dabc_activation_with_full_knowledge, validate_trust_graph_lifecycle_record,
    validate_trust_graph_rollback_record, validator_registry_lifecycle_payload_hash,
    verify_dabc_replay_bundle, verify_validator_registry_update, CobaltDomain, CobaltFaultModel,
    DabcPendingPair, DabcReplayBundle, EssentialSubset, EssentialSubsetConfig, LinkageReport,
    MvbaCandidate, RbcEcho, RbcPropose, RbcReady, TrustGraph, TrustGraphLifecycleRecord,
    TrustGraphRollbackRecord, VALIDATOR_REGISTRY_OP_SUSPEND,
};
use postfiat_types::ValidatorRegistryEntry;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct CrashRestartScenario {
    name: &'static str,
    crash_point: &'static str,
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

fn registry_entry(node_id: &str, public_key_hex: &str, active: bool) -> ValidatorRegistryEntry {
    ValidatorRegistryEntry {
        node_id: node_id.to_string(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: public_key_hex.to_string(),
        active,
    }
}

fn roundtrip<T>(value: &T) -> Result<T, String>
where
    T: Serialize + DeserializeOwned,
{
    serde_json::from_slice(&serde_json::to_vec(value).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
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

fn rbc_messages(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(RbcPropose, Vec<RbcEcho>, Vec<RbcReady>), String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        701,
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
    Ok((propose, echoes, readies))
}

fn atomic_swap_candidate(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    amendment_slot: u64,
    payload_hash: &str,
) -> Result<MvbaCandidate, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        amendment_slot,
        payload_hash,
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, "validator-1", "")?;
    mvba_candidate_from_rbc_accept(domain, &propose, &accept)
}

fn accepted_count(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    propose: &RbcPropose,
    echoes: &[RbcEcho],
    readies: &[RbcReady],
) -> Result<usize, String> {
    let mut accepted = 0;
    for validator in validators(7) {
        let trust_view = view(graph, &validator)?;
        let echo_eval = evaluate_rbc_echo_support(domain, trust_view, propose, echoes)?;
        let ready_eval = evaluate_rbc_ready_support(domain, trust_view, propose, readies)?;
        if echo_eval.strong_support
            && rbc_ready_allowed_from_echo(&echo_eval)
            && rbc_accept_allowed_from_ready(&ready_eval)
        {
            accepted += 1;
        }
    }
    Ok(accepted)
}

fn dabc_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    atomic_swap_payload_hash: &str,
) -> Result<DabcReplayBundle, String> {
    let view_1 = view(graph, "validator-1")?;
    let view_2 = view(graph, "validator-2")?;
    let propose_a = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        11,
        atomic_swap_payload_hash,
        "",
    )?;
    let accept_a = build_rbc_accept(domain, &propose_a, "validator-1", "")?;
    let candidate_a = mvba_candidate_from_rbc_accept(domain, &propose_a, &accept_a)?;
    let set_a = build_mvba_valid_input_set(domain, view_1, root('d'), vec![candidate_a])?;
    let first = ratify_dabc_amendment(domain, graph, &set_a, None, 20)?;

    let propose_b = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        12,
        root('c'),
        "",
    )?;
    let accept_b = build_rbc_accept(domain, &propose_b, "validator-2", "")?;
    let candidate_b = mvba_candidate_from_rbc_accept(domain, &propose_b, &accept_b)?;
    let set_b = build_mvba_valid_input_set(domain, view_2, root('e'), vec![candidate_b])?;
    let second = ratify_dabc_amendment(domain, graph, &set_b, Some(&first), 21)?;

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

fn graph_update(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(TrustGraph, LinkageReport, TrustGraphLifecycleRecord), String> {
    let previous_view = view(graph, "validator-1")?;
    let updated = build_trust_view(
        domain,
        "validator-1",
        previous_view.view_version + 1,
        previous_view.essential_subsets.clone(),
        "",
    )?;
    let (new_graph, record) = build_trust_view_update_transition(
        domain,
        graph,
        updated,
        40,
        &CobaltFaultModel::default(),
    )?;
    let linkage = analyze_trust_graph(domain, &new_graph, &CobaltFaultModel::default())?;
    Ok((new_graph, linkage, record))
}

fn rollback_update(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<(TrustGraph, LinkageReport, TrustGraphRollbackRecord), String> {
    let previous_view = view(graph, "validator-1")?;
    let unsafe_view = build_trust_view(
        domain,
        "validator-1",
        previous_view.view_version + 1,
        vec![subset(domain, &["validator-1"], 0, 1)],
        "",
    )?;
    let mut bad_views = graph.trust_views.clone();
    let bad_slot = bad_views
        .iter_mut()
        .find(|view| view.validator == "validator-1")
        .ok_or_else(|| "missing bad view slot".to_string())?;
    *bad_slot = unsafe_view;
    let bad_graph = build_trust_graph(
        domain,
        graph.graph_version + 1,
        graph.registry_root.clone(),
        44,
        Some(graph.trust_graph_root.clone()),
        bad_views,
    )?;
    let bad_linkage = analyze_trust_graph(domain, &bad_graph, &CobaltFaultModel::default())?;
    let (rollback_graph, rollback_linkage, rollback_record) =
        build_trust_graph_rollback_transition(domain, graph, &bad_graph, 45, &bad_linkage)?;
    validate_trust_graph_rollback_record(
        domain,
        graph,
        &bad_graph,
        &rollback_graph,
        &bad_linkage,
        &rollback_linkage,
        &rollback_record,
    )?;
    Ok((rollback_graph, rollback_linkage, rollback_record))
}

fn scenario_rbc_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<CrashRestartScenario, String> {
    let (propose, echoes, readies) = rbc_messages(domain, graph)?;
    let before = accepted_count(domain, graph, &propose, &echoes, &readies)?;
    let restored_propose = roundtrip(&propose)?;
    let mut restored_echoes: Vec<RbcEcho> = roundtrip(&echoes)?;
    let mut restored_readies: Vec<RbcReady> = roundtrip(&readies)?;
    restored_echoes.extend(roundtrip::<Vec<RbcEcho>>(&echoes)?);
    restored_readies.extend(roundtrip::<Vec<RbcReady>>(&readies)?);
    let after = accepted_count(
        domain,
        graph,
        &restored_propose,
        &restored_echoes,
        &restored_readies,
    )?;
    Ok(CrashRestartScenario {
        name: "rbc_restart_replay_is_idempotent",
        crash_point: "rbc",
        expected: "replayed RBC messages retain the same accepted validator count despite duplicate delivery",
        observed: json!({ "accepted_before": before, "accepted_after_restart": after }),
        ok: before == 7 && after == 7,
    })
}

fn scenario_abba_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<CrashRestartScenario, String> {
    let agreement_id = root('7');
    let mut state =
        build_abba_round_state(graph.trust_graph_root.clone(), agreement_id.clone(), 1)?;
    state.init_messages.push(build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id.clone(),
        1,
        true,
        "",
    )?);
    state.init_messages.push(build_abba_init(
        domain,
        graph.trust_graph_root.clone(),
        "validator-3",
        agreement_id,
        1,
        false,
        "",
    )?);
    let restored = roundtrip(&state)?;
    let evidence = detect_abba_round_equivocations(domain, &restored)?;
    Ok(CrashRestartScenario {
        name: "abba_restart_preserves_equivocation_evidence",
        crash_point: "abba",
        expected: "after restart, ABBA same-sender equivocation is still detected and not double-counted as a valid sender",
        observed: json!({ "equivocation_count_after_restart": evidence.len(), "evidence": evidence }),
        ok: evidence.len() == 1,
    })
}

fn scenario_dabc_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    atomic_batch: &AtomicSwapBatchFixture,
) -> Result<CrashRestartScenario, String> {
    let batch_payload_hash = atomic_batch.reference.payload_hash.as_str();
    let bundle = dabc_bundle(domain, graph, batch_payload_hash)?;
    let restored = roundtrip(&bundle)?;
    let report = verify_dabc_replay_bundle(domain, graph, &restored)?;
    let replayed_payload_hash = restored
        .ratified_amendments
        .first()
        .map(|ratified| ratified.candidate.payload_hash.as_str());
    Ok(CrashRestartScenario {
        name: "mvba_dabc_restart_replay_verifies_once",
        crash_point: "mvba_dabc",
        expected:
            "DABC replay bundle survives restart and verifies to the same ordered ratification ids",
        observed: json!({
            "bundle_id": restored.bundle_id,
            "ratified_count": report.ratified_count,
            "activation_count": report.activation_count,
            "ratification_ids": report.ratification_ids,
            "atomic_swap_batch_id": atomic_batch.reference.batch_id,
            "atomic_swap_batch_payload_hash": batch_payload_hash,
            "replayed_atomic_swap_payload_hash": replayed_payload_hash,
        }),
        ok: restored.bundle_id == bundle.bundle_id
            && report.ratified_count == 2
            && report.activation_count == 2
            && replayed_payload_hash == Some(batch_payload_hash),
    })
}

fn scenario_atomic_swap_candidate_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    atomic_batch: &AtomicSwapBatchFixture,
) -> Result<CrashRestartScenario, String> {
    let batch_payload_hash = atomic_batch.reference.payload_hash.as_str();
    let candidate = atomic_swap_candidate(domain, graph, 702, batch_payload_hash)?;
    let restored_candidate = roundtrip(&candidate)?;
    let restored_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-1")?,
        root('9'),
        vec![restored_candidate.clone()],
    )?;
    let ratified = ratify_dabc_amendment(domain, graph, &restored_set, None, 703)?;
    let output_payload_hash = restored_set
        .candidates
        .first()
        .map(|output| output.payload_hash.as_str());
    Ok(CrashRestartScenario {
        name: "atomic_swap_candidate_restart_is_indivisible",
        crash_point: "atomic_swap_candidate_persisted_before_round_completion",
        expected: "restart and ratification preserve one production-derived atomic-swap batch payload hash as one MVBA candidate; consensus never exposes or splits its two transfer legs",
        observed: json!({
            "batch_id": atomic_batch.reference.batch_id,
            "batch_payload_hash": batch_payload_hash,
            "serialized_batch_bytes": atomic_batch.serialized_batch.len(),
            "candidate_hash": restored_candidate.candidate_id,
            "candidate_count_after_restart": restored_set.candidates.len(),
            "transaction_count_in_serialized_batch": atomic_batch.batch.transaction_count(),
            "atomic_swap_transaction_count": atomic_batch.batch.atomic_swap_transactions.len(),
            "consensus_visible_transfer_legs": 0,
            "mvba_output_payload_hash": output_payload_hash,
            "ratified_payload_hash": ratified.candidate.payload_hash,
        }),
        ok: restored_candidate == candidate
            && restored_candidate.payload_hash == batch_payload_hash
            && restored_set.candidates.len() == 1
            && restored_set.output_candidate_id == restored_candidate.candidate_id
            && output_payload_hash == Some(batch_payload_hash)
            && ratified.candidate.payload_hash == batch_payload_hash
            && atomic_batch.batch.transaction_count() == 1
            && atomic_batch.batch.atomic_swap_transactions.len() == 1,
    })
}

fn scenario_graph_activation_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<CrashRestartScenario, String> {
    let (new_graph, linkage, record) = graph_update(domain, graph)?;
    let restored_graph = roundtrip(&new_graph)?;
    let restored_linkage = roundtrip(&linkage)?;
    let restored_record = roundtrip(&record)?;
    let result = validate_trust_graph_lifecycle_record(
        domain,
        graph,
        &restored_graph,
        &restored_linkage,
        &restored_record,
    );
    Ok(CrashRestartScenario {
        name: "graph_activation_restart_preserves_lifecycle_record",
        crash_point: "graph_activation",
        expected: "trust graph activation record survives restart and still validates against the new graph root",
        observed: json!({
            "new_trust_graph_root": restored_graph.trust_graph_root,
            "record_id": restored_record.record_id,
            "error": result.as_ref().err(),
        }),
        ok: result.is_ok(),
    })
}

fn scenario_validator_suspension_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<CrashRestartScenario, String> {
    let config = EssentialSubsetConfig {
        validators: validators(7),
        quorum: 5,
    };
    let new_validators = validators(7)
        .into_iter()
        .filter(|validator| validator != "validator-1")
        .collect::<Vec<_>>();
    let request = postfiat_consensus_cobalt::ValidatorRegistryUpdateRequest {
        activation_height: 50,
        previous_registry_root: graph.registry_root.clone(),
        new_registry_root: root('f'),
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators: validators(7),
        new_validators,
        operation: VALIDATOR_REGISTRY_OP_SUSPEND.to_string(),
        subject_node_id: "validator-1".to_string(),
        previous_record: Some(registry_entry("validator-1", "ab12", true)),
        new_record: Some(registry_entry("validator-1", "ab12", false)),
    };
    let update = certify_validator_registry_update(domain, &config, request, validators(5))?;
    let payload_hash = validator_registry_lifecycle_payload_hash(domain, &update)?;
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        51,
        payload_hash.clone(),
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, "validator-1", "")?;
    let candidate = mvba_candidate_from_rbc_accept(domain, &propose, &accept)?;
    let input_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-1")?,
        root('8'),
        vec![candidate],
    )?;
    let ratified =
        ratify_dabc_amendment(domain, graph, &input_set, None, update.activation_height)?;
    let lifecycle = bind_dabc_ratification_to_validator_registry_update(
        domain, graph, &ratified, None, &update,
    )?;
    let restored_update = roundtrip(&update)?;
    let restored_lifecycle = roundtrip(&lifecycle)?;
    let verify = verify_validator_registry_update(domain, &restored_update);
    Ok(CrashRestartScenario {
        name: "validator_suspension_restart_preserves_dabc_binding",
        crash_point: "validator_suspension",
        expected: "after restart, the suspension update still verifies and its DABC binding points at the same update id",
        observed: json!({
            "registry_update_id": restored_update.update_id,
            "lifecycle_registry_update_id": restored_lifecycle.registry_update_id,
            "subject": restored_lifecycle.subject_node_id,
            "operation": restored_lifecycle.operation,
            "verify_error": verify.as_ref().err(),
        }),
        ok: verify.is_ok()
            && restored_lifecycle.registry_update_id == restored_update.update_id
            && restored_lifecycle.operation == VALIDATOR_REGISTRY_OP_SUSPEND,
    })
}

fn scenario_rollback_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
) -> Result<CrashRestartScenario, String> {
    let (rollback_graph, rollback_linkage, rollback_record) = rollback_update(domain, graph)?;
    let restored_graph = roundtrip(&rollback_graph)?;
    let restored_linkage = roundtrip(&rollback_linkage)?;
    let restored_record = roundtrip(&rollback_record)?;
    let payload_hash = trust_graph_rollback_payload_hash(domain, &restored_record)?;
    Ok(CrashRestartScenario {
        name: "rollback_restart_restores_authority_graph",
        crash_point: "rollback",
        expected: "rollback record survives restart, restores the authority trust views, and keeps the same rollback payload hash",
        observed: json!({
            "rollback_trust_graph_root": restored_graph.trust_graph_root,
            "rollback_record_id": restored_record.record_id,
            "payload_hash": payload_hash,
            "unsafe_pairs_after_rollback": restored_linkage.unsafe_pairs.len(),
        }),
        ok: restored_graph.trust_views == graph.trust_views
            && restored_linkage.unsafe_pairs.is_empty()
            && restored_record.rollback_trust_graph_root == restored_graph.trust_graph_root,
    })
}

fn scenario_stale_replay_rejected_after_restart(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    atomic_batch: &AtomicSwapBatchFixture,
) -> Result<CrashRestartScenario, String> {
    let batch_payload_hash = atomic_batch.reference.payload_hash.as_str();
    let bundle = dabc_bundle(domain, graph, batch_payload_hash)?;
    let restored_old_bundle = roundtrip(&bundle)?;
    let replayed_payload_hash = restored_old_bundle
        .ratified_amendments
        .first()
        .map(|ratified| ratified.candidate.payload_hash.as_str());
    let (new_graph, _linkage, _record) = graph_update(domain, graph)?;
    let replay = verify_dabc_replay_bundle(domain, &new_graph, &restored_old_bundle);
    let observed_error = replay
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    Ok(CrashRestartScenario {
        name: "stale_dabc_replay_rejected_after_graph_restart",
        crash_point: "stale_replay_after_restart",
        expected: "after restart onto a newer graph, old DABC replay evidence is rejected by trust graph root",
        observed: json!({
            "batch_payload_hash": batch_payload_hash,
            "replayed_payload_hash": replayed_payload_hash,
            "error": observed_error,
        }),
        ok: replayed_payload_hash == Some(batch_payload_hash)
            && observed_error.contains("trust graph root mismatch"),
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, graph) = fixture()?;
    let atomic_batch = build_atomic_swap_batch(&domain)?;
    let scenarios = vec![
        scenario_rbc_restart(&domain, &graph)?,
        scenario_abba_restart(&domain, &graph)?,
        scenario_dabc_restart(&domain, &graph, &atomic_batch)?,
        scenario_atomic_swap_candidate_restart(&domain, &graph, &atomic_batch)?,
        scenario_graph_activation_restart(&domain, &graph)?,
        scenario_validator_suspension_restart(&domain, &graph)?,
        scenario_rollback_restart(&domain, &graph)?,
        scenario_stale_replay_rejected_after_restart(&domain, &graph, &atomic_batch)?,
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
        "schema": "postfiat-testnet-cobalt-crash-restart-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": graph.trust_views.len(),
        "trust_graph_root": graph.trust_graph_root,
        "atomic_swap_batch_id": atomic_batch.reference.batch_id,
        "atomic_swap_batch_payload_hash": atomic_batch.reference.payload_hash,
        "atomic_swap_serialized_batch_bytes": atomic_batch.serialized_batch.len(),
        "checks": {
            "seven_logical_validators": graph.trust_views.len() == 7,
            "all_crash_restart_scenarios_passed": ok,
            "rbc_restart_idempotent": scenarios.iter().any(|scenario| scenario.name == "rbc_restart_replay_is_idempotent" && scenario.ok),
            "abba_restart_preserves_evidence": scenarios.iter().any(|scenario| scenario.name == "abba_restart_preserves_equivocation_evidence" && scenario.ok),
            "mvba_dabc_restart_replays": scenarios.iter().any(|scenario| scenario.name == "mvba_dabc_restart_replay_verifies_once" && scenario.ok),
            "atomic_swap_candidate_restart_indivisible": scenarios.iter().any(|scenario| scenario.name == "atomic_swap_candidate_restart_is_indivisible" && scenario.ok),
            "graph_activation_restart_validates": scenarios.iter().any(|scenario| scenario.name == "graph_activation_restart_preserves_lifecycle_record" && scenario.ok),
            "validator_suspension_restart_validates": scenarios.iter().any(|scenario| scenario.name == "validator_suspension_restart_preserves_dabc_binding" && scenario.ok),
            "rollback_restart_validates": scenarios.iter().any(|scenario| scenario.name == "rollback_restart_restores_authority_graph" && scenario.ok),
            "stale_replay_after_restart_rejected": scenarios.iter().any(|scenario| scenario.name == "stale_dabc_replay_rejected_after_graph_restart" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt crash/restart report failed".into())
    }
}
