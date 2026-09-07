use postfiat_types::{YoloCollectionPublicValuesV1, YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES};

/// This verifier kind is intentionally distinct from every NAV reserve or
/// mint-authorizing proof kind.
pub const YOLO_COLLECTION_SP1_GROTH16_VERIFIER_V1: &str = "postfiat-yolo-collection-sp1-groth16-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoloCollectionSp1VerifierConfig<'a> {
    pub verifier_kind: &'a str,
    pub sp1_program_vkey: &'a str,
    pub max_proof_bytes: u64,
    pub max_public_values_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YoloCollectionVerifyContext<'a> {
    pub program_sha256: &'a str,
    pub epoch_sha256: &'a str,
    pub methodology_sha256: &'a str,
    pub collector_code_sha256: &'a str,
    pub source_id_sha256: &'a str,
    pub account_application_identity_sha256: &'a str,
    pub commitments_sha256: &'a str,
    /// Produced only after the separate Nitro evidence verifier accepts every
    /// statement in this collection.
    pub attested_collection_sha256: &'a str,
    pub normalized_input_root_sha256: &'a str,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YoloCollectionVerifyError {
    InvalidVerifierConfig,
    CryptographicProofInvalid,
    PublicValuesDecode,
    ProgramMismatch,
    EpochMismatch,
    MethodologyMismatch,
    CollectorMismatch,
    SourceMismatch,
    AccountApplicationMismatch,
    CommitmentsMismatch,
    AttestationMismatch,
    NormalizedInputMismatch,
    SnapshotCountMismatch,
}

/// Verify a collection proof and bind every disclosed digest to explicit
/// caller expectations. Success records data provenance only: it does not
/// establish reserves, value an index, authorize issuance, or authorize trades.
pub fn verify_yolo_collection_sp1_groth16(
    config: &YoloCollectionSp1VerifierConfig<'_>,
    context: &YoloCollectionVerifyContext<'_>,
    sp1_proof_bytes: &[u8],
    sp1_public_values: &[u8],
) -> Result<YoloCollectionPublicValuesV1, YoloCollectionVerifyError> {
    if config.max_proof_bytes == 0
        || config.max_public_values_bytes != YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES as u64
    {
        return Err(YoloCollectionVerifyError::InvalidVerifierConfig);
    }
    verify_bounded_sp1_groth16_with_config(
        config.verifier_kind,
        YOLO_COLLECTION_SP1_GROTH16_VERIFIER_V1,
        config.sp1_program_vkey,
        config.max_proof_bytes,
        config.max_public_values_bytes,
        sp1_proof_bytes,
        sp1_public_values,
    )
    .map_err(|_| YoloCollectionVerifyError::CryptographicProofInvalid)?;
    let values = YoloCollectionPublicValuesV1::decode(sp1_public_values)
        .map_err(|_| YoloCollectionVerifyError::PublicValuesDecode)?;
    validate_yolo_collection_public_values_context(&values, context)?;
    Ok(values)
}

/// Context validation is separate so all bindings can be regression-tested
/// without manufacturing a Groth16 proof for each mismatch.
pub fn validate_yolo_collection_public_values_context(
    values: &YoloCollectionPublicValuesV1,
    context: &YoloCollectionVerifyContext<'_>,
) -> Result<(), YoloCollectionVerifyError> {
    values
        .validate()
        .map_err(|_| YoloCollectionVerifyError::PublicValuesDecode)?;
    for (actual, expected, error) in [
        (
            values.program_sha256.as_str(),
            context.program_sha256,
            YoloCollectionVerifyError::ProgramMismatch,
        ),
        (
            values.epoch_sha256.as_str(),
            context.epoch_sha256,
            YoloCollectionVerifyError::EpochMismatch,
        ),
        (
            values.methodology_sha256.as_str(),
            context.methodology_sha256,
            YoloCollectionVerifyError::MethodologyMismatch,
        ),
        (
            values.collector_code_sha256.as_str(),
            context.collector_code_sha256,
            YoloCollectionVerifyError::CollectorMismatch,
        ),
        (
            values.source_id_sha256.as_str(),
            context.source_id_sha256,
            YoloCollectionVerifyError::SourceMismatch,
        ),
        (
            values.account_application_identity_sha256.as_str(),
            context.account_application_identity_sha256,
            YoloCollectionVerifyError::AccountApplicationMismatch,
        ),
        (
            values.commitments_sha256.as_str(),
            context.commitments_sha256,
            YoloCollectionVerifyError::CommitmentsMismatch,
        ),
        (
            values.attested_collection_sha256.as_str(),
            context.attested_collection_sha256,
            YoloCollectionVerifyError::AttestationMismatch,
        ),
        (
            values.normalized_input_root_sha256.as_str(),
            context.normalized_input_root_sha256,
            YoloCollectionVerifyError::NormalizedInputMismatch,
        ),
    ] {
        if actual != expected {
            return Err(error);
        }
    }
    if values.snapshot_count != context.snapshot_count {
        return Err(YoloCollectionVerifyError::SnapshotCountMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod yolo_collection_verifier_tests {
    use super::*;
    use postfiat_types::YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1;

    fn values() -> YoloCollectionPublicValuesV1 {
        YoloCollectionPublicValuesV1 {
            schema: YOLO_COLLECTION_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            program_sha256: "01".repeat(32),
            epoch_sha256: "02".repeat(32),
            methodology_sha256: "03".repeat(32),
            collector_code_sha256: "04".repeat(32),
            source_id_sha256: "05".repeat(32),
            account_application_identity_sha256: "06".repeat(32),
            commitments_sha256: "07".repeat(32),
            attested_collection_sha256: "08".repeat(32),
            normalized_input_root_sha256: "09".repeat(32),
            snapshot_count: 5,
        }
    }

    fn context<'a>(values: &'a YoloCollectionPublicValuesV1) -> YoloCollectionVerifyContext<'a> {
        YoloCollectionVerifyContext {
            program_sha256: &values.program_sha256,
            epoch_sha256: &values.epoch_sha256,
            methodology_sha256: &values.methodology_sha256,
            collector_code_sha256: &values.collector_code_sha256,
            source_id_sha256: &values.source_id_sha256,
            account_application_identity_sha256: &values.account_application_identity_sha256,
            commitments_sha256: &values.commitments_sha256,
            attested_collection_sha256: &values.attested_collection_sha256,
            normalized_input_root_sha256: &values.normalized_input_root_sha256,
            snapshot_count: values.snapshot_count,
        }
    }

    #[test]
    fn collection_context_binds_every_public_value() {
        let values = values();
        assert_eq!(
            validate_yolo_collection_public_values_context(&values, &context(&values)),
            Ok(())
        );

        let mut altered = values.clone();
        altered.attested_collection_sha256 = "0a".repeat(32);
        assert_eq!(
            validate_yolo_collection_public_values_context(&altered, &context(&values)),
            Err(YoloCollectionVerifyError::AttestationMismatch)
        );
    }

    #[test]
    fn collection_verifier_has_no_reserve_or_value_fields() {
        let encoded = values().encode().unwrap();
        assert_eq!(encoded.len(), YOLO_COLLECTION_PUBLIC_VALUES_V1_BYTES);
        assert_ne!(
            YOLO_COLLECTION_SP1_GROTH16_VERIFIER_V1,
            postfiat_types::NAV_PROFILE_VERIFIER_SP1_NAV_RESERVE_V1
        );
    }
}
