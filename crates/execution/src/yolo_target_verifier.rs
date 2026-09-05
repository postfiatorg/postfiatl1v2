use postfiat_types::{
    YoloTargetPublicValuesV1, YoloTargetRegisterOperationV1, YoloTargetSubmitOperationV1,
    YOLO_TARGET_MAX_PROOF_BYTES_V1, YOLO_TARGET_PUBLIC_VALUES_V1_BYTES,
    YOLO_TARGET_SP1_GROTH16_VERIFIER_V1,
};

/// The register operation is fetched from immutable consensus state, never
/// reconstructed from proof-supplied values. No NAV reserve semantics apply.
pub fn verify_yolo_target_sp1_groth16(
    registration: &YoloTargetRegisterOperationV1,
    submission: &YoloTargetSubmitOperationV1,
) -> Result<YoloTargetPublicValuesV1, String> {
    registration.validate()?;
    submission.validate()?;
    if submission.registration_id != registration.registration_id()
        || submission.submitter != registration.submitter
        || submission.replay_id_sha256 != registration.replay_id_sha256
    {
        return Err("target submission registration/submitter/replay binding differs".to_string());
    }
    let values = YoloTargetPublicValuesV1::decode(&submission.sp1_public_values)?;
    registration.validate_public_values(&values)?;
    verify_bounded_sp1_groth16_with_config(
        YOLO_TARGET_SP1_GROTH16_VERIFIER_V1,
        YOLO_TARGET_SP1_GROTH16_VERIFIER_V1,
        &registration.sp1_program_vkey,
        YOLO_TARGET_MAX_PROOF_BYTES_V1 as u64,
        YOLO_TARGET_PUBLIC_VALUES_V1_BYTES as u64,
        &submission.sp1_proof_bytes,
        &submission.sp1_public_values,
    )
    .map_err(|_| "target Groth16 verification failed".to_string())?;
    Ok(values)
}

fn require_yolo_target_activation(
    compatibility: AssetExecutionCompatibility,
    height: u64,
) -> Result<(), (&'static str, String)> {
    if !compatibility
        .yolo_target_activation_height
        .is_some_and(|activation| height >= activation)
    {
        return Err((
            "yolo_target_inactive",
            "YOLO target receipt feature is inactive".to_string(),
        ));
    }
    Ok(())
}

fn register_yolo_target_run(
    ledger: &mut LedgerState,
    operation: &YoloTargetRegisterOperationV1,
    height: u64,
) -> Result<(), (&'static str, String)> {
    operation
        .validate()
        .map_err(|e| ("invalid_yolo_registration", e))?;
    if operation.activation_height <= height {
        return Err((
            "invalid_yolo_activation",
            "target run activation must be strictly in the future".to_string(),
        ));
    }
    if ledger.yolo_target_registrations.iter().any(|row| {
        row.operation.registrant == operation.registrant
            && (row.operation.replay_id_sha256 == operation.replay_id_sha256
                || row.operation.series_id_sha256 == operation.series_id_sha256
                    && row.operation.epoch_sha256 == operation.epoch_sha256)
    }) {
        return Err((
            "duplicate_yolo_registration",
            "registrant already registered this series/epoch or replay identifier".to_string(),
        ));
    }
    ledger
        .yolo_target_registrations
        .push(postfiat_types::YoloTargetRegistrationV1 {
            registration_id: operation.registration_id(),
            registered_height: height,
            operation: operation.clone(),
        });
    Ok(())
}

fn submit_yolo_target_receipt(
    ledger: &mut LedgerState,
    operation: &YoloTargetSubmitOperationV1,
    transaction_hash: &str,
    height: u64,
) -> Result<(), (&'static str, String)> {
    operation
        .validate()
        .map_err(|e| ("invalid_yolo_submission", e))?;
    let registration = ledger
        .yolo_target_registrations
        .iter()
        .find(|row| row.registration_id == operation.registration_id)
        .ok_or_else(|| {
            (
                "unknown_yolo_registration",
                "target run has not been registered".to_string(),
            )
        })?;
    if registration.registration_id != registration.operation.registration_id()
        || registration.registered_height >= registration.operation.activation_height
    {
        return Err((
            "invalid_yolo_registration",
            "stored target registration is inconsistent".to_string(),
        ));
    }
    if height < registration.operation.activation_height {
        return Err((
            "yolo_run_inactive",
            "registered target run is not active yet".to_string(),
        ));
    }
    if ledger
        .yolo_target_receipts
        .iter()
        .any(|row| row.registration_id == operation.registration_id)
    {
        return Err((
            "duplicate_yolo_receipt",
            "target run already has a receipt".to_string(),
        ));
    }
    verify_yolo_target_sp1_groth16(&registration.operation, operation)
        .map_err(|e| ("invalid_yolo_proof", e))?;
    ledger
        .yolo_target_receipts
        .push(postfiat_types::YoloTargetReceiptV1 {
            registration_id: operation.registration_id.clone(),
            transaction_hash: transaction_hash.to_string(),
            inclusion_height: height,
            operation: operation.clone(),
        });
    Ok(())
}
