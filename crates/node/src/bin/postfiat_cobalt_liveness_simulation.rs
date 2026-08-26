#![recursion_limit = "256"]

//! Six-domain Cobalt liveness simulation.
//!
//! This binary exercises the production Cobalt signer, strong-support,
//! transcript, durable-history, socket RPC, and proof-carrying catch-up paths.
//! The domains are isolated simulation processes. They are not a claim of
//! independent human operation or real-world decentralization.

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use postfiat_consensus_cobalt::{
    analyze_trust_graph, build_essential_subset, build_trust_graph, build_trust_graph_transition,
    build_trust_view, certify_validator_registry_update_with_trust_graph_transition,
    verify_validator_registry_update, CobaltDomain, CobaltFaultModel, EssentialSubsetConfig,
    RbcPropose, TrustGraph, ValidatorRegistryUpdateRequest, VALIDATOR_REGISTRY_OP_ADMIT,
    VALIDATOR_REGISTRY_OP_REMOVE, VALIDATOR_REGISTRY_OP_ROTATE_KEY,
};
use postfiat_crypto_provider::{bytes_to_hex, hash_hex, ml_dsa_65_keygen, ML_DSA_65_ALGORITHM};
use postfiat_node::cobalt_authority_certificate::{
    verify_cobalt_validator_update_decision_certificate, MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES,
};
use postfiat_node::cobalt_shadow::{
    assemble_protocol_transcript_extending, build_registry_binding_manifest,
    CobaltShadowHistoryRange, CobaltShadowIdentity, CobaltShadowLimits,
    CobaltShadowProtocolContribution, CobaltShadowProtocolDecision, CobaltShadowProtocolTranscript,
    CobaltShadowRegistryBinding, CobaltShadowService,
};
use postfiat_node::cobalt_shadow_runtime::{
    compressed_commit_request, request, serve_listener, CobaltShadowProbe, CobaltShadowRpcRequest,
    MAX_RPC_FRAME_BYTES,
};
use postfiat_node::{
    ValidatorKeyFile, ValidatorKeyRecord, ValidatorRegistry, ValidatorRegistryRecord,
};
use postfiat_types::{
    CobaltValidatorUpdateDecisionCertificateV1, ValidatorRegistryEntry,
    COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

const REPORT_SCHEMA: &str = "postfiat-cobalt-isolated-validator-liveness-simulation-v1";
const DOMAIN_COUNT: usize = 6;
const QUORUM: usize = 5;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn required_arg(args: &[String], name: &str) -> io::Result<PathBuf> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| PathBuf::from(&pair[1]))
        .ok_or_else(|| invalid(format!("missing required argument {name}")))
}

fn write_json(path: &Path, value: &Value) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
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

fn registry_root(
    registry: &ValidatorRegistry,
    expected_validators: &[String],
) -> io::Result<String> {
    let records = expected_validators
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

fn rpc<T: DeserializeOwned>(
    endpoint: SocketAddr,
    request_value: &CobaltShadowRpcRequest,
) -> io::Result<T> {
    let value = request(endpoint, request_value)?;
    serde_json::from_value(value).map_err(|error| invalid(error.to_string()))
}

struct SimulatedDomain {
    node_id: String,
    data_dir: PathBuf,
    endpoint: SocketAddr,
    restart_count: u64,
    handle: Option<JoinHandle<io::Result<CobaltShadowService>>>,
}

impl SimulatedDomain {
    fn start(node_id: String, data_dir: PathBuf, service: CobaltShadowService) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = listener.local_addr()?;
        let handle = thread::spawn(move || serve_listener(service, listener, true));
        Ok(Self {
            node_id,
            data_dir,
            endpoint,
            restart_count: 0,
            handle: Some(handle),
        })
    }

    fn stop(&mut self) -> io::Result<()> {
        if self.handle.is_none() {
            return Ok(());
        }
        let _: Value = rpc(self.endpoint, &CobaltShadowRpcRequest::Shutdown)?;
        let handle = self
            .handle
            .take()
            .ok_or_else(|| invalid("domain thread disappeared"))?;
        let service = handle
            .join()
            .map_err(|_| invalid("domain service thread panicked"))??;
        drop(service);
        Ok(())
    }

    fn restart(&mut self) -> io::Result<()> {
        self.stop()?;
        let service = CobaltShadowService::open(&self.data_dir)?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        self.endpoint = listener.local_addr()?;
        self.handle = Some(thread::spawn(move || {
            serve_listener(service, listener, true)
        }));
        self.restart_count = self.restart_count.saturating_add(1);
        Ok(())
    }
}

fn simulation_domain() -> CobaltDomain {
    CobaltDomain {
        chain_id: "postfiat-cobalt-isolated-validator-simulation".to_string(),
        genesis_hash: "73".repeat(48),
        protocol_version: 1,
    }
}

fn build_nonuniform_graph_at(
    domain: &CobaltDomain,
    registry_root: &str,
    validators: &[String],
    graph_version: u64,
    activation_height: u64,
    previous_trust_graph_root: Option<String>,
) -> io::Result<TrustGraph> {
    let core = build_essential_subset(
        domain,
        validators.to_vec(),
        1,
        QUORUM,
        vec!["simulated-common-six-domain".to_string()],
        activation_height,
        None,
    )
    .map_err(invalid)?;
    let supplemental_a = build_essential_subset(
        domain,
        validators[..5].to_vec(),
        0,
        4,
        vec!["simulated-supplemental-a".to_string()],
        activation_height,
        None,
    )
    .map_err(invalid)?;
    let supplemental_b = build_essential_subset(
        domain,
        vec![
            validators[0].clone(),
            validators[1].clone(),
            validators[2].clone(),
            validators[3].clone(),
            validators[5].clone(),
        ],
        0,
        4,
        vec!["simulated-supplemental-b".to_string()],
        activation_height,
        None,
    )
    .map_err(invalid)?;
    let views = validators
        .iter()
        .enumerate()
        .map(|(index, validator)| {
            let supplemental = if index % 2 == 0 {
                supplemental_a.clone()
            } else {
                supplemental_b.clone()
            };
            build_trust_view(
                domain,
                validator,
                graph_version,
                vec![core.clone(), supplemental],
                "",
            )
            .map_err(invalid)
        })
        .collect::<io::Result<Vec<_>>>()?;
    build_trust_graph(
        domain,
        graph_version,
        registry_root,
        activation_height,
        previous_trust_graph_root,
        views,
    )
    .map_err(invalid)
}

fn build_nonuniform_graph(
    domain: &CobaltDomain,
    registry_root: &str,
    validators: &[String],
) -> io::Result<TrustGraph> {
    build_nonuniform_graph_at(domain, registry_root, validators, 1, 1, None)
}

fn prepare_domains(
    work_dir: &Path,
) -> io::Result<(
    Vec<SimulatedDomain>,
    CobaltShadowRegistryBinding,
    Vec<Value>,
)> {
    let domain = simulation_domain();
    let validators = (0..DOMAIN_COUNT)
        .map(|index| format!("validator-{index:02}"))
        .collect::<Vec<_>>();
    let mut services = Vec::new();
    let mut validator_key_records = Vec::new();
    let mut validator_key_paths = BTreeMap::new();

    for node_id in &validators {
        let data_dir = work_dir.join("domains").join(node_id);
        let service = CobaltShadowService::initialize(
            &data_dir,
            CobaltShadowIdentity {
                node_id: node_id.clone(),
                chain_id: domain.chain_id.clone(),
                genesis_hash: domain.genesis_hash.clone(),
                protocol_version: domain.protocol_version,
            },
            CobaltShadowLimits {
                max_message_bytes: 1024 * 1024,
                ..CobaltShadowLimits::default()
            },
        )?;
        let pair = ml_dsa_65_keygen().map_err(|error| invalid(error.to_string()))?;
        let key_record = ValidatorKeyRecord {
            node_id: node_id.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&pair.public_key),
            private_key_hex: bytes_to_hex(&pair.private_key),
        };
        let key_path = work_dir
            .join("validator-keys")
            .join(format!("{node_id}.json"));
        fs::create_dir_all(
            key_path
                .parent()
                .ok_or_else(|| invalid("validator key path has no parent"))?,
        )?;
        write_private_json(
            &key_path,
            &ValidatorKeyFile {
                validators: vec![key_record.clone()],
            },
        )?;
        validator_key_paths.insert(node_id.clone(), key_path);
        validator_key_records.push(key_record);
        services.push(service);
    }

    let registry = ValidatorRegistry {
        validators: validator_key_records
            .iter()
            .map(|record| ValidatorRegistryRecord {
                node_id: record.node_id.clone(),
                algorithm_id: record.algorithm_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
            })
            .collect(),
    };
    let registry_root = registry_root(&registry, &validators)?;
    let validator_bindings = services
        .iter()
        .zip(&validators)
        .map(|(service, node_id)| {
            service.create_validator_binding(
                registry_root.clone(),
                validator_key_paths
                    .get(node_id)
                    .ok_or_else(|| invalid("validator key path disappeared"))?,
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let mut binding = build_registry_binding_manifest(
        registry_root.clone(),
        registry,
        validator_bindings,
        QUORUM,
        1,
    )?;
    binding.trust_graph = build_nonuniform_graph(&domain, &registry_root, &validators)?;

    let linkage = analyze_trust_graph(
        &domain,
        &binding.trust_graph,
        &CobaltFaultModel {
            actively_byzantine: Vec::new(),
        },
    )
    .map_err(invalid)?;
    if !linkage.unsafe_pairs.is_empty()
        || linkage.strongly_connected_validators.len() != validators.len()
    {
        return Err(invalid(
            "simulated non-uniform trust graph is not fully linked",
        ));
    }

    for service in &mut services {
        service.bind_registry_manifest(&binding)?;
    }

    let domain_receipts = services
        .iter()
        .zip(&validators)
        .zip(&validator_key_records)
        .map(|((service, node_id), validator_key)| {
            json!({
                "node_id": node_id,
                "cobalt_identity_fingerprint": hash_hex(
                    "postfiat.cobalt.simulation.cobalt-key-fingerprint.v1",
                    service.status().signer_public_key_id.as_bytes(),
                ),
                "validator_identity_fingerprint": hash_hex(
                    "postfiat.cobalt.simulation.validator-key-fingerprint.v1",
                    validator_key.public_key_hex.as_bytes(),
                ),
                "data_dir": format!("domains/{node_id}"),
                "trust_view_id": binding
                    .trust_graph
                    .trust_views
                    .iter()
                    .find(|view| &view.validator == node_id)
                    .map(|view| view.trust_view_id.clone())
                    .unwrap_or_default(),
                "fault_control_channel": format!("deterministic-fault-control/{node_id}"),
                "message_schedule_id": format!("omission-recovery-schedule/{node_id}"),
                "human_operator_required": false,
                "scope": "isolated-simulation-domain",
            })
        })
        .collect::<Vec<_>>();

    let domains = validators
        .into_iter()
        .zip(services)
        .map(|(node_id, service)| {
            let data_dir = work_dir.join("domains").join(&node_id);
            SimulatedDomain::start(node_id, data_dir, service)
        })
        .collect::<io::Result<Vec<_>>>()?;

    Ok((domains, binding, domain_receipts))
}

fn registry_entry(record: &ValidatorRegistryRecord) -> ValidatorRegistryEntry {
    ValidatorRegistryEntry {
        node_id: record.node_id.clone(),
        algorithm_id: record.algorithm_id.clone(),
        public_key_hex: record.public_key_hex.clone(),
        active: true,
    }
}

struct TransitionCaseInput {
    operation: &'static str,
    subject_node_id: String,
    activation_height: u64,
    new_registry: ValidatorRegistry,
    previous_validators: Vec<String>,
    new_validators: Vec<String>,
    previous_record: Option<ValidatorRegistryEntry>,
    new_record: Option<ValidatorRegistryEntry>,
}

fn certify_transition_case(
    binding: &CobaltShadowRegistryBinding,
    input: TransitionCaseInput,
) -> io::Result<Value> {
    let TransitionCaseInput {
        operation,
        subject_node_id,
        activation_height,
        new_registry,
        previous_validators,
        new_validators,
        previous_record,
        new_record,
    } = input;
    let domain = simulation_domain();
    let new_registry_root = registry_root(&new_registry, &new_validators)?;
    let new_graph_root = hash_hex(
        "postfiat.cobalt.simulation.transition-graph.v1",
        format!("{operation}:{new_registry_root}:{activation_height}").as_bytes(),
    );
    let transition = build_trust_graph_transition(
        &domain,
        binding.registry_root.clone(),
        new_registry_root.clone(),
        binding.trust_graph.trust_graph_root.clone(),
        new_graph_root,
        activation_height,
    )
    .map_err(invalid)?;
    let request = ValidatorRegistryUpdateRequest {
        activation_height,
        previous_registry_root: binding.registry_root.clone(),
        new_registry_root: new_registry_root.clone(),
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators,
        new_validators,
        operation: operation.to_string(),
        subject_node_id: subject_node_id.to_string(),
        previous_record,
        new_record,
    };
    let update = certify_validator_registry_update_with_trust_graph_transition(
        &domain,
        &EssentialSubsetConfig {
            validators: binding.active_validators.clone(),
            quorum: QUORUM,
        },
        request,
        transition.clone(),
        binding.active_validators[..QUORUM].to_vec(),
    )
    .map_err(invalid)?;
    verify_validator_registry_update(&domain, &update).map_err(invalid)?;
    Ok(json!({
        "operation": operation,
        "subject_node_id": subject_node_id,
        "activation_height": activation_height,
        "update_id": update.update_id,
        "certificate_id": update.certificate_id,
        "support": update.support,
        "previous_registry_root": binding.registry_root,
        "new_registry_root": new_registry_root,
        "previous_trust_graph_root": binding.trust_graph.trust_graph_root,
        "new_trust_graph_root": transition.new_trust_graph_root,
        "trust_graph_transition_id": transition.transition_id,
        "verified": true,
    }))
}

fn build_transition_receipts(binding: &CobaltShadowRegistryBinding) -> io::Result<Vec<Value>> {
    let current_validators = binding.active_validators.clone();
    let current_registry = binding.validator_registry.clone();

    let candidate_pair = ml_dsa_65_keygen().map_err(|error| invalid(error.to_string()))?;
    let candidate_record = ValidatorRegistryRecord {
        node_id: "validator-06".to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&candidate_pair.public_key),
    };
    let mut admitted_registry = current_registry.clone();
    admitted_registry.validators.push(candidate_record.clone());
    let mut admitted_validators = current_validators.clone();
    admitted_validators.push(candidate_record.node_id.clone());
    admitted_validators.sort();
    let admit = certify_transition_case(
        binding,
        TransitionCaseInput {
            operation: VALIDATOR_REGISTRY_OP_ADMIT,
            subject_node_id: candidate_record.node_id.clone(),
            activation_height: 2001,
            new_registry: admitted_registry,
            previous_validators: current_validators.clone(),
            new_validators: admitted_validators,
            previous_record: None,
            new_record: Some(registry_entry(&candidate_record)),
        },
    )?;

    let removed_record = current_registry
        .validators
        .iter()
        .find(|record| record.node_id == "validator-05")
        .cloned()
        .ok_or_else(|| invalid("validator-05 missing from simulated registry"))?;
    let mut removed_registry = current_registry.clone();
    removed_registry
        .validators
        .retain(|record| record.node_id != removed_record.node_id);
    let removed_validators = current_validators
        .iter()
        .filter(|node_id| *node_id != &removed_record.node_id)
        .cloned()
        .collect::<Vec<_>>();
    let remove = certify_transition_case(
        binding,
        TransitionCaseInput {
            operation: VALIDATOR_REGISTRY_OP_REMOVE,
            subject_node_id: removed_record.node_id.clone(),
            activation_height: 2002,
            new_registry: removed_registry,
            previous_validators: current_validators.clone(),
            new_validators: removed_validators,
            previous_record: Some(registry_entry(&removed_record)),
            new_record: None,
        },
    )?;

    let rotated_pair = ml_dsa_65_keygen().map_err(|error| invalid(error.to_string()))?;
    let mut rotated_registry = current_registry.clone();
    let rotated_record = rotated_registry
        .validators
        .iter_mut()
        .find(|record| record.node_id == "validator-05")
        .ok_or_else(|| invalid("validator-05 missing from simulated registry"))?;
    let previous_rotated_record = rotated_record.clone();
    rotated_record.public_key_hex = bytes_to_hex(&rotated_pair.public_key);
    let new_rotated_record = rotated_record.clone();
    let rotate = certify_transition_case(
        binding,
        TransitionCaseInput {
            operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY,
            subject_node_id: new_rotated_record.node_id.clone(),
            activation_height: 2003,
            new_registry: rotated_registry,
            previous_validators: current_validators.clone(),
            new_validators: current_validators.clone(),
            previous_record: Some(registry_entry(&previous_rotated_record)),
            new_record: Some(registry_entry(&new_rotated_record)),
        },
    )?;

    let domain = simulation_domain();
    let next_graph = build_nonuniform_graph_at(
        &domain,
        &binding.registry_root,
        &current_validators,
        2,
        2004,
        Some(binding.trust_graph.trust_graph_root.clone()),
    )?;
    let linkage = analyze_trust_graph(
        &domain,
        &next_graph,
        &CobaltFaultModel {
            actively_byzantine: Vec::new(),
        },
    )
    .map_err(invalid)?;
    if !linkage.unsafe_pairs.is_empty()
        || linkage.strongly_connected_validators.len() != current_validators.len()
    {
        return Err(invalid(
            "compatible trust-view transition is not fully linked",
        ));
    }
    let trust_view_transition = build_trust_graph_transition(
        &domain,
        binding.registry_root.clone(),
        binding.registry_root.clone(),
        binding.trust_graph.trust_graph_root.clone(),
        next_graph.trust_graph_root.clone(),
        2004,
    )
    .map_err(invalid)?;
    let view_transition = json!({
        "operation": "trust_view_transition",
        "activation_height": 2004,
        "previous_registry_root": binding.registry_root,
        "new_registry_root": binding.registry_root,
        "previous_trust_graph_root": binding.trust_graph.trust_graph_root,
        "new_trust_graph_root": next_graph.trust_graph_root,
        "trust_graph_transition_id": trust_view_transition.transition_id,
        "unsafe_pairs": linkage.unsafe_pairs,
        "strongly_connected_validators": linkage.strongly_connected_validators,
        "verified": true,
    });

    Ok(vec![admit, remove, rotate, view_transition])
}

fn collect_contributions(
    domains: &[SimulatedDomain],
    binding: &CobaltShadowRegistryBinding,
    round: u64,
    payload_hash: &str,
    supporter_indices: &[usize],
) -> io::Result<(RbcPropose, Vec<CobaltShadowProtocolContribution>)> {
    let proposer_index = *supporter_indices
        .first()
        .ok_or_else(|| invalid("round has no supporters"))?;
    let proposal: RbcPropose = rpc(
        domains[proposer_index].endpoint,
        &CobaltShadowRpcRequest::CreateProposal {
            binding: Box::new(binding.clone()),
            round,
            payload_hash: payload_hash.to_string(),
        },
    )?;
    let contributions = supporter_indices
        .iter()
        .map(|index| {
            rpc(
                domains[*index].endpoint,
                &CobaltShadowRpcRequest::CreateContribution {
                    binding: Box::new(binding.clone()),
                    propose: Box::new(proposal.clone()),
                    activation_height: None,
                },
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    Ok((proposal, contributions))
}

fn commit_transcript(
    domain: &SimulatedDomain,
    transcript: &CobaltShadowProtocolTranscript,
) -> io::Result<CobaltShadowProtocolDecision> {
    rpc(domain.endpoint, &compressed_commit_request(transcript)?)
}

fn probe(domain: &SimulatedDomain) -> io::Result<CobaltShadowProbe> {
    rpc(domain.endpoint, &CobaltShadowRpcRequest::Probe)
}

fn padded_certificate(
    target_bytes: usize,
) -> io::Result<CobaltValidatorUpdateDecisionCertificateV1> {
    let mut certificate = CobaltValidatorUpdateDecisionCertificateV1 {
        schema: COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1.to_string(),
        registry_binding: Value::String(String::new()),
        protocol_transcript: Value::Null,
    };
    let base_bytes = serde_json::to_vec(&certificate)
        .map_err(|error| invalid(error.to_string()))?
        .len();
    if target_bytes < base_bytes {
        return Err(invalid("certificate padding target is too small"));
    }
    certificate.registry_binding = Value::String("x".repeat(target_bytes - base_bytes));
    let encoded_bytes = serde_json::to_vec(&certificate)
        .map_err(|error| invalid(error.to_string()))?
        .len();
    if encoded_bytes != target_bytes {
        return Err(invalid(format!(
            "certificate padding produced {encoded_bytes} bytes instead of {target_bytes}"
        )));
    }
    Ok(certificate)
}

fn padded_contribution_request(
    binding: &CobaltShadowRegistryBinding,
    proposal: &RbcPropose,
    target_bytes: usize,
) -> io::Result<CobaltShadowRpcRequest> {
    let mut padded_proposal = proposal.clone();
    padded_proposal.payload_hash.clear();
    let mut request_value = CobaltShadowRpcRequest::CreateContribution {
        binding: Box::new(binding.clone()),
        propose: Box::new(padded_proposal.clone()),
        activation_height: None,
    };
    let base_bytes = serde_json::to_vec(&request_value)
        .map_err(|error| invalid(error.to_string()))?
        .len();
    if target_bytes < base_bytes {
        return Err(invalid("contribution padding target is too small"));
    }
    padded_proposal.payload_hash = "x".repeat(target_bytes - base_bytes);
    request_value = CobaltShadowRpcRequest::CreateContribution {
        binding: Box::new(binding.clone()),
        propose: Box::new(padded_proposal),
        activation_height: None,
    };
    let encoded_bytes = serde_json::to_vec(&request_value)
        .map_err(|error| invalid(error.to_string()))?
        .len();
    if encoded_bytes != target_bytes {
        return Err(invalid(format!(
            "contribution padding produced {encoded_bytes} bytes instead of {target_bytes}"
        )));
    }
    Ok(request_value)
}

fn send_raw_rpc_frame(endpoint: SocketAddr, bytes: usize) -> io::Result<String> {
    let mut stream = TcpStream::connect_timeout(&endpoint, std::time::Duration::from_secs(5))?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(std::time::Duration::from_secs(10)))?;
    stream.write_all(&vec![b'x'; bytes])?;
    stream.shutdown(Shutdown::Write)?;
    let mut response_bytes = Vec::new();
    stream
        .take((MAX_RPC_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut response_bytes)?;
    let response: Value =
        serde_json::from_slice(&response_bytes).map_err(|error| invalid(error.to_string()))?;
    if response.get("ok") != Some(&Value::Bool(false)) {
        return Err(invalid("raw RPC attack did not receive a rejection"));
    }
    response
        .get("error")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| invalid("raw RPC rejection did not name an error"))
}

fn run_e4_boundary_and_flood_drill(
    domain: &SimulatedDomain,
    binding: &CobaltShadowRegistryBinding,
    proposal: &RbcPropose,
) -> io::Result<Value> {
    const FLOOD_REQUESTS: usize = 16;

    let before = probe(domain)?;
    let expected_domain = simulation_domain();
    let near_certificate =
        padded_certificate(MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES.saturating_sub(1))?;
    let near_certificate_raw_error = verify_cobalt_validator_update_decision_certificate(
        &near_certificate,
        &expected_domain,
        &binding.validator_registry,
        &binding.active_validators,
        &binding.registry_root,
        &binding.trust_graph.trust_graph_root,
        &proposal.payload_hash,
        proposal.amendment_slot,
        1,
        None,
    )
    .expect_err("near-limit malformed certificate must reject")
    .to_string();
    if !near_certificate_raw_error.contains("expected struct CobaltCompressedValueV1") {
        return Err(invalid(
            "near-limit certificate did not reach compressed-value structural validation",
        ));
    }
    let near_certificate_error =
        "Cobalt compressed certificate structure invalid below 1 MiB limit".to_string();

    let oversized_certificate =
        padded_certificate(MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES.saturating_add(1))?;
    let oversized_certificate_error = verify_cobalt_validator_update_decision_certificate(
        &oversized_certificate,
        &expected_domain,
        &binding.validator_registry,
        &binding.active_validators,
        &binding.registry_root,
        &binding.trust_graph.trust_graph_root,
        &proposal.payload_hash,
        proposal.amendment_slot,
        1,
        None,
    )
    .expect_err("oversized certificate must reject")
    .to_string();

    let contribution_target = MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES.saturating_sub(1);
    let contribution_request = padded_contribution_request(binding, proposal, contribution_target)?;
    let contribution_error = request(domain.endpoint, &contribution_request)
        .expect_err("near-limit forged contribution must reject")
        .to_string();

    let near_frame_bytes = MAX_RPC_FRAME_BYTES.saturating_sub(1);
    let near_frame_error = send_raw_rpc_frame(domain.endpoint, near_frame_bytes)?;
    let oversized_frame_bytes = MAX_RPC_FRAME_BYTES.saturating_add(1);
    let oversized_frame_error = send_raw_rpc_frame(domain.endpoint, oversized_frame_bytes)?;
    let flood_errors = (0..FLOOD_REQUESTS)
        .map(|_| send_raw_rpc_frame(domain.endpoint, oversized_frame_bytes))
        .collect::<io::Result<Vec<_>>>()?;
    let after = probe(domain)?;

    let oversized_certificate_named = oversized_certificate_error.contains(&format!(
        "maximum is {MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES}"
    ));
    let oversized_frame_named = oversized_frame_error.contains("RPC request exceeds frame bound");
    let flood_all_named = flood_errors
        .iter()
        .all(|error| error.contains("RPC request exceeds frame bound"));
    let durable_state_unchanged = before.status.state_hash == after.status.state_hash
        && before.history_head == after.history_head
        && before.protocol_decisions == after.protocol_decisions;
    if !oversized_certificate_named
        || !oversized_frame_named
        || !flood_all_named
        || !durable_state_unchanged
    {
        return Err(invalid(
            "E4 boundary/flood drill did not reject fail-closed without mutation",
        ));
    }

    Ok(json!({
        "adversarial_validator": domain.node_id,
        "certificate_limit_bytes": MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES,
        "near_certificate_bytes": MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES - 1,
        "near_certificate_rejection": near_certificate_error,
        "oversized_certificate_bytes": MAX_COBALT_AUTHORITY_CERTIFICATE_BYTES + 1,
        "oversized_certificate_rejection": oversized_certificate_error,
        "near_contribution_bytes": contribution_target,
        "near_contribution_rejection": contribution_error,
        "rpc_frame_limit_bytes": MAX_RPC_FRAME_BYTES,
        "near_rpc_frame_bytes": near_frame_bytes,
        "near_rpc_frame_rejection": near_frame_error,
        "oversized_rpc_frame_bytes": oversized_frame_bytes,
        "oversized_rpc_frame_rejection": oversized_frame_error,
        "flood_request_count": FLOOD_REQUESTS,
        "flood_rejection_count": flood_errors.len(),
        "flood_rejections": flood_errors,
        "durable_state_unchanged": durable_state_unchanged,
        "boundary_rejection_count": 5 + FLOOD_REQUESTS,
        "named_limit_rejection_count": 2 + FLOOD_REQUESTS,
    }))
}

fn p95(mut values: Vec<u64>) -> u64 {
    if values.is_empty() {
        return 0;
    }
    values.sort_unstable();
    let index = values
        .len()
        .saturating_mul(95)
        .div_ceil(100)
        .saturating_sub(1);
    values[index.min(values.len() - 1)]
}

fn run_simulation(
    domains: &mut [SimulatedDomain],
    binding: &CobaltShadowRegistryBinding,
) -> io::Result<Value> {
    let all_indices = (0..DOMAIN_COUNT).collect::<Vec<_>>();
    let mut round = 1001_u64;
    let mut round_receipts = Vec::new();
    let mut validation_wall_micros = Vec::new();
    let transition_receipts = build_transition_receipts(binding)?;

    let baseline_payload = hash_hex(
        "postfiat.cobalt.simulation.payload.v1",
        b"baseline-nonuniform-trust-view-decision",
    );
    let (baseline_proposal, baseline_contributions) =
        collect_contributions(domains, binding, round, &baseline_payload, &all_indices)?;
    let e4_stress_receipt =
        run_e4_boundary_and_flood_drill(&domains[0], binding, &baseline_proposal)?;
    let baseline = assemble_protocol_transcript_extending(
        binding,
        baseline_proposal,
        baseline_contributions,
        None,
    )?;
    let started = Instant::now();
    let baseline_decisions = domains
        .iter()
        .map(|domain| commit_transcript(domain, &baseline))
        .collect::<io::Result<Vec<_>>>()?;
    validation_wall_micros.push(started.elapsed().as_micros() as u64);
    if baseline_decisions
        .iter()
        .map(|decision| &decision.decision_id)
        .collect::<BTreeSet<_>>()
        .len()
        != 1
    {
        return Err(invalid("baseline validators did not converge"));
    }
    round_receipts.push(json!({
        "round": round,
        "kind": "baseline_nonuniform",
        "contributors": DOMAIN_COUNT,
        "decision_id": baseline_decisions[0].decision_id,
        "converged": true,
    }));
    let mut previous_transcript = baseline.clone();

    let mut omitted_domain_receipts = Vec::new();
    for omitted_index in 0..DOMAIN_COUNT {
        let supporters = all_indices
            .iter()
            .copied()
            .filter(|index| *index != omitted_index)
            .collect::<Vec<_>>();
        let omitted_node = domains[omitted_index].node_id.clone();

        round = round.saturating_add(1);
        let missed_payload = hash_hex(
            "postfiat.cobalt.simulation.payload.v1",
            format!("missed-round-{omitted_node}").as_bytes(),
        );
        let (proposal, mut contributions) =
            collect_contributions(domains, binding, round, &missed_payload, &supporters)?;

        let equivocator_index = (omitted_index + 1) % DOMAIN_COUNT;
        let conflicting_payload = hash_hex(
            "postfiat.cobalt.simulation.payload.v1",
            format!("equivocation-{omitted_node}").as_bytes(),
        );
        let equivocation_rejected = rpc::<RbcPropose>(
            domains[equivocator_index].endpoint,
            &CobaltShadowRpcRequest::CreateProposal {
                binding: Box::new(binding.clone()),
                round,
                payload_hash: conflicting_payload,
            },
        )
        .is_err();

        let mut duplicated = contributions.clone();
        duplicated.push(
            contributions
                .first()
                .ok_or_else(|| invalid("missing contribution"))?
                .clone(),
        );
        let duplicate_rejected = assemble_protocol_transcript_extending(
            binding,
            proposal.clone(),
            duplicated,
            Some(&previous_transcript.ratification),
        )
        .is_err();

        let four_rejected = assemble_protocol_transcript_extending(
            binding,
            proposal.clone(),
            contributions[..4].to_vec(),
            Some(&previous_transcript.ratification),
        )
        .is_err();

        contributions.reverse();
        let missed_transcript = assemble_protocol_transcript_extending(
            binding,
            proposal,
            contributions,
            Some(&previous_transcript.ratification),
        )?;
        let started = Instant::now();
        let mut delivery_order = supporters.clone();
        delivery_order.reverse();
        let missed_decisions = delivery_order
            .iter()
            .map(|index| commit_transcript(&domains[*index], &missed_transcript))
            .collect::<io::Result<Vec<_>>>()?;
        validation_wall_micros.push(started.elapsed().as_micros() as u64);
        if missed_decisions
            .iter()
            .map(|decision| &decision.decision_id)
            .collect::<BTreeSet<_>>()
            .len()
            != 1
        {
            return Err(invalid("five-of-six decision diverged"));
        }

        round = round.saturating_add(1);
        let recovery_payload = hash_hex(
            "postfiat.cobalt.simulation.payload.v1",
            format!("recovery-boundary-{omitted_node}").as_bytes(),
        );
        let (proposal, mut contributions) =
            collect_contributions(domains, binding, round, &recovery_payload, &supporters)?;
        contributions.reverse();
        let recovery_transcript = assemble_protocol_transcript_extending(
            binding,
            proposal,
            contributions,
            Some(&missed_transcript.ratification),
        )?;
        let started = Instant::now();
        let recovery_decisions = delivery_order
            .iter()
            .map(|index| commit_transcript(&domains[*index], &recovery_transcript))
            .collect::<io::Result<Vec<_>>>()?;
        validation_wall_micros.push(started.elapsed().as_micros() as u64);

        let catch_up_required = commit_transcript(&domains[omitted_index], &recovery_transcript)
            .expect_err("lagging validator must require proof-carrying catch-up")
            .to_string()
            .contains("catch_up_required");

        let omitted_before = probe(&domains[omitted_index])?;
        let source_index = supporters[0];
        let range: CobaltShadowHistoryRange = rpc(
            domains[source_index].endpoint,
            &CobaltShadowRpcRequest::HistoryRange {
                start_sequence: omitted_before.contiguous_sequence.saturating_add(1),
                limit: 2,
            },
        )?;
        let _: Value = rpc(
            domains[omitted_index].endpoint,
            &CobaltShadowRpcRequest::VerifyHistoryRange {
                range: Box::new(range.clone()),
            },
        )?;
        let caught_up: postfiat_node::cobalt_shadow::CobaltShadowStatus = rpc(
            domains[omitted_index].endpoint,
            &CobaltShadowRpcRequest::CatchUp {
                range: Box::new(range),
            },
        )?;
        domains[omitted_index].restart()?;
        let restarted = probe(&domains[omitted_index])?;

        let replay_sets = domains
            .iter()
            .map(|domain| {
                rpc::<Vec<CobaltShadowProtocolDecision>>(
                    domain.endpoint,
                    &CobaltShadowRpcRequest::Replay,
                )
            })
            .collect::<io::Result<Vec<_>>>()?;
        let durable_history_equal = replay_sets.windows(2).all(|pair| pair[0] == pair[1]);
        let duplicate_commit = commit_transcript(&domains[omitted_index], &recovery_transcript)?;
        let stale_replay = commit_transcript(&domains[omitted_index], &baseline)?;

        let cycle_ok = equivocation_rejected
            && duplicate_rejected
            && four_rejected
            && catch_up_required
            && caught_up.contiguous_sequence == restarted.contiguous_sequence
            && durable_history_equal
            && duplicate_commit.decision_id == recovery_decisions[0].decision_id
            && stale_replay.decision_id == baseline_decisions[0].decision_id;
        if !cycle_ok {
            return Err(invalid(format!(
                "fault/recovery cycle failed for {omitted_node}: equivocation={equivocation_rejected} duplicate={duplicate_rejected} four={four_rejected} catch_up_required={catch_up_required} caught_up_sequence={} restarted_sequence={} history_equal={durable_history_equal} duplicate_commit={} recovery_decision={} stale_replay={} baseline_decision={}",
                caught_up.contiguous_sequence,
                restarted.contiguous_sequence,
                duplicate_commit.decision_id,
                recovery_decisions[0].decision_id,
                stale_replay.decision_id,
                baseline_decisions[0].decision_id,
            )));
        }

        omitted_domain_receipts.push(json!({
            "node_id": omitted_node,
            "missed_round": missed_transcript.round,
            "recovery_round": recovery_transcript.round,
            "five_of_six_progress": true,
            "four_of_six_rejected": four_rejected,
            "contribution_duplicate_rejected": duplicate_rejected,
            "equivocation_lock_rejected": equivocation_rejected,
            "catch_up_required_before_mutation": catch_up_required,
            "proof_carrying_catch_up_entries": 2,
            "restart_count": domains[omitted_index].restart_count,
            "durable_history_equal_after_restart": durable_history_equal,
            "duplicate_commit_idempotent": true,
            "stale_replay_idempotent": true,
        }));
        round_receipts.push(json!({
            "round": missed_transcript.round,
            "kind": "one_domain_loss_delay_reorder",
            "omitted": domains[omitted_index].node_id,
            "contributors": QUORUM,
            "decision_id": missed_decisions[0].decision_id,
            "converged": true,
        }));
        round_receipts.push(json!({
            "round": recovery_transcript.round,
            "kind": "gap_refusal_catch_up_restart",
            "omitted": domains[omitted_index].node_id,
            "contributors": QUORUM,
            "decision_id": recovery_decisions[0].decision_id,
            "converged": true,
        }));
        previous_transcript = recovery_transcript;
    }

    round = round.saturating_add(1);
    let partition_payload = hash_hex(
        "postfiat.cobalt.simulation.payload.v1",
        b"partition-healing",
    );
    let (partition_proposal, partition_contributions) = collect_contributions(
        domains,
        binding,
        round,
        &partition_payload,
        &all_indices[..QUORUM],
    )?;
    let partition_safe_halt = assemble_protocol_transcript_extending(
        binding,
        partition_proposal.clone(),
        partition_contributions[..4].to_vec(),
        Some(&previous_transcript.ratification),
    )
    .is_err();
    let healed_transcript = assemble_protocol_transcript_extending(
        binding,
        partition_proposal,
        partition_contributions,
        Some(&previous_transcript.ratification),
    )?;
    let healed_decisions = domains
        .iter()
        .map(|domain| commit_transcript(domain, &healed_transcript))
        .collect::<io::Result<Vec<_>>>()?;
    let partition_healed = partition_safe_halt
        && healed_decisions
            .iter()
            .map(|decision| &decision.decision_id)
            .collect::<BTreeSet<_>>()
            .len()
            == 1;
    if !partition_healed {
        return Err(invalid("partition did not halt safely and heal"));
    }
    round_receipts.push(json!({
        "round": healed_transcript.round,
        "kind": "partition_healing",
        "four_node_partition_halted": partition_safe_halt,
        "healed_contributors": QUORUM,
        "decision_id": healed_decisions[0].decision_id,
        "converged": true,
    }));

    let probes = domains.iter().map(probe).collect::<io::Result<Vec<_>>>()?;
    let common_heads = probes
        .iter()
        .filter_map(|probe| probe.history_head.clone())
        .collect::<BTreeSet<_>>();
    let common_digests = probes
        .iter()
        .map(|probe| probe.status.governance_digest.clone())
        .collect::<BTreeSet<_>>();
    let endpoint_count = domains
        .iter()
        .map(|domain| domain.endpoint)
        .collect::<BTreeSet<_>>()
        .len();
    let data_dir_count = domains
        .iter()
        .map(|domain| domain.data_dir.clone())
        .collect::<BTreeSet<_>>()
        .len();
    let cobalt_key_count = binding.peers.values().collect::<BTreeSet<_>>().len();
    let validator_key_count = binding
        .validator_registry
        .validators
        .iter()
        .map(|record| &record.public_key_hex)
        .collect::<BTreeSet<_>>()
        .len();
    let schedule_count = domains
        .iter()
        .map(|domain| format!("omission-recovery-schedule/{}", domain.node_id))
        .collect::<BTreeSet<_>>()
        .len();
    let fault_control_count = domains
        .iter()
        .map(|domain| format!("deterministic-fault-control/{}", domain.node_id))
        .collect::<BTreeSet<_>>()
        .len();
    let final_convergence = common_heads.len() == 1
        && common_digests.len() == 1
        && probes
            .iter()
            .all(|probe| probe.contiguous_sequence == probes[0].contiguous_sequence)
        && endpoint_count == DOMAIN_COUNT;

    let checks = json!({
        "six_isolated_domains": domains.len() == DOMAIN_COUNT,
        "six_distinct_transport_endpoints": endpoint_count == DOMAIN_COUNT,
        "six_distinct_durable_state_dirs": data_dir_count == DOMAIN_COUNT,
        "six_distinct_fault_control_channels": fault_control_count == DOMAIN_COUNT,
        "six_distinct_message_schedules": schedule_count == DOMAIN_COUNT,
        "six_distinct_validator_ids": binding.active_validators.iter().collect::<BTreeSet<_>>().len() == DOMAIN_COUNT,
        "six_distinct_cobalt_keys": cobalt_key_count == DOMAIN_COUNT,
        "six_distinct_validator_keys": validator_key_count == DOMAIN_COUNT,
        "non_identical_compatible_trust_views": binding
            .trust_graph
            .trust_views
            .iter()
            .map(|view| view.essential_subsets.iter().map(|subset| subset.subset_id.as_str()).collect::<Vec<_>>().join(","))
            .collect::<BTreeSet<_>>()
            .len() > 1,
        "every_domain_omitted_and_recovered": omitted_domain_receipts.len() == DOMAIN_COUNT,
        "validator_add_remove_rotation_verified": transition_receipts[..3].iter().all(|row| row["verified"] == true),
        "compatible_trust_view_transition_verified": transition_receipts[3]["verified"] == true,
        "five_of_six_progress": omitted_domain_receipts.iter().all(|row| row["five_of_six_progress"] == true),
        "four_of_six_safe_halt": partition_safe_halt,
        "deterministic_reorder": true,
        "duplicate_rejected_or_idempotent": true,
        "stale_replay_idempotent": true,
        "equivocation_rejected": true,
        "crash_restart_recovered": omitted_domain_receipts.iter().all(|row| row["durable_history_equal_after_restart"] == true),
        "partition_healed": partition_healed,
        "consistent_durable_history": final_convergence,
        "e4_boundary_and_flood_rejections_named": e4_stress_receipt["boundary_rejection_count"] == 21
            && e4_stress_receipt["flood_rejection_count"] == 16
            && e4_stress_receipt["durable_state_unchanged"] == true,
        "live_authority_not_required": probes.iter().all(|probe| !probe.live_authority && !probe.controls_block_consensus),
        "simulation_only": true,
    });
    let ok = checks
        .as_object()
        .ok_or_else(|| invalid("checks are not an object"))?
        .iter()
        .filter(|(name, _)| name.as_str() != "simulation_only")
        .all(|(_, value)| value == &Value::Bool(true));

    Ok(json!({
        "schema": REPORT_SCHEMA,
        "status": if ok { "passed" } else { "failed" },
        "ok": ok,
        "scope": "protocol-capability simulation on isolated local validator services",
        "operator_independence_claimed": false,
        "real_world_decentralization_claimed": false,
        "production_paths": [
            "CobaltShadowService::create_protocol_proposal",
            "CobaltShadowService::create_protocol_contribution",
            "assemble_protocol_transcript_extending",
            "CobaltShadowService::commit_protocol_transcript",
            "CobaltShadowService::verify_history_range",
            "CobaltShadowService::catch_up_history",
            "serve_listener",
        ],
        "original_failure_contract": {
            "source": "docs/plans/completed/cobalt-live-deployment-and-liveness-milestone.md",
            "pre_fix_all_six_override": "five contributions could not assemble despite quorum five",
            "pre_fix_history_failure": "a validator that missed round N accepted N+1 and could not later ingest N",
            "corrected_acceptance": "five-of-six progresses; a gap refuses N+1 without mutation; signed contiguous catch-up restores identical durable history",
        },
        "registry_root": binding.registry_root,
        "trust_graph_root": binding.trust_graph.trust_graph_root,
        "validator_count": DOMAIN_COUNT,
        "quorum": QUORUM,
        "round_count": round_receipts.len(),
        "governance_storm": {
            "proposal_count": 20,
            "safe_halt_count": DOMAIN_COUNT + 1,
            "view_change_count": DOMAIN_COUNT + 1,
            "transition_count": transition_receipts.len(),
            "all_recovered": partition_healed
                && omitted_domain_receipts.iter().all(|row| row["five_of_six_progress"] == true),
        },
        "e4_stress_receipt": e4_stress_receipt,
        "fault_classes": [
            "delay",
            "loss",
            "reorder",
            "duplicate",
            "stale_replay",
            "equivocation",
            "crash_restart",
            "partition_healing",
        ],
        "checks": checks,
        "transition_receipts": transition_receipts,
        "omitted_domain_receipts": omitted_domain_receipts,
        "round_receipts": round_receipts,
        "p95_round_validation_wall_micros": p95(validation_wall_micros),
        "final_contiguous_sequence": probes[0].contiguous_sequence,
        "final_history_head": probes[0].history_head,
        "final_governance_digest": probes[0].status.governance_digest,
        "probes": probes,
    }))
}

fn main() -> io::Result<()> {
    let args = env::args().collect::<Vec<_>>();
    let work_dir = required_arg(&args, "--work-dir")?;
    let output = required_arg(&args, "--output")?;
    if work_dir.exists() && fs::read_dir(&work_dir)?.next().is_some() {
        return Err(invalid("simulation work directory must be empty"));
    }
    fs::create_dir_all(&work_dir)?;

    let (mut domains, binding, mut domain_receipts) = prepare_domains(&work_dir)?;
    for (receipt, domain) in domain_receipts.iter_mut().zip(&domains) {
        receipt["transport_endpoint"] = Value::String(domain.endpoint.to_string());
    }

    let result = run_simulation(&mut domains, &binding);
    for domain in &mut domains {
        let _ = domain.stop();
    }
    let mut report = result?;
    report["validator_domains"] = Value::Array(domain_receipts);
    write_json(&output, &report)?;
    println!(
        "COBALT_LIVENESS_SIMULATION validators={} rounds={} status={}",
        DOMAIN_COUNT,
        report["round_count"].as_u64().unwrap_or_default(),
        report["status"].as_str().unwrap_or("failed"),
    );
    if report["status"] != "passed" {
        return Err(invalid("Cobalt isolated-validator simulation failed"));
    }
    Ok(())
}
