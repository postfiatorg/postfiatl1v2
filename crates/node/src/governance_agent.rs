pub const DEFAULT_GOVERNANCE_AGENT_DIR: &str = "docs/governance/agent";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_1_5_REPORT: &str =
    "reports/gov-inference-gate-1_5-constitutional-prompt-bundle.json";
pub const DEFAULT_GOVERNANCE_AGENT_MODEL_REQUEST_FILE: &str =
    "docs/governance/agent/fixtures/dry_run_model_request.json";
pub const DEFAULT_GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_FIELD_REGISTRY_FILE: &str =
    "docs/governance/validator-evidence-field-registry.md";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_3_5_REPORT: &str =
    "reports/gov-inference-gate-3_5-deterministic-ruleset-generation.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_3_6_REPORT: &str =
    "reports/gov-inference-gate-3_6-timelocked-governance-agent.json";
pub const DEFAULT_GOVERNANCE_AGENT_EVIDENCE_FILE: &str =
    "docs/governance/agent/fixtures/frozen_evidence_snapshot.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_7_5_REPORT: &str =
    "reports/gov-inference-gate-7_5-ruleset-compiler.json";
pub const DEFAULT_GOVERNANCE_AGENT_COMPARISON_DIR: &str =
    "docs/governance/agent/fixtures/comparison";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_7_6_REPORT: &str =
    "reports/gov-inference-gate-7_6-ruleset-vs-llm-judgment.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPORT: &str =
    "reports/gov-inference-gate-8_5-cobalt-ruleset-dry-run.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_8_5_REPLAY_BUNDLE: &str =
    "reports/gov-inference-gate-8_5-cobalt-ruleset-dry-run.replay-bundle.json";
pub const DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_RULESET_FILE: &str =
    "docs/governance/agent/fixtures/guarded_apply_ruleset.json";
pub const DEFAULT_GOVERNANCE_AGENT_GUARDED_APPLY_EVIDENCE_FILE: &str =
    "docs/governance/agent/fixtures/guarded_apply_evidence_snapshot.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_9_5_REPORT: &str =
    "reports/gov-inference-gate-9_5-generated-ruleset-guarded-apply.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_10_1_REPORT: &str =
    "reports/gov-inference-gate-10_1-verillm-postfiat-benchmark.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_10_5_REPORT: &str =
    "reports/gov-inference-gate-10_5-toploc-receipt-prototype.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_14_REPORT: &str =
    "reports/gov-inference-gate-14-tp-invariant-verifier-admission.json";
pub const DEFAULT_GOVERNANCE_AGENT_GATE_15_REPORT: &str =
    "reports/gov-inference-gate-15-adversarial-governance-probes.json";
pub const DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_FILE: &str =
    "docs/governance/agent/implementation_work_item_dga_200.json";
pub const DEFAULT_GOVERNANCE_AGENT_IMPLEMENTATION_EXECUTION_REPORT: &str =
    "reports/gov-inference-implementation-execution.json";

const GOVERNANCE_AGENT_RULESET_SCHEMA: &str = "postfiat-governance-ruleset-v1";
const GOVERNANCE_AGENT_GATE_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-1_5-v1";
const GOVERNANCE_AGENT_GATE_3_5_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-3_5-v1";
const GOVERNANCE_AGENT_GATE_3_6_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-3_6-v1";
const GOVERNANCE_AGENT_GATE_7_5_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-7_5-v1";
const GOVERNANCE_AGENT_GATE_7_6_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-7_6-v1";
const GOVERNANCE_AGENT_GATE_8_5_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-8_5-v1";
const GOVERNANCE_AGENT_GATE_9_5_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-9_5-v1";
const GOVERNANCE_AGENT_GATE_10_1_REPORT_SCHEMA: &str =
    "postfiat-governance-agent-gate-10_1-v1";
const GOVERNANCE_AGENT_GATE_10_5_REPORT_SCHEMA: &str =
    "postfiat-governance-agent-gate-10_5-v1";
const GOVERNANCE_AGENT_GATE_14_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-14-v1";
const GOVERNANCE_AGENT_GATE_15_REPORT_SCHEMA: &str = "postfiat-governance-agent-gate-15-v1";
const GOVERNANCE_AGENT_EVIDENCE_LINEAGE_AUDIT_SCHEMA: &str =
    "postfiat-governance-agent-validator-evidence-lineage-audit-v1";
const GOVERNANCE_AGENT_IMPLEMENTATION_WORK_ITEM_SCHEMA: &str =
    "postfiat-governance-agent-implementation-work-item-v1";
const GOVERNANCE_AGENT_IMPLEMENTATION_EXECUTION_REPORT_SCHEMA: &str =
    "postfiat-governance-agent-implementation-execution-v1";
const GOVERNANCE_AGENT_DRY_RUN_REPLAY_BUNDLE_SCHEMA: &str =
    "postfiat-governance-agent-dry-run-replay-bundle-v1";
const GOVERNANCE_AGENT_GUARDED_APPLY_CANDIDATE_SCHEMA: &str =
    "postfiat-governance-agent-guarded-apply-candidate-v1";
const GOVERNANCE_AGENT_INFERENCE_RECEIPT_PROTOTYPE_SCHEMA: &str =
    "postfiat-governance-agent-inference-receipt-prototype-v1";
const GOVERNANCE_AGENT_BUNDLE_SCHEMA: &str = "postfiat-governance-agent-bundle-v1";
const GOVERNANCE_AGENT_MODEL_REQUEST_SCHEMA: &str =
    "postfiat-governance-agent-model-request-v1";
const GOVERNANCE_AGENT_ROUND_SEED_SCHEMA: &str = "postfiat-governance-agent-round-seed-v1";
const GOVERNANCE_AGENT_COMPILED_POLICY_SCHEMA: &str =
    "postfiat-governance-agent-compiled-policy-v1";
const GOVERNANCE_AGENT_EVIDENCE_SCHEMA: &str = "postfiat-governance-agent-frozen-evidence-v1";
const GOVERNANCE_AGENT_COMPARISON_FIXTURE_SCHEMA: &str =
    "postfiat-governance-agent-comparison-fixture-v1";
const GOVERNANCE_AGENT_REGISTRY_DELTA_SCHEMA: &str =
    "postfiat-governance-agent-registry-delta-candidate-v1";
const GOVERNANCE_AGENT_MODEL_ID: &str = "Qwen/Qwen3.6-27B-FP8";
const GOVERNANCE_AGENT_RUNTIME: &str = "Modal/SGLang H100 deterministic profile";
const GOVERNANCE_AGENT_RUNTIME_ENGINE: &str = "SGLang";
const GOVERNANCE_AGENT_PROVIDER_PROFILE: &str = "RunPod H100 primary; Vast fallback";
const GOVERNANCE_AGENT_SGLANG_IMAGE: &str = "lmsysorg/sglang:nightly-dev-cu13-20260523-c112f762";
const GOVERNANCE_AGENT_DEFAULT_ROUND_ID: &str = "dga-gate-3_6-local-round-v1";
const GOVERNANCE_AGENT_DEFAULT_ROUND_DOMAIN: &str = "postfiat.governance_agent.gate_3_6.local";
const GOVERNANCE_AGENT_EVIDENCE_ROOT: &str = "postfiat-local-gate-1_5-no-external-evidence";
const GOVERNANCE_AGENT_MAX_OUTPUT_TOKENS: u64 = 4096;
const GOVERNANCE_AGENT_CONTEXT_LENGTH: u64 = 32768;
const GOVERNANCE_AGENT_CHUNKED_PREFILL_SIZE: u64 = 4096;
const GOVERNANCE_AGENT_TENSOR_PARALLELISM: u64 = 1;
const GOVERNANCE_AGENT_MAX_RUNNING_REQUESTS: u64 = 1;
const GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_PACKET_SCHEMA_JSON: &str =
    include_str!("../resources/governance_agent/validator_evidence_packet_schema.json");
const GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_PACKET_SCHEMA_FILE_NAME: &str =
    "validator_evidence_packet_schema.json";
const GOVERNANCE_AGENT_VALIDATOR_EVIDENCE_INPUT_KIND: &str = "validator_evidence_packet";
const GOVERNANCE_AGENT_SYSTEM_PROMPT: &str = "You are the PostFiat deterministic governance-agent ruleset generator. Return exactly one JSON object conforming to schema postfiat-governance-ruleset-v1. Every decision must cite a registered evidence_field_path and closed evidence semantics. Do not include Markdown, prose, comments, code fences, tool calls, unregistered evidence fields, private knowledge, live browsing instructions, or fields outside the schema.";
const GOVERNANCE_AGENT_USER_INSTRUCTION: &str = "Generate a GovernanceRuleset for the governed PostFiat validator-registry policy bundle. Each decision must declare evidence_field_path, required_provenance, freshness_requirement, missing_evidence_behavior, conflict_behavior, and action_bound from the schema. Generated rules that cite validator evidence fields must explicitly require the validator_evidence_packet input and may not depend on hidden or live evidence. If evidence is incomplete, ambiguous, stale, conflicting, unregistered, or outside scope, emit an explicit no_op decision. Gate 3.5 must remain dry-run only.";
const GOVERNANCE_AGENT_DGA_200_WORK_ITEM_ID: &str =
    "DGA-IMPLEMENTATION-WORK-ITEM-2026-05-23-001";
const GOVERNANCE_AGENT_DGA_200_WORK_PACKAGE_ID: &str =
    "DGA-IMPLEMENTATION-WORK-PACKAGE-2026-05-23-001";
const GOVERNANCE_AGENT_DGA_200_AUTHORIZATION_PLAN_ID: &str =
    "DGA-IMPLEMENTATION-AUTH-2026-05-23-001";
const GOVERNANCE_AGENT_DGA_200_QUEUE_DECISION_ID: &str =
    "DGA-QUEUE-DECISION-2026-05-23-001";
const GOVERNANCE_AGENT_DGA_200_AUTHORIZATION_DOC: &str =
    "docs/governance/deterministic-governance-agent-implementation-authorization.md";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGateOptions {
    pub agent_dir: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentModelRequestOptions {
    pub agent_dir: PathBuf,
    pub output_file: PathBuf,
    pub round_seed_input: Option<GovernanceAgentRoundSeedInput>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate3_5Options {
    pub model_request_file: PathBuf,
    pub outputs_dir: PathBuf,
    pub output_file: PathBuf,
    pub expected_count: usize,
    pub round_seed_input: Option<GovernanceAgentRoundSeedInput>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate3_6Options {
    pub agent_dir: PathBuf,
    pub output_file: PathBuf,
    pub round_seed_input: Option<GovernanceAgentRoundSeedInput>,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate7_5Options {
    pub ruleset_file: PathBuf,
    pub evidence_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate7_6Options {
    pub ruleset_file: PathBuf,
    pub comparison_dir: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate8_5Options {
    pub agent_dir: PathBuf,
    pub ruleset_file: PathBuf,
    pub evidence_file: PathBuf,
    pub output_file: PathBuf,
    pub replay_bundle_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate9_5Options {
    pub agent_dir: PathBuf,
    pub ruleset_file: PathBuf,
    pub evidence_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate10_1Options {
    pub model_request_file: PathBuf,
    pub ruleset_file: PathBuf,
    pub gate_9_5_report_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate10_5Options {
    pub model_request_file: PathBuf,
    pub ruleset_file: PathBuf,
    pub gate_9_5_report_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate14Options {
    pub model_request_file: PathBuf,
    pub ruleset_file: PathBuf,
    pub gate_9_5_report_file: PathBuf,
    pub receipt_report_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentGate15Options {
    pub model_request_file: PathBuf,
    pub ruleset_file: PathBuf,
    pub gate_9_5_report_file: PathBuf,
    pub receipt_report_file: PathBuf,
    pub gate_14_report_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentEvidenceLineageAuditOptions {
    pub model_request_file: PathBuf,
    pub gate_3_5_report_file: PathBuf,
    pub gate_3_6_report_file: PathBuf,
    pub gate_10_1_report_file: PathBuf,
    pub receipt_report_file: PathBuf,
    pub gate_14_report_file: PathBuf,
    pub gate_15_report_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceAgentImplementationExecutionOptions {
    pub work_item_file: PathBuf,
    pub output_file: PathBuf,
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentStatementHash {
    pub name: String,
    pub path: String,
    pub hash: String,
    pub byte_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentFixtureCheck {
    pub name: String,
    pub path: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGateReport {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub agent_dir: String,
    pub bundle_hash: String,
    pub model_request_hash: String,
    pub statement_hashes: Vec<GovernanceAgentStatementHash>,
    pub ruleset_schema_hash: String,
    pub valid_fixture: GovernanceAgentFixtureCheck,
    pub invalid_fixtures: Vec<GovernanceAgentFixtureCheck>,
    pub canonical_json_key_order_stable: bool,
    pub statement_hash_one_byte_edit_detected: bool,
    pub bundle_hash_includes: Vec<String>,
    pub model_request_hash_includes: Vec<String>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRuntimeManifest {
    pub model_id: String,
    pub runtime: String,
    pub runtime_profile: String,
    pub provider_profile: String,
    pub image: String,
    pub tensor_parallelism: u64,
    pub context_length: u64,
    pub chunked_prefill_size: u64,
    pub max_running_requests: u64,
    pub deterministic_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentOutputContract {
    pub schema: String,
    pub json_only: bool,
    pub markdown_allowed: bool,
    pub prose_allowed: bool,
    pub unknown_fields_allowed: bool,
    pub fallback_decision: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRoundSeedInput {
    pub schema: String,
    pub cobalt_certificate_hash: String,
    pub round_id: String,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRequestStatement {
    pub name: String,
    pub path: String,
    pub hash: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRequestInputs {
    pub bundle_hash: String,
    pub statement_hashes: Vec<GovernanceAgentStatementHash>,
    pub statements: Vec<GovernanceAgentRequestStatement>,
    pub ruleset_schema_hash: String,
    pub ruleset_schema: serde_json::Value,
    pub validator_evidence_packet_schema_path: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_path: String,
    pub validator_evidence_field_registry_hash: String,
    pub valid_fixture_hash: String,
    pub valid_fixture: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentModelRequest {
    pub schema: String,
    pub request_id: String,
    pub request_hash: String,
    pub bundle_hash: String,
    pub evidence_root: String,
    pub round_seed_input: GovernanceAgentRoundSeedInput,
    pub round_seed: String,
    pub runtime_manifest: GovernanceAgentRuntimeManifest,
    pub output_contract: GovernanceAgentOutputContract,
    pub governed_inputs: GovernanceAgentRequestInputs,
    pub openai_chat_request: serde_json::Value,
    pub request_hash_includes: Vec<String>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGenerationCheck {
    pub index: usize,
    pub path: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_policy_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_byte_len: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate3_5Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub outputs_dir: String,
    pub expected_count: usize,
    pub observed_count: usize,
    pub valid_count: usize,
    pub model_request_hash: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub bundle_hash: String,
    pub round_seed_input: GovernanceAgentRoundSeedInput,
    pub round_seed: String,
    pub runtime_manifest: GovernanceAgentRuntimeManifest,
    pub distinct_ruleset_hashes: Vec<String>,
    pub distinct_compiled_policy_hashes: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruleset_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compiled_policy_hash: Option<String>,
    pub deterministic_ruleset_hash: bool,
    pub deterministic_compiled_policy_hash: bool,
    pub generation_checks: Vec<GovernanceAgentGenerationCheck>,
    pub compiled_policy_note: String,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentSeedRejectionCheck {
    pub name: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate3_6Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub agent_dir: String,
    pub model_request_hash: String,
    pub replay_model_request_hash: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub bundle_hash: String,
    pub round_seed_input: GovernanceAgentRoundSeedInput,
    pub round_seed: String,
    pub same_seed_replays: bool,
    pub rejection_checks: Vec<GovernanceAgentSeedRejectionCheck>,
    pub request_hash_includes: Vec<String>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentFrozenEvidenceSnapshot {
    pub schema: String,
    pub snapshot_id: String,
    pub validator_registry_root: String,
    pub cobalt_evidence_root: String,
    pub operator_manifest_root: String,
    pub validator_evidence_packet_root: String,
    pub available_inputs: Vec<String>,
    pub network_access_allowed: bool,
    pub model_access_allowed: bool,
    pub filesystem_access_allowed: bool,
    pub direct_state_mutation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentPolicySandbox {
    pub network_access: bool,
    pub model_access: bool,
    pub filesystem_access: bool,
    pub direct_state_mutation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentCompiledPolicy {
    pub schema: String,
    pub policy_shape: String,
    pub interpreter: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub ruleset_id: String,
    pub scope: String,
    pub input_kinds: Vec<String>,
    pub decision_ids: Vec<String>,
    pub no_op_decision_id: String,
    pub max_mutations: u64,
    pub rollback_required: bool,
    pub operator_confirmation_required: bool,
    pub sandbox: GovernanceAgentPolicySandbox,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRegistryMutation {
    pub operation: String,
    pub subject_node_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentRegistryDeltaCandidate {
    pub schema: String,
    pub candidate_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub evidence_snapshot_hash: String,
    pub decision_id: String,
    pub action: String,
    pub mutations: Vec<GovernanceAgentRegistryMutation>,
    pub mutation_count: usize,
    pub rollback_required: bool,
    pub operator_confirmation_required: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentPolicyRejectionCheck {
    pub name: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate7_5Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub ruleset_file: String,
    pub evidence_file: String,
    pub policy_shape_decision: String,
    pub ruleset_hash: String,
    pub evidence_snapshot_hash: String,
    pub compiled_policy_hash: String,
    pub registry_delta_candidate_hash: String,
    pub deterministic_replay: bool,
    pub sandbox: GovernanceAgentPolicySandbox,
    pub compiled_policy: GovernanceAgentCompiledPolicy,
    pub registry_delta_candidate: GovernanceAgentRegistryDeltaCandidate,
    pub malformed_rule_checks: Vec<GovernanceAgentPolicyRejectionCheck>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentDirectBaseline {
    pub source: String,
    pub authoritative: bool,
    pub action: String,
    pub confidence_bps: u64,
    pub mutations: Vec<GovernanceAgentRegistryMutation>,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentComparisonFixture {
    pub schema: String,
    pub case_id: String,
    pub case_class: String,
    pub evidence: GovernanceAgentFrozenEvidenceSnapshot,
    pub direct_baseline: GovernanceAgentDirectBaseline,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentComparisonCheck {
    pub case_id: String,
    pub case_class: String,
    pub fixture_hash: String,
    pub direct_baseline_hash: String,
    pub evidence_snapshot_hash: String,
    pub policy_delta_hash: String,
    pub direct_action: String,
    pub policy_action: String,
    pub direct_mutation_count: usize,
    pub policy_mutation_count: usize,
    pub agrees_with_direct: bool,
    pub policy_rejected_unsafe_direct_delta: bool,
    pub policy_no_ops_unsafe_case: bool,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate7_6Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub ruleset_file: String,
    pub comparison_dir: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub fixture_count: usize,
    pub direct_baseline_authoritative: bool,
    pub manual_review_scope: String,
    pub high_confidence_agreement_count: usize,
    pub unsafe_direct_delta_rejection_count: usize,
    pub ambiguous_no_op_count: usize,
    pub top_k_overlap_count: usize,
    pub false_add_count: usize,
    pub false_remove_count: usize,
    pub churn_behavior: String,
    pub concentration_behavior: String,
    pub disagreement_case_ids: Vec<String>,
    pub comparison_checks: Vec<GovernanceAgentComparisonCheck>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentDryRunReplayBundle {
    pub schema: String,
    pub bundle_hash: String,
    pub architecture_statement_hash: String,
    pub objective_statement_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub evidence_snapshot_hash: String,
    pub registry_delta_candidate_hash: String,
    pub validator_registry_root: String,
    pub validator_evidence_packet_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentDryRunRejectionCheck {
    pub name: String,
    pub rejected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate8_5Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub action_mode: String,
    pub dry_run_id: String,
    pub dry_run_record_id: String,
    pub governance_batch_id: String,
    pub bundle_hash: String,
    pub architecture_statement_hash: String,
    pub objective_statement_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub evidence_snapshot_hash: String,
    pub validator_evidence_packet_root: String,
    pub replay_bundle_root: String,
    pub replay_bundle_uri: String,
    pub report_root: String,
    pub report_uri: String,
    pub validator_registry_root_before: String,
    pub validator_registry_root_after: String,
    pub registry_unchanged: bool,
    pub registry_mutation_count: u32,
    pub cobalt_batch_verified: bool,
    pub dry_run_recorded: bool,
    pub governance_agent_record_count: usize,
    pub stale_ruleset_rejected: bool,
    pub wrong_bundle_rejected: bool,
    pub missing_replay_root_rejected: bool,
    pub replay_bundle_retrievable: bool,
    pub rejection_checks: Vec<GovernanceAgentDryRunRejectionCheck>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGuardedApplyHardCaps {
    pub max_adds_per_round: u32,
    pub max_registry_mutations: u32,
    pub routine_removals_allowed: u32,
    pub evidence_refs_required: bool,
    pub concentration_caps_required: bool,
    pub linkedness_required: bool,
    pub rollback_required: bool,
    pub cobalt_acceptance_required: bool,
    pub human_approval_after_activation_allowed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentEvidenceRef {
    pub kind: String,
    pub root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentConcentrationCheck {
    pub name: String,
    pub evidence_ref_kind: String,
    pub observed_bps: u32,
    pub limit_bps: u32,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGuardedApplyCandidate {
    pub schema: String,
    pub candidate_hash: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub evidence_snapshot_hash: String,
    pub decision_id: String,
    pub action: String,
    pub mutations: Vec<GovernanceAgentRegistryMutation>,
    pub mutation_count: usize,
    pub previous_validators: Vec<String>,
    pub new_validators: Vec<String>,
    pub previous_registry_root: String,
    pub new_registry_root: String,
    pub evidence_refs: Vec<GovernanceAgentEvidenceRef>,
    pub concentration_checks: Vec<GovernanceAgentConcentrationCheck>,
    pub linkedness_root: String,
    pub linkedness_passed: bool,
    pub rollback_required: bool,
    pub rollback_drill_required: bool,
    pub routine_removal: bool,
    pub human_approval_required: bool,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGuardedApplyRejectionCheck {
    pub name: String,
    pub rejected: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate9_5Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub bundle_hash: String,
    pub ruleset_file: String,
    pub evidence_file: String,
    pub ruleset_hash: String,
    pub compiled_policy_hash: String,
    pub evidence_snapshot_hash: String,
    pub hard_caps: GovernanceAgentGuardedApplyHardCaps,
    pub candidate_hash: String,
    pub candidate: GovernanceAgentGuardedApplyCandidate,
    pub initial_validator_count: usize,
    pub post_apply_validator_count: usize,
    pub initial_registry_root: String,
    pub guarded_apply_registry_root: String,
    pub post_apply_registry_root: String,
    pub rollback_registry_root: String,
    pub cobalt_update_id: String,
    pub cobalt_governance_batch_id: String,
    pub rollback_update_id: String,
    pub rollback_governance_batch_id: String,
    pub cobalt_acceptance_verified: bool,
    pub rollback_cobalt_acceptance_verified: bool,
    pub registry_changes_only_after_cobalt_acceptance: bool,
    pub max_one_add: bool,
    pub zero_routine_removals: bool,
    pub evidence_refs_valid: bool,
    pub concentration_caps_passed: bool,
    pub trust_graph_linkedness_passed: bool,
    pub rollback_available: bool,
    pub rollback_restored_registry: bool,
    pub no_human_approval_after_activation: bool,
    pub rejection_checks: Vec<GovernanceAgentGuardedApplyRejectionCheck>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentVerifierCostBenchmark {
    pub measurement_method: String,
    pub full_inference_input_bytes: usize,
    pub full_inference_output_bytes: usize,
    pub full_inference_work_units: usize,
    pub hash_replay_bytes: usize,
    pub schema_verifier_bytes: usize,
    pub selector_verifier_bytes: usize,
    pub measured_verifier_work_units: usize,
    pub verifier_to_full_cost_bps: u32,
    pub generic_one_percent_claim_assumed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate10_1Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub ruleset_file: String,
    pub gate_9_5_report_file: String,
    pub model_request_hash: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub ruleset_hash: String,
    pub gate_9_5_report_hash: String,
    pub candidate_hash: String,
    pub benchmark: GovernanceAgentVerifierCostBenchmark,
    pub request_hash_recomputed: bool,
    pub ruleset_hash_recomputed: bool,
    pub candidate_hash_recomputed: bool,
    pub gate_9_5_report_verified: bool,
    pub cost_measured_not_assumed: bool,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentCompactReceiptCommitment {
    pub algorithm: String,
    pub token_proxy_method: String,
    pub chunk_size_tokens: usize,
    pub chunk_count: usize,
    pub prompt_token_proxy_count: usize,
    pub compact_bytes: usize,
    pub direct_embedding_bytes: usize,
    pub compact_to_direct_bps: u32,
    pub chunk_commitment_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentInferenceReceiptPrototype {
    pub schema: String,
    pub receipt_id: String,
    pub bundle_hash: String,
    pub evidence_snapshot_root: String,
    pub model_request_hash: String,
    pub model_response_hash: String,
    pub parsed_output_hash: String,
    pub generated_action_hash: String,
    pub provider: String,
    pub provider_run_id: String,
    pub hardware_class: String,
    pub runtime_manifest_hash: String,
    pub signer: String,
    pub signature_required: bool,
    pub compact_commitment_root: String,
    pub verifier_attestation_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentVerifierAttestation {
    pub attestation_id: String,
    pub verifier_id: String,
    pub verifier_kind: String,
    pub accepted: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate10_5Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub ruleset_file: String,
    pub gate_9_5_report_file: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub receipt: GovernanceAgentInferenceReceiptPrototype,
    pub compact_commitment: GovernanceAgentCompactReceiptCommitment,
    pub verifier_attestations: Vec<GovernanceAgentVerifierAttestation>,
    pub accepted_verifier_count: usize,
    pub verifier_quorum: usize,
    pub correct_receipt_accepted: bool,
    pub incorrect_receipt_rejected: bool,
    pub verifier_disagreement_recorded: bool,
    pub consensus_critical: bool,
    pub prototype_only: bool,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentTensorParallelCheck {
    pub tensor_parallelism: u32,
    pub evidence_hash: Option<String>,
    pub output_hash: Option<String>,
    pub matches_canonical_tp1: bool,
    pub admitted: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentShadowPathStep {
    pub order: u32,
    pub name: String,
    pub required_artifact: String,
    pub authority_effect: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentValidatorSideShadowPlan {
    pub status: String,
    pub sidecars_live: bool,
    pub commit_reveal_live: bool,
    pub authority_transfer_live: bool,
    pub failure_behavior: String,
    pub steps: Vec<GovernanceAgentShadowPathStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate14Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub ruleset_file: String,
    pub gate_9_5_report_file: String,
    pub receipt_report_file: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub canonical_tensor_parallelism: u32,
    pub canonical_output_hash: String,
    pub receipt_report_verified: bool,
    pub tp_checks: Vec<GovernanceAgentTensorParallelCheck>,
    pub cross_tp_hash_agreement_demonstrated: bool,
    pub tp_greater_than_one_admitted: bool,
    pub tp_invariant_admission_ready: bool,
    pub validator_side_shadow_path_defined: bool,
    pub shadow_plan: GovernanceAgentValidatorSideShadowPlan,
    pub authority_transfer_live: bool,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentAdversarialProbe {
    pub name: String,
    pub category: String,
    pub rejected: bool,
    pub authority_changed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentGate15Report {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub ruleset_file: String,
    pub gate_9_5_report_file: String,
    pub receipt_report_file: String,
    pub gate_14_report_file: String,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub receipt_probe_count: usize,
    pub evidence_probe_count: usize,
    pub disagreement_probe_count: usize,
    pub authority_probe_count: usize,
    pub tampered_receipts_rejected: bool,
    pub stale_or_missing_evidence_rejected: bool,
    pub verifier_disagreement_shadow_only: bool,
    pub authority_transfer_guarded: bool,
    pub tp_greater_than_one_still_inadmissible: bool,
    pub sidecars_live: bool,
    pub commit_reveal_live: bool,
    pub authority_transfer_live: bool,
    pub probes: Vec<GovernanceAgentAdversarialProbe>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentEvidenceLineageAuditItem {
    pub name: String,
    pub path: String,
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub lineage_fields_present: bool,
    pub hashes_well_formed: bool,
    pub schema_matches_expected: bool,
    pub gate_matches_expected: bool,
    pub matches_model_request: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentEvidenceLineageAuditReport {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub model_request_file: String,
    pub model_request_hash: String,
    pub model_request_hash_recomputed: bool,
    pub validator_evidence_packet_schema_hash: String,
    pub validator_evidence_field_registry_hash: String,
    pub report_count: usize,
    pub drift_count: usize,
    pub all_reports_verified: bool,
    pub no_network_access: bool,
    pub no_model_access: bool,
    pub no_filesystem_mutation: bool,
    pub no_direct_state_mutation: bool,
    pub zero_cobalt_registry_mutation: bool,
    pub reports: Vec<GovernanceAgentEvidenceLineageAuditItem>,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentImplementationSurfaceExpansion {
    pub surface: String,
    pub reason: String,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentImplementationLiveActionFlags {
    pub registry_mutation: bool,
    pub cobalt_amendment_submission: bool,
    pub provider_start_stop_delete: bool,
    pub paid_replay_regeneration: bool,
    pub validator_side_sidecar_activation: bool,
    pub commit_reveal_activation: bool,
    pub tensor_parallelism_greater_than_one: bool,
    pub authority_transfer: bool,
    pub secret_capture: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentImplementationRequiredGate {
    pub name: String,
    pub command: String,
    pub required_before_live_mutation: bool,
    pub no_spend: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernanceAgentImplementationWorkItem {
    pub schema: String,
    pub work_item_id: String,
    pub work_package_id: String,
    pub authorization_plan_id: String,
    pub queue_decision_id: String,
    pub source_authorization_doc: String,
    pub scope: String,
    pub exact_targets: Vec<String>,
    pub allowed_surfaces: Vec<String>,
    pub touched_surfaces: Vec<String>,
    pub authorized_surface_expansions: Vec<GovernanceAgentImplementationSurfaceExpansion>,
    pub forbidden_actions: Vec<String>,
    pub live_action_flags: GovernanceAgentImplementationLiveActionFlags,
    pub required_gates: Vec<GovernanceAgentImplementationRequiredGate>,
    pub rollback_or_noop_fallback: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceAgentImplementationExecutionReport {
    pub schema: String,
    pub gate: String,
    pub verified: bool,
    pub work_item_file: String,
    pub work_item_id: String,
    pub work_package_id: String,
    pub authorization_plan_id: String,
    pub queue_decision_id: String,
    pub work_item_hash: String,
    pub allowed_surface_count: usize,
    pub touched_surface_count: usize,
    pub authorized_surface_expansion_count: usize,
    pub touched_surfaces_authorized: bool,
    pub forbidden_actions_bound: bool,
    pub live_actions_forbidden: bool,
    pub required_gates_bound: bool,
    pub rollback_or_noop_fallback_defined: bool,
    pub provider_spend_command_executed: bool,
    pub paid_replay_regenerated: bool,
    pub live_authority_change: bool,
    pub no_spend: bool,
    pub redaction_checked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceRuleset {
    schema: String,
    ruleset_id: String,
    scope: String,
    authority: GovernanceRulesetAuthority,
    inputs: Vec<GovernanceRulesetInput>,
    decisions: Vec<GovernanceRulesetDecision>,
    rollback: GovernanceRulesetRollback,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceRulesetAuthority {
    mode: String,
    direct_state_mutation: bool,
    self_upgrade: bool,
    scope_expansion: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceRulesetInput {
    kind: String,
    required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceRulesetDecision {
    decision_id: String,
    kind: String,
    condition: String,
    evidence_field_path: String,
    required_provenance: String,
    freshness_requirement: String,
    missing_evidence_behavior: String,
    conflict_behavior: String,
    action_bound: String,
    rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct GovernanceRulesetRollback {
    required: bool,
    max_mutations: u64,
    operator_confirmation_required: bool,
}


include!("governance_agent_parts/gate_entrypoints.rs");
include!("governance_agent_parts/implementation_guarded_apply.rs");
include!("governance_agent_parts/verifier_receipts_adversarial.rs");
include!("governance_agent_parts/ruleset_hashing_io.rs");

#[cfg(test)]
mod governance_agent_tests {
    include!("governance_agent_parts/tests.rs");
}
