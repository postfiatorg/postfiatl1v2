    fn assert_asset_orchard_public_artifact_redacted(
        label: &str,
        artifact: &str,
        private_notes: &[&AssetOrchardWalletNote],
    ) {
        for forbidden_field in [
            "note",
            "diversifier",
            "g_d",
            "pk_d",
            "rho",
            "psi",
            "rcm",
            "nk",
            "rivk",
            "spend_auth_signing_key",
            "rseed",
        ] {
            let encoded_field = format!("\"{forbidden_field}\"");
            assert!(
                !artifact.contains(&encoded_field),
                "public AssetOrchard artifact {label} leaked private field {forbidden_field}"
            );
        }

        for note in private_notes {
            let private_values = [
                note.note.diversifier.as_str(),
                note.note.g_d.as_hex(),
                note.note.pk_d.as_hex(),
                note.note.rho.as_hex(),
                note.note.psi.as_hex(),
                note.note.rcm.as_str(),
                note.nk.expose_secret().as_hex(),
                note.rivk.expose_secret().as_str(),
                note.spend_auth_signing_key.expose_secret().as_str(),
                note.rseed.expose_secret().as_str(),
            ];
            for private_value in private_values {
                assert!(
                    !private_value.is_empty(),
                    "private AssetOrchard test fixture must not contain empty markers"
                );
                assert!(
                    !artifact.contains(private_value),
                    "public AssetOrchard artifact {label} leaked a private note-opening or spend-authority value"
                );
            }
        }
    }

    #[test]
    fn asset_transactions_apply_from_batch_replay_and_account_tx() {
        let data_dir = unique_test_dir("postfiat-asset-batch-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");
        let holder_key = ml_dsa_65_keygen().expect("holder keygen");
        let holder = address_from_public_key(&holder_key.public_key);
        let holder_public_key_hex = bytes_to_hex(&holder_key.public_key);
        let holder_private_key_hex = bytes_to_hex(&holder_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let funding = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            holder.clone(),
            ACCOUNT_RESERVE + 500,
            1,
        )
        .expect("fund holder transfer");
        let funding_batch = build_transaction_batch(&mempool_batch_domain(&genesis), vec![funding])
            .expect("funding batch")
            .batch;
        let funding_batch_file = data_dir.join("fund-holder-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(funding_receipts[0].accepted, "{funding_receipts:?}");
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
                code: "USD".to_string(),
                version: 1,
                precision: 6,
                display_name: "US Dollar".to_string(),
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
            &holder,
            &holder_public_key_hex,
            &holder_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder.clone(),
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
                to: holder.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 40,
            }),
        );

        let batch_domain = mempool_batch_domain(&genesis);
        let batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &batch_domain,
            Vec::new(),
            Vec::new(),
            vec![create.clone(), trust_set.clone(), issue.clone()],
        )
        .expect("asset batch")
        .batch;
        let batch_file = data_dir.join("asset-batch.json");
        write_batch_file(&batch_file, &batch).expect("write asset batch");

        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply asset batch");
        assert_eq!(receipts.len(), 3);
        assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
        assert_eq!(receipts[0].tx_id, asset_transaction_tx_id(&create));
        assert_eq!(receipts[1].tx_id, asset_transaction_tx_id(&trust_set));
        assert_eq!(receipts[2].tx_id, asset_transaction_tx_id(&issue));

        let ledger = store.read_ledger().expect("ledger after assets");
        assert_eq!(ledger.asset_definitions.len(), 1);
        assert_eq!(
            ledger
                .trustline_for_account_asset(&holder, &asset_id)
                .expect("holder trustline")
                .balance,
            40
        );

        let info = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("asset_info");
        assert_eq!(info.schema, "postfiat-asset-info-v1");
        assert!(info.found);
        let asset = info.asset.expect("asset info row");
        assert_eq!(asset.asset_id, asset_id);
        assert_eq!(asset.issuer, faucet_key.address);
        assert_eq!(asset.code, "USD");
        assert_eq!(asset.outstanding_supply, 40);
        assert_eq!(asset.trustline_count, 1);
        assert_eq!(asset.holder_count, 1);

        let missing_info = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: "0".repeat(postfiat_types::ISSUED_ASSET_ID_HEX_LEN),
        })
        .expect("missing asset_info");
        assert!(!missing_info.found);
        assert!(missing_info.asset.is_none());

        let lines = account_lines(AccountLinesOptions {
            data_dir: data_dir.clone(),
            account: holder.clone(),
            issuer: Some(faucet_key.address.clone()),
            asset_id: Some(asset_id.clone()),
            limit: Some(10),
        })
        .expect("account_lines");
        assert_eq!(lines.schema, "postfiat-account-lines-v1");
        assert_eq!(lines.line_count, 1);
        assert_eq!(lines.lines[0].asset_id, asset_id);
        assert_eq!(lines.lines[0].balance, 40);
        assert_eq!(lines.lines[0].limit, 100);

        let account_assets_report = account_assets(AccountAssetsOptions {
            data_dir: data_dir.clone(),
            account: holder.clone(),
            asset_id: None,
            limit: Some(10),
        })
        .expect("account_assets");
        assert_eq!(account_assets_report.schema, "postfiat-account-assets-v1");
        assert_eq!(account_assets_report.asset_count, 1);
        assert_eq!(account_assets_report.assets[0].balance, 40);

        let issuer_assets_report = issuer_assets(IssuerAssetsOptions {
            data_dir: data_dir.clone(),
            issuer: faucet_key.address.clone(),
            limit: Some(10),
        })
        .expect("issuer_assets");
        assert_eq!(issuer_assets_report.schema, "postfiat-issuer-assets-v1");
        assert_eq!(issuer_assets_report.asset_count, 1);
        assert_eq!(issuer_assets_report.assets[0].asset_id, asset_id);
        assert_eq!(issuer_assets_report.assets[0].outstanding_supply, 40);

        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("asset batch replay verification");
        let finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: receipts[2].tx_id.clone(),
            audit_block_log: true,
        })
        .expect("asset tx finality");
        assert!(finality.confirmed);
        assert_eq!(finality.receipt_count, 3);

        let holder_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(2),
            to_height: Some(2),
            limit: Some(10),
        })
        .expect("holder account_tx");
        assert_eq!(holder_history.row_count, 2);
        assert_eq!(holder_history.rows[0].transaction_kind, TRUST_SET_TRANSACTION_KIND);
        assert_eq!(
            holder_history.rows[1].transaction_kind,
            ISSUED_PAYMENT_TRANSACTION_KIND
        );
        assert_eq!(
            holder_history.rows[1].asset_id.as_deref(),
            Some(asset_id.as_str())
        );
        assert_eq!(
            holder_history.rows[1].issuer.as_deref(),
            Some(faucet_key.address.as_str())
        );
        assert_eq!(holder_history.rows[1].amount, 40);
        assert_eq!(holder_history.rows[1].accepted, Some(true));

        fs::remove_dir_all(data_dir).expect("cleanup asset batch test");
    }

    #[test]
    fn asset_orchard_ingress_and_disclosed_egress_round_trip_issued_asset() {
        let data_dir = unique_test_dir("postfiat-asset-orchard-ingress-test");
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
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(funding_receipts[0].accepted, "{funding_receipts:?}");

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
                code: "A651".to_string(),
                version: 1,
                precision: 6,
                display_name: "NAVCoin a651".to_string(),
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
            vec![create.clone(), trust_set.clone(), issue.clone()],
        )
        .expect("asset setup batch")
        .batch;
        let setup_batch_file = data_dir.join("asset-setup-batch.json");
        write_batch_file(&setup_batch_file, &setup_batch).expect("write asset setup batch");
        let setup_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: setup_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup batch");
        assert!(
            setup_receipts.iter().all(|receipt| receipt.accepted),
            "{setup_receipts:?}"
        );
        let ledger_before_ingress = store.read_ledger().expect("ledger before ingress");
        assert_eq!(
            ledger_before_ingress
                .trustline_for_account_asset(&holder_key.address, &asset_id)
                .expect("holder trustline before ingress")
                .balance,
            40
        );
        assert_eq!(
            global_issued_asset_supply(
                &ledger_before_ingress,
                &store.read_shielded().expect("shielded before ingress"),
                &asset_id,
            )
            .expect("issued supply before ingress"),
            40
        );

        let ingress_file = data_dir.join("asset-orchard-ingress.json");
        let note_file = data_dir.join("asset-orchard-note.json");
        let ingress_report = create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
            data_dir: data_dir.clone(),
            key_file: holder_key_file,
            asset_id: asset_id.clone(),
            amount: 17,
            fee: 0,
            note_seed_hex: bytes_to_hex(&[23u8; 32]),
            encrypted_output_hex: None,
            ingress_file: ingress_file.clone(),
            note_file: note_file.clone(),
            overwrite: false,
        })
        .expect("create AssetOrchard ingress");
        assert_eq!(ingress_report.schema, ASSET_ORCHARD_INGRESS_REPORT_SCHEMA);
        assert_eq!(ingress_report.pool_id, ASSET_ORCHARD_POOL_ID_V1);
        assert_eq!(ingress_report.asset_id, asset_id);
        assert_eq!(ingress_report.amount, 17);
        assert!(ingress_report.burn_fee >= ingress_report.minimum_burn_fee);
        #[cfg(unix)]
        assert_private_asset_orchard_note_modes(&note_file);
        let reloaded_note: AssetOrchardWalletNote =
            read_json_file(&note_file, "AssetOrchard wallet note reload")
                .expect("reload AssetOrchard wallet note");
        assert_eq!(
            reloaded_note.output_commitment.as_hex(),
            ingress_report.output_commitment
        );

        let batch_file = data_dir.join("asset-orchard-ingress-batch.json");
        let batch = create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
            data_dir: data_dir.clone(),
            ingress_file,
            batch_file: batch_file.clone(),
        })
        .expect("create AssetOrchard ingress batch");
        let serialized_batch = serde_json::to_string(&batch).expect("serialize ingress batch");
        for forbidden in ["\"note\"", "\"rho\"", "\"psi\"", "\"rcm\"", "\"value\""] {
            assert!(
                !serialized_batch.contains(forbidden),
                "live AssetOrchard ingress leaked private note field {forbidden}: {serialized_batch}"
            );
        }
        assert!(matches!(
            batch.actions.first(),
            Some(ShieldedAction::AssetOrchardIngressV2(_))
        ));

        let receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply AssetOrchard ingress batch");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        assert_eq!(receipts[0].code, "accepted");
        assert_eq!(receipts[0].fee_charged, ingress_report.burn_fee);

        let ledger_after_ingress = store.read_ledger().expect("ledger after ingress");
        assert_eq!(
            ledger_after_ingress
                .trustline_for_account_asset(&holder_key.address, &asset_id)
                .expect("holder trustline after ingress")
                .balance,
            23
        );
        let shielded_after = store.read_shielded().expect("shielded after ingress");
        let pool = shielded_after
            .orchard
            .as_ref()
            .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
            .expect("asset orchard pool");
        assert_eq!(pool.output_commitments.len(), 1);
        assert_eq!(pool.output_commitments[0], ingress_report.output_commitment);
        assert_eq!(pool.asset_orchard_outputs.len(), 1);
        assert_eq!(
            pool.asset_orchard_outputs[0].output_commitment,
            ingress_report.output_commitment
        );
        assert_eq!(pool.asset_orchard_balances.len(), 1);
        assert_eq!(pool.asset_orchard_balances[0].asset_id, asset_id);
        assert_eq!(pool.asset_orchard_balances[0].ingress_total, 17);
        assert_eq!(pool.asset_orchard_balances[0].egress_total, 0);
        assert_eq!(pool.asset_orchard_balances[0].live_total, 17);
        assert_eq!(
            global_issued_asset_supply(&ledger_after_ingress, &shielded_after, &asset_id)
                .expect("issued supply after ingress"),
            40,
            "private ingress must move custody without changing issued supply"
        );
        assert_eq!(pool.root_history.len(), 2);
        verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after ingress");

        let egress_file = data_dir.join("asset-orchard-egress.json");
        let egress_report = create_asset_orchard_egress(AssetOrchardEgressCreateOptions {
            data_dir: data_dir.clone(),
            note_file: note_file.clone(),
            to: holder_key.address.clone(),
            amount: Some(17),
            egress_file: egress_file.clone(),
            overwrite: false,
        })
        .expect("create AssetOrchard disclosed egress");
        assert_eq!(egress_report.schema, ASSET_ORCHARD_EGRESS_REPORT_SCHEMA);
        assert_eq!(egress_report.pool_id, ASSET_ORCHARD_POOL_ID_V1);
        assert_eq!(egress_report.to, holder_key.address);
        assert_eq!(egress_report.asset_id, asset_id);
        assert_eq!(egress_report.amount, 17);
        assert_eq!(
            egress_report.output_commitment,
            ingress_report.output_commitment
        );
        assert!(egress_report.verified);
        assert_eq!(
            egress_report.privacy,
            "disclosed_note_opening_whole_note_egress"
        );

        let egress_batch_file = data_dir.join("asset-orchard-egress-batch.json");
        let egress_batch = create_asset_orchard_egress_batch(AssetOrchardEgressBatchOptions {
            data_dir: data_dir.clone(),
            egress_file: egress_file.clone(),
            batch_file: egress_batch_file.clone(),
        })
        .expect("create AssetOrchard disclosed egress batch");
        let egress_payload = match egress_batch.actions.first() {
            Some(ShieldedAction::AssetOrchardEgressV1(payload)) => payload.clone(),
            other => panic!("expected AssetOrchard egress action, got {other:?}"),
        };

        let worker_count = 8;
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(worker_count));
        let workers = (0..worker_count)
            .map(|_| {
                let barrier = barrier.clone();
                let data_dir = data_dir.clone();
                let egress_batch_file = egress_batch_file.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    apply_shielded_batch(ApplyBatchOptions {
                        data_dir,
                        batch_file: egress_batch_file,
                        certificate_file: None,
                    })
                })
            })
            .collect::<Vec<_>>();
        let concurrent_results = workers
            .into_iter()
            .map(|worker| {
                worker
                    .join()
                    .expect("concurrent AssetOrchard egress worker")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            concurrent_results
                .iter()
                .filter_map(|result| result.as_ref().ok())
                .flatten()
                .filter(|receipt| receipt.accepted && receipt.code == "accepted")
                .count(),
            1,
            "exactly one concurrent egress may consume the note: {concurrent_results:?}"
        );
        assert!(
            concurrent_results
                .iter()
                .filter_map(|result| result.as_ref().err())
                .all(|error| error.kind() == std::io::ErrorKind::AlreadyExists),
            "losing concurrent egresses must fail at ordered-batch idempotency: {concurrent_results:?}"
        );
        assert_eq!(
            concurrent_results.iter().filter(|result| result.is_err()).count(),
            worker_count - 1,
            "every losing concurrent egress must reject: {concurrent_results:?}"
        );

        let ledger_after_egress = store.read_ledger().expect("ledger after egress");
        assert_eq!(
            ledger_after_egress
                .trustline_for_account_asset(&holder_key.address, &asset_id)
                .expect("holder trustline after egress")
                .balance,
            40
        );
        let shielded_after_egress = store.read_shielded().expect("shielded after egress");
        let pool_after_egress = shielded_after_egress
            .orchard
            .as_ref()
            .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
            .expect("asset orchard pool after egress");
        assert!(
            pool_after_egress.is_nullified(&egress_report.nullifier),
            "egress nullifier was not recorded"
        );
        assert_eq!(pool_after_egress.asset_orchard_balances.len(), 1);
        assert_eq!(pool_after_egress.asset_orchard_balances[0].asset_id, asset_id);
        assert_eq!(
            pool_after_egress.asset_orchard_balances[0].ingress_total,
            17
        );
        assert_eq!(pool_after_egress.asset_orchard_balances[0].egress_total, 17);
        assert_eq!(pool_after_egress.asset_orchard_balances[0].live_total, 0);
        assert_eq!(
            global_issued_asset_supply(
                &ledger_after_egress,
                &shielded_after_egress,
                &asset_id,
            )
            .expect("issued supply after egress"),
            40,
            "private egress must move custody without changing issued supply"
        );
        verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after egress");

        let mut duplicate_ledger = ledger_after_egress.clone();
        let mut duplicate_shielded = shielded_after_egress.clone();
        let duplicate_receipt = apply_asset_orchard_egress_action_to_state(
            &genesis,
            &mut duplicate_ledger,
            &mut duplicate_shielded,
            &egress_payload,
        )
        .expect("duplicate egress apply");
        assert!(!duplicate_receipt.accepted);
        assert_eq!(duplicate_receipt.code, "duplicate_nullifier");

        let snapshot_dir = data_dir.with_extension("issued-snapshot");
        let restored_dir = data_dir.with_extension("issued-restored");
        let source_status = status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("issued source status before snapshot");
        let snapshot = export_snapshot(SnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export issued-custody snapshot");
        assert_eq!(snapshot.state_root, source_status.state_root);
        let restored = import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: Some("issued-snapshot-restored".to_string()),
        })
        .expect("restore issued-custody snapshot");
        assert_eq!(restored.state_root, source_status.state_root);
        let restored_store = NodeStore::new(&restored_dir);
        assert_eq!(
            global_issued_asset_supply(
                &restored_store.read_ledger().expect("restored issued ledger"),
                &restored_store
                    .read_shielded()
                    .expect("restored issued shielded state"),
                &asset_id,
            )
            .expect("restored issued supply"),
            40,
            "snapshot restore must preserve issued supply across public and private custody"
        );
        verify_blocks(NodeOptions {
            data_dir: restored_dir.clone(),
        })
        .expect("restored issued snapshot must replay exactly");

        fs::remove_dir_all(data_dir).expect("cleanup asset orchard ingress test");
        fs::remove_dir_all(snapshot_dir).expect("cleanup issued snapshot");
        fs::remove_dir_all(restored_dir).expect("cleanup restored issued snapshot");
    }

    #[test]
    fn asset_orchard_nav_usd_e8_activation_preserves_historical_replay() {
        assert_eq!(
            asset_orchard_nav_ratio_denominator(
                ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT - 1
            ),
            postfiat_types::VAULT_BRIDGE_UNIT
        );
        assert_eq!(
            asset_orchard_nav_ratio_denominator(ASSET_ORCHARD_NAV_USD_E8_ACTIVATION_HEIGHT),
            postfiat_types::NAV_USD_E8_UNIT
        );
    }

    #[test]
    fn wan_devnet_invalid_asset_orchard_swap_proof_is_rejected_and_valid_swap_still_applies() {
        let data_dir = unique_test_dir("postfiat-wan-devnet-invalid-asset-orchard-swap-proof");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-wan-devnet".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init node");

        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");
        let holder_a_key_file = data_dir.join("asset-holder-a.key.json");
        let holder_a_key = write_test_master_key(&holder_a_key_file, [41u8; 32]);
        let holder_b_key_file = data_dir.join("asset-holder-b.key.json");
        let holder_b_key = write_test_master_key(&holder_b_key_file, [42u8; 32]);
        assert_ne!(holder_a_key.address, holder_b_key.address);

        let ledger = store.read_ledger().expect("ledger");
        let funding_a = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            holder_a_key.address.clone(),
            ACCOUNT_RESERVE + 2_000,
            1,
        )
        .expect("fund holder A transfer");
        let mut funding_ledger = ledger.clone();
        assert!(execute_transfer(&genesis, &mut funding_ledger, &funding_a).accepted);
        let funding_b = build_signed_transfer_for_key(
            &genesis,
            &funding_ledger,
            &faucet_key,
            holder_b_key.address.clone(),
            ACCOUNT_RESERVE + 2_000,
            2,
        )
        .expect("fund holder B transfer");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![funding_a, funding_b],
        )
        .expect("two-wallet funding batch")
        .batch;
        let funding_batch_file = data_dir.join("fund-two-holders-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert_eq!(funding_receipts.len(), 2);
        assert!(
            funding_receipts.iter().all(|receipt| receipt.accepted),
            "{funding_receipts:?}"
        );

        let ledger = store.read_ledger().expect("funded ledger");
        let create_a = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "A651".to_string(),
                version: 1,
                precision: 6,
                display_name: "NAVCoin a651".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create_a, 1).accepted);
        let asset_a = dry_run_ledger.asset_definitions[0].asset_id.clone();
        let create_b = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "PFUSDC".to_string(),
                version: 1,
                precision: 6,
                display_name: "PostFiat USDC".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create_b, 1).accepted);
        let asset_b = dry_run_ledger.asset_definitions[1].asset_id.clone();
        let trust_a_holder_a = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder_a_key.address,
            &holder_a_key.public_key_hex,
            &holder_a_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_a_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_a.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &trust_a_holder_a, 1)
                .accepted
        );
        let trust_b_holder_a = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder_a_key.address,
            &holder_a_key.public_key_hex,
            &holder_a_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_a_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_b.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &trust_b_holder_a, 1)
                .accepted
        );
        let trust_a_holder_b = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder_b_key.address,
            &holder_b_key.public_key_hex,
            &holder_b_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_b_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_a.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &trust_a_holder_b, 1)
                .accepted
        );
        let trust_b_holder_b = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder_b_key.address,
            &holder_b_key.public_key_hex,
            &holder_b_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: holder_b_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_b.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &trust_b_holder_b, 1)
                .accepted
        );
        let issue_a = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            5,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: holder_a_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_a.clone(),
                amount: 40,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &issue_a, 1).accepted);
        let issue_b = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            6,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: holder_b_key.address.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_b.clone(),
                amount: 50,
            }),
        );

        let setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![
                create_a.clone(),
                create_b.clone(),
                trust_a_holder_a.clone(),
                trust_b_holder_a.clone(),
                trust_a_holder_b.clone(),
                trust_b_holder_b.clone(),
                issue_a.clone(),
                issue_b.clone(),
            ],
        )
        .expect("asset setup batch")
        .batch;
        let setup_batch_file = data_dir.join("asset-setup-batch.json");
        write_batch_file(&setup_batch_file, &setup_batch).expect("write asset setup batch");
        let setup_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: setup_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup batch");
        assert!(
            setup_receipts.iter().all(|receipt| receipt.accepted),
            "{setup_receipts:?}"
        );

        let mut pricing_ledger = store.read_ledger().expect("pricing ledger");
        let pricing_profile = postfiat_types::NavProofProfile::new(
            faucet_key.address.clone(),
            postfiat_types::NAV_PROFILE_VERIFIER_PLACEHOLDER,
            "test-finalized-nav",
            100,
            1,
            100,
            0,
            0,
            0,
            0,
            "",
            "",
            "",
            0,
            0,
        )
        .expect("pricing profile");
        let mut pricing_asset = postfiat_types::NavTrackedAsset::new(
            asset_a.clone(),
            faucet_key.address.clone(),
            faucet_key.address.clone(),
            pricing_profile.profile_id.clone(),
            "USD",
            faucet_key.address.clone(),
        )
        .expect("pricing NAV asset");
        pricing_asset.finalized_epoch = 59;
        pricing_asset.circulating_supply = 40;
        pricing_asset.nav_per_unit = 2 * postfiat_types::VAULT_BRIDGE_UNIT;
        pricing_asset.finalized_reserve_packet_hash = "ab".repeat(48);
        pricing_ledger.nav_proof_profiles.push(pricing_profile);
        pricing_ledger.nav_assets.push(pricing_asset);
        store.write_ledger(&pricing_ledger).expect("write pricing ledger");

        let custom_ciphertext_error = create_asset_orchard_ingress(
            AssetOrchardIngressCreateOptions {
                data_dir: data_dir.clone(),
                key_file: holder_a_key_file.clone(),
                asset_id: asset_a.clone(),
                amount: 1,
                fee: 0,
                note_seed_hex: bytes_to_hex(&[50u8; 32]),
                encrypted_output_hex: Some(bytes_to_hex(b"plaintext-label")),
                ingress_file: data_dir.join("custom-ciphertext-ingress.json"),
                note_file: data_dir.join("custom-ciphertext-note.json"),
                overwrite: false,
            },
        )
        .expect_err("custom ingress encrypted output must be rejected");
        assert_eq!(custom_ciphertext_error.kind(), io::ErrorKind::InvalidInput);

        let ingress_a_file = data_dir.join("asset-a-ingress.json");
        let note_a_file = data_dir.join("asset-a-note.json");
        let ingress_a_note_seed = bytes_to_hex(&[51u8; 32]);
        let ingress_a = create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
            data_dir: data_dir.clone(),
            key_file: holder_a_key_file,
            asset_id: asset_a.clone(),
            amount: 17,
            fee: 0,
            note_seed_hex: ingress_a_note_seed.clone(),
            encrypted_output_hex: None,
            ingress_file: ingress_a_file.clone(),
            note_file: note_a_file.clone(),
            overwrite: false,
        })
        .expect("create asset A ingress");
        let ingress_a_file_contents: AssetOrchardIngressFile =
            read_json_file(&ingress_a_file, "encrypted ingress A").expect("read ingress A");
        let ingress_a_wallet_note: AssetOrchardWalletNote =
            read_json_file(&note_a_file, "private ingress A note").expect("read ingress A note");
        let legacy_ingress = AssetOrchardIngressActionPayload {
            burn_transaction: ingress_a_file_contents.payload.burn_transaction.clone(),
            pool_id: ingress_a_file_contents.payload.pool_id.clone(),
            asset_id: ingress_a_file_contents.payload.asset_id.clone(),
            amount: ingress_a_file_contents.payload.amount,
            output_commitment: ingress_a_file_contents.payload.output_commitment.clone(),
            encrypted_output: ingress_a_file_contents.payload.encrypted_output.clone(),
            note: asset_orchard_ingress_note_from_public(&ingress_a_wallet_note.note),
        };
        let legacy_batch = build_shielded_action_batch(
            &genesis,
            vec![ShieldedAction::AssetOrchardIngressV1(
                legacy_ingress.clone(),
            )],
        )
        .expect("build historical ingress v1 fixture");
        let admission_error = reject_live_legacy_cleartext_shielded_actions(&legacy_batch)
            .expect_err("ingress v1 must fail live admission");
        assert_eq!(admission_error.kind(), io::ErrorKind::PermissionDenied);
        let mixed_version_batch = build_shielded_action_batch(
            &genesis,
            vec![
                ShieldedAction::AssetOrchardIngressV1(legacy_ingress.clone()),
                ShieldedAction::AssetOrchardIngressV2(
                    ingress_a_file_contents.payload.clone(),
                ),
            ],
        )
        .expect("build mixed ingress-version attack fixture");
        let mixed_error = reject_live_legacy_cleartext_shielded_actions(&mixed_version_batch)
            .expect_err("a valid v2 action must not mask a legacy v1 action");
        assert_eq!(mixed_error.kind(), io::ErrorKind::PermissionDenied);

        let ledger_before_legacy = store.read_ledger().expect("ledger before legacy ingress");
        let shielded_before_legacy = store
            .read_shielded()
            .expect("shielded state before legacy ingress");
        let mut live_ledger = ledger_before_legacy.clone();
        let mut live_shielded = shielded_before_legacy.clone();
        let live_receipts = execute_shielded_batch(
            &genesis,
            &mut live_ledger,
            &mut live_shielded,
            &legacy_batch,
            1,
            AssetExecutionCompatibility::strict(),
            false,
            false,
        );
        assert_eq!(
            live_receipts[0].code,
            "asset_orchard_ingress_v1_privacy_disabled"
        );
        assert_eq!(live_ledger, ledger_before_legacy);
        assert_eq!(live_shielded, shielded_before_legacy);

        let mut replay_ledger = ledger_before_legacy.clone();
        let mut replay_shielded = shielded_before_legacy.clone();
        let replay_receipts = execute_shielded_batch(
            &genesis,
            &mut replay_ledger,
            &mut replay_shielded,
            &legacy_batch,
            1,
            AssetExecutionCompatibility::strict(),
            false,
            true,
        );
        assert!(replay_receipts[0].accepted, "{replay_receipts:?}");
        assert_ne!(replay_ledger, ledger_before_legacy);
        assert_ne!(replay_shielded, shielded_before_legacy);

        let ingress_a_ciphertext = hex_to_bytes(&ingress_a_file_contents.payload.encrypted_output)
            .expect("ingress ciphertext");
        let recovered_ingress_a = decrypt_asset_orchard_wallet_note(
            &genesis.chain_id,
            asset_orchard_domain_genesis_hash(&genesis_hash(&genesis)).expect("genesis hash"),
            genesis.protocol_version,
            &ingress_a_note_seed,
            &ingress_a_file_contents.payload.output_commitment,
            &ingress_a_ciphertext,
        )
        .expect("scan ingress ciphertext")
        .expect("ingress recipient match");
        assert_eq!(recovered_ingress_a.asset_id, asset_a);
        assert_eq!(recovered_ingress_a.value, 17);
        let ingress_a_batch_file = data_dir.join("asset-a-ingress.batch.json");
        create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
            data_dir: data_dir.clone(),
            ingress_file: ingress_a_file,
            batch_file: ingress_a_batch_file.clone(),
        })
        .expect("create asset A ingress batch");
        let ingress_a_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: ingress_a_batch_file.clone(),
            certificate_file: None,
        })
        .expect("apply asset A ingress");
        assert!(ingress_a_receipts[0].accepted, "{ingress_a_receipts:?}");
        let replay_error = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: data_dir.join("asset-a-ingress.batch.json"),
            certificate_file: None,
        })
        .expect_err("an accepted v2 ingress batch must not replay");
        assert_eq!(replay_error.kind(), io::ErrorKind::AlreadyExists);

        let ingress_b_file = data_dir.join("asset-b-ingress.json");
        let note_b_file = data_dir.join("asset-b-note.json");
        let ingress_b = create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
            data_dir: data_dir.clone(),
            key_file: holder_b_key_file,
            asset_id: asset_b.clone(),
            amount: 34,
            fee: 0,
            note_seed_hex: bytes_to_hex(&[52u8; 32]),
            encrypted_output_hex: None,
            ingress_file: ingress_b_file.clone(),
            note_file: note_b_file.clone(),
            overwrite: false,
        })
        .expect("create asset B ingress");
        let ingress_b_file_contents: AssetOrchardIngressFile =
            read_json_file(&ingress_b_file, "encrypted ingress B").expect("read ingress B");
        let ingress_b_wallet_note: AssetOrchardWalletNote =
            read_json_file(&note_b_file, "private ingress B note").expect("read ingress B note");
        let ingress_b_batch_file = data_dir.join("asset-b-ingress.batch.json");
        create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
            data_dir: data_dir.clone(),
            ingress_file: ingress_b_file,
            batch_file: ingress_b_batch_file.clone(),
        })
        .expect("create asset B ingress batch");
        let ingress_b_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: ingress_b_batch_file.clone(),
            certificate_file: None,
        })
        .expect("apply asset B ingress");
        assert!(ingress_b_receipts[0].accepted, "{ingress_b_receipts:?}");

        let action_file = data_dir.join("asset-orchard-swap-action.json");
        let output_a_file = data_dir.join("asset-orchard-output-a.json");
        let output_b_file = data_dir.join("asset-orchard-output-b.json");
        let pricing_claim_file = data_dir.join("asset-orchard-pricing-claim.json");
        let base_tag = AssetTag::derive(&asset_a).expect("base tag");
        let quote_tag = AssetTag::derive(&asset_b).expect("quote tag");
        fs::write(&pricing_claim_file, serde_json::to_vec(&AssetOrchardPricingClaim {
            nav_epoch: 59,
            reserve_packet_hash: "ab".repeat(48),
            ratio_numerator: 2,
            ratio_denominator: 1,
            mode: "at_nav_with_band".to_string(),
            band_bps: 0,
            base_asset_tag_lo: base_tag.lo,
            base_asset_tag_hi: base_tag.hi,
            quote_asset_tag_lo: quote_tag.lo,
            quote_asset_tag_hi: quote_tag.hi,
        }).expect("pricing json")).expect("write pricing claim");
        let output_note_seeds = [bytes_to_hex(&[61u8; 32]), bytes_to_hex(&[62u8; 32])];
        let swap_report = create_asset_orchard_swap_action(AssetOrchardSwapCreateOptions {
            data_dir: data_dir.clone(),
            input_note_files: [note_a_file, note_b_file],
            output_note_seed_hexes: output_note_seeds.clone(),
            pricing_claim_file,
            action_file: action_file.clone(),
            output_note_files: [output_a_file.clone(), output_b_file.clone()],
            overwrite: false,
        })
        .expect("create AssetOrchard swap action");
        assert_eq!(swap_report.schema, ASSET_ORCHARD_SWAP_CREATE_REPORT_SCHEMA);
        assert_eq!(swap_report.pool_id, ASSET_ORCHARD_POOL_ID_V1);
        assert_eq!(swap_report.nullifiers.len(), 2);
        assert_eq!(swap_report.output_commitments.len(), 2);
        assert!(swap_report.proof_bytes > 0);
        let original_output_b: AssetOrchardWalletNote =
            read_json_file(&output_b_file, "original output B").expect("read output B note");
        #[cfg(unix)]
        {
            assert_private_asset_orchard_note_modes(&output_a_file);
            assert_private_asset_orchard_note_modes(&output_b_file);
        }

        let swap_action_json =
            fs::read_to_string(&action_file).expect("read AssetOrchard swap action");
        assert!(!swap_action_json.contains("\"asset_id\""));
        assert!(!swap_action_json.contains("\"value\""));
        assert!(!swap_action_json.contains(&asset_a));
        assert!(!swap_action_json.contains(&asset_b));

        let parsed_action: AssetOrchardSwapAction =
            serde_json::from_str(&swap_action_json).expect("parse proven pricing action");
        for (index, output_note_seed) in output_note_seeds.iter().enumerate() {
            let recovered = decrypt_asset_orchard_wallet_note(
                &genesis.chain_id,
                asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
                    .expect("swap genesis hash"),
                genesis.protocol_version,
                output_note_seed,
                parsed_action.output_commitments[index].as_hex(),
                &parsed_action.encrypted_outputs[index]
                    .to_bytes()
                    .expect("swap ciphertext"),
            )
            .expect("scan swap ciphertext")
            .expect("swap recipient match");
            assert_eq!(
                recovered.output_commitment,
                parsed_action.output_commitments[index]
            );
        }
        let pricing_domain = orchard_authorizing_domain(&genesis, &parsed_action.pool_id)
            .expect("pricing authorizing domain");
        let verified_pricing = verify_serialized_asset_orchard_swap_action(
            &parsed_action,
            &pricing_domain,
        )
        .expect("verify pricing-bound action");
        validate_asset_orchard_swap_pricing_against_ledger(&pricing_ledger, &verified_pricing, 1)
            .expect("at-NAV pricing must pass");
        let mut stale_epoch = pricing_ledger.clone();
        stale_epoch.nav_assets[0].finalized_epoch += 1;
        assert_eq!(
            validate_asset_orchard_swap_pricing_against_ledger(&stale_epoch, &verified_pricing, 1)
                .expect_err("stale pricing epoch must fail")
                .code(),
            "asset_orchard_pricing_epoch_mismatch"
        );
        let mut wrong_packet = pricing_ledger.clone();
        wrong_packet.nav_assets[0].finalized_reserve_packet_hash = "cd".repeat(48);
        assert_eq!(
            validate_asset_orchard_swap_pricing_against_ledger(&wrong_packet, &verified_pricing, 1)
                .expect_err("wrong reserve packet must fail")
                .code(),
            "asset_orchard_pricing_packet_mismatch"
        );
        let mut off_band = pricing_ledger.clone();
        off_band.nav_assets[0].nav_per_unit += 1;
        assert_eq!(
            validate_asset_orchard_swap_pricing_against_ledger(&off_band, &verified_pricing, 1)
                .expect_err("off-band private amounts must fail")
                .code(),
            "asset_orchard_pricing_off_band"
        );

        let mut invalid_swap_action: AssetOrchardSwapAction =
            serde_json::from_str(&swap_action_json).expect("parse AssetOrchard swap action");
        invalid_swap_action.proof =
            AssetOrchardProofBytes::from_bytes(b"invalid-proof").expect("invalid proof bytes");
        let invalid_receipts = apply_raw_shielded_swap_payload(
            &data_dir,
            &genesis,
            "asset-orchard-invalid-proof.batch.json",
            serde_json::to_string(&invalid_swap_action).expect("serialize invalid swap action"),
        );
        assert_eq!(invalid_receipts.len(), 1);
        assert!(!invalid_receipts[0].accepted, "{invalid_receipts:?}");
        assert_eq!(
            invalid_receipts[0].code,
            "asset_orchard_swap_proof_verification_failed"
        );
        let shielded_after_invalid = store.read_shielded().expect("shielded after invalid swap");
        let invalid_pool = shielded_after_invalid
            .orchard
            .as_ref()
            .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
            .expect("asset orchard pool after invalid swap");
        assert!(invalid_pool.nullifiers.is_empty());
        assert_eq!(invalid_pool.output_commitments.len(), 2);

        let swap_batch_file = data_dir.join("asset-orchard-swap.batch.json");
        create_shielded_swap_action_batch(ShieldedSwapActionBatchOptions {
            data_dir: data_dir.clone(),
            swap_file: action_file,
            batch_file: swap_batch_file.clone(),
        })
        .expect("create AssetOrchard swap batch");
        let swap_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: swap_batch_file.clone(),
            certificate_file: None,
        })
        .expect("apply AssetOrchard swap batch");
        assert_eq!(swap_receipts.len(), 1);
        assert!(swap_receipts[0].accepted, "{swap_receipts:?}");

        let ledger_after_swap = store.read_ledger().expect("ledger after swap");
        assert_eq!(
            ledger_after_swap
                .trustline_for_account_asset(&holder_a_key.address, &asset_a)
                .expect("holder A asset A trustline")
                .balance,
            23
        );
        assert_eq!(
            ledger_after_swap
                .trustline_for_account_asset(&holder_b_key.address, &asset_b)
                .expect("holder B asset B trustline")
                .balance,
            16
        );
        let shielded_after_swap = store.read_shielded().expect("shielded after swap");
        let pool = shielded_after_swap
            .orchard
            .as_ref()
            .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
            .expect("asset orchard pool after swap");
        assert_eq!(pool.nullifiers.len(), 2);
        assert_eq!(pool.output_commitments.len(), 4);
        assert_eq!(pool.asset_orchard_outputs.len(), 4);
        assert!(pool
            .output_commitments
            .iter()
            .any(|commitment| commitment == &ingress_a.output_commitment));
        assert!(pool
            .output_commitments
            .iter()
            .any(|commitment| commitment == &ingress_b.output_commitment));
        for output_commitment in &swap_report.output_commitments {
            assert!(pool
                .output_commitments
                .iter()
                .any(|existing| existing == output_commitment));
        }
        let original_output_a: AssetOrchardWalletNote =
            read_json_file(&output_a_file, "original output A").expect("read output A note");
        fs::remove_file(&output_a_file).expect("remove local output A note before chain scan");
        let recovered_output_a_file = data_dir.join("asset-orchard-recovered-output-a.json");
        let scan_report = asset_orchard_scan(AssetOrchardScanOptions {
            data_dir: data_dir.clone(),
            note_seed_hex: output_note_seeds[0].clone(),
            note_file: recovered_output_a_file.clone(),
            overwrite: false,
        })
        .expect("recover output A from chain-only ciphertext");
        assert!(scan_report.recovered);
        assert_eq!(scan_report.chain_output_count, 4);
        assert_eq!(scan_report.encrypted_output_v1_count, 4);
        assert_eq!(scan_report.legacy_output_count, 0);
        let recovered_output_a: AssetOrchardWalletNote =
            read_json_file(&recovered_output_a_file, "recovered output A")
                .expect("read recovered output A note");
        assert_eq!(recovered_output_a, original_output_a);
        verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after AssetOrchard swap");

        let egress_asset_id = recovered_output_a.asset_id.clone();
        let egress_amount = recovered_output_a.value;
        let ledger_before_egress = store.read_ledger().expect("ledger before private egress");
        let public_balance_before_egress = ledger_before_egress
            .trustline_for_account_asset(&holder_a_key.address, &egress_asset_id)
            .expect("holder A egress-asset trustline before private egress")
            .balance;
        let supply_before_egress = global_issued_asset_supply(
            &ledger_before_egress,
            &store
                .read_shielded()
                .expect("shielded state before private egress"),
            &egress_asset_id,
        )
        .expect("issued supply before private egress");
        let private_egress_file = data_dir.join("asset-orchard-private-egress.json");
        let private_egress_report =
            create_asset_orchard_private_egress(AssetOrchardPrivateEgressCreateOptions {
                data_dir: data_dir.clone(),
                note_file: recovered_output_a_file.clone(),
                to: holder_a_key.address.clone(),
                asset_id: Some(egress_asset_id.clone()),
                amount: Some(egress_amount),
                fee: 0,
                policy_id: "wallet_private_egress_public_exit_v1".to_string(),
                disclosure_hash: "privacy-p0-issued-asset-round-trip".to_string(),
                egress_file: private_egress_file.clone(),
                overwrite: false,
            })
            .expect("create AssetOrchard private egress");
        assert!(private_egress_report.verified);
        assert_eq!(private_egress_report.asset_id, egress_asset_id);
        assert_eq!(private_egress_report.amount, egress_amount);
        assert_eq!(private_egress_report.fee, 0);
        assert_eq!(
            private_egress_report.privacy,
            "private_note_opening_egress_proof_public_exit"
        );

        let private_egress_batch_file =
            data_dir.join("asset-orchard-private-egress.batch.json");
        create_asset_orchard_private_egress_batch(
            AssetOrchardPrivateEgressBatchOptions {
                data_dir: data_dir.clone(),
                egress_file: private_egress_file.clone(),
                batch_file: private_egress_batch_file.clone(),
            },
        )
        .expect("create AssetOrchard private egress batch");
        let private_egress_receipts = apply_shielded_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: private_egress_batch_file.clone(),
            certificate_file: None,
        })
        .expect("apply AssetOrchard private egress batch");
        assert_eq!(private_egress_receipts.len(), 1);
        assert!(
            private_egress_receipts[0].accepted,
            "{private_egress_receipts:?}"
        );
        assert_eq!(private_egress_receipts[0].code, "accepted");

        let note_status = asset_orchard_note_status(AssetOrchardNoteStatusOptions {
            data_dir: data_dir.clone(),
            note_file: recovered_output_a_file,
        })
        .expect("read private-egress input note status");
        assert!(note_status.pool_output);
        assert!(note_status.spent);
        assert!(!note_status.spendable);
        assert_eq!(note_status.nullifier, private_egress_report.nullifier);

        let ledger_after_egress = store.read_ledger().expect("ledger after private egress");
        let public_balance_after_egress = ledger_after_egress
            .trustline_for_account_asset(&holder_a_key.address, &egress_asset_id)
            .expect("holder A egress-asset trustline after private egress")
            .balance;
        assert_eq!(
            public_balance_after_egress,
            public_balance_before_egress + egress_amount,
            "private egress must credit the exact public issued-asset amount"
        );
        let shielded_after_egress = store
            .read_shielded()
            .expect("shielded state after private egress");
        assert_eq!(
            global_issued_asset_supply(
                &ledger_after_egress,
                &shielded_after_egress,
                &egress_asset_id,
            )
            .expect("issued supply after private egress"),
            supply_before_egress,
            "private egress must move custody without changing global issued supply"
        );
        verify_shielded(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify shielded state after AssetOrchard private egress");

        let private_notes = [
            &ingress_a_wallet_note,
            &ingress_b_wallet_note,
            &original_output_a,
            &original_output_b,
        ];
        let public_artifacts = [
            (
                "ingress-a-envelope",
                serde_json::to_string(&ingress_a_file_contents)
                    .expect("serialize public ingress A envelope"),
            ),
            (
                "ingress-b-envelope",
                serde_json::to_string(&ingress_b_file_contents)
                    .expect("serialize public ingress B envelope"),
            ),
            (
                "ingress-a-batch",
                fs::read_to_string(&ingress_a_batch_file).expect("read public ingress A batch"),
            ),
            (
                "ingress-b-batch",
                fs::read_to_string(&ingress_b_batch_file).expect("read public ingress B batch"),
            ),
            ("swap-action", swap_action_json),
            (
                "swap-batch",
                fs::read_to_string(&swap_batch_file).expect("read public private-swap batch"),
            ),
            (
                "private-egress-envelope",
                fs::read_to_string(&private_egress_file)
                    .expect("read public private-egress envelope"),
            ),
            (
                "private-egress-batch",
                fs::read_to_string(&private_egress_batch_file)
                    .expect("read public private-egress batch"),
            ),
            (
                "batch-archive",
                serde_json::to_string(&store.read_batch_archive().expect("read batch archive"))
                    .expect("serialize public batch archive"),
            ),
            (
                "block-log",
                serde_json::to_string(&store.read_blocks().expect("read block log"))
                    .expect("serialize public block log"),
            ),
            (
                "receipt-log",
                serde_json::to_string(&store.read_receipts().expect("read receipt log"))
                    .expect("serialize public receipt log"),
            ),
            (
                "ledger",
                serde_json::to_string(&ledger_after_egress).expect("serialize public ledger"),
            ),
            (
                "shielded-state",
                serde_json::to_string(&shielded_after_egress)
                    .expect("serialize public shielded state"),
            ),
        ];
        for (label, artifact) in &public_artifacts {
            assert_asset_orchard_public_artifact_redacted(label, artifact, &private_notes);
        }

        fs::remove_dir_all(data_dir).expect("cleanup asset orchard swap from ingress test");
    }
