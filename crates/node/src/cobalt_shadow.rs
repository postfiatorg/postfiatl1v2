//! Durable, governance-only Cobalt shadow service.
//!
//! This module is deliberately disconnected from block ordering, transaction
//! execution, and live governance authorization.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Instant;

use postfiat_consensus_cobalt::{
    abba_strong_support, build_canonical_unl_trust_graph, build_essential_subset,
    build_mvba_valid_input_set, build_trust_graph, build_trust_view,
    evaluate_abba_finish_support_signed, evaluate_rbc_ready_support_signed,
    mvba_candidate_from_rbc_accept, ratify_dabc_amendment, sign_abba_aux, sign_abba_conf,
    sign_abba_finish, sign_abba_init, sign_dabc_full_knowledge_check, sign_rbc_accept,
    sign_rbc_echo, sign_rbc_propose, sign_rbc_ready, validate_abba_aux_signed,
    validate_abba_conf_signed, validate_abba_finish_signed, validate_abba_init_signed,
    validate_dabc_full_knowledge_check_signed, validate_dabc_ratified_amendment,
    validate_rbc_accept_signed, validate_rbc_echo_signed, validate_rbc_propose_signed,
    validate_rbc_ready_signed, validate_trust_graph, AbbaAux, AbbaConf, AbbaFinish, AbbaInit,
    CobaltDomain, CobaltSignatureCommittee, DabcFullKnowledgeCheck, DabcPendingPair,
    DabcRatifiedAmendment, MvbaValidInputSet, RbcAccept, RbcEcho, RbcPropose, RbcReady, TrustGraph,
};
use postfiat_crypto_provider::{
    bytes_to_hex, hash_hex, hex_to_bytes, ml_dsa_65_keygen, ml_dsa_65_sign_with_context,
    ml_dsa_65_validate_public_key, ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
    ML_DSA_65_SIGNATURE_BYTES,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::{
    read_validator_key_file, validator_key_record, validator_registry_record,
    validator_registry_root, ValidatorRegistry,
};

pub const COBALT_SHADOW_STATE_SCHEMA: &str = "postfiat-cobalt-shadow-state-v2";
pub const COBALT_SHADOW_PRIVATE_SCHEMA: &str = "postfiat-cobalt-shadow-private-v1";
pub const COBALT_SHADOW_MESSAGE_SCHEMA: &str = "postfiat-cobalt-shadow-message-v1";
pub const COBALT_SHADOW_BEACON_COMMITMENT_SCHEMA: &str =
    "postfiat-cobalt-shadow-beacon-commitment-v1";
pub const COBALT_SHADOW_BEACON_REVEAL_SCHEMA: &str = "postfiat-cobalt-shadow-beacon-reveal-v1";
pub const COBALT_SHADOW_RANDOMNESS_SOURCE: &str = "ml-dsa65-threshold-commit-reveal-v1";
pub const COBALT_SHADOW_AUTHORITY_MODE: &str = "shadow-advisory";
pub const COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA: &str =
    "postfiat-cobalt-shadow-protocol-transcript-v1";
pub const COBALT_SHADOW_VALIDATOR_BINDING_SCHEMA: &str =
    "postfiat-cobalt-shadow-validator-binding-v1";
pub const COBALT_SHADOW_REGISTRY_BINDING_SCHEMA: &str =
    "postfiat-cobalt-shadow-registry-binding-v1";
pub const COBALT_SHADOW_PROTOCOL_CONTRIBUTION_SCHEMA: &str =
    "postfiat-cobalt-shadow-protocol-contribution-v1";

const STATE_FILE: &str = "state.json";
const PRIVATE_FILE: &str = "signer-private.json";
const STATE_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-shadow/state/v1";
const MESSAGE_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-shadow/message/v1";
const BEACON_COMMITMENT_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/cobalt-shadow/beacon-commitment/v1";
const BEACON_REVEAL_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-shadow/beacon-reveal/v1";
const VALIDATOR_BINDING_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat-l1-v2/cobalt-shadow/validator-binding/v1";
const MAX_STATE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_PRIVATE_FILE_BYTES: u64 = 4 * 1024 * 1024;
const ENTROPY_BYTES: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CobaltShadowMessageKind {
    Rbc,
    Abba,
    Mvba,
    Dabc,
    FullKnowledge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowLimits {
    pub max_peers: usize,
    pub max_queue_messages: usize,
    pub max_message_bytes: usize,
    pub max_seen_messages: usize,
    pub max_randomness_rounds: usize,
}

impl Default for CobaltShadowLimits {
    fn default() -> Self {
        Self {
            max_peers: 64,
            max_queue_messages: 256,
            max_message_bytes: 64 * 1024,
            max_seen_messages: 4096,
            max_randomness_rounds: 128,
        }
    }
}

impl CobaltShadowLimits {
    fn validate(&self) -> io::Result<()> {
        if self.max_peers == 0 || self.max_peers > 256 {
            return Err(invalid("max_peers must be in 1..=256"));
        }
        if self.max_queue_messages == 0 || self.max_queue_messages > 4096 {
            return Err(invalid("max_queue_messages must be in 1..=4096"));
        }
        if !(1024..=1024 * 1024).contains(&self.max_message_bytes) {
            return Err(invalid("max_message_bytes must be in 1024..=1048576"));
        }
        if self.max_seen_messages < self.max_queue_messages || self.max_seen_messages > 65_536 {
            return Err(invalid("max_seen_messages is outside its safe bound"));
        }
        if self.max_randomness_rounds == 0 || self.max_randomness_rounds > 4096 {
            return Err(invalid("max_randomness_rounds must be in 1..=4096"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowIdentity {
    pub node_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowValidatorBinding {
    pub schema: String,
    pub node_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub cobalt_public_key_hex: String,
    pub validator_algorithm: String,
    pub validator_public_key_hex: String,
    pub statement_hash: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowRegistryBinding {
    pub schema: String,
    pub registry_root: String,
    pub active_validators: Vec<String>,
    pub validator_registry: ValidatorRegistry,
    pub trust_graph: TrustGraph,
    pub peers: BTreeMap<String, String>,
    pub validator_bindings: Vec<CobaltShadowValidatorBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowMessage {
    pub schema: String,
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub sender: String,
    pub sequence: u64,
    pub round: u64,
    pub kind: CobaltShadowMessageKind,
    pub payload_hash: String,
    pub common_randomness_hash: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowBeaconCommitment {
    pub schema: String,
    pub sender: String,
    pub round: u64,
    pub commitment_hash: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowBeaconReveal {
    pub schema: String,
    pub sender: String,
    pub round: u64,
    pub commitment_hash: String,
    pub entropy_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowRandomnessRecord {
    pub round: u64,
    pub source: String,
    pub participants: Vec<String>,
    pub contributors: Vec<String>,
    pub threshold: usize,
    pub commitment_root: String,
    pub randomness_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowProtocolTranscript {
    pub schema: String,
    pub registry_root: String,
    pub trust_graph: TrustGraph,
    pub round: u64,
    pub payload_hash: String,
    pub agreement_id: String,
    pub rbc_propose: RbcPropose,
    pub rbc_echoes: Vec<RbcEcho>,
    pub rbc_readies: Vec<RbcReady>,
    pub rbc_accepts: Vec<RbcAccept>,
    pub abba_inits: Vec<AbbaInit>,
    pub abba_auxes: Vec<AbbaAux>,
    pub abba_confs: Vec<AbbaConf>,
    pub abba_finishes: Vec<AbbaFinish>,
    pub mvba_input: MvbaValidInputSet,
    pub ratification: DabcRatifiedAmendment,
    pub full_knowledge_checks: Vec<DabcFullKnowledgeCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowProtocolContribution {
    pub schema: String,
    pub node_id: String,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub round: u64,
    pub payload_hash: String,
    pub agreement_id: String,
    pub rbc_echo: RbcEcho,
    pub rbc_ready: RbcReady,
    pub rbc_accept: RbcAccept,
    pub abba_init: AbbaInit,
    pub abba_aux: AbbaAux,
    pub abba_conf: AbbaConf,
    pub abba_finish: AbbaFinish,
    pub full_knowledge_check: DabcFullKnowledgeCheck,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowProtocolDecision {
    pub round: u64,
    pub transcript_hash: String,
    pub payload_hash: String,
    pub agreement_id: String,
    pub output_candidate_id: String,
    pub ratification_id: String,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub signed_message_count: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowMetrics {
    pub received_messages: u64,
    pub received_bytes: u64,
    pub transmitted_messages: u64,
    pub transmitted_bytes: u64,
    pub protocol_rounds: u64,
    pub protocol_validation_micros: u64,
    pub stage_validation_micros: BTreeMap<String, u64>,
    pub replay_checks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PendingBeacon {
    entropy_hex: String,
    commitment: CobaltShadowBeaconCommitment,
    reveal: Option<CobaltShadowBeaconReveal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct CobaltShadowPrivateState {
    schema: String,
    node_id: String,
    algorithm_id: String,
    public_key_hex: String,
    private_key_hex: String,
    pending_beacons: BTreeMap<u64, PendingBeacon>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowState {
    pub schema: String,
    pub identity: CobaltShadowIdentity,
    pub authority_mode: String,
    pub live_authority: bool,
    pub controls_block_consensus: bool,
    pub public_key_hex: String,
    pub signer_algorithm: String,
    pub boot_count: u64,
    pub outbound_high_watermark: u64,
    pub inbound_high_watermarks: BTreeMap<String, u64>,
    pub peer_public_keys: BTreeMap<String, String>,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub queued_messages: Vec<CobaltShadowMessage>,
    pub seen_message_ids: Vec<String>,
    pub accepted_messages: u64,
    pub duplicate_messages: u64,
    pub rejected_messages: u64,
    pub randomness_rounds: Vec<CobaltShadowRandomnessRecord>,
    pub outbound_protocol_locks: BTreeMap<u64, String>,
    pub ratification_locks: BTreeMap<u64, String>,
    pub protocol_decisions: BTreeMap<u64, CobaltShadowProtocolDecision>,
    pub protocol_high_watermark: u64,
    pub metrics: CobaltShadowMetrics,
    pub limits: CobaltShadowLimits,
    pub governance_digest: String,
    pub state_hash: String,
    pub state_signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowStatus {
    pub schema: String,
    pub node_id: String,
    pub authority_mode: String,
    pub live_authority: bool,
    pub controls_block_consensus: bool,
    pub signer_algorithm: String,
    pub signer_public_key_id: String,
    pub signer_private_key_loaded: bool,
    pub boot_count: u64,
    pub peer_count: usize,
    pub registry_root: String,
    pub trust_graph_root: String,
    pub queue_depth: usize,
    pub accepted_messages: u64,
    pub duplicate_messages: u64,
    pub rejected_messages: u64,
    pub outbound_high_watermark: u64,
    pub randomness_rounds: usize,
    pub ratification_lock_count: usize,
    pub protocol_decision_count: usize,
    pub protocol_high_watermark: u64,
    pub messages_received: u64,
    pub bytes_received: u64,
    pub messages_transmitted: u64,
    pub bytes_transmitted: u64,
    pub protocol_validation_micros: u64,
    pub stage_validation_micros: BTreeMap<String, u64>,
    pub latest_randomness_hash: Option<String>,
    pub governance_digest: String,
    pub state_hash: String,
    pub transport_healthy: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowDrillChecks {
    pub bounded_transport: bool,
    pub production_randomness: bool,
    pub randomness_failure_fails_closed: bool,
    pub restart_recovered_queue: bool,
    pub duplicate_delivery_idempotent: bool,
    pub equivocation_rejected: bool,
    pub bad_signature_rejected: bool,
    pub partition_healed: bool,
    pub censorship_healed: bool,
    pub member_loss_converged: bool,
    pub all_nodes_converged: bool,
    pub live_authority_remained_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltShadowDrillReport {
    pub schema: String,
    pub status: String,
    pub ok: bool,
    pub scope: String,
    pub validator_count: usize,
    pub active_contributor_count: usize,
    pub common_randomness_hash: String,
    pub converged_governance_digest: String,
    pub checks: CobaltShadowDrillChecks,
    pub nodes: Vec<CobaltShadowStatus>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CobaltShadowReceiveOutcome {
    Queued,
    Duplicate,
}

pub struct CobaltShadowService {
    data_dir: PathBuf,
    private: CobaltShadowPrivateState,
    state: CobaltShadowState,
}

impl CobaltShadowService {
    pub fn initialize(
        data_dir: impl Into<PathBuf>,
        identity: CobaltShadowIdentity,
        limits: CobaltShadowLimits,
    ) -> io::Result<Self> {
        validate_identity(&identity)?;
        limits.validate()?;
        let data_dir = data_dir.into();
        fs::create_dir_all(&data_dir)?;
        if data_dir.join(STATE_FILE).exists() || data_dir.join(PRIVATE_FILE).exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Cobalt shadow state already exists",
            ));
        }
        let key_pair = ml_dsa_65_keygen().map_err(crypto_error)?;
        let public_key_hex = bytes_to_hex(&key_pair.public_key);
        let private = CobaltShadowPrivateState {
            schema: COBALT_SHADOW_PRIVATE_SCHEMA.to_string(),
            node_id: identity.node_id.clone(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: public_key_hex.clone(),
            private_key_hex: bytes_to_hex(&key_pair.private_key),
            pending_beacons: BTreeMap::new(),
        };
        let mut peer_public_keys = BTreeMap::new();
        peer_public_keys.insert(identity.node_id.clone(), public_key_hex.clone());
        let state = CobaltShadowState {
            schema: COBALT_SHADOW_STATE_SCHEMA.to_string(),
            identity,
            authority_mode: COBALT_SHADOW_AUTHORITY_MODE.to_string(),
            live_authority: false,
            controls_block_consensus: false,
            public_key_hex,
            signer_algorithm: ML_DSA_65_ALGORITHM.to_string(),
            boot_count: 1,
            outbound_high_watermark: 0,
            inbound_high_watermarks: BTreeMap::new(),
            peer_public_keys,
            registry_root: String::new(),
            trust_graph_root: String::new(),
            queued_messages: Vec::new(),
            seen_message_ids: Vec::new(),
            accepted_messages: 0,
            duplicate_messages: 0,
            rejected_messages: 0,
            randomness_rounds: Vec::new(),
            outbound_protocol_locks: BTreeMap::new(),
            ratification_locks: BTreeMap::new(),
            protocol_decisions: BTreeMap::new(),
            protocol_high_watermark: 0,
            metrics: CobaltShadowMetrics::default(),
            limits,
            governance_digest: hash_hex("postfiat.cobalt.shadow.empty.v1", b""),
            state_hash: String::new(),
            state_signature_hex: String::new(),
        };
        let mut service = Self {
            data_dir,
            private,
            state,
        };
        service.persist_private()?;
        service.persist_state()?;
        Ok(service)
    }

    pub fn open(data_dir: impl Into<PathBuf>) -> io::Result<Self> {
        let data_dir = data_dir.into();
        let private = read_bounded_json(&data_dir.join(PRIVATE_FILE), MAX_PRIVATE_FILE_BYTES)?;
        validate_private_file_permissions(&data_dir.join(PRIVATE_FILE))?;
        let state = read_bounded_json(&data_dir.join(STATE_FILE), MAX_STATE_FILE_BYTES)?;
        let mut service = Self {
            data_dir,
            private,
            state,
        };
        service.validate_loaded()?;
        service.state.boot_count = service
            .state
            .boot_count
            .checked_add(1)
            .ok_or_else(|| invalid("boot count overflow"))?;
        service.persist_state()?;
        Ok(service)
    }

    pub fn inspect(data_dir: impl Into<PathBuf>) -> io::Result<CobaltShadowStatus> {
        let data_dir = data_dir.into();
        let private = read_bounded_json(&data_dir.join(PRIVATE_FILE), MAX_PRIVATE_FILE_BYTES)?;
        validate_private_file_permissions(&data_dir.join(PRIVATE_FILE))?;
        let state = read_bounded_json(&data_dir.join(STATE_FILE), MAX_STATE_FILE_BYTES)?;
        let service = Self {
            data_dir,
            private,
            state,
        };
        service.validate_loaded()?;
        Ok(service.status())
    }

    pub fn state(&self) -> &CobaltShadowState {
        &self.state
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn status(&self) -> CobaltShadowStatus {
        CobaltShadowStatus {
            schema: "postfiat-cobalt-shadow-status-v1".to_string(),
            node_id: self.state.identity.node_id.clone(),
            authority_mode: self.state.authority_mode.clone(),
            live_authority: self.state.live_authority,
            controls_block_consensus: self.state.controls_block_consensus,
            signer_algorithm: self.state.signer_algorithm.clone(),
            signer_public_key_id: hash_hex(
                "postfiat.cobalt.shadow.signer.public-key.v1",
                self.state.public_key_hex.as_bytes(),
            ),
            signer_private_key_loaded: !self.private.private_key_hex.is_empty(),
            boot_count: self.state.boot_count,
            peer_count: self.state.peer_public_keys.len(),
            registry_root: self.state.registry_root.clone(),
            trust_graph_root: self.state.trust_graph_root.clone(),
            queue_depth: self.state.queued_messages.len(),
            accepted_messages: self.state.accepted_messages,
            duplicate_messages: self.state.duplicate_messages,
            rejected_messages: self.state.rejected_messages,
            outbound_high_watermark: self.state.outbound_high_watermark,
            randomness_rounds: self.state.randomness_rounds.len(),
            ratification_lock_count: self.state.ratification_locks.len(),
            protocol_decision_count: self.state.protocol_decisions.len(),
            protocol_high_watermark: self.state.protocol_high_watermark,
            messages_received: self.state.metrics.received_messages,
            bytes_received: self.state.metrics.received_bytes,
            messages_transmitted: self.state.metrics.transmitted_messages,
            bytes_transmitted: self.state.metrics.transmitted_bytes,
            protocol_validation_micros: self.state.metrics.protocol_validation_micros,
            stage_validation_micros: self.state.metrics.stage_validation_micros.clone(),
            latest_randomness_hash: self
                .state
                .randomness_rounds
                .last()
                .map(|record| record.randomness_hash.clone()),
            governance_digest: self.state.governance_digest.clone(),
            state_hash: self.state.state_hash.clone(),
            transport_healthy: self.state.queued_messages.len()
                <= self.state.limits.max_queue_messages,
        }
    }

    pub fn create_validator_binding(
        &self,
        registry_root: impl Into<String>,
        validator_key_file: &Path,
    ) -> io::Result<CobaltShadowValidatorBinding> {
        let registry_root = registry_root.into();
        validate_hash("registry root", &registry_root)?;
        let key_file = read_validator_key_file(validator_key_file)?;
        let key_record = validator_key_record(&key_file, &self.state.identity.node_id)?;
        if key_record.algorithm_id != ML_DSA_65_ALGORITHM {
            return Err(invalid(
                "validator binding requires an ML-DSA-65 validator key",
            ));
        }
        let mut binding = CobaltShadowValidatorBinding {
            schema: COBALT_SHADOW_VALIDATOR_BINDING_SCHEMA.to_string(),
            node_id: self.state.identity.node_id.clone(),
            chain_id: self.state.identity.chain_id.clone(),
            genesis_hash: self.state.identity.genesis_hash.clone(),
            protocol_version: self.state.identity.protocol_version,
            registry_root,
            cobalt_public_key_hex: self.state.public_key_hex.clone(),
            validator_algorithm: key_record.algorithm_id.clone(),
            validator_public_key_hex: key_record.public_key_hex.clone(),
            statement_hash: String::new(),
            signature_hex: String::new(),
        };
        binding.statement_hash = validator_binding_statement_hash(&binding)?;
        let private_key =
            Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(crypto_error)?);
        binding.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                private_key.as_slice(),
                binding.statement_hash.as_bytes(),
                VALIDATOR_BINDING_SIGNATURE_CONTEXT,
            )
            .map_err(crypto_error)?,
        );
        validate_validator_binding(&binding)?;
        Ok(binding)
    }

    pub fn bind_registry_manifest(
        &mut self,
        binding: &CobaltShadowRegistryBinding,
    ) -> io::Result<()> {
        if binding.schema != COBALT_SHADOW_REGISTRY_BINDING_SCHEMA {
            return Err(invalid("unsupported Cobalt shadow registry binding schema"));
        }
        if binding.active_validators.is_empty()
            || binding.active_validators.len() > self.state.limits.max_peers
        {
            return Err(invalid("active validator set is empty or oversized"));
        }
        let mut active_validators = binding.active_validators.clone();
        active_validators.sort();
        active_validators.dedup();
        if active_validators != binding.active_validators {
            return Err(invalid("active validators must be sorted and unique"));
        }
        let computed_root =
            validator_registry_root(&binding.validator_registry, &binding.active_validators)?;
        if computed_root != binding.registry_root {
            return Err(invalid("live validator registry root mismatch"));
        }
        let peer_ids = binding.peers.keys().cloned().collect::<Vec<_>>();
        if peer_ids != binding.active_validators {
            return Err(invalid("Cobalt peers do not match active validators"));
        }
        let graph_ids = binding
            .trust_graph
            .trust_views
            .iter()
            .map(|view| view.validator.clone())
            .collect::<Vec<_>>();
        if graph_ids != binding.active_validators {
            return Err(invalid("trust graph does not match active validators"));
        }
        if binding.validator_bindings.len() != binding.active_validators.len() {
            return Err(invalid("validator binding cardinality mismatch"));
        }
        let mut seen = BTreeSet::new();
        for validator_binding in &binding.validator_bindings {
            validate_validator_binding(validator_binding)?;
            if !seen.insert(validator_binding.node_id.clone()) {
                return Err(invalid("duplicate validator binding"));
            }
            if validator_binding.chain_id != self.state.identity.chain_id
                || validator_binding.genesis_hash != self.state.identity.genesis_hash
                || validator_binding.protocol_version != self.state.identity.protocol_version
                || validator_binding.registry_root != binding.registry_root
                || binding.peers.get(&validator_binding.node_id)
                    != Some(&validator_binding.cobalt_public_key_hex)
            {
                return Err(invalid("validator binding domain or signer mismatch"));
            }
            let registry_record =
                validator_registry_record(&binding.validator_registry, &validator_binding.node_id)?;
            if registry_record.algorithm_id != validator_binding.validator_algorithm
                || registry_record.public_key_hex != validator_binding.validator_public_key_hex
            {
                return Err(invalid(
                    "validator binding is not anchored in the live registry",
                ));
            }
        }
        if seen.into_iter().collect::<Vec<_>>() != binding.active_validators {
            return Err(invalid("validator bindings do not match active validators"));
        }
        if binding.peers.get(&self.state.identity.node_id) != Some(&self.state.public_key_hex) {
            return Err(invalid(
                "local Cobalt signer is absent from the live binding",
            ));
        }
        self.bind_registry(
            binding.registry_root.clone(),
            &binding.trust_graph,
            binding.peers.clone(),
        )
    }

    pub fn replace_peer_registry(&mut self, peers: BTreeMap<String, String>) -> io::Result<()> {
        if peers.is_empty() || peers.len() > self.state.limits.max_peers {
            return Err(invalid("peer registry is empty or oversized"));
        }
        if peers.get(&self.state.identity.node_id) != Some(&self.state.public_key_hex) {
            return Err(invalid(
                "peer registry does not preserve the local signer key",
            ));
        }
        for (node_id, public_key_hex) in &peers {
            validate_node_id(node_id)?;
            let public_key = hex_to_bytes(public_key_hex).map_err(crypto_error)?;
            ml_dsa_65_validate_public_key(&public_key).map_err(crypto_error)?;
        }
        self.state.peer_public_keys = peers;
        self.persist_state()
    }

    pub fn bind_registry(
        &mut self,
        registry_root: impl Into<String>,
        graph: &TrustGraph,
        peers: BTreeMap<String, String>,
    ) -> io::Result<()> {
        let registry_root = registry_root.into();
        validate_hash("registry root", &registry_root)?;
        validate_hash("trust graph root", &graph.trust_graph_root)?;
        let domain = self.cobalt_domain();
        validate_trust_graph(&domain, graph).map_err(consensus_error)?;
        if graph.registry_root != registry_root {
            return Err(invalid("trust graph is bound to a different registry root"));
        }
        let graph_validators = graph
            .trust_views
            .iter()
            .map(|view| view.validator.clone())
            .collect::<BTreeSet<_>>();
        let peer_validators = peers.keys().cloned().collect::<BTreeSet<_>>();
        if graph_validators != peer_validators {
            return Err(invalid("trust graph validators do not match peer registry"));
        }
        self.replace_peer_registry(peers)?;
        self.state.registry_root = registry_root;
        self.state.trust_graph_root = graph.trust_graph_root.clone();
        self.persist_state()
    }

    pub fn reserve_protocol_round(
        &mut self,
        round: u64,
        payload_hash: impl Into<String>,
    ) -> io::Result<()> {
        if round == 0 {
            return Err(invalid("protocol round must be nonzero"));
        }
        let payload_hash = payload_hash.into();
        validate_hash("protocol payload", &payload_hash)?;
        if self.state.registry_root.is_empty() || self.state.trust_graph_root.is_empty() {
            return Err(invalid("validator registry is not bound"));
        }
        if let Some(existing) = self.state.outbound_protocol_locks.get(&round) {
            if existing == &payload_hash {
                return Ok(());
            }
            return Err(invalid("protocol round is locked to another payload"));
        }
        self.state
            .outbound_protocol_locks
            .insert(round, payload_hash);
        // The payload lock is durable before any stage signature can leave the
        // process. A restart can skip work but cannot sign a conflicting value.
        self.persist_state()
    }

    pub fn create_protocol_proposal(
        &mut self,
        binding: &CobaltShadowRegistryBinding,
        round: u64,
        payload_hash: impl Into<String>,
    ) -> io::Result<RbcPropose> {
        self.bind_registry_manifest(binding)?;
        let payload_hash = payload_hash.into();
        self.reserve_protocol_round(round, payload_hash.clone())?;
        let private_key = Zeroizing::new(self.protocol_private_key()?);
        sign_rbc_propose(
            &self.cobalt_domain(),
            binding.trust_graph.trust_graph_root.clone(),
            self.state.identity.node_id.clone(),
            round,
            payload_hash,
            private_key.as_slice(),
        )
        .map_err(consensus_error)
    }

    pub fn create_protocol_contribution(
        &mut self,
        binding: &CobaltShadowRegistryBinding,
        propose: &RbcPropose,
    ) -> io::Result<CobaltShadowProtocolContribution> {
        self.bind_registry_manifest(binding)?;
        let domain = self.cobalt_domain();
        let committee = self.signature_committee()?;
        validate_rbc_propose_signed(&domain, &committee, propose).map_err(consensus_error)?;
        if propose.trust_graph_root != binding.trust_graph.trust_graph_root {
            return Err(invalid("protocol proposal trust graph mismatch"));
        }
        self.reserve_protocol_round(propose.amendment_slot, propose.payload_hash.clone())?;
        let private_key = Zeroizing::new(self.protocol_private_key()?);
        let node_id = self.state.identity.node_id.clone();
        let agreement_id = hash_serialized(
            "postfiat.cobalt.shadow.agreement.v1",
            &(
                propose.amendment_slot,
                &propose.message_id,
                &binding.trust_graph.trust_graph_root,
            ),
        )?;
        let rbc_echo = sign_rbc_echo(&domain, propose, &node_id, private_key.as_slice())
            .map_err(consensus_error)?;
        let rbc_ready = sign_rbc_ready(&domain, propose, &node_id, private_key.as_slice())
            .map_err(consensus_error)?;
        let rbc_accept = sign_rbc_accept(&domain, propose, &node_id, private_key.as_slice())
            .map_err(consensus_error)?;
        let abba_init = sign_abba_init(
            &domain,
            binding.trust_graph.trust_graph_root.clone(),
            &node_id,
            agreement_id.clone(),
            propose.amendment_slot,
            true,
            private_key.as_slice(),
        )
        .map_err(consensus_error)?;
        let abba_aux = sign_abba_aux(
            &domain,
            binding.trust_graph.trust_graph_root.clone(),
            &node_id,
            agreement_id.clone(),
            propose.amendment_slot,
            true,
            private_key.as_slice(),
        )
        .map_err(consensus_error)?;
        let abba_conf = sign_abba_conf(
            &domain,
            binding.trust_graph.trust_graph_root.clone(),
            &node_id,
            agreement_id.clone(),
            propose.amendment_slot,
            true,
            private_key.as_slice(),
        )
        .map_err(consensus_error)?;
        let abba_finish = sign_abba_finish(
            &domain,
            binding.trust_graph.trust_graph_root.clone(),
            &node_id,
            agreement_id.clone(),
            propose.amendment_slot,
            true,
            private_key.as_slice(),
        )
        .map_err(consensus_error)?;
        let candidate = mvba_candidate_from_rbc_accept(&domain, propose, &rbc_accept)
            .map_err(consensus_error)?;
        let mvba_input = build_mvba_valid_input_set(
            &domain,
            binding
                .trust_graph
                .trust_views
                .first()
                .ok_or_else(|| invalid("trust graph has no views"))?,
            agreement_id.clone(),
            vec![candidate],
        )
        .map_err(consensus_error)?;
        let checkpoint_height = propose
            .amendment_slot
            .checked_add(1)
            .ok_or_else(|| invalid("protocol activation height overflow"))?;
        let full_knowledge_check = sign_dabc_full_knowledge_check(
            &domain,
            binding.trust_graph.trust_graph_root.clone(),
            &node_id,
            checkpoint_height,
            vec![DabcPendingPair {
                amendment_slot: propose.amendment_slot,
                output_candidate_id: mvba_input.output_candidate_id,
            }],
            private_key.as_slice(),
        )
        .map_err(consensus_error)?;
        Ok(CobaltShadowProtocolContribution {
            schema: COBALT_SHADOW_PROTOCOL_CONTRIBUTION_SCHEMA.to_string(),
            node_id,
            registry_root: binding.registry_root.clone(),
            trust_graph_root: binding.trust_graph.trust_graph_root.clone(),
            round: propose.amendment_slot,
            payload_hash: propose.payload_hash.clone(),
            agreement_id,
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

    pub fn commit_protocol_transcript(
        &mut self,
        transcript: &CobaltShadowProtocolTranscript,
    ) -> io::Result<CobaltShadowProtocolDecision> {
        let encoded = serde_json::to_vec(transcript).map_err(json_error)?;
        if encoded.len() > self.state.limits.max_message_bytes.saturating_mul(32) {
            return Err(invalid("protocol transcript exceeds transport bound"));
        }
        let started = Instant::now();
        let (decision, stage_micros) = self.validate_protocol_transcript(transcript)?;
        if let Some(existing) = self.state.ratification_locks.get(&transcript.round) {
            if existing != &decision.ratification_id {
                return Err(invalid("ratification lock conflict"));
            }
            return self
                .state
                .protocol_decisions
                .get(&transcript.round)
                .cloned()
                .ok_or_else(|| invalid("ratification lock has no decision"));
        }
        if transcript.round < self.state.protocol_high_watermark {
            return Err(invalid(
                "stale protocol transcript is below the high-water mark",
            ));
        }
        self.state
            .ratification_locks
            .insert(transcript.round, decision.ratification_id.clone());
        self.state
            .protocol_decisions
            .insert(transcript.round, decision.clone());
        self.state.protocol_high_watermark =
            self.state.protocol_high_watermark.max(transcript.round);
        self.state.metrics.protocol_rounds = self.state.metrics.protocol_rounds.saturating_add(1);
        self.state.metrics.protocol_validation_micros = self
            .state
            .metrics
            .protocol_validation_micros
            .saturating_add(started.elapsed().as_micros().try_into().unwrap_or(u64::MAX));
        for (stage, micros) in stage_micros {
            let total = self
                .state
                .metrics
                .stage_validation_micros
                .entry(stage)
                .or_default();
            *total = total.saturating_add(micros);
        }
        self.state.metrics.received_messages = self
            .state
            .metrics
            .received_messages
            .saturating_add(decision.signed_message_count as u64);
        self.state.metrics.received_bytes = self
            .state
            .metrics
            .received_bytes
            .saturating_add(encoded.len() as u64);
        self.state.governance_digest = hash_serialized(
            "postfiat.cobalt.shadow.protocol-decisions.v1",
            &self.state.protocol_decisions,
        )?;
        self.persist_state()?;
        Ok(decision)
    }

    pub fn replay_protocol_state(&mut self) -> io::Result<Vec<CobaltShadowProtocolDecision>> {
        self.validate_loaded()?;
        for (round, decision) in &self.state.protocol_decisions {
            if self.state.ratification_locks.get(round) != Some(&decision.ratification_id) {
                return Err(invalid("protocol replay found a lock mismatch"));
            }
        }
        self.state.metrics.replay_checks = self.state.metrics.replay_checks.saturating_add(1);
        self.persist_state()?;
        Ok(self.state.protocol_decisions.values().cloned().collect())
    }

    fn validate_protocol_transcript(
        &self,
        transcript: &CobaltShadowProtocolTranscript,
    ) -> io::Result<(CobaltShadowProtocolDecision, BTreeMap<String, u64>)> {
        if transcript.schema != COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA
            || transcript.round == 0
            || transcript.registry_root != self.state.registry_root
            || transcript.trust_graph.registry_root != self.state.registry_root
            || transcript.trust_graph.trust_graph_root != self.state.trust_graph_root
            || transcript.payload_hash != transcript.rbc_propose.payload_hash
        {
            return Err(invalid("protocol transcript domain or root mismatch"));
        }
        if let Some(outbound_lock) = self.state.outbound_protocol_locks.get(&transcript.round) {
            if outbound_lock != &transcript.payload_hash {
                return Err(invalid(
                    "protocol transcript conflicts with the durable outbound lock",
                ));
            }
        }
        let domain = self.cobalt_domain();
        validate_trust_graph(&domain, &transcript.trust_graph).map_err(consensus_error)?;
        let committee = self.signature_committee()?;
        let mut stage_micros = BTreeMap::new();
        let rbc_started = Instant::now();
        validate_rbc_propose_signed(&domain, &committee, &transcript.rbc_propose)
            .map_err(consensus_error)?;
        for message in &transcript.rbc_echoes {
            validate_rbc_echo_signed(&domain, &committee, message, &transcript.rbc_propose)
                .map_err(consensus_error)?;
        }
        for message in &transcript.rbc_readies {
            validate_rbc_ready_signed(&domain, &committee, message, &transcript.rbc_propose)
                .map_err(consensus_error)?;
        }
        for message in &transcript.rbc_accepts {
            validate_rbc_accept_signed(&domain, &committee, message, &transcript.rbc_propose)
                .map_err(consensus_error)?;
        }
        let local_view = transcript
            .trust_graph
            .trust_views
            .iter()
            .find(|view| view.validator == self.state.identity.node_id)
            .ok_or_else(|| invalid("local validator has no trust view"))?;
        let ready_support = evaluate_rbc_ready_support_signed(
            &domain,
            &committee,
            local_view,
            &transcript.rbc_propose,
            &transcript.rbc_readies,
        )
        .map_err(consensus_error)?;
        if !ready_support.strong_support {
            return Err(invalid("RBC ready strong support was not reached"));
        }
        stage_micros.insert(
            "rbc".to_string(),
            rbc_started
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
        );
        let abba_started = Instant::now();
        for message in &transcript.abba_inits {
            validate_abba_init_signed(&domain, &committee, message).map_err(consensus_error)?;
        }
        for message in &transcript.abba_auxes {
            validate_abba_aux_signed(&domain, &committee, message).map_err(consensus_error)?;
        }
        for message in &transcript.abba_confs {
            validate_abba_conf_signed(&domain, &committee, message).map_err(consensus_error)?;
        }
        for message in &transcript.abba_finishes {
            validate_abba_finish_signed(&domain, &committee, message).map_err(consensus_error)?;
        }
        let finish_support = evaluate_abba_finish_support_signed(
            &domain,
            &committee,
            local_view,
            &transcript.agreement_id,
            transcript.round,
            true,
            &transcript.abba_finishes,
        )
        .map_err(consensus_error)?;
        if !abba_strong_support(&finish_support) {
            return Err(invalid("ABBA finish strong support was not reached"));
        }
        stage_micros.insert(
            "abba".to_string(),
            abba_started
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
        );
        let mvba_started = Instant::now();
        let first_accept = transcript
            .rbc_accepts
            .first()
            .ok_or_else(|| invalid("protocol transcript has no RBC accept"))?;
        let candidate =
            mvba_candidate_from_rbc_accept(&domain, &transcript.rbc_propose, first_accept)
                .map_err(consensus_error)?;
        let input_view = transcript
            .trust_graph
            .trust_views
            .iter()
            .find(|view| view.trust_view_id == transcript.mvba_input.trust_view_id)
            .ok_or_else(|| invalid("MVBA input trust view is not in graph"))?;
        let expected_input = build_mvba_valid_input_set(
            &domain,
            input_view,
            transcript.agreement_id.clone(),
            vec![candidate],
        )
        .map_err(consensus_error)?;
        if expected_input != transcript.mvba_input {
            return Err(invalid("MVBA input set mismatch"));
        }
        stage_micros.insert(
            "mvba".to_string(),
            mvba_started
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
        );
        let dabc_started = Instant::now();
        validate_dabc_ratified_amendment(
            &domain,
            &transcript.trust_graph,
            &transcript.ratification,
            None,
        )
        .map_err(consensus_error)?;
        let expected_ratification = ratify_dabc_amendment(
            &domain,
            &transcript.trust_graph,
            &transcript.mvba_input,
            None,
            transcript.ratification.activation_height,
        )
        .map_err(consensus_error)?;
        if expected_ratification != transcript.ratification {
            return Err(invalid("DABC ratification mismatch"));
        }
        for check in &transcript.full_knowledge_checks {
            validate_dabc_full_knowledge_check_signed(&domain, &committee, check)
                .map_err(consensus_error)?;
        }
        if transcript.full_knowledge_checks.len() != committee.len() {
            return Err(invalid("DABC full-knowledge signer set is incomplete"));
        }
        stage_micros.insert(
            "dabc".to_string(),
            dabc_started
                .elapsed()
                .as_micros()
                .try_into()
                .unwrap_or(u64::MAX),
        );
        let transcript_hash =
            hash_serialized("postfiat.cobalt.shadow.protocol-transcript.v1", transcript)?;
        let signed_message_count = 1
            + transcript.rbc_echoes.len()
            + transcript.rbc_readies.len()
            + transcript.rbc_accepts.len()
            + transcript.abba_inits.len()
            + transcript.abba_auxes.len()
            + transcript.abba_confs.len()
            + transcript.abba_finishes.len()
            + transcript.full_knowledge_checks.len();
        Ok((
            CobaltShadowProtocolDecision {
                round: transcript.round,
                transcript_hash,
                payload_hash: transcript.payload_hash.clone(),
                agreement_id: transcript.agreement_id.clone(),
                output_candidate_id: transcript.mvba_input.output_candidate_id.clone(),
                ratification_id: transcript.ratification.ratification_id.clone(),
                registry_root: transcript.registry_root.clone(),
                trust_graph_root: transcript.trust_graph.trust_graph_root.clone(),
                signed_message_count,
            },
            stage_micros,
        ))
    }

    fn cobalt_domain(&self) -> CobaltDomain {
        CobaltDomain {
            chain_id: self.state.identity.chain_id.clone(),
            genesis_hash: self.state.identity.genesis_hash.clone(),
            protocol_version: self.state.identity.protocol_version,
        }
    }

    fn signature_committee(&self) -> io::Result<CobaltSignatureCommittee> {
        let mut committee = CobaltSignatureCommittee::default();
        for (validator, public_key_hex) in &self.state.peer_public_keys {
            committee
                .insert_hex(validator.clone(), public_key_hex)
                .map_err(consensus_error)?;
        }
        Ok(committee)
    }

    pub(crate) fn protocol_private_key(&self) -> io::Result<Vec<u8>> {
        hex_to_bytes(&self.private.private_key_hex).map_err(crypto_error)
    }

    pub fn create_beacon_commitment(
        &mut self,
        round: u64,
    ) -> io::Result<CobaltShadowBeaconCommitment> {
        if let Some(pending) = self.private.pending_beacons.get(&round) {
            return Ok(pending.commitment.clone());
        }
        if self.private.pending_beacons.len() >= self.state.limits.max_randomness_rounds {
            return Err(invalid("pending randomness rounds are full"));
        }
        let entropy_hex = bytes_to_hex(&os_random_bytes::<ENTROPY_BYTES>()?);
        let commitment_hash = beacon_commitment_hash(
            &self.state.identity,
            &self.state.identity.node_id,
            round,
            &entropy_hex,
        )?;
        let mut commitment = CobaltShadowBeaconCommitment {
            schema: COBALT_SHADOW_BEACON_COMMITMENT_SCHEMA.to_string(),
            sender: self.state.identity.node_id.clone(),
            round,
            commitment_hash,
            signature_hex: String::new(),
        };
        commitment.signature_hex = self.sign_bytes(
            &beacon_commitment_signing_bytes(&self.state.identity, &commitment)?,
            BEACON_COMMITMENT_SIGNATURE_CONTEXT,
        )?;
        self.private.pending_beacons.insert(
            round,
            PendingBeacon {
                entropy_hex,
                commitment: commitment.clone(),
                reveal: None,
            },
        );
        self.persist_private()?;
        Ok(commitment)
    }

    pub fn create_beacon_reveal(&mut self, round: u64) -> io::Result<CobaltShadowBeaconReveal> {
        let pending = self
            .private
            .pending_beacons
            .get(&round)
            .cloned()
            .ok_or_else(|| invalid("beacon commitment does not exist"))?;
        if let Some(reveal) = pending.reveal {
            return Ok(reveal);
        }
        let mut reveal = CobaltShadowBeaconReveal {
            schema: COBALT_SHADOW_BEACON_REVEAL_SCHEMA.to_string(),
            sender: self.state.identity.node_id.clone(),
            round,
            commitment_hash: pending.commitment.commitment_hash,
            entropy_hex: pending.entropy_hex,
            signature_hex: String::new(),
        };
        reveal.signature_hex = self.sign_bytes(
            &beacon_reveal_signing_bytes(&self.state.identity, &reveal)?,
            BEACON_REVEAL_SIGNATURE_CONTEXT,
        )?;
        self.private
            .pending_beacons
            .get_mut(&round)
            .expect("pending beacon checked")
            .reveal = Some(reveal.clone());
        self.persist_private()?;
        Ok(reveal)
    }

    pub fn install_common_randomness(
        &mut self,
        round: u64,
        participants: Vec<String>,
        threshold: usize,
        commitments: Vec<CobaltShadowBeaconCommitment>,
        reveals: Vec<CobaltShadowBeaconReveal>,
    ) -> io::Result<CobaltShadowRandomnessRecord> {
        if let Some(existing) = self
            .state
            .randomness_rounds
            .iter()
            .find(|record| record.round == round)
        {
            return Ok(existing.clone());
        }
        if self.state.randomness_rounds.len() >= self.state.limits.max_randomness_rounds {
            return Err(invalid("randomness history is full"));
        }
        let participants = sorted_unique(participants, "randomness participant")?;
        if participants.is_empty() || participants.len() > self.state.limits.max_peers {
            return Err(invalid("randomness participant set is invalid"));
        }
        if threshold == 0 || threshold > participants.len() {
            return Err(invalid("randomness threshold is invalid"));
        }
        if commitments.len() > participants.len() || reveals.len() > participants.len() {
            return Err(invalid("randomness evidence is oversized"));
        }
        let participant_set = participants.iter().cloned().collect::<BTreeSet<_>>();
        let mut commitment_map = BTreeMap::new();
        for commitment in commitments {
            if commitment.schema != COBALT_SHADOW_BEACON_COMMITMENT_SCHEMA
                || commitment.round != round
                || !participant_set.contains(&commitment.sender)
            {
                return Err(invalid("beacon commitment domain mismatch"));
            }
            verify_peer_signature(
                &self.state,
                &commitment.sender,
                &beacon_commitment_signing_bytes(&self.state.identity, &commitment)?,
                &commitment.signature_hex,
                BEACON_COMMITMENT_SIGNATURE_CONTEXT,
            )?;
            if commitment_map
                .insert(commitment.sender.clone(), commitment)
                .is_some()
            {
                return Err(invalid("duplicate beacon commitment"));
            }
        }
        let mut reveal_map = BTreeMap::new();
        for reveal in reveals {
            if reveal.schema != COBALT_SHADOW_BEACON_REVEAL_SCHEMA
                || reveal.round != round
                || !participant_set.contains(&reveal.sender)
            {
                return Err(invalid("beacon reveal domain mismatch"));
            }
            let commitment = commitment_map
                .get(&reveal.sender)
                .ok_or_else(|| invalid("beacon reveal has no commitment"))?;
            if commitment.commitment_hash != reveal.commitment_hash
                || beacon_commitment_hash(
                    &self.state.identity,
                    &reveal.sender,
                    round,
                    &reveal.entropy_hex,
                )? != reveal.commitment_hash
            {
                return Err(invalid("beacon reveal does not open commitment"));
            }
            verify_peer_signature(
                &self.state,
                &reveal.sender,
                &beacon_reveal_signing_bytes(&self.state.identity, &reveal)?,
                &reveal.signature_hex,
                BEACON_REVEAL_SIGNATURE_CONTEXT,
            )?;
            if reveal_map.insert(reveal.sender.clone(), reveal).is_some() {
                return Err(invalid("duplicate beacon reveal"));
            }
        }
        if reveal_map.len() < threshold {
            return Err(invalid("common randomness reveal threshold not reached"));
        }
        let contributors = reveal_map.keys().cloned().collect::<Vec<_>>();
        let commitment_root = hash_serialized(
            "postfiat.cobalt.shadow.beacon.commitment-root.v1",
            &(round, &participants, &commitment_map),
        )?;
        let randomness_hash = hash_serialized(
            "postfiat.cobalt.shadow.common-randomness.v1",
            &(
                &self.state.identity.chain_id,
                &self.state.identity.genesis_hash,
                self.state.identity.protocol_version,
                round,
                &participants,
                threshold,
                &commitment_root,
                &reveal_map,
            ),
        )?;
        let record = CobaltShadowRandomnessRecord {
            round,
            source: COBALT_SHADOW_RANDOMNESS_SOURCE.to_string(),
            participants,
            contributors,
            threshold,
            commitment_root,
            randomness_hash,
        };
        self.state.randomness_rounds.push(record.clone());
        self.state
            .randomness_rounds
            .sort_by_key(|entry| entry.round);
        self.private.pending_beacons.remove(&round);
        self.persist_private()?;
        self.persist_state()?;
        Ok(record)
    }

    pub fn sign_message(
        &mut self,
        round: u64,
        kind: CobaltShadowMessageKind,
        payload_hash: impl Into<String>,
    ) -> io::Result<CobaltShadowMessage> {
        let payload_hash = payload_hash.into();
        validate_hash("payload", &payload_hash)?;
        let randomness_hash = self
            .state
            .randomness_rounds
            .iter()
            .find(|record| record.round == round)
            .map(|record| record.randomness_hash.clone())
            .ok_or_else(|| invalid("round has no installed common randomness"))?;
        let sequence = self
            .state
            .outbound_high_watermark
            .checked_add(1)
            .ok_or_else(|| invalid("outbound sequence overflow"))?;
        // Persist before returning a signature. A crash may skip a sequence,
        // but it can never cause signer sequence reuse.
        self.state.outbound_high_watermark = sequence;
        self.persist_state()?;
        let mut message = CobaltShadowMessage {
            schema: COBALT_SHADOW_MESSAGE_SCHEMA.to_string(),
            message_id: String::new(),
            chain_id: self.state.identity.chain_id.clone(),
            genesis_hash: self.state.identity.genesis_hash.clone(),
            protocol_version: self.state.identity.protocol_version,
            sender: self.state.identity.node_id.clone(),
            sequence,
            round,
            kind,
            payload_hash,
            common_randomness_hash: randomness_hash,
            signature_hex: String::new(),
        };
        message.message_id = shadow_message_id(&message)?;
        message.signature_hex = self.sign_bytes(
            &shadow_message_signing_bytes(&message)?,
            MESSAGE_SIGNATURE_CONTEXT,
        )?;
        let encoded_len = serde_json::to_vec(&message).map_err(json_error)?.len() as u64;
        self.state.metrics.transmitted_messages =
            self.state.metrics.transmitted_messages.saturating_add(1);
        self.state.metrics.transmitted_bytes = self
            .state
            .metrics
            .transmitted_bytes
            .saturating_add(encoded_len);
        self.persist_state()?;
        Ok(message)
    }

    pub fn receive(
        &mut self,
        message: CobaltShadowMessage,
    ) -> io::Result<CobaltShadowReceiveOutcome> {
        match self.receive_inner(message) {
            Ok(outcome) => Ok(outcome),
            Err(error) => {
                self.state.rejected_messages = self.state.rejected_messages.saturating_add(1);
                self.persist_state()?;
                Err(error)
            }
        }
    }

    fn receive_inner(
        &mut self,
        message: CobaltShadowMessage,
    ) -> io::Result<CobaltShadowReceiveOutcome> {
        let encoded = serde_json::to_vec(&message).map_err(json_error)?;
        if encoded.len() > self.state.limits.max_message_bytes {
            return Err(invalid("message exceeds transport bound"));
        }
        validate_message_domain(&self.state.identity, &message)?;
        if shadow_message_id(&message)? != message.message_id {
            return Err(invalid("message id mismatch"));
        }
        verify_peer_signature(
            &self.state,
            &message.sender,
            &shadow_message_signing_bytes(&message)?,
            &message.signature_hex,
            MESSAGE_SIGNATURE_CONTEXT,
        )?;
        let known = self
            .state
            .seen_message_ids
            .iter()
            .any(|id| id == &message.message_id)
            || self
                .state
                .queued_messages
                .iter()
                .any(|queued| queued.message_id == message.message_id);
        if known {
            self.state.duplicate_messages = self.state.duplicate_messages.saturating_add(1);
            self.persist_state()?;
            return Ok(CobaltShadowReceiveOutcome::Duplicate);
        }
        let high_watermark = self
            .state
            .inbound_high_watermarks
            .get(&message.sender)
            .copied()
            .unwrap_or(0);
        if message.sequence <= high_watermark
            || self.state.queued_messages.iter().any(|queued| {
                queued.sender == message.sender && queued.sequence == message.sequence
            })
        {
            return Err(invalid("replay or equivocation sequence rejected"));
        }
        if !self.state.randomness_rounds.iter().any(|record| {
            record.round == message.round
                && record.randomness_hash == message.common_randomness_hash
        }) {
            return Err(invalid("message randomness binding mismatch"));
        }
        if self.state.queued_messages.len() >= self.state.limits.max_queue_messages {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "Cobalt shadow transport queue is full",
            ));
        }
        self.state.metrics.received_messages =
            self.state.metrics.received_messages.saturating_add(1);
        self.state.metrics.received_bytes = self
            .state
            .metrics
            .received_bytes
            .saturating_add(encoded.len() as u64);
        self.state.queued_messages.push(message);
        self.persist_state()?;
        Ok(CobaltShadowReceiveOutcome::Queued)
    }

    pub fn process_all(&mut self) -> io::Result<usize> {
        self.state.queued_messages.sort_by(|left, right| {
            (left.round, &left.sender, left.sequence, &left.message_id).cmp(&(
                right.round,
                &right.sender,
                right.sequence,
                &right.message_id,
            ))
        });
        let queued = std::mem::take(&mut self.state.queued_messages);
        let processed = queued.len();
        for message in queued {
            self.state
                .inbound_high_watermarks
                .insert(message.sender.clone(), message.sequence);
            self.state.seen_message_ids.push(message.message_id);
            self.state.accepted_messages = self.state.accepted_messages.saturating_add(1);
        }
        if self.state.seen_message_ids.len() > self.state.limits.max_seen_messages {
            let remove = self.state.seen_message_ids.len() - self.state.limits.max_seen_messages;
            self.state.seen_message_ids.drain(..remove);
        }
        self.state.seen_message_ids.sort();
        self.state.seen_message_ids.dedup();
        self.state.governance_digest = hash_serialized(
            "postfiat.cobalt.shadow.governance-digest.v1",
            &(
                &self.state.randomness_rounds,
                &self.state.seen_message_ids,
                &self.state.inbound_high_watermarks,
            ),
        )?;
        self.persist_state()?;
        Ok(processed)
    }

    fn sign_bytes(&self, bytes: &[u8], context: &[u8]) -> io::Result<String> {
        let private_key = hex_to_bytes(&self.private.private_key_hex).map_err(crypto_error)?;
        ml_dsa_65_sign_with_context(&private_key, bytes, context)
            .map(|signature| bytes_to_hex(&signature))
            .map_err(crypto_error)
    }

    fn sign_equivocation_for_drill(
        &self,
        original: &CobaltShadowMessage,
        payload_hash: String,
    ) -> io::Result<CobaltShadowMessage> {
        validate_hash("equivocation payload", &payload_hash)?;
        let mut message = original.clone();
        message.payload_hash = payload_hash;
        message.message_id = shadow_message_id(&message)?;
        message.signature_hex = self.sign_bytes(
            &shadow_message_signing_bytes(&message)?,
            MESSAGE_SIGNATURE_CONTEXT,
        )?;
        Ok(message)
    }

    fn persist_private(&self) -> io::Result<()> {
        let encoded = serde_json::to_vec_pretty(&self.private).map_err(json_error)?;
        if encoded.len() as u64 > MAX_PRIVATE_FILE_BYTES {
            return Err(invalid("private state exceeds file bound"));
        }
        atomic_write_private(&self.data_dir.join(PRIVATE_FILE), &encoded)
    }

    fn persist_state(&mut self) -> io::Result<()> {
        self.state.state_hash.clear();
        self.state.state_signature_hex.clear();
        let canonical = serde_json::to_vec(&self.state).map_err(json_error)?;
        self.state.state_hash = hash_hex("postfiat.cobalt.shadow.state.v1", &canonical);
        self.state.state_signature_hex =
            self.sign_bytes(self.state.state_hash.as_bytes(), STATE_SIGNATURE_CONTEXT)?;
        let encoded = serde_json::to_vec_pretty(&self.state).map_err(json_error)?;
        if encoded.len() as u64 > MAX_STATE_FILE_BYTES {
            return Err(invalid("state exceeds file bound"));
        }
        postfiat_storage::atomic_write(self.data_dir.join(STATE_FILE), encoded)
    }

    fn validate_loaded(&self) -> io::Result<()> {
        if self.private.schema != COBALT_SHADOW_PRIVATE_SCHEMA
            || self.state.schema != COBALT_SHADOW_STATE_SCHEMA
            || self.private.node_id != self.state.identity.node_id
            || self.private.algorithm_id != ML_DSA_65_ALGORITHM
            || self.state.signer_algorithm != ML_DSA_65_ALGORITHM
            || self.private.public_key_hex != self.state.public_key_hex
            || self.state.authority_mode != COBALT_SHADOW_AUTHORITY_MODE
            || self.state.live_authority
            || self.state.controls_block_consensus
        {
            return Err(invalid("persisted identity or scope mismatch"));
        }
        validate_identity(&self.state.identity)?;
        self.state.limits.validate()?;
        if self.state.peer_public_keys.len() > self.state.limits.max_peers
            || self.state.queued_messages.len() > self.state.limits.max_queue_messages
            || self.state.seen_message_ids.len() > self.state.limits.max_seen_messages
            || self.state.randomness_rounds.len() > self.state.limits.max_randomness_rounds
            || self.state.outbound_protocol_locks.len() > self.state.limits.max_randomness_rounds
            || self.state.ratification_locks.len() > self.state.limits.max_randomness_rounds
            || self.state.protocol_decisions.len() > self.state.limits.max_randomness_rounds
        {
            return Err(invalid("persisted collection exceeds bound"));
        }
        if self.state.registry_root.is_empty() != self.state.trust_graph_root.is_empty() {
            return Err(invalid("persisted registry binding is incomplete"));
        }
        if !self.state.registry_root.is_empty() {
            validate_hash("persisted registry root", &self.state.registry_root)?;
            validate_hash("persisted trust graph root", &self.state.trust_graph_root)?;
        }
        if self
            .state
            .protocol_decisions
            .iter()
            .any(|(round, decision)| {
                self.state.ratification_locks.get(round) != Some(&decision.ratification_id)
                    || decision.round != *round
            })
        {
            return Err(invalid("persisted protocol decision lock mismatch"));
        }
        let public_key = hex_to_bytes(&self.state.public_key_hex).map_err(crypto_error)?;
        ml_dsa_65_validate_public_key(&public_key).map_err(crypto_error)?;
        let private_key = hex_to_bytes(&self.private.private_key_hex).map_err(crypto_error)?;
        let proof = ml_dsa_65_sign_with_context(
            &private_key,
            b"postfiat-cobalt-shadow-key-pair-check",
            STATE_SIGNATURE_CONTEXT,
        )
        .map_err(crypto_error)?;
        if !ml_dsa_65_verify_with_context(
            &public_key,
            b"postfiat-cobalt-shadow-key-pair-check",
            &proof,
            STATE_SIGNATURE_CONTEXT,
        ) {
            return Err(invalid("signer key pair mismatch"));
        }
        let mut unsigned = self.state.clone();
        let expected_hash = unsigned.state_hash.clone();
        let signature = hex_to_bytes(&unsigned.state_signature_hex).map_err(crypto_error)?;
        unsigned.state_hash.clear();
        unsigned.state_signature_hex.clear();
        let canonical = serde_json::to_vec(&unsigned).map_err(json_error)?;
        if hash_hex("postfiat.cobalt.shadow.state.v1", &canonical) != expected_hash
            || !ml_dsa_65_verify_with_context(
                &public_key,
                expected_hash.as_bytes(),
                &signature,
                STATE_SIGNATURE_CONTEXT,
            )
        {
            return Err(invalid("state signature verification failed"));
        }
        Ok(())
    }
}

pub fn build_registry_binding_manifest(
    registry_root: impl Into<String>,
    validator_registry: ValidatorRegistry,
    mut validator_bindings: Vec<CobaltShadowValidatorBinding>,
    quorum: usize,
    activation_height: u64,
) -> io::Result<CobaltShadowRegistryBinding> {
    let registry_root = registry_root.into();
    validate_hash("registry root", &registry_root)?;
    if validator_bindings.len() < 3 {
        return Err(invalid(
            "Cobalt registry binding requires at least three validators",
        ));
    }
    validator_bindings.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    if validator_bindings
        .windows(2)
        .any(|pair| pair[0].node_id == pair[1].node_id)
    {
        return Err(invalid(
            "Cobalt registry binding contains duplicate validators",
        ));
    }
    let active_validators = validator_bindings
        .iter()
        .map(|binding| binding.node_id.clone())
        .collect::<Vec<_>>();
    if quorum == 0 || quorum > active_validators.len() {
        return Err(invalid("Cobalt registry binding quorum is invalid"));
    }
    let computed_root = validator_registry_root(&validator_registry, &active_validators)?;
    if computed_root != registry_root {
        return Err(invalid(
            "Cobalt registry binding does not match the live registry root",
        ));
    }
    let first = validator_bindings
        .first()
        .ok_or_else(|| invalid("Cobalt registry binding is empty"))?;
    let domain = CobaltDomain {
        chain_id: first.chain_id.clone(),
        genesis_hash: first.genesis_hash.clone(),
        protocol_version: first.protocol_version,
    };
    let mut peers = BTreeMap::new();
    for binding in &validator_bindings {
        validate_validator_binding(binding)?;
        if binding.chain_id != domain.chain_id
            || binding.genesis_hash != domain.genesis_hash
            || binding.protocol_version != domain.protocol_version
            || binding.registry_root != registry_root
        {
            return Err(invalid("Cobalt validator binding domain mismatch"));
        }
        let registry_record = validator_registry_record(&validator_registry, &binding.node_id)?;
        if registry_record.algorithm_id != binding.validator_algorithm
            || registry_record.public_key_hex != binding.validator_public_key_hex
        {
            return Err(invalid(
                "Cobalt validator binding is not anchored in the live registry",
            ));
        }
        peers.insert(
            binding.node_id.clone(),
            binding.cobalt_public_key_hex.clone(),
        );
    }
    let trust_graph = build_canonical_unl_trust_graph(
        &domain,
        1,
        registry_root.clone(),
        activation_height,
        None,
        active_validators.clone(),
        quorum,
    )
    .map_err(consensus_error)?;
    Ok(CobaltShadowRegistryBinding {
        schema: COBALT_SHADOW_REGISTRY_BINDING_SCHEMA.to_string(),
        registry_root,
        active_validators,
        validator_registry,
        trust_graph,
        peers,
        validator_bindings,
    })
}

pub fn assemble_protocol_transcript(
    binding: &CobaltShadowRegistryBinding,
    propose: RbcPropose,
    mut contributions: Vec<CobaltShadowProtocolContribution>,
) -> io::Result<CobaltShadowProtocolTranscript> {
    if propose.trust_graph_root != binding.trust_graph.trust_graph_root
        || propose.chain_id != binding.trust_graph.chain_id
        || propose.genesis_hash != binding.trust_graph.genesis_hash
        || propose.protocol_version != binding.trust_graph.protocol_version
        || !binding.active_validators.contains(&propose.sender)
    {
        return Err(invalid("protocol proposal does not match registry binding"));
    }
    contributions.sort_by(|left, right| left.node_id.cmp(&right.node_id));
    let contributor_ids = contributions
        .iter()
        .map(|contribution| contribution.node_id.clone())
        .collect::<Vec<_>>();
    if contributor_ids != binding.active_validators {
        return Err(invalid(
            "protocol contributions do not match active validators",
        ));
    }
    let agreement_id = hash_serialized(
        "postfiat.cobalt.shadow.agreement.v1",
        &(
            propose.amendment_slot,
            &propose.message_id,
            &binding.trust_graph.trust_graph_root,
        ),
    )?;
    for contribution in &contributions {
        if contribution.schema != COBALT_SHADOW_PROTOCOL_CONTRIBUTION_SCHEMA
            || contribution.registry_root != binding.registry_root
            || contribution.trust_graph_root != binding.trust_graph.trust_graph_root
            || contribution.round != propose.amendment_slot
            || contribution.payload_hash != propose.payload_hash
            || contribution.agreement_id != agreement_id
            || contribution.rbc_echo.sender != contribution.node_id
            || contribution.rbc_ready.sender != contribution.node_id
            || contribution.rbc_accept.sender != contribution.node_id
            || contribution.abba_init.sender != contribution.node_id
            || contribution.abba_aux.sender != contribution.node_id
            || contribution.abba_conf.sender != contribution.node_id
            || contribution.abba_finish.sender != contribution.node_id
            || contribution.full_knowledge_check.sender != contribution.node_id
        {
            return Err(invalid("protocol contribution binding mismatch"));
        }
    }
    let domain = CobaltDomain {
        chain_id: binding.trust_graph.chain_id.clone(),
        genesis_hash: binding.trust_graph.genesis_hash.clone(),
        protocol_version: binding.trust_graph.protocol_version,
    };
    let candidate = mvba_candidate_from_rbc_accept(&domain, &propose, &contributions[0].rbc_accept)
        .map_err(consensus_error)?;
    let mvba_input = build_mvba_valid_input_set(
        &domain,
        binding
            .trust_graph
            .trust_views
            .first()
            .ok_or_else(|| invalid("trust graph has no views"))?,
        agreement_id.clone(),
        vec![candidate],
    )
    .map_err(consensus_error)?;
    let activation_height = propose
        .amendment_slot
        .checked_add(1)
        .ok_or_else(|| invalid("protocol activation height overflow"))?;
    let ratification = ratify_dabc_amendment(
        &domain,
        &binding.trust_graph,
        &mvba_input,
        None,
        activation_height,
    )
    .map_err(consensus_error)?;
    Ok(CobaltShadowProtocolTranscript {
        schema: COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA.to_string(),
        registry_root: binding.registry_root.clone(),
        trust_graph: binding.trust_graph.clone(),
        round: propose.amendment_slot,
        payload_hash: propose.payload_hash.clone(),
        agreement_id,
        rbc_propose: propose,
        rbc_echoes: contributions
            .iter()
            .map(|contribution| contribution.rbc_echo.clone())
            .collect(),
        rbc_readies: contributions
            .iter()
            .map(|contribution| contribution.rbc_ready.clone())
            .collect(),
        rbc_accepts: contributions
            .iter()
            .map(|contribution| contribution.rbc_accept.clone())
            .collect(),
        abba_inits: contributions
            .iter()
            .map(|contribution| contribution.abba_init.clone())
            .collect(),
        abba_auxes: contributions
            .iter()
            .map(|contribution| contribution.abba_aux.clone())
            .collect(),
        abba_confs: contributions
            .iter()
            .map(|contribution| contribution.abba_conf.clone())
            .collect(),
        abba_finishes: contributions
            .iter()
            .map(|contribution| contribution.abba_finish.clone())
            .collect(),
        mvba_input,
        ratification,
        full_knowledge_checks: contributions
            .into_iter()
            .map(|contribution| contribution.full_knowledge_check)
            .collect(),
    })
}

pub fn build_signed_protocol_transcript(
    services: &mut [CobaltShadowService],
    round: u64,
    payload_hash: impl Into<String>,
) -> io::Result<CobaltShadowProtocolTranscript> {
    if services.len() < 3 {
        return Err(invalid(
            "signed protocol transcript requires at least three validators",
        ));
    }
    let payload_hash = payload_hash.into();
    validate_hash("protocol payload", &payload_hash)?;
    let identity = services[0].state.identity.clone();
    if services.iter().any(|service| {
        service.state.identity.chain_id != identity.chain_id
            || service.state.identity.genesis_hash != identity.genesis_hash
            || service.state.identity.protocol_version != identity.protocol_version
    }) {
        return Err(invalid("protocol fleet domain mismatch"));
    }
    let peers = services
        .iter()
        .map(|service| {
            (
                service.state.identity.node_id.clone(),
                service.state.public_key_hex.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    if peers.len() != services.len() {
        return Err(invalid("protocol fleet contains duplicate validator ids"));
    }
    let registry_root = hash_serialized("postfiat.cobalt.shadow.validator-registry.v1", &peers)?;
    let domain = CobaltDomain {
        chain_id: identity.chain_id,
        genesis_hash: identity.genesis_hash,
        protocol_version: identity.protocol_version,
    };
    let validators = peers.keys().cloned().collect::<Vec<_>>();
    let subset = build_essential_subset(
        &domain,
        validators.clone(),
        0,
        validators.len(),
        vec!["local-socket-drill".to_string()],
        1,
        None,
    )
    .map_err(consensus_error)?;
    let views = validators
        .iter()
        .map(|validator| build_trust_view(&domain, validator, 1, vec![subset.clone()], ""))
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let graph = build_trust_graph(&domain, 1, registry_root.clone(), 1, None, views)
        .map_err(consensus_error)?;
    for service in services.iter_mut() {
        service.bind_registry(registry_root.clone(), &graph, peers.clone())?;
        service.reserve_protocol_round(round, payload_hash.clone())?;
    }
    let private_keys = services
        .iter()
        .map(CobaltShadowService::protocol_private_key)
        .collect::<io::Result<Vec<_>>>()?;
    let proposer = validators[0].clone();
    let propose = sign_rbc_propose(
        &domain,
        graph.trust_graph_root.clone(),
        proposer,
        round,
        payload_hash.clone(),
        &private_keys[0],
    )
    .map_err(consensus_error)?;
    let rbc_echoes = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| sign_rbc_echo(&domain, &propose, validator, key))
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let rbc_readies = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| sign_rbc_ready(&domain, &propose, validator, key))
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let rbc_accepts = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| sign_rbc_accept(&domain, &propose, validator, key))
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let agreement_id = hash_serialized(
        "postfiat.cobalt.shadow.agreement.v1",
        &(round, &propose.message_id, &graph.trust_graph_root),
    )?;
    let abba_inits = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| {
            sign_abba_init(
                &domain,
                graph.trust_graph_root.clone(),
                validator,
                agreement_id.clone(),
                round,
                true,
                key,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let abba_auxes = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| {
            sign_abba_aux(
                &domain,
                graph.trust_graph_root.clone(),
                validator,
                agreement_id.clone(),
                round,
                true,
                key,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let abba_confs = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| {
            sign_abba_conf(
                &domain,
                graph.trust_graph_root.clone(),
                validator,
                agreement_id.clone(),
                round,
                true,
                key,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let abba_finishes = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| {
            sign_abba_finish(
                &domain,
                graph.trust_graph_root.clone(),
                validator,
                agreement_id.clone(),
                round,
                true,
                key,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    let candidate = mvba_candidate_from_rbc_accept(&domain, &propose, &rbc_accepts[0])
        .map_err(consensus_error)?;
    let mvba_input = build_mvba_valid_input_set(
        &domain,
        &graph.trust_views[0],
        agreement_id.clone(),
        vec![candidate],
    )
    .map_err(consensus_error)?;
    let activation_height = round
        .checked_add(1)
        .ok_or_else(|| invalid("protocol activation height overflow"))?;
    let ratification = ratify_dabc_amendment(&domain, &graph, &mvba_input, None, activation_height)
        .map_err(consensus_error)?;
    let pending = vec![DabcPendingPair {
        amendment_slot: round,
        output_candidate_id: mvba_input.output_candidate_id.clone(),
    }];
    let full_knowledge_checks = validators
        .iter()
        .zip(&private_keys)
        .map(|(validator, key)| {
            sign_dabc_full_knowledge_check(
                &domain,
                graph.trust_graph_root.clone(),
                validator,
                activation_height,
                pending.clone(),
                key,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(consensus_error)?;
    Ok(CobaltShadowProtocolTranscript {
        schema: COBALT_SHADOW_PROTOCOL_TRANSCRIPT_SCHEMA.to_string(),
        registry_root,
        trust_graph: graph,
        round,
        payload_hash,
        agreement_id,
        rbc_propose: propose,
        rbc_echoes,
        rbc_readies,
        rbc_accepts,
        abba_inits,
        abba_auxes,
        abba_confs,
        abba_finishes,
        mvba_input,
        ratification,
        full_knowledge_checks,
    })
}

pub fn run_cobalt_shadow_adversarial_drill(
    root: impl Into<PathBuf>,
) -> io::Result<CobaltShadowDrillReport> {
    let root = root.into();
    fs::create_dir_all(&root)?;
    let identity = |node_id: &str| CobaltShadowIdentity {
        node_id: node_id.to_string(),
        chain_id: "postfiat-cobalt-shadow-drill".to_string(),
        genesis_hash: "01".repeat(48),
        protocol_version: 1,
    };
    let mut fleet = (0..4)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let peers = fleet
        .iter()
        .map(|service| {
            (
                service.state.identity.node_id.clone(),
                service.state.public_key_hex.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for service in &mut fleet {
        service.replace_peer_registry(peers.clone())?;
    }

    let round = 1;
    let participants = peers.keys().cloned().collect::<Vec<_>>();
    let commitments = fleet
        .iter_mut()
        .take(3)
        .map(|service| service.create_beacon_commitment(round))
        .collect::<io::Result<Vec<_>>>()?;
    let reveals = fleet
        .iter_mut()
        .take(3)
        .map(|service| service.create_beacon_reveal(round))
        .collect::<io::Result<Vec<_>>>()?;
    let randomness_failure_fails_closed = fleet[3]
        .install_common_randomness(
            round,
            participants.clone(),
            3,
            commitments[..2].to_vec(),
            reveals[..2].to_vec(),
        )
        .is_err();
    let records = fleet
        .iter_mut()
        .map(|service| {
            service.install_common_randomness(
                round,
                participants.clone(),
                3,
                commitments.clone(),
                reveals.clone(),
            )
        })
        .collect::<io::Result<Vec<_>>>()?;
    let randomness_set = records
        .iter()
        .map(|record| record.randomness_hash.clone())
        .collect::<BTreeSet<_>>();
    let common_randomness_hash = records[0].randomness_hash.clone();

    let messages = vec![
        fleet[0].sign_message(
            round,
            CobaltShadowMessageKind::Rbc,
            hash_hex("postfiat.cobalt.shadow.drill.payload.v1", b"rbc"),
        )?,
        fleet[1].sign_message(
            round,
            CobaltShadowMessageKind::Abba,
            hash_hex("postfiat.cobalt.shadow.drill.payload.v1", b"abba"),
        )?,
        fleet[2].sign_message(
            round,
            CobaltShadowMessageKind::Dabc,
            hash_hex("postfiat.cobalt.shadow.drill.payload.v1", b"dabc"),
        )?,
    ];

    // Partition: 0/1 exchange only with each other; validator 2 sees only its
    // own message; validator 3 is the missing member. Validator 1 then restarts
    // with a queued message to exercise durable recovery.
    fleet[0].receive(messages[0].clone())?;
    fleet[0].receive(messages[1].clone())?;
    fleet[1].receive(messages[0].clone())?;
    fleet[1].receive(messages[1].clone())?;
    fleet[2].receive(messages[2].clone())?;
    let queued_before_restart = fleet[1].state.queued_messages.len();
    let restarted = CobaltShadowService::open(root.join("validator-1"))?;
    let restart_recovered_queue =
        queued_before_restart == 2 && restarted.state.queued_messages.len() == 2;
    fleet[1] = restarted;

    // Healing delivers the exact same authenticated set to every node. Exact
    // redelivery is idempotent; the third message was censored until healing.
    let mut duplicate_delivery_idempotent = true;
    for service in &mut fleet {
        for message in &messages {
            match service.receive(message.clone())? {
                CobaltShadowReceiveOutcome::Queued => {}
                CobaltShadowReceiveOutcome::Duplicate => {
                    duplicate_delivery_idempotent &= true;
                }
            }
        }
        service.process_all()?;
        duplicate_delivery_idempotent &= matches!(
            service.receive(messages[0].clone())?,
            CobaltShadowReceiveOutcome::Duplicate
        );
    }

    let equivocation = fleet[0].sign_equivocation_for_drill(
        &messages[0],
        hash_hex("postfiat.cobalt.shadow.drill.payload.v1", b"equivocation"),
    )?;
    let equivocation_rejected = fleet[2]
        .receive(equivocation)
        .expect_err("drill equivocation must fail")
        .to_string()
        .contains("replay or equivocation");
    let mut bad_signature = messages[1].clone();
    let replacement = if bad_signature.signature_hex.starts_with("00") {
        "01"
    } else {
        "00"
    };
    bad_signature.signature_hex.replace_range(0..2, replacement);
    let bad_signature_rejected = fleet[2]
        .receive(bad_signature)
        .expect_err("drill bad signature must fail")
        .to_string()
        .contains("signature verification");

    let digests = fleet
        .iter()
        .map(|service| service.state.governance_digest.clone())
        .collect::<BTreeSet<_>>();
    let converged_governance_digest = fleet[0].state.governance_digest.clone();
    let nodes = fleet
        .iter()
        .map(CobaltShadowService::status)
        .collect::<Vec<_>>();
    let all_nodes_converged =
        digests.len() == 1 && nodes.iter().all(|status| status.accepted_messages == 3);
    let checks = CobaltShadowDrillChecks {
        bounded_transport: nodes.iter().all(|status| status.transport_healthy),
        production_randomness: randomness_set.len() == 1
            && records.iter().all(|record| {
                record.source == COBALT_SHADOW_RANDOMNESS_SOURCE && record.contributors.len() == 3
            }),
        randomness_failure_fails_closed,
        restart_recovered_queue,
        duplicate_delivery_idempotent,
        equivocation_rejected,
        bad_signature_rejected,
        partition_healed: all_nodes_converged,
        censorship_healed: all_nodes_converged,
        member_loss_converged: all_nodes_converged && records[0].contributors.len() == 3,
        all_nodes_converged,
        live_authority_remained_disabled: nodes.iter().all(|status| {
            !status.live_authority
                && !status.controls_block_consensus
                && status.authority_mode == COBALT_SHADOW_AUTHORITY_MODE
        }),
    };
    let ok = [
        checks.bounded_transport,
        checks.production_randomness,
        checks.randomness_failure_fails_closed,
        checks.restart_recovered_queue,
        checks.duplicate_delivery_idempotent,
        checks.equivocation_rejected,
        checks.bad_signature_rejected,
        checks.partition_healed,
        checks.censorship_healed,
        checks.member_loss_converged,
        checks.all_nodes_converged,
        checks.live_authority_remained_disabled,
    ]
    .into_iter()
    .all(|check| check);
    Ok(CobaltShadowDrillReport {
        schema: "postfiat-cobalt-shadow-adversarial-drill-v1".to_string(),
        status: if ok { "passed" } else { "failed" }.to_string(),
        ok,
        scope: "governance-only-shadow".to_string(),
        validator_count: 4,
        active_contributor_count: 3,
        common_randomness_hash,
        converged_governance_digest,
        checks,
        nodes,
    })
}

fn validator_binding_statement_hash(binding: &CobaltShadowValidatorBinding) -> io::Result<String> {
    hash_serialized(
        "postfiat.cobalt.shadow.validator-binding.v1",
        &(
            binding.schema.as_str(),
            binding.node_id.as_str(),
            binding.chain_id.as_str(),
            binding.genesis_hash.as_str(),
            binding.protocol_version,
            binding.registry_root.as_str(),
            binding.cobalt_public_key_hex.as_str(),
            binding.validator_algorithm.as_str(),
            binding.validator_public_key_hex.as_str(),
        ),
    )
}

fn validate_validator_binding(binding: &CobaltShadowValidatorBinding) -> io::Result<()> {
    if binding.schema != COBALT_SHADOW_VALIDATOR_BINDING_SCHEMA {
        return Err(invalid(
            "unsupported Cobalt shadow validator binding schema",
        ));
    }
    validate_node_id(&binding.node_id)?;
    validate_identity(&CobaltShadowIdentity {
        node_id: binding.node_id.clone(),
        chain_id: binding.chain_id.clone(),
        genesis_hash: binding.genesis_hash.clone(),
        protocol_version: binding.protocol_version,
    })?;
    validate_hash("validator binding registry root", &binding.registry_root)?;
    validate_hash("validator binding statement", &binding.statement_hash)?;
    if binding.validator_algorithm != ML_DSA_65_ALGORITHM {
        return Err(invalid("unsupported validator binding algorithm"));
    }
    let cobalt_public_key = hex_to_bytes(&binding.cobalt_public_key_hex).map_err(crypto_error)?;
    ml_dsa_65_validate_public_key(&cobalt_public_key).map_err(crypto_error)?;
    let validator_public_key =
        hex_to_bytes(&binding.validator_public_key_hex).map_err(crypto_error)?;
    ml_dsa_65_validate_public_key(&validator_public_key).map_err(crypto_error)?;
    let expected_hash = validator_binding_statement_hash(binding)?;
    if expected_hash != binding.statement_hash {
        return Err(invalid("validator binding statement hash mismatch"));
    }
    let signature = hex_to_bytes(&binding.signature_hex).map_err(crypto_error)?;
    if !ml_dsa_65_verify_with_context(
        &validator_public_key,
        binding.statement_hash.as_bytes(),
        &signature,
        VALIDATOR_BINDING_SIGNATURE_CONTEXT,
    ) {
        return Err(invalid("validator binding signature verification failed"));
    }
    Ok(())
}

fn validate_identity(identity: &CobaltShadowIdentity) -> io::Result<()> {
    validate_node_id(&identity.node_id)?;
    if identity.chain_id.is_empty() || identity.chain_id.len() > 128 {
        return Err(invalid("chain id is empty or oversized"));
    }
    validate_hash("genesis hash", &identity.genesis_hash)?;
    if identity.protocol_version == 0 {
        return Err(invalid("protocol version must be nonzero"));
    }
    Ok(())
}

fn validate_node_id(node_id: &str) -> io::Result<()> {
    if node_id.is_empty()
        || node_id.len() > 128
        || !node_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(invalid("node id is malformed"));
    }
    Ok(())
}

fn validate_hash(label: &str, hash: &str) -> io::Result<()> {
    if hash.len() != 96 || !hash.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid(format!("{label} must be a 48-byte hex digest")));
    }
    Ok(())
}

fn validate_message_domain(
    identity: &CobaltShadowIdentity,
    message: &CobaltShadowMessage,
) -> io::Result<()> {
    if message.schema != COBALT_SHADOW_MESSAGE_SCHEMA
        || message.chain_id != identity.chain_id
        || message.genesis_hash != identity.genesis_hash
        || message.protocol_version != identity.protocol_version
        || message.sequence == 0
    {
        return Err(invalid("message domain mismatch"));
    }
    validate_node_id(&message.sender)?;
    validate_hash("message id", &message.message_id)?;
    validate_hash("payload", &message.payload_hash)?;
    validate_hash("common randomness", &message.common_randomness_hash)
}

fn shadow_message_id(message: &CobaltShadowMessage) -> io::Result<String> {
    hash_serialized(
        "postfiat.cobalt.shadow.message-id.v1",
        &(
            &message.schema,
            &message.chain_id,
            &message.genesis_hash,
            message.protocol_version,
            &message.sender,
            message.sequence,
            message.round,
            message.kind,
            &message.payload_hash,
            &message.common_randomness_hash,
        ),
    )
}

fn shadow_message_signing_bytes(message: &CobaltShadowMessage) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        &message.schema,
        &message.message_id,
        &message.chain_id,
        &message.genesis_hash,
        message.protocol_version,
        &message.sender,
        message.sequence,
        message.round,
        message.kind,
        &message.payload_hash,
        &message.common_randomness_hash,
    ))
    .map_err(json_error)
}

fn beacon_commitment_hash(
    identity: &CobaltShadowIdentity,
    sender: &str,
    round: u64,
    entropy_hex: &str,
) -> io::Result<String> {
    if hex_to_bytes(entropy_hex).map_err(crypto_error)?.len() != ENTROPY_BYTES {
        return Err(invalid("beacon entropy must be 32 bytes"));
    }
    hash_serialized(
        "postfiat.cobalt.shadow.beacon.commitment.v1",
        &(
            &identity.chain_id,
            &identity.genesis_hash,
            identity.protocol_version,
            sender,
            round,
            entropy_hex,
        ),
    )
}

fn beacon_commitment_signing_bytes(
    identity: &CobaltShadowIdentity,
    commitment: &CobaltShadowBeaconCommitment,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        &commitment.schema,
        &identity.chain_id,
        &identity.genesis_hash,
        identity.protocol_version,
        &commitment.sender,
        commitment.round,
        &commitment.commitment_hash,
    ))
    .map_err(json_error)
}

fn beacon_reveal_signing_bytes(
    identity: &CobaltShadowIdentity,
    reveal: &CobaltShadowBeaconReveal,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        &reveal.schema,
        &identity.chain_id,
        &identity.genesis_hash,
        identity.protocol_version,
        &reveal.sender,
        reveal.round,
        &reveal.commitment_hash,
        &reveal.entropy_hex,
    ))
    .map_err(json_error)
}

fn verify_peer_signature(
    state: &CobaltShadowState,
    sender: &str,
    payload: &[u8],
    signature_hex: &str,
    context: &[u8],
) -> io::Result<()> {
    let public_key_hex = state
        .peer_public_keys
        .get(sender)
        .ok_or_else(|| invalid("sender is not in peer registry"))?;
    let public_key = hex_to_bytes(public_key_hex).map_err(crypto_error)?;
    let signature = hex_to_bytes(signature_hex).map_err(crypto_error)?;
    if signature.len() != ML_DSA_65_SIGNATURE_BYTES
        || !ml_dsa_65_verify_with_context(&public_key, payload, &signature, context)
    {
        return Err(invalid("message signature verification failed"));
    }
    Ok(())
}

fn sorted_unique(mut values: Vec<String>, label: &str) -> io::Result<Vec<String>> {
    for value in &values {
        validate_node_id(value)?;
    }
    values.sort();
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(invalid(format!("duplicate {label}")));
    }
    Ok(values)
}

fn hash_serialized<T: Serialize>(domain: &str, value: &T) -> io::Result<String> {
    serde_json::to_vec(value)
        .map(|encoded| hash_hex(domain, &encoded))
        .map_err(json_error)
}

fn os_random_bytes<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0u8; N];
    File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn atomic_write_private(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| invalid("private path has no parent"))?;
    fs::create_dir_all(parent)?;
    let suffix = bytes_to_hex(&os_random_bytes::<8>()?);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| invalid("private path is invalid"))?;
    let temporary = parent.join(format!(".{file_name}.{suffix}.tmp"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temporary)?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    File::open(parent)?.sync_all()
}

fn validate_private_file_permissions(path: &Path) -> io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_file() {
        return Err(invalid("private key path is not a regular file"));
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private key file must not be accessible by group or other",
        ));
    }
    Ok(())
}

fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &Path, max: u64) -> io::Result<T> {
    let metadata = fs::metadata(path)?;
    if metadata.len() > max {
        return Err(invalid("state file exceeds read bound"));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    File::open(path)?.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(invalid("state file exceeds read bound"));
    }
    serde_json::from_slice(&bytes).map_err(json_error)
}

fn json_error(error: serde_json::Error) -> io::Error {
    invalid(error.to_string())
}

fn crypto_error(error: impl std::fmt::Display) -> io::Error {
    invalid(error.to_string())
}

fn consensus_error(error: impl std::fmt::Display) -> io::Error {
    invalid(format!("Cobalt protocol validation failed: {error}"))
}

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("postfiat-cobalt-shadow-{label}-{nonce}"))
    }

    fn identity(node_id: &str) -> CobaltShadowIdentity {
        CobaltShadowIdentity {
            node_id: node_id.to_string(),
            chain_id: "postfiat-shadow-test".to_string(),
            genesis_hash: "01".repeat(48),
            protocol_version: 1,
        }
    }

    fn two_node_fleet(root: &Path, limits: CobaltShadowLimits) -> Vec<CobaltShadowService> {
        let mut fleet = (0..2)
            .map(|index| {
                CobaltShadowService::initialize(
                    root.join(format!("validator-{index}")),
                    identity(&format!("validator-{index}")),
                    limits.clone(),
                )
                .expect("initialize")
            })
            .collect::<Vec<_>>();
        let peers = fleet
            .iter()
            .map(|service| {
                (
                    service.state.identity.node_id.clone(),
                    service.state.public_key_hex.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        for service in &mut fleet {
            service
                .replace_peer_registry(peers.clone())
                .expect("replace peers");
        }
        let participants = peers.keys().cloned().collect::<Vec<_>>();
        let commitments = fleet
            .iter_mut()
            .map(|service| service.create_beacon_commitment(1).expect("commit"))
            .collect::<Vec<_>>();
        let reveals = fleet
            .iter_mut()
            .map(|service| service.create_beacon_reveal(1).expect("reveal"))
            .collect::<Vec<_>>();
        for service in &mut fleet {
            service
                .install_common_randomness(
                    1,
                    participants.clone(),
                    2,
                    commitments.clone(),
                    reveals.clone(),
                )
                .expect("install randomness");
        }
        fleet
    }

    #[test]
    fn adversarial_drill_converges_without_live_authority() {
        let root = test_dir("drill");
        let report = run_cobalt_shadow_adversarial_drill(&root).expect("drill");
        assert!(report.ok, "{report:#?}");
        assert!(report.checks.restart_recovered_queue);
        assert!(report.checks.partition_healed);
        assert!(report.checks.censorship_healed);
        assert!(report.checks.member_loss_converged);
        assert!(report.checks.equivocation_rejected);
        assert!(report.checks.bad_signature_rejected);
        assert!(report.checks.randomness_failure_fails_closed);
        assert!(report.checks.live_authority_remained_disabled);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn queue_bound_is_enforced_before_durable_acceptance() {
        let root = test_dir("queue-bound");
        let limits = CobaltShadowLimits {
            max_queue_messages: 1,
            max_seen_messages: 1,
            ..CobaltShadowLimits::default()
        };
        let mut fleet = two_node_fleet(&root, limits);
        let first = fleet[0]
            .sign_message(
                1,
                CobaltShadowMessageKind::Rbc,
                hash_hex("test.payload", b"first"),
            )
            .expect("first");
        let second = fleet[0]
            .sign_message(
                1,
                CobaltShadowMessageKind::Abba,
                hash_hex("test.payload", b"second"),
            )
            .expect("second");
        fleet[1].receive(first).expect("queue first");
        let error = fleet[1].receive(second).expect_err("queue must be full");
        assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
        assert_eq!(fleet[1].state.queued_messages.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn restart_verifies_state_signature_and_private_permissions() {
        let root = test_dir("restart");
        let fleet = two_node_fleet(&root, CobaltShadowLimits::default());
        let before = fleet[0].status();
        drop(fleet);
        let reopened =
            CobaltShadowService::open(root.join("validator-0")).expect("verified restart");
        assert_eq!(reopened.status().boot_count, before.boot_count + 1);
        assert!(!reopened.status().live_authority);
        #[cfg(unix)]
        {
            let mode = fs::metadata(root.join("validator-0").join(PRIVATE_FILE))
                .expect("private metadata")
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0);
        }
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn live_registry_binding_requires_validator_signed_sidecar_keys() {
        let root = test_dir("live-registry-binding");
        let mut fleet = (0..4)
            .map(|index| {
                CobaltShadowService::initialize(
                    root.join(format!("validator-{index}")),
                    identity(&format!("validator-{index}")),
                    CobaltShadowLimits::default(),
                )
                .expect("initialize")
            })
            .collect::<Vec<_>>();
        let key_records = (0..4)
            .map(|index| {
                crate::create_validator_key_record(format!("validator-{index}"))
                    .expect("validator key")
            })
            .collect::<Vec<_>>();
        let registry = ValidatorRegistry {
            validators: key_records
                .iter()
                .map(|record| crate::ValidatorRegistryRecord {
                    node_id: record.node_id.clone(),
                    algorithm_id: record.algorithm_id.clone(),
                    public_key_hex: record.public_key_hex.clone(),
                })
                .collect(),
        };
        let active_validators = key_records
            .iter()
            .map(|record| record.node_id.clone())
            .collect::<Vec<_>>();
        let registry_root =
            validator_registry_root(&registry, &active_validators).expect("registry root");
        let bindings = fleet
            .iter()
            .zip(&key_records)
            .map(|(service, record)| {
                let key_path = root.join(format!("{}.validator-keys.json", record.node_id));
                crate::write_validator_key_file(
                    &key_path,
                    &crate::ValidatorKeyFile {
                        validators: vec![record.clone()],
                    },
                )
                .expect("write validator key");
                service
                    .create_validator_binding(registry_root.clone(), &key_path)
                    .expect("create validator binding")
            })
            .collect::<Vec<_>>();
        let manifest = build_registry_binding_manifest(
            registry_root.clone(),
            registry,
            bindings.clone(),
            3,
            911,
        )
        .expect("build registry binding");
        for service in &mut fleet {
            service
                .bind_registry_manifest(&manifest)
                .expect("bind live registry");
            assert_eq!(service.status().registry_root, registry_root);
            assert_eq!(service.status().peer_count, 4);
        }
        let payload_hash = hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"distributed");
        let proposal = fleet[0]
            .create_protocol_proposal(&manifest, 912, payload_hash)
            .expect("create proposal");
        let contributions = fleet
            .iter_mut()
            .map(|service| {
                service
                    .create_protocol_contribution(&manifest, &proposal)
                    .expect("create contribution")
            })
            .collect::<Vec<_>>();
        assert!(assemble_protocol_transcript(
            &manifest,
            proposal.clone(),
            contributions[..2].to_vec(),
        )
        .is_err());
        let transcript = assemble_protocol_transcript(&manifest, proposal, contributions)
            .expect("assemble transcript");
        let decisions = fleet
            .iter_mut()
            .map(|service| {
                service
                    .commit_protocol_transcript(&transcript)
                    .expect("commit distributed transcript")
            })
            .collect::<Vec<_>>();
        assert!(decisions.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(decisions[0].signed_message_count, 33);
        let mut tampered = manifest;
        tampered.validator_bindings[0].cobalt_public_key_hex = "00".repeat(1952);
        assert!(fleet[0].bind_registry_manifest(&tampered).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn signed_protocol_transcript_converges_and_replays_after_restart() {
        let root = test_dir("signed-protocol");
        let mut fleet = (0..3)
            .map(|index| {
                CobaltShadowService::initialize(
                    root.join(format!("validator-{index}")),
                    identity(&format!("validator-{index}")),
                    CobaltShadowLimits::default(),
                )
                .expect("initialize")
            })
            .collect::<Vec<_>>();
        let transcript = build_signed_protocol_transcript(
            &mut fleet,
            7,
            hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"ratify"),
        )
        .expect("build signed transcript");
        let decisions = fleet
            .iter_mut()
            .map(|service| {
                service
                    .commit_protocol_transcript(&transcript)
                    .expect("commit transcript")
            })
            .collect::<Vec<_>>();
        assert!(decisions.windows(2).all(|pair| pair[0] == pair[1]));
        assert_eq!(decisions[0].signed_message_count, 25);
        assert!(fleet.iter().all(|service| {
            !service.status().live_authority
                && !service.status().controls_block_consensus
                && service.status().protocol_decision_count == 1
        }));
        drop(fleet);
        let mut restarted =
            CobaltShadowService::open(root.join("validator-1")).expect("restart service");
        let replay = restarted.replay_protocol_state().expect("replay state");
        assert_eq!(replay, vec![decisions[1].clone()]);
        assert_eq!(restarted.status().ratification_lock_count, 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn signed_protocol_transcript_rejects_tamper_and_wrong_root() {
        let root = test_dir("signed-protocol-tamper");
        let mut fleet = (0..3)
            .map(|index| {
                CobaltShadowService::initialize(
                    root.join(format!("validator-{index}")),
                    identity(&format!("validator-{index}")),
                    CobaltShadowLimits::default(),
                )
                .expect("initialize")
            })
            .collect::<Vec<_>>();
        let transcript = build_signed_protocol_transcript(
            &mut fleet,
            9,
            hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"tamper"),
        )
        .expect("build signed transcript");
        let mut tampered = transcript.clone();
        tampered.rbc_echoes[0]
            .signature_hex
            .replace_range(0..2, "00");
        assert!(fleet[0].commit_protocol_transcript(&tampered).is_err());
        let valid_old = transcript.clone();
        let mut wrong_root = transcript;
        wrong_root.registry_root = "ff".repeat(48);
        assert!(fleet[0].commit_protocol_transcript(&wrong_root).is_err());
        assert_eq!(fleet[0].status().protocol_decision_count, 0);
        let newer = build_signed_protocol_transcript(
            &mut fleet,
            10,
            hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"newer"),
        )
        .expect("build newer transcript");
        fleet[0]
            .commit_protocol_transcript(&newer)
            .expect("commit newer transcript");
        assert!(fleet[0].commit_protocol_transcript(&valid_old).is_err());
        assert_eq!(fleet[0].status().protocol_decision_count, 1);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
