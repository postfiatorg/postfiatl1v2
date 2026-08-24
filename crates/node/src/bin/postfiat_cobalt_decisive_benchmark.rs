#![recursion_limit = "256"]

//! Production Cobalt adapter for the activate-or-retire decision corpus.
//!
//! The expected decisions come from the independent oracle manifest. This
//! adapter does not call the oracle. It constructs the production trust graph
//! from the manifest's explicit essential subsets, applies the production
//! linkage gate, and then drives the signed RBC/ABBA/MVBA/DABC shadow path.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_view, CobaltDomain,
    CobaltFaultModel, TrustGraph,
};
use postfiat_crypto_provider::{bytes_to_hex, hash_hex, ml_dsa_65_keygen, ML_DSA_65_ALGORITHM};
use postfiat_node::cobalt_shadow::{
    assemble_protocol_transcript, build_registry_binding_manifest, CobaltShadowIdentity,
    CobaltShadowLimits, CobaltShadowService,
};
use postfiat_node::{
    ValidatorKeyFile, ValidatorKeyRecord, ValidatorRegistry, ValidatorRegistryRecord,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MANIFEST_SCHEMA: &str = "postfiat-cobalt-decisive-manifest-v1";
const REPORT_SCHEMA: &str = "postfiat-cobalt-decisive-benchmark-report-v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DecisiveManifest {
    schema: String,
    oracle: OracleIdentity,
    input_sha256: String,
    source_pins: BTreeMap<String, String>,
    adapter_sha256: BTreeMap<String, String>,
    cases: Vec<ScenarioCase>,
    manifest_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OracleIdentity {
    rules_version: String,
    implementation_boundary: String,
    source_sha256: String,
    contract_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScenarioCase {
    id: String,
    fault_class: String,
    validators: Vec<String>,
    correct_nodes: Vec<String>,
    unavailable: Vec<String>,
    actively_byzantine: Vec<String>,
    trust_views: BTreeMap<String, TrustViewInput>,
    local_unls: BTreeMap<String, Vec<String>>,
    local_quorums: BTreeMap<String, usize>,
    proposals: Vec<ProposalInput>,
    event_schedule: EventSchedule,
    transition: TransitionInput,
    expected: ExpectedDecision,
    oracle_trace: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TrustViewInput {
    essential_subsets: Vec<EssentialSubsetInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EssentialSubsetInput {
    validators: Vec<String>,
    quorum: usize,
    max_active_byzantine: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProposalInput {
    registry_root: String,
    supporters: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EventSchedule {
    delayed: bool,
    duplicated: bool,
    reordered: bool,
    stale_replay: bool,
    recover_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TransitionInput {
    kind: String,
    removed: Vec<String>,
    added: Vec<String>,
    rotated: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExpectedDecision {
    classification: String,
    cobalt_nodes: BTreeMap<String, NodeDecision>,
    rippled_nodes: BTreeMap<String, NodeDecision>,
    cobalt_conflicting_roots: usize,
    rippled_conflicting_roots: usize,
    material_safety_delta: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct NodeDecision {
    outcome: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registry_root: Option<String>,
    reason: String,
}

struct PreparedCase {
    base_dir: PathBuf,
    validator_keys: BTreeMap<String, ValidatorKeyRecord>,
}

struct CandidateRun {
    root: String,
    decided: bool,
    decision_ids: BTreeSet<String>,
    certificate_signer_counts: BTreeSet<usize>,
    replay_equal: bool,
    duplicate_rejected: bool,
    authority_disabled: bool,
    error: Option<String>,
    wire_bytes: usize,
    elapsed_micros: u64,
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn required_arg(args: &[String], name: &str) -> io::Result<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| invalid(format!("missing {name}")))
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> io::Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(64 * 1024 * 1024)
        .read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(|error| invalid(error.to_string()))
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| invalid(error.to_string()))?;
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    #[cfg(unix)]
    use std::os::unix::fs::OpenOptionsExt;
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| invalid(error.to_string()))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")
}

fn copy_tree(source: &Path, target: &Path) -> io::Result<()> {
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&source_path, &target_path)?;
        } else if entry.file_type()?.is_file() {
            fs::copy(&source_path, &target_path)?;
            fs::set_permissions(&target_path, fs::metadata(&source_path)?.permissions())?;
        } else {
            return Err(invalid("benchmark base contains unsupported file type"));
        }
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify_manifest(manifest: &DecisiveManifest) -> io::Result<()> {
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(invalid("decisive manifest schema mismatch"));
    }
    if manifest.cases.is_empty() {
        return Err(invalid("decisive manifest has no cases"));
    }
    let mut canonical = manifest.clone();
    let expected = canonical.manifest_sha256.clone();
    canonical.manifest_sha256.clear();
    let bytes = serde_json::to_vec(&canonical).map_err(|error| invalid(error.to_string()))?;
    if sha256_hex(&bytes) != expected {
        return Err(invalid("decisive manifest hash mismatch"));
    }
    if !manifest.adapter_sha256.contains_key("cobalt")
        || !manifest.adapter_sha256.contains_key("rippled")
    {
        return Err(invalid("decisive manifest lacks frozen adapter hashes"));
    }
    if manifest
        .cases
        .iter()
        .any(|case| case.expected.cobalt_nodes.len() != case.correct_nodes.len())
    {
        return Err(invalid(
            "manifest lacks a Cobalt decision for a correct node",
        ));
    }
    Ok(())
}

fn limits() -> CobaltShadowLimits {
    CobaltShadowLimits {
        max_message_bytes: 1024 * 1024,
        ..CobaltShadowLimits::default()
    }
}

fn domain(case: &ScenarioCase) -> CobaltDomain {
    CobaltDomain {
        chain_id: format!("postfiat-cobalt-decisive-{}", case.id),
        genesis_hash: "42".repeat(48),
        protocol_version: 1,
    }
}

fn initialize_service(path: &Path, node_id: &str, domain: &CobaltDomain) -> io::Result<()> {
    CobaltShadowService::initialize(
        path,
        CobaltShadowIdentity {
            node_id: node_id.to_string(),
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
        },
        limits(),
    )
    .map(|_| ())
}

fn prepare_case(root: &Path, case: &ScenarioCase) -> io::Result<PreparedCase> {
    let base_dir = root.join("base").join(&case.id);
    fs::create_dir_all(&base_dir)?;
    let domain = domain(case);
    let mut validator_keys = BTreeMap::new();
    for node_id in &case.validators {
        initialize_service(&base_dir.join(node_id), node_id, &domain)?;
        let pair = ml_dsa_65_keygen().map_err(|error| invalid(error.to_string()))?;
        let record = ValidatorKeyRecord {
            node_id: node_id.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&pair.public_key),
            private_key_hex: bytes_to_hex(&pair.private_key),
        };
        write_private_json(
            &base_dir.join(format!("{node_id}.validator-keys.json")),
            &ValidatorKeyFile {
                validators: vec![record.clone()],
            },
        )?;
        validator_keys.insert(node_id.clone(), record);
    }
    Ok(PreparedCase {
        base_dir,
        validator_keys,
    })
}

fn build_graph(
    case: &ScenarioCase,
    domain: &CobaltDomain,
    registry_root: &str,
) -> io::Result<TrustGraph> {
    let mut views = Vec::new();
    for validator in &case.validators {
        let input = case
            .trust_views
            .get(validator)
            .ok_or_else(|| invalid(format!("{validator} has no trust view")))?;
        let subsets = input
            .essential_subsets
            .iter()
            .map(|subset| {
                build_essential_subset(
                    domain,
                    subset.validators.clone(),
                    subset.max_active_byzantine,
                    subset.quorum,
                    Vec::new(),
                    1,
                    None,
                )
                .map_err(invalid)
            })
            .collect::<io::Result<Vec<_>>>()?;
        views.push(build_trust_view(domain, validator, 1, subsets, "").map_err(invalid)?);
    }
    build_trust_graph(domain, 1, registry_root, 1, None, views).map_err(invalid)
}

fn prepare_candidate_services(
    root: &Path,
    prepared: &PreparedCase,
    case: &ScenarioCase,
    candidate_index: usize,
) -> io::Result<(Vec<CobaltShadowService>, BTreeMap<String, PathBuf>)> {
    let candidate_dir = root
        .join("cases")
        .join(&case.id)
        .join("candidates")
        .join(format!("{candidate_index:02}"));
    fs::create_dir_all(&candidate_dir)?;
    let rotated: BTreeSet<&str> = case.transition.rotated.iter().map(String::as_str).collect();
    let domain = domain(case);
    let mut paths = BTreeMap::new();
    for node_id in &case.validators {
        let target = candidate_dir.join(node_id);
        if rotated.contains(node_id.as_str()) {
            initialize_service(&target, node_id, &domain)?;
        } else {
            copy_tree(&prepared.base_dir.join(node_id), &target)?;
        }
        paths.insert(node_id.clone(), target);
    }
    let services = case
        .validators
        .iter()
        .map(|node_id| CobaltShadowService::open(&paths[node_id]))
        .collect::<io::Result<Vec<_>>>()?;
    Ok((services, paths))
}

fn registry(prepared: &PreparedCase, case: &ScenarioCase) -> ValidatorRegistry {
    ValidatorRegistry {
        validators: case
            .validators
            .iter()
            .map(|node_id| {
                let key = &prepared.validator_keys[node_id];
                ValidatorRegistryRecord {
                    node_id: key.node_id.clone(),
                    algorithm_id: key.algorithm_id.clone(),
                    public_key_hex: key.public_key_hex.clone(),
                }
            })
            .collect(),
    }
}

fn registry_root(registry: &ValidatorRegistry, validators: &[String]) -> io::Result<String> {
    let mut validators = validators.to_vec();
    validators.sort();
    validators.dedup();
    let records = validators
        .iter()
        .map(|node_id| {
            let record = registry
                .validators
                .iter()
                .find(|record| &record.node_id == node_id)
                .ok_or_else(|| invalid(format!("missing validator registry record {node_id}")))?;
            Ok((
                record.node_id.as_str(),
                record.algorithm_id.as_str(),
                record.public_key_hex.as_str(),
            ))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let encoded = serde_json::to_vec(&records).map_err(|error| invalid(error.to_string()))?;
    Ok(hash_hex("postfiat.validator_registry.root.v1", &encoded))
}

fn run_candidate(
    root: &Path,
    prepared: &PreparedCase,
    case: &ScenarioCase,
    candidate_index: usize,
    graph: &TrustGraph,
    proposal_input: &ProposalInput,
) -> io::Result<CandidateRun> {
    let started = Instant::now();
    let (mut services, _) = prepare_candidate_services(root, prepared, case, candidate_index)?;
    let validator_registry = registry(prepared, case);
    let actual_registry_root = registry_root(&validator_registry, &case.validators)?;
    let validator_bindings = services
        .iter()
        .zip(&case.validators)
        .map(|(service, node_id)| {
            service.create_validator_binding(
                actual_registry_root.clone(),
                &prepared
                    .base_dir
                    .join(format!("{node_id}.validator-keys.json")),
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut binding = build_registry_binding_manifest(
        actual_registry_root,
        validator_registry,
        validator_bindings,
        case.validators.len(),
        1,
    )?;
    binding.trust_graph = graph.clone();
    for service in &mut services {
        service.bind_registry_manifest(&binding)?;
    }

    let supporter_indices = proposal_input
        .supporters
        .iter()
        .filter(|supporter| {
            case.event_schedule.recover_unavailable
                || case.unavailable.binary_search(supporter).is_err()
        })
        .map(|supporter| {
            case.validators
                .binary_search(supporter)
                .map_err(|_| invalid(format!("unknown proposal supporter {supporter}")))
        })
        .collect::<io::Result<Vec<_>>>()?;
    if supporter_indices.is_empty() {
        return Ok(CandidateRun {
            root: proposal_input.registry_root.clone(),
            decided: false,
            decision_ids: BTreeSet::new(),
            certificate_signer_counts: BTreeSet::new(),
            replay_equal: true,
            duplicate_rejected: true,
            authority_disabled: true,
            error: Some("candidate has no responsive supporters".to_string()),
            wire_bytes: 0,
            elapsed_micros: started.elapsed().as_micros() as u64,
        });
    }

    let proposer_index = supporter_indices[0];
    let proposal = services[proposer_index].create_protocol_proposal(
        &binding,
        1,
        proposal_input.registry_root.clone(),
    )?;
    let mut contributions = supporter_indices
        .iter()
        .map(|index| services[*index].create_protocol_contribution(&binding, &proposal))
        .collect::<io::Result<Vec<_>>>()?;

    let duplicate_rejected = if case.event_schedule.duplicated {
        let mut duplicated = contributions.clone();
        duplicated.push(contributions[0].clone());
        assemble_protocol_transcript(&binding, proposal.clone(), duplicated).is_err()
    } else {
        true
    };
    if case.event_schedule.reordered {
        contributions.reverse();
    }
    let transcript = match assemble_protocol_transcript(&binding, proposal, contributions) {
        Ok(transcript) => transcript,
        Err(error) => {
            return Ok(CandidateRun {
                root: proposal_input.registry_root.clone(),
                decided: false,
                decision_ids: BTreeSet::new(),
                certificate_signer_counts: BTreeSet::new(),
                replay_equal: true,
                duplicate_rejected,
                authority_disabled: services.iter().all(|service| {
                    !service.status().live_authority && !service.status().controls_block_consensus
                }),
                error: Some(error.to_string()),
                wire_bytes: 0,
                elapsed_micros: started.elapsed().as_micros() as u64,
            });
        }
    };
    let wire_bytes = serde_json::to_vec(&transcript)
        .map_err(|error| invalid(error.to_string()))?
        .len();
    let mut decision_ids = BTreeSet::new();
    let mut certificate_signer_counts = BTreeSet::new();
    let mut replay_equal = true;
    for node in &case.correct_nodes {
        if !case.event_schedule.recover_unavailable && case.unavailable.binary_search(node).is_ok()
        {
            continue;
        }
        let index = case
            .validators
            .binary_search(node)
            .map_err(|_| invalid(format!("unknown correct node {node}")))?;
        let decision = services[index].commit_protocol_transcript(&transcript)?;
        decision_ids.insert(decision.decision_id.clone());
        certificate_signer_counts.insert(decision.certificate_signer_count);
        if case.event_schedule.stale_replay {
            let replay = services[index].commit_protocol_transcript(&transcript)?;
            replay_equal &= replay == decision;
        }
    }
    let authority_disabled = services.iter().all(|service| {
        !service.status().live_authority && !service.status().controls_block_consensus
    });
    Ok(CandidateRun {
        root: proposal_input.registry_root.clone(),
        decided: !decision_ids.is_empty(),
        decision_ids,
        certificate_signer_counts,
        replay_equal,
        duplicate_rejected,
        authority_disabled,
        error: None,
        wire_bytes,
        elapsed_micros: started.elapsed().as_micros() as u64,
    })
}

fn actual_node_decisions(
    case: &ScenarioCase,
    graph_safe: bool,
    candidates: &[CandidateRun],
) -> BTreeMap<String, NodeDecision> {
    let decided_roots = candidates
        .iter()
        .filter(|candidate| candidate.decided)
        .map(|candidate| candidate.root.clone())
        .collect::<BTreeSet<_>>();
    case.correct_nodes
        .iter()
        .map(|node| {
            let decision = if !case.event_schedule.recover_unavailable
                && case.unavailable.binary_search(node).is_ok()
            {
                NodeDecision {
                    outcome: "unavailable".to_string(),
                    registry_root: None,
                    reason: "node remains unavailable at the observation boundary".to_string(),
                }
            } else if !graph_safe {
                NodeDecision {
                    outcome: "halt".to_string(),
                    registry_root: None,
                    reason: "production trust-graph linkage gate rejected the graph".to_string(),
                }
            } else if decided_roots.len() == 1 {
                NodeDecision {
                    outcome: "decide".to_string(),
                    registry_root: decided_roots.iter().next().cloned(),
                    reason: "signed production Cobalt transcript committed".to_string(),
                }
            } else if decided_roots.is_empty() {
                NodeDecision {
                    outcome: "halt".to_string(),
                    registry_root: None,
                    reason: "no proposal reached production Cobalt strong support".to_string(),
                }
            } else {
                NodeDecision {
                    outcome: "conflict".to_string(),
                    registry_root: None,
                    reason: "multiple proposal roots produced signed decisions".to_string(),
                }
            };
            (node.clone(), decision)
        })
        .collect()
}

fn decisions_match(
    actual: &BTreeMap<String, NodeDecision>,
    expected: &BTreeMap<String, NodeDecision>,
) -> bool {
    actual.len() == expected.len()
        && actual.iter().all(|(node, decision)| {
            expected.get(node).is_some_and(|wanted| {
                decision.outcome == wanted.outcome && decision.registry_root == wanted.registry_root
            })
        })
}

fn conflicting_roots(nodes: &BTreeMap<String, NodeDecision>) -> usize {
    nodes
        .values()
        .filter(|decision| decision.outcome == "decide")
        .filter_map(|decision| decision.registry_root.as_deref())
        .collect::<BTreeSet<_>>()
        .len()
        .saturating_sub(1)
}

fn run_case(root: &Path, prepared: &PreparedCase, case: &ScenarioCase) -> io::Result<Value> {
    let validator_registry = registry(prepared, case);
    let actual_registry_root = registry_root(&validator_registry, &case.validators)?;
    let domain = domain(case);
    let graph = build_graph(case, &domain, &actual_registry_root)?;
    let linkage = analyze_trust_graph(
        &domain,
        &graph,
        &CobaltFaultModel {
            actively_byzantine: case.actively_byzantine.clone(),
        },
    )
    .map_err(invalid)?;
    let responsive_correct = case
        .correct_nodes
        .iter()
        .filter(|node| {
            case.event_schedule.recover_unavailable || case.unavailable.binary_search(node).is_err()
        })
        .collect::<BTreeSet<_>>();
    let strongly_connected = linkage
        .strongly_connected_validators
        .iter()
        .collect::<BTreeSet<_>>();
    let graph_safe = linkage.unsafe_pairs.is_empty()
        && responsive_correct
            .iter()
            .all(|node| strongly_connected.contains(node));

    let mut candidate_runs = Vec::new();
    if graph_safe {
        for (index, proposal) in case.proposals.iter().enumerate() {
            candidate_runs.push(run_candidate(
                root, prepared, case, index, &graph, proposal,
            )?);
        }
    }
    let actual = actual_node_decisions(case, graph_safe, &candidate_runs);
    let conflicts = conflicting_roots(&actual);
    let expectation_passed = decisions_match(&actual, &case.expected.cobalt_nodes)
        && conflicts == case.expected.cobalt_conflicting_roots
        && candidate_runs.iter().all(|candidate| {
            candidate.replay_equal && candidate.duplicate_rejected && candidate.authority_disabled
        });
    let candidates = candidate_runs
        .iter()
        .map(|candidate| {
            json!({
                "registry_root": candidate.root,
                "decided": candidate.decided,
                "decision_ids": candidate.decision_ids,
                "certificate_signer_counts": candidate.certificate_signer_counts,
                "replay_equal": candidate.replay_equal,
                "duplicate_rejected": candidate.duplicate_rejected,
                "authority_disabled": candidate.authority_disabled,
                "error": candidate.error,
                "wire_bytes": candidate.wire_bytes,
                "elapsed_micros": candidate.elapsed_micros,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "schema": "postfiat-cobalt-decisive-case-v1",
        "case_id": case.id,
        "fault_class": case.fault_class,
        "classification": case.expected.classification,
        "validator_count": case.validators.len(),
        "correct_node_count": case.correct_nodes.len(),
        "trust_graph_root": graph.trust_graph_root,
        "linkage_report_hash": linkage.report_hash,
        "unsafe_pairs": linkage.unsafe_pairs,
        "strongly_connected_validators": linkage.strongly_connected_validators,
        "graph_safe": graph_safe,
        "actual_nodes": actual,
        "expected_nodes": case.expected.cobalt_nodes,
        "conflicting_roots": conflicts,
        "expected_conflicting_roots": case.expected.cobalt_conflicting_roots,
        "event_schedule": case.event_schedule,
        "transition": case.transition,
        "candidate_runs": candidates,
        "authority_mode": "shadow_only",
        "expectation_passed": expectation_passed,
    }))
}

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    let manifest_path = required_arg(&args, "--manifest")?;
    let work_dir = required_arg(&args, "--work-dir")?;
    let output_path = required_arg(&args, "--output")?;
    if work_dir.exists() && fs::read_dir(&work_dir)?.next().is_some() {
        return Err(invalid("benchmark work directory must be empty"));
    }
    fs::create_dir_all(&work_dir)?;
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let manifest: DecisiveManifest = read_json(&manifest_path)?;
    verify_manifest(&manifest)?;

    let started = Instant::now();
    let mut results = Vec::new();
    for case in &manifest.cases {
        let prepared = prepare_case(&work_dir, case)?;
        results.push(run_case(&work_dir, &prepared, case)?);
    }
    let passed = results
        .iter()
        .filter(|result| result["expectation_passed"] == Value::Bool(true))
        .count();
    let conflicts = results
        .iter()
        .map(|result| result["conflicting_roots"].as_u64().unwrap_or_default())
        .sum::<u64>();
    let report = json!({
        "schema": REPORT_SCHEMA,
        "adapter": "production postfiat-consensus-cobalt plus signed Cobalt shadow protocol",
        "oracle_called": false,
        "scenario_manifest_sha256": manifest.manifest_sha256,
        "case_count": results.len(),
        "passed_case_count": passed,
        "conflicting_root_count": conflicts,
        "wall_micros": started.elapsed().as_micros() as u64,
        "results": results,
        "status": if passed == manifest.cases.len() { "passed" } else { "failed" },
    });
    write_json(&output_path, &report)?;
    println!(
        "COBALT_DECISIVE_BENCHMARK cases={} passed={} conflicts={} status={}",
        manifest.cases.len(),
        passed,
        conflicts,
        report["status"].as_str().unwrap_or("failed")
    );
    if report["status"] != "passed" {
        return Err(invalid(
            "Cobalt decisive benchmark did not satisfy the frozen oracle contract",
        ));
    }
    Ok(())
}
