    fn transfer_activation(genesis: &Genesis, height: u32) -> GovernanceAmendment {
        let mut amendment = pricing_enforcement_amendment(
            GOVERNANCE_KIND_ASSET_ORCHARD_TRANSFER_ACTIVATION_HEIGHT,
            height,
        );
        amendment.chain_id = genesis.chain_id.clone();
        amendment.genesis_hash = genesis_hash(genesis);
        amendment
    }

    fn commit_transfer_activation(data_dir: &Path, slot: u64, height: u32) {
        let validators = vec!["validator-0".to_string()];
        let amendment_file = data_dir.join("transfer-activation.json");
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.to_owned(),
            validators: validators.clone(),
            support: validators,
            kind: GOVERNANCE_KIND_ASSET_ORCHARD_TRANSFER_ACTIVATION_HEIGHT.into(),
            value: height,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: amendment_file.clone(),
        })
        .unwrap();
        let signed = vault_bridge_governed_route::sign_single_validator_amendment(
            data_dir,
            &amendment_file,
            slot,
            "transfer-activation",
        );
        let batch_file = data_dir.join("transfer-activation.batch.json");
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.to_owned(),
            amendment_file: Some(signed),
            registry_update_file: None,
            batch_file: batch_file.clone(),
        })
        .unwrap();
        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.to_owned(),
            batch_file,
            certificate_file: None,
        })
        .unwrap();
        assert!(receipts[0].accepted, "{receipts:?}");
    }

    #[test]
    fn asset_orchard_transfer_activation_replays_the_committed_boundary() {
        let data_dir = unique_test_dir("orchard-transfer-committed-boundary");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".into(),
            node_id: "validator-0".into(),
            validator_count: 1,
        })
        .unwrap();
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().unwrap();
        let ledger = store.read_ledger().unwrap();
        let shielded = store.read_shielded().unwrap();
        // Each envelope is identifiable in the archive and deliberately fails
        // decoding if it reaches the transfer rule.
        for (height, code) in [
            (1, "asset_orchard_transfer_not_active"),
            (3, "asset_orchard_transfer_not_active"),
            (4, "asset_orchard_transfer_bad_payload"),
        ] {
            if height == 3 {
                commit_transfer_activation(&data_dir, 2, 4);
            }
            let action =
                ShieldedAction::AssetOrchardTransferV1(AssetOrchardTransferActionPayload {
                    transfer_json: format!("{{\"height\":{height}}}"),
                    burn_transaction: None,
                });
            let batch = build_shielded_action_batch(&genesis, vec![action]).unwrap();
            let batch_file = data_dir.join(format!("transfer-{height}.json"));
            write_shielded_action_batch_file(&batch_file, &batch).unwrap();
            let receipts = apply_shielded_batch(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: None,
            })
            .unwrap();
            assert_eq!(store.read_chain_tip().unwrap().height, height);
            assert_eq!(receipts[0].code, code);
            assert_eq!(store.read_ledger().unwrap(), ledger);
            assert_eq!(store.read_shielded().unwrap(), shielded);
        }
        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .unwrap();
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);
        fs::remove_dir_all(data_dir).unwrap();
    }

    #[test]
    fn asset_orchard_transfer_activation_controls_admission_execution_and_replay() {
        let fixture = private_action_fixture("transfer-governance-boundary");
        let store = NodeStore::new(&fixture.data_dir);
        let mut governance = store.read_governance().unwrap();
        let height = store.read_chain_tip().unwrap().height + 1;
        let fields = postfiat_types::PRIVATE_TRANSFER_JSON_FIELDS
            .iter()
            .map(|field| (field.to_string(), serde_json::Value::Null))
            .collect::<serde_json::Map<_, _>>();
        // An admissible envelope, deliberately invalid as a transfer. At activation
        // it must reach normal decoding; before activation it must stop at the gate.
        let action = ShieldedAction::AssetOrchardTransferV1(AssetOrchardTransferActionPayload {
            transfer_json: serde_json::to_string(&fields).unwrap(),
            burn_transaction: None,
        });
        let request = private_submit_request(&fixture, &serde_json::to_string(&action).unwrap());
        let batch = build_shielded_action_batch(&fixture.genesis, vec![action]).unwrap();
        let ledger = store.read_ledger().unwrap();
        let shielded = store.read_shielded().unwrap();
        for (schedule, expected) in [
            (None, "asset_orchard_transfer_not_active"),
            (Some(height + 1), "asset_orchard_transfer_not_active"),
            (Some(height), "asset_orchard_transfer_bad_payload"),
        ] {
            if let Some(at) = schedule {
                governance.apply(transfer_activation(
                    &fixture.genesis,
                    at.try_into().unwrap(),
                ));
            }
            store.write_governance(&governance).unwrap();
            assert_refused(
                private_action_submit(&fixture.data_dir, &request).unwrap_err(),
                expected,
            );
            for replay in [false, true] {
                let rules = if replay {
                    asset_execution_compatibility_with_chain_activation(
                        AssetExecutionCompatibility::wan_devnet_legacy_strict_domain_validation(),
                        &fixture.genesis,
                        &governance,
                    )
                } else {
                    asset_execution_compatibility_for_genesis_and_governance(
                        &fixture.genesis,
                        &governance,
                    )
                };
                let mut next_ledger = ledger.clone();
                let mut next_shielded = shielded.clone();
                let receipts = execute_shielded_batch(
                    &fixture.genesis,
                    &mut next_ledger,
                    &mut next_shielded,
                    &batch,
                    height,
                    rules,
                    governance.orchard_pool_paused,
                    governance.shielded_atomic_batch_activation_height(),
                    &governance.asset_orchard_egress_policy_registry(),
                    replay,
                );
                assert_eq!(receipts[0].code, expected);
                assert_eq!(next_ledger, ledger);
                assert_eq!(next_shielded, shielded);
            }
        }
        let height32 = u32::try_from(height).unwrap();
        assert!(governance_amendment_lifecycle_rejection(
            &transfer_activation(&fixture.genesis, height32),
            height
        )
        .is_some());
        assert!(governance_amendment_lifecycle_rejection(
            &transfer_activation(&fixture.genesis, height32 + 1),
            height
        )
        .is_none());
        let mut paused = transfer_activation(&fixture.genesis, height32 - 1);
        paused.paused = true;
        governance.apply(paused);
        governance.apply(transfer_activation(&fixture.genesis, height32 + 100));
        assert_eq!(
            governance.asset_orchard_transfer_activation_height(),
            Some(height)
        );
        fs::remove_dir_all(&fixture.data_dir).unwrap();
    }

    /// A private payment between two wallets on a real ledger, end to end: shield, pay with
    /// change, the recipient finds the payment with its own keys, then takes part of it out
    /// in public. Each step is applied through the same batch path a validator uses.
    #[test]
    #[ignore = "builds the transfer proving key and makes five proofs; release-scale"]
    fn asset_orchard_private_transfer_pays_scans_and_exits_on_a_ledger() {
        let data_dir = unique_test_dir("postfiat-asset-orchard-transfer-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        commit_transfer_activation(&data_dir, 1, 2);
        let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");
        let holder_key_file = data_dir.join("asset-holder.key.json");
        let holder_key = write_test_master_key(&holder_key_file, [17u8; 32]);

        // Fund a public account, create an issued asset and give the account 40 of it.
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
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "Test pfUSDC".to_string(),
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

        let sender_seed = bytes_to_hex(&[23u8; 32]);
        let recipient_seed = bytes_to_hex(&[29u8; 32]);
        let stranger_seed = bytes_to_hex(&[31u8; 32]);
        let balance = |store: &NodeStore| {
            store
                .read_ledger()
                .expect("ledger")
                .trustline_for_account_asset(&holder_key.address, &asset_id)
                .expect("holder trustline")
                .balance
        };
        let pool_live = |store: &NodeStore| {
            let shielded = store.read_shielded().expect("shielded");
            let pool = shielded.orchard.as_ref().expect("pool");
            (
                pool.asset_orchard_balances[0].live_total,
                pool.nullifiers.len(),
                pool.output_commitments.len(),
            )
        };
        let apply = |transfer_file: &Path, label: &str| {
            let batch_file = data_dir.join(format!("{label}-batch.json"));
            let batch = create_asset_orchard_transfer_batch(AssetOrchardTransferBatchOptions {
                data_dir: data_dir.clone(),
                transfer_file: transfer_file.to_path_buf(),
                batch_file: batch_file.clone(),
            })
            .expect("transfer batch");
            let receipts = apply_shielded_batch(ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: None,
            });
            (serde_json::to_string(&batch).expect("batch json"), receipts)
        };

        // 1. Shield 17 through the transfer itself, so the pool note provably holds 17.
        //    This step is public by nature.
        let shield_file = data_dir.join("shield.json");
        let sender_note_file = data_dir.join("sender-note.json");
        let shield_report = create_asset_orchard_transfer(AssetOrchardTransferCreateOptions {
            data_dir: data_dir.clone(),
            note_files: Vec::new(),
            shield: Some(AssetOrchardTransferShieldOptions {
                key_file: holder_key_file.clone(),
                asset_id: asset_id.clone(),
                amount: 17,
                fee: 0,
            }),
            change_seed_hex: sender_seed.clone(),
            pay: None,
            exit: None,
            transfer_file: shield_file.clone(),
            change_note_file: sender_note_file.clone(),
            overwrite: false,
        })
        .expect("create shield");
        println!("shield built in {:.1} s", shield_report.build_seconds);
        assert_eq!(shield_report.change_amount, 17);

        // A shield whose attached burn is for a different amount than the proof covers is
        // refused, and nothing moves.
        let mut tampered: AssetOrchardTransferFile =
            read_json_file(&shield_file, "shield file").expect("read shield");
        if let Some(burn) = tampered.payload.burn_transaction.as_mut() {
            if let AssetTransactionOperation::AssetBurn(operation) = &mut burn.unsigned.operation {
                operation.amount = 1;
            }
        }
        let tampered_file = data_dir.join("shield-tampered.json");
        std::fs::write(&tampered_file, serde_json::to_string(&tampered).expect("json"))
            .expect("write tampered shield");
        let (_, receipts) = apply(&tampered_file, "shield-tampered");
        let receipts = receipts.expect("apply tampered shield");
        assert!(!receipts[0].accepted, "a shield with a smaller burn was accepted");
        assert_eq!(receipts[0].code, "asset_orchard_transfer_entry_burn_mismatch");
        assert_eq!(balance(&store), 40);
        assert!(store.read_shielded().expect("shielded").orchard.is_none());

        let (_, receipts) = apply(&shield_file, "shield");
        let receipts = receipts.expect("apply shield");
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!(balance(&store), 23);
        assert_eq!(pool_live(&store), (17, 2, 2));

        // 2. Pay 5 to the recipient's shielded address. Change of 12 returns to the sender.
        let recipient_address =
            asset_orchard_shielded_address_for_seed(&recipient_seed).expect("recipient address");
        assert!(recipient_address.starts_with("pfs1"));
        let pay_file = data_dir.join("pay.json");
        let change_file = data_dir.join("sender-change.json");
        let report = create_asset_orchard_transfer(AssetOrchardTransferCreateOptions {
            data_dir: data_dir.clone(),
            note_files: vec![sender_note_file.clone()],
            shield: None,
            change_seed_hex: sender_seed.clone(),
            pay: Some((recipient_address.clone(), 5)),
            exit: None,
            transfer_file: pay_file.clone(),
            change_note_file: change_file.clone(),
            overwrite: false,
        })
        .expect("create payment");
        println!("payment built in {:.1} s, proof {} bytes", report.build_seconds, report.proof_bytes);
        assert_eq!(report.change_amount, 12);
        let (public_json, receipts) = apply(&pay_file, "pay");
        let receipts = receipts.expect("apply payment");
        assert!(receipts[0].accepted, "{receipts:?}");
        // The published batch names no asset and no account, and has no exit.
        for hidden in [asset_id.as_str(), holder_key.address.as_str(), "\\\"exit\\\""] {
            assert!(!public_json.contains(hidden), "the payment published `{hidden}`");
        }
        // Nothing public moved: same account balance, same pool total for the asset.
        assert_eq!(balance(&store), 23);
        assert_eq!(pool_live(&store), (17, 4, 4));

        // 3. Each wallet finds its own notes from the chain alone.
        let scan = |seed: &str| scan_asset_orchard_transfer_notes(&data_dir, seed).expect("scan");
        let recipient_view = scan(&recipient_seed);
        assert_eq!(recipient_view.address, recipient_address);
        assert_eq!(recipient_view.unspent.len(), 1);
        assert_eq!(
            (recipient_view.unspent[0].value, recipient_view.unspent[0].asset_id.as_str()),
            (5, asset_id.as_str())
        );
        let sender_view = scan(&sender_seed);
        assert_eq!(
            sender_view.unspent.iter().map(|note| note.value).collect::<Vec<_>>(),
            vec![12]
        );
        // The sender's shielded 17 shows as spent.
        assert!(sender_view.notes.iter().any(|note| note.value == 17 && note.spent));
        assert!(scan(&stranger_seed).notes.is_empty());

        // 4. The same payment again, and the spent note again, are refused.
        // The ledger refuses an identical batch outright, before it reaches the pool rules.
        let (_, replay) = apply(&pay_file, "replay");
        assert!(replay.is_err(), "the same payment was applied twice");
        let double_file = data_dir.join("double.json");
        let double = create_asset_orchard_transfer(AssetOrchardTransferCreateOptions {
            data_dir: data_dir.clone(),
            note_files: vec![sender_note_file],
            shield: None,
            change_seed_hex: sender_seed.clone(),
            pay: Some((recipient_address, 1)),
            exit: None,
            transfer_file: double_file.clone(),
            change_note_file: data_dir.join("double-change.json"),
            overwrite: false,
        });
        match double {
            Ok(_) => {
                let (_, receipts) = apply(&double_file, "double");
                let receipts = receipts.expect("apply double spend");
                assert!(!receipts[0].accepted, "a spent note was spent again");
                assert_eq!(receipts[0].code, "duplicate_nullifier");
            }
            Err(error) => panic!("the wallet could not even build the double spend: {error}"),
        }
        assert_eq!(pool_live(&store), (17, 4, 4));

        // 5. The recipient takes 3 out in public and keeps 2 private.
        let received_file = data_dir.join("recipient-note.json");
        write_asset_orchard_wallet_note_file(&received_file, &recipient_view.unspent[0])
            .expect("write received note");
        let exit_file = data_dir.join("exit.json");
        let exit_report = create_asset_orchard_transfer(AssetOrchardTransferCreateOptions {
            data_dir: data_dir.clone(),
            note_files: vec![received_file],
            shield: None,
            change_seed_hex: recipient_seed.clone(),
            pay: None,
            exit: Some((holder_key.address.clone(), 3)),
            transfer_file: exit_file.clone(),
            change_note_file: data_dir.join("recipient-change.json"),
            overwrite: false,
        })
        .expect("create exit");
        println!("exit built in {:.1} s", exit_report.build_seconds);
        assert_eq!(exit_report.change_amount, 2);
        let (_, receipts) = apply(&exit_file, "exit");
        let receipts = receipts.expect("apply exit");
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!(balance(&store), 26);
        assert_eq!(pool_live(&store), (14, 6, 6));
        assert_eq!(
            global_issued_asset_supply(
                &store.read_ledger().expect("ledger"),
                &store.read_shielded().expect("shielded"),
                &asset_id,
            )
            .expect("issued supply"),
            40,
            "private payments and exits must not change issued supply"
        );
        let recipient_after = scan(&recipient_seed);
        assert_eq!(
            recipient_after.unspent.iter().map(|note| note.value).collect::<Vec<_>>(),
            vec![2]
        );
        assert_eq!(recipient_after.notes.iter().filter(|note| note.spent).count(), 1);
        verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after the exit");
    }
