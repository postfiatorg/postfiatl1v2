fn yolo_node_registration() -> postfiat_types::YoloTargetRegisterOperationV1 {
    postfiat_types::YoloTargetRegisterOperationV1 {
        registrant: "synthetic-registrant".into(),
        submitter: "synthetic-submitter".into(),
        program_sha256: "01".repeat(32),
        sp1_program_vkey: format!("0x{}", "02".repeat(32)),
        methodology_sha256: "03".repeat(32),
        parameter_manifest_sha256: "04".repeat(32),
        collection_manifest_sha256: "05".repeat(32),
        series_id_sha256: "06".repeat(32),
        underlier_id_sha256: "07".repeat(32),
        epoch_sha256: "08".repeat(32),
        prior_state_sha256: "09".repeat(32),
        replay_id_sha256: "0a".repeat(32),
        activation_height: 12,
    }
}

#[test]
fn yolo_node_registration_persists_and_queries_without_false_finality() {
    let data_dir = unique_test_dir("yolo-registration-query");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "yolo-synthetic-local".into(),
        node_id: "validator-0".into(),
        validator_count: 1,
    })
    .unwrap();
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().unwrap();
    let operation = yolo_node_registration();
    let id = operation.registration_id();
    ledger
        .yolo_target_registrations
        .push(postfiat_types::YoloTargetRegistrationV1 {
            registration_id: id.clone(),
            registered_height: 10,
            operation,
        });
    store.write_ledger(&ledger).unwrap();
    drop(store);
    assert_eq!(NodeStore::new(&data_dir).read_ledger().unwrap(), ledger);
    let result = yolo_target_receipt_query(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &id,
    )
    .unwrap();
    assert!(result["registration"].is_object());
    assert!(result["receipt"].is_null());
    assert!(result["finality"].is_null());
    assert!(yolo_target_receipt_query(
        NodeOptions {
            data_dir: data_dir.clone()
        },
        "../invalid"
    )
    .is_err());
    std::fs::remove_dir_all(data_dir).unwrap();
}

#[test]
fn yolo_state_commitment_covers_each_new_field_and_preserves_absent_legacy_fields() {
    let empty = LedgerState::new(vec![]);
    let serialized = serde_json::to_value(&empty).unwrap();
    assert!(serialized.get("yolo_target_registrations").is_none());
    assert!(serialized.get("yolo_target_receipts").is_none());
    fn commitment(ledger: &LedgerState) -> Vec<u8> {
        let mut out = vec![];
        super::state_commitment::append_ledger_state(
            &mut out, ledger, true, true, false, false, true, false, false,
        )
        .unwrap();
        out
    }
    let mut ledger = empty.clone();
    let operation = yolo_node_registration();
    ledger
        .yolo_target_registrations
        .push(postfiat_types::YoloTargetRegistrationV1 {
            registration_id: operation.registration_id(),
            registered_height: 10,
            operation,
        });
    assert_ne!(commitment(&empty), commitment(&ledger));
    let baseline = commitment(&ledger);
    let value = serde_json::to_value(&ledger.yolo_target_registrations[0].operation).unwrap();
    for field in value.as_object().unwrap().keys() {
        let mut changed = value.clone();
        changed[field] = if field == "activation_height" {
            99.into()
        } else {
            "changed".into()
        };
        let mut altered = ledger.clone();
        altered.yolo_target_registrations[0].operation = serde_json::from_value(changed).unwrap();
        assert_ne!(
            baseline,
            commitment(&altered),
            "uncommitted registration {field}"
        );
    }
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../execution/testdata/yolo-target-v1-synthetic-proof.json"
    ))
    .unwrap();
    let public =
        postfiat_crypto_provider::hex_to_bytes(fixture["publicValuesHex"].as_str().unwrap())
            .unwrap();
    let submission = postfiat_types::YoloTargetSubmitOperationV1 {
        submitter: "synthetic-submitter".into(),
        registration_id: ledger.yolo_target_registrations[0].registration_id.clone(),
        replay_id_sha256: "0a".repeat(32),
        values: postfiat_types::YoloTargetPublicValuesV1::decode(&public).unwrap(),
        sp1_public_values: public,
        sp1_proof_bytes: postfiat_crypto_provider::hex_to_bytes(
            fixture["proofCalldataHex"].as_str().unwrap(),
        )
        .unwrap(),
    };
    ledger
        .yolo_target_receipts
        .push(postfiat_types::YoloTargetReceiptV1 {
            registration_id: submission.registration_id.clone(),
            transaction_hash: "ab".repeat(48),
            inclusion_height: 12,
            operation: submission,
        });
    let with_receipt = commitment(&ledger);
    assert_ne!(baseline, with_receipt);
    let row = serde_json::to_value(&ledger.yolo_target_receipts[0]).unwrap();
    for field in row.as_object().unwrap().keys() {
        let mut changed = row.clone();
        match field.as_str() {
            "inclusion_height" => changed[field] = 99.into(),
            "operation" => changed[field]["sp1_proof_bytes"][0] = 0.into(),
            _ => changed[field] = "changed".into(),
        }
        let mut altered = ledger.clone();
        altered.yolo_target_receipts[0] = serde_json::from_value(changed).unwrap();
        assert_ne!(
            with_receipt,
            commitment(&altered),
            "uncommitted receipt {field}"
        );
    }
}

#[test]
fn yolo_four_validator_certificates_finality_restart_and_chain_replay() {
    qualify_yolo_four_validator_chain(include_str!(
        "../../../execution/testdata/yolo-target-v1-synthetic-proof.json"
    ));
}

#[test]
fn yolo_aws_compatible_guest_four_validator_finality_restart_and_replay() {
    qualify_yolo_four_validator_chain(include_str!(
        "../../../execution/testdata/yolo-target-v1-aws-compatible-proof.json"
    ));
}

fn qualify_yolo_four_validator_chain(fixture_json: &str) {
    let root = unique_test_dir("yolo-four-validator-chain");
    let primary = root.join("validator-0");
    init(InitOptions {
        data_dir: primary.clone(),
        chain_id: "yolo-synthetic-certified".into(),
        node_id: "validator-0".into(),
        validator_count: 4,
    })
    .unwrap();
    let keys = read_validator_key_file(&primary.join(VALIDATOR_KEYS_FILE)).unwrap();
    write_split_validator_key_files(&primary, &keys);
    fn copy_directory(source: &Path, destination: &Path) {
        std::fs::create_dir_all(destination).unwrap();
        for entry in std::fs::read_dir(source).unwrap() {
            let entry = entry.unwrap();
            let target = destination.join(entry.file_name());
            if entry.file_type().unwrap().is_dir() {
                copy_directory(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }
    let mut nodes = vec![primary.clone()];
    for i in 1..4 {
        let path = root.join(format!("validator-{i}"));
        copy_directory(&primary, &path);
        let store = NodeStore::new(&path);
        let mut state = store.read_node_state().unwrap();
        state.node_id = format!("validator-{i}");
        store.write_node_state(&state).unwrap();
        nodes.push(path);
    }
    let validators = local_validator_ids(4).unwrap();
    let amendment = primary.join("yolo-activation.json");
    ratify_governance(RatifyGovernanceOptions {
        data_dir: primary.clone(),
        validators: validators.clone(),
        support: validators,
        kind: postfiat_types::GOVERNANCE_KIND_YOLO_TARGET_ACTIVATION_HEIGHT.into(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: amendment.clone(),
    })
    .unwrap();
    let mut authorizations = vec![];
    for i in 0..4 {
        let validator = format!("validator-{i}");
        let authorization_file = primary.join(format!("{validator}.yolo-authorization.json"));
        sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
            data_dir: primary.clone(), amendment_file: amendment.clone(), validator: validator.clone(),
            validator_key_file: primary.join(format!("{validator}.validator_keys.json")),
            proposal_slot: 1, expires_at_height: 20, authorization_file: authorization_file.clone(),
        }).unwrap();
        authorizations.push(authorization_file);
    }
    let signed_amendment = primary.join("yolo-activation-signed.json");
    assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
        data_dir: primary.clone(), amendment_file: amendment, authorization_files: authorizations,
        proposal_slot: 1, output_file: signed_amendment.clone(),
    }).unwrap();
    let governance_batch = primary.join("activation-batch.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: primary.clone(),
        amendment_file: Some(signed_amendment),
        registry_update_file: None,
        batch_file: governance_batch.clone(),
    })
    .unwrap();
    let mut rounds = vec![];
    let mut certify = |height: u64, kind: &str, batch_file: PathBuf| {
        let certificate = root.join(format!("round-{height}.certificate.json"));
        let report = certify_batch_round(BatchCertificateRoundOptions {
            data_dir: primary.clone(),
            batch_kind: Some(kind.into()),
            batch_file: batch_file.clone(),
            validator_key_dir: primary.clone(),
            vote_dir: root.join(format!("votes-{height}")),
            proposal_file: root.join(format!("proposal-{height}.json")),
            certificate_file: certificate.clone(),
            block_height: Some(height),
            view: None,
            timeout_certificate_file: None,
            skip_block_log_verify: false,
        })
        .unwrap();
        assert!(report.round_ok);
        assert_eq!(report.vote_count, 4);
        rounds.push(report);
        let mut accepted = vec![];
        for directory in &nodes {
            let options = ApplyBatchOptions {
                data_dir: directory.clone(),
                batch_file: batch_file.clone(),
                certificate_file: Some(certificate.clone()),
            };
            let receipts = if kind == BATCH_KIND_GOVERNANCE {
                apply_governance_batch(options)
            } else {
                apply_batch(options)
            }
            .unwrap();
            accepted.push(receipts.iter().map(|r| r.accepted).collect::<Vec<_>>());
        }
        assert!(accepted.iter().all(|row| row == &accepted[0]));
        accepted[0].clone()
    };
    assert_eq!(
        certify(1, BATCH_KIND_GOVERNANCE, governance_batch),
        vec![true]
    );
    let store = NodeStore::new(&primary);
    let genesis = store.read_genesis().unwrap();
    let signer = read_transfer_key_file(&primary, None).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(fixture_json).unwrap();
    let public = hex_to_bytes(fixture["publicValuesHex"].as_str().unwrap()).unwrap();
    let values = postfiat_types::YoloTargetPublicValuesV1::decode(&public).unwrap();
    let registration = postfiat_types::YoloTargetRegisterOperationV1 {
        registrant: signer.address.clone(),
        submitter: signer.address.clone(),
        program_sha256: values.program_sha256.clone(),
        sp1_program_vkey: fixture["program"]["programVkey"].as_str().unwrap().into(),
        methodology_sha256: values.methodology_sha256.clone(),
        parameter_manifest_sha256: values.parameter_manifest_sha256.clone(),
        collection_manifest_sha256: values.collection_manifest_sha256.clone(),
        series_id_sha256: values.series_id_sha256.clone(),
        underlier_id_sha256: values.underlier_id_sha256.clone(),
        epoch_sha256: values.epoch_sha256.clone(),
        prior_state_sha256: values.prior_state_sha256.clone(),
        replay_id_sha256: "ab".repeat(32),
        activation_height: 4,
    };
    let registration_id = registration.registration_id();
    let submission = postfiat_types::YoloTargetSubmitOperationV1 {
        submitter: signer.address.clone(),
        registration_id: registration_id.clone(),
        replay_id_sha256: registration.replay_id_sha256.clone(),
        values,
        sp1_public_values: public,
        sp1_proof_bytes: hex_to_bytes(fixture["proofCalldataHex"].as_str().unwrap()).unwrap(),
    };
    let mut valid_tx_id = String::new();
    for height in 2..=5 {
        let ledger = store.read_ledger().unwrap();
        let operation = if height == 2 {
            AssetTransactionOperation::YoloTargetRegisterV1(registration.clone())
        } else {
            AssetTransactionOperation::YoloTargetSubmitV1(submission.clone())
        };
        let transaction = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &signer.address,
            &signer.public_key_hex,
            &signer.private_key_hex,
            operation.transaction_kind(),
            ledger.account(&signer.address).unwrap().sequence + 1,
            operation,
        );
        if height == 4 {
            valid_tx_id = postfiat_execution::asset_transaction_tx_id(&transaction);
        }
        let batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            vec![],
            vec![],
            vec![transaction],
        )
        .unwrap()
        .batch;
        let path = primary.join(format!("target-{height}.batch.json"));
        write_batch_file(&path, &batch).unwrap();
        assert_eq!(
            certify(height, BATCH_KIND_TRANSPARENT, path),
            vec![height == 2 || height == 4]
        );
    }
    let expected = store.read_ledger().unwrap();
    assert_eq!(expected.yolo_target_receipts.len(), 1);
    let mut reports = vec![];
    for directory in &nodes {
        // Reopen durable stores and independently replay all certified blocks.
        let restarted = NodeStore::new(directory);
        assert_eq!(restarted.read_ledger().unwrap(), expected);
        let replay = verify_blocks(NodeOptions {
            data_dir: directory.clone(),
        })
        .unwrap();
        let receipt = yolo_target_receipt_query(
            NodeOptions {
                data_dir: directory.clone(),
            },
            &registration_id,
        )
        .unwrap();
        assert_eq!(receipt["receipt"]["transaction_hash"], valid_tx_id);
        assert_eq!(receipt["finality"]["confirmed"], true);
        assert!(replay.verified);
        reports.push(serde_json::json!({"validator": restarted.read_node_state().unwrap().node_id, "replay": replay, "receipt": receipt}));
    }
    if let Some(output) = std::env::var_os("YOLO_QUALIFICATION_REPORT") {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)
            .unwrap();
        serde_json::to_writer_pretty(file, &serde_json::json!({"schema":"postfiat.yolo.local_validator_qualification.v1", "scope":"synthetic four-store certificate qualification; no network deployment", "rounds":rounds, "validators":reports})).unwrap();
    }
    if std::env::var_os("YOLO_QUALIFICATION_KEEP_DIRECTORY").is_none() {
        std::fs::remove_dir_all(root).unwrap();
    }
}
