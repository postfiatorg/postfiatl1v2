fn yolo_fixture() -> (YoloTargetRegisterOperationV1, YoloTargetSubmitOperationV1) {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../testdata/yolo-target-v1-synthetic-proof.json"
    ))
    .unwrap();
    let public = hex_to_bytes(fixture["publicValuesHex"].as_str().unwrap()).unwrap();
    let values = YoloTargetPublicValuesV1::decode(&public).unwrap();
    let registration = YoloTargetRegisterOperationV1 {
        registrant: "synthetic-registrant".to_string(),
        submitter: "synthetic-submitter".to_string(),
        program_sha256: values.program_sha256.clone(),
        sp1_program_vkey: fixture["program"]["programVkey"]
            .as_str()
            .unwrap()
            .to_string(),
        methodology_sha256: values.methodology_sha256.clone(),
        parameter_manifest_sha256: values.parameter_manifest_sha256.clone(),
        collection_manifest_sha256: values.collection_manifest_sha256.clone(),
        series_id_sha256: values.series_id_sha256.clone(),
        underlier_id_sha256: values.underlier_id_sha256.clone(),
        epoch_sha256: values.epoch_sha256.clone(),
        prior_state_sha256: values.prior_state_sha256.clone(),
        replay_id_sha256: "ab".repeat(32),
        activation_height: 12,
    };
    let submission = YoloTargetSubmitOperationV1 {
        submitter: registration.submitter.clone(),
        registration_id: registration.registration_id(),
        replay_id_sha256: registration.replay_id_sha256.clone(),
        values,
        sp1_proof_bytes: hex_to_bytes(fixture["proofCalldataHex"].as_str().unwrap()).unwrap(),
        sp1_public_values: public,
    };
    (registration, submission)
}

#[test]
fn yolo_real_groth16_verifier_binds_every_redundant_field_and_registered_expectation() {
    let (registration, submission) = yolo_fixture();
    assert_eq!(registration.registration_id(), "825f93ecc52f60fefdc4c4464399352c9f45cad73b103088def2e48ae5f2a2d40c06dd783abede7a8e1f2bc049df2c7d");
    assert_eq!(
        verify_yolo_target_sp1_groth16(&registration, &submission).unwrap(),
        submission.values
    );
    let object = serde_json::to_value(&submission.values).unwrap();
    for field in object.as_object().unwrap().keys() {
        let mut value = object.clone();
        if field == "schema" {
            value[field] = "unknown".into();
        } else if field == "status" {
            value[field] = "HALTED_INSTRUMENT".into();
        } else if field.ends_with("count") {
            value[field] = 63.into();
        } else {
            value[field] = "ee".repeat(32).into();
        }
        let mut changed = submission.clone();
        changed.values = serde_json::from_value(value).unwrap();
        assert!(
            verify_yolo_target_sp1_groth16(&registration, &changed).is_err(),
            "redundant {field}"
        );
    }
    for field in [
        "program_sha256",
        "sp1_program_vkey",
        "methodology_sha256",
        "parameter_manifest_sha256",
        "collection_manifest_sha256",
        "series_id_sha256",
        "underlier_id_sha256",
        "epoch_sha256",
        "prior_state_sha256",
        "submitter",
        "replay_id_sha256",
    ] {
        let mut value = serde_json::to_value(&registration).unwrap();
        value[field] = if field == "sp1_program_vkey" {
            format!("0x{}", "ee".repeat(32)).into()
        } else {
            "ee".repeat(32).into()
        };
        let changed: YoloTargetRegisterOperationV1 = serde_json::from_value(value).unwrap();
        let mut proof = submission.clone();
        proof.registration_id = changed.registration_id();
        assert!(
            verify_yolo_target_sp1_groth16(&changed, &proof).is_err(),
            "registered {field}"
        );
    }
}

#[test]
fn yolo_malformed_and_changed_proof_public_bytes_reject() {
    let (registration, submission) = yolo_fixture();
    for mode in 0..6 {
        let mut changed = submission.clone();
        match mode {
            0 => {
                changed.sp1_proof_bytes.pop();
            }
            1 => changed.sp1_proof_bytes[10] ^= 1,
            2 => changed.sp1_proof_bytes = vec![0; YOLO_TARGET_MAX_PROOF_BYTES_V1 + 1],
            3 => {
                changed.sp1_public_values.pop();
            }
            4 => changed.sp1_public_values.push(0),
            _ => {
                changed.values.target_sha256 = "ee".repeat(32);
                changed.sp1_public_values = changed.values.encode().unwrap();
            }
        }
        assert!(verify_yolo_target_sp1_groth16(&registration, &changed).is_err());
    }
}

#[test]
fn yolo_signed_lifecycle_activation_replay_persistence_and_no_asset_authority() {
    let genesis = Genesis::new("yolo-synthetic-local");
    let keys = ml_dsa_65_keygen().unwrap();
    let address = address_from_public_key(&keys.public_key);
    let initial = LedgerState::new(vec![Account::new(
        address.clone(),
        1_000_000,
        Some(bytes_to_hex(&keys.public_key)),
    )]);
    let (mut registration, mut submission) = yolo_fixture();
    registration.registrant = address.clone();
    registration.submitter = address.clone();
    submission.submitter = address;
    submission.registration_id = registration.registration_id();
    let register = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &initial,
        &keys,
        postfiat_types::YOLO_TARGET_REGISTER_TRANSACTION_KIND_V1,
        1,
        AssetTransactionOperation::YoloTargetRegisterV1(registration.clone()),
    );
    let active = AssetExecutionCompatibility::strict().with_yolo_target_activation_height(Some(10));
    let mut ledger = initial.clone();
    let inactive = execute_asset_transaction(&genesis, &mut ledger, &register, 10);
    assert!(!inactive.accepted);
    assert!(ledger.yolo_target_registrations.is_empty());
    // Use fresh state so rejected-transaction fee semantics cannot affect sequence.
    ledger = initial.clone();
    let receipt =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &register, 10, active);
    assert!(receipt.accepted, "{receipt:?}");
    let submit = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &keys,
        postfiat_types::YOLO_TARGET_SUBMIT_TRANSACTION_KIND_V1,
        2,
        AssetTransactionOperation::YoloTargetSubmitV1(submission.clone()),
    );
    let mut before_activation = ledger.clone();
    assert!(
        !execute_asset_transaction_with_compatibility(
            &genesis,
            &mut before_activation,
            &submit,
            11,
            active
        )
        .accepted
    );
    assert!(before_activation.yolo_target_receipts.is_empty());
    let before = ledger.clone();
    let receipt =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &submit, 12, active);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(ledger.yolo_target_receipts.len(), 1);
    assert_eq!(
        ledger.yolo_target_receipts[0].transaction_hash,
        asset_transaction_tx_id(&submit)
    );
    let mut unchanged = ledger.clone();
    unchanged.accounts = before.accounts.clone();
    unchanged.yolo_target_receipts.clear();
    assert_eq!(
        unchanged, before,
        "target receipt mutated another state domain"
    );
    let duplicate = signed_asset_transaction_with_minimum_fee(
        &genesis,
        &ledger,
        &keys,
        postfiat_types::YOLO_TARGET_SUBMIT_TRANSACTION_KIND_V1,
        3,
        AssetTransactionOperation::YoloTargetSubmitV1(submission),
    );
    assert!(
        !execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger.clone(),
            &duplicate,
            13,
            active
        )
        .accepted
    );
    let restored: LedgerState =
        serde_json::from_slice(&serde_json::to_vec(&ledger).unwrap()).unwrap();
    assert_eq!(restored, ledger);
    let mut replay = initial;
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut replay, &register, 10, active)
            .accepted
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut replay, &submit, 12, active)
            .accepted
    );
    assert_eq!(replay, restored);
    let mut tampered = register;
    if let AssetTransactionOperation::YoloTargetRegisterV1(value) = &mut tampered.unsigned.operation
    {
        value.prior_state_sha256 = "ee".repeat(32);
    }
    assert!(
        !execute_asset_transaction_with_compatibility(
            &genesis,
            &mut LedgerState::new(replay.accounts.clone()),
            &tampered,
            10,
            active
        )
        .accepted
    );
}
