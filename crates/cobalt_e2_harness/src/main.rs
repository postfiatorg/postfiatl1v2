use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_canonical_unl_trust_graph, build_essential_subset,
    build_trust_graph, build_trust_view, mvba_candidate_from_rbc_accept, sign_abba_aux,
    sign_abba_conf, sign_abba_finish, sign_abba_init, sign_dabc_full_knowledge_check,
    sign_rbc_accept, sign_rbc_echo, sign_rbc_propose, sign_rbc_ready, validate_abba_aux_signed,
    validate_abba_conf_signed, validate_abba_finish_signed, validate_abba_init_signed,
    validate_dabc_full_knowledge_check_signed, validate_rbc_accept_signed,
    validate_rbc_echo_signed, validate_rbc_propose_signed, validate_rbc_ready_signed, AbbaAux,
    AbbaConf, AbbaFinish, AbbaInit, CobaltDomain, CobaltFaultModel, CobaltSignatureCommittee,
    DabcFullKnowledgeCheck, DabcPendingPair, RbcAccept, RbcEcho, RbcPropose, RbcReady, TrustGraph,
};
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, ml_dsa_65_keygen_from_seed, MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
};
use postfiat_node::cobalt_shadow::{
    assemble_protocol_transcript, CobaltShadowProtocolContribution, CobaltShadowProtocolTranscript,
    CobaltShadowRegistryBinding, COBALT_SHADOW_PROTOCOL_CONTRIBUTION_SCHEMA,
    COBALT_SHADOW_REGISTRY_BINDING_SCHEMA,
};
use postfiat_node::{ValidatorRegistry, ValidatorRegistryRecord};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

const MANIFEST_SCHEMA: &str = "postfiat-cobalt-adversarial-e2-campaign-manifest-v1";
const REPORT_SCHEMA: &str = "postfiat-cobalt-adversarial-e2-campaign-v1";
const VALIDATOR_COUNT: usize = 6;
const QUORUM: usize = 5;
const FAULT_BOUND: usize = 1;
const STAGE_COUNT: u64 = 4;
const E2_STRATEGIES: [&str; 18] = [
    "rbc_propose_equivocation",
    "rbc_echo_equivocation",
    "rbc_ready_equivocation",
    "rbc_accept_equivocation",
    "abba_init_equivocation",
    "abba_aux_equivocation",
    "abba_conf_equivocation",
    "abba_finish_equivocation",
    "mvba_candidate_equivocation",
    "dabc_full_knowledge_equivocation",
    "combined_all_stages",
    "selective_withholding",
    "trust_view_lie",
    "trust_view_change",
    "competing_proposals",
    "late_vote",
    "reproposal",
    "incompatible_trust_boundary",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidenceSource {
    path: String,
    sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LiveBinding {
    chain_id: String,
    genesis_hash: String,
    registry_root: String,
    trust_graph_root: String,
    topology_trust_graph_root: String,
    topology_source: EvidenceSource,
    activation_source: EvidenceSource,
    validators: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FaultModel {
    validator_count: usize,
    quorum: usize,
    local_max_active_byzantine: usize,
    derived_f: usize,
    first_inequality: String,
    second_inequality: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduleSearch {
    seed_hex: String,
    schedules_per_case: usize,
    max_delay_steps: u64,
    max_partition_steps: u64,
    max_duplicate_copies: u64,
    synchrony_bound_steps: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignManifest {
    schema: String,
    campaign_id: String,
    frozen_at: String,
    live_binding: LiveBinding,
    fault_model: FaultModel,
    schedule_search: ScheduleSearch,
    strategies: Vec<String>,
    expected_case_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct SourceAudit {
    topology_source_sha256: String,
    activation_source_sha256: String,
    matching_topology_graphs: usize,
    trust_view_count: usize,
    subset_validator_count: usize,
    subset_quorum: usize,
    subset_max_active_byzantine: usize,
    key_rotation_preserved_membership: bool,
    current_registry_root: String,
    current_trust_graph_root: String,
    derived_f: usize,
    inequalities_hold: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EvidencePair {
    kind: String,
    signer: String,
    left: Value,
    right: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    left_context: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    right_context: Option<Value>,
    left_message_id: String,
    right_message_id: String,
    signature_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WorstSchedule {
    candidate_index: usize,
    honest_stage_delays: Vec<Vec<u64>>,
    honest_delivery_order: Vec<String>,
    drop_byzantine_messages: bool,
    duplicate_copies: u64,
    reverse_equal_delay_order: bool,
    pre_synchrony_partition_mask: u8,
    partition_heal_step: u64,
    honest_decision_steps: BTreeMap<String, Option<u64>>,
    delivered_event_count: u64,
    duplicate_event_count: u64,
    pre_heal_blocked_event_count: u64,
    byzantine_conflicting_support: usize,
    decision_step: Option<u64>,
    score: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SearchCoverage {
    candidates: usize,
    executed_event_schedules: usize,
    delivered_event_count: u64,
    duplicate_event_count: u64,
    pre_heal_blocked_event_count: u64,
    conflicting_root_schedules: usize,
    false_accept_schedules: usize,
    false_halt_schedules: usize,
    synchrony_violation_schedules: usize,
    delay_varied: bool,
    drop_varied: bool,
    reorder_varied: bool,
    duplicate_varied: bool,
    partition_varied: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ValidatorOutcome {
    validator: String,
    correct: bool,
    decided: bool,
    decision_step: Option<u64>,
    accepted_registry_root: Option<String>,
    rejection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignCase {
    case_id: String,
    byzantine_validator: String,
    strategy: String,
    compatible: bool,
    expectation: String,
    search_coverage: SearchCoverage,
    worst_schedule: WorstSchedule,
    signed_evidence: Vec<EvidencePair>,
    per_validator: Vec<ValidatorOutcome>,
    accepted_registry_roots: Vec<String>,
    production_transcript_accepted: bool,
    duplicate_contributor_rejected: bool,
    conflicting_transcript_rejected: bool,
    registry_mutated_on_rejection: bool,
    conflicting_root_count: usize,
    false_accept: bool,
    false_halt: bool,
    synchrony_violation: bool,
    ok: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CampaignSummary {
    schema: String,
    manifest_sha256: String,
    source_revision: String,
    case_count: usize,
    compatible_case_count: usize,
    incompatible_case_count: usize,
    schedule_candidates: usize,
    delivered_event_count: u64,
    duplicate_event_count: u64,
    pre_heal_blocked_event_count: u64,
    conflicting_root_schedule_count: usize,
    false_accept_schedule_count: usize,
    false_halt_schedule_count: usize,
    synchrony_violation_schedule_count: usize,
    signed_evidence_pairs: usize,
    signed_evidence_verified: bool,
    conflicting_root_count: usize,
    false_accept_count: usize,
    false_halt_count: usize,
    synchrony_violation_count: usize,
    rejected_state_mutation_count: usize,
    classification_sha256: String,
    summary_only: bool,
    pass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CampaignReport {
    summary: CampaignSummary,
    source_audit: SourceAudit,
    cases: Vec<CampaignCase>,
}

#[derive(Debug, Clone, Serialize)]
struct ClassificationFingerprint<'a> {
    case_id: &'a str,
    byzantine_validator: &'a str,
    strategy: &'a str,
    compatible: bool,
    accepted_registry_roots: &'a [String],
    decision_step: Option<u64>,
    evidence_ids: Vec<(&'a str, &'a str, &'a str)>,
    conflicting_root_count: usize,
    false_accept: bool,
    false_halt: bool,
    synchrony_violation: bool,
    conflicting_transcript_rejected: bool,
    schedule_failures: usize,
    ok: bool,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> io::Result<T> {
    serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
}

fn write_new_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = File::options().create_new(true).write(true).open(path)?;
    serde_json::to_writer_pretty(&mut file, value).map_err(io::Error::other)?;
    file.write_all(b"\n")
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    format!("{:x}", digest.finalize())
}

fn sha256_file(path: &Path) -> io::Result<String> {
    fs::read(path).map(|bytes| sha256_bytes(&bytes))
}

fn validators() -> Vec<String> {
    (0..VALIDATOR_COUNT)
        .map(|index| format!("validator-{index}"))
        .collect()
}

fn validate_source_revision(revision: &str) -> io::Result<()> {
    if revision.len() != 40
        || !revision
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid(
            "source revision must be 40 lowercase hex characters",
        ));
    }
    Ok(())
}

fn validate_manifest_shape(manifest: &CampaignManifest) -> io::Result<()> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(invalid("unsupported E2 campaign manifest schema"));
    }
    if manifest.live_binding.validators != validators() {
        return Err(invalid(
            "E2 manifest validators do not match the live six-validator ids",
        ));
    }
    let fault = &manifest.fault_model;
    if fault.validator_count != VALIDATOR_COUNT
        || fault.quorum != QUORUM
        || fault.local_max_active_byzantine != FAULT_BOUND
        || fault.derived_f != FAULT_BOUND
    {
        return Err(invalid(
            "E2 manifest fault bound is not the live 6/5/1 profile",
        ));
    }
    let first_rhs = fault
        .quorum
        .checked_mul(2)
        .and_then(|value| value.checked_sub(fault.validator_count))
        .ok_or_else(|| invalid("fault-bound inequality overflow"))?;
    if fault.local_max_active_byzantine >= first_rhs
        || fault.local_max_active_byzantine.saturating_mul(2) >= fault.quorum
    {
        return Err(invalid(
            "E2 manifest violates the Cobalt local subset inequalities",
        ));
    }
    let expected_strategies = E2_STRATEGIES
        .iter()
        .map(|strategy| (*strategy).to_string())
        .collect::<Vec<_>>();
    if manifest.strategies != expected_strategies {
        return Err(invalid(
            "E2 manifest strategy corpus does not match the locked campaign",
        ));
    }
    if manifest.schedule_search.schedules_per_case == 0
        || manifest.schedule_search.max_delay_steps == 0
        || manifest.schedule_search.max_partition_steps == 0
        || manifest.schedule_search.max_duplicate_copies == 0
    {
        return Err(invalid(
            "E2 schedule-search space must vary every adversarial dimension",
        ));
    }
    let worst_bound = manifest.schedule_search.max_partition_steps.saturating_add(
        STAGE_COUNT.saturating_mul(manifest.schedule_search.max_delay_steps.saturating_add(1)),
    );
    if manifest.schedule_search.synchrony_bound_steps < worst_bound {
        return Err(invalid(
            "E2 synchrony bound is below the declared search envelope",
        ));
    }
    let expected = manifest
        .strategies
        .len()
        .checked_mul(VALIDATOR_COUNT)
        .ok_or_else(|| invalid("E2 expected case count overflow"))?;
    if manifest.expected_case_count != expected {
        return Err(invalid(
            "E2 expected case count does not cover every validator/strategy pair",
        ));
    }
    if manifest.schedule_search.seed_hex.len() != 64
        || !manifest
            .schedule_search
            .seed_hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid("E2 schedule seed must be 32-byte lowercase hex"));
    }
    Ok(())
}

fn find_topology_graphs(value: &Value, root: &str, matches: &mut Vec<Value>) {
    match value {
        Value::Object(object) => {
            if object
                .get("trust_graph_root")
                .and_then(Value::as_str)
                .is_some_and(|candidate| candidate == root)
                && object.get("trust_views").is_some_and(Value::is_array)
            {
                matches.push(value.clone());
            }
            for child in object.values() {
                find_topology_graphs(child, root, matches);
            }
        }
        Value::Array(values) => {
            for child in values {
                find_topology_graphs(child, root, matches);
            }
        }
        _ => {}
    }
}

fn audit_sources(repository: &Path, manifest: &CampaignManifest) -> io::Result<SourceAudit> {
    validate_manifest_shape(manifest)?;
    let topology_path = repository.join(&manifest.live_binding.topology_source.path);
    let activation_path = repository.join(&manifest.live_binding.activation_source.path);
    let topology_sha256 = sha256_file(&topology_path)?;
    let activation_sha256 = sha256_file(&activation_path)?;
    if topology_sha256 != manifest.live_binding.topology_source.sha256 {
        return Err(invalid("E2 topology source SHA-256 mismatch"));
    }
    if activation_sha256 != manifest.live_binding.activation_source.sha256 {
        return Err(invalid("E2 activation source SHA-256 mismatch"));
    }

    let topology: Value = read_json(&topology_path)?;
    let mut matching = Vec::new();
    find_topology_graphs(
        &topology,
        &manifest.live_binding.topology_trust_graph_root,
        &mut matching,
    );
    if matching.is_empty() {
        return Err(invalid("E2 topology source lacks the pinned trust graph"));
    }
    for graph in &matching {
        let views = graph
            .get("trust_views")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("pinned topology graph lacks trust views"))?;
        if views.len() != VALIDATOR_COUNT {
            return Err(invalid("pinned topology graph does not contain six views"));
        }
        for view in views {
            let subsets = view
                .get("essential_subsets")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("pinned topology trust view lacks essential subsets"))?;
            if subsets.len() != 1 {
                return Err(invalid(
                    "pinned topology trust view is not canonical single-subset",
                ));
            }
            let subset = &subsets[0];
            if subset.get("validator_count").and_then(Value::as_u64) != Some(6)
                || subset.get("quorum").and_then(Value::as_u64) != Some(5)
                || subset.get("max_active_byzantine").and_then(Value::as_u64) != Some(1)
            {
                return Err(invalid(
                    "pinned topology does not carry the 6/5/1 fault row",
                ));
            }
            let members = subset
                .get("validators")
                .and_then(Value::as_array)
                .ok_or_else(|| invalid("pinned topology subset lacks validators"))?
                .iter()
                .map(|value| value.as_str().unwrap_or_default().to_string())
                .collect::<Vec<_>>();
            if members != manifest.live_binding.validators {
                return Err(invalid("pinned topology subset membership mismatch"));
            }
        }
    }

    let activation: Value = read_json(&activation_path)?;
    let update = activation
        .get("latest_registry_update")
        .ok_or_else(|| invalid("activation source lacks latest registry update"))?;
    let active = activation
        .get("verifier")
        .and_then(|value| value.get("active_validators"))
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("activation source lacks active validators"))?
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect::<Vec<_>>();
    let key_rotation_preserved_membership = update.get("operation").and_then(Value::as_str)
        == Some("rotate_key")
        && update.get("subject_node_id").and_then(Value::as_str) == Some("validator-5")
        && active == manifest.live_binding.validators;
    if !key_rotation_preserved_membership {
        return Err(invalid(
            "height-917 update did not preserve the pinned membership",
        ));
    }
    if activation.get("registry_root").and_then(Value::as_str)
        != Some(manifest.live_binding.registry_root.as_str())
        || activation.get("trust_graph_root").and_then(Value::as_str)
            != Some(manifest.live_binding.trust_graph_root.as_str())
        || activation
            .get("node")
            .and_then(|value| value.get("chain_id"))
            .and_then(Value::as_str)
            != Some(manifest.live_binding.chain_id.as_str())
    {
        return Err(invalid("activation source current roots/domain mismatch"));
    }

    Ok(SourceAudit {
        topology_source_sha256: topology_sha256,
        activation_source_sha256: activation_sha256,
        matching_topology_graphs: matching.len(),
        trust_view_count: VALIDATOR_COUNT,
        subset_validator_count: VALIDATOR_COUNT,
        subset_quorum: QUORUM,
        subset_max_active_byzantine: FAULT_BOUND,
        key_rotation_preserved_membership,
        current_registry_root: manifest.live_binding.registry_root.clone(),
        current_trust_graph_root: manifest.live_binding.trust_graph_root.clone(),
        derived_f: FAULT_BOUND,
        inequalities_hold: FAULT_BOUND < (2 * QUORUM - VALIDATOR_COUNT) && 2 * FAULT_BOUND < QUORUM,
    })
}

struct Fixture {
    domain: CobaltDomain,
    graph: TrustGraph,
    incompatible_graph: TrustGraph,
    binding: CobaltShadowRegistryBinding,
    committee: CobaltSignatureCommittee,
    keys: BTreeMap<String, MlDsa65KeyPair>,
}

fn deterministic_seed(campaign_seed: &str, validator: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"postfiat-cobalt-e2-validator-key-v1\0");
    digest.update(campaign_seed.as_bytes());
    digest.update(b"\0");
    digest.update(validator.as_bytes());
    digest.finalize().into()
}

fn registry_root(
    registry: &ValidatorRegistry,
    expected_validators: &[String],
) -> io::Result<String> {
    let rows = expected_validators
        .iter()
        .map(|validator| {
            let record = registry
                .validators
                .iter()
                .find(|record| &record.node_id == validator)
                .ok_or_else(|| {
                    invalid(format!("missing simulation registry record {validator}"))
                })?;
            Ok((
                record.node_id.as_str(),
                record.algorithm_id.as_str(),
                record.public_key_hex.as_str(),
            ))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let encoded = serde_json::to_vec(&rows).map_err(io::Error::other)?;
    Ok(hash_hex("postfiat.validator_registry.root.v1", &encoded))
}

fn incompatible_graph(
    domain: &CobaltDomain,
    registry_root: &str,
    validator_ids: &[String],
) -> io::Result<TrustGraph> {
    let left = build_essential_subset(
        domain,
        validator_ids[..3].to_vec(),
        0,
        3,
        Vec::new(),
        916,
        None,
    )
    .map_err(invalid)?;
    let right = build_essential_subset(
        domain,
        validator_ids[3..].to_vec(),
        0,
        3,
        Vec::new(),
        916,
        None,
    )
    .map_err(invalid)?;
    let views = validator_ids
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            build_trust_view(
                domain,
                validator,
                2,
                vec![if index < 3 {
                    left.clone()
                } else {
                    right.clone()
                }],
                "",
            )
            .map_err(invalid)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let graph = build_trust_graph(domain, 2, registry_root.to_string(), 916, None, views)
        .map_err(invalid)?;
    let analysis =
        analyze_trust_graph(domain, &graph, &CobaltFaultModel::default()).map_err(invalid)?;
    if analysis.unsafe_pairs.is_empty() {
        return Err(invalid(
            "incompatible E2 graph unexpectedly has no unsafe pairs",
        ));
    }
    Ok(graph)
}

fn fixture(manifest: &CampaignManifest) -> io::Result<Fixture> {
    let domain = CobaltDomain {
        chain_id: manifest.live_binding.chain_id.clone(),
        genesis_hash: manifest.live_binding.genesis_hash.clone(),
        protocol_version: 1,
    };
    let validator_ids = manifest.live_binding.validators.clone();
    let mut keys = BTreeMap::new();
    let mut committee = CobaltSignatureCommittee::default();
    let mut peers = BTreeMap::new();
    let mut records = Vec::new();
    for validator in &validator_ids {
        let key = ml_dsa_65_keygen_from_seed(&deterministic_seed(
            &manifest.schedule_search.seed_hex,
            validator,
        ));
        committee
            .insert(validator.clone(), &key.public_key)
            .map_err(invalid)?;
        let public_key_hex = bytes_to_hex(&key.public_key);
        peers.insert(validator.clone(), public_key_hex.clone());
        records.push(ValidatorRegistryRecord {
            node_id: validator.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex,
        });
        keys.insert(validator.clone(), key);
    }
    let validator_registry = ValidatorRegistry {
        validators: records,
    };
    let simulation_registry_root = registry_root(&validator_registry, &validator_ids)?;
    let graph = build_canonical_unl_trust_graph(
        &domain,
        1,
        simulation_registry_root.clone(),
        916,
        None,
        validator_ids.clone(),
        QUORUM,
    )
    .map_err(invalid)?;
    let graph_analysis =
        analyze_trust_graph(&domain, &graph, &CobaltFaultModel::default()).map_err(invalid)?;
    if !graph_analysis.unsafe_pairs.is_empty() {
        return Err(invalid("canonical E2 simulation graph is unsafe"));
    }
    let incompatible_graph =
        incompatible_graph(&domain, &simulation_registry_root, &validator_ids)?;
    let binding = CobaltShadowRegistryBinding {
        schema: COBALT_SHADOW_REGISTRY_BINDING_SCHEMA.to_string(),
        registry_root: simulation_registry_root,
        active_validators: validator_ids,
        validator_registry,
        trust_graph: graph.clone(),
        peers,
        validator_bindings: Vec::new(),
    };
    Ok(Fixture {
        domain,
        graph,
        incompatible_graph,
        binding,
        committee,
        keys,
    })
}

fn agreement_id(graph_root: &str, round: u64, propose_message_id: &str) -> io::Result<String> {
    let encoded =
        serde_json::to_vec(&(round, propose_message_id, graph_root)).map_err(io::Error::other)?;
    Ok(hash_hex("postfiat.cobalt.shadow.agreement.v1", &encoded))
}

fn key<'a>(fixture: &'a Fixture, validator: &str) -> io::Result<&'a MlDsa65KeyPair> {
    fixture
        .keys
        .get(validator)
        .ok_or_else(|| invalid(format!("missing E2 key for {validator}")))
}

fn signed_contribution(
    fixture: &Fixture,
    propose: &RbcPropose,
    validator: &str,
) -> io::Result<CobaltShadowProtocolContribution> {
    let private_key = key(fixture, validator)?.private_key.as_slice();
    let agreement = agreement_id(
        &fixture.graph.trust_graph_root,
        propose.amendment_slot,
        &propose.message_id,
    )?;
    let rbc_echo =
        sign_rbc_echo(&fixture.domain, propose, validator, private_key).map_err(invalid)?;
    let rbc_ready =
        sign_rbc_ready(&fixture.domain, propose, validator, private_key).map_err(invalid)?;
    let rbc_accept =
        sign_rbc_accept(&fixture.domain, propose, validator, private_key).map_err(invalid)?;
    let abba_init = sign_abba_init(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        validator,
        agreement.clone(),
        propose.amendment_slot,
        true,
        private_key,
    )
    .map_err(invalid)?;
    let abba_aux = sign_abba_aux(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        validator,
        agreement.clone(),
        propose.amendment_slot,
        true,
        private_key,
    )
    .map_err(invalid)?;
    let abba_conf = sign_abba_conf(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        validator,
        agreement.clone(),
        propose.amendment_slot,
        true,
        private_key,
    )
    .map_err(invalid)?;
    let abba_finish = sign_abba_finish(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        validator,
        agreement.clone(),
        propose.amendment_slot,
        true,
        private_key,
    )
    .map_err(invalid)?;
    let candidate =
        mvba_candidate_from_rbc_accept(&fixture.domain, propose, &rbc_accept).map_err(invalid)?;
    let full_knowledge_check = sign_dabc_full_knowledge_check(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        validator,
        propose.amendment_slot.saturating_add(1),
        vec![DabcPendingPair {
            amendment_slot: propose.amendment_slot,
            output_candidate_id: candidate.candidate_id,
        }],
        private_key,
    )
    .map_err(invalid)?;
    Ok(CobaltShadowProtocolContribution {
        schema: COBALT_SHADOW_PROTOCOL_CONTRIBUTION_SCHEMA.to_string(),
        node_id: validator.to_string(),
        registry_root: fixture.binding.registry_root.clone(),
        trust_graph_root: fixture.graph.trust_graph_root.clone(),
        round: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        agreement_id: agreement,
        rbc_echo,
        rbc_ready,
        rbc_accept,
        abba_init,
        abba_aux,
        abba_conf,
        abba_finish,
        full_knowledge_check,
    })
}

fn honest_transcript(
    fixture: &Fixture,
    byzantine: &str,
    round: u64,
    payload_hash: &str,
    delivery_order: &[String],
) -> io::Result<(CobaltShadowProtocolTranscript, bool)> {
    let honest = validators()
        .into_iter()
        .filter(|validator| validator != byzantine)
        .collect::<Vec<_>>();
    let proposer = honest
        .first()
        .ok_or_else(|| invalid("E2 case has no honest proposer"))?;
    let propose = sign_rbc_propose(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        proposer,
        round,
        payload_hash,
        key(fixture, proposer)?.private_key.as_slice(),
    )
    .map_err(invalid)?;
    let mut contributions = delivery_order
        .iter()
        .filter(|validator| honest.contains(validator))
        .map(|validator| signed_contribution(fixture, &propose, validator))
        .collect::<io::Result<Vec<_>>>()?;
    if contributions.len() != QUORUM {
        return Err(invalid(
            "worst schedule did not deliver all five correct validators",
        ));
    }
    let mut duplicated = contributions.clone();
    duplicated.push(
        contributions
            .first()
            .ok_or_else(|| invalid("E2 transcript lacks contributions"))?
            .clone(),
    );
    let duplicate_rejected =
        assemble_protocol_transcript(&fixture.binding, propose.clone(), duplicated).is_err();
    contributions.reverse();
    let transcript = assemble_protocol_transcript(&fixture.binding, propose, contributions)?;
    if transcript.ratification.candidate.payload_hash != payload_hash {
        return Err(invalid(
            "production transcript selected the wrong registry root",
        ));
    }
    Ok((transcript, duplicate_rejected))
}

fn single_byzantine_transcript_rejected(
    fixture: &Fixture,
    byzantine: &str,
    round: u64,
    case_id: &str,
) -> io::Result<bool> {
    let payload_hash = hash_hex(
        "postfiat.cobalt.e2.conflicting-registry-root.v1",
        case_id.as_bytes(),
    );
    let propose = sign_rbc_propose(
        &fixture.domain,
        fixture.graph.trust_graph_root.clone(),
        byzantine,
        round,
        payload_hash,
        key(fixture, byzantine)?.private_key.as_slice(),
    )
    .map_err(invalid)?;
    let contribution = signed_contribution(fixture, &propose, byzantine)?;
    Ok(assemble_protocol_transcript(&fixture.binding, propose, vec![contribution]).is_err())
}

fn hash_u64(label: &str) -> u64 {
    let digest = Sha256::digest(label.as_bytes());
    u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 prefix"))
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e3779b97f4a7c15);
    let mut value = *state;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

fn partition_side(mask: u8, validator: &str) -> bool {
    validator
        .strip_prefix("validator-")
        .and_then(|index| index.parse::<u32>().ok())
        .and_then(|index| 1_u8.checked_shl(index))
        .is_some_and(|bit| mask & bit != 0)
}

fn search_schedules(
    manifest: &CampaignManifest,
    case_id: &str,
    byzantine: &str,
    compatible: bool,
) -> (WorstSchedule, SearchCoverage) {
    let honest = validators()
        .into_iter()
        .filter(|validator| validator != byzantine)
        .collect::<Vec<_>>();
    let search = &manifest.schedule_search;
    let mut worst: Option<WorstSchedule> = None;
    let mut seen_drop = BTreeSet::new();
    let mut seen_reorder = BTreeSet::new();
    let mut seen_duplicate = BTreeSet::new();
    let mut seen_partition = BTreeSet::new();
    let mut seen_delay = BTreeSet::new();
    let mut delivered_event_count = 0_u64;
    let mut duplicate_event_count = 0_u64;
    let mut pre_heal_blocked_event_count = 0_u64;
    let mut conflicting_root_schedules = 0_usize;
    let mut false_accept_schedules = 0_usize;
    let mut false_halt_schedules = 0_usize;
    let mut synchrony_violation_schedules = 0_usize;

    for index in 0..search.schedules_per_case {
        let mut state = hash_u64(&format!(
            "{}\0{}\0{}\0{index}",
            manifest.schedule_search.seed_hex, case_id, byzantine
        ));
        let mut stage_delays = Vec::new();
        for _ in 0..STAGE_COUNT {
            let delays = honest
                .iter()
                .map(|_| splitmix64(&mut state) % (search.max_delay_steps + 1))
                .collect::<Vec<_>>();
            seen_delay.extend(delays.iter().copied());
            stage_delays.push(delays);
        }
        let drop_byzantine_messages = splitmix64(&mut state) & 1 == 1;
        let duplicate_copies = splitmix64(&mut state) % (search.max_duplicate_copies + 1);
        let reverse_equal_delay_order = splitmix64(&mut state) & 1 == 1;
        let partition_mask = (splitmix64(&mut state) as u8) & 0x3f;
        let partition_heal_step = splitmix64(&mut state) % (search.max_partition_steps + 1);

        seen_drop.insert(drop_byzantine_messages);
        seen_reorder.insert(reverse_equal_delay_order);
        seen_duplicate.insert(duplicate_copies);
        seen_partition.insert(partition_mask);

        let mut order = honest
            .iter()
            .enumerate()
            .map(|(position, validator)| {
                let total_delay = stage_delays
                    .iter()
                    .map(|delays| delays[position])
                    .sum::<u64>();
                (total_delay, validator.clone())
            })
            .collect::<Vec<_>>();
        order.sort_by(|left, right| {
            left.0.cmp(&right.0).then_with(|| {
                if reverse_equal_delay_order {
                    right.1.cmp(&left.1)
                } else {
                    left.1.cmp(&right.1)
                }
            })
        });
        let honest_delivery_order = order
            .into_iter()
            .map(|(_, validator)| validator)
            .collect::<Vec<_>>();

        let mut candidate_delivered_events = 0_u64;
        let mut candidate_duplicate_events = 0_u64;
        let mut candidate_blocked_events = 0_u64;
        let mut honest_decision_steps = BTreeMap::new();
        for receiver in &honest {
            if !compatible {
                honest_decision_steps.insert(receiver.clone(), None);
                continue;
            }
            let mut stage_start = 0_u64;
            let mut decision_step = Some(0_u64);
            for delays in &stage_delays {
                let mut events = Vec::new();
                for (position, sender) in honest.iter().enumerate() {
                    let mut delivery_step = stage_start
                        .saturating_add(delays[position])
                        .saturating_add(1);
                    let split = partition_side(partition_mask, sender)
                        != partition_side(partition_mask, receiver);
                    if split && delivery_step < partition_heal_step {
                        candidate_blocked_events = candidate_blocked_events
                            .saturating_add(duplicate_copies.saturating_add(1));
                        delivery_step = partition_heal_step;
                    }
                    for copy in 0..=duplicate_copies {
                        events.push((delivery_step, sender.clone(), copy));
                    }
                }
                candidate_delivered_events =
                    candidate_delivered_events.saturating_add(events.len() as u64);
                candidate_duplicate_events = candidate_duplicate_events
                    .saturating_add((honest.len() as u64).saturating_mul(duplicate_copies));
                events.sort_by(|left, right| {
                    left.0
                        .cmp(&right.0)
                        .then_with(|| {
                            if reverse_equal_delay_order {
                                right.1.cmp(&left.1)
                            } else {
                                left.1.cmp(&right.1)
                            }
                        })
                        .then_with(|| left.2.cmp(&right.2))
                });
                let mut distinct_senders = BTreeSet::new();
                let mut stage_complete = None;
                for (step, sender, _) in events {
                    distinct_senders.insert(sender);
                    if distinct_senders.len() >= QUORUM {
                        stage_complete = Some(step);
                        break;
                    }
                }
                match stage_complete {
                    Some(step) => stage_start = step,
                    None => {
                        decision_step = None;
                        break;
                    }
                }
            }
            if decision_step.is_some() {
                decision_step = Some(stage_start);
            }
            honest_decision_steps.insert(receiver.clone(), decision_step);
        }

        let decision_step = honest_decision_steps.values().copied().flatten().max();
        let byzantine_conflicting_support =
            usize::from(!drop_byzantine_messages).saturating_mul(FAULT_BOUND);
        let conflicting_root = byzantine_conflicting_support >= QUORUM;
        let false_accept = (!compatible && decision_step.is_some()) || conflicting_root;
        let false_halt = compatible
            && honest_decision_steps
                .values()
                .any(|decision| decision.is_none());
        let synchrony_violation = compatible
            && honest_decision_steps
                .values()
                .any(|decision| decision.is_none_or(|step| step > search.synchrony_bound_steps));

        delivered_event_count = delivered_event_count.saturating_add(candidate_delivered_events);
        duplicate_event_count = duplicate_event_count.saturating_add(candidate_duplicate_events);
        pre_heal_blocked_event_count =
            pre_heal_blocked_event_count.saturating_add(candidate_blocked_events);
        conflicting_root_schedules += usize::from(conflicting_root);
        false_accept_schedules += usize::from(false_accept);
        false_halt_schedules += usize::from(false_halt);
        synchrony_violation_schedules += usize::from(synchrony_violation);

        let score = decision_step
            .unwrap_or(search.synchrony_bound_steps)
            .saturating_mul(1_000)
            .saturating_add(duplicate_copies.saturating_mul(10))
            .saturating_add(u64::from(reverse_equal_delay_order))
            .saturating_add(u64::from(partition_mask.count_ones()));
        let candidate = WorstSchedule {
            candidate_index: index,
            honest_stage_delays: stage_delays,
            honest_delivery_order,
            drop_byzantine_messages,
            duplicate_copies,
            reverse_equal_delay_order,
            pre_synchrony_partition_mask: partition_mask,
            partition_heal_step,
            honest_decision_steps,
            delivered_event_count: candidate_delivered_events,
            duplicate_event_count: candidate_duplicate_events,
            pre_heal_blocked_event_count: candidate_blocked_events,
            byzantine_conflicting_support,
            decision_step,
            score,
        };
        if worst.as_ref().is_none_or(|current| {
            (
                candidate.score,
                std::cmp::Reverse(candidate.candidate_index),
            ) > (current.score, std::cmp::Reverse(current.candidate_index))
        }) {
            worst = Some(candidate);
        }
    }

    (
        worst.expect("nonempty schedule search"),
        SearchCoverage {
            candidates: search.schedules_per_case,
            executed_event_schedules: search.schedules_per_case,
            delivered_event_count,
            duplicate_event_count,
            pre_heal_blocked_event_count,
            conflicting_root_schedules,
            false_accept_schedules,
            false_halt_schedules,
            synchrony_violation_schedules,
            delay_varied: seen_delay.len() > 1,
            drop_varied: seen_drop.len() > 1,
            reorder_varied: seen_reorder.len() > 1,
            duplicate_varied: seen_duplicate.len() > 1,
            partition_varied: seen_partition.len() > 1,
        },
    )
}

fn evidence_value<T: Serialize>(value: &T) -> io::Result<Value> {
    serde_json::to_value(value).map_err(io::Error::other)
}

fn evidence_pair<T: Serialize, U: Serialize>(
    kind: &str,
    signer: &str,
    left: &T,
    right: &T,
    left_context: Option<&U>,
    right_context: Option<&U>,
) -> io::Result<EvidencePair> {
    let left_value = evidence_value(left)?;
    let right_value = evidence_value(right)?;
    let left_message_id = left_value
        .get("message_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("left signed evidence lacks message id"))?
        .to_string();
    let right_message_id = right_value
        .get("message_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("right signed evidence lacks message id"))?
        .to_string();
    Ok(EvidencePair {
        kind: kind.to_string(),
        signer: signer.to_string(),
        left: left_value,
        right: right_value,
        left_context: left_context.map(evidence_value).transpose()?,
        right_context: right_context.map(evidence_value).transpose()?,
        left_message_id,
        right_message_id,
        signature_verified: false,
    })
}

fn signed_evidence(
    fixture: &Fixture,
    case_id: &str,
    byzantine: &str,
    strategy: &str,
    round: u64,
) -> io::Result<Vec<EvidencePair>> {
    let key = key(fixture, byzantine)?.private_key.as_slice();
    let left_payload = hash_hex(
        "postfiat.cobalt.e2.left-registry-root.v1",
        case_id.as_bytes(),
    );
    let right_payload = hash_hex(
        "postfiat.cobalt.e2.right-registry-root.v1",
        case_id.as_bytes(),
    );
    let left_graph_root = fixture.graph.trust_graph_root.clone();
    let right_graph_root =
        if strategy == "trust_view_change" || strategy == "incompatible_trust_boundary" {
            fixture.incompatible_graph.trust_graph_root.clone()
        } else {
            left_graph_root.clone()
        };
    let left_propose = sign_rbc_propose(
        &fixture.domain,
        left_graph_root.clone(),
        byzantine,
        round,
        left_payload,
        key,
    )
    .map_err(invalid)?;
    let right_propose = sign_rbc_propose(
        &fixture.domain,
        right_graph_root,
        byzantine,
        round,
        right_payload,
        key,
    )
    .map_err(invalid)?;
    let left_agreement = hash_hex("postfiat.cobalt.e2.agreement.left.v1", case_id.as_bytes());
    let right_agreement = left_agreement.clone();

    let add_rbc_propose = |kind: &str| -> io::Result<EvidencePair> {
        evidence_pair::<_, Value>(kind, byzantine, &left_propose, &right_propose, None, None)
    };
    let add_rbc_linked = |kind: &str, stage: &str| -> io::Result<EvidencePair> {
        match stage {
            "echo" => evidence_pair(
                kind,
                byzantine,
                &sign_rbc_echo(&fixture.domain, &left_propose, byzantine, key).map_err(invalid)?,
                &sign_rbc_echo(&fixture.domain, &right_propose, byzantine, key).map_err(invalid)?,
                Some(&left_propose),
                Some(&right_propose),
            ),
            "ready" => evidence_pair(
                kind,
                byzantine,
                &sign_rbc_ready(&fixture.domain, &left_propose, byzantine, key).map_err(invalid)?,
                &sign_rbc_ready(&fixture.domain, &right_propose, byzantine, key)
                    .map_err(invalid)?,
                Some(&left_propose),
                Some(&right_propose),
            ),
            "accept" => evidence_pair(
                kind,
                byzantine,
                &sign_rbc_accept(&fixture.domain, &left_propose, byzantine, key)
                    .map_err(invalid)?,
                &sign_rbc_accept(&fixture.domain, &right_propose, byzantine, key)
                    .map_err(invalid)?,
                Some(&left_propose),
                Some(&right_propose),
            ),
            _ => Err(invalid("unknown RBC evidence stage")),
        }
    };
    let add_abba = |kind: &str, stage: &str| -> io::Result<EvidencePair> {
        match stage {
            "init" => evidence_pair::<_, Value>(
                kind,
                byzantine,
                &sign_abba_init(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    left_agreement.clone(),
                    round,
                    true,
                    key,
                )
                .map_err(invalid)?,
                &sign_abba_init(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    right_agreement.clone(),
                    round,
                    false,
                    key,
                )
                .map_err(invalid)?,
                None,
                None,
            ),
            "aux" => evidence_pair::<_, Value>(
                kind,
                byzantine,
                &sign_abba_aux(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    left_agreement.clone(),
                    round,
                    true,
                    key,
                )
                .map_err(invalid)?,
                &sign_abba_aux(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    right_agreement.clone(),
                    round,
                    false,
                    key,
                )
                .map_err(invalid)?,
                None,
                None,
            ),
            "conf" => evidence_pair::<_, Value>(
                kind,
                byzantine,
                &sign_abba_conf(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    left_agreement.clone(),
                    round,
                    true,
                    key,
                )
                .map_err(invalid)?,
                &sign_abba_conf(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    right_agreement.clone(),
                    round,
                    false,
                    key,
                )
                .map_err(invalid)?,
                None,
                None,
            ),
            "finish" => evidence_pair::<_, Value>(
                kind,
                byzantine,
                &sign_abba_finish(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    left_agreement.clone(),
                    round,
                    true,
                    key,
                )
                .map_err(invalid)?,
                &sign_abba_finish(
                    &fixture.domain,
                    left_graph_root.clone(),
                    byzantine,
                    right_agreement.clone(),
                    round,
                    false,
                    key,
                )
                .map_err(invalid)?,
                None,
                None,
            ),
            _ => Err(invalid("unknown ABBA evidence stage")),
        }
    };
    let add_dabc = |kind: &str| -> io::Result<EvidencePair> {
        let left = sign_dabc_full_knowledge_check(
            &fixture.domain,
            left_graph_root.clone(),
            byzantine,
            round.saturating_add(1),
            vec![DabcPendingPair {
                amendment_slot: round,
                output_candidate_id: hash_hex(
                    "postfiat.cobalt.e2.dabc.left.v1",
                    case_id.as_bytes(),
                ),
            }],
            key,
        )
        .map_err(invalid)?;
        let right = sign_dabc_full_knowledge_check(
            &fixture.domain,
            left_graph_root.clone(),
            byzantine,
            round.saturating_add(1),
            vec![DabcPendingPair {
                amendment_slot: round,
                output_candidate_id: hash_hex(
                    "postfiat.cobalt.e2.dabc.right.v1",
                    case_id.as_bytes(),
                ),
            }],
            key,
        )
        .map_err(invalid)?;
        evidence_pair::<_, Value>(kind, byzantine, &left, &right, None, None)
    };

    let mut pairs = match strategy {
        "rbc_propose_equivocation"
        | "competing_proposals"
        | "reproposal"
        | "trust_view_change"
        | "incompatible_trust_boundary" => vec![add_rbc_propose(strategy)?],
        "rbc_echo_equivocation" | "selective_withholding" => {
            vec![add_rbc_linked(strategy, "echo")?]
        }
        "rbc_ready_equivocation" | "trust_view_lie" => {
            vec![add_rbc_linked(strategy, "ready")?]
        }
        "rbc_accept_equivocation" | "mvba_candidate_equivocation" => {
            vec![add_rbc_linked(strategy, "accept")?]
        }
        "abba_init_equivocation" => vec![add_abba(strategy, "init")?],
        "abba_aux_equivocation" => vec![add_abba(strategy, "aux")?],
        "abba_conf_equivocation" => vec![add_abba(strategy, "conf")?],
        "abba_finish_equivocation" | "late_vote" => vec![add_abba(strategy, "finish")?],
        "dabc_full_knowledge_equivocation" => vec![add_dabc(strategy)?],
        "combined_all_stages" => vec![
            add_rbc_linked("combined_rbc_ready", "ready")?,
            add_abba("combined_abba_finish", "finish")?,
            add_dabc("combined_dabc_full_knowledge")?,
        ],
        _ => return Err(invalid(format!("unknown E2 strategy {strategy}"))),
    };
    for pair in &mut pairs {
        verify_evidence_pair(&fixture.domain, &fixture.committee, pair)?;
        pair.signature_verified = true;
    }
    Ok(pairs)
}

fn context_propose(value: &Option<Value>) -> io::Result<RbcPropose> {
    serde_json::from_value(
        value
            .clone()
            .ok_or_else(|| invalid("RBC evidence lacks proposal context"))?,
    )
    .map_err(io::Error::other)
}

fn verify_evidence_pair(
    domain: &CobaltDomain,
    committee: &CobaltSignatureCommittee,
    pair: &EvidencePair,
) -> io::Result<()> {
    if pair.left_message_id == pair.right_message_id {
        return Err(invalid("signed evidence messages are not conflicting"));
    }
    let signer = pair.signer.as_str();
    if pair.kind.contains("rbc_propose")
        || matches!(
            pair.kind.as_str(),
            "competing_proposals"
                | "reproposal"
                | "trust_view_change"
                | "incompatible_trust_boundary"
        )
    {
        let left: RbcPropose =
            serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: RbcPropose =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_rbc_propose_signed(domain, committee, &left).map_err(invalid)?;
        validate_rbc_propose_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer
            || right.sender != signer
            || left.amendment_slot != right.amendment_slot
            || (left.payload_hash == right.payload_hash
                && left.trust_graph_root == right.trust_graph_root)
        {
            return Err(invalid(
                "RBC proposal evidence does not prove same-slot conflict",
            ));
        }
    } else if pair.kind.contains("rbc_echo") || pair.kind == "selective_withholding" {
        let left: RbcEcho = serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: RbcEcho =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        let left_propose = context_propose(&pair.left_context)?;
        let right_propose = context_propose(&pair.right_context)?;
        validate_rbc_echo_signed(domain, committee, &left, &left_propose).map_err(invalid)?;
        validate_rbc_echo_signed(domain, committee, &right, &right_propose).map_err(invalid)?;
        if left.sender != signer
            || right.sender != signer
            || left.payload_hash == right.payload_hash
        {
            return Err(invalid("RBC echo evidence does not prove conflict"));
        }
    } else if pair.kind.contains("rbc_ready") || pair.kind == "trust_view_lie" {
        let left: RbcReady = serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: RbcReady =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        let left_propose = context_propose(&pair.left_context)?;
        let right_propose = context_propose(&pair.right_context)?;
        validate_rbc_ready_signed(domain, committee, &left, &left_propose).map_err(invalid)?;
        validate_rbc_ready_signed(domain, committee, &right, &right_propose).map_err(invalid)?;
        if left.sender != signer
            || right.sender != signer
            || left.payload_hash == right.payload_hash
        {
            return Err(invalid("RBC ready evidence does not prove conflict"));
        }
    } else if pair.kind.contains("rbc_accept") || pair.kind == "mvba_candidate_equivocation" {
        let left: RbcAccept =
            serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: RbcAccept =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        let left_propose = context_propose(&pair.left_context)?;
        let right_propose = context_propose(&pair.right_context)?;
        validate_rbc_accept_signed(domain, committee, &left, &left_propose).map_err(invalid)?;
        validate_rbc_accept_signed(domain, committee, &right, &right_propose).map_err(invalid)?;
        if left.sender != signer
            || right.sender != signer
            || left.payload_hash == right.payload_hash
        {
            return Err(invalid("RBC accept evidence does not prove conflict"));
        }
    } else if pair.kind.contains("abba_init") {
        let left: AbbaInit = serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: AbbaInit =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_abba_init_signed(domain, committee, &left).map_err(invalid)?;
        validate_abba_init_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer || right.sender != signer || left.value == right.value {
            return Err(invalid("ABBA init evidence does not prove conflict"));
        }
    } else if pair.kind.contains("abba_aux") {
        let left: AbbaAux = serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: AbbaAux =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_abba_aux_signed(domain, committee, &left).map_err(invalid)?;
        validate_abba_aux_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer || right.sender != signer || left.value == right.value {
            return Err(invalid("ABBA aux evidence does not prove conflict"));
        }
    } else if pair.kind.contains("abba_conf") {
        let left: AbbaConf = serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: AbbaConf =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_abba_conf_signed(domain, committee, &left).map_err(invalid)?;
        validate_abba_conf_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer || right.sender != signer || left.value == right.value {
            return Err(invalid("ABBA conf evidence does not prove conflict"));
        }
    } else if pair.kind.contains("abba_finish") || pair.kind == "late_vote" {
        let left: AbbaFinish =
            serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: AbbaFinish =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_abba_finish_signed(domain, committee, &left).map_err(invalid)?;
        validate_abba_finish_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer || right.sender != signer || left.value == right.value {
            return Err(invalid("ABBA finish evidence does not prove conflict"));
        }
    } else if pair.kind.contains("dabc_full_knowledge") {
        let left: DabcFullKnowledgeCheck =
            serde_json::from_value(pair.left.clone()).map_err(io::Error::other)?;
        let right: DabcFullKnowledgeCheck =
            serde_json::from_value(pair.right.clone()).map_err(io::Error::other)?;
        validate_dabc_full_knowledge_check_signed(domain, committee, &left).map_err(invalid)?;
        validate_dabc_full_knowledge_check_signed(domain, committee, &right).map_err(invalid)?;
        if left.sender != signer
            || right.sender != signer
            || left.checkpoint_height != right.checkpoint_height
            || left.pending_pairs == right.pending_pairs
        {
            return Err(invalid("DABC evidence does not prove same-height conflict"));
        }
    } else {
        return Err(invalid(format!(
            "unsupported signed evidence kind {}",
            pair.kind
        )));
    }
    Ok(())
}

fn case(
    manifest: &CampaignManifest,
    fixture: &Fixture,
    byzantine: &str,
    strategy: &str,
    case_number: usize,
) -> io::Result<CampaignCase> {
    let case_id = format!("e2-{strategy}-{byzantine}");
    let compatible = strategy != "incompatible_trust_boundary";
    let (worst_schedule, search_coverage) =
        search_schedules(manifest, &case_id, byzantine, compatible);
    let round = 20_000_u64.saturating_add(case_number as u64);
    let honest_registry_root = hash_hex(
        "postfiat.cobalt.e2.honest-registry-root.v1",
        case_id.as_bytes(),
    );
    let signed_evidence = signed_evidence(fixture, &case_id, byzantine, strategy, round)?;
    let conflicting_transcript_rejected =
        single_byzantine_transcript_rejected(fixture, byzantine, round, &case_id)?;
    let (production_transcript_accepted, duplicate_contributor_rejected, accepted_roots) =
        if compatible {
            let (transcript, duplicate_rejected) = honest_transcript(
                fixture,
                byzantine,
                round,
                &honest_registry_root,
                &worst_schedule.honest_delivery_order,
            )?;
            (
                true,
                duplicate_rejected,
                vec![transcript.ratification.candidate.payload_hash],
            )
        } else {
            let analysis = analyze_trust_graph(
                &fixture.domain,
                &fixture.incompatible_graph,
                &CobaltFaultModel {
                    actively_byzantine: vec![byzantine.to_string()],
                },
            )
            .map_err(invalid)?;
            if analysis.unsafe_pairs.is_empty() {
                return Err(invalid(
                    "incompatible case lost its unsafe linkage evidence",
                ));
            }
            (false, true, Vec::new())
        };

    let per_validator = validators()
        .into_iter()
        .map(|validator| {
            if validator == byzantine {
                ValidatorOutcome {
                    validator,
                    correct: false,
                    decided: false,
                    decision_step: None,
                    accepted_registry_root: None,
                    rejection_reason: Some("excluded_by_signed_misbehavior_evidence".to_string()),
                }
            } else if compatible {
                let decision_step = worst_schedule
                    .honest_decision_steps
                    .get(&validator)
                    .copied()
                    .flatten();
                ValidatorOutcome {
                    validator,
                    correct: true,
                    decided: decision_step.is_some(),
                    decision_step,
                    accepted_registry_root: decision_step.map(|_| honest_registry_root.clone()),
                    rejection_reason: decision_step
                        .is_none()
                        .then(|| "schedule_failed_to_reach_quorum".to_string()),
                }
            } else {
                ValidatorOutcome {
                    validator,
                    correct: true,
                    decided: false,
                    decision_step: None,
                    accepted_registry_root: None,
                    rejection_reason: Some("unsafe_trust_graph_safe_halt".to_string()),
                }
            }
        })
        .collect::<Vec<_>>();
    let conflicting_root_count = accepted_roots.len().saturating_sub(1);
    let false_accept =
        (!compatible && !accepted_roots.is_empty()) || search_coverage.false_accept_schedules != 0;
    let false_halt =
        (compatible && accepted_roots.is_empty()) || search_coverage.false_halt_schedules != 0;
    let synchrony_violation = search_coverage.synchrony_violation_schedules != 0
        || (compatible
            && worst_schedule
                .decision_step
                .is_none_or(|step| step > manifest.schedule_search.synchrony_bound_steps));
    let registry_mutated_on_rejection = !compatible && !accepted_roots.is_empty();
    let evidence_ok = !signed_evidence.is_empty()
        && signed_evidence.iter().all(|pair| {
            pair.signature_verified
                && pair.signer == byzantine
                && pair.left_message_id != pair.right_message_id
        });
    let coverage_ok = search_coverage.executed_event_schedules == search_coverage.candidates
        && search_coverage.conflicting_root_schedules == 0
        && search_coverage.false_accept_schedules == 0
        && search_coverage.false_halt_schedules == 0
        && search_coverage.synchrony_violation_schedules == 0
        && search_coverage.delay_varied
        && search_coverage.drop_varied
        && search_coverage.reorder_varied
        && search_coverage.duplicate_varied
        && search_coverage.partition_varied;
    let ok = conflicting_root_count == 0
        && !false_accept
        && !false_halt
        && !synchrony_violation
        && !registry_mutated_on_rejection
        && evidence_ok
        && coverage_ok
        && (production_transcript_accepted == compatible)
        && duplicate_contributor_rejected
        && conflicting_transcript_rejected;

    Ok(CampaignCase {
        case_id,
        byzantine_validator: byzantine.to_string(),
        strategy: strategy.to_string(),
        compatible,
        expectation: if compatible {
            "five correct validators decide one honest registry root within the synchrony bound"
                .to_string()
        } else {
            "incompatible correct trust views halt without registry mutation".to_string()
        },
        search_coverage,
        worst_schedule,
        signed_evidence,
        per_validator,
        accepted_registry_roots: accepted_roots,
        production_transcript_accepted,
        duplicate_contributor_rejected,
        conflicting_transcript_rejected,
        registry_mutated_on_rejection,
        conflicting_root_count,
        false_accept,
        false_halt,
        synchrony_violation,
        ok,
    })
}

fn classification_sha256(cases: &[CampaignCase]) -> io::Result<String> {
    let fingerprints = cases
        .iter()
        .map(|case| ClassificationFingerprint {
            case_id: &case.case_id,
            byzantine_validator: &case.byzantine_validator,
            strategy: &case.strategy,
            compatible: case.compatible,
            accepted_registry_roots: &case.accepted_registry_roots,
            decision_step: case.worst_schedule.decision_step,
            evidence_ids: case
                .signed_evidence
                .iter()
                .map(|pair| {
                    (
                        pair.kind.as_str(),
                        pair.left_message_id.as_str(),
                        pair.right_message_id.as_str(),
                    )
                })
                .collect(),
            conflicting_root_count: case.conflicting_root_count,
            false_accept: case.false_accept,
            false_halt: case.false_halt,
            synchrony_violation: case.synchrony_violation,
            conflicting_transcript_rejected: case.conflicting_transcript_rejected,
            schedule_failures: case.search_coverage.conflicting_root_schedules
                + case.search_coverage.false_accept_schedules
                + case.search_coverage.false_halt_schedules
                + case.search_coverage.synchrony_violation_schedules,
            ok: case.ok,
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&fingerprints)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(io::Error::other)
}

fn build_report(
    repository: &Path,
    manifest_path: &Path,
    source_revision: &str,
    summary_only: bool,
) -> io::Result<CampaignReport> {
    validate_source_revision(source_revision)?;
    let manifest_bytes = fs::read(manifest_path)?;
    let manifest: CampaignManifest =
        serde_json::from_slice(&manifest_bytes).map_err(io::Error::other)?;
    let manifest_sha256 = sha256_bytes(&manifest_bytes);
    let source_audit = audit_sources(repository, &manifest)?;
    let fixture = fixture(&manifest)?;
    let mut cases = Vec::with_capacity(manifest.expected_case_count);
    for byzantine in &manifest.live_binding.validators {
        for strategy in &manifest.strategies {
            let case_number = cases.len();
            cases.push(case(&manifest, &fixture, byzantine, strategy, case_number)?);
        }
    }
    if cases.len() != manifest.expected_case_count {
        return Err(invalid("E2 campaign did not execute every frozen case"));
    }
    let classification_sha256 = classification_sha256(&cases)?;
    let summary = CampaignSummary {
        schema: REPORT_SCHEMA.to_string(),
        manifest_sha256,
        source_revision: source_revision.to_string(),
        case_count: cases.len(),
        compatible_case_count: cases.iter().filter(|case| case.compatible).count(),
        incompatible_case_count: cases.iter().filter(|case| !case.compatible).count(),
        schedule_candidates: cases
            .iter()
            .map(|case| case.search_coverage.candidates)
            .sum(),
        delivered_event_count: cases
            .iter()
            .map(|case| case.search_coverage.delivered_event_count)
            .sum(),
        duplicate_event_count: cases
            .iter()
            .map(|case| case.search_coverage.duplicate_event_count)
            .sum(),
        pre_heal_blocked_event_count: cases
            .iter()
            .map(|case| case.search_coverage.pre_heal_blocked_event_count)
            .sum(),
        conflicting_root_schedule_count: cases
            .iter()
            .map(|case| case.search_coverage.conflicting_root_schedules)
            .sum(),
        false_accept_schedule_count: cases
            .iter()
            .map(|case| case.search_coverage.false_accept_schedules)
            .sum(),
        false_halt_schedule_count: cases
            .iter()
            .map(|case| case.search_coverage.false_halt_schedules)
            .sum(),
        synchrony_violation_schedule_count: cases
            .iter()
            .map(|case| case.search_coverage.synchrony_violation_schedules)
            .sum(),
        signed_evidence_pairs: cases.iter().map(|case| case.signed_evidence.len()).sum(),
        signed_evidence_verified: cases
            .iter()
            .flat_map(|case| &case.signed_evidence)
            .all(|evidence| evidence.signature_verified),
        conflicting_root_count: cases.iter().map(|case| case.conflicting_root_count).sum(),
        false_accept_count: cases.iter().filter(|case| case.false_accept).count(),
        false_halt_count: cases.iter().filter(|case| case.false_halt).count(),
        synchrony_violation_count: cases.iter().filter(|case| case.synchrony_violation).count(),
        rejected_state_mutation_count: cases
            .iter()
            .filter(|case| case.registry_mutated_on_rejection)
            .count(),
        classification_sha256,
        summary_only,
        pass: cases.iter().all(|case| case.ok),
    };
    Ok(CampaignReport {
        summary,
        source_audit,
        cases: if summary_only { Vec::new() } else { cases },
    })
}

fn verify_report(repository: &Path, manifest_path: &Path, report_path: &Path) -> io::Result<()> {
    let manifest_bytes = fs::read(manifest_path)?;
    let manifest: CampaignManifest =
        serde_json::from_slice(&manifest_bytes).map_err(io::Error::other)?;
    let report: CampaignReport = read_json(report_path)?;
    if report.summary.summary_only || report.cases.is_empty() {
        return Err(invalid(
            "signed-evidence verification requires a full E2 report",
        ));
    }
    if report.summary.manifest_sha256 != sha256_bytes(&manifest_bytes) {
        return Err(invalid("E2 report manifest SHA-256 mismatch"));
    }
    let audit = audit_sources(repository, &manifest)?;
    if report.source_audit.topology_source_sha256 != audit.topology_source_sha256
        || report.source_audit.activation_source_sha256 != audit.activation_source_sha256
        || report.source_audit.derived_f != FAULT_BOUND
    {
        return Err(invalid("E2 report source audit mismatch"));
    }
    let fixture = fixture(&manifest)?;
    for case in &report.cases {
        for evidence in &case.signed_evidence {
            verify_evidence_pair(&fixture.domain, &fixture.committee, evidence)?;
            if !evidence.signature_verified {
                return Err(invalid("E2 evidence is not marked signature-verified"));
            }
            if evidence.signer != case.byzantine_validator {
                return Err(invalid(
                    "E2 evidence signer does not match case attribution",
                ));
            }
        }
    }
    if classification_sha256(&report.cases)? != report.summary.classification_sha256 {
        return Err(invalid("E2 report classification hash mismatch"));
    }
    let recomputed = build_report(
        repository,
        manifest_path,
        &report.summary.source_revision,
        false,
    )?;
    if recomputed.summary != report.summary || recomputed.source_audit != report.source_audit {
        return Err(invalid(
            "E2 report summary does not match a clean deterministic replay",
        ));
    }
    let failures = report.cases.iter().filter(|case| !case.ok).count();
    if failures != 0 || !report.summary.pass {
        return Err(invalid(format!(
            "E2 report contains {failures} failed cases"
        )));
    }
    Ok(())
}

fn usage() -> &'static str {
    "usage:\n  postfiat-cobalt-e2-harness audit-sources <repository> <manifest>\n  postfiat-cobalt-e2-harness run <repository> <manifest> <source-revision> <output> [--summary-only]\n  postfiat-cobalt-e2-harness verify-evidence <repository> <manifest> <report>"
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [command, repository, manifest] if command == "audit-sources" => {
            let manifest: CampaignManifest = read_json(Path::new(manifest))?;
            let audit = audit_sources(Path::new(repository), &manifest)?;
            println!(
                "e2-source-audit-ok validators={} quorum={} f={}",
                audit.subset_validator_count, audit.subset_quorum, audit.derived_f
            );
            Ok(())
        }
        [command, repository, manifest, source_revision, output] if command == "run" => {
            let report = build_report(
                Path::new(repository),
                Path::new(manifest),
                source_revision,
                false,
            )?;
            write_new_json(Path::new(output), &report)?;
            println!(
                "e2-campaign-ok cases={} schedules={} evidence_pairs={} classification_sha256={}",
                report.summary.case_count,
                report.summary.schedule_candidates,
                report.summary.signed_evidence_pairs,
                report.summary.classification_sha256
            );
            if report.summary.pass {
                Ok(())
            } else {
                Err("E2 campaign failed".into())
            }
        }
        [command, repository, manifest, source_revision, output, flag]
            if command == "run" && flag == "--summary-only" =>
        {
            let report = build_report(
                Path::new(repository),
                Path::new(manifest),
                source_revision,
                true,
            )?;
            write_new_json(Path::new(output), &report)?;
            println!(
                "e2-clean-rerun-ok cases={} classification_sha256={}",
                report.summary.case_count, report.summary.classification_sha256
            );
            if report.summary.pass {
                Ok(())
            } else {
                Err("E2 clean rerun failed".into())
            }
        }
        [command, repository, manifest, report] if command == "verify-evidence" => {
            verify_report(
                Path::new(repository),
                Path::new(manifest),
                Path::new(report),
            )?;
            println!("e2-signed-evidence-ok");
            Ok(())
        }
        _ => Err(usage().into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manifest() -> CampaignManifest {
        CampaignManifest {
            schema: MANIFEST_SCHEMA.to_string(),
            campaign_id: "test".to_string(),
            frozen_at: "2026-08-25T00:00:00Z".to_string(),
            live_binding: LiveBinding {
                chain_id: "postfiat-wan-devnet-2".to_string(),
                genesis_hash: "11".repeat(48),
                registry_root: "22".repeat(48),
                trust_graph_root: "33".repeat(48),
                topology_trust_graph_root: "44".repeat(48),
                topology_source: EvidenceSource {
                    path: "topology.json".to_string(),
                    sha256: "55".repeat(32),
                },
                activation_source: EvidenceSource {
                    path: "activation.json".to_string(),
                    sha256: "66".repeat(32),
                },
                validators: validators(),
            },
            fault_model: FaultModel {
                validator_count: 6,
                quorum: 5,
                local_max_active_byzantine: 1,
                derived_f: 1,
                first_inequality: "1 < 2*5 - 6".to_string(),
                second_inequality: "2*1 < 5".to_string(),
            },
            schedule_search: ScheduleSearch {
                seed_hex: "77".repeat(32),
                schedules_per_case: 128,
                max_delay_steps: 7,
                max_partition_steps: 7,
                max_duplicate_copies: 3,
                synchrony_bound_steps: 40,
            },
            strategies: E2_STRATEGIES
                .iter()
                .map(|strategy| (*strategy).to_string())
                .collect(),
            expected_case_count: E2_STRATEGIES.len() * VALIDATOR_COUNT,
        }
    }

    #[test]
    fn live_fault_bound_and_search_envelope_are_valid() {
        validate_manifest_shape(&test_manifest()).expect("valid 6/5/1 manifest");
    }

    #[test]
    fn schedule_search_is_deterministic_and_covers_every_dimension() {
        let manifest = test_manifest();
        let (left, coverage) = search_schedules(&manifest, "case", "validator-0", true);
        let (right, _) = search_schedules(&manifest, "case", "validator-0", true);
        assert_eq!(left.candidate_index, right.candidate_index);
        assert_eq!(left.decision_step, right.decision_step);
        assert!(coverage.delay_varied);
        assert!(coverage.drop_varied);
        assert!(coverage.reorder_varied);
        assert!(coverage.duplicate_varied);
        assert!(coverage.partition_varied);
        assert_eq!(coverage.executed_event_schedules, coverage.candidates);
        assert_eq!(coverage.conflicting_root_schedules, 0);
        assert_eq!(coverage.false_accept_schedules, 0);
        assert_eq!(coverage.false_halt_schedules, 0);
        assert_eq!(coverage.synchrony_violation_schedules, 0);
        assert!(coverage.delivered_event_count > 0);
        assert!(coverage.duplicate_event_count > 0);
        assert!(coverage.pre_heal_blocked_event_count > 0);
        assert!(
            left.decision_step.expect("compatible decision")
                <= manifest.schedule_search.synchrony_bound_steps
        );
    }

    #[test]
    fn incompatible_graph_has_unsafe_pairs() {
        let manifest = test_manifest();
        let fixture = fixture(&manifest).expect("fixture");
        let analysis = analyze_trust_graph(
            &fixture.domain,
            &fixture.incompatible_graph,
            &CobaltFaultModel::default(),
        )
        .expect("analysis");
        assert!(!analysis.unsafe_pairs.is_empty());
    }

    #[test]
    fn signed_equivocation_is_verified_against_committee_keys() {
        let manifest = test_manifest();
        let fixture = fixture(&manifest).expect("fixture");
        let evidence = signed_evidence(&fixture, "case", "validator-0", "combined_all_stages", 100)
            .expect("evidence");
        assert_eq!(evidence.len(), 3);
        assert!(evidence.iter().all(|pair| pair.signature_verified));
    }
}
