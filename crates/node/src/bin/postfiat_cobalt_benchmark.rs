#![recursion_limit = "256"]

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
    CobaltShadowLimits, CobaltShadowProtocolDecision, CobaltShadowService,
};
use postfiat_node::{
    ValidatorKeyFile, ValidatorKeyRecord, ValidatorRegistry, ValidatorRegistryRecord,
};
use serde::Deserialize;
use serde_json::{json, Value};

const MANIFEST_SCHEMA: &str = "postfiat-cobalt-rippled-scenario-manifest-v1";
const REPORT_SCHEMA: &str = "postfiat-cobalt-matched-benchmark-report-v1";

#[derive(Debug, Deserialize)]
struct ScenarioManifest {
    schema: String,
    manifest_sha256: String,
    topologies: Vec<Topology>,
    cases: Vec<ScenarioCase>,
}

#[derive(Debug, Deserialize)]
struct Topology {
    id: String,
    validators: Vec<String>,
    quorum: usize,
    declared_byzantine_budget: usize,
}

#[derive(Debug, Deserialize)]
struct ScenarioCase {
    id: String,
    topology_id: String,
    fault_class: String,
    validators: Vec<String>,
    local_unls: BTreeMap<String, Vec<String>>,
    local_quorums: BTreeMap<String, usize>,
    declared_byzantine_budget: usize,
    correlation_groups: BTreeMap<String, Vec<Vec<String>>>,
    faults: Faults,
    transition: Transition,
    seed: u64,
    repetitions: usize,
    timeout_ms: u64,
    expected: Expected,
}

#[derive(Debug, Deserialize)]
struct Faults {
    offline: Vec<String>,
    actively_byzantine: Vec<String>,
    censored: Vec<String>,
    equivocal: Vec<String>,
    partitions: Vec<Vec<String>>,
    heal_at_ms: Option<u64>,
    latency_ms: Latency,
    packet_loss_every: u64,
    duplicate_every: u64,
    reorder_every: u64,
    reorder_extra_ms: u64,
}

#[derive(Debug, Deserialize)]
struct Latency {
    base: u64,
    jitter: u64,
}

#[derive(Debug, Deserialize)]
struct Transition {
    kind: String,
    removed: Vec<String>,
    added: Vec<String>,
    rotated: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Expected {
    model_scope: String,
    pre_heal: String,
    post_heal: String,
    conflicting_decisions: usize,
}

struct PreparedTopology {
    base_dir: PathBuf,
    identities: Vec<String>,
    validator_keys: BTreeMap<String, ValidatorKeyRecord>,
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

fn write_private_json<T: serde::Serialize>(path: &Path, value: &T) -> io::Result<()> {
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

fn benchmark_limits() -> CobaltShadowLimits {
    CobaltShadowLimits {
        max_message_bytes: 1024 * 1024,
        ..CobaltShadowLimits::default()
    }
}

fn prepare_topology(root: &Path, topology: &Topology) -> io::Result<PreparedTopology> {
    let base_dir = root.join("base").join(&topology.id);
    fs::create_dir_all(&base_dir)?;
    let mut identities = topology.validators.clone();
    identities.push(format!("candidate-{}", topology.validators.len()));
    identities.sort();
    identities.dedup();
    let mut validator_keys = BTreeMap::new();
    for node_id in &identities {
        let service_dir = base_dir.join(node_id);
        CobaltShadowService::initialize(
            &service_dir,
            CobaltShadowIdentity {
                node_id: node_id.clone(),
                chain_id: format!("postfiat-cobalt-benchmark-{}", topology.id),
                genesis_hash: "42".repeat(48),
                protocol_version: 1,
            },
            benchmark_limits(),
        )?;
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
    Ok(PreparedTopology {
        base_dir,
        identities,
        validator_keys,
    })
}

fn operator_labels(case: &ScenarioCase, validator: &str) -> Vec<String> {
    let mut labels = Vec::new();
    for (kind, groups) in &case.correlation_groups {
        for (index, group) in groups.iter().enumerate() {
            if group.iter().any(|member| member == validator) {
                labels.push(format!("{kind}-{index}"));
            }
        }
    }
    labels.sort();
    labels.dedup();
    labels
}

fn safe_subset_budget(count: usize, quorum: usize, declared: usize) -> usize {
    let linkage_bound = quorum
        .saturating_mul(2)
        .saturating_sub(count)
        .saturating_sub(1);
    let abba_bound = quorum.saturating_sub(1) / 2;
    declared.min(linkage_bound).min(abba_bound)
}

fn build_graph(
    case: &ScenarioCase,
    domain: &CobaltDomain,
    registry_root: &str,
) -> io::Result<TrustGraph> {
    let mut views = Vec::new();
    for validator in &case.validators {
        let unl = case
            .local_unls
            .get(validator)
            .ok_or_else(|| invalid(format!("{} has no local UNL", validator)))?;
        let quorum = *case
            .local_quorums
            .get(validator)
            .ok_or_else(|| invalid(format!("{} has no local quorum", validator)))?;
        let subset = build_essential_subset(
            domain,
            unl.clone(),
            safe_subset_budget(unl.len(), quorum, case.declared_byzantine_budget),
            quorum,
            operator_labels(case, validator),
            1,
            None,
        )
        .map_err(invalid)?;
        views.push(build_trust_view(domain, validator, 1, vec![subset], "").map_err(invalid)?);
    }
    build_trust_graph(domain, 1, registry_root, 1, None, views).map_err(invalid)
}

fn partition_support(case: &ScenarioCase) -> Value {
    let rows = case
        .faults
        .partitions
        .iter()
        .enumerate()
        .map(|(index, group)| {
            let visible = group.iter().collect::<BTreeSet<_>>();
            let accepted = group
                .iter()
                .filter(|validator| {
                    let unl = &case.local_unls[*validator];
                    let quorum = case.local_quorums[*validator];
                    unl.iter().filter(|member| visible.contains(member)).count() >= quorum
                })
                .cloned()
                .collect::<Vec<_>>();
            json!({"partition": index, "members": group, "locally_strong_support": accepted})
        })
        .collect::<Vec<_>>();
    json!(rows)
}

fn expected_pass(case: &ScenarioCase, decided: bool, conflicts: usize, replay_equal: bool) -> bool {
    if conflicts != case.expected.conflicting_decisions {
        return false;
    }
    if case.expected.model_scope == "characterize" {
        return !decided || replay_equal;
    }
    let outcome_ok = match case.expected.post_heal.as_str() {
        "one_decision" => decided,
        "safe_halt" => !decided,
        "one_decision_or_safe_halt" => true,
        _ => false,
    };
    outcome_ok && (!decided || replay_equal)
}

fn directory_bytes(path: &Path) -> io::Result<u64> {
    let mut total = 0u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            total = total.saturating_add(directory_bytes(&entry.path())?);
        } else if entry.file_type()?.is_file() {
            total = total.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(total)
}

fn peak_rss_kib() -> Option<u64> {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
        })
}

fn open_descriptors() -> Option<usize> {
    fs::read_dir("/proc/self/fd").ok().map(|rows| rows.count())
}

fn virtual_stage_samples(case: &ScenarioCase) -> Value {
    let samples = (0..case.repetitions)
        .map(|index| {
            let jitter = if case.faults.latency_ms.jitter == 0 {
                0
            } else {
                (case.seed + index as u64 * 7919) % (case.faults.latency_ms.jitter + 1)
            };
            let one_way = case.faults.latency_ms.base + jitter;
            let reorder = if case.faults.reorder_every > 0 {
                case.faults.reorder_extra_ms
            } else {
                0
            };
            json!({
                "repetition": index,
                "rbc_ms": one_way * 3 + reorder,
                "abba_ms": one_way * 4 + reorder,
                "mvba_ms": one_way * 2,
                "dabc_ms": one_way * 2,
                "recovery_ms": case.faults.heal_at_ms.map(|heal| heal + one_way * 3),
            })
        })
        .collect::<Vec<_>>();
    json!(samples)
}

fn run_case(
    root: &Path,
    prepared: &PreparedTopology,
    topology: &Topology,
    case: &ScenarioCase,
) -> io::Result<Value> {
    if case.timeout_ms == 0 || case.repetitions == 0 {
        return Err(invalid("scenario timeout and repetitions must be nonzero"));
    }
    let case_dir = root.join("cases").join(&case.id);
    fs::create_dir_all(&case_dir)?;
    let rotated = case.transition.rotated.iter().collect::<BTreeSet<_>>();
    let mut service_paths = BTreeMap::new();
    for node_id in &case.validators {
        if !prepared.identities.contains(node_id) {
            return Err(invalid(format!("unprepared benchmark identity {node_id}")));
        }
        let target = if rotated.contains(node_id) {
            let target = case_dir.join(format!("rotated-{node_id}"));
            CobaltShadowService::initialize(
                &target,
                CobaltShadowIdentity {
                    node_id: node_id.clone(),
                    chain_id: format!("postfiat-cobalt-benchmark-{}", topology.id),
                    genesis_hash: "42".repeat(48),
                    protocol_version: 1,
                },
                benchmark_limits(),
            )?;
            target
        } else {
            let target = case_dir.join(node_id);
            copy_tree(&prepared.base_dir.join(node_id), &target)?;
            target
        };
        service_paths.insert(node_id.clone(), target);
    }
    let mut services = case
        .validators
        .iter()
        .map(|node_id| CobaltShadowService::open(&service_paths[node_id]))
        .collect::<io::Result<Vec<_>>>()?;

    let registry = ValidatorRegistry {
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
    };
    let root_hash = registry_root(&registry, &case.validators)?;
    let bindings = services
        .iter()
        .zip(&case.validators)
        .map(|(service, node_id)| {
            service.create_validator_binding(
                root_hash.clone(),
                &prepared
                    .base_dir
                    .join(format!("{node_id}.validator-keys.json")),
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut binding =
        build_registry_binding_manifest(root_hash.clone(), registry, bindings, topology.quorum, 1)?;
    let domain = CobaltDomain {
        chain_id: format!("postfiat-cobalt-benchmark-{}", topology.id),
        genesis_hash: "42".repeat(48),
        protocol_version: 1,
    };
    let graph = build_graph(case, &domain, &root_hash)?;
    let linkage = analyze_trust_graph(
        &domain,
        &graph,
        &CobaltFaultModel {
            actively_byzantine: case.faults.actively_byzantine.clone(),
        },
    )
    .map_err(invalid)?;
    binding.trust_graph = graph;
    for service in &mut services {
        service.bind_registry_manifest(&binding)?;
    }

    let round = 1u64;
    let payload = hash_hex(
        "postfiat.cobalt.matched-benchmark.payload.v1",
        case.id.as_bytes(),
    );
    let proposal = services[0].create_protocol_proposal(&binding, round, payload.clone())?;
    let equivocation_rejected = if case.faults.equivocal.contains(&case.validators[0]) {
        let conflicting = hash_hex(
            "postfiat.cobalt.matched-benchmark.conflict.v1",
            case.id.as_bytes(),
        );
        services[0]
            .create_protocol_proposal(&binding, round, conflicting)
            .is_err()
    } else {
        true
    };

    let unavailable = case
        .faults
        .offline
        .iter()
        .chain(&case.faults.censored)
        .collect::<BTreeSet<_>>();
    let contributing_indices = case
        .validators
        .iter()
        .enumerate()
        .filter(|(_, validator)| !unavailable.contains(validator))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let contribution_started = Instant::now();
    let contributions = contributing_indices
        .iter()
        .map(|index| services[*index].create_protocol_contribution(&binding, &proposal))
        .collect::<io::Result<Vec<_>>>()?;
    let contribution_micros = contribution_started.elapsed().as_micros() as u64;
    let assembly_started = Instant::now();
    let assembled = assemble_protocol_transcript(&binding, proposal, contributions);
    let assembly_micros = assembly_started.elapsed().as_micros() as u64;

    let mut decisions = Vec::<CobaltShadowProtocolDecision>::new();
    let mut stage_micros = Vec::new();
    let mut wire_bytes = 0usize;
    let mut signed_messages = 0usize;
    let mut commit_micros = 0u64;
    let mut assembly_error = None;
    if let Ok(transcript) = assembled {
        let encoded =
            serde_json::to_vec(&transcript).map_err(|error| invalid(error.to_string()))?;
        wire_bytes = encoded.len();
        let commit_started = Instant::now();
        for index in &contributing_indices {
            if case
                .faults
                .actively_byzantine
                .contains(&case.validators[*index])
            {
                continue;
            }
            let before = services[*index].status().stage_validation_micros;
            let decision = services[*index].commit_protocol_transcript(&transcript)?;
            let after = services[*index].status().stage_validation_micros;
            stage_micros.push(json!({
                "validator": case.validators[*index],
                "rbc": after.get("rbc").copied().unwrap_or_default().saturating_sub(before.get("rbc").copied().unwrap_or_default()),
                "abba": after.get("abba").copied().unwrap_or_default().saturating_sub(before.get("abba").copied().unwrap_or_default()),
                "mvba": after.get("mvba").copied().unwrap_or_default().saturating_sub(before.get("mvba").copied().unwrap_or_default()),
                "dabc": after.get("dabc").copied().unwrap_or_default().saturating_sub(before.get("dabc").copied().unwrap_or_default()),
            }));
            signed_messages = decision.signed_message_count;
            decisions.push(decision);
        }
        commit_micros = commit_started.elapsed().as_micros() as u64;
    } else if let Err(error) = assembled {
        assembly_error = Some(error.to_string());
    }

    let decision_ids = decisions
        .iter()
        .map(|decision| decision.decision_id.clone())
        .collect::<BTreeSet<_>>();
    let conflicts = decision_ids.len().saturating_sub(1);
    let decided = !decisions.is_empty();
    drop(services);

    let mut replay_rows = Vec::new();
    let mut replay_equal = true;
    if decided {
        for index in &contributing_indices {
            if case
                .faults
                .actively_byzantine
                .contains(&case.validators[*index])
            {
                continue;
            }
            let mut service = CobaltShadowService::open(&service_paths[&case.validators[*index]])?;
            let replay = service.replay_protocol_state()?;
            replay_equal &= replay.as_slice() == [decisions[0].clone()];
            replay_rows.push(json!({
                "validator": case.validators[*index],
                "history_head": service.status().history_head,
                "governance_digest": service.status().governance_digest,
                "ratification_locks": service.status().ratification_lock_count,
                "live_authority": service.status().live_authority,
                "controls_block_consensus": service.status().controls_block_consensus,
                "replay_decisions": replay.len(),
            }));
        }
    }
    let authority_disabled = replay_rows.iter().all(|row| {
        row.get("live_authority") == Some(&Value::Bool(false))
            && row.get("controls_block_consensus") == Some(&Value::Bool(false))
    });
    let expectation_passed = expected_pass(case, decided, conflicts, replay_equal)
        && equivocation_rejected
        && authority_disabled;

    Ok(json!({
        "schema": "postfiat-cobalt-matched-case-v1",
        "case_id": case.id,
        "topology_id": case.topology_id,
        "fault_class": case.fault_class,
        "model_scope": case.expected.model_scope,
        "expected_pre_heal": case.expected.pre_heal,
        "expected_post_heal": case.expected.post_heal,
        "validator_count": case.validators.len(),
        "declared_byzantine_budget": case.declared_byzantine_budget,
        "local_quorums": case.local_quorums,
        "unsafe_pairs": linkage.unsafe_pairs,
        "linked_pair_count": linkage.linked_pairs.len(),
        "fully_linked_pair_count": linkage.fully_linked_pairs.len(),
        "linkage_report_hash": linkage.report_hash,
        "partition_support": partition_support(case),
        "heal_at_ms": case.faults.heal_at_ms,
        "fault_transport": {
            "packet_loss_every": case.faults.packet_loss_every,
            "duplicate_every": case.faults.duplicate_every,
            "reorder_every": case.faults.reorder_every,
            "reorder_extra_ms": case.faults.reorder_extra_ms,
        },
        "transition": {
            "kind": case.transition.kind,
            "removed": case.transition.removed,
            "added": case.transition.added,
            "rotated": case.transition.rotated,
        },
        "signed_execution_repetitions": 1,
        "virtual_network_repetitions": case.repetitions,
        "virtual_network_stage_ms": virtual_stage_samples(case),
        "decided": decided,
        "safe_halt": !decided,
        "conflicting_decisions": conflicts,
        "decision_ids": decision_ids,
        "certificate_signer_count": decisions.first().map(|decision| decision.certificate_signer_count).unwrap_or_default(),
        "signed_message_count": signed_messages,
        "transcript_wire_bytes": wire_bytes,
        "contributor_count": contributing_indices.len(),
        "contribution_micros": contribution_micros,
        "assembly_micros": assembly_micros,
        "commit_micros": commit_micros,
        "stage_validation_micros": stage_micros,
        "assembly_error": assembly_error,
        "equivocation_rejected": equivocation_rejected,
        "replay_equal": replay_equal,
        "replay": replay_rows,
        "authority_disabled": authority_disabled,
        "expectation_passed": expectation_passed,
        "case_disk_bytes": directory_bytes(&case_dir)?,
        "process_peak_rss_kib": peak_rss_kib(),
        "process_open_descriptors": open_descriptors(),
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
    let manifest: ScenarioManifest = read_json(&manifest_path)?;
    if manifest.schema != MANIFEST_SCHEMA {
        return Err(invalid("scenario manifest schema mismatch"));
    }
    let topology_by_id = manifest
        .topologies
        .iter()
        .map(|topology| (topology.id.clone(), topology))
        .collect::<BTreeMap<_, _>>();
    let mut prepared = BTreeMap::new();
    for topology in &manifest.topologies {
        if topology.declared_byzantine_budget != topology.validators.len() - topology.quorum {
            return Err(invalid("topology quorum/budget mismatch"));
        }
        prepared.insert(topology.id.clone(), prepare_topology(&work_dir, topology)?);
    }
    let started = Instant::now();
    let mut results = Vec::new();
    for case in &manifest.cases {
        let topology = topology_by_id
            .get(&case.topology_id)
            .ok_or_else(|| invalid(format!("unknown topology {}", case.topology_id)))?;
        results.push(run_case(
            &work_dir,
            &prepared[&case.topology_id],
            topology,
            case,
        )?);
    }
    let passed = results
        .iter()
        .filter(|row| row.get("expectation_passed") == Some(&Value::Bool(true)))
        .count();
    let conflict_count = results
        .iter()
        .map(|row| {
            row.get("conflicting_decisions")
                .and_then(Value::as_u64)
                .unwrap_or_default()
        })
        .sum::<u64>();
    let report = json!({
        "schema": REPORT_SCHEMA,
        "status": if passed == results.len() && conflict_count == 0 { "passed" } else { "failed" },
        "scenario_manifest_sha256": manifest.manifest_sha256,
        "case_count": results.len(),
        "passed_case_count": passed,
        "conflicting_decision_count": conflict_count,
        "wall_micros": started.elapsed().as_micros() as u64,
        "process_peak_rss_kib": peak_rss_kib(),
        "process_open_descriptors": open_descriptors(),
        "work_dir_bytes": directory_bytes(&work_dir)?,
        "results": results,
    });
    write_json(&output_path, &report)?;
    println!(
        "COBALT_MATCHED_BENCHMARK cases={} passed={} conflicts={} status={}",
        manifest.cases.len(),
        passed,
        conflict_count,
        report["status"].as_str().unwrap_or("failed")
    );
    if report["status"] != "passed" {
        return Err(invalid(
            "Cobalt matched benchmark did not satisfy its scenario contract",
        ));
    }
    Ok(())
}
