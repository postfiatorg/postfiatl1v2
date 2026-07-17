use postfiat_consensus_cobalt::{
    analyze_trust_graph, bind_dabc_ratification_to_trust_graph_rollback_record,
    build_dabc_full_knowledge_check, build_dabc_full_knowledge_checkpoint,
    build_dabc_replay_bundle, build_essential_subset, build_mvba_valid_input_set, build_rbc_accept,
    build_rbc_propose, build_trust_graph, build_trust_graph_rollback_transition, build_trust_view,
    build_trust_view_update_transition, mvba_candidate_from_rbc_accept, ratify_dabc_amendment,
    trust_graph_rollback_payload_hash, validate_dabc_activation_with_full_knowledge,
    validate_trust_graph_rollback_record, verify_dabc_replay_bundle, CobaltDomain,
    CobaltFaultModel, DabcPendingPair, EssentialSubset, LinkageReport, TrustGraph,
    TrustGraphRollbackRatificationInput, TrustGraphRollbackRecord,
    TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE,
};
use postfiat_crypto_provider::hash_hex;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::json;

#[derive(Debug, Serialize)]
struct RollbackScenario {
    name: &'static str,
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

fn unsafe_view(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
) -> Result<postfiat_consensus_cobalt::TrustView, String> {
    let previous = view(authority_graph, "validator-1")?;
    build_trust_view(
        domain,
        "validator-1",
        previous.view_version + 1,
        vec![subset(domain, &["validator-1"], 0, 1)],
        "",
    )
}

fn unsafe_graph(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
) -> Result<(TrustGraph, LinkageReport), String> {
    let mut views = authority_graph.trust_views.clone();
    let bad_slot = views
        .iter_mut()
        .find(|view| view.validator == "validator-1")
        .ok_or_else(|| "missing bad view slot".to_string())?;
    *bad_slot = unsafe_view(domain, authority_graph)?;
    let bad_graph = build_trust_graph(
        domain,
        authority_graph
            .graph_version
            .checked_add(1)
            .ok_or_else(|| "graph version overflow".to_string())?,
        authority_graph.registry_root.clone(),
        40,
        Some(authority_graph.trust_graph_root.clone()),
        views,
    )?;
    let bad_linkage = analyze_trust_graph(domain, &bad_graph, &CobaltFaultModel::default())?;
    Ok((bad_graph, bad_linkage))
}

fn ratify_rollback(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
    rollback_record: &TrustGraphRollbackRecord,
) -> Result<postfiat_consensus_cobalt::DabcRatifiedAmendment, String> {
    let payload_hash = trust_graph_rollback_payload_hash(domain, rollback_record)?;
    let propose = build_rbc_propose(
        domain,
        authority_graph.trust_graph_root.clone(),
        "validator-0",
        701,
        payload_hash,
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, "validator-1", "")?;
    let candidate = mvba_candidate_from_rbc_accept(domain, &propose, &accept)?;
    let input_set = build_mvba_valid_input_set(
        domain,
        view(authority_graph, "validator-1")?,
        hash_hex(
            "postfiat.test.cobalt.rollback_recovery.agreement.v1",
            b"rollback",
        ),
        vec![candidate],
    )?;
    ratify_dabc_amendment(
        domain,
        authority_graph,
        &input_set,
        None,
        rollback_record.rollback_activation_height,
    )
}

fn replay_rollback(
    domain: &CobaltDomain,
    authority_graph: &TrustGraph,
    ratified: postfiat_consensus_cobalt::DabcRatifiedAmendment,
) -> Result<postfiat_consensus_cobalt::DabcReplayReport, String> {
    let pending_pair = DabcPendingPair {
        amendment_slot: ratified.amendment_slot,
        output_candidate_id: ratified.output_candidate_id.clone(),
    };
    let checks = validators(5)
        .into_iter()
        .map(|sender| {
            build_dabc_full_knowledge_check(
                domain,
                authority_graph.trust_graph_root.clone(),
                sender,
                ratified.activation_height,
                vec![pending_pair.clone()],
                "",
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let checkpoint = build_dabc_full_knowledge_checkpoint(
        domain,
        authority_graph,
        "validator-1",
        ratified.activation_height,
        ratified.activation_height,
        checks,
    )?;
    let chain = vec![ratified.clone()];
    let activation = validate_dabc_activation_with_full_knowledge(
        domain,
        authority_graph,
        &chain,
        &ratified,
        &checkpoint,
    )?;
    let bundle = build_dabc_replay_bundle(
        domain,
        authority_graph,
        chain,
        vec![checkpoint],
        vec![activation],
    )?;
    let restored = roundtrip(&bundle)?;
    verify_dabc_replay_bundle(domain, authority_graph, &restored)
}

fn roundtrip<T>(value: &T) -> Result<T, String>
where
    T: Serialize + DeserializeOwned + PartialEq,
{
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let parsed = serde_json::from_slice::<T>(&bytes).map_err(|error| error.to_string())?;
    if &parsed != value {
        return Err("roundtrip changed rollback replay artifact".to_string());
    }
    Ok(parsed)
}

fn rejection_scenario(
    name: &'static str,
    expected: &'static str,
    result: Result<(), String>,
) -> RollbackScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    RollbackScenario {
        name,
        expected,
        observed: json!({ "error": observed_error }),
        ok: observed_error.contains(expected),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, authority_graph) = fixture()?;
    let unsafe_lifecycle_result = build_trust_view_update_transition(
        &domain,
        &authority_graph,
        unsafe_view(&domain, &authority_graph)?,
        40,
        &CobaltFaultModel::default(),
    )
    .map(|_| ());

    let (bad_graph, bad_linkage) = unsafe_graph(&domain, &authority_graph)?;
    let (rollback_graph, rollback_linkage, rollback_record) =
        build_trust_graph_rollback_transition(
            &domain,
            &authority_graph,
            &bad_graph,
            45,
            &bad_linkage,
        )?;
    validate_trust_graph_rollback_record(
        &domain,
        &authority_graph,
        &bad_graph,
        &rollback_graph,
        &bad_linkage,
        &rollback_linkage,
        &rollback_record,
    )?;

    let ratified = ratify_rollback(&domain, &authority_graph, &rollback_record)?;
    let dabc_rollback = bind_dabc_ratification_to_trust_graph_rollback_record(
        TrustGraphRollbackRatificationInput {
            domain: &domain,
            authority_graph: &authority_graph,
            failed_graph: &bad_graph,
            rollback_graph: &rollback_graph,
            bad_linkage_report: &bad_linkage,
            rollback_linkage_report: &rollback_linkage,
            ratified: &ratified,
            previous_ratified: None,
            record: &rollback_record,
        },
    )?;
    let replay_report = replay_rollback(&domain, &authority_graph, ratified.clone())?;

    let tampered_record_result = {
        let mut tampered = rollback_record.clone();
        tampered.reason = "operator_override".to_string();
        validate_trust_graph_rollback_record(
            &domain,
            &authority_graph,
            &bad_graph,
            &rollback_graph,
            &bad_linkage,
            &rollback_linkage,
            &tampered,
        )
    };
    let wrong_payload_result = {
        let wrong_propose = build_rbc_propose(
            &domain,
            authority_graph.trust_graph_root.clone(),
            "validator-0",
            702,
            root('d'),
            "",
        )?;
        let wrong_accept = build_rbc_accept(&domain, &wrong_propose, "validator-1", "")?;
        let wrong_candidate =
            mvba_candidate_from_rbc_accept(&domain, &wrong_propose, &wrong_accept)?;
        let wrong_input_set = build_mvba_valid_input_set(
            &domain,
            view(&authority_graph, "validator-1")?,
            hash_hex(
                "postfiat.test.cobalt.rollback_recovery.agreement.v1",
                b"wrong-rollback",
            ),
            vec![wrong_candidate],
        )?;
        let wrong_ratified = ratify_dabc_amendment(
            &domain,
            &authority_graph,
            &wrong_input_set,
            None,
            rollback_record.rollback_activation_height,
        )?;
        bind_dabc_ratification_to_trust_graph_rollback_record(TrustGraphRollbackRatificationInput {
            domain: &domain,
            authority_graph: &authority_graph,
            failed_graph: &bad_graph,
            rollback_graph: &rollback_graph,
            bad_linkage_report: &bad_linkage,
            rollback_linkage_report: &rollback_linkage,
            ratified: &wrong_ratified,
            previous_ratified: None,
            record: &rollback_record,
        })
        .map(|_| ())
    };

    let scenarios = vec![
        rejection_scenario(
            "unsafe_lifecycle_update_rejected_before_activation",
            "unsafe before activation",
            unsafe_lifecycle_result,
        ),
        RollbackScenario {
            name: "byzantine_forced_graph_has_unsafe_linkage",
            expected: "unsafe pairs are present",
            observed: json!({
                "unsafe_pair_count": bad_linkage.unsafe_pairs.len(),
                "first_unsafe_pair": bad_linkage.unsafe_pairs.first(),
            }),
            ok: !bad_linkage.unsafe_pairs.is_empty(),
        },
        RollbackScenario {
            name: "rollback_restores_authority_trust_views",
            expected: "rollback graph trust views match authority graph",
            observed: json!({
                "authority_trust_graph_root": authority_graph.trust_graph_root,
                "failed_trust_graph_root": bad_graph.trust_graph_root,
                "rollback_trust_graph_root": rollback_graph.trust_graph_root,
                "rollback_reason": rollback_record.reason,
                "rollback_unsafe_pair_count": rollback_linkage.unsafe_pairs.len(),
            }),
            ok: rollback_graph.trust_views == authority_graph.trust_views
                && rollback_linkage.unsafe_pairs.is_empty()
                && rollback_record.reason == TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE,
        },
        RollbackScenario {
            name: "rollback_ratified_by_dabc",
            expected: "DABC rollback binding points to ratified rollback record",
            observed: json!({
                "dabc_ratification_id": dabc_rollback.dabc_ratification_id,
                "rollback_record_id": dabc_rollback.rollback_record_id,
                "rollback_ratification_id": dabc_rollback.rollback_ratification_id,
            }),
            ok: dabc_rollback.dabc_ratification_id == ratified.ratification_id
                && dabc_rollback.rollback_record_id == rollback_record.record_id,
        },
        RollbackScenario {
            name: "rollback_replay_bundle_verifies_after_roundtrip",
            expected: "offline DABC replay verifies the rollback ratification and activation",
            observed: json!({
                "bundle_id": replay_report.bundle_id,
                "ratified_count": replay_report.ratified_count,
                "activation_count": replay_report.activation_count,
                "checkpoint_count": replay_report.checkpoint_count,
                "highest_activation_height": replay_report.highest_activation_height,
            }),
            ok: replay_report.ratified_count == 1
                && replay_report.activation_count == 1
                && replay_report.checkpoint_count == 1
                && replay_report.highest_activation_height
                    == rollback_record.rollback_activation_height,
        },
        rejection_scenario(
            "tampered_rollback_record_rejected",
            "reason mismatch",
            tampered_record_result.map(|_| ()),
        ),
        rejection_scenario(
            "wrong_dabc_rollback_payload_rejected",
            "payload hash mismatch",
            wrong_payload_result,
        ),
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
        "schema": "postfiat-testnet-cobalt-rollback-recovery-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": authority_graph.trust_views.len(),
        "authority_trust_graph_root": authority_graph.trust_graph_root,
        "failed_trust_graph_root": bad_graph.trust_graph_root,
        "rollback_trust_graph_root": rollback_graph.trust_graph_root,
        "checks": {
            "seven_logical_validators": authority_graph.trust_views.len() == 7,
            "unsafe_update_rejected_before_activation": scenarios.iter().any(|scenario| scenario.name == "unsafe_lifecycle_update_rejected_before_activation" && scenario.ok),
            "bad_graph_has_unsafe_linkage": scenarios.iter().any(|scenario| scenario.name == "byzantine_forced_graph_has_unsafe_linkage" && scenario.ok),
            "rollback_restores_authority_graph": scenarios.iter().any(|scenario| scenario.name == "rollback_restores_authority_trust_views" && scenario.ok),
            "rollback_ratified_by_dabc": scenarios.iter().any(|scenario| scenario.name == "rollback_ratified_by_dabc" && scenario.ok),
            "rollback_replay_verifies_after_roundtrip": scenarios.iter().any(|scenario| scenario.name == "rollback_replay_bundle_verifies_after_roundtrip" && scenario.ok),
            "tampered_rollback_record_rejected": scenarios.iter().any(|scenario| scenario.name == "tampered_rollback_record_rejected" && scenario.ok),
            "wrong_dabc_rollback_payload_rejected": scenarios.iter().any(|scenario| scenario.name == "wrong_dabc_rollback_payload_rejected" && scenario.ok),
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt rollback recovery report failed".into())
    }
}
