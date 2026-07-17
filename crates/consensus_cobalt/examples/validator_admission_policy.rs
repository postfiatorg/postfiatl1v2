use postfiat_consensus_cobalt::*;

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn control_group(
    validator_id: &str,
    operator_group: &str,
    release_manager_group: &str,
    key_management_group: &str,
    funding_source_group: &str,
) -> ValidatorAdmissionControlGroup {
    ValidatorAdmissionControlGroup {
        validator_id: validator_id.to_string(),
        operator_group: operator_group.to_string(),
        release_manager_group: release_manager_group.to_string(),
        key_management_group: key_management_group.to_string(),
        funding_source_group: funding_source_group.to_string(),
    }
}

fn evidence_ref(field_id: &str) -> ValidatorAdmissionEvidenceRef {
    ValidatorAdmissionEvidenceRef {
        field_id: field_id.to_string(),
        source_hash: root('e'),
        missing: false,
        stale: false,
        conflicting: false,
    }
}

fn clean_packet() -> ValidatorAdmissionEvidencePacket {
    let mut evidence_refs = vec![
        evidence_ref(VALIDATOR_ADMISSION_FIELD_ACCOUNTABILITY),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_RHO),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_COBALT_LINKEDNESS),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_MODEL_CLASSIFICATION),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_OPERATOR_MANIFEST),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_RELIABILITY),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP),
        evidence_ref(VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER),
    ];
    evidence_refs.sort_by(|left, right| left.field_id.cmp(&right.field_id));
    ValidatorAdmissionEvidencePacket {
        packet_id: "admission-packet-clean".to_string(),
        registry_root: root('a'),
        candidate: ValidatorAdmissionCandidateEvidence {
            validator_id: "validator-new".to_string(),
            public_key_hash: root('b'),
            reliability_bps: Some(9_980),
            accountability_score: Some(85),
            rho_score: Some(0),
            operator_manifest_signed: Some(true),
            domain_control_proved: Some(true),
            cobalt_linkedness_safe: Some(true),
            control_group: control_group(
                "validator-new",
                "operator-new",
                "release-new",
                "kms-new",
                "funding-new",
            ),
        },
        active_validators: vec![
            control_group(
                "validator-0",
                "operator-0",
                "release-0",
                "kms-0",
                "funding-0",
            ),
            control_group(
                "validator-1",
                "operator-1",
                "release-1",
                "kms-1",
                "funding-1",
            ),
        ],
        evidence_refs,
        model_output: Some(ValidatorAdmissionModelOutput {
            classification: "independent".to_string(),
            cited_fields: vec![
                VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL.to_string(),
                VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE.to_string(),
                VALIDATOR_ADMISSION_FIELD_KEY_MANAGEMENT.to_string(),
                VALIDATOR_ADMISSION_FIELD_OPERATOR_GROUP.to_string(),
                VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER.to_string(),
            ],
            parsed_output_root: root('c'),
            replay_certificate_root: root('d'),
        }),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let domain = CobaltDomain {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: root('0'),
        protocol_version: 1,
    };
    let policy = ValidatorAdmissionPolicy::controlled_testnet_v1();

    let clean = clean_packet();

    let mut shared_control = clean_packet();
    shared_control.packet_id = "admission-packet-shared-control".to_string();
    shared_control.candidate.control_group.release_manager_group = "release-1".to_string();
    shared_control.candidate.control_group.key_management_group = "kms-1".to_string();

    let mut missing_domain = clean_packet();
    missing_domain.packet_id = "admission-packet-missing-domain".to_string();
    missing_domain.candidate.domain_control_proved = None;
    for reference in &mut missing_domain.evidence_refs {
        if reference.field_id == VALIDATOR_ADMISSION_FIELD_DOMAIN_CONTROL {
            reference.missing = true;
        }
    }

    let mut contradictory = clean_packet();
    contradictory.packet_id = "admission-packet-contradictory".to_string();
    for reference in &mut contradictory.evidence_refs {
        if reference.field_id == VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER
            || reference.field_id == VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE
        {
            reference.conflicting = true;
        }
    }
    contradictory.model_output = Some(ValidatorAdmissionModelOutput {
        classification: "contradictory".to_string(),
        cited_fields: vec![
            VALIDATOR_ADMISSION_FIELD_FUNDING_SOURCE.to_string(),
            VALIDATOR_ADMISSION_FIELD_RELEASE_MANAGER.to_string(),
        ],
        parsed_output_root: root('c'),
        replay_certificate_root: root('d'),
    });

    let mut unknown_model_field = clean_packet();
    unknown_model_field.packet_id = "admission-packet-unknown-model-field".to_string();
    unknown_model_field.model_output = Some(ValidatorAdmissionModelOutput {
        classification: "independent".to_string(),
        cited_fields: vec!["validator.private.kyc_status".to_string()],
        parsed_output_root: root('c'),
        replay_certificate_root: root('d'),
    });

    let report = build_validator_admission_report(
        &domain,
        &policy,
        vec![
            ("clean-independent-admit".to_string(), clean),
            ("shared-control-reject".to_string(), shared_control),
            ("missing-domain-hold".to_string(), missing_domain),
            ("contradictory-evidence-hold".to_string(), contradictory),
            ("unknown-model-field-hold".to_string(), unknown_model_field),
        ],
    )?;
    emit_example_report(&report)?;
    Ok(())
}
