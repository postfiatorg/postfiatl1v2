    #[test]
    fn verifies_governance_replay_package_for_registry_update() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-governance-replay-package-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-governance-replay-package".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init governance replay package test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let full_registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let previous_validators = local_validator_ids(2).expect("previous validators");
        let new_validators = local_validator_ids(3).expect("new validators");
        let previous_registry =
            validator_registry_subset_for_validators(&full_registry, &previous_validators)
                .expect("previous registry subset");
        let new_registry =
            validator_registry_subset_for_validators(&full_registry, &new_validators)
                .expect("new registry subset");
        let previous_registry_file = data_dir.join("previous-registry.json");
        let new_registry_file = data_dir.join("new-registry.json");
        write_validator_registry_file(&previous_registry_file, &previous_registry)
            .expect("write previous registry");
        write_validator_registry_file(&new_registry_file, &new_registry)
            .expect("write new registry");
        let previous_registry_root =
            validator_registry_root(&previous_registry, &previous_validators)
                .expect("previous registry root");
        let new_registry_root =
            validator_registry_root(&new_registry, &new_validators).expect("new registry root");
        let manifest_dir = data_dir.join("operator-manifests");
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        for (index, validator_id) in previous_validators.iter().enumerate() {
            let hot_key = validator_registry_record(&full_registry, validator_id)
                .expect("validator registry record")
                .public_key_hex
                .clone();
            let manifest = signed_test_operator_manifest(
                &genesis.chain_id,
                "controlled-testnet",
                validator_id,
                &hot_key,
                [(index as u8) + 77; 32],
                &format!("operator-{index}"),
            );
            write_test_operator_manifest(
                &manifest_dir.join(format!("{validator_id}.operator-manifest.json")),
                &manifest,
            );
        }
        let genesis_bundle_file = data_dir.join("genesis-governance-bundle.json");
        let genesis_bundle = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
            data_dir: data_dir.clone(),
            manifest_dir,
            validators: previous_validators.clone(),
            quorum: bft_quorum_threshold(previous_validators.len()).expect("quorum"),
            network: "controlled-testnet".to_string(),
            output_file: genesis_bundle_file.clone(),
        })
        .expect("create genesis governance bundle");
        assert_eq!(genesis_bundle.registry_root, previous_registry_root);
        let amendment_validators = local_validator_ids(3).expect("amendment validators");
        let amendment_file = data_dir.join("crypto-policy-v2-amendment.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: amendment_validators.clone(),
            support: amendment_validators,
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: amendment_file.clone(),
        })
        .expect("ratify amendment replay fixture");
        let amendment_batch_file = data_dir.join("crypto-policy-v2-amendment-batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(amendment_file),
            registry_update_file: None,
            batch_file: amendment_batch_file.clone(),
        })
        .expect("create amendment replay fixture batch");
        let amendment_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: amendment_batch_file,
            certificate_file: None,
        })
        .expect("apply amendment replay fixture");
        assert!(amendment_receipts[0].accepted, "{amendment_receipts:?}");
        let governance = NodeStore::new(&data_dir)
            .read_governance()
            .expect("governance after amendment replay fixture");
        let amendment_governance_report = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify governance after amendment replay fixture");
        let amendment_replay_bundle = GovernanceAmendmentReplayBundle {
            schema: "postfiat-cobalt-amendment-replay-bundle-v0".to_string(),
            ordered_amendment_count: governance.amendments.len(),
            ordered_activation_record_count: governance.amendment_activation_records.len(),
            ordered_supersession_record_count: governance.amendment_supersession_records.len(),
            ordered_rollback_record_count: governance.amendment_rollback_records.len(),
            ordered_amendments: governance.amendments,
            ordered_activation_records: governance.amendment_activation_records,
            ordered_supersession_records: governance.amendment_supersession_records,
            ordered_rollback_records: governance.amendment_rollback_records,
            final_governance: GovernanceAmendmentReplayFinalGovernance {
                active_validator_count: amendment_governance_report.active_validator_count,
                crypto_policy_version: amendment_governance_report.crypto_policy_version,
                bridge_witness_epoch: amendment_governance_report.bridge_witness_epoch,
                authority_mode: amendment_governance_report.authority_mode,
                amendment_count: amendment_governance_report.amendment_count,
                latest_amendment_id: amendment_governance_report.latest_amendment_id,
                amendment_activation_record_count: amendment_governance_report
                    .amendment_activation_record_count,
                latest_amendment_activation_record_id: amendment_governance_report
                    .latest_amendment_activation_record_id,
                amendment_supersession_record_count: amendment_governance_report
                    .amendment_supersession_record_count,
                latest_amendment_supersession_record_id: amendment_governance_report
                    .latest_amendment_supersession_record_id,
                amendment_rollback_record_count: amendment_governance_report
                    .amendment_rollback_record_count,
                latest_amendment_rollback_record_id: amendment_governance_report
                    .latest_amendment_rollback_record_id,
            },
        };
        let amendment_replay_bundle_file = data_dir.join("governance-amendment-replay-bundle.json");
        let amendment_replay_bundle_json = serde_json::to_string_pretty(&amendment_replay_bundle)
            .expect("serialize amendment replay bundle");
        atomic_write(
            &amendment_replay_bundle_file,
            format!("{amendment_replay_bundle_json}\n"),
        )
        .expect("write amendment replay bundle");
        assert!(
            verify_governance_amendment_replay_bundle(GovernanceAmendmentReplayVerifyOptions {
                bundle_file: amendment_replay_bundle_file.clone(),
            })
            .expect("verify amendment replay fixture")
            .verified
        );
        let admitted = validator_registry_record(&full_registry, "validator-2")
            .expect("admitted validator")
            .clone();
        let admitted_entry = ValidatorRegistryEntry {
            node_id: admitted.node_id,
            algorithm_id: admitted.algorithm_id,
            public_key_hex: admitted.public_key_hex,
            active: true,
        };
        let admitted_entry_file = data_dir.join("admitted-validator-entry.json");
        let admitted_entry_json =
            serde_json::to_string_pretty(&admitted_entry).expect("serialize entry");
        atomic_write(&admitted_entry_file, format!("{admitted_entry_json}\n"))
            .expect("write admitted entry");
        let update_file = data_dir.join("validator-registry-update.json");
        let update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dir.clone(),
            validators: previous_validators.clone(),
            support: previous_validators.clone(),
            activation_height: 4,
            previous_registry_root: previous_registry_root.clone(),
            new_registry_root: new_registry_root.clone(),
            previous_validators: previous_validators.clone(),
            new_validators,
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-2".to_string(),
            previous_record_file: None,
            new_record_file: Some(admitted_entry_file),
            update_file: update_file.clone(),
        })
        .expect("create registry update");
        let governance_batch_file = data_dir.join("governance-batch.json");
        let batch = create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: None,
            registry_update_file: Some(update_file),
            batch_file: governance_batch_file,
        })
        .expect("create governance batch");
        let package_file = data_dir.join("governance-replay-package.json");
        let package = create_governance_replay_package(GovernanceReplayBuildOptions {
            data_dir: data_dir.clone(),
            genesis_bundle_file: Some(genesis_bundle_file),
            previous_registry_file,
            update_file: data_dir.join("validator-registry-update.json"),
            new_registry_file,
            amendment_replay_bundle_file: Some(amendment_replay_bundle_file),
            governance_batch_file: Some(data_dir.join("governance-batch.json")),
            post_change_block_file: None,
            post_change_batch_file: None,
            post_change_certificate_file: None,
            output_file: package_file.clone(),
            overwrite: false,
        })
        .expect("build replay package");
        assert_eq!(package.chain_id.as_deref(), Some(genesis.chain_id.as_str()));
        assert_eq!(
            package.expected_update_id.as_deref(),
            Some(update.update_id.as_str())
        );
        assert_eq!(
            package.expected_batch_id.as_deref(),
            Some(batch.batch_id.as_str())
        );
        assert_eq!(
            package.genesis_bundle_file.as_deref(),
            Some("genesis-governance-bundle.json")
        );
        assert_eq!(package.previous_registry_file, "previous-registry.json");
        assert_eq!(package.update_file, "validator-registry-update.json");
        assert_eq!(package.new_registry_file, "new-registry.json");
        assert_eq!(
            package.amendment_replay_bundle_file.as_deref(),
            Some("governance-amendment-replay-bundle.json")
        );

        let report = verify_governance_replay_package(GovernanceReplayVerifyOptions {
            data_dir: data_dir.clone(),
            package_file,
        })
        .expect("verify replay package");

        assert!(report.verified);
        assert_eq!(report.update_id, update.update_id);
        assert_eq!(report.operation, VALIDATOR_REGISTRY_OP_ADMIT);
        assert!(report.previous_registry_root_verified);
        assert!(report.new_registry_root_verified);
        assert!(report.governance_genesis_bundle_verified);
        assert_eq!(
            report.governance_genesis_bundle_hash.as_deref(),
            Some(genesis_bundle.bundle_hash.as_str())
        );
        assert_eq!(
            report.governance_genesis_registry_root.as_deref(),
            Some(previous_registry_root.as_str())
        );
        assert_eq!(report.governance_genesis_operator_manifest_count, Some(2));
        assert!(report.governance_batch_verified);
        assert!(report.governance_batch_contains_update);
        assert_eq!(report.governance_batch_id, batch.batch_id);
        assert!(report.amendment_replay_verified);
        assert_eq!(report.amendment_replay_amendment_count, Some(1));
        assert_eq!(report.amendment_replay_activation_record_count, Some(1));
        assert_eq!(report.amendment_replay_supersession_record_count, Some(0));
        assert_eq!(report.amendment_replay_rollback_record_count, Some(0));
        assert!(!report.post_change_certificate_verified);

        fs::remove_dir_all(data_dir).expect("cleanup governance replay package data");
    }

    #[test]
    fn verifies_governance_amendment_replay_bundle() {
        let data_dir = unique_test_dir("postfiat-governance-amendment-replay-bundle-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-governance-amendment-replay-bundle".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 5,
        })
        .expect("init governance amendment replay bundle test");
        let validators = local_validator_ids(5).expect("validators");

        let apply_governance_file = |label: &str, amendment_file: PathBuf| {
            let batch_file = data_dir.join(format!("{label}.batch.json"));
            create_governance_batch(GovernanceBatchOptions {
                data_dir: data_dir.clone(),
                amendment_file: Some(amendment_file),
                registry_update_file: None,
                batch_file: batch_file.clone(),
            })
            .expect("create governance batch");
            let receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: None,
            })
            .expect("apply governance batch");
            assert!(receipts[0].accepted, "{receipts:?}");
        };

        let crypto_v2_file = data_dir.join("crypto-policy-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: crypto_v2_file.clone(),
        })
        .expect("ratify crypto v2");
        apply_governance_file("crypto-policy-v2", crypto_v2_file);

        let bridge_v2_file = data_dir.join("bridge-witness-epoch-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: bridge_v2_file.clone(),
        })
        .expect("ratify bridge v2");
        apply_governance_file("bridge-witness-epoch-v2", bridge_v2_file);

        let validator_set_file = data_dir.join("validator-set-6.json");
        ratify_validator_set(RatifyValidatorSetOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            validator_count: 6,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: validator_set_file.clone(),
        })
        .expect("ratify validator set 6");
        apply_governance_file("validator-set-6", validator_set_file);

        let crypto_v3_file = data_dir.join("crypto-policy-v3.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 3,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: crypto_v3_file.clone(),
        })
        .expect("ratify crypto v3");
        apply_governance_file("crypto-policy-v3", crypto_v3_file);

        let crypto_v2_rollback_file = data_dir.join("crypto-policy-v2-rollback.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators,
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 1,
            veto_until_height: 0,
            paused: false,
            amendment_file: crypto_v2_rollback_file.clone(),
        })
        .expect("ratify crypto v2 rollback");
        apply_governance_file("crypto-policy-v2-rollback", crypto_v2_rollback_file);

        let governance = NodeStore::new(&data_dir)
            .read_governance()
            .expect("governance");
        let final_report = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify governance");
        assert_eq!(final_report.amendment_count, 5);
        assert_eq!(final_report.amendment_supersession_record_count, 2);
        assert_eq!(final_report.amendment_rollback_record_count, 1);

        let bundle = GovernanceAmendmentReplayBundle {
            schema: "postfiat-cobalt-amendment-replay-bundle-v0".to_string(),
            ordered_amendment_count: governance.amendments.len(),
            ordered_activation_record_count: governance.amendment_activation_records.len(),
            ordered_supersession_record_count: governance.amendment_supersession_records.len(),
            ordered_rollback_record_count: governance.amendment_rollback_records.len(),
            ordered_amendments: governance.amendments.clone(),
            ordered_activation_records: governance.amendment_activation_records.clone(),
            ordered_supersession_records: governance.amendment_supersession_records.clone(),
            ordered_rollback_records: governance.amendment_rollback_records.clone(),
            final_governance: GovernanceAmendmentReplayFinalGovernance {
                active_validator_count: final_report.active_validator_count,
                crypto_policy_version: final_report.crypto_policy_version,
                bridge_witness_epoch: final_report.bridge_witness_epoch,
                authority_mode: final_report.authority_mode,
                amendment_count: final_report.amendment_count,
                latest_amendment_id: final_report.latest_amendment_id,
                amendment_activation_record_count: final_report.amendment_activation_record_count,
                latest_amendment_activation_record_id: final_report
                    .latest_amendment_activation_record_id,
                amendment_supersession_record_count: final_report
                    .amendment_supersession_record_count,
                latest_amendment_supersession_record_id: final_report
                    .latest_amendment_supersession_record_id,
                amendment_rollback_record_count: final_report.amendment_rollback_record_count,
                latest_amendment_rollback_record_id: final_report
                    .latest_amendment_rollback_record_id,
            },
        };
        let bundle_file = data_dir.join("governance-amendment-replay-bundle.json");
        let bundle_json = serde_json::to_string_pretty(&bundle).expect("serialize bundle");
        atomic_write(&bundle_file, format!("{bundle_json}\n")).expect("write bundle");

        let report =
            verify_governance_amendment_replay_bundle(GovernanceAmendmentReplayVerifyOptions {
                bundle_file: bundle_file.clone(),
            })
            .expect("verify amendment replay bundle");
        assert!(report.verified);
        assert_eq!(report.amendment_count, 5);
        assert_eq!(report.activation_record_count, 5);
        assert_eq!(report.supersession_record_count, 2);
        assert_eq!(report.rollback_record_count, 1);

        let mut tampered = bundle;
        tampered.ordered_activation_records[0].amendment_id = "tampered-amendment-id".to_string();
        let tampered_file = data_dir.join("governance-amendment-replay-bundle-tampered.json");
        let tampered_json = serde_json::to_string_pretty(&tampered).expect("serialize tampered");
        atomic_write(&tampered_file, format!("{tampered_json}\n")).expect("write tampered");
        let tamper_error =
            verify_governance_amendment_replay_bundle(GovernanceAmendmentReplayVerifyOptions {
                bundle_file: tampered_file,
            })
            .expect_err("tampered amendment replay bundle should fail verification");
        assert!(
            tamper_error.to_string().contains("activation record"),
            "{tamper_error}"
        );

        fs::remove_dir_all(data_dir).expect("cleanup amendment replay bundle data");
    }

    #[test]
    fn verifies_validator_registry_lifecycle_replay_bundle() {
        let data_dir = unique_test_dir("postfiat-validator-registry-lifecycle-replay-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-validator-registry-lifecycle-replay".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 6,
        })
        .expect("init validator registry lifecycle replay test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let domain = cobalt_domain(&genesis);
        let full_registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("full validator registry");
        let initial_validators = local_validator_ids(5).expect("initial validators");
        let initial_registry =
            validator_registry_subset_for_validators(&full_registry, &initial_validators)
                .expect("initial registry");
        let mut registry = initial_registry.clone();
        let mut current_validators = initial_validators.clone();
        let mut ordered_updates = Vec::new();

        let entry_from_record = |node_id: &str, active: bool| -> ValidatorRegistryEntry {
            let record =
                validator_registry_record(&full_registry, node_id).expect("registry record");
            ValidatorRegistryEntry {
                node_id: record.node_id.clone(),
                algorithm_id: record.algorithm_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
                active,
            }
        };
        let write_entry = |label: &str, entry: &ValidatorRegistryEntry| -> PathBuf {
            let path = data_dir.join(format!("{label}.json"));
            let json = serde_json::to_string_pretty(entry).expect("serialize registry entry");
            atomic_write(&path, format!("{json}\n")).expect("write registry entry");
            path
        };

        let admitted_validators = local_validator_ids(6).expect("admitted validators");
        let admitted_registry =
            validator_registry_subset_for_validators(&full_registry, &admitted_validators)
                .expect("admitted registry");
        let admitted_entry_file = write_entry(
            "validator-5-admit-entry",
            &entry_from_record("validator-5", true),
        );
        let admit_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dir.clone(),
            validators: current_validators.clone(),
            support: current_validators.clone(),
            activation_height: 1,
            previous_registry_root: validator_registry_root(&registry, &current_validators)
                .expect("admit previous root"),
            new_registry_root: validator_registry_root(&admitted_registry, &admitted_validators)
                .expect("admit new root"),
            previous_validators: current_validators.clone(),
            new_validators: admitted_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
            subject_node_id: "validator-5".to_string(),
            previous_record_file: None,
            new_record_file: Some(admitted_entry_file),
            update_file: data_dir.join("validator-5-admit.update.json"),
        })
        .expect("create admit update");
        current_validators = apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &admit_update,
            admit_update.activation_height,
            "lifecycle replay test admit",
        )
        .expect("apply admit update");
        ordered_updates.push(admit_update);

        let remove_previous_entry_file = write_entry(
            "validator-5-remove-previous",
            &entry_from_record("validator-5", true),
        );
        let remove_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dir.clone(),
            validators: current_validators.clone(),
            support: current_validators.clone(),
            activation_height: 2,
            previous_registry_root: validator_registry_root(&registry, &current_validators)
                .expect("remove previous root"),
            new_registry_root: validator_registry_root(&initial_registry, &initial_validators)
                .expect("remove new root"),
            previous_validators: current_validators.clone(),
            new_validators: initial_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
            subject_node_id: "validator-5".to_string(),
            previous_record_file: Some(remove_previous_entry_file),
            new_record_file: None,
            update_file: data_dir.join("validator-5-remove.update.json"),
        })
        .expect("create remove update");
        current_validators = apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &remove_update,
            remove_update.activation_height,
            "lifecycle replay test remove",
        )
        .expect("apply remove update");
        ordered_updates.push(remove_update);

        let suspended_validators = vec![
            "validator-0".to_string(),
            "validator-2".to_string(),
            "validator-3".to_string(),
            "validator-4".to_string(),
        ];
        let mut suspended_registry = registry.clone();
        suspended_registry
            .validators
            .retain(|record| record.node_id != "validator-1");
        let suspend_previous_entry_file = write_entry(
            "validator-1-suspend-previous",
            &entry_from_record("validator-1", true),
        );
        let suspend_new_entry_file = write_entry(
            "validator-1-suspend-new",
            &entry_from_record("validator-1", false),
        );
        let suspend_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dir.clone(),
            validators: current_validators.clone(),
            support: current_validators.clone(),
            activation_height: 3,
            previous_registry_root: validator_registry_root(&registry, &current_validators)
                .expect("suspend previous root"),
            new_registry_root: validator_registry_root(&suspended_registry, &suspended_validators)
                .expect("suspend new root"),
            previous_validators: current_validators.clone(),
            new_validators: suspended_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_SUSPEND.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record_file: Some(suspend_previous_entry_file),
            new_record_file: Some(suspend_new_entry_file),
            update_file: data_dir.join("validator-1-suspend.update.json"),
        })
        .expect("create suspend update");
        current_validators = apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &suspend_update,
            suspend_update.activation_height,
            "lifecycle replay test suspend",
        )
        .expect("apply suspend update");
        ordered_updates.push(suspend_update);

        let reactivate_previous_entry_file = write_entry(
            "validator-1-reactivate-previous",
            &entry_from_record("validator-1", false),
        );
        let reactivate_new_entry_file = write_entry(
            "validator-1-reactivate-new",
            &entry_from_record("validator-1", true),
        );
        let reactivate_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
            data_dir: data_dir.clone(),
            validators: current_validators.clone(),
            support: current_validators.clone(),
            activation_height: 4,
            previous_registry_root: validator_registry_root(&registry, &current_validators)
                .expect("reactivate previous root"),
            new_registry_root: validator_registry_root(&initial_registry, &initial_validators)
                .expect("reactivate new root"),
            previous_validators: current_validators.clone(),
            new_validators: initial_validators.clone(),
            operation: VALIDATOR_REGISTRY_OP_REACTIVATE.to_string(),
            subject_node_id: "validator-1".to_string(),
            previous_record_file: Some(reactivate_previous_entry_file),
            new_record_file: Some(reactivate_new_entry_file),
            update_file: data_dir.join("validator-1-reactivate.update.json"),
        })
        .expect("create reactivate update");
        current_validators = apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            &reactivate_update,
            reactivate_update.activation_height,
            "lifecycle replay test reactivate",
        )
        .expect("apply reactivate update");
        ordered_updates.push(reactivate_update);

        let final_registry_root =
            validator_registry_root(&registry, &current_validators).expect("final root");
        let bundle = ValidatorRegistryLifecycleReplayBundle {
            schema: "postfiat-validator-registry-lifecycle-replay-bundle-v0".to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            initial_registry,
            initial_validators,
            ordered_updates,
            final_validators: current_validators,
            final_registry_root,
        };
        let bundle_file = data_dir.join("validator-registry-lifecycle-replay-bundle.json");
        let bundle_json = serde_json::to_string_pretty(&bundle).expect("serialize bundle");
        atomic_write(&bundle_file, format!("{bundle_json}\n")).expect("write bundle");

        let report = verify_validator_registry_lifecycle_replay_bundle(
            ValidatorRegistryLifecycleReplayVerifyOptions {
                bundle_file: bundle_file.clone(),
            },
        )
        .expect("verify validator registry lifecycle replay bundle");
        assert!(report.verified);
        assert_eq!(report.initial_validator_count, 5);
        assert_eq!(report.final_validator_count, 5);
        assert_eq!(
            report.operations,
            vec![
                VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
                VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
                VALIDATOR_REGISTRY_OP_SUSPEND.to_string(),
                VALIDATOR_REGISTRY_OP_REACTIVATE.to_string(),
            ]
        );

        let mut out_of_order = bundle;
        out_of_order.ordered_updates.swap(1, 2);
        let out_of_order_file =
            data_dir.join("validator-registry-lifecycle-replay-out-of-order.json");
        let out_of_order_json =
            serde_json::to_string_pretty(&out_of_order).expect("serialize out-of-order bundle");
        atomic_write(&out_of_order_file, format!("{out_of_order_json}\n"))
            .expect("write out-of-order bundle");
        let out_of_order_error = verify_validator_registry_lifecycle_replay_bundle(
            ValidatorRegistryLifecycleReplayVerifyOptions {
                bundle_file: out_of_order_file,
            },
        )
        .expect_err("out-of-order lifecycle replay must fail");
        assert!(
            out_of_order_error.to_string().contains("validator order"),
            "{out_of_order_error}"
        );

        fs::remove_dir_all(data_dir).expect("cleanup validator registry lifecycle replay data");
    }

    #[test]
    fn archived_replay_accepts_legacy_registry_update_domain_only_in_history() {
        let data_dir = unique_test_dir("postfiat-legacy-registry-update-domain-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-legacy-registry-update-domain".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init legacy registry domain test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let canonical_domain = cobalt_domain(&genesis);
        let legacy_domain = CobaltDomain {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: "b".repeat(96),
            protocol_version: genesis.protocol_version,
        };
        let full_registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let previous_validators = local_validator_ids(2).expect("previous validators");
        let new_validators = local_validator_ids(3).expect("new validators");
        let previous_registry =
            validator_registry_subset_for_validators(&full_registry, &previous_validators)
                .expect("previous registry");
        let new_registry = validator_registry_subset_for_validators(&full_registry, &new_validators)
            .expect("new registry");
        let admitted_record =
            validator_registry_record(&full_registry, "validator-2").expect("admitted record");
        let legacy_update = certify_validator_registry_update(
            &legacy_domain,
            &EssentialSubsetConfig::all_of(previous_validators.clone()),
            ValidatorRegistryUpdateRequest {
                activation_height: 1,
                previous_registry_root: validator_registry_root(
                    &previous_registry,
                    &previous_validators,
                )
                .expect("previous root"),
                new_registry_root: validator_registry_root(&new_registry, &new_validators)
                    .expect("new root"),
                previous_trust_graph_root: None,
                new_trust_graph_root: None,
                trust_graph_transition_id: None,
                previous_validators: previous_validators.clone(),
                new_validators: new_validators.clone(),
                operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
                subject_node_id: "validator-2".to_string(),
                previous_record: None,
                new_record: Some(ValidatorRegistryEntry {
                    node_id: admitted_record.node_id.clone(),
                    algorithm_id: admitted_record.algorithm_id.clone(),
                    public_key_hex: admitted_record.public_key_hex.clone(),
                    active: true,
                }),
            },
            previous_validators.clone(),
        )
        .expect("legacy-domain registry update");

        let canonical_error =
            verify_cobalt_validator_registry_update(&canonical_domain, &legacy_update)
                .expect_err("canonical live path must reject legacy domain");
        assert_eq!(canonical_error, "validator registry update domain mismatch");

        let update_file = data_dir.join("legacy-domain-registry-update.json");
        write_validator_registry_update_file(&update_file, &legacy_update)
            .expect("write legacy update");
        let live_batch_error = create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: None,
            registry_update_file: Some(update_file),
            batch_file: data_dir.join("legacy-domain-live-batch.json"),
        })
        .expect_err("normal live batch creation must reject legacy update domain");
        assert!(
            live_batch_error
                .to_string()
                .contains("validator registry update domain mismatch"),
            "{live_batch_error}"
        );

        let batch_id = chain_bound_action_batch_id_for_genesis_hash(
            &genesis,
            &legacy_domain.genesis_hash,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &Vec::<GovernanceAmendment>::new(),
                &vec![legacy_update.clone()],
            ),
        )
        .expect("legacy governance batch id");
        let historical_batch =
            GovernanceActionBatch::with_registry_updates(batch_id, Vec::new(), vec![
                legacy_update.clone(),
            ]);
        verify_archived_governance_action_batch_id(&genesis, &historical_batch)
            .expect("archived replay accepts self-consistent legacy domain");

        let mut historical_governance = GovernanceState::new(new_validators.len() as u32);
        historical_governance.validator_registry_updates = vec![legacy_update.clone()];
        historical_governance.active_validators = new_validators.clone();
        historical_governance.active_validator_count = new_validators.len() as u32;
        store
            .write_governance(&historical_governance)
            .expect("write historical governance state");
        verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("governance verification accepts historical legacy registry domain");

        let mut registry = previous_registry;
        let applied_validators = apply_historical_validator_registry_update_to_registry(
            &genesis,
            &mut registry,
            &legacy_update,
            legacy_update.activation_height,
            "legacy domain replay test",
        )
        .expect("apply historical legacy update");
        assert_eq!(applied_validators, new_validators);

        fs::remove_dir_all(data_dir).expect("cleanup legacy registry domain test data");
    }

    #[test]
    fn history_status_and_prune_plan_fail_closed_inside_retention_window() {
        let data_dir = unique_test_dir("postfiat-history-retention-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-history-retention".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init history retention test");

        let options = HistoryOptions::with_defaults(data_dir.clone());
        let status = history_status(options.clone()).expect("history status");
        assert_eq!(status.schema, "postfiat-history-status-v1");
        assert_eq!(status.current_height, 0);
        assert!(status.block_log_verified);
        assert!(status.partial_history_ready);
        assert_eq!(status.local_block_range.count, 0);
        assert!(status.storage_files.iter().any(|file| file.bytes > 0));

        let plan = history_prune_plan(options).expect("history prune plan");
        assert_eq!(plan.schema, "postfiat-history-prune-plan-v1");
        assert!(plan.dry_run);
        assert!(!plan.prune_allowed);
        assert_eq!(plan.computed_prune_up_to_height, None);
        assert!(plan
            .refusal_reasons
            .iter()
            .any(|reason| reason.contains("retention window")));

        fs::remove_dir_all(data_dir).expect("cleanup history retention test");
    }

    #[test]
    fn archive_handoff_proof_allows_prune_plan_after_retention_boundary() {
        let data_dir = unique_test_dir("postfiat-history-archive-handoff-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-history-archive-handoff".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init archive handoff test");
        let batch_file = data_dir.join("handoff-transfer.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfarchivehandoff000000000000000000000".to_string(),
            amount: 25,
            batch_file: batch_file.clone(),
        })
        .expect("create handoff batch");
        let validator_keys =
            read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
        write_split_validator_key_files(&data_dir, &validator_keys);
        let certificate_file = data_dir.join("handoff-transfer.block-certificate.json");
        certify_batch_round(BatchCertificateRoundOptions {
            data_dir: data_dir.clone(),
            batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
            batch_file: batch_file.clone(),
            validator_key_dir: data_dir.clone(),
            vote_dir: data_dir.join("handoff-votes"),
            proposal_file: data_dir.join("handoff-transfer.block-proposal.json"),
            certificate_file: certificate_file.clone(),
            block_height: Some(1),
            view: None,
            timeout_certificate_file: None,
            skip_block_log_verify: false,
        })
        .expect("certify handoff batch");
        apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: Some(certificate_file),
        })
        .expect("apply handoff batch");

        let proof_file = data_dir.join("archive-handoff.json");
        let proof = create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
            data_dir: data_dir.clone(),
            from_height: 1,
            to_height: 1,
            archive_uri: Some("archive://controlled-testnet/unit-test".to_string()),
            output_file: proof_file.clone(),
            overwrite: false,
        })
        .expect("create archive handoff proof");
        assert_eq!(proof.block_count, 1);
        assert_eq!(proof.batch_count, 1);

        let verified = verify_history_archive_handoff(HistoryArchiveHandoffVerifyOptions {
            data_dir: data_dir.clone(),
            proof_file: proof_file.clone(),
        })
        .expect("verify archive handoff proof");
        assert!(verified.verified);
        assert_eq!(verified.proof_hash, proof.proof_hash);

        let archive_bundle_file = data_dir.join("archive-window.json");
        let archive_bundle = export_history_archive_window(HistoryArchiveWindowExportOptions {
            data_dir: data_dir.clone(),
            from_height: 1,
            to_height: 1,
            archive_uri: Some("archive://controlled-testnet/unit-test".to_string()),
            output_file: archive_bundle_file.clone(),
            overwrite: false,
        })
        .expect("export archive window");
        assert_eq!(archive_bundle.proof.proof_hash, proof.proof_hash);
        assert_eq!(archive_bundle.blocks.len(), 1);
        assert_eq!(archive_bundle.batches.len(), 1);
        assert_eq!(archive_bundle.receipts.len(), 1);
        let archive_bundle_verified =
            verify_history_archive_window_bundle(HistoryArchiveWindowVerifyOptions {
                bundle_file: archive_bundle_file,
            })
            .expect("verify archive window");
        assert!(archive_bundle_verified.verified);
        assert_eq!(
            archive_bundle_verified.bundle_hash,
            archive_bundle.bundle_hash
        );

        let mut options = HistoryOptions::with_defaults(data_dir.clone());
        options.retain_recent_blocks = 0;
        options.minimum_replay_window_blocks = 0;
        options.archive_handoff_file = Some(proof_file);
        let plan = history_prune_plan(options).expect("history prune plan with handoff");
        assert!(plan.prune_allowed);
        assert!(plan.archive_handoff_present);
        assert!(plan.archive_handoff_verified);
        assert_eq!(plan.computed_prune_up_to_height, Some(1));
        assert_eq!(plan.eligible_block_count, 1);
        assert_eq!(plan.eligible_batch_count, 1);
        assert_eq!(plan.refusal_reasons, Vec::<String>::new());

        fs::remove_dir_all(data_dir).expect("cleanup archive handoff test");
    }

    #[test]
    fn history_prune_writes_checkpoint_and_allows_post_prune_block() {
        let data_dir = unique_test_dir("postfiat-history-prune-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-history-prune".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init history prune test");

        let first_batch_file = data_dir.join("first-transfer.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfhistoryprune1000000000000000000000".to_string(),
            amount: ACCOUNT_RESERVE + 15,
            batch_file: first_batch_file.clone(),
        })
        .expect("create first prune batch");
        let first_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: first_batch_file,
            certificate_file: None,
        })
        .expect("apply first prune batch");
        assert!(first_receipts[0].accepted, "{first_receipts:?}");

        let proof_file = data_dir.join("archive-handoff.json");
        let proof = create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
            data_dir: data_dir.clone(),
            from_height: 1,
            to_height: 1,
            archive_uri: Some("archive://controlled-testnet/unit-test-prune".to_string()),
            output_file: proof_file.clone(),
            overwrite: false,
        })
        .expect("create prune archive handoff");
        assert_eq!(proof.block_count, 1);

        let archive_bundle_file = data_dir.join("prune-archive-window.json");
        let archive_bundle = export_history_archive_window(HistoryArchiveWindowExportOptions {
            data_dir: data_dir.clone(),
            from_height: 1,
            to_height: 1,
            archive_uri: Some("archive://controlled-testnet/unit-test-prune".to_string()),
            output_file: archive_bundle_file.clone(),
            overwrite: false,
        })
        .expect("export prune archive window");
        assert_eq!(archive_bundle.proof.proof_hash, proof.proof_hash);

        let mut prune_options = HistoryOptions::with_defaults(data_dir.clone());
        prune_options.retain_recent_blocks = 0;
        prune_options.minimum_replay_window_blocks = 0;
        prune_options.archive_handoff_file = Some(proof_file);
        let prune_report = history_prune(prune_options).expect("history prune");
        assert!(prune_report.pruned);
        assert_eq!(prune_report.pruned_block_count, 1);
        assert_eq!(prune_report.pruned_batch_count, 1);
        assert_eq!(prune_report.pruned_receipt_count, 1);
        assert_eq!(prune_report.after_block_range.count, 0);
        assert!(prune_report.verify_after_prune.verified);
        assert_eq!(prune_report.verify_after_prune.block_count, 0);

        let store = NodeStore::new(&data_dir);
        let checkpoint = read_history_checkpoint_state_optional(&store)
            .expect("read history checkpoint")
            .expect("checkpoint written");
        assert_eq!(checkpoint.schema, "postfiat-history-checkpoint-v2");
        assert_eq!(
            checkpoint.native_fee_burn_total,
            Some(u128::from(first_receipts[0].fee_burned))
        );
        assert_eq!(
            native_pft_live_total(&checkpoint.ledger, &checkpoint.shielded)
                .expect("checkpoint live native supply")
                + checkpoint.native_fee_burn_total.expect("checkpoint burn total"),
            u128::from(
                store
                    .read_genesis()
                    .expect("checkpoint genesis")
                    .expected_native_supply_atoms()
            )
        );
        assert_eq!(checkpoint.pruned_up_to_height, 1);
        assert_eq!(
            checkpoint.checkpoint_hash,
            prune_report.checkpoint.checkpoint_hash
        );
        assert_eq!(checkpoint.ordered_batches.len(), 1);
        let journal = read_history_prune_journal(&store).expect("read prune journal");
        assert_eq!(journal.records.len(), 1);
        assert_eq!(journal.records[0].pruned_up_to_height, 1);
        assert_eq!(
            journal.records[0].checkpoint_hash,
            prune_report.checkpoint.checkpoint_hash
        );

        let status_after_prune =
            history_status(HistoryOptions::with_defaults(data_dir.clone())).expect("status");
        assert_eq!(status_after_prune.current_height, 1);
        assert_eq!(status_after_prune.local_block_range.count, 0);
        assert!(status_after_prune.block_log_verified);

        let import_report = import_history_archive_window(HistoryArchiveWindowImportOptions {
            data_dir: data_dir.clone(),
            bundle_file: archive_bundle_file.clone(),
            overwrite: false,
        })
        .expect("import archive window after prune");
        assert!(import_report.imported);
        assert_eq!(import_report.from_height, 1);
        assert_eq!(import_report.to_height, 1);
        assert_eq!(import_report.archived_window_count, 1);
        assert_eq!(import_report.bundle_hash, archive_bundle.bundle_hash);
        assert!(Path::new(&import_report.archive_file).exists());
        let idempotent_import = import_history_archive_window(HistoryArchiveWindowImportOptions {
            data_dir: data_dir.clone(),
            bundle_file: archive_bundle_file,
            overwrite: false,
        })
        .expect("idempotent archive window import");
        assert!(!idempotent_import.imported);
        assert_eq!(idempotent_import.archived_window_count, 1);
        let status_after_import =
            history_status(HistoryOptions::with_defaults(data_dir.clone())).expect("status");
        assert_eq!(status_after_import.current_height, 1);
        assert_eq!(status_after_import.local_block_range.count, 0);
        assert!(status_after_import.block_log_verified);

        let second_batch_file = data_dir.join("second-transfer.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfhistoryprune2000000000000000000000".to_string(),
            amount: ACCOUNT_RESERVE + 15,
            batch_file: second_batch_file.clone(),
        })
        .expect("create second prune batch");
        let second_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: second_batch_file,
            certificate_file: None,
        })
        .expect("apply second prune batch");
        assert!(second_receipts[0].accepted, "{second_receipts:?}");

        let status_after_append =
            history_status(HistoryOptions::with_defaults(data_dir.clone())).expect("status");
        assert_eq!(status_after_append.current_height, 2);
        assert_eq!(status_after_append.local_block_range.first_height, Some(2));
        assert_eq!(status_after_append.local_block_range.last_height, Some(2));
        assert_eq!(status_after_append.local_block_range.count, 1);
        let verify_after_append = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify after post-prune append");
        assert!(verify_after_append.verified);
        assert_eq!(verify_after_append.block_count, 1);

        let mut legacy_checkpoint = checkpoint;
        legacy_checkpoint.schema = "postfiat-history-checkpoint-v1".to_string();
        legacy_checkpoint.native_fee_burn_total = None;
        let trusted_checkpoint_faucet_balance = legacy_checkpoint.ledger.accounts[0].balance;
        legacy_checkpoint.ledger.accounts[0].balance = u64::MAX;
        legacy_checkpoint.checkpoint_hash.clear();
        let legacy_encoded = serde_json::to_vec(&legacy_checkpoint)
            .expect("serialize legacy checkpoint fixture");
        legacy_checkpoint.checkpoint_hash = hash_hex(
            "postfiat.history_checkpoint.v1",
            &legacy_encoded,
        );
        atomic_write(
            data_dir.join(HISTORY_CHECKPOINT_FILE),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&legacy_checkpoint)
                    .expect("serialize legacy checkpoint")
            ),
        )
        .expect("write legacy checkpoint fixture");
        let legacy_error = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("legacy checkpoint without cumulative fee burns must fail closed");
        assert!(
            legacy_error
                .to_string()
                .contains("unsupported history checkpoint schema"),
            "{legacy_error}"
        );

        let legacy_backup_file = data_dir.join("history-checkpoint-v1.backup.json");
        let legacy_file_bytes = std::fs::read(data_dir.join(HISTORY_CHECKPOINT_FILE))
            .expect("read legacy checkpoint before failed rebuild");
        let imported_archive_file = PathBuf::from(&import_report.archive_file);
        let mut tampered_archive_bundle = archive_bundle.clone();
        tampered_archive_bundle.blocks[0].header.block_hash = "tampered-block-hash".to_string();
        atomic_write(
            &imported_archive_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&tampered_archive_bundle)
                    .expect("serialize tampered imported archive fixture")
            ),
        )
        .expect("write tampered imported archive fixture");
        let tampered_error = history_checkpoint_rebuild_from_archive(
            HistoryCheckpointRebuildFromArchiveOptions {
                data_dir: data_dir.clone(),
                backup_file: legacy_backup_file.clone(),
            },
        )
        .expect_err("tampered imported archive must fail before checkpoint mutation");
        assert!(
            tampered_error
                .to_string()
                .contains("history archive window bundle hash mismatch"),
            "{tampered_error}"
        );
        assert_eq!(
            std::fs::read(data_dir.join(HISTORY_CHECKPOINT_FILE))
                .expect("read checkpoint after failed rebuild"),
            legacy_file_bytes
        );
        assert!(!legacy_backup_file.exists());
        atomic_write(
            &imported_archive_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&archive_bundle)
                    .expect("serialize verified imported archive fixture")
            ),
        )
        .expect("restore verified imported archive fixture");

        let rebuild = history_checkpoint_rebuild_from_archive(
            HistoryCheckpointRebuildFromArchiveOptions {
                data_dir: data_dir.clone(),
                backup_file: legacy_backup_file.clone(),
            },
        )
        .expect("rebuild legacy checkpoint from verified imported archive");
        assert!(rebuild.rebuilt);
        assert_eq!(rebuild.legacy_schema, "postfiat-history-checkpoint-v1");
        assert_eq!(rebuild.pruned_up_to_height, 1);
        assert_eq!(rebuild.archive_from_height, 1);
        assert_eq!(rebuild.archive_to_height, 1);
        assert_eq!(rebuild.archive_window_count, 1);
        assert_eq!(rebuild.checkpoint.schema, "postfiat-history-checkpoint-v2");
        assert_eq!(
            rebuild.checkpoint.native_fee_burn_total,
            Some(u128::from(first_receipts[0].fee_burned))
        );
        assert_eq!(
            rebuild.checkpoint.ledger.accounts[0].balance,
            trusted_checkpoint_faucet_balance,
            "legacy economic state must be ignored and reconstructed from archive"
        );
        assert!(legacy_backup_file.exists());
        let rebuilt_verification = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuilt v2 checkpoint verifies retained suffix");
        assert!(rebuilt_verification.verified);
        assert_eq!(rebuilt_verification.block_count, 1);

        fs::remove_dir_all(data_dir).expect("cleanup history prune test");
    }

    #[test]
    fn history_prune_recover_completes_pending_prune_after_checkpoint_write() {
        let data_dir = unique_test_dir("postfiat-history-prune-recover-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-history-prune-recover".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init history prune recovery test");

        let batch_file = data_dir.join("recovery-transfer.batch.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfhistoryrecover00000000000000000000".to_string(),
            amount: ACCOUNT_RESERVE + 15,
            batch_file: batch_file.clone(),
        })
        .expect("create recovery batch");
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply recovery batch");
        assert!(receipts[0].accepted, "{receipts:?}");

        let proof_file = data_dir.join("archive-handoff.json");
        create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
            data_dir: data_dir.clone(),
            from_height: 1,
            to_height: 1,
            archive_uri: Some("archive://controlled-testnet/unit-test-recover".to_string()),
            output_file: proof_file.clone(),
            overwrite: false,
        })
        .expect("create recovery archive handoff");

        let mut options = HistoryOptions::with_defaults(data_dir.clone());
        options.retain_recent_blocks = 0;
        options.minimum_replay_window_blocks = 0;
        options.archive_handoff_file = Some(proof_file.clone());
        let plan = history_prune_plan(options).expect("history prune plan");
        assert!(plan.prune_allowed);

        let store = NodeStore::new(&data_dir);
        let proof: HistoryArchiveHandoffProof =
            read_json_file(&proof_file, "history archive handoff proof").expect("read proof");
        let checkpoint =
            build_history_checkpoint_state(&store, 1, &proof).expect("build checkpoint");
        let artifacts = build_history_prune_artifacts(
            &plan,
            checkpoint,
            store.read_blocks().expect("read blocks"),
            store.read_batch_archive().expect("read archive"),
            store.read_receipts().expect("read receipts"),
        )
        .expect("build prune artifacts");
        write_history_prune_pending_file(&store, &artifacts.pending).expect("write pending");
        write_history_checkpoint_state_file(
            &store.data_dir().join(HISTORY_CHECKPOINT_FILE),
            &artifacts.pending.checkpoint,
        )
        .expect("write checkpoint");

        let recovery = history_prune_recover(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("recover pending prune");
        assert!(recovery.recovered);
        assert_eq!(recovery.pruned_up_to_height, Some(1));
        assert_eq!(recovery.pruned_block_count, Some(1));
        assert_eq!(
            recovery.after_block_range.as_ref().map(|range| range.count),
            Some(0)
        );
        assert!(recovery
            .verify_after_recovery
            .as_ref()
            .is_some_and(|report| report.verified && report.block_count == 0));
        assert!(!store.data_dir().join(HISTORY_PRUNE_PENDING_FILE).exists());
        let journal = read_history_prune_journal(&store).expect("read prune journal");
        assert_eq!(journal.records.len(), 1);
        let status_after_recovery =
            history_status(HistoryOptions::with_defaults(data_dir.clone())).expect("status");
        assert_eq!(status_after_recovery.current_height, 1);
        assert_eq!(status_after_recovery.local_block_range.count, 0);
        assert!(status_after_recovery.block_log_verified);

        fs::remove_dir_all(data_dir).expect("cleanup history prune recovery test");
    }

    #[test]
    fn governance_amendment_lifecycle_fails_closed_before_ordering() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-governance-amendment-lifecycle-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init governance amendment lifecycle test");

        let validators = vec!["validator-0".to_string()];
        let delayed_amendment_file = data_dir.join("crypto-policy-v3-delayed.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 3,
            activation_height: 3,
            veto_until_height: 2,
            paused: false,
            amendment_file: delayed_amendment_file.clone(),
        })
        .expect("ratify delayed crypto policy");
        let delayed_batch_file = data_dir.join("crypto-policy-v3-delayed.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(delayed_amendment_file),
            registry_update_file: None,
            batch_file: delayed_batch_file.clone(),
        })
        .expect("create delayed governance batch");

        let early_error = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: delayed_batch_file.clone(),
            certificate_file: None,
        })
        .expect_err("delayed amendment should not order inside veto window");
        assert!(
            early_error
                .to_string()
                .contains("governance_amendment_veto_window"),
            "{early_error}"
        );

        let immediate_crypto_file = data_dir.join("crypto-policy-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: immediate_crypto_file.clone(),
        })
        .expect("ratify immediate crypto policy");
        let immediate_crypto_batch = data_dir.join("crypto-policy-v2.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(immediate_crypto_file),
            registry_update_file: None,
            batch_file: immediate_crypto_batch.clone(),
        })
        .expect("create immediate crypto batch");
        assert!(
            apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file: immediate_crypto_batch,
                certificate_file: None,
            })
            .expect("apply immediate crypto batch")[0]
                .accepted
        );

        let immediate_bridge_file = data_dir.join("bridge-witness-epoch-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: postfiat_types::GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: immediate_bridge_file.clone(),
        })
        .expect("ratify immediate bridge epoch");
        let immediate_bridge_batch = data_dir.join("bridge-witness-epoch-v2.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(immediate_bridge_file),
            registry_update_file: None,
            batch_file: immediate_bridge_batch.clone(),
        })
        .expect("create immediate bridge batch");
        assert!(
            apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file: immediate_bridge_batch,
                certificate_file: None,
            })
            .expect("apply immediate bridge batch")[0]
                .accepted
        );

        let delayed_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: delayed_batch_file,
            certificate_file: None,
        })
        .expect("apply delayed amendment after activation height");
        assert!(delayed_receipts[0].accepted, "{delayed_receipts:?}");
        let governance_report = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify delayed governance");
        assert_eq!(governance_report.crypto_policy_version, 3);
        assert_eq!(governance_report.bridge_witness_epoch, 2);
        assert_eq!(governance_report.amendment_count, 3);
        assert_eq!(governance_report.amendment_activation_record_count, 3);
        assert!(!governance_report
            .latest_amendment_activation_record_id
            .is_empty());
        assert_eq!(governance_report.amendment_supersession_record_count, 1);
        assert!(!governance_report
            .latest_amendment_supersession_record_id
            .is_empty());

        let store = NodeStore::new(&data_dir);
        let governance_before_tamper = store
            .read_governance()
            .expect("governance before activation record tamper");
        let latest_activation = governance_before_tamper
            .amendment_activation_records
            .last()
            .expect("latest activation record");
        assert_eq!(latest_activation.kind, GOVERNANCE_KIND_CRYPTO_POLICY);
        assert_eq!(latest_activation.previous_value, 2);
        assert_eq!(latest_activation.new_value, 3);
        assert_eq!(latest_activation.activation_height, 3);
        assert_eq!(latest_activation.veto_until_height, 2);
        assert_eq!(latest_activation.activated_height, 3);
        assert_eq!(
            latest_activation.activation_record_id,
            governance_report.latest_amendment_activation_record_id
        );
        let latest_supersession = governance_before_tamper
            .amendment_supersession_records
            .last()
            .expect("latest supersession record");
        assert_eq!(
            latest_supersession.superseded_amendment_id,
            governance_before_tamper.amendments[0].amendment_id
        );
        assert_eq!(
            latest_supersession.superseding_amendment_id,
            governance_before_tamper.amendments[2].amendment_id
        );
        assert_eq!(latest_supersession.kind, GOVERNANCE_KIND_CRYPTO_POLICY);
        assert_eq!(latest_supersession.previous_value, 2);
        assert_eq!(latest_supersession.new_value, 3);
        assert_eq!(latest_supersession.supersession_height, 3);
        assert_eq!(
            latest_supersession.supersession_record_id,
            governance_report.latest_amendment_supersession_record_id
        );

        let mut governance_with_tampered_activation = governance_before_tamper.clone();
        governance_with_tampered_activation.amendment_activation_records[2].previous_value = 99;
        store
            .write_governance(&governance_with_tampered_activation)
            .expect("write tampered activation record");
        let activation_tamper_error = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("tampered activation record should fail governance verification");
        assert!(
            activation_tamper_error
                .to_string()
                .contains("activation record id mismatch"),
            "{activation_tamper_error}"
        );
        store
            .write_governance(&governance_before_tamper)
            .expect("restore activation record");
        let mut governance_with_tampered_supersession = governance_before_tamper.clone();
        governance_with_tampered_supersession.amendment_supersession_records[0].previous_value = 99;
        store
            .write_governance(&governance_with_tampered_supersession)
            .expect("write tampered supersession record");
        let supersession_tamper_error = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("tampered supersession record should fail governance verification");
        assert!(
            supersession_tamper_error
                .to_string()
                .contains("supersession record id mismatch"),
            "{supersession_tamper_error}"
        );
        store
            .write_governance(&governance_before_tamper)
            .expect("restore supersession record");

        let rollback_crypto_file = data_dir.join("crypto-policy-v2-rollback.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 4,
            veto_until_height: 3,
            paused: false,
            amendment_file: rollback_crypto_file.clone(),
        })
        .expect("ratify rollback crypto policy");
        let rollback_crypto_batch = data_dir.join("crypto-policy-v2-rollback.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(rollback_crypto_file),
            registry_update_file: None,
            batch_file: rollback_crypto_batch.clone(),
        })
        .expect("create rollback crypto batch");
        let rollback_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: rollback_crypto_batch,
            certificate_file: None,
        })
        .expect("apply rollback crypto batch");
        assert!(rollback_receipts[0].accepted, "{rollback_receipts:?}");
        let rollback_report = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify rollback governance");
        assert_eq!(rollback_report.crypto_policy_version, 2);
        assert_eq!(rollback_report.amendment_count, 4);
        assert_eq!(rollback_report.amendment_activation_record_count, 4);
        assert_eq!(rollback_report.amendment_supersession_record_count, 2);
        assert_eq!(rollback_report.amendment_rollback_record_count, 1);
        assert!(!rollback_report
            .latest_amendment_rollback_record_id
            .is_empty());

        let governance_after_rollback = store.read_governance().expect("governance after rollback");
        let latest_rollback = governance_after_rollback
            .amendment_rollback_records
            .last()
            .expect("latest rollback record");
        assert_eq!(
            latest_rollback.rolled_back_amendment_id,
            governance_after_rollback.amendments[2].amendment_id
        );
        assert_eq!(
            latest_rollback.restored_amendment_id,
            governance_after_rollback.amendments[0].amendment_id
        );
        assert_eq!(
            latest_rollback.rollback_amendment_id,
            governance_after_rollback.amendments[3].amendment_id
        );
        assert_eq!(latest_rollback.kind, GOVERNANCE_KIND_CRYPTO_POLICY);
        assert_eq!(latest_rollback.previous_value, 3);
        assert_eq!(latest_rollback.restored_value, 2);
        assert_eq!(latest_rollback.rollback_height, 4);
        assert_eq!(
            latest_rollback.rollback_record_id,
            rollback_report.latest_amendment_rollback_record_id
        );

        let mut governance_with_tampered_rollback = governance_after_rollback.clone();
        governance_with_tampered_rollback.amendment_rollback_records[0].previous_value = 99;
        store
            .write_governance(&governance_with_tampered_rollback)
            .expect("write tampered rollback record");
        let rollback_tamper_error = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("tampered rollback record should fail governance verification");
        assert!(
            rollback_tamper_error
                .to_string()
                .contains("rollback record id mismatch"),
            "{rollback_tamper_error}"
        );
        store
            .write_governance(&governance_after_rollback)
            .expect("restore rollback record");

        let paused_amendment_file = data_dir.join("bridge-witness-epoch-v4-paused.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators,
            kind: postfiat_types::GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH.to_string(),
            value: 4,
            activation_height: 4,
            veto_until_height: 3,
            paused: true,
            amendment_file: paused_amendment_file.clone(),
        })
        .expect("ratify paused amendment");
        let paused_batch_file = data_dir.join("bridge-witness-epoch-v4-paused.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(paused_amendment_file),
            registry_update_file: None,
            batch_file: paused_batch_file.clone(),
        })
        .expect("create paused governance batch");
        let paused_error = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: paused_batch_file,
            certificate_file: None,
        })
        .expect_err("paused amendment should not order");
        assert!(
            paused_error
                .to_string()
                .contains("governance_amendment_paused"),
            "{paused_error}"
        );

        fs::remove_dir_all(data_dir).expect("cleanup governance amendment lifecycle test");
    }

    #[test]
    fn governance_policy_updates_and_bridge_witness_epoch_are_ordered() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-governance-policy-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init governance policy test");

        let validators = vec!["validator-0".to_string()];
        let crypto_amendment_file = data_dir.join("crypto-policy-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: crypto_amendment_file.clone(),
        })
        .expect("ratify crypto policy");
        let crypto_batch_file = data_dir.join("crypto-policy-v2.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(crypto_amendment_file),
            registry_update_file: None,
            batch_file: crypto_batch_file.clone(),
        })
        .expect("create crypto policy batch");
        let crypto_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: crypto_batch_file,

            certificate_file: None,
        })
        .expect("apply crypto policy batch");
        assert!(crypto_receipts[0].accepted, "{crypto_receipts:?}");

        let bridge_epoch_amendment_file = data_dir.join("bridge-witness-epoch-v2.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators,
            kind: postfiat_types::GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH.to_string(),
            value: 2,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: bridge_epoch_amendment_file.clone(),
        })
        .expect("ratify bridge witness epoch");
        let bridge_epoch_batch_file = data_dir.join("bridge-witness-epoch-v2.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(bridge_epoch_amendment_file),
            registry_update_file: None,
            batch_file: bridge_epoch_batch_file.clone(),
        })
        .expect("create bridge witness epoch batch");
        let bridge_epoch_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: bridge_epoch_batch_file,

            certificate_file: None,
        })
        .expect("apply bridge witness epoch batch");
        assert!(
            bridge_epoch_receipts[0].accepted,
            "{bridge_epoch_receipts:?}"
        );

        let governance_report = verify_governance(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify policy governance");
        assert_eq!(governance_report.crypto_policy_version, 2);
        assert_eq!(governance_report.bridge_witness_epoch, 2);
        assert_eq!(governance_report.amendment_count, 2);

        let bridge_domain_batch_file = data_dir.join("bridge-domain.batch.json");
        create_bridge_domain_batch(BridgeDomainBatchOptions {
            data_dir: data_dir.clone(),
            domain_id: "governed-bridge".to_string(),
            name: "Governed Bridge".to_string(),
            source_chain: "xrpl-governed".to_string(),
            target_chain: "postfiat-local".to_string(),
            bridge_id: "governed-bridge".to_string(),
            door_account: "door:governed-bridge".to_string(),
            inbound_cap: 100,
            outbound_cap: 100,
            batch_file: bridge_domain_batch_file.clone(),
        })
        .expect("create bridge domain batch");
        let domain_receipts = apply_bridge_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: bridge_domain_batch_file,

            certificate_file: None,
        })
        .expect("apply bridge domain batch");
        assert!(domain_receipts[0].accepted, "{domain_receipts:?}");

        let stale_epoch_batch_file = data_dir.join("bridge-transfer-stale-epoch.batch.json");
        create_bridge_transfer_batch(BridgeTransferBatchOptions {
            data_dir: data_dir.clone(),
            domain_id: "governed-bridge".to_string(),
            direction: "inbound".to_string(),
            from: "external:alice".to_string(),
            to: "pfgoverned".to_string(),
            asset_id: "POSTFIAT".to_string(),
            amount: 1,
            witness_id: "governed-witness-stale".to_string(),
            witness_epoch: Some(1),
            witness_signer: DEFAULT_BRIDGE_WITNESS_SIGNER.to_string(),
            batch_file: stale_epoch_batch_file.clone(),
        })
        .expect("create stale witness epoch batch");
        let stale_epoch_receipts = apply_bridge_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: stale_epoch_batch_file,

            certificate_file: None,
        })
        .expect("apply stale epoch bridge batch");
        assert!(
            !stale_epoch_receipts[0].accepted,
            "{stale_epoch_receipts:?}"
        );
        assert_eq!(stale_epoch_receipts[0].code, "bad_witness_epoch");

        let current_epoch_batch_file = data_dir.join("bridge-transfer-current-epoch.batch.json");
        let current_epoch_batch = create_bridge_transfer_batch(BridgeTransferBatchOptions {
            data_dir: data_dir.clone(),
            domain_id: "governed-bridge".to_string(),
            direction: "inbound".to_string(),
            from: "external:alice".to_string(),
            to: "pfgoverned".to_string(),
            asset_id: "POSTFIAT".to_string(),
            amount: 2,
            witness_id: "governed-witness-current".to_string(),
            witness_epoch: None,
            witness_signer: DEFAULT_BRIDGE_WITNESS_SIGNER.to_string(),
            batch_file: current_epoch_batch_file.clone(),
        })
        .expect("create current witness epoch batch");
        match &current_epoch_batch.actions[0] {
            BridgeAction::Transfer(action) => {
                assert_eq!(action.witness_epoch, 2);
                assert!(action.witness_attestation.is_some());
            }
            other => panic!("expected transfer action, got {other:?}"),
        }
        let current_epoch_receipts = apply_bridge_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: current_epoch_batch_file,

            certificate_file: None,
        })
        .expect("apply current epoch bridge batch");
        assert!(
            current_epoch_receipts[0].accepted,
            "{current_epoch_receipts:?}"
        );

        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify policy bridge blocks");

        std::fs::remove_dir_all(data_dir).expect("cleanup governance policy test");
    }

    #[test]
    fn snapshot_export_rejects_direct_state_mutation() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-direct-snapshot-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let snapshot_dir = data_dir.with_file_name("postfiat-node-direct-snapshot");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init direct snapshot test");

        let receipt = transfer(TransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfdirectsnapshot00000000000000000000000".to_string(),
            amount: ACCOUNT_RESERVE,
        })
        .expect("direct transfer");
        assert!(receipt.accepted, "{receipt:?}");

        let error = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect_err("direct mutation should block snapshot export");
        assert!(
            error
                .to_string()
                .contains("snapshot export block verification failed"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup direct snapshot data");
        if snapshot_dir.exists() {
            std::fs::remove_dir_all(snapshot_dir).expect("cleanup direct snapshot");
        }
    }

    #[test]
    fn snapshot_export_rejects_tampered_governance_state() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-governance-snapshot-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let snapshot_dir = data_dir.with_file_name("postfiat-node-governance-snapshot");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init governance snapshot test");
        let amendment_file = data_dir.join("validator-set-amendment.json");
        let batch_file = data_dir.join("validator-set-amendment.batch.json");
        ratify_validator_set(RatifyValidatorSetOptions {
            data_dir: data_dir.clone(),
            validators: vec!["validator-0".to_string()],
            support: vec!["validator-0".to_string()],
            validator_count: 1,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: amendment_file.clone(),
        })
        .expect("ratify governance amendment");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(amendment_file),
            registry_update_file: None,
            batch_file: batch_file.clone(),
        })
        .expect("create governance batch");
        let receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply governance batch");
        assert!(receipts[0].accepted, "{receipts:?}");

        let store = NodeStore::new(&data_dir);
        let mut governance = store.read_governance().expect("governance before tamper");
        governance.amendments[0].votes[0].vote_id = "snapshot-tampered-governance-vote".to_string();
        store
            .write_governance(&governance)
            .expect("write tampered governance state");
        let error = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect_err("tampered governance should block snapshot export");
        assert!(
            error
                .to_string()
                .contains("snapshot export governance verification failed"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup governance snapshot data");
        if snapshot_dir.exists() {
            std::fs::remove_dir_all(snapshot_dir).expect("cleanup governance snapshot");
        }
    }

    #[test]
    fn snapshot_export_rejects_tampered_shielded_state() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-shielded-snapshot-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let snapshot_dir = data_dir.with_file_name("postfiat-node-shielded-snapshot");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init shielded snapshot test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("snapshot genesis");
        let mut shielded = store.read_shielded().expect("empty shielded snapshot state");
        mint_debug_note_with_creator_for_chain(
            &mut shielded,
            postfiat_privacy::ShieldedChainContext {
                chain_id: &genesis.chain_id,
                genesis_hash: &genesis_hash(&genesis),
            },
            "shielded-snapshot-owner".to_string(),
            "POSTFIAT".to_string(),
            11,
            "historical snapshot fixture".to_string(),
            "test-only-historical-import".to_string(),
        )
        .expect("seed internally consistent historical state");
        store
            .write_shielded(&shielded)
            .expect("write historical snapshot fixture");
        shielded.turnstile_events[0].event_id = "tampered-shielded-turnstile-event".to_string();
        store
            .write_shielded(&shielded)
            .expect("write tampered shielded state");
        let error = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect_err("tampered shielded state should block snapshot export");
        assert!(
            error
                .to_string()
                .contains("snapshot export shielded verification failed"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup shielded snapshot data");
        if snapshot_dir.exists() {
            std::fs::remove_dir_all(snapshot_dir).expect("cleanup shielded snapshot");
        }
    }

    #[test]
    fn snapshot_export_rejects_tampered_mempool_state() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-mempool-snapshot-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let snapshot_dir = data_dir.with_file_name("postfiat-node-mempool-snapshot");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init mempool snapshot test");
        submit_transfer_to_mempool(TransferOptions {
            data_dir: data_dir.clone(),
            key_file: None,
            to: "pfmempoolsnapshot000000000000000000000".to_string(),
            amount: ACCOUNT_RESERVE,
        })
        .expect("submit mempool transfer");

        let store = NodeStore::new(&data_dir);
        let mut mempool = store.read_mempool().expect("mempool before tamper");
        mempool.pending[0].tx_id = "snapshot-tampered-mempool-tx".to_string();
        store
            .write_mempool(&mempool)
            .expect("write tampered mempool");
        let error = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect_err("tampered mempool should block snapshot export");
        assert!(
            error
                .to_string()
                .contains("snapshot export mempool verification failed"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup mempool snapshot data");
        if snapshot_dir.exists() {
            std::fs::remove_dir_all(snapshot_dir).expect("cleanup mempool snapshot");
        }
    }

    #[test]
    fn verify_blocks_rejects_tampered_genesis_faucet_account() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-faucet-account-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init faucet account test");

        let faucet_account_path = data_dir.join(FAUCET_ACCOUNT_FILE);
        let mut faucet_account =
            read_faucet_account_file(&faucet_account_path).expect("read faucet account");
        let mut invalid_key_account = faucet_account.clone();
        invalid_key_account.public_key_hex = Some("not-hex".to_string());
        let invalid_key_json =
            serde_json::to_string_pretty(&invalid_key_account).expect("invalid faucet json");
        atomic_write(&faucet_account_path, format!("{invalid_key_json}\n"))
            .expect("write invalid faucet public key");
        let invalid_key_error = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("invalid faucet public key should fail genesis replay");
        assert!(
            invalid_key_error.to_string().contains("invalid hex"),
            "{invalid_key_error}"
        );
        write_faucet_account_file(&faucet_account_path, &faucet_account)
            .expect("restore faucet account after invalid public key");
        faucet_account.balance = faucet_account
            .balance
            .checked_sub(1)
            .expect("faucet balance should be positive");
        let tampered_faucet_json =
            serde_json::to_string_pretty(&faucet_account).expect("tampered faucet json");
        atomic_write(&faucet_account_path, format!("{tampered_faucet_json}\n"))
            .expect("write tampered faucet account without validation");

        let error = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("tampered faucet account should fail genesis replay");
        assert!(
            error.to_string().contains("genesis native supply"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup faucet account test");
    }

    #[test]
    fn verify_blocks_rejects_coordinated_genesis_native_supply_rewrite() {
        let data_dir = unique_test_dir("postfiat-genesis-native-supply-rewrite-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init native supply rewrite test");

        let store = NodeStore::new(&data_dir);
        let faucet_account_path = data_dir.join(FAUCET_ACCOUNT_FILE);
        let mut rewritten_faucet =
            read_faucet_account_file(&faucet_account_path).expect("read genesis faucet account");
        rewritten_faucet.balance = rewritten_faucet
            .balance
            .checked_sub(1)
            .expect("genesis faucet balance is positive");
        let rewritten_faucet_json =
            serde_json::to_string_pretty(&rewritten_faucet).expect("rewritten faucet json");
        atomic_write(
            &faucet_account_path,
            format!("{rewritten_faucet_json}\n"),
        )
        .expect("rewrite replay-base faucet account without validation");

        let mut rewritten_ledger = store.read_ledger().expect("read height-zero ledger");
        rewritten_ledger.accounts[0].balance = rewritten_faucet.balance;
        store
            .write_ledger(&rewritten_ledger)
            .expect("rewrite height-zero ledger consistently");

        let error = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect_err("coordinated genesis native-supply rewrite must fail closed");
        assert!(
            error.to_string().contains("genesis native supply"),
            "{error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup native supply rewrite test");
    }

    #[test]
    fn operator_manifest_verify_rejects_tamper_and_private_material() {
        let data_dir = unique_test_dir("postfiat-operator-manifest-test");
        let manifest_dir = data_dir.with_file_name("postfiat-operator-manifest-test-manifests");
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init operator manifest test");
        let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let hot_key = validator_registry_record(&registry, "validator-0")
            .expect("validator registry record")
            .public_key_hex
            .clone();
        let master_key_file = manifest_dir.join("validator-0.master-key.json");
        let manifest_file = manifest_dir.join("validator-0.operator-manifest.json");
        write_test_master_key(&master_key_file, [31u8; 32]);
        let manifest = create_operator_manifest(OperatorManifestCreateOptions {
            master_key_file,
            chain_id: "postfiat-local".to_string(),
            network: "controlled-testnet".to_string(),
            validator_id: "validator-0".to_string(),
            hot_public_key_hex: hot_key,
            operator: "operator-zero".to_string(),
            contact: "validator-0@operators.example".to_string(),
            provider_group: "provider-a".to_string(),
            region_group: "region-a".to_string(),
            jurisdiction_group: "jurisdiction-a".to_string(),
            legal_domain_group: "legal-a".to_string(),
            funding_domain_group: "funding-a".to_string(),
            rotation_state: "active".to_string(),
            effective_height: 0,
            trust_graph_root: None,
            trust_graph_version: None,
            trust_view_id: None,
            trust_view_version: None,
            output_file: manifest_file.clone(),
            overwrite: false,
        })
        .expect("create operator manifest");
        let manifest_json =
            std::fs::read_to_string(&manifest_file).expect("read created operator manifest");
        assert!(!manifest_json.contains("private_key_hex"));
        assert!(!manifest_json.contains("seed_hex"));

        let report = verify_operator_manifest(OperatorManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
        })
        .expect("verify operator manifest");
        assert!(report.verified);
        assert_eq!(report.manifest_hash, manifest.manifest_hash);
        assert_eq!(report.validator_id, "validator-0");
        assert!(report.manifest_signer_matches_master);
        assert!(report.signature_verified);
        assert!(report.redaction_checked);

        let mut tampered = manifest.clone();
        tampered.operator = "operator-tampered".to_string();
        write_test_operator_manifest(&manifest_file, &tampered);
        let tamper_error = verify_operator_manifest(OperatorManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
        })
        .expect_err("tampered operator manifest should fail");
        assert!(
            tamper_error.to_string().contains("signature verification"),
            "{tamper_error}"
        );

        atomic_write(
            &manifest_file,
            r#"{"private_key_hex":"not allowed in operator manifests"}"#,
        )
        .expect("write private material marker");
        let private_material_error =
            verify_operator_manifest(OperatorManifestVerifyOptions { manifest_file })
                .expect_err("operator manifest private material marker should fail");
        assert!(
            private_material_error
                .to_string()
                .contains("private material marker"),
            "{private_material_error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup operator manifest test data");
        std::fs::remove_dir_all(manifest_dir).expect("cleanup operator manifest test manifests");
    }

    #[test]
    fn operator_manifest_signs_cobalt_trust_metadata() {
        let data_dir = unique_test_dir("postfiat-operator-manifest-cobalt-test");
        let manifest_dir =
            data_dir.with_file_name("postfiat-operator-manifest-cobalt-test-manifests");
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init operator manifest Cobalt test");
        let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let hot_key = validator_registry_record(&registry, "validator-0")
            .expect("validator registry record")
            .public_key_hex
            .clone();
        let master_key_file = manifest_dir.join("validator-0.master-key.json");
        let manifest_file = manifest_dir.join("validator-0.operator-manifest.json");
        write_test_master_key(&master_key_file, [33u8; 32]);
        let cobalt_trust = test_cobalt_trust_binding("graph-7", 7, "validator-0-view-3", 3);
        let manifest = create_operator_manifest(OperatorManifestCreateOptions {
            master_key_file,
            chain_id: "postfiat-local".to_string(),
            network: "controlled-testnet".to_string(),
            validator_id: "validator-0".to_string(),
            hot_public_key_hex: hot_key,
            operator: "operator-zero".to_string(),
            contact: "validator-0@operators.example".to_string(),
            provider_group: "provider-a".to_string(),
            region_group: "region-a".to_string(),
            jurisdiction_group: "jurisdiction-a".to_string(),
            legal_domain_group: "legal-a".to_string(),
            funding_domain_group: "funding-a".to_string(),
            rotation_state: "active".to_string(),
            effective_height: 0,
            trust_graph_root: Some(cobalt_trust.trust_graph_root.clone()),
            trust_graph_version: Some(cobalt_trust.trust_graph_version),
            trust_view_id: Some(cobalt_trust.trust_view_id.clone()),
            trust_view_version: Some(cobalt_trust.trust_view_version),
            output_file: manifest_file.clone(),
            overwrite: false,
        })
        .expect("create operator manifest with Cobalt trust");
        assert_eq!(manifest.cobalt_trust.as_ref(), Some(&cobalt_trust));

        let report = verify_operator_manifest(OperatorManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
        })
        .expect("verify operator manifest with Cobalt trust");
        assert!(report.verified);
        assert_eq!(report.cobalt_trust.as_ref(), Some(&cobalt_trust));

        let mut tampered = manifest.clone();
        tampered.cobalt_trust.as_mut().unwrap().trust_view_id =
            test_cobalt_trust_binding("graph-7", 7, "validator-0-view-tampered", 3)
                .trust_view_id;
        write_test_operator_manifest(&manifest_file, &tampered);
        let tamper_error = verify_operator_manifest(OperatorManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
        })
        .expect_err("tampered Cobalt trust metadata should fail");
        assert!(
            tamper_error.to_string().contains("signature verification"),
            "{tamper_error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup operator manifest Cobalt test data");
        std::fs::remove_dir_all(manifest_dir)
            .expect("cleanup operator manifest Cobalt test manifests");
    }

    #[test]
    fn governance_genesis_bundle_binds_registry_and_operator_manifests() {
        let root_dir = unique_test_dir("postfiat-governance-genesis-bundle-test");
        let data_dir = root_dir.join("node");
        let manifest_dir = root_dir.join("manifests");
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init governance genesis bundle test");
        let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let validators = local_validator_ids(3).expect("validator ids");
        for (index, validator_id) in validators.iter().enumerate() {
            let hot_key = validator_registry_record(&registry, validator_id)
                .expect("validator registry record")
                .public_key_hex
                .clone();
            let manifest = signed_test_operator_manifest(
                "postfiat-local",
                "controlled-testnet",
                validator_id,
                &hot_key,
                [(index as u8) + 41; 32],
                &format!("operator-{index}"),
            );
            write_test_operator_manifest(
                &manifest_dir.join(format!("{validator_id}.operator-manifest.json")),
                &manifest,
            );
        }

        let quorum = bft_quorum_threshold(validators.len()).expect("quorum");
        let bundle_file = root_dir.join("genesis-governance-bundle.json");
        let bundle = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
            data_dir: data_dir.clone(),
            manifest_dir: manifest_dir.clone(),
            validators: validators.clone(),
            quorum,
            network: "controlled-testnet".to_string(),
            output_file: bundle_file.clone(),
        })
        .expect("create governance genesis bundle");
        assert_eq!(bundle.schema, GOVERNANCE_GENESIS_BUNDLE_SCHEMA);
        assert_eq!(bundle.validator_count, 3);
        assert_eq!(bundle.operator_manifests.len(), 3);
        assert_eq!(
            bundle.operator_manifests[0].manifest_file,
            "manifests/validator-0.operator-manifest.json"
        );
        assert_eq!(
            bundle.registry_root,
            validator_registry_root(&registry, &validators).expect("registry root")
        );

        let report = verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
            data_dir: data_dir.clone(),
            bundle_file: bundle_file.clone(),
        })
        .expect("verify governance genesis bundle");
        assert!(report.verified);
        assert_eq!(report.bundle_hash, bundle.bundle_hash);
        assert_eq!(report.validator_count, 3);
        assert!(report.operator_manifests_verified);

        let replacement = signed_test_operator_manifest(
            "postfiat-local",
            "controlled-testnet",
            "validator-0",
            &validator_registry_record(&registry, "validator-0")
                .expect("validator registry record")
                .public_key_hex,
            [99u8; 32],
            "operator-zero-replacement",
        );
        write_test_operator_manifest(
            &manifest_dir.join("validator-0.operator-manifest.json"),
            &replacement,
        );
        let tamper_error = verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
            data_dir: data_dir.clone(),
            bundle_file,
        })
        .expect_err("tampered referenced operator manifest should fail");
        assert!(
            tamper_error
                .to_string()
                .contains("operator manifest hash mismatch"),
            "{tamper_error}"
        );

        std::fs::remove_dir_all(root_dir).expect("cleanup governance genesis bundle test");
    }

    #[test]
    fn governance_genesis_bundle_rejects_duplicate_and_stale_cobalt_views() {
        let root_dir = unique_test_dir("postfiat-governance-genesis-cobalt-test");
        let data_dir = root_dir.join("node");
        let manifest_dir = root_dir.join("manifests");
        fs::create_dir_all(&manifest_dir).expect("create manifest dir");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init governance genesis Cobalt test");
        let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
        let validators = local_validator_ids(3).expect("validator ids");
        let graph_root = test_cobalt_trust_binding("graph-11", 11, "unused", 1).trust_graph_root;
        let graph_version = 11;
        let valid_bindings = validators
            .iter()
            .enumerate()
            .map(|(index, validator_id)| OperatorCobaltTrustBinding {
                trust_graph_root: graph_root.clone(),
                trust_graph_version: graph_version,
                trust_view_id: test_cobalt_trust_binding(
                    "graph-11",
                    graph_version,
                    &format!("{validator_id}-view"),
                    (index as u64) + 1,
                )
                .trust_view_id,
                trust_view_version: (index as u64) + 1,
            })
            .collect::<Vec<_>>();

        for (index, validator_id) in validators.iter().enumerate() {
            let hot_key = validator_registry_record(&registry, validator_id)
                .expect("validator registry record")
                .public_key_hex
                .clone();
            let manifest = signed_test_operator_manifest_with_cobalt_trust(
                "postfiat-local",
                "controlled-testnet",
                validator_id,
                &hot_key,
                [(index as u8) + 51; 32],
                &format!("operator-{index}"),
                Some(valid_bindings[index].clone()),
            );
            write_test_operator_manifest(
                &manifest_dir.join(format!("{validator_id}.operator-manifest.json")),
                &manifest,
            );
        }

        let quorum = bft_quorum_threshold(validators.len()).expect("quorum");
        let bundle_file = root_dir.join("genesis-governance-bundle-cobalt.json");
        let bundle = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
            data_dir: data_dir.clone(),
            manifest_dir: manifest_dir.clone(),
            validators: validators.clone(),
            quorum,
            network: "controlled-testnet".to_string(),
            output_file: bundle_file.clone(),
        })
        .expect("create governance genesis bundle with Cobalt trust");
        assert_eq!(
            bundle.operator_manifests[0].cobalt_trust.as_ref(),
            Some(&valid_bindings[0])
        );
        verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
            data_dir: data_dir.clone(),
            bundle_file: bundle_file.clone(),
        })
        .expect("verify governance genesis bundle with Cobalt trust");

        let duplicate_manifest = signed_test_operator_manifest_with_cobalt_trust(
            "postfiat-local",
            "controlled-testnet",
            "validator-2",
            &validator_registry_record(&registry, "validator-2")
                .expect("validator registry record")
                .public_key_hex,
            [99u8; 32],
            "operator-two-duplicate-view",
            Some(valid_bindings[1].clone()),
        );
        write_test_operator_manifest(
            &manifest_dir.join("validator-2.operator-manifest.json"),
            &duplicate_manifest,
        );
        let duplicate_error = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
            data_dir: data_dir.clone(),
            manifest_dir: manifest_dir.clone(),
            validators: validators.clone(),
            quorum,
            network: "controlled-testnet".to_string(),
            output_file: root_dir.join("duplicate-view-bundle.json"),
        })
        .expect_err("duplicate trust view id should fail");
        assert!(
            duplicate_error.to_string().contains("duplicate Cobalt trust view id"),
            "{duplicate_error}"
        );

        let stale_binding = OperatorCobaltTrustBinding {
            trust_graph_root: test_cobalt_trust_binding("graph-10", 10, "unused", 1)
                .trust_graph_root,
            trust_graph_version: 10,
            trust_view_id: valid_bindings[2].trust_view_id.clone(),
            trust_view_version: valid_bindings[2].trust_view_version,
        };
        let stale_manifest = signed_test_operator_manifest_with_cobalt_trust(
            "postfiat-local",
            "controlled-testnet",
            "validator-2",
            &validator_registry_record(&registry, "validator-2")
                .expect("validator registry record")
                .public_key_hex,
            [100u8; 32],
            "operator-two-stale-view",
            Some(stale_binding),
        );
        write_test_operator_manifest(
            &manifest_dir.join("validator-2.operator-manifest.json"),
            &stale_manifest,
        );
        let stale_error = create_governance_genesis_bundle(GovernanceGenesisBundleOptions {
            data_dir: data_dir.clone(),
            manifest_dir: manifest_dir.clone(),
            validators,
            quorum,
            network: "controlled-testnet".to_string(),
            output_file: root_dir.join("stale-view-bundle.json"),
        })
        .expect_err("stale trust graph metadata should fail");
        assert!(
            stale_error
                .to_string()
                .contains("stale or mixed Cobalt trust graph metadata"),
            "{stale_error}"
        );

        std::fs::remove_dir_all(root_dir).expect("cleanup governance genesis Cobalt test");
    }

    #[cfg(unix)]
    #[test]
    fn init_writes_private_key_files_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-private-key-mode-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init private key mode test");
        validator_keys(ValidatorKeysOptions {
            data_dir: data_dir.clone(),
            validators: 4,
            local_only: false,
        })
        .expect("expand validator keys");

        for file_name in [FAUCET_KEY_FILE, VALIDATOR_KEYS_FILE] {
            let file_path = data_dir.join(file_name);
            let mode = std::fs::metadata(&file_path)
                .expect("private key metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600, "{file_name} mode should be 0600");

            let mut permissions = std::fs::metadata(&file_path)
                .expect("private key metadata for permission regression")
                .permissions();
            permissions.set_mode(0o644);
            std::fs::set_permissions(&file_path, permissions)
                .expect("weaken private key permissions");
            let permission_error = validate_local_keys(ValidatorKeysOptions {
                data_dir: data_dir.clone(),
                validators: 4,
                local_only: false,
            })
            .expect_err("local key validation should reject loose private key permissions");
            assert!(
                permission_error.to_string().contains("expected 600"),
                "{permission_error}"
            );

            let mut permissions = std::fs::metadata(&file_path)
                .expect("private key metadata for permission restore")
                .permissions();
            permissions.set_mode(0o600);
            std::fs::set_permissions(&file_path, permissions)
                .expect("restore private key permissions");
        }

        std::fs::remove_dir_all(data_dir).expect("cleanup private key mode test");
    }

    #[test]
    fn snapshot_import_rejects_bad_manifest_file_set() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-node-manifest-file-set-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let snapshot_dir = data_dir.with_file_name("postfiat-node-manifest-file-set-snapshot");
        let missing_restored_dir =
            data_dir.with_file_name("postfiat-node-manifest-file-set-missing-restored");
        let duplicate_restored_dir =
            data_dir.with_file_name("postfiat-node-manifest-file-set-duplicate-restored");
        let escape_restored_dir =
            data_dir.with_file_name("postfiat-node-manifest-file-set-escape-restored");

        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init manifest file set test");
        let manifest = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export manifest file set snapshot");

        let manifest_path = snapshot_dir.join(SNAPSHOT_MANIFEST_FILE);
        let mut missing_manifest = manifest.clone();
        missing_manifest
            .files
            .retain(|file| file.name != LEDGER_FILE);
        write_snapshot_manifest(&manifest_path, &missing_manifest).expect("write missing manifest");
        let missing_error = import_snapshot(SnapshotImportOptions {
            data_dir: missing_restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: None,
        })
        .expect_err("missing snapshot file should fail import");
        assert!(
            missing_error
                .to_string()
                .contains("missing snapshot file `ledger.json`"),
            "{missing_error}"
        );

        let mut duplicate_manifest = manifest.clone();
        duplicate_manifest
            .files
            .push(duplicate_manifest.files[0].clone());
        write_snapshot_manifest(&manifest_path, &duplicate_manifest)
            .expect("write duplicate manifest");
        let duplicate_error = import_snapshot(SnapshotImportOptions {
            data_dir: duplicate_restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: None,
        })
        .expect_err("duplicate snapshot file should fail import");
        assert!(
            duplicate_error
                .to_string()
                .contains("duplicate snapshot file"),
            "{duplicate_error}"
        );

        let mut escape_manifest = manifest;
        escape_manifest.files[0].name = "../escaped.json".to_string();
        write_snapshot_manifest(&manifest_path, &escape_manifest).expect("write escape manifest");
        let escape_error = import_snapshot(SnapshotImportOptions {
            data_dir: escape_restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: None,
        })
        .expect_err("unexpected snapshot file should fail import");
        assert!(
            escape_error
                .to_string()
                .contains("unexpected snapshot file `../escaped.json`"),
            "{escape_error}"
        );

        std::fs::remove_dir_all(data_dir).expect("cleanup manifest file set data");
        std::fs::remove_dir_all(snapshot_dir).expect("cleanup manifest file set snapshot");
        if missing_restored_dir.exists() {
            std::fs::remove_dir_all(missing_restored_dir)
                .expect("cleanup missing manifest restore");
        }
        if duplicate_restored_dir.exists() {
            std::fs::remove_dir_all(duplicate_restored_dir)
                .expect("cleanup duplicate manifest restore");
        }
        if escape_restored_dir.exists() {
            std::fs::remove_dir_all(escape_restored_dir).expect("cleanup escape manifest restore");
        }
    }

    #[test]
    fn fastswap_epoch_one_bootstrap_is_governance_bound_and_canonically_committed() {
        let data_dir = std::env::temp_dir().join(format!(
            "postfiat-fastswap-governance-bootstrap-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-fastswap-bootstrap".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("init FastSwap bootstrap test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("bootstrap genesis");
        let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
            .expect("bootstrap validator keys");
        let mut committee = postfiat_types::FastSwapCommitteeV1 {
            domain: postfiat_types::FastSwapCommitteeDomainV1 {
                chain: postfiat_types::FastSwapChainDomainV1 {
                    chain_id: genesis.chain_id.clone(),
                    genesis_hash: postfiat_types::FastSwapOpaqueHashV1(
                        hex_to_bytes(&genesis_hash(&genesis))
                            .expect("genesis hash hex")
                            .try_into()
                            .expect("genesis hash width"),
                    ),
                    protocol_version: genesis.protocol_version,
                },
                fastswap_schema_version: postfiat_types::FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: postfiat_types::FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators: validator_keys
                .validators
                .iter()
                .map(|record| postfiat_types::FastSwapValidatorV1 {
                    validator_id: record.node_id.clone(),
                    public_key: hex_to_bytes(&record.public_key_hex).expect("validator public key"),
                })
                .collect(),
        };
        committee
            .validators
            .sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        let payload = postfiat_types::FastSwapGovernanceBootstrapPayloadV1 {
            committee: committee.clone(),
            asset_rules: vec![postfiat_types::FastAssetRuleV1 {
                asset_id: postfiat_types::FastAssetIdV1::native_pft(),
                asset_definition_hash: postfiat_types::FastAssetDefinitionHashV1::ZERO,
                issuer_address: "native".to_string(),
                issuer_control_pubkey: vec![1],
                requires_authorization: false,
                freeze_enabled: false,
                clawback_enabled: false,
                fast_lane_enabled: true,
                valid_from_height: 1,
                valid_through_height: 100,
            }],
            policies: Vec::new(),
            activation_height: 10,
        };
        let payload_file = data_dir.join("fastswap-bootstrap-payload.json");
        let amendment_file = data_dir.join("fastswap-bootstrap-amendment.unsigned.json");
        let signed_amendment_file = data_dir.join("fastswap-bootstrap-amendment.signed.json");
        let unsigned_batch_file = data_dir.join("fastswap-bootstrap-batch.unsigned.json");
        let batch_file = data_dir.join("fastswap-bootstrap-batch.json");
        atomic_write(
            &payload_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&payload).expect("bootstrap payload JSON")
            ),
        )
        .expect("write bootstrap payload");
        let validators = local_validator_ids(4).expect("bootstrap validator ids");
        create_fastswap_governance_bootstrap(
            FastSwapGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                validators: validators.clone(),
                support: validators.clone(),
                activation_height: 0,
                veto_until_height: 0,
                paused: false,
                payload_file: payload_file.clone(),
                amendment_file: amendment_file.clone(),
                batch_file: unsigned_batch_file,
            },
        )
        .expect("create unsigned FastSwap governance bootstrap proposal");
        let split_key_files = write_split_validator_key_files(&data_dir, &validator_keys);
        let authorization_files = split_key_files
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file =
                    data_dir.join(format!("{validator}.fastswap-authorization.json"));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    amendment_file: amendment_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 1,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign FastSwap governance authorization");
                authorization_file
            })
            .collect::<Vec<_>>();
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file,
            authorization_files,
            proposal_slot: 1,
            output_file: signed_amendment_file.clone(),
        })
        .expect("assemble signed FastSwap governance amendment");
        let batch = assemble_signed_fastswap_governance_bootstrap(
            SignedFastSwapGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                payload_file,
                signed_amendment_file,
                proposal_slot: 1,
                batch_file: batch_file.clone(),
            },
        )
        .expect("assemble signed FastSwap governance bootstrap");
        assert_eq!(batch.fastswap_bootstraps.len(), 1);
        verify_governance_action_batch_id(&genesis, &batch).expect("verify bootstrap batch");

        let mut tampered = batch.clone();
        tampered.fastswap_bootstraps[0].payload.activation_height = 11;
        assert!(verify_governance_action_batch_id(&genesis, &tampered).is_err());

        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply FastSwap governance bootstrap");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        let ledger = store.read_ledger().expect("bootstrap ledger");
        assert_eq!(ledger.fastswap_committees, vec![committee]);
        assert_eq!(ledger.fastswap_activation_height, Some(10));
        assert_eq!(ledger.fast_lane_asset_rules.len(), 1);
        let governance = store.read_governance().expect("bootstrap governance");
        assert_eq!(governance.amendments.len(), 1);
        assert_eq!(
            governance.amendments[0],
            batch.fastswap_bootstraps[0].amendment
        );
        verify_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify bootstrap state");
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay FastSwap bootstrap block with committed FastLane ledger state");

        fs::remove_dir_all(data_dir).expect("cleanup FastSwap bootstrap test");
    }

    #[test]
    fn fastpay_recovery_bootstrap_is_signed_future_activated_and_tamper_atomic() {
        let data_dir = unique_test_dir("postfiat-fastpay-recovery-governance-bootstrap");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-fastpay-recovery-bootstrap".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("init FastPay recovery bootstrap test");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("bootstrap genesis");
        let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
            .expect("bootstrap validator keys");
        let validators = local_validator_ids(4).expect("bootstrap validators");
        let activation_height = 10;
        let committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            genesis.chain_id.clone(),
            genesis_hash(&genesis),
            genesis.protocol_version,
            1,
            activation_height,
            100,
            validator_keys
                .validators
                .iter()
                .map(|record| (record.node_id.clone(), record.public_key_hex.clone()))
                .collect(),
        )
        .expect("FastPay recovery committee");
        let payload = postfiat_types::FastPayRecoveryGovernancePayloadV1 {
            policy: postfiat_types::FastPayRecoveryPolicyV1 {
                schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
                activation_height,
                max_validity_blocks: 20,
                max_recovery_blocks: 20,
            },
            committee: committee.clone(),
        };
        let payload_file = data_dir.join("fastpay-recovery-payload.json");
        let amendment_file = data_dir.join("fastpay-recovery-amendment.unsigned.json");
        let signed_amendment_file = data_dir.join("fastpay-recovery-amendment.signed.json");
        let unsigned_batch_file = data_dir.join("fastpay-recovery-batch.unsigned.json");
        let batch_file = data_dir.join("fastpay-recovery-batch.json");
        atomic_write(
            &payload_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&payload).expect("recovery payload JSON")
            ),
        )
        .expect("write FastPay recovery payload");

        let unsigned = create_fastpay_recovery_governance_bootstrap(
            FastPayRecoveryGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                validators: validators.clone(),
                support: validators.clone(),
                veto_until_height: 0,
                payload_file: payload_file.clone(),
                amendment_file: amendment_file.clone(),
                batch_file: unsigned_batch_file,
            },
        )
        .expect("create unsigned FastPay recovery bootstrap");
        assert_eq!(unsigned.fastpay_recovery_bootstraps.len(), 1);
        assert_eq!(
            unsigned.fastpay_recovery_bootstraps[0]
                .amendment
                .activation_height,
            0,
            "the signed payload, not governance admission, carries feature activation"
        );

        let split_key_files = write_split_validator_key_files(&data_dir, &validator_keys);
        let authorization_files = split_key_files
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file =
                    data_dir.join(format!("{validator}.fastpay-recovery-authorization.json"));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    amendment_file: amendment_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 1,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign FastPay recovery governance authorization");
                authorization_file
            })
            .collect::<Vec<_>>();
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file,
            authorization_files,
            proposal_slot: 1,
            output_file: signed_amendment_file.clone(),
        })
        .expect("assemble signed FastPay recovery amendment");
        let batch = assemble_signed_fastpay_recovery_governance_bootstrap(
            SignedFastPayRecoveryGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                payload_file,
                signed_amendment_file,
                proposal_slot: 1,
                batch_file: batch_file.clone(),
            },
        )
        .expect("assemble signed FastPay recovery bootstrap");
        verify_governance_action_batch_id(&genesis, &batch)
            .expect("verify FastPay recovery batch identity");

        let ledger_before = store.read_ledger().expect("ledger before tamper");
        let governance_before = store.read_governance().expect("governance before tamper");
        let mut tampered = batch.clone();
        tampered.fastpay_recovery_bootstraps[0]
            .payload
            .policy
            .max_validity_blocks = 21;
        let tampered_file = data_dir.join("fastpay-recovery-batch.tampered.json");
        write_governance_action_batch_file(&tampered_file, &tampered)
            .expect("write tampered FastPay recovery batch");
        let error = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: tampered_file,
            certificate_file: None,
        })
        .expect_err("tampered FastPay recovery payload must fail before mutation");
        assert!(
            error.to_string().contains("payload binding")
                || error.to_string().contains("batch id mismatch"),
            "{error}"
        );
        assert_eq!(
            store.read_ledger().expect("ledger after tamper"),
            ledger_before
        );
        assert_eq!(
            store.read_governance().expect("governance after tamper"),
            governance_before
        );

        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply signed FastPay recovery bootstrap");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!(receipts[0].code, "fastpay_recovery_bootstrap_applied");
        let ledger = store.read_ledger().expect("ledger after bootstrap");
        assert_eq!(ledger.fastpay_recovery_policy, Some(payload.policy.clone()));
        assert_eq!(
            ledger.fastpay_recovery_committees,
            vec![committee.clone()]
        );
        assert!(
            store.read_chain_tip().expect("tip after bootstrap").height < activation_height,
            "bootstrap must commit before feature activation"
        );
        let governance = store.read_governance().expect("governance after bootstrap");
        assert_eq!(governance.amendments.len(), 1);
        assert_eq!(
            governance.amendments[0],
            batch.fastpay_recovery_bootstraps[0].amendment
        );
        verify_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify FastPay recovery bootstrap state");
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay FastPay recovery bootstrap block");

        let rotated_committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            genesis.chain_id.clone(),
            genesis_hash(&genesis),
            genesis.protocol_version,
            2,
            101,
            200,
            validator_keys
                .validators
                .iter()
                .map(|record| (record.node_id.clone(), record.public_key_hex.clone()))
                .collect(),
        )
        .expect("rotated FastPay recovery committee");
        let rotation_payload = postfiat_types::FastPayRecoveryGovernancePayloadV1 {
            policy: payload.policy.clone(),
            committee: rotated_committee.clone(),
        };
        let rotation_payload_file = data_dir.join("fastpay-recovery-rotation-payload.json");
        let rotation_amendment_file =
            data_dir.join("fastpay-recovery-rotation-amendment.unsigned.json");
        let rotation_signed_amendment_file =
            data_dir.join("fastpay-recovery-rotation-amendment.signed.json");
        let rotation_unsigned_batch_file =
            data_dir.join("fastpay-recovery-rotation-batch.unsigned.json");
        let rotation_batch_file = data_dir.join("fastpay-recovery-rotation-batch.json");
        atomic_write(
            &rotation_payload_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&rotation_payload)
                    .expect("rotation recovery payload JSON")
            ),
        )
        .expect("write FastPay recovery rotation payload");
        create_fastpay_recovery_governance_bootstrap(
            FastPayRecoveryGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                validators: validators.clone(),
                support: validators.clone(),
                veto_until_height: 0,
                payload_file: rotation_payload_file.clone(),
                amendment_file: rotation_amendment_file.clone(),
                batch_file: rotation_unsigned_batch_file,
            },
        )
        .expect("create unsigned FastPay recovery rotation");
        let rotation_authorizations = split_key_files
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file =
                    data_dir.join(format!("{validator}.fastpay-recovery-rotation.json"));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    amendment_file: rotation_amendment_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 2,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign FastPay recovery rotation authorization");
                authorization_file
            })
            .collect::<Vec<_>>();
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file: rotation_amendment_file,
            authorization_files: rotation_authorizations,
            proposal_slot: 2,
            output_file: rotation_signed_amendment_file.clone(),
        })
        .expect("assemble signed FastPay recovery rotation amendment");
        let rotation_batch = assemble_signed_fastpay_recovery_governance_bootstrap(
            SignedFastPayRecoveryGovernanceBootstrapOptions {
                data_dir: data_dir.clone(),
                payload_file: rotation_payload_file,
                signed_amendment_file: rotation_signed_amendment_file,
                proposal_slot: 2,
                batch_file: rotation_batch_file.clone(),
            },
        )
        .expect("assemble signed FastPay recovery rotation");
        let rotation_receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: rotation_batch_file,
            certificate_file: None,
        })
        .expect("apply signed FastPay recovery rotation");
        assert_eq!(rotation_receipts.len(), 1);
        assert!(rotation_receipts[0].accepted, "{rotation_receipts:?}");
        assert_eq!(
            rotation_receipts[0].code,
            "fastpay_recovery_committee_rotated"
        );
        let ledger = store.read_ledger().expect("ledger after rotation");
        assert_eq!(ledger.fastpay_recovery_policy, Some(payload.policy));
        assert_eq!(
            ledger.fastpay_recovery_committees,
            vec![committee, rotated_committee]
        );
        assert!(
            store.read_chain_tip().expect("tip after rotation").height < 101,
            "rotation must commit before the new admission window"
        );
        let governance = store.read_governance().expect("governance after rotation");
        assert_eq!(governance.amendments.len(), 2);
        assert_eq!(
            governance.amendments[1],
            rotation_batch.fastpay_recovery_bootstraps[0].amendment
        );
        verify_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify FastPay recovery rotation state");
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay FastPay recovery rotation block");

        let source_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("source status after FastPay recovery bootstrap");
        let snapshot_dir = data_dir.with_file_name(format!(
            "{}-snapshot",
            data_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test directory name")
        ));
        let restored_dir = data_dir.with_file_name(format!(
            "{}-restored",
            data_dir
                .file_name()
                .and_then(|name| name.to_str())
                .expect("test directory name")
        ));
        let manifest = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export FastPay recovery snapshot");
        assert_eq!(manifest.state_root, source_status.state_root);
        let restored = import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: Some("validator-restored".to_string()),
        })
        .expect("restore FastPay recovery snapshot");
        assert_eq!(restored.state_root, source_status.state_root);
        let restored_store = NodeStore::new(&restored_dir);
        assert_eq!(
            restored_store
                .read_ledger()
                .expect("restored FastPay recovery ledger"),
            ledger
        );
        verify_state(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("verify restored FastPay recovery state");
        verify_blocks(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("replay restored FastPay recovery history");

        fs::remove_dir_all(data_dir).expect("cleanup FastPay recovery bootstrap test");
        fs::remove_dir_all(snapshot_dir).expect("cleanup FastPay recovery snapshot");
        fs::remove_dir_all(restored_dir).expect("cleanup restored FastPay recovery snapshot");
    }

    #[test]
    fn fastpay_recovery_batch_field_preserves_legacy_governance_identity_and_json() {
        let data_dir = unique_test_dir("postfiat-fastpay-recovery-legacy-batch-identity");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-fastpay-recovery-legacy-batch".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init legacy governance identity test");
        let genesis = NodeStore::new(&data_dir)
            .read_genesis()
            .expect("legacy identity genesis");
        let validators = local_validator_ids(3).expect("legacy identity validators");
        let amendment = postfiat_consensus_cobalt::ratify_governance_amendment(
            &cobalt_domain(&genesis),
            &EssentialSubsetConfig::all_of(validators.clone()),
            GOVERNANCE_KIND_CRYPTO_POLICY,
            2,
            validators,
        )
        .expect("legacy governance amendment");
        let batch_id = chain_bound_action_batch_id(
            &genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &vec![amendment.clone()],
        )
        .expect("legacy one-tuple batch identity");
        let legacy = GovernanceActionBatch::new(batch_id.clone(), vec![amendment]);
        let encoded = serde_json::to_string(&legacy).expect("legacy batch JSON");
        assert!(
            !encoded.contains("fastpay_recovery_bootstraps"),
            "an empty appended field must not change historical JSON bytes"
        );
        let decoded: GovernanceActionBatch =
            serde_json::from_str(&encoded).expect("decode legacy batch without appended field");
        assert_eq!(decoded.batch_id, batch_id);
        assert!(decoded.fastpay_recovery_bootstraps.is_empty());
        verify_governance_action_batch_id(&genesis, &decoded)
            .expect("new verifier must preserve legacy one-tuple identity");

        fs::remove_dir_all(data_dir).expect("cleanup legacy governance identity test");
    }
