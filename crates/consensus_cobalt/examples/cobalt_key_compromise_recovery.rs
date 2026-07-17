use postfiat_consensus_cobalt::{
    analyze_trust_graph, bind_dabc_ratification_to_validator_registry_update,
    build_cobalt_block_membership_binding, build_essential_subset, build_mvba_valid_input_set,
    build_rbc_accept, build_rbc_propose, build_transaction_network_membership, build_trust_graph,
    build_trust_graph_transition, build_trust_view,
    certify_validator_registry_update_with_trust_graph_transition, mvba_candidate_from_rbc_accept,
    ratify_dabc_amendment, validate_cobalt_block_against_transaction_network_transition,
    validate_transaction_network_transition, validator_registry_lifecycle_payload_hash,
    verify_validator_registry_update, CobaltDomain, CobaltFaultModel, EssentialSubset,
    EssentialSubsetConfig, TrustGraph, TrustGraphTransition, ValidatorRegistryUpdateRequest,
    VALIDATOR_REGISTRY_OP_REACTIVATE, VALIDATOR_REGISTRY_OP_ROTATE_KEY,
    VALIDATOR_REGISTRY_OP_SUSPEND,
};
use postfiat_crypto_provider::hash_hex;
use postfiat_types::{ValidatorRegistryEntry, ValidatorRegistryUpdateRecord};
use serde::Serialize;
use serde_json::json;

const SUBJECT: &str = "validator-1";
const OLD_COMPROMISED_KEY: &str = "ab12";
const ROTATED_KEY: &str = "cd34";

#[derive(Debug, Serialize)]
struct LifecycleStep {
    name: &'static str,
    operation: String,
    previous_registry_root: String,
    new_registry_root: String,
    previous_trust_graph_root: String,
    new_trust_graph_root: String,
    activation_height: u64,
    support: Vec<String>,
    registry_update_id: String,
    dabc_ratification_id: String,
    lifecycle_ratification_id: String,
    subject_previous_active: bool,
    subject_new_active: bool,
    subject_previous_key: String,
    subject_new_key: String,
    ok: bool,
}

struct CertifyAndBindInput {
    request: ValidatorRegistryUpdateRequest,
    transition: TrustGraphTransition,
    support: Vec<String>,
    slot: u64,
    name: &'static str,
}

#[derive(Debug, Serialize)]
struct RejectionScenario {
    name: &'static str,
    expected: &'static str,
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

fn active_without_subject() -> Vec<String> {
    validators(7)
        .into_iter()
        .filter(|validator| validator != SUBJECT)
        .collect()
}

fn support_without_subject() -> Vec<String> {
    vec![
        "validator-0".to_string(),
        "validator-2".to_string(),
        "validator-3".to_string(),
        "validator-4".to_string(),
        "validator-5".to_string(),
    ]
}

fn registry_entry(node_id: &str, public_key_hex: &str, active: bool) -> ValidatorRegistryEntry {
    ValidatorRegistryEntry {
        node_id: node_id.to_string(),
        algorithm_id: "ML-DSA-65".to_string(),
        public_key_hex: public_key_hex.to_string(),
        active,
    }
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

fn next_graph(
    domain: &CobaltDomain,
    previous: &TrustGraph,
    graph_version: u64,
    registry_root: String,
    activation_height: u64,
) -> Result<TrustGraph, String> {
    build_trust_graph(
        domain,
        graph_version,
        registry_root,
        activation_height,
        Some(previous.trust_graph_root.clone()),
        previous.trust_views.clone(),
    )
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

fn ratify_payload(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    payload_hash: String,
    slot: u64,
    activation_height: u64,
) -> Result<postfiat_consensus_cobalt::DabcRatifiedAmendment, String> {
    let propose = build_rbc_propose(
        domain,
        graph.trust_graph_root.clone(),
        "validator-0",
        slot,
        payload_hash,
        "",
    )?;
    let accept = build_rbc_accept(domain, &propose, "validator-2", "")?;
    let candidate = mvba_candidate_from_rbc_accept(domain, &propose, &accept)?;
    let input_set = build_mvba_valid_input_set(
        domain,
        view(graph, "validator-2")?,
        hash_hex(
            "postfiat.test.cobalt.key_compromise_recovery.agreement.v1",
            &slot.to_le_bytes(),
        ),
        vec![candidate],
    )?;
    ratify_dabc_amendment(domain, graph, &input_set, None, activation_height)
}

fn certify_and_bind(
    domain: &CobaltDomain,
    graph: &TrustGraph,
    config: &EssentialSubsetConfig,
    input: CertifyAndBindInput,
) -> Result<(ValidatorRegistryUpdateRecord, LifecycleStep), String> {
    let update = certify_validator_registry_update_with_trust_graph_transition(
        domain,
        config,
        input.request,
        input.transition,
        input.support.clone(),
    )?;
    verify_validator_registry_update(domain, &update)?;
    let payload_hash = validator_registry_lifecycle_payload_hash(domain, &update)?;
    let ratified = ratify_payload(
        domain,
        graph,
        payload_hash,
        input.slot,
        update.activation_height,
    )?;
    let lifecycle = bind_dabc_ratification_to_validator_registry_update(
        domain, graph, &ratified, None, &update,
    )?;
    let previous_record = update
        .previous_record
        .as_ref()
        .ok_or_else(|| "missing previous record".to_string())?;
    let new_record = update
        .new_record
        .as_ref()
        .ok_or_else(|| "missing new record".to_string())?;
    let step = LifecycleStep {
        name: input.name,
        operation: lifecycle.operation.clone(),
        previous_registry_root: lifecycle.previous_registry_root.clone(),
        new_registry_root: lifecycle.new_registry_root.clone(),
        previous_trust_graph_root: update.previous_trust_graph_root.clone().unwrap_or_default(),
        new_trust_graph_root: update.new_trust_graph_root.clone().unwrap_or_default(),
        activation_height: lifecycle.activation_height,
        support: input.support,
        registry_update_id: lifecycle.registry_update_id.clone(),
        dabc_ratification_id: lifecycle.dabc_ratification_id.clone(),
        lifecycle_ratification_id: lifecycle.lifecycle_ratification_id.clone(),
        subject_previous_active: previous_record.active,
        subject_new_active: new_record.active,
        subject_previous_key: previous_record.public_key_hex.clone(),
        subject_new_key: new_record.public_key_hex.clone(),
        ok: lifecycle.registry_update_id == update.update_id
            && lifecycle.dabc_ratification_id == ratified.ratification_id
            && lifecycle.subject_node_id == SUBJECT,
    };
    Ok((update, step))
}

fn rejection(
    name: &'static str,
    expected: &'static str,
    result: Result<(), String>,
) -> RejectionScenario {
    let observed_error = result
        .err()
        .unwrap_or_else(|| "unexpected success".to_string());
    RejectionScenario {
        name,
        expected,
        ok: observed_error.contains(expected),
        observed_error,
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (domain, g0) = fixture()?;
    let g1 = next_graph(&domain, &g0, 3, root('c'), 40)?;
    let g2 = next_graph(&domain, &g1, 4, root('d'), 50)?;
    let g3 = next_graph(&domain, &g2, 5, root('e'), 60)?;

    let config_all = EssentialSubsetConfig {
        validators: validators(7),
        quorum: 5,
    };
    let config_suspended = EssentialSubsetConfig {
        validators: active_without_subject(),
        quorum: 5,
    };

    let suspend_request = ValidatorRegistryUpdateRequest {
        activation_height: 40,
        previous_registry_root: g0.registry_root.clone(),
        new_registry_root: g1.registry_root.clone(),
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators: validators(7),
        new_validators: active_without_subject(),
        operation: VALIDATOR_REGISTRY_OP_SUSPEND.to_string(),
        subject_node_id: SUBJECT.to_string(),
        previous_record: Some(registry_entry(SUBJECT, OLD_COMPROMISED_KEY, true)),
        new_record: Some(registry_entry(SUBJECT, OLD_COMPROMISED_KEY, false)),
    };
    let suspend_transition = build_trust_graph_transition(
        &domain,
        g0.registry_root.clone(),
        g1.registry_root.clone(),
        g0.trust_graph_root.clone(),
        g1.trust_graph_root.clone(),
        40,
    )?;
    let (_suspend_update, suspend_step) = certify_and_bind(
        &domain,
        &g0,
        &config_all,
        CertifyAndBindInput {
            request: suspend_request,
            transition: suspend_transition,
            support: support_without_subject(),
            slot: 410,
            name: "suspend_compromised_validator",
        },
    )?;

    let rotate_request = ValidatorRegistryUpdateRequest {
        activation_height: 50,
        previous_registry_root: g1.registry_root.clone(),
        new_registry_root: g2.registry_root.clone(),
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators: active_without_subject(),
        new_validators: active_without_subject(),
        operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: SUBJECT.to_string(),
        previous_record: Some(registry_entry(SUBJECT, OLD_COMPROMISED_KEY, false)),
        new_record: Some(registry_entry(SUBJECT, ROTATED_KEY, false)),
    };
    let rotate_transition = build_trust_graph_transition(
        &domain,
        g1.registry_root.clone(),
        g2.registry_root.clone(),
        g1.trust_graph_root.clone(),
        g2.trust_graph_root.clone(),
        50,
    )?;
    let stale_support_error = {
        let compromised_support = vec![
            "validator-0".to_string(),
            SUBJECT.to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
            "validator-4".to_string(),
        ];
        certify_validator_registry_update_with_trust_graph_transition(
            &domain,
            &config_suspended,
            rotate_request.clone(),
            rotate_transition.clone(),
            compromised_support,
        )
        .map(|_| ())
    };
    let (rotate_update, rotate_step) = certify_and_bind(
        &domain,
        &g1,
        &config_suspended,
        CertifyAndBindInput {
            request: rotate_request,
            transition: rotate_transition,
            support: support_without_subject(),
            slot: 510,
            name: "rotate_key_while_inactive",
        },
    )?;

    let tampered_vote_error = {
        let mut tampered = rotate_update.clone();
        tampered.support = vec![
            "validator-0".to_string(),
            SUBJECT.to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
            "validator-4".to_string(),
            "validator-5".to_string(),
        ];
        verify_validator_registry_update(&domain, &tampered).map(|_| ())
    };

    let reactivate_request = ValidatorRegistryUpdateRequest {
        activation_height: 60,
        previous_registry_root: g2.registry_root.clone(),
        new_registry_root: g3.registry_root.clone(),
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators: active_without_subject(),
        new_validators: validators(7),
        operation: VALIDATOR_REGISTRY_OP_REACTIVATE.to_string(),
        subject_node_id: SUBJECT.to_string(),
        previous_record: Some(registry_entry(SUBJECT, ROTATED_KEY, false)),
        new_record: Some(registry_entry(SUBJECT, ROTATED_KEY, true)),
    };
    let reactivate_transition = build_trust_graph_transition(
        &domain,
        g2.registry_root.clone(),
        g3.registry_root.clone(),
        g2.trust_graph_root.clone(),
        g3.trust_graph_root.clone(),
        60,
    )?;
    let (_reactivate_update, reactivate_step) = certify_and_bind(
        &domain,
        &g2,
        &config_suspended,
        CertifyAndBindInput {
            request: reactivate_request,
            transition: reactivate_transition,
            support: support_without_subject(),
            slot: 610,
            name: "reactivate_rotated_key",
        },
    )?;

    let old_key_reactivation_error = {
        let bad_request = ValidatorRegistryUpdateRequest {
            activation_height: 60,
            previous_registry_root: g2.registry_root.clone(),
            new_registry_root: g3.registry_root.clone(),
            previous_trust_graph_root: None,
            new_trust_graph_root: None,
            trust_graph_transition_id: None,
            previous_validators: active_without_subject(),
            new_validators: validators(7),
            operation: VALIDATOR_REGISTRY_OP_REACTIVATE.to_string(),
            subject_node_id: SUBJECT.to_string(),
            previous_record: Some(registry_entry(SUBJECT, ROTATED_KEY, false)),
            new_record: Some(registry_entry(SUBJECT, OLD_COMPROMISED_KEY, true)),
        };
        let transition = build_trust_graph_transition(
            &domain,
            g2.registry_root.clone(),
            g3.registry_root.clone(),
            g2.trust_graph_root.clone(),
            g3.trust_graph_root.clone(),
            60,
        )?;
        certify_validator_registry_update_with_trust_graph_transition(
            &domain,
            &config_suspended,
            bad_request,
            transition,
            support_without_subject(),
        )
        .map(|_| ())
    };

    let network_before =
        build_transaction_network_membership(&domain, &g0, 1, validators(7), 5, 10)?;
    let network_suspended =
        build_transaction_network_membership(&domain, &g1, 2, active_without_subject(), 5, 40)?;
    validate_transaction_network_transition(&network_before, &network_suspended)?;
    let old_binding_at_suspension =
        build_cobalt_block_membership_binding(&domain, &network_before, root('1'), 40, SUBJECT)?;
    let old_proposer_after_suspension =
        validate_cobalt_block_against_transaction_network_transition(
            &domain,
            &network_before,
            &network_suspended,
            &old_binding_at_suspension,
        );
    let direct_suspended_proposer =
        build_cobalt_block_membership_binding(&domain, &network_suspended, root('2'), 41, SUBJECT)
            .map(|_| ());

    let network_rotated =
        build_transaction_network_membership(&domain, &g2, 3, active_without_subject(), 5, 50)?;
    let network_reactivated =
        build_transaction_network_membership(&domain, &g3, 4, validators(7), 5, 60)?;
    validate_transaction_network_transition(&network_rotated, &network_reactivated)?;
    let reactivated_binding = build_cobalt_block_membership_binding(
        &domain,
        &network_reactivated,
        root('3'),
        60,
        SUBJECT,
    )?;
    validate_cobalt_block_against_transaction_network_transition(
        &domain,
        &network_rotated,
        &network_reactivated,
        &reactivated_binding,
    )?;

    let lifecycle_steps = vec![suspend_step, rotate_step, reactivate_step];
    let rejections = vec![
        rejection(
            "stale_compromised_support_rejected_after_suspension",
            "insufficient registry update support",
            stale_support_error,
        ),
        rejection(
            "tampered_compromised_vote_rejected_after_rotation",
            "support includes non-validator",
            tampered_vote_error,
        ),
        rejection(
            "old_compromised_key_reactivation_rejected",
            "reactivation cannot rotate key material",
            old_key_reactivation_error,
        ),
        rejection(
            "compromised_proposer_rejected_after_suspension",
            "outside transaction network",
            old_proposer_after_suspension,
        ),
        rejection(
            "direct_suspended_proposer_rejected",
            "outside transaction network",
            direct_suspended_proposer,
        ),
    ];
    let step_failures = lifecycle_steps
        .iter()
        .filter(|step| !step.ok)
        .map(|step| step.name)
        .collect::<Vec<_>>();
    let rejection_failures = rejections
        .iter()
        .filter(|scenario| !scenario.ok)
        .map(|scenario| scenario.name)
        .collect::<Vec<_>>();
    let ok = step_failures.is_empty() && rejection_failures.is_empty();
    let generated_at_unix_seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let report = json!({
        "schema": "postfiat-testnet-cobalt-key-compromise-recovery-v1",
        "generated_at_unix_seconds": generated_at_unix_seconds,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "controlled-pre-testnet",
        "validator_count": g0.trust_views.len(),
        "subject_validator": SUBJECT,
        "initial_trust_graph_root": g0.trust_graph_root,
        "final_trust_graph_root": g3.trust_graph_root,
        "checks": {
            "seven_logical_validators": g0.trust_views.len() == 7,
            "suspension_ratified_by_dabc": lifecycle_steps.iter().any(|step| step.name == "suspend_compromised_validator" && step.ok),
            "inactive_key_rotation_ratified_by_dabc": lifecycle_steps.iter().any(|step| step.name == "rotate_key_while_inactive" && step.ok),
            "reactivation_ratified_by_dabc": lifecycle_steps.iter().any(|step| step.name == "reactivate_rotated_key" && step.ok),
            "stale_compromised_support_rejected": rejections.iter().any(|scenario| scenario.name == "stale_compromised_support_rejected_after_suspension" && scenario.ok),
            "tampered_compromised_vote_rejected": rejections.iter().any(|scenario| scenario.name == "tampered_compromised_vote_rejected_after_rotation" && scenario.ok),
            "old_compromised_key_reactivation_rejected": rejections.iter().any(|scenario| scenario.name == "old_compromised_key_reactivation_rejected" && scenario.ok),
            "compromised_proposer_rejected_while_suspended": rejections.iter().any(|scenario| scenario.name == "compromised_proposer_rejected_after_suspension" && scenario.ok)
                && rejections.iter().any(|scenario| scenario.name == "direct_suspended_proposer_rejected" && scenario.ok),
            "reactivated_validator_can_propose_after_new_key_lifecycle": reactivated_binding.proposer == SUBJECT,
            "outside_operators_required": false,
        },
        "step_failures": step_failures,
        "rejection_failures": rejection_failures,
        "lifecycle_steps": lifecycle_steps,
        "rejections": rejections,
        "reactivated_block_binding": reactivated_binding,
    });
    postfiat_consensus_cobalt::emit_example_report(&report)?;
    if ok {
        Ok(())
    } else {
        Err("Cobalt key compromise recovery report failed".into())
    }
}
