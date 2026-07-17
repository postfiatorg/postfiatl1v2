use super::*;

#[test]
fn escrow_create_applies_from_batch_and_replays() {
    let data_dir = unique_test_dir("postfiat-escrow-batch-test");
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
    let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
    let recipient = address_from_public_key(&recipient_key.public_key);
    let ledger = store.read_ledger().expect("ledger");
    let owner_start_balance = ledger
        .account(&faucet_key.address)
        .expect("faucet account")
        .balance;
    let escrow_id =
        postfiat_types::escrow_id(&genesis.chain_id, &faucet_key.address, 1)
            .expect("escrow id");

    let create = signed_escrow_transaction_for_test(
        &genesis,
        &ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CREATE_TRANSACTION_KIND,
        1,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            amount: ACCOUNT_RESERVE + 25,
            condition: "batch-secret".to_string(),
            finish_after: 2,
            cancel_after: 5,
        }),
    );
    let batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![create.clone()],
    )
    .expect("escrow batch")
    .batch;
    let batch_file = data_dir.join("escrow-create-batch.json");
    write_batch_file(&batch_file, &batch).expect("write escrow batch");

    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply escrow batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(
        receipts[0].tx_id,
        postfiat_execution::escrow_transaction_tx_id(&create)
    );

    let ledger = store.read_ledger().expect("ledger after escrow create");
    assert_eq!(
        ledger.account(&faucet_key.address).expect("owner").balance,
        owner_start_balance - create.unsigned.fee - ACCOUNT_RESERVE - 25
    );
    assert_eq!(
        ledger.account(&faucet_key.address).expect("owner").sequence,
        1
    );
    let escrow = ledger.escrow(&escrow_id).expect("escrow state");
    assert_eq!(escrow.state, ESCROW_STATE_OPEN);
    assert_eq!(escrow.owner, faucet_key.address);
    assert_eq!(escrow.recipient, recipient);
    assert_eq!(escrow.amount, ACCOUNT_RESERVE + 25);
    assert_eq!(escrow.created_height, 1);
    let indexes = ledger
        .escrow_indexes(&genesis.chain_id)
        .expect("escrow indexes");
    let escrow_ids = vec![escrow_id.clone()];
    assert_eq!(
        indexes
            .by_owner
            .get(&faucet_key.address)
            .expect("owner index"),
        &escrow_ids
    );
    assert_eq!(
        indexes
            .by_recipient
            .get(&recipient)
            .expect("recipient index"),
        &escrow_ids
    );
    let condition_hash =
        postfiat_types::escrow_condition_hash("batch-secret").expect("condition hash");
    assert_eq!(
        indexes
            .by_condition_hash
            .get(&condition_hash)
            .expect("condition index"),
        &escrow_ids
    );
    assert_eq!(
        indexes.by_expiry_height.get(&5).expect("expiry index"),
        &escrow_ids
    );

    let info = escrow_info(EscrowInfoOptions {
        data_dir: data_dir.clone(),
        escrow_id: escrow_id.clone(),
    })
    .expect("escrow_info");
    assert_eq!(info.schema, "postfiat-escrow-info-v1");
    assert!(info.found);
    let info_escrow = info.escrow.expect("escrow_info escrow");
    assert_eq!(info_escrow.escrow_id, escrow_id);
    assert_eq!(info_escrow.owner, faucet_key.address);
    assert_eq!(info_escrow.recipient, recipient);
    assert_eq!(info_escrow.amount, ACCOUNT_RESERVE + 25);
    assert_eq!(info_escrow.condition_hash.as_deref(), Some(condition_hash.as_str()));
    assert_eq!(info_escrow.state, ESCROW_STATE_OPEN);

    let missing_info = escrow_info(EscrowInfoOptions {
        data_dir: data_dir.clone(),
        escrow_id: "0".repeat(postfiat_types::ESCROW_ID_HEX_LEN),
    })
    .expect("missing escrow_info");
    assert!(!missing_info.found);
    assert!(missing_info.escrow.is_none());

    let owner_escrows = account_escrows(AccountEscrowsOptions {
        data_dir: data_dir.clone(),
        account: faucet_key.address.clone(),
        role: Some("owner".to_string()),
        state: Some(ESCROW_STATE_OPEN.to_string()),
        limit: Some(10),
    })
    .expect("owner account_escrows");
    assert_eq!(owner_escrows.schema, "postfiat-account-escrows-v1");
    assert_eq!(owner_escrows.escrow_count, 1);
    assert_eq!(owner_escrows.escrows[0].escrow_id, escrow_id);
    assert_eq!(owner_escrows.escrows[0].condition_hash.as_deref(), Some(condition_hash.as_str()));

    let recipient_escrows = account_escrows(AccountEscrowsOptions {
        data_dir: data_dir.clone(),
        account: recipient.clone(),
        role: Some("recipient".to_string()),
        state: Some(ESCROW_STATE_OPEN.to_string()),
        limit: Some(10),
    })
    .expect("recipient account_escrows");
    assert_eq!(recipient_escrows.escrow_count, 1);
    assert_eq!(recipient_escrows.escrows[0].escrow_id, escrow_id);

    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("rebuild escrow account_tx index");
    let owner_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: faucet_key.address.clone(),
        from_height: Some(1),
        to_height: Some(1),
        limit: Some(10),
    })
    .expect("owner escrow account_tx");
    assert!(owner_history.index_used);
    assert_eq!(owner_history.row_count, 1);
    assert_eq!(
        owner_history.rows[0].transaction_kind,
        ESCROW_CREATE_TRANSACTION_KIND
    );
    assert_eq!(owner_history.rows[0].escrow_id.as_deref(), Some(escrow_id.as_str()));
    assert_eq!(
        owner_history.rows[0].condition_hash.as_deref(),
        Some(condition_hash.as_str())
    );
    assert_eq!(owner_history.rows[0].amount, ACCOUNT_RESERVE + 25);
    assert_eq!(owner_history.rows[0].from_address, faucet_key.address);
    assert_eq!(owner_history.rows[0].to_address, recipient);
    assert_eq!(owner_history.rows[0].accepted, Some(true));

    let recipient_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: recipient.clone(),
        from_height: Some(1),
        to_height: Some(1),
        limit: Some(10),
    })
    .expect("recipient escrow account_tx");
    assert!(recipient_history.index_used);
    assert_eq!(recipient_history.row_count, 1);
    assert_eq!(recipient_history.rows[0], owner_history.rows[0]);

    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("escrow batch replay verification");
    assert!(verified.verified);
    assert_eq!(verified.block_count, 1);

    let finality = tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: receipts[0].tx_id.clone(),
        audit_block_log: true,
    })
    .expect("escrow tx finality");
    assert!(finality.confirmed);
    assert_eq!(finality.receipt_count, 1);

    fs::remove_dir_all(data_dir).expect("cleanup escrow batch test");
}

#[test]
fn escrow_fee_quote_mempool_batch_and_replay_flow() {
    let data_dir = unique_test_dir("postfiat-escrow-mempool-test");
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
    let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
    let recipient = address_from_public_key(&recipient_key.public_key);
    let ledger = store.read_ledger().expect("ledger");
    let owner_start_balance = ledger
        .account(&faucet_key.address)
        .expect("faucet account")
        .balance;

    let create_operation =
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            amount: ACCOUNT_RESERVE + 30,
            condition: "mempool-secret".to_string(),
            finish_after: 1,
            cancel_after: 5,
        });
    let create_quote = escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: data_dir.clone(),
        source: faucet_key.address.clone(),
        operation_json: serde_json::to_string(&create_operation).expect("create op json"),
        sequence: None,
    })
    .expect("escrow create fee quote");
    assert_eq!(create_quote.transaction_kind, ESCROW_CREATE_TRANSACTION_KIND);
    assert_eq!(create_quote.sequence, 1);
    assert!(create_quote.state_expansion_fee >= postfiat_execution::ESCROW_STATE_EXPANSION_FEE);
    assert_eq!(create_quote.operation, create_operation);

    let create = signed_escrow_transaction_for_test(
        &genesis,
        &ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CREATE_TRANSACTION_KIND,
        create_quote.sequence,
        create_quote.operation.clone(),
    );
    assert_eq!(create.unsigned.fee, create_quote.minimum_fee);
    let create_entry = submit_signed_escrow_transaction_json_to_mempool(
        SignedEscrowTransactionJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_escrow_transaction_json: serde_json::to_string(&create)
                .expect("signed escrow json"),
        },
    )
    .expect("submit escrow create");
    assert_eq!(
        create_entry.tx_id,
        postfiat_execution::escrow_transaction_tx_id(&create)
    );

    let mempool_report = verify_mempool(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify escrow mempool");
    assert!(mempool_report.verified);
    assert_eq!(mempool_report.pending_count, 1);

    let batch_file = data_dir.join("escrow-mempool-batch.json");
    let batch = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 1,
    })
    .expect("create escrow mempool batch");
    assert_eq!(batch.transaction_count(), 1);
    assert_eq!(batch.escrow_transactions.len(), 1);
    assert!(store.read_mempool().expect("mempool after batch").is_empty());

    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply escrow mempool batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");

    let escrow_id =
        postfiat_types::escrow_id(&genesis.chain_id, &faucet_key.address, create_quote.sequence)
            .expect("escrow id");
    let ledger = store.read_ledger().expect("ledger after escrow mempool");
    assert!(ledger.escrow(&escrow_id).is_some());
    assert_eq!(
        ledger.account(&faucet_key.address).expect("owner").balance,
        owner_start_balance - create.unsigned.fee - ACCOUNT_RESERVE - 30
    );

    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("escrow mempool replay verification");
    let finality = tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: receipts[0].tx_id.clone(),
        audit_block_log: true,
    })
    .expect("escrow mempool finality");
    assert!(finality.confirmed);

    fs::remove_dir_all(data_dir).expect("cleanup escrow mempool test");
}

#[test]
fn escrow_restart_replay_preserves_locked_funds_and_release_edges() {
    let data_dir = unique_test_dir("postfiat-escrow-restart-replay-test");
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
    let recipient_key = ml_dsa_65_keygen().expect("recipient keygen");
    let recipient = address_from_public_key(&recipient_key.public_key);
    let recipient_public_key_hex = bytes_to_hex(&recipient_key.public_key);
    let recipient_private_key_hex = bytes_to_hex(&recipient_key.private_key);
    let ledger = store.read_ledger().expect("ledger");
    let owner_start_balance = ledger
        .account(&faucet_key.address)
        .expect("faucet account")
        .balance;
    let recipient_funding_amount = ACCOUNT_RESERVE + 200;
    let first_escrow_amount = ACCOUNT_RESERVE + 80;
    let second_escrow_amount = ACCOUNT_RESERVE + 60;

    let fund_recipient = build_signed_transfer_for_key(
        &genesis,
        &ledger,
        &faucet_key,
        recipient.clone(),
        recipient_funding_amount,
        1,
    )
    .expect("fund recipient transfer");
    let funding_fee = fund_recipient.unsigned.fee;
    let funding_batch =
        build_transaction_batch(&mempool_batch_domain(&genesis), vec![fund_recipient])
            .expect("funding batch")
            .batch;
    let funding_batch_file = data_dir.join("escrow-replay-funding-batch.json");
    write_batch_file(&funding_batch_file, &funding_batch).expect("write funding batch");
    let funding_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: funding_batch_file,
        certificate_file: None,
    })
    .expect("apply funding batch");
    assert_eq!(funding_receipts.len(), 1);
    assert!(funding_receipts[0].accepted, "{funding_receipts:?}");

    let ledger = store.read_ledger().expect("funded ledger");
    let first_escrow_id =
        postfiat_types::escrow_id(&genesis.chain_id, &faucet_key.address, 2)
            .expect("first escrow id");
    let first_create = signed_escrow_transaction_for_test(
        &genesis,
        &ledger,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CREATE_TRANSACTION_KIND,
        2,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            amount: first_escrow_amount,
            condition: "restart-secret".to_string(),
            finish_after: 4,
            cancel_after: 6,
        }),
    );
    let first_create_fee = first_create.unsigned.fee;
    let first_create_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![first_create.clone()],
    )
    .expect("first escrow create batch")
    .batch;
    let first_create_batch_file = data_dir.join("first-escrow-create-batch.json");
    write_batch_file(&first_create_batch_file, &first_create_batch)
        .expect("write first escrow create batch");
    let first_create_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_create_batch_file,
        certificate_file: None,
    })
    .expect("apply first escrow create batch");
    assert_eq!(first_create_receipts.len(), 1);
    assert!(first_create_receipts[0].accepted, "{first_create_receipts:?}");

    let ledger_after_create = store.read_ledger().expect("ledger after first escrow");
    let owner_after_first_create = owner_start_balance
        - recipient_funding_amount
        - funding_fee
        - first_create_fee
        - first_escrow_amount;
    assert_eq!(
        ledger_after_create
            .account(&faucet_key.address)
            .expect("owner")
            .balance,
        owner_after_first_create
    );
    assert_eq!(
        ledger_after_create
            .escrow(&first_escrow_id)
            .expect("first escrow")
            .state,
        ESCROW_STATE_OPEN
    );

    let double_spend = build_signed_transfer_for_key(
        &genesis,
        &ledger_after_create,
        &faucet_key,
        recipient.clone(),
        owner_after_first_create,
        3,
    )
    .expect("overspend locked funds transfer");
    let double_spend_batch =
        build_transaction_batch(&mempool_batch_domain(&genesis), vec![double_spend])
            .expect("double spend batch")
            .batch;
    let double_spend_batch_file = data_dir.join("escrow-double-spend-batch.json");
    write_batch_file(&double_spend_batch_file, &double_spend_batch)
        .expect("write double spend batch");
    let double_spend_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: double_spend_batch_file,
        certificate_file: None,
    })
    .expect("apply double spend batch");
    assert_eq!(double_spend_receipts.len(), 1);
    assert!(!double_spend_receipts[0].accepted, "{double_spend_receipts:?}");
    assert_eq!(double_spend_receipts[0].code, "insufficient_funds");
    assert_eq!(
        store.read_ledger().expect("ledger after rejected double spend"),
        ledger_after_create
    );

    let replay_after_double_spend = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay after rejected double spend");
    assert!(replay_after_double_spend.verified);
    assert_eq!(replay_after_double_spend.block_count, 3);
    let restarted_store = NodeStore::new(&data_dir);
    assert_eq!(
        restarted_store
            .read_ledger()
            .expect("restart ledger after double spend"),
        ledger_after_create
    );

    let finish = signed_escrow_transaction_for_test(
        &genesis,
        &ledger_after_create,
        &recipient,
        &recipient_public_key_hex,
        &recipient_private_key_hex,
        ESCROW_FINISH_TRANSACTION_KIND,
        1,
        EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: first_escrow_id.clone(),
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            fulfillment: "restart-secret".to_string(),
        }),
    );
    let finish_fee = finish.unsigned.fee;
    let finish_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![finish.clone()],
    )
    .expect("finish batch")
    .batch;
    let finish_batch_file = data_dir.join("first-escrow-finish-batch.json");
    write_batch_file(&finish_batch_file, &finish_batch).expect("write finish batch");
    let finish_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: finish_batch_file,
        certificate_file: None,
    })
    .expect("apply finish batch");
    assert_eq!(finish_receipts.len(), 1);
    assert!(finish_receipts[0].accepted, "{finish_receipts:?}");

    let ledger_after_finish = store.read_ledger().expect("ledger after finish");
    assert_eq!(
        ledger_after_finish
            .escrow(&first_escrow_id)
            .expect("first escrow")
            .state,
        ESCROW_STATE_FINISHED
    );
    assert_eq!(
        ledger_after_finish
            .account(&recipient)
            .expect("recipient")
            .balance,
        recipient_funding_amount - finish_fee + first_escrow_amount
    );
    assert_eq!(
        ledger_after_finish
            .account(&faucet_key.address)
            .expect("owner")
            .balance,
        owner_after_first_create
    );

    let cancel_finished = signed_escrow_transaction_for_test(
        &genesis,
        &ledger_after_finish,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CANCEL_TRANSACTION_KIND,
        3,
        EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
            escrow_id: first_escrow_id.clone(),
            owner: faucet_key.address.clone(),
        }),
    );
    let cancel_finished_batch =
        postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![cancel_finished],
        )
        .expect("cancel finished batch")
        .batch;
    let cancel_finished_batch_file = data_dir.join("cancel-finished-escrow-batch.json");
    write_batch_file(&cancel_finished_batch_file, &cancel_finished_batch)
        .expect("write cancel finished batch");
    let cancel_finished_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: cancel_finished_batch_file,
        certificate_file: None,
    })
    .expect("apply cancel finished batch");
    assert_eq!(cancel_finished_receipts.len(), 1);
    assert!(!cancel_finished_receipts[0].accepted, "{cancel_finished_receipts:?}");
    assert_eq!(cancel_finished_receipts[0].code, "escrow_not_open");
    assert_eq!(
        store
            .read_ledger()
            .expect("ledger after rejected cancel finished"),
        ledger_after_finish
    );

    let second_escrow_id =
        postfiat_types::escrow_id(&genesis.chain_id, &faucet_key.address, 3)
            .expect("second escrow id");
    let second_create = signed_escrow_transaction_for_test(
        &genesis,
        &ledger_after_finish,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CREATE_TRANSACTION_KIND,
        3,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            amount: second_escrow_amount,
            condition: String::new(),
            finish_after: 0,
            cancel_after: 8,
        }),
    );
    let second_create_fee = second_create.unsigned.fee;
    let second_create_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![second_create],
    )
    .expect("second escrow create batch")
    .batch;
    let second_create_batch_file = data_dir.join("second-escrow-create-batch.json");
    write_batch_file(&second_create_batch_file, &second_create_batch)
        .expect("write second escrow create batch");
    let second_create_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: second_create_batch_file,
        certificate_file: None,
    })
    .expect("apply second escrow create batch");
    assert_eq!(second_create_receipts.len(), 1);
    assert!(second_create_receipts[0].accepted, "{second_create_receipts:?}");
    let ledger_after_second_create = store
        .read_ledger()
        .expect("ledger after second escrow create");
    let owner_after_second_create =
        owner_after_first_create - second_create_fee - second_escrow_amount;
    assert_eq!(
        ledger_after_second_create
            .account(&faucet_key.address)
            .expect("owner")
            .balance,
        owner_after_second_create
    );
    assert_eq!(
        ledger_after_second_create
            .escrow(&second_escrow_id)
            .expect("second escrow")
            .state,
        ESCROW_STATE_OPEN
    );

    let cancel_second = signed_escrow_transaction_for_test(
        &genesis,
        &ledger_after_second_create,
        &faucet_key.address,
        &faucet_key.public_key_hex,
        &faucet_key.private_key_hex,
        ESCROW_CANCEL_TRANSACTION_KIND,
        4,
        EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
            escrow_id: second_escrow_id.clone(),
            owner: faucet_key.address.clone(),
        }),
    );
    let early_cancel_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![cancel_second.clone()],
    )
    .expect("early cancel batch")
    .batch;
    let early_cancel_batch_file = data_dir.join("early-cancel-second-escrow-batch.json");
    write_batch_file(&early_cancel_batch_file, &early_cancel_batch)
        .expect("write early cancel batch");
    let early_cancel_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: early_cancel_batch_file,
        certificate_file: None,
    })
    .expect("apply early cancel batch");
    assert_eq!(early_cancel_receipts.len(), 1);
    assert!(!early_cancel_receipts[0].accepted, "{early_cancel_receipts:?}");
    assert_eq!(early_cancel_receipts[0].code, "escrow_cancel_too_early");
    assert_eq!(
        store.read_ledger().expect("ledger after early cancel"),
        ledger_after_second_create
    );

    let replay_after_early_cancel = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay after early cancel");
    assert!(replay_after_early_cancel.verified);
    assert_eq!(replay_after_early_cancel.block_count, 7);
    let restarted_store = NodeStore::new(&data_dir);
    assert_eq!(
        restarted_store
            .read_ledger()
            .expect("restart ledger after early cancel"),
        ledger_after_second_create
    );

    let cancel_second_retry = {
        let mut unsigned = cancel_second.unsigned.clone();
        unsigned.fee += 1;
        let private_key = hex_to_bytes(&faucet_key.private_key_hex).expect("owner key bytes");
        let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes())
            .expect("sign retry cancel");
        SignedEscrowTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: faucet_key.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        }
    };
    let cancel_second_fee = cancel_second_retry.unsigned.fee;
    let cancel_second_batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
        &mempool_batch_domain(&genesis),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![cancel_second_retry],
    )
    .expect("cancel second batch")
    .batch;
    let cancel_second_batch_file = data_dir.join("cancel-second-escrow-batch.json");
    write_batch_file(&cancel_second_batch_file, &cancel_second_batch)
        .expect("write cancel second batch");
    let cancel_second_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: cancel_second_batch_file,
        certificate_file: None,
    })
    .expect("apply cancel second batch");
    assert_eq!(cancel_second_receipts.len(), 1);
    assert!(cancel_second_receipts[0].accepted, "{cancel_second_receipts:?}");
    let ledger_after_cancel = store.read_ledger().expect("ledger after cancel");
    assert_eq!(
        ledger_after_cancel
            .escrow(&second_escrow_id)
            .expect("second escrow")
            .state,
        ESCROW_STATE_CANCELED
    );
    assert_eq!(
        ledger_after_cancel
            .account(&faucet_key.address)
            .expect("owner")
            .balance,
        owner_after_first_create - second_create_fee - cancel_second_fee
    );

    let finish_canceled = signed_escrow_transaction_for_test(
        &genesis,
        &ledger_after_cancel,
        &recipient,
        &recipient_public_key_hex,
        &recipient_private_key_hex,
        ESCROW_FINISH_TRANSACTION_KIND,
        2,
        EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: second_escrow_id.clone(),
            owner: faucet_key.address.clone(),
            recipient: recipient.clone(),
            fulfillment: String::new(),
        }),
    );
    let finish_canceled_batch =
        postfiat_mempool_dag::build_mixed_transaction_batch_with_escrows(
            &mempool_batch_domain(&genesis),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![finish_canceled],
        )
        .expect("finish canceled batch")
        .batch;
    let finish_canceled_batch_file = data_dir.join("finish-canceled-escrow-batch.json");
    write_batch_file(&finish_canceled_batch_file, &finish_canceled_batch)
        .expect("write finish canceled batch");
    let finish_canceled_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: finish_canceled_batch_file,
        certificate_file: None,
    })
    .expect("apply finish canceled batch");
    assert_eq!(finish_canceled_receipts.len(), 1);
    assert!(!finish_canceled_receipts[0].accepted, "{finish_canceled_receipts:?}");
    assert_eq!(finish_canceled_receipts[0].code, "escrow_not_open");
    assert_eq!(
        store.read_ledger().expect("ledger after finish canceled"),
        ledger_after_cancel
    );

    let final_ledger = store.read_ledger().expect("final ledger");
    final_ledger
        .validate_escrow_state(&genesis.chain_id)
        .expect("valid final escrow state");
    assert!(
        final_ledger
            .escrows
            .iter()
            .all(|escrow| escrow.state != ESCROW_STATE_OPEN),
        "{:?}",
        final_ledger.escrows
    );
    let final_owner = final_ledger
        .account(&faucet_key.address)
        .expect("final owner")
        .balance;
    let final_recipient = final_ledger
        .account(&recipient)
        .expect("final recipient")
        .balance;
    assert_eq!(
        final_owner + final_recipient,
        owner_start_balance - funding_fee - first_create_fee - finish_fee - second_create_fee
            - cancel_second_fee
    );
    let open_owner_escrows = account_escrows(AccountEscrowsOptions {
        data_dir: data_dir.clone(),
        account: faucet_key.address.clone(),
        role: Some("owner".to_string()),
        state: Some(ESCROW_STATE_OPEN.to_string()),
        limit: Some(10),
    })
    .expect("open owner escrows");
    assert_eq!(open_owner_escrows.escrow_count, 0);

    let final_replay = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("final escrow replay verification");
    assert!(final_replay.verified);
    assert_eq!(final_replay.block_count, 9);
    let final_restart_store = NodeStore::new(&data_dir);
    assert_eq!(
        final_restart_store
            .read_ledger()
            .expect("final restart ledger"),
        final_ledger
    );

    fs::remove_dir_all(data_dir).expect("cleanup escrow restart replay test");
}
