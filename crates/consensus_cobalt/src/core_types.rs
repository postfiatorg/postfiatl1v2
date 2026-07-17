use std::collections::{BTreeMap, BTreeSet};

use postfiat_crypto_provider::hash_hex;
use postfiat_types::{
    GovernanceAmendment, GovernanceVote, ValidatorRegistryEntry, ValidatorRegistryUpdateRecord,
    GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED, GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
    GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT, GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE,
    GOVERNANCE_KIND_AUTHORITY_MODE, GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH, GOVERNANCE_KIND_CRYPTO_POLICY,
    GOVERNANCE_KIND_ORCHARD_POOL_PAUSE, GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT,
    GOVERNANCE_KIND_VALIDATOR_SET, GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
    GOVERNANCE_VAULT_BRIDGE_ROUTE_KIND_PREFIX_V1, FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
    FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1, FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
    FASTSWAP_SCHEMA_VERSION_V1,
};
use serde::{Deserialize, Serialize};

pub const CRATE_PURPOSE: &str = "Cobalt governance and validator-set control";
pub const COBALT_REPORT_ENV: &str = "REPORT";
pub const COBALT_REPORT_ROOT_ENV: &str = "REPORT_ROOT";
pub const COBALT_ALLOW_ABSOLUTE_REPORT_ENV: &str = "COBALT_ALLOW_ABSOLUTE_REPORT";

pub type EssentialSubsetId = String;
pub type TrustViewId = String;
pub type TrustGraphRoot = String;

const MAX_DABC_FULL_KNOWLEDGE_INTERVALS: usize = 4096;
const MAX_DABC_FULL_KNOWLEDGE_CHECKS: usize = 65_536;
const MAX_DABC_PENDING_PAIRS_PER_CHECK: usize = 1024;
const MAX_COBALT_SIGNATURE_HEX_LEN: usize = 8192;
pub const MAX_MVBA_CANDIDATES_PER_SET: usize = 1024;

pub fn emit_example_report<T: Serialize>(report: &T) -> Result<(), Box<dyn std::error::Error>> {
    let body = serde_json::to_string_pretty(report)? + "\n";
    if let Ok(path) = std::env::var(COBALT_REPORT_ENV) {
        let report_root = std::env::var_os(COBALT_REPORT_ROOT_ENV).map(std::path::PathBuf::from);
        let allow_absolute = std::env::var(COBALT_ALLOW_ABSOLUTE_REPORT_ENV)
            .map(|value| value == "1")
            .unwrap_or(false);
        let path = resolve_example_report_path(&path, report_root.as_deref(), allow_absolute)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?;
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}

pub fn resolve_example_report_path(
    report_path: &str,
    report_root: Option<&std::path::Path>,
    allow_absolute: bool,
) -> Result<std::path::PathBuf, String> {
    if report_path.trim().is_empty() {
        return Err(format!("{COBALT_REPORT_ENV} must not be empty"));
    }
    let path = std::path::PathBuf::from(report_path);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(format!(
            "{COBALT_REPORT_ENV} must not contain parent-directory components"
        ));
    }
    if let Some(root) = report_root {
        if path.is_absolute() {
            return Err(format!(
                "{COBALT_REPORT_ENV} must be relative when {COBALT_REPORT_ROOT_ENV} is set"
            ));
        }
        return Ok(root.join(path));
    }
    if path.is_absolute() && !allow_absolute {
        return Err(format!(
            "absolute {COBALT_REPORT_ENV} paths require {COBALT_ALLOW_ABSOLUTE_REPORT_ENV}=1"
        ));
    }
    Ok(path)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EssentialSubsetConfig {
    pub validators: Vec<String>,
    pub quorum: usize,
}

impl EssentialSubsetConfig {
    pub fn all_of(validators: Vec<String>) -> Self {
        let validators: Vec<String> = validators
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self {
            quorum: validators.len(),
            validators,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EssentialSubset {
    pub subset_id: EssentialSubsetId,
    pub validators: Vec<String>,
    pub validator_count: usize,
    pub max_active_byzantine: usize,
    pub quorum: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operator_labels: Vec<String>,
    pub activation_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deactivation_height: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustView {
    pub trust_view_id: TrustViewId,
    pub validator: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub view_version: u64,
    pub essential_subsets: Vec<EssentialSubset>,
    pub derived_unl: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustGraph {
    pub trust_graph_root: TrustGraphRoot,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub graph_version: u64,
    pub registry_root: String,
    pub activation_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_trust_graph_root: Option<TrustGraphRoot>,
    pub trust_views: Vec<TrustView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustGraphTransition {
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub activation_height: u64,
    pub transition_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltSafetyWitnessProfile {
    pub byzantine_budget: usize,
    pub max_cover_subsets: usize,
    pub require_cleared_challenge_state: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltSafetyWitnessInput {
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub activation_height: u64,
    pub challenge_state: String,
    pub profile: CobaltSafetyWitnessProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltSafetyWitnessSubsetRow {
    pub graph_root: TrustGraphRoot,
    pub subset_id: EssentialSubsetId,
    pub validators: Vec<String>,
    pub validator_count: usize,
    pub max_active_byzantine: usize,
    pub quorum: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltSafetyWitnessIntersectionRow {
    pub old_subset_id: EssentialSubsetId,
    pub new_subset_id: EssentialSubsetId,
    pub intersection: Vec<String>,
    pub intersection_size: usize,
    pub byzantine_budget: usize,
    pub safe: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltSafetyWitnessReport {
    pub schema: String,
    pub accepted: bool,
    pub reason: String,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub activation_height: u64,
    pub challenge_state: String,
    pub byzantine_budget: usize,
    pub max_cover_subsets: usize,
    pub old_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    pub new_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    pub intersections: Vec<CobaltSafetyWitnessIntersectionRow>,
    pub rejected_counterexamples: Vec<CobaltSafetyWitnessIntersectionRow>,
    pub report_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltCoverRejectedSubset {
    pub graph_root: TrustGraphRoot,
    pub trust_view_validator: String,
    pub subset_id: EssentialSubsetId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltCoverExtractionReport {
    pub schema: String,
    pub accepted: bool,
    pub complete: bool,
    pub reason: String,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub activation_height: u64,
    pub byzantine_budget: usize,
    pub max_cover_subsets: usize,
    pub old_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    pub new_cover: Vec<CobaltSafetyWitnessSubsetRow>,
    pub total_cover_subsets: usize,
    pub rejected_subsets: Vec<CobaltCoverRejectedSubset>,
    pub report_hash: String,
}

pub const TRUST_GRAPH_LIFECYCLE_OP_TRUST_VIEW_UPDATE: &str = "trust_view_update";
pub const TRUST_GRAPH_LIFECYCLE_OP_ESSENTIAL_SUBSET_UPDATE: &str = "essential_subset_update";
pub const TRUST_GRAPH_ROLLBACK_REASON_UNSAFE_LINKAGE: &str = "unsafe_linkage";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustGraphLifecycleRecord {
    pub record_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub operation: String,
    pub subject_validator: String,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub trust_graph_transition_id: String,
    pub activation_height: u64,
    pub previous_trust_view_id: TrustViewId,
    pub new_trust_view_id: TrustViewId,
    pub previous_subset_ids: Vec<EssentialSubsetId>,
    pub new_subset_ids: Vec<EssentialSubsetId>,
    pub linkage_report_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustGraphRollbackRecord {
    pub record_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub authority_trust_graph_root: TrustGraphRoot,
    pub failed_trust_graph_root: TrustGraphRoot,
    pub rollback_trust_graph_root: TrustGraphRoot,
    pub registry_root: String,
    pub failed_activation_height: u64,
    pub rollback_activation_height: u64,
    pub bad_linkage_report_hash: String,
    pub rollback_linkage_report_hash: String,
    pub trust_graph_transition_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CobaltFaultModel {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actively_byzantine: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorPair {
    pub left: String,
    pub right: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsafePairReport {
    pub left: String,
    pub right: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectivityReport {
    pub validator: String,
    pub derived_unl: Vec<String>,
    pub extended_unl: Vec<String>,
    pub fully_linked_with: Vec<String>,
    pub weakly_connected_in_known_graph: bool,
    pub strongly_connected_known_closure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkageReport {
    pub trust_graph_root: String,
    pub registry_root: String,
    pub trust_view_count: usize,
    pub actively_byzantine: Vec<String>,
    pub linked_pairs: Vec<ValidatorPair>,
    pub fully_linked_pairs: Vec<ValidatorPair>,
    pub unsafe_pairs: Vec<UnsafePairReport>,
    pub weakly_connected_validators: Vec<String>,
    pub strongly_connected_validators: Vec<String>,
    pub connectivity: Vec<ConnectivityReport>,
    pub report_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltProposal {
    pub instance_id: String,
    pub proposal_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub proposer: String,
    pub kind: String,
    pub value: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltVote {
    pub vote_id: String,
    pub instance_id: String,
    pub proposal_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub validator: String,
    pub accept: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltCertificate {
    pub certificate_id: String,
    pub instance_id: String,
    pub proposal_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub quorum: usize,
    pub votes: Vec<CobaltVote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CobaltGovernanceMode {
    Canonical,
    NonUniform,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonUniformSatisfiedSubset {
    pub subset_id: EssentialSubsetId,
    pub validator_count: usize,
    pub max_active_byzantine: usize,
    pub quorum: usize,
    pub support: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NonUniformGovernanceCertificate {
    pub certificate_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub instance_id: String,
    pub proposal_id: String,
    pub support: Vec<String>,
    pub satisfied_subsets: Vec<NonUniformSatisfiedSubset>,
    pub linkage_report_hash: String,
    pub votes: Vec<GovernanceVote>,
}

pub const RBC_MESSAGE_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-rbc/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcPropose {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub amendment_slot: u64,
    pub payload_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcEcho {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub proposer: String,
    pub amendment_slot: u64,
    pub payload_hash: String,
    pub propose_message_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcReady {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub proposer: String,
    pub amendment_slot: u64,
    pub payload_hash: String,
    pub propose_message_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcAccept {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub proposer: String,
    pub amendment_slot: u64,
    pub payload_hash: String,
    pub propose_message_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcSupportEvaluation {
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub message_kind: String,
    pub propose_message_id: String,
    pub amendment_slot: u64,
    pub payload_hash: String,
    pub support: Vec<String>,
    pub weak_support: bool,
    pub strong_support: bool,
    pub strong_satisfied_subsets: Vec<NonUniformSatisfiedSubset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbcConflictingAcceptEvidence {
    pub evidence_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub amendment_slot: u64,
    pub proposer: String,
    pub left_sender: String,
    pub right_sender: String,
    pub left_payload_hash: String,
    pub right_payload_hash: String,
    pub left_propose_message_id: String,
    pub right_propose_message_id: String,
    pub linked: bool,
    pub fully_linked: bool,
    pub reason: String,
}

pub const ABBA_MESSAGE_SIGNATURE_CONTEXT: &[u8] = b"postfiat-l1-v2/cobalt-abba/v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaInit {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub agreement_id: String,
    pub round: u64,
    pub value: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaAux {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub agreement_id: String,
    pub round: u64,
    pub value: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaConf {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub agreement_id: String,
    pub round: u64,
    pub value: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaFinish {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub agreement_id: String,
    pub round: u64,
    pub value: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaRoundState {
    pub trust_graph_root: TrustGraphRoot,
    pub agreement_id: String,
    pub round: u64,
    pub init_messages: Vec<AbbaInit>,
    pub aux_messages: Vec<AbbaAux>,
    pub conf_messages: Vec<AbbaConf>,
    pub finish_messages: Vec<AbbaFinish>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaSupportEvaluation {
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub message_kind: String,
    pub agreement_id: String,
    pub round: u64,
    pub value: bool,
    pub support: Vec<String>,
    pub weak_support: bool,
    pub strong_support: bool,
    pub strong_satisfied_subsets: Vec<NonUniformSatisfiedSubset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaConflictingFinishEvidence {
    pub evidence_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub agreement_id: String,
    pub round: u64,
    pub left_sender: String,
    pub right_sender: String,
    pub left_value: bool,
    pub right_value: bool,
    pub linked: bool,
    pub fully_linked: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbbaEquivocationEvidence {
    pub evidence_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub agreement_id: String,
    pub round: u64,
    pub message_kind: String,
    pub sender: String,
    pub left_value: bool,
    pub right_value: bool,
    pub left_message_id: String,
    pub right_message_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CobaltRuntimeMode {
    Simulation,
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AbbaCommonRandomSource {
    DeterministicTest {
        seed_hex: String,
    },
    SignedBeacon {
        beacon_id: String,
        output_hash: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MvbaCandidate {
    pub candidate_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub amendment_slot: u64,
    pub proposer: String,
    pub payload_hash: String,
    pub propose_message_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MvbaValidInputSet {
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub agreement_id: String,
    pub candidates: Vec<MvbaCandidate>,
    pub output_candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcRatifiedAmendment {
    pub ratification_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub sequence: u64,
    pub amendment_slot: u64,
    pub parent_ratification_id: String,
    pub mvba_agreement_id: String,
    pub output_candidate_id: String,
    pub candidate: MvbaCandidate,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcPendingPair {
    pub amendment_slot: u64,
    pub output_candidate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcFullKnowledgeCheck {
    pub message_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub trust_graph_root: TrustGraphRoot,
    pub sender: String,
    pub checkpoint_height: u64,
    pub pending_pairs: Vec<DabcPendingPair>,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcFullKnowledgeCheckpoint {
    pub checkpoint_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub interval_height: u64,
    pub wait_until_height: u64,
    pub covered_heights: Vec<u64>,
    pub checks: Vec<DabcFullKnowledgeCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcActivationEvidence {
    pub activation_id: String,
    pub ratification_id: String,
    pub checkpoint_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub trust_view_id: TrustViewId,
    pub local_validator: String,
    pub ratified_sequence: u64,
    pub amendment_slot: u64,
    pub activation_height: u64,
    pub wait_until_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcReplayBundle {
    pub bundle_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub ratified_amendments: Vec<DabcRatifiedAmendment>,
    pub full_knowledge_checkpoints: Vec<DabcFullKnowledgeCheckpoint>,
    pub activation_evidence: Vec<DabcActivationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcReplayReport {
    pub bundle_id: String,
    pub ratified_count: usize,
    pub activation_count: usize,
    pub checkpoint_count: usize,
    pub highest_sequence: u64,
    pub highest_activation_height: u64,
    pub ratification_ids: Vec<String>,
    pub checkpoint_ids: Vec<String>,
    pub activation_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcValidatorLifecycleRatification {
    pub lifecycle_ratification_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub dabc_ratification_id: String,
    pub registry_update_id: String,
    pub operation: String,
    pub subject_node_id: String,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub payload_hash: String,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcTrustGraphLifecycleRatification {
    pub lifecycle_ratification_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub dabc_ratification_id: String,
    pub trust_graph_lifecycle_record_id: String,
    pub operation: String,
    pub subject_validator: String,
    pub previous_trust_graph_root: TrustGraphRoot,
    pub new_trust_graph_root: TrustGraphRoot,
    pub payload_hash: String,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcTrustGraphRollbackRatification {
    pub rollback_ratification_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub dabc_ratification_id: String,
    pub rollback_record_id: String,
    pub authority_trust_graph_root: TrustGraphRoot,
    pub failed_trust_graph_root: TrustGraphRoot,
    pub rollback_trust_graph_root: TrustGraphRoot,
    pub payload_hash: String,
    pub activation_height: u64,
}

pub struct TrustGraphRollbackRatificationInput<'a> {
    pub domain: &'a CobaltDomain,
    pub authority_graph: &'a TrustGraph,
    pub failed_graph: &'a TrustGraph,
    pub rollback_graph: &'a TrustGraph,
    pub bad_linkage_report: &'a LinkageReport,
    pub rollback_linkage_report: &'a LinkageReport,
    pub ratified: &'a DabcRatifiedAmendment,
    pub previous_ratified: Option<&'a DabcRatifiedAmendment>,
    pub record: &'a TrustGraphRollbackRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DabcTransactionNetworkRatification {
    pub transaction_network_ratification_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub dabc_ratification_id: String,
    pub transaction_network_id: String,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub payload_hash: String,
    pub governance_epoch: u64,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransactionNetworkMembership {
    pub transaction_network_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub governance_epoch: u64,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub activation_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CobaltBlockMembershipBinding {
    pub binding_id: String,
    pub block_hash: String,
    pub block_height: u64,
    pub proposer: String,
    pub registry_root: String,
    pub trust_graph_root: TrustGraphRoot,
    pub governance_epoch: u64,
    pub transaction_network_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct GovernanceAmendmentLifecycle {
    pub activation_height: u64,
    pub veto_until_height: u64,
    pub paused: bool,
}

impl GovernanceAmendmentLifecycle {
    pub fn immediate() -> Self {
        Self::default()
    }

    fn is_immediate(&self) -> bool {
        self.activation_height == 0 && self.veto_until_height == 0 && !self.paused
    }
}

pub const VALIDATOR_REGISTRY_UPDATE_SCHEMA: &str = "postfiat.validator_registry_update.v1";
pub const VALIDATOR_REGISTRY_OP_ADMIT: &str = "admit";
pub const VALIDATOR_REGISTRY_OP_REMOVE: &str = "remove";
pub const VALIDATOR_REGISTRY_OP_SUSPEND: &str = "suspend";
pub const VALIDATOR_REGISTRY_OP_REACTIVATE: &str = "reactivate";
pub const VALIDATOR_REGISTRY_OP_ROTATE_KEY: &str = "rotate_key";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorRegistryUpdateRequest {
    pub activation_height: u64,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_trust_graph_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_trust_graph_root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_graph_transition_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub previous_validators: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub new_validators: Vec<String>,
    pub operation: String,
    pub subject_node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_record: Option<ValidatorRegistryEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_record: Option<ValidatorRegistryEntry>,
}
