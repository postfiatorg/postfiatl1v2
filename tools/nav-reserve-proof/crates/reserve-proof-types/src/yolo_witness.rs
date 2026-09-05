//! Versioned private inputs and independently supplied public acceptance pins.

use crate::yolo_cbor::{decode_strict_cbor, CborLimits};
use crate::yolo_collection::{
    domain_sha256, parse_date, parse_python_utc, validate_digest, YoloCollectionEpochV2,
    YoloSnapshotCommitmentV2,
};
use crate::yolo_target::{YoloPortfolioParametersV1, YoloPortfolioTargetInputV1};
use postfiat_types::YoloTargetPublicValuesV1;
use serde::{Deserialize, Serialize};

pub const TARGET_WITNESS_SCHEMA: &str = "postfiat.yolo.target_proof_witness.v1";
pub const TARGET_MANIFEST_SCHEMA: &str = "postfiat.yolo.target_collection_manifest.v1";
pub const NITRO_POLICY_SCHEMA: &str = "postfiat.yolo.nitro_proof_policy.v1";
pub const STATEMENT_SCHEMA: &str = "postfiat.yolo.attested_statement.v1";
pub const EVIDENCE_SCHEMA: &str = "postfiat.yolo.tee_evidence.v1";
pub const BINDING_SCHEMA: &str = "postfiat.yolo.statement_attestation_binding.v1";
pub const MAX_TARGET_WITNESS_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_STATEMENTS: usize = 65;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PcrMeasurementV1 {
    pub index: u8,
    pub sha384: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttestationTimeRuleV1 {
    EpochWindowEnd,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StatementRuleV1 {
    CollectionAndTargetInputV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NitroProofPolicyV1 {
    pub schema: String,
    pub root_certificate_sha256: String,
    pub approved_pcr_sets: Vec<Vec<PcrMeasurementV1>>,
    pub max_age_ms: u64,
    pub max_future_skew_ms: u64,
    pub time_rule: AttestationTimeRuleV1,
    pub statement_rule: StatementRuleV1,
    pub verifier_id: String,
}

pub(crate) fn bounded_text(name: &str, value: &str, maximum: usize) -> Result<(), String> {
    if value.is_empty() || value.len() > maximum || value.contains('\0') {
        return Err(format!("{name} exceeds its text bounds"));
    }
    Ok(())
}

pub(crate) fn decode_hex(name: &str, value: &str, maximum: usize) -> Result<Vec<u8>, String> {
    if value.is_empty()
        || value.len() % 2 != 0
        || value.len() / 2 > maximum
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!("{name} is not bounded lowercase hexadecimal"));
    }
    hex::decode(value).map_err(|_| format!("{name} is invalid hex"))
}

impl NitroProofPolicyV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != NITRO_POLICY_SCHEMA {
            return Err("Nitro proof policy schema mismatch".into());
        }
        validate_digest("root_certificate_sha256", &self.root_certificate_sha256)?;
        bounded_text("verifier_id", &self.verifier_id, 256)?;
        if self.max_age_ms == 0
            || self.max_age_ms > i64::MAX as u64
            || self.max_future_skew_ms > i64::MAX as u64
        {
            return Err("Nitro time allowances exceed their integer bounds".into());
        }
        if self.approved_pcr_sets.is_empty() || self.approved_pcr_sets.len() > 32 {
            return Err("approved PCR set count exceeds its bounds".into());
        }
        let mut previous_set: Option<Vec<(u8, &str)>> = None;
        for set in &self.approved_pcr_sets {
            if set.is_empty() || set.len() > 32 || set[0].index != 0 {
                return Err("PCR policy must include PCR0 within its bounds".into());
            }
            let mut previous_index = None;
            for pcr in set {
                if pcr.index >= 32 || previous_index.is_some_and(|previous| pcr.index <= previous) {
                    return Err("PCR indices must be bounded, sorted and unique".into());
                }
                let digest = decode_hex("PCR SHA-384", &pcr.sha384, 48)?;
                if digest.len() != 48 || digest.iter().all(|byte| *byte == 0) {
                    return Err("approved PCR must be a nonzero SHA-384 measurement".into());
                }
                previous_index = Some(pcr.index);
            }
            let current: Vec<_> = set
                .iter()
                .map(|pcr| (pcr.index, pcr.sha384.as_str()))
                .collect();
            if previous_set
                .as_ref()
                .is_some_and(|previous| previous >= &current)
            {
                return Err("approved PCR sets must be sorted and unique".into());
            }
            previous_set = Some(current);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetCollectionManifestV1 {
    pub schema: String,
    pub run_id: String,
    pub series_id: String,
    pub underlier_id: String,
    pub trade_date: String,
    pub parameter_manifest_sha256: String,
    pub normalization_sources_sha256: String,
    pub epoch: YoloCollectionEpochV2,
    pub proof_policy: NitroProofPolicyV1,
}

impl TargetCollectionManifestV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != TARGET_MANIFEST_SCHEMA {
            return Err("target collection manifest schema mismatch".into());
        }
        for (name, value) in [
            ("run_id", &self.run_id),
            ("series_id", &self.series_id),
            ("underlier_id", &self.underlier_id),
        ] {
            bounded_text(name, value, 256)?;
        }
        parse_date("trade_date", &self.trade_date)?;
        validate_digest("parameter_manifest_sha256", &self.parameter_manifest_sha256)?;
        validate_digest(
            "normalization_sources_sha256",
            &self.normalization_sources_sha256,
        )?;
        self.proof_policy.validate()?;
        self.verification_time_ms()?;
        Ok(())
    }

    pub fn sha256(&self) -> Result<String, String> {
        self.validate()?;
        domain_sha256(TARGET_MANIFEST_SCHEMA, self)
    }

    pub fn verification_time_ms(&self) -> Result<u64, String> {
        match self.proof_policy.time_rule {
            AttestationTimeRuleV1::EpochWindowEnd => {
                let end = parse_python_utc("epoch.window_end_utc", &self.epoch.window_end_utc)?;
                u64::try_from(end.timestamp_millis())
                    .map_err(|_| "verification time precedes Unix epoch".into())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeeEvidenceV1 {
    pub schema: String,
    pub platform: String,
    pub trust_class: String,
    pub hardware_backed: bool,
    pub debug_mode: bool,
    pub measurement: String,
    pub statement_key: String,
    pub attestation_document_sha256: String,
    pub verifier_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AttestedStatementV1 {
    pub schema: String,
    pub statement_kind: String,
    pub epoch_sha256: String,
    pub input_sha256: String,
    pub output_sha256: String,
    pub sequence: u32,
    pub previous_statement_sha256: Option<String>,
    pub signature: String,
    pub evidence: TeeEvidenceV1,
}

impl AttestedStatementV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != STATEMENT_SCHEMA || self.evidence.schema != EVIDENCE_SCHEMA {
            return Err("statement or evidence schema mismatch".into());
        }
        for (name, value) in [
            ("epoch_sha256", &self.epoch_sha256),
            ("input_sha256", &self.input_sha256),
            ("output_sha256", &self.output_sha256),
            ("statement_key", &self.evidence.statement_key),
            (
                "attestation_document_sha256",
                &self.evidence.attestation_document_sha256,
            ),
        ] {
            validate_digest(name, value)?;
        }
        if let Some(previous) = &self.previous_statement_sha256 {
            validate_digest("previous_statement_sha256", previous)?;
        }
        bounded_text("statement_kind", &self.statement_kind, 64)?;
        bounded_text("measurement", &self.evidence.measurement, 101)?;
        bounded_text("verifier_id", &self.evidence.verifier_id, 256)?;
        bounded_text("signature", &self.signature, 88)?;
        if self.evidence.platform != "aws-nitro"
            || self.evidence.trust_class != "attested"
            || !self.evidence.hardware_backed
            || self.evidence.debug_mode
            || self.sequence >= MAX_STATEMENTS as u32
        {
            return Err("statement does not claim bounded non-debug Nitro evidence".into());
        }
        Ok(())
    }

    fn unsigned_value(&self) -> Result<serde_json::Value, String> {
        let mut value = serde_json::to_value(self).map_err(|_| "statement serialization failed")?;
        value
            .as_object_mut()
            .ok_or("statement must be an object")?
            .remove("signature");
        Ok(value)
    }

    pub fn attestation_binding_digest(&self) -> Result<String, String> {
        let mut value = self.unsigned_value()?;
        value["schema"] = BINDING_SCHEMA.into();
        value["evidence"]
            .as_object_mut()
            .ok_or("evidence must be an object")?
            .remove("attestationDocumentSha256");
        domain_sha256(BINDING_SCHEMA, &value)
    }

    pub fn signing_digest(&self) -> Result<String, String> {
        domain_sha256(STATEMENT_SCHEMA, &self.unsigned_value()?)
    }
    pub fn sha256(&self) -> Result<String, String> {
        domain_sha256(STATEMENT_SCHEMA, self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetProofWitnessV1 {
    pub schema: String,
    pub program_sha256: String,
    pub manifest: TargetCollectionManifestV1,
    pub parameters: YoloPortfolioParametersV1,
    pub commitments: Vec<YoloSnapshotCommitmentV2>,
    pub target_input: YoloPortfolioTargetInputV1,
    pub statements: Vec<AttestedStatementV1>,
    pub attestation_documents_hex: Vec<String>,
    pub root_certificate_der_hex: String,
}

impl TargetProofWitnessV1 {
    pub fn validate_bounds(&self) -> Result<(), String> {
        if self.schema != TARGET_WITNESS_SCHEMA {
            return Err("target witness schema mismatch".into());
        }
        validate_digest("program_sha256", &self.program_sha256)?;
        self.manifest.validate()?;
        self.parameters.validate()?;
        if self.commitments.is_empty()
            || self.commitments.len() > 64
            || self.statements.len() != self.commitments.len() + 1
            || self.attestation_documents_hex.len() != self.statements.len()
            || self.target_input.snapshots.len() > 64
            || self.target_input.positions.len() > 65_536
            || self.target_input.incumbent_symbols.len() > 65_536
        {
            return Err("target witness collection or state count exceeds its bounds".into());
        }
        for snapshot in &self.target_input.snapshots {
            if snapshot.contracts.len() > 65_536 {
                return Err("normalized contract count exceeds its bound".into());
            }
        }
        for statement in &self.statements {
            statement.validate()?;
        }
        for document in &self.attestation_documents_hex {
            decode_hex("Nitro document", document, 64 * 1024)?;
        }
        decode_hex("root certificate", &self.root_certificate_der_hex, 1024)?;
        Ok(())
    }
}

pub fn decode_target_witness(input: &[u8]) -> Result<TargetProofWitnessV1, String> {
    let value = decode_strict_cbor(
        input,
        CborLimits {
            bytes: MAX_TARGET_WITNESS_BYTES,
            items: 2_000_000,
            depth: 32,
        },
    )?;
    if !matches!(value, serde_cbor::Value::Map(_)) {
        return Err("target witness must be a CBOR map".into());
    }
    // The strict pass has already rejected duplicates, non-minimal heads and
    // all size/depth violations. Decode the original bytes directly instead
    // of serializing the entire generic Value back into a second CBOR buffer.
    let witness: TargetProofWitnessV1 = serde_cbor::from_slice(input)
        .map_err(|_| "target witness fields or types do not match")?;
    witness.validate_bounds()?;
    Ok(witness)
}

/// These pins come from the operator's approved public run record, never from
/// values copied out of an untrusted private witness or proof sidecar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TargetAcceptanceV1 {
    pub program_sha256: String,
    pub collection_manifest_sha256: String,
}

impl TargetAcceptanceV1 {
    pub fn verify(&self, public: &YoloTargetPublicValuesV1) -> Result<(), String> {
        validate_digest("expected program", &self.program_sha256)?;
        validate_digest("expected manifest", &self.collection_manifest_sha256)?;
        public.validate()?;
        if self.program_sha256 != public.program_sha256
            || self.collection_manifest_sha256 != public.collection_manifest_sha256
        {
            return Err(
                "public program or collection manifest differs from independently approved pins"
                    .into(),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> NitroProofPolicyV1 {
        NitroProofPolicyV1 {
            schema: NITRO_POLICY_SCHEMA.into(),
            root_certificate_sha256: "01".repeat(32),
            approved_pcr_sets: vec![vec![PcrMeasurementV1 {
                index: 0,
                sha384: "02".repeat(48),
            }]],
            max_age_ms: 60_000,
            max_future_skew_ms: 1000,
            time_rule: AttestationTimeRuleV1::EpochWindowEnd,
            statement_rule: StatementRuleV1::CollectionAndTargetInputV1,
            verifier_id: "synthetic-only".into(),
        }
    }

    #[test]
    fn policy_serialization_is_explicit_and_rejects_unknown_or_missing_rules() {
        let policy = policy();
        policy.validate().unwrap();
        let encoded = serde_cbor::to_vec(&policy).unwrap();
        let decoded: NitroProofPolicyV1 = serde_cbor::value::from_value(
            decode_strict_cbor(&encoded, crate::yolo_cbor::NITRO_CBOR_LIMITS).unwrap(),
        )
        .unwrap();
        assert_eq!(policy, decoded);
        let original = serde_json::to_value(&policy).unwrap();
        for field in original.as_object().unwrap().keys() {
            let mut missing = original.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                serde_json::from_value::<NitroProofPolicyV1>(missing).is_err(),
                "defaulted {field}"
            );
        }
        for field in ["now", "timeRule", "statementRule", "schema"] {
            let mut unknown = original.clone();
            unknown[field] = "unapproved".into();
            match serde_json::from_value::<NitroProofPolicyV1>(unknown) {
                Ok(value) => assert!(value.validate().is_err()),
                Err(_) => {}
            }
        }
    }

    #[test]
    fn policy_rejects_debug_duplicate_and_unbounded_measurements() {
        let mut invalid = policy();
        invalid.approved_pcr_sets[0][0].sha384 = "00".repeat(48);
        assert!(invalid.validate().is_err());
        let mut invalid = policy();
        invalid
            .approved_pcr_sets
            .push(invalid.approved_pcr_sets[0].clone());
        assert!(invalid.validate().is_err());
        let mut invalid = policy();
        let duplicate = invalid.approved_pcr_sets[0][0].clone();
        invalid.approved_pcr_sets[0].push(duplicate);
        assert!(invalid.validate().is_err());
        let mut invalid = policy();
        invalid.max_age_ms = 0;
        assert!(invalid.validate().is_err());
        let mut invalid = policy();
        invalid.max_future_skew_ms = u64::MAX;
        assert!(invalid.validate().is_err());
        assert!(decode_target_witness(&[0xa0]).is_err());
        assert!(decode_target_witness(&[0xd2, 0xa0]).is_err());
    }
}
