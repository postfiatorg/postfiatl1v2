pub const VALIDATOR_ADMISSION_POLICY_SCHEMA: &str = "postfiat.validator_admission_policy.v1";
pub const VALIDATOR_ADMISSION_DECISION_SCHEMA: &str = "postfiat.validator_admission_decision.v1";
pub const VALIDATOR_ADMISSION_REPORT_SCHEMA: &str = "postfiat.validator_admission_report.v1";

pub const VALIDATOR_ADMISSION_ACTION_ADMIT: &str = "admit";
pub const VALIDATOR_ADMISSION_ACTION_HOLD: &str = "hold";
pub const VALIDATOR_ADMISSION_ACTION_REJECT: &str = "reject";
pub const VALIDATOR_ADMISSION_DELTA_ADD: &str = "add";
pub const VALIDATOR_ADMISSION_DELTA_NO_OP: &str = "no_op";

pub const VALIDATOR_ADMISSION_FIELD_RELIABILITY: &str =
    "validator.performance.uptime_window_bps";
pub const VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY: &str =
    "validator.admission.accountability_score";
pub const VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST: &str =
    "validator.operator_manifest.signature_valid";
pub const VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL: &str =
    "validator.identity.key_domain_binding.status";
pub const VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP: &str = "validator.topology.operator_group";
pub const VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER: &str =
    "validator.topology.release_manager_group";
pub const VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT: &str =
    "validator.topology.key_management_group";
pub const VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE: &str =
    "validator.topology.funding_source_group";
pub const VALIDATOR_ADMISSION_FIELD_RHO: &str = "validator.admission.rho_score";
pub const VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS: &str =
    "validator.cobalt.linkedness_safe";
pub const VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION: &str =
    "validator.model.operator_independence_classification";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionPolicy {
    pub schema: String,
    pub policy_version: u32,
    pub min_reliability_bps: u16,
    pub min_accountability_score: u8,
    pub max_rho_score: u8,
    pub max_adds_per_round: u8,
    pub require_source_hashes: bool,
    pub require_model_classification: bool,
    pub forbid_shared_operator_group: bool,
    pub forbid_shared_release_manager: bool,
    pub forbid_shared_key_management: bool,
    pub forbid_shared_funding_source: bool,
}

impl ValidatorAdmissionPolicy {
    pub fn controlled_testnet_v1() -> Self {
        Self {
            schema: VALIDATOR_ADMISSION_POLICY_SCHEMA.to_string(),
            policy_version: 1,
            min_reliability_bps: 9_950,
            min_accountability_score: 70,
            max_rho_score: 0,
            max_adds_per_round: 1,
            require_source_hashes: true,
            require_model_classification: true,
            forbid_shared_operator_group: true,
            forbid_shared_release_manager: true,
            forbid_shared_key_management: true,
            forbid_shared_funding_source: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionEvidenceRef {
    pub field_id: String,
    pub source_hash: String,
    pub missing: bool,
    pub stale: bool,
    pub conflicting: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionControlGroup {
    pub validator_id: String,
    pub operator_group: String,
    pub release_manager_group: String,
    pub key_management_group: String,
    pub funding_source_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionCandidateEvidence {
    pub validator_id: String,
    pub public_key_hash: String,
    pub reliability_bps: Option<u16>,
    pub accountability_score: Option<u8>,
    pub rho_score: Option<u8>,
    pub operator_manifest_signed: Option<bool>,
    pub domain_control_proved: Option<bool>,
    pub cobalt_linkedness_safe: Option<bool>,
    pub control_group: ValidatorAdmissionControlGroup,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionModelOutput {
    pub classification: String,
    pub cited_fields: Vec<String>,
    pub parsed_output_root: String,
    pub replay_certificate_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionEvidencePacket {
    pub packet_id: String,
    pub registry_root: String,
    pub candidate: ValidatorAdmissionCandidateEvidence,
    pub active_validators: Vec<ValidatorAdmissionControlGroup>,
    pub evidence_refs: Vec<ValidatorAdmissionEvidenceRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_output: Option<ValidatorAdmissionModelOutput>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionRegistryDeltaCandidate {
    pub delta_kind: String,
    pub mutation_count: u8,
    pub subject_validator: String,
    pub previous_registry_root: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_record_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionDecision {
    pub schema: String,
    pub decision_id: String,
    pub policy_root: String,
    pub packet_root: String,
    pub validator_id: String,
    pub action: String,
    pub reason_codes: Vec<String>,
    pub failed_fields: Vec<String>,
    pub correlation_cluster: Vec<String>,
    pub required_followup_evidence: Vec<String>,
    pub registry_delta_candidate: ValidatorAdmissionRegistryDeltaCandidate,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionReportCase {
    pub case_id: String,
    pub packet_root: String,
    pub action: String,
    pub decision_id: String,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ValidatorAdmissionReport {
    pub schema: String,
    pub policy_root: String,
    pub cases: Vec<ValidatorAdmissionReportCase>,
    pub report_hash: String,
}

pub fn validator_admission_policy_root(
    policy: &ValidatorAdmissionPolicy,
) -> Result<String, String> {
    validate_validator_admission_policy(policy)?;
    let encoded = serde_json::to_vec(policy).map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.validator_admission_policy.root.v1", &encoded))
}

pub fn validator_admission_packet_root(
    packet: &ValidatorAdmissionEvidencePacket,
) -> Result<String, String> {
    validate_validator_admission_packet_shape(packet)?;
    let encoded = serde_json::to_vec(packet).map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.validator_admission_packet.root.v1", &encoded))
}

pub fn evaluate_validator_admission(
    domain: &CobaltDomain,
    policy: &ValidatorAdmissionPolicy,
    packet: &ValidatorAdmissionEvidencePacket,
) -> Result<ValidatorAdmissionDecision, String> {
    validate_domain(domain)?;
    validate_validator_admission_policy(policy)?;
    validate_validator_admission_packet_shape(packet)?;

    let policy_root = validator_admission_policy_root(policy)?;
    let packet_root = validator_admission_packet_root(packet)?;
    let evidence_index = validator_admission_evidence_index(packet)?;

    let mut hold_reasons = std::collections::BTreeSet::new();
    let mut reject_reasons = std::collections::BTreeSet::new();
    let mut failed_fields = std::collections::BTreeSet::new();
    let mut followup = std::collections::BTreeSet::new();
    let mut correlation_cluster =
        std::collections::BTreeSet::from([packet.candidate.validator_id.clone()]);

    for field in required_validator_admission_fields(policy) {
        match evidence_index.get(field) {
            Some(reference) if reference.missing => {
                hold_reasons.insert("missing_required_evidence".to_string());
                failed_fields.insert(field.to_string());
                followup.insert(field.to_string());
            }
            Some(reference) if reference.stale => {
                hold_reasons.insert("stale_required_evidence".to_string());
                failed_fields.insert(field.to_string());
                followup.insert(field.to_string());
            }
            Some(reference) if reference.conflicting => {
                hold_reasons.insert("conflicting_required_evidence".to_string());
                failed_fields.insert(field.to_string());
                followup.insert(field.to_string());
            }
            Some(_) => {}
            None => {
                hold_reasons.insert("missing_required_evidence".to_string());
                failed_fields.insert(field.to_string());
                followup.insert(field.to_string());
            }
        }
    }

    match packet.candidate.reliability_bps {
        Some(value) if value >= policy.min_reliability_bps => {}
        Some(_) => {
            reject_reasons.insert("reliability_below_floor".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_RELIABILITY.to_string());
        }
        None => {
            hold_reasons.insert("missing_reliability".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_RELIABILITY.to_string());
            followup.insert(VALIDATOR_ADMISSION_FIELD_RELIABILITY.to_string());
        }
    }

    match packet.candidate.accountability_score {
        Some(value) if value >= policy.min_accountability_score => {}
        Some(_) => {
            reject_reasons.insert("accountability_below_floor".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY.to_string());
        }
        None => {
            hold_reasons.insert("missing_accountability".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY.to_string());
            followup.insert(VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY.to_string());
        }
    }

    match packet.candidate.rho_score {
        Some(value) if value <= policy.max_rho_score => {}
        Some(_) => {
            reject_reasons.insert("rho_above_cap".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_RHO.to_string());
        }
        None => {
            hold_reasons.insert("missing_rho".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_RHO.to_string());
            followup.insert(VALIDATOR_ADMISSION_FIELD_RHO.to_string());
        }
    }

    require_bool_gate(
        packet.candidate.operator_manifest_signed,
        VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST,
        "missing_operator_manifest_signature",
        "operator_manifest_signature_false",
        &mut hold_reasons,
        &mut reject_reasons,
        &mut failed_fields,
        &mut followup,
    );
    require_bool_gate(
        packet.candidate.domain_control_proved,
        VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL,
        "missing_domain_control",
        "domain_control_false",
        &mut hold_reasons,
        &mut reject_reasons,
        &mut failed_fields,
        &mut followup,
    );
    require_bool_gate(
        packet.candidate.cobalt_linkedness_safe,
        VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS,
        "missing_cobalt_linkedness",
        "cobalt_linkedness_unsafe",
        &mut hold_reasons,
        &mut reject_reasons,
        &mut failed_fields,
        &mut followup,
    );

    for active in &packet.active_validators {
        if policy.forbid_shared_operator_group
            && nonempty_equal(
                &packet.candidate.control_group.operator_group,
                &active.operator_group,
            )
        {
            reject_reasons.insert("shared_operator_group".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP.to_string());
            correlation_cluster.insert(active.validator_id.clone());
        }
        if policy.forbid_shared_release_manager
            && nonempty_equal(
                &packet.candidate.control_group.release_manager_group,
                &active.release_manager_group,
            )
        {
            reject_reasons.insert("shared_release_manager".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER.to_string());
            correlation_cluster.insert(active.validator_id.clone());
        }
        if policy.forbid_shared_key_management
            && nonempty_equal(
                &packet.candidate.control_group.key_management_group,
                &active.key_management_group,
            )
        {
            reject_reasons.insert("shared_key_management".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT.to_string());
            correlation_cluster.insert(active.validator_id.clone());
        }
        if policy.forbid_shared_funding_source
            && nonempty_equal(
                &packet.candidate.control_group.funding_source_group,
                &active.funding_source_group,
            )
        {
            reject_reasons.insert("shared_funding_source".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE.to_string());
            correlation_cluster.insert(active.validator_id.clone());
        }
    }

    if policy.require_model_classification {
        evaluate_validator_admission_model_output(
            packet,
            &evidence_index,
            &mut hold_reasons,
            &mut failed_fields,
            &mut followup,
        )?;
    }

    let action = if !reject_reasons.is_empty() {
        VALIDATOR_ADMISSION_ACTION_REJECT
    } else if !hold_reasons.is_empty() {
        VALIDATOR_ADMISSION_ACTION_HOLD
    } else {
        VALIDATOR_ADMISSION_ACTION_ADMIT
    };

    let mut reason_codes = reject_reasons;
    reason_codes.extend(hold_reasons);
    if reason_codes.is_empty() {
        reason_codes.insert("all_gates_passed".to_string());
    }

    let registry_delta_candidate = validator_admission_delta_candidate(action, packet)?;
    let mut decision = ValidatorAdmissionDecision {
        schema: VALIDATOR_ADMISSION_DECISION_SCHEMA.to_string(),
        decision_id: String::new(),
        policy_root,
        packet_root,
        validator_id: packet.candidate.validator_id.clone(),
        action: action.to_string(),
        reason_codes: reason_codes.into_iter().collect(),
        failed_fields: failed_fields.into_iter().collect(),
        correlation_cluster: correlation_cluster.into_iter().collect(),
        required_followup_evidence: followup.into_iter().collect(),
        registry_delta_candidate,
    };
    decision.decision_id = validator_admission_decision_id(domain, &decision)?;
    Ok(decision)
}

pub fn build_validator_admission_report(
    domain: &CobaltDomain,
    policy: &ValidatorAdmissionPolicy,
    cases: Vec<(String, ValidatorAdmissionEvidencePacket)>,
) -> Result<ValidatorAdmissionReport, String> {
    if cases.is_empty() {
        return Err("validator admission report requires at least one case".to_string());
    }
    let policy_root = validator_admission_policy_root(policy)?;
    let mut report_cases = Vec::with_capacity(cases.len());
    let mut seen = std::collections::BTreeSet::new();
    for (case_id, packet) in cases {
        validate_node_id("validator admission report case_id", &case_id)?;
        if !seen.insert(case_id.clone()) {
            return Err("validator admission report case ids must be unique".to_string());
        }
        let decision = evaluate_validator_admission(domain, policy, &packet)?;
        report_cases.push(ValidatorAdmissionReportCase {
            case_id,
            packet_root: decision.packet_root,
            action: decision.action,
            decision_id: decision.decision_id,
            reason_codes: decision.reason_codes,
        });
    }
    report_cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let mut report = ValidatorAdmissionReport {
        schema: VALIDATOR_ADMISSION_REPORT_SCHEMA.to_string(),
        policy_root,
        cases: report_cases,
        report_hash: String::new(),
    };
    let encoded = serde_json::to_vec(&(
        report.schema.as_str(),
        report.policy_root.as_str(),
        report.cases.as_slice(),
    ))
    .map_err(|error| error.to_string())?;
    report.report_hash = hash_hex("postfiat.validator_admission_report.v1", &encoded);
    Ok(report)
}

fn validator_admission_decision_id(
    domain: &CobaltDomain,
    decision: &ValidatorAdmissionDecision,
) -> Result<String, String> {
    validate_domain(domain)?;
    let encoded = serde_json::to_vec(&(
        domain.chain_id.as_str(),
        domain.genesis_hash.as_str(),
        domain.protocol_version,
        decision.schema.as_str(),
        decision.policy_root.as_str(),
        decision.packet_root.as_str(),
        decision.validator_id.as_str(),
        decision.action.as_str(),
        decision.reason_codes.as_slice(),
        decision.failed_fields.as_slice(),
        decision.correlation_cluster.as_slice(),
        decision.required_followup_evidence.as_slice(),
        &decision.registry_delta_candidate,
    ))
    .map_err(|error| error.to_string())?;
    Ok(hash_hex("postfiat.validator_admission_decision.v1", &encoded))
}

fn validator_admission_delta_candidate(
    action: &str,
    packet: &ValidatorAdmissionEvidencePacket,
) -> Result<ValidatorAdmissionRegistryDeltaCandidate, String> {
    let candidate_record_hash = if action == VALIDATOR_ADMISSION_ACTION_ADMIT {
        let encoded = serde_json::to_vec(&(
            packet.candidate.validator_id.as_str(),
            packet.candidate.public_key_hash.as_str(),
            packet.candidate.control_group.operator_group.as_str(),
            packet
                .candidate
                .control_group
                .release_manager_group
                .as_str(),
            packet
                .candidate
                .control_group
                .key_management_group
                .as_str(),
            packet
                .candidate
                .control_group
                .funding_source_group
                .as_str(),
        ))
        .map_err(|error| error.to_string())?;
        Some(hash_hex(
            "postfiat.validator_admission_candidate_record.v1",
            &encoded,
        ))
    } else {
        None
    };
    Ok(ValidatorAdmissionRegistryDeltaCandidate {
        delta_kind: if action == VALIDATOR_ADMISSION_ACTION_ADMIT {
            VALIDATOR_ADMISSION_DELTA_ADD.to_string()
        } else {
            VALIDATOR_ADMISSION_DELTA_NO_OP.to_string()
        },
        mutation_count: if action == VALIDATOR_ADMISSION_ACTION_ADMIT {
            1
        } else {
            0
        },
        subject_validator: packet.candidate.validator_id.clone(),
        previous_registry_root: packet.registry_root.clone(),
        candidate_record_hash,
    })
}

fn evaluate_validator_admission_model_output(
    packet: &ValidatorAdmissionEvidencePacket,
    evidence_index: &std::collections::BTreeMap<&str, &ValidatorAdmissionEvidenceRef>,
    hold_reasons: &mut std::collections::BTreeSet<String>,
    failed_fields: &mut std::collections::BTreeSet<String>,
    followup: &mut std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let Some(model) = &packet.model_output else {
        hold_reasons.insert("missing_model_classification".to_string());
        failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        followup.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        return Ok(());
    };
    validate_hash_hex("validator admission model parsed output root", &model.parsed_output_root)?;
    validate_hash_hex(
        "validator admission model replay certificate root",
        &model.replay_certificate_root,
    )?;
    if model.cited_fields.is_empty() {
        hold_reasons.insert("model_cited_no_fields".to_string());
        failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
    }
    let cited = sorted_unique(&model.cited_fields);
    if cited != model.cited_fields {
        return Err("validator admission model cited fields must be sorted unique".to_string());
    }
    for field in &model.cited_fields {
        if !validator_admission_allowed_field(field) || !evidence_index.contains_key(field.as_str())
        {
            hold_reasons.insert("model_cited_unknown_field".to_string());
            failed_fields.insert(field.clone());
            followup.insert(field.clone());
        }
    }
    match model.classification.as_str() {
        "independent" => {}
        "cosmetic_diversity" => {
            hold_reasons.insert("model_classified_cosmetic_diversity".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        }
        "contradictory" => {
            hold_reasons.insert("model_classified_contradictory".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        }
        "insufficient" => {
            hold_reasons.insert("model_classified_insufficient".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        }
        _ => {
            hold_reasons.insert("model_classification_unknown".to_string());
            failed_fields.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
            followup.insert(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION.to_string());
        }
    }
    Ok(())
}

fn require_bool_gate(
    value: Option<bool>,
    field: &str,
    missing_reason: &str,
    false_reason: &str,
    hold_reasons: &mut std::collections::BTreeSet<String>,
    reject_reasons: &mut std::collections::BTreeSet<String>,
    failed_fields: &mut std::collections::BTreeSet<String>,
    followup: &mut std::collections::BTreeSet<String>,
) {
    match value {
        Some(true) => {}
        Some(false) => {
            reject_reasons.insert(false_reason.to_string());
            failed_fields.insert(field.to_string());
        }
        None => {
            hold_reasons.insert(missing_reason.to_string());
            failed_fields.insert(field.to_string());
            followup.insert(field.to_string());
        }
    }
}

fn validator_admission_evidence_index(
    packet: &ValidatorAdmissionEvidencePacket,
) -> Result<std::collections::BTreeMap<&str, &ValidatorAdmissionEvidenceRef>, String> {
    let mut by_field = std::collections::BTreeMap::new();
    for reference in &packet.evidence_refs {
        if by_field
            .insert(reference.field_id.as_str(), reference)
            .is_some()
        {
            return Err("validator admission evidence refs must be unique by field".to_string());
        }
    }
    Ok(by_field)
}

fn required_validator_admission_fields(policy: &ValidatorAdmissionPolicy) -> Vec<&'static str> {
    let mut fields = vec![
        VALIDATOR_ADMISSION_FIELD_RELIABILITY,
        VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY,
        VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST,
        VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL,
        VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP,
        VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER,
        VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT,
        VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE,
        VALIDATOR_ADMISSION_FIELD_RHO,
        VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS,
    ];
    if policy.require_model_classification {
        fields.push(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION);
    }
    fields
}

fn validate_validator_admission_policy(policy: &ValidatorAdmissionPolicy) -> Result<(), String> {
    if policy.schema != VALIDATOR_ADMISSION_POLICY_SCHEMA {
        return Err("validator admission policy schema mismatch".to_string());
    }
    if policy.policy_version == 0 {
        return Err("validator admission policy version must be nonzero".to_string());
    }
    if policy.min_reliability_bps > 10_000 {
        return Err("validator admission reliability floor exceeds 10000 bps".to_string());
    }
    if policy.min_accountability_score > 100 {
        return Err("validator admission accountability floor exceeds 100".to_string());
    }
    if policy.max_rho_score > 100 {
        return Err("validator admission rho cap exceeds 100".to_string());
    }
    if policy.max_adds_per_round == 0 {
        return Err("validator admission policy must allow at least one add per round".to_string());
    }
    Ok(())
}

fn validate_validator_admission_packet_shape(
    packet: &ValidatorAdmissionEvidencePacket,
) -> Result<(), String> {
    validate_node_id("validator admission packet id", &packet.packet_id)?;
    validate_hash_hex("validator admission packet registry root", &packet.registry_root)?;
    validate_validator_admission_candidate(&packet.candidate)?;
    if packet
        .active_validators
        .iter()
        .any(|validator| validator.validator_id == packet.candidate.validator_id)
    {
        return Err("validator admission candidate already appears active".to_string());
    }
    let active_ids: Vec<String> = packet
        .active_validators
        .iter()
        .map(|validator| validator.validator_id.clone())
        .collect();
    if sorted_unique(&active_ids) != active_ids {
        return Err("validator admission active validators must be sorted unique".to_string());
    }
    for active in &packet.active_validators {
        validate_validator_admission_control_group("active", active)?;
    }
    if packet.evidence_refs.is_empty() {
        return Err("validator admission evidence refs must be nonempty".to_string());
    }
    let mut previous_field: Option<&str> = None;
    for reference in &packet.evidence_refs {
        if !validator_admission_allowed_field(&reference.field_id) {
            return Err(format!(
                "validator admission evidence field `{}` is not registered for policy v1",
                reference.field_id
            ));
        }
        validate_hash_hex("validator admission evidence source hash", &reference.source_hash)?;
        if let Some(previous) = previous_field {
            if previous >= reference.field_id.as_str() {
                return Err(
                    "validator admission evidence refs must be sorted unique by field".to_string(),
                );
            }
        }
        previous_field = Some(reference.field_id.as_str());
    }
    Ok(())
}

fn validate_validator_admission_candidate(
    candidate: &ValidatorAdmissionCandidateEvidence,
) -> Result<(), String> {
    validate_node_id("validator admission candidate id", &candidate.validator_id)?;
    validate_hash_hex(
        "validator admission candidate public key hash",
        &candidate.public_key_hash,
    )?;
    if let Some(value) = candidate.reliability_bps {
        if value > 10_000 {
            return Err("validator admission candidate reliability exceeds 10000 bps".to_string());
        }
    }
    if let Some(value) = candidate.accountability_score {
        if value > 100 {
            return Err(
                "validator admission candidate accountability score exceeds 100".to_string(),
            );
        }
    }
    if let Some(value) = candidate.rho_score {
        if value > 100 {
            return Err("validator admission candidate rho score exceeds 100".to_string());
        }
    }
    if candidate.control_group.validator_id != candidate.validator_id {
        return Err("validator admission candidate control group id mismatch".to_string());
    }
    validate_validator_admission_control_group("candidate", &candidate.control_group)
}

fn validate_validator_admission_control_group(
    label: &str,
    control: &ValidatorAdmissionControlGroup,
) -> Result<(), String> {
    validate_node_id(&format!("validator admission {label} control id"), &control.validator_id)?;
    validate_node_id(
        &format!("validator admission {label} operator group"),
        &control.operator_group,
    )?;
    validate_node_id(
        &format!("validator admission {label} release manager group"),
        &control.release_manager_group,
    )?;
    validate_node_id(
        &format!("validator admission {label} key management group"),
        &control.key_management_group,
    )?;
    validate_node_id(
        &format!("validator admission {label} funding source group"),
        &control.funding_source_group,
    )
}

fn validator_admission_allowed_field(field: &str) -> bool {
    matches!(
        field,
        VALIDATOR_ADMISSION_FIELD_RELIABILITY
            | VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY
            | VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST
            | VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL
            | VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP
            | VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER
            | VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT
            | VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE
            | VALIDATOR_ADMISSION_FIELD_RHO
            | VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS
            | VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION
    )
}

fn nonempty_equal(left: &str, right: &str) -> bool {
    !left.trim().is_empty() && left == right
}
