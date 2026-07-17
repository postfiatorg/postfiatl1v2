use super::*;

#[test]
fn local_transfer_builder_rejects_exhausted_sequence_without_panicking() {
    let data_dir = unique_test_dir("postfiat-transfer-builder-sequence-overflow-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    let faucet_key = read_transfer_key_file(&data_dir, None).expect("faucet key");
    ledger
        .account_mut(&faucet_key.address)
        .expect("faucet account")
        .sequence = u64::MAX;
    let before = ledger.clone();

    let error = build_signed_transfer(
        &store.read_genesis().expect("genesis"),
        &ledger,
        &data_dir,
        None,
        faucet_key.address,
        1,
    )
    .expect_err("exhausted sequence must fail closed");

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    assert!(error.to_string().contains("sequence is exhausted"), "{error}");
    assert_eq!(ledger, before);
}

#[test]
fn init_then_run_once() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-node-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    let initialized = init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");

    assert_eq!(initialized.chain_id, "postfiat-local");
    assert_eq!(initialized.node_id, "validator-0");
    assert_eq!(initialized.status, "initialized");
    assert_eq!(initialized.block_height, 0);
    assert_eq!(initialized.block_tip_hash, "genesis");
    assert_eq!(initialized.mempool_pending, 0);

    let key_file = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("faucet key");
    let faucet = account(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &key_file.address,
    )
    .expect("faucet account");

    assert_eq!(faucet.balance, DEFAULT_FAUCET_BALANCE);
    let faucet_key_path = data_dir.join(FAUCET_KEY_FILE);
    let donor_key_file = create_dev_key_file().expect("donor development key");
    let mut mismatched_dev_key = key_file.clone();
    mismatched_dev_key.address = donor_key_file.address;
    mismatched_dev_key.public_key_hex = donor_key_file.public_key_hex;
    let mismatched_dev_key_json =
        serde_json::to_string_pretty(&mismatched_dev_key).expect("mismatched dev key json");
    atomic_write(&faucet_key_path, format!("{mismatched_dev_key_json}\n"))
        .expect("write mismatched faucet key");
    set_private_file_permissions(&faucet_key_path)
        .expect("preserve private mode for mismatched faucet key");
    let dev_key_mismatch_error = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("development key file should reject mismatched public/private key pair");
    assert!(
        dev_key_mismatch_error
            .to_string()
            .contains("development key public/private key mismatch"),
        "{dev_key_mismatch_error}"
    );
    let local_dev_key_mismatch_error = validate_local_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 1,
        local_only: false,
    })
    .expect_err("local key validation should reject mismatched development key pair");
    assert!(
        local_dev_key_mismatch_error
            .to_string()
            .contains("development key public/private key mismatch"),
        "{local_dev_key_mismatch_error}"
    );
    write_key_file(&faucet_key_path, &key_file).expect("restore faucet key after self-check");

    let validator_registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let validator_registry =
        read_validator_registry_file(&validator_registry_path).expect("validator registry");
    fs::remove_file(&validator_registry_path).expect("remove zero-block validator registry");
    let zero_block_registry_error = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("zero-block state verification should require validator registry");
    assert!(
        zero_block_registry_error
            .to_string()
            .contains("validator_registry.json"),
        "{zero_block_registry_error}"
    );
    write_validator_registry_file(&validator_registry_path, &validator_registry)
        .expect("restore zero-block validator registry");
    let undersized_registry = ValidatorRegistry {
        validators: Vec::new(),
    };
    write_validator_registry_file(&validator_registry_path, &undersized_registry)
        .expect("write undersized zero-block validator registry");
    let undersized_registry_error = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("zero-block state verification should reject undersized registry");
    assert!(
        undersized_registry_error
            .to_string()
            .contains("expected at least"),
        "{undersized_registry_error}"
    );
    write_validator_registry_file(&validator_registry_path, &validator_registry)
        .expect("restore full zero-block validator registry");
    let wrong_validator_registry = ValidatorRegistry {
        validators: vec![ValidatorRegistryRecord {
            node_id: "validator-999".to_string(),
            algorithm_id: validator_registry.validators[0].algorithm_id.clone(),
            public_key_hex: validator_registry.validators[0].public_key_hex.clone(),
        }],
    };
    write_validator_registry_file(&validator_registry_path, &wrong_validator_registry)
        .expect("write wrong zero-block validator registry");
    let wrong_registry_error = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("zero-block state verification should reject missing active validator key");
    assert!(
        wrong_registry_error
            .to_string()
            .contains("missing validator registry key `validator-0`"),
        "{wrong_registry_error}"
    );
    write_validator_registry_file(&validator_registry_path, &validator_registry)
        .expect("restore active zero-block validator registry");
    let mut invalid_hex_registry = validator_registry.clone();
    invalid_hex_registry.validators[0].public_key_hex = "not-hex".to_string();
    write_validator_registry_file(&validator_registry_path, &invalid_hex_registry)
        .expect("write invalid-hex zero-block validator registry");
    let invalid_hex_registry_error = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("zero-block state verification should reject non-hex validator public key");
    assert!(
        invalid_hex_registry_error
            .to_string()
            .contains("invalid public key hex"),
        "{invalid_hex_registry_error}"
    );
    let mut invalid_key_registry = validator_registry.clone();
    invalid_key_registry.validators[0].public_key_hex = "00".to_string();
    write_validator_registry_file(&validator_registry_path, &invalid_key_registry)
        .expect("write invalid-key zero-block validator registry");
    let invalid_key_registry_error = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("zero-block state verification should reject malformed validator public key");
    assert!(
        invalid_key_registry_error
            .to_string()
            .contains("invalid ML-DSA-65 public key"),
        "{invalid_key_registry_error}"
    );
    write_validator_registry_file(&validator_registry_path, &validator_registry)
        .expect("restore valid zero-block validator registry");
    let validator_key_path = data_dir.join(VALIDATOR_KEYS_FILE);
    let expanded_validator_keys = validator_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 2,
        local_only: false,
    })
    .expect("expand validator keys for self-check regression");
    let mut mismatched_validator_keys = expanded_validator_keys.clone();
    mismatched_validator_keys.validators[0].public_key_hex =
        expanded_validator_keys.validators[1].public_key_hex.clone();
    let mismatched_validator_keys_json =
        serde_json::to_string_pretty(&mismatched_validator_keys)
            .expect("mismatched validator key json");
    atomic_write(
        &validator_key_path,
        format!("{mismatched_validator_keys_json}\n"),
    )
    .expect("write mismatched validator key fixture");
    set_private_file_permissions(&validator_key_path)
        .expect("preserve private mode for mismatched validator key fixture");
    let key_mismatch_error = validator_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 2,
        local_only: false,
    })
    .expect_err("validator key file should reject mismatched public/private key pair");
    assert!(
        key_mismatch_error
            .to_string()
            .contains("public/private key mismatch"),
        "{key_mismatch_error}"
    );
    let local_validator_key_mismatch_error = validate_local_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 2,
        local_only: false,
    })
    .expect_err("local key validation should reject mismatched validator key pair");
    assert!(
        local_validator_key_mismatch_error
            .to_string()
            .contains("public/private key mismatch"),
        "{local_validator_key_mismatch_error}"
    );
    write_validator_key_file(&validator_key_path, &expanded_validator_keys)
        .expect("restore validator key file after self-check regression");
    let split_validator_keys = ValidatorKeyFile {
        validators: vec![expanded_validator_keys.validators[0].clone()],
    };
    write_validator_key_file(&validator_key_path, &split_validator_keys)
        .expect("write split local validator key file");
    let split_local_report = validate_local_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 2,
        local_only: true,
    })
    .expect("split local validator key should validate for local node");
    assert_eq!(split_local_report.validator_key_count, 1);
    assert_eq!(split_local_report.required_validator_count, 1);
    let split_combined_error = validate_local_keys(ValidatorKeysOptions {
        data_dir: data_dir.clone(),
        validators: 2,
        local_only: false,
    })
    .expect_err("split local validator key should not satisfy combined validation");
    assert!(
        split_combined_error
            .to_string()
            .contains("expected at least 2"),
        "{split_combined_error}"
    );
    write_validator_key_file(&validator_key_path, &expanded_validator_keys)
        .expect("restore combined validator key file after split-local regression");

    let running = run_once(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("run once");

    assert_eq!(running.status, "running");
    assert!(running.last_run_unix > 0);

    let batch_file = data_dir.join("batch.json");
    let batch = create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000001".to_string(),
        amount: 25,
        batch_file: batch_file.clone(),
    })
    .expect("create batch");
    assert_eq!(batch.transactions.len(), 1);

    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    let receipt_log = crate::receipts(ReceiptQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: Some(receipts[0].tx_id.clone()),
        limit: None,
    })
    .expect("receipt query");
    assert_eq!(receipt_log, vec![receipts[0].clone()]);

    let pending = submit_transfer_to_mempool(TransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000002".to_string(),
        amount: 15,
    })
    .expect("submit pending transfer");
    assert_eq!(pending.transfer.unsigned.sequence, 2);
    let second_pending = submit_transfer_to_mempool(TransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000003".to_string(),
        amount: ACCOUNT_RESERVE,
    })
    .expect("submit second pending transfer");
    assert_eq!(second_pending.transfer.unsigned.sequence, 3);
    assert!(submit_transfer_to_mempool(TransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000004".to_string(),
        amount: DEFAULT_FAUCET_BALANCE,
    })
    .is_err());
    assert_eq!(
        mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool")
        .len(),
        2
    );
    let mempool_report = verify_mempool(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify pending mempool");
    assert!(mempool_report.verified);
    assert_eq!(mempool_report.pending_count, 2);
    assert_eq!(mempool_report.sender_count, 1);
    assert_eq!(mempool_report.total_amount, 25);
    assert_eq!(
        mempool_report.total_fee,
        pending.transfer.unsigned.fee + second_pending.transfer.unsigned.fee
    );

    let mempool_store = NodeStore::new(&data_dir);
    let mempool_before_tamper = mempool_store.read_mempool().expect("mempool before tamper");
    let mut mempool_with_tampered_tx_id = mempool_before_tamper.clone();
    mempool_with_tampered_tx_id.pending[0].tx_id = "tampered-pending-tx-id".to_string();
    mempool_store
        .write_mempool(&mempool_with_tampered_tx_id)
        .expect("write tampered mempool");
    let mempool_verification_error = verify_mempool(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("tampered mempool tx id should fail verification");
    assert!(
        mempool_verification_error
            .to_string()
            .contains("tx id mismatch"),
        "{mempool_verification_error}"
    );
    mempool_store
        .write_mempool(&mempool_before_tamper)
        .expect("restore mempool after tamper");
    let oversized_batch_error = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("oversized-mempool-batch.json"),
        max_transactions: MAX_BATCH_TRANSACTIONS + 1,
    })
    .expect_err("oversized batch transaction request should fail");
    assert_eq!(
        oversized_batch_error.kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after oversized batch request")
        .len(),
        2
    );
    let failing_batch_path = data_dir.join("mempool-batch-output-dir");
    fs::create_dir(&failing_batch_path).expect("create batch output dir");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: failing_batch_path.clone(),
        max_transactions: 10,
    })
    .expect_err("batch file write failure should not prune mempool");
    assert_eq!(
        mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("mempool after failed batch write")
        .len(),
        2
    );
    fs::remove_dir(&failing_batch_path).expect("remove batch output dir");
    let mempool_batch_file = data_dir.join("mempool-batch.json");
    let mempool_batch = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: mempool_batch_file.clone(),
        max_transactions: 10,
    })
    .expect("create mempool batch");
    assert_eq!(mempool_batch.transactions.len(), 2);
    assert!(mempool_state(NodeOptions {
        data_dir: data_dir.clone()
    })
    .expect("mempool after batch")
    .is_empty());
    let mempool_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: mempool_batch_file,

        certificate_file: None,
    })
    .expect("apply mempool batch");
    assert!(mempool_receipts[0].accepted, "{mempool_receipts:?}");
    assert!(mempool_receipts[1].accepted, "{mempool_receipts:?}");
    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks");
    assert_eq!(block_log.len(), 2);
    assert_eq!(block_log[1].header.height, 2);
    assert_eq!(block_log[1].header.batch_kind, "transparent");
    assert!(!block_log[1].header.certificate_id.is_empty());
    assert_eq!(
        block_log[1].header.certificate.validators,
        vec!["validator-0".to_string()]
    );
    assert_eq!(block_log[1].header.certificate.quorum, 1);
    assert_eq!(block_log[1].header.certificate.votes.len(), 1);
    assert_eq!(
        block_log[1].receipt_ids,
        vec![
            mempool_receipts[0].tx_id.clone(),
            mempool_receipts[1].tx_id.clone()
        ]
    );
    let finality = tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: mempool_receipts[0].tx_id.clone(),
        audit_block_log: true,
    })
    .expect("tx finality proof");
    assert_eq!(finality.schema, "postfiat-tx-finality-v1");
    assert_eq!(finality.tx_id, mempool_receipts[0].tx_id);
    assert!(finality.confirmed);
    assert!(finality.block_log_verified);
    assert_eq!(finality.receipt, mempool_receipts[0]);
    assert_eq!(finality.receipt_index, 0);
    assert_eq!(finality.receipt_count, 2);
    assert_eq!(finality.block.header.height, block_log[1].header.height);
    assert_eq!(
        finality.block.header.block_hash,
        block_log[1].header.block_hash
    );
    assert_eq!(
        finality.block.header.certificate_id,
        block_log[1].header.certificate_id
    );
    assert_eq!(finality.block.receipt_ids, block_log[1].receipt_ids);
    assert!(!finality.proof_id.is_empty());

    let account_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: pending.transfer.unsigned.from.clone(),
        from_height: Some(2),
        to_height: Some(2),
        limit: Some(10),
    })
    .expect("account tx history");
    assert_eq!(account_history.schema, "postfiat-account-tx-v1");
    assert_eq!(account_history.address, pending.transfer.unsigned.from);
    assert_eq!(account_history.scan_limit, 10);
    assert!(!account_history.index_used);
    assert_eq!(account_history.scanned_block_count, 1);
    assert_eq!(account_history.archive_lookup_count, 1);
    assert!(!account_history.truncated);
    assert_eq!(account_history.row_count, 2);
    assert_eq!(account_history.rows.len(), 2);
    assert_eq!(account_history.rows[0].tx_id, mempool_receipts[0].tx_id);
    assert_eq!(account_history.rows[0].block_height, 2);
    assert_eq!(account_history.rows[0].transaction_index, 0);
    assert_eq!(
        account_history.rows[0].from_address,
        pending.transfer.unsigned.from
    );
    assert_eq!(
        account_history.rows[0].to_address,
        pending.transfer.unsigned.to
    );
    assert_eq!(account_history.rows[0].amount, pending.transfer.unsigned.amount);
    assert_eq!(account_history.rows[0].fee, pending.transfer.unsigned.fee);
    assert_eq!(
        account_history.rows[0].sequence,
        pending.transfer.unsigned.sequence
    );
    assert_eq!(account_history.rows[0].accepted, Some(true));
    assert_eq!(
        account_history.rows[0].receipt_code.as_deref(),
        Some("accepted")
    );

    let auto_index_status = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("account tx index status before explicit build");
    assert!(!auto_index_status.index_present);
    assert!(!auto_index_status.index_usable);
    assert_eq!(
        auto_index_status.reason.as_deref(),
        Some("account_tx index file is absent")
    );
    assert!(!auto_index_status.disk_index_present);
    assert!(!auto_index_status.disk_index_usable);
    assert_eq!(auto_index_status.indexed_row_count, 0);
    assert_eq!(auto_index_status.account_count, 0);

    let index_build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("build account tx index");
    assert_eq!(index_build.schema, "postfiat-account-tx-index-build-v1");
    assert!(index_build.index_usable);
    assert!(index_build.disk_index_usable);
    assert_eq!(index_build.indexed_block_count, 2);
    assert_eq!(index_build.indexed_row_count, 3);
    assert!(index_build.account_count >= 3);

    let index_status = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("account tx index status");
    assert!(index_status.index_present);
    assert!(index_status.index_usable);
    assert_eq!(index_status.reason, None);
    assert_eq!(index_status.indexed_row_count, index_build.indexed_row_count);
    assert_eq!(index_status.tip_hash, index_build.tip_hash);

    let indexed_account_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: pending.transfer.unsigned.from.clone(),
        from_height: Some(2),
        to_height: Some(2),
        limit: Some(10),
    })
    .expect("indexed account tx history");
    assert!(indexed_account_history.index_used);
    assert_eq!(indexed_account_history.archive_lookup_count, 0);
    assert_eq!(indexed_account_history.row_count, account_history.row_count);
    assert_eq!(indexed_account_history.rows, account_history.rows);

    let recipient_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: second_pending.transfer.unsigned.to.clone(),
        from_height: Some(2),
        to_height: Some(2),
        limit: Some(1),
    })
    .expect("recipient account tx history");
    assert!(recipient_history.truncated);
    assert_eq!(recipient_history.row_count, 1);
    assert_eq!(recipient_history.rows[0].tx_id, mempool_receipts[1].tx_id);

    assert!(tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: "not-a-transaction-id".to_string(),
        audit_block_log: true,
    })
    .expect_err("malformed tx finality query should fail")
    .to_string()
    .contains("96 lowercase hex"));
    let block_report = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify blocks");
    assert!(block_report.verified);
    assert_eq!(block_report.block_count, 2);
    let validator_registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let validator_registry =
        read_validator_registry_file(&validator_registry_path).expect("validator registry");
    std::fs::remove_file(&validator_registry_path).expect("remove validator registry");
    let missing_registry_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("missing validator registry should fail block verification");
    assert!(
        missing_registry_error
            .to_string()
            .contains(VALIDATOR_REGISTRY_FILE),
        "{missing_registry_error}"
    );
    write_validator_registry_file(&validator_registry_path, &validator_registry)
        .expect("restore validator registry");
    let state_report = verify_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify full state");
    assert_eq!(state_report.schema, "postfiat-state-verification-v1");
    assert!(state_report.verified);
    assert_eq!(state_report.block_log.block_count, 2);
    assert_eq!(state_report.mempool.pending_count, 0);
    assert_eq!(state_report.governance.active_validator_count, 1);
    assert!(state_report.bridge.verified);
    assert!(state_report.shielded.verified);
    let status_with_blocks = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status with blocks");
    assert_eq!(status_with_blocks.block_height, 2);
    assert_eq!(status_with_blocks.block_tip_hash, block_report.tip_hash);
    assert_eq!(status_with_blocks.mempool_pending, 0);
    let metrics = crate::metrics(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("metrics");
    assert_eq!(metrics.schema, "postfiat-node-metrics-v1");
    assert_eq!(metrics.consensus.active_validator_count, 1);
    assert_eq!(metrics.consensus.block_certificate_count, 2);
    assert_eq!(metrics.consensus.block_certificate_vote_count, 2);
    assert_eq!(metrics.consensus.recent_certificate_window_blocks, 2);
    assert_eq!(metrics.consensus.recent_certificate_vote_count, 2);
    assert_eq!(metrics.consensus.local_recent_certificate_vote_count, 2);
    assert_eq!(metrics.consensus.local_recent_certificate_participation_ppm, 1_000_000);
    assert!(metrics.observed_unix_ms > 0);
    assert_eq!(metrics.ordering.block_height, 2);
    assert_eq!(metrics.ordering.ordered_batch_count, 2);
    assert_eq!(metrics.execution.receipt_count, 3);
    assert!(metrics.execution.burned_fee_total > 0);
    assert_eq!(metrics.execution.account_reserve, ACCOUNT_RESERVE);
    assert_eq!(metrics.execution.minimum_transfer_fee, MIN_TRANSFER_FEE);
    assert_eq!(
        metrics.execution.transfer_account_creation_fee,
        postfiat_execution::TRANSFER_ACCOUNT_CREATION_FEE
    );
    assert_eq!(
        metrics.execution.transfer_fee_byte_quantum,
        postfiat_execution::TRANSFER_FEE_BYTE_QUANTUM as u64
    );
    assert_eq!(
        metrics.execution.transfer_fee_per_quantum,
        postfiat_execution::TRANSFER_FEE_PER_QUANTUM
    );
    assert_eq!(metrics.assets.asset_count, 0);
    assert_eq!(metrics.assets.trustline_count, 0);
    assert_eq!(metrics.assets.holder_count, 0);
    assert_eq!(metrics.assets.total_outstanding_supply, 0);
    assert_eq!(metrics.assets.open_issued_escrow_count, 0);
    assert_eq!(metrics.assets.open_issued_offer_count, 0);
    assert_eq!(metrics.assets.clawback_enabled_asset_count, 0);
    assert_eq!(metrics.mempool.pending, 0);
    assert_eq!(metrics.shielded.turnstile_event_count, 0);
    assert_eq!(
        metrics.storage.replicated_state_file_count,
        SNAPSHOT_FILES.len() as u64
    );
    assert!(metrics.storage.filesystem_total_bytes > 0);
    assert!(
        metrics.storage.filesystem_available_bytes <= metrics.storage.filesystem_total_bytes
    );
    assert!(metrics.storage.filesystem_available_ppm <= 1_000_000);
    assert_eq!(metrics.proofs.last_verify_micros, 0);
    record_local_proof_verify_latency(&NodeStore::new(&data_dir), 12.345)
        .expect("record local proof latency");
    let metrics_with_proof = crate::metrics(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("metrics with proof latency");
    assert_eq!(metrics_with_proof.proofs.last_verify_micros, 12_345);
    assert!(metrics_with_proof.proofs.last_observed_unix_ms > 0);
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let archive = store.read_batch_archive().expect("batch archive");
    assert_eq!(archive.len(), 2);
    assert!(archive
        .find("transparent", &block_log[1].header.batch_id)
        .is_some());
    let blocks_before_replay_tamper = store.read_blocks().expect("blocks before replay tamper");
    let mut blocks_with_certificate_tamper = blocks_before_replay_tamper.clone();
    blocks_with_certificate_tamper
        .blocks
        .last_mut()
        .expect("latest block for certificate tamper")
        .header
        .certificate
        .votes[0]
        .vote_id = "tampered-block-vote".to_string();
    store
        .write_blocks(&blocks_with_certificate_tamper)
        .expect("write certificate tampered blocks");
    let certificate_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("tampered block certificate should fail verification");
    assert!(
        certificate_error
            .to_string()
            .contains("certificate vote mismatch"),
        "{certificate_error}"
    );
    store
        .write_blocks(&blocks_before_replay_tamper)
        .expect("restore certificate tampered blocks");
    let mut blocks_with_replay_tamper = blocks_before_replay_tamper.clone();
    let replay_tamper = blocks_with_replay_tamper
        .blocks
        .last_mut()
        .expect("latest block for replay tamper");
    replay_tamper.header.state_root =
        hash_hex("postfiat.test.replay_state_root.v1", b"tampered-state");
    let tampered_evidence = BlockEvidence::from_block(replay_tamper);
    let (tampered_certificate_id, tampered_certificate) = block_certificate(
        &genesis,
        &tampered_evidence,
        &validator_keys,
        &local_validator_ids(status_with_blocks.validator_count).expect("validator ids"),
    )
    .expect("tampered certificate");
    replay_tamper.header.certificate_id = tampered_certificate_id;
    replay_tamper.header.certificate = tampered_certificate;
    let tampered_evidence = BlockEvidence::from_block(replay_tamper);
    replay_tamper.header.block_hash = block_hash(
        &genesis,
        &tampered_evidence,
        &replay_tamper.header.certificate_id,
    )
    .expect("tampered block hash");
    store
        .write_blocks(&blocks_with_replay_tamper)
        .expect("write replay tampered blocks");
    let replay_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("replay state root tamper should fail verification");
    assert!(
        replay_error.to_string().contains("replay state root"),
        "{replay_error}"
    );
    store
        .write_blocks(&blocks_before_replay_tamper)
        .expect("restore replay tampered blocks");
    let archived_batches = batch_archive(BatchArchiveQueryOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some("transparent".to_string()),
        batch_id: None,
        limit: None,
    })
    .expect("query batch archive");
    assert_eq!(archived_batches.len(), 2);
    let latest_archived_batches = batch_archive(BatchArchiveQueryOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some("transparent".to_string()),
        batch_id: None,
        limit: Some(1),
    })
    .expect("query latest batch archive");
    assert_eq!(latest_archived_batches.len(), 1);
    assert_eq!(
        latest_archived_batches[0].batch_id,
        block_log[1].header.batch_id
    );
    let exact_archived_batch = batch_archive(BatchArchiveQueryOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some("transparent".to_string()),
        batch_id: Some(block_log[1].header.batch_id.clone()),
        limit: None,
    })
    .expect("query exact batch archive");
    assert_eq!(
        exact_archived_batch,
        vec![latest_archived_batches[0].clone()]
    );
    let mut archive_with_tampered_payload = archive.clone();
    let tampered_archive_entry = archive_with_tampered_payload
        .batches
        .iter_mut()
        .find(|entry| {
            entry.batch_kind == "transparent" && entry.batch_id == block_log[1].header.batch_id
        })
        .expect("find transparent archive entry");
    tampered_archive_entry.payload_json = tampered_archive_entry
        .payload_json
        .replace(&block_log[1].header.batch_id, "tampered-batch-id");
    tampered_archive_entry.payload_hash = batch_archive_payload_hash(
        &genesis,
        &tampered_archive_entry.batch_kind,
        &tampered_archive_entry.batch_id,
        &tampered_archive_entry.payload_json,
    )
    .expect("tampered archive hash");
    store
        .write_batch_archive(&archive_with_tampered_payload)
        .expect("write tampered archive");
    let archive_payload_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("tampered archive payload should fail verification");
    let archive_payload_error = archive_payload_error.to_string();
    assert!(
        archive_payload_error.contains("archived transparent payload invalid")
            || archive_payload_error.contains("archived payload batch id mismatch"),
        "{archive_payload_error}"
    );
    store
        .write_batch_archive(&archive)
        .expect("restore batch archive");
    let mut archive_with_duplicate = archive.clone();
    archive_with_duplicate
        .batches
        .push(archive_with_duplicate.batches[0].clone());
    store
        .write_batch_archive(&archive_with_duplicate)
        .expect("write duplicate archive");
    let duplicate_archive_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("duplicate archive entry should fail verification");
    assert!(
        duplicate_archive_error
            .to_string()
            .contains("duplicate archived batch"),
        "{duplicate_archive_error}"
    );
    store
        .write_batch_archive(&archive)
        .expect("restore batch archive after duplicate");
    let mut archive_with_orphan = archive.clone();
    let mut orphan_archive_entry = archive_with_orphan.batches[0].clone();
    orphan_archive_entry.batch_id = "orphan-batch-id".to_string();
    archive_with_orphan.batches.push(orphan_archive_entry);
    store
        .write_batch_archive(&archive_with_orphan)
        .expect("write orphan archive");
    let orphan_archive_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("orphan archive entry should fail verification");
    assert!(
        orphan_archive_error
            .to_string()
            .contains("has no matching block"),
        "{orphan_archive_error}"
    );
    store
        .write_batch_archive(&archive)
        .expect("restore batch archive after orphan");
    let ordered_batches = store.read_ordered_batches().expect("ordered batches");
    let mut tampered_ordered_batches = ordered_batches.clone();
    tampered_ordered_batches[1] = "tampered-batch-id".to_string();
    store
        .write_ordered_batches(&tampered_ordered_batches)
        .expect("write tampered ordered batches");
    let ordered_journal_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("tampered ordered batch journal should fail verification");
    assert!(
        ordered_journal_error
            .to_string()
            .contains("ordered batch id mismatch"),
        "{ordered_journal_error}"
    );
    store
        .write_ordered_batches(&ordered_batches)
        .expect("restore ordered batches");
    let receipts_before_tamper = store.read_receipts().expect("receipts before tamper");
    let mut receipts_missing_block_entry = receipts_before_tamper.clone();
    receipts_missing_block_entry.retain(|receipt| receipt.tx_id != mempool_receipts[0].tx_id);
    store
        .write_receipts(&receipts_missing_block_entry)
        .expect("write tampered receipts");
    let receipt_link_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("missing block receipt should fail verification");
    assert!(
        receipt_link_error
            .to_string()
            .contains("references missing receipt"),
        "{receipt_link_error}"
    );
    store
        .write_receipts(&receipts_before_tamper)
        .expect("restore receipts");
    let mut receipts_with_orphan = receipts_before_tamper.clone();
    receipts_with_orphan.push(Receipt::accepted(
        "orphan-receipt-id",
        "receipt not linked from any block",
    ));
    store
        .write_receipts(&receipts_with_orphan)
        .expect("write orphan receipt");
    let orphan_receipt_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("orphan receipt should fail verification");
    assert!(
        orphan_receipt_error
            .to_string()
            .contains("appears 1 time(s) in receipts but 0 time(s) in blocks"),
        "{orphan_receipt_error}"
    );
    store
        .write_receipts(&receipts_before_tamper)
        .expect("restore receipts after orphan");

    let governance_amendment_file = data_dir.join("validator-set-amendment-2.json");
    let governance_batch_file = data_dir.join("governance-batch.json");
    let governance_amendment = ratify_validator_set(RatifyValidatorSetOptions {
        data_dir: data_dir.clone(),
        validators: vec!["validator-0".to_string()],
        support: vec!["validator-0".to_string()],
        validator_count: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: governance_amendment_file.clone(),
    })
    .expect("ratify governance amendment");
    let genesis = store
        .read_genesis()
        .expect("read genesis for governance test");
    assert_eq!(governance_amendment.chain_id, genesis.chain_id);
    assert_eq!(governance_amendment.genesis_hash, genesis_hash(&genesis));
    assert_eq!(
        governance_amendment.protocol_version,
        genesis.protocol_version
    );
    let tampered_governance_amendment_file =
        data_dir.join("validator-set-amendment-domain-tampered.json");
    let mut tampered_governance_amendment = governance_amendment.clone();
    tampered_governance_amendment.genesis_hash = "other-genesis".to_string();
    write_amendment_file(
        &tampered_governance_amendment_file,
        &tampered_governance_amendment,
    )
    .expect("write tampered governance amendment");
    let governance_amendment_domain_error = create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(tampered_governance_amendment_file),
        registry_update_file: None,
        batch_file: data_dir.join("governance-batch-domain-tampered.json"),
    })
    .expect_err("wrong governance amendment domain should fail");
    assert!(
        governance_amendment_domain_error
            .to_string()
            .contains("governance amendment domain mismatch"),
        "{governance_amendment_domain_error}"
    );
    let tampered_governance_vote_file =
        data_dir.join("validator-set-amendment-vote-tampered.json");
    let mut tampered_governance_vote = governance_amendment.clone();
    tampered_governance_vote.votes[0].vote_id = "tampered-cobalt-vote".to_string();
    write_amendment_file(&tampered_governance_vote_file, &tampered_governance_vote)
        .expect("write tampered governance vote");
    let governance_vote_error = create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(tampered_governance_vote_file),
        registry_update_file: None,
        batch_file: data_dir.join("governance-batch-vote-tampered.json"),
    })
    .expect_err("wrong Cobalt vote evidence should fail");
    assert!(
        governance_vote_error
            .to_string()
            .contains("governance amendment vote id mismatch"),
        "{governance_vote_error}"
    );
    let governance_batch = create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(governance_amendment_file),
        registry_update_file: None,
        batch_file: governance_batch_file.clone(),
    })
    .expect("create governance batch");
    assert_eq!(governance_batch.amendments, vec![governance_amendment]);
    let tampered_governance_batch_file = data_dir.join("governance-batch-tampered.json");
    let mut tampered_governance_batch = governance_batch.clone();
    tampered_governance_batch.batch_id = "tampered-governance-batch-id".to_string();
    write_governance_action_batch_file(
        &tampered_governance_batch_file,
        &tampered_governance_batch,
    )
    .expect("write tampered governance batch");
    let governance_batch_id_error = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: tampered_governance_batch_file,

        certificate_file: None,
    })
    .expect_err("tampered governance batch id should fail");
    assert!(
        governance_batch_id_error
            .to_string()
            .contains("governance batch id mismatch"),
        "{governance_batch_id_error}"
    );
    let governance_receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: governance_batch_file,

        certificate_file: None,
    })
    .expect("apply governance batch");
    assert!(governance_receipts[0].accepted, "{governance_receipts:?}");
    assert_eq!(
        status(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("status after governance batch")
        .validator_count,
        2
    );
    let governance_report = verify_governance(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify governance state");
    assert!(governance_report.verified);
    assert_eq!(governance_report.cobalt_mode, "canonical");
    assert_eq!(governance_report.trust_graph_root, None);
    assert_eq!(governance_report.amendment_count, 1);
    let explicit_canonical_report = verify_governance_with_options(GovernanceVerifyOptions {
        data_dir: data_dir.clone(),
        cobalt_mode: "canonical".to_string(),
        trust_graph_root: None,
    })
    .expect("explicit canonical mode verifies governance state");
    assert!(explicit_canonical_report.verified);
    assert_eq!(explicit_canonical_report.cobalt_mode, "canonical");
    let nonuniform_missing_root = verify_governance_with_options(GovernanceVerifyOptions {
        data_dir: data_dir.clone(),
        cobalt_mode: "non-uniform".to_string(),
        trust_graph_root: None,
    })
    .expect_err("non-uniform mode requires an explicit trust graph root");
    assert!(
        nonuniform_missing_root
            .to_string()
            .contains("requires --trust-graph-root"),
        "{nonuniform_missing_root}"
    );
    let nonuniform_canonical_evidence = verify_governance_with_options(
        GovernanceVerifyOptions {
            data_dir: data_dir.clone(),
            cobalt_mode: "non-uniform".to_string(),
            trust_graph_root: Some("a".repeat(96)),
        },
    )
    .expect_err("non-uniform mode rejects canonical governance evidence");
    assert!(
        nonuniform_canonical_evidence
            .to_string()
            .contains("canonical governance amendment evidence"),
        "{nonuniform_canonical_evidence}"
    );
    let governance_before_tamper = store.read_governance().expect("governance before tamper");
    let mut governance_with_tampered_vote = governance_before_tamper.clone();
    governance_with_tampered_vote.amendments[0].votes[0].vote_id =
        "persisted-tampered-cobalt-vote".to_string();
    store
        .write_governance(&governance_with_tampered_vote)
        .expect("write tampered governance state");
    let governance_verification_error = verify_governance(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("persisted tampered governance state should fail");
    assert!(
        governance_verification_error
            .to_string()
            .contains("governance amendment vote id mismatch"),
        "{governance_verification_error}"
    );
    store
        .write_governance(&governance_before_tamper)
        .expect("restore governance after tamper");

    let shield_mint_batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::Mint(ShieldMintAction {
            owner: "forbidden".to_string(),
            asset_id: "POSTFIAT".to_string(),
            amount: 77,
            memo: "live cleartext mint must remain disabled".to_string(),
        })],
    )
    .expect("build legacy fixture for batch-id validation");
    let tampered_shield_mint_batch_file = data_dir.join("shield-mint-batch-tampered.json");
    let mut tampered_shield_mint_batch = shield_mint_batch.clone();
    tampered_shield_mint_batch.batch_id = "tampered-shielded-batch-id".to_string();
    write_shielded_action_batch_file(
        &tampered_shield_mint_batch_file,
        &tampered_shield_mint_batch,
    )
    .expect("write tampered shielded batch");
    let shielded_batch_id_error = apply_shielded_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: tampered_shield_mint_batch_file,

        certificate_file: None,
    })
    .expect_err("tampered shielded batch id should fail");
    assert!(
        shielded_batch_id_error
            .to_string()
            .contains("shielded batch id mismatch"),
        "{shielded_batch_id_error}"
    );
    let mint_error = create_shielded_mint_batch(ShieldMintBatchOptions {
        data_dir: data_dir.clone(),
        owner: "dave".to_string(),
        asset_id: "POSTFIAT".to_string(),
        amount: 77,
        memo: "live mint forbidden".to_string(),
        batch_file: data_dir.join("forbidden-shield-mint-batch.json"),
    })
    .expect_err("legacy cleartext mint construction must remain disabled");
    assert_eq!(mint_error.kind(), io::ErrorKind::PermissionDenied);
    let spend_error = create_shielded_spend_batch(ShieldSpendBatchOptions {
        data_dir: data_dir.clone(),
        note_id: "historical-note".to_string(),
        to: "erin".to_string(),
        amount: 40,
        memo: "ordered spend".to_string(),
        batch_file: data_dir.join("shield-spend-batch.json"),
    })
    .expect_err("legacy cleartext spend construction must remain disabled");
    assert_eq!(spend_error.kind(), io::ErrorKind::PermissionDenied);

    let shielded_report = verify_shielded(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify shielded state");
    assert!(shielded_report.verified);
    assert_eq!(shielded_report.turnstile_event_count, 0);
    assert_eq!(shielded_report.bootstrap_deposit_total, 0);
    assert_eq!(shielded_report.migration_total, 0);
    assert_eq!(shielded_report.spent_note_count, 0);
    assert_eq!(shielded_report.live_note_count, 0);
    assert_eq!(shielded_report.tree_root.len(), 96);

    let shielded_before_tamper = store.read_shielded().expect("shielded before tamper");
    let mut shielded_with_tampered_note = shielded_before_tamper.clone();
    shielded_with_tampered_note
        .nullifiers
        .push("orphan-tampered-nullifier".to_string());
    store
        .write_shielded(&shielded_with_tampered_note)
        .expect("write tampered shielded note");
    let shielded_verification_error = verify_shielded(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("persisted orphan shielded nullifier should fail");
    assert!(
        shielded_verification_error
            .to_string()
            .contains("does not match a persisted note"),
        "{shielded_verification_error}"
    );
    store
        .write_shielded(&shielded_before_tamper)
        .expect("restore shielded after tamper");

    let bridge_domain_batch_file = data_dir.join("bridge-domain-batch.json");
    let bridge_domain_batch = create_bridge_domain_batch(BridgeDomainBatchOptions {
        data_dir: data_dir.clone(),
        domain_id: "batched-bridge".to_string(),
        name: "Batched Bridge".to_string(),
        source_chain: "xrpl-devnet".to_string(),
        target_chain: "postfiat-local".to_string(),
        bridge_id: "batched-bridge".to_string(),
        door_account: "door:batched-bridge".to_string(),
        inbound_cap: 30,
        outbound_cap: 30,
        batch_file: bridge_domain_batch_file.clone(),
    })
    .expect("create bridge domain batch");
    assert_eq!(bridge_domain_batch.actions.len(), 1);
    let tampered_bridge_domain_batch_file = data_dir.join("bridge-domain-batch-tampered.json");
    let mut tampered_bridge_domain_batch = bridge_domain_batch.clone();
    tampered_bridge_domain_batch.batch_id = "tampered-bridge-batch-id".to_string();
    write_bridge_action_batch_file(
        &tampered_bridge_domain_batch_file,
        &tampered_bridge_domain_batch,
    )
    .expect("write tampered bridge batch");
    let bridge_batch_id_error = apply_bridge_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: tampered_bridge_domain_batch_file,

        certificate_file: None,
    })
    .expect_err("tampered bridge batch id should fail");
    assert!(
        bridge_batch_id_error
            .to_string()
            .contains("bridge batch id mismatch"),
        "{bridge_batch_id_error}"
    );
    let domain_receipts = apply_bridge_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: bridge_domain_batch_file,

        certificate_file: None,
    })
    .expect("apply bridge domain batch");
    assert!(domain_receipts[0].accepted, "{domain_receipts:?}");

    let bridge_transfer_batch_file = data_dir.join("bridge-transfer-batch.json");
    create_bridge_transfer_batch(BridgeTransferBatchOptions {
        data_dir: data_dir.clone(),
        domain_id: "batched-bridge".to_string(),
        direction: "inbound".to_string(),
        from: "external:batched".to_string(),
        to: "pfbatched".to_string(),
        asset_id: "POSTFIAT".to_string(),
        amount: 10,
        witness_id: "batched-witness-1".to_string(),
        witness_epoch: None,
        witness_signer: DEFAULT_BRIDGE_WITNESS_SIGNER.to_string(),
        batch_file: bridge_transfer_batch_file.clone(),
    })
    .expect("create bridge transfer batch");
    let bridge_receipts = apply_bridge_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: bridge_transfer_batch_file,

        certificate_file: None,
    })
    .expect("apply bridge transfer batch");
    assert!(bridge_receipts[0].accepted, "{bridge_receipts:?}");
    let bridge_after_transfer = bridge_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("bridge state after transfer");
    let bridge_transfer = bridge_after_transfer
        .transfers
        .last()
        .expect("accepted bridge transfer");
    assert_eq!(bridge_transfer.source_chain, "xrpl-devnet");
    assert_eq!(bridge_transfer.target_chain, "postfiat-local");
    assert_eq!(bridge_transfer.bridge_id, "batched-bridge");
    assert_eq!(bridge_transfer.door_account, "door:batched-bridge");
    let attestation = bridge_transfer
        .witness_attestation
        .as_ref()
        .expect("bridge witness attestation");
    assert_eq!(attestation.signer, DEFAULT_BRIDGE_WITNESS_SIGNER);
    assert_eq!(attestation.algorithm_id, ML_DSA_65_ALGORITHM);
    assert!(!attestation.attestation_id.is_empty());
    assert!(!attestation.signature_hex.is_empty());

    let bridge_duplicate_batch_file = data_dir.join("bridge-duplicate-batch.json");
    create_bridge_transfer_batch(BridgeTransferBatchOptions {
        data_dir: data_dir.clone(),
        domain_id: "batched-bridge".to_string(),
        direction: "inbound".to_string(),
        from: "external:batched".to_string(),
        to: "pfbatched-replay".to_string(),
        asset_id: "POSTFIAT".to_string(),
        amount: 1,
        witness_id: "batched-witness-1".to_string(),
        witness_epoch: None,
        witness_signer: DEFAULT_BRIDGE_WITNESS_SIGNER.to_string(),
        batch_file: bridge_duplicate_batch_file.clone(),
    })
    .expect("create duplicate bridge batch");
    let bridge_duplicate_receipts = apply_bridge_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: bridge_duplicate_batch_file,

        certificate_file: None,
    })
    .expect("apply duplicate bridge batch");
    assert!(
        !bridge_duplicate_receipts[0].accepted,
        "{bridge_duplicate_receipts:?}"
    );
    assert_eq!(bridge_duplicate_receipts[0].code, "duplicate_witness");

    let source_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("source status");
    let source_store = NodeStore::new(&data_dir);
    let source_native_total = native_pft_live_total(
        &source_store.read_ledger().expect("source ledger"),
        &source_store.read_shielded().expect("source shielded state"),
    )
    .expect("source native live total");
    let snapshot_dir = data_dir.with_file_name("postfiat-node-test-snapshot");
    let restored_dir = data_dir.with_file_name("postfiat-node-test-restored");
    let manifest = export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export snapshot");
    assert_eq!(manifest.state_root, source_status.state_root);
    assert_eq!(manifest.chain_id, source_status.chain_id);
    assert_eq!(manifest.genesis_hash, source_status.genesis_hash);
    assert_eq!(manifest.protocol_version, source_status.protocol_version);
    assert_eq!(manifest.block_height, source_status.block_height);
    assert_eq!(manifest.block_tip_hash, source_status.block_tip_hash);
    assert!(manifest.files.iter().any(|file| file.name == LEDGER_FILE));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.name == FAUCET_ACCOUNT_FILE));
    assert!(!manifest
        .files
        .iter()
        .any(|file| file.name == FAUCET_KEY_FILE));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.name == VALIDATOR_REGISTRY_FILE));
    assert!(manifest
        .files
        .iter()
        .any(|file| file.name == VALIDATOR_REGISTRY_GENESIS_FILE));
    assert!(!manifest
        .files
        .iter()
        .any(|file| file.name == VALIDATOR_KEYS_FILE));
    assert!(snapshot_dir.join(FAUCET_ACCOUNT_FILE).exists());
    assert!(!snapshot_dir.join(FAUCET_KEY_FILE).exists());
    assert!(snapshot_dir.join(VALIDATOR_REGISTRY_FILE).exists());
    assert!(snapshot_dir.join(VALIDATOR_REGISTRY_GENESIS_FILE).exists());
    assert!(!snapshot_dir.join(VALIDATOR_KEYS_FILE).exists());

    let snapshot_manifest_path = snapshot_dir.join(SNAPSHOT_MANIFEST_FILE);
    let domain_mismatch_restored_dir =
        data_dir.with_file_name("postfiat-node-test-domain-mismatch-restored");
    let mut domain_mismatch_manifest = manifest.clone();
    domain_mismatch_manifest.genesis_hash = "tampered-genesis-hash".to_string();
    write_snapshot_manifest(&snapshot_manifest_path, &domain_mismatch_manifest)
        .expect("write domain mismatch manifest");
    let domain_mismatch_error = import_snapshot(SnapshotImportOptions {
        data_dir: domain_mismatch_restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: None,
    })
    .expect_err("snapshot genesis hash mismatch should fail import");
    assert!(
        domain_mismatch_error
            .to_string()
            .contains("restored genesis hash"),
        "{domain_mismatch_error}"
    );
    write_snapshot_manifest(&snapshot_manifest_path, &manifest).expect("restore manifest");

    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_string()),
    })
    .expect("import snapshot");
    assert_eq!(restored.state_root, source_status.state_root);
    assert_eq!(restored.block_height, source_status.block_height);
    assert_eq!(restored.block_tip_hash, source_status.block_tip_hash);
    assert_eq!(restored.node_id, "validator-restored");
    let restored_store = NodeStore::new(&restored_dir);
    assert_eq!(
        native_pft_live_total(
            &restored_store.read_ledger().expect("restored ledger"),
            &restored_store
                .read_shielded()
                .expect("restored shielded state"),
        )
        .expect("restored native live total"),
        source_native_total,
        "snapshot restore must preserve every native custody lane exactly"
    );
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("restored snapshot preserves native supply and replay");

    std::fs::remove_dir_all(data_dir).expect("cleanup");
    std::fs::remove_dir_all(snapshot_dir).expect("cleanup snapshot");
    std::fs::remove_dir_all(restored_dir).expect("cleanup restored");
    if domain_mismatch_restored_dir.exists() {
        std::fs::remove_dir_all(domain_mismatch_restored_dir)
            .expect("cleanup domain mismatch restored");
    }
}

#[test]
fn signed_snapshot_roundtrip_rejects_tampering_and_preserves_signer_isolation() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let data_dir =
        std::env::temp_dir().join(format!("postfiat-signed-snapshot-source-{unique}"));
    let snapshot_dir =
        std::env::temp_dir().join(format!("postfiat-signed-snapshot-artifact-{unique}"));
    let restored_dir =
        std::env::temp_dir().join(format!("postfiat-signed-snapshot-restored-{unique}"));
    let rejected_dir =
        std::env::temp_dir().join(format!("postfiat-signed-snapshot-rejected-{unique}"));
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 3,
    })
    .expect("init signed snapshot source");

    let publisher_key_file = data_dir.join("snapshot-publisher.private.json");
    let publisher_key = create_dev_key_file().expect("create snapshot publisher key");
    write_key_file(&publisher_key_file, &publisher_key)
        .expect("write snapshot publisher key");
    let trusted_key_file = data_dir.join("snapshot-publisher.public.json");
    let trusted = export_snapshot_publisher_public_key(
        SnapshotPublisherKeyExportOptions {
            publisher_key_file: publisher_key_file.clone(),
            public_key_file: trusted_key_file.clone(),
        },
    )
    .expect("export trusted snapshot publisher key");
    let signed = export_signed_snapshot(SignedSnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        publisher_key_file,
    })
    .expect("export signed snapshot");
    assert_eq!(signed.publisher, trusted.publisher);
    assert!(!snapshot_dir.join(VALIDATOR_KEYS_FILE).exists());
    assert!(!snapshot_dir.join(FAUCET_KEY_FILE).exists());
    assert!(!snapshot_dir
        .join("snapshot-publisher.private.json")
        .exists());

    let restored = import_signed_snapshot(SignedSnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        trusted_publisher_key_file: trusted_key_file.clone(),
        node_id: Some("validator-replacement".to_string()),
    })
    .expect("import signed snapshot");
    assert_eq!(restored.state_root, signed.manifest.state_root);
    assert_eq!(restored.node_id, "validator-replacement");
    assert!(!restored_dir.join(VALIDATOR_KEYS_FILE).exists());

    let signed_file = snapshot_dir.join(SIGNED_SNAPSHOT_MANIFEST_FILE);
    let original = std::fs::read(&signed_file).expect("read signed manifest");
    let mut tampered: SignedSnapshotManifest =
        serde_json::from_slice(&original).expect("parse signed manifest");
    tampered.manifest.block_height = tampered.manifest.block_height.saturating_add(1);
    atomic_write(
        &signed_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&tampered).expect("tampered signed manifest json")
        ),
    )
    .expect("write tampered signed manifest");
    let error = import_signed_snapshot(SignedSnapshotImportOptions {
        data_dir: rejected_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        trusted_publisher_key_file: trusted_key_file,
        node_id: Some("validator-rejected".to_string()),
    })
    .expect_err("tampered signed snapshot must fail");
    assert!(error.to_string().contains("signature verification"), "{error}");

    atomic_write(&signed_file, original).expect("restore signed manifest");
    for path in [data_dir, snapshot_dir, restored_dir, rejected_dir] {
        if path.exists() {
            std::fs::remove_dir_all(path).expect("cleanup signed snapshot test");
        }
    }
}

#[test]
fn snapshot_v5_restores_only_never_activated_consensus_v2_genesis() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("postfiat-snapshot-v5-migration-{unique}"));
    let legacy_source = root.join("legacy-source");
    let legacy_snapshot = root.join("legacy-snapshot");
    let legacy_restored = root.join("legacy-restored");
    init(InitOptions {
        data_dir: legacy_source.clone(),
        chain_id: "postfiat-snapshot-v5-legacy".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init legacy source");
    let mut manifest = export_snapshot(SnapshotExportOptions {
        data_dir: legacy_source,
        snapshot_dir: legacy_snapshot.clone(),
    })
    .expect("export v6-compatible legacy source");
    manifest.snapshot_version = LEGACY_SNAPSHOT_VERSION;
    manifest.files.retain(|file| {
        file.name != CONSENSUS_V2_SAFETY_SNAPSHOT_FILE
            && file.name != CONSENSUS_V2_QC_SNAPSHOT_FILE
            && file.name != OWNED_LOCKS_FILE
            && file.name != OWNED_LOCKS_WAL_FILE
            && file.name != FASTPAY_SPECULATIVE_JOURNAL_FILE
    });
    for name in [
        CONSENSUS_V2_SAFETY_SNAPSHOT_FILE,
        CONSENSUS_V2_QC_SNAPSHOT_FILE,
        OWNED_LOCKS_FILE,
        OWNED_LOCKS_WAL_FILE,
        FASTPAY_SPECULATIVE_JOURNAL_FILE,
    ] {
        std::fs::remove_file(legacy_snapshot.join(name)).expect("remove v6-only artifact");
    }
    write_snapshot_manifest(
        &legacy_snapshot.join(SNAPSHOT_MANIFEST_FILE),
        &manifest,
    )
    .expect("write v5 manifest");
    import_snapshot(SnapshotImportOptions {
        data_dir: legacy_restored,
        snapshot_dir: legacy_snapshot,
        node_id: None,
    })
    .expect("v5 never-activated restore");

    let active_source = root.join("active-source");
    let active_snapshot = root.join("active-snapshot");
    init_consensus_v2(InitConsensusV2Options {
        data_dir: active_source.clone(),
        chain_id: "postfiat-snapshot-v5-active".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
        activation_height: 1,
    })
    .expect("init active source");
    let mut active_manifest = export_snapshot(SnapshotExportOptions {
        data_dir: active_source,
        snapshot_dir: active_snapshot.clone(),
    })
    .expect("export active source");
    active_manifest.snapshot_version = LEGACY_SNAPSHOT_VERSION;
    active_manifest.files.retain(|file| {
        file.name != CONSENSUS_V2_SAFETY_SNAPSHOT_FILE
            && file.name != CONSENSUS_V2_QC_SNAPSHOT_FILE
            && file.name != OWNED_LOCKS_FILE
            && file.name != OWNED_LOCKS_WAL_FILE
            && file.name != FASTPAY_SPECULATIVE_JOURNAL_FILE
    });
    for name in [
        CONSENSUS_V2_SAFETY_SNAPSHOT_FILE,
        CONSENSUS_V2_QC_SNAPSHOT_FILE,
        OWNED_LOCKS_FILE,
        OWNED_LOCKS_WAL_FILE,
        FASTPAY_SPECULATIVE_JOURNAL_FILE,
    ] {
        std::fs::remove_file(active_snapshot.join(name)).expect("remove v6-only artifact");
    }
    write_snapshot_manifest(
        &active_snapshot.join(SNAPSHOT_MANIFEST_FILE),
        &active_manifest,
    )
    .expect("write active v5 manifest");
    let error = import_snapshot(SnapshotImportOptions {
        data_dir: root.join("active-rejected"),
        snapshot_dir: active_snapshot,
        node_id: None,
    })
    .expect_err("v5 must not restore an activated consensus v2 signer");
    assert!(
        error
            .to_string()
            .contains("legacy snapshot cannot restore an activated consensus v2 signer"),
        "{error}"
    );
    std::fs::remove_dir_all(root).expect("cleanup v5 migration test");
}

#[test]
fn snapshot_import_rejects_nonempty_destination_before_any_mutation() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("postfiat-snapshot-no-overlay-{unique}"));
    let source = root.join("source");
    let snapshot = root.join("snapshot");
    let destination = root.join("destination");
    init(InitOptions {
        data_dir: source.clone(),
        chain_id: "postfiat-snapshot-source".to_string(),
        node_id: "validator-source".to_string(),
        validator_count: 1,
    })
    .expect("init snapshot source");
    export_snapshot(SnapshotExportOptions {
        data_dir: source,
        snapshot_dir: snapshot.clone(),
    })
    .expect("export snapshot source");
    init(InitOptions {
        data_dir: destination.clone(),
        chain_id: "postfiat-snapshot-existing".to_string(),
        node_id: "validator-existing".to_string(),
        validator_count: 1,
    })
    .expect("init nonempty destination");
    let genesis_path = destination.join(GENESIS_FILE);
    let genesis_before = std::fs::read(&genesis_path).expect("read destination genesis");

    let error = import_snapshot(SnapshotImportOptions {
        data_dir: destination.clone(),
        snapshot_dir: snapshot,
        node_id: None,
    })
    .expect_err("snapshot import must reject an overlay destination");
    assert!(
        error
            .to_string()
            .contains("snapshot import destination must not already exist"),
        "{error}"
    );
    assert_eq!(
        std::fs::read(genesis_path).expect("reread destination genesis"),
        genesis_before,
        "a rejected snapshot overlay must not mutate existing state"
    );
    std::fs::remove_dir_all(root).expect("cleanup no-overlay snapshot test");
}

#[test]
fn signed_deployment_manifest_rejects_tampering_expiry_and_wrong_publisher() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("postfiat-deployment-manifest-{unique}"));
    std::fs::create_dir_all(&root).expect("create deployment manifest root");
    let generic_key_file = root.join("generic.private.json");
    let generic_key = create_dev_key_file().expect("create generic development key");
    write_key_file(&generic_key_file, &generic_key).expect("write generic development key");
    let generic_error = export_deployment_publisher_public_key(
        DeploymentPublisherKeyExportOptions {
            publisher_key_file: generic_key_file,
            public_key_file: root.join("generic.public.json"),
        },
    )
    .expect_err("generic development key must not become a deployment publisher key");
    assert!(
        generic_error.to_string().contains("deployment publisher key"),
        "{generic_error}"
    );

    let publisher_key_file = root.join("publisher.private.json");
    let publisher = create_deployment_publisher_private_key(
        DeploymentPublisherKeyCreateOptions {
            publisher_key_file: publisher_key_file.clone(),
        },
    )
    .expect("create dedicated deployment publisher key");
    let publisher_key_bytes =
        std::fs::read(&publisher_key_file).expect("read dedicated deployment publisher key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&publisher_key_file)
                .expect("deployment publisher key metadata")
                .permissions()
                .mode()
                & 0o777,
            0o600,
            "deployment publisher key must be private"
        );
    }
    let overwrite_error = create_deployment_publisher_private_key(
        DeploymentPublisherKeyCreateOptions {
            publisher_key_file: publisher_key_file.clone(),
        },
    )
    .expect_err("deployment publisher key creation must not overwrite an existing key");
    assert_eq!(overwrite_error.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(
        std::fs::read(&publisher_key_file).expect("re-read deployment publisher key"),
        publisher_key_bytes,
        "refused key creation must leave the original private key unchanged"
    );

    let mut wrong_purpose = read_deployment_publisher_private_key(&publisher_key_file)
        .expect("read dedicated deployment publisher key");
    wrong_purpose.purpose = "validator-signing".to_string();
    let wrong_purpose_key_file = root.join("wrong-purpose.private.json");
    atomic_write(
        &wrong_purpose_key_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&wrong_purpose)
                .expect("serialize wrong-purpose deployment key")
        ),
    )
    .expect("write wrong-purpose deployment key");
    set_private_file_permissions(&wrong_purpose_key_file)
        .expect("protect wrong-purpose deployment key");
    let wrong_purpose_error = export_deployment_publisher_public_key(
        DeploymentPublisherKeyExportOptions {
            publisher_key_file: wrong_purpose_key_file,
            public_key_file: root.join("wrong-purpose.public.json"),
        },
    )
    .expect_err("wrong-purpose deployment key must fail");
    assert!(
        wrong_purpose_error
            .to_string()
            .contains("unsupported schema, purpose, or algorithm"),
        "{wrong_purpose_error}"
    );

    let mut wrong_schema = read_deployment_publisher_private_key(&publisher_key_file)
        .expect("read dedicated deployment publisher key for schema mutation");
    wrong_schema.schema = "postfiat.validator_private_key.v1".to_string();
    let wrong_schema_key_file = root.join("wrong-schema.private.json");
    atomic_write(
        &wrong_schema_key_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&wrong_schema)
                .expect("serialize wrong-schema deployment key")
        ),
    )
    .expect("write wrong-schema deployment key");
    set_private_file_permissions(&wrong_schema_key_file)
        .expect("protect wrong-schema deployment key");
    let wrong_schema_error = export_deployment_publisher_public_key(
        DeploymentPublisherKeyExportOptions {
            publisher_key_file: wrong_schema_key_file,
            public_key_file: root.join("wrong-schema.public.json"),
        },
    )
    .expect_err("wrong-schema deployment key must fail");
    assert!(
        wrong_schema_error
            .to_string()
            .contains("unsupported schema, purpose, or algorithm"),
        "{wrong_schema_error}"
    );

    let mut invalid_material = read_deployment_publisher_private_key(&publisher_key_file)
        .expect("read dedicated deployment publisher key for material mutation");
    invalid_material.private_key_hex = generic_key.private_key_hex.clone();
    let invalid_material_key_file = root.join("invalid-material.private.json");
    atomic_write(
        &invalid_material_key_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&invalid_material)
                .expect("serialize invalid-material deployment key")
        ),
    )
    .expect("write invalid-material deployment key");
    set_private_file_permissions(&invalid_material_key_file)
        .expect("protect invalid-material deployment key");
    let invalid_material_error = export_deployment_publisher_public_key(
        DeploymentPublisherKeyExportOptions {
            publisher_key_file: invalid_material_key_file,
            public_key_file: root.join("invalid-material.public.json"),
        },
    )
    .expect_err("private material from another key must fail");
    assert!(
        invalid_material_error
            .to_string()
            .contains("private key does not match public key"),
        "{invalid_material_error}"
    );

    let trusted_key_file = root.join("publisher.public.json");
    let trusted = export_deployment_publisher_public_key(DeploymentPublisherKeyExportOptions {
        publisher_key_file: publisher_key_file.clone(),
        public_key_file: trusted_key_file.clone(),
    })
    .expect("export deployment publisher key");
    assert_eq!(trusted, publisher);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&trusted_key_file)
                .expect("deployment publisher public-key metadata")
                .permissions()
                .mode()
                & 0o777,
            0o644,
            "the service user must be able to read the public trust anchor"
        );
    }

    let binary = root.join("postfiat-node");
    let unit = root.join("postfiat.service");
    let environment = root.join("postfiat.env");
    let transport_unit = root.join("postfiat.transport.service");
    let transport_environment = root.join("postfiat.transport.env");
    let topology = root.join("topology.json");
    let validator_bindings = root.join("validator-bindings.json");
    let swap_metadata = root.join("swap.metadata.json");
    let egress_metadata = root.join("egress.metadata.json");
    for (path, contents) in [
        (&binary, b"binary-v1".as_slice()),
        (&unit, b"unit-v1".as_slice()),
        (&environment, b"environment-v1".as_slice()),
        (&transport_unit, b"transport-unit-v1".as_slice()),
        (&transport_environment, b"transport-environment-v1".as_slice()),
        (&swap_metadata, b"swap-metadata-v1".as_slice()),
        (&egress_metadata, b"egress-metadata-v1".as_slice()),
    ] {
        atomic_write(path, contents).expect("write deployment input");
    }
    let mut topology_peers = Vec::new();
    let mut binding_entries = Vec::new();
    for validator_index in 0_u16..6 {
        let validator_id = format!("validator-{validator_index}");
        let (rpc_unit, rpc_environment, binding_transport_unit, binding_transport_environment) =
            if validator_index == 0 {
                (
                    unit.clone(),
                    environment.clone(),
                    transport_unit.clone(),
                    transport_environment.clone(),
                )
            } else {
                let rpc_unit = root.join(format!("{validator_id}.rpc.service"));
                let rpc_environment = root.join(format!("{validator_id}.rpc.env"));
                let transport_unit = root.join(format!("{validator_id}.transport.service"));
                let transport_environment =
                    root.join(format!("{validator_id}.transport.env"));
                for (path, contents) in [
                    (&rpc_unit, format!("{validator_id}-rpc-unit").into_bytes()),
                    (
                        &rpc_environment,
                        format!("{validator_id}-rpc-environment").into_bytes(),
                    ),
                    (
                        &transport_unit,
                        format!("{validator_id}-transport-unit").into_bytes(),
                    ),
                    (
                        &transport_environment,
                        format!("{validator_id}-transport-environment").into_bytes(),
                    ),
                ] {
                    atomic_write(path, contents)
                        .expect("write unique validator deployment input");
                }
                (
                    rpc_unit,
                    rpc_environment,
                    transport_unit,
                    transport_environment,
                )
            };
        let p2p_port = 26_650_u16 + validator_index;
        let rpc_port = 27_650_u16 + validator_index;
        topology_peers.push(serde_json::json!({
            "node_id": validator_id,
            "host": "127.0.0.1",
            "p2p_port": p2p_port,
            "rpc_port": rpc_port,
            "p2p_address": format!("127.0.0.1:{p2p_port}")
        }));
        binding_entries.push(serde_json::json!({
            "validator_id": format!("validator-{validator_index}"),
            "services": [{
                "service_id": "rpc",
                "service_unit_file": rpc_unit.to_string_lossy(),
                "environment_file": rpc_environment.to_string_lossy()
            }, {
                "service_id": "transport",
                "service_unit_file": binding_transport_unit.to_string_lossy(),
                "environment_file": binding_transport_environment.to_string_lossy()
            }]
        }));
    }
    atomic_write(
        &topology,
        format!(
            "{}\n",
            serde_json::json!({
                "topology_id": "deployment-test-topology",
                "chain_id": "postfiat-local",
                "genesis_hash": "11".repeat(48),
                "protocol_version": 1,
                "peers": topology_peers
            })
        ),
    )
    .expect("write deployment topology");
    atomic_write(
        &validator_bindings,
        format!(
            "{}\n",
            serde_json::json!({
                "schema": DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA,
                "validators": binding_entries
            })
        ),
    )
    .expect("write six-validator deployment bindings");
    let now = unix_now();
    let manifest_file = root.join("deployment-manifest.json");
    let manifest = create_deployment_manifest(DeploymentManifestCreateOptions {
        deployment_id: "test-deployment".to_string(),
        valid_from_unix: now.saturating_sub(1),
        valid_until_unix: now.saturating_add(60),
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "11".repeat(48),
        git_revision: "0123456789abcdef".to_string(),
        binary_file: binary.clone(),
        build_profile: "release".to_string(),
        build_features: vec!["transport".to_string(), "privacy".to_string()],
        protocol_version: 1,
        rpc_schema: "postfiat-local-rpc-v1".to_string(),
        service_unit_file: unit.clone(),
        environment_file: environment.clone(),
        validator_bindings_file: validator_bindings.clone(),
        topology_file: topology.clone(),
        swap_circuit_metadata_file: swap_metadata.clone(),
        private_egress_circuit_metadata_file: egress_metadata.clone(),
        publisher_key_file,
        manifest_file: manifest_file.clone(),
    })
    .expect("create signed deployment manifest");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&manifest_file)
                .expect("deployment manifest metadata")
                .permissions()
                .mode()
                & 0o777,
            0o644,
            "the service user must be able to execute the manifest preflight"
        );
    }
    assert_eq!(manifest.build_features, vec!["privacy", "transport"]);
    let expected_validator_ids = (0..6)
        .map(|validator_index| format!("validator-{validator_index}"))
        .collect::<Vec<_>>();
    assert_eq!(
        manifest
            .validator_bindings
            .iter()
            .map(|binding| binding.validator_id.clone())
            .collect::<Vec<_>>(),
        expected_validator_ids,
        "one signed manifest must bind all six distinct validators"
    );
    verify_deployment_manifest(DeploymentManifestVerifyOptions {
        manifest_file: manifest_file.clone(),
        trusted_publisher_key_file: trusted_key_file.clone(),
        now_unix: Some(now),
        validator_id: None,
        validator_bindings_file: None,
        runtime_binary_file: Some(binary.clone()),
        runtime_topology_file: Some(topology.clone()),
        runtime_swap_circuit_metadata_file: Some(swap_metadata.clone()),
        runtime_private_egress_circuit_metadata_file: Some(egress_metadata.clone()),
    })
    .expect("verify deployment manifest");
    for validator_id in &expected_validator_ids {
        verify_deployment_manifest(DeploymentManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
            trusted_publisher_key_file: trusted_key_file.clone(),
            now_unix: Some(now),
            validator_id: Some(validator_id.clone()),
            validator_bindings_file: Some(validator_bindings.clone()),
            runtime_binary_file: None,
            runtime_topology_file: None,
            runtime_swap_circuit_metadata_file: None,
            runtime_private_egress_circuit_metadata_file: None,
        })
        .expect("verify local deployment validator binding");
        let runtime_identity = deployment_runtime_identity_from_config(
            Some(manifest_file.clone().into_os_string()),
            Some(validator_id.clone().into()),
            Some(validator_bindings.clone().into_os_string()),
            Some(binary.clone().into_os_string()),
            Some(topology.clone().into_os_string()),
            Some(swap_metadata.clone().into_os_string()),
            Some(egress_metadata.clone().into_os_string()),
        )
        .expect("read local deployment runtime identity");
        assert!(runtime_identity.manifest_sha256.is_some());
        assert_eq!(
            runtime_identity.validator_id.as_deref(),
            Some(validator_id.as_str())
        );
        assert_eq!(
            runtime_identity
                .service_artifacts
                .iter()
                .map(|service| service.service_id.as_str())
                .collect::<Vec<_>>(),
            vec!["rpc", "transport"],
        );
        assert_eq!(
            runtime_identity.runtime_artifacts,
            Some(DeploymentRuntimeArtifactHashes {
                binary_sha256: manifest.binary_sha256.clone(),
                topology_sha256: manifest.topology_sha256.clone(),
                swap_circuit_metadata_sha256: manifest
                    .swap_circuit_metadata_sha256
                    .clone(),
                private_egress_circuit_metadata_sha256: manifest
                    .private_egress_circuit_metadata_sha256
                    .clone(),
            })
        );
    }
    let partial_runtime = deployment_runtime_identity_from_config(
        Some(manifest_file.clone().into_os_string()),
        Some("validator-0".into()),
        None,
        None,
        None,
        None,
        None,
    )
    .expect_err("partial runtime deployment binding must fail");
    assert!(
        partial_runtime.to_string().contains("must be configured together"),
        "{partial_runtime}"
    );
    let partial_runtime_artifacts = deployment_runtime_identity_from_config(
        Some(manifest_file.clone().into_os_string()),
        Some("validator-0".into()),
        Some(validator_bindings.clone().into_os_string()),
        Some(binary.clone().into_os_string()),
        None,
        None,
        None,
    )
    .expect_err("partial runtime artifact identity must fail");
    assert!(
        partial_runtime_artifacts
            .to_string()
            .contains("runtime artifact paths must be configured together"),
        "{partial_runtime_artifacts}"
    );

    let wrong_binary = root.join("wrong-postfiat-node");
    let wrong_topology = root.join("wrong-topology.json");
    let wrong_swap_metadata = root.join("wrong-swap.metadata.json");
    let wrong_egress_metadata = root.join("wrong-egress.metadata.json");
    for path in [
        &wrong_binary,
        &wrong_topology,
        &wrong_swap_metadata,
        &wrong_egress_metadata,
    ] {
        atomic_write(path, b"wrong-runtime-artifact")
            .expect("write mismatched runtime artifact");
    }
    let runtime_identity_mismatch = deployment_runtime_identity_from_config(
        Some(manifest_file.clone().into_os_string()),
        Some("validator-0".into()),
        Some(validator_bindings.clone().into_os_string()),
        Some(wrong_binary.clone().into_os_string()),
        Some(topology.clone().into_os_string()),
        Some(swap_metadata.clone().into_os_string()),
        Some(egress_metadata.clone().into_os_string()),
    )
    .expect_err("runtime status identity must fail on a changed binary");
    assert!(
        runtime_identity_mismatch
            .to_string()
            .contains("runtime artifacts do not match manifest"),
        "{runtime_identity_mismatch}"
    );
    for (label, runtime_binary, runtime_topology, runtime_swap, runtime_egress) in [
        (
            "binary",
            wrong_binary,
            topology.clone(),
            swap_metadata.clone(),
            egress_metadata.clone(),
        ),
        (
            "topology",
            binary.clone(),
            wrong_topology,
            swap_metadata.clone(),
            egress_metadata.clone(),
        ),
        (
            "swap metadata",
            binary.clone(),
            topology.clone(),
            wrong_swap_metadata,
            egress_metadata.clone(),
        ),
        (
            "private-egress metadata",
            binary.clone(),
            topology.clone(),
            swap_metadata.clone(),
            wrong_egress_metadata,
        ),
    ] {
        let mismatch = verify_deployment_manifest(DeploymentManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
            trusted_publisher_key_file: trusted_key_file.clone(),
            now_unix: Some(now),
            validator_id: None,
            validator_bindings_file: None,
            runtime_binary_file: Some(runtime_binary),
            runtime_topology_file: Some(runtime_topology),
            runtime_swap_circuit_metadata_file: Some(runtime_swap),
            runtime_private_egress_circuit_metadata_file: Some(runtime_egress),
        })
        .expect_err("mismatched runtime artifact must fail");
        assert!(
            mismatch
                .to_string()
                .contains("runtime artifacts do not match signed manifest"),
            "{label}: {mismatch}"
        );
    }
    let partial_runtime_verify = verify_deployment_manifest(
        DeploymentManifestVerifyOptions {
            manifest_file: manifest_file.clone(),
            trusted_publisher_key_file: trusted_key_file.clone(),
            now_unix: Some(now),
            validator_id: None,
            validator_bindings_file: None,
            runtime_binary_file: Some(binary.clone()),
            runtime_topology_file: None,
            runtime_swap_circuit_metadata_file: None,
            runtime_private_egress_circuit_metadata_file: None,
        },
    )
    .expect_err("partial runtime artifact verification must fail");
    assert!(
        partial_runtime_verify
            .to_string()
            .contains("runtime artifact files must be supplied together"),
        "{partial_runtime_verify}"
    );
    let mismatched_environment = root.join("mismatched-validator.env");
    atomic_write(&mismatched_environment, b"environment-v2")
        .expect("write mismatched environment");
    let mismatched_bindings = root.join("mismatched-validator-bindings.json");
    atomic_write(
        &mismatched_bindings,
        format!(
            "{}\n",
            serde_json::json!({
                "schema": DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA,
                "validators": [{
                    "validator_id": "validator-0",
                    "services": [{
                        "service_id": "rpc",
                        "service_unit_file": unit.to_string_lossy(),
                        "environment_file": mismatched_environment.to_string_lossy()
                    }, {
                        "service_id": "transport",
                        "service_unit_file": transport_unit.to_string_lossy(),
                        "environment_file": transport_environment.to_string_lossy()
                    }]
                }]
            })
        ),
    )
    .expect("write mismatched deployment validator bindings");
    let mismatch = verify_deployment_manifest(DeploymentManifestVerifyOptions {
        manifest_file: manifest_file.clone(),
        trusted_publisher_key_file: trusted_key_file.clone(),
        now_unix: Some(now),
        validator_id: Some("validator-0".to_string()),
        validator_bindings_file: Some(mismatched_bindings.clone()),
        runtime_binary_file: None,
        runtime_topology_file: None,
        runtime_swap_circuit_metadata_file: None,
        runtime_private_egress_circuit_metadata_file: None,
    })
    .expect_err("mismatched local validator binding must fail");
    assert!(
        mismatch.to_string().contains("do not match signed validator binding"),
        "{mismatch}"
    );
    let runtime_binding_mismatch = deployment_runtime_identity_from_config(
        Some(manifest_file.clone().into_os_string()),
        Some("validator-0".into()),
        Some(mismatched_bindings.into_os_string()),
        Some(binary.clone().into_os_string()),
        Some(topology.clone().into_os_string()),
        Some(swap_metadata.clone().into_os_string()),
        Some(egress_metadata.clone().into_os_string()),
    )
    .expect_err("runtime status identity must fail on changed service artifacts");
    assert!(
        runtime_binding_mismatch
            .to_string()
            .contains("service artifacts do not match manifest"),
        "{runtime_binding_mismatch}"
    );
    let missing_transport_bindings = root.join("missing-transport-bindings.json");
    atomic_write(
        &missing_transport_bindings,
        format!(
            "{}\n",
            serde_json::json!({
                "schema": DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA,
                "validators": [{
                    "validator_id": "validator-0",
                    "services": [{
                        "service_id": "rpc",
                        "service_unit_file": unit.to_string_lossy(),
                        "environment_file": environment.to_string_lossy()
                    }]
                }]
            })
        ),
    )
    .expect("write missing transport bindings");
    let missing_transport = read_deployment_validator_bindings_file(
        &missing_transport_bindings,
    )
    .expect_err("missing transport binding must fail");
    assert!(
        missing_transport.to_string().contains("rpc and transport"),
        "{missing_transport}"
    );
    let duplicate_validator_bindings = root.join("duplicate-validator-bindings.json");
    atomic_write(
        &duplicate_validator_bindings,
        format!(
            "{}\n",
            serde_json::json!({
                "schema": DEPLOYMENT_VALIDATOR_BINDINGS_SCHEMA,
                "validators": [{
                    "validator_id": "validator-0",
                    "services": [{
                        "service_id": "rpc",
                        "service_unit_file": unit.to_string_lossy(),
                        "environment_file": environment.to_string_lossy()
                    }, {
                        "service_id": "transport",
                        "service_unit_file": transport_unit.to_string_lossy(),
                        "environment_file": transport_environment.to_string_lossy()
                    }]
                }, {
                    "validator_id": "validator-0",
                    "services": [{
                        "service_id": "rpc",
                        "service_unit_file": unit.to_string_lossy(),
                        "environment_file": environment.to_string_lossy()
                    }, {
                        "service_id": "transport",
                        "service_unit_file": transport_unit.to_string_lossy(),
                        "environment_file": transport_environment.to_string_lossy()
                    }]
                }]
            })
        ),
    )
    .expect("write duplicate validator bindings");
    let duplicate = read_deployment_validator_bindings_file(&duplicate_validator_bindings)
        .expect_err("duplicate validator binding must fail");
    assert!(
        duplicate.to_string().contains("not strictly sorted"),
        "{duplicate}"
    );
    let expired = verify_deployment_manifest(DeploymentManifestVerifyOptions {
        manifest_file: manifest_file.clone(),
        trusted_publisher_key_file: trusted_key_file.clone(),
        now_unix: Some(now.saturating_add(61)),
        validator_id: None,
        validator_bindings_file: None,
        runtime_binary_file: None,
        runtime_topology_file: None,
        runtime_swap_circuit_metadata_file: None,
        runtime_private_egress_circuit_metadata_file: None,
    })
    .expect_err("expired deployment manifest must fail");
    assert!(expired.to_string().contains("expired"), "{expired}");

    let mut tampered = manifest;
    tampered.binary_sha256 = "00".repeat(32);
    atomic_write(
        &manifest_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&tampered).expect("tampered deployment json")
        ),
    )
    .expect("write tampered deployment manifest");
    let error = verify_deployment_manifest(DeploymentManifestVerifyOptions {
        manifest_file,
        trusted_publisher_key_file: trusted_key_file,
        now_unix: Some(now),
        validator_id: None,
        validator_bindings_file: None,
        runtime_binary_file: None,
        runtime_topology_file: None,
        runtime_swap_circuit_metadata_file: None,
        runtime_private_egress_circuit_metadata_file: None,
    })
    .expect_err("tampered deployment manifest must fail");
    assert!(error.to_string().contains("signature verification"), "{error}");
    std::fs::remove_dir_all(root).expect("cleanup deployment manifest test");
}

#[test]
fn deployment_validator_unit_stage_is_canonical_and_non_overwriting() {
    let root = unique_test_dir("postfiat-deployment-unit-stage");
    std::fs::create_dir_all(&root).expect("create deployment stage test root");
    let topology_file = root.join("topology.json");
    let peers = (0_u16..6)
        .map(|index| {
            serde_json::json!({
                "node_id": format!("validator-{index}"),
                "host": format!("10.0.0.{}", index + 1),
                "p2p_port": 26_650_u16 + index * 2,
                "rpc_port": 27_650_u16 + index,
                "p2p_address": format!("/ip4/10.0.0.{}/tcp/{}", index + 1, 26_650_u16 + index * 2),
            })
        })
        .collect::<Vec<_>>();
    atomic_write(
        &topology_file,
        format!(
            "{}\n",
            serde_json::json!({
                "topology_id": "stage-test-topology",
                "chain_id": "postfiat-stage-test",
                "genesis_hash": "11".repeat(48),
                "protocol_version": 1,
                "peers": peers,
            })
        ),
    )
    .expect("write deployment stage topology");
    let binary_file = root.join("postfiat-node");
    let swap_metadata = root.join("swap.metadata.json");
    let private_egress_metadata = root.join("private-egress.metadata.json");
    atomic_write(&binary_file, b"release-binary").expect("write staged binary input");
    atomic_write(&swap_metadata, b"swap-metadata").expect("write swap metadata input");
    atomic_write(&private_egress_metadata, b"private-egress-metadata")
        .expect("write private-egress metadata input");
    let output_dir = root.join("candidate");
    let options = DeploymentValidatorUnitsStageOptions {
        release_id: "release-test-1".to_string(),
        topology_file,
        binary_file,
        swap_circuit_metadata_file: swap_metadata,
        private_egress_circuit_metadata_file: private_egress_metadata,
        output_dir: output_dir.clone(),
    };
    let report = stage_deployment_validator_units(options.clone())
        .expect("stage canonical validator units");
    assert_eq!(report.schema, DEPLOYMENT_VALIDATOR_UNIT_STAGE_SCHEMA);
    assert_eq!(report.validators.len(), 6);
    assert!(report.binary_file.is_file());
    assert_eq!(
        std::fs::read(&report.binary_file).expect("read staged binary"),
        b"release-binary"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_eq!(
            std::fs::metadata(&report.binary_file)
                .expect("staged binary metadata")
                .permissions()
                .mode()
                & 0o111,
            0o111,
        );
        for ancestor in [
            report.rootfs_dir.clone(),
            report
                .topology_file
                .parent()
                .expect("staged topology parent")
                .to_path_buf(),
            report
                .binary_file
                .parent()
                .expect("staged binary parent")
                .to_path_buf(),
        ] {
            assert_eq!(
                std::fs::metadata(&ancestor)
                    .expect("staged public directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755,
                "staged release directories must be traversable by the service user"
            );
        }
        for public_artifact in [
            report.topology_file.clone(),
            report.swap_circuit_metadata_file.clone(),
            report.private_egress_circuit_metadata_file.clone(),
        ] {
            assert_eq!(
                std::fs::metadata(&public_artifact)
                    .expect("staged public artifact metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o644,
                "staged public artifacts must be readable by the service user"
            );
        }
    }
    let signing_bindings =
        read_deployment_validator_bindings_file(&report.signing_bindings_file)
            .expect("read canonical signing bindings");
    assert_eq!(signing_bindings.len(), 6);
    for row in &report.validators {
        let rpc_unit = std::fs::read_to_string(&row.rpc_unit_file)
            .expect("read staged RPC unit");
        let transport_unit = std::fs::read_to_string(&row.transport_unit_file)
            .expect("read staged transport unit");
        let transport_environment =
            std::fs::read_to_string(&row.transport_environment_file)
                .expect("read staged transport environment");
        assert!(rpc_unit.contains("--spool-dir"));
        assert!(rpc_unit.contains("--ready-file"));
        assert!(rpc_unit.contains("--bind-host 127.0.0.1"));
        assert!(rpc_unit.contains("--max-requests 10000"));
        assert!(rpc_unit.contains("--unsafe-devnet-json-storage"));
        assert!(rpc_unit.contains("--allow-mempool-submit-finality"));
        assert!(!rpc_unit.contains("--allow-mempool-submit --"));
        assert!(!rpc_unit.contains("0.0.0.0"));
        assert!(!rpc_unit.contains("1000000"));
        assert!(rpc_unit.contains("--runtime-binary-file"));
        assert!(rpc_unit.contains("--runtime-topology-file"));
        assert_eq!(rpc_unit.matches("ExecStartPre=").count(), 3);
        assert!(rpc_unit.contains(
            "ExecStartPre=+/usr/bin/install -d -o postfiat -g postfiat -m 0700 /var/lib/postfiat/validator-"
        ));
        assert!(rpc_unit.contains("/finality-artifacts"));
        assert!(transport_unit.contains("--runtime-swap-circuit-metadata-file"));
        assert!(transport_unit.contains("--bind-host 10.0.0."));
        assert!(transport_unit.contains("--max-connections 10000"));
        assert!(transport_unit.contains("--unsafe-devnet-file-signer"));
        assert!(transport_unit.contains("--unsafe-devnet-json-storage"));
        assert!(!transport_unit.contains("1000000"));
        assert!(transport_unit
            .contains("--runtime-private-egress-circuit-metadata-file"));
        assert_eq!(transport_unit.matches("ExecStartPre=").count(), 2);
        for unit in [&rpc_unit, &transport_unit] {
            for directive in [
                "NoNewPrivileges=true",
                "ProtectSystem=strict",
                "ProtectHome=true",
                "ProtectControlGroups=true",
                "ProtectKernelTunables=true",
                "ProtectKernelModules=true",
                "ProtectKernelLogs=true",
                "RestrictSUIDSGID=true",
                "LockPersonality=true",
                "RestrictRealtime=true",
                "SystemCallArchitectures=native",
                "CapabilityBoundingSet=",
                "AmbientCapabilities=",
                "LimitNOFILE=65536",
                "LimitCORE=0",
                "TasksMax=1024",
                "UMask=0077",
            ] {
                assert!(unit.contains(directive), "missing {directive}");
            }
        }
        assert!(transport_environment
            .contains("POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=1"));
        assert!(transport_environment
            .contains("POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1"));
        assert!(!transport_environment.contains("PRIVATE_EGRESS_VERIFIER=0"));
        assert!(!transport_environment.contains("POSTFIAT_ALLOW_PUBLIC_TRANSPORT_BIND"));
        let runtime_bindings: DeploymentValidatorBindingsFile = read_json_file(
            &row.runtime_bindings_file,
            "staged runtime bindings",
        )
        .expect("read staged runtime binding file");
        assert_eq!(runtime_bindings.validators.len(), 1);
        assert_eq!(runtime_bindings.validators[0].validator_id, row.validator_id);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            for public_artifact in [
                &row.rpc_unit_file,
                &row.rpc_environment_file,
                &row.transport_unit_file,
                &row.transport_environment_file,
                &row.runtime_bindings_file,
            ] {
                assert_eq!(
                    std::fs::metadata(public_artifact)
                        .expect("staged service artifact metadata")
                        .permissions()
                        .mode()
                        & 0o777,
                    0o644,
                    "staged service artifacts must be readable by the service user"
                );
            }
        }
    }
    let overwrite = stage_deployment_validator_units(options)
        .expect_err("deployment stage must not overwrite an existing release");
    assert_eq!(overwrite.kind(), io::ErrorKind::AlreadyExists);
    std::fs::remove_dir_all(root).expect("cleanup deployment unit stage test");
}

fn copy_dir_all(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

#[test]
fn historical_external_certificate_applies_via_catch_up_replay_path() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let seed_dir = testdata.join("wan-devnet-height-2-seed");
    let catchup_fixtures = testdata.join("wan-devnet-catchup-block-3");
    assert!(
        seed_dir.is_dir(),
        "missing height-2 seed workdir at {}",
        seed_dir.display()
    );
    assert!(
        catchup_fixtures.is_dir(),
        "missing block-3 catch-up fixtures at {}",
        catchup_fixtures.display()
    );

    let data_dir = unique_test_dir("postfiat-historical-cert-catchup-replay");
    copy_dir_all(&seed_dir, &data_dir).expect("seed height-2 workdir");
    let batch_file = catchup_fixtures.join("batch.json");
    let certificate_file = catchup_fixtures.join("block-certificate.json");
    let replay_block_file = catchup_fixtures.join("block.json");
    let expected_block: BlockRecord =
        read_json_file(&replay_block_file, "historical replay block").expect("read block");

    apply_batch_with_replay(
        ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: batch_file.clone(),
            certificate_file: Some(certificate_file),
        },
        Some(replay_block_file),
    )
    .expect("apply historical external certificate with replay evidence");

    let status_after = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status after historical apply");
    assert_eq!(status_after.block_height, expected_block.header.height);
    assert_eq!(status_after.state_root, expected_block.header.state_root);

    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks after historical apply");
    let block = block_log
        .iter()
        .find(|block| block.header.height == expected_block.header.height)
        .expect("committed historical block");
    assert_eq!(block.header.proposer, expected_block.header.proposer);
    assert_eq!(block.receipt_ids, expected_block.receipt_ids);
    assert_eq!(block.header.block_hash, expected_block.header.block_hash);

    let verification = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    });
    if let Err(error) = verification {
        let error = error.to_string();
        assert!(
            error.contains("does not match current state root")
                && error.contains("or replay state root"),
            "{error}"
        );
    }

    fs::remove_dir_all(&data_dir).expect("cleanup historical catch-up replay test");
}

#[test]
fn historical_external_certificate_rejects_state_divergent_catch_up_without_mutation() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let seed_dir = testdata.join("wan-devnet-height-2-seed");
    let catchup_fixtures = testdata.join("wan-devnet-catchup-block-3");
    let data_dir = unique_test_dir("postfiat-historical-cert-state-divergence");
    copy_dir_all(&seed_dir, &data_dir).expect("seed height-2 workdir");

    let store = NodeStore::new(&data_dir);
    let mut divergent_ledger = store.read_ledger().expect("read seed ledger");
    divergent_ledger.accounts[0].balance = divergent_ledger.accounts[0]
        .balance
        .checked_add(1)
        .expect("fixture balance increment");
    store
        .write_ledger(&divergent_ledger)
        .expect("persist divergent pre-replay ledger");
    let ledger_before = store.read_ledger().expect("read divergent ledger");
    let tip_before = store.read_chain_tip().expect("read pre-replay tip");
    let blocks_before = store.read_blocks().expect("read pre-replay blocks");

    let error = apply_batch_with_replay(
        ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: catchup_fixtures.join("batch.json"),
            certificate_file: Some(catchup_fixtures.join("block-certificate.json")),
        },
        Some(catchup_fixtures.join("block.json")),
    )
    .expect_err("historical certificate must not bless state-divergent replay");
    assert!(
        error
            .to_string()
            .contains("historical replay state root mismatch"),
        "{error}"
    );
    assert_eq!(store.read_ledger().expect("ledger after rejection"), ledger_before);
    assert_eq!(store.read_chain_tip().expect("tip after rejection"), tip_before);
    assert_eq!(store.read_blocks().expect("blocks after rejection"), blocks_before);

    fs::remove_dir_all(&data_dir).expect("cleanup state-divergent replay test");
}

#[test]
fn historical_external_certificate_rejects_wrong_local_parent_without_mutation() {
    let testdata = Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata");
    let seed_dir = testdata.join("wan-devnet-height-2-seed");
    let catchup_fixtures = testdata.join("wan-devnet-catchup-block-3");
    let data_dir = unique_test_dir("postfiat-historical-cert-parent-divergence");
    copy_dir_all(&seed_dir, &data_dir).expect("seed height-2 workdir");

    let store = NodeStore::new(&data_dir);
    let mut divergent_tip = store.read_chain_tip().expect("read seed tip");
    divergent_tip.block_hash = "ab".repeat(48);
    store
        .write_chain_tip(&divergent_tip)
        .expect("persist divergent pre-replay tip");
    let ledger_before = store.read_ledger().expect("read pre-replay ledger");
    let blocks_before = store.read_blocks().expect("read pre-replay blocks");

    let error = apply_batch_with_replay(
        ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: catchup_fixtures.join("batch.json"),
            certificate_file: Some(catchup_fixtures.join("block-certificate.json")),
        },
        Some(catchup_fixtures.join("block.json")),
    )
    .expect_err("historical certificate must not attach to a different local parent");
    assert!(
        error.to_string().contains("historical replay parent mismatch"),
        "{error}"
    );
    assert_eq!(store.read_ledger().expect("ledger after rejection"), ledger_before);
    assert_eq!(store.read_chain_tip().expect("tip after rejection"), divergent_tip);
    assert_eq!(store.read_blocks().expect("blocks after rejection"), blocks_before);

    fs::remove_dir_all(&data_dir).expect("cleanup parent-divergent replay test");
}
