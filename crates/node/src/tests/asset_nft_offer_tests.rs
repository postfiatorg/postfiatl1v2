    #[test]
    fn asset_replay_preserves_conservation_and_trustline_invariants() {
        let data_dir = unique_test_dir("postfiat-asset-invariant-replay-test");
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
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let holder = address_from_public_key(&holder_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let holder_public_key_hex = bytes_to_hex(&holder_key.public_key);
        let holder_private_key_hex = bytes_to_hex(&holder_key.private_key);
        let recipient_public_key_hex = bytes_to_hex(&recipient_key.public_key);
        let recipient_private_key_hex = bytes_to_hex(&recipient_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_holder = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            holder.clone(),
            ACCOUNT_RESERVE + 700,
            1,
        )
        .expect("fund holder transfer");
        let fund_recipient = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            recipient.clone(),
            ACCOUNT_RESERVE + 700,
            2,
        )
        .expect("fund recipient transfer");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![fund_holder, fund_recipient],
        )
        .expect("funding batch")
        .batch;
        let funding_batch_file = data_dir.join("fund-asset-holders-batch.json");
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
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "JPY".to_string(),
                version: 1,
                precision: 0,
                display_name: "Yen".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();
        assert_asset_invariants_for_test(&genesis, &dry_run_ledger, &asset_id, 0, &[]);

        let holder_trust = signed_asset_transaction_for_test(
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
                limit: 300,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &holder_trust, 1).accepted
        );

        let recipient_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 150,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &recipient_trust, 1).accepted
        );
        assert_asset_invariants_for_test(
            &genesis,
            &dry_run_ledger,
            &asset_id,
            0,
            &[(holder.as_str(), 0), (recipient.as_str(), 0)],
        );

        let issue = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: holder.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 200,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &issue, 1).accepted);
        assert_asset_invariants_for_test(
            &genesis,
            &dry_run_ledger,
            &asset_id,
            200,
            &[(holder.as_str(), 200), (recipient.as_str(), 0)],
        );

        let holder_to_recipient = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder,
            &holder_public_key_hex,
            &holder_private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: holder.clone(),
                to: recipient.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 70,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &holder_to_recipient, 1).accepted
        );

        let recipient_burn = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            ASSET_BURN_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetBurn(AssetBurnOperation {
                owner: recipient.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 20,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &recipient_burn, 1).accepted
        );
        assert_asset_invariants_for_test(
            &genesis,
            &dry_run_ledger,
            &asset_id,
            180,
            &[(holder.as_str(), 130), (recipient.as_str(), 50)],
        );

        let asset_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![
                create.clone(),
                holder_trust.clone(),
                recipient_trust.clone(),
                issue.clone(),
                holder_to_recipient.clone(),
                recipient_burn.clone(),
            ],
        )
        .expect("asset invariant batch")
        .batch;
        let asset_batch_file = data_dir.join("asset-invariant-batch.json");
        write_batch_file(&asset_batch_file, &asset_batch).expect("write asset batch");

        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: asset_batch_file,
            certificate_file: None,
        })
        .expect("apply asset invariant batch");
        assert_eq!(receipts.len(), 6);
        assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
        assert_eq!(receipts[5].tx_id, asset_transaction_tx_id(&recipient_burn));

        let ledger = store.read_ledger().expect("ledger after asset invariants");
        assert_asset_invariants_for_test(
            &genesis,
            &ledger,
            &asset_id,
            180,
            &[(holder.as_str(), 130), (recipient.as_str(), 50)],
        );

        let info = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("asset_info");
        let asset = info.asset.expect("asset info row");
        assert_eq!(asset.outstanding_supply, 180);
        assert_eq!(asset.trustline_count, 2);
        assert_eq!(asset.holder_count, 2);

        let holder_lines = account_lines(AccountLinesOptions {
            data_dir: data_dir.clone(),
            account: holder.clone(),
            issuer: Some(faucet_key.address.clone()),
            asset_id: Some(asset_id.clone()),
            limit: Some(10),
        })
        .expect("holder account_lines");
        assert_eq!(holder_lines.line_count, 1);
        assert_eq!(holder_lines.lines[0].balance, 130);
        assert_eq!(holder_lines.lines[0].limit, 300);

        let recipient_assets = account_assets(AccountAssetsOptions {
            data_dir: data_dir.clone(),
            account: recipient.clone(),
            asset_id: Some(asset_id.clone()),
            limit: Some(10),
        })
        .expect("recipient account_assets");
        assert_eq!(recipient_assets.asset_count, 1);
        assert_eq!(recipient_assets.assets[0].balance, 50);

        let issuer_report = issuer_assets(IssuerAssetsOptions {
            data_dir: data_dir.clone(),
            issuer: faucet_key.address.clone(),
            limit: Some(10),
        })
        .expect("issuer_assets");
        assert_eq!(issuer_report.asset_count, 1);
        assert_eq!(issuer_report.assets[0].outstanding_supply, 180);
        assert_eq!(issuer_report.assets[0].holder_count, 2);

        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("asset invariant replay verification");
        let burn_finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: receipts[5].tx_id.clone(),
            audit_block_log: true,
        })
        .expect("burn tx finality");
        assert!(burn_finality.confirmed);
        assert_eq!(burn_finality.receipt_count, 6);

        let recipient_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: recipient.clone(),
            from_height: Some(2),
            to_height: Some(2),
            limit: Some(10),
        })
        .expect("recipient account_tx");
        assert_eq!(recipient_history.row_count, 3);
        assert_eq!(
            recipient_history.rows[0].transaction_kind,
            TRUST_SET_TRANSACTION_KIND
        );
        assert_eq!(
            recipient_history.rows[1].transaction_kind,
            ISSUED_PAYMENT_TRANSACTION_KIND
        );
        assert_eq!(
            recipient_history.rows[2].transaction_kind,
            ASSET_BURN_TRANSACTION_KIND
        );
        assert_eq!(
            recipient_history.rows[2].asset_id.as_deref(),
            Some(asset_id.as_str())
        );
        assert_eq!(recipient_history.rows[2].amount, 20);
        assert_eq!(recipient_history.rows[2].accepted, Some(true));

        fs::remove_dir_all(data_dir).expect("cleanup asset invariant replay test");
    }
    #[test]
    fn issued_asset_escrow_fee_mempool_account_tx_and_replay_flow() {
        let data_dir = unique_test_dir("postfiat-issued-asset-escrow-test");
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
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let holder = address_from_public_key(&holder_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let holder_public_key_hex = bytes_to_hex(&holder_key.public_key);
        let holder_private_key_hex = bytes_to_hex(&holder_key.private_key);
        let recipient_public_key_hex = bytes_to_hex(&recipient_key.public_key);
        let recipient_private_key_hex = bytes_to_hex(&recipient_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_holder = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            holder.clone(),
            ACCOUNT_RESERVE + 700,
            1,
        )
        .expect("fund holder transfer");
        let fund_recipient = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            recipient.clone(),
            ACCOUNT_RESERVE + 700,
            2,
        )
        .expect("fund recipient transfer");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![fund_holder, fund_recipient],
        )
        .expect("funding batch")
        .batch;
        let funding_batch_file = data_dir.join("fund-issued-escrow-holders-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(
            funding_receipts.iter().all(|receipt| receipt.accepted),
            "{funding_receipts:?}"
        );

        let ledger = store.read_ledger().expect("funded ledger");
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "IESC".to_string(),
                version: 1,
                precision: 0,
                display_name: "Issued Escrow".to_string(),
                max_supply: Some(150),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();

        let holder_trust = signed_asset_transaction_for_test(
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
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &holder_trust, 1).accepted);

        let recipient_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: recipient.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &recipient_trust, 1).accepted
        );

        let issue = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: holder.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 90,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &issue, 1).accepted);
        assert_asset_invariants_for_test(
            &genesis,
            &dry_run_ledger,
            &asset_id,
            90,
            &[(holder.as_str(), 90), (recipient.as_str(), 0)],
        );

        let asset_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![
                create.clone(),
                holder_trust.clone(),
                recipient_trust.clone(),
                issue.clone(),
            ],
        )
        .expect("asset setup batch")
        .batch;
        let asset_batch_file = data_dir.join("issued-escrow-asset-setup-batch.json");
        write_batch_file(&asset_batch_file, &asset_batch).expect("write asset setup batch");
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: asset_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup batch");
        assert_eq!(receipts.len(), 4);
        assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");

        let create_operation =
            EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
                owner: holder.clone(),
                recipient: recipient.clone(),
                asset_id: asset_id.clone(),
                amount: 35,
                condition: "node-issued-secret".to_string(),
                finish_after: 3,
                cancel_after: 6,
            });
        let create_quote = escrow_fee_quote(EscrowFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: holder.clone(),
            operation_json: serde_json::to_string(&create_operation).expect("escrow op json"),
            sequence: None,
        })
        .expect("issued escrow fee quote");
        assert_eq!(create_quote.transaction_kind, ESCROW_CREATE_TRANSACTION_KIND);
        assert_eq!(create_quote.sequence, 2);
        assert!(create_quote.state_expansion_fee >= postfiat_execution::ESCROW_STATE_EXPANSION_FEE);
        assert_eq!(create_quote.operation, create_operation);

        let ledger = store.read_ledger().expect("ledger after asset setup");
        let create_escrow = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &holder,
            &holder_public_key_hex,
            &holder_private_key_hex,
            ESCROW_CREATE_TRANSACTION_KIND,
            create_quote.sequence,
            create_quote.operation.clone(),
        );
        assert_eq!(create_escrow.unsigned.fee, create_quote.minimum_fee);
        let create_entry = submit_signed_escrow_transaction_json_to_mempool(
            SignedEscrowTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_escrow_transaction_json: serde_json::to_string(&create_escrow)
                    .expect("signed issued escrow json"),
            },
        )
        .expect("submit issued escrow");
        assert_eq!(
            create_entry.tx_id,
            postfiat_execution::escrow_transaction_tx_id(&create_escrow)
        );

        let mempool_report = verify_mempool(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify issued escrow mempool");
        assert!(mempool_report.verified);
        assert_eq!(mempool_report.pending_count, 1);

        let batch_file = data_dir.join("issued-asset-escrow-mempool-batch.json");
        let batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create issued escrow mempool batch");
        assert_eq!(batch.escrow_transactions.len(), 1);
        let create_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply issued escrow create batch");
        assert_eq!(create_receipts.len(), 1);
        assert!(create_receipts[0].accepted, "{create_receipts:?}");

        let escrow_id =
            postfiat_types::escrow_id(&genesis.chain_id, &holder, create_quote.sequence)
                .expect("issued escrow id");
        let ledger = store.read_ledger().expect("ledger after issued escrow");
        assert_asset_invariants_for_test(
            &genesis,
            &ledger,
            &asset_id,
            90,
            &[(holder.as_str(), 55), (recipient.as_str(), 0)],
        );
        let escrow = ledger.escrow(&escrow_id).expect("issued escrow");
        assert_eq!(escrow.asset_id, asset_id);
        assert_eq!(escrow.state, ESCROW_STATE_OPEN);

        let asset = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("asset_info with open issued escrow")
        .asset
        .expect("asset info row");
        assert_eq!(asset.outstanding_supply, 90);
        assert_eq!(asset.holder_count, 1);

        let metrics = crate::metrics(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("metrics with open issued escrow");
        assert_eq!(metrics.assets.asset_count, 1);
        assert_eq!(metrics.assets.trustline_count, 2);
        assert_eq!(metrics.assets.holder_count, 1);
        assert_eq!(metrics.assets.total_outstanding_supply, 90);
        assert_eq!(metrics.assets.open_issued_escrow_count, 1);
        assert_eq!(metrics.assets.open_issued_escrow_amount, 35);
        assert_eq!(metrics.assets.open_issued_offer_count, 0);
        assert_eq!(metrics.assets.freeze_enabled_asset_count, 1);

        rebuild_account_tx_index(AccountTxIndexOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuild issued escrow account_tx index");
        let holder_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(3),
            to_height: Some(3),
            limit: Some(10),
        })
        .expect("holder issued escrow account_tx");
        assert!(holder_history.index_used);
        assert_eq!(holder_history.row_count, 1);
        assert_eq!(
            holder_history.rows[0].transaction_kind,
            ESCROW_CREATE_TRANSACTION_KIND
        );
        assert_eq!(holder_history.rows[0].asset_id.as_deref(), Some(asset_id.as_str()));
        assert_eq!(
            holder_history.rows[0].escrow_id.as_deref(),
            Some(escrow_id.as_str())
        );
        assert_eq!(holder_history.rows[0].amount, 35);
        assert_eq!(holder_history.rows[0].accepted, Some(true));

        let recipient_escrows = account_escrows(AccountEscrowsOptions {
            data_dir: data_dir.clone(),
            account: recipient.clone(),
            role: Some("recipient".to_string()),
            state: Some(ESCROW_STATE_OPEN.to_string()),
            limit: Some(10),
        })
        .expect("recipient issued escrows");
        assert_eq!(recipient_escrows.escrow_count, 1);
        assert_eq!(recipient_escrows.escrows[0].asset_id, asset_id);

        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("issued escrow replay after create");

        let finish = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            ESCROW_FINISH_TRANSACTION_KIND,
            2,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: escrow_id.clone(),
                owner: holder.clone(),
                recipient: recipient.clone(),
                fulfillment: "node-issued-secret".to_string(),
            }),
        );
        let finish_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![finish.clone()],
        )
        .expect("issued escrow finish batch")
        .batch;
        let finish_batch_file = data_dir.join("issued-asset-escrow-finish-batch.json");
        write_batch_file(&finish_batch_file, &finish_batch).expect("write finish batch");
        let finish_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: finish_batch_file,
            certificate_file: None,
        })
        .expect("apply issued escrow finish");
        assert_eq!(finish_receipts.len(), 1);
        assert!(finish_receipts[0].accepted, "{finish_receipts:?}");

        let ledger = store.read_ledger().expect("ledger after issued escrow finish");
        assert_eq!(
            ledger.escrow(&escrow_id).expect("finished issued escrow").state,
            ESCROW_STATE_FINISHED
        );
        assert_asset_invariants_for_test(
            &genesis,
            &ledger,
            &asset_id,
            90,
            &[(holder.as_str(), 55), (recipient.as_str(), 35)],
        );
        let asset = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("asset_info after issued escrow finish")
        .asset
        .expect("asset info row");
        assert_eq!(asset.outstanding_supply, 90);
        assert_eq!(asset.holder_count, 2);

        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("issued escrow replay verification");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);
        let finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: finish_receipts[0].tx_id.clone(),
            audit_block_log: true,
        })
        .expect("issued escrow finish finality");
        assert!(finality.confirmed);

        fs::remove_dir_all(data_dir).expect("cleanup issued asset escrow test");
    }

    #[test]
    fn atomic_settlement_template_builds_pft_issued_swap_through_escrow_rails() {
        let data_dir = unique_test_dir("postfiat-atomic-template-test");
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
        let pft_owner_key = ml_dsa_65_keygen().expect("pft owner keygen");
        let issued_owner_key = ml_dsa_65_keygen().expect("issued owner keygen");
        let pft_owner = address_from_public_key(&pft_owner_key.public_key);
        let issued_owner = address_from_public_key(&issued_owner_key.public_key);
        let pft_owner_public_key_hex = bytes_to_hex(&pft_owner_key.public_key);
        let pft_owner_private_key_hex = bytes_to_hex(&pft_owner_key.private_key);
        let issued_owner_public_key_hex = bytes_to_hex(&issued_owner_key.public_key);
        let issued_owner_private_key_hex = bytes_to_hex(&issued_owner_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_pft_owner = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            pft_owner.clone(),
            ACCOUNT_RESERVE + 900,
            1,
        )
        .expect("fund pft owner");
        let fund_issued_owner = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            issued_owner.clone(),
            ACCOUNT_RESERVE + 900,
            2,
        )
        .expect("fund issued owner");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![fund_pft_owner, fund_issued_owner],
        )
        .expect("funding batch")
        .batch;
        let funding_batch_file = data_dir.join("atomic-template-funding.batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(
            funding_receipts.iter().all(|receipt| receipt.accepted),
            "{funding_receipts:?}"
        );

        let ledger = store.read_ledger().expect("funded ledger");
        let create_asset = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "SWAP".to_string(),
                version: 1,
                precision: 0,
                display_name: "Atomic Swap".to_string(),
                max_supply: Some(120),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create_asset, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();

        let pft_owner_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &pft_owner,
            &pft_owner_public_key_hex,
            &pft_owner_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: pft_owner.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &pft_owner_trust, 1).accepted
        );
        let issued_owner_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &issued_owner,
            &issued_owner_public_key_hex,
            &issued_owner_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: issued_owner.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 100,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(
            execute_asset_transaction(&genesis, &mut dry_run_ledger, &issued_owner_trust, 1).accepted
        );
        let issue_asset = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: issued_owner.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 80,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &issue_asset, 1).accepted);
        let asset_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![
                create_asset.clone(),
                pft_owner_trust.clone(),
                issued_owner_trust.clone(),
                issue_asset.clone(),
            ],
        )
        .expect("asset setup batch")
        .batch;
        let asset_batch_file = data_dir.join("atomic-template-asset-setup.batch.json");
        write_batch_file(&asset_batch_file, &asset_batch).expect("write asset setup batch");
        let asset_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: asset_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup batch");
        assert!(
            asset_receipts.iter().all(|receipt| receipt.accepted),
            "{asset_receipts:?}"
        );

        let template = atomic_settlement_template(AtomicSettlementTemplateOptions {
            data_dir: data_dir.clone(),
            left_owner: pft_owner.clone(),
            left_recipient: issued_owner.clone(),
            left_asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            left_amount: 100,
            right_owner: issued_owner.clone(),
            right_recipient: pft_owner.clone(),
            right_asset_id: asset_id.clone(),
            right_amount: 30,
            condition: "atomic-shared-secret".to_string(),
            finish_after: 3,
            cancel_after: 8,
            left_sequence: None,
            right_sequence: None,
        })
        .expect("atomic settlement template");
        assert_eq!(template.schema, "postfiat-atomic-settlement-template-v1");
        assert_eq!(template.left.sequence, 2);
        assert_eq!(template.right.sequence, 2);
        assert_eq!(template.left.sequence_source, "ledger_mempool");
        assert_eq!(template.right.sequence_source, "ledger_mempool");
        assert_eq!(
            template.left.transaction_kind,
            ESCROW_CREATE_TRANSACTION_KIND
        );
        assert_eq!(
            template.right.transaction_kind,
            ESCROW_CREATE_TRANSACTION_KIND
        );
        assert_eq!(template.left.operation.transaction_kind(), ESCROW_CREATE_TRANSACTION_KIND);
        assert_eq!(template.right.operation.transaction_kind(), ESCROW_CREATE_TRANSACTION_KIND);
        assert_ne!(template.left.escrow_id, template.right.escrow_id);
        assert_eq!(
            template.condition_hash,
            postfiat_types::escrow_condition_hash("atomic-shared-secret")
                .expect("condition hash")
        );

        let ledger = store.read_ledger().expect("ledger before atomic escrows");
        let left_create = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &pft_owner,
            &pft_owner_public_key_hex,
            &pft_owner_private_key_hex,
            ESCROW_CREATE_TRANSACTION_KIND,
            template.left.sequence,
            template.left.operation.clone(),
        );
        let right_create = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &issued_owner,
            &issued_owner_public_key_hex,
            &issued_owner_private_key_hex,
            ESCROW_CREATE_TRANSACTION_KIND,
            template.right.sequence,
            template.right.operation.clone(),
        );
        submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_escrow_transaction_json: serde_json::to_string(&left_create)
                .expect("left create json"),
        })
        .expect("submit left escrow");
        submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_escrow_transaction_json: serde_json::to_string(&right_create)
                .expect("right create json"),
        })
        .expect("submit right escrow");
        let mempool_report = verify_mempool(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify atomic template mempool");
        assert!(mempool_report.verified);
        assert_eq!(mempool_report.pending_count, 2);

        let create_batch_file = data_dir.join("atomic-template-create.batch.json");
        let create_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: create_batch_file.clone(),
            max_transactions: 2,
        })
        .expect("create atomic template escrow batch");
        assert_eq!(create_batch.escrow_transactions.len(), 2);
        let create_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: create_batch_file,
            certificate_file: None,
        })
        .expect("apply atomic template escrows");
        assert_eq!(create_receipts.len(), 2);
        assert!(
            create_receipts.iter().all(|receipt| receipt.accepted),
            "{create_receipts:?}"
        );

        let ledger = store.read_ledger().expect("ledger after atomic escrow create");
        assert_eq!(
            ledger
                .escrow(&template.left.escrow_id)
                .expect("left escrow")
                .state,
            ESCROW_STATE_OPEN
        );
        assert_eq!(
            ledger
                .escrow(&template.right.escrow_id)
                .expect("right escrow")
                .state,
            ESCROW_STATE_OPEN
        );
        assert_asset_invariants_for_test(
            &genesis,
            &ledger,
            &asset_id,
            80,
            &[(pft_owner.as_str(), 0), (issued_owner.as_str(), 50)],
        );
        rebuild_account_tx_index(AccountTxIndexOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuild atomic settlement account_tx index");
        let pft_owner_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: pft_owner.clone(),
            from_height: Some(3),
            to_height: Some(3),
            limit: Some(10),
        })
        .expect("pft owner atomic account_tx");
        assert!(pft_owner_history.index_used);
        assert_eq!(pft_owner_history.row_count, 2);
        assert!(
            pft_owner_history
                .rows
                .iter()
                .all(|row| row.condition_hash.as_deref() == Some(template.condition_hash.as_str()))
        );
        assert!(
            pft_owner_history
                .rows
                .iter()
                .any(|row| row.escrow_id.as_deref() == Some(template.left.escrow_id.as_str()))
        );
        assert!(
            pft_owner_history
                .rows
                .iter()
                .any(|row| row.escrow_id.as_deref() == Some(template.right.escrow_id.as_str())
                    && row.asset_id.as_deref() == Some(asset_id.as_str()))
        );

        let left_finish = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &issued_owner,
            &issued_owner_public_key_hex,
            &issued_owner_private_key_hex,
            ESCROW_FINISH_TRANSACTION_KIND,
            3,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: template.left.escrow_id.clone(),
                owner: pft_owner.clone(),
                recipient: issued_owner.clone(),
                fulfillment: "atomic-shared-secret".to_string(),
            }),
        );
        let right_finish = signed_escrow_transaction_for_test(
            &genesis,
            &ledger,
            &pft_owner,
            &pft_owner_public_key_hex,
            &pft_owner_private_key_hex,
            ESCROW_FINISH_TRANSACTION_KIND,
            3,
            EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
                escrow_id: template.right.escrow_id.clone(),
                owner: issued_owner.clone(),
                recipient: pft_owner.clone(),
                fulfillment: "atomic-shared-secret".to_string(),
            }),
        );
        let finish_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![left_finish, right_finish],
        )
        .expect("atomic finish batch")
        .batch;
        let finish_batch_file = data_dir.join("atomic-template-finish.batch.json");
        write_batch_file(&finish_batch_file, &finish_batch).expect("write atomic finish batch");
        let finish_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: finish_batch_file,
            certificate_file: None,
        })
        .expect("apply atomic finish batch");
        assert_eq!(finish_receipts.len(), 2);
        assert!(
            finish_receipts.iter().all(|receipt| receipt.accepted),
            "{finish_receipts:?}"
        );

        let ledger = store.read_ledger().expect("ledger after atomic finish");
        assert_eq!(
            ledger
                .escrow(&template.left.escrow_id)
                .expect("left finished escrow")
                .state,
            ESCROW_STATE_FINISHED
        );
        assert_eq!(
            ledger
                .escrow(&template.right.escrow_id)
                .expect("right finished escrow")
                .state,
            ESCROW_STATE_FINISHED
        );
        assert_asset_invariants_for_test(
            &genesis,
            &ledger,
            &asset_id,
            80,
            &[(pft_owner.as_str(), 30), (issued_owner.as_str(), 50)],
        );
        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("atomic template replay verification");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);

        fs::remove_dir_all(data_dir).expect("cleanup atomic template test");
    }

    #[test]
    fn asset_fee_quote_mempool_batch_and_replay_flow() {
        let data_dir = unique_test_dir("postfiat-asset-mempool-test");
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
        let create_operation = AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: faucet_key.address.clone(),
            code: "EUR".to_string(),
            version: 1,
            precision: 2,
            display_name: "Euro".to_string(),
            max_supply: Some(10_000),
            requires_authorization: false,
            freeze_enabled: true,
            clawback_enabled: true,
        });
        let create_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&create_operation).expect("create op json"),
            sequence: None,
        })
        .expect("asset create fee quote");
        assert_eq!(create_quote.transaction_kind, ASSET_CREATE_TRANSACTION_KIND);
        assert_eq!(create_quote.sequence, 2);
        assert!(create_quote.state_expansion_fee >= postfiat_execution::ASSET_DEFINITION_STATE_EXPANSION_FEE);
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            create_quote.sequence,
            create_operation.clone(),
        );
        assert_eq!(create.unsigned.fee, create_quote.minimum_fee);
        let create_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&create)
                    .expect("signed create json"),
            },
        )
        .expect("submit asset create");
        assert_eq!(create_entry.tx_id, asset_transaction_tx_id(&create));

        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create, 1).accepted);
        let asset_id = postfiat_types::issued_asset_id(
            &genesis.chain_id,
            &faucet_key.address,
            "EUR",
            1,
        )
        .expect("asset id");
        let trust_operation = AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.clone(),
            issuer: faucet_key.address.clone(),
            asset_id: asset_id.clone(),
            limit: 250,
            authorized: false,
            frozen: false,
            reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
        });
        let trust_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: holder.clone(),
            operation_json: serde_json::to_string(&trust_operation).expect("trust op json"),
            sequence: None,
        })
        .expect("trust set fee quote");
        assert_eq!(trust_quote.transaction_kind, TRUST_SET_TRANSACTION_KIND);
        assert_eq!(trust_quote.sequence, 1);
        assert!(trust_quote.state_expansion_fee >= postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE);
        let trust_set = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &holder,
            &holder_public_key_hex,
            &holder_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            trust_quote.sequence,
            trust_operation,
        );
        assert_eq!(trust_set.unsigned.fee, trust_quote.minimum_fee);
        let trust_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&trust_set)
                    .expect("signed trust json"),
            },
        )
        .expect("submit trust set");
        assert_eq!(trust_entry.tx_id, asset_transaction_tx_id(&trust_set));

        let mempool = mempool_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("mempool state");
        assert_eq!(mempool.pending.len(), 0);
        assert_eq!(mempool.pending_payment_v2.len(), 0);
        assert_eq!(mempool.pending_asset_transactions.len(), 2);
        let mempool_report = verify_mempool(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify asset mempool");
        assert_eq!(mempool_report.pending_count, 2);
        assert_eq!(mempool_report.latest_tx_id, trust_entry.tx_id);
        assert_eq!(
            mempool_report.total_fee,
            create.unsigned.fee + trust_set.unsigned.fee
        );

        let batch_file = data_dir.join("asset-mempool-batch.json");
        let batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: batch_file.clone(),
            max_transactions: 2,
        })
        .expect("create asset mempool batch");
        assert_eq!(batch.transactions.len(), 0);
        assert_eq!(batch.payments_v2.len(), 0);
        assert_eq!(batch.asset_transactions.len(), 2);
        assert!(mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after batch")
        .is_empty());

        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply asset mempool batch");
        assert_eq!(receipts.len(), 2);
        assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
        assert_eq!(receipts[0].tx_id, create_entry.tx_id);
        assert_eq!(receipts[1].tx_id, trust_entry.tx_id);
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("asset mempool replay verification");

        let holder_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(2),
            to_height: Some(2),
            limit: Some(10),
        })
        .expect("holder account_tx");
        assert_eq!(holder_history.row_count, 1);
        assert_eq!(holder_history.rows[0].tx_id, trust_entry.tx_id);
        assert_eq!(holder_history.rows[0].transaction_kind, TRUST_SET_TRANSACTION_KIND);
        assert_eq!(holder_history.rows[0].trustline_authorized, Some(false));
        assert_eq!(holder_history.rows[0].trustline_frozen, Some(false));

        let freeze_operation = AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.clone(),
            issuer: faucet_key.address.clone(),
            asset_id: asset_id.clone(),
            limit: 250,
            authorized: true,
            frozen: true,
            reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
        });
        let freeze_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&freeze_operation).expect("freeze op json"),
            sequence: None,
        })
        .expect("freeze fee quote");
        assert_eq!(freeze_quote.transaction_kind, TRUST_SET_TRANSACTION_KIND);
        assert_eq!(freeze_quote.sequence, 3);
        assert_eq!(freeze_quote.state_expansion_fee, 0);
        let freeze = signed_asset_transaction_for_test(
            &genesis,
            &store.read_ledger().expect("ledger before freeze"),
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            freeze_quote.sequence,
            freeze_operation,
        );
        assert_eq!(freeze.unsigned.fee, freeze_quote.minimum_fee);
        let freeze_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&freeze)
                    .expect("signed freeze json"),
            },
        )
        .expect("submit issuer freeze");
        assert_eq!(freeze_entry.tx_id, asset_transaction_tx_id(&freeze));
        let freeze_batch_file = data_dir.join("asset-freeze-batch.json");
        let freeze_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: freeze_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create freeze batch");
        assert_eq!(freeze_batch.asset_transactions.len(), 1);
        let freeze_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: freeze_batch_file,
            certificate_file: None,
        })
        .expect("apply freeze batch");
        assert_eq!(freeze_receipts.len(), 1);
        assert!(freeze_receipts[0].accepted, "{freeze_receipts:?}");
        let ledger = store.read_ledger().expect("ledger after freeze");
        let frozen_line = ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder frozen line");
        assert!(frozen_line.authorized);
        assert!(frozen_line.frozen);
        let frozen_account_lines = account_lines(AccountLinesOptions {
            data_dir: data_dir.clone(),
            account: holder.clone(),
            issuer: Some(faucet_key.address.clone()),
            asset_id: Some(asset_id.clone()),
            limit: Some(10),
        })
        .expect("account_lines after freeze");
        assert_eq!(frozen_account_lines.lines.len(), 1);
        assert!(frozen_account_lines.lines[0].authorized);
        assert!(frozen_account_lines.lines[0].frozen);
        let freeze_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(3),
            to_height: Some(3),
            limit: Some(10),
        })
        .expect("holder freeze account_tx");
        assert_eq!(freeze_history.row_count, 1);
        assert_eq!(freeze_history.rows[0].tx_id, freeze_entry.tx_id);
        assert_eq!(freeze_history.rows[0].trustline_authorized, Some(true));
        assert_eq!(freeze_history.rows[0].trustline_frozen, Some(true));

        let unfreeze_operation = AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.clone(),
            issuer: faucet_key.address.clone(),
            asset_id: asset_id.clone(),
            limit: 250,
            authorized: true,
            frozen: false,
            reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
        });
        let unfreeze_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&unfreeze_operation).expect("unfreeze op json"),
            sequence: None,
        })
        .expect("unfreeze fee quote");
        assert_eq!(unfreeze_quote.sequence, 4);
        assert_eq!(unfreeze_quote.state_expansion_fee, 0);
        let unfreeze = signed_asset_transaction_for_test(
            &genesis,
            &store.read_ledger().expect("ledger before unfreeze"),
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            unfreeze_quote.sequence,
            unfreeze_operation,
        );
        let unfreeze_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&unfreeze)
                    .expect("signed unfreeze json"),
            },
        )
        .expect("submit issuer unfreeze");
        assert_eq!(unfreeze_entry.tx_id, asset_transaction_tx_id(&unfreeze));
        let unfreeze_batch_file = data_dir.join("asset-unfreeze-batch.json");
        let unfreeze_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: unfreeze_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create unfreeze batch");
        assert_eq!(unfreeze_batch.asset_transactions.len(), 1);
        let unfreeze_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: unfreeze_batch_file,
            certificate_file: None,
        })
        .expect("apply unfreeze batch");
        assert_eq!(unfreeze_receipts.len(), 1);
        assert!(unfreeze_receipts[0].accepted, "{unfreeze_receipts:?}");
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("asset issuer controls replay verification");
        let ledger = store.read_ledger().expect("ledger after unfreeze");
        let unfrozen_line = ledger
            .trustline_for_account_asset(&holder, &asset_id)
            .expect("holder unfrozen line");
        assert!(unfrozen_line.authorized);
        assert!(!unfrozen_line.frozen);
        let unfreeze_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(4),
            to_height: Some(4),
            limit: Some(10),
        })
        .expect("holder unfreeze account_tx");
        assert_eq!(unfreeze_history.row_count, 1);
        assert_eq!(unfreeze_history.rows[0].tx_id, unfreeze_entry.tx_id);
        assert_eq!(unfreeze_history.rows[0].trustline_authorized, Some(true));
        assert_eq!(unfreeze_history.rows[0].trustline_frozen, Some(false));

        let issue_operation = AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: faucet_key.address.clone(),
            to: holder.clone(),
            issuer: faucet_key.address.clone(),
            asset_id: asset_id.clone(),
            amount: 60,
        });
        let issue_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&issue_operation).expect("issue op json"),
            sequence: None,
        })
        .expect("issue fee quote");
        assert_eq!(issue_quote.transaction_kind, ISSUED_PAYMENT_TRANSACTION_KIND);
        assert_eq!(issue_quote.sequence, 5);
        assert_eq!(issue_quote.state_expansion_fee, 0);
        let issue = signed_asset_transaction_for_test(
            &genesis,
            &store.read_ledger().expect("ledger before issue"),
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            issue_quote.sequence,
            issue_operation,
        );
        let issue_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&issue)
                    .expect("signed issue json"),
            },
        )
        .expect("submit issued payment");
        assert_eq!(issue_entry.tx_id, asset_transaction_tx_id(&issue));

        let clawback_operation = AssetTransactionOperation::AssetClawback(
            AssetClawbackOperation {
                owner: holder.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 15,
            },
        );
        let clawback_quote = asset_fee_quote(AssetFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&clawback_operation)
                .expect("clawback op json"),
            sequence: None,
        })
        .expect("clawback fee quote");
        assert_eq!(
            clawback_quote.transaction_kind,
            ASSET_CLAWBACK_TRANSACTION_KIND
        );
        assert_eq!(clawback_quote.sequence, 6);
        assert_eq!(clawback_quote.state_expansion_fee, 0);
        let clawback = signed_asset_transaction_for_test(
            &genesis,
            &store.read_ledger().expect("ledger before clawback quote"),
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CLAWBACK_TRANSACTION_KIND,
            clawback_quote.sequence,
            clawback_operation,
        );
        let clawback_entry = submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&clawback)
                    .expect("signed clawback json"),
            },
        )
        .expect("submit asset clawback");
        assert_eq!(clawback_entry.tx_id, asset_transaction_tx_id(&clawback));
        let clawback_batch_file = data_dir.join("asset-clawback-batch.json");
        let clawback_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: clawback_batch_file.clone(),
            max_transactions: 2,
        })
        .expect("create issue and clawback batch");
        assert_eq!(clawback_batch.asset_transactions.len(), 2);
        let clawback_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: clawback_batch_file,
            certificate_file: None,
        })
        .expect("apply clawback batch");
        assert_eq!(clawback_receipts.len(), 2);
        assert!(
            clawback_receipts.iter().all(|receipt| receipt.accepted),
            "{clawback_receipts:?}"
        );
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("asset clawback replay verification");
        let ledger = store.read_ledger().expect("ledger after clawback");
        assert_eq!(
            ledger
                .trustline_for_account_asset(&holder, &asset_id)
                .expect("holder line after clawback")
                .balance,
            45
        );
        let metrics = crate::metrics(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("metrics after asset clawback");
        assert_eq!(metrics.assets.asset_count, 1);
        assert_eq!(metrics.assets.trustline_count, 1);
        assert_eq!(metrics.assets.holder_count, 1);
        assert_eq!(metrics.assets.total_outstanding_supply, 45);
        assert_eq!(metrics.assets.freeze_enabled_asset_count, 1);
        assert_eq!(metrics.assets.clawback_enabled_asset_count, 1);
        let clawback_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: holder.clone(),
            from_height: Some(5),
            to_height: Some(5),
            limit: Some(10),
        })
        .expect("holder clawback account_tx");
        assert_eq!(clawback_history.row_count, 2);
        assert_eq!(clawback_history.rows[0].tx_id, issue_entry.tx_id);
        assert_eq!(
            clawback_history.rows[0].transaction_kind,
            ISSUED_PAYMENT_TRANSACTION_KIND
        );
        assert_eq!(clawback_history.rows[0].amount, 60);
        assert_eq!(clawback_history.rows[1].tx_id, clawback_entry.tx_id);
        assert_eq!(
            clawback_history.rows[1].transaction_kind,
            ASSET_CLAWBACK_TRANSACTION_KIND
        );
        assert_eq!(clawback_history.rows[1].from_address, holder);
        assert_eq!(clawback_history.rows[1].to_address, faucet_key.address);
        assert_eq!(clawback_history.rows[1].amount, 15);
        assert_eq!(clawback_history.rows[1].asset_id.as_deref(), Some(asset_id.as_str()));

        fs::remove_dir_all(data_dir).expect("cleanup asset mempool test");
    }

    #[test]
    fn nft_fee_quote_mempool_batch_replay_and_account_tx_flow() {
        let data_dir = unique_test_dir("postfiat-nft-mempool-test");
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
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let owner_public_key_hex = bytes_to_hex(&owner_key.public_key);
        let owner_private_key_hex = bytes_to_hex(&owner_key.private_key);
        let recipient_public_key_hex = bytes_to_hex(&recipient_key.public_key);
        let recipient_private_key_hex = bytes_to_hex(&recipient_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_owner = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            owner.clone(),
            ACCOUNT_RESERVE + 500,
            1,
        )
        .expect("fund owner transfer");
        let fund_recipient = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            recipient.clone(),
            ACCOUNT_RESERVE + 500,
            2,
        )
        .expect("fund recipient transfer");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![fund_owner, fund_recipient],
        )
        .expect("funding batch")
        .batch;
        let funding_batch_file = data_dir.join("fund-nft-accounts-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(
            funding_receipts.iter().all(|receipt| receipt.accepted),
            "{funding_receipts:?}"
        );

        let mint_operation = NftTransactionOperation::NftMint(NftMintOperation {
            issuer: faucet_key.address.clone(),
            collection_id: "NFT-003".to_string(),
            serial: 1,
            owner: owner.clone(),
            metadata_hash: "ab".repeat(32),
            metadata_uri: "ipfs://postfiat-nft-003".to_string(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: 0,
            issuer_transfer_fee: 7,
        });
        let mint_quote = nft_fee_quote(NftFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&mint_operation).expect("mint op json"),
            sequence: None,
        })
        .expect("nft mint fee quote");
        assert_eq!(mint_quote.transaction_kind, NFT_MINT_TRANSACTION_KIND);
        assert_eq!(mint_quote.sequence, 3);
        assert_eq!(mint_quote.issuer_transfer_fee, 0);
        assert!(mint_quote.issuer_transfer_fee_recipient.is_none());
        assert!(
            mint_quote.state_expansion_fee >= postfiat_execution::NFT_STATE_EXPANSION_FEE
        );
        let ledger = store.read_ledger().expect("funded ledger");
        let mint = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            NFT_MINT_TRANSACTION_KIND,
            mint_quote.sequence,
            mint_operation.clone(),
        );
        assert_eq!(mint.unsigned.fee, mint_quote.minimum_fee);
        let mint_entry = submit_signed_nft_transaction_json_to_mempool(
            SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&mint).expect("mint json"),
            },
        )
        .expect("submit nft mint");
        assert_eq!(mint_entry.tx_id, nft_transaction_tx_id(&mint));
        assert!(
            submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&mint)
                    .expect("duplicate mint json"),
            })
            .is_err(),
            "duplicate pending NFT transaction should be rejected"
        );

        let mempool_report = verify_mempool(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify nft mint mempool");
        assert_eq!(mempool_report.pending_count, 1);
        assert_eq!(mempool_report.latest_tx_id, mint_entry.tx_id);
        assert_eq!(mempool_report.total_fee, mint.unsigned.fee);

        let mint_batch_file = data_dir.join("nft-mint-mempool-batch.json");
        let mint_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: mint_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create nft mint batch");
        assert_eq!(mint_batch.nft_transactions.len(), 1);
        assert!(mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after mint batch")
        .is_empty());
        let mint_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: mint_batch_file,
            certificate_file: None,
        })
        .expect("apply nft mint batch");
        assert_eq!(mint_receipts.len(), 1);
        assert!(mint_receipts[0].accepted, "{mint_receipts:?}");
        assert_eq!(mint_receipts[0].tx_id, mint_entry.tx_id);
        let nft_id = postfiat_types::nft_id(&genesis.chain_id, &faucet_key.address, "NFT-003", 1)
            .expect("nft id");
        let ledger = store.read_ledger().expect("ledger after mint");
        assert_eq!(ledger.nft(&nft_id).expect("minted nft").owner, owner);
        assert_eq!(
            ledger.nft(&nft_id).expect("minted nft").issuer_transfer_fee,
            7
        );
        let indexes = ledger.nft_indexes(&genesis.chain_id).expect("nft indexes");
        assert_eq!(indexes.by_owner.get(&owner), Some(&vec![nft_id.clone()]));
        assert_eq!(
            indexes.by_issuer.get(&faucet_key.address),
            Some(&vec![nft_id.clone()])
        );
        let nft_info_report = nft_info(NftInfoOptions {
            data_dir: data_dir.clone(),
            nft_id: nft_id.clone(),
        })
        .expect("nft_info after mint");
        assert_eq!(nft_info_report.schema, "postfiat-nft-info-v1");
        assert!(nft_info_report.found);
        let nft_report = nft_info_report.nft.expect("minted nft report");
        assert_eq!(nft_report.nft_id, nft_id);
        assert_eq!(nft_report.owner, owner);
        assert_eq!(nft_report.issuer, faucet_key.address);
        assert_eq!(nft_report.collection_id, "NFT-003");
        assert_eq!(nft_report.serial, 1);
        assert_eq!(nft_report.issuer_transfer_fee, 7);
        assert_eq!(nft_report.collection_flags, 0);
        assert!(nft_report.transferable);
        assert!(!nft_report.collection_transfer_locked);
        assert!(!nft_report.collection_burn_locked);
        assert!(!nft_report.burned);
        let owner_nfts = account_nfts(AccountNftsOptions {
            data_dir: data_dir.clone(),
            account: owner.clone(),
            include_burned: false,
            limit: Some(10),
        })
        .expect("owner account_nfts after mint");
        assert_eq!(owner_nfts.schema, "postfiat-account-nfts-v1");
        assert_eq!(owner_nfts.nft_count, 1);
        assert_eq!(owner_nfts.nfts[0].nft_id, nft_id);
        let issuer_nfts_report = issuer_nfts(IssuerNftsOptions {
            data_dir: data_dir.clone(),
            issuer: faucet_key.address.clone(),
            collection_id: Some("NFT-003".to_string()),
            include_burned: false,
            limit: Some(10),
        })
        .expect("issuer_nfts after mint");
        assert_eq!(issuer_nfts_report.schema, "postfiat-issuer-nfts-v1");
        assert_eq!(issuer_nfts_report.nft_count, 1);
        assert_eq!(issuer_nfts_report.nfts[0].nft_id, nft_id);
        assert_eq!(issuer_nfts_report.nfts[0].issuer_transfer_fee, 7);

        let owner_mint_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: owner.clone(),
            from_height: Some(2),
            to_height: Some(2),
            limit: Some(10),
        })
        .expect("owner nft mint account_tx");
        assert_eq!(owner_mint_history.row_count, 1);
        assert_eq!(
            owner_mint_history.rows[0].transaction_kind,
            NFT_MINT_TRANSACTION_KIND
        );
        assert_eq!(owner_mint_history.rows[0].nft_id.as_deref(), Some(nft_id.as_str()));
        assert_eq!(
            owner_mint_history.rows[0].issuer.as_deref(),
            Some(faucet_key.address.as_str())
        );

        let unauthorized_transfer = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            NFT_TRANSFER_TRANSACTION_KIND,
            1,
            NftTransactionOperation::NftTransfer(NftTransferOperation {
                nft_id: nft_id.clone(),
                from: recipient.clone(),
                to: owner.clone(),
                issuer: String::new(),
                issuer_transfer_fee: 0,
            }),
        );
        assert!(
            submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&unauthorized_transfer)
                    .expect("unauthorized transfer json"),
            })
            .is_err(),
            "non-owner transfer must be rejected at mempool admission"
        );

        let transfer_operation = NftTransactionOperation::NftTransfer(NftTransferOperation {
            nft_id: nft_id.clone(),
            from: owner.clone(),
            to: recipient.clone(),
            issuer: String::new(),
            issuer_transfer_fee: 0,
        });
        let transfer_quote = nft_fee_quote(NftFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: owner.clone(),
            operation_json: serde_json::to_string(&transfer_operation).expect("transfer op json"),
            sequence: None,
        })
        .expect("nft transfer fee quote");
        assert_eq!(transfer_quote.transaction_kind, NFT_TRANSFER_TRANSACTION_KIND);
        assert_eq!(transfer_quote.sequence, 1);
        assert_eq!(transfer_quote.state_expansion_fee, 0);
        assert_eq!(transfer_quote.issuer_transfer_fee, 7);
        assert_eq!(
            transfer_quote.issuer_transfer_fee_recipient.as_deref(),
            Some(faucet_key.address.as_str())
        );
        let NftTransactionOperation::NftTransfer(quoted_transfer_operation) =
            &transfer_quote.operation
        else {
            panic!("expected nft transfer quote operation");
        };
        assert_eq!(quoted_transfer_operation.issuer, faucet_key.address);
        assert_eq!(quoted_transfer_operation.issuer_transfer_fee, 7);
        let owner_balance_before_transfer = ledger.account(&owner).expect("owner").balance;
        let issuer_balance_before_transfer =
            ledger.account(&faucet_key.address).expect("issuer").balance;
        let transfer = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &owner,
            &owner_public_key_hex,
            &owner_private_key_hex,
            NFT_TRANSFER_TRANSACTION_KIND,
            transfer_quote.sequence,
            transfer_quote.operation.clone(),
        );
        let transfer_entry = submit_signed_nft_transaction_json_to_mempool(
            SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&transfer)
                    .expect("transfer json"),
            },
        )
        .expect("submit nft transfer");
        let transfer_batch_file = data_dir.join("nft-transfer-mempool-batch.json");
        let transfer_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: transfer_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create nft transfer batch");
        assert_eq!(transfer_batch.nft_transactions.len(), 1);
        let transfer_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: transfer_batch_file,
            certificate_file: None,
        })
        .expect("apply nft transfer batch");
        assert_eq!(transfer_receipts[0].tx_id, transfer_entry.tx_id);
        assert!(transfer_receipts[0].accepted, "{transfer_receipts:?}");
        assert_eq!(transfer_receipts[0].nft_issuer_transfer_fee, 7);
        assert_eq!(
            transfer_receipts[0]
                .nft_issuer_transfer_fee_recipient
                .as_deref(),
            Some(faucet_key.address.as_str())
        );
        let ledger = store.read_ledger().expect("ledger after transfer");
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            owner_balance_before_transfer - transfer.unsigned.fee - 7
        );
        assert_eq!(
            ledger.account(&faucet_key.address).expect("issuer").balance,
            issuer_balance_before_transfer + 7
        );
        assert_eq!(
            ledger.nft(&nft_id).expect("transferred nft").owner,
            recipient
        );
        let owner_nfts_after_transfer = account_nfts(AccountNftsOptions {
            data_dir: data_dir.clone(),
            account: owner.clone(),
            include_burned: false,
            limit: Some(10),
        })
        .expect("owner account_nfts after transfer");
        assert_eq!(owner_nfts_after_transfer.nft_count, 0);
        let recipient_nfts_after_transfer = account_nfts(AccountNftsOptions {
            data_dir: data_dir.clone(),
            account: recipient.clone(),
            include_burned: false,
            limit: Some(10),
        })
        .expect("recipient account_nfts after transfer");
        assert_eq!(recipient_nfts_after_transfer.nft_count, 1);
        assert_eq!(recipient_nfts_after_transfer.nfts[0].nft_id, nft_id);

        let recipient_transfer_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: recipient.clone(),
            from_height: Some(3),
            to_height: Some(3),
            limit: Some(10),
        })
        .expect("recipient nft transfer account_tx");
        assert_eq!(recipient_transfer_history.row_count, 1);
        assert_eq!(
            recipient_transfer_history.rows[0].transaction_kind,
            NFT_TRANSFER_TRANSACTION_KIND
        );
        assert_eq!(
            recipient_transfer_history.rows[0].nft_id.as_deref(),
            Some(nft_id.as_str())
        );
        assert_eq!(
            recipient_transfer_history.rows[0].issuer.as_deref(),
            Some(faucet_key.address.as_str())
        );
        assert_eq!(
            recipient_transfer_history.rows[0].nft_issuer_transfer_fee,
            Some(7)
        );

        let burn_operation = NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: nft_id.clone(),
            owner: recipient.clone(),
        });
        let burn_quote = nft_fee_quote(NftFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: recipient.clone(),
            operation_json: serde_json::to_string(&burn_operation).expect("burn op json"),
            sequence: None,
        })
        .expect("nft burn fee quote");
        assert_eq!(burn_quote.transaction_kind, NFT_BURN_TRANSACTION_KIND);
        assert_eq!(burn_quote.sequence, 1);
        let burn = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &recipient,
            &recipient_public_key_hex,
            &recipient_private_key_hex,
            NFT_BURN_TRANSACTION_KIND,
            burn_quote.sequence,
            burn_operation,
        );
        let burn_entry = submit_signed_nft_transaction_json_to_mempool(
            SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&burn).expect("burn json"),
            },
        )
        .expect("submit nft burn");
        let burn_batch_file = data_dir.join("nft-burn-mempool-batch.json");
        let burn_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: burn_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create nft burn batch");
        assert_eq!(burn_batch.nft_transactions.len(), 1);
        let burn_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: burn_batch_file,
            certificate_file: None,
        })
        .expect("apply nft burn batch");
        assert_eq!(burn_receipts[0].tx_id, burn_entry.tx_id);
        assert!(burn_receipts[0].accepted, "{burn_receipts:?}");
        let burn_finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: burn_entry.tx_id.clone(),
            audit_block_log: true,
        })
        .expect("nft burn finality");
        assert!(burn_finality.confirmed);
        let ledger = store.read_ledger().expect("ledger after burn");
        let burned = ledger.nft(&nft_id).expect("burned nft");
        assert!(burned.burned);
        assert_eq!(burned.owner, recipient);
        let indexes = ledger.nft_indexes(&genesis.chain_id).expect("nft indexes after burn");
        assert!(!indexes
            .by_owner
            .get(&recipient)
            .is_some_and(|ids| ids.contains(&nft_id)));
        assert_eq!(
            indexes.by_issuer.get(&faucet_key.address),
            Some(&vec![nft_id.clone()])
        );
        let recipient_live_nfts_after_burn = account_nfts(AccountNftsOptions {
            data_dir: data_dir.clone(),
            account: recipient.clone(),
            include_burned: false,
            limit: Some(10),
        })
        .expect("recipient live account_nfts after burn");
        assert_eq!(recipient_live_nfts_after_burn.nft_count, 0);
        let recipient_all_nfts_after_burn = account_nfts(AccountNftsOptions {
            data_dir: data_dir.clone(),
            account: recipient.clone(),
            include_burned: true,
            limit: Some(10),
        })
        .expect("recipient burned account_nfts after burn");
        assert_eq!(recipient_all_nfts_after_burn.nft_count, 1);
        assert!(recipient_all_nfts_after_burn.nfts[0].burned);
        let issuer_live_nfts_after_burn = issuer_nfts(IssuerNftsOptions {
            data_dir: data_dir.clone(),
            issuer: faucet_key.address.clone(),
            collection_id: Some("NFT-003".to_string()),
            include_burned: false,
            limit: Some(10),
        })
        .expect("issuer live nfts after burn");
        assert_eq!(issuer_live_nfts_after_burn.nft_count, 0);
        let issuer_all_nfts_after_burn = issuer_nfts(IssuerNftsOptions {
            data_dir: data_dir.clone(),
            issuer: faucet_key.address.clone(),
            collection_id: Some("NFT-003".to_string()),
            include_burned: true,
            limit: Some(10),
        })
        .expect("issuer burned nfts after burn");
        assert_eq!(issuer_all_nfts_after_burn.nft_count, 1);
        assert_eq!(issuer_all_nfts_after_burn.nfts[0].nft_id, nft_id);
        assert!(issuer_all_nfts_after_burn.nfts[0].burned);

        let recipient_burn_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: recipient.clone(),
            from_height: Some(4),
            to_height: Some(4),
            limit: Some(10),
        })
        .expect("recipient nft burn account_tx");
        assert_eq!(recipient_burn_history.row_count, 1);
        assert_eq!(
            recipient_burn_history.rows[0].transaction_kind,
            NFT_BURN_TRANSACTION_KIND
        );
        assert_eq!(
            recipient_burn_history.rows[0].nft_id.as_deref(),
            Some(nft_id.as_str())
        );

        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("nft replay verification");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);

        fs::remove_dir_all(data_dir).expect("cleanup nft mempool test");
    }

    #[test]
    fn nft_collection_policy_flags_flow_through_rpc_account_tx_and_mempool() {
        let data_dir = unique_test_dir("postfiat-nft-collection-policy-test");
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
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
        let owner = address_from_public_key(&owner_key.public_key);
        let recipient = address_from_public_key(&recipient_key.public_key);
        let owner_public_key_hex = bytes_to_hex(&owner_key.public_key);
        let owner_private_key_hex = bytes_to_hex(&owner_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_owner = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            owner.clone(),
            ACCOUNT_RESERVE + 500,
            1,
        )
        .expect("fund owner transfer");
        let fund_recipient = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            recipient.clone(),
            ACCOUNT_RESERVE + 500,
            2,
        )
        .expect("fund recipient transfer");
        let funding_batch = build_transaction_batch(
            &mempool_batch_domain(&genesis),
            vec![fund_owner, fund_recipient],
        )
        .expect("funding batch")
        .batch;
        let funding_batch_file = data_dir.join("fund-nft-policy-accounts-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(
            funding_receipts.iter().all(|receipt| receipt.accepted),
            "{funding_receipts:?}"
        );

        let collection_flags =
            NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED;
        let mint_operation = NftTransactionOperation::NftMint(NftMintOperation {
            issuer: faucet_key.address.clone(),
            collection_id: "NFT-POLICY".to_string(),
            serial: 1,
            owner: owner.clone(),
            metadata_hash: "ab".repeat(32),
            metadata_uri: String::new(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags,
            issuer_transfer_fee: 0,
        });
        let mint_quote = nft_fee_quote(NftFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: faucet_key.address.clone(),
            operation_json: serde_json::to_string(&mint_operation).expect("mint op json"),
            sequence: None,
        })
        .expect("nft policy mint fee quote");
        let ledger = store.read_ledger().expect("funded ledger");
        let mint = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            NFT_MINT_TRANSACTION_KIND,
            mint_quote.sequence,
            mint_operation,
        );
        submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_nft_transaction_json: serde_json::to_string(&mint).expect("mint json"),
        })
        .expect("submit nft policy mint");
        let mint_batch_file = data_dir.join("nft-policy-mint-batch.json");
        create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: mint_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create nft policy mint batch");
        let mint_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: mint_batch_file,
            certificate_file: None,
        })
        .expect("apply nft policy mint batch");
        assert_eq!(mint_receipts.len(), 1);
        assert!(mint_receipts[0].accepted, "{mint_receipts:?}");
        assert_eq!(mint_receipts[0].nft_collection_flags, collection_flags);

        let nft_id =
            postfiat_types::nft_id(&genesis.chain_id, &faucet_key.address, "NFT-POLICY", 1)
                .expect("nft id");
        let nft_info_report = nft_info(NftInfoOptions {
            data_dir: data_dir.clone(),
            nft_id: nft_id.clone(),
        })
        .expect("nft policy info");
        let nft_report = nft_info_report.nft.expect("nft policy report");
        assert_eq!(nft_report.collection_flags, collection_flags);
        assert!(nft_report.collection_transfer_locked);
        assert!(nft_report.collection_burn_locked);

        let owner_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: owner.clone(),
            from_height: Some(2),
            to_height: Some(2),
            limit: Some(10),
        })
        .expect("owner nft policy account_tx");
        assert_eq!(owner_history.row_count, 1);
        assert_eq!(
            owner_history.rows[0].transaction_kind,
            NFT_MINT_TRANSACTION_KIND
        );
        assert_eq!(owner_history.rows[0].nft_id.as_deref(), Some(nft_id.as_str()));
        assert_eq!(
            owner_history.rows[0].nft_collection_flags,
            Some(collection_flags)
        );

        let transfer_operation = NftTransactionOperation::NftTransfer(NftTransferOperation {
            nft_id: nft_id.clone(),
            from: owner.clone(),
            to: recipient,
            issuer: String::new(),
            issuer_transfer_fee: 0,
        });
        assert!(
            nft_fee_quote(NftFeeQuoteOptions {
                data_dir: data_dir.clone(),
                source: owner.clone(),
                operation_json: serde_json::to_string(&transfer_operation)
                    .expect("transfer op json"),
                sequence: None,
            })
            .is_err(),
            "collection transfer lock must reject NFT fee quotes"
        );
        let ledger = store.read_ledger().expect("ledger after policy mint");
        let transfer = signed_nft_transaction_for_test(
            &genesis,
            &ledger,
            &owner,
            &owner_public_key_hex,
            &owner_private_key_hex,
            NFT_TRANSFER_TRANSACTION_KIND,
            1,
            transfer_operation,
        );
        assert!(
            submit_signed_nft_transaction_json_to_mempool(SignedNftTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_nft_transaction_json: serde_json::to_string(&transfer)
                    .expect("transfer json"),
            })
            .is_err(),
            "collection transfer lock must reject mempool admission"
        );

        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("nft policy replay verification");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 2);

        fs::remove_dir_all(data_dir).expect("cleanup nft policy test");
    }

    #[test]
    fn offer_fee_quote_mempool_batch_replay_and_account_tx_flow() {
        let data_dir = unique_test_dir("postfiat-offer-mempool-test");
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
        let owner_key = ml_dsa_65_keygen().expect("owner keygen");
        let owner = address_from_public_key(&owner_key.public_key);
        let owner_public_key_hex = bytes_to_hex(&owner_key.public_key);
        let owner_private_key_hex = bytes_to_hex(&owner_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_owner = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            owner.clone(),
            ACCOUNT_RESERVE + 700,
            1,
        )
        .expect("fund offer owner transfer");
        let funding_batch =
            build_transaction_batch(&mempool_batch_domain(&genesis), vec![fund_owner])
                .expect("funding batch")
                .batch;
        let funding_batch_file = data_dir.join("fund-offer-owner-batch.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding batch");
        assert!(funding_receipts[0].accepted, "{funding_receipts:?}");

        let ledger = store.read_ledger().expect("funded ledger");
        let create_asset = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            2,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "DEX".to_string(),
                version: 1,
                precision: 2,
                display_name: "DEX Test".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create_asset, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();
        let trust_set = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &owner,
            &owner_public_key_hex,
            &owner_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: owner.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 250,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        let asset_setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![create_asset, trust_set],
        )
        .expect("offer asset setup batch")
        .batch;
        let asset_setup_batch_file = data_dir.join("offer-asset-setup-batch.json");
        write_batch_file(&asset_setup_batch_file, &asset_setup_batch)
            .expect("write offer asset setup batch");
        let setup_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: asset_setup_batch_file,
            certificate_file: None,
        })
        .expect("apply offer asset setup");
        assert!(
            setup_receipts.iter().all(|receipt| receipt.accepted),
            "{setup_receipts:?}"
        );

        let ledger = store.read_ledger().expect("ledger after offer asset setup");
        let owner_balance_before_offer = ledger.account(&owner).expect("owner").balance;
        let native_pft_before_offer = native_pft_account_offer_total_for_test(&ledger);
        let create_operation = OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: owner.clone(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_gets_amount: 60,
            taker_pays_asset_id: asset_id.clone(),
            taker_pays_amount: 30,
            expiration_height: 10,
        });
        let create_quote = offer_fee_quote(OfferFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: owner.clone(),
            operation_json: serde_json::to_string(&create_operation).expect("offer op json"),
            sequence: None,
        })
        .expect("offer create fee quote");
        assert_eq!(create_quote.transaction_kind, OFFER_CREATE_TRANSACTION_KIND);
        assert_eq!(create_quote.sequence, 2);
        assert!(create_quote.state_expansion_fee >= postfiat_execution::OFFER_STATE_EXPANSION_FEE);
        assert_eq!(create_quote.offer_object_reserve, OFFER_OBJECT_RESERVE);
        assert!(create_quote.sender_meets_reserve_after_fee_and_reserve);
        let create_offer = signed_offer_transaction_for_test(
            &genesis,
            &ledger,
            &owner,
            &owner_public_key_hex,
            &owner_private_key_hex,
            OFFER_CREATE_TRANSACTION_KIND,
            create_quote.sequence,
            create_operation.clone(),
        );
        assert_eq!(create_offer.unsigned.fee, create_quote.minimum_fee);
        let create_entry = submit_signed_offer_transaction_json_to_mempool(
            SignedOfferTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_offer_transaction_json: serde_json::to_string(&create_offer)
                    .expect("signed offer create json"),
            },
        )
        .expect("submit offer create");
        assert_eq!(create_entry.tx_id, offer_transaction_tx_id(&create_offer));
        assert!(
            submit_signed_offer_transaction_json_to_mempool(
                SignedOfferTransactionJsonSubmitOptions {
                    data_dir: data_dir.clone(),
                    signed_offer_transaction_json: serde_json::to_string(&create_offer)
                        .expect("duplicate offer create json"),
                },
            )
            .is_err(),
            "duplicate pending offer transaction should be rejected"
        );

        let mempool = mempool_state(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("offer mempool state");
        assert_eq!(mempool.pending_offer_transactions.len(), 1);
        let mempool_report = verify_mempool(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify offer mempool");
        assert!(mempool_report.verified);
        assert_eq!(mempool_report.pending_count, 1);
        assert_eq!(mempool_report.latest_tx_id, create_entry.tx_id);
        assert_eq!(mempool_report.total_fee, create_offer.unsigned.fee);

        let create_batch_file = data_dir.join("offer-create-mempool-batch.json");
        let create_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: create_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create offer mempool batch");
        assert_eq!(create_batch.offer_transactions.len(), 1);
        assert!(mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after offer create batch")
        .is_empty());
        let create_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: create_batch_file,
            certificate_file: None,
        })
        .expect("apply offer create");
        assert_eq!(create_receipts.len(), 1);
        assert!(create_receipts[0].accepted, "{create_receipts:?}");
        assert_eq!(create_receipts[0].tx_id, create_entry.tx_id);

        let offer_id =
            postfiat_types::offer_id(&genesis.chain_id, &owner, create_quote.sequence)
                .expect("offer id");
        let ledger = store.read_ledger().expect("ledger after offer create");
        let offer = ledger.offer(&offer_id).expect("created offer");
        assert_eq!(offer.state, OFFER_STATE_OPEN);
        assert_eq!(offer.reserve_paid, OFFER_OBJECT_RESERVE);
        assert_eq!(offer.taker_gets_amount_remaining, 60);
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            owner_balance_before_offer - create_offer.unsigned.fee - OFFER_OBJECT_RESERVE - 60
        );
        assert_eq!(
            native_pft_before_offer - create_offer.unsigned.fee,
            native_pft_account_offer_total_for_test(&ledger)
        );
        let offer_info_report = offer_info(OfferInfoOptions {
            data_dir: data_dir.clone(),
            offer_id: offer_id.clone(),
        })
        .expect("offer info after create");
        assert!(offer_info_report.found);
        let offer_report = offer_info_report.offer.expect("created offer report");
        assert_eq!(offer_report.offer_id, offer_id);
        assert_eq!(offer_report.owner, owner);
        assert_eq!(offer_report.state, OFFER_STATE_OPEN);
        assert_eq!(offer_report.taker_gets_amount_remaining, 60);
        assert_eq!(offer_report.taker_pays_amount_remaining, 30);
        let owner_open_offers = account_offers(AccountOffersOptions {
            data_dir: data_dir.clone(),
            account: owner.clone(),
            state: Some(OFFER_STATE_OPEN.to_string()),
            limit: Some(10),
        })
        .expect("owner open offers after create");
        assert_eq!(owner_open_offers.offer_count, 1);
        assert_eq!(owner_open_offers.offers[0].offer_id, offer_id);
        let book_open_offers = book_offers(BookOffersOptions {
            data_dir: data_dir.clone(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_pays_asset_id: asset_id.clone(),
            limit: Some(10),
        })
        .expect("book open offers after create");
        assert_eq!(book_open_offers.offer_count, 1);
        assert_eq!(book_open_offers.offers[0].offer_id, offer_id);
        rebuild_account_tx_index(AccountTxIndexOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuild offer create account_tx index");
        let create_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: owner.clone(),
            from_height: Some(3),
            to_height: Some(3),
            limit: Some(10),
        })
        .expect("offer create account_tx");
        assert!(create_history.index_used);
        assert_eq!(create_history.row_count, 1);
        assert_eq!(
            create_history.rows[0].transaction_kind,
            OFFER_CREATE_TRANSACTION_KIND
        );
        assert_eq!(create_history.rows[0].offer_id.as_deref(), Some(offer_id.as_str()));
        assert_eq!(create_history.rows[0].asset_id.as_deref(), Some(asset_id.as_str()));
        assert_eq!(create_history.rows[0].amount, 60);
        assert_eq!(create_history.rows[0].accepted, Some(true));
        let create_finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: create_entry.tx_id.clone(),
            audit_block_log: true,
        })
        .expect("offer create finality");
        assert!(create_finality.confirmed);
        assert_eq!(create_finality.block_count, 3);

        let cancel_operation = OfferTransactionOperation::OfferCancel(OfferCancelOperation {
            offer_id: offer_id.clone(),
            owner: owner.clone(),
        });
        let cancel_quote = offer_fee_quote(OfferFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: owner.clone(),
            operation_json: serde_json::to_string(&cancel_operation).expect("offer cancel op json"),
            sequence: None,
        })
        .expect("offer cancel fee quote");
        assert_eq!(cancel_quote.transaction_kind, OFFER_CANCEL_TRANSACTION_KIND);
        assert_eq!(cancel_quote.sequence, 3);
        assert_eq!(cancel_quote.state_expansion_fee, 0);
        assert_eq!(cancel_quote.offer_object_reserve, 0);
        let cancel_offer = signed_offer_transaction_for_test(
            &genesis,
            &ledger,
            &owner,
            &owner_public_key_hex,
            &owner_private_key_hex,
            OFFER_CANCEL_TRANSACTION_KIND,
            cancel_quote.sequence,
            cancel_operation,
        );
        assert_eq!(cancel_offer.unsigned.fee, cancel_quote.minimum_fee);
        let cancel_entry = submit_signed_offer_transaction_json_to_mempool(
            SignedOfferTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_offer_transaction_json: serde_json::to_string(&cancel_offer)
                    .expect("signed offer cancel json"),
            },
        )
        .expect("submit offer cancel");
        assert_eq!(cancel_entry.tx_id, offer_transaction_tx_id(&cancel_offer));
        let cancel_batch_file = data_dir.join("offer-cancel-mempool-batch.json");
        let cancel_batch = create_mempool_batch(MempoolBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: cancel_batch_file.clone(),
            max_transactions: 1,
        })
        .expect("create offer cancel batch");
        assert_eq!(cancel_batch.offer_transactions.len(), 1);
        let cancel_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: cancel_batch_file,
            certificate_file: None,
        })
        .expect("apply offer cancel");
        assert_eq!(cancel_receipts.len(), 1);
        assert!(cancel_receipts[0].accepted, "{cancel_receipts:?}");
        assert_eq!(cancel_receipts[0].tx_id, cancel_entry.tx_id);

        let ledger = store.read_ledger().expect("ledger after offer cancel");
        let offer = ledger.offer(&offer_id).expect("canceled offer");
        assert_eq!(offer.state, OFFER_STATE_CANCELED);
        assert_eq!(offer.reserve_paid, 0);
        assert_eq!(
            ledger.account(&owner).expect("owner").balance,
            owner_balance_before_offer - create_offer.unsigned.fee - cancel_offer.unsigned.fee
        );
        assert_eq!(
            native_pft_before_offer - create_offer.unsigned.fee - cancel_offer.unsigned.fee,
            native_pft_account_offer_total_for_test(&ledger)
        );
        let canceled_offer_info = offer_info(OfferInfoOptions {
            data_dir: data_dir.clone(),
            offer_id: offer_id.clone(),
        })
        .expect("offer info after cancel");
        assert!(canceled_offer_info.found);
        assert_eq!(
            canceled_offer_info
                .offer
                .as_ref()
                .expect("canceled offer report")
                .state,
            OFFER_STATE_CANCELED
        );
        let owner_canceled_offers = account_offers(AccountOffersOptions {
            data_dir: data_dir.clone(),
            account: owner.clone(),
            state: Some(OFFER_STATE_CANCELED.to_string()),
            limit: Some(10),
        })
        .expect("owner canceled offers after cancel");
        assert_eq!(owner_canceled_offers.offer_count, 1);
        assert_eq!(owner_canceled_offers.offers[0].offer_id, offer_id);
        let owner_open_after_cancel = account_offers(AccountOffersOptions {
            data_dir: data_dir.clone(),
            account: owner.clone(),
            state: Some(OFFER_STATE_OPEN.to_string()),
            limit: Some(10),
        })
        .expect("owner open offers after cancel");
        assert_eq!(owner_open_after_cancel.offer_count, 0);
        let book_after_cancel = book_offers(BookOffersOptions {
            data_dir: data_dir.clone(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_pays_asset_id: asset_id.clone(),
            limit: Some(10),
        })
        .expect("book offers after cancel");
        assert_eq!(book_after_cancel.offer_count, 0);
        rebuild_account_tx_index(AccountTxIndexOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuild offer cancel account_tx index");
        let cancel_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: owner.clone(),
            from_height: Some(4),
            to_height: Some(4),
            limit: Some(10),
        })
        .expect("offer cancel account_tx");
        assert!(cancel_history.index_used);
        assert_eq!(cancel_history.row_count, 1);
        assert_eq!(
            cancel_history.rows[0].transaction_kind,
            OFFER_CANCEL_TRANSACTION_KIND
        );
        assert_eq!(cancel_history.rows[0].offer_id.as_deref(), Some(offer_id.as_str()));
        assert_eq!(cancel_history.rows[0].amount, 0);
        assert_eq!(cancel_history.rows[0].accepted, Some(true));
        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("offer replay verification");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);

        fs::remove_dir_all(data_dir).expect("cleanup offer mempool test");
    }

    #[test]
    fn offer_create_matching_replay_and_maker_account_tx_flow() {
        let data_dir = unique_test_dir("postfiat-offer-match-test");
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
        let maker_key = ml_dsa_65_keygen().expect("maker keygen");
        let taker_key = ml_dsa_65_keygen().expect("taker keygen");
        let maker = address_from_public_key(&maker_key.public_key);
        let taker = address_from_public_key(&taker_key.public_key);
        let maker_public_key_hex = bytes_to_hex(&maker_key.public_key);
        let maker_private_key_hex = bytes_to_hex(&maker_key.private_key);
        let taker_public_key_hex = bytes_to_hex(&taker_key.public_key);
        let taker_private_key_hex = bytes_to_hex(&taker_key.private_key);

        let ledger = store.read_ledger().expect("ledger");
        let fund_maker = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            maker.clone(),
            ACCOUNT_RESERVE + 500,
            1,
        )
        .expect("fund maker");
        let fund_taker = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            taker.clone(),
            ACCOUNT_RESERVE + 500,
            2,
        )
        .expect("fund taker");
        let funding_batch =
            build_transaction_batch(&mempool_batch_domain(&genesis), vec![fund_maker, fund_taker])
                .expect("funding batch")
                .batch;
        let funding_batch_file = data_dir.join("fund-offer-match-accounts.json");
        write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
        let funding_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: funding_batch_file,
            certificate_file: None,
        })
        .expect("apply funding");
        assert!(funding_receipts.iter().all(|receipt| receipt.accepted));

        let ledger = store.read_ledger().expect("funded ledger");
        let create_asset = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            3,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "MATCH".to_string(),
                version: 1,
                precision: 2,
                display_name: "DEX Match".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: true,
                clawback_enabled: false,
            }),
        );
        let mut dry_run_ledger = ledger.clone();
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &create_asset, 1).accepted);
        let asset_id = dry_run_ledger.asset_definitions[0].asset_id.clone();
        let maker_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &maker,
            &maker_public_key_hex,
            &maker_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: maker.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 200,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &maker_trust, 1).accepted);
        let taker_trust = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &taker,
            &taker_public_key_hex,
            &taker_private_key_hex,
            TRUST_SET_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::TrustSet(TrustSetOperation {
                account: taker.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                limit: 200,
                authorized: false,
                frozen: false,
                reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
            }),
        );
        assert!(execute_asset_transaction(&genesis, &mut dry_run_ledger, &taker_trust, 1).accepted);
        let issue_to_maker = signed_asset_transaction_for_test(
            &genesis,
            &dry_run_ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            4,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: faucet_key.address.clone(),
                to: maker.clone(),
                issuer: faucet_key.address.clone(),
                asset_id: asset_id.clone(),
                amount: 100,
            }),
        );
        let asset_setup_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            vec![create_asset, maker_trust, taker_trust, issue_to_maker],
        )
        .expect("asset setup batch")
        .batch;
        let asset_setup_batch_file = data_dir.join("offer-match-asset-setup.json");
        write_batch_file(&asset_setup_batch_file, &asset_setup_batch)
            .expect("write asset setup batch");
        let setup_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: asset_setup_batch_file,
            certificate_file: None,
        })
        .expect("apply asset setup");
        assert!(setup_receipts.iter().all(|receipt| receipt.accepted));

        let ledger = store.read_ledger().expect("ledger before maker offer");
        let native_pft_before_offers = native_pft_account_offer_total_for_test(&ledger);
        assert_offer_conservation_for_test(
            &genesis,
            &ledger,
            &asset_id,
            100,
            &[(&maker, 100), (&taker, 0)],
            native_pft_before_offers,
        );
        let maker_offer = signed_offer_transaction_for_test_at_height(
            &genesis,
            &ledger,
            &maker,
            &maker_public_key_hex,
            &maker_private_key_hex,
            OFFER_CREATE_TRANSACTION_KIND,
            2,
            OfferTransactionOperation::OfferCreate(OfferCreateOperation {
                owner: maker.clone(),
                taker_gets_asset_id: asset_id.clone(),
                taker_gets_amount: 40,
                taker_pays_asset_id: "PFT".to_string(),
                taker_pays_amount: 80,
                expiration_height: 20,
            }),
            3,
        );
        let maker_offer_fee = maker_offer.unsigned.fee;
        let maker_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_offers(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![maker_offer.clone()],
        )
        .expect("maker offer batch")
        .batch;
        let maker_batch_file = data_dir.join("maker-offer-batch.json");
        write_batch_file(&maker_batch_file, &maker_batch).expect("write maker batch");
        let maker_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: maker_batch_file,
            certificate_file: None,
        })
        .expect("apply maker offer");
        assert!(maker_receipts[0].accepted, "{maker_receipts:?}");
        let maker_offer_id = postfiat_types::offer_id(&genesis.chain_id, &maker, 2)
            .expect("maker offer id");
        let maker_offer_info = offer_info(OfferInfoOptions {
            data_dir: data_dir.clone(),
            offer_id: maker_offer_id.clone(),
        })
        .expect("maker offer info after create");
        assert!(maker_offer_info.found);
        assert_eq!(
            maker_offer_info
                .offer
                .as_ref()
                .expect("maker offer report")
                .state,
            OFFER_STATE_OPEN
        );
        let maker_book = book_offers(BookOffersOptions {
            data_dir: data_dir.clone(),
            taker_gets_asset_id: asset_id.clone(),
            taker_pays_asset_id: "PFT".to_string(),
            limit: Some(10),
        })
        .expect("maker book after create");
        assert_eq!(maker_book.offer_count, 1);
        assert_eq!(maker_book.offers[0].offer_id, maker_offer_id);

        let asset = asset_info(AssetInfoOptions {
            data_dir: data_dir.clone(),
            asset_id: asset_id.clone(),
        })
        .expect("asset_info with open issued offer")
        .asset
        .expect("asset info row");
        assert_eq!(asset.outstanding_supply, 100);
        assert_eq!(asset.trustline_count, 2);
        assert_eq!(asset.holder_count, 1);
        let metrics = crate::metrics(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("metrics with open issued offer");
        assert_eq!(metrics.assets.asset_count, 1);
        assert_eq!(metrics.assets.trustline_count, 2);
        assert_eq!(metrics.assets.holder_count, 1);
        assert_eq!(metrics.assets.total_outstanding_supply, 100);
        assert_eq!(metrics.assets.open_issued_escrow_count, 0);
        assert_eq!(metrics.assets.open_issued_offer_count, 1);
        assert_eq!(metrics.assets.open_issued_offer_amount, 40);
        assert_eq!(metrics.assets.freeze_enabled_asset_count, 1);

        let ledger = store.read_ledger().expect("ledger before taker offer");
        assert_offer_conservation_for_test(
            &genesis,
            &ledger,
            &asset_id,
            100,
            &[(&maker, 60), (&taker, 0)],
            native_pft_before_offers - maker_offer_fee,
        );
        let taker_operation = OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: taker.clone(),
            taker_gets_asset_id: "PFT".to_string(),
            taker_gets_amount: 80,
            taker_pays_asset_id: asset_id.clone(),
            taker_pays_amount: 40,
            expiration_height: 20,
        });
        let taker_quote = offer_fee_quote(OfferFeeQuoteOptions {
            data_dir: data_dir.clone(),
            source: taker.clone(),
            operation_json: serde_json::to_string(&taker_operation).expect("taker op json"),
            sequence: None,
        })
        .expect("taker offer fee quote");
        assert_eq!(taker_quote.transaction_kind, OFFER_CREATE_TRANSACTION_KIND);
        assert_eq!(taker_quote.sequence, 2);
        assert_eq!(taker_quote.estimated_cross_count, 1);
        assert_eq!(
            taker_quote.max_dex_crosses_per_transaction,
            postfiat_types::MAX_DEX_CROSSES_PER_TRANSACTION as u64
        );
        assert_eq!(taker_quote.match_fee, postfiat_execution::OFFER_MATCH_CROSS_FEE);
        assert_eq!(taker_quote.state_expansion_fee, 0);
        assert!(!taker_quote.will_create_residual_offer);
        assert_eq!(taker_quote.offer_object_reserve, 0);
        assert!(taker_quote.sender_meets_reserve_after_fee_and_reserve);
        let taker_offer = signed_offer_transaction_for_test_at_height(
            &genesis,
            &ledger,
            &taker,
            &taker_public_key_hex,
            &taker_private_key_hex,
            OFFER_CREATE_TRANSACTION_KIND,
            taker_quote.sequence,
            taker_operation,
            4,
        );
        assert_eq!(taker_offer.unsigned.fee, taker_quote.minimum_fee);
        let taker_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_offers(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![taker_offer.clone()],
        )
        .expect("taker offer batch")
        .batch;
        let taker_batch_file = data_dir.join("taker-offer-batch.json");
        write_batch_file(&taker_batch_file, &taker_batch).expect("write taker batch");
        let taker_receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: taker_batch_file,
            certificate_file: None,
        })
        .expect("apply taker offer");
        assert_eq!(taker_receipts.len(), 1);
        assert!(taker_receipts[0].accepted, "{taker_receipts:?}");
        assert_eq!(taker_receipts[0].tx_id, offer_transaction_tx_id(&taker_offer));
        assert_eq!(taker_receipts[0].code, "filled");
        assert_eq!(taker_receipts[0].offer_fills.len(), 1);
        assert_eq!(taker_receipts[0].offer_fills[0].maker_offer_id, maker_offer_id);
        let taker_finality = tx_finality(TxFinalityQueryOptions {
            data_dir: data_dir.clone(),
            tx_id: taker_receipts[0].tx_id.clone(),
            audit_block_log: true,
        })
        .expect("taker match finality");
        assert!(taker_finality.confirmed);
        assert_eq!(taker_finality.receipt.code, "filled");
        assert_eq!(taker_finality.receipt.offer_fills.len(), 1);

        let ledger = store.read_ledger().expect("ledger after match");
        assert_offer_conservation_for_test(
            &genesis,
            &ledger,
            &asset_id,
            100,
            &[(&maker, 60), (&taker, 40)],
            native_pft_before_offers - maker_offer_fee - taker_offer.unsigned.fee,
        );
        assert_eq!(
            ledger.offer(&maker_offer_id).expect("maker offer").state,
            OFFER_STATE_FILLED
        );
        let filled_maker_offer_info = offer_info(OfferInfoOptions {
            data_dir: data_dir.clone(),
            offer_id: maker_offer_id.clone(),
        })
        .expect("maker offer info after fill");
        assert!(filled_maker_offer_info.found);
        let filled_maker_offer = filled_maker_offer_info
            .offer
            .as_ref()
            .expect("filled maker offer report");
        assert_eq!(filled_maker_offer.state, OFFER_STATE_FILLED);
        assert_eq!(filled_maker_offer.taker_gets_amount_remaining, 0);
        assert_eq!(filled_maker_offer.taker_pays_amount_remaining, 0);
        let maker_book_after_fill = book_offers(BookOffersOptions {
            data_dir: data_dir.clone(),
            taker_gets_asset_id: asset_id.clone(),
            taker_pays_asset_id: "PFT".to_string(),
            limit: Some(10),
        })
        .expect("maker book after fill");
        assert_eq!(maker_book_after_fill.offer_count, 0);
        let maker_filled_offers = account_offers(AccountOffersOptions {
            data_dir: data_dir.clone(),
            account: maker.clone(),
            state: Some(OFFER_STATE_FILLED.to_string()),
            limit: Some(10),
        })
        .expect("maker filled offers");
        assert_eq!(maker_filled_offers.offer_count, 1);
        assert_eq!(maker_filled_offers.offers[0].offer_id, maker_offer_id);
        assert!(ledger
            .offer(&postfiat_types::offer_id(&genesis.chain_id, &taker, 2).expect("taker id"))
            .is_none());
        assert_eq!(
            ledger
                .trustline_for_account_asset(&taker, &asset_id)
            .expect("taker trustline")
                .balance,
            40
        );

        rebuild_account_tx_index(AccountTxIndexOptions {
            data_dir: data_dir.clone(),
        })
        .expect("rebuild offer matching account_tx index");
        let maker_history = account_tx(AccountTxQueryOptions {
            data_dir: data_dir.clone(),
            address: maker.clone(),
            from_height: Some(4),
            to_height: Some(4),
            limit: Some(10),
        })
        .expect("maker fill account_tx");
        assert!(maker_history.index_used);
        assert_eq!(maker_history.row_count, 1);
        let maker_row = &maker_history.rows[0];
        assert_eq!(maker_row.tx_role.as_deref(), Some(OFFER_TX_ROLE_MAKER));
        assert_eq!(maker_row.offer_id.as_deref(), Some(maker_offer_id.as_str()));
        assert_eq!(maker_row.amount, 40);
        assert_eq!(maker_row.asset_id.as_deref(), Some(asset_id.as_str()));
        assert_eq!(maker_row.fill_index, Some(0));
        assert_eq!(maker_row.accepted, Some(true));

        let replay = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("offer matching replay");
        assert!(replay.verified);
        assert_eq!(replay.block_count, 4);

        fs::remove_dir_all(data_dir).expect("cleanup offer match test");
    }
    #[test]
    fn mixed_family_admission_uses_the_same_order_as_batch_execution() {
        let data_dir = unique_test_dir("postfiat-mixed-mempool-admission-order-test");
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 1,
        })
        .expect("init");
        let store = NodeStore::new(&data_dir);
        let genesis = store.read_genesis().expect("genesis");
        let ledger = store.read_ledger().expect("ledger");
        let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");

        // Asset transactions execute after transfers in a mixed batch. Persist
        // asset sequence 1 first, then offer transfer sequence 2. Simulating
        // "existing mempool, then candidate" accepts this, while the real
        // batch order runs transfer sequence 2 first and rejects it.
        let create = signed_asset_transaction_for_test(
            &genesis,
            &ledger,
            &faucet_key.address,
            &faucet_key.public_key_hex,
            &faucet_key.private_key_hex,
            ASSET_CREATE_TRANSACTION_KIND,
            1,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: faucet_key.address.clone(),
                code: "ORD".to_string(),
                version: 1,
                precision: 0,
                display_name: "Admission order".to_string(),
                max_supply: Some(1_000),
                requires_authorization: false,
                freeze_enabled: false,
                clawback_enabled: false,
            }),
        );
        submit_signed_asset_transaction_json_to_mempool(
            SignedAssetTransactionJsonSubmitOptions {
                data_dir: data_dir.clone(),
                signed_asset_transaction_json: serde_json::to_string(&create)
                    .expect("asset JSON"),
            },
        )
        .expect("admit pending asset sequence 1");

        let transfer = build_signed_transfer_for_key(
            &genesis,
            &ledger,
            &faucet_key,
            faucet_key.address.clone(),
            1,
            2,
        )
        .expect("build transfer sequence 2");
        let before = store.read_mempool().expect("mempool before candidate");
        let transfer_file = data_dir.join("sequence-2-transfer.json");
        write_signed_transfer_file(&transfer_file, &transfer).expect("write signed transfer");

        let error = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
            data_dir: data_dir.clone(),
            transfer_file,
        })
        .expect_err("candidate invalid in canonical batch order must not be admitted");

        assert!(error.to_string().contains("bad_sequence"), "{error}");
        assert_eq!(
            store.read_mempool().expect("mempool after candidate"),
            before
        );
    }
