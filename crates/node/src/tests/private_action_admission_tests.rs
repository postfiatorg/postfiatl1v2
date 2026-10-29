    // W-1 S3: validator-owned private action admission, round service and receipt.

    struct PrivateActionFixture {
        data_dir: PathBuf,
        genesis: Genesis,
        holder_key_file: PathBuf,
        asset_id: String,
    }

    fn private_action_fixture(prefix: &str) -> PrivateActionFixture {
        let data_dir = unique_test_dir(prefix);
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");
        let holder_key_file = data_dir.join("asset-holder.key.json");
        let holder_key = write_test_master_key(&holder_key_file, [17u8; 32]);
        let ledger = store.read_ledger().expect("ledger");
        let funding = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            holder_key.address.clone(),
            ACCOUNT_RESERVE + 1_000,
            1,
        )
        .expect("fund holder transfer");
        let funding_batch = build_transaction_batch(&mempool_batch_domain(&genesis), vec![funding])
            .expect("funding batch")
            .batch;
        let funding_batch_file = data_dir.join("fund-holder-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        assert!(apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch")[0]
            .accepted);
        let ledger = store.read_ledger().expect("funded ledger");
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "A666".to_string(),
                version: 1,
                precision: 6,
                display_name: "NAVCoin a666".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();
        let trust_set = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder_key.address,
            &holder_key.public_key_hex,
            &holder_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &trust_set, 1).accepted);
        let issue = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: holder_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 40,
            }),
        );
        let setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![create, trust_set, issue],
        )
        .expect("asset setup batch")
        .batch;
        let setup_batch_file = data_dir.join("asset-setup-batch.json");
        write_batch_file(&setup_batch_file, &setup_batch).expect("write asset setup batch");
        assert!(apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: setup_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup batch")
        .iter()
        .all(|receipt| receipt.accepted));
        let mut governance = store.read_governance().expect("read governance");
        governance.apply(GovernanceAmendment {
            amendment_id: "activate-shielded-atomic-batches".to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            instance_id: "private-action-test".to_string(),
            proposal_id: "private-action-test".to_string(),
            certificate_id: "private-action-test".to_string(),
            proposer: "validator-0".to_string(),
            validators: vec!["validator-0".to_string()],
            quorum: 1,
            kind: GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT.to_string(),
            value: 1,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: vec!["validator-0".to_string()],
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        });
        store.write_governance(&governance).expect("write governance");
        PrivateActionFixture {
            data_dir,
            genesis,
            holder_key_file,
            asset_id,
        }
    }

    /// A user-built ingress action: the exact public JSON the wallet would persist.
    fn private_ingress_action_json(fixture: &PrivateActionFixture, amount: u64, tag: u8) -> String {
        let ingress_file = fixture.data_dir.join(format!("private-ingress-{tag}.json"));
        create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
            data_dir: fixture.data_dir.clone(),
            key_file: fixture.holder_key_file.clone(),
            asset_id: fixture.asset_id.clone(),
            amount,
            fee: 0,
            note_seed_hex: bytes_to_hex(&[tag; 32]),
            encrypted_output_hex: None,
            ingress_file: ingress_file.clone(),
            note_file: fixture.data_dir.join(format!("private-note-{tag}.json")),
            overwrite: false,
        })
        .expect("create ingress");
        let file: AssetOrchardIngressFile =
            read_json_file(&ingress_file, "private ingress").expect("read ingress");
        serde_json::to_string(&ShieldedAction::AssetOrchardIngressV2(file.payload))
            .expect("ingress action json")
    }

    fn private_submit_request(
        fixture: &PrivateActionFixture,
        action_json: &str,
    ) -> PrivateActionSubmitRequest {
        let domain = PrivateActionSubmitDomain {
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            protocol_version: fixture.genesis.protocol_version,
            pool_id: PRIVATE_ACTION_POOL_ID.to_string(),
            session_scope: "native:test-install".to_string(),
        };
        PrivateActionSubmitRequest {
            intent_id: private_action_intent_id(&domain.intent_domain(), action_json)
                .expect("intent id"),
            action_hash: private_action_hash_for_json(action_json).expect("action hash"),
            action_json: action_json.to_string(),
            domain,
        }
    }

    fn assert_refused(error: io::Error, code: &str) {
        assert!(
            error.to_string().starts_with(&format!("{code}: ")),
            "expected {code}, got {error}"
        );
    }

    /// Stands in for the certified round on a one-validator chain: commits the
    /// exact reserved batch bytes.
    fn apply_reserved_batch(data_dir: &Path, record: &PrivateActionRecord) -> Vec<Receipt> {
        let batch_file = data_dir.join(format!("round-{}.json", &record.batch_id[..16]));
        fs::write(&batch_file, &record.batch_json).expect("write reserved batch");
        apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.to_path_buf(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply reserved batch")
    }

    #[test]
    fn private_action_intent_id_matches_the_wallet_helper_rule() {
        let domain = postfiat_types::PrivateActionDomain {
            chain_id: "postfiat-wan-devnet-2".to_string(),
            genesis_hash: "ab".repeat(48),
            protocol_version: 1,
        };
        let action = r#"{"kind":"shielded_swap_v1","swap_json":"{}"}"#;
        let bytes = format!(
            "[{{\"chain_id\":\"postfiat-wan-devnet-2\",\"genesis_hash\":\"{}\",\"protocol_version\":1}},{}]",
            "ab".repeat(48),
            serde_json::to_string(action).unwrap()
        );
        assert_eq!(
            private_action_intent_id(&domain, action).unwrap(),
            bytes_to_hex(&hash_bytes("tasknode.private-intent.v1", bytes.as_bytes()))
        );
        // Pinned on both sides: python/tests/test_private_action_cli.py.
        assert_eq!(
            private_action_intent_id(&domain, action).unwrap(),
            "118a9e46af58af181b70f2549388b66da940fedc76730dadee4d79c7a373b58169f04e077e649e9b9d610f7c5f588451"
        );
    }

    #[test]
    fn private_action_admission_matches_the_contract_fixture_vectors() {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../tests/fixtures/private-actions-v1/vectors.json"
        ))
        .expect("contract vectors");
        let domain: postfiat_types::PrivateActionDomain =
            serde_json::from_value(vectors["domain"].clone()).expect("vector domain");
        let positive = vectors["positive"].as_array().expect("positive vectors");
        assert_eq!(positive.len(), 3);
        for vector in positive {
            let action_json = vector["action_json"].as_str().unwrap();
            let action = decode_private_action(action_json).expect("vector decodes");
            assert_eq!(private_action_kind(&action), vector["kind"].as_str().unwrap());
            assert_eq!(
                private_action_intent_id(&domain, action_json).unwrap(),
                vector["intent_id"].as_str().unwrap()
            );
            assert_eq!(
                private_action_hash_for_json(action_json).unwrap(),
                vector["action_hash"].as_str().unwrap()
            );
            let batch: ShieldedActionBatch =
                serde_json::from_str(vector["batch_json"].as_str().unwrap()).unwrap();
            assert!(batch.atomic);
            assert_eq!(batch.actions, vec![action]);
        }
        for vector in vectors["negative"].as_array().expect("negative vectors") {
            let action_json = vector["action_json"].as_str().unwrap();
            let refusal = vector["refusal"].as_str().unwrap();
            match refusal {
                "private_intent_mismatch" => assert_ne!(
                    private_action_intent_id(&domain, action_json).unwrap(),
                    vector["intent_id"].as_str().unwrap()
                ),
                "private_action_duplicate" => assert_eq!(
                    private_action_hash_for_json(action_json).unwrap(),
                    vector["action_hash"].as_str().unwrap()
                ),
                code => assert_refused(decode_private_action(action_json).unwrap_err(), code),
            }
        }
    }

    #[test]
    fn private_action_decode_refuses_legacy_kinds_and_unknown_fields() {
        let legacy = serde_json::to_string(&ShieldedAction::Mint(ShieldMintAction {
            owner: "a".to_string(),
            asset_id: "b".to_string(),
            amount: 1,
            memo: String::new(),
        }))
        .unwrap();
        assert_refused(
            decode_private_action(&legacy).unwrap_err(),
            "private_action_kind_refused",
        );
        let egress_v1 = r#"{"kind":"asset_orchard_egress_v1"}"#;
        assert!(decode_private_action(egress_v1).is_err());
        let fixture = private_action_fixture("postfiat-private-action-decode");
        let action = private_ingress_action_json(&fixture, 5, 31);
        let decoded = decode_private_action(&action).expect("decode ingress v2");
        assert_eq!(private_action_kind(&decoded), "asset_orchard_ingress_v2");
        let mut value: serde_json::Value = serde_json::from_str(&action).unwrap();
        value["note"] = serde_json::json!({"rho": "00"});
        assert_refused(
            decode_private_action(&value.to_string()).unwrap_err(),
            "private_action_unknown_fields",
        );
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_submit_reserves_once_and_refuses_conflicts_and_replays() {
        let fixture = private_action_fixture("postfiat-private-action-submit");
        let action = private_ingress_action_json(&fixture, 7, 41);
        let request = private_submit_request(&fixture, &action);

        let mut wrong_domain = request.clone();
        wrong_domain.domain.pool_id = "other-pool".to_string();
        assert_refused(
            private_action_submit(&fixture.data_dir, &wrong_domain).unwrap_err(),
            "private_action_wrong_domain",
        );
        let mut wrong_intent = request.clone();
        wrong_intent.intent_id = "00".repeat(48);
        assert_refused(
            private_action_submit(&fixture.data_dir, &wrong_intent).unwrap_err(),
            "private_intent_mismatch",
        );
        let mut wrong_hash = request.clone();
        wrong_hash.action_hash = "00".repeat(48);
        assert_refused(
            private_action_submit(&fixture.data_dir, &wrong_hash).unwrap_err(),
            "private_action_hash_mismatch",
        );

        let first = private_action_submit(&fixture.data_dir, &request).expect("reserve");
        assert!(!first.existing);
        assert_eq!(first.record.state, PrivateActionState::Queued);
        assert_eq!(first.record.action_json, action);
        let batch: ShieldedActionBatch = serde_json::from_str(&first.record.batch_json).unwrap();
        assert!(batch.atomic);
        verify_shielded_action_batch_id(&fixture.genesis, &batch).expect("canonical batch id");
        assert_eq!(batch.batch_id, first.record.batch_id);

        // The same intent and bytes return the existing record; nothing new is written.
        let again = private_action_submit(&fixture.data_dir, &request).expect("idempotent");
        assert!(again.existing);
        assert_eq!(again.record, first.record);
        let log = fixture
            .data_dir
            .join("private_action_admission")
            .join("records.jsonl");
        assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 1);

        // Different bytes under the reserved intent are refused.
        let mut other_bytes = request.clone();
        other_bytes.action_json = format!("{action} ");
        other_bytes.action_hash = request.action_hash.clone();
        assert_refused(
            private_action_submit(&fixture.data_dir, &other_bytes).unwrap_err(),
            "private_intent_mismatch",
        );
        let mut other_scope = request.clone();
        other_scope.domain.session_scope = "native:other-install".to_string();
        assert_refused(
            private_action_submit(&fixture.data_dir, &other_scope).unwrap_err(),
            "private_intent_conflict",
        );

        // The same typed action re-encoded is a new intent and a new action hash
        // but the same canonical batch: a replay.
        let reencoded = serde_json::to_string_pretty(
            &serde_json::from_str::<serde_json::Value>(&action).unwrap(),
        )
        .unwrap();
        let replay = private_submit_request(&fixture, &reencoded);
        assert_ne!(replay.intent_id, request.intent_id);
        assert_refused(
            private_action_submit(&fixture.data_dir, &replay).unwrap_err(),
            "private_action_duplicate",
        );

        assert_eq!(
            private_action_receipt(&fixture.data_dir, &"11".repeat(48))
                .unwrap()
                .state,
            "pending_unknown"
        );
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_service_runs_one_round_and_receipt_carries_bindings() {
        let fixture = private_action_fixture("postfiat-private-action-round");
        let action = private_ingress_action_json(&fixture, 9, 51);
        let request = private_submit_request(&fixture, &action);
        private_action_submit(&fixture.data_dir, &request).expect("reserve");
        // A crash after the reservation: the record survives queued, exactly once.
        let reopened = PrivateActionAdmissionStore::open(&fixture.data_dir).expect("reopen");
        assert_eq!(
            reopened.get(&request.intent_id).unwrap().state,
            PrivateActionState::Queued
        );
        drop(reopened);
        private_action_submit(&fixture.data_dir, &request).expect("duplicate submit");

        let mut rounds = 0;
        let mut sent_batches = Vec::new();
        let steps = private_action_service_step(
            &fixture.data_dir,
            |_| Ok(true),
            |record, _| {
                rounds += 1;
                sent_batches.push(record.batch_json.clone());
                let receipts = apply_reserved_batch(&fixture.data_dir, record);
                assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
                PrivateActionDispatch::Finished
            },
        )
        .expect("service step");
        assert_eq!(rounds, 1);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].to, PrivateActionState::Accepted);
        let reserved = PrivateActionAdmissionStore::open(&fixture.data_dir)
            .unwrap()
            .get(&request.intent_id)
            .unwrap()
            .clone();
        assert_eq!(sent_batches, vec![reserved.batch_json.clone()]);

        // Later passes and duplicate submits never run another round.
        let steps = private_action_service_step(&fixture.data_dir, |_| Ok(true), |_, _| {
            panic!("a finished intent must not be dispatched again")
        })
        .expect("idle step");
        assert!(steps.is_empty());
        let duplicate = private_action_submit(&fixture.data_dir, &request).expect("after accept");
        assert!(duplicate.existing);
        assert_eq!(duplicate.record.state, PrivateActionState::Accepted);

        let receipt = private_action_receipt(&fixture.data_dir, &request.intent_id).unwrap();
        assert_eq!(receipt.state, "accepted");
        let record = receipt.record.unwrap();
        assert_eq!(record.action_json, action);
        assert_eq!(record.action_hash, request.action_hash);
        let outcome = record.outcome.unwrap();
        assert!(outcome.accepted);
        assert_eq!(outcome.state_effects, "applied");
        assert!(outcome.fee_charged > 0);
        let inclusion = outcome.inclusion.unwrap();
        let tip = NodeStore::new(&fixture.data_dir).read_blocks().unwrap();
        let block = tip.blocks.last().unwrap();
        assert_eq!(block.header.batch_id, record.batch_id);
        assert_eq!(inclusion.height, block.header.height);
        assert_eq!(inclusion.block_hash, block.header.block_hash);
        assert_eq!(inclusion.state_root, block.header.state_root);
        assert_eq!(inclusion.certificate_id, block.header.certificate_id);
        assert_eq!(inclusion.receipt_ids, block.receipt_ids);
        let contract = receipt.contract_receipt.expect("contract receipt");
        assert_eq!(contract.schema, postfiat_types::PRIVATE_ACTIONS_SCHEMA_V1);
        assert_eq!(contract.state, postfiat_types::PrivateAdmissionState::Accepted);
        assert_eq!(contract.inclusion_height, Some(inclusion.height));
        assert_eq!(contract.transaction_id.as_ref(), block.receipt_ids.first());
        assert_eq!(contract.fee, Some(outcome.fee_charged));
        let finality = contract.finality.expect("finality evidence");
        assert_eq!(finality.kind, PRIVATE_ACTION_FINALITY_KIND);
        assert_eq!(finality.block_hash, block.header.block_hash);
        assert_eq!(finality.quorum as usize, block.header.certificate.quorum);
        assert_eq!(
            finality.certificate_json,
            serde_json::to_string(&block.header.certificate).unwrap()
        );
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_service_reconciles_an_interrupted_dispatch_from_the_chain() {
        let fixture = private_action_fixture("postfiat-private-action-reconcile");
        let landed = private_submit_request(&fixture, &private_ingress_action_json(&fixture, 3, 61));
        let lost = private_submit_request(&fixture, &private_ingress_action_json(&fixture, 4, 62));
        private_action_submit(&fixture.data_dir, &landed).expect("reserve landed");
        private_action_submit(&fixture.data_dir, &lost).expect("reserve lost");
        // Simulate crashes mid-round: both records were written `dispatched`;
        // only the first batch reached the chain.
        let mut store = PrivateActionAdmissionStore::open(&fixture.data_dir).unwrap();
        for intent in [&landed.intent_id, &lost.intent_id] {
            let mut record = store.get(intent).unwrap().clone();
            record.state = PrivateActionState::Dispatched;
            record.dispatch_count = 1;
            store.write(record).unwrap();
        }
        let landed_record = store.get(&landed.intent_id).unwrap().clone();
        drop(store);
        apply_reserved_batch(&fixture.data_dir, &landed_record);

        let steps = private_action_service_step(&fixture.data_dir, |_| Ok(true), |_, _| {
            panic!("an interrupted dispatch is reconciled, never re-run")
        })
        .expect("reconcile");
        assert_eq!(steps.len(), 2);
        let store = PrivateActionAdmissionStore::open(&fixture.data_dir).unwrap();
        assert_eq!(
            store.get(&landed.intent_id).unwrap().state,
            PrivateActionState::Accepted
        );
        assert_eq!(
            store.get(&lost.intent_id).unwrap().state,
            PrivateActionState::Unknown
        );
        assert_eq!(store.get(&lost.intent_id).unwrap().dispatch_count, 1);
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_service_final_rejection_carries_code_and_fee() {
        let fixture = private_action_fixture("postfiat-private-action-reject");
        // Two actions signed at the same account sequence: each verifies alone.
        let first = private_submit_request(&fixture, &private_ingress_action_json(&fixture, 2, 71));
        let second = private_submit_request(&fixture, &private_ingress_action_json(&fixture, 3, 72));
        private_action_submit(&fixture.data_dir, &first).expect("reserve first");
        private_action_submit(&fixture.data_dir, &second).expect("reserve second");
        let mut rounds = 0;
        private_action_service_step(&fixture.data_dir, |_| Ok(true), |record, _| {
            rounds += 1;
            apply_reserved_batch(&fixture.data_dir, record);
            PrivateActionDispatch::Finished
        })
        .expect("first round");
        let steps = private_action_service_step(&fixture.data_dir, |_| Ok(true), |_, _| {
            rounds += 1;
            PrivateActionDispatch::Finished
        })
        .expect("second pass");
        assert_eq!(rounds, 1, "the stale action must be refused before any round");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].to, PrivateActionState::Rejected);
        let states = [&first, &second].map(|request| {
            private_action_receipt(&fixture.data_dir, &request.intent_id)
                .unwrap()
                .state
        });
        assert!(states.contains(&"accepted".to_string()), "{states:?}");
        let receipt = private_action_receipt(&fixture.data_dir, &steps[0].intent_id).unwrap();
        assert_eq!(receipt.state, "rejected");
        let outcome = receipt.record.unwrap().outcome.unwrap();
        assert!(!outcome.accepted);
        assert!(!outcome.code.is_empty() && outcome.code != "atomic_batch_aborted");
        assert_eq!(outcome.fee_charged, 0);
        assert_eq!(outcome.state_effects, "none");
        assert!(outcome.inclusion.is_none());
        fs::remove_dir_all(fixture.data_dir).ok();
    }

    #[test]
    fn private_action_store_tolerates_a_torn_tail_and_refuses_corruption() {
        let data_dir = unique_test_dir("postfiat-private-action-store");
        let log_dir = data_dir.join("private_action_admission");
        fs::create_dir_all(&log_dir).unwrap();
        let record = PrivateActionRecord {
            schema: PRIVATE_ACTION_RECORD_SCHEMA.to_string(),
            intent_id: "aa".repeat(48),
            domain: PrivateActionSubmitDomain {
                chain_id: "c".to_string(),
                genesis_hash: "g".to_string(),
                protocol_version: 1,
                pool_id: PRIVATE_ACTION_POOL_ID.to_string(),
                session_scope: "s".to_string(),
            },
            action_hash: "bb".repeat(48),
            action_kind: "asset_orchard_ingress_v2".to_string(),
            action_json: "{}".to_string(),
            batch_id: "cc".repeat(48),
            batch_json: "{}".to_string(),
            state: PrivateActionState::Queued,
            received_height: 2,
            dispatch_count: 0,
            outcome: None,
        };
        let line = serde_json::to_string(&record).unwrap();
        fs::write(log_dir.join("records.jsonl"), format!("{line}\n{}", &line[..20])).unwrap();
        let store = PrivateActionAdmissionStore::open(&data_dir).expect("torn tail tolerated");
        assert_eq!(store.get(&record.intent_id), Some(&record));
        assert_eq!(store.intent_for_batch_id(&record.batch_id), Some(record.intent_id.as_str()));
        let mut store = store;
        let mut dispatched = record.clone();
        dispatched.state = PrivateActionState::Dispatched;
        store.write(dispatched.clone()).expect("append after torn tail");
        drop(store);
        let store = PrivateActionAdmissionStore::open(&data_dir).expect("clean append");
        assert_eq!(store.get(&record.intent_id), Some(&dispatched));
        drop(store);
        fs::write(log_dir.join("records.jsonl"), format!("{}\n{line}\n", &line[..20])).unwrap();
        assert!(PrivateActionAdmissionStore::open(&data_dir).is_err());
        fs::remove_dir_all(data_dir).ok();
    }
