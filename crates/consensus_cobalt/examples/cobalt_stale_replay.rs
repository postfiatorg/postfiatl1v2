#[path = "support/atomic_swap_batch.rs"]
mod atomic_swap_batch;

use atomic_swap_batch::build_atomic_swap_batch;
use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_canonical_unl_trust_graph, build_dabc_full_knowledge_check,
    build_dabc_full_knowledge_checkpoint, build_dabc_replay_bundle, build_essential_subset,
    build_mvba_valid_input_set, build_rbc_accept, build_rbc_propose, build_trust_view,
    build_trust_view_update_transition, certify_nonuniform_governance_amendment,
    mvba_candidate_from_rbc_accept, propose_nonuniform_governance_amendment, ratify_dabc_amendment,
    validate_dabc_activation_with_full_knowledge, verify_dabc_replay_bundle,
    verify_nonuniform_governance_certificate, CobaltDomain, CobaltFaultModel, DabcPendingPair,
    DabcReplayBundle, EssentialSubset, LinkageReport, MvbaCandidate,
    NonUniformGovernanceCertificate, TrustGraph,
};
use postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY;
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
struct ReplayScenario {
    name: &'static str,
    replay: &'static str,
    expected_rejection: &'static str,
    observed_error: String,
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

fn g0_g1_fixture() -> Result<(CobaltDomain, TrustGraph, TrustGraph), String> {
    let domain = CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    };
    let validator_ids = validators(7);
    let g0 = build_canonical_unl_trust_graph(&domain, 1, root('a'), 7, None, validator_ids, 5)?;
    let validator_1_g0 = g0
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-1")
        .ok_or_else(|| "missing validator-1 G0 view".to_string())?;
    let mut g1_subsets = validator_1_g0.essential_subsets.clone();
    g1_subsets.push(subset(
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
    ));
    let validator_1_g1 = build_trust_view(&domain, "validator-1", 2, g1_subsets, "")?;
    let (g1, _record) = build_trust_view_update_transition(
        &domain,
        &g0,
        validator_1_g1,
        30,
        &CobaltFaultModel::default(),
    )?;
    Ok((domain, g0, g1))
}

fn nonuniform_certificate(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    linkage: &LinkageReport,
    local_validator: &str,
    value: u32,
) -> Result<NonUniformGovernanceCertificate, String> {
    let proposal = propose_nonuniform_governance_amendment(
        domain,
        graph,
        GOVERNANCE_KIND_CRYPTO_POLICY,
        value,
    )?;
    certify_nonuniform_governance_amendment(
        domain,
        graph,
        linkage,
        local_validator,
        &proposal,
        validators(7).into_iter().take(5).collect(),
        graph.activation_height,
    )
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

fn dabc_bundle(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    atomic_swap_payload_hash: &str,
) -> Result<DabcReplayBundle, String> {
    let view_1 = graph
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-1")
        .ok_or_else(|| "missing validator-1 view".to_string())?;
    let view_2 = graph
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-2")
        .ok_or_else(|| "missing validator-2 view".to_string())?;
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

fn scenario(
    name: &'static str,
    replay: &'static str,
    expected_rejection: &'static str,
    result: Result<(), String>,
) -> ReplayScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    ReplayScenario {
        name,
        replay,
        expected_rejection,
        ok: observed_error.contains(expected_rejection),
        observed_error,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, g0, g1) = g0_g1_fixture()?;
    let atomic_batch = build_atomic_swap_batch(&domain)?;
    let atomic_swap_payload_hash = atomic_batch.reference.payload_hash.as_str();
    let g0_linkage = analyze_trust_graph(&domain, &g0, &CobaltFaultModel::default())?;
    let g1_linkage = analyze_trust_graph(&domain, &g1, &CobaltFaultModel::default())?;
    let g0_proposal =
        propose_nonuniform_governance_amendment(&domain, &g0, GOVERNANCE_KIND_CRYPTO_POLICY, 2)?;
    let g1_proposal =
        propose_nonuniform_governance_amendment(&domain, &g1, GOVERNANCE_KIND_CRYPTO_POLICY, 3)?;
    let g0_cert = nonuniform_certificate(&domain, &g0, &g0_linkage, "validator-1", 2)?;
    let g1_cert = nonuniform_certificate(&domain, &g1, &g1_linkage, "validator-1", 3)?;
    let g0_bundle = dabc_bundle(&domain, &g0, atomic_swap_payload_hash)?;

    let stale_g0_cert = verify_nonuniform_governance_certificate(
        &domain,
        &g1,
        &g1_linkage,
        &g1_proposal,
        &g0_cert,
        g1.activation_height,
    );

    let stale_g0_proposal = verify_nonuniform_governance_certificate(
        &domain,
        &g1,
        &g1_linkage,
        &g0_proposal,
        &g1_cert,
        g1.activation_height,
    );

    let stale_g0_linkage = verify_nonuniform_governance_certificate(
        &domain,
        &g1,
        &g0_linkage,
        &g1_proposal,
        &g1_cert,
        g1.activation_height,
    );

    let mut old_registry_cert = g1_cert.clone();
    old_registry_cert.registry_root = root('f');
    let stale_registry_root = verify_nonuniform_governance_certificate(
        &domain,
        &g1,
        &g1_linkage,
        &g1_proposal,
        &old_registry_cert,
        g1.activation_height,
    );

    let old_view_id = g0
        .trust_views
        .iter()
        .find(|view| view.validator == "validator-1")
        .ok_or("missing validator-1 G0 view")?
        .trust_view_id
        .clone();
    let mut stale_view_cert = g1_cert.clone();
    stale_view_cert.trust_view_id = old_view_id;
    let stale_trust_view_id = verify_nonuniform_governance_certificate(
        &domain,
        &g1,
        &g1_linkage,
        &g1_proposal,
        &stale_view_cert,
        g1.activation_height,
    );

    let stale_dabc_bundle = verify_dabc_replay_bundle(&domain, &g1, &g0_bundle).map(|_| ());

    let stale_atomic_swap_candidate =
        atomic_swap_candidate(&domain, &g0, 702, atomic_swap_payload_hash)?;
    let restored_atomic_swap_candidate: MvbaCandidate =
        serde_json::from_slice(&serde_json::to_vec(&stale_atomic_swap_candidate)?)?;
    let stale_atomic_swap_candidate_hash = restored_atomic_swap_candidate.candidate_id.clone();
    let g0_atomic_swap_input = build_mvba_valid_input_set(
        &domain,
        g0.trust_views
            .iter()
            .find(|view| view.validator == "validator-1")
            .ok_or("missing validator-1 G0 view")?,
        root('8'),
        vec![restored_atomic_swap_candidate.clone()],
    )?;
    let g0_atomic_swap_ratified =
        ratify_dabc_amendment(&domain, &g0, &g0_atomic_swap_input, None, 703)?;
    let stale_atomic_swap_input = build_mvba_valid_input_set(
        &domain,
        g1.trust_views
            .iter()
            .find(|view| view.validator == "validator-1")
            .ok_or("missing validator-1 G1 view")?,
        root('9'),
        vec![restored_atomic_swap_candidate.clone()],
    )?;
    let stale_atomic_swap_replay =
        ratify_dabc_amendment(&domain, &g1, &stale_atomic_swap_input, None, 703).map(|_| ());

    let scenarios = vec![
        scenario(
            "old_g0_certificate_rejected_after_g1_activation",
            "verify a G0 non-uniform certificate against active G1",
            "trust graph root mismatch",
            stale_g0_cert,
        ),
        scenario(
            "old_g0_proposal_rejected_after_g1_activation",
            "verify a G1 certificate against a proposal bound to old G0",
            "proposal instance mismatch",
            stale_g0_proposal,
        ),
        scenario(
            "old_g0_linkage_report_rejected_after_g1_activation",
            "verify a G1 certificate using a stale G0 linkage report",
            "linkage report trust graph root mismatch",
            stale_g0_linkage,
        ),
        scenario(
            "old_registry_root_rejected",
            "verify a certificate carrying a non-active registry root",
            "registry root mismatch",
            stale_registry_root,
        ),
        scenario(
            "old_trust_view_id_rejected",
            "verify a G1 certificate carrying validator-1's old G0 trust view id",
            "stale trust view id",
            stale_trust_view_id,
        ),
        scenario(
            "old_dabc_replay_bundle_rejected_after_g1_activation",
            "verify a DABC replay bundle bound to old G0 against active G1",
            "trust graph root mismatch",
            stale_dabc_bundle,
        ),
        scenario(
            "old_atomic_swap_candidate_rejected_after_g1_activation",
            "replay one production-derived atomic-swap batch/candidate payload hash bound to old G0 against active G1",
            "MVBA valid input candidate trust graph root mismatch",
            stale_atomic_swap_replay,
        ),
    ];
    let atomic_swap_payload_bound = g0_bundle
        .ratified_amendments
        .first()
        .is_some_and(|ratified| ratified.candidate.payload_hash == atomic_swap_payload_hash)
        && restored_atomic_swap_candidate.payload_hash == atomic_swap_payload_hash
        && g0_atomic_swap_ratified.candidate.payload_hash == atomic_swap_payload_hash
        && stale_atomic_swap_input
            .candidates
            .first()
            .is_some_and(|candidate| candidate.payload_hash == atomic_swap_payload_hash)
        && atomic_batch.batch.transaction_count() == 1
        && atomic_batch.batch.atomic_swap_transactions.len() == 1;
    let mut scenario_failures = scenarios
        .iter()
        .filter(|scenario| !scenario.ok)
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    if !atomic_swap_payload_bound {
        scenario_failures.push("atomic_swap_payload_hash_not_bound_end_to_end");
    }
    let ok = scenario_failures.is_empty();
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-stale-replay-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": g1.trust_views.len(),
        "old_trust_graph_root": g0.trust_graph_root,
        "active_trust_graph_root": g1.trust_graph_root,
        "atomic_swap_batch_id": atomic_batch.reference.batch_id,
        "atomic_swap_batch_payload_hash": atomic_swap_payload_hash,
        "atomic_swap_serialized_batch_bytes": atomic_batch.serialized_batch.len(),
        "atomic_swap_transaction_count": atomic_batch.batch.atomic_swap_transactions.len(),
        "checks": {
            "seven_logical_validators": g1.trust_views.len() == 7,
            "old_and_active_roots_differ": g0.trust_graph_root != g1.trust_graph_root,
            "all_stale_replay_scenarios_rejected": ok,
            "atomic_swap_candidate_replay_rejected": scenarios.iter().any(|scenario| scenario.name == "old_atomic_swap_candidate_rejected_after_g1_activation" && scenario.ok),
            "atomic_swap_payload_hash_bound_end_to_end": atomic_swap_payload_bound,
            "g0_ratified_atomic_swap_payload_hash": g0_atomic_swap_ratified.candidate.payload_hash,
            "stale_atomic_swap_payload_hash": restored_atomic_swap_candidate.payload_hash,
            "stale_atomic_swap_candidate_hash": stale_atomic_swap_candidate_hash,
            "outside_operators_required": false,
        },
        "scenario_failures": scenario_failures,
        "scenarios": scenarios,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt stale replay report failed".into())
    }
}
